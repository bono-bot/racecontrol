#Requires -Version 5.1
<#
.SYNOPSIS
  Kaizen-class root-cause fix for james (.27) multi-homed-same-subnet broadcast risk.

.DESCRIPTION
  Background:
    james historically had Ethernet (192.168.31.27, static) and Wi-Fi both bound to
    192.168.31.0/24, doubling all broadcast/multicast traffic (mDNS, LLMNR, SSDP, WSD,
    NetBIOS) and suspected of triggering router crashes when both NICs were Up.

  This script applies the architectural fix (server-class hygiene), not a workaround:

    Stage 4   Disable Realtek LSOv2 (IPv4 + IPv6) on Ethernet
    Stage 5a  Stop + disable discovery services not needed on a venue server:
                Bonjour Service, SSDPSRV, FDResPub, fdPHost, lltdsvc, Browser
    Stage 5b  Disable LLMNR via Group Policy registry value
    Stage 5c  Disable NetBIOS-over-TCPIP on each IPv4 binding (Ethernet + Wi-Fi)
    Stage 5d  Hard-disable the Wi-Fi adapter (was previously only disconnected)

  Stages NOT automated here (need separate review):
    Stage 3   Scheduled task that auto-disables Wi-Fi on Ethernet UP event 10000
    Stage 6   Removal of stale Wintun virtual adapters (needs interactive review)
    Stage 7   ER7206 enterprise router as gateway (separate hardware work)

  Companion read-only diagnostic (no admin needed):
    scripts/multi-homing-storm-capture.ps1
    -- runs a per-second NIC stat + gateway ping capture during a Wi-Fi reconnect.

.PARAMETER Mode
  Status   Read-only inventory of every stage's current vs stored state. No admin.
  DryRun   Default. Logs what each stage would do. No changes. No admin.
  Apply    Executes all stages. Requires Administrator.
  Rollback Reverses every stage marked applied=true in the state file. Requires Administrator.

.PARAMETER Force
  Skip the interactive confirmation in Apply / Rollback.

.PARAMETER StateFile
  JSON file recording each stage's applied flag + previous state for rollback.
  Default: racecontrol/data/multi-homing-kaizen-state.json

.PARAMETER LogDir
  Directory for per-run timestamped log file. Default: racecontrol/data

.EXAMPLE
  powershell.exe -ExecutionPolicy Bypass -File .\james-multi-homing-kaizen-fix.ps1 -Mode Status

.EXAMPLE
  powershell.exe -ExecutionPolicy Bypass -File .\james-multi-homing-kaizen-fix.ps1 -Mode DryRun

.EXAMPLE
  # From an elevated PowerShell prompt:
  powershell.exe -ExecutionPolicy Bypass -File .\james-multi-homing-kaizen-fix.ps1 -Mode Apply

.EXAMPLE
  powershell.exe -ExecutionPolicy Bypass -File .\james-multi-homing-kaizen-fix.ps1 -Mode Rollback

.NOTES
  Pre-Apply safety:  refuses to run if Ethernet is not Up (would lose all connectivity).
  Idempotent:        Apply on an already-applied stage is a no-op.
  Reversible:        every stage records its prior value before changing it.
  Hypothesis status: duplicate-broadcast cause is SUSPECTED, not confirmed by pktmon.
                     Run multi-homing-storm-capture.ps1 first if confirmation is wanted.
#>

[CmdletBinding()]
param(
  [ValidateSet("Status","DryRun","Apply","Rollback")]
  [string]$Mode = "DryRun",
  [switch]$Force,
  [string]$StateFile = "C:\Users\bono\racingpoint\racecontrol\data\multi-homing-kaizen-state.json",
  [string]$LogDir = "C:\Users\bono\racingpoint\racecontrol\data",
  # -SkipWiFiDisable: skip Stage 5d (Wi-Fi adapter hard-disable). Use on machines
  # that are NOT multi-homed and where Wi-Fi may be needed as backup admin path
  # (e.g. server .23, POS .130). james uses the default (Wi-Fi disable IS run).
  [switch]$SkipWiFiDisable
)

$ErrorActionPreference = "Continue"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"
if (-not (Test-Path $LogDir)) { New-Item -ItemType Directory -Path $LogDir -Force | Out-Null }
$LogFile = Join-Path $LogDir "multi-homing-kaizen-$Mode-$timestamp.log"

# ----------------------------------------------------------------------------------
# Helpers
# ----------------------------------------------------------------------------------

function Write-Log {
  param(
    [Parameter(Mandatory=$true)][string]$Message,
    [string]$Level = "INFO"
  )
  $line = "[{0}] [{1,-8}] {2}" -f (Get-Date -Format "HH:mm:ss"), $Level, $Message
  Add-Content -Path $LogFile -Value $line -Encoding UTF8
  $color = switch ($Level) {
    "ERROR"    { "Red" }
    "WARN"     { "Yellow" }
    "DRYRUN"   { "Cyan" }
    "APPLY"    { "Green" }
    "ROLLBACK" { "Magenta" }
    "STATUS"   { "Gray" }
    default    { "White" }
  }
  Write-Host $line -ForegroundColor $color
}

function Test-IsAdmin {
  $id = [Security.Principal.WindowsIdentity]::GetCurrent()
  $p  = New-Object Security.Principal.WindowsPrincipal($id)
  return $p.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Read-State {
  if (Test-Path $StateFile) {
    try {
      $raw = Get-Content $StateFile -Raw -ErrorAction Stop
      return ($raw | ConvertFrom-Json -ErrorAction Stop)
    } catch {
      Write-Log "WARN: state file unreadable, starting fresh: $_" "WARN"
    }
  }
  return [pscustomobject]@{
    schema_version = 1
    applied_at     = $null
    rollback_at    = $null
    stages         = @()
  }
}

function Write-StateFile {
  param($State)
  $dir = Split-Path -Parent $StateFile
  if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
  $State | ConvertTo-Json -Depth 10 | Set-Content -Path $StateFile -Encoding UTF8
  Write-Log "state file written: $StateFile" "INFO"
}

function Get-StageEntry {
  param($State, [string]$Id)
  return ($State.stages | Where-Object { $_.id -eq $Id } | Select-Object -First 1)
}

function Set-StageEntry {
  param($State, [string]$Id, [bool]$Applied, $PreviousState)
  $existing = Get-StageEntry -State $State -Id $Id
  if ($existing) {
    $existing.applied        = $Applied
    $existing.previous_state = $PreviousState
    $existing.last_change    = (Get-Date).ToString("o")
  } else {
    $entry = [pscustomobject]@{
      id             = $Id
      applied        = $Applied
      previous_state = $PreviousState
      last_change    = (Get-Date).ToString("o")
    }
    $State.stages = @($State.stages) + $entry
  }
}

# ----------------------------------------------------------------------------------
# Stage 4 -- Disable Realtek LSOv2 on Ethernet
# ----------------------------------------------------------------------------------

function Invoke-StageLso {
  param([string]$Mode, $State, [string]$AdapterName = "Ethernet")
  $id = "S4-LSO-$AdapterName"
  $stored = Get-StageEntry -State $State -Id $id

  $nic = Get-NetAdapter -Name $AdapterName -ErrorAction SilentlyContinue
  if (-not $nic) { Write-Log "$id : adapter '$AdapterName' not found, skip" "WARN"; return }

  # Get-NetAdapterLso returns ONE object per adapter with IPv4Enabled / IPv6Enabled
  # properties (not multiple rows filterable by IPVersion). Earlier filter version
  # silently produced $null and broke the idempotency check.
  $lso = Get-NetAdapterLso -Name $AdapterName -ErrorAction SilentlyContinue
  $v4 = if ($lso) { [bool]$lso.IPv4Enabled } else { $null }
  $v6 = if ($lso) { [bool]$lso.IPv6Enabled } else { $null }

  switch ($Mode) {
    "Status"  { Write-Log "$id : current LSO IPv4=$v4 IPv6=$v6 ; stored.applied=$($stored.applied)" "STATUS" }
    "DryRun"  { Write-Log "$id : would Disable-NetAdapterLso -Name $AdapterName -IPv4 -IPv6 (current IPv4=$v4 IPv6=$v6)" "DRYRUN" }
    "Apply"   {
      if ($v4 -eq $false -and $v6 -eq $false) {
        Write-Log "$id : already disabled, no-op" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ ipv4 = $v4; ipv6 = $v6 })
        return
      }
      try {
        Disable-NetAdapterLso -Name $AdapterName -IPv4 -IPv6 -ErrorAction Stop
        Write-Log "$id : disabled LSO IPv4 + IPv6 on $AdapterName" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ ipv4 = $v4; ipv6 = $v6 })
      } catch { Write-Log "$id : Disable-NetAdapterLso FAILED: $_" "ERROR" }
    }
    "Rollback" {
      if (-not $stored -or -not $stored.applied) { Write-Log "$id : not previously applied, skip" "INFO"; return }
      $prev = $stored.previous_state
      try {
        if ($prev.ipv4) { Enable-NetAdapterLso -Name $AdapterName -IPv4 -ErrorAction Stop }
        if ($prev.ipv6) { Enable-NetAdapterLso -Name $AdapterName -IPv6 -ErrorAction Stop }
        Write-Log "$id : restored LSO IPv4=$($prev.ipv4) IPv6=$($prev.ipv6)" "ROLLBACK"
        Set-StageEntry -State $State -Id $id -Applied $false -PreviousState $null
      } catch { Write-Log "$id : Enable-NetAdapterLso FAILED: $_" "ERROR" }
    }
  }
}

# ----------------------------------------------------------------------------------
# Stage 5a -- Stop + disable discovery services
# ----------------------------------------------------------------------------------

function Invoke-StageService {
  param([string]$Mode, $State, [string]$ServiceName)
  $id = "S5a-Service-$ServiceName"
  $stored = Get-StageEntry -State $State -Id $id

  $svc = Get-Service -Name $ServiceName -ErrorAction SilentlyContinue
  if (-not $svc) {
    Write-Log "$id : service '$ServiceName' not installed, skip" "INFO"
    return
  }
  $curStatus    = $svc.Status
  $curStartType = (Get-Service -Name $ServiceName).StartType

  switch ($Mode) {
    "Status" { Write-Log "$id : status=$curStatus start=$curStartType ; stored.applied=$($stored.applied)" "STATUS" }
    "DryRun" { Write-Log "$id : would Stop-Service + Set-Service -StartupType Disabled (current status=$curStatus start=$curStartType)" "DRYRUN" }
    "Apply"  {
      if ($curStartType -eq "Disabled" -and $curStatus -eq "Stopped") {
        Write-Log "$id : already stopped + disabled, no-op" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ status = "$curStatus"; starttype = "$curStartType" })
        return
      }
      try {
        # Disable startup FIRST (so the service can't come back even if stop hangs).
        Set-Service -Name $ServiceName -StartupType Disabled -ErrorAction Stop
        # Stop asynchronously via sc.exe (Stop-Service -NoWait is PS7+; sc is PS5.1-safe).
        # Skip if service is already Stopped or already StopPending from a prior run.
        if ($curStatus -ne "Stopped" -and $curStatus -ne "StopPending") {
          $null = & sc.exe stop $ServiceName 2>&1
        }
        # Poll up to 8s for the service to leave Running. We do NOT fail if it stays
        # in StopPending -- SSDPSRV is a known offender that can take minutes to
        # fully stop while still being effectively disabled for our purposes.
        $deadline = (Get-Date).AddSeconds(8)
        do {
          $cur = (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue).Status
          if ($cur -in @("Stopped","StopPending")) { break }
          Start-Sleep -Milliseconds 500
        } while ((Get-Date) -lt $deadline)
        $finalStatus = (Get-Service -Name $ServiceName -ErrorAction SilentlyContinue).Status
        Write-Log "$id : disabled + stop submitted (final status=$finalStatus)" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ status = "$curStatus"; starttype = "$curStartType" })
      } catch { Write-Log "$id : FAILED: $_" "ERROR" }
    }
    "Rollback" {
      if (-not $stored -or -not $stored.applied) { Write-Log "$id : not previously applied, skip" "INFO"; return }
      $prev = $stored.previous_state
      try {
        # Map AutomaticDelayedStart -> Automatic (PS5.1 Set-Service compatibility).
        $restoreStart = $prev.starttype
        if ($restoreStart -eq "AutomaticDelayedStart") { $restoreStart = "Automatic" }
        Set-Service -Name $ServiceName -StartupType $restoreStart -ErrorAction Stop
        if ($prev.status -eq "Running") { Start-Service -Name $ServiceName -ErrorAction Continue }
        Write-Log "$id : restored start=$restoreStart status->$($prev.status)" "ROLLBACK"
        Set-StageEntry -State $State -Id $id -Applied $false -PreviousState $null
      } catch { Write-Log "$id : FAILED: $_" "ERROR" }
    }
  }
}

# ----------------------------------------------------------------------------------
# Stage 5b -- Disable LLMNR via registry
# ----------------------------------------------------------------------------------

function Invoke-StageLlmnr {
  param([string]$Mode, $State)
  $id = "S5b-LLMNR"
  $regPath  = "HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient"
  $valName  = "EnableMulticast"
  $stored   = Get-StageEntry -State $State -Id $id

  $existed = $false
  $current = $null
  if (Test-Path $regPath) {
    $prop = Get-ItemProperty -Path $regPath -Name $valName -ErrorAction SilentlyContinue
    if ($prop) { $existed = $true; $current = $prop.$valName }
  }

  switch ($Mode) {
    "Status"  { Write-Log "$id : EnableMulticast existed=$existed value=$current ; stored.applied=$($stored.applied)" "STATUS" }
    "DryRun"  { Write-Log "$id : would set $regPath\$valName = 0 (DWORD) (existed=$existed value=$current)" "DRYRUN" }
    "Apply"   {
      if ($current -eq 0) {
        Write-Log "$id : already 0, no-op" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ existed = $existed; value = $current })
        return
      }
      try {
        if (-not (Test-Path $regPath)) { New-Item -Path $regPath -Force | Out-Null }
        Set-ItemProperty -Path $regPath -Name $valName -Value 0 -Type DWord -ErrorAction Stop
        Write-Log "$id : EnableMulticast=0 written" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ existed = $existed; value = $current })
      } catch { Write-Log "$id : FAILED: $_" "ERROR" }
    }
    "Rollback" {
      if (-not $stored -or -not $stored.applied) { Write-Log "$id : not previously applied, skip" "INFO"; return }
      $prev = $stored.previous_state
      try {
        if (-not $prev.existed) {
          Remove-ItemProperty -Path $regPath -Name $valName -ErrorAction SilentlyContinue
          Write-Log "$id : value deleted (was absent before)" "ROLLBACK"
        } else {
          Set-ItemProperty -Path $regPath -Name $valName -Value $prev.value -Type DWord -ErrorAction Stop
          Write-Log "$id : restored EnableMulticast=$($prev.value)" "ROLLBACK"
        }
        Set-StageEntry -State $State -Id $id -Applied $false -PreviousState $null
      } catch { Write-Log "$id : FAILED: $_" "ERROR" }
    }
  }
}

# ----------------------------------------------------------------------------------
# Stage 5c -- Disable NetBIOS-over-TCPIP per IPv4 binding
# ----------------------------------------------------------------------------------

function Invoke-StageNetbios {
  param([string]$Mode, $State, [string]$AdapterName)
  $id = "S5c-NetBIOS-$AdapterName"
  $stored = Get-StageEntry -State $State -Id $id

  $nic = Get-NetAdapter -Name $AdapterName -ErrorAction SilentlyContinue
  if (-not $nic) { Write-Log "$id : adapter '$AdapterName' not found, skip" "INFO"; return }

  $netCfg = Get-WmiObject Win32_NetworkAdapterConfiguration -Filter "InterfaceIndex=$($nic.InterfaceIndex)" -ErrorAction SilentlyContinue
  if (-not $netCfg) { Write-Log "$id : no Win32_NetworkAdapterConfiguration for InterfaceIndex=$($nic.InterfaceIndex), skip" "WARN"; return }
  $current = $netCfg.TcpipNetbiosOptions   # 0=DHCP-default 1=enable 2=disable

  switch ($Mode) {
    "Status" { Write-Log "$id : TcpipNetbiosOptions=$current ; stored.applied=$($stored.applied)" "STATUS" }
    "DryRun" { Write-Log "$id : would call SetTcpipNetbios(2) on $AdapterName (current=$current)" "DRYRUN" }
    "Apply"  {
      if ($current -eq 2) {
        Write-Log "$id : already disabled, no-op" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ value = $current })
        return
      }
      $r = $netCfg.SetTcpipNetbios(2)
      # ReturnValue 0 = success. 1 = success, reboot required. Treat both as applied.
      if ($r.ReturnValue -in 0,1) {
        Write-Log "$id : SetTcpipNetbios(2) -> ReturnValue=$($r.ReturnValue)" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ value = $current })
      } else {
        Write-Log "$id : SetTcpipNetbios(2) FAILED ReturnValue=$($r.ReturnValue)" "ERROR"
      }
    }
    "Rollback" {
      if (-not $stored -or -not $stored.applied) { Write-Log "$id : not previously applied, skip" "INFO"; return }
      $prev = [uint32]$stored.previous_state.value
      $r = $netCfg.SetTcpipNetbios($prev)
      Write-Log "$id : restore SetTcpipNetbios($prev) -> ReturnValue=$($r.ReturnValue)" "ROLLBACK"
      Set-StageEntry -State $State -Id $id -Applied $false -PreviousState $null
    }
  }
}

# ----------------------------------------------------------------------------------
# Stage 5d -- Hard-disable Wi-Fi adapter
# ----------------------------------------------------------------------------------

function Invoke-StageDisableWifi {
  param([string]$Mode, $State, [string]$AdapterName = "Wi-Fi")
  $id = "S5d-WiFi-Disable"
  $stored = Get-StageEntry -State $State -Id $id

  $nic = Get-NetAdapter -Name $AdapterName -ErrorAction SilentlyContinue
  if (-not $nic) { Write-Log "$id : adapter '$AdapterName' not found, skip" "INFO"; return }
  $curStatus = $nic.Status        # Up | Disconnected | Disabled
  $curAdmin  = $nic.AdminStatus   # Up | Down

  switch ($Mode) {
    "Status" { Write-Log "$id : status=$curStatus admin=$curAdmin ; stored.applied=$($stored.applied)" "STATUS" }
    "DryRun" { Write-Log "$id : would Disable-NetAdapter -Name '$AdapterName' (current status=$curStatus admin=$curAdmin)" "DRYRUN" }
    "Apply"  {
      if ($curAdmin -eq "Down") {
        Write-Log "$id : already admin Down, no-op" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ status = "$curStatus"; admin = "$curAdmin" })
        return
      }
      try {
        Disable-NetAdapter -Name $AdapterName -Confirm:$false -ErrorAction Stop
        Write-Log "$id : disabled $AdapterName" "APPLY"
        Set-StageEntry -State $State -Id $id -Applied $true -PreviousState ([pscustomobject]@{ status = "$curStatus"; admin = "$curAdmin" })
      } catch { Write-Log "$id : FAILED: $_" "ERROR" }
    }
    "Rollback" {
      if (-not $stored -or -not $stored.applied) { Write-Log "$id : not previously applied, skip" "INFO"; return }
      try {
        Enable-NetAdapter -Name $AdapterName -Confirm:$false -ErrorAction Stop
        Write-Log "$id : enabled $AdapterName" "ROLLBACK"
        Set-StageEntry -State $State -Id $id -Applied $false -PreviousState $null
      } catch { Write-Log "$id : FAILED: $_" "ERROR" }
    }
  }
}

# ----------------------------------------------------------------------------------
# Main
# ----------------------------------------------------------------------------------

Write-Log "=== james-multi-homing-kaizen-fix.ps1 starting (Mode=$Mode) ==="
Write-Log "host=$env:COMPUTERNAME user=$env:USERNAME pid=$PID"
Write-Log "log=$LogFile"
Write-Log "state=$StateFile"

# Admin gate for write modes.
if ($Mode -in @("Apply","Rollback")) {
  if (-not (Test-IsAdmin)) {
    Write-Log "ERROR: -Mode $Mode requires Administrator. Re-run from an elevated PowerShell prompt." "ERROR"
    exit 2
  }
}

# Connectivity safety check before Apply -- never disable Wi-Fi while Ethernet is down.
if ($Mode -eq "Apply") {
  $eth = Get-NetAdapter -Name "Ethernet" -ErrorAction SilentlyContinue
  if (-not $eth) {
    Write-Log "ERROR: Ethernet adapter not found. Refusing to proceed." "ERROR"
    exit 3
  }
  if ($eth.Status -ne "Up") {
    Write-Log "ERROR: Ethernet status='$($eth.Status)', not 'Up'. Refusing to proceed (would lose connectivity)." "ERROR"
    exit 3
  }
  Write-Log "pre-flight: Ethernet status=Up, OK to proceed" "INFO"
}

# Confirmation gate (skipped with -Force).
if ($Mode -in @("Apply","Rollback") -and -not $Force) {
  Write-Host ""
  Write-Host "About to run Mode=$Mode on $env:COMPUTERNAME. Continue? [y/N] " -NoNewline -ForegroundColor Yellow
  $ans = Read-Host
  if ($ans -ne "y") { Write-Log "user declined, exiting" "INFO"; exit 1 }
}

$state = Read-State

# Stage 4 -- LSO
Invoke-StageLso        -Mode $Mode -State $state -AdapterName "Ethernet"

# Stage 5a -- discovery services
# "Browser" was previously in this list but on Win11 it resolves to the `bowser`
# kernel driver -- not a configurable Win32 service, so Set-Service rejects it.
# Removed 2026-05-06 after the first Apply run logged a persistent ERROR row.
$services = @("Bonjour Service","SSDPSRV","FDResPub","fdPHost","lltdsvc")
foreach ($svc in $services) { Invoke-StageService -Mode $Mode -State $state -ServiceName $svc }

# Stage 5b -- LLMNR
Invoke-StageLlmnr      -Mode $Mode -State $state

# Stage 5c -- NetBIOS-over-TCPIP per binding
Invoke-StageNetbios    -Mode $Mode -State $state -AdapterName "Ethernet"
Invoke-StageNetbios    -Mode $Mode -State $state -AdapterName "Wi-Fi"

# Stage 5d -- hard-disable Wi-Fi adapter (do this LAST so prior stages still see Wi-Fi binding)
# Skipped on non-multi-homed targets where Wi-Fi is a useful backup admin path.
if ($SkipWiFiDisable) {
  Write-Log "S5d-WiFi-Disable : SKIPPED (-SkipWiFiDisable flag set)" "INFO"
} else {
  Invoke-StageDisableWifi -Mode $Mode -State $state -AdapterName "Wi-Fi"
}

# Persist state for write modes.
if ($Mode -eq "Apply")    { $state.applied_at  = (Get-Date).ToString("o"); Write-StateFile $state }
if ($Mode -eq "Rollback") { $state.rollback_at = (Get-Date).ToString("o"); Write-StateFile $state }

Write-Log "=== Done (Mode=$Mode) ==="
switch ($Mode) {
  "Status"   { Write-Log "no changes were made (Status mode)" "INFO" }
  "DryRun"   { Write-Log "no changes were made (DryRun mode). To apply, re-run elevated: -Mode Apply" "INFO" }
  "Apply"    {
    Write-Log "next steps:" "INFO"
    Write-Log "  1. Optional: re-enable Wi-Fi briefly and run scripts/multi-homing-storm-capture.ps1 to verify reduced TX/RX rate" "INFO"
    Write-Log "  2. Verify Ethernet still reachable: Test-Connection 192.168.31.1 -Count 4" "INFO"
    Write-Log "  3. Reverse with: -Mode Rollback (uses state file $StateFile)" "INFO"
  }
  "Rollback" { Write-Log "rollback complete; verify with: -Mode Status" "INFO" }
}
