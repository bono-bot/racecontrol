#!/usr/bin/env bash
# v2-deploy/halo-probe-deploy-source-attribution.sh — F9 v1 HALO probe (PACT-20260503-004)
#
# Verifies that running surface deploys trace to F9 invocation via SWAPLOG.md attribution.
#
# Usage: bash v2-deploy/halo-probe-deploy-source-attribution.sh [--surface=NAME] [--strict]
# Flags:
#   --surface=NAME   probe a single surface (default: all enumerated surfaces)
#   --strict         exit non-zero on ATTRIBUTION-GAP (default: advisory; non-zero only on VIOLATION)
#
# Exit codes (non-strict mode = F9 v1 default):
#   0 = all PASS or all whitelisted; no manual-deploy violations
#   2 = CONSTRAINT-019 VIOLATION detected (caller=other in SWAPLOG)
#
# Exit codes (strict mode = F9 Phase 3 fail-CLOSED):
#   0 = all PASS
#   1 = ATTRIBUTION-GAP (deploy predates F9 OR not in whitelist)
#   2 = CONSTRAINT-019 VIOLATION
#
# Composes-with: v2-deploy/SWAPLOG.md (audit-trail) + v2-deploy/.attribution-whitelist (pre-F9 deploys)
# Spec: `.planning/specs/v2-step-2.5-f9-atomic-deploy.md` Phase 3 fail-CLOSED gating

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SWAPLOG="${SCRIPT_DIR}/SWAPLOG.md"
WHITELIST="${SCRIPT_DIR}/.attribution-whitelist"

ALL_SURFACES=("racecontrol" "rc-agent" "web" "kiosk" "admin" "comms-link" "pwa")
TARGET_SURFACE=""
STRICT=0

while [ $# -gt 0 ]; do
  case "$1" in
    --surface=*)  TARGET_SURFACE="${1#--surface=}";;
    --strict)     STRICT=1;;
    --help|-h)
      sed -n '1,18p' "$0"
      exit 0
      ;;
    *)
      echo "ERROR: unknown arg $1" >&2
      exit 1
      ;;
  esac
  shift
done

PASS_COUNT=0
GAP_COUNT=0
VIOLATION_COUNT=0
PROBED_COUNT=0

declare -A FINDINGS

probe_surface() {
  local surface="$1"
  local deployed_sha=""

  # Best-effort source-of-truth for currently-running surface SHA.
  # F9 v1: best-effort enumeration; full impl gates on per-surface health endpoint exposing build_id.
  case "$surface" in
    racecontrol)
      # Probe local racecontrol /api/v1/health for build_id (if running)
      deployed_sha=$(curl -sm 2 http://localhost:8080/api/v1/health 2>/dev/null | grep -oE '"build_id"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/' || echo "")
      [ -z "$deployed_sha" ] && deployed_sha=$(curl -sm 2 http://localhost:3500/api/v1/health 2>/dev/null | grep -oE '"build_id"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/' || echo "")
      ;;
    rc-agent)
      # Probe pm2 / rc-sentry for fleet attribution
      deployed_sha=$(curl -sm 2 http://localhost:8091/health 2>/dev/null | grep -oE '"build_id"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/' || echo "")
      ;;
    web|kiosk|admin)
      local port
      case "$surface" in
        web)   port=3200;;
        kiosk) port=3300;;
        admin) port=3201;;
      esac
      deployed_sha=$(curl -sm 2 "http://localhost:${port}/api/build_id" 2>/dev/null || echo "")
      ;;
    comms-link)
      deployed_sha=$(curl -sm 2 "http://localhost:8766/health" 2>/dev/null | grep -oE '"build_id"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/' || echo "")
      ;;
    pwa)
      deployed_sha=$(curl -sm 4 "https://app.racingpoint.cloud/api/build_id" 2>/dev/null || echo "")
      ;;
  esac

  PROBED_COUNT=$((PROBED_COUNT + 1))

  if [ -z "$deployed_sha" ]; then
    FINDINGS[$surface]="UNREACHABLE — no health endpoint response (surface offline OR build_id not exposed yet)"
    return 0
  fi

  # Truncate to first 40 chars (handle trailing whitespace / version-suffix variants)
  deployed_sha="${deployed_sha:0:40}"

  if [ ! -f "$SWAPLOG" ]; then
    FINDINGS[$surface]="ATTRIBUTION-GAP — SWAPLOG.md not yet authored (deploy predates F9 v1 ship; whitelist if intentional)"
    GAP_COUNT=$((GAP_COUNT + 1))
    return 0
  fi

  # Search SWAPLOG for matching surface + sha
  local match
  match=$(awk -v s="$surface" -v sha="$deployed_sha" -F'|' '$4 ~ sha && $3 ~ s {print $0}' "$SWAPLOG" | tail -1)

  if [ -z "$match" ]; then
    # Check whitelist
    if [ -f "$WHITELIST" ] && grep -qE "^${surface}[[:space:]]+${deployed_sha}" "$WHITELIST"; then
      FINDINGS[$surface]="WHITELISTED — sha=${deployed_sha:0:12} predates F9 ship per .attribution-whitelist"
      PASS_COUNT=$((PASS_COUNT + 1))
      return 0
    fi
    FINDINGS[$surface]="ATTRIBUTION-GAP — sha=${deployed_sha:0:12} not in SWAPLOG and not whitelisted"
    GAP_COUNT=$((GAP_COUNT + 1))
    return 0
  fi

  # Extract caller field (column 7 in the | row | format from deploy.sh actually field 8 0-indexed, but awk is 1-indexed)
  # Row: "| ts_utc | ts_ist | surface | git_sha | author | manifest_path | exit_code | duration_s | notes |"
  # Field 1 = empty (leading |), then 2..10
  local caller
  # Caller is encoded in manifest_path basename or in notes field; in v1 we infer from manifest_path containing "v2-deploy/manifests/"
  if echo "$match" | grep -qE 'v2-deploy/manifests/'; then
    caller="f9-deploy-sh"
  elif echo "$match" | grep -qE 'github/workflows'; then
    caller="f9-deploy-yml"
  else
    caller="other"
  fi

  if [ "$caller" = "other" ]; then
    FINDINGS[$surface]="VIOLATION — sha=${deployed_sha:0:12} in SWAPLOG with caller=other (manual deploy detected; CONSTRAINT-019 violation)"
    VIOLATION_COUNT=$((VIOLATION_COUNT + 1))
    return 0
  fi

  FINDINGS[$surface]="PASS — sha=${deployed_sha:0:12} attributed to ${caller}"
  PASS_COUNT=$((PASS_COUNT + 1))
  return 0
}

# ---------- main ----------

if [ -n "$TARGET_SURFACE" ]; then
  probe_surface "$TARGET_SURFACE"
else
  for s in "${ALL_SURFACES[@]}"; do
    probe_surface "$s"
  done
fi

echo ""
echo "================================================================================"
echo "[HALO-PROBE] deploy-source-attribution — F9 v1 (advisory unless --strict)"
echo "[HALO-PROBE] probed=${PROBED_COUNT}  pass=${PASS_COUNT}  gaps=${GAP_COUNT}  violations=${VIOLATION_COUNT}"
echo "================================================================================"
for s in "${!FINDINGS[@]}"; do
  printf "  [%-12s] %s\n" "$s" "${FINDINGS[$s]}"
done
echo ""

# Exit logic
if [ "$VIOLATION_COUNT" -gt 0 ]; then
  echo "[HALO-PROBE] EXIT 2: CONSTRAINT-019 violation(s) detected" >&2
  exit 2
fi

if [ "$STRICT" -eq 1 ] && [ "$GAP_COUNT" -gt 0 ]; then
  echo "[HALO-PROBE] EXIT 1: ATTRIBUTION-GAP outside whitelist (--strict mode)" >&2
  exit 1
fi

echo "[HALO-PROBE] EXIT 0: no blocking findings (gaps are advisory in v1)"
exit 0
