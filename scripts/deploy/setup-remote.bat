@echo off
REM ============================================================
REM  Racing Point Pod Remote Access — Enables WinRM
REM
REM  Double-click on any pod to enable remote management.
REM  Works on LAN without internet.
REM
REM  After this, run commands from James:
REM    powershell Invoke-Command -ComputerName <pod-ip> -ScriptBlock { hostname }
REM
REM  LINTER WARNING: This is a Windows batch file.
REM  All null redirects use %NUL% variable. DO NOT replace
REM  with /dev/null — that is Linux syntax and WILL break.
REM ============================================================

REM ── Auto-elevate to Administrator if needed ─────────────────
fsutil dirty query %SYSTEMDRIVE% >nul 2>&1
if ERRORLEVEL 1 (
    echo Requesting Administrator privileges...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

setlocal EnableDelayedExpansion
title Racing Point Remote Access Setup

REM Linter-proof null device
set NUL=nul

echo.
echo ========================================
echo   Racing Point Remote Access Setup
echo ========================================
echo.

REM ── Step 1: Set network profile to Private ─────────────────
echo [1/4] Setting network to Private...
powershell -NoProfile -Command "Get-NetConnectionProfile | Set-NetConnectionProfile -NetworkCategory Private" 1>%NUL% 2>%NUL%
echo    Network profile set to Private

REM ── Step 2: Enable and configure WinRM ─────────────────────
echo [2/4] Enabling WinRM...
winrm quickconfig -quiet 1>%NUL% 2>%NUL%
winrm set winrm/config/service @{AllowUnencrypted="true"} 1>%NUL% 2>%NUL%
winrm set winrm/config/service/auth @{Basic="true"} 1>%NUL% 2>%NUL%
sc config WinRM start=auto 1>%NUL%
net start WinRM 1>%NUL% 2>%NUL%
echo    WinRM enabled and set to auto-start

REM ── Step 3: Firewall rule ──────────────────────────────────
echo [3/4] Setting firewall rule...
netsh advfirewall firewall add rule name="WinRM-HTTP" dir=in action=allow protocol=TCP localport=5985 1>%NUL% 2>%NUL%
echo    Firewall rule added for port 5985

REM ── Step 4: Verify ─────────────────────────────────────────
echo [4/4] Verifying...
set PROBLEMS=0

sc query WinRM | findstr /I "RUNNING" 1>%NUL% 2>%NUL%
if ERRORLEVEL 1 (
    echo    [FAIL] WinRM not running
    set /a PROBLEMS+=1
) else (
    echo    [OK]   WinRM is running
)

netsh advfirewall firewall show rule name="WinRM-HTTP" 1>%NUL% 2>%NUL%
if ERRORLEVEL 1 (
    echo    [FAIL] Firewall rule missing
    set /a PROBLEMS+=1
) else (
    echo    [OK]   Firewall rule present
)

echo.
if !PROBLEMS! NEQ 0 (
    echo ========================================
    echo   WARNING: !PROBLEMS! issues found
    echo   Remote access may not work.
    echo ========================================
) else (
    echo ========================================
    echo   REMOTE ACCESS READY
    echo   Pod IP: check with ipconfig
    echo   Port: 5985 (WinRM)
    echo ========================================
)
echo.
pause
