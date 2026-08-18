@echo off
REM Stop electrs-doge only. Does NOT kill :3000 - command.dog API.
set "HERE=%~dp0"
if exist "%HERE%electrs-doge-kill.ps1" goto :kill_ps1
if exist "%HERE%electrs-doge.bat" goto :kill_bat
echo Stopping electrs-doge...
taskkill /F /IM electrs.exe 2>nul
echo Done.
exit /b 0
:kill_ps1
powershell -NoProfile -ExecutionPolicy Bypass -File "%HERE%electrs-doge-kill.ps1"
exit /b %ERRORLEVEL%
:kill_bat
call "%HERE%electrs-doge.bat" kill
exit /b %ERRORLEVEL%
