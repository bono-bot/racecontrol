@echo off
cd /d C:\RacingPoint
netsh advfirewall set allprofiles state off 1>nul 2>nul
rem --- Kill bloatware on boot ---
taskkill /F /IM Variable_dump.exe 1>nul 2>nul
taskkill /F /IM "Creative Cloud UI Helper.exe" 1>nul 2>nul
taskkill /F /IM M365Copilot.exe 1>nul 2>nul
taskkill /F /IM Copilot.exe 1>nul 2>nul
taskkill /F /IM ollama.exe 1>nul 2>nul
taskkill /F /IM ClockifyWindows.exe 1>nul 2>nul
taskkill /F /IM OneDrive.exe 1>nul 2>nul
taskkill /F /IM powershell.exe 1>nul 2>nul
taskkill /F /IM rc-agent.exe 1>nul 2>nul
timeout /t 3 /nobreak 1>nul
if exist rc-agent-new.exe (
    del /Q rc-agent.exe 1>nul 2>nul
    timeout /t 1 /nobreak 1>nul
    if exist rc-agent.exe del /Q rc-agent.exe 1>nul 2>nul
    move rc-agent-new.exe rc-agent.exe 1>nul
)
start /D C:\RacingPoint "" C:\RacingPoint\rc-agent.exe
