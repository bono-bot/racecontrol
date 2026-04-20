# chunk-integrity-probe.ps1
# Runs from James .27 (not from the servers themselves — "verify from user's browser" rule).
# Every $ProbeCadenceSec:
#   1. Fetch the sentinel page for each target.
#   2. Extract every /_next/static/(chunks|css)/* reference from the HTML.
#   3. HEAD each; count non-200.
#   4. If any target hits $ConsecutiveFailuresToAlert consecutive probes with non-200s:
#        a. Call MI Tier 0 (/mesh/audit-check-service) to confirm the seed exists.
#        b. Append incident to local log + INBOX.md.
#        c. Rate-limit per target so we don't spam.
#   5. Write heartbeat file (lets rc-watchdog on .27 audit the monitor).
#
# Does NOT auto-restart. Restart stays in the single-authority lane:
#   - venue: schtasks /Run /TN StartAdmin|StartWeb|StartKiosk on .23
#   - cloud: pm2 restart <name> via comms-link relay
# Humans (or the .27 AI healer if explicitly wired) read the incident and act.
#
# Invocation:
#   pwsh -NoProfile -File scripts/watchdog/chunk-integrity-probe.ps1 `
#        -Config scripts/watchdog/chunk-probes.json `
#        -ServiceKey $env:RCAGENT_SERVICE_KEY
#
# Schtask wrapper: start-chunk-integrity-probe.bat (TODO, separate commit).

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Config,
    [string]$ServiceKey = $env:RCAGENT_SERVICE_KEY,
    [switch]$Once
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $Config)) {
    Write-Error "Config not found: $Config"
    exit 2
}

$cfg = Get-Content -Raw -Path $Config | ConvertFrom-Json

$heartbeatDir = Split-Path -Parent $cfg.heartbeat_file
if (-not (Test-Path $heartbeatDir)) { New-Item -ItemType Directory -Path $heartbeatDir -Force | Out-Null }
$logDir = Split-Path -Parent $cfg.incidents_log
if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir -Force | Out-Null }

# Per-target state kept in memory for the lifetime of this process
$state = @{}
foreach ($t in $cfg.targets) {
    $state[$t.name] = [PSCustomObject]@{
        ConsecutiveFailures = 0
        AlertsThisHour      = 0
        HourWindowStart     = Get-Date
    }
}

function Write-IncidentLog {
    param([string]$Line)
    $ts = (Get-Date).ToString('yyyy-MM-dd HH:mm:ss')
    Add-Content -Path $cfg.incidents_log -Value "$ts | $Line" -ErrorAction SilentlyContinue
}

function Write-Heartbeat {
    $beat = @{
        last_run_utc = (Get-Date).ToUniversalTime().ToString('o')
        targets      = $cfg.targets.Count
    } | ConvertTo-Json -Compress
    Set-Content -Path $cfg.heartbeat_file -Value $beat -ErrorAction SilentlyContinue
}

function Test-Target {
    param($Target)

    $sentinel = $Target.url
    try {
        $html = Invoke-WebRequest -Uri $sentinel -UseBasicParsing -TimeoutSec 10 -ErrorAction Stop
    } catch {
        Write-IncidentLog "PROBE_FETCH_FAILED target=$($Target.name) url=$sentinel err=$($_.Exception.Message)"
        return @{ Ok = $false; FetchFailed = $true; NonTwoHundred = @() }
    }

    # Extract every _next/static/(chunks|css|media)/* URL from the HTML
    $assetRegex = '/_next/static/(chunks|css|media)/[^"\\<>\s]+'
    $assets = [regex]::Matches($html.Content, $assetRegex) | ForEach-Object { $_.Value } | Sort-Object -Unique

    if ($assets.Count -eq 0) {
        # No chunks at all probably means the HTML itself is broken. Treat as failure.
        Write-IncidentLog "PROBE_NO_ASSETS target=$($Target.name) url=$sentinel html_len=$($html.Content.Length)"
        return @{ Ok = $false; FetchFailed = $false; NonTwoHundred = @("<no_assets_in_html>") }
    }

    # HEAD each asset. URL is absolute if it starts with http, else prepend the sentinel's origin.
    $origin = [uri]$sentinel
    $originPrefix = "$($origin.Scheme)://$($origin.Host)$( if ($origin.Port -and $origin.Port -notin 80,443) { ':' + $origin.Port } )"

    $bad = @()
    foreach ($a in $assets) {
        $assetUrl = if ($a.StartsWith('http')) { $a } else { "$originPrefix$a" }
        try {
            $r = Invoke-WebRequest -Uri $assetUrl -Method Head -UseBasicParsing -TimeoutSec 8 -ErrorAction Stop
            if ($r.StatusCode -ne 200) { $bad += "$($r.StatusCode) $assetUrl" }
        } catch {
            $code = 'ERR'
            try { $code = $_.Exception.Response.StatusCode.value__ } catch {}
            $bad += "$code $assetUrl"
        }
    }

    return @{ Ok = ($bad.Count -eq 0); FetchFailed = $false; NonTwoHundred = $bad; AssetsChecked = $assets.Count }
}

function Call-MeshTier0 {
    param($Target)

    $endpoint = if ($Target.env -eq 'cloud') { $cfg.mesh_tier0_endpoint_cloud } else { $cfg.mesh_tier0_endpoint_venue }
    if (-not $ServiceKey) {
        Write-IncidentLog "MESH_TIER0_SKIPPED target=$($Target.name) reason=no_service_key"
        return $null
    }

    $body = @{ problem_key = $Target.fingerprint } | ConvertTo-Json -Compress
    try {
        $r = Invoke-RestMethod -Uri $endpoint -Method Post -Body $body `
            -Headers @{ 'X-Service-Key' = $ServiceKey; 'Content-Type' = 'application/json' } `
            -TimeoutSec 10 -ErrorAction Stop
        return $r
    } catch {
        Write-IncidentLog "MESH_TIER0_FAILED target=$($Target.name) endpoint=$endpoint err=$($_.Exception.Message)"
        return $null
    }
}

function Append-Inbox {
    param([string]$TargetName, [string]$Summary, [string]$MeshMatch)

    $ts = (Get-Date).ToString('yyyy-MM-dd HH:mm IST')
    $block = @"

## $ts — from chunk-integrity-probe
- Target: $TargetName
- Signal: manifest drift detected (chunks returning non-200)
- Tier 0 (MI): $MeshMatch
- Evidence: $Summary
- Action required: human-triggered restart (see seed's fix_action). This probe does NOT auto-restart.

"@
    Add-Content -Path $cfg.inbox_path -Value $block -ErrorAction SilentlyContinue
}

function Rate-LimitOk {
    param($TargetName)
    $s = $state[$TargetName]
    if (((Get-Date) - $s.HourWindowStart).TotalSeconds -ge 3600) {
        $s.HourWindowStart = Get-Date
        $s.AlertsThisHour = 0
    }
    return ($s.AlertsThisHour -lt [int]$cfg.rate_limit_alerts_per_hour_per_target)
}

function Run-OnePass {
    foreach ($t in $cfg.targets) {
        $result = Test-Target -Target $t
        $s = $state[$t.name]

        if ($result.Ok) {
            if ($s.ConsecutiveFailures -gt 0) {
                Write-IncidentLog "RECOVERED target=$($t.name) after_n_failures=$($s.ConsecutiveFailures)"
            }
            $s.ConsecutiveFailures = 0
            continue
        }

        $s.ConsecutiveFailures += 1
        Write-IncidentLog "DRIFT_DETECTED target=$($t.name) consecutive=$($s.ConsecutiveFailures) non200_count=$($result.NonTwoHundred.Count) sample=$($result.NonTwoHundred | Select-Object -First 3 -join ' ; ')"

        if ($s.ConsecutiveFailures -ge [int]$cfg.consecutive_failures_to_alert -and (Rate-LimitOk -TargetName $t.name)) {
            $mesh = Call-MeshTier0 -Target $t
            $meshSummary = if ($mesh -and $mesh.matched) { "matched seed $($t.fingerprint) severity=$($mesh.severity)" } elseif ($mesh) { "no seed match" } else { "lookup failed" }
            $evidenceSummary = ($result.NonTwoHundred | Select-Object -First 5) -join '; '
            Append-Inbox -TargetName $t.name -Summary $evidenceSummary -MeshMatch $meshSummary
            Write-IncidentLog "ALERTED target=$($t.name) mesh=$meshSummary"
            $s.AlertsThisHour += 1
        }
    }
    Write-Heartbeat
}

if ($Once) {
    Run-OnePass
    exit 0
}

# Daemon loop
while ($true) {
    try { Run-OnePass } catch { Write-IncidentLog "LOOP_ERR $($_.Exception.Message)" }
    Start-Sleep -Seconds ([int]$cfg.probe_cadence_sec)
}
