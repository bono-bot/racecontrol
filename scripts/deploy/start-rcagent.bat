@echo off
cd /d C:\RacingPoint
set RUST_BACKTRACE=1

rem --- v3.0 MMA-01: Bootstrap env vars (read FIRST, before any config) ---
set OPENROUTER_KEY=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
set OPENROUTER_MGMT_KEY=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

rem --- v3.0: Clear stale sentinels (prevents stuck pods) ---
del /Q MAINTENANCE_MODE 1>nul 2>nul
del /Q GRACEFUL_RELAUNCH 1>nul 2>nul
del /Q rcagent-restart-sentinel.txt 1>nul 2>nul
del /Q OTA_DEPLOYING 1>nul 2>nul

rem --- MMA 5-model consensus + adversarial review: Edge session restore prevention (2026-03-31) ---
rem Disable Startup Boost (pre-spawns Edge carrying session state)
reg add "HKLM\SOFTWARE\Policies\Microsoft\Edge" /v StartupBoostEnabled /t REG_DWORD /d 0 /f 1>nul 2>nul
rem Disable background mode (keeps Edge alive after last window closes)
reg add "HKLM\SOFTWARE\Policies\Microsoft\Edge" /v BackgroundModeEnabled /t REG_DWORD /d 0 /f 1>nul 2>nul
rem Hide "Restore pages" dialog on crash recovery
reg add "HKLM\SOFTWARE\Policies\Microsoft\Edge" /v HideRestoreDialogEnabled /t REG_DWORD /d 1 /f 1>nul 2>nul
rem Suppress Edge First Run Experience in dedicated profile
reg add "HKLM\SOFTWARE\Policies\Microsoft\Edge" /v HideFirstRunExperience /t REG_DWORD /d 1 /f 1>nul 2>nul
rem Kill any lingering Edge processes before session data cleanup
taskkill /F /IM msedge.exe /T 1>nul 2>nul
taskkill /F /IM msedgewebview2.exe /T 1>nul 2>nul
rem Wipe dedicated Edge profile session data (prevents --app window restoration)
if exist "%LOCALAPPDATA%\RacingPoint\EdgeProfile\Default\Sessions" rd /s /q "%LOCALAPPDATA%\RacingPoint\EdgeProfile\Default\Sessions" 1>nul 2>nul
if exist "%LOCALAPPDATA%\RacingPoint\EdgeProfile\Default\Session Storage" rd /s /q "%LOCALAPPDATA%\RacingPoint\EdgeProfile\Default\Session Storage" 1>nul 2>nul
del /Q "%LOCALAPPDATA%\RacingPoint\EdgeProfile\Default\Current Session" 1>nul 2>nul
del /Q "%LOCALAPPDATA%\RacingPoint\EdgeProfile\Default\Current Tabs" 1>nul 2>nul
del /Q "%LOCALAPPDATA%\RacingPoint\EdgeProfile\Default\Last Session" 1>nul 2>nul
del /Q "%LOCALAPPDATA%\RacingPoint\EdgeProfile\Default\Last Tabs" 1>nul 2>nul

rem --- Enforce power settings (prevents ConspitLink flicker regression) ---
powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c 1>nul 2>nul
powercfg /SETACVALUEINDEX SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0 1>nul 2>nul
powercfg /SETDCVALUEINDEX SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0 1>nul 2>nul
powercfg /SETACTIVE SCHEME_CURRENT 1>nul 2>nul

rem --- Firewall rule ---
netsh advfirewall firewall add rule name="RCAgent" dir=in action=allow protocol=TCP localport=8090 1>nul 2>nul

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
ping -n 4 127.0.0.1 >nul

rem --- Clean deprecated binary naming (pre-hash era) ---
del /Q rc-agent-old.exe 1>nul 2>nul
del /Q rc-agent-new.exe 1>nul 2>nul
del /Q rc-agent-swap.exe 1>nul 2>nul
del /Q rc-sentry-old.exe 1>nul 2>nul
del /Q rc-sentry-new.exe 1>nul 2>nul

rem --- Binary swap (hash-based versioning) ---
set "STAGED="
for /f "delims=" %%F in ('dir /B /O-D rc-agent-????????*.exe 2^>nul') do (
    if not "%%F"=="rc-agent.exe" (
        if not defined STAGED set "STAGED=%%F"
    )
)
if not defined STAGED goto :start_agent
del /Q rc-agent-prev.exe 1>nul 2>nul
if exist rc-agent.exe ren rc-agent.exe rc-agent-prev.exe 1>nul 2>nul
ping -n 2 127.0.0.1 >nul
if exist rc-agent.exe del /Q rc-agent.exe 1>nul 2>nul
ren "%STAGED%" rc-agent.exe 1>nul
:start_agent
start "" /D C:\RacingPoint rc-agent.exe 2>> rc-agent-stderr.log
