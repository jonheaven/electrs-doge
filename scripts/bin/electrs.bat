@echo off
REM Short alias: electrs compile / launch / kill -> electrs-doge.bat
REM No unescaped parentheses: cmd eats them when this bat is CALLed from a parenthesized block.
if not exist "%~dp0electrs-doge.bat" goto :missing
call "%~dp0electrs-doge.bat" %*
exit /b %ERRORLEVEL%
:missing
echo Missing %~dp0electrs-doge.bat
exit /b 1
