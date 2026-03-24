$steamLog = "C:\Program Files (x86)\Steam\logs\console_log.txt"
Write-Host "=== Last 5 Steam log lines ==="
Get-Content $steamLog -Tail 5

Write-Host "`n=== Steam process ==="
$s = Get-Process steam -ErrorAction SilentlyContinue
if ($s) { Write-Host "RUNNING PID=$($s.Id)" } else { Write-Host "NOT RUNNING" }
