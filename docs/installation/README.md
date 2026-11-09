# Firmware installation

1. Identify your [board and FPGA module](../hardware/README.md).
2. Open the [release index](../releases/README.md) and select a release for your
   exact board and revision. Wait for a matching release before flashing.
3. Download the firmware ZIP and its matching `-SHA256.txt` asset to the same
   directory. Check the digest before extracting or flashing. On macOS, run
   `shasum -a 256 -c <checksum-file>`. On Linux, run
   `sha256sum -c <checksum-file>`. On Windows, compare the output of
   `Get-FileHash <firmware-zip> -Algorithm SHA256` with the checksum file.
4. Read the release's installation and recovery instructions. Make any backup
   described there, use the specified flasher and programming mode, and copy
   SD card files to the specified locations.
5. Follow the release notes to verify the boot menu, inputs and supported cores.

Programming targets and SD card layouts vary by platform and version.
