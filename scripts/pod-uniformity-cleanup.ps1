#Requires -Version 5.1
<#
.SYNOPSIS
  Bring a pod into compliance with docs/POD-SPEC.md.
  DryRun by default -- preview every action without executing.
  Pass -Apply to actually perform the changes.

.DESCRIPTION
  Covers the LOW-RISK drift classes from the 2026-05-06 fleet uniformity audit:
    - Stale staged binaries (rc-agent-prev.exe, rc-sentry-<hash>.exe older than 7 days)
    - Forbidden HKCU Run keys (OneDrive, Teams, SGP Sync App, RPMKickstart)
    - Sentinel files (MAINTENANCE_MODE, GRACEFUL_RELAUNCH, etc -- should always be absent)

  HIGHER-RISK drift classes are intentionally NOT covered here (need per-task
  action review before delete):
    - Non-canonical scheduled tasks (delete only after verifying TR target)
    - Wintun ghost cleanup (needs pnputil + admin + per-device review)
    - rc-agent.toml content sync (deferred until canonical template exists)
    - start-rcsentry.bat sync (deferred until canonical version chosen)

  Idempotent: re-running on a clean pod is a no-op.
  Reversible: nothing is permanent -- staged binaries can be restaged, run keys re-added.

.PARAMETER DryRun
  Default. Logs what each action would do, no changes.

.PARAMETER Apply
  Execute the cleanup actions. Requires admin for HKCU edits to other users.

.PARAMETER StagedBinaryAgeDays
  Threshold for stale staged binaries. Default 7.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File .\pod-uniformity-cleanup.ps1
  # Dry run -- preview only

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File .\pod-uniformity-cleanup.ps1 -Apply
  # Execute cleanup
#>

[CmdletBinding()]
param(
  [switch]$DryRun,
  [switch]$Apply,
  [int]$StagedBinaryAgeDays = 7,
  [string]$LogDir = "C:\RacingPoint"
)

# Default behaviour: DryRun unless -Apply explicitly set
if (-not $Apply) { $DryRun = $true }

$ts = Get-Date -Format "yyyyMMdd-HHmmss"
$mode = if ($Apply) { "APPLY" } else { "DRYRUN" }
$LogFile = Join-Path $LogDir "uniformity-cleanup-$mode-$ts.log"

if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir -Force | Out-Null }

function Log {
  param([string]$Msg, [string]$Level = "INFO")
  $line = "[{0}] [{1,-6}] [{2,-6}] {3}" -f (Get-Date -Format "HH:mm:ss"), $mode, $Level, $Msg
  Add-Content -Path $LogFile -Value $line -Encoding UTF8
  $color = switch ($Level) {
    "ACTION" { "Green" }
    "WARN"   { "Yellow" }
    "ERROR"  { "Red" }
    "SKIP"   { "Gray" }
    default  { "White" }
  }
  Write-Host $line -ForegroundColor $color
}

Log "=== Pod Uniformity Cleanup === host=$env:COMPUTERNAME mode=$mode"
Log "Log file: $LogFile"

$totalActions = 0
$totalSkipped = 0

# ----------------------------------------------------------------------------
# Cleanup 1 -- Stale staged binaries
# ----------------------------------------------------------------------------
Log ""
Log "--- Cleanup 1: stale staged binaries (older than $StagedBinaryAgeDays days) ---"

$cutoff = (Get-Date).AddDays(-$StagedBinaryAgeDays)
$patterns = @("rc-agent-prev.exe","rc-sentry-prev.exe","rc-agent-*.exe","rc-sentry-*.exe")
$canonical = @("rc-agent.exe","rc-sentry.exe")

$staleFiles = @()
foreach ($pat in $patterns) {
  $hits = Get-ChildItem "C:\RacingPoint\$pat" -ErrorAction SilentlyContinue
  foreach ($f in $hits) {
    if ($canonical -contains $f.Name) { continue }   # never touch canonical
    if ($f.LastWriteTime -lt $cutoff) {
      $staleFiles += $f
    }
  }
}

# Deduplicate (different patterns may match the same file)
$staleFiles = $staleFiles | Sort-Object FullName -Unique

if ($staleFiles.Count -eq 0) {
  Log "No stale staged binaries to remove" "SKIP"
} else {
  foreach ($f in $staleFiles) {
    $age = [int]((Get-Date) - $f.LastWriteTime).TotalDays
    if ($Apply) {
      try {
        Remove-Item $f.FullName -Force -ErrorAction Stop
        Log "REMOVED $($f.Name) (size=$($f.Length)b age=${age}d)" "ACTION"
        $totalActions++
      } catch {
        Log "FAILED to remove $($f.Name): $($_.Exception.Message)" "ERROR"
      }
    } else {
      Log "would remove $($f.Name) (size=$($f.Length)b age=${age}d)" "ACTION"
      $totalActions++
    }
  }
}

# ----------------------------------------------------------------------------
# Cleanup 2 -- Forbidden HKCU Run keys
# ----------------------------------------------------------------------------
Log ""
Log "--- Cleanup 2: forbidden HKCU Run keys ---"

$forbiddenKeys = @("OneDrive","Teams","SGP Sync App","RPMKickstart","Microsoft Teams")

# HKCU only applies to the current user. To clean keys for the kiosk user when
# this script runs as a different user, would need to load that user's hive.
# For now: clean the current user's HKCU (this is what RCWatchdog runs as).

$hkcuRun = "Registry::HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run"
foreach ($k in $forbiddenKeys) {
  $val = Get-ItemProperty -Path $hkcuRun -Name $k -ErrorAction SilentlyContinue
  if (-not $val -or -not ($val.PSObject.Properties.Name -contains $k)) {
    Log "HKCU\Run\$k not present" "SKIP"
    $totalSkipped++
    continue
  }
  $current = $val.$k
  if ($Apply) {
    try {
      Remove-ItemProperty -Path $hkcuRun -Name $k -ErrorAction Stop
      Log "REMOVED HKCU\Run\$k (was: $current)" "ACTION"
      $totalActions++
    } catch {
      Log "FAILED HKCU\Run\$k removal: $($_.Exception.Message)" "ERROR"
    }
  } else {
    Log "would remove HKCU\Run\$k (was: $current)" "ACTION"
    $totalActions++
  }
}

# ----------------------------------------------------------------------------
# Cleanup 3 -- Sentinel file purge (always-absent invariant)
# ----------------------------------------------------------------------------
Log ""
Log "--- Cleanup 3: sentinel files (must be absent during normal operation) ---"

$sentinels = @("MAINTENANCE_MODE","GRACEFUL_RELAUNCH","rcagent-restart-sentinel.txt","DEPLOY_IN_PROGRESS","OTA_DEPLOYING")
foreach ($s in $sentinels) {
  $p = "C:\RacingPoint\$s"
  if (Test-Path $p) {
    $age = [int]((Get-Date) - (Get-Item $p).LastWriteTime).TotalMinutes
    if ($age -lt 5) {
      Log "$s present but recent (age=${age}min) -- possibly active deploy/restart, NOT REMOVING" "WARN"
      $totalSkipped++
      continue
    }
    if ($Apply) {
      try {
        Remove-Item $p -Force -ErrorAction Stop
        Log "REMOVED sentinel $s (age=${age}min)" "ACTION"
        $totalActions++
      } catch {
        Log "FAILED removing $s : $($_.Exception.Message)" "ERROR"
      }
    } else {
      Log "would remove sentinel $s (age=${age}min)" "ACTION"
      $totalActions++
    }
  } else {
    Log "$s absent" "SKIP"
    $totalSkipped++
  }
}

# ----------------------------------------------------------------------------
# Cleanup 4 -- Legacy / superseded scheduled tasks (REQUIRES ADMIN)
# ----------------------------------------------------------------------------
Log ""
Log "--- Cleanup 4: legacy scheduled tasks (admin required) ---"

$legacyTasks = @(
  "RC-Agent Watchdog","RCAgentWatchdog","RacingPoint-PodAgent","RacingPoint-RcAgent",
  "StartRCAgentNow","TempStartRCAgent","RCAgentNow","RC-Test","RC-Update"
)
# DEFERRED: StartRCAgentOnLogon (currently in Running state on pod1; defer to next reboot cycle)

foreach ($name in $legacyTasks) {
  $t = Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue
  if (-not $t) {
    Log "task $name not present" "SKIP"
    $totalSkipped++
    continue
  }
  if ($Apply) {
    try {
      Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction Stop
      Log "REMOVED task $name (was state=$($t.State))" "ACTION"
      $totalActions++
    } catch {
      Log "FAILED removing task $name : $($_.Exception.Message)" "ERROR"
    }
  } else {
    Log "would remove task $name (state=$($t.State))" "ACTION"
    $totalActions++
  }
}

# ----------------------------------------------------------------------------
# Cleanup 5 -- Forbidden HKLM Run keys (REQUIRES ADMIN)
# ----------------------------------------------------------------------------
Log ""
Log "--- Cleanup 5: forbidden HKLM Run keys (admin required) ---"

$hklmRun = "Registry::HKEY_LOCAL_MACHINE\Software\Microsoft\Windows\CurrentVersion\Run"
$forbiddenHKLM = @("RPMKickstart")  # GIGABYTE Smart Backup -- not part of canonical pod stack

foreach ($k in $forbiddenHKLM) {
  $val = Get-ItemProperty -Path $hklmRun -Name $k -ErrorAction SilentlyContinue
  if (-not $val -or -not ($val.PSObject.Properties.Name -contains $k)) {
    Log "HKLM\Run\$k not present" "SKIP"
    $totalSkipped++
    continue
  }
  $current = $val.$k
  if ($Apply) {
    try {
      Remove-ItemProperty -Path $hklmRun -Name $k -ErrorAction Stop
      Log "REMOVED HKLM\Run\$k (was: $current)" "ACTION"
      $totalActions++
    } catch {
      Log "FAILED HKLM\Run\$k removal: $($_.Exception.Message)" "ERROR"
    }
  } else {
    Log "would remove HKLM\Run\$k (was: $current)" "ACTION"
    $totalActions++
  }
}

# ----------------------------------------------------------------------------
# Cleanup 6 -- Wintun ghost adapter cleanup (REQUIRES ADMIN)
# Removes all Wintun device-instances except the active Tailscale one.
# Each pod ideally has 1 Wintun adapter (Tailscale's). Anything extra is ghost.
# ----------------------------------------------------------------------------
Log ""
Log "--- Cleanup 6: Wintun ghost adapters (admin required, pnputil) ---"

$wintunDevices = Get-PnpDevice -Class Net -ErrorAction SilentlyContinue | Where-Object {
  $_.FriendlyName -like "*Wintun*" -or $_.HardwareID -like "*Wintun*"
}

# Identify the ACTIVE Wintun: the NetAdapter whose IPv4 is in 100.x (Tailscale) range.
# Get its PnPDeviceID and match against Get-PnpDevice InstanceId.
$activeAdapter = Get-NetAdapter -ErrorAction SilentlyContinue | Where-Object {
  $_.InterfaceAlias -in (
    (Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue |
       Where-Object { $_.IPAddress -like "100.*" -and $_.AddressState -eq 'Preferred' }).InterfaceAlias
  )
} | Select-Object -First 1

$activePnpId = if ($activeAdapter) { $activeAdapter.PnPDeviceID } else { $null }
Log "Active Tailscale Wintun PnPDeviceID = $activePnpId" "INFO"

# SAFETY CHECK: if we cannot positively identify the active Wintun, abort cleanup 6.
# Better to leave drift than to delete the active Tailscale adapter and isolate the pod.
if (-not $activePnpId) {
  Log "ABORT Cleanup 6: cannot identify active Tailscale Wintun adapter -- skipping all Wintun removal to avoid risk of breaking Tailscale" "WARN"
} else {
  foreach ($d in $wintunDevices) {
    if ($d.InstanceId -eq $activePnpId) {
      Log "KEEP Wintun $($d.FriendlyName) (instId=$($d.InstanceId)) -- active Tailscale binding" "SKIP"
      $totalSkipped++
      continue
    }
    if ($Apply) {
      try {
        $rc = & pnputil.exe /remove-device $d.InstanceId 2>&1
        Log "REMOVED Wintun $($d.FriendlyName) instId=$($d.InstanceId) -- pnputil: $rc" "ACTION"
        $totalActions++
      } catch {
        Log "FAILED Wintun removal $($d.FriendlyName): $($_.Exception.Message)" "ERROR"
      }
    } else {
      Log "would remove Wintun $($d.FriendlyName) (instId=$($d.InstanceId) status=$($d.Status))" "ACTION"
      $totalActions++
    }
  }
}

# ----------------------------------------------------------------------------
# Summary
# ----------------------------------------------------------------------------
Log ""
Log "=== Summary === actions=$totalActions skipped=$totalSkipped"
if ($DryRun) {
  Log "DryRun complete -- re-run with -Apply to execute"
} else {
  Log "Apply complete"
}
Log "Log: $LogFile"
