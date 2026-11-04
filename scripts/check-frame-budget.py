#!/usr/bin/env python3
"""Check measured mean/p95 against 1000/60 ms; this is not presented-frame proof."""
import csv
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
parent = root / 'target/validation-reports'
parent.mkdir(parents=True, exist_ok=True)
folder = Path(tempfile.mkdtemp(prefix='frame-budget-', dir=parent))
report = folder / 'profile.json'
frames = folder / 'frames.csv'
env = dict(os.environ, BYTESCAPE_PROFILE_TO=str(report), BYTESCAPE_PROFILE_FRAMES_TO=str(frames))
with (folder / 'run.log').open('w') as log:
    result = subprocess.run([str(root / 'target/release/bytescape'), '--million', '--profile'],
                            cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=180)
if result.returncode:
    raise SystemExit(result.returncode)
data = json.loads(report.read_text())
assert data['source'] == 'synthetic' and data['files'] == 1_000_000 and data['frames'] == 600
budget = 1000 / 60
with frames.open() as samples:
    intervals = sorted(float(row['interval_ms']) for row in csv.DictReader(samples) if row['phase'] == 'completed')
assert len(intervals) == 600 and all(math.isfinite(v) and v >= 0 for v in intervals)
mean = sum(intervals) / len(intervals)
p95 = intervals[math.ceil(len(intervals)*.95)-1]
passed = mean <= budget and p95 <= budget
print(f'Raw mean={mean:.6f} ms; p95={p95:.6f} ms; budget={budget:.6f} ms')
print(('WITHIN' if passed else 'EXCEEDS') + ' mean/p95 60 FPS interval budget: ' + json.dumps(data))
print('No GPU timestamps or presented-frame proof. Evidence: ' + str(folder))
raise SystemExit(0 if passed else 1)
