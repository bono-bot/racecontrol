param([string]$PodIP = "192.168.31.91")
$podUrl = "http://${PodIP}:8090"

# Kill existing processes first
Write-Host "Stopping existing processes..."
$body = @{ cmd = 'taskkill /IM rc-agent.exe /F 2>nul & taskkill /IM pod-agent.exe /F 2>nul & echo ok' } | ConvertTo-Json
try { Invoke-RestMethod -Uri "$podUrl/exec" -Method Post -ContentType "application/json" -Body $body -TimeoutSec 10 | Out-Null } catch {}

# Wait - pod-agent will die, that's expected. We'll write files first, then instructions.
Start-Sleep 2

# Read watchdog scripts
$rcWatchdog = Get-Content "C:\Users\bono\racingpoint\racecontrol\pod-scripts\watchdog-rc-agent.cmd" -Raw
$paWatchdog = Get-Content "C:\Users\bono\racingpoint\racecontrol\pod-scripts\watchdog-pod-agent.cmd" -Raw

Write-Host ""
Write-Host "Pod-agent was killed. On Pod 8, run these commands:"
Write-Host ""
Write-Host '  cd /d C:\RacingPoint'
Write-Host '  start pod-agent.exe'
Write-Host ""
Write-Host "Then I'll deploy the watchdog scripts remotely."
