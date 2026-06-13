#!/usr/bin/env pwsh
# Start electrs-doge (Electrum + Esplora HTTP) against local Dogecoin Core.
# Index DB: %DOGECOIN_DATA_DIR%\electrs\mainnet  (default F:\DogecoinData\electrs\mainnet)
# Blocks/RPC cookie: %DOGECOIN_DATA_DIR%  (same tree as dogecoind)

param(
    [switch] $Build,
    [switch] $FullMode,   # full index (more disk); default is light until you opt in
    [switch] $NoWindow
)

$ErrorActionPreference = 'Stop'

$ScriptDir = $PSScriptRoot
. (Join-Path $ScriptDir 'dogecoin-env.ps1')

if (-not $env:ELECTRS_REPO) {
    $env:ELECTRS_REPO = (Resolve-Path (Join-Path $ScriptDir '..\..')).Path
}

$repo = $env:ELECTRS_REPO
if (-not (Test-Path -LiteralPath $repo)) {
    Write-Error "ELECTRS_REPO not found: $repo"
}

if (-not $env:DOGECOIN_DATA_DIR) {
    Write-Error 'DOGECOIN_DATA_DIR is not set and F:\DogecoinData was not found. Set DOGECOIN_DATA_DIR before launching.'
}

if (-not $env:ELECTRS_DB_DIR) {
    $env:ELECTRS_DB_DIR = Join-Path $env:DOGECOIN_DATA_DIR 'electrs'
}

$network = if ($env:ELECTRS_NETWORK) { $env:ELECTRS_NETWORK } else { 'mainnet' }
$daemonDir = $env:DOGECOIN_DATA_DIR
$dbDir = $env:ELECTRS_DB_DIR
$daemonRpc = if ($env:ELECTRS_DAEMON_RPC_ADDR) { $env:ELECTRS_DAEMON_RPC_ADDR } else { '127.0.0.1:22555' }
$electrumAddr = if ($env:ELECTRS_ELECTRUM_ADDR) { $env:ELECTRS_ELECTRUM_ADDR } else { '127.0.0.1:50001' }
$httpAddr = if ($env:ELECTRS_HTTP_ADDR) { $env:ELECTRS_HTTP_ADDR } else { '127.0.0.1:3000' }
$verbosity = if ($env:ELECTRS_VERBOSE) { $env:ELECTRS_VERBOSE } else { '-vvvv' }

New-Item -ItemType Directory -Path $dbDir -Force | Out-Null

$exe = Join-Path $repo 'target\release\electrs.exe'
if ($Build -or -not (Test-Path -LiteralPath $exe)) {
    Write-Host 'Building electrs (release)...' -ForegroundColor Yellow
    Push-Location $repo
    cargo build --release
    if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
    Pop-Location
}

if (-not (Test-Path -LiteralPath $exe)) {
    Write-Error "Binary missing after build: $exe"
}

$cookieArg = @()
if ($env:DOGE_RPC_USERNAME -and $env:DOGE_RPC_PASSWORD) {
    $cookieArg = @('--cookie', "$($env:DOGE_RPC_USERNAME):$($env:DOGE_RPC_PASSWORD)")
}

$extraArgs = @()
$useLightMode = -not $FullMode
if ($env:ELECTRS_FULL_MODE -eq '1' -or $env:ELECTRS_FULL_MODE -eq 'true') {
    $useLightMode = $false
}
if ($env:ELECTRS_LIGHTMODE -eq '0' -or $env:ELECTRS_LIGHTMODE -eq 'false') {
    $useLightMode = $false
}
if ($useLightMode) {
    $extraArgs += '--lightmode'
}
if ($env:ELECTRS_EXTRA_ARGS) {
    $extraArgs += ($env:ELECTRS_EXTRA_ARGS -split '\s+' | Where-Object { $_ })
}

$electrsArgs = @(
    $verbosity,
    '--timestamp',
    '--network', $network,
    '--daemon-dir', $daemonDir,
    '--blocks-dir', (Join-Path $daemonDir 'blocks'),
    '--db-dir', $dbDir,
    '--daemon-rpc-addr', $daemonRpc,
    '--electrum-rpc-addr', $electrumAddr,
    '--http-addr', $httpAddr
) + $cookieArg + $extraArgs

Write-Host 'Starting electrs-doge...' -ForegroundColor Cyan
Write-Host "  Repo:       $repo" -ForegroundColor DarkGray
Write-Host "  Core dir:   $daemonDir" -ForegroundColor DarkGray
Write-Host "  Index DB:   $(Join-Path $dbDir $network)" -ForegroundColor DarkGray
Write-Host "  Mode:       $(if ($useLightMode) { 'light (less disk, more Core RPC)' } else { 'full index' })" -ForegroundColor $(if ($useLightMode) { 'Yellow' } else { 'Green' })
Write-Host "  Electrum:   tcp://$electrumAddr" -ForegroundColor Green
Write-Host "  HTTP API:   http://$httpAddr" -ForegroundColor Green
Write-Host ''
Write-Host 'dogexplorer (.env):' -ForegroundColor Yellow
Write-Host '  DOGEXP_ADDRESS_API=electrum'
Write-Host "  DOGEXP_ELECTRUM_SERVERS=tcp://$electrumAddr"
Write-Host ''

$argLine = ($electrsArgs | ForEach-Object {
    if ($_ -match '\s') { "`"$_`"" } else { $_ }
}) -join ' '

if ($NoWindow) {
    Push-Location $repo
    & $exe @electrsArgs
    Pop-Location
} else {
    Start-Process -FilePath 'cmd.exe' -ArgumentList @(
        '/k',
        "cd /d `"$repo`" && title electrs-doge && `"$exe`" $argLine"
    ) -WindowStyle Normal
    Write-Host 'Launched electrs-doge in a new window.' -ForegroundColor Green
}
