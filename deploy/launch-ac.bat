@echo off
REM RaceControl AC Launcher — separate process for game launch isolation.
REM Spawned by rc-agent as a disposable subprocess. If this process is killed
REM (by CSP, Steam, anti-cheat), rc-agent survives and can detect the failure.
REM
REM Usage: launch-ac.bat <ac_dir> <mode> [server_ip] [server_port]
REM   mode: sp (single player) or mp (multiplayer)
REM
REM VMS Pattern: SimLauncher.exe is a separate transient process that exits
REM after AC is running. We do the same — this bat launches AC and exits.

setlocal enabledelayedexpansion

set AC_DIR=%~1
set MODE=%~2
set SERVER_IP=%~3
set SERVER_PORT=%~4

if "%AC_DIR%"=="" (
    echo ERROR: AC directory not specified
    exit /b 1
)

if not exist "%AC_DIR%\acs.exe" (
    echo ERROR: acs.exe not found in %AC_DIR%
    exit /b 2
)

REM Kill any existing AC instances first (VMS pattern: double-launch prevention)
taskkill /F /IM acs.exe >nul 2>&1
taskkill /F /IM AssettoCorsa.exe >nul 2>&1

REM Brief wait for process cleanup
ping -n 2 127.0.0.1 >nul

REM Launch AC — use start /B so this bat can exit independently
echo Launching acs.exe from %AC_DIR%...
start "" /D "%AC_DIR%" acs.exe

REM Wait briefly to confirm process started
ping -n 3 127.0.0.1 >nul

REM Check if acs.exe is running
tasklist /FI "IMAGENAME eq acs.exe" /FO CSV /NH | findstr /I "acs.exe" >nul
if errorlevel 1 (
    echo ERROR: acs.exe failed to start
    exit /b 3
)

echo AC launched successfully
exit /b 0
