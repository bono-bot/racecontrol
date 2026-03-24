@echo off
REM ============================================================
REM  Racing Point Pod SSH Setup — OpenSSH with key-based auth
REM
REM  Double-click on any pod (auto-elevates to admin).
REM  Works without internet (OpenSSH is built into Windows 11).
REM
REM  After this: ssh bono@<pod-ip>  (no password needed)
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
title Racing Point SSH Setup

REM Linter-proof null device
set NUL=nul

echo.
echo ========================================
echo   Racing Point SSH Setup
echo ========================================
echo.

REM ── Step 1: Check if OpenSSH Server is already installed ────
echo [1/6] Checking OpenSSH Server status...
sc query sshd 1>%NUL% 2>%NUL%
if !ERRORLEVEL!==0 (
    echo    OpenSSH Server already installed
    goto :configure_sshd
)

REM ── Step 2: Install OpenSSH Server ─────────────────────────
echo [2/6] Installing OpenSSH Server...
echo    This may take 30-60 seconds...

REM Try PowerShell first (works on Win11 with or without internet)
powershell -NoProfile -Command "Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0 -ErrorAction Stop" 1>%NUL% 2>%NUL%
if !ERRORLEVEL!==0 (
    echo    Installed via Windows capability
    goto :verify_install
)

REM Fallback: DISM (works offline from component store)
echo    PowerShell method failed, trying DISM...
dism /Online /Add-Capability /CapabilityName:OpenSSH.Server~~~~0.0.1.0 1>%NUL% 2>%NUL%
if !ERRORLEVEL!==0 (
    echo    Installed via DISM
    goto :verify_install
)

REM Both methods failed
echo    FAILED: Could not install OpenSSH Server
echo    This pod may need internet for first-time install.
echo    Try connecting to WiFi and run this again.
echo.
pause
exit /b 1

:verify_install
REM Verify the service exists now
sc query sshd 1>%NUL% 2>%NUL%
if ERRORLEVEL 1 (
    echo    FAILED: sshd service not found after install
    echo    Try rebooting the pod and running this again.
    echo.
    pause
    exit /b 1
)
echo    sshd service verified

:configure_sshd
REM ── Step 3: Deploy sshd_config (no admin key override) ─────
echo [3/6] Writing sshd_config...

set SSHD_CONFIG=C:\ProgramData\ssh\sshd_config
if not exist "C:\ProgramData\ssh" mkdir "C:\ProgramData\ssh" 2>%NUL%

REM Stop sshd before writing config
net stop sshd 1>%NUL% 2>%NUL%

REM Write clean config — NO Match Group administrators block
REM This means ALL users (including admins) use ~/.ssh/authorized_keys
(
echo # Racing Point Pod SSH Config
echo Port 22
echo AddressFamily any
echo ListenAddress 0.0.0.0
echo ListenAddress ::
echo.
echo PubkeyAuthentication yes
echo AuthorizedKeysFile .ssh/authorized_keys
echo PasswordAuthentication yes
echo PermitEmptyPasswords no
echo.
echo Subsystem sftp sftp-server.exe
) > "%SSHD_CONFIG%"

echo    Config written (admin key override REMOVED)

REM Set sshd to auto-start
sc config sshd start=auto 1>%NUL%
echo    Set sshd to auto-start on boot

REM ── Step 4: Deploy James's SSH public key ──────────────────
echo [4/6] Deploying James's SSH key...

set USER_SSH=C:\Users\bono\.ssh
if not exist "%USER_SSH%" mkdir "%USER_SSH%" 2>%NUL%

REM Use PowerShell to write the key cleanly (no trailing spaces from batch echo)
powershell -NoProfile -Command "Set-Content -Path '%USER_SSH%\authorized_keys' -Value 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGpwLi/oX9iymSjea6I3iG6QUQmX9XsJ0fDma/3MTLQ/ james@racingpoint.in' -Encoding ASCII -NoNewline"
echo    Key written to C:\Users\bono\.ssh\authorized_keys

REM ── Step 5: Firewall rule ──────────────────────────────────
echo [5/6] Setting firewall rule for port 22...
netsh advfirewall firewall delete rule name="OpenSSH-Server-In-TCP" 1>%NUL% 2>%NUL%
netsh advfirewall firewall add rule name="OpenSSH-Server-In-TCP" dir=in action=allow protocol=TCP localport=22 1>%NUL%
echo    Firewall rule added

REM ── Step 6: Set default shell + start sshd ─────────────────
echo [6/6] Final setup...
reg add "HKLM\SOFTWARE\OpenSSH" /v DefaultShell /d "C:\Windows\System32\cmd.exe" /f 1>%NUL%
echo    Default shell: cmd.exe

timeout /t 2 /nobreak 1>%NUL%
net start sshd 1>%NUL% 2>%NUL%
if ERRORLEVEL 1 (
    echo    WARNING: Could not start sshd — will start on reboot
) else (
    echo    sshd started
)

REM ── Verify ─────────────────────────────────────────────────
echo.
echo ── Verification ──
set PROBLEMS=0

sc query sshd | findstr /I "RUNNING" 1>%NUL% 2>%NUL%
if ERRORLEVEL 1 (
    echo    [WARN] sshd not running — will start on reboot
) else (
    echo    [OK]   sshd is running
)

netsh advfirewall firewall show rule name="OpenSSH-Server-In-TCP" 1>%NUL% 2>%NUL%
if ERRORLEVEL 1 (
    echo    [FAIL] Firewall rule missing
    set /a PROBLEMS+=1
) else (
    echo    [OK]   Firewall rule present
)

sc qc sshd | findstr /I "AUTO_START" 1>%NUL% 2>%NUL%
if ERRORLEVEL 1 (
    echo    [FAIL] sshd not set to auto-start
    set /a PROBLEMS+=1
) else (
    echo    [OK]   sshd set to auto-start
)

if exist "C:\Users\bono\.ssh\authorized_keys" (
    echo    [OK]   authorized_keys deployed
) else (
    echo    [FAIL] authorized_keys missing
    set /a PROBLEMS+=1
)

echo.
if !PROBLEMS! NEQ 0 (
    echo ========================================
    echo   WARNING: !PROBLEMS! issues found
    echo   SSH may not work until reboot.
    echo ========================================
) else (
    echo ========================================
    echo   SSH READY — Key-based auth enabled
    echo   Connect: ssh bono@[pod-ip]
    echo   No password needed from James ^(.27^)
    echo ========================================
)
echo.
pause
