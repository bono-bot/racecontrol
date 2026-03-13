# pod-lockdown.ps1 -- One-time kiosk lockdown for Racing Point pods
# Run as Administrator on each pod (via pod-agent /exec or manual install)
# Idempotent: safe to re-run. Requires Explorer restart (automatic).
#
# Reverts: To undo, pass -Undo flag and restart Explorer.
# Admin login is required to access regedit/powershell on locked-down pods.
#
# Changes applied:
#   1. Taskbar auto-hide via StuckRects3 (customer cannot accidentally click taskbar)
#   2. Win key blocked via NoWinKeys (prevents Start Menu access)
#   3. Windows Update restart prompts suppressed (prevents mid-session popups)
#
# NOT blocked: Alt+Tab (customer sees lock screen behind game -- acceptable per CONTEXT.md)

param(
    [switch]$Undo  # Pass -Undo to revert all lockdown changes
)

$ErrorActionPreference = 'SilentlyContinue'

if ($Undo) {
    Write-Host "[pod-lockdown] Reverting lockdown changes..."

    # Restore taskbar visibility (value 2 = visible/normal)
    $p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3'
    $v = (Get-ItemProperty -Path $p -ErrorAction SilentlyContinue).Settings
    if ($v) {
        $v[8] = 2  # 2 = visible (normal)
        Set-ItemProperty -Path $p -Name Settings -Value $v
        Write-Host "[pod-lockdown] Taskbar visibility restored"
    }

    # Remove Win key block
    Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer' -Name 'NoWinKeys' -ErrorAction SilentlyContinue
    Write-Host "[pod-lockdown] Win key block removed"

    # Remove Windows Update suppression
    Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Name 'NoAutoRebootWithLoggedOnUsers' -ErrorAction SilentlyContinue
    Remove-ItemProperty -Path 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU' -Name 'AUOptions' -ErrorAction SilentlyContinue
    Write-Host "[pod-lockdown] Windows Update suppression removed"

    Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
    Write-Host "[pod-lockdown] Lockdown reverted. Explorer restarting..."
    exit 0
}

Write-Host "[pod-lockdown] Applying kiosk lockdown..."

# 1. Auto-hide taskbar (customer cannot find it during a session)
#    StuckRects3 Settings[8] controls taskbar visibility:
#      2 = visible (default), 3 = always-on-top + auto-hide (kiosk mode)
$p = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StuckRects3'
$v = (Get-ItemProperty -Path $p -ErrorAction SilentlyContinue).Settings
if ($v) {
    $v[8] = 3  # 3 = always-on-top + auto-hide (effectively hidden for kiosk)
    Set-ItemProperty -Path $p -Name Settings -Value $v
    Write-Host "[pod-lockdown] Taskbar auto-hide enabled (StuckRects3)"
} else {
    Write-Host "[pod-lockdown] WARNING: StuckRects3 not found -- taskbar state unchanged"
}

# 2. Block Win key (prevents Start Menu access)
#    NoWinKeys = 1 disables Win key and Win+* shortcuts via Group Policy registry
$ep = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer'
New-Item -Path $ep -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path $ep -Name 'NoWinKeys' -Value 1 -Type DWord
Write-Host "[pod-lockdown] Win key blocked (NoWinKeys=1)"

# 3. Suppress Windows Update restart notifications
#    NoAutoRebootWithLoggedOnUsers = 1 prevents forced restarts while user is logged in
#    AUOptions = 2 = download only, no auto-install (avoids surprise installs mid-session)
$wup = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate\AU'
New-Item -Path $wup -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path $wup -Name 'NoAutoRebootWithLoggedOnUsers' -Value 1 -Type DWord
Set-ItemProperty -Path $wup -Name 'AUOptions' -Value 2 -Type DWord
Write-Host "[pod-lockdown] Windows Update restart prompts suppressed"

# 4. Restart Explorer to apply taskbar change immediately
Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
Write-Host "[pod-lockdown] Explorer restarted to apply taskbar change."
Write-Host ""
Write-Host "[pod-lockdown] Lockdown applied successfully."
Write-Host ""
Write-Host "To revert: powershell -ExecutionPolicy Bypass -File pod-lockdown.ps1 -Undo"
