# james-firewall-rules.ps1 — Protect James machine services from customer WiFi
# MMA-#4: go2rtc :1984 and Ollama :11434 reachable from customer phones
#
# Allows: server (.23), pods (.89,.33,.28,.88,.86,.87,.38,.91), POS (.20), Tailscale
# Blocks: all other 192.168.31.0/24 on sensitive ports

$SensitivePorts = @(1984, 11434, 8766, 9999)  # go2rtc, Ollama, comms-link, webterm
$AllowedIPs = @(
    "192.168.31.23",   # Server
    "192.168.31.20",   # POS
    "192.168.31.89",   # Pod 1
    "192.168.31.33",   # Pod 2
    "192.168.31.28",   # Pod 3
    "192.168.31.88",   # Pod 4
    "192.168.31.86",   # Pod 5
    "192.168.31.87",   # Pod 6
    "192.168.31.38",   # Pod 7
    "192.168.31.91",   # Pod 8
    "127.0.0.1"        # Localhost
)

Write-Host "James machine firewall rules — protecting sensitive ports from customer WiFi"

# Remove old rules
Get-NetFirewallRule -DisplayName "RP-JAMES-*" -ErrorAction SilentlyContinue | Remove-NetFirewallRule

# Allow from infrastructure IPs
foreach ($ip in $AllowedIPs) {
    New-NetFirewallRule -DisplayName "RP-JAMES-Allow-$ip" `
        -Direction Inbound -Action Allow `
        -RemoteAddress $ip `
        -Protocol TCP -LocalPort $SensitivePorts `
        -Profile Any -ErrorAction SilentlyContinue | Out-Null
}

# Allow Tailscale subnet
New-NetFirewallRule -DisplayName "RP-JAMES-Allow-Tailscale" `
    -Direction Inbound -Action Allow `
    -RemoteAddress "100.64.0.0/10" `
    -Protocol TCP -LocalPort $SensitivePorts `
    -Profile Any -ErrorAction SilentlyContinue | Out-Null

# Block all other 192.168.31.x on sensitive ports
New-NetFirewallRule -DisplayName "RP-JAMES-Block-CustomerWiFi" `
    -Direction Inbound -Action Block `
    -RemoteAddress "192.168.31.0/24" `
    -Protocol TCP -LocalPort $SensitivePorts `
    -Profile Any -ErrorAction SilentlyContinue | Out-Null

Write-Host "Done: $($AllowedIPs.Count) IPs allowed, customer WiFi blocked on ports: $($SensitivePorts -join ', ')"
