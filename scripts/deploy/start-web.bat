@echo off
REM Racing Point Web Dashboard Watchdog Launcher
REM Runs at login via HKLM Run key
start "" /B powershell.exe -ExecutionPolicy Bypass -WindowStyle Hidden -File "C:\RacingPoint\start-web-watchdog.ps1"
