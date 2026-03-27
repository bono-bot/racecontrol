#!/usr/bin/env bash
# Validates that all NEXT_PUBLIC_ vars referenced in source code exist in .env.production.local
set -euo pipefail
ERRORS=0
for APP in kiosk pwa web admin; do
  APP_DIR="$(dirname "$0")/../$APP"
  [ -d "$APP_DIR/src" ] || continue
  VARS=$(grep -roh "NEXT_PUBLIC_[A-Z_]*" "$APP_DIR/src/" 2>/dev/null | sort -u)
  for VAR in $VARS; do
    if ! grep -q "$VAR" "$APP_DIR/.env.production.local" 2>/dev/null; then
      echo "ERROR: $APP missing $VAR in .env.production.local"
      ERRORS=$((ERRORS + 1))
    fi
  done
done
exit $ERRORS
