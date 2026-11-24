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
