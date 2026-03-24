Write-Host "=== rc-agent session ==="
$agent = Get-Process rc-agent -ErrorAction SilentlyContinue
if ($agent) {
    Write-Host "PID: $($agent.Id), SessionId: $($agent.SessionId)"
} else {
    Write-Host "rc-agent not running"
}

Write-Host "`n=== sentry session ==="
$sentry = Get-Process rc-sentry -ErrorAction SilentlyContinue
if ($sentry) {
    Write-Host "PID: $($sentry.Id), SessionId: $($sentry.SessionId)"
}

Write-Host "`n=== All rc-* processes ==="
Get-Process | Where-Object { $_.Name -like "rc-*" } | Select-Object Name, Id, SessionId | Format-Table
