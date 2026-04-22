# game-inventory.ps1 — Authoritative Steam game inventory for one pod.
#
# Why this exists:
#   The earlier pod audit hardcoded 'C:\Program Files (x86)\Steam\steamapps\common\<game>'
#   and reported F1 25 as "NOT INSTALLED" on Pod 3 — false negative. Steam supports
#   multiple library folders (secondary SSDs are standard for sim rigs), listed in
#   libraryfolders.vdf. The authoritative source for "is game X installed" is the
#   set of appmanifest_*.acf files across ALL configured libraries.
#
# Output: JSON-per-line. Easy to merge across pods.

$ErrorActionPreference = 'SilentlyContinue'

# ─── Locate Steam ────────────────────────────────────────────────────────
$steamRoot = $null
$candidates = @(
    (Get-ItemProperty 'HKCU:\Software\Valve\Steam' -Name SteamPath -EA SilentlyContinue).SteamPath,
    (Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam' -Name InstallPath -EA SilentlyContinue).InstallPath,
    'C:\Program Files (x86)\Steam',
    'C:\Program Files\Steam'
) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
$steamRoot = $candidates

if (-not $steamRoot) {
    Write-Host "{`"host`":`"$env:COMPUTERNAME`",`"error`":`"steam_not_found`"}"
    exit 0
}

# ─── Parse libraryfolders.vdf ────────────────────────────────────────────
$libraryVdf = Join-Path $steamRoot 'steamapps\libraryfolders.vdf'
$libraryPaths = @()
if (Test-Path $libraryVdf) {
    $vdf = Get-Content $libraryVdf -Raw
    # match    "path"    "X:\\Something"
    $vdfMatches = [regex]::Matches($vdf, '"path"\s+"([^"]+)"')
    foreach ($m in $vdfMatches) {
        $p = $m.Groups[1].Value -replace '\\\\', '\'
        if (Test-Path $p) { $libraryPaths += $p }
    }
}
# Always include the primary Steam dir even if vdf missing
if ($libraryPaths -notcontains $steamRoot) { $libraryPaths = @($steamRoot) + $libraryPaths }

# ─── Enumerate every appmanifest in every library ────────────────────────
$games = @()
foreach ($lib in $libraryPaths) {
    $steamapps = Join-Path $lib 'steamapps'
    if (-not (Test-Path $steamapps)) { continue }
    Get-ChildItem $steamapps -Filter 'appmanifest_*.acf' -File -EA SilentlyContinue | ForEach-Object {
        $content = Get-Content $_.FullName -Raw
        $appid = if ($content -match '"appid"\s+"(\d+)"') { $Matches[1] } else { '0' }
        $name  = if ($content -match '"name"\s+"([^"]+)"')   { $Matches[1] } else { '(unknown)' }
        $installdir = if ($content -match '"installdir"\s+"([^"]+)"') { $Matches[1] } else { '' }
        $sizeBytes  = if ($content -match '"SizeOnDisk"\s+"(\d+)"') { [int64]$Matches[1] } else { 0 }
        $lastOwner  = if ($content -match '"LastOwner"\s+"(\d+)"') { $Matches[1] } else { '' }
        $installed = $false
        if ($installdir) {
            $full = Join-Path $steamapps "common\$installdir"
            $installed = Test-Path $full
        }
        $games += [pscustomobject]@{
            appid      = $appid
            name       = $name
            installdir = $installdir
            library    = $lib
            size_gb    = [math]::Round($sizeBytes / 1GB, 2)
            last_owner = $lastOwner
            on_disk    = $installed
        }
    }
}

# ─── Check non-Steam sims (iRacing has its own launcher) ─────────────────
$nonSteam = @()
$iracingPaths = @(
    "$env:USERPROFILE\Documents\iRacing",
    "$env:PROGRAMDATA\iRacing",
    'C:\Program Files\iRacing',
    'C:\iRacing'
)
foreach ($p in $iracingPaths) {
    if (Test-Path $p) {
        $nonSteam += [pscustomobject]@{ name = 'iRacing'; path = $p; source = 'standalone' }
        break
    }
}

# ─── Active Steam user ───────────────────────────────────────────────────
$activeUser = (Get-ItemProperty 'HKCU:\Software\Valve\Steam\ActiveProcess' -EA SilentlyContinue).ActiveUser
$activeUser = if ($activeUser) { "$activeUser" } else { "0" }

# ─── Emit one JSON line per game + one summary line ──────────────────────
$summary = [pscustomobject]@{
    host            = $env:COMPUTERNAME
    user            = $env:USERNAME
    steam_root      = $steamRoot
    steam_libraries = $libraryPaths
    steam_active_user = $activeUser
    steam_game_count  = $games.Count
    nonsteam_games    = $nonSteam
    timestamp_utc     = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
}
$summary | ConvertTo-Json -Compress -Depth 4 | Write-Host

$unique = $games | Sort-Object appid -Unique
# Also fix steam_game_count in the summary we already emitted — recount here
foreach ($g in $unique | Sort-Object name) {
    $g | Select-Object @{n='host';e={$env:COMPUTERNAME}}, appid, name, on_disk, size_gb, library, installdir |
        ConvertTo-Json -Compress | Write-Host
}
