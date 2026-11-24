# First installation and switching systems — 0.3.0

Supports Tang Nano 20K + BL616 with GW2A(R)-18 and Winbond W25Q64 (8 MiB)
FPGA boot flash. This is a hardware-test candidate; no physical first-install
or conversion has been performed for this release.

1. Prepare the selected console or Arcade release and its SD-card contents.
   Back up the SD card yourself. The flasher never formats or writes the card.
2. Power off, remove the card and disconnect HDMI. Connect only this board.
   Hold UPDATE while connecting USB to enter the BL616 ROM programming mode.
3. Choose the release ZIP/folder/manifest. Set **Operation → First install /
   switch system** and confirm that the board is a Tang Nano 20K.
4. **Step 1 — Prepare Nano 20K.** The flasher makes a durable full 4 MiB
   controller backup before installing the pinned Sipeed programmer into its
   primary slot. Every byte after that slot is preserved and full readback is
   checked. No efuses or whole-chip erase operations are used.
5. **Step 2 — Install FPGA boot image.** Reconnect the same board normally,
   without UPDATE, keeping the card/HDMI disconnected. Select its programmer
   if more than one appears, and confirm the board again. The flasher checks
   the FPGA and external-flash IDs, saves a full 8 MiB FPGA flash backup, writes
   the selected package’s CPS1 480p (Arcade) or NES (console) boot image, and
   verifies the complete flash. Bytes after the boot-image erase span are
   preserved. Windows requires a libusb/WinUSB-compatible JTAG interface driver.
6. **Step 3 — Finish controller installation.** Reconnect in UPDATE mode and
   confirm the same board. The flasher checks the saved backups, programmer
   and reserved regions before writing the application, cache and core bank.
   Arcade gets a validated recovery image and complete protocol-28 pending
   record. Console gets its four-core bank and clean staging/cache space.
   The primary programmer, reserved/calibration regions and final sector are
   preserved. Another full controller backup and full readback are recorded.
7. Only **Installed & verified** means all stages completed. Disconnect all
   power, insert the prepared card, reconnect video and power on normally.
   Keep all backups until the selected system boots and games have been checked.

The app retains the pending installation across restarts after completed
steps. If a step fails, keep the backup folder and activity log; reconnect the
same board and use the pending step. It does not automatically roll back a
failed flash or claim physical boot validation. A partially written FPGA boot
image can be retried against its retained original backup; outside bytes must
still match. The controller programmer remains available during the final
application step.

The first-install action intentionally replaces an unknown or different system;
it does not apply the update-only requirement for an already matching menu and
cache. Update-only mode remains the default. Games and SD files are unchanged.

Sipeed documents the Nano 20K programmer, the secondary application address and
secondary-boot requirement in its [debugger update guide](https://wiki.sipeed.com/hardware/en/tang/common-doc/update_debugger).
This release pins Nano 20K programmer **2025030317**, SHA-256
`bc5b1f733a13562c898ff66b540038c116c3d77ca202d0afebd1e7af1c3b3203`.
FPGA access uses a pinned [openFPGALoader](https://github.com/trabucayre/openFPGALoader/tree/3c52ab1487ff759e1a3790346455ec7e92ba8c44)
build, with explicit device selection and complete readback in addition to
its write verification.

The Mac 0.3.0 release requires Apple Silicon and macOS 15 or newer, matching
the bundled native programmer and USB libraries. The programmer executable is
checked before the first setup write.
