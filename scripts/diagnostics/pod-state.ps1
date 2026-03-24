Write-Host "=== rc-agent process ==="
$agent = Get-Process rc-agent -ErrorAction SilentlyContinue
if ($agent) { Write-Host "RUNNING PID=$($agent.Id) Session=$($agent.SessionId)" } else { Write-Host "NOT RUNNING" }

Write-Host "=== rc-agent binaries ==="
Get-ChildItem C:\RacingPoint\rc-agent*.exe -ErrorAction SilentlyContinue | ForEach-Object { Write-Host "$($_.Name) $($_.Length) bytes" }

Write-Host "=== rc-agent health ==="
try { $h = Invoke-RestMethod http://localhost:8090/health -TimeoutSec 3; Write-Host "UP build=$($h.build_id) uptime=$($h.uptime_secs)s" } catch { Write-Host "DOWN" }
