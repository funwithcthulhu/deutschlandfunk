# build-installer.ps1 - Build a release binary and Inno Setup installer for DLF LingQ Reader
#
# Prerequisites (one-time setup):
#   1. Install Rust:         https://rustup.rs
#   2. Install Inno Setup:   winget install JRSoftware.InnoSetup
#
# Usage:
#   .\scripts\build-installer.ps1
#
# Output:
#   installer\output\dlf-lingq-reader-setup.exe

$ErrorActionPreference = "Stop"

# Find iscc.exe (Inno Setup compiler)
$candidates = @(
    "${env:LOCALAPPDATA}\Programs\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "${env:ProgramFiles}\Inno Setup 6\ISCC.exe"
)
try { $found = (Get-Command iscc -ErrorAction SilentlyContinue).Source; if ($found) { $candidates += $found } } catch {}
$iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1

if (-not $iscc) {
    Write-Host "ERROR: Inno Setup not found." -ForegroundColor Red
    Write-Host "Install it with:  winget install JRSoftware.InnoSetup" -ForegroundColor Yellow
    exit 1
}

Write-Host "=== Building DLF LingQ Reader release binary ===" -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

Write-Host ""
Write-Host "=== Creating installer with Inno Setup ===" -ForegroundColor Cyan
Write-Host "Using: $iscc"
& $iscc "installer\deutschlandfunk-reader.iss"
if ($LASTEXITCODE -ne 0) { throw "Inno Setup compilation failed" }

$exe = Get-Item "installer\output\dlf-lingq-reader-setup.exe"
Write-Host ""
Write-Host "=== Done! ===" -ForegroundColor Green
Write-Host "Installer: $($exe.FullName)" -ForegroundColor Yellow
Write-Host "File size: $([math]::Round($exe.Length / 1MB, 1)) MB"
Write-Host ""
Write-Host "Double-click dlf-lingq-reader-setup.exe to install." -ForegroundColor Gray
