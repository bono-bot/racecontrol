@echo off
cd /d C:\RacingPoint

rem --- Enforce power settings (prevents ConspitLink flicker regression) ---
powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c 1>nul 2>nul
powercfg /SETACVALUEINDEX SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0 1>nul 2>nul
powercfg /SETDCVALUEINDEX SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0 1>nul 2>nul
powercfg /SETACTIVE SCHEME_CURRENT 1>nul 2>nul

rem --- Kill bloatware and prevent process multiplication ---
taskkill /F /IM Variable_dump.exe 1>nul 2>nul
taskkill /F /IM "Creative Cloud UI Helper.exe" 1>nul 2>nul
taskkill /F /IM M365Copilot.exe 1>nul 2>nul
taskkill /F /IM Copilot.exe 1>nul 2>nul
taskkill /F /IM ollama.exe 1>nul 2>nul
taskkill /F /IM ClockifyWindows.exe 1>nul 2>nul
taskkill /F /IM OneDrive.exe 1>nul 2>nul
taskkill /F /IM powershell.exe 1>nul 2>nul
taskkill /F /IM ConspitLink2.0.exe 1>nul 2>nul
taskkill /F /IM rc-agent.exe 1>nul 2>nul
timeout /t 3 /nobreak 1>nul

rem --- Binary swap ---
if not exist rc-agent-new.exe goto :skip_swap
del /Q rc-agent.exe 1>nul 2>nul
timeout /t 1 /nobreak 1>nul
if exist rc-agent.exe del /Q rc-agent.exe 1>nul 2>nul
move rc-agent-new.exe rc-agent.exe 1>nul
:skip_swap

rem --- Start ConspitLink singleton then rc-agent ---
start /D C:\RacingPoint "" C:\RacingPoint\rc-agent.exe
