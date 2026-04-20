# register-pos-watchdog.ps1 - Register POS-Watchdog scheduled task
# Called by install-pos.bat during fresh POS provisioning.
# Runs as the installing user in Session 1 (InteractiveToken) so the
# watchdog can manipulate GUI processes (Chrome, msedge, rc-agent).
#
# Source of truth: racecontrol/scripts/deploy/register-pos-watchdog.ps1
# Mirror copy: deploy-staging/pos-deploy/register-pos-watchdog.ps1

$ErrorActionPreference = 'Stop'

$taskName = "POS-Watchdog"
$scriptPath = "C:\RacingPoint\watchdog-pos.ps1"

if (-not (Test-Path $scriptPath)) {
    Write-Host "[ERROR] $scriptPath not found - cannot register POS-Watchdog."
    exit 1
}

# Remove any pre-existing task (idempotent)
Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue | Out-Null

$action = New-ScheduledTaskAction `
    -Execute "powershell.exe" `
    -Argument "-NoProfile -WindowStyle Hidden -ExecutionPolicy Bypass -File $scriptPath"

# Every 2 minutes, starting now, indefinitely
$trigger = New-ScheduledTaskTrigger -Once -At (Get-Date)
$trigger.Repetition = (New-ScheduledTaskTrigger -Once -At (Get-Date) `
    -RepetitionInterval (New-TimeSpan -Minutes 2) `
    -RepetitionDuration ([TimeSpan]::MaxValue)).Repetition

# InteractiveToken = runs as the logged-on user in Session 1 (Chrome GUI access).
# No password required.
$principal = New-ScheduledTaskPrincipal `
    -UserId "$env:USERDOMAIN\$env:USERNAME" `
    -LogonType Interactive `
    -RunLevel Limited

$settings = New-ScheduledTaskSettingsSet `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -StartWhenAvailable `
    -MultipleInstances IgnoreNew

Register-ScheduledTask `
    -TaskName $taskName `
    -Action $action `
    -Trigger $trigger `
    -Principal $principal `
    -Settings $settings `
    -Force | Out-Null

Write-Host "[INFO] POS-Watchdog registered (every 2 min, as $env:USERNAME, Session 1)"
