@echo off
REM register-backup-task.bat — Register daily database backup (P0 DR fix)
schtasks /Create /SC DAILY /ST 03:00 /TN "DatabaseBackup" /TR "\"C:\Program Files\Git\bin\bash.exe\" \"C:\Users\bono\racingpoint\racecontrol\scripts\backup-databases.sh\"" /RU bono /F
if %ERRORLEVEL% EQU 0 (
    echo DatabaseBackup task registered — runs daily at 03:00 IST
) else (
    echo Failed — run as Administrator
)
