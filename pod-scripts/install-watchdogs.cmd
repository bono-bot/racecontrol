@echo off
echo Installing watchdog services for RacingPoint...

:: Copy watchdog scripts
copy /y "%~dp0watchdog-rc-agent.cmd" "C:\RacingPoint\watchdog-rc-agent.cmd"
copy /y "%~dp0watchdog-pod-agent.cmd" "C:\RacingPoint\watchdog-pod-agent.cmd"

:: Create startup tasks (run at logon, auto-restart)
schtasks /create /tn "RacingPoint\PodAgent" /tr "C:\RacingPoint\watchdog-pod-agent.cmd" /sc onlogon /rl highest /f
schtasks /create /tn "RacingPoint\RcAgent" /tr "C:\RacingPoint\watchdog-rc-agent.cmd" /sc onlogon /rl highest /f

echo.
echo Done! Watchdogs will auto-start on login and restart on crash.
echo To start now, run:
echo   start "" "C:\RacingPoint\watchdog-pod-agent.cmd"
echo   start "" "C:\RacingPoint\watchdog-rc-agent.cmd"
pause
