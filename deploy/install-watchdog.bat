@echo off
REM Install rc-watchdog as Windows SYSTEM service
REM Run as Administrator on each pod
REM Usage: install-watchdog.bat

echo === RCWatchdog Service Installer ===

REM Stop existing service if running
sc.exe stop RCWatchdog >nul 2>&1
timeout /t 2 /nobreak >nul

REM Delete existing service if registered
sc.exe delete RCWatchdog >nul 2>&1
timeout /t 1 /nobreak >nul

REM Register service
sc.exe create RCWatchdog binPath= "C:\RacingPoint\rc-watchdog.exe" start= auto obj= LocalSystem DisplayName= "RaceControl Watchdog"
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to create service
    exit /b 1
)

REM Set description
sc.exe description RCWatchdog "Monitors rc-agent and restarts it in Session 1 after crashes"

REM Set failure actions: restart after 5s / 10s / 30s, reset counter after 1 hour
sc.exe failure RCWatchdog reset= 3600 actions= restart/5000/restart/10000/restart/30000

REM Start the service
sc.exe start RCWatchdog
if %ERRORLEVEL% neq 0 (
    echo WARNING: Service created but failed to start (may need rc-watchdog.exe at C:\RacingPoint\)
) else (
    echo Service started successfully
)

REM Verify
sc.exe query RCWatchdog
echo.
echo === Installation complete ===
echo Verify with: sc.exe query RCWatchdog
echo Check failure config: sc.exe qfailure RCWatchdog
