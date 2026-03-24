# Deploy RaceControl binary on server
$src = "http://192.168.31.27:9998/racecontrol.exe"
$dest = "C:\RacingPoint\racecontrol.exe"
$backup = "C:\RacingPoint\racecontrol-backup.exe"
$temp = "C:\RacingPoint\racecontrol-new.exe"

Write-Output "Downloading new binary..."
curl.exe -s -o $temp $src
$size = (Get-Item $temp).Length
Write-Output "Downloaded: $size bytes"
if ($size -lt 1000000) { Write-Output "ERROR: Binary too small"; exit 1 }

Write-Output "Backing up old binary..."
if (Test-Path $dest) { Copy-Item $dest $backup -Force }

Write-Output "Stopping racecontrol..."
Stop-Process -Name racecontrol -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3

Write-Output "Replacing binary..."
Move-Item $temp $dest -Force

Write-Output "Starting racecontrol..."
Start-Process -FilePath $dest -WorkingDirectory "C:\RacingPoint" -NoNewWindow
Start-Sleep -Seconds 5

Write-Output "DEPLOY_COMPLETE"
