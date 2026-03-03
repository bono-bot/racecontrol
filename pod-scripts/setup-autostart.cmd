@echo off
REM Install auto-start scheduled tasks for pod-agent and rc-agent watchdogs.
REM Run this ON each pod (as administrator) to set up boot-time auto-start.
REM Prerequisites: watchdog-pod-agent.cmd and watchdog-rc-agent.cmd must exist in C:\RacingPoint\

cd /d C:\RacingPoint

REM Create scheduled task for pod-agent watchdog (runs at system startup)
schtasks /Create /TN "RacingPoint\PodAgentWatchdog" /TR "cmd /c C:\RacingPoint\watchdog-pod-agent.cmd" /SC ONSTART /RU SYSTEM /F
echo Pod-agent watchdog scheduled task created.

REM Create scheduled task for rc-agent watchdog (runs at system startup, 10s delay)
schtasks /Create /TN "RacingPoint\RcAgentWatchdog" /TR "cmd /c C:\RacingPoint\watchdog-rc-agent.cmd" /SC ONSTART /RU SYSTEM /DELAY 0000:10 /F
echo RC-agent watchdog scheduled task created.

echo.
echo Done! Both agents will auto-start on boot.
echo To verify: schtasks /Query /TN "RacingPoint\PodAgentWatchdog"
echo            schtasks /Query /TN "RacingPoint\RcAgentWatchdog"
pause
