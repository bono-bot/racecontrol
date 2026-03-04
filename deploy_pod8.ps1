$podUrl = "http://192.168.31.91:8090"

# Step 1: Kill rc-agent
Write-Host "Step 1: Killing rc-agent..."
$body = @{ cmd = 'cd /d C:\RacingPoint & taskkill /IM rc-agent.exe /F' } | ConvertTo-Json
try {
    $r = Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body -TimeoutSec 10
    Write-Host "  $($r.stdout) $($r.stderr)"
} catch { Write-Host "  $($_.Exception.Message)" }
Start-Sleep -Seconds 3

# Step 2: Download new binary
Write-Host "Step 2: Downloading..."
$body2 = @{ cmd = 'cd /d C:\RacingPoint & curl -s -o rc-agent-new.exe http://192.168.31.35:9999/rc-agent.exe' } | ConvertTo-Json
try {
    $r2 = Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body2 -TimeoutSec 30
    Write-Host "  exit: $($r2.exit_code)"
} catch { Write-Host "  $($_.Exception.Message)" }
Start-Sleep -Seconds 1

# Step 3: Replace
Write-Host "Step 3: Replacing..."
$body3 = @{ cmd = 'cd /d C:\RacingPoint & del /f rc-agent-old.exe 2>nul & ren rc-agent.exe rc-agent-old.exe & ren rc-agent-new.exe rc-agent.exe & echo REPLACED' } | ConvertTo-Json
try {
    $r3 = Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body3 -TimeoutSec 10
    Write-Host "  $($r3.stdout)"
} catch { Write-Host "  $($_.Exception.Message)" }

# Step 4: Start (use schtasks to avoid blocking)
Write-Host "Step 4: Starting via schtasks..."
$body4 = @{ cmd = 'cd /d C:\RacingPoint & schtasks /create /tn "StartRcAgent" /tr "C:\RacingPoint\rc-agent.exe" /sc once /st 00:00 /f' } | ConvertTo-Json
try {
    $r4 = Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body4 -TimeoutSec 10
    Write-Host "  create: $($r4.stdout) $($r4.stderr)"
} catch { Write-Host "  $($_.Exception.Message)" }

$body5 = @{ cmd = 'schtasks /run /tn "StartRcAgent"' } | ConvertTo-Json
try {
    $r5 = Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body5 -TimeoutSec 10
    Write-Host "  run: $($r5.stdout) $($r5.stderr)"
} catch { Write-Host "  $($_.Exception.Message)" }

Write-Host "`nDone!"
