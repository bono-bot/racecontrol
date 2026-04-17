@echo off
REM racecontrol-watchdog.bat
REM Simple watchdog: checks if racecontrol.exe is running, restarts if dead.
REM Runs via Task Scheduler every 2 minutes on the server (.23).
REM Layer 1 redundancy - no dependencies on PowerShell or Rust services.

cd /d C:\RacingPoint

REM Check OTA sentinel - do not restart during OTA
if exist OTA_DEPLOYING goto :ota_skip

REM Check maintenance mode sentinel (with 30min TTL auto-clear)
if not exist MAINTENANCE_MODE goto :check_running
powershell -NoProfile -Command "exit ([int](((Get-Date) - (Get-Item 'C:\RacingPoint\MAINTENANCE_MODE').LastWriteTime).TotalMinutes -ge 30))" 1>nul 2>nul
if %ERRORLEVEL% equ 1 goto :maintenance_ttl_clear
goto :maintenance_skip

:maintenance_ttl_clear
del /Q MAINTENANCE_MODE 1>nul 2>nul
echo %date% %time% WATCHDOG: MAINTENANCE_MODE stale ^>30min - TTL cleared, proceeding >> C:\RacingPoint\racecontrol-watchdog.log

:check_running
REM Check if racecontrol.exe is running
tasklist /FI "IMAGENAME eq racecontrol.exe" 2>nul | find /i "racecontrol.exe" >nul
if %ERRORLEVEL% equ 0 goto :running

REM racecontrol is NOT running - try to restart
echo %date% %time% WATCHDOG: racecontrol.exe not running - restarting >> C:\RacingPoint\racecontrol-watchdog.log

REM Try schtasks first (most reliable)
schtasks /Run /TN StartRCDirect >nul 2>&1
if %ERRORLEVEL% equ 0 goto :restarted

REM Fallback: direct start
echo %date% %time% WATCHDOG: schtasks failed, direct start >> C:\RacingPoint\racecontrol-watchdog.log
start /D C:\RacingPoint "" C:\RacingPoint\racecontrol.exe

:restarted
echo %date% %time% WATCHDOG: restart command sent >> C:\RacingPoint\racecontrol-watchdog.log

REM Wait 10 seconds then verify health
ping -n 11 127.0.0.1 >nul
curl -s --connect-timeout 5 http://127.0.0.1:8080/api/v1/health >nul 2>&1
if %ERRORLEVEL% equ 0 goto :verified_ok
echo %date% %time% WATCHDOG: health check failed after restart >> C:\RacingPoint\racecontrol-watchdog.log
goto :end

:verified_ok
echo %date% %time% WATCHDOG: restart SUCCESS - health OK >> C:\RacingPoint\racecontrol-watchdog.log
goto :end

:running
REM racecontrol is running - also verify health endpoint
curl -s --connect-timeout 5 http://127.0.0.1:8080/api/v1/health >nul 2>&1
if %ERRORLEVEL% equ 0 goto :end
REM Process running but health failing - log it (don't restart yet, let next cycle check)
echo %date% %time% WATCHDOG: process alive but health endpoint unreachable >> C:\RacingPoint\racecontrol-watchdog.log
goto :end

:ota_skip
echo %date% %time% WATCHDOG: OTA in progress - skipping >> C:\RacingPoint\racecontrol-watchdog.log
goto :end

:maintenance_skip
echo %date% %time% WATCHDOG: MAINTENANCE_MODE active (^<30min) - skipping >> C:\RacingPoint\racecontrol-watchdog.log
goto :end

:end
