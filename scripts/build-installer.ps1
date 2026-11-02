<#
.SYNOPSIS
  Builds the 64-bit and 32-bit DLLs and the Windows installer
  (target\installer\bikey-setup-<version>.exe).

.PARAMETER Iscc
  Path to Inno Setup's ISCC.exe. Defaults to the INNO_ISCC environment variable,
  then ISCC on PATH.
#>
param([string]$Iscc = $env:INNO_ISCC)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Iscc) { $Iscc = (Get-Command ISCC.exe -ErrorAction Stop).Source }

Push-Location $root
try {
    cargo build --release -p bikey-tsf
    if ($LASTEXITCODE -ne 0) { throw 'cargo build (x64) failed' }
    cargo build --release -p bikey-tsf --target i686-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'cargo build (x86) failed' }

    $match = Select-String -Path 'Cargo.toml' -Pattern '^version = "(.+)"' | Select-Object -First 1
    if (-not $match) { throw 'version not found in Cargo.toml' }
    $version = $match.Matches[0].Groups[1].Value

    & $Iscc "/DAppVersion=$version" 'installer\bikey.iss'
    if ($LASTEXITCODE -ne 0) { throw 'ISCC failed' }
    Get-Item "target\installer\bikey-setup-$version.exe"
}
finally {
    Pop-Location
}
