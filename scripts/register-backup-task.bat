@echo off
REM register-backup-task.bat — Register daily database backup (P0 DR fix)
REM Run as Administrator: right-click > Run as Administrator
REM Uses 8.3 short path (C:\PROGRA~1\Git) to avoid schtasks space-in-path quoting issue.
schtasks /Create /SC DAILY /ST 03:00 /TN "DatabaseBackup" /TR "C:\PROGRA~1\Git\bin\bash.exe C:\Users\bono\racingpoint\racecontrol\scripts\backup-databases.sh" /RU bono /F
if %ERRORLEVEL% EQU 0 goto :success
echo Failed -- run as Administrator
goto :done
:success
echo DatabaseBackup task registered -- runs daily at 03:00 IST
:done
