@echo off
title RC-Agent Watchdog
cd /d C:\RacingPoint

:loop
echo [%date% %time%] Starting rc-agent.exe...
rc-agent.exe
echo [%date% %time%] rc-agent exited (code %ERRORLEVEL%). Restarting in 5 seconds...
timeout /t 5 /nobreak >nul
goto loop
