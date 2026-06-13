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
    Copy-Item -LiteralPath $f.FullName -Destination (Join-Path $TargetBin $f.Name) -Force
    Write-Host "Installed $($f.Name) -> $TargetBin"
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

Write-Host @"

Done. From a new terminal:
  electrs-doge-launch          # light mode by default; index under F:\DogecoinData\electrs
  electrs-doge-launch -Build
  electrs-doge-launch -FullMode   # full index when you have disk on a new PC
  electrs-doge-kill

Explorer address history (dogexplorer\.env):
  DOGEXP_ADDRESS_API=electrum
  DOGEXP_ELECTRUM_SERVERS=tcp://127.0.0.1:50001

"@
