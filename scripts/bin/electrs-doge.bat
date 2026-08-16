@echo off
setlocal EnableExtensions
REM electrs-doge helper (Electrum :50001 + Esplora HTTP :3003)
REM Prefer: dogenals launch  (starts this as part of the eco)

set "SCRIPT_DIR=%~dp0"
set "CMD=%~1"
if "%CMD%"=="" set "CMD=help"

if defined ELECTRS_REPO (
  set "ELECTRS_ROOT=%ELECTRS_REPO%"
) else (
  set "ELECTRS_ROOT=%USERPROFILE%\Desktop\dogeco\electrs-doge"
)
if exist "%SCRIPT_DIR%electrs-doge-launch.ps1" (
  set "LAUNCH_PS1=%SCRIPT_DIR%electrs-doge-launch.ps1"
  set "KILL_PS1=%SCRIPT_DIR%electrs-doge-kill.ps1"
) else (
  set "LAUNCH_PS1=%ELECTRS_ROOT%\scripts\bin\electrs-doge-launch.ps1"
  set "KILL_PS1=%ELECTRS_ROOT%\scripts\bin\electrs-doge-kill.ps1"
)

if /I "%CMD%"=="help" goto :help
if /I "%CMD%"=="launch" goto :launch
if /I "%CMD%"=="start" goto :launch
if /I "%CMD%"=="up" goto :launch
if /I "%CMD%"=="kill" goto :kill
if /I "%CMD%"=="stop" goto :kill
if /I "%CMD%"=="down" goto :kill
if /I "%CMD%"=="status" goto :status
echo Unknown command: %CMD%
echo.
goto :help

:help
echo electrs-doge — Electrum + Esplora HTTP for Dogecoin Core
echo.
echo Usage:
echo   electrs-doge launch     start (logged when called from dogenals)
echo   electrs-doge kill       stop electrs.exe only ^(not Core, not :3000^)
echo   electrs-doge status     Electrum :50001 / HTTP :3003
echo.
echo Full stack: dogenals launch
echo Skip:       set DOGENALS_SKIP_ELECTRS=1
echo HTTP:       http://127.0.0.1:3003  -^> https://electrs.command.dog
echo Electrum:   tcp://127.0.0.1:50001  ^(dogexplorer address pages^)
exit /b 0

:launch
if not exist "%LAUNCH_PS1%" (
  echo Missing %LAUNCH_PS1%
  exit /b 1
)
if /I "%ELECTRS_LOGGED%"=="0" (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%LAUNCH_PS1%" %2 %3 %4
) else (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%LAUNCH_PS1%" -Logged %2 %3 %4
)
exit /b %ERRORLEVEL%

:kill
if exist "%KILL_PS1%" (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%KILL_PS1%"
  exit /b %ERRORLEVEL%
)
taskkill /F /IM electrs.exe 2>nul
exit /b 0

:status
echo electrs-doge:
tasklist /FI "IMAGENAME eq electrs.exe" 2>nul | find /I "electrs.exe" >nul
if errorlevel 1 (
  echo   process: not running
) else (
  echo   process: electrs.exe
)
powershell -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ports=50001,3003; foreach($p in $ports){ $c=Get-NetTCPConnection -LocalPort $p -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1; if($c){ Write-Output ('  :'+$p+' listening') } else { Write-Output ('  :'+$p+' down') } }"
exit /b 0
