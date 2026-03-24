@echo off
cd /d C:\RacingPoint
taskkill /F /IM racecontrol.exe 1>/dev/null 2>/dev/null
timeout /t 2 /nobreak 1>/dev/null
if not exist racecontrol-new.exe goto :startrc
del /Q racecontrol.exe 1>/dev/null 2>/dev/null
timeout /t 1 /nobreak 1>/dev/null
if exist racecontrol.exe del /Q racecontrol.exe 1>/dev/null 2>/dev/null
move racecontrol-new.exe racecontrol.exe 1>/dev/null
:startrc
start "" /B powershell -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\RacingPoint\start-racecontrol-watchdog.ps1
