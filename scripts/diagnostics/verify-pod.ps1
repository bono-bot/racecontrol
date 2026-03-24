# Pod Verification Script
# Checks: desktop clean, pop-ups disabled, notepad killed, registry correct

$pass = 0
$fail = 0

function Check($name, $condition) {
    if ($condition) {
        Write-Output "PASS: $name"
        $script:pass++
    } else {
        Write-Output "FAIL: $name"
        $script:fail++
    }
}

# --- 1. Desktop cleanliness ---
$userDirs = Get-ChildItem 'C:\Users' -Directory | Where-Object {
    $_.Name -notin @('Public', 'Default', 'Default User', 'All Users', 'DefaultAppPool', 'bono')
}
foreach ($u in $userDirs) {
    $desk = Join-Path $u.FullName 'Desktop'
    if (-not (Test-Path $desk)) { continue }

    $allowed = @(
        'Assetto Corsa.url', 'Assetto Corsa EVO.url', 'Assetto Corsa Rally.url',
        'F1r 25.url', 'Le Mans Ultimate.url', 'iRacing.url', 'iRacing UI.lnk',
        'iRacing Member Website.lnk', 'Forza Horizon 5.url', 'NASCAR 25.url',
        'SteamVR.url', 'Content Manager.exe', 'Content Manager.lnk',
        'Conspit Link 2.0.lnk', 'Staff', 'desktop.ini'
    )

    $items = Get-ChildItem $desk -Force | Where-Object { $_.Name -notin $allowed }
    $clutter = $items | Select-Object -ExpandProperty Name

    Check "Desktop clean ($($u.Name))" ($clutter.Count -eq 0)
    if ($clutter.Count -gt 0) {
        Write-Output "  Clutter: $($clutter -join ', ')"
    }

    # Staff folder exists
    Check "Staff folder exists ($($u.Name))" (Test-Path (Join-Path $desk 'Staff'))
}

# --- 2. Notepad not running ---
$notepad = Get-Process notepad -ErrorAction SilentlyContinue
Check "No Notepad processes" ($null -eq $notepad)

# --- 3. Notepad session restore disabled ---
$profiles = Get-ChildItem 'Registry::HKEY_USERS' | Where-Object { $_.Name -match 'S-1-5-21-\d+-\d+-\d+-\d+$' }
foreach ($p in $profiles) {
    $val = Get-ItemProperty "Registry::$($p.Name)\Software\Microsoft\Notepad" -Name fResumeOpenTabs -ErrorAction SilentlyContinue
    if ($val) {
        Check "Notepad restore disabled ($(Split-Path $p.Name -Leaf))" ($val.fResumeOpenTabs -eq 0)
    } else {
        Check "Notepad restore key exists ($(Split-Path $p.Name -Leaf))" $false
    }
}

# --- 4. Windows tips disabled ---
$cloud = Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\CloudContent' -ErrorAction SilentlyContinue
Check "Windows tips disabled" ($cloud.DisableSoftLanding -eq 1)
Check "Consumer features disabled" ($cloud.DisableWindowsConsumerFeatures -eq 1)

# --- 5. Notification center disabled ---
$notif = Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\Explorer' -ErrorAction SilentlyContinue
Check "Notification center disabled" ($notif.DisableNotificationCenter -eq 1)

# --- 6. Windows Update nag disabled ---
$wu = Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\WindowsUpdate' -ErrorAction SilentlyContinue
Check "WU restart notification disabled" ($wu.SetAutoRestartNotificationDisable -eq 1)

# --- 7. OOBE nag disabled ---
$oobe = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\UserProfileEngagement' -ErrorAction SilentlyContinue
Check "OOBE nag disabled" ($oobe.ScoobeSystemSettingEnabled -eq 0)

# --- 8. Toast notifications disabled ---
$toast = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Notifications\Settings' -ErrorAction SilentlyContinue
Check "Global toasts disabled" ($toast.NOC_GLOBAL_SETTING_TOASTS_ENABLED -eq 0)

# --- 9. Edge first-run disabled ---
$edge = Get-ItemProperty 'HKLM:\SOFTWARE\Policies\Microsoft\Edge' -ErrorAction SilentlyContinue
Check "Edge first-run disabled" ($edge.HideFirstRunExperience -eq 1)

# --- 10. Start menu ads disabled (check first found profile) ---
if ($profiles.Count -gt 0) {
    $p = $profiles[0]
    $cdm = Get-ItemProperty "Registry::$($p.Name)\Software\Microsoft\Windows\CurrentVersion\ContentDeliveryManager" -ErrorAction SilentlyContinue
    Check "Start menu suggestions disabled" ($cdm.SystemPaneSuggestionsEnabled -eq 0)
    Check "Silent app installs disabled" ($cdm.SilentInstalledAppsEnabled -eq 0)
}

# --- Summary ---
Write-Output ""
Write-Output "=== RESULT: $pass PASS / $fail FAIL ==="
