@echo off
REM Wrapper for Windows Task Scheduler to run nightly-audit.sh
REM Created 2026-04-04 for SmartPipes-NightlyAudit scheduled task
cd /d C:\Users\bono\racingpoint\racecontrol
"C:\Program Files\Git\bin\bash.exe" -c "bash scripts/smart-pipes/nightly-audit.sh >> .smart-pipes-results/nightly-cron.log 2>&1"
