# server-health-monitor.ps1
# Runs on James (.27) via Task Scheduler every 2 minutes.
# Checks racecontrol health on server (.23) and attempts remote restart if down.
# Layer 3 redundancy: external monitoring independent of server processes.

# Config
$serverLanIp = "192.168.31.23"
$serverTailscaleIp = "100.125.108.37"
$serverUser = "ADMIN"
$healthUrl = "http://${serverLanIp}:8080/api/v1/health"
$sentryHealthUrl = "http://${serverLanIp}:8091/health"
$logFile = "C:\RacingPoint\server-health-monitor.log"
$stateFile = "C:\RacingPoint\server-health-monitor.state"
$maxLogSizeKB = 256
$commsLinkDir = "C:\Users\bono\racingpoint\comms-link"

function Write-Log($msg) {
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "$ts | $msg"
    Add-Content -Path $logFile -Value $line -ErrorAction SilentlyContinue
    # Rotate
    if ((Test-Path $logFile) -and ((Get-Item $logFile).Length / 1KB -gt $maxLogSizeKB)) {
        Move-Item -Path $logFile -Destination "${logFile}.prev" -Force -ErrorAction SilentlyContinue
    }
}

function Get-State {
    if (Test-Path $stateFile) {
        try {
            return Get-Content $stateFile | ConvertFrom-Json
        } catch {
            return @{ consecutive_fails = 0; last_restart_attempt = ""; total_restarts = 0 }
        }
    }
    return @{ consecutive_fails = 0; last_restart_attempt = ""; total_restarts = 0 }
}

function Save-State($state) {
    $state | ConvertTo-Json | Set-Content -Path $stateFile -ErrorAction SilentlyContinue
}

$expectedBuildId = ""  # Set after deploy; empty = skip build_id check

function Test-ServerHealth {
    try {
        $response = Invoke-WebRequest -Uri $healthUrl -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
        if ($response.StatusCode -eq 200) {
            $json = $response.Content | ConvertFrom-Json
            if ($json.status -eq "ok") {
                # Check build_id if expected value is configured
                if ($expectedBuildId -and $json.build_id) {
                    if ($json.build_id -ne $expectedBuildId) {
                        Write-Log "WARNING: build_id mismatch - expected=$expectedBuildId actual=$($json.build_id)"
                    }
                }
                return $true
            }
        }
        return $false
    } catch {
        return $false
    }
}

function Test-ServerPing {
    $result = Test-Connection -ComputerName $serverLanIp -Count 1 -Quiet -ErrorAction SilentlyContinue
    return $result
}

function Test-SentryHealth {
    try {
        $response = Invoke-WebRequest -Uri $sentryHealthUrl -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
        return ($response.StatusCode -eq 200)
    } catch {
        return $false
    }
}

function Send-Alert($message) {
    # Alert via comms-link WebSocket (notify Bono)
    try {
        $env:COMMS_PSK = "85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0"
        $env:COMMS_URL = "ws://srv1422716.hstgr.cloud:8765"
        $sendScript = Join-Path $commsLinkDir "send-message.js"
        if (Test-Path $sendScript) {
            $nodeExe = "C:\Program Files\nodejs\node.exe"
            if (-not (Test-Path $nodeExe)) { $nodeExe = "node" }
            & $nodeExe $sendScript "[SERVER-MONITOR] $message" 2>&1 | Out-Null
        }
    } catch {
        Write-Log "Alert send failed: $_"
    }
}

function Restart-ViaSSH {
    param([string]$ip, [string]$label)

    Write-Log "Attempting restart via SSH [${label} ${ip}]..."
    try {
        # Try schtasks first (most reliable for non-interactive restart)
        $result = ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no -o BatchMode=yes "${serverUser}@${ip}" "schtasks /Run /TN StartRCDirect" 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Log "SSH restart via schtasks succeeded [$label]"
            return $true
        }

        # Fallback: direct start
        $result = ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no -o BatchMode=yes "${serverUser}@${ip}" "cd /d C:\RacingPoint && start /B racecontrol.exe" 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Log "SSH direct start succeeded [$label]"
            return $true
        }

        Write-Log "SSH restart failed [$label]: $result"
        return $false
    } catch {
        Write-Log "SSH exception [$label]: $_"
        return $false
    }
}

function Restart-ViaSentry {
    Write-Log "Attempting restart via rc-sentry exec..."
    try {
        # rc-sentry has its own exec endpoint on :8091
        $body = '{"cmd":"schtasks /Run /TN StartRCDirect","reason":"server-health-monitor: racecontrol down"}'
        $response = Invoke-WebRequest -Uri "http://${serverLanIp}:8091/exec" -Method POST -Body $body -ContentType "application/json" -TimeoutSec 10 -UseBasicParsing -ErrorAction Stop
        if ($response.StatusCode -eq 200) {
            Write-Log "Sentry exec restart succeeded"
            return $true
        }
            $code = $response.StatusCode
        Write-Log "Sentry exec returned: $code"
        return $false
    } catch {
        Write-Log "Sentry exec failed: $_"
        return $false
    }
}

function Restart-ViaTaskSchedulerRemote {
    Write-Log "Attempting restart via remote schtasks..."
    try {
        $result = schtasks /Run /S $serverLanIp /U $serverUser /TN StartRCDirect 2>&1
        if ($LASTEXITCODE -eq 0) {
            Write-Log "Remote schtasks restart succeeded"
            return $true
        }
        Write-Log "Remote schtasks failed: $result"
        return $false
    } catch {
        Write-Log "Remote schtasks exception: $_"
        return $false
    }
}

# === MAIN ===

$state = Get-State

# Check if server is even reachable
$pingOk = Test-ServerPing
if (-not $pingOk) {
    Write-Log "Server unreachable - ping failed - network issue or server powered off"
    $state.consecutive_fails++
    Save-State $state
    if ($state.consecutive_fails -ge 5) {
        Send-Alert "Server .23 unreachable for 10+ minutes (ping fails). May need physical check."
    }
    exit 0
}

# Check racecontrol health
$healthy = Test-ServerHealth
if ($healthy) {
    if ($state.consecutive_fails -gt 0) {
        $prevFails = $state.consecutive_fails
        Write-Log "RECOVERED: racecontrol healthy after $prevFails failed checks"
        if ($state.consecutive_fails -ge 3) {
            Send-Alert "Server recovered. Racecontrol healthy again after $prevFails failed checks."
        }
    }
    # Heartbeat log every 10th check (~20 min) so we know the monitor is alive
    $checkCount = if ($state.PSObject.Properties['check_count']) { $state.check_count } else { 0 }
    $checkCount++
    $state | Add-Member -NotePropertyName check_count -NotePropertyValue $checkCount -Force
    if ($checkCount % 10 -eq 1) {
        Write-Log "OK - server healthy (check #$checkCount)"
    }
    $state.consecutive_fails = 0
    Save-State $state
    exit 0
}

# Health check failed
$state.consecutive_fails++
$failCount = $state.consecutive_fails
Write-Log "Health check FAILED - $failCount consecutive"

# Hysteresis: wait for 2 consecutive failures (4 minutes) before acting
if ($state.consecutive_fails -lt 2) {
    Write-Log "Waiting for hysteresis - need 2 consecutive failures"
    Save-State $state
    exit 0
}

# Rate limit: max 1 restart attempt per 5 minutes
if ($state.last_restart_attempt) {
    $lastAttempt = [DateTime]::Parse($state.last_restart_attempt)
    $elapsed = (Get-Date) - $lastAttempt
    if ($elapsed.TotalMinutes -lt 5) {
        $elapsedSec = [int]$elapsed.TotalSeconds
        Write-Log "Rate limited: last restart attempt was ${elapsedSec}s ago - min 5min"
        Save-State $state
        exit 0
    }
}

# === ATTEMPT RESTART (escalating methods) ===

Write-Log "=== RESTART SEQUENCE INITIATED ==="
$state.last_restart_attempt = (Get-Date).ToString("o")
$state.total_restarts++
Save-State $state

$downMinutes = $state.consecutive_fails * 2
Send-Alert "Racecontrol DOWN on server .23 for $downMinutes minutes. Attempting remote restart..."

$restarted = $false

# Method 1: rc-sentry exec (if sentry is alive)
$sentryAlive = Test-SentryHealth
if ($sentryAlive -and -not $restarted) {
    $restarted = Restart-ViaSentry
}

# Method 2: Remote schtasks
if (-not $restarted) {
    $restarted = Restart-ViaTaskSchedulerRemote
}

# Method 3: SSH via LAN
if (-not $restarted) {
    $restarted = Restart-ViaSSH -ip $serverLanIp -label "LAN"
}

# Method 4: SSH via Tailscale
if (-not $restarted) {
    $restarted = Restart-ViaSSH -ip $serverTailscaleIp -label "Tailscale"
}

if ($restarted) {
    Write-Log "Restart command sent. Will verify on next check in 2 min."
    Send-Alert "Restart command sent to server. Verifying in 2 minutes..."
} else {
    Write-Log "ALL RESTART METHODS FAILED  - requires physical intervention"
    Send-Alert "CRITICAL: All 4 restart methods failed for racecontrol on server .23. Physical restart required!"
}

Save-State $state
