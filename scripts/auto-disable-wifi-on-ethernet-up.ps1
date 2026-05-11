#Requires -Version 5.1
<#
.SYNOPSIS
  Stage 3 of the multi-homing kaizen plan: handler invoked by a NetworkProfile
  event-triggered scheduled task. If Ethernet is Up AND Wi-Fi is admin Up,
  disable Wi-Fi to prevent re-creation of the multi-homed-same-subnet condition.

.DESCRIPTION
  Trigger: Microsoft-Windows-NetworkProfile/Operational EventID=10000
  Runs as SYSTEM (registered by setup-auto-disable-wifi-task.ps1). Idempotent:
  if Wi-Fi is already admin Down, exits cleanly. If Ethernet is not Up,
  exits cleanly (no spurious disables on transient events).

  Designed for james (.27) where Wi-Fi was hard-disabled by Stage 5d of the
  kaizen-fix. This task is insurance: any time the OS auto-re-enables Wi-Fi
  (Windows update, profile reset, manual click), the next Ethernet UP event
  triggers re-disable within seconds.

.LOGGING
  Appends to C:\Users\bono\racingpoint\racecontrol\data\auto-disable-wifi.log
  (single file, not per-run timestamped — keep history compact).
#>

$ErrorActionPreference = "Continue"
$logFile = "C:\Users\bono\racingpoint\racecontrol\data\auto-disable-wifi.log"
$logDir  = Split-Path -Parent $logFile
if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir -Force | Out-Null }

function Log {
  param([string]$Msg, [string]$Level = "INFO")
  $line = "[{0}] [{1,-6}] {2}" -f (Get-Date -Format "yyyy-MM-dd HH:mm:ss"), $Level, $Msg
  Add-Content -Path $logFile -Value $line -Encoding UTF8
}

Log "=== handler triggered (PID=$PID, user=$env:USERNAME) ==="

$eth  = Get-NetAdapter -Name "Ethernet" -ErrorAction SilentlyContinue
$wifi = Get-NetAdapter -Name "Wi-Fi"   -ErrorAction SilentlyContinue

if (-not $eth)  { Log "Ethernet adapter not found, exiting" "WARN" ; exit 0 }
if (-not $wifi) { Log "Wi-Fi adapter not found (already cleaned up?), exiting" "INFO" ; exit 0 }

Log ("Ethernet: status={0} admin={1}" -f $eth.Status, $eth.AdminStatus)
Log ("Wi-Fi   : status={0} admin={1}" -f $wifi.Status, $wifi.AdminStatus)

# Only act if Ethernet is genuinely Up (avoid acting on Wi-Fi-only network events).
if ($eth.Status -ne "Up") {
  Log "Ethernet not Up, no multi-home risk, exiting" "INFO"
  exit 0
}

# If Wi-Fi is already admin Down, nothing to do.
if ($wifi.AdminStatus -eq "Down") {
  Log "Wi-Fi already admin Down, no action needed" "INFO"
  exit 0
}

# Both Ethernet Up and Wi-Fi admin Up -> disable Wi-Fi.
try {
  Log "Ethernet Up + Wi-Fi admin Up -> disabling Wi-Fi adapter (multi-home prevention)" "ACTION"
  Disable-NetAdapter -Name "Wi-Fi" -Confirm:$false -ErrorAction Stop
  Log "Wi-Fi disabled successfully" "OK"
  exit 0
} catch {
  Log ("Disable-NetAdapter failed: " + $_.Exception.Message) "ERROR"
  exit 1
}
