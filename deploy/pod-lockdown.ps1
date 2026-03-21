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
#   4. USB mass storage disabled (USBSTOR Start=4, prevents data exfiltration)
#   5. Accessibility shortcuts disabled (Sticky/Filter/Toggle Keys hotkeys)
#   6. Task Manager disabled (DisableTaskMgr=1, grays out in Ctrl+Alt+Del)
#   7. Explorer restart to apply taskbar change
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

    # Restore USB mass storage
    Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\USBSTOR' -Name 'Start' -Value 3 -Type DWord
    Write-Host "[pod-lockdown] USB mass storage re-enabled"

    # Restore accessibility shortcuts (510 = default with keyboard shortcut enabled)
    Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\StickyKeys' -Name 'Flags' -Value '510' -Type String
    Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\Keyboard Response' -Name 'Flags' -Value '126' -Type String
    Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\ToggleKeys' -Name 'Flags' -Value '62' -Type String
    Write-Host "[pod-lockdown] Accessibility shortcuts restored"

    # Re-enable Task Manager
    Remove-ItemProperty -Path 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\System' -Name 'DisableTaskMgr' -ErrorAction SilentlyContinue
    Write-Host "[pod-lockdown] Task Manager re-enabled"

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

# 4. Disable USB mass storage (prevents data exfiltration via USB stick)
#    Start = 4 means "Disabled" for the USBSTOR driver service
#    Does NOT affect HID devices (keyboards, mice, wheelbases)
$usb = 'HKLM:\SYSTEM\CurrentControlSet\Services\USBSTOR'
Set-ItemProperty -Path $usb -Name 'Start' -Value 4 -Type DWord
Write-Host "[pod-lockdown] USB mass storage disabled (USBSTOR Start=4)"

# 5. Disable accessibility keyboard shortcuts (prevents Sticky Keys popup escape)
#    Flags values disable the "keyboard shortcut to turn on" feature
#    506 = Sticky Keys shortcut (5x Shift) disabled
#    122 = Filter Keys shortcut disabled
#    58  = Toggle Keys shortcut disabled
$sk = 'HKCU:\Control Panel\Accessibility\StickyKeys'
Set-ItemProperty -Path $sk -Name 'Flags' -Value '506' -Type String
$fk = 'HKCU:\Control Panel\Accessibility\Keyboard Response'
Set-ItemProperty -Path $fk -Name 'Flags' -Value '122' -Type String
$tk = 'HKCU:\Control Panel\Accessibility\ToggleKeys'
Set-ItemProperty -Path $tk -Name 'Flags' -Value '58' -Type String
Write-Host "[pod-lockdown] Accessibility shortcuts disabled (Sticky/Filter/Toggle Keys)"

# 6. Disable Task Manager (Ctrl+Alt+Del menu still appears but Task Manager is grayed out)
#    Ctrl+Alt+Del is a kernel-level SAS -- cannot be intercepted by user-mode hooks
$sys = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\System'
New-Item -Path $sys -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path $sys -Name 'DisableTaskMgr' -Value 1 -Type DWord
Write-Host "[pod-lockdown] Task Manager disabled (DisableTaskMgr=1)"

# 7. Restart Explorer to apply taskbar change immediately
Stop-Process -Name explorer -Force -ErrorAction SilentlyContinue
Write-Host "[pod-lockdown] Explorer restarted to apply taskbar change."
Write-Host ""
Write-Host "[pod-lockdown] Lockdown applied successfully."
Write-Host ""
Write-Host "To revert: powershell -ExecutionPolicy Bypass -File pod-lockdown.ps1 -Undo"
