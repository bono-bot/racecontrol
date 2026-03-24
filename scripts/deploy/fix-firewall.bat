@echo off
REM ============================================================
REM  Pod Firewall Fix — Allow inbound ICMP + TCP 8090
REM  Usage: Run as administrator on affected pod
REM ============================================================

echo.
echo ========================================
echo   Pod Firewall Fix
echo ========================================
echo.

REM Remove any stale rules first (idempotent)
echo [1/4] Removing stale firewall rules...
netsh advfirewall firewall delete rule name="AllowICMP" 2>nul
netsh advfirewall firewall delete rule name="RCAgent" 2>nul
netsh advfirewall firewall delete rule name="PodAgent" 2>nul
netsh advfirewall firewall delete rule name="RacingPoint Pod Agent" 2>nul

echo [2/4] Adding ICMP (ping) allow rule...
netsh advfirewall firewall add rule name="AllowICMP" protocol=icmpv4:8,any dir=in action=allow
if ERRORLEVEL 1 (
    echo    FAILED — run as Administrator!
    pause
    exit /b 1
)
echo    OK

echo [3/4] Adding TCP 8090 (rc-agent remote ops) allow rule...
netsh advfirewall firewall add rule name="RCAgent" dir=in action=allow protocol=TCP localport=8090
if ERRORLEVEL 1 (
    echo    FAILED
    pause
    exit /b 1
)
echo    OK

echo [4/4] Verifying rules...
netsh advfirewall firewall show rule name="AllowICMP" | findstr "Enabled"
netsh advfirewall firewall show rule name="RCAgent" | findstr "Enabled"

echo.
echo ========================================
echo   Firewall rules applied successfully
echo   Pod should now be pingable and
echo   reachable on port 8090.
echo ========================================
echo.
pause
