# provision-startrcdirect.ps1
#
# Creates or updates the StartRCDirect scheduled task on the server (.23).
# Run as admin on the server once during provisioning, or after the task is
# accidentally deleted.
#
# StartRCDirect is the restart path used by racecontrol-watchdog.bat when it
# detects racecontrol.exe is dead. It must be configured with:
#   /RU SYSTEM        - runs as SYSTEM account (no interactive login required)
#   /LO SERVICE       - Logon Mode: Interactive/Background (not "Interactive only")
#
# Why this matters:
# If the task is set to "Logon Mode: Interactive only" + "Run As: <user>", then
# `schtasks /Run /TN StartRCDirect` silently no-ops whenever <user> is not
# interactively logged in. schtasks.exe still returns "SUCCESS: Attempted to run"
# and exit code 0, but Last Run Time never updates. The watchdog bat checks only
# the schtasks exit code and believes the restart succeeded - racecontrol stays
# dead until someone manually intervenes.
#
# Incident (2026-04-11 20:23-20:50 IST): this failure mode caused a 25+ minute
# server outage. Fix: change the task to /RU SYSTEM /LO SERVICE. Also pairs with
# start-racecontrol-direct.bat which must use `start` for detached console spawn.

$ErrorActionPreference = "Stop"

$TaskName = "StartRCDirect"
$BatPath  = "C:\RacingPoint\start-racecontrol-direct.bat"

if (-not (Test-Path $BatPath)) {
    Write-Error "Required bat file missing: $BatPath"
    exit 1
}

# Delete existing task if present (idempotent re-provisioning)
schtasks /Query /TN $TaskName 2>$null | Out-Null
if ($LASTEXITCODE -eq 0) {
    Write-Host "Deleting existing $TaskName task..."
    schtasks /Delete /TN $TaskName /F | Out-Null
}

# Create with correct parameters:
#   /RU SYSTEM            - run as SYSTEM account
#   /SC ONCE /ST 23:59    - manual trigger only (no automatic schedule)
#   /SD 01/01/2000        - arbitrary past date
#   (Logon Mode defaults to Interactive/Background when /RU SYSTEM is used)
Write-Host "Creating $TaskName as SYSTEM..."
schtasks /Create `
    /TN $TaskName `
    /TR "$BatPath" `
    /SC ONCE `
    /ST 23:59 `
    /SD 01/01/2000 `
    /RU SYSTEM `
    /F | Out-Null

if ($LASTEXITCODE -ne 0) {
    Write-Error "schtasks /Create failed with exit code $LASTEXITCODE"
    exit 1
}

# Verify
$info = schtasks /Query /TN $TaskName /FO LIST /V 2>$null
$runAs    = ($info | Select-String "Run As User:").Line
$logonMode = ($info | Select-String "Logon Mode:").Line
$state     = ($info | Select-String "Scheduled Task State:").Line

Write-Host ""
Write-Host "Verification:"
Write-Host "  $runAs"
Write-Host "  $logonMode"
Write-Host "  $state"
Write-Host ""

if ($runAs -notmatch "SYSTEM") {
    Write-Error "Verification failed: Run As User is not SYSTEM"
    exit 1
}
if ($logonMode -notmatch "Interactive/Background|Background") {
    Write-Error "Verification failed: Logon Mode is not Interactive/Background"
    exit 1
}

Write-Host "StartRCDirect provisioned successfully."
