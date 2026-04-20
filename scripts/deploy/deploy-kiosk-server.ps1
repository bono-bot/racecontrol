# RaceControl Kiosk Deploy — run this on server .23 as Admin.
#
# Exit codes:
#   0 — kiosk :3300 bound and /kiosk returned HTTP 200
#   1 — port 3300 still held after 15s kill-wait
#   2 — zip download from James :9998 failed
#   3 — zip <1MB (staging server likely returned HTML 404)
#   4 — C:\RacingPoint\kiosk locked; Remove-Item failed after 5 retries
#   5 — start-kiosk.bat missing from zip
#   6 — :3300 bound but /kiosk returned no 200
#   7 — :3300 did not bind within 15s of starting
#   8 — schtasks /Create RCKiosk failed
#   9 — schtasks /Run RCKiosk failed
#
# History:
#   2026-04-20 — fix(deploy-kiosk): port-based kill + CIM CommandLine zombie-kill
#                + schtasks-based start (NOT Start-Process, which detaches when
#                SSH session closes and gets killed with parent).
#
#                Regression history: the SAME Get-Process CommandLine bug was
#                patched 2026-04-12 (cc5277e8 LOGBOOK row) via a file named
#                deploy-kiosk-server-FIXED.ps1 SCP'd to the server, but the fix
#                was NEVER committed back to git. When this script is re-SCP'd
#                from the repo, it overwrites the fixed version. This commit
#                lands the fix in git so it's permanent.
#
#                Why schtasks instead of Start-Process: the 2026-04-12 session
#                also hit "Start-Process -WindowStyle Hidden for node child
#                detaches on SSH session close" — the child gets killed when
#                the parent SSH session exits. schtasks /Run survives SSH
#                disconnect because the child is spawned under the Task
#                Scheduler service, not the SSH session's process tree.
#
#                Root causes diagnosed via DEBUG TOOLS FIRST evidence:
#                  Get-Process node | Select CommandLine  -> column is NULL
#                  Get-CimInstance Win32_Process -Filter "Name='node.exe'"
#                    -> returns real CommandLine strings

$ErrorActionPreference = "Stop"

# --- 0. Transcript so SSH non-interactive sessions capture ALL steps ---
# PS 5.1 Write-Host goes to the Information stream, which is buffered/dropped over
# non-interactive SSH. Start-Transcript mirrors EVERYTHING (Host, Output, Error,
# Warning, Verbose) to a file the caller can cat after the run. 2026-04-20 deploy
# had [3/6]-[6/6] Write-Host lines silent over SSH — this fixes that class.
$transcriptPath = "C:\RacingPoint\kiosk-deploy-transcript.txt"
try {
    Start-Transcript -Path $transcriptPath -Force | Out-Null
} catch {
    Write-Host "  WARN: Start-Transcript failed: $_" -ForegroundColor Yellow
}

Write-Host "=== RaceControl Kiosk Deploy ===" -ForegroundColor Cyan

# --- 1. Stop existing kiosk ---
Write-Host "[1/6] Stopping existing kiosk..." -ForegroundColor Yellow
$killed = New-Object System.Collections.Generic.HashSet[int]

# 1a. Kill whoever holds port 3300 (authoritative)
Get-NetTCPConnection -LocalPort 3300 -State Listen -ErrorAction SilentlyContinue | ForEach-Object {
    $victim = $_.OwningProcess
    try {
        Stop-Process -Id $victim -Force -ErrorAction Stop
        [void]$killed.Add($victim)
        Write-Host "  Killed PID $victim (port 3300 listener)" -ForegroundColor DarkGray
    } catch {
        Write-Host "  WARN: could not kill listener PID ${victim}: $_" -ForegroundColor Red
    }
}

# 1b. CIM-based kill of kiosk-node zombies (not bound to port, e.g. EADDRINUSE failers).
#     Get-Process CommandLine is NULL on default Windows PS; must use Get-CimInstance.
#     Match kiosk server.js but exclude web/admin/racecontrol to avoid killing siblings.
Get-CimInstance Win32_Process -Filter "Name='node.exe'" -ErrorAction SilentlyContinue |
    Where-Object {
        $cl = $_.CommandLine
        if (-not $cl) { return $false }
        # Positive match: kiosk path or bare server.js
        $isKioskish = ($cl -match 'RacingPoint\\kiosk') -or ($cl -match 'server\.js')
        # Exclusions: other services that also run server.js
        $isExcluded = ($cl -match 'web\\server\.js') -or ($cl -match 'admin\\server\.js') -or ($cl -match 'RacingPoint\\web') -or ($cl -match 'RacingPoint\\admin') -or ($cl -match 'racecontrol')
        return $isKioskish -and -not $isExcluded
    } |
    ForEach-Object {
        $victim = [int]$_.ProcessId
        if (-not $killed.Contains($victim)) {
            try {
                Stop-Process -Id $victim -Force -ErrorAction Stop
                [void]$killed.Add($victim)
                Write-Host "  Killed PID $victim (CIM: kiosk node.exe, CommandLine: $($_.CommandLine))" -ForegroundColor DarkGray
            } catch {
                Write-Host "  WARN: could not kill zombie PID ${victim}: $_" -ForegroundColor Red
            }
        }
    }

# 1c. Wait up to 15s for port 3300 to be free
$portFree = $false
for ($i = 0; $i -lt 15; $i++) {
    if (-not (Get-NetTCPConnection -LocalPort 3300 -State Listen -ErrorAction SilentlyContinue)) {
        $portFree = $true; break
    }
    Start-Sleep 1
}
if (-not $portFree) {
    Write-Host "  ABORT: port 3300 still held after 15s." -ForegroundColor Red
    exit 1
}
if ($killed.Count -eq 0) {
    Write-Host "  No existing kiosk process to stop." -ForegroundColor DarkGray
} else {
    Write-Host "  Stopped $($killed.Count) process(es)." -ForegroundColor DarkGray
}

# --- 2. Download zip from James staging ---
Write-Host "[2/6] Downloading kiosk from James (.27:9998)..." -ForegroundColor Yellow
$zipPath = "C:\RacingPoint\kiosk-deploy.zip"
New-Item -ItemType Directory -Force -Path "C:\RacingPoint" | Out-Null
try {
    Invoke-WebRequest -Uri "http://192.168.31.27:9998/kiosk-deploy.zip" -OutFile $zipPath -UseBasicParsing -TimeoutSec 60
} catch {
    Write-Host "  ABORT: download failed: $_" -ForegroundColor Red
    exit 2
}
$zipSize = (Get-Item $zipPath).Length
Write-Host "  Downloaded: $([math]::Round($zipSize/1MB, 2)) MB"
if ($zipSize -lt 1MB) {
    Write-Host "  ABORT: zip <1MB — staging server probably served HTML 404." -ForegroundColor Red
    exit 3
}

# --- 3. Extract (retry for lingering file handles) ---
Write-Host "[3/6] Extracting..." -ForegroundColor Yellow
if (Test-Path "C:\RacingPoint\kiosk") {
    $removed = $false
    for ($i = 0; $i -lt 5; $i++) {
        try {
            Remove-Item -Recurse -Force "C:\RacingPoint\kiosk" -ErrorAction Stop
            $removed = $true; break
        } catch {
            Start-Sleep 1
        }
    }
    if (-not $removed) {
        Write-Host "  ABORT: C:\RacingPoint\kiosk still locked after 5 retries." -ForegroundColor Red
        exit 4
    }
}
Expand-Archive -Path $zipPath -DestinationPath "C:\RacingPoint\kiosk" -Force
Remove-Item $zipPath

# --- 4. HKLM Run (idempotent) ---
Write-Host "[4/6] Registering auto-start..." -ForegroundColor Yellow
Set-ItemProperty -Path "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" -Name "RCKiosk" -Value "C:\RacingPoint\kiosk\start-kiosk.bat"

# --- 5. Desktop shortcut (idempotent) ---
Write-Host "[5/6] Creating desktop shortcut..." -ForegroundColor Yellow
$ws = New-Object -ComObject WScript.Shell
$lnk = $ws.CreateShortcut([Environment]::GetFolderPath('CommonDesktopDirectory') + '\RaceControl Kiosk.lnk')
$lnk.TargetPath = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
$lnk.Arguments = '--kiosk http://localhost:3300/kiosk --edge-kiosk-type=fullscreen'
$lnk.IconLocation = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe,0'
$lnk.Description = 'RaceControl Kiosk Terminal'
$lnk.Save()

# --- 6. Start kiosk via schtasks RCKiosk (survives SSH disconnect) ---
Write-Host "[6/6] Starting kiosk..." -ForegroundColor Yellow
$batPath = "C:\RacingPoint\kiosk\start-kiosk.bat"
if (-not (Test-Path $batPath)) {
    Write-Host "  ABORT: $batPath missing from zip — rebuild staging zip with start-kiosk.bat at standalone root." -ForegroundColor Red
    exit 5
}

# 6a. Ensure RCKiosk schtask exists (idempotent — register if missing).
#     schtasks runs the child under the Task Scheduler service, not the SSH
#     session's process tree, so the kiosk survives SSH disconnect.
#     Start-Process -WindowStyle Hidden detaches on SSH exit — avoid.
$schtaskName = "RCKiosk"
$null = schtasks.exe /Query /TN $schtaskName 2>$null
$schtaskExists = ($LASTEXITCODE -eq 0)

if (-not $schtaskExists) {
    Write-Host "  Registering schtask '$schtaskName'..." -ForegroundColor DarkGray
    # /SC ONSTART — runs at boot (pairs with HKLM Run as belt+suspenders)
    # /RU SYSTEM /RL HIGHEST — HTTP server doesn't need GUI/Session 1; Session 0 is fine
    schtasks.exe /Create /TN $schtaskName /TR "`"$batPath`"" /SC ONSTART /RU SYSTEM /RL HIGHEST /F | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "  ABORT: schtasks /Create $schtaskName failed (exit $LASTEXITCODE)" -ForegroundColor Red
        exit 8
    }
}

# 6b. Run the schtask (fires start-kiosk.bat -> node server.js under SYSTEM)
schtasks.exe /Run /TN $schtaskName | Out-Null
if ($LASTEXITCODE -ne 0) {
    Write-Host "  ABORT: schtasks /Run /TN $schtaskName failed (exit $LASTEXITCODE)" -ForegroundColor Red
    exit 9
}

# --- 7. Verify :3300 comes up ---
$bound = $false
for ($i = 0; $i -lt 15; $i++) {
    if (Get-NetTCPConnection -LocalPort 3300 -State Listen -ErrorAction SilentlyContinue) { $bound = $true; break }
    Start-Sleep 1
}
if (-not $bound) {
    Write-Host "`n=== FAIL === :3300 did not bind within 15s. Check C:\RacingPoint\kiosk-err.txt" -ForegroundColor Red
    exit 7
}

$resp = try { Invoke-WebRequest -Uri "http://localhost:3300/kiosk" -UseBasicParsing -TimeoutSec 5 } catch { $null }
if ($resp -and $resp.StatusCode -eq 200) {
    Write-Host "`n=== SUCCESS === Kiosk :3300 responded HTTP 200." -ForegroundColor Green
    Write-Host "Desktop shortcut: RaceControl Kiosk"
    Write-Host "Auto-start: HKLM Run\RCKiosk -> $batPath"
    exit 0
} else {
    Write-Host "`n=== WARNING === :3300 bound but /kiosk returned no HTTP 200. Check C:\RacingPoint\kiosk-err.txt" -ForegroundColor Red
    exit 6
}
