@echo off
REM RaceControl AC Launcher — VMS SimLauncher clone.
REM Spawned by rc-agent as an isolated subprocess.
REM Usage: launch-ac.bat <ac_dir> <mode> [server_ip] [server_http_port] [password]
REM Exit codes: 0=OK, 1=no ac_dir, 2=no acs.exe, 3=acs failed, 4=CM not found(MP), 5=CM failed(MP)

setlocal enabledelayedexpansion

set "AC_DIR=%~1"
set "MODE=%~2"
set "SERVER_IP=%~3"
set "SERVER_HTTP_PORT=%~4"
set "SERVER_PASSWORD=%~5"

if "%AC_DIR%"=="" goto :err_no_dir
if not exist "%AC_DIR%\acs.exe" goto :err_no_acs

REM Kill existing AC instances (VMS double-launch prevention)
taskkill /F /IM acs.exe >nul 2>&1
taskkill /F /IM AssettoCorsa.exe >nul 2>&1
ping -n 2 127.0.0.1 >nul

REM ─── FIND CONTENT MANAGER ────────────────────────────────────────────────
set "CM_EXE="
if exist "%AC_DIR%\Content Manager.exe" set "CM_EXE=%AC_DIR%\Content Manager.exe"
if not defined CM_EXE if exist "%USERPROFILE%\Desktop\Content Manager.exe" set "CM_EXE=%USERPROFILE%\Desktop\Content Manager.exe"
if not defined CM_EXE if exist "%USERPROFILE%\Downloads\content-manager\Content Manager.exe" set "CM_EXE=%USERPROFILE%\Downloads\content-manager\Content Manager.exe"
if not defined CM_EXE if exist "%LOCALAPPDATA%\Programs\Content Manager\Content Manager.exe" set "CM_EXE=%LOCALAPPDATA%\Programs\Content Manager\Content Manager.exe"
if not defined CM_EXE if exist "C:\content-manager\Content Manager.exe" set "CM_EXE=C:\content-manager\Content Manager.exe"

REM Route by mode
if /I "%MODE%"=="mp" goto :launch_mp

REM ─── SINGLE PLAYER ───────────────────────────────────────────────────────
:launch_sp
if not defined CM_EXE goto :sp_direct
echo Launching via Content Manager (SP): !CM_EXE!
start "" "!CM_EXE!" --race
ping -n 8 127.0.0.1 >nul
goto :verify_launch

:sp_direct
echo WARNING: Content Manager not found — launching acs.exe directly
start "" /D "%AC_DIR%" acs.exe
goto :verify_launch

REM ─── MULTIPLAYER ──────────────────────────────────────────────────────────
:launch_mp
if not defined CM_EXE goto :mp_direct
echo Launching via Content Manager (MP): !CM_EXE!
set "CM_URI=acmanager://race/online?ip=%SERVER_IP%^&httpPort=%SERVER_HTTP_PORT%"
if not "%SERVER_PASSWORD%"=="" set "CM_URI=!CM_URI!^&password=%SERVER_PASSWORD%"
start "" "!CM_URI!"
ping -n 8 127.0.0.1 >nul
goto :verify_launch

:mp_direct
echo WARNING: Content Manager not found — launching acs.exe directly for MP
start "" /D "%AC_DIR%" acs.exe
goto :verify_launch

REM ─── VERIFY ──────────────────────────────────────────────────────────────
:verify_launch
ping -n 3 127.0.0.1 >nul
tasklist /FI "IMAGENAME eq acs.exe" /FO CSV /NH 2>nul | findstr /I "acs.exe" >nul
if errorlevel 1 goto :err_acs_failed
echo AC launched successfully (mode=%MODE%)
exit /b 0

REM ─── ERRORS ──────────────────────────────────────────────────────────────
:err_no_dir
echo ERROR: AC directory not specified
exit /b 1

:err_no_acs
echo ERROR: acs.exe not found in %AC_DIR%
exit /b 2

:err_acs_failed
echo ERROR: acs.exe failed to start
exit /b 3
