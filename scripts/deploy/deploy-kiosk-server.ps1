# RaceControl Kiosk Deploy — run this on the server as Admin
$ErrorActionPreference = "Stop"
Write-Host "=== RaceControl Kiosk Deploy ===" -ForegroundColor Cyan

# 1. Kill existing kiosk
Write-Host "[1/6] Stopping existing kiosk..." -ForegroundColor Yellow
Get-Process node -ErrorAction SilentlyContinue | Where-Object { $_.CommandLine -like '*server.js*' } | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 2

# 2. Download zip from James
Write-Host "[2/6] Downloading kiosk from James (.27)..." -ForegroundColor Yellow
$zipPath = "C:\RacingPoint\kiosk-deploy.zip"
New-Item -ItemType Directory -Force -Path "C:\RacingPoint" | Out-Null
Invoke-WebRequest -Uri "http://192.168.31.27:9998/kiosk-deploy.zip" -OutFile $zipPath
Write-Host "  Downloaded: $((Get-Item $zipPath).Length / 1MB) MB"

# 3. Extract
Write-Host "[3/6] Extracting..." -ForegroundColor Yellow
if (Test-Path "C:\RacingPoint\kiosk") { Remove-Item -Recurse -Force "C:\RacingPoint\kiosk" }
Expand-Archive -Path $zipPath -DestinationPath "C:\RacingPoint\kiosk" -Force
Remove-Item $zipPath

# 4. HKLM Run key
Write-Host "[4/6] Registering auto-start..." -ForegroundColor Yellow
Set-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" -Name "RCKiosk" -Value "C:\RacingPoint\kiosk\start-kiosk.bat"

# 5. Desktop shortcut
Write-Host "[5/6] Creating desktop shortcut..." -ForegroundColor Yellow
$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut([Environment]::GetFolderPath('CommonDesktopDirectory') + '\RaceControl Kiosk.lnk')
$lnk.TargetPath = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
$lnk.Arguments = '--kiosk http://localhost:3300/kiosk --edge-kiosk-type=fullscreen'
$lnk.IconLocation = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe,0'
$lnk.Description = 'RaceControl Kiosk Terminal'
$lnk.Save()

# 6. Start kiosk
Write-Host "[6/6] Starting kiosk..." -ForegroundColor Yellow
$env:PORT = "3300"
$env:HOSTNAME = "0.0.0.0"
Start-Process -FilePath "C:\RacingPoint\kiosk\node.exe" -ArgumentList "C:\RacingPoint\kiosk\server.js" -WorkingDirectory "C:\RacingPoint\kiosk" -WindowStyle Hidden -RedirectStandardOutput "C:\RacingPoint\kiosk-log.txt" -RedirectStandardError "C:\RacingPoint\kiosk-err.txt"
Start-Sleep 3

# Verify
$resp = try { Invoke-WebRequest -Uri "http://localhost:3300/kiosk" -UseBasicParsing -TimeoutSec 5 } catch { $null }
if ($resp -and $resp.StatusCode -eq 200) {
    Write-Host "`n=== SUCCESS === Kiosk running at http://localhost:3300/kiosk" -ForegroundColor Green
} else {
    Write-Host "`n=== WARNING === Kiosk may not be responding yet. Check C:\RacingPoint\kiosk-err.txt" -ForegroundColor Red
}
Write-Host "Desktop shortcut: RaceControl Kiosk"
Write-Host "Auto-start: HKLM Run\RCKiosk"
