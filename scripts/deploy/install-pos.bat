@echo off
REM ============================================================
REM  POS PC Setup - SSH + Tailscale + rc-agent + Chrome kiosk + watchdog
REM  Run as Administrator from pendrive
REM  Usage: Right-click > Run as administrator
REM
REM  Source of truth: racecontrol/scripts/deploy/install-pos.bat
REM  Mirror copy:     deploy-staging/pos-deploy/install-pos.bat
REM
REM  2026-04-20 - Added STEP 5 (Chrome watchdog + POS-Watchdog schtask)
REM  and STEP 6 (HKLM Run BillingDashboard -> Chrome kiosk) so a fresh
REM  re-image no longer regresses to the Edge kiosk orphan bug (POS-03).
REM  Required pendrive payload: watchdog-pos.ps1 (Chrome variant),
REM  register-pos-watchdog.ps1, rc-agent.exe, tailscale-setup.exe.
REM ============================================================

echo ============================================================
echo   Racing Point - POS PC Setup
echo   Installs: OpenSSH, Tailscale, rc-agent, Chrome watchdog
echo   MUST be run as Administrator
echo ============================================================
echo.

REM --- Check admin ---
net session >nul 2>nul
if %ERRORLEVEL% NEQ 0 goto :NOT_ADMIN

REM --- Detect drive letter ---
set "SCRIPT_DIR=%~dp0"
echo [INFO] Running from: %SCRIPT_DIR%
echo.

REM ============================================================
REM  STEP 1: Enable OpenSSH Server
REM ============================================================
echo [STEP 1/7] Installing OpenSSH Server...

sc query sshd >nul 2>nul
if %ERRORLEVEL% EQU 0 goto :SSH_EXISTS

dism /Online /Add-Capability /CapabilityName:OpenSSH.Server~~~~0.0.1.0
if %ERRORLEVEL% NEQ 0 goto :SSH_DISM_FAIL

:SSH_EXISTS
echo [INFO] OpenSSH Server is installed

sc config sshd start=auto >nul 2>nul
net start sshd >nul 2>nul
echo [INFO] SSH service started and set to auto-start

netsh advfirewall firewall add rule name="OpenSSH-Server" dir=in action=allow protocol=TCP localport=22 >nul 2>nul
echo [INFO] Firewall rule added for port 22

REM ============================================================
REM  STEP 2: Deploy SSH authorized key (James)
REM ============================================================
echo.
echo [STEP 2/7] Setting up SSH key auth for James...

if not exist "C:\Users\%USERNAME%\.ssh" mkdir "C:\Users\%USERNAME%\.ssh"

echo ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGpwLi/oX9iymSjea6I3iG6QUQmX9XsJ0fDma/3MTLQ/ james@racingpoint.in> "C:\Users\%USERNAME%\.ssh\authorized_keys"

if not exist "C:\ProgramData\ssh" mkdir "C:\ProgramData\ssh"
echo ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGpwLi/oX9iymSjea6I3iG6QUQmX9XsJ0fDma/3MTLQ/ james@racingpoint.in> "C:\ProgramData\ssh\administrators_authorized_keys"

icacls "C:\ProgramData\ssh\administrators_authorized_keys" /inheritance:r /grant "SYSTEM:F" /grant "Administrators:F" >nul 2>nul

reg add "HKLM\SOFTWARE\OpenSSH" /v DefaultShell /t REG_SZ /d "C:\Windows\System32\cmd.exe" /f >nul 2>nul

net stop sshd >nul 2>nul
net start sshd >nul 2>nul
echo [INFO] SSH key deployed, service restarted

REM ============================================================
REM  STEP 3: Install Tailscale
REM ============================================================
echo.
echo [STEP 3/7] Installing Tailscale...

where tailscale >nul 2>nul
if %ERRORLEVEL% EQU 0 goto :TS_EXISTS

if exist "%SCRIPT_DIR%tailscale-setup.exe" goto :TS_INSTALL
echo [WARN] tailscale-setup.exe not found on pendrive
echo [WARN] Download from https://tailscale.com/download/windows
echo [WARN] Place tailscale-setup.exe next to this script and re-run
goto :TS_SKIP

:TS_INSTALL
echo [INFO] Installing Tailscale from pendrive...
"%SCRIPT_DIR%tailscale-setup.exe" /quiet /norestart
echo [INFO] Tailscale installed. Run 'tailscale up' after reboot to authenticate.
goto :TS_DONE

:TS_EXISTS
echo [INFO] Tailscale already installed

:TS_DONE
net start Tailscale >nul 2>nul
echo [INFO] Tailscale service started
echo [NOTE] After script completes, run: tailscale up
echo         Then authenticate in browser to join the tailnet

:TS_SKIP

REM ============================================================
REM  STEP 4: Deploy rc-agent
REM ============================================================
echo.
echo [STEP 4/7] Deploying rc-agent...

if not exist "C:\RacingPoint" mkdir "C:\RacingPoint"

if not exist "%SCRIPT_DIR%rc-agent.exe" goto :NO_AGENT
copy /Y "%SCRIPT_DIR%rc-agent.exe" "C:\RacingPoint\rc-agent.exe" >nul
echo [INFO] rc-agent.exe copied to C:\RacingPoint\

echo [server]> "C:\RacingPoint\rc-agent.toml"
echo ws_url = "ws://192.168.31.23:8080/ws/agent">> "C:\RacingPoint\rc-agent.toml"
echo http_port = 8090>> "C:\RacingPoint\rc-agent.toml"
echo pod_id = "pos_1">> "C:\RacingPoint\rc-agent.toml"
echo pod_number = 0>> "C:\RacingPoint\rc-agent.toml"
echo pod_name = "POS 1">> "C:\RacingPoint\rc-agent.toml"
echo.>> "C:\RacingPoint\rc-agent.toml"
echo [agent]>> "C:\RacingPoint\rc-agent.toml"
echo role = "pos">> "C:\RacingPoint\rc-agent.toml"
echo.>> "C:\RacingPoint\rc-agent.toml"
echo [service]>> "C:\RacingPoint\rc-agent.toml"
echo terminal_secret = "rp-terminal-2026">> "C:\RacingPoint\rc-agent.toml"
echo [INFO] rc-agent.toml created

echo @echo off> "C:\RacingPoint\start-rcagent.bat"
echo REM Start rc-agent on POS PC>> "C:\RacingPoint\start-rcagent.bat"
echo cd /d C:\RacingPoint>> "C:\RacingPoint\start-rcagent.bat"
echo rc-agent.exe>> "C:\RacingPoint\start-rcagent.bat"

reg add "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" /v RCAgent /t REG_SZ /d "C:\RacingPoint\start-rcagent.bat" /f >nul 2>nul
echo [INFO] rc-agent registered for auto-start

start "" /min "C:\RacingPoint\rc-agent.exe"
echo [INFO] rc-agent started
goto :AGENT_DONE

:NO_AGENT
echo [WARN] rc-agent.exe not found on pendrive
echo [WARN] Place rc-agent.exe next to this script and re-run

:AGENT_DONE

REM ============================================================
REM  STEP 5: Deploy Chrome POS-Watchdog (monitors Chrome kiosk + rc-agent)
REM ============================================================
echo.
echo [STEP 5/7] Deploying POS-Watchdog (Chrome-aware)...

if not exist "%SCRIPT_DIR%watchdog-pos.ps1" goto :NO_WATCHDOG
copy /Y "%SCRIPT_DIR%watchdog-pos.ps1" "C:\RacingPoint\watchdog-pos.ps1" >nul
echo [INFO] watchdog-pos.ps1 copied to C:\RacingPoint\

if not exist "%SCRIPT_DIR%register-pos-watchdog.ps1" goto :NO_REGISTRAR
powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%register-pos-watchdog.ps1"
if %ERRORLEVEL% NEQ 0 goto :WATCHDOG_REG_FAIL
echo [INFO] POS-Watchdog scheduled task registered (every 2 min, Session 1)
goto :WATCHDOG_DONE

:NO_WATCHDOG
echo [WARN] watchdog-pos.ps1 not found on pendrive - POS will lack Chrome kiosk watchdog
echo [WARN] Refresh from racecontrol/scripts/deploy/pos-watchdog.ps1 and re-run
goto :WATCHDOG_DONE

:NO_REGISTRAR
echo [WARN] register-pos-watchdog.ps1 not found on pendrive - skipping schtask registration
echo [WARN] Refresh from racecontrol/scripts/deploy/register-pos-watchdog.ps1 and re-run
goto :WATCHDOG_DONE

:WATCHDOG_REG_FAIL
echo [WARN] POS-Watchdog registration failed (see PowerShell output above)

:WATCHDOG_DONE

REM ============================================================
REM  STEP 6: Register Chrome kiosk HKLM Run (BillingDashboard)
REM ============================================================
echo.
echo [STEP 6/7] Registering Chrome kiosk autostart (HKLM Run BillingDashboard)...

set "CHROME_EXE=C:\Program Files\Google\Chrome\Application\chrome.exe"
if not exist "%CHROME_EXE%" goto :CHROME_MISSING

REM Build a .reg file in TEMP (avoids cmd.exe quoting hell).
REM .reg format escapes inner quotes with \" and backslashes with \\.
set "REGFILE=%TEMP%\billing-dashboard.reg"
echo Windows Registry Editor Version 5.00> "%REGFILE%"
echo.>> "%REGFILE%"
echo [HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Run]>> "%REGFILE%"
echo "BillingDashboard"="\"C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe\" --kiosk http://192.168.31.23:3200/billing --no-first-run --remote-debugging-port=9222 --user-data-dir=C:\\RacingPoint\\chrome-kiosk-profile">> "%REGFILE%"

reg import "%REGFILE%" >nul 2>nul
if %ERRORLEVEL% EQU 0 goto :CHROME_OK

echo [WARN] Failed to import BillingDashboard reg entry
goto :CHROME_DONE

:CHROME_OK
echo [INFO] HKLM Run BillingDashboard registered (Chrome kiosk -^> :3200/billing)
goto :CHROME_DONE

:CHROME_MISSING
echo [WARN] Chrome not found at %CHROME_EXE%
echo [WARN] Install Chrome and re-run this script, or set HKLM Run BillingDashboard manually

:CHROME_DONE

REM ============================================================
REM  STEP 7: Reboot
REM ============================================================
echo.
echo [STEP 7/7] Setup complete!
echo.
echo   SSH:              INSTALLED (port 22, key auth for james@racingpoint.in)
echo   Tailscale:        CHECK ABOVE (run 'tailscale up' after reboot)
echo   rc-agent:         CHECK ABOVE (auto-starts on boot)
echo   POS-Watchdog:     CHECK ABOVE (runs every 2 min in user session)
echo   Chrome kiosk:     CHECK ABOVE (HKLM Run BillingDashboard -^> :3200/billing)
echo.
echo   The PC will reboot in 30 seconds to apply all changes.
echo   Press Ctrl+C to cancel reboot.
echo.
shutdown /r /t 30 /c "Racing Point POS setup complete - rebooting"
goto :EOF

:NOT_ADMIN
echo.
echo [ERROR] This script must be run as Administrator!
echo         Right-click the script and select "Run as administrator"
echo.
pause
goto :EOF

:SSH_DISM_FAIL
echo [ERROR] Failed to install OpenSSH via DISM
echo         Try manually: Settings ^> Apps ^> Optional Features ^> OpenSSH Server
pause
goto :EOF
