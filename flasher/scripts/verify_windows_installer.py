"""Check Windows packaging outputs without accessing a board or flashing it."""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]


def require_x64_pe(path):
    data = path.read_bytes()
    assert data[:2] == b'MZ', f'Not a Windows executable: {path}'
    pe = struct.unpack_from('<I', data, 0x3C)[0]
    assert data[pe:pe + 4] == b'PE\0\0', f'Invalid PE header: {path}'
    assert struct.unpack_from('<H', data, pe + 4)[0] == 0x8664, f'Not x64: {path}'


def main():
    resources = ROOT / 'src-tauri/resources/bl616'
    helper = resources / 'nanoblast-bl616/nanoblast-bl616.exe'
    require_x64_pe(helper)
    digest = hashlib.sha256(helper.read_bytes()).hexdigest()
    assert digest == (resources / 'sha256.txt').read_text().strip(), 'Helper hash mismatch'
    assert (resources / 'dependencies.json').is_file(), 'Missing helper dependency inventory'
    assert any((resources / 'licenses').rglob('*')), 'Missing helper licenses'
    output = ROOT / 'src-tauri/target/x86_64-pc-windows-msvc/release'
    require_x64_pe(output / 'tangfpga-flasher.exe')
    installers = list((output / 'bundle/nsis').glob('*-setup.exe'))
    assert len(installers) == 1, f'Expected one NSIS installer, found {len(installers)}'
    # The NSIS launcher can be x86 even when its app payload is x64.
    assert installers[0].read_bytes()[:2] == b'MZ', 'Invalid NSIS launcher'
    installer = installers[0]
    checksum = hashlib.sha256(installer.read_bytes()).hexdigest()
    installer.with_suffix('.exe.sha256').write_text(f'{checksum}  {installer.name}\n')
    (installer.parent / 'validation.json').write_text(json.dumps({
        'installer': installer.name,
        'sha256': checksum,
        'app_and_helper_windows_x64': True,
        'helper_hash_verified': True,
        'signed': False,
        'hardware_tested': False,
        'installer_ui_tested': False,
    }, indent=2) + '\n')
    print(f'Windows installer packaged: {installer}')
    print('Installation, uninstall and physical flashing still require Windows testing.')


if __name__ == '__main__':
    main()
