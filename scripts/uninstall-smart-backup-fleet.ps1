#Requires -Version 5.1
<#
.SYNOPSIS
  Uninstall GIGABYTE Smart Backup MSI fleet-wide.
  DryRun by default; pass -Apply to execute.

.DESCRIPTION
  Permanent fix for HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce\RPMKickstart
  re-queueing every boot. Smart Backup is the parent that re-adds the RunOnce entry
  via MSI startup repair. Cleanup 2 in pod-uniformity-cleanup.ps1 removes the RunOnce
  value but the parent re-queues it next boot. Removing Smart Backup itself ends the loop.

  Fleet evidence (2026-05-06 audit):
    - Smart Backup MSI ProductCode: {BC1FA5CF-A36F-4C61-9638-09D0B431B006}
    - Confirmed on pods 3, 5, 6 (DisplayVersion 3.24.0829.1)
    - Pod 3 inspection: no service / no process / no install dir at common paths
      (dormant; only acts via RunOnce on boot)

  Class: bloatware uninstall (Captain-physical-action item per POD-SPEC §6).
  Reversible: yes, can be reinstalled from GIGABYTE driver bundle if needed.

.PARAMETER Pod
  Pod number(s) to target. Comma-separated (e.g. "3,5,6") or "all" for pods 1-8.

.PARAMETER DryRun
  Default. Logs action plan only.

.PARAMETER Apply
  Execute uninstall on each target pod.

.EXAMPLE
  .\uninstall-smart-backup-fleet.ps1 -Pod 3
  # Dry run preview for pod 3

.EXAMPLE
  .\uninstall-smart-backup-fleet.ps1 -Pod 3,5,6 -Apply
  # Uninstall on pods 3, 5, 6 sequentially

.EXAMPLE
  .\uninstall-smart-backup-fleet.ps1 -Pod all -Apply
  # Uninstall fleet-wide (only pods that have it)

.NOTES
  Author: james (2026-05-06)
  Authorization required: yes (modifies installed software on production pods)
  Tested: not tested at authoring time (DryRun-only validation)
#>

[CmdletBinding()]
param(
  [Parameter(Mandatory=$true)]
  [string]$Pod,
  [switch]$DryRun,
  [switch]$Apply
)

if (-not $Apply) { $DryRun = $true }

$ProductCode = "{BC1FA5CF-A36F-4C61-9638-09D0B431B006}"
$RemoteLog   = "C:\RacingPoint\smartbackup-uninstall.log"

# Resolve pod list
$pods = if ($Pod -eq "all") {
  1..8
} else {
  $Pod -split ',' | ForEach-Object { [int]$_.Trim() }
}

$ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
$mode = if ($Apply) { "APPLY" } else { "DRYRUN" }

Write-Host ""
Write-Host "Smart Backup Fleet Uninstall ($mode) - $ts"
Write-Host "Targets: pods $($pods -join ', ')"
Write-Host "ProductCode: $ProductCode"
Write-Host ""

foreach ($n in $pods) {
  $podHost = "pod$n"
  Write-Host "===== $podHost ====="

  # Step 1 - check presence (read-only)
  $checkScript = @"
`$msi = Get-ItemProperty HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\* -ErrorAction SilentlyContinue |
  Where-Object { `$_.PSChildName -eq '$ProductCode' -or `$_.DisplayName -like '*Smart Backup*' }
if (`$msi) {
  Write-Output ("PRESENT: " + `$msi.DisplayName + " v" + `$msi.DisplayVersion)
} else {
  Write-Output "ABSENT"
}
`$ro = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce' -ErrorAction SilentlyContinue
if (`$ro -and `$ro.RPMKickstart) {
  Write-Output ("RPMKickstart_RUNONCE: " + `$ro.RPMKickstart)
} else {
  Write-Output "RPMKickstart_RUNONCE: absent"
}
"@

  $check = ssh -o ConnectTimeout=8 $podHost "powershell -NoProfile -Command `"$checkScript`"" 2>&1
  Write-Host "  $check"

  if ($check -notmatch "PRESENT:") {
    Write-Host "  SKIP: Smart Backup not present on $podHost"
    Write-Host ""
    continue
  }

  if ($DryRun) {
    Write-Host "  DRYRUN: would run msiexec /X $ProductCode /qn /norestart on $podHost"
    Write-Host "  DRYRUN: log -> $RemoteLog"
    Write-Host ""
    continue
  }

  # Step 2 - apply uninstall
  $applyScript = @"
`$proc = Start-Process -FilePath msiexec.exe `
  -ArgumentList '/X$ProductCode','/qn','/norestart','/L*v','$RemoteLog' `
  -Wait -PassThru -NoNewWindow
Write-Output ('ExitCode=' + `$proc.ExitCode)
`$still = Get-ItemProperty HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\* -ErrorAction SilentlyContinue |
  Where-Object { `$_.PSChildName -eq '$ProductCode' -or `$_.DisplayName -like '*Smart Backup*' }
if (`$still) { Write-Output 'STILL_PRESENT' } else { Write-Output 'UNINSTALLED_OK' }
`$ro = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce' -ErrorAction SilentlyContinue
if (`$ro -and `$ro.RPMKickstart) {
  Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce' -Name RPMKickstart -Force
  Write-Output 'RPMKickstart_RUNONCE: removed_post_uninstall'
} else {
  Write-Output 'RPMKickstart_RUNONCE: already_clear'
}
"@

  Write-Host "  APPLYING uninstall..."
  $result = ssh -o ConnectTimeout=60 $podHost "powershell -NoProfile -Command `"$applyScript`"" 2>&1
  Write-Host "  $result"
  Write-Host ""
}

Write-Host ""
Write-Host "Done. Mode: $mode"
if ($DryRun) {
  Write-Host "To execute: rerun with -Apply"
}
