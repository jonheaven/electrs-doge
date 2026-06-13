#!/usr/bin/env pwsh
# Reuse Kabosu shared Dogecoin Core env (DOGECOIN_DATA_DIR, RPC creds from dogecoin.conf).

$ScriptDir = $PSScriptRoot
$DogecoRoot = (Resolve-Path (Join-Path $ScriptDir '..\..\..')).Path
$sharedEnv = Join-Path $DogecoRoot 'kabosu\scripts\bin\dogecoin-env.ps1'

if (Test-Path -LiteralPath $sharedEnv) {
    . $sharedEnv
} else {
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
}

if (-not $env:ELECTRS_DB_DIR -and $env:DOGECOIN_DATA_DIR) {
    $env:ELECTRS_DB_DIR = Join-Path $env:DOGECOIN_DATA_DIR 'electrs'
}
