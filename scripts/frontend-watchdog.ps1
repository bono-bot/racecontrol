# frontend-watchdog.ps1
# Singleton watchdog for all 3 Next.js frontend apps on server (.23)
# Monitors kiosk(:3300), web(:3200), admin(:3201) via HTTP health check.
# Auto-restarts crashed apps. Runs as schtask.

# Singleton mutex — prevent multiple instances
$mutexName = "Global\FrontendWatchdog"
$mutex = New-Object System.Threading.Mutex($false, $mutexName)
if (-not $mutex.WaitOne(0)) {
    Write-Host "Another frontend watchdog is already running. Exiting."
    exit 0
}

$logFile = "C:\RacingPoint\frontend-watchdog.log"
$maxLogSizeKB = 512
$checkIntervalSec = 30

# App definitions
$apps = @(
    @{
        Name = "kiosk"
        Port = 3300
        HealthUrl = "http://127.0.0.1:3300/kiosk/api/health"
        Dir = "C:\RacingPoint\kiosk"
        Env = @{ PORT = "3300"; HOSTNAME = "0.0.0.0" }
    },
    @{
        Name = "web"
        Port = 3200
        HealthUrl = "http://127.0.0.1:3200/api/health"
        Dir = "C:\RacingPoint\web"
        Env = @{ PORT = "3200"; HOSTNAME = "0.0.0.0" }
    },
    @{
        Name = "admin"
        Port = 3201
        HealthUrl = "http://127.0.0.1:3201/api/health"
        Dir = "C:\RacingPoint\admin"
        Env = @{ PORT = "3201"; HOSTNAME = "0.0.0.0" }
    }
)

# Track consecutive failures per app for backoff
$failCounts = @{}
foreach ($app in $apps) { $failCounts[$app.Name] = 0 }

function Write-Log {
    param([string]$msg)
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "$ts | $msg"
    Add-Content -Path $logFile -Value $line -ErrorAction SilentlyContinue
    Write-Host $line
    if ((Test-Path $logFile) -and ((Get-Item $logFile).Length / 1KB -gt $maxLogSizeKB)) {
        Move-Item -Path $logFile -Destination "C:\RacingPoint\frontend-watchdog-prev.log" -Force -ErrorAction SilentlyContinue
    }
}

function Test-AppHealth {
    param([string]$url)
    try {
        $response = Invoke-WebRequest -Uri $url -TimeoutSec 5 -UseBasicParsing -ErrorAction Stop
        return ($response.StatusCode -eq 200)
    } catch {
        return $false
    }
}

function Test-PortListening {
    param([int]$port)
    try {
        $conn = New-Object System.Net.Sockets.TcpClient
        $conn.Connect("127.0.0.1", $port)
        $conn.Close()
        return $true
    } catch {
        return $false
    }
}

function Start-App {
    param([hashtable]$app)
    Write-Log "RESTART $($app.Name) on :$($app.Port)"

    # Kill any existing node process on this port
    $listeners = netstat -ano | Select-String ":$($app.Port)\s+.*LISTENING" | ForEach-Object {
        ($_ -split '\s+')[-1]
    } | Sort-Object -Unique
    foreach ($pid in $listeners) {
        if ($pid -and $pid -ne "0") {
            Write-Log "  Killing PID $pid on :$($app.Port)"
            Stop-Process -Id ([int]$pid) -Force -ErrorAction SilentlyContinue
        }
    }
    Start-Sleep -Seconds 2

    # Set env vars and start node
    foreach ($key in $app.Env.Keys) {
        [System.Environment]::SetEnvironmentVariable($key, $app.Env[$key], "Process")
    }

    $nodeExe = "C:\Program Files\nodejs\node.exe"
    $serverJs = Join-Path $app.Dir "server.js"

    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $nodeExe
    $psi.Arguments = $serverJs
    $psi.WorkingDirectory = $app.Dir
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    foreach ($key in $app.Env.Keys) {
        $psi.EnvironmentVariables[$key] = $app.Env[$key]
    }

    try {
        $proc = [System.Diagnostics.Process]::Start($psi)
        Write-Log "  Started $($app.Name) PID=$($proc.Id)"
        Start-Sleep -Seconds 3

        # Verify it's actually listening
        if (Test-PortListening -port $app.Port) {
            Write-Log "  CONFIRMED $($app.Name) listening on :$($app.Port)"
        } else {
            Write-Log "  WARNING $($app.Name) started but not yet listening on :$($app.Port)"
        }
    } catch {
        Write-Log "  FAILED to start $($app.Name): $_"
    }
}

# Main loop
Write-Log "Frontend watchdog started. Monitoring kiosk(:3300), web(:3200), admin(:3201)"

try {
    while ($true) {
        foreach ($app in $apps) {
            $healthy = Test-AppHealth -url $app.HealthUrl

            if ($healthy) {
                if ($failCounts[$app.Name] -gt 0) {
                    Write-Log "$($app.Name) recovered after $($failCounts[$app.Name]) failures"
                }
                $failCounts[$app.Name] = 0
            } else {
                $failCounts[$app.Name]++
                $fc = $failCounts[$app.Name]

                # Only restart after 2 consecutive failures (avoid transient blips)
                if ($fc -ge 2) {
                    # Backoff: don't restart more than once per 60s
                    if ($fc -eq 2 -or ($fc % 4 -eq 0)) {
                        Write-Log "$($app.Name) FAILED health check ($fc consecutive). Restarting..."
                        Start-App -app $app
                    } else {
                        Write-Log "$($app.Name) still down ($fc consecutive). Waiting for backoff cycle."
                    }
                } else {
                    Write-Log "$($app.Name) health check failed (1st). Will retry next cycle."
                }
            }
        }
        Start-Sleep -Seconds $checkIntervalSec
    }
} finally {
    $mutex.ReleaseMutex()
    $mutex.Dispose()
    Write-Log "Frontend watchdog stopped."
}
