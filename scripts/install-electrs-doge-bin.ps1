#!/usr/bin/env pwsh
<#
.SYNOPSIS
  Copy electrs-doge launch helpers into %USERPROFILE%\bin.

.EXAMPLE
  .\scripts\install-electrs-doge-bin.ps1 -DogecoRoot "$env:USERPROFILE\Desktop\dogeco"
#>
[CmdletBinding()]
param(
    [string] $DogecoRoot = '',
    [string] $TargetBin = ''
)

$ErrorActionPreference = 'Stop'

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$srcBin = Join-Path $here 'bin'

if (-not $DogecoRoot) {
    $DogecoRoot = Split-Path -Parent (Split-Path -Parent $here)
}
$DogecoRoot = [System.IO.Path]::GetFullPath($DogecoRoot)

if (-not $TargetBin) {
    $TargetBin = Join-Path $env:USERPROFILE 'bin'
}
New-Item -ItemType Directory -Path $TargetBin -Force | Out-Null

foreach ($f in Get-ChildItem -LiteralPath $srcBin -File) {
    $dst = Join-Path $TargetBin $f.Name
    if (Test-Path -LiteralPath $dst) {
        Remove-Item -LiteralPath $dst -Force
    }
    $linked = $false
    try {
        New-Item -ItemType HardLink -Path $dst -Target $f.FullName | Out-Null
        $linked = $true
        Write-Host "hardlink $($f.Name) -> $TargetBin"
    } catch {
        $linked = $false
    }
    if (-not $linked) {
        Copy-Item -LiteralPath $f.FullName -Destination $dst -Force
        Write-Host "copied $($f.Name) -> $TargetBin"
    }
}

$electrsRepo = Join-Path $DogecoRoot 'electrs-doge'
[System.Environment]::SetEnvironmentVariable('ELECTRS_REPO', $electrsRepo, 'User')
if (-not [System.Environment]::GetEnvironmentVariable('DOGECOIN_DATA_DIR', 'User')) {
    if (Test-Path -LiteralPath 'F:\DogecoinData') {
        [System.Environment]::SetEnvironmentVariable('DOGECOIN_DATA_DIR', 'F:\DogecoinData', 'User')
    }
}
if (-not [System.Environment]::GetEnvironmentVariable('ELECTRS_DB_DIR', 'User')) {
    $dataDir = [System.Environment]::GetEnvironmentVariable('DOGECOIN_DATA_DIR', 'User')
    if (-not $dataDir) { $dataDir = 'F:\DogecoinData' }
    [System.Environment]::SetEnvironmentVariable('ELECTRS_DB_DIR', (Join-Path $dataDir 'electrs'), 'User')
}
[System.Environment]::SetEnvironmentVariable('ELECTRS_LIGHTMODE', '1', 'User')
[System.Environment]::SetEnvironmentVariable('ELECTRS_HTTP_ADDR', '127.0.0.1:3003', 'User')

Write-Host @"

Done. Prefer the full stack:
  dogenals launch              # starts electrs-doge (Electrum :50001 + Esplora :3003)
  dogenals kill                # stops electrs.exe (not Core)
  dogenals launch electrs      # electrs only
  set DOGENALS_SKIP_ELECTRS=1  # omit from dogenals launch

Standalone:
  electrs compile              # parks in-use electrs.exe, cargo build --release
  electrs-doge compile         # same
  electrs-doge launch
  electrs-doge-launch -FullMode   # full index when you have disk on a new PC
  electrs-doge kill

Explorer address history (dogexplorer\backend\.env):
  DOGEXP_ADDRESS_API=electrum
  DOGEXP_ELECTRUM_SERVERS=tcp://127.0.0.1:50001

Esplora HTTP (not command.dog :3000):
  http://127.0.0.1:3003  ->  https://electrs.command.dog

"@
