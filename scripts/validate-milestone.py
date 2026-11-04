#!/usr/bin/env python3
"""Measure the public scan service and native UI separately on a disposable fixture."""
import argparse
import csv
import hashlib
from datetime import datetime
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile
import time
from zoneinfo import ZoneInfo

REPO = Path(__file__).resolve().parents[1]
BINARY = REPO / 'target/release/bytescape'
BUDGET_MS = 1000 / 60


def environment():
    commands = [('cpu', ['/usr/sbin/sysctl', '-n', 'machdep.cpu.brand_string']),
                ('memory_bytes', ['/usr/sbin/sysctl', '-n', 'hw.memsize']),
                ('uptime', ['/usr/bin/uptime']),
                ('pressure', ['/usr/bin/top', '-l', '1', '-n', '0'])]
    return {key: subprocess.check_output(command, text=True).strip() for key, command in commands}


def stats(values):
    if not values:
        return None
    ordered = sorted(values)
    return dict(samples=len(values), mean_ms=sum(values)/len(values),
                p95_ms=ordered[math.ceil(len(values)*.95)-1], max_ms=ordered[-1],
                over_60fps_budget=sum(v > BUDGET_MS for v in values),
                mean_and_p95_within_budget=sum(values)/len(values) <= BUDGET_MS
                and ordered[math.ceil(len(values)*.95)-1] <= BUDGET_MS)


def execute(command, log, env=None):
    start = time.monotonic()
    with log.open('w') as output:
        result = subprocess.run(command, cwd=REPO, env=env, stdout=output,
                                stderr=subprocess.STDOUT, timeout=180)
    if result.returncode:
        raise RuntimeError(f'{command[1:]} exited {result.returncode}; see {log}')
    return (time.monotonic()-start)*1000


def service_case(folder, name, root, extra, expected=None):
    before = environment()
    log = folder / f'{name}.log'
    wall = execute([str(BINARY), '--scan-benchmark', str(root), *extra], log)
    result = dict(line.split('=', 1) for line in log.read_text().splitlines() if '=' in line)
    for key in list(result):
        try:
            result[key] = float(result[key]) if '.' in result[key] else int(result[key])
        except ValueError:
            pass
    if expected:
        assert result['state'] == 'Complete', result
        for key in ('files', 'nodes', 'logical', 'allocated'):
            assert result[key] == expected[key], (name, key, result[key], expected[key])
    else:
        assert result['state'] == 'Cancelled', result
        assert isinstance(result['cancel_observed_ms'], (int, float))
        assert result['cancel_observed_ms'] < 1000, result
    return dict(environment=before, process_wall_ms=wall, measurement=result)


def native_case(folder, name, args, expected=None):
    report = folder / f'{name}.json'
    frames = folder / f'{name}-frames.csv'
    before = environment()
    env = dict(os.environ, BYTESCAPE_PROFILE_TO=str(report), BYTESCAPE_PROFILE_FRAMES_TO=str(frames))
    wall = execute([str(BINARY), '--profile', *args], folder / f'{name}.log', env)
    result = json.loads(report.read_text())
    phases = dict(scan=[], completed=[])
    with frames.open() as samples:
        for row in csv.DictReader(samples):
            value = float(row['interval_ms'])
            assert math.isfinite(value) and value >= 0
            phases[row['phase']].append(value)
    summaries = {phase: stats(values) for phase, values in phases.items()}
    assert len(phases['completed']) == result['frames'] == 600
    assert len(phases['scan']) == result['during_scan_frames']
    assert abs(summaries['completed']['mean_ms'] - result['frame_interval_mean_ms']) < .002
    assert abs(summaries['completed']['p95_ms'] - result['frame_interval_p95_ms']) < .002
    if expected:
        assert result['source'] == 'filesystem' and result['scan_state'] == 'Complete', result
        for key in ('files', 'nodes', 'logical', 'allocated'):
            assert result[key] == expected[key], (name, key, result[key], expected[key])
    elif name == 'native-cancel':
        assert result['scan_state'] == 'Cancelled' and result['source'] == 'filesystem', result
        assert result['cancel_status_drawn_ms'] is not None and result['cancel_status_drawn_ms'] < 1000, result
    else:
        assert result['source'] == 'synthetic' and result['files'] == 1_000_000, result
    return dict(environment=before, process_wall_ms=wall, measurement=result, raw_frame_summaries=summaries)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--output', type=Path, default=REPO / 'target/validation-reports')
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    root = Path(manifest['root']).resolve()
    assert root.parent == (REPO / 'target/validation-fixtures').resolve(), 'use only generated disposable fixtures'
    args.output.mkdir(parents=True, exist_ok=True)
    folder = Path(tempfile.mkdtemp(prefix='run-', dir=args.output)).resolve()
    (args.output / 'current-run.txt').write_text(str(folder)+'\n')
    report = dict(timestamp=datetime.now(ZoneInfo('Europe/Prague')).isoformat(),
                  executable_sha256=hashlib.sha256(BINARY.read_bytes()).hexdigest(),
                  fixture=manifest, frame_budget_ms=BUDGET_MS,
                  limitations='UI frame-start intervals are not presented-frame or GPU timestamp measurements', cases={})
    cases = report['cases']
    actions = [
        ('scan-fast', lambda: service_case(folder, 'scan-fast', root, [], manifest['expected'])),
        ('scan-slow', lambda: service_case(folder, 'scan-slow', root, ['--poll-ms', '3000'], manifest['expected'])),
        ('scan-cancel', lambda: service_case(folder, 'scan-cancel', root, ['--cancel-after-ms', '100'])),
        ('synthetic-million', lambda: native_case(folder, 'synthetic-million', ['--million'])),
        ('native-real', lambda: native_case(folder, 'native-real', ['--scan', str(root)], manifest['expected'])),
        ('native-cancel', lambda: native_case(folder, 'native-cancel', ['--scan', str(root), '--profile-cancel-after-ms', '100'])),
    ]
    # A real empty root also exercises profiling navigation without invented node IDs.
    empty = Path(tempfile.mkdtemp(prefix='empty-', dir=root.parent)).resolve()
    actions.append(('native-empty', lambda: native_case(folder, 'native-empty', ['--scan', str(empty)],
                    dict(files=0, nodes=1, logical=0, allocated=0))))
    for name, action in actions:
        print('Starting '+name, flush=True)
        cases[name] = action()
        (folder / 'summary.json').write_text(json.dumps(report, indent=2)+'\n')
        print('Completed '+name, flush=True)
    print('Evidence: '+str(folder), flush=True)


if __name__ == '__main__':
    main()
