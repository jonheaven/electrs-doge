#!/usr/bin/env pwsh
# Release-build electrs.exe. Parks the in-use binary so cargo can write
# while the indexer keeps running (same trick as dogenals compile).
# Never stops Dogecoin Core.
#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch] $CleanupParked
)

$ErrorActionPreference = 'Stop'

$ScriptDir = $PSScriptRoot
. (Join-Path $ScriptDir 'dogecoin-env.ps1')

if (-not $env:ELECTRS_REPO) {
    $env:ELECTRS_REPO = (Resolve-Path (Join-Path $ScriptDir '..\..')).Path
}
$repo = $env:ELECTRS_REPO
$exe = Join-Path $repo 'target\release\electrs.exe'

function Test-FileLocked([string] $Path) {
    if (-not (Test-Path -LiteralPath $Path)) { return $false }
    try {
        $fs = [System.IO.File]::Open($Path, 'Open', 'ReadWrite', 'None')
        $fs.Dispose()
        return $false
    } catch {
        return $true
    }
}

function Get-ParkedSidecars([string] $ExePath) {
    $dir = Split-Path -Parent $ExePath
    if (-not (Test-Path -LiteralPath $dir)) { return @() }
    $leaf = Split-Path -Leaf $ExePath
    $pdb = [System.IO.Path]::ChangeExtension($leaf, '.pdb')
    Get-ChildItem -LiteralPath $dir -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -like "$leaf.running*" -or $_.Name -like "$pdb.running*" }
}

function Unlock-ReleaseBinary([string] $ExePath) {
    $pdb = [System.IO.Path]::ChangeExtension($ExePath, '.pdb')
    foreach ($p in @($ExePath, $pdb)) {
        if (-not (Test-Path -LiteralPath $p)) { continue }
        $parked = "$p.running"
        if (Test-Path -LiteralPath $parked) {
            if (Test-FileLocked $parked) {
                $parked = "$p.running.$(Get-Date -Format yyyyMMddHHmmss)"
            } else {
                Remove-Item -LiteralPath $parked -Force
            }
        }
        if (Test-FileLocked $p) {
            Write-Host "Parking in-use binary - electrs stays up: $p" -ForegroundColor Yellow
            Move-Item -LiteralPath $p -Destination $parked -Force
        }
    }
}

function Remove-ParkedBinaries {
    $removed = 0
    foreach ($f in (Get-ParkedSidecars $exe)) {
        if (Test-FileLocked $f.FullName) {
            Write-Host "Still locked, process not dead yet: $($f.FullName)" -ForegroundColor Yellow
            continue
        }
        Remove-Item -LiteralPath $f.FullName -Force
        Write-Host "Removed parked $($f.Name)"
        $removed++
    }
    if ($removed -eq 0) {
        Write-Host "No parked electrs binaries to remove."
    }
}

if ($CleanupParked) {
    Remove-ParkedBinaries
    exit 0
}

if (-not (Test-Path -LiteralPath (Join-Path $repo 'Cargo.toml'))) {
    Write-Error "electrs-doge Cargo.toml not found at $repo"
}

$null = Get-Command cargo -ErrorAction Stop

Write-Host "electrs-doge compile - release (running electrs stays up)" -ForegroundColor Cyan
Write-Host "  Repo: $repo" -ForegroundColor DarkGray
Write-Host "  Out:  $exe" -ForegroundColor DarkGray
Write-Host "  Wait for Finished release - LTO takes a while." -ForegroundColor Yellow
Write-Host ""

Unlock-ReleaseBinary $exe

Push-Location $repo
try {
    cargo build --release
    $code = $LASTEXITCODE
} finally {
    Pop-Location
}

if ($code -ne 0) {
    Write-Host "Compile failed (exit $code)." -ForegroundColor Red
    Write-Host "If the log shows LNK1104 / Access denied on electrs.exe: electrs-doge kill, then electrs-doge compile." -ForegroundColor Yellow
    exit $code
}

Write-Host ""
Write-Host "Built: $exe" -ForegroundColor Green
Write-Host "Stack is still on the old binary. Brief switchover:" -ForegroundColor Yellow
Write-Host "  electrs-doge kill"
Write-Host "  electrs-doge launch"
Write-Host "Or: dogenals kill electrs && dogenals launch electrs"
exit 0
