Write-Output "Stopping node..."
Get-Process -Name node -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 3

$old = "C:\RacingPoint\web"
$zip = "C:\RacingPoint\web-dashboard-new.zip"

Write-Output "Removing old web dir..."
if (Test-Path $old) { Remove-Item -Path $old -Recurse -Force }

Write-Output "Extracting..."
Expand-Archive -Path $zip -DestinationPath $old -Force

Write-Output "Verifying..."
if (Test-Path "$old\server.js") { Write-Output "server.js OK" } else { Write-Output "server.js MISSING"; exit 1 }
if (Test-Path "$old\.next\static") { Write-Output ".next/static OK" } else { Write-Output ".next/static MISSING"; exit 1 }

Write-Output "Starting node on port 3200..."
$env:PORT = "3200"
$env:HOSTNAME = "127.0.0.1"
Start-Process -FilePath "C:\Program Files\nodejs\node.exe" -ArgumentList "server.js" -WorkingDirectory $old -NoNewWindow -RedirectStandardOutput "C:\RacingPoint\web-stdout.log" -RedirectStandardError "C:\RacingPoint\web-stderr.log"

Start-Sleep -Seconds 3
$check = Invoke-WebRequest -Uri "http://127.0.0.1:3200/" -TimeoutSec 5 -ErrorAction SilentlyContinue
if ($check.StatusCode -eq 200) {
    Write-Output "DEPLOY_SUCCESS - Dashboard responding on :3200"
} else {
    Write-Output "DEPLOY_WARNING - Dashboard may not be ready yet"
}
