# upload-db.ps1 — Upload racecontrol.db snapshot to Google Drive
# Runs on James .27 every 5 minutes via Task Scheduler
# ASCII-only — no special characters in script source
#
# Flow:
#   1. SCP racecontrol.db from server .23 to local temp
#   2. Upload binary to Google Drive folder (overwrite existing)
#   3. Write sync-status.json with timestamp
#
# Credentials: loaded from db-sync.env (NEVER hardcode in this file)

param(
    [string]$ServerIP = "192.168.31.23",
    [string]$ServerUser = "ADMIN",
    [string]$DbPath = "C:/RacingPoint/data/racecontrol.db",
    [string]$LocalTemp = "C:/Users/bono/racingpoint/deploy-staging/db-sync",
    [string]$StatusFile = "C:/Users/bono/racingpoint/deploy-staging/db-sync/sync-status.json",
    [string]$EnvFile = "C:/Users/bono/racingpoint/racecontrol/scripts/db-sync/db-sync.env"
)

$ErrorActionPreference = "Stop"

# Load credentials from env file
if (-not (Test-Path $EnvFile)) {
    Write-Host "ERROR: Credentials file not found: $EnvFile"
    exit 1
}
$envLines = Get-Content $EnvFile | Where-Object { $_ -match '=' -and $_ -notmatch '^\s*#' }
$env = @{}
foreach ($line in $envLines) {
    $parts = $line -split '=', 2
    $env[$parts[0].Trim()] = $parts[1].Trim()
}
$FolderId = $env['DRIVE_FOLDER_ID']

function Get-AccessToken {
    $tokenBody = @{
        client_id = $env['GOOGLE_CLIENT_ID']
        client_secret = $env['GOOGLE_CLIENT_SECRET']
        refresh_token = $env['GOOGLE_REFRESH_TOKEN']
        grant_type = 'refresh_token'
    }
    $resp = Invoke-RestMethod -Uri 'https://oauth2.googleapis.com/token' -Method POST -Body $tokenBody
    return $resp.access_token
}

function Write-Status {
    param([string]$Status, [string]$Message, [long]$DbSize = 0)
    $now = Get-Date -Format 'yyyy-MM-ddTHH:mm:ss'
    $ist = (Get-Date).ToUniversalTime().AddHours(5).AddMinutes(30).ToString('yyyy-MM-ddTHH:mm:ss')
    $obj = @{
        status = $Status
        message = $Message
        timestamp_utc = $now
        timestamp_ist = $ist
        db_size_bytes = $DbSize
    } | ConvertTo-Json
    $obj | Set-Content -Path $StatusFile -Encoding UTF8
}

# Ensure temp dir exists
if (-not (Test-Path $LocalTemp)) {
    New-Item -ItemType Directory -Path $LocalTemp -Force | Out-Null
}

try {
    # Step 1: SCP the database from server
    $localDb = Join-Path $LocalTemp "racecontrol.db"
    Write-Host "[1/4] Copying DB from $ServerUser@$ServerIP..."

    # SQLite in WAL mode is safe to copy while running.
    # SCP the main DB file (required).
    scp "$ServerUser@100.125.108.37:C:/RacingPoint/data/racecontrol.db" $localDb 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Status "error" "SCP of racecontrol.db failed"
        Write-Host "ERROR: SCP failed"
        exit 1
    }
    # WAL/SHM are optional (may not exist if not in WAL mode)
    $ErrorActionPreference = "Continue"
    scp "$ServerUser@100.125.108.37:C:/RacingPoint/data/racecontrol.db-wal" "$localDb-wal" 2>$null
    scp "$ServerUser@100.125.108.37:C:/RacingPoint/data/racecontrol.db-shm" "$localDb-shm" 2>$null
    $ErrorActionPreference = "Stop"

    if (-not (Test-Path $localDb)) {
        Write-Status "error" "SCP failed - no local DB file"
        Write-Host "ERROR: SCP failed"
        exit 1
    }

    $dbSize = (Get-Item $localDb).Length
    Write-Host "  OK: $([math]::Round($dbSize / 1024))KB"

    # Step 2: Get access token
    Write-Host "[2/4] Getting Drive access token..."
    $token = Get-AccessToken
    Write-Host "  OK"

    # Step 3: Check if file already exists in folder (for update vs create)
    Write-Host "[3/4] Checking existing file..."
    $query = "name = 'racecontrol.db' and '$FolderId' in parents and trashed = false"
    $encoded = [System.Uri]::EscapeDataString($query)
    $searchUrl = "https://www.googleapis.com/drive/v3/files?q=$encoded&fields=files(id,name,size)"
    $searchHeaders = @{ Authorization = "Bearer $token" }
    $existing = Invoke-RestMethod -Uri $searchUrl -Headers $searchHeaders

    # Step 4: Upload (update existing or create new)
    Write-Host "[4/4] Uploading to Drive..."
    $dbBytes = [System.IO.File]::ReadAllBytes($localDb)

    if ($existing.files -and $existing.files.Count -gt 0) {
        # Update existing file (PATCH) — media upload supports large files
        $fileId = $existing.files[0].id
        $uploadUrl = "https://www.googleapis.com/upload/drive/v3/files/$($fileId)?uploadType=media&fields=id,name,size,modifiedTime"
        $uploadHeaders = @{
            Authorization = "Bearer $token"
            'Content-Type' = 'application/x-sqlite3'
        }
        $result = Invoke-RestMethod -Uri $uploadUrl -Method PATCH -Headers $uploadHeaders -Body $dbBytes
        Write-Host "  Updated: id=$($result.id), size=$($result.size), modified=$($result.modifiedTime)"
    } else {
        # Create new file via resumable upload (supports large files)
        $metadata = @{
            name = 'racecontrol.db'
            parents = @($FolderId)
        } | ConvertTo-Json -Compress
        $initHeaders = @{
            Authorization = "Bearer $token"
            'Content-Type' = 'application/json; charset=UTF-8'
            'X-Upload-Content-Type' = 'application/x-sqlite3'
            'X-Upload-Content-Length' = $dbBytes.Length.ToString()
        }
        $initResp = Invoke-WebRequest -Uri 'https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&fields=id,name,size,modifiedTime' -Method POST -Headers $initHeaders -Body $metadata
        $uploadUri = $initResp.Headers['Location']
        if (-not $uploadUri) {
            $uploadUri = $initResp.Headers['location']
        }
        $uploadHeaders = @{
            'Content-Type' = 'application/x-sqlite3'
            'Content-Length' = $dbBytes.Length.ToString()
        }
        $result = Invoke-RestMethod -Uri $uploadUri -Method PUT -Headers $uploadHeaders -Body $dbBytes
        Write-Host "  Created: id=$($result.id), size=$($result.size), modified=$($result.modifiedTime)"
    }

    Write-Status "ok" "Upload complete" $dbSize
    Write-Host "=== SYNC OK ==="

} catch {
    $errMsg = $_.Exception.Message
    Write-Status "error" $errMsg
    Write-Host "ERROR: $errMsg"
    exit 1
}
