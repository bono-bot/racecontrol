#!/bin/bash
# =============================================================================
# deploy-nextjs.sh — Universal Next.js deploy for Racing Point
# Usage: bash deploy-nextjs.sh <app> [server]
#   app:    admin | kiosk | web
#   server: target SSH host (default: ADMIN@192.168.31.23)
#
# Pipeline:
#   [1/8] Build locally (npm run build)
#   [2/8] Verify build output (server.js + .next/static + page count)
#   [3/8] Package (standalone + static → zip)
#   [4/8] Pre-deploy health snapshot (capture current pages_before)
#   [5/8] Upload & Deploy (SCP + SSH: stop → backup → extract → start)
#   [6/8] Health check (refuse degraded deploys)
#   [7/8] Log deploy result to racecontrol
#   [8/8] Rollback if health check failed
#
# Standing rule: NEVER deploy without this script. Manual deploys cause
# stale builds with missing pages (happened 3+ times across all apps).
# =============================================================================
set -euo pipefail

START_EPOCH=$(date +%s)
APP="${1:-}"
SERVER="${2:-ADMIN@192.168.31.23}"
STAGING="C:/Users/bono/racingpoint/deploy-staging"
RC_URL="http://192.168.31.23:8080"
DEPLOY_LOG_URL="$RC_URL/api/v1/deploy-log"
DEPLOY_RESULT="unknown"
PAGES_BEFORE=0
PAGES_AFTER=0
MISSING_PAGES=""
ERROR=""

# --- App config ---
case "$APP" in
  admin)
    SRC="C:/Users/bono/racingpoint/racingpoint-admin"
    REMOTE_DIR='C:\RacingPoint\admin'
    PORT=3200
    ZIP_NAME="admin-deploy.zip"
    SERVICE_NAME="racingpoint-admin"
    ;;
  kiosk)
    SRC="C:/Users/bono/racingpoint/racecontrol/kiosk"
    REMOTE_DIR='C:\RacingPoint\kiosk'
    PORT=3300
    ZIP_NAME="kiosk-deploy.zip"
    SERVICE_NAME="racecontrol-kiosk"
    ;;
  web)
    SRC="C:/Users/bono/racingpoint/racecontrol/web"
    REMOTE_DIR='C:\RacingPoint\web'
    PORT=3200
    ZIP_NAME="web-deploy.zip"
    SERVICE_NAME="racecontrol-web"
    ;;
  *)
    echo "Usage: bash deploy-nextjs.sh <admin|kiosk|web> [server]"
    exit 1
    ;;
esac

echo "=== Deploy $APP to $SERVER ==="
echo "  Source:  $SRC"
echo "  Remote:  $REMOTE_DIR"
echo "  Port:    $PORT"
echo "  Service: $SERVICE_NAME"
echo ""

# --- Helper: log deploy result to racecontrol (best-effort) ---
log_deploy() {
  local RESULT="$1"
  local LOG_ERROR="${2:-}"
  local END_EPOCH
  END_EPOCH=$(date +%s)
  local DURATION=$(( END_EPOCH - START_EPOCH ))
  local BUILD_HASH
  BUILD_HASH=$(git -C "$SRC" rev-parse --short HEAD 2>/dev/null || echo "unknown")

  local TMPFILE
  TMPFILE=$(mktemp /tmp/deploy-log-XXXXXX.json)

  # Write JSON to file (standing rule: no inline JSON in Git Bash)
  cat > "$TMPFILE" <<ENDJSON
{
  "app": "$APP",
  "result": "$RESULT",
  "deployer": "james",
  "pages_before": $PAGES_BEFORE,
  "pages_after": $PAGES_AFTER,
  "pages_missing": "$MISSING_PAGES",
  "duration_secs": $DURATION,
  "error": "$LOG_ERROR",
  "build_hash": "$BUILD_HASH"
}
ENDJSON

  echo "  Logging deploy: result=$RESULT duration=${DURATION}s hash=$BUILD_HASH"
  curl -s -X POST "$DEPLOY_LOG_URL" \
    -H "Content-Type: application/json" \
    -d @"$TMPFILE" > /dev/null 2>&1 || echo "  WARNING: Could not log to racecontrol (non-fatal)"

  rm -f "$TMPFILE"
}

# --- Helper: start node on server ---
start_node_on_server() {
  ssh -o StrictHostKeyChecking=no "$SERVER" "powershell -Command \"
    \\\$env:PORT = '$PORT'
    \\\$env:HOSTNAME = '0.0.0.0'
    \\\$env:RC_URL = '$RC_URL'
    \\\$env:RC_JWT_SECRET = 'UKLvoxSUMRPsKckeN17vJ-ORNgkTpfVO2MvS_JA5TMo'
    Start-Process -FilePath 'C:\\Program Files\\nodejs\\node.exe' \`
      -ArgumentList 'server.js' \`
      -WorkingDirectory '$REMOTE_DIR' \`
      -WindowStyle Hidden \`
      -RedirectStandardOutput 'C:\\RacingPoint\\$APP-stdout.log' \`
      -RedirectStandardError 'C:\\RacingPoint\\$APP-stderr.log'
    Write-Output 'Node started on port $PORT'
  \""
}

# --- Helper: stop node on target port ---
stop_node_on_port() {
  ssh -o StrictHostKeyChecking=no "$SERVER" "powershell -Command \"
    Get-Process node -ErrorAction SilentlyContinue | Where-Object {
      try { (Get-NetTCPConnection -OwningProcess \\\$_.Id -ErrorAction SilentlyContinue).LocalPort -contains $PORT } catch { \\\$false }
    } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep 2
    Write-Output 'Stopped node on port $PORT'
  \""
}

# --- Helper: check health endpoint, sets HEALTH_RESPONSE ---
HEALTH_RESPONSE=""
check_health() {
  # Each app has a basePath — health endpoint must include it
  local BASE_PATH=""
  case "$APP" in
    kiosk) BASE_PATH="/kiosk" ;;
    admin) BASE_PATH="/admin" ;;
    web)   BASE_PATH="" ;;
  esac
  HEALTH_RESPONSE=$(curl -s --max-time 10 "http://192.168.31.23:$PORT${BASE_PATH}/api/health" 2>/dev/null || echo "")
}

# =========================================================================
# [1/8] Build
# =========================================================================
echo "[1/8] Building $APP..."
cd "$SRC"
npm run build
echo "  Build complete."

# =========================================================================
# [2/8] Verify build output
# =========================================================================
echo "[2/8] Verifying build..."
STANDALONE="$SRC/.next/standalone"
STATIC="$SRC/.next/static"

if [ ! -f "$STANDALONE/server.js" ]; then
  echo "FATAL: $STANDALONE/server.js not found. Build failed?"
  exit 2
fi
if [ ! -d "$STATIC" ]; then
  echo "FATAL: $STATIC/ not found. Build incomplete?"
  exit 2
fi

PAGE_COUNT=$(find "$SRC/.next/server/app" -name "*.html" | wc -l)
echo "  server.js: OK"
echo "  .next/static: OK"
echo "  Pages built: $PAGE_COUNT"

if [ "$PAGE_COUNT" -lt 5 ]; then
  echo "FATAL: Only $PAGE_COUNT pages built. Expected 10+. Build may be broken."
  exit 2
fi

# =========================================================================
# [3/8] Package
# =========================================================================
echo "[3/8] Packaging..."

# Copy static into standalone (Next.js standalone requirement)
rm -rf "$STANDALONE/.next/static"
mkdir -p "$STANDALONE/.next"
cp -r "$STATIC" "$STANDALONE/.next/static"

# Copy public/ if it exists
if [ -d "$SRC/public" ]; then
  cp -r "$SRC/public" "$STANDALONE/public"
fi

# Create zip
ZIP_PATH="$STAGING/$ZIP_NAME"
rm -f "$ZIP_PATH"
powershell -Command "Compress-Archive -Path '$STANDALONE/*' -DestinationPath '$ZIP_PATH' -Force"

ZIP_SIZE=$(du -sh "$ZIP_PATH" | cut -f1)
echo "  Package: $ZIP_NAME ($ZIP_SIZE)"

# =========================================================================
# [4/8] Pre-deploy health snapshot (curl /api/health for pages_before)
# =========================================================================
echo "[4/8] Pre-deploy health snapshot..."
check_health

if [ -n "$HEALTH_RESPONSE" ]; then
  # Extract pages_available using grep/sed (no jq dependency)
  PAGES_BEFORE=$(echo "$HEALTH_RESPONSE" | grep -o '"pages_available":[0-9]*' | grep -o '[0-9]*' || echo "0")
  if [ -z "$PAGES_BEFORE" ]; then
    PAGES_BEFORE=0
  fi
  echo "  Current app responding: pages_available=$PAGES_BEFORE"
else
  PAGES_BEFORE=0
  echo "  App not currently running (no health response)"
fi

# =========================================================================
# [5/8] Upload & Deploy (with backup)
# =========================================================================
echo "[5/8] Uploading and deploying..."

# Upload via SCP
echo "  SCP upload..."
scp -o StrictHostKeyChecking=no "$ZIP_PATH" "$SERVER:C:/RacingPoint/$ZIP_NAME"

# Stop existing node process
echo "  Stopping existing node on port $PORT..."
stop_node_on_port

# Create backup of current app, extract new, start
echo "  Backup + Extract + Start on server..."
ssh -o StrictHostKeyChecking=no "$SERVER" "powershell -Command \"
  \\\$ErrorActionPreference = 'Stop'

  # Backup current app directory if it exists
  if (Test-Path '$REMOTE_DIR') {
    Write-Output 'Creating backup...'
    if (Test-Path 'C:\\RacingPoint\\$APP-backup.zip') {
      Remove-Item 'C:\\RacingPoint\\$APP-backup.zip' -Force
    }
    Compress-Archive -Path '$REMOTE_DIR\\*' -DestinationPath 'C:\\RacingPoint\\$APP-backup.zip' -Force
    Write-Output 'Backup created: $APP-backup.zip'
  }

  # Remove old app directory
  if (Test-Path '$REMOTE_DIR') {
    Remove-Item -Recurse -Force '$REMOTE_DIR'
  }

  # Extract new
  Write-Output 'Extracting new deploy...'
  Expand-Archive -Path 'C:\\RacingPoint\\$ZIP_NAME' -DestinationPath '$REMOTE_DIR' -Force
  Remove-Item 'C:\\RacingPoint\\$ZIP_NAME'

  # Verify files exist
  if (-not (Test-Path '$REMOTE_DIR\\server.js')) { throw 'server.js MISSING after extract' }
  if (-not (Test-Path '$REMOTE_DIR\\.next\\static')) { throw '.next/static MISSING after extract' }
  Write-Output 'Extract verified: server.js + .next/static present'
\""

# Start node
echo "  Starting node..."
start_node_on_server

echo "  Waiting for app to start..."
sleep 5

# =========================================================================
# [6/8] Health check — refuse degraded deploys (curl /api/health gate)
# =========================================================================
echo "[6/8] Health verification..."

HEALTH_OK=false
for ATTEMPT in 1 2 3; do
  echo "  Health check attempt $ATTEMPT/3..."
  check_health

  if [ -z "$HEALTH_RESPONSE" ]; then
    echo "  No response, waiting 5s..."
    sleep 5
    continue
  fi

  echo "  Response: $HEALTH_RESPONSE"

  # Check for success: "status":"ok" AND "healthy":true
  if echo "$HEALTH_RESPONSE" | grep -q '"status":"ok"' && echo "$HEALTH_RESPONSE" | grep -q '"healthy":true'; then
    HEALTH_OK=true
    DEPLOY_RESULT="success"
    break
  fi

  # If we got a response but it's degraded, no need to retry
  if echo "$HEALTH_RESPONSE" | grep -q '"degraded"'; then
    echo "  Status: DEGRADED — deploy refused"
    break
  fi

  sleep 5
done

# Extract post-deploy metrics
if [ -n "$HEALTH_RESPONSE" ]; then
  PAGES_AFTER=$(echo "$HEALTH_RESPONSE" | grep -o '"pages_available":[0-9]*' | grep -o '[0-9]*' || echo "0")
  if [ -z "$PAGES_AFTER" ]; then
    PAGES_AFTER=0
  fi
  MISSING_PAGES=$(echo "$HEALTH_RESPONSE" | grep -o '"pages_missing":\[[^]]*\]' || echo "")
fi

if [ "$HEALTH_OK" = true ]; then
  DEPLOY_RESULT="success"
  ERROR=""
  echo ""
  echo "  HEALTH CHECK PASSED: $SERVICE_NAME on :$PORT — all pages verified"
else
  DEPLOY_RESULT="failed"
  if [ -z "$HEALTH_RESPONSE" ]; then
    ERROR="health endpoint unreachable after 3 attempts"
  else
    ERROR="health check returned degraded or unhealthy"
  fi
  echo ""
  echo "  HEALTH CHECK FAILED: $ERROR"
fi

# =========================================================================
# [7/8] Log deploy result to racecontrol
# =========================================================================
echo "[7/8] Logging deploy result..."
log_deploy "$DEPLOY_RESULT" "$ERROR"

# =========================================================================
# [8/8] Rollback if health check failed
# =========================================================================
if [ "$DEPLOY_RESULT" = "failed" ]; then
  echo "[8/8] ROLLING BACK to previous version..."

  # Check if backup exists
  BACKUP_EXISTS=$(ssh -o StrictHostKeyChecking=no "$SERVER" "powershell -Command \"Test-Path 'C:\\RacingPoint\\$APP-backup.zip'\"" 2>/dev/null || echo "False")

  if echo "$BACKUP_EXISTS" | grep -qi "true"; then
    # Stop the broken node
    echo "  Stopping broken node..."
    stop_node_on_port

    # Restore from backup
    echo "  Restoring from backup..."
    ssh -o StrictHostKeyChecking=no "$SERVER" "powershell -Command \"
      \\\$ErrorActionPreference = 'Stop'
      if (Test-Path '$REMOTE_DIR') { Remove-Item -Recurse -Force '$REMOTE_DIR' }
      Expand-Archive -Path 'C:\\RacingPoint\\$APP-backup.zip' -DestinationPath '$REMOTE_DIR' -Force
      Write-Output 'Backup restored'
    \""

    # Restart node with restored version
    echo "  Starting restored version..."
    start_node_on_server

    echo "  Waiting for restored app to start..."
    sleep 5

    # Verify rollback health
    check_health
    if [ -n "$HEALTH_RESPONSE" ]; then
      echo "  Rollback health: $HEALTH_RESPONSE"
    else
      echo "  WARNING: Restored version not responding yet"
    fi

    # Log rollback
    log_deploy "rollback" "auto-rollback after failed deploy"

    echo ""
    echo "=== ROLLBACK COMPLETE === Previous version of $APP restored on :$PORT"
  else
    echo "  WARNING: No backup found ($APP-backup.zip). Cannot rollback."
    echo ""
    echo "=== DEPLOY FAILED === $APP on :$PORT — no backup to rollback to"
  fi

  exit 1
else
  echo "[8/8] No rollback needed."
  echo ""
  echo "=== DEPLOY SUCCESS === $APP ($SERVICE_NAME) on :$PORT — all pages verified"
  exit 0
fi
