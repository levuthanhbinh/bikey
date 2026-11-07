<#
.SYNOPSIS
  Builds the 64-bit and 32-bit DLLs and the Windows installer
  (target\installer\bikey-setup-<version>.exe).

.PARAMETER Iscc
  Path to Inno Setup's ISCC.exe. Defaults to the INNO_ISCC environment variable,
  then ISCC on PATH.

.PARAMETER DllDir
  Package the DLLs in this folder (x64\bikey.dll and x86\bikey.dll) instead of
  the ones in target\. The release workflow uses it for code-signed DLLs.

.PARAMETER NoBuild
  Skip cargo build. Useful with -DllDir.
#>
param(
    [string]$Iscc = $env:INNO_ISCC,
    [string]$DllDir,
    [switch]$NoBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
if (-not $Iscc) { $Iscc = (Get-Command ISCC.exe -ErrorAction Stop).Source }

Push-Location $root
try {
    if (-not $NoBuild) {
        cargo build --release --locked -p bikey-tsf
        if ($LASTEXITCODE -ne 0) { throw 'cargo build (x64) failed' }
        cargo build --release --locked -p bikey-tsf --target i686-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) { throw 'cargo build (x86) failed' }
    }

    $match = Select-String -Path 'Cargo.toml' -Pattern '^version = "(.+)"' | Select-Object -First 1
    if (-not $match) { throw 'version not found in Cargo.toml' }
    $version = $match.Matches[0].Groups[1].Value

    $defines = @("/DAppVersion=$version")
    if ($DllDir) {
        $dir = (Resolve-Path $DllDir).Path
        foreach ($arch in 'x64', 'x86') {
            if (-not (Test-Path "$dir\$arch\bikey.dll")) { throw "missing $dir\$arch\bikey.dll" }
        }
        $defines += "/DX64Dll=$dir\x64\bikey.dll", "/DX86Dll=$dir\x86\bikey.dll"
    }

    & $Iscc @defines 'installer\bikey.iss'
    if ($LASTEXITCODE -ne 0) { throw 'ISCC failed' }
    Get-Item "target\installer\bikey-setup-$version.exe"
}
finally {
    Pop-Location
}
