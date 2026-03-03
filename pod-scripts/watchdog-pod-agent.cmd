@echo off
title Pod-Agent Watchdog
cd /d C:\RacingPoint

:loop
echo [%date% %time%] Starting pod-agent.exe...
pod-agent.exe
echo [%date% %time%] pod-agent exited (code %ERRORLEVEL%). Restarting in 5 seconds...
timeout /t 5 /nobreak >nul
goto loop
