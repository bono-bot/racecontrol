#Requires -Version 5.1
<#
.SYNOPSIS
  Remove GIGABYTE Smart Backup fleet-wide.
  DryRun by default; pass -Apply to execute.

.DESCRIPTION
  Permanent fix for HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce\RPMKickstart
  re-queueing every boot. Smart Backup is the parent that re-adds the RunOnce entry.

  REMOVAL STRATEGY (validated on pods 3, 5, 6 on 2026-05-06):
    msiexec /X is NOT used because the LocalPackage cache is empty across the fleet
    (under HKLM\...\Installer\UserData\S-1-5-18\Products\<obfuscated>) - returns 1619.
    The actual blocker for cleanup is `RPMDaemon.exe`, the Smart Backup daemon, which
    has a registry change notification handler on HKLM\...\RunOnce that re-adds
    `RPMKickstart` within ~9ms of any delete. Once the daemon is killed, no respawn
    mechanism is observed (no service, no scheduled task, no Run/RunOnce entry).

  Sequence:
    1. Kill RPM* processes (RPMDaemon, RPMExec, RPMMgr, RPMKickstart, RPMKickstartEx)
    2. Wait 2s, verify dead
    3. Delete install dir `C:\Program Files\GIGABYTE\Smart Backup\`
    4. Delete ARP key `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\{GUID}`
    5. Delete Installer Products key `HKLM\SOFTWARE\Classes\Installer\Products\<obfuscated>`
    6. Delete UserData key `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\<obfuscated>`
    7. Delete RunOnce value `RPMKickstart`
    8. Wait 5s, verify no respawn / no re-add

  Reversible: yes. Reinstall from GIGABYTE driver bundle re-creates everything.

  Implementation: writes worker .ps1 to local temp, SCPs to remote `C:\Temp\`, runs
  via SSH `-File`. Avoids the inline-PS-over-SSH anti-pattern.

.PARAMETER Pod
  Pod number(s) to target. Comma-separated (e.g. "3,5,6") or "all" for pods 1-8.

.PARAMETER DryRun
  Default. Logs action plan only.

.PARAMETER Apply
  Execute removal on each target pod.

.EXAMPLE
  .\uninstall-smart-backup-fleet.ps1 -Pod 3,5,6 -Apply
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
$ObfuscatedCode = "FC5AF1CBF63A16C46983900D4B130B60"
$InstallDir = "C:\Program Files\GIGABYTE\Smart Backup"
$RemoteWorker = "C:\Temp\smartbackup-remove-worker.ps1"
$LocalWorker  = Join-Path $env:TEMP "smartbackup-remove-worker.ps1"

# --- Worker script (parameterized) ---
$workerLines = @(
  'param(',
  '  [string]$ProductCode,',
  '  [string]$ObfuscatedCode,',
  '  [string]$InstallDir,',
  '  [switch]$Apply',
  ')',
  '$ErrorActionPreference = "Continue"',
  '',
  '$arpKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\$ProductCode"',
  '$prodKey = "HKLM:\SOFTWARE\Classes\Installer\Products\$ObfuscatedCode"',
  '$userDataKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Installer\UserData\S-1-5-18\Products\$ObfuscatedCode"',
  '$runOnceKey = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce"',
  '',
  'Write-Output "=== Pre-state ==="',
  'Write-Output ("ARP: " + (Test-Path $arpKey))',
  'Write-Output ("INSTALLER_PRODUCTS: " + (Test-Path $prodKey))',
  'Write-Output ("USERDATA: " + (Test-Path $userDataKey))',
  'Write-Output ("INSTALLDIR: " + (Test-Path $InstallDir))',
  '$ro = Get-ItemProperty $runOnceKey -ErrorAction SilentlyContinue',
  'if ($ro -and $ro.RPMKickstart) { Write-Output ("RUNONCE: " + $ro.RPMKickstart) } else { Write-Output "RUNONCE: absent" }',
  '$rpm = Get-Process | Where-Object { $_.ProcessName -like "RPM*" } | Select-Object ProcessName, Id',
  'if ($rpm) { Write-Output ("RPM_PROCS: " + (($rpm | ForEach-Object { $_.ProcessName + "(pid=" + $_.Id + ")" }) -join ", ")) } else { Write-Output "RPM_PROCS: none" }',
  '',
  '$anyTrace = (Test-Path $arpKey) -or (Test-Path $prodKey) -or (Test-Path $userDataKey) -or (Test-Path $InstallDir) -or ($ro -and $ro.RPMKickstart) -or ($rpm)',
  'if (-not $anyTrace) { Write-Output "RESULT: ALREADY_CLEAN"; exit 0 }',
  'if (-not $Apply) { Write-Output "RESULT: DRYRUN_NO_ACTION"; exit 0 }',
  '',
  'Write-Output ""',
  'Write-Output "=== Step 1: Kill RPM processes ==="',
  '$killed = @()',
  'foreach ($name in @("RPMDaemon","RPMExec","RPMMgr","RPMKickstart","RPMKickstartEx")) {',
  '  $procs = Get-Process -Name $name -ErrorAction SilentlyContinue',
  '  if ($procs) {',
  '    foreach ($p in $procs) {',
  '      try { Stop-Process -Id $p.Id -Force -ErrorAction Stop; $killed += "$name(pid=$($p.Id))" }',
  '      catch { $killed += "$name(pid=$($p.Id))_KILL_FAILED:$($_.Exception.Message)" }',
  '    }',
  '  }',
  '}',
  'if ($killed) { Write-Output ("KILLED: " + ($killed -join ", ")) } else { Write-Output "No RPM processes" }',
  'Start-Sleep -Seconds 2',
  '',
  'Write-Output ""',
  'Write-Output "=== Step 2: Remove install dir ==="',
  'if (Test-Path $InstallDir) {',
  '  Remove-Item -Path $InstallDir -Recurse -Force -ErrorAction SilentlyContinue',
  '  Write-Output ("INSTALLDIR_REMOVED: " + (-not (Test-Path $InstallDir)))',
  '} else {',
  '  Write-Output "INSTALLDIR_ALREADY_GONE"',
  '}',
  '',
  'Write-Output ""',
  'Write-Output "=== Step 3: Remove registry keys ==="',
  'foreach ($k in @($arpKey, $prodKey, $userDataKey)) {',
  '  if (Test-Path $k) {',
  '    Remove-Item -Path $k -Recurse -Force -ErrorAction SilentlyContinue',
  '    Write-Output ("KEY_REMOVED: " + $k + " -> " + (-not (Test-Path $k)))',
  '  } else {',
  '    Write-Output ("KEY_ABSENT: " + $k)',
  '  }',
  '}',
  '',
  'Write-Output ""',
  'Write-Output "=== Step 4: Remove RunOnce RPMKickstart ==="',
  'Remove-ItemProperty -Path $runOnceKey -Name RPMKickstart -Force -ErrorAction SilentlyContinue',
  '$ro2 = Get-ItemProperty $runOnceKey -ErrorAction SilentlyContinue',
  'if ($ro2 -and $ro2.RPMKickstart) { Write-Output "RUNONCE_STILL_PRESENT" } else { Write-Output "RUNONCE_REMOVED" }',
  '',
  'Write-Output ""',
  'Write-Output "=== Step 5: Wait 5s, verify no respawn ==="',
  'Start-Sleep -Seconds 5',
  '$rpm2 = Get-Process | Where-Object { $_.ProcessName -like "RPM*" } | Select-Object ProcessName, Id',
  'if ($rpm2) {',
  '  Write-Output ("RPM_RESPAWNED: " + (($rpm2 | ForEach-Object { $_.ProcessName + "(pid=" + $_.Id + ")" }) -join ", "))',
  '} else {',
  '  Write-Output "NO_RPM_RESPAWN"',
  '}',
  '$ro3 = Get-ItemProperty $runOnceKey -ErrorAction SilentlyContinue',
  'if ($ro3 -and $ro3.RPMKickstart) { Write-Output ("RUNONCE_RE_ADDED: " + $ro3.RPMKickstart) } else { Write-Output "RUNONCE_STAYS_DELETED" }',
  'if (Test-Path $InstallDir) { Write-Output "INSTALLDIR_AFTER_5s: STILL_PRESENT" } else { Write-Output "INSTALLDIR_AFTER_5s: GONE" }',
  '',
  '$arpFinal = Test-Path $arpKey',
  '$prodFinal = Test-Path $prodKey',
  '$userDataFinal = Test-Path $userDataKey',
  '$dirFinal = Test-Path $InstallDir',
  '$roFinal = Get-ItemProperty $runOnceKey -ErrorAction SilentlyContinue',
  '$runOnceFinal = if ($roFinal -and $roFinal.RPMKickstart) { $roFinal.RPMKickstart } else { $null }',
  'if ((-not $arpFinal) -and (-not $prodFinal) -and (-not $userDataFinal) -and (-not $dirFinal) -and (-not $runOnceFinal) -and (-not $rpm2)) {',
  '  Write-Output "RESULT: REMOVED_OK"',
  '} else {',
  '  Write-Output "RESULT: PARTIAL_SOME_TRACES_REMAIN"',
  '}'
)
$workerLines | Set-Content -Path $LocalWorker -Encoding ASCII

# Resolve pod list
$pods = if ($Pod -eq "all") { 1..8 } else { $Pod -split ',' | ForEach-Object { [int]$_.Trim() } }

$ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
$mode = if ($Apply) { "APPLY" } else { "DRYRUN" }

Write-Host ""
Write-Host "Smart Backup Fleet Removal ($mode) - $ts"
Write-Host "Targets: pods $($pods -join ', ')"
Write-Host ""

foreach ($n in $pods) {
  $podHost = "pod$n"
  Write-Host "===== $podHost ====="

  ssh -o ConnectTimeout=8 $podHost "if not exist C:\Temp mkdir C:\Temp" 2>&1 | Out-Null
  $scpOut = scp -o ConnectTimeout=8 $LocalWorker "${podHost}:${RemoteWorker}" 2>&1
  if ($LASTEXITCODE -ne 0) {
    Write-Host "  SCP FAILED: $scpOut"
    Write-Host ""
    continue
  }

  $applyFlag = if ($Apply) { "-Apply" } else { "" }
  $cmd = "powershell -NoProfile -ExecutionPolicy Bypass -File $RemoteWorker -ProductCode '$ProductCode' -ObfuscatedCode '$ObfuscatedCode' -InstallDir '$InstallDir' $applyFlag"

  Write-Host "  Running worker..."
  $result = ssh -o ConnectTimeout=120 $podHost "$cmd" 2>&1
  foreach ($line in $result) { Write-Host "  $line" }
  Write-Host ""
}

Write-Host "Done. Mode: $mode"
if ($DryRun) { Write-Host "To execute: rerun with -Apply" }
