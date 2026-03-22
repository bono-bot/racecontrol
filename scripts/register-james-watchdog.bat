@echo off
:: register-james-watchdog.bat
:: Replaces james_watchdog.ps1 Task Scheduler task with rc-watchdog.exe
:: Run as Administrator on James (.27)

set BINARY=C:\Users\bono\racingpoint\deploy-staging\rc-watchdog.exe
set TASK_NAME=CommsLink-DaemonWatchdog
set RUN_KEY=HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run

:: 1. Delete old PS1 task if present
schtasks /Delete /TN "%TASK_NAME%" /F 2>/dev/null
echo [1] Old task deleted (or was absent)

:: 2. Register rc-watchdog.exe as the new Task Scheduler task
schtasks /Create /TN "%TASK_NAME%" /TR "\"%BINARY%\"" /SC MINUTE /MO 2 /RU SYSTEM /RL HIGHEST /F
if %ERRORLEVEL% neq 0 goto fail_task
echo [2] Task Scheduler task registered: %TASK_NAME%

:: 3. Add HKLM Run entry for boot-start
reg add "%RUN_KEY%" /v RCWatchdog /t REG_SZ /d "\"%BINARY%\"" /f
if %ERRORLEVEL% neq 0 goto fail_reg
echo [3] HKLM Run entry added: RCWatchdog

:: 4. Trigger one immediate run to confirm the binary works
schtasks /Run /TN "%TASK_NAME%"
echo [4] Immediate run triggered
goto done

:fail_task
echo ERROR: Failed to register Task Scheduler task
exit /b 1

:fail_reg
echo ERROR: Failed to add HKLM Run entry
exit /b 1

:done
echo [OK] James watchdog migration complete
exit /b 0
