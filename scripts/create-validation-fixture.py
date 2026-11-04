#!/usr/bin/env python3
"""Create a fresh disposable known tree, then independently count lstat metadata."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import shutil
import stat
import tempfile
import time

repo = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--files', type=int, default=100_000)
parser.add_argument('--manifest', type=Path, required=True)
args = parser.parse_args()
if not 1 <= args.files <= 1_000_000:
    parser.error('--files must be between 1 and 1000000')
if shutil.disk_usage(repo).free < args.files * 8192 + 256 * 1024**2:
    parser.error('insufficient free space for disposable fixture and metadata reserve')
parent = repo / 'target' / 'validation-fixtures'
parent.mkdir(parents=True, exist_ok=True)
root = Path(tempfile.mkdtemp(prefix='known-', dir=parent)).resolve()
start = time.monotonic()
for i in range(args.files):
    folder = root / f'group-{i // 1000:04}'
    folder.mkdir(exist_ok=True)
    extension = ('txt', 'mov', 'rs')[i % 3]
    (folder / f'item-{i:07}.{extension}').write_bytes(b'x' * (i % 73))
(root / 'Prázdná složka').mkdir()
long_folder = root / ('Dlouhá cesta – projekty a archiv ' + 'x' * 80)
long_folder.mkdir()
(long_folder / 'prázdný.txt').touch()
(root / '.hidden').write_bytes(b'abc')
with (root / 'sparse.bin').open('xb') as output:
    output.truncate(512 * 1024**2)
package = root / 'Sample.app' / 'Contents'
package.mkdir(parents=True)
(package / 'Info.plist').write_bytes(plistlib.dumps({'CFBundleName': 'Sample', 'CFBundlePackageType': 'APPL'}))
os.link(root / 'group-0000' / 'item-0000001.mov' if args.files > 1 else root / 'group-0000' / 'item-0000000.txt', root / 'hard-link.txt')
os.symlink('.', root / 'cycle-link')
# Only this freshly created fixture is enumerated. No content reads or following links.
seen = set()
expected = dict(nodes=1, files=0, logical=0, allocated=0)
for current, dirs, files in os.walk(root, followlinks=False):
    for name in dirs + files:
        metadata = os.lstat(Path(current) / name)
        expected['nodes'] += 1
        if stat.S_ISREG(metadata.st_mode):
            expected['files'] += 1
            key = metadata.st_dev, metadata.st_ino
            if key in seen:
                continue
            seen.add(key)
        elif not stat.S_ISLNK(metadata.st_mode):
            continue
        expected['logical'] += metadata.st_size
        expected['allocated'] += metadata.st_blocks * 512
args.manifest.parent.mkdir(parents=True, exist_ok=True)
args.manifest.write_text(json.dumps(dict(root=str(root), requested_files=args.files,
    creation_and_manifest_ms=(time.monotonic() - start) * 1000, expected=expected), indent=2) + '\n')
print(root, flush=True)
