#!/bin/bash
# check-frontend-env.sh — pre-build NEXT_PUBLIC_ env var audit
# Usage: bash check-frontend-env.sh <app_src_dir>
# Exits 0 if all NEXT_PUBLIC_ vars are present with LAN IPs.
# Exits 1 if any var is missing or uses localhost.
# Standing rule: run before every Next.js build (DQ-02)
set -euo pipefail

APP_SRC="${1:-}"

if [ -z "$APP_SRC" ]; then
  echo "Usage: bash check-frontend-env.sh <app_src_dir>"
  echo "  e.g. bash check-frontend-env.sh /c/Users/bono/racingpoint/racecontrol/web"
  exit 1
fi

# Derive app name from directory
APP_NAME=$(basename "$APP_SRC")

# Check that source directory exists
if [ ! -d "$APP_SRC/src" ]; then
  echo "ERROR: $APP_SRC/src/ does not exist — wrong app directory?"
  exit 1
fi

# Check that .env.production.local exists
ENV_FILE="$APP_SRC/.env.production.local"
if [ ! -f "$ENV_FILE" ]; then
  echo "ERROR: $ENV_FILE not found — create it with LAN IPs before building"
  exit 1
fi

# Scan source for all NEXT_PUBLIC_ variable references
VARS=$(grep -rn "NEXT_PUBLIC_" "$APP_SRC/src/" --include="*.ts" --include="*.tsx" 2>/dev/null \
  | grep -o "NEXT_PUBLIC_[A-Z_]*" \
  | sort -u)

if [ -z "$VARS" ]; then
  echo "WARNING: No NEXT_PUBLIC_ variables found in $APP_NAME/src/ — nothing to audit"
  exit 0
fi

VAR_COUNT=0
FAIL=0

while IFS= read -r VAR; do
  VAR_COUNT=$((VAR_COUNT + 1))

  # Check if var exists in .env.production.local
  VAR_VALUE=$(grep "^${VAR}=" "$ENV_FILE" 2>/dev/null | head -1 | sed "s/^${VAR}=//" || true)

  if [ -z "$VAR_VALUE" ]; then
    echo "ERROR: $VAR not found in $APP_NAME/.env.production.local"
    FAIL=1
    continue
  fi

  # Check for localhost or 127.0.0.1
  if echo "$VAR_VALUE" | grep -qiE "(localhost|127\.0\.0\.1)"; then
    echo "ERROR: $VAR uses localhost — must be LAN IP (192.168.31.23)"
    echo "  Current value: $VAR_VALUE"
    FAIL=1
    continue
  fi
done <<< "$VARS"

if [ "$FAIL" -eq 1 ]; then
  echo "FAIL: env var audit failed for $APP_NAME — fix .env.production.local before building"
  exit 1
fi

echo "OK: $APP_NAME env var audit passed — $VAR_COUNT NEXT_PUBLIC_ vars, all have LAN IPs"
exit 0
