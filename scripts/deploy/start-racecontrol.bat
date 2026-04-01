@echo off
cd /d C:\RacingPoint
set RUST_BACKTRACE=1
set OPENROUTER_MGMT_KEY=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
taskkill /F /IM racecontrol.exe 1>/dev/null 2>/dev/null
timeout /t 2 /nobreak 1>/dev/null
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
start "" /B powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\RacingPoint\start-racecontrol-watchdog.ps1
