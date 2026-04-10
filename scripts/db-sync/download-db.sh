#!/usr/bin/env bash
# download-db.sh — Download racecontrol.db from Google Drive to cloud server
# Runs on Bono VPS every 5 minutes via cron
#
# Flow:
#   1. Get OAuth access token
#   2. Find racecontrol.db in sync folder
#   3. Check if remote is newer than local
#   4. Download and replace local DB
#   5. Write sync-status.json
#
# Credentials: loaded from db-sync.env (NEVER hardcode in this file)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ENV_FILE="${ENV_FILE:-$SCRIPT_DIR/db-sync.env}"

# Load credentials from env file
if [ ! -f "$ENV_FILE" ]; then
    echo "ERROR: Credentials file not found: $ENV_FILE"
    exit 1
fi
# shellcheck disable=SC1090
set -a
source "$ENV_FILE"
set +a

FOLDER_ID="$DRIVE_FOLDER_ID"
CLIENT_ID="$GOOGLE_CLIENT_ID"
CLIENT_SECRET="$GOOGLE_CLIENT_SECRET"
REFRESH_TOKEN="$GOOGLE_REFRESH_TOKEN"

# Paths (Bono VPS layout)
RC_DATA_DIR="${RC_DATA_DIR:-/root/racingpoint/racecontrol/data}"
DB_PATH="$RC_DATA_DIR/racecontrol.db"
STATUS_FILE="$RC_DATA_DIR/sync-status.json"
TEMP_DB="$RC_DATA_DIR/racecontrol-incoming.db"

write_status() {
    local status="$1" message="$2" size="${3:-0}"
    local ts
    ts=$(date -u '+%Y-%m-%dT%H:%M:%S')
    cat > "$STATUS_FILE" <<EOF
{"status":"$status","message":"$message","timestamp_utc":"$ts","db_size_bytes":$size}
EOF
}

# Step 1: Get access token
echo "[1/5] Getting access token..."
TOKEN=$(curl -s -X POST 'https://oauth2.googleapis.com/token' \
    -d "client_id=$CLIENT_ID" \
    -d "client_secret=$CLIENT_SECRET" \
    -d "refresh_token=$REFRESH_TOKEN" \
    -d "grant_type=refresh_token" | jq -r '.access_token')

if [ -z "$TOKEN" ] || [ "$TOKEN" = "null" ]; then
    write_status "error" "Failed to get access token"
    echo "ERROR: Token failed"
    exit 1
fi
echo "  OK"

# Step 2: Find the file in sync folder
echo "[2/5] Finding racecontrol.db in Drive..."
QUERY="name = 'racecontrol.db' and '${FOLDER_ID}' in parents and trashed = false"
ENCODED_QUERY=$(python3 -c "import urllib.parse; print(urllib.parse.quote('''$QUERY'''))")
FILE_INFO=$(curl -s -H "Authorization: Bearer $TOKEN" \
    "https://www.googleapis.com/drive/v3/files?q=${ENCODED_QUERY}&fields=files(id,name,size,modifiedTime)")

FILE_ID=$(echo "$FILE_INFO" | jq -r '.files[0].id // empty')
REMOTE_SIZE=$(echo "$FILE_INFO" | jq -r '.files[0].size // "0"')
REMOTE_MODIFIED=$(echo "$FILE_INFO" | jq -r '.files[0].modifiedTime // empty')

if [ -z "$FILE_ID" ]; then
    write_status "error" "racecontrol.db not found in Drive folder"
    echo "ERROR: File not found"
    exit 1
fi
echo "  Found: id=$FILE_ID, size=$REMOTE_SIZE, modified=$REMOTE_MODIFIED"

# Step 3: Check if we need to download (compare size + modification time)
echo "[3/5] Checking if download needed..."
NEED_DOWNLOAD=true
if [ -f "$DB_PATH" ]; then
    LOCAL_SIZE=$(stat -c%s "$DB_PATH" 2>/dev/null || echo "0")
    if [ "$LOCAL_SIZE" = "$REMOTE_SIZE" ]; then
        # Same size — check if remote modified time is newer than our status
        if [ -f "$STATUS_FILE" ]; then
            LAST_SYNC=$(jq -r '.timestamp_utc // ""' "$STATUS_FILE" 2>/dev/null || echo "")
            if [ -n "$LAST_SYNC" ]; then
                LAST_EPOCH=$(date -d "$LAST_SYNC" '+%s' 2>/dev/null || echo "0")
                REMOTE_EPOCH=$(date -d "$REMOTE_MODIFIED" '+%s' 2>/dev/null || echo "999999999")
                if [ "$REMOTE_EPOCH" -le "$LAST_EPOCH" ]; then
                    NEED_DOWNLOAD=false
                    echo "  Skipping: remote not newer than last sync"
                fi
            fi
        fi
    else
        echo "  Size differs: local=$LOCAL_SIZE, remote=$REMOTE_SIZE"
    fi
else
    echo "  No local DB, downloading"
fi

if [ "$NEED_DOWNLOAD" = false ]; then
    write_status "ok" "No update needed" "$REMOTE_SIZE"
    echo "=== SYNC OK (no change) ==="
    exit 0
fi

# Step 4: Download
echo "[4/5] Downloading ($((REMOTE_SIZE / 1024))KB)..."
HTTP_CODE=$(curl -s -w "%{http_code}" -o "$TEMP_DB" \
    -H "Authorization: Bearer $TOKEN" \
    "https://www.googleapis.com/drive/v3/files/${FILE_ID}?alt=media")

if [ "$HTTP_CODE" != "200" ]; then
    rm -f "$TEMP_DB"
    write_status "error" "Download failed: HTTP $HTTP_CODE"
    echo "ERROR: Download HTTP $HTTP_CODE"
    exit 1
fi

DL_SIZE=$(stat -c%s "$TEMP_DB" 2>/dev/null || echo "0")
if [ "$DL_SIZE" -lt 1024 ]; then
    rm -f "$TEMP_DB"
    write_status "error" "Downloaded file too small: ${DL_SIZE} bytes"
    echo "ERROR: File too small ($DL_SIZE bytes)"
    exit 1
fi
echo "  Downloaded: ${DL_SIZE} bytes"

# Step 5: Atomic swap
echo "[5/5] Replacing local DB..."
# Keep one backup
if [ -f "$DB_PATH" ]; then
    cp "$DB_PATH" "${DB_PATH}.prev" 2>/dev/null || true
fi
mv "$TEMP_DB" "$DB_PATH"

# Remove stale WAL/SHM (cloud DB runs fresh, not in WAL mode from venue)
rm -f "${DB_PATH}-wal" "${DB_PATH}-shm"

write_status "ok" "Download complete" "$DL_SIZE"
echo "=== SYNC OK ==="
