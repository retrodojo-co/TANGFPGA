# TangFPGA Flasher 0.1.1

A TangFPGA-branded desktop fork of NANOBLAST Flasher 0.2.0, with a dedicated
app identity (`co.retrodojo.tangfpga.flasher`), black/red interface with the supplied TangFPGA logo and native icons.
The original NANOBLAST app, backups and preferences are independent.

## Supported firmware and connection

This version preserves the upstream **20K BL616 UPDATE / NBCRXZ01** updater.
It accepts a compatible downloaded release ZIP, extracted release folder or
JSON manifest containing a menu and the four matching NES, SMS/GG, GB/GBC and
experimental SNES cores. It checks file lengths, checksums, core headers and
flash layout before writing, saves a full backup, and verifies readback.

The TangFPGA name does not add Nano 9K, Console 60K, direct JTAG, raw `.fs`
or alternate Arcade flash-layout support. Use the exact firmware package and
connection procedure specified for your hardware. Unsupported packages are
rejected. No new firmware is bundled for automatic installation.

1. Download a firmware release that explicitly supports this updater/layout.
2. Connect the supported console in BL616 UPDATE mode as described by its
   hardware instructions. Select the correct port if several are present.
3. Choose the release and click **Flash firmware & cores**.
4. Keep power connected throughout backup, writing and verification.
5. After success, disconnect every USB/power cable for five seconds and reconnect.

Backups and verified-layout receipts live in the app data folder. On macOS:
`~/Library/Application Support/co.retrodojo.tangfpga.flasher/`; on Windows:
`%APPDATA%\co.retrodojo.tangfpga.flasher\`. Existing NANOBLAST receipts are not
imported, so an unrecognized installed layout may require separate recovery.
Keep any existing NANOBLAST backups.

## Build

Use Node 22, Rust stable, and Python 3.11 (64-bit) for helper generation.
On Windows install Visual Studio 2022 Build Tools with Desktop development
with C++, the Windows SDK, and Rust's MSVC toolchain. Then run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build_windows.ps1
```

The result is a per-user x64 NSIS installer under
`src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`. WebView2's
bootstrapper is embedded; its initial runtime installation may need Internet.
Run `python scripts/collect_release.py --target x86_64-pc-windows-msvc` to
collect the installer and checksum.

On an Apple Silicon Mac:

```sh
python3.11 -m venv .build-venv
.build-venv/bin/python -m pip install -r scripts/requirements-bl616.txt
.build-venv/bin/python scripts/stage_bl616.py
npm ci
npm run check
cargo test --locked --manifest-path src-tauri/Cargo.toml --features desktop
npm run tauri build -- --bundles app,dmg
python3 scripts/collect_release.py
```

The GitHub workflow at `../.github/workflows/flasher-release.yml` builds Mac
Apple Silicon and Windows x64 installers on native runners. Trigger it manually
or with a `flasher-v*` tag. It uploads build artifacts without publishing a release.
A source ZIP/build kit is not a Windows installer. See [release status](docs/release-status.md)
for the actual outputs verified in this workspace.

Mac builds are ad-hoc signed and not notarized. Windows builds are unsigned.
Hardware flashing and Windows install/uninstall testing remain to be performed.
Intel Mac and Windows ARM64 builds are not included in this first version.

`npm run dev` opens an isolated browser simulation with connected, offline and
failure modes. It never accesses real files or devices. Regenerate icons with
`npm run tauri -- icon src-tauri/icons/tangfpga.svg`.

## Attribution

Derived from the MIT-licensed NANOBLAST setup utility (source workspace commit
`a5c0fe94c3cdcba79eaeb441fb0c3b12fead342f`, including local Flasher 0.2.0 updates).
See [LICENSE](LICENSE). Internal upstream filenames and wire-format identifiers
are retained for compatibility. The unmodified audited firmware assets are
validation/recovery reference data, with their original GPL/per-file notices
and corresponding source archive under `src-tauri/resources/nanoblast-v0.2.0/`.
They are not installed without a manually selected release. Bundled Python
helper dependencies retain their license inventory under resources/bl616.
