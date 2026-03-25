@echo off
REM Install racecontrol and rc-sentry as Windows Services on the server (.23)
REM Run as Administrator on the server
REM This provides Layer 1 redundancy: OS-level auto-restart on crash

echo === Racing Point Server Service Installer ===
echo.

REM --- RACECONTROL SERVICE ---
echo [1/4] Installing RaceControl service...

REM Stop and remove existing
sc.exe stop RaceControl >nul 2>&1
ping -n 3 127.0.0.1 >nul
sc.exe delete RaceControl >nul 2>&1
ping -n 2 127.0.0.1 >nul

REM Create service
sc.exe create RaceControl binPath= "C:\RacingPoint\racecontrol.exe" start= auto obj= LocalSystem DisplayName= "RaceControl Server"
if %ERRORLEVEL% neq 0 goto :rc_fail

REM Description
sc.exe description RaceControl "Racing Point racecontrol server (port 8080). Auto-restarts on crash."

REM Failure actions: restart after 5s / 10s / 30s, reset counter after 1 hour
sc.exe failure RaceControl reset= 3600 actions= restart/5000/restart/10000/restart/30000

REM Set recovery on non-crash exits too
reg add "HKLM\SYSTEM\CurrentControlSet\Services\RaceControl" /v FailureActionsOnNonCrashFailures /t REG_DWORD /d 1 /f >nul 2>&1

echo [OK] RaceControl service installed
goto :sentry

:rc_fail
echo [FAIL] Could not create RaceControl service
echo Make sure racecontrol.exe exists at C:\RacingPoint\racecontrol.exe
goto :sentry

:sentry
echo.
echo [2/4] Installing RC-Sentry service (server mode)...

REM Stop and remove existing
sc.exe stop RCSentryServer >nul 2>&1
ping -n 3 127.0.0.1 >nul
sc.exe delete RCSentryServer >nul 2>&1
ping -n 2 127.0.0.1 >nul

REM Create service — rc-sentry monitors racecontrol on the server
sc.exe create RCSentryServer binPath= "C:\RacingPoint\rc-sentry.exe" start= auto obj= LocalSystem DisplayName= "RC-Sentry Server Monitor"
if %ERRORLEVEL% neq 0 goto :sentry_fail

sc.exe description RCSentryServer "Monitors racecontrol health (port 8080) and auto-restarts on crash. Layer 2 redundancy."
sc.exe failure RCSentryServer reset= 3600 actions= restart/5000/restart/10000/restart/30000
reg add "HKLM\SYSTEM\CurrentControlSet\Services\RCSentryServer" /v FailureActionsOnNonCrashFailures /t REG_DWORD /d 1 /f >nul 2>&1

echo [OK] RCSentryServer service installed
goto :watchdog

:sentry_fail
echo [FAIL] Could not create RCSentryServer service
goto :watchdog

:watchdog
echo.
echo [3/4] Creating rc-sentry server config...

REM Create rc-sentry.toml configured for SERVER mode (monitor racecontrol, not rc-agent)
echo [sentry]> C:\RacingPoint\rc-sentry.toml
echo service_name = "racecontrol">> C:\RacingPoint\rc-sentry.toml
echo health_addr = "127.0.0.1:8080">> C:\RacingPoint\rc-sentry.toml
echo health_path = "/api/v1/health">> C:\RacingPoint\rc-sentry.toml
echo service_port = 8080>> C:\RacingPoint\rc-sentry.toml
echo process_name = "racecontrol.exe">> C:\RacingPoint\rc-sentry.toml
echo start_script = "C:\\RacingPoint\\start-racecontrol.bat">> C:\RacingPoint\rc-sentry.toml
echo service_toml = "C:\\RacingPoint\\racecontrol.toml">> C:\RacingPoint\rc-sentry.toml
echo startup_log = "C:\\RacingPoint\\racecontrol-startup.log">> C:\RacingPoint\rc-sentry.toml
echo stderr_log = "C:\\RacingPoint\\racecontrol-stderr.log">> C:\RacingPoint\rc-sentry.toml

echo [OK] rc-sentry.toml written for server mode
goto :start_services

:start_services
echo.
echo [4/4] Starting services...

REM Start rc-sentry first (it will monitor racecontrol)
sc.exe start RCSentryServer >nul 2>&1
if %ERRORLEVEL% neq 0 echo [WARN] RCSentryServer failed to start

REM Start racecontrol
sc.exe start RaceControl >nul 2>&1
if %ERRORLEVEL% neq 0 echo [WARN] RaceControl failed to start

ping -n 10 127.0.0.1 >nul

echo.
echo === Verifying ===
sc.exe query RaceControl | findstr STATE
sc.exe query RCSentryServer | findstr STATE
echo.
echo === Failure recovery config ===
sc.exe qfailure RaceControl
echo.
echo === Installation complete ===
echo.
echo Both services will auto-start on boot and auto-restart on crash.
echo Racecontrol: 5s / 10s / 30s restart delays
echo RC-Sentry: independent watchdog with health polling
echo.
echo To check status:   sc.exe query RaceControl
echo To stop:           sc.exe stop RaceControl
echo To remove:         sc.exe delete RaceControl
