#!/bin/bash
# deploy-audit.sh — Scan git diff and derive ALL deployment actions needed.
# DMP (Deploy Manifest Protocol) enforcement script.
#
# Usage: bash scripts/deploy/deploy-audit.sh [old_hash] [new_hash]
#   Defaults: old_hash = last deployed build, new_hash = HEAD
#
# Outputs a checklist of required deployment actions based on changed files.
# Exit code: 0 if all actions documented, 1 if gaps found.

set -euo pipefail

OLD_HASH="${1:-}"
NEW_HASH="${2:-HEAD}"

if [ -z "$OLD_HASH" ]; then
  echo "Usage: deploy-audit.sh <old_deployed_hash> [new_hash]"
  echo "  old_deployed_hash: the build_id currently running on production"
  echo "  new_hash: target hash to deploy (default: HEAD)"
  exit 1
fi

echo "========================================="
echo "  DEPLOY MANIFEST AUDIT"
echo "  From: $OLD_HASH → To: $NEW_HASH"
echo "========================================="
echo ""

# Get changed files
CHANGED=$(git diff --name-only "$OLD_HASH".."$NEW_HASH" 2>/dev/null)
if [ -z "$CHANGED" ]; then
  echo "No changes between $OLD_HASH and $NEW_HASH"
  exit 0
fi

# ─── Detect required actions ─────────────────────────────────────────────────

NEED_RC_AGENT=false
NEED_RACECONTROL=false
NEED_RC_SENTRY=false
NEED_KIOSK=false
NEED_WEB=false
NEED_ADMIN=false
NEED_SHARED_TYPES=false
NEED_BAT_SYNC=false
NEED_DB_MIGRATION=false
NEED_CONFIG_CHECK=false
NEED_DEPLOY_SCRIPTS=false

while IFS= read -r file; do
  case "$file" in
    crates/rc-agent/src/*) NEED_RC_AGENT=true ;;
    crates/racecontrol/src/*) NEED_RACECONTROL=true ;;
    crates/rc-sentry/src/*) NEED_RC_SENTRY=true ;;
    crates/rc-common/src/*) NEED_RC_AGENT=true; NEED_RACECONTROL=true ;;
    kiosk/*) NEED_KIOSK=true ;;
    web/*) NEED_WEB=true ;;
    apps/admin/*|racingpoint-admin/*) NEED_ADMIN=true ;;
    packages/shared-types/*) NEED_SHARED_TYPES=true ;;
    scripts/deploy/*.bat) NEED_BAT_SYNC=true ;;
    scripts/deploy/*.sh) NEED_DEPLOY_SCRIPTS=true ;;
  esac
done <<< "$CHANGED"

# Check for DB migrations in Rust code
if echo "$CHANGED" | grep -q "crates/.*\.rs$"; then
  DB_CHANGES=$(git diff "$OLD_HASH".."$NEW_HASH" -- 'crates/**/*.rs' | grep -iE "CREATE TABLE|ALTER TABLE|ADD COLUMN" | head -5)
  if [ -n "$DB_CHANGES" ]; then
    NEED_DB_MIGRATION=true
  fi
fi

# Check for new TOML config references
if echo "$CHANGED" | grep -q "crates/.*\.rs$"; then
  CONFIG_REFS=$(git diff "$OLD_HASH".."$NEW_HASH" -- 'crates/**/*.rs' | grep -E "config\.(auth|server|pod|billing|ac_server|mma|mesh)" | grep "^+" | head -5)
  if [ -n "$CONFIG_REFS" ]; then
    NEED_CONFIG_CHECK=true
  fi
fi

# If shared-types changed, all frontends that import it need rebuild
if $NEED_SHARED_TYPES; then
  echo "WARNING: packages/shared-types changed — check which frontends import it"
  # Auto-detect importers
  for app in kiosk web apps/admin; do
    if [ -f "$app/package.json" ] && grep -q "shared-types\|@racingpoint/shared" "$app/package.json" 2>/dev/null; then
      echo "  → $app imports shared-types — rebuild required"
      case "$app" in
        kiosk) NEED_KIOSK=true ;;
        web) NEED_WEB=true ;;
        apps/admin) NEED_ADMIN=true ;;
      esac
    fi
  done
fi

# ─── Output checklist ────────────────────────────────────────────────────────

echo "DEPLOYMENT ACTIONS REQUIRED:"
echo ""

TOTAL=0
print_action() {
  local status="$1"
  local target="$2"
  local action="$3"
  local cmd="$4"
  TOTAL=$((TOTAL + 1))
  echo "  [$status] $target: $action"
  [ -n "$cmd" ] && echo "        → $cmd"
}

if $NEED_RC_AGENT; then
  print_action "TODO" "Pods 1-8 + POS" "Rebuild + deploy rc-agent binary" "cargo build --release --bin rc-agent && bash scripts/deploy/deploy-all-pods.sh"
fi

if $NEED_RACECONTROL; then
  print_action "TODO" "Server .23" "Rebuild + deploy racecontrol binary" "cargo build --release --bin racecontrol && bash deploy-staging/deploy-server.sh"
  print_action "TODO" "Cloud (Bono VPS)" "Deploy racecontrol binary to cloud" "comms-link relay: git_pull + rebuild"
fi

if $NEED_RC_SENTRY; then
  print_action "TODO" "All pods" "Rebuild + deploy rc-sentry binary" "cargo build --release --bin rc-sentry"
fi

if $NEED_KIOSK; then
  print_action "TODO" "Server .23:3300" "Rebuild kiosk app" "cd kiosk && npm run build && tar + scp + restart"
  print_action "TODO" "Cloud VPS:3300" "Rebuild kiosk on cloud" "comms-link relay: rebuild kiosk"
fi

if $NEED_WEB; then
  print_action "TODO" "Server .23:3200" "Rebuild web app" "cd web && npm run build && tar + scp + restart"
  print_action "TODO" "Cloud VPS:3200" "Rebuild web on cloud" "comms-link relay: rebuild web"
fi

if $NEED_ADMIN; then
  print_action "TODO" "Server .23:3201" "Rebuild admin app" "cd admin && npm run build && tar + scp + restart"
  print_action "TODO" "Cloud VPS:3201" "Rebuild admin on cloud" "comms-link relay: rebuild admin"
fi

if $NEED_BAT_SYNC; then
  print_action "TODO" "Pods 1-8 + POS" "Sync bat files to all pods" "scp scripts/deploy/start-rcagent.bat to each pod"
fi

if $NEED_DB_MIGRATION; then
  echo ""
  echo "  ⚠ DATABASE CHANGES DETECTED:"
  echo "$DB_CHANGES" | while read -r line; do echo "    $line"; done
  print_action "TODO" "Server .23 + Cloud" "Verify DB migration applied" "Check tables/columns exist on both DBs"
fi

if $NEED_CONFIG_CHECK; then
  echo ""
  echo "  ⚠ CONFIG REFERENCES DETECTED:"
  echo "$CONFIG_REFS" | while read -r line; do echo "    $line"; done
  print_action "TODO" "Server .23" "Verify racecontrol.toml has new config sections" "Check toml on server"
fi

# Always-required actions
echo ""
echo "ALWAYS VERIFY:"
print_action "TODO" "Server .23" "Clear stale MAINTENANCE_MODE if present" "del C:\\RacingPoint\\MAINTENANCE_MODE"
print_action "TODO" "All targets" "build_id matches expected on every target" "curl health endpoints"
print_action "TODO" "Cloud" "Cloud parity — same binary + frontend versions" "Compare build_ids"

echo ""
echo "========================================="
echo "  TOTAL ACTIONS: $TOTAL"
echo "========================================="
echo ""
echo "Run this BEFORE marking any phase/milestone as shipped."
echo "Every [TODO] must become [DONE] with evidence."
