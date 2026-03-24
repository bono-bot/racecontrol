# Disable Windows pop-ups, notifications, tips, and Notepad auto-restore
# Run as SYSTEM via rc-agent

$ErrorActionPreference = 'SilentlyContinue'

# --- Kill Notepad if running ---
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Write-Output "Killed Notepad processes"

# --- Disable Notepad session restore (Win11) ---
# Prevents Notepad from reopening previous tabs on login
$notepadReg = 'HKCU:\Software\Microsoft\Notepad'
New-Item -Path $notepadReg -Force | Out-Null
Set-ItemProperty -Path $notepadReg -Name 'fResumeOpenTabs' -Value 0 -Type DWord
Set-ItemProperty -Path $notepadReg -Name 'fWindowsNotepadAutoSave' -Value 0 -Type DWord
Write-Output "Disabled Notepad session restore"

# Also for all users via HKLM default profile
reg load HKU\DefaultUser C:\Users\Default\NTUSER.DAT 2>$null
reg add "HKU\DefaultUser\Software\Microsoft\Notepad" /v fResumeOpenTabs /t REG_DWORD /d 0 /f 2>$null
reg add "HKU\DefaultUser\Software\Microsoft\Notepad" /v fWindowsNotepadAutoSave /t REG_DWORD /d 0 /f 2>$null
reg unload HKU\DefaultUser 2>$null

# For logged-in users (iterate loaded profiles)
$profiles = Get-ChildItem 'Registry::HKEY_USERS' | Where-Object { $_.Name -match 'S-1-5-21-\d+-\d+-\d+-\d+$' }
foreach ($p in $profiles) {
    $path = "Registry::$($p.Name)\Software\Microsoft\Notepad"
    New-Item -Path $path -Force | Out-Null
    Set-ItemProperty -Path $path -Name 'fResumeOpenTabs' -Value 0 -Type DWord
    Set-ItemProperty -Path $path -Name 'fWindowsNotepadAutoSave' -Value 0 -Type DWord
}
Write-Output "Disabled Notepad restore for all user profiles"

# --- Disable Windows Tips & Suggestions ---
$cdm = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent'
New-Item -Path $cdm -Force | Out-Null
Set-ItemProperty -Path $cdm -Name 'DisableSoftLanding' -Value 1 -Type DWord
Set-ItemProperty -Path $cdm -Name 'DisableWindowsConsumerFeatures' -Value 1 -Type DWord
Set-ItemProperty -Path $cdm -Name 'DisableCloudOptimizedContent' -Value 1 -Type DWord
Set-ItemProperty -Path $cdm -Name 'DisableTailoredExperiencesWithDiagnosticData' -Value 1 -Type DWord
Write-Output "Disabled Windows tips & suggestions"

# --- Disable Toast Notifications ---
$toast = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer'
New-Item -Path $toast -Force | Out-Null
Set-ItemProperty -Path $toast -Name 'DisableNotificationCenter' -Value 1 -Type DWord
Write-Output "Disabled notification center"

# --- Disable Windows Update notifications ---
$wu = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate'
New-Item -Path $wu -Force | Out-Null
Set-ItemProperty -Path $wu -Name 'SetAutoRestartNotificationDisable' -Value 1 -Type DWord
Write-Output "Disabled Windows Update restart notifications"

# --- Disable "Get Even More Out of Windows" / OOBE nag ---
$oobe = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\UserProfileEngagement'
New-Item -Path $oobe -Force | Out-Null
Set-ItemProperty -Path $oobe -Name 'ScoobeSystemSettingEnabled' -Value 0 -Type DWord
Write-Output "Disabled OOBE nag screen"

# --- Disable "Finish setting up" reminders ---
foreach ($p in $profiles) {
    $path = "Registry::$($p.Name)\Software\Microsoft\Windows\CurrentVersion\UserProfileEngagement"
    New-Item -Path $path -Force | Out-Null
    Set-ItemProperty -Path $path -Name 'ScoobeSystemSettingEnabled' -Value 0 -Type DWord
}
Write-Output "Disabled 'Finish setting up' for all users"

# --- Disable Start Menu suggestions/ads ---
foreach ($p in $profiles) {
    $path = "Registry::$($p.Name)\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager"
    New-Item -Path $path -Force | Out-Null
    Set-ItemProperty -Path $path -Name 'SubscribedContent-338389Enabled' -Value 0 -Type DWord
    Set-ItemProperty -Path $path -Name 'SubscribedContent-310093Enabled' -Value 0 -Type DWord
    Set-ItemProperty -Path $path -Name 'SubscribedContent-338393Enabled' -Value 0 -Type DWord
    Set-ItemProperty -Path $path -Name 'SubscribedContent-353694Enabled' -Value 0 -Type DWord
    Set-ItemProperty -Path $path -Name 'SubscribedContent-353696Enabled' -Value 0 -Type DWord
    Set-ItemProperty -Path $path -Name 'SystemPaneSuggestionsEnabled' -Value 0 -Type DWord
    Set-ItemProperty -Path $path -Name 'SilentInstalledAppsEnabled' -Value 0 -Type DWord
}
Write-Output "Disabled Start Menu suggestions & silent app installs"

# --- Disable Focus Assist / Do Not Disturb pop-ups ---
$focus = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Notifications\Settings'
New-Item -Path $focus -Force | Out-Null
Set-ItemProperty -Path $focus -Name 'NOC_GLOBAL_SETTING_TOASTS_ENABLED' -Value 0 -Type DWord
Write-Output "Disabled global toast notifications"

# --- Disable Edge first-run / welcome ---
$edge = 'HKLM:\SOFTWARE\Policies\Microsoft\Edge'
New-Item -Path $edge -Force | Out-Null
Set-ItemProperty -Path $edge -Name 'HideFirstRunExperience' -Value 1 -Type DWord
Write-Output "Disabled Edge first-run experience"

Write-Output "`n=== ALL DONE ==="
