# ac-content-inventory.ps1 — Enumerate Assetto Corsa cars + tracks on one pod.
#
# Output: JSON lines. First line = summary. Remaining lines = one object per item.
# Authoritative source: steamapps\appmanifest_244210.acf -> installdir field,
# joined to the library path that contains the manifest. Handles secondary
# library folders (secondary SSDs are common on sim rigs).

$ErrorActionPreference = 'SilentlyContinue'

# ─── Locate Steam + libraries ────────────────────────────────────────────
$steamRoot = @(
    (Get-ItemProperty 'HKCU:\Software\Valve\Steam' -Name SteamPath -EA SilentlyContinue).SteamPath,
    (Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam' -Name InstallPath -EA SilentlyContinue).InstallPath,
    'C:\Program Files (x86)\Steam',
    'C:\Program Files\Steam'
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1

if (-not $steamRoot) { Write-Host '{"error":"steam_not_found"}'; exit 0 }

$libraryPaths = @()
$libraryVdf = Join-Path $steamRoot 'steamapps\libraryfolders.vdf'
if (Test-Path $libraryVdf) {
    $vdf = Get-Content $libraryVdf -Raw
    $vdfMatches = [regex]::Matches($vdf, '"path"\s+"([^"]+)"')
    foreach ($m in $vdfMatches) {
        $p = $m.Groups[1].Value -replace '\\\\', '\'
        if (Test-Path $p) { $libraryPaths += $p }
    }
}
if ($libraryPaths.Count -eq 0) { $libraryPaths = @($steamRoot) }

# ─── Locate AC (appid 244210) ─────────────────────────────────────────────
$acPath = $null
$acInstallDir = $null
foreach ($lib in $libraryPaths) {
    $manifest = Join-Path $lib 'steamapps\appmanifest_244210.acf'
    if (Test-Path $manifest) {
        $content = Get-Content $manifest -Raw
        if ($content -match '"installdir"\s+"([^"]+)"') {
            $acInstallDir = $Matches[1]
            $candidate = Join-Path $lib "steamapps\common\$acInstallDir"
            if (Test-Path $candidate) { $acPath = $candidate; break }
        }
    }
}

if (-not $acPath) {
    $summary = [pscustomobject]@{
        host = $env:COMPUTERNAME
        ac_installed = $false
        error = 'ac_not_found'
        libraries_checked = $libraryPaths
    }
    $summary | ConvertTo-Json -Compress -Depth 4 | Write-Host
    exit 0
}

# ─── Enumerate cars ───────────────────────────────────────────────────────
$carsDir = Join-Path $acPath 'content\cars'
$cars = @()
if (Test-Path $carsDir) {
    Get-ChildItem $carsDir -Directory -EA SilentlyContinue | ForEach-Object {
        $carSlug = $_.Name
        $uiJson = Join-Path $_.FullName 'ui\ui_car.json'
        $displayName = $carSlug
        $brand = ''
        if (Test-Path $uiJson) {
            try {
                $raw = Get-Content $uiJson -Raw -EA SilentlyContinue
                # AC JSON sometimes has invalid escapes — clean minimally
                $raw = $raw -replace '\\(?![\\/"nrtbf])', '\\'
                $j = $raw | ConvertFrom-Json -EA SilentlyContinue
                if ($j -and $j.name) { $displayName = $j.name }
                if ($j -and $j.brand) { $brand = $j.brand }
            } catch { }
        }
        $cars += [pscustomobject]@{
            slug  = $carSlug
            name  = $displayName
            brand = $brand
        }
    }
}

# ─── Enumerate tracks ─────────────────────────────────────────────────────
$tracksDir = Join-Path $acPath 'content\tracks'
$tracks = @()
if (Test-Path $tracksDir) {
    Get-ChildItem $tracksDir -Directory -EA SilentlyContinue | ForEach-Object {
        $trackSlug = $_.Name
        # Tracks may have multiple layouts under ui\<layout>\ui_track.json
        $uiRoot = Join-Path $_.FullName 'ui'
        $layouts = @()
        if (Test-Path $uiRoot) {
            # Root-level ui_track.json (default layout)
            $rootJson = Join-Path $uiRoot 'ui_track.json'
            if (Test-Path $rootJson) { $layouts += '_default' }
            # Subdirs each = additional layout
            Get-ChildItem $uiRoot -Directory -EA SilentlyContinue | ForEach-Object {
                if (Test-Path (Join-Path $_.FullName 'ui_track.json')) {
                    $layouts += $_.Name
                }
            }
        }
        $displayName = $trackSlug
        $rootJson = Join-Path $uiRoot 'ui_track.json'
        if (Test-Path $rootJson) {
            try {
                $raw = Get-Content $rootJson -Raw -EA SilentlyContinue
                $raw = $raw -replace '\\(?![\\/"nrtbf])', '\\'
                $j = $raw | ConvertFrom-Json -EA SilentlyContinue
                if ($j -and $j.name) { $displayName = $j.name }
            } catch { }
        }
        $tracks += [pscustomobject]@{
            slug    = $trackSlug
            name    = $displayName
            layouts = $layouts
        }
    }
}

# ─── Emit ─────────────────────────────────────────────────────────────────
$summary = [pscustomobject]@{
    host            = $env:COMPUTERNAME
    ac_installed    = $true
    ac_path         = $acPath
    ac_installdir   = $acInstallDir
    cars_count      = $cars.Count
    tracks_count    = $tracks.Count
    tracks_layout_count = ($tracks | ForEach-Object { $_.layouts.Count } | Measure-Object -Sum).Sum
    timestamp_utc   = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
}
$summary | ConvertTo-Json -Compress -Depth 4 | Write-Host

foreach ($c in $cars | Sort-Object slug) {
    [pscustomobject]@{ host = $env:COMPUTERNAME; type = 'car'; slug = $c.slug; name = $c.name; brand = $c.brand } |
        ConvertTo-Json -Compress | Write-Host
}
foreach ($t in $tracks | Sort-Object slug) {
    [pscustomobject]@{ host = $env:COMPUTERNAME; type = 'track'; slug = $t.slug; name = $t.name; layouts = $t.layouts } |
        ConvertTo-Json -Compress | Write-Host
}
