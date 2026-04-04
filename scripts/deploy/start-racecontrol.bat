@echo off
cd /d C:\RacingPoint
set RUST_BACKTRACE=1
rem OpenRouter keys auto-loaded from data/openrouter-mma-key.txt by key recovery module
taskkill /F /IM racecontrol.exe 1>nul 2>nul
ping -n 4 127.0.0.1 >nul
rem --- Binary swap (hash-based versioning) ---
set "STAGED="
for /f "delims=" %%F in ('dir /B /O-D racecontrol-????????*.exe 2^>nul') do (
    if not "%%F"=="racecontrol.exe" (
        if not defined STAGED set "STAGED=%%F"
    )
)
if not defined STAGED goto :startrc
del /Q racecontrol-prev.exe 1>nul 2>nul
if exist racecontrol.exe ren racecontrol.exe racecontrol-prev.exe 1>nul 2>nul
timeout /t 1 /nobreak 1>nul
if exist racecontrol.exe del /Q racecontrol.exe 1>nul 2>nul
ren "%STAGED%" racecontrol.exe 1>nul
:startrc
rem --- Ensure Node.exe firewall rule exists (kiosk :3300, web :3200, admin :3201) ---
netsh advfirewall firewall add rule name="NodeJS RaceControl" dir=in action=allow program="C:\RacingPoint\nodejs\node-v22.14.0-win-x64\node.exe" enable=yes 1>nul 2>nul
start "" /B powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\RacingPoint\start-racecontrol-watchdog.ps1
