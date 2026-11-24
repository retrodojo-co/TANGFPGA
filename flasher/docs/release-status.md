# TangFPGA Flasher 0.3.0 — first installation and conversion

Adds an explicit three-step Nano 20K first-install/switch-system flow, full
controller and FPGA backups, pinned vendor primary programmer, verified FPGA
boot programming and final controller/system installation. Supports an empty
user-flash board, console → Arcade, and Arcade → console. Existing update-only
mode stays conservative and is the default.

Local checks: production frontend build, nine frontend tests, 24 Rust desktop
tests, and 46 Python tests pass. Synthetic two-chip tests cover blank → console,
blank → Arcade → console → Arcade, protected regions, exact write bounds,
confirmation gates, wrong devices, partial backups, mismatched controller
serials and corrupted readback. The browser completes all three stages without
hardware access. Native installers are being rebuilt.

Hardware boot/first-install/conversion tests and Windows install/uninstall tests
remain pending. This release is a prerelease candidate; Mac is ad-hoc signed and
not notarized, Windows is unsigned. See [first-installation instructions](first-installation.md).

# TangFPGA Flasher 0.2.0 — Arcade support

Adds automatic `TANGFPGA_ARCADE_V1` package detection and application-only updates
for an existing Tang Nano 20K + BL616 Arcade installation. Protocol-28 recovery
cache validation is required; no blank-board bootstrap or console-to-Arcade
conversion is performed. SD-card files must be installed separately.

Validation: eight frontend tests, 24 local Rust desktop tests, 38 Python tests
and the production frontend build pass. Local r16 folder/manifest and private r17 folder/ZIP selection
and write plans match their respective reference updaters exactly, using synthetic
flash captures with the real reference images. No private ROM data or captures
are included in source or installer distributions. Physical flashing remains
untested. Both native build/test/package paths pass in
[run 45400672411](https://github.com/retrodojo-co/TANGFPGA/actions/runs/45400672411).
The downloaded Mac DMG passes signature, integrity, version and helper-hash checks.
Its frozen helper successfully selects the local r17 ZIP and produces the exact
expected application-only offline plan without hardware access. Windows
app/helper x64 architecture and helper checksum checks pass on the native runner.

Installers, source, usage notes and SHA-256 checksums are in
`artifacts/release/v0.2.0/`. See [Arcade preview](arcade-preview.jpg).

# TangFPGA Flasher 0.1.1 branding update

Uses the supplied white/red TangFPGA wordmark with RGB stripes, a black/red
interface, and matching desktop icons. Firmware update behavior is unchanged.
Both Mac Apple Silicon and Windows x64 installers passed the native build,
test and packaging workflow: [run 45307154228](https://github.com/retrodojo-co/TANGFPGA/actions/runs/45307154228).
The Mac ad-hoc signature, DMG integrity, version and bundled helper checksum
were also verified locally. Hardware testing remains pending.

Version 0.1.1 packages and checksums are in `artifacts/release/v0.1.1/`.

# TangFPGA Flasher 0.1.0 release status

Prepared 24 November 2026. Public release remains unpublished.

| Platform | State |
| --- | --- |
| Mac Apple Silicon | `.app` and `.dmg` built locally. Ad-hoc signature, disk-image integrity, app identifier/version and bundled helper SHA-256 verified. Not notarized. |
| Windows x64 | Native NSIS installer built on Windows 2022. Windows x64 app/helper PE architecture and helper SHA-256 checks passed. Installer UI and hardware testing remain pending. |
| Intel Mac / Windows ARM64 | Not included in this version. |

Local validation: TypeScript/Vite production build, seven frontend simulation
tests, 24 Rust desktop tests, 31 Python helper tests and the frozen Mac helper
self-check passed. The browser preview completed a simulated install at the
440 × 820 desktop window size; see [preview](preview.jpg).

No physical flashing has been performed for this fork. Windows install/uninstall
and USB/driver behavior require testing on a Windows machine. Firmware support
remains the inherited 20K BL616/NBCRXZ01 layout with all four console cores.
9K, 60K and alternate Arcade layouts are not added by this branding change.

Both native jobs passed in [GitHub Actions run 45304547725](https://github.com/retrodojo-co/TANGFPGA/actions/runs/45304547725).

Generated installers and checksums are staged in ignored `artifacts/release/`.
A Windows source/build kit is not a prebuilt Windows installer.

The workflow uses native macOS 15 ARM64 and Windows 2022 x64 runners, per
[GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
