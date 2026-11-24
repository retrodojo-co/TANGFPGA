#!/usr/bin/env python3
"""Stage a trusted, locally built openFPGALoader and its non-system runtime libraries.
Run on the target platform. Never downloads or executes firmware/programming commands.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

def run(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT).strip()

def dependencies(binary):
    if sys.platform == 'darwin':
        return [line.strip().split(' (')[0] for line in run('otool', '-L', str(binary)).splitlines()[1:]]
    if sys.platform == 'win32':
        names = re.findall(r'DLL Name:\s*(\S+)', run('objdump', '-p', str(binary)))
        paths = []
        for name in names:
            found = next((Path(p) / name for p in os.environ['PATH'].split(os.pathsep) if (Path(p) / name).is_file()), None)
            if found is None and (Path(os.environ['WINDIR']) / 'System32' / name).exists():
                continue
            if found is None:
                raise RuntimeError(f'Missing runtime dependency: {name}')
            if str(found).lower().startswith(os.environ['WINDIR'].lower()):
                continue
            paths.append(str(found))
        return paths
    output = run('ldd', str(binary))
    if 'not found' in output:
        raise RuntimeError(output)
    return re.findall(r'=>\s+(/\S+)', output)

def system_library(path):
    if sys.platform == 'darwin':
        return path.startswith(('/usr/lib/', '/System/'))
    return bool(re.match(r'^(libc\.|libm\.|libpthread\.|libdl\.|librt\.|ld-linux)', Path(path).name))

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('executable', type=Path)
    parser.add_argument('--license', type=Path, required=True, help='Upstream openFPGALoader LICENSE')
    parser.add_argument('--notices', type=Path, action='append', default=[], help='Dependency license file (repeat)')
    args = parser.parse_args()
    source = args.executable.resolve(strict=True)
    version = run(str(source), '-V')
    if 'openFPGALoader' not in version:
        raise RuntimeError('Unexpected executable')
    help_text = run(str(source), '--help')
    for flag in ['--busdev-num', '--detect', '--write-flash', '--write-sram', '--verify', '--dump-flash', '--file-size', '--external-flash']:
        if flag not in help_text:
            raise RuntimeError(f'Missing option: {flag}')
    if 'tangnano20k' not in run(str(source), '--list-boards'):
        raise RuntimeError('Tang Nano 20K not supported')
    dest = ROOT / 'src-tauri/resources/programmer'
    dest.mkdir(parents=True, exist_ok=True)
    name = 'openFPGALoader.exe' if sys.platform == 'win32' else 'openFPGALoader'
    target = dest / name
    shutil.copy2(source, target)
    target.chmod(target.stat().st_mode | 0o200)
    pending = [(source, target)]
    copied = {source: target}
    while pending:
        original, staged = pending.pop()
        for dependency in dependencies(original):
            if system_library(dependency):
                continue
            if not Path(dependency).is_absolute():
                # Built-from-source binaries should link concrete paths; refuse unresolved rpaths.
                raise RuntimeError(f'Unresolved library path: {dependency}')
            dep = Path(dependency).resolve(strict=True)
            output = dest / Path(dependency).name
            if dep not in copied:
                if output.exists() and output not in copied.values():
                    raise RuntimeError(f'Library name collision or stale staging directory: {output}')
                shutil.copy2(dep, output)
                output.chmod(output.stat().st_mode | 0o200)
                copied[dep] = output
                pending.append((dep, output))
            if sys.platform == 'darwin':
                run('install_name_tool', '-change', dependency, '@loader_path/' + output.name, str(staged))
        if sys.platform == 'darwin' and staged.suffix == '.dylib':
            run('install_name_tool', '-id', '@loader_path/' + staged.name, str(staged))
    if sys.platform == 'darwin':
        for file in copied.values():
            run('codesign', '--force', '--sign', '-', str(file))
    licenses = dest / 'licenses'
    licenses.mkdir(exist_ok=True)
    shutil.copy2(args.license, licenses / 'openFPGALoader-LICENSE')
    for index, notice in enumerate(args.notices):
        shutil.copy2(notice, licenses / f'{index}-{notice.parent.name}-{notice.name}')
    license_roots = ([Path('/opt/homebrew/opt/hidapi'), Path('/opt/homebrew/opt/libusb'), Path('/opt/homebrew/opt/libftdi')]
                     if sys.platform == 'darwin' else [Path('C:/msys64/mingw64/share/licenses')])
    for license_root in license_roots:
        if not license_root.exists(): continue
        candidates = license_root.rglob('*') if sys.platform == 'win32' else license_root.glob('*')
        for notice in candidates:
            if notice.is_file() and any(term in notice.name.lower() for term in ('license','copying','copyright')):
                relative = notice.relative_to(license_root)
                output = licenses / license_root.name / relative
                output.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(notice, output)
    (dest / 'sha256.txt').write_text(hashlib.sha256(target.read_bytes()).hexdigest() + '\n')
    (dest / 'provenance.json').write_text(json.dumps({'version': version, 'platform': sys.platform, 'libraries': [p.name for p in copied.values()], 'upstream': 'https://github.com/trabucayre/openFPGALoader', 'license_review_required': True}, indent=2))
    env = os.environ.copy()
    if sys.platform.startswith('linux'):
        env['LD_LIBRARY_PATH'] = str(dest)
    subprocess.run([str(target), '-V'], env=env, check=True)
    print(f'Staged {version} and {len(copied)-1} libraries. Review licenses before redistribution.')

if __name__ == '__main__':
    main()
