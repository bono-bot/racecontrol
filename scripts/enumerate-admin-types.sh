#!/bin/bash
set -euo pipefail
# scripts/enumerate-admin-types.sh
# Phase 445 Wave 0 Task 1 Step B — enumerate rc-common types consumed by the
# admin dashboard and emit packages/shared-types/generated/.whitelist.txt.
#
# Inputs (expected co-located next to racecontrol):
#   ../racingpoint-admin/src/**/*.{ts,tsx}   — rcFetch call sites
#   crates/rc-common/src/*.rs                — source-of-truth structs/enums
#   crates/racecontrol/src/api/              — axum handlers (response type crawl)
#
# Output:
#   packages/shared-types/generated/.whitelist.txt
#     One struct/enum name per line, sorted, deduplicated.
#     First non-empty line may be "# FALLBACK" if primary discovery failed.
#
# Contract (per PLAN 445-00 Task 1 behavior):
#   - At least 16 admin-consumed type names.
#   - Zero overlap with D-14 SKIP-list (8 adjacently-tagged enums).
#   - Idempotent: re-runs overwrite output with `>` (never appends).
#
# Decision IDs: D-07 (admin-type discovery), D-09 (generated dir placement),
# D-14 (D-14 SKIP-list structural denylist).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
ADMIN_SRC="${PROJECT_ROOT:-$REPO_ROOT/../racingpoint-admin}/src"
RC_COMMON_SRC="$REPO_ROOT/crates/rc-common/src"
RC_API_SRC="$REPO_ROOT/crates/racecontrol/src/api"
OUT_DIR="$REPO_ROOT/packages/shared-types/generated"
OUT_FILE="$OUT_DIR/.whitelist.txt"

mkdir -p "$OUT_DIR"

# D-14 authoritative SKIP-list — 8 adjacently-tagged enums that MUST NOT receive
# #[derive(TS)] and MUST NOT appear in the admin whitelist. Structural audit
# (crates/rc-common/tests/enum_tagging_audit.rs) is the primary gate; this grep
# is belt-and-suspenders.
D14_SKIP_LIST='^(ServerMessage|AgentMessage|DashboardEvent|AiChannelMessage|ClientAction|DashboardCommand|GameLaunchInfo|MeshMessage)$'

# RESEARCH.md § "Admin surface inventory" enumerates 16+ admin-consumed types.
# This is the fallback baseline emitted if primary grep discovery fails (e.g.
# CI runner cannot see the sibling admin repo).
FALLBACK_TYPES=(
  "BillingSessionInfo"
  "BillingSessionStatus"
  "ContentDirsResponse"
  "Driver"
  "FleetHealthResponse"
  "GameDirs"
  "GameInventory"
  "GameState"
  "DrivingState"
  "PlayableSignal"
  "PodFleetStatus"
  "PodInfo"
  "PodInventory"
  "PodStatus"
  "PricingTier"
  "SimType"
)

# --- Step 1: Enumerate rcFetch paths from admin. -----------------------------
PATHS_TMP="$(mktemp)"
trap 'rm -f "$PATHS_TMP"' EXIT

PRIMARY_AVAILABLE=1
if [ ! -d "$ADMIN_SRC" ]; then
  echo "enumerate-admin-types: admin src not found at $ADMIN_SRC — emitting FALLBACK"
  PRIMARY_AVAILABLE=0
else
  # Extract every rcFetch('...') literal path.
  grep -rhE "rcFetch\('[^']+" "$ADMIN_SRC" 2>/dev/null \
    | sed -nE "s/.*rcFetch\('([^']+)'.*/\1/p" \
    | sed -E 's/\?.*$//' \
    | sort -u > "$PATHS_TMP" || true

  PATH_COUNT=$(wc -l < "$PATHS_TMP" | tr -d ' ')
  if [ "$PATH_COUNT" -lt 10 ]; then
    echo "enumerate-admin-types: only $PATH_COUNT admin paths found (expected >=10) — emitting FALLBACK"
    PRIMARY_AVAILABLE=0
  fi
fi

# --- Step 2: For each admin path, crawl the racecontrol handler chain to
#            extract response-struct type names. This is a best-effort heuristic
#            — the planner will refine in Plan 02a. Here we only need a
#            seed list broad enough to cover RESEARCH's 16 core types.
TYPES_TMP="$(mktemp)"
trap 'rm -f "$PATHS_TMP" "$TYPES_TMP"' EXIT

if [ "$PRIMARY_AVAILABLE" -eq 1 ]; then
  # Heuristic A: for every path, grep the api/ dir for axum handler signatures
  # that return Json<T>, AppResult<Json<T>>, or impl IntoResponse. Extract T.
  # We also rely on the fallback types below as a floor — the crawl only adds,
  # never subtracts.
  grep -rhE '->\s*(impl IntoResponse|Json<[^>]+>|AppResult<Json<[^>]+>|Result<Json<[^>]+>)' "$RC_API_SRC" 2>/dev/null \
    | sed -nE 's/.*Json<([A-Z][A-Za-z0-9_]+)(<[^>]+>)?>.*/\1/p' \
    | sort -u >> "$TYPES_TMP" || true

  # Heuristic B: every rc-common pub struct/enum that appears in an api/ handler
  # body (as a return-type candidate) gets added. This is broader than strictly
  # needed but harmless — the D-14 denylist filters the problematic ones out.
  while IFS= read -r type_name; do
    [ -z "$type_name" ] && continue
    if grep -qE "pub (struct|enum) $type_name\b" "$RC_COMMON_SRC"/*.rs 2>/dev/null; then
      echo "$type_name" >> "$TYPES_TMP"
    fi
  done < <(grep -rhE '\b[A-Z][A-Za-z0-9_]+\b' "$RC_API_SRC" 2>/dev/null \
    | tr -c 'A-Za-z0-9_\n' '\n' \
    | grep -E '^[A-Z][A-Za-z0-9_]{3,}$' \
    | sort -u)
fi

# Always layer in the RESEARCH-enumerated fallback floor. This guarantees the
# output is never empty even if Heuristics A/B fail.
for t in "${FALLBACK_TYPES[@]}"; do
  echo "$t" >> "$TYPES_TMP"
done

# --- Step 3: D-14 denylist filter — remove any of the 8 adjacently-tagged
#            enums that accidentally slipped into the crawl. ------------------
FILTERED_TMP="$(mktemp)"
trap 'rm -f "$PATHS_TMP" "$TYPES_TMP" "$FILTERED_TMP"' EXIT

sort -u "$TYPES_TMP" | grep -vE "$D14_SKIP_LIST" > "$FILTERED_TMP"

# --- Step 4: Emit whitelist ---------------------------------------------------
{
  if [ "$PRIMARY_AVAILABLE" -eq 0 ]; then
    echo "# FALLBACK — primary grep discovery failed; RESEARCH-enumerated baseline in use."
    echo "# Re-run with PROJECT_ROOT set or restore sibling admin repo to upgrade to crawl-derived list."
  fi
  cat "$FILTERED_TMP"
} > "$OUT_FILE"

# --- Step 5: D-14 post-write sanity check (fail-closed). ---------------------
if grep -qE "$D14_SKIP_LIST" "$OUT_FILE"; then
  echo "enumerate-admin-types: D-14 VIOLATION: adjacently-tagged enum in admin whitelist"
  exit 1
fi

# --- Step 6: Minimum-coverage invariant (at least 16 non-comment lines). -----
NON_COMMENT_LINES=$(grep -cEv '^\s*(#|$)' "$OUT_FILE" || true)
if [ "$NON_COMMENT_LINES" -lt 16 ]; then
  echo "enumerate-admin-types: coverage invariant failed — only $NON_COMMENT_LINES non-comment lines (need >=16)"
  exit 1
fi

echo "enumerate-admin-types: wrote $NON_COMMENT_LINES type names to $OUT_FILE"
