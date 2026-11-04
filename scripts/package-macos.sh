#!/bin/zsh
set -euo pipefail
cd -- "${0:A:h:h}"
mkdir -p dist/ByteScape.app/Contents/MacOS
cp target/release/bytescape dist/ByteScape.app/Contents/MacOS/ByteScape
task_version=$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["package"]["version"])')
task_numeric_version="${task_version%%-*}"
cat > dist/ByteScape.app/Contents/Info.plist <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>ByteScape</string>
<key>CFBundleIdentifier</key><string>local.bytescape.prototype</string>
<key>CFBundleName</key><string>ByteScape</string>
<key>CFBundleDisplayName</key><string>ByteScape</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>${task_numeric_version}</string>
<key>CFBundleVersion</key><string>1</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --sign - dist/ByteScape.app
# A local, ad-hoc signed build; the manifest ties it to the exact source inputs.
python3 - <<'PY'
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib
root = Path.cwd()
inputs = sorted([Path('Cargo.toml'), Path('Cargo.lock'),
                 *Path('src').rglob('*.rs'), *Path('src').rglob('*.wgsl'),
                 *[p for p in Path('assets').rglob('*') if p.is_file()]])
digest = hashlib.sha256()
for path in inputs:
    digest.update(str(path).encode() + b'\0' + path.read_bytes())
binary = Path('dist/ByteScape.app/Contents/MacOS/ByteScape')
manifest = dict(version=tomllib.loads(Path('Cargo.toml').read_text())['package']['version'],
                source_commit=subprocess.check_output(['git','rev-parse','HEAD'], text=True).strip(),
                source_tree_dirty=bool(subprocess.check_output(['git','status','--porcelain','--',
                     'src','Cargo.toml','Cargo.lock','assets'], text=True).strip()),
                source_inputs_sha256=digest.hexdigest(),
                input_executable_sha256=hashlib.sha256(Path('target/release/bytescape').read_bytes()).hexdigest(),
                executable_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                scope='Local prototype, ad-hoc signed; not notarized or publicly distributed')
Path('dist/ByteScape-build.json').write_text(json.dumps(manifest, indent=2) + '\n')
PY
codesign --verify --strict dist/ByteScape.app
