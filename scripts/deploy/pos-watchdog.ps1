# POS Watchdog - checks Chrome kiosk + rc-agent, restarts if down
# Runs every 2 minutes via Task Scheduler "POS-Watchdog"
# Deploy target: C:\RacingPoint\watchdog-pos.ps1 on POS machine (.130)
#
# 2026-04-20 - Rewritten from Edge to Chrome after the 2026-04-20 00:55 IST
# kiosk swap. Previous Edge-based version respawned msedge every boot behind
# Chrome's fullscreen window (invisible orphan). Matches HKLM Run
# BillingDashboard flags exactly so Chrome relaunches with the same user
# profile, remote-debugging-port, and kiosk URL.
#
# Source of truth: racecontrol/scripts/deploy/pos-watchdog.ps1
# Post-swap invariant: scripts/audit/kiosk-swap-verify.ps1 must exit 0.

$logFile = "C:\RacingPoint\pos-watchdog.log"
function Log($msg) {
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    Add-Content -Path $logFile -Value "$ts $msg"
}

# --- Check Chrome kiosk browser ---
# Match HKLM Run BillingDashboard exactly so manual launch parity is maintained.
$chromeExe  = "C:\Program Files\Google\Chrome\Application\chrome.exe"
$kioskUrl   = "http://192.168.31.23:3200/billing"
$chromeArgs = @(
    "--kiosk", $kioskUrl,
    "--no-first-run",
    "--remote-debugging-port=9222",
    "--user-data-dir=C:\RacingPoint\chrome-kiosk-profile"
)

$chrome = Get-Process chrome -ErrorAction SilentlyContinue
if (-not $chrome) {
    Log "Chrome kiosk NOT running, restarting with billing URL"
    if (Test-Path $chromeExe) {
        Start-Process $chromeExe -ArgumentList $chromeArgs
        Log "Chrome kiosk restarted with billing URL"
    } else {
        Log "ERROR: Chrome not found at $chromeExe - cannot restart kiosk"
    }
}

# --- Guard against Edge orphan (post-2026-04-20 swap hygiene) ---
# If msedge is running, something un-swapped launched it. Kill + log.
# This is defense in depth; the structural fix is no autostart surface
# references msedge (verified by kiosk-swap-verify.ps1).
$edge = Get-Process msedge -ErrorAction SilentlyContinue
if ($edge) {
    $pids = ($edge | ForEach-Object { $_.Id }) -join ","
    Log "WARN: msedge orphan(s) detected (PIDs $pids) - killing. Investigate autostart surface drift."
    $edge | Stop-Process -Force -ErrorAction SilentlyContinue
}

# --- Check rc-agent ---
# rc-agent is the correct binary name on POS (not rc-pos-agent).
# Direct start avoids schtasks Session 0 GUI failure - this watchdog already
# runs in the user session via Task Scheduler "Run only when user is logged on".
$agent = Get-Process rc-agent -ErrorAction SilentlyContinue
if (-not $agent) {
    Log "rc-agent NOT running, restarting via start-rcagent.bat"
    Start-Process -FilePath "C:\RacingPoint\start-rcagent.bat" -WorkingDirectory "C:\RacingPoint" -WindowStyle Hidden
    Log "rc-agent restart initiated via start-rcagent.bat (direct start, not schtasks)"
}
