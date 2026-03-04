$podUrl = "http://192.168.31.91:8090"

# Start rc-agent with explicit working directory using start /d
$body = @{ cmd = 'cmd /c start /d "C:\RacingPoint" /min "" "C:\RacingPoint\rc-agent.exe"' } | ConvertTo-Json
try {
    $r = Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body -TimeoutSec 8
    Write-Host "Result: $($r.stdout) $($r.stderr) exit=$($r.exit_code)"
} catch {
    Write-Host "Timed out (expected for background process)"
}

Start-Sleep -Seconds 8
try {
    $pod = (Invoke-RestMethod -Uri 'http://localhost:8080/api/v1/pods/pod_8').pod
    Write-Host "Pod 8 last_seen: $($pod.last_seen)"
    Write-Host "Pod 8 status: $($pod.status)"
} catch { Write-Host "Could not check" }
