#Requires -Version 5.1
<#
.SYNOPSIS
    Deploy Tailscale to all 8 Racing Point pods and the server via WinRM.

.DESCRIPTION
    Downloads Tailscale MSI from James's HTTP deploy server, installs silently
    as a Windows Service, then joins the Racing Point tailnet with a pre-auth key.

    CANARY-FIRST: Deploys to Pod 8 first. Pauses for manual verification.
    Type 'yes' to roll to remaining pods + server, or 'no' to abort.

.PREREQUISITES
    1. Tailscale MSI must be in James's deploy-staging directory:
       C:\Users\bono\racingpoint\deploy-staging\tailscale-setup-latest-amd64.msi
    2. Start the HTTP server before running this script:
       python -m http.server 9998 --directory C:\Users\bono\racingpoint\deploy-staging --bind 0.0.0.0
    3. Replace PREAUTH_KEY_REPLACE_ME below with a real reusable pre-auth key from:
       Tailscale Admin Console -> Settings -> Keys -> Generate auth key
       - Check: Reusable, Pre-authorized
       - Expiry: 90 days max, note expiry date in a comment here
    4. Replace ADMIN_PASSWORD_REPLACE_ME with the pod Windows admin password.
    5. Run from James's machine (192.168.31.27) as a normal user (not admin).

.NOTES
    Auth key expiry: <SET_THIS_WHEN_GENERATED> -- add to Uday's calendar for renewal.
    Port conflict: Tailscale uses UDP 41641 (WireGuard) -- not blocked by venue firewall (internal LAN).
    Relay port: 8099 (bono_relay endpoint on server) -- outside AC HTTP port range 8081-8096.
#>

# --- CONFIGURATION -- EDIT BEFORE RUNNING -------------------------------------------

$PREAUTH_KEY = "PREAUTH_KEY_REPLACE_ME"   # tskey-auth-xxxxx... from Tailscale admin console
$ADMIN_USER  = "RacingPoint"              # Windows admin username on all pods (same across fleet)
$ADMIN_PASS  = "ADMIN_PASSWORD_REPLACE_ME" # Windows admin password -- ask Uday

$MSI_DOWNLOAD_URL = "http://192.168.31.27:9998/tailscale-setup-latest-amd64.msi"
$LOCAL_MSI_PATH   = "C:\RacingPoint\tailscale.msi"
$TAILSCALE_EXE    = "C:\Program Files\Tailscale\tailscale.exe"

# --- POD DEFINITIONS ----------------------------------------------------------------

$Pods = @(
    @{ Number = 1; IP = "192.168.31.89"; Hostname = "pod-1" }
    @{ Number = 2; IP = "192.168.31.33"; Hostname = "pod-2" }
    @{ Number = 3; IP = "192.168.31.28"; Hostname = "pod-3" }
    @{ Number = 4; IP = "192.168.31.88"; Hostname = "pod-4" }
    @{ Number = 5; IP = "192.168.31.86"; Hostname = "pod-5" }
    @{ Number = 6; IP = "192.168.31.87"; Hostname = "pod-6" }
    @{ Number = 7; IP = "192.168.31.38"; Hostname = "pod-7" }
    @{ Number = 8; IP = "192.168.31.91"; Hostname = "pod-8" }  # CANARY
)

$Server = @{ IP = "192.168.31.23"; Hostname = "racing-point-server" }

# --- GUARD RAILS --------------------------------------------------------------------

if ($PREAUTH_KEY -eq "PREAUTH_KEY_REPLACE_ME") {
    Write-Error "ERROR: Replace PREAUTH_KEY_REPLACE_ME with a real Tailscale pre-auth key before running."
    exit 1
}

if ($ADMIN_PASS -eq "ADMIN_PASSWORD_REPLACE_ME") {
    Write-Error "ERROR: Replace ADMIN_PASSWORD_REPLACE_ME with the pod admin password before running."
    exit 1
}

# --- CREDENTIAL ---------------------------------------------------------------------

$SecurePass = ConvertTo-SecureString $ADMIN_PASS -AsPlainText -Force
$Cred = New-Object System.Management.Automation.PSCredential($ADMIN_USER, $SecurePass)

# --- INSTALL FUNCTION ---------------------------------------------------------------

function Install-Tailscale {
    param(
        [string]$ComputerName,
        [string]$TailscaleHostname,
        [System.Management.Automation.PSCredential]$Credential
    )

    Write-Host "`n>>> Installing Tailscale on $ComputerName (hostname: $TailscaleHostname)" -ForegroundColor Cyan

    # Step 1: Download MSI from James's HTTP server (inside WinRM session -- avoids UNC double-hop)
    Write-Host "  [1/4] Downloading MSI from $MSI_DOWNLOAD_URL ..."
    try {
        Invoke-Command -ComputerName $ComputerName -Credential $Credential -ScriptBlock {
            param($Url, $Dest)
            # Ensure destination directory exists
            if (-not (Test-Path "C:\RacingPoint")) {
                New-Item -ItemType Directory -Path "C:\RacingPoint" -Force | Out-Null
            }
            # Delete stale MSI if present
            if (Test-Path $Dest) { Remove-Item $Dest -Force }
            Invoke-WebRequest -Uri $Url -OutFile $Dest -UseBasicParsing
            $size = (Get-Item $Dest).Length
            Write-Host "    Downloaded: $Dest ($size bytes)"
        } -ArgumentList $MSI_DOWNLOAD_URL, $LOCAL_MSI_PATH
    } catch {
        Write-Error "  FAILED: Download failed on $ComputerName -- $_"
        return $false
    }

    # Step 2: Silent MSI install (registers tailscaled Windows Service)
    Write-Host "  [2/4] Installing MSI silently ..."
    try {
        $result = Invoke-Command -ComputerName $ComputerName -Credential $Credential -ScriptBlock {
            param($MsiPath)
            $proc = Start-Process msiexec.exe `
                -ArgumentList "/i `"$MsiPath`" /quiet /norestart TS_UNATTENDEDMODE=always TS_NOLAUNCH=true" `
                -Wait -PassThru
            return $proc.ExitCode
        } -ArgumentList $LOCAL_MSI_PATH
        if ($result -ne 0) {
            Write-Error "  FAILED: msiexec exit code $result on $ComputerName (1603=permissions, 1619=MSI not found)"
            return $false
        }
        Write-Host "    MSI install complete (exit code 0)"
    } catch {
        Write-Error "  FAILED: MSI install error on $ComputerName -- $_"
        return $false
    }

    # Step 3: Wait for tailscaled service to fully start
    Write-Host "  [3/4] Waiting 5s for tailscaled service to start ..."
    Start-Sleep -Seconds 5

    # Step 4: Join tailnet with pre-auth key
    Write-Host "  [4/4] Joining tailnet as '$TailscaleHostname' ..."
    try {
        Invoke-Command -ComputerName $ComputerName -Credential $Credential -ScriptBlock {
            param($ExePath, $AuthKey, $Hostname)
            & $ExePath up `
                --unattended `
                --auth-key=$AuthKey `
                --hostname=$Hostname `
                --reset
        } -ArgumentList $TAILSCALE_EXE, $PREAUTH_KEY, $TailscaleHostname
    } catch {
        Write-Error "  FAILED: tailscale up failed on $ComputerName -- $_"
        return $false
    }

    # Verify assigned Tailscale IP
    $tsIp = Invoke-Command -ComputerName $ComputerName -Credential $Credential -ScriptBlock {
        param($ExePath)
        try { & $ExePath ip -4 } catch { "ERROR: $($_.Exception.Message)" }
    } -ArgumentList $TAILSCALE_EXE

    if ($tsIp -match "^100\.") {
        Write-Host "  SUCCESS: $ComputerName joined tailnet -- Tailscale IP: $tsIp" -ForegroundColor Green
        return $true
    } else {
        Write-Warning "  WARNING: Unexpected Tailscale IP response: '$tsIp' -- verify in Tailscale admin console"
        return $false
    }
}

# --- CANARY: POD 8 FIRST ------------------------------------------------------------

Write-Host "`n============================================================" -ForegroundColor Yellow
Write-Host " PHASE 27: Tailscale Fleet Deploy -- CANARY: Pod 8 first" -ForegroundColor Yellow
Write-Host "============================================================`n" -ForegroundColor Yellow

$pod8 = $Pods | Where-Object { $_.Number -eq 8 }
$canaryResult = Install-Tailscale -ComputerName $pod8.IP -TailscaleHostname $pod8.Hostname -Credential $Cred

if (-not $canaryResult) {
    Write-Error "`nCANARY FAILED on Pod 8. Fix the issue before rolling to fleet. Aborting."
    exit 1
}

Write-Host "`n============================================================" -ForegroundColor Yellow
Write-Host " CANARY COMPLETE. Verify Pod 8 in Tailscale admin console:" -ForegroundColor Yellow
Write-Host "   1. Visit https://login.tailscale.com/admin/machines" -ForegroundColor Yellow
Write-Host "   2. Confirm 'pod-8' appears with a 100.x.x.x IP" -ForegroundColor Yellow
Write-Host "   3. From James's machine (after James joins tailnet): ping <pod-8-tailscale-ip>" -ForegroundColor Yellow
Write-Host "============================================================`n" -ForegroundColor Yellow

$confirm = Read-Host "Roll to all 8 pods + server? Type 'yes' to continue, anything else to abort"
if ($confirm -ne "yes") {
    Write-Host "Aborted by user. Pod 8 is enrolled, fleet rollout skipped." -ForegroundColor Red
    exit 0
}

# --- FLEET ROLLOUT: PODS 1-7 + SERVER -----------------------------------------------

Write-Host "`n>>> Rolling out to Pods 1-7 + Server ..." -ForegroundColor Cyan

$failures = @()

# Pods 1-7 (skip Pod 8 -- already done)
foreach ($pod in ($Pods | Where-Object { $_.Number -ne 8 })) {
    $ok = Install-Tailscale -ComputerName $pod.IP -TailscaleHostname $pod.Hostname -Credential $Cred
    if (-not $ok) { $failures += $pod.Hostname }
}

# Server
$ok = Install-Tailscale -ComputerName $Server.IP -TailscaleHostname $Server.Hostname -Credential $Cred
if (-not $ok) { $failures += $Server.Hostname }

# --- SUMMARY ------------------------------------------------------------------------

Write-Host "`n============================================================" -ForegroundColor Yellow
Write-Host " FLEET DEPLOY SUMMARY" -ForegroundColor Yellow
Write-Host "============================================================" -ForegroundColor Yellow

if ($failures.Count -eq 0) {
    Write-Host " All 9 devices (8 pods + server) joined tailnet successfully." -ForegroundColor Green
} else {
    Write-Host " Failures: $($failures -join ', ')" -ForegroundColor Red
    Write-Host " Re-run script for failed devices after investigating." -ForegroundColor Red
}

Write-Host "`nNext steps:" -ForegroundColor Cyan
Write-Host "  1. Verify all devices in Tailscale admin console (https://login.tailscale.com/admin/machines)"
Write-Host "  2. Note server's Tailscale IP (racing-point-server) -- needed for racecontrol.toml [bono] section"
Write-Host "  3. Note Bono's VPS Tailscale IP -- Bono must join tailnet separately"
Write-Host "  4. Run Plan 05: update racecontrol.toml with Tailscale IPs and deploy new racecontrol binary"
Write-Host ""
