@echo off
echo Stopping electrs-doge...
taskkill /F /IM electrs.exe 2>nul
for /f "tokens=2" %%p in ('netstat -ano ^| findstr ":50001 " ^| findstr LISTENING') do taskkill /F /PID %%p 2>nul
for /f "tokens=2" %%p in ('netstat -ano ^| findstr ":3000 " ^| findstr LISTENING') do taskkill /F /PID %%p 2>nul
echo Done.
exit /b 0
