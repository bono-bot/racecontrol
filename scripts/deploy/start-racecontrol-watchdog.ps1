# start-racecontrol-watchdog.ps1
# Singleton watchdog for racecontrol.exe on the server (.23)
# Monitors port 8080, auto-restarts on crash with escalating backoff.
# Launched by start-racecontrol.bat

# Singleton mutex
$mutexName = "Global\RaceControlWatchdog"
$mutex = New-Object System.Threading.Mutex($false, $mutexName)
if (-not $mutex.WaitOne(0)) {
    Write-Host "Another watchdog instance is already running. Exiting."
    exit 0
}

# Config
$healthUrl = "http://127.0.0.1:8080/api/v1/health"
$processName = "racecontrol"
$logFile = "C:\RacingPoint\racecontrol-watchdog.log"
$checkIntervalSec = 10
$maxLogSizeKB = 512

# Backoff state
$consecutiveFails = 0
$restartCount = 0
$restartWindowStart = Get-Date
$maintenanceMode = $false
$maintenanceStart = $null
$backoffDelays = @(5, 30, 120, 600)

function Write-Log {
    param([string]$msg)
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "$ts | $msg"
    Add-Content -Path $logFile -Value $line -ErrorAction SilentlyContinue
    if ((Test-Path $logFile) -and ((Get-Item $logFile).Length / 1KB -gt $maxLogSizeKB)) {
        Move-Item -Path $logFile -Destination "C:\RacingPoint\racecontrol-watchdog-prev.log" -Force -ErrorAction SilentlyContinue
    }
}

function Test-Health {
    try {
        $response = Invoke-WebRequest -Uri $healthUrl -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
        if ($response.StatusCode -eq 200) {
            $json = $response.Content | ConvertFrom-Json
            if ($json.status -eq "ok") { return $true }
        }
        return $false
    } catch {
        return $false
    }
}

function Test-ProcessRunning {
    $proc = Get-Process -Name $processName -ErrorAction SilentlyContinue
    return ($null -ne $proc)
}

function Restart-Racecontrol {
    param([string]$reason)

    # Check OTA sentinel
    if (Test-Path "C:\RacingPoint\OTA_DEPLOYING") {
        Write-Log "OTA_DEPLOYING sentinel present - skipping restart"
        return
    }

    # Check maintenance mode
    if ($maintenanceMode) {
        $elapsed = (Get-Date) - $maintenanceStart
        $elapsedMin = [int]$elapsed.TotalMinutes
        if ($elapsedMin -lt 30) {
            Write-Log "MAINTENANCE_MODE active ${elapsedMin}min - skipping restart"
            return
        }
        $script:maintenanceMode = $false
        $script:maintenanceStart = $null
        $script:restartCount = 0
        $script:restartWindowStart = Get-Date
        Write-Log "MAINTENANCE_MODE auto-cleared after 30 minutes"
    }

    # Restart storm detection: 3+ restarts in 10 min = maintenance mode
    $windowElapsed = (Get-Date) - $restartWindowStart
    if ($windowElapsed.TotalMinutes -gt 10) {
        $script:restartCount = 0
        $script:restartWindowStart = Get-Date
    }
    $script:restartCount++
    if ($restartCount -ge 3) {
        $script:maintenanceMode = $true
        $script:maintenanceStart = Get-Date
        Write-Log "MAINTENANCE_MODE ACTIVATED - 3 restarts in 10 minutes. Auto-clears in 30min."
        $modeTs = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
        Set-Content -Path "C:\RacingPoint\MAINTENANCE_MODE" -Value "watchdog-escalation $modeTs"
        return
    }

    # Calculate backoff delay
    $backoffIdx = [Math]::Min($consecutiveFails, $backoffDelays.Count - 1)
    $delay = $backoffDelays[$backoffIdx]

    Write-Log "RESTART: $reason - attempt $restartCount backoff ${delay}s"

    # Kill existing racecontrol
    Stop-Process -Name $processName -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2

    # Start via schtasks
    $schtResult = schtasks /Run /TN StartRCDirect 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Log "schtasks failed: $schtResult - trying direct start"
        Start-Process -FilePath "C:\RacingPoint\racecontrol.exe" -WorkingDirectory "C:\RacingPoint" -WindowStyle Hidden
    }

    Start-Sleep -Seconds $delay

    # Verify restart worked
    if (Test-Health) {
        Write-Log "RESTART SUCCESS: racecontrol healthy after restart"
        $script:consecutiveFails = 0
    } else {
        Write-Log "RESTART FAILED: racecontrol still unhealthy after ${delay}s"
        $script:consecutiveFails++
    }
}

# Main loop
Write-Log "Watchdog started - PID $PID"

# Give racecontrol time to start on first boot
Start-Sleep -Seconds 15

try {
    while ($true) {
        $healthy = Test-Health
        $running = Test-ProcessRunning

        if ($healthy) {
            if ($consecutiveFails -gt 0) {
                Write-Log "Recovered: racecontrol healthy after $consecutiveFails failed checks"
            }
            $consecutiveFails = 0
        } elseif (-not $running) {
            $consecutiveFails++
            Restart-Racecontrol -reason "process not running"
        } else {
            $consecutiveFails++
            if ($consecutiveFails -ge 3) {
                Restart-Racecontrol -reason "health check failed ${consecutiveFails}x - process running but unresponsive"
            } else {
                Write-Log "Health check failed ${consecutiveFails}/3 - waiting for hysteresis"
            }
        }

        Start-Sleep -Seconds $checkIntervalSec
    }
} finally {
    $mutex.ReleaseMutex()
    $mutex.Dispose()
    Write-Log "Watchdog stopped"
}
