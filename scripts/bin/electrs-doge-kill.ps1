#!/usr/bin/env pwsh
# Stop electrs-doge only. Never touches Dogecoin Core or command.dog :3000.
#Requires -Version 5.1
$ErrorActionPreference = 'Continue'

function Get-ListenPort([string] $Addr, [int] $Fallback) {
    if ($Addr -match ':(\d+)\s*$') { return [int]$Matches[1] }
    return $Fallback
}

function Stop-ElectrsOnPort([int] $Port) {
    if ($Port -eq 3000) {
        Write-Host "Refusing to kill :3000 (command.dog API)." -ForegroundColor Yellow
        return
    }
    try {
        $conns = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
    } catch {
        return
    }
    foreach ($c in $conns) {
        try {
            $p = Get-Process -Id $c.OwningProcess -ErrorAction Stop
            if ($p.ProcessName -notmatch '^electrs') {
                Write-Host "  skip :$Port pid $($p.Id) ($($p.ProcessName)) — not electrs" -ForegroundColor DarkGray
                continue
            }
            Stop-Process -Id $p.Id -Force -ErrorAction Stop
            Write-Host "  killed electrs pid $($p.Id) on :$Port"
        } catch {}
    }
}

$electrumPort = Get-ListenPort ($env:ELECTRS_ELECTRUM_ADDR) 50001
$httpAddr = if ($env:ELECTRS_HTTP_ADDR) { $env:ELECTRS_HTTP_ADDR } else { '127.0.0.1:3003' }
if ($httpAddr -match ':3000\s*$') { $httpAddr = '127.0.0.1:3003' }
$httpPort = Get-ListenPort $httpAddr 3003

Write-Host 'Stopping electrs-doge...' -ForegroundColor Yellow
Get-Process -Name electrs -ErrorAction SilentlyContinue | ForEach-Object {
    try {
        Stop-Process -Id $_.Id -Force -ErrorAction Stop
        Write-Host "  killed electrs.exe pid $($_.Id)"
    } catch {}
}
Stop-ElectrsOnPort $electrumPort
Stop-ElectrsOnPort $httpPort
Write-Host 'electrs-doge stopped.' -ForegroundColor Green
