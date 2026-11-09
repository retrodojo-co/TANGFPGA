![TANGFPGA](assets/logos/tangfpga.svg)

# TANGFPGA

TANGFPGA is a firmware project for Sipeed Tang FPGA boards. Each product targets
a specific board and has its own firmware, menu, core compatibility and release
history.

## Hardware and downloads

| Product | Target board | Project status | Firmware downloads |
| --- | --- | --- | --- |
| **TANGFPGA 9K** | Sipeed Tang Nano 9K | Planned | [Releases](https://github.com/retrodojo-co/TANGFPGA/releases?q=9k-v) · look for `9k-v…` |
| **TANGFPGA 20K** | Sipeed Tang Nano 20K | Development baseline established | [Releases](https://github.com/retrodojo-co/TANGFPGA/releases?q=20k-v) · look for `20k-v…` |
| **TANGFPGA 60K** | Sipeed Tang Console with Mega 60K SOM | Planned | [Releases](https://github.com/retrodojo-co/TANGFPGA/releases?q=60k-v) · look for `60k-v…` |

Firmware downloads will appear after the first approved release for each board.
Use the [hardware guide](docs/hardware/README.md) to identify your board and
module before selecting a package.

## FPGA cores

The 20K development baseline includes NES / Famicom, Master System / Game Gear
and Game Boy / Game Boy Colour. SNES is experimental in that baseline.
Released core support will be listed by version in the
[compatibility matrix](docs/compatibility/README.md).

## Installation

Start with the [installation guide](docs/installation/README.md). Verify the
package's SHA-256 checksum and follow the flashing and SD card instructions in
its release notes. The exact board and any required controller firmware are
specified per release.

## Release history

See the [release index](docs/releases/README.md) or browse
[all GitHub Releases](https://github.com/retrodojo-co/TANGFPGA/releases).
There are currently no TANGFPGA firmware releases.

## Known limitations

- Firmware packages are specific to their target board and hardware revisions.
- TANGFPGA has yet to publish a validated core, controller or display
  compatibility list.
- SNES remains experimental in the 20K development baseline.

## Roadmap

- Validate the 20K firmware, menu and cores on hardware and document installation
  and recovery.
- Bring up independent boot and menu implementations for 9K and 60K.
- Publish per-board firmware packages and expand the compatibility matrices as
  validation is completed.

See [licensing](LICENSE) for the scope of this documentation repository.
