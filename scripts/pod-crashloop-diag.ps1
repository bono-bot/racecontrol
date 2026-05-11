#Requires -Version 5.1
# Read-only diagnostic for a pod that's crash-looping rc-agent.
# Run via: ssh pod<N> 'powershell -NoProfile -ExecutionPolicy Bypass -File C:/RacingPoint/pod-crashloop-diag.ps1'
# No admin required. No state mutation.

Write-Output "=== HOST ==="
Write-Output ("hostname: {0}" -f $env:COMPUTERNAME)
Write-Output ("user:     {0}" -f $env:USERNAME)
Write-Output ("uptime:   {0:N0} min" -f ((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).TotalMinutes)
Write-Output ""

Write-Output "=== SENTINELS (any present blocks restart) ==="
foreach ($name in @("MAINTENANCE_MODE","GRACEFUL_RELAUNCH","rcagent-restart-sentinel.txt","DEPLOY_IN_PROGRESS","OTA_DEPLOYING")) {
  $p = "C:\RacingPoint\$name"
  if (Test-Path $p) {
    $info = Get-Item $p
    Write-Output ("PRESENT: {0}  (size={1}b mtime={2})" -f $p, $info.Length, $info.LastWriteTime)
  } else {
    Write-Output ("absent : $p")
  }
}
Write-Output ""

Write-Output "=== rc-agent BINARY ==="
foreach ($f in @("rc-agent.exe","rc-sentry.exe","rc-agent.toml","rc-sentry.toml","start-rcagent.bat")) {
  $p = "C:\RacingPoint\$f"
  if (Test-Path $p) {
    $info = Get-Item $p
    Write-Output ("PRESENT: {0,-20}  size={1,12:N0}  mtime={2}" -f $f, $info.Length, $info.LastWriteTime)
  } else {
    Write-Output ("MISSING: $f")
  }
}
Write-Output ""

Write-Output "=== rc-agent process ==="
$rc = Get-Process -Name rc-agent -ErrorAction SilentlyContinue
if ($rc) {
  foreach ($p in $rc) { Write-Output ("PID={0} session={1} CPU={2:N1}s WS={3:N1}MB started={4}" -f $p.Id, $p.SessionId, [int]$p.CPU, ($p.WorkingSet64/1MB), $p.StartTime) }
} else { Write-Output "rc-agent not running" }
$rs = Get-Process -Name rc-sentry -ErrorAction SilentlyContinue
if ($rs) {
  foreach ($p in $rs) { Write-Output ("PID={0} session={1} CPU={2:N1}s started={3}  rc-sentry" -f $p.Id, $p.SessionId, [int]$p.CPU, $p.StartTime) }
} else { Write-Output "rc-sentry not running" }
Write-Output ""

Write-Output "=== /health locally on pod ==="
foreach ($port in @(8090, 8091)) {
  try {
    $r = Invoke-WebRequest -UseBasicParsing -Uri "http://localhost:$port/health" -TimeoutSec 4
    Write-Output ("port $port : HTTP $($r.StatusCode) bodylen=$($r.Content.Length)")
  } catch {
    Write-Output ("port $port : ERR $($_.Exception.Message)")
  }
}
Write-Output ""

Write-Output "=== connectivity to server.23:8080 ==="
$tn = Test-NetConnection -ComputerName 192.168.31.23 -Port 8080 -InformationLevel Quiet -WarningAction SilentlyContinue
Write-Output ("Test-NetConnection 192.168.31.23:8080 : $tn")
try {
  $r = Invoke-WebRequest -UseBasicParsing -Uri "http://192.168.31.23:8080/api/v1/health" -TimeoutSec 5
  Write-Output ("/api/v1/health : HTTP $($r.StatusCode) bodylen=$($r.Content.Length)")
} catch {
  Write-Output ("/api/v1/health : ERR $($_.Exception.Message)")
}
Write-Output ""

Write-Output "=== rc-agent.toml [core] section ==="
$toml = Get-Content "C:\RacingPoint\rc-agent.toml" -ErrorAction SilentlyContinue
if ($toml) {
  $coreStart = $false
  foreach ($line in $toml) {
    if ($line -match '^\[core\]') { $coreStart = $true; Write-Output $line; continue }
    if ($coreStart -and $line -match '^\[') { break }
    if ($coreStart) {
      if ($line -match '(?i)(secret|key|password|token|service_key)') {
        Write-Output ($line -replace '=.*$','=***REDACTED***')
      } else { Write-Output $line }
    }
  }
} else { Write-Output "rc-agent.toml not found" }
Write-Output ""

Write-Output "=== latest rc-agent jsonl log (last 8 lines) ==="
$logs = Get-ChildItem "C:\RacingPoint\rc-agent*.jsonl","C:\RacingPoint\logs\rc-agent*.jsonl" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($logs) {
  Write-Output $logs.FullName
  Get-Content $logs.FullName -Tail 8
} else { Write-Output "no rc-agent jsonl found" }
Write-Output ""

Write-Output "=== IP config (Up adapters with IPv4) ==="
$ips = Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.InterfaceAlias -notmatch '^Loopback' -and $_.AddressState -eq 'Preferred' }
foreach ($x in $ips) {
  Write-Output ("  {0,-30} {1}/{2}" -f $x.InterfaceAlias, $x.IPAddress, $x.PrefixLength)
}
Write-Output ""

Write-Output "=== default routes ==="
$rt = Get-NetRoute -DestinationPrefix 0.0.0.0/0 -ErrorAction SilentlyContinue
foreach ($r in $rt) {
  Write-Output ("  via {0,-15} on {1,-30} metric={2}" -f $r.NextHop, $r.InterfaceAlias, $r.RouteMetric)
}
Write-Output ""

Write-Output "=== reach probes ==="
$tgts = @{ "LAN gateway 192.168.31.1" = "192.168.31.1"; "server LAN 192.168.31.23" = "192.168.31.23"; "server Tailscale 100.125.108.37" = "100.125.108.37" }
foreach ($k in $tgts.Keys) {
  $ok = Test-Connection -ComputerName $tgts[$k] -Count 2 -Quiet -ErrorAction SilentlyContinue
  Write-Output ("  ping $k = $ok")
}
Write-Output ""

Write-Output "=== latest rc-sentry jsonl log (last 6 lines) ==="
$slogs = Get-ChildItem "C:\RacingPoint\rc-sentry*.jsonl","C:\RacingPoint\logs\rc-sentry*.jsonl" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($slogs) {
  Write-Output $slogs.FullName
  Get-Content $slogs.FullName -Tail 6
} else { Write-Output "no rc-sentry jsonl found" }
