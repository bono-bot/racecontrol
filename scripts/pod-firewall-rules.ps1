# pod-firewall-rules.ps1 — Compensate for no VLAN with Windows Firewall rules
#
# Since the TP-Link router likely doesn't support VLANs, we use Windows
# Firewall on each pod to block traffic from unknown IPs (customer WiFi).
#
# Allows: server (.23), james (.27), POS (.20), other pods (.89,.33,.28,.88,.86,.87,.38,.91)
# Blocks: everything else on 192.168.31.0/24 (customer phones, spectator)
#
# Run on each pod: powershell -ExecutionPolicy Bypass -File pod-firewall-rules.ps1
# Or deploy via rc-sentry exec.

$ErrorActionPreference = "Stop"

Write-Host "=== Pod Firewall Rules (VLAN Compensator) ==="

# Known infrastructure IPs
$AllowedIPs = @(
    "192.168.31.23",  # Server
    "192.168.31.27",  # James
    "192.168.31.20",  # POS
    "192.168.31.89",  # Pod 1
    "192.168.31.33",  # Pod 2
    "192.168.31.28",  # Pod 3
    "192.168.31.88",  # Pod 4
    "192.168.31.86",  # Pod 5
    "192.168.31.87",  # Pod 6
    "192.168.31.38",  # Pod 7
    "192.168.31.91",  # Pod 8
    "192.168.31.18",  # NVR
    "192.168.31.1"    # Router/gateway
)

# Rule name prefix for management
$RulePrefix = "RP-VLAN-"

# Remove old rules
Write-Host "Removing old RP-VLAN-* rules..."
Get-NetFirewallRule -DisplayName "${RulePrefix}*" -ErrorAction SilentlyContinue | Remove-NetFirewallRule

# Allow all traffic from known infrastructure IPs
foreach ($ip in $AllowedIPs) {
    $ruleName = "${RulePrefix}Allow-${ip}"
    New-NetFirewallRule -DisplayName $ruleName `
        -Direction Inbound `
        -RemoteAddress $ip `
        -Action Allow `
        -Protocol Any `
        -Profile Any `
        -Enabled True | Out-Null
    Write-Host "  ALLOW inbound from $ip"
}

# Allow localhost and Tailscale
New-NetFirewallRule -DisplayName "${RulePrefix}Allow-Localhost" `
    -Direction Inbound -RemoteAddress "127.0.0.1" -Action Allow -Protocol Any -Profile Any -Enabled True | Out-Null
New-NetFirewallRule -DisplayName "${RulePrefix}Allow-Tailscale" `
    -Direction Inbound -RemoteAddress "100.64.0.0/10" -Action Allow -Protocol Any -Profile Any -Enabled True | Out-Null
Write-Host "  ALLOW localhost + Tailscale (100.64.0.0/10)"

# Block all other 192.168.31.x traffic (customer WiFi devices)
New-NetFirewallRule -DisplayName "${RulePrefix}Block-WiFi-Clients" `
    -Direction Inbound `
    -RemoteAddress "192.168.31.0/24" `
    -Action Block `
    -Protocol Any `
    -Profile Any `
    -Enabled True | Out-Null
Write-Host "  BLOCK all other 192.168.31.0/24 (customer WiFi)"

# Note: Allow rules take precedence over Block rules in Windows Firewall
# because they are evaluated first (specific > general).

Write-Host ""
Write-Host "=== Firewall rules applied ==="
Write-Host "Allowed: $($AllowedIPs.Count) infrastructure IPs + localhost + Tailscale"
Write-Host "Blocked: All other 192.168.31.0/24 addresses"
Write-Host ""
Write-Host "To verify: Get-NetFirewallRule -DisplayName 'RP-VLAN-*' | Format-Table"
Write-Host "To remove: Get-NetFirewallRule -DisplayName 'RP-VLAN-*' | Remove-NetFirewallRule"
