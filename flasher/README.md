# TangFPGA Flasher 0.3.0

A TangFPGA-branded desktop fork of NANOBLAST Flasher 0.2.0, with a dedicated
app identity (`co.retrodojo.tangfpga.flasher`), black/red interface with the supplied TangFPGA logo and native icons.
The original NANOBLAST app, backups and preferences are independent.

## Supported firmware and connection

The app detects two release formats automatically:

| Release | Programming behavior |
| --- | --- |
| Console `manifest.json` / NBCRXZ01 | Existing menu and four matching console cores, with the inherited layout validation |
| Arcade `release.json` / TANGFPGA_ARCADE_V1 | BL616 application only, preserving the primary programmer, FPGA recovery cache and every byte outside `0x40000–0xE8000` |

**Update existing system** retains the conservative update-only checks.
**First install / switch system** supports a factory/empty user-flash Nano 20K,
console → Arcade and Arcade → console. It deliberately replaces the selected
system after confirmation and backs up both flash chips. See the
[first-installation guide](docs/first-installation.md).

The guided setup has three steps: prepare the BL616 programmer in UPDATE mode,
reconnect normally to back up/program/verify the FPGA boot flash, then return to
UPDATE mode to install and verify the controller application and system data.
It installs the pinned, board-specific Sipeed Nano 20K secondary-boot programmer.
The pending installation persists across app restarts between completed steps.

Choose a complete release ZIP, extracted folder, or its exact JSON manifest.
Every required firmware/video file is checked by size and SHA-256, and the BL616
firmware header and payload are validated. A full 4 MiB backup is retained
before any write; the complete flash is read back and compared with the plan.
Unknown applications, invalid cache data and incompatible markers stop before
writing. Separate verified-application receipts permit later Arcade updates.

ZIP selection copies only manifest-declared firmware and video/reference images
into the app's private cache. Game files, source archives and release-supplied
Python scripts are neither extracted nor executed. Files in a private release
stay local. The flasher does not copy SD-card files; prepare the card separately following
the release notes. First-install mode uses the selected package’s CPS1 480p or
NES image to provision the FPGA boot flash.

Nano 9K, Console 60K and arbitrary raw `.fs` flashing remain unsupported.
First-install mode targets Nano 20K / GW2A(R)-18 with Winbond 8 MiB FPGA flash.

1. Download a firmware release that explicitly supports this updater/layout.
2. For Arcade, power off, remove the SD card and disconnect HDMI first.
   Connect the supported console in BL616 UPDATE mode as described by its
   hardware instructions. Select the correct port if several are present.
3. Choose the release. Check whether the app shows a console or Arcade update,
   then click **Flash firmware & cores** or **Flash Arcade application**.
4. Keep power connected throughout backup, writing and verification.
5. After success, disconnect every USB/power cable for five seconds. For Arcade,
   insert the prepared SD card and reconnect video, then power on without UPDATE.

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

On an Apple Silicon Mac, install CMake, libusb, libftdi and hidapi with Homebrew,
then build and stage the pinned programmer before the Tauri build:

```sh
git clone https://github.com/trabucayre/openFPGALoader.git .programmer-source
git -C .programmer-source checkout 3c52ab1487ff759e1a3790346455ec7e92ba8c44
cmake -S .programmer-source -B .programmer-build -DCMAKE_BUILD_TYPE=Release
cmake --build .programmer-build --parallel 3
python3.11 -m venv .build-venv
.build-venv/bin/python -m pip install -r scripts/requirements-bl616.txt
.build-venv/bin/python scripts/stage_programmer.py .programmer-build/openFPGALoader --license .programmer-source/LICENSE
git -C .programmer-source archive --format=tar.gz -o ../src-tauri/resources/programmer/openFPGALoader-source.tar.gz HEAD
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

The bundled Nano 20K programmer firmware is an unmodified Sipeed vendor image,
with its URL, version and SHA-256 in `resources/installer/SOURCE.json`; it is not
MIT-licensed flasher code. The FPGA programmer carries its upstream GPL licence,
corresponding-source archive and dependency notices in `resources/programmer/`.
