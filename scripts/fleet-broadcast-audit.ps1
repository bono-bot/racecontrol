#Requires -Version 5.1
<#
.SYNOPSIS
  Portable read-only fleet hygiene audit. Inventory broadcast / discovery state on
  any Racing Point Windows machine without modifying anything.

.DESCRIPTION
  Collects the same state classes the james-multi-homing-kaizen-fix targets:
    1. Identity        hostname / OS / uptime / Tailscale node + IP
    2. NIC inventory   per-adapter Status / AdminStatus / IP / gateway / MAC, plus
                       multi-home-same-subnet detection
    3. LSO state       per Realtek/Intel/etc adapter (IPv4/IPv6/V1)
    4. Discovery       Bonjour Service / SSDPSRV / FDResPub / fdPHost / lltdsvc
    5. LLMNR           HKLM Group Policy EnableMulticast value
    6. NetBIOS-TCPIP   per IP-enabled adapter binding (0=DHCP / 1=on / 2=off)
    7. Tailscale       Wintun adapter count + likely-ghost detection

  Output:
    - Console human-readable summary (color-coded)
    - JSON file at %TEMP%\fleet-audit-<hostname>-<yyyyMMdd-HHmmss>.json
      containing every observation for cross-machine comparison.

  Strictly read-only. No admin required. No external dependencies. Safe to run
  on production. Uses only Get-* / Test-Path / sc.exe query cmdlets. Idempotent
  by definition (no state mutation).

.PARAMETER OutDir
  Directory for the JSON dump. Default: $env:TEMP

.PARAMETER ConsoleOnly
  Skip writing the JSON file; print only.

.EXAMPLE
  powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\fleet-broadcast-audit.ps1

.EXAMPLE
  # Then upload the JSON back to james for cross-machine comparison:
  scp $env:TEMP\fleet-audit-*.json james:/c/Users/bono/racingpoint/racecontrol/data/

.NOTES
  Run from PowerShell on any RP target (Server .23 / POS .130 / Pods / Spectator /
  James for self-comparison). On a pod, copy via rc-sentry then invoke via /exec
  with `cmd /c powershell -NoProfile -ExecutionPolicy Bypass -File <path>`.
#>

[CmdletBinding()]
param(
  [string]$OutDir = $env:TEMP,
  [switch]$ConsoleOnly
)

$ErrorActionPreference = "Continue"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

function Write-Section {
  param([string]$Title)
  Write-Host ""
  Write-Host ("=== {0} ===" -f $Title) -ForegroundColor Cyan
}

function Write-KV {
  param([string]$K, $V, [string]$Color = "Gray")
  Write-Host ("  {0,-32} {1}" -f $K, $V) -ForegroundColor $Color
}

function Test-PrivateIPv4 {
  param([string]$Ip)
  if (-not $Ip) { return $false }
  return ($Ip -match '^(10\.|192\.168\.|172\.(1[6-9]|2\d|3[01])\.)')
}

# ---------------------------------------------------------------------------
# Collection
# ---------------------------------------------------------------------------

$audit = [ordered]@{
  schema_version = 1
  collected_at   = (Get-Date).ToString("o")
  hostname       = $env:COMPUTERNAME
  user           = $env:USERNAME
}

# 1. Identity ---------------------------------------------------------------
$os = Get-CimInstance Win32_OperatingSystem
$audit.os = [ordered]@{
  caption        = $os.Caption
  build          = $os.BuildNumber
  last_boot_utc  = $os.LastBootUpTime.ToUniversalTime().ToString("o")
  uptime_minutes = [int]((Get-Date) - $os.LastBootUpTime).TotalMinutes
}

# Tailscale (best-effort, no fail if absent)
$tsCli = "C:\Program Files\Tailscale\tailscale.exe"
if (Test-Path $tsCli) {
  try {
    $tsStatus = & $tsCli status --json 2>$null | ConvertFrom-Json
    $self = $tsStatus.Self
    $audit.tailscale = [ordered]@{
      installed   = $true
      hostname    = $self.HostName
      dnsname     = $self.DNSName
      tailnet_ip  = ($self.TailscaleIPs | Select-Object -First 1)
      online      = $self.Online
      backend     = $tsStatus.BackendState
    }
  } catch {
    $audit.tailscale = @{ installed = $true; query_failed = "$_" }
  }
} else {
  $audit.tailscale = @{ installed = $false }
}

# 2. NIC inventory ----------------------------------------------------------
$nicRows = @()
foreach ($n in (Get-NetAdapter -ErrorAction SilentlyContinue)) {
  $cfg = Get-WmiObject Win32_NetworkAdapterConfiguration -Filter "InterfaceIndex=$($n.InterfaceIndex)" -ErrorAction SilentlyContinue
  $ipv4Addrs = @($cfg.IPAddress | Where-Object { $_ -and $_ -match '^\d+\.\d+\.\d+\.\d+$' })
  $gw = @($cfg.DefaultIPGateway)
  $row = [ordered]@{
    name              = $n.Name
    if_desc           = $n.InterfaceDescription
    if_index          = $n.InterfaceIndex
    mac               = $n.MacAddress
    status            = "$($n.Status)"
    admin_status      = "$($n.AdminStatus)"
    media_state       = "$($n.MediaConnectionState)"
    link_speed        = "$($n.LinkSpeed)"
    is_virtual        = $n.Virtual
    ipv4              = $ipv4Addrs
    default_gateway   = $gw
    netbios_option    = if ($cfg) { $cfg.TcpipNetbiosOptions } else { $null }
    ipv4_subnet_first = if ($ipv4Addrs.Count -gt 0) { ($ipv4Addrs[0] -replace '\.\d+$','') } else { $null }
  }
  $nicRows += [pscustomobject]$row
}
$audit.nics = $nicRows

# Multi-home-same-subnet detection
$subnetCounts = $nicRows |
  Where-Object { $_.status -eq "Up" -and $_.ipv4_subnet_first -and (Test-PrivateIPv4 $_.ipv4[0]) } |
  Group-Object -Property ipv4_subnet_first |
  Where-Object { $_.Count -gt 1 }
$audit.multihome_same_subnet = @($subnetCounts | ForEach-Object {
  [pscustomobject]@{
    subnet     = $_.Name
    nic_count  = $_.Count
    nic_names  = ($_.Group.name -join ",")
  }
})

# 3. LSO state per adapter --------------------------------------------------
$lsoRows = @()
foreach ($n in (Get-NetAdapter -ErrorAction SilentlyContinue)) {
  $lso = Get-NetAdapterLso -Name $n.Name -ErrorAction SilentlyContinue
  if ($lso) {
    $lsoRows += [pscustomobject]@{
      name             = $n.Name
      ifdesc           = $lso.InterfaceDescription
      ipv4_enabled     = [bool]$lso.IPv4Enabled
      ipv6_enabled     = [bool]$lso.IPv6Enabled
      v1_ipv4_enabled  = [bool]$lso.V1IPv4Enabled
    }
  }
}
$audit.lso = $lsoRows

# 4. Discovery service state ------------------------------------------------
$svcList = @("Bonjour Service","SSDPSRV","FDResPub","fdPHost","lltdsvc")
$svcRows = @()
foreach ($name in $svcList) {
  $s = Get-Service -Name $name -ErrorAction SilentlyContinue
  if ($s) {
    $svcRows += [pscustomobject]@{
      name       = $name
      installed  = $true
      status     = "$($s.Status)"
      start_type = "$((Get-Service -Name $name).StartType)"
    }
  } else {
    $svcRows += [pscustomobject]@{
      name      = $name
      installed = $false
    }
  }
}
$audit.services = $svcRows

# 5. LLMNR registry ---------------------------------------------------------
$llmnrPath = "HKLM:\SOFTWARE\Policies\Microsoft\Windows NT\DNSClient"
$llmnr = [ordered]@{ path = $llmnrPath; key_present = (Test-Path $llmnrPath); enable_multicast = $null }
if ($llmnr.key_present) {
  $prop = Get-ItemProperty -Path $llmnrPath -Name EnableMulticast -ErrorAction SilentlyContinue
  if ($prop) { $llmnr.enable_multicast = $prop.EnableMulticast }
}
$audit.llmnr = $llmnr

# 6. NetBIOS already collected per-NIC above (netbios_option field)

# 7. Tailscale / Wintun adapter ghost detection -----------------------------
# Wintun adapters present in Win32_NetworkAdapter but NetEnabled=False or NetConnectionStatus<>2
# are likely ghosts. Real Tailscale adapter is the one currently bound to the tailnet IP.
$wintunAll = Get-WmiObject Win32_NetworkAdapter -Filter "Manufacturer LIKE '%WireGuard%' OR Description LIKE '%Wintun%' OR Description LIKE '%Tailscale%'" -ErrorAction SilentlyContinue
$wintunRows = @()
foreach ($w in $wintunAll) {
  $wintunRows += [pscustomobject]@{
    name              = $w.Name
    description       = $w.Description
    manufacturer      = $w.Manufacturer
    net_enabled       = $w.NetEnabled
    connection_status = $w.NetConnectionStatus       # 2=Connected, 0=Disconnected, 7=Media disconnected, etc
    pnp_device_id     = $w.PNPDeviceID
    # TimeOfLastReset omitted -- WMI returns CIM-string, not DateTime; not used.
  }
}
$audit.wintun = [ordered]@{
  total_count   = $wintunRows.Count
  enabled_count = @($wintunRows | Where-Object { $_.net_enabled }).Count
  ghost_likely  = @($wintunRows | Where-Object { -not $_.net_enabled }).Count
  rows          = $wintunRows
}

# ---------------------------------------------------------------------------
# Console summary
# ---------------------------------------------------------------------------

Write-Host ""
Write-Host ("############################################################") -ForegroundColor White
Write-Host ("#  fleet-broadcast-audit on {0,-30}#" -f $audit.hostname) -ForegroundColor White
Write-Host ("############################################################") -ForegroundColor White

Write-Section "Identity"
Write-KV "hostname"        $audit.hostname
Write-KV "os"              $audit.os.caption
Write-KV "build"           $audit.os.build
Write-KV "uptime (min)"    $audit.os.uptime_minutes
if ($audit.tailscale.installed) {
  Write-KV "tailscale node"  ("{0} / {1}" -f $audit.tailscale.hostname, $audit.tailscale.tailnet_ip)
  Write-KV "tailscale state" $audit.tailscale.backend
} else {
  Write-KV "tailscale"       "NOT INSTALLED"
}

Write-Section "NIC inventory ({0} adapters)" -f $nicRows.Count
foreach ($n in $nicRows) {
  $color = if ($n.status -eq "Up") { "Green" } elseif ($n.status -eq "Disabled") { "Yellow" } else { "DarkGray" }
  $ipStr = if ($n.ipv4.Count -gt 0) { ($n.ipv4 -join ",") } else { "(no IPv4)" }
  Write-Host ("  {0,-12} {1,-9} ipv4={2,-15} mac={3,-17} {4}" -f $n.name, $n.status, $ipStr, $n.mac, $n.if_desc) -ForegroundColor $color
}
if ($audit.multihome_same_subnet.Count -gt 0) {
  Write-Host ""
  foreach ($mh in $audit.multihome_same_subnet) {
    Write-Host ("  ALERT: multi-home same subnet -- {0}.x has {1} adapters: {2}" -f $mh.subnet, $mh.nic_count, $mh.nic_names) -ForegroundColor Red
  }
} else {
  Write-Host ""
  Write-Host "  no multi-home-same-subnet detected" -ForegroundColor Green
}

Write-Section "LSO state"
foreach ($l in $lsoRows) {
  $color = if ($l.ipv4_enabled -or $l.ipv6_enabled -or $l.v1_ipv4_enabled) { "Yellow" } else { "Green" }
  Write-Host ("  {0,-12} IPv4={1,-5} IPv6={2,-5} V1IPv4={3,-5}  {4}" -f $l.name, $l.ipv4_enabled, $l.ipv6_enabled, $l.v1_ipv4_enabled, $l.ifdesc) -ForegroundColor $color
}

Write-Section "Discovery services"
foreach ($s in $svcRows) {
  if (-not $s.installed) {
    Write-Host ("  {0,-20} (not installed)" -f $s.name) -ForegroundColor DarkGray
  } else {
    $color = if ($s.start_type -eq "Disabled") { "Green" } elseif ($s.status -eq "Running") { "Yellow" } else { "Gray" }
    Write-Host ("  {0,-20} status={1,-12} start={2}" -f $s.name, $s.status, $s.start_type) -ForegroundColor $color
  }
}

Write-Section "LLMNR"
if (-not $audit.llmnr.key_present) {
  Write-Host "  policy key absent (LLMNR is at default = ENABLED)" -ForegroundColor Yellow
} elseif ($audit.llmnr.enable_multicast -eq 0) {
  Write-Host "  EnableMulticast=0 (LLMNR DISABLED via policy)" -ForegroundColor Green
} else {
  Write-Host ("  EnableMulticast={0}" -f $audit.llmnr.enable_multicast) -ForegroundColor Yellow
}

Write-Section "NetBIOS-over-TCPIP per NIC"
foreach ($n in $nicRows) {
  $opt = $n.netbios_option
  $label = switch ($opt) { 0 { "DHCP-default" } 1 { "ENABLED" } 2 { "DISABLED" } default { "(none)" } }
  $color = if ($opt -eq 2) { "Green" } elseif ($opt -in 0,1) { "Yellow" } else { "DarkGray" }
  Write-Host ("  {0,-12} option={1}  ({2})" -f $n.name, $opt, $label) -ForegroundColor $color
}

Write-Section "Tailscale / Wintun adapters"
Write-KV "total"          $audit.wintun.total_count
Write-KV "enabled (live)" $audit.wintun.enabled_count
Write-KV "ghost-likely"   $audit.wintun.ghost_likely
foreach ($w in $audit.wintun.rows) {
  $color = if ($w.net_enabled) { "Green" } else { "Yellow" }
  Write-Host ("  enabled={0,-5} status={1,-3} {2}" -f $w.net_enabled, $w.connection_status, $w.description) -ForegroundColor $color
}

# ---------------------------------------------------------------------------
# JSON dump
# ---------------------------------------------------------------------------

if (-not $ConsoleOnly) {
  if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir -Force | Out-Null }
  $jsonFile = Join-Path $OutDir ("fleet-audit-{0}-{1}.json" -f $audit.hostname, $timestamp)
  $audit | ConvertTo-Json -Depth 10 | Set-Content -Path $jsonFile -Encoding UTF8
  Write-Host ""
  Write-Host ("JSON dump: {0}" -f $jsonFile) -ForegroundColor Cyan
  Write-Host "Upload back to james for cross-machine comparison."
}
