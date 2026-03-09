@echo off
cd /d C:\RacingPoint

:loop
tasklist /FI "IMAGENAME eq rc-agent.exe" /NH 2>nul | find /i "rc-agent.exe" >nul
if errorlevel 1 (
    echo [%date% %time%] rc-agent not running, starting... >> C:\RacingPoint\watchdog.log
    start "" C:\RacingPoint\rc-agent.exe
)
timeout /t 30 /nobreak >nul
goto loop
