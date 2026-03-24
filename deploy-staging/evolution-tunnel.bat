@echo off
REM Evolution API SSH tunnel — forwards localhost:53622 to Bono VPS:53622
REM Fallback for when Tailscale is down. Runs as scheduled task every 2 min.
REM Standing rule: have a rollback plan. If tunnel causes issues, disable task:
REM   schtasks /Change /TN EvolutionTunnel /DISABLE

REM Check if tunnel already running
netstat -ano | findstr "LISTEN" | findstr ":53622" >nul 2>&1
if %ERRORLEVEL%==0 goto :already_running

REM Start tunnel (background, auto-reconnect on failure)
start /B ssh -o StrictHostKeyChecking=no -o ServerAliveInterval=30 -o ServerAliveCountMax=3 -o ExitOnForwardFailure=yes -N -L 53622:localhost:53622 root@100.70.177.44
echo %DATE% %TIME% Tunnel started >> C:\RacingPoint\logs\evolution-tunnel.log
goto :eof

:already_running
REM Tunnel running, nothing to do
goto :eof
