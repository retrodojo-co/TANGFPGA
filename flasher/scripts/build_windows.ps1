# Run on Windows x64 with Node 22, Python 3.11, Rust MSVC and VS C++ tools.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Checked {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE" }
}

if ($env:OS -ne 'Windows_NT' -or -not [Environment]::Is64BitOperatingSystem) {
    throw 'Build this installer on 64-bit Windows.'
}
foreach ($tool in @('node', 'npm.cmd', 'py', 'cargo', 'rustup', 'git')) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "Missing $tool. See docs/windows-installer.md for build prerequisites."
    }
}
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    Invoke-Checked 'py' @('-3.11', '-c', "import struct; assert struct.calcsize('P') == 8, 'Install 64-bit Python 3.11'")
    Invoke-Checked 'rustup' @('target', 'add', 'x86_64-pc-windows-msvc')
    $venv = Join-Path $root '.windows-build/venv'
    Invoke-Checked 'py' @('-3.11', '-m', 'venv', $venv)
    $python = Join-Path $venv 'Scripts/python.exe'
    Invoke-Checked $python @('-m', 'pip', 'install', '-r', 'scripts/requirements-bl616.txt')
    # Clear generated target-specific output, so a source ZIP from a Mac cannot
    # leave a Mach-O helper or dylibs in the Windows installer.
    $helper = Join-Path $root 'src-tauri/resources/bl616'
    foreach ($name in @('nanoblast-bl616', 'licenses', 'dependencies.json', 'sha256.txt')) {
        $generated = Join-Path $helper $name
        if (Test-Path $generated) { Remove-Item $generated -Recurse -Force }
    }
    # A native programmer is required for first installation and conversion.
    $mingw = 'C:\msys64\mingw64\bin'
    if (-not (Test-Path (Join-Path $mingw 'cmake.exe'))) { throw 'Install MSYS2 MINGW64 programmer prerequisites from docs/windows-installer.md.' }
    $env:PATH = $mingw + ';' + $env:PATH
    $programmerSource = Join-Path $root '.windows-build/openFPGALoader'
    if (-not (Test-Path (Join-Path $programmerSource '.git'))) { Invoke-Checked 'git' @('clone', '--no-checkout', 'https://github.com/trabucayre/openFPGALoader.git', $programmerSource) }
    Invoke-Checked 'git' @('-C', $programmerSource, 'fetch', 'origin', '3c52ab1487ff759e1a3790346455ec7e92ba8c44')
    Invoke-Checked 'git' @('-C', $programmerSource, 'checkout', '--detach', '3c52ab1487ff759e1a3790346455ec7e92ba8c44')
    $programmerBuild = Join-Path $root '.windows-build/programmer-build'
    Invoke-Checked 'cmake' @('-S', $programmerSource, '-B', $programmerBuild, '-G', 'Ninja', '-DCMAKE_BUILD_TYPE=Release', '-DBUILD_TESTING=OFF')
    Invoke-Checked 'cmake' @('--build', $programmerBuild, '--parallel', '3')
    $staged = Join-Path $root 'src-tauri/resources/programmer'
    if (Test-Path $staged) { Remove-Item $staged -Recurse -Force }
    Invoke-Checked $python @('scripts/stage_programmer.py', (Join-Path $programmerBuild 'openFPGALoader.exe'), '--license', (Join-Path $programmerSource 'LICENSE'))
    Invoke-Checked 'git' @('-C', $programmerSource, 'archive', '--format=tar.gz', '-o', (Join-Path $staged 'openFPGALoader-source.tar.gz'), 'HEAD')
    Invoke-Checked $python @('scripts/stage_bl616.py')
    Invoke-Checked $python @('-m', 'unittest', 'discover', '-s', 'scripts/bl616', '-v')
    Invoke-Checked 'npm.cmd' @('ci')
    Invoke-Checked 'npm.cmd' @('run', 'check')
    Invoke-Checked 'cargo' @('test', '--locked', '--manifest-path', 'src-tauri/Cargo.toml', '--features', 'desktop', '--target', 'x86_64-pc-windows-msvc')
    Invoke-Checked 'npm.cmd' @('run', 'tauri', 'build', '--', '--target', 'x86_64-pc-windows-msvc', '--bundles', 'nsis')
    Invoke-Checked $python @('scripts/verify_windows_installer.py')
} finally {
    Pop-Location
}
