@echo off
REM RaceControl AC Launcher — VMS SimLauncher clone.
REM
REM This is a SEPARATE DISPOSABLE PROCESS, exactly like VMS's SimLauncher.exe.
REM Spawned by rc-agent as an isolated subprocess. If this process is killed
REM (by CSP, Steam, anti-cheat), rc-agent survives and detects the failure.
REM
REM KEY VMS PATTERN: cmd.exe becomes the parent of acs.exe, NOT rc-agent.
REM If acs.exe crashes, the crash handler sees cmd.exe as parent — not rc-agent.
REM This prevents anti-cheat from flagging rc-agent as the launcher.
REM
REM Usage: launch-ac.bat <ac_dir> <mode> [server_ip] [server_http_port] [password]
REM   mode: sp (single player) or mp (multiplayer)
REM
REM Exit codes:
REM   0 = AC launched successfully
REM   1 = AC directory not specified
REM   2 = acs.exe not found
REM   3 = acs.exe failed to start
REM   4 = Content Manager not found (MP only)
REM   5 = Content Manager launch failed (MP only)

setlocal enabledelayedexpansion

set AC_DIR=%~1
set MODE=%~2
set SERVER_IP=%~3
set SERVER_HTTP_PORT=%~4
set SERVER_PASSWORD=%~5

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

REM Brief wait for process cleanup (VMS uses similar delay)
ping -n 2 127.0.0.1 >nul

REM ─── MULTIPLAYER MODE ─────────────────────────────────────────────────────
if /I "%MODE%"=="mp" goto :launch_mp

REM ─── FIND CONTENT MANAGER ──────────────────────────────────────────────────
REM CM is required for CSP integration. Search known locations on pods.
set CM_EXE=
if exist "%AC_DIR%\Content Manager.exe" set "CM_EXE=%AC_DIR%\Content Manager.exe"
if "%CM_EXE%"=="" if exist "%USERPROFILE%\Desktop\Content Manager.exe" set "CM_EXE=%USERPROFILE%\Desktop\Content Manager.exe"
if "%CM_EXE%"=="" if exist "%USERPROFILE%\Downloads\content-manager\Content Manager.exe" set "CM_EXE=%USERPROFILE%\Downloads\content-manager\Content Manager.exe"
if "%CM_EXE%"=="" if exist "%LOCALAPPDATA%\Programs\Content Manager\Content Manager.exe" set "CM_EXE=%LOCALAPPDATA%\Programs\Content Manager\Content Manager.exe"
if "%CM_EXE%"=="" if exist "C:\content-manager\Content Manager.exe" set "CM_EXE=C:\content-manager\Content Manager.exe"

REM ─── SINGLE PLAYER MODE ───────────────────────────────────────────────────
if /I "%MODE%"=="mp" goto :launch_mp

:launch_sp
if "%CM_EXE%"=="" goto :sp_direct

REM Launch via Content Manager — race.ini already written by rc-agent
echo Launching via Content Manager (SP): %CM_EXE%
start "" "%CM_EXE%" --race
REM CM takes a few seconds to parse config and launch acs.exe
ping -n 8 127.0.0.1 >nul
goto :verify_launch

:sp_direct
REM Fallback: direct acs.exe (no CM — CSP may not initialize properly)
echo WARNING: Content Manager not found — launching acs.exe directly (CSP may not work)
start "" /D "%AC_DIR%" acs.exe
goto :verify_launch

REM ─── MULTIPLAYER MODE ─────────────────────────────────────────────────────
:launch_mp
if "%CM_EXE%"=="" goto :mp_direct

echo Launching via Content Manager (MP): %CM_EXE%

REM Build CM URI for multiplayer
set CM_URI=acmanager://race/online?ip=%SERVER_IP%^&httpPort=%SERVER_HTTP_PORT%
if not "%SERVER_PASSWORD%"=="" set CM_URI=%CM_URI%^&password=%SERVER_PASSWORD%

start "" "%CM_URI%"
REM Wait longer for CM to load and launch AC
ping -n 8 127.0.0.1 >nul
goto :verify_launch

REM Fallback: direct acs.exe with [REMOTE] ACTIVE=1 in race.ini
:mp_direct
echo WARNING: Content Manager not found — launching acs.exe directly for multiplayer
start "" /D "%AC_DIR%" acs.exe
goto :verify_launch

REM ─── VERIFY LAUNCH ────────────────────────────────────────────────────────
:verify_launch
REM Wait for AC to start
ping -n 3 127.0.0.1 >nul

REM Check if acs.exe is running
tasklist /FI "IMAGENAME eq acs.exe" /FO CSV /NH | findstr /I "acs.exe" >nul
if errorlevel 1 (
    echo ERROR: acs.exe failed to start
    exit /b 3
)

echo AC launched successfully (mode=%MODE%)
REM VMS pattern: launcher EXITS after game is running (transient, like SimLauncher)
exit /b 0
