$podUrl = "http://192.168.31.91:8090"

# Step 1: Kill old rc-agent if running
Write-Host "Killing rc-agent..."
$body = @{ cmd = 'cd /d C:\RacingPoint & taskkill /IM rc-agent.exe /F 2>nul & echo ok' } | ConvertTo-Json
try {
    $r = Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body -TimeoutSec 10
    Write-Host "  $($r.stdout.Trim())"
} catch { Write-Host "  timeout" }
Start-Sleep 2

# Step 2: Delete old binaries and download fresh
Write-Host "Downloading new rc-agent..."
$body2 = @{ cmd = 'cd /d C:\RacingPoint & del /f rc-agent.exe 2>nul & del /f rc-agent-old.exe 2>nul & curl -s -o rc-agent.exe http://192.168.31.35:9999/rc-agent.exe & echo downloaded' } | ConvertTo-Json
try {
    $r2 = Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body2 -TimeoutSec 30
    Write-Host "  $($r2.stdout.Trim())"
} catch { Write-Host "  timeout" }
Start-Sleep 1

# Step 3: Verify size
Write-Host "Verifying..."
try {
    $files = Invoke-RestMethod -Uri "$podUrl/files?path=C:/RacingPoint" -TimeoutSec 5
    $agent = $files | Where-Object { $_.name -eq 'rc-agent.exe' }
    Write-Host "  rc-agent.exe size: $($agent.size) bytes"
} catch { Write-Host "  could not verify" }
