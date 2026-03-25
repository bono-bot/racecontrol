@echo off
:: RaceControl Agent Watchdog
:: Checks if rc-agent.exe is running; restarts via start-rcagent.bat if not.
:: Deployed as a scheduled task running every 1 minute on each pod.
:: The 3-second JS reload in the lock screen browser picks up state changes
:: automatically — no browser restart needed from the watchdog.

tasklist /NH /FI "IMAGENAME eq rc-agent.exe" 2>nul | find /i "rc-agent.exe" >nul
if errorlevel 1 goto do_restart
goto end

:do_restart
echo %DATE% %TIME% rc-agent not running -- restarting >> C:\RacingPoint\watchdog.log
call C:\RacingPoint\start-rcagent.bat

:end
