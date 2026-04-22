@echo off
REM This bat is invoked by the ConspitLink-Cull scheduled task every 5 min.
REM CWD resolves from the active worktree where this file lives.
cd /d %~dp0..
"C:\Program Files\Git\bin\bash.exe" -c "bash scripts/conspitlink-cull.sh >> logs/conspitlink-cull.log 2>&1"
