#!/usr/bin/env bash
# v2-state-refresh.sh — auto-rebuild a §S-(N+1) snapshot for comms-link/V2-MASTER-STATE.md
#
# PACT-20260503-002 Piece 2 (refresh script).
# Captain authorization 2026-05-03. Doctrine-class.
#
# Inputs (ground truth, in priority order):
#   1. racecontrol git: ls-tree origin/main + amend1 + V2-named branches
#   2. racecontrol git: for-each-ref + rev-list ahead-counts
#   3. racecontrol git: worktree list + stash list
#   4. comms-link/PACTS.md tail
#   5. comms-link/data/pact-slots.jsonl tail
#   6. comms-link/INBOX.md 2026-05-* entries (timestamp grep only)
#   7. memory key files: V2 customer-workflows / wallet Framing C / core product / V1→V2 nav v2 / F1-F6
#   8. .tmp/design-bundle/ (.jsx file count + which ported)
#
# Outputs:
#   --dry-run       (default): print §S-(N+1) section to stdout
#   --append        : append §S-(N+1) to comms-link/V2-MASTER-STATE.md
#   --commit        : append + git add + git commit (atomic-publish primitive)
#
# Composes-with: PACT-026 atomic-publish, PACT-CHARTER §V2.0, V2-MASTER-STATE.md append-only invariant.

set -euo pipefail

MODE="${1:-dry-run}"
RACECONTROL_ROOT="${RACECONTROL_ROOT:-/c/Users/bono/racingpoint/racecontrol}"
COMMS_LINK_ROOT="${COMMS_LINK_ROOT:-/c/Users/bono/racingpoint/comms-link}"
MEMORY_DIR="${MEMORY_DIR:-/c/Users/bono/.claude/projects/C--Users-bono/memory}"
DESIGN_BUNDLE_ROOT="${DESIGN_BUNDLE_ROOT:-/c/Users/bono/.tmp/design-bundle/extracted/racing-point-esports/project}"

V2_MASTER_STATE="$COMMS_LINK_ROOT/V2-MASTER-STATE.md"

# Compute snapshot ID (next §S-N) by counting existing §S-N headers + 1
NEXT_N=1
if [ -f "$V2_MASTER_STATE" ]; then
  EXISTING=$(grep -c '^## §S-[0-9]' "$V2_MASTER_STATE" 2>/dev/null || echo 0)
  NEXT_N=$((EXISTING + 1))
fi

NOW_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
NOW_IST="$(TZ='Asia/Kolkata' date +'%Y-%m-%d %H:%M IST')"
SCRIPT_VERSION="0.1.0 (PACT-20260503-002 Piece 2 initial)"

# Sub-routines

count_v2_tsx_on_ref() {
  local ref="$1"
  local n
  n=$(git -C "$RACECONTROL_ROOT" ls-tree -r --name-only "$ref" 2>/dev/null \
    | grep -E '(kiosk|web|admin|pwa)/.*v2.*\.tsx' \
    | wc -l \
    | tr -d ' ')
  echo "${n:-0}"
}

count_ahead_of_main() {
  local ref="$1"
  git -C "$RACECONTROL_ROOT" rev-list --count "main..$ref" 2>/dev/null || echo 0
}

last_commit_subject() {
  local ref="$1"
  git -C "$RACECONTROL_ROOT" log -1 --pretty=format:'%s' "$ref" 2>/dev/null | cut -c1-100 || true
}

count_design_bundle_files() {
  if [ -d "$DESIGN_BUNDLE_ROOT" ]; then
    ls "$DESIGN_BUNDLE_ROOT"/*.jsx 2>/dev/null | wc -l | tr -d ' '
  else
    echo 0
  fi
}

# Header

cat <<HEADER

## §S-${NEXT_N} — Snapshot ${NOW_IST} (auto-refresh via v2-state-refresh.sh)

### §S-${NEXT_N}.0 — Provenance

**Authored:** ${NOW_IST} (UTC ${NOW_UTC})
**Method:** \`scripts/v2-state-refresh.sh ${MODE}\` (script version ${SCRIPT_VERSION})
**Mode:** ${MODE}
HEADER

# §X.1 — Surface counts on key refs

V2_ON_MAIN=$(count_v2_tsx_on_ref "origin/main")
V2_ON_AMEND1=$(count_v2_tsx_on_ref "amend1-section1-heart-admin-substrate")
V2_HOST_AHEAD=$(count_ahead_of_main "pact-20260503-001-v2-host-3500-substrate")
AMEND1_AHEAD=$(count_ahead_of_main "amend1-section1-heart-admin-substrate")

cat <<COUNTS

### §S-${NEXT_N}.1 — Surface .tsx counts (V2 paths in kiosk/web/admin/pwa)

| Ref | V2 .tsx count | Ahead of main |
|---|---|---|
| \`origin/main\` | ${V2_ON_MAIN} | (baseline) |
| \`amend1-section1-heart-admin-substrate\` | ${V2_ON_AMEND1} | ${AMEND1_AHEAD} |
| \`pact-20260503-001-v2-host-3500-substrate\` | (web-v2/ scaffold; not counted in V2 .tsx pattern) | ${V2_HOST_AHEAD} |

COUNTS

# §X.2 — Unmerged V2-substrate branches

cat <<BRANCHES_HEADER
### §S-${NEXT_N}.2 — Branches with unmerged commits ahead of main (V2-relevant filter)

| Branch | Ahead | Last commit subject |
|---|---|---|
BRANCHES_HEADER

# Curated V2-relevant branch list (process-of-elimination filter from §S-1)
V2_BRANCHES=(
  "amend1-section1-heart-admin-substrate"
  "pact-20260503-001-v2-host-3500-substrate"
  "pact-008-phase1-substrate-types"
  "pact-014-amend-1-phase-b-rc-agent-info-txt"
  "feat/pwa-spinal-cord-b001-20260424"
  "feat/admin-gateway-contract-spec-20260423"
  "spec/admin-panel-spinal-cord-20260422"
  "origin/pact-007-c6-disk-pressure-fix-launch-contract-v2"
)

for branch in "${V2_BRANCHES[@]}"; do
  ahead=$(count_ahead_of_main "$branch")
  subject=$(last_commit_subject "$branch")
  if [ -z "$subject" ]; then
    echo "| \`$branch\` | (ref not found) | — |"
  else
    echo "| \`$branch\` | $ahead | $subject |"
  fi
done

# §X.3 — Worktrees (V2-relevance flag)

cat <<WORKTREES_HEADER

### §S-${NEXT_N}.3 — Git worktrees

WORKTREES_HEADER

git -C "$RACECONTROL_ROOT" worktree list 2>/dev/null \
  | awk '{print "  - `" $0 "`"}' \
  || echo "  (worktree list unavailable)"

# §X.4 — In-flight PACTs (PACTS.md tail)

cat <<PACTS_HEADER

### §S-${NEXT_N}.4 — In-flight PACTs (latest 8 from PACTS.md)

PACTS_HEADER

if [ -f "$COMMS_LINK_ROOT/PACTS.md" ]; then
  grep -E '^\| PACT-2026' "$COMMS_LINK_ROOT/PACTS.md" \
    | tail -8 \
    | awk -F'|' '{
        gsub(/^ +| +$/, "", $2);
        gsub(/^ +| +$/, "", $3);
        gsub(/^ +| +$/, "", $4);
        printf("  - %s · %s · status: %s...\n", $2, $3, substr($4, 1, 80));
      }'
else
  echo "  (PACTS.md unavailable)"
fi

# §X.5 — pact-slots.jsonl tail (last 5 actions)

cat <<SLOTS_HEADER

### §S-${NEXT_N}.5 — pact-slots.jsonl tail (latest 5 actions)

SLOTS_HEADER

if [ -f "$COMMS_LINK_ROOT/data/pact-slots.jsonl" ]; then
  tail -5 "$COMMS_LINK_ROOT/data/pact-slots.jsonl" \
    | while IFS= read -r line; do
        ts=$(echo "$line"   | sed -n 's/.*"ts":"\([^"]*\)".*/\1/p')
        action=$(echo "$line" | sed -n 's/.*"action":"\([^"]*\)".*/\1/p')
        slot=$(echo "$line"   | sed -n 's/.*"slot":"\([^"]*\)".*/\1/p')
        ai=$(echo "$line"     | sed -n 's/.*"ai":"\([^"]*\)".*/\1/p')
        echo "  - ${ts:-?} · ${slot:-?} · ${action:-?} · ${ai:-?}"
      done
else
  echo "  (pact-slots.jsonl unavailable)"
fi

# §X.6 — INBOX recent activity (timestamp grep only — no body content)

cat <<INBOX_HEADER

### §S-${NEXT_N}.6 — INBOX recent activity (last 10 dated entries 2026-05-*)

INBOX_HEADER

if [ -f "$COMMS_LINK_ROOT/INBOX.md" ]; then
  grep -E '^## 2026-05-' "$COMMS_LINK_ROOT/INBOX.md" \
    | head -10 \
    | sed 's/^/  - /'
else
  echo "  (INBOX.md unavailable)"
fi

# §X.7 — Design bundle .jsx file count

DESIGN_FILES=$(count_design_bundle_files)

cat <<DESIGN

### §S-${NEXT_N}.7 — Design source bundle

\`$DESIGN_BUNDLE_ROOT\` contains ${DESIGN_FILES} .jsx file(s).

DESIGN

# §X.8 — Memory file presence check (key V2 doctrine files)

cat <<MEMORY_HEADER
### §S-${NEXT_N}.8 — Memory file presence check

| File | Present? |
|---|---|
MEMORY_HEADER

KEY_FILES=(
  "project_v2_customer_workflows_consolidated_20260503.md"
  "project_v2_wallet_framing_c_locked_20260503.md"
  "project_v2_core_product_definition.md"
  "project_v1_to_v2_navigation_plan.md"
  "project_v1_to_v2_navigation_plan_AMENDMENTS_v2.md"
  "project_v2_six_foundation_contracts_mma_validated_20260502.md"
  "project_v2_pillar_cascade_12_of_14_anchored_iter4_20260502.md"
  "feedback_v2_doctrine_alignment_drift_g9_pact_20260503_002.md"
)

for f in "${KEY_FILES[@]}"; do
  if [ -f "$MEMORY_DIR/$f" ]; then
    echo "| \`$f\` | YES |"
  else
    echo "| \`$f\` | **MISSING** |"
  fi
done

# §X.9 — NOT verified in this auto-refresh

cat <<NOT_TESTED

### §S-${NEXT_N}.9 — NOT verified in this auto-refresh

- Bono VPS state (Linux side; james cannot probe directly without bono daemon)
- Cloud apps deployed-version state (\`app.racingpoint.cloud\`, \`racingpoint.cloud/admin\`)
- Production deploy state on Server .23 / Pods 1-8 / POS .130
- Whether amend1 builds clean today
- INBOX message bodies (timestamp grep only)
- Stash content beyond filename-stat
- Bilateral parity of this snapshot (gates on bono \`git_pull\` + bono-side append)

NOT_TESTED

# §X.10 — Refresh metadata footer

cat <<META

### §S-${NEXT_N}.10 — Refresh metadata

| Field | Value |
|---|---|
| Snapshot ID | §S-${NEXT_N} |
| Authored-by | \`v2-state-refresh.sh ${MODE}\` (mode: ${MODE}) |
| Script version | ${SCRIPT_VERSION} |
| Author timestamp | ${NOW_IST} (UTC ${NOW_UTC}) |
| Surface .tsx on main | ${V2_ON_MAIN} |
| Surface .tsx on amend1 | ${V2_ON_AMEND1} |
| amend1 ahead-of-main | ${AMEND1_AHEAD} |
| Refresh staleness threshold | 24h advisory / 7d forced |

META

# Append-mode handler

if [ "$MODE" = "append" ] || [ "$MODE" = "--append" ] || [ "$MODE" = "commit" ] || [ "$MODE" = "--commit" ]; then
  echo "" >&2
  echo "[v2-state-refresh] MODE=${MODE} requested but stdout-only path active in this version." >&2
  echo "[v2-state-refresh] To complete: redirect stdout to V2-MASTER-STATE.md and commit manually:" >&2
  echo "  bash scripts/v2-state-refresh.sh dry-run >> $V2_MASTER_STATE" >&2
  echo "  cd $COMMS_LINK_ROOT && git add V2-MASTER-STATE.md && git commit -m 'v2-state-refresh: §S-${NEXT_N} auto-snapshot'" >&2
  echo "[v2-state-refresh] Direct-append mode deferred to v0.2 (gates on Captain G33-GUIDE-CONFIRM ratify of PACT-20260503-002)." >&2
fi
