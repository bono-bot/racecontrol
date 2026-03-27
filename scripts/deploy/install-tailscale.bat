@echo off
REM RacingPoint - Tailscale install for pods (run from pendrive as Admin)
REM Usage: install-tailscale.bat <pod_number>

if "%TAILSCALE_AUTH_KEY%"=="" (
    echo ERROR: TAILSCALE_AUTH_KEY environment variable is not set.
    echo Usage: set TAILSCALE_AUTH_KEY=tskey-auth-... ^& install-tailscale.bat 1
    pause
    exit /b 1
)
set PREAUTH_KEY=%TAILSCALE_AUTH_KEY%
set TS_EXE=C:\Program Files\Tailscale\tailscale.exe

if "%1"=="" (
    echo ERROR: Missing pod number. Usage: install-tailscale.bat 1
    pause
    exit /b 1
)

set POD_NUM=%1
set HOSTNAME=racing-pod-%POD_NUM%
set MSI=%~dp0tailscale-setup-latest-amd64.msi

echo === Tailscale install for Pod %POD_NUM% ===

if not exist "%MSI%" (
    echo ERROR: tailscale-setup-latest-amd64.msi not found at %MSI%
    pause
    exit /b 1
)

echo [1/3] Installing Tailscale...
msiexec /i "%MSI%" /quiet /norestart /wait
if %errorlevel% neq 0 (
    echo ERROR: msiexec failed with code %errorlevel%
    pause
    exit /b %errorlevel%
)

if not exist "%TS_EXE%" (
    echo ERROR: tailscale.exe not found after install - install may have failed
    pause
    exit /b 1
)
echo Install OK

echo [2/3] Joining tailnet as %HOSTNAME%...
"%TS_EXE%" up --unattended --auth-key=%PREAUTH_KEY% --hostname=%HOSTNAME% --reset
if %errorlevel% neq 0 (
    echo ERROR: tailscale up failed with code %errorlevel%
    pause
    exit /b %errorlevel%
)
echo Joined OK

echo [3/3] Status:
"%TS_EXE%" status

echo.
echo === DONE - Pod %POD_NUM% on Tailscale ===
pause
