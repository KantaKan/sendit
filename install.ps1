# Build sendit from this clone and install it for the current user.
# Afterwards, search "sendit" in the Start menu.
#   Usage (from the repo folder):  powershell -ExecutionPolicy Bypass -File .\install.ps1
$ErrorActionPreference = 'Stop'

foreach ($tool in 'cargo', 'npm') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "$tool was not found. Install Rust (https://rustup.rs) and Node.js (https://nodejs.org), then run this again."
    }
}
if ((rustup show active-toolchain) -notmatch 'msvc') {
    throw "Rust must use the MSVC toolchain on Windows: run 'rustup default stable-msvc' (needs Visual Studio Build Tools with C++)."
}

Push-Location (Join-Path $PSScriptRoot 'app')
try {
    npm ci
    if ($LASTEXITCODE) { throw 'npm ci failed' }
    npx tauri build --bundles nsis
    if ($LASTEXITCODE) { throw 'build failed' }
} finally {
    Pop-Location
}

$setup = Get-ChildItem (Join-Path $PSScriptRoot 'target\release\bundle\nsis\*-setup.exe') |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $setup) { throw 'The installer was not built.' }

Write-Host "Installing $($setup.Name)..."
Start-Process $setup.FullName -ArgumentList '/S' -Wait
Write-Host 'Done. Search "sendit" in the Start menu.'
