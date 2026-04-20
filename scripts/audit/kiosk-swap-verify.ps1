param(
    [Parameter(Mandatory=$true)][string]$OldBinary,
    [Parameter(Mandatory=$true)][string]$NewBinary,
    [int]$MaxOldProcs = 0,
    [string]$BaselineOut = ""
)

# Post-swap invariant check. Run within 5 min of any kiosk browser swap.
# Fails (exit 1) if:
#   - any autostart surface still references $OldBinary
#   - running process count of $OldBinary > $MaxOldProcs
#   - no autostart surface references $NewBinary (sanity check swap actually happened)
#
# Surfaces checked: HKLM/HKCU Run+RunOnce, Startup folders (all-users + user),
# scheduled tasks (excluding Microsoft-built-in), auto-start services, and
# any .ps1/.bat/.cmd/.reg/.xml file under C:\RacingPoint\ referencing a browser.

$ErrorActionPreference = 'SilentlyContinue'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$enumerator = Join-Path $here 'autostart-surfaces.ps1'
if (-not (Test-Path $enumerator)) {
    Write-Error "Missing $enumerator"
    exit 2
}

$tmpJson = [System.IO.Path]::GetTempFileName()
& powershell -NoProfile -ExecutionPolicy Bypass -File $enumerator -OutFile $tmpJson | Out-Null
$state = Get-Content $tmpJson -Raw | ConvertFrom-Json
if ($BaselineOut) { Copy-Item $tmpJson $BaselineOut -Force }
Remove-Item $tmpJson -Force -ErrorAction SilentlyContinue

$oldRefs = @()
$newRefs = @()

function Add-Refs($surfaceName, $items, $getString) {
    foreach ($i in $items) {
        $s = & $getString $i
        if ($s -like "*$OldBinary*") { $script:oldRefs += ,@{ surface = $surfaceName; ref = $s } }
        if ($s -like "*$NewBinary*") { $script:newRefs += ,@{ surface = $surfaceName; ref = $s } }
    }
}

Add-Refs "hklm_run"          $state.surfaces.hklm_run          { param($x) "$($x.name)=$($x.value)" }
Add-Refs "hklm_runonce"      $state.surfaces.hklm_runonce      { param($x) "$($x.name)=$($x.value)" }
Add-Refs "hkcu_run"          $state.surfaces.hkcu_run          { param($x) "$($x.name)=$($x.value)" }
Add-Refs "hkcu_runonce"      $state.surfaces.hkcu_runonce      { param($x) "$($x.name)=$($x.value)" }
Add-Refs "startup_all_users" $state.surfaces.startup_all_users { param($x) "$($x.name): $($x.target) $($x.arguments)" }
Add-Refs "startup_user"      $state.surfaces.startup_user      { param($x) "$($x.name): $($x.target) $($x.arguments)" }
Add-Refs "scheduled_tasks"   $state.surfaces.scheduled_tasks   { param($x) "$($x.name): $($x.command)" }
Add-Refs "auto_services"     $state.surfaces.auto_services     { param($x) "$($x.name): $($x.path)" }
Add-Refs "on_disk"           $state.surfaces.on_disk_browser_refs { param($x) "$($x.path): $($x.matches)" }

$runningOld = ($state.surfaces.running_browsers | Where-Object { $_.name -eq $OldBinary } | Measure-Object).Count
$runningNew = ($state.surfaces.running_browsers | Where-Object { $_.name -eq $NewBinary } | Measure-Object).Count

$fail = $false
$report = @()

if ($oldRefs.Count -gt 0) {
    $fail = $true
    $report += "FAIL: $OldBinary still referenced in $($oldRefs.Count) autostart surface(s):"
    foreach ($r in $oldRefs) { $report += "  - [$($r.surface)] $($r.ref)" }
} else {
    $report += "OK:   No autostart surface references $OldBinary"
}

if ($runningOld -gt $MaxOldProcs) {
    $fail = $true
    $report += "FAIL: $OldBinary process count = $runningOld (max allowed = $MaxOldProcs)"
} else {
    $report += "OK:   $OldBinary process count = $runningOld (<= $MaxOldProcs)"
}

if ($newRefs.Count -eq 0) {
    $fail = $true
    $report += "FAIL: $NewBinary is NOT referenced by any autostart surface - swap did not take effect"
} else {
    $report += "OK:   $NewBinary referenced by $($newRefs.Count) autostart surface(s)"
}

if ($runningNew -eq 0) {
    $report += "WARN: $NewBinary not currently running (may not have launched yet - wait and re-check)"
} else {
    $report += "OK:   $NewBinary process count = $runningNew"
}

$report | ForEach-Object { Write-Host $_ }

if ($fail) { exit 1 } else { exit 0 }
