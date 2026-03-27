@echo off
cd /d C:\RacingPoint
taskkill /F /IM rc-sentry.exe 1>nul 2>nul
ping -n 3 127.0.0.1 1>nul
rem --- Binary swap (hash-based versioning) ---
set "STAGED="
for /f "delims=" %%F in ('dir /B /O-D rc-sentry-????????*.exe 2^>nul') do (
    if not "%%F"=="rc-sentry.exe" (
        if not defined STAGED set "STAGED=%%F"
    )
)
if not defined STAGED goto :skip_swap
del /Q rc-sentry-prev.exe 1>nul 2>nul
if exist rc-sentry.exe ren rc-sentry.exe rc-sentry-prev.exe 1>nul 2>nul
ping -n 2 127.0.0.1 1>nul
if exist rc-sentry.exe del /Q rc-sentry.exe 1>nul 2>nul
ren "%STAGED%" rc-sentry.exe 1>nul
:skip_swap
start /D C:\RacingPoint "" C:\RacingPoint\rc-sentry.exe
