@echo off
REM Start the kiosk Next.js standalone server on port 3300
REM Deploy to C:\RacingPoint\start-kiosk.bat on Server .23
REM Add to HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run or Scheduled Task

cd /d C:\RacingPoint\kiosk

set PORT=3300
set HOSTNAME=0.0.0.0

REM Kill any existing kiosk process on port 3300
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :3300 ^| findstr LISTENING') do (
    taskkill /PID %%a /F >/dev/null 2>&1
)

REM Start the standalone Next.js server (minimized)
start "RaceControl-Kiosk" /MIN node server.js
