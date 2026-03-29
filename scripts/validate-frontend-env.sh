#!/usr/bin/env bash
# validate-frontend-env.sh — NEXT_PUBLIC_ build validation (GAP-9)
#
# Scans all Next.js apps for NEXT_PUBLIC_ references and verifies
# each variable has a value in the corresponding .env.production.local.
#
# Designed to be called as a build pre-step:
#   bash scripts/validate-frontend-env.sh || exit 1
#
# Exit 0 = all vars have values, exit 1 = missing vars found

set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
RESET='\033[0m'

APPS=("kiosk" "web" "apps/racingpoint-admin" "pwa")
TOTAL_MISSING=0

echo "============================================================"
echo "NEXT_PUBLIC_ Environment Validation (GAP-9)"
echo "============================================================"
echo ""

for app in "${APPS[@]}"; do
  APP_DIR="${REPO_ROOT}/${app}"
  if [ ! -d "$APP_DIR" ]; then
    echo -e "${YELLOW}SKIP: ${app} — directory not found${RESET}"
    continue
  fi

  echo "--- ${app} ---"

  # Find all NEXT_PUBLIC_ references in source
  VARS=$(grep -roh 'NEXT_PUBLIC_[A-Z_]*' "${APP_DIR}/src/" "${APP_DIR}/app/" "${APP_DIR}/pages/" 2>/dev/null | sort -u || true)

  if [ -z "$VARS" ]; then
    echo "  No NEXT_PUBLIC_ vars referenced"
    continue
  fi

  # Check each var in .env.production.local
  ENV_FILE="${APP_DIR}/.env.production.local"
  if [ ! -f "$ENV_FILE" ]; then
    # Try parent .env.production.local
    ENV_FILE="${REPO_ROOT}/.env.production.local"
  fi

  MISSING=0
  for var in $VARS; do
    if [ -f "$ENV_FILE" ] && grep -q "^${var}=" "$ENV_FILE"; then
      # Check if value is non-empty
      VALUE=$(grep "^${var}=" "$ENV_FILE" | head -1 | cut -d= -f2-)
      if [ -z "$VALUE" ]; then
        echo -e "  ${RED}EMPTY: ${var} (defined but no value)${RESET}"
        ((MISSING++))
      fi
    else
      echo -e "  ${RED}MISSING: ${var}${RESET}"
      ((MISSING++))
    fi
  done

  VAR_COUNT=$(echo "$VARS" | wc -w)
  PRESENT=$((VAR_COUNT - MISSING))

  if [ $MISSING -eq 0 ]; then
    echo -e "  ${GREEN}ALL ${VAR_COUNT} vars have values${RESET}"
  else
    echo -e "  ${PRESENT}/${VAR_COUNT} present, ${RED}${MISSING} MISSING${RESET}"
  fi
  TOTAL_MISSING=$((TOTAL_MISSING + MISSING))
  echo ""
done

echo "============================================================"
if [ $TOTAL_MISSING -gt 0 ]; then
  echo -e "${RED}BLOCKED: ${TOTAL_MISSING} missing NEXT_PUBLIC_ variable(s)${RESET}"
  echo "Fix: Add missing vars to .env.production.local before building"
  exit 1
fi

echo -e "${GREEN}ALL NEXT_PUBLIC_ variables validated${RESET}"
exit 0
