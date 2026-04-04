@echo off
REM Kill any existing frontend watchdog PowerShell instances
for /f "tokens=2" %%P in ('wmic process where "CommandLine like '%%frontend-watchdog.ps1%%' and Name='powershell.exe'" get ProcessId 2^>nul ^| findstr /r "[0-9]"') do (
    taskkill /F /PID %%P >nul 2>&1
)
cd /D C:\RacingPoint
REM Use 'start' WITHOUT /B to create a detached process that survives schtask exit
start "" /D C:\RacingPoint powershell.exe -ExecutionPolicy Bypass -WindowStyle Hidden -File "C:\RacingPoint\frontend-watchdog.ps1"
