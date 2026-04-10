# Test Google Drive API access — loads credentials from db-sync.env
# ASCII-only — no special characters

param(
    [string]$EnvFile = "C:/Users/bono/racingpoint/racecontrol/scripts/db-sync/db-sync.env"
)

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

$tokenBody = @{
    client_id = $env['GOOGLE_CLIENT_ID']
    client_secret = $env['GOOGLE_CLIENT_SECRET']
    refresh_token = $env['GOOGLE_REFRESH_TOKEN']
    grant_type = 'refresh_token'
}
$tokenResponse = Invoke-RestMethod -Uri 'https://oauth2.googleapis.com/token' -Method POST -Body $tokenBody
$accessToken = $tokenResponse.access_token
$headers = @{ Authorization = "Bearer $accessToken" }

Write-Host "=== Token OK ==="

$FolderId = $env['DRIVE_FOLDER_ID']

# List files in sync folder
$query = "'{0}' in parents and trashed = false" -f $FolderId
$encoded = [System.Uri]::EscapeDataString($query)
$url = "https://www.googleapis.com/drive/v3/files?q=$encoded&fields=files(id,name,mimeType,size,modifiedTime)"
$results = Invoke-RestMethod -Uri $url -Headers $headers

if ($results.files -and $results.files.Count -gt 0) {
    Write-Host "=== Files in RacingPoint-DB-Sync folder ==="
    foreach ($f in $results.files) {
        Write-Host "  $($f.name) | $($f.size) bytes | $($f.modifiedTime)"
    }
} else {
    Write-Host "=== Folder is empty ==="
}
Write-Host "=== Folder ID: $FolderId ==="
