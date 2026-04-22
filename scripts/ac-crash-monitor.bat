@echo off
REM This bat is invoked by the AC-Crash-Monitor scheduled task every 2 min.
REM The CWD below resolves from the active worktree where this file lives.
REM Stable install path: the scheduled task points at the committed copy in
REM whichever worktree is currently serving as James's canonical checkout.
cd /d %~dp0..
"C:\Program Files\Git\bin\bash.exe" -c "bash scripts/ac-crash-monitor.sh >> logs/ac-crash-monitor.log 2>&1"
