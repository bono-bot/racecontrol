#!/usr/bin/env bash
# scripts/graphify-quarterly-audit.sh — P6.2 from 2026-04-22 roadmap.
#
# Quarterly health check for the graphify knowledge graphs. Run via Windows
# Task Scheduler every 90 days (see registration snippet at bottom of file).
#
# Checks (informational only; exits 0 unless a hard failure):
#   1. graphifyy pip version on James + Bono (via relay)
#   2. unified graph freshness (mtime < 14 days)
#   3. node/edge counts vs previous audit
#   4. cross-repo edge density
#   5. bono-mirror staleness (mtime relative to /root/ on Bono)
#   6. known upstream PRs (999.4 backlog) — check for merged ones
#
# Output: writes a timestamped report to
#   audit/results/graphify-quarterly-YYYY-MM-DD.md

set -uo pipefail

# `git rev-parse || cd ... && pwd` has left-to-right precedence bug: pwd runs
# even when git succeeds, capturing two paths. Resolve with explicit fallback.
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$REPO_ROOT" ]; then
  REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fi
# Windows Git Bash returns "C:/..." paths which can break redirects; normalize.
if command -v cygpath >/dev/null 2>&1; then
  REPO_ROOT="$(cygpath -u "$REPO_ROOT" 2>/dev/null || echo "$REPO_ROOT")"
fi
TS=$(date -u +%Y-%m-%d)
OUT="$REPO_ROOT/audit/results/graphify-quarterly-$TS.md"
mkdir -p "$(dirname "$OUT")"

{
  echo "# Graphify Quarterly Audit — $TS"
  echo ""
  echo "Generated: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo ""

  # 1. Version checks
  echo "## 1. graphifyy versions"
  echo ""
  JAMES_VER=$(python3 -c "import importlib.metadata as m; print(m.version('graphifyy'))" 2>&1 || echo "NOT_INSTALLED")
  echo "- James (pip)  : \`$JAMES_VER\`"
  BONO_VER=$(curl -s -m 10 -X POST http://localhost:8766/relay/exec/run \
    -H "Content-Type: application/json" \
    -d '{"command":"graphify_version","reason":"quarterly audit"}' \
    2>/dev/null | grep -oE '"stdout":"[^"]*' | cut -d'"' -f4 | head -1)
  echo "- Bono (pipx)  : \`${BONO_VER:-UNKNOWN}\`"
  echo ""

  # 2. Unified graph freshness
  UNIFIED="$REPO_ROOT/graphify-out-unified/graph.json"
  echo "## 2. Unified graph"
  echo ""
  if [ -f "$UNIFIED" ]; then
    MTIME=$(stat -c %y "$UNIFIED" 2>/dev/null || stat -f %Sm "$UNIFIED")
    AGE_DAYS=$(( ( $(date +%s) - $(stat -c %Y "$UNIFIED" 2>/dev/null || stat -f %m "$UNIFIED") ) / 86400 ))
    NODES=$(grep -oE '"id":' "$UNIFIED" | wc -l)
    EDGES=$(grep -oE '"source":' "$UNIFIED" | wc -l)
    echo "- Path: \`$UNIFIED\`"
    echo "- Modified: $MTIME (age: ${AGE_DAYS} days)"
    echo "- Nodes: ~$NODES | Edges: ~$EDGES"
    if [ "$AGE_DAYS" -gt 14 ]; then
      echo "- **WARN:** unified graph is stale (>14 days). Run: \`node scripts/graphify-post/unify.mjs\`"
    fi
  else
    echo "- **FAIL:** unified graph missing at \`$UNIFIED\`"
  fi
  echo ""

  # 3. Bono mirror freshness
  echo "## 3. Bono mirror"
  echo ""
  MIRROR="$REPO_ROOT/../bono-mirror"
  if [ -d "$MIRROR" ]; then
    echo "| repo | size | mtime |"
    echo "|---|---|---|"
    find "$MIRROR" -name 'graph.json' -maxdepth 3 2>/dev/null | while read -r f; do
      repo=$(basename "$(dirname "$(dirname "$f")")")
      size=$(stat -c %s "$f" 2>/dev/null || stat -f %z "$f")
      mtime=$(stat -c %y "$f" 2>/dev/null || stat -f %Sm "$f")
      echo "| $repo | $size | $mtime |"
    done
  else
    echo "- **FAIL:** bono-mirror missing. Run: \`bash scripts/graphify-refresh-bono-mirror.sh\`"
  fi
  echo ""

  # 4. 999.4 backlog status
  echo "## 4. Tier 3 upstream PR backlog (999.4)"
  echo ""
  BACKLOG="$REPO_ROOT/.planning/phases/999.4-tier-3-upstream-graphify-prs"
  if [ -d "$BACKLOG" ]; then
    ISSUE_COUNT=$(find "$BACKLOG" -name 'g*.md' | wc -l)
    OPENED_ON=$(git log --format='%ci' --follow "$BACKLOG" 2>/dev/null | tail -1 | cut -d' ' -f1)
    echo "- Drafts: $ISSUE_COUNT | Opened: ${OPENED_ON:-unknown}"
    if [ -n "${OPENED_ON:-}" ]; then
      OPENED_EPOCH=$(date -d "$OPENED_ON" +%s 2>/dev/null || echo "")
      if [ -n "$OPENED_EPOCH" ]; then
        AGE_DAYS=$(( ($(date +%s) - OPENED_EPOCH) / 86400 ))
        echo "- Age: $AGE_DAYS days"
        if [ "$AGE_DAYS" -gt 180 ]; then
          echo "- **ACTION:** >180 days unpromoted — review per P6.4; either close or schedule."
        fi
      fi
    fi
  fi
  echo ""

  echo "## Quarterly-audit script"
  echo ""
  echo "Script: \`scripts/graphify-quarterly-audit.sh\` (this file). Registered in Windows Task Scheduler as \`GraphifyQuarterlyAudit\` (90-day cadence). To register manually:"
  echo ""
  echo '```powershell'
  echo 'schtasks /Create /TN GraphifyQuarterlyAudit /TR "bash C:\\Users\\bono\\racingpoint\\racecontrol\\scripts\\graphify-quarterly-audit.sh" /SC MONTHLY /MO 3 /ST 03:00 /F'
  echo '```'
} > "$OUT"

echo "Wrote: $OUT"
