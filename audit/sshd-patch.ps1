# sshd_config hardening — MMA 4-model consensus (2026-03-31)
# Adds: ClientAliveInterval, ClientAliveCountMax, MaxSessions
# Safe: only APPENDS if settings not already present

$sshdConfig = "C:\ProgramData\ssh\sshd_config"
$content = Get-Content $sshdConfig -Raw

$changes = @()

# ClientAliveInterval 30 (keepalive every 30s)
if ($content -notmatch '(?m)^ClientAliveInterval\s') {
    $content = $content -replace '(?m)^#ClientAliveInterval\s+\d+', 'ClientAliveInterval 30'
    if ($content -notmatch '(?m)^ClientAliveInterval\s') {
        $content += "`r`nClientAliveInterval 30"
    }
    $changes += "ClientAliveInterval 30"
}

# ClientAliveCountMax 3
if ($content -notmatch '(?m)^ClientAliveCountMax\s') {
    $content = $content -replace '(?m)^#ClientAliveCountMax\s+\d+', 'ClientAliveCountMax 3'
    if ($content -notmatch '(?m)^ClientAliveCountMax\s') {
        $content += "`r`nClientAliveCountMax 3"
    }
    $changes += "ClientAliveCountMax 3"
}

# MaxSessions 50 (default 10 is too low for fleet automation)
if ($content -notmatch '(?m)^MaxSessions\s') {
    $content = $content -replace '(?m)^#MaxSessions\s+\d+', 'MaxSessions 50'
    if ($content -notmatch '(?m)^MaxSessions\s') {
        $content += "`r`nMaxSessions 50"
    }
    $changes += "MaxSessions 50"
}

if ($changes.Count -gt 0) {
    # Backup first
    Copy-Item $sshdConfig "$sshdConfig.bak-$(Get-Date -Format 'yyyy-MM-dd')" -Force
    Set-Content $sshdConfig $content -NoNewline
    Restart-Service sshd -Force
    Write-Output "PATCHED: $($changes -join ', '). sshd restarted."
} else {
    Write-Output "ALREADY_PATCHED: no changes needed."
}
