# Windows x64 installer

Build on 64-bit Windows with Node 22, Python 3.11 x64, Rust stable MSVC and
Visual Studio 2022 C++ Build Tools plus Windows SDK installed.

From the flasher directory, run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/build_windows.ps1
```

The script freezes a native Windows BL616 helper, runs the Python, frontend
and Rust desktop checks, builds the NSIS installer and validates the PE
architecture and helper checksum. Output:
`src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`.

The installer uses the TangFPGA name/icon, a per-user install, and an embedded
WebView2 bootstrapper. The first runtime installation may need Internet.
It is unsigned. Installation, uninstall, driver/port enumeration and hardware
flashing must be checked on Windows before treating it as a validated release.

The source/build-kit ZIP contains source and this build entry point, not a
prebuilt `.exe`. A Mac cannot freeze the Windows Python helper. The repository
workflow builds the app and helper together on a native Windows runner.

## Programmer build prerequisites for 0.3.0

Install MSYS2 in `C:\msys64` and use its UCRT64 shell to install:

```sh
pacman -S --needed mingw-w64-ucrt-x86_64-toolchain mingw-w64-ucrt-x86_64-cmake mingw-w64-ucrt-x86_64-ninja mingw-w64-ucrt-x86_64-pkgconf mingw-w64-ucrt-x86_64-libusb mingw-w64-ucrt-x86_64-libftdi mingw-w64-ucrt-x86_64-hidapi mingw-w64-ucrt-x86_64-zlib
```

The PowerShell build script checks out and builds the pinned openFPGALoader,
stages its runtime DLLs and corresponding source, then freezes the BL616 helper
and packages both into the installer. These tools are bundled for users; they
do not need a build environment. A compatible USB driver is needed for the
FPGA programmer phase. The app does not automatically replace USB drivers.
