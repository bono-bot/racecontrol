#Requires -Version 5.1
<#
.SYNOPSIS
  Comprehensive read-only audit of pod state to identify drift between pods.
  Outputs JSON to stdout AND to C:\RacingPoint\uniformity-audit-<host>-<ts>.json.
  Designed to be SCP'd then run via ssh — no admin required.

.DESCRIPTION
  Captures everything that should be identical across pods (except pod_number / IP):
    - Binary inventory (rc-agent.exe, rc-sentry.exe, configs, bats) — size + sha256 + mtime
    - Process state (rc-agent, rc-sentry — PIDs, sessions, uptime)
    - Services (kaizen-disabled set + canonical-running set)
    - Scheduled tasks under \RacingPoint\ + Windows-known autostart tasks
    - HKLM/HKCU Run keys
    - Network: IP / gateway / DNS / MAC / Wi-Fi state
    - Wintun adapters (current + ghost count)
    - Sentinels (MAINTENANCE_MODE / GRACEFUL_RELAUNCH / etc.)
    - Recent crashes (rc-agent termination.log if present)
    - Disk free on C:
    - Windows uptime
    - Logged-in user in Session 1
    - AC install detection (Content Manager / Steam / standalone)
    - Game-content directory listing (catalog drift — expected to vary)

  All output is read-only. No state mutation.
#>

$ErrorActionPreference = "Continue"
$hostName = $env:COMPUTERNAME
$ts = Get-Date -Format "yyyyMMdd-HHmmss"
$outFile = "C:\RacingPoint\uniformity-audit-$hostName-$ts.json"

function Get-FileFingerprint {
  param([string]$Path)
  if (-not (Test-Path $Path)) { return $null }
  $info = Get-Item $Path
  $sha = $null
  try { $sha = (Get-FileHash -Path $Path -Algorithm SHA256 -ErrorAction Stop).Hash.Substring(0,16) } catch {}
  return [pscustomobject]@{
    path   = $Path
    size   = $info.Length
    mtime  = $info.LastWriteTime.ToString("o")
    sha256_short = $sha
  }
}

function Get-AdapterSummary {
  $adapters = Get-NetAdapter -ErrorAction SilentlyContinue
  $result = @()
  foreach ($a in $adapters) {
    $result += [pscustomobject]@{
      name = $a.Name
      ifindex = $a.InterfaceIndex
      status = $a.Status.ToString()
      admin_status = $a.AdminStatus.ToString()
      mac = $a.MacAddress
      link_speed = $a.LinkSpeed
      driver_desc = $a.InterfaceDescription
    }
  }
  return $result
}

function Get-WintunCount {
  $all = @(Get-PnpDevice -Class Net -ErrorAction SilentlyContinue | Where-Object { $_.FriendlyName -like "*Wintun*" -or $_.HardwareID -like "*Wintun*" })
  $present = @($all | Where-Object { $_.Status -eq "OK" })
  $ghost = @($all | Where-Object { $_.Status -ne "OK" })
  return [pscustomobject]@{
    total = $all.Count
    present = $present.Count
    ghost = $ghost.Count
  }
}

function Get-ServiceState {
  param([string[]]$Names)
  $result = @()
  foreach ($n in $Names) {
    $svc = Get-Service -Name $n -ErrorAction SilentlyContinue
    if ($svc) {
      $startup = (Get-CimInstance -ClassName Win32_Service -Filter "Name='$n'" -ErrorAction SilentlyContinue).StartMode
      $result += [pscustomobject]@{ name = $n; status = $svc.Status.ToString(); startup = "$startup" }
    } else {
      $result += [pscustomobject]@{ name = $n; status = "NOT_FOUND"; startup = $null }
    }
  }
  return $result
}

function Get-RegistryRunKeys {
  $result = @()
  foreach ($hive in @("HKLM","HKCU")) {
    foreach ($key in @("Software\Microsoft\Windows\CurrentVersion\Run","Software\Microsoft\Windows\CurrentVersion\RunOnce")) {
      $path = "$hive`:\$key"
      try {
        $vals = Get-ItemProperty -Path $path -ErrorAction Stop
        $vals.PSObject.Properties | Where-Object { $_.Name -notmatch '^PS' } | ForEach-Object {
          $result += [pscustomobject]@{ hive = $hive; key = $key; name = $_.Name; value = "$($_.Value)" }
        }
      } catch {}
    }
  }
  return $result
}

function Get-RacingPointTasks {
  $tasks = Get-ScheduledTask -ErrorAction SilentlyContinue
  $result = @()
  foreach ($t in $tasks) {
    if ($t.TaskName -match "RC|Racing|RP|StartRC|RCAgent|RCSentry|RCWatchdog|AutoDisable") {
      $action = $t.Actions[0]
      $result += [pscustomobject]@{
        name = $t.TaskName
        path = $t.TaskPath
        state = "$($t.State)"
        principal_user = $t.Principal.UserId
        run_level = "$($t.Principal.RunLevel)"
        execute = $action.Execute
        arguments = $action.Arguments
      }
    }
  }
  return $result
}

function Get-RcSentinels {
  $sentinels = @("MAINTENANCE_MODE","GRACEFUL_RELAUNCH","rcagent-restart-sentinel.txt","DEPLOY_IN_PROGRESS","OTA_DEPLOYING")
  $result = @()
  foreach ($s in $sentinels) {
    $p = "C:\RacingPoint\$s"
    if (Test-Path $p) {
      $i = Get-Item $p
      $result += [pscustomobject]@{ name = $s; present = $true; size = $i.Length; mtime = $i.LastWriteTime.ToString("o") }
    } else {
      $result += [pscustomobject]@{ name = $s; present = $false }
    }
  }
  return $result
}

# --- Main collection -------------------------------------------------------

$audit = [ordered]@{}
$audit.audit_version = "1.0"
$audit.audit_ts_iso = (Get-Date).ToString("o")
$audit.host = $hostName

# OS / boot
$os = Get-CimInstance Win32_OperatingSystem -ErrorAction SilentlyContinue
$audit.os = [pscustomobject]@{
  caption = $os.Caption
  build_number = $os.BuildNumber
  install_date = if ($os.InstallDate) { $os.InstallDate.ToString("o") } else { $null }
  last_boot = if ($os.LastBootUpTime) { $os.LastBootUpTime.ToString("o") } else { $null }
  uptime_min = if ($os.LastBootUpTime) { [int]((Get-Date) - $os.LastBootUpTime).TotalMinutes } else { $null }
}

# Logged-in user (Session 1)
try {
  $sess = (& quser.exe 2>$null) -join "`n"
  $audit.logged_in_users = $sess
} catch { $audit.logged_in_users = "(quser unavailable)" }

# Disk free
$c = Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='C:'" -ErrorAction SilentlyContinue
$audit.disk_c = [pscustomobject]@{
  size_gb = [math]::Round($c.Size / 1GB, 1)
  free_gb = [math]::Round($c.FreeSpace / 1GB, 1)
  pct_free = if ($c.Size) { [math]::Round(100 * $c.FreeSpace / $c.Size, 1) } else { $null }
}

# Binary fingerprints
$audit.binaries = [ordered]@{}
foreach ($f in @("rc-agent.exe","rc-sentry.exe","rc-agent.toml","rc-sentry.toml","start-rcagent.bat","start-rcsentry.bat")) {
  $audit.binaries[$f] = Get-FileFingerprint "C:\RacingPoint\$f"
}

# Staged binaries (drift indicators — should be empty under v26.0+)
$staged = Get-ChildItem "C:\RacingPoint\rc-agent-*.exe","C:\RacingPoint\rc-sentry-*.exe","C:\RacingPoint\rc-agent-prev.exe","C:\RacingPoint\rc-sentry-prev.exe" -ErrorAction SilentlyContinue
$audit.staged_binaries = @($staged | ForEach-Object { [pscustomobject]@{ name = $_.Name; size = $_.Length; mtime = $_.LastWriteTime.ToString("o") } })

# rc-agent.toml [core] section (server URL etc.)
$tomlContent = Get-Content "C:\RacingPoint\rc-agent.toml" -ErrorAction SilentlyContinue
$audit.rc_agent_toml_core = @()
if ($tomlContent) {
  $inCore = $false
  foreach ($line in $tomlContent) {
    if ($line -match '^\[core\]') { $inCore = $true; continue }
    if ($inCore -and $line -match '^\[') { break }
    if ($inCore -and $line.Trim()) {
      $redacted = $line -replace '(?i)(secret|key|password|token|service_key)\s*=.*$', '$1=***REDACTED***'
      $audit.rc_agent_toml_core += $redacted
    }
  }
}

# Processes
$rcAgentProcs = Get-Process -Name rc-agent -ErrorAction SilentlyContinue
$rcSentryProcs = Get-Process -Name rc-sentry -ErrorAction SilentlyContinue
$audit.processes = [pscustomobject]@{
  rc_agent = @($rcAgentProcs | ForEach-Object { [pscustomobject]@{ pid = $_.Id; session = $_.SessionId; cpu_s = [int]$_.CPU; ws_mb = [math]::Round($_.WorkingSet64 / 1MB, 1); start_time = $_.StartTime.ToString("o") } })
  rc_sentry = @($rcSentryProcs | ForEach-Object { [pscustomobject]@{ pid = $_.Id; session = $_.SessionId; cpu_s = [int]$_.CPU; ws_mb = [math]::Round($_.WorkingSet64 / 1MB, 1); start_time = $_.StartTime.ToString("o") } })
}

# Local /health from rc-agent + rc-sentry
$audit.local_health = [ordered]@{}
foreach ($pair in @(@{name="rc_agent_8090"; port=8090}, @{name="rc_sentry_8091"; port=8091})) {
  try {
    $r = Invoke-WebRequest -UseBasicParsing -Uri "http://localhost:$($pair.port)/health" -TimeoutSec 4 -ErrorAction Stop
    $audit.local_health[$pair.name] = [pscustomobject]@{ status = $r.StatusCode; body_first_300 = $r.Content.Substring(0, [Math]::Min(300, $r.Content.Length)) }
  } catch {
    $audit.local_health[$pair.name] = [pscustomobject]@{ status = "ERR"; body_first_300 = "$($_.Exception.Message)" }
  }
}

# Services — kaizen-disabled set + canonical
$kaizenDisabled = @("Bonjour Service","SSDPSRV","FDResPub","fdPHost","lltdsvc")
$canonicalRunning = @("RCWatchdog","Tailscale","Themes","Audiosrv","Schedule")
$audit.services_kaizen = Get-ServiceState $kaizenDisabled
$audit.services_canonical = Get-ServiceState $canonicalRunning

# Scheduled tasks (RC-related)
$audit.scheduled_tasks = Get-RacingPointTasks

# Registry Run keys
$audit.run_keys = Get-RegistryRunKeys

# Network
$audit.network = [ordered]@{
  adapters = Get-AdapterSummary
  ipv4 = @(Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.AddressState -eq 'Preferred' -and $_.InterfaceAlias -notmatch 'Loopback' } | ForEach-Object { [pscustomobject]@{ alias = $_.InterfaceAlias; ip = $_.IPAddress; prefix = $_.PrefixLength } })
  default_routes = @(Get-NetRoute -DestinationPrefix 0.0.0.0/0 -ErrorAction SilentlyContinue | ForEach-Object { [pscustomobject]@{ alias = $_.InterfaceAlias; nexthop = $_.NextHop; metric = $_.RouteMetric } })
  dns = @(Get-DnsClientServerAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.ServerAddresses } | ForEach-Object { [pscustomobject]@{ alias = $_.InterfaceAlias; servers = ($_.ServerAddresses -join ',') } })
}

# Wintun ghost count
$audit.wintun = Get-WintunCount

# Sentinels
$audit.sentinels = Get-RcSentinels

# Recent rc-agent crashes (termination.log)
$termLog = "C:\RacingPoint\termination.log"
if (Test-Path $termLog) {
  $audit.termination_log_tail = (Get-Content $termLog -Tail 5 -ErrorAction SilentlyContinue) -join "`n"
} else {
  $audit.termination_log_tail = "(no termination.log)"
}

# rc-agent jsonl most recent + line count
$rcLog = Get-ChildItem "C:\RacingPoint\rc-agent-*.jsonl","C:\RacingPoint\logs\rc-agent-*.jsonl" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($rcLog) {
  $audit.rc_agent_log = [pscustomobject]@{
    file = $rcLog.FullName
    size = $rcLog.Length
    mtime = $rcLog.LastWriteTime.ToString("o")
    age_min = [int]((Get-Date) - $rcLog.LastWriteTime).TotalMinutes
  }
} else {
  $audit.rc_agent_log = $null
}

# AC install detection (read-only)
$audit.ac_install = [ordered]@{}
$acPaths = @{
  "ContentManager" = "C:\Users\Public\Documents\AC Content Manager"
  "Steam_AC" = "C:\Program Files (x86)\Steam\steamapps\common\assettocorsa"
  "Standalone" = "C:\Program Files\Assetto Corsa"
}
foreach ($k in $acPaths.Keys) {
  $audit.ac_install[$k] = (Test-Path $acPaths[$k])
}

# Output
$json = $audit | ConvertTo-Json -Depth 8 -Compress:$false
$json | Out-File -FilePath $outFile -Encoding UTF8
Write-Output $json
