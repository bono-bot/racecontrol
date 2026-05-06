#Requires -Version 5.1
<#
.SYNOPSIS
  Stage 3 setup: register the scheduled task that auto-disables Wi-Fi when
  Ethernet comes Up. Self-elevates if not already running as Administrator.

.DESCRIPTION
  Run from any non-elevated PowerShell:
    powershell -NoProfile -ExecutionPolicy Bypass -File .\setup-auto-disable-wifi-task.ps1

  Self-elevation triggers a single UAC prompt; click Yes; the elevated copy
  registers the task and exits. Idempotent — re-running just updates the task.

  Task spec:
    Name         : AutoDisableWiFiOnEthernetUp
    Trigger      : Microsoft-Windows-NetworkProfile/Operational EventID=10000
    Action       : powershell -NoProfile -ExecutionPolicy Bypass -File <handler>
    Run as       : SYSTEM (NT AUTHORITY\SYSTEM)
    Run level    : Highest privilege

.NOTES
  Verify after install:  Get-ScheduledTask -TaskName AutoDisableWiFiOnEthernetUp
  Test (manual fire):    Start-ScheduledTask -TaskName AutoDisableWiFiOnEthernetUp
  Inspect log:           Get-Content C:/Users/bono/racingpoint/racecontrol/data/auto-disable-wifi.log -Tail 20
  Remove:                Unregister-ScheduledTask -TaskName AutoDisableWiFiOnEthernetUp -Confirm:$false
#>

[CmdletBinding()]
param(
  [string]$TaskName = "AutoDisableWiFiOnEthernetUp",
  [string]$HandlerPath = "C:\Users\bono\racingpoint\racecontrol\scripts\auto-disable-wifi-on-ethernet-up.ps1"
)

# --- Self-elevate if not admin ---------------------------------------------
function Test-IsAdmin {
  $id = [Security.Principal.WindowsIdentity]::GetCurrent()
  $p  = New-Object Security.Principal.WindowsPrincipal($id)
  return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-IsAdmin)) {
  Write-Host "Not elevated. Re-launching with UAC prompt..." -ForegroundColor Yellow
  $args = @("-NoProfile","-ExecutionPolicy","Bypass","-File",$PSCommandPath,
            "-TaskName",$TaskName,"-HandlerPath",$HandlerPath)
  Start-Process -FilePath "powershell.exe" -Verb RunAs -ArgumentList $args -Wait
  exit
}

Write-Host "Running elevated. Registering task '$TaskName'..." -ForegroundColor Green

# --- Sanity checks ----------------------------------------------------------
if (-not (Test-Path $HandlerPath)) {
  Write-Error "Handler script not found: $HandlerPath"
  exit 2
}

# --- Build task definition --------------------------------------------------
# Trigger: NetworkProfile event 10000 (network connection established).
# We filter in the handler script (only act when Ethernet is Up), so the
# trigger fires generously and the handler decides.
$triggerXml = @"
<QueryList>
  <Query Id="0" Path="Microsoft-Windows-NetworkProfile/Operational">
    <Select Path="Microsoft-Windows-NetworkProfile/Operational">
      *[System[Provider[@Name='Microsoft-Windows-NetworkProfile'] and EventID=10000]]
    </Select>
  </Query>
</QueryList>
"@

$action = New-ScheduledTaskAction `
  -Execute "powershell.exe" `
  -Argument ('-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "{0}"' -f $HandlerPath)

$trigger = New-ScheduledTaskTrigger -AtStartup  # placeholder, overwritten below
$cimTrigger = Get-CimClass -ClassName MSFT_TaskEventTrigger -Namespace Root/Microsoft/Windows/TaskScheduler
$eventTrigger = New-CimInstance -CimClass $cimTrigger -ClientOnly
$eventTrigger.Enabled = $true
$eventTrigger.Subscription = $triggerXml

$principal = New-ScheduledTaskPrincipal `
  -UserId "NT AUTHORITY\SYSTEM" `
  -LogonType ServiceAccount `
  -RunLevel Highest

$settings = New-ScheduledTaskSettingsSet `
  -AllowStartIfOnBatteries `
  -DontStopIfGoingOnBatteries `
  -StartWhenAvailable `
  -ExecutionTimeLimit (New-TimeSpan -Minutes 2) `
  -MultipleInstances IgnoreNew

# --- Register ---------------------------------------------------------------
$existing = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
if ($existing) {
  Write-Host "Task '$TaskName' exists, unregistering before re-registration..."
  Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
}

Register-ScheduledTask `
  -TaskName $TaskName `
  -Action $action `
  -Trigger $eventTrigger `
  -Principal $principal `
  -Settings $settings `
  -Description "Auto-disables Wi-Fi adapter when Ethernet network is established. Stage 3 of multi-homing kaizen plan. Handler: $HandlerPath"

Write-Host ""
Write-Host "=== Task registered ===" -ForegroundColor Green
Get-ScheduledTask -TaskName $TaskName | Format-List TaskName, State, Author
Write-Host ""
Write-Host "=== Smoke test (fires handler manually, ignores trigger) ===" -ForegroundColor Cyan
Start-ScheduledTask -TaskName $TaskName
Start-Sleep -Seconds 3
$info = Get-ScheduledTaskInfo -TaskName $TaskName
Write-Host ("LastRunTime  : {0}" -f $info.LastRunTime)
Write-Host ("LastResult   : 0x{0:X} ({0})" -f $info.LastTaskResult)

Write-Host ""
Write-Host "=== Recent handler log ===" -ForegroundColor Cyan
$log = "C:\Users\bono\racingpoint\racecontrol\data\auto-disable-wifi.log"
if (Test-Path $log) {
  Get-Content $log -Tail 6
} else {
  Write-Host "(log not yet created -- handler may not have run)"
}
