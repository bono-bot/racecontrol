param(
    [string]$ZipPath = "C:\RacingPoint\web-dashboard-new.zip",
    [string]$WebDir = "C:\RacingPoint\web",
    [int]$WebPort = 3200
)

$ErrorActionPreference = "Stop"

Write-Output "[deploy-web-targeted] Finding process holding port $WebPort..."
$portLine = netstat -ano | Select-String ":$WebPort\s+0\.0\.0\.0:0\s+LISTENING"
if (-not $portLine) {
    Write-Output "[deploy-web-targeted] WARN: no process listening on :$WebPort. Skipping kill step."
    $webPid = $null
} else {
    $webPid = ($portLine -split '\s+')[-1]
    Write-Output "[deploy-web-targeted] PID on :$WebPort = $webPid"
    Write-Output "[deploy-web-targeted] Killing PID $webPid only (admin :3201 + kiosk :3300 preserved)..."
    Stop-Process -Id $webPid -Force
    Start-Sleep -Seconds 3
}

Write-Output "[deploy-web-targeted] Removing old web dir..."
if (Test-Path $WebDir) { Remove-Item -Path $WebDir -Recurse -Force }

Write-Output "[deploy-web-targeted] Extracting zip..."
Expand-Archive -Path $ZipPath -DestinationPath $WebDir -Force

if (-not (Test-Path "$WebDir\server.js")) {
    Write-Output "[deploy-web-targeted] FAIL: server.js missing after extract"
    exit 1
}
if (-not (Test-Path "$WebDir\.next\static")) {
    Write-Output "[deploy-web-targeted] FAIL: .next/static missing after extract"
    exit 1
}

Write-Output "[deploy-web-targeted] Starting node on port $WebPort..."
$env:PORT = "$WebPort"
$env:HOSTNAME = "127.0.0.1"
Start-Process -FilePath "C:\Program Files\nodejs\node.exe" `
    -ArgumentList "server.js" `
    -WorkingDirectory $WebDir `
    -WindowStyle Hidden `
    -RedirectStandardOutput "C:\RacingPoint\web-stdout.log" `
    -RedirectStandardError "C:\RacingPoint\web-stderr.log"

Start-Sleep -Seconds 5

$check = try { Invoke-WebRequest -Uri "http://127.0.0.1:$WebPort/" -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop } catch { $null }
if ($check -and $check.StatusCode -eq 200) {
    Write-Output "[deploy-web-targeted] DEPLOY_SUCCESS — :$WebPort responding"
    exit 0
} else {
    Write-Output "[deploy-web-targeted] DEPLOY_WARNING — :$WebPort not responding yet, may still be starting"
    exit 0
}
