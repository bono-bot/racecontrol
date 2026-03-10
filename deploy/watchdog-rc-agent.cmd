@echo off
cd /d C:\RacingPoint

:loop
tasklist /FI "IMAGENAME eq rc-agent.exe" /NH 2>nul | find /i "rc-agent.exe" >nul
if errorlevel 1 (
    if exist "C:\RacingPoint\rc-agent.exe" (
        echo [%date% %time%] rc-agent not running, starting... >> C:\RacingPoint\watchdog.log
        start "" /B C:\RacingPoint\rc-agent.exe
    ) else (
        echo [%date% %time%] rc-agent.exe missing — skipping start >> C:\RacingPoint\watchdog.log
    )
)
tasklist /FI "IMAGENAME eq pod-agent.exe" /NH 2>nul | find /i "pod-agent.exe" >nul
if errorlevel 1 (
    if exist "C:\RacingPoint\pod-agent.exe" (
        echo [%date% %time%] pod-agent not running, starting... >> C:\RacingPoint\watchdog.log
        start "" /B C:\RacingPoint\pod-agent.exe
    )
)
timeout /t 30 /nobreak >nul
goto loop
