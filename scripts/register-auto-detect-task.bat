@echo off
:: register-auto-detect-task.bat
:: Registers auto-detect.sh as a daily Task Scheduler task at 02:30 IST
:: Run as Administrator on James (.27)
:: PREREQUISITE: auto-detect.sh must have PID guard + safety gates (Phase 211 Plan 01)

set BASH_EXE=C:\Program Files\Git\bin\bash.exe
set SCRIPT=C:\Users\bono\racingpoint\racecontrol\scripts\auto-detect.sh
set TASK_NAME=AutoDetect-Daily
set AUDIT_PIN_VALUE=261121

:: Verify bash exists
if not exist "%BASH_EXE%" goto no_bash

:: 1. Delete old task if present
schtasks /Delete /TN "%TASK_NAME%" /F 2>nul
echo [1] Cleared old task (if any)

:: 2. Verify safety gates exist in auto-detect.sh before registering
"%BASH_EXE%" -c "grep -q '_acquire_run_lock' '%SCRIPT%'"
if %ERRORLEVEL% neq 0 goto no_safety

:: 3. Register daily task at 02:30
schtasks /Create /TN "%TASK_NAME%" /TR "\"%BASH_EXE%\" -c \"AUDIT_PIN=%AUDIT_PIN_VALUE% bash '%SCRIPT%'\"" /SC DAILY /ST 02:30 /RU SYSTEM /RL HIGHEST /F
if %ERRORLEVEL% neq 0 goto fail_task
echo [3] Task registered: %TASK_NAME% at 02:30 daily

:: 4. Verify registration
schtasks /Query /TN "%TASK_NAME%" /FO LIST
echo.
goto done

:no_bash
echo ERROR: Git Bash not found at %BASH_EXE%
echo Run "where bash" to find the correct path and update this script
exit /b 2

:no_safety
echo ERROR: auto-detect.sh missing PID guard (_acquire_run_lock)
echo Phase 211 Plan 01 must complete before registering the task
exit /b 2

:fail_task
echo ERROR: Failed to register Task Scheduler task
exit /b 1

:done
echo [OK] AutoDetect-Daily registered -- runs nightly at 02:30 IST
exit /b 0
