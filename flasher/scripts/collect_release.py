"""Collect actual native installers with SHA-256; never substitutes a build kit."""
import argparse
import hashlib
from pathlib import Path
import shutil

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument('--target', default='')
args = parser.parse_args()
build = root / 'src-tauri/target'
if args.target:
    build /= args.target
bundle = build / 'release/bundle'
windows = 'windows' in args.target
files = list((bundle / ('nsis' if windows else 'dmg')).glob('*.exe' if windows else '*.dmg'))
if len(files) != 1:
    raise SystemExit(f'Expected one native installer under {bundle}, found {len(files)}')
out = root / 'artifacts/release'
out.mkdir(parents=True, exist_ok=True)
for source in files:
    name = f'TangFPGA-Flasher-0.1.0-{ "windows-x64-setup.exe" if windows else "mac-arm64.dmg" }'
    dest = out / name
    shutil.copy2(source, dest)
    digest = hashlib.sha256(dest.read_bytes()).hexdigest()
    (out / (name + '.sha256')).write_text(f'{digest}  {name}\n')
    print(dest)

# Keep the upstream MIT notice with every installer distribution.
shutil.copy2(root / 'LICENSE', out / 'LICENSE.txt')
shutil.copy2(root / 'README.md', out / 'BUILD-AND-USAGE.md')
