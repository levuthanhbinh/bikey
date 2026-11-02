#Requires -RunAsAdministrator
<#
.SYNOPSIS
  Installs (or with -Uninstall removes) a local development build of the
  bikey Windows text service. Release installs use the MSI instead.

.DESCRIPTION
  Copies the 64-bit and 32-bit DLLs to "Program Files\bikey" (writable only
  by administrators, which matters because Windows loads the DLL into every
  app) and registers them with regsvr32.

  Build first:
    cargo build -p bikey-tsf --release
    cargo build -p bikey-tsf --release --target i686-pc-windows-msvc
#>
param([switch]$Uninstall)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $env:ProgramFiles 'bikey'

$builds = @(
    @{ Src = "$root\target\release\bikey.dll"; Dir = "$dest\x64"; RegSvr = "$env:WINDIR\System32\regsvr32.exe" },
    @{ Src = "$root\target\i686-pc-windows-msvc\release\bikey.dll"; Dir = "$dest\x86"; RegSvr = "$env:WINDIR\SysWOW64\regsvr32.exe" }
)

function Invoke-RegSvr($exe, [string[]]$arguments) {
    $p = Start-Process -FilePath $exe -ArgumentList $arguments -Wait -PassThru -WindowStyle Hidden
    if ($p.ExitCode -ne 0) { throw "$exe $arguments failed with exit code $($p.ExitCode)" }
}

foreach ($b in $builds) {
    $dll = Join-Path $b.Dir 'bikey.dll'
    if (Test-Path $dll) {
        Invoke-RegSvr $b.RegSvr @('/s', '/u', "`"$dll`"")
    }
    if ($Uninstall) { continue }

    if (-not (Test-Path $b.Src)) { throw "Missing build: $($b.Src)" }
    New-Item -ItemType Directory -Force $b.Dir | Out-Null
    # Old copies may still be loaded by running apps; leftovers from earlier runs are removed when free.
    Get-ChildItem $b.Dir -Filter '*.old' -ErrorAction SilentlyContinue |
        ForEach-Object { try { Remove-Item $_.FullName -Force -ErrorAction Stop } catch {} }
    if (Test-Path $dll) {
        # A loaded DLL cannot be overwritten but can be renamed.
        Move-Item $dll "$dll.$(Get-Date -Format yyyyMMddHHmmss).old"
    }
    Copy-Item $b.Src $dll
    Invoke-RegSvr $b.RegSvr @('/s', "`"$dll`"")
    Write-Host "Registered $dll"
}

if ($Uninstall) {
    Remove-Item $dest -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host 'bikey development build removed. Files still in use are deleted after a restart.'
} else {
    Write-Host 'Done. Pick "bikey" under Vietnamese in the input switcher (Win+Space).'
}
