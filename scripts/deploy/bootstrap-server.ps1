# RaceControl Server Bootstrap — run ONCE on the server as Admin
# Sets up permanent remote access + kiosk
$ErrorActionPreference = "Stop"
$JAMES_HTTP = "http://192.168.31.27:9998"

Write-Host "`n=== RaceControl Server Bootstrap ===" -ForegroundColor Cyan

# PHASE 1: Upgrade racecontrol binary (enables :8090 server_ops)
Write-Host "`n[PHASE 1] Upgrading racecontrol binary..." -ForegroundColor Cyan

Write-Host "  Stopping racecontrol..."
Get-Process racecontrol -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep 3

Write-Host "  Downloading new racecontrol.exe..."
Invoke-WebRequest -Uri "$JAMES_HTTP/racecontrol.exe" -OutFile "C:\RacingPoint\racecontrol.exe.new"
$size = (Get-Item "C:\RacingPoint\racecontrol.exe.new").Length
Write-Host "  Downloaded: $([math]::Round($size / 1MB, 1)) MB"

if ($size -lt 10000000) {
    Write-Host "  ERROR: Binary too small ($size bytes). Aborting." -ForegroundColor Red
    Remove-Item "C:\RacingPoint\racecontrol.exe.new"
    exit 1
}

Write-Host "  Replacing binary..."
if (Test-Path "C:\RacingPoint\racecontrol.exe") {
    Remove-Item "C:\RacingPoint\racecontrol.exe" -Force
}
Rename-Item "C:\RacingPoint\racecontrol.exe.new" "C:\RacingPoint\racecontrol.exe"

Write-Host "  Starting racecontrol..."
Start-Process -FilePath "C:\RacingPoint\start-racecontrol.bat" -WorkingDirectory "C:\RacingPoint"
Start-Sleep 5

# Verify :8090 is up
$ping = try { (Invoke-WebRequest -Uri "http://localhost:8090/ping" -UseBasicParsing -TimeoutSec 5).Content } catch { "" }
if ($ping -eq "pong") {
    Write-Host "  SUCCESS: server_ops on :8090 responding" -ForegroundColor Green
} else {
    Write-Host "  WARNING: :8090 not responding yet (may need a few more seconds)" -ForegroundColor Yellow
}

# PHASE 2: Deploy Kiosk
Write-Host "`n[PHASE 2] Deploying kiosk..." -ForegroundColor Cyan

Get-Process node -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "*kiosk*" } | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep 2

Write-Host "  Downloading kiosk..."
Invoke-WebRequest -Uri "$JAMES_HTTP/kiosk-deploy.zip" -OutFile "C:\RacingPoint\kiosk-deploy.zip"

if (Test-Path "C:\RacingPoint\kiosk") { Remove-Item -Recurse -Force "C:\RacingPoint\kiosk" }
Expand-Archive -Path "C:\RacingPoint\kiosk-deploy.zip" -DestinationPath "C:\RacingPoint\kiosk" -Force
Remove-Item "C:\RacingPoint\kiosk-deploy.zip"

Set-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" -Name "RCKiosk" -Value "C:\RacingPoint\kiosk\start-kiosk.bat"

$env:PORT = "3300"; $env:HOSTNAME = "0.0.0.0"
Start-Process -FilePath "C:\RacingPoint\kiosk\node.exe" -ArgumentList "C:\RacingPoint\kiosk\server.js" -WorkingDirectory "C:\RacingPoint\kiosk" -WindowStyle Hidden
Start-Sleep 3
Write-Host "  Kiosk deployed and started on :3300" -ForegroundColor Green

# PHASE 3: Desktop Shortcut
Write-Host "`n[PHASE 3] Creating desktop shortcut..." -ForegroundColor Cyan

$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut([Environment]::GetFolderPath('CommonDesktopDirectory') + '\RaceControl Kiosk.lnk')
$lnk.TargetPath = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
$lnk.Arguments = '--kiosk http://localhost:3300/kiosk --edge-kiosk-type=fullscreen'
$lnk.IconLocation = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe,0'
$lnk.Description = 'RaceControl Kiosk Terminal'
$lnk.Save()
Write-Host "  Desktop shortcut created" -ForegroundColor Green

# PHASE 4: Enable OpenSSH Server
Write-Host "`n[PHASE 4] Setting up OpenSSH..." -ForegroundColor Cyan

$sshCap = Get-WindowsCapability -Online | Where-Object Name -like 'OpenSSH.Server*'
if ($sshCap.State -ne 'Installed') {
    Write-Host "  Installing OpenSSH Server..."
    try {
        Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
        Write-Host "  OpenSSH installed" -ForegroundColor Green
    } catch {
        Write-Host "  OpenSSH install failed: $_" -ForegroundColor Yellow
        Write-Host "  Will use server_ops :8090 as primary remote channel" -ForegroundColor Yellow
    }
} else {
    Write-Host "  OpenSSH already installed"
}

if (Get-Service sshd -ErrorAction SilentlyContinue) {
    Set-Service -Name sshd -StartupType Automatic
    Start-Service sshd -ErrorAction SilentlyContinue
    Write-Host "  sshd started and set to auto-start" -ForegroundColor Green

    $authKeysDir = "C:\ProgramData\ssh"
    if (!(Test-Path $authKeysDir)) { New-Item -ItemType Directory -Path $authKeysDir -Force | Out-Null }
    $authKeysFile = "$authKeysDir\administrators_authorized_keys"
    try {
        Invoke-WebRequest -Uri "$JAMES_HTTP/authorized_keys" -OutFile $authKeysFile -ErrorAction Stop
        icacls $authKeysFile /inheritance:r /grant "SYSTEM:(F)" /grant "Administrators:(F)" | Out-Null
        Write-Host "  SSH key deployed" -ForegroundColor Green
    } catch {
        Write-Host "  Could not fetch SSH key" -ForegroundColor Yellow
    }

    New-NetFirewallRule -Name "OpenSSH-Server" -DisplayName "OpenSSH Server" -Protocol TCP -LocalPort 22 -Action Allow -Direction Inbound -ErrorAction SilentlyContinue | Out-Null
} else {
    Write-Host "  sshd not available — server_ops :8090 is the primary channel" -ForegroundColor Yellow
}

# PHASE 5: Tailscale SSH
Write-Host "`n[PHASE 5] Enabling Tailscale SSH..." -ForegroundColor Cyan
try {
    & tailscale set --ssh 2>&1 | Out-Null
    Write-Host "  Tailscale SSH enabled" -ForegroundColor Green
} catch {
    Write-Host "  Tailscale SSH failed: $_" -ForegroundColor Yellow
}

# PHASE 6: Firewall
Write-Host "`n[PHASE 6] Firewall rules..." -ForegroundColor Cyan
New-NetFirewallRule -Name "RC-ServerOps" -DisplayName "RaceControl Server Ops (8090)" -Protocol TCP -LocalPort 8090 -Action Allow -Direction Inbound -ErrorAction SilentlyContinue | Out-Null
New-NetFirewallRule -Name "RC-Kiosk" -DisplayName "RaceControl Kiosk (3300)" -Protocol TCP -LocalPort 3300 -Action Allow -Direction Inbound -ErrorAction SilentlyContinue | Out-Null
Write-Host "  Firewall rules created" -ForegroundColor Green

# SUMMARY
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  BOOTSTRAP COMPLETE" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  RaceControl:  http://localhost:8080"
Write-Host "  Server Ops:   http://localhost:8090/ping"
Write-Host "  Kiosk:        http://localhost:3300/kiosk"
Write-Host "  Desktop:      RaceControl Kiosk shortcut"
Write-Host ""
Write-Host "  Test from James:" -ForegroundColor Yellow
Write-Host "    curl http://192.168.31.23:8090/ping"
Write-Host ""
pause
