# repair-frontend-watchdog-task.ps1 — PACT-20260428-029
# Re-register StartFrontendWatchdog with 3 triggers: BootTrigger + LogonTrigger
# + 5-min Repetition. Idempotent. See proposals/PACT-20260428-029-*.md.

$ErrorActionPreference = 'Stop'
$TaskName = 'StartFrontendWatchdog'
$ScriptPath = 'C:\RacingPoint\frontend-watchdog.ps1'

if (-not (Test-Path $ScriptPath)) {
    Write-Error "Watchdog script missing at $ScriptPath"
    exit 2
}

$bootTrigger = New-ScheduledTaskTrigger -AtStartup
$logonTrigger = New-ScheduledTaskTrigger -AtLogOn
$repTrigger = New-ScheduledTaskTrigger -Once -At (Get-Date) -RepetitionInterval (New-TimeSpan -Minutes 5)

$action = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument "-ExecutionPolicy Bypass -WindowStyle Hidden -File $ScriptPath"
$principal = New-ScheduledTaskPrincipal -UserId 'SYSTEM' -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -MultipleInstances IgnoreNew -StartWhenAvailable -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero)

Write-Output "Re-registering $TaskName with BootTrigger + LogonTrigger + 5-min Repetition"
Register-ScheduledTask -TaskName $TaskName -Trigger @($bootTrigger, $logonTrigger, $repTrigger) -Action $action -Principal $principal -Settings $settings -Force | Out-Null

Write-Output "--- VERIFY ---"
$task = Get-ScheduledTask -TaskName $TaskName
$info = Get-ScheduledTaskInfo -TaskName $TaskName
Write-Output ("State: " + $task.State)
Write-Output ("Last Run: " + $info.LastRunTime)
Write-Output ("Last Result: " + $info.LastTaskResult)
Write-Output ("Next Run: " + $info.NextRunTime)
Write-Output "--- TRIGGERS ---"
$task.Triggers | ForEach-Object {
    $type = $_.GetType().Name
    $rep = if ($_.Repetition) { " repetition=" + $_.Repetition.Interval } else { "" }
    Write-Output ("  " + $type + $rep)
}
Write-Output "DONE."
