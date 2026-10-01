#!/usr/bin/env pwsh
# Reuse Kabosu shared Dogecoin Core env (DOGECOIN_DATA_DIR, RPC creds from dogecoin.conf).
# Safe from ~/bin copies: prefer ELECTRS_REPO / dogestack, never guess Core datadir wrong.

$ScriptDir = $PSScriptRoot
$candidates = @(
    (Join-Path $ScriptDir '..\..\..'),
    (Join-Path $env:USERPROFILE 'dogestack'),
    'C:\Users\jheav\dogestack'
)
$DogecoRoot = $null
foreach ($c in $candidates) {
    try {
        $full = [System.IO.Path]::GetFullPath($c)
        if (Test-Path -LiteralPath (Join-Path $full 'electrs-doge')) {
            $DogecoRoot = $full
            break
        }
    } catch {}
}

if ($DogecoRoot) {
    $sharedEnv = Join-Path $DogecoRoot 'kabosu\scripts\bin\dogecoin-env.ps1'
    if (Test-Path -LiteralPath $sharedEnv) {
        . $sharedEnv
    }
}

if (-not $env:DOGECOIN_DATA_DIR) {
    if (Test-Path -LiteralPath 'F:\DogecoinData') {
        $env:DOGECOIN_DATA_DIR = 'F:\DogecoinData'
    }
    elseif ($env:APPDATA) {
        $env:DOGECOIN_DATA_DIR = Join-Path $env:APPDATA 'Dogecoin'
    }
}
if (-not $env:DOGECOIN_CONF -and $env:DOGECOIN_DATA_DIR) {
    $env:DOGECOIN_CONF = Join-Path $env:DOGECOIN_DATA_DIR 'dogecoin.conf'
}

if (-not $env:ELECTRS_DB_DIR -and $env:DOGECOIN_DATA_DIR) {
    $env:ELECTRS_DB_DIR = Join-Path $env:DOGECOIN_DATA_DIR 'electrs'
}

$liveRepo = Join-Path $env:USERPROFILE 'dogestack\electrs-doge'
$repoOk = $env:ELECTRS_REPO -and (Test-Path -LiteralPath (Join-Path $env:ELECTRS_REPO 'Cargo.toml'))
if (-not $repoOk) {
    if (Test-Path -LiteralPath (Join-Path $liveRepo 'Cargo.toml')) {
        $env:ELECTRS_REPO = $liveRepo
    }
    elseif ($DogecoRoot) {
        $env:ELECTRS_REPO = Join-Path $DogecoRoot 'electrs-doge'
    }
}
