#!/usr/bin/env bash
# memory-reconciliation.sh — Detect drift between memory, code, and deployed state
#
# Run weekly or after intensive sessions (MMA audits, milestone ships).
# Discovered 2026-04-03: 6 issues were "open" in memory but already fixed in code.
#
# Usage:
#   bash scripts/memory-reconciliation.sh

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
MEMORY_DIR="$HOME/.claude/projects/C--Users-bono/memory"

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
RESET='\033[0m'

ISSUES=0

echo "============================================================"
echo "Memory ↔ Code Reconciliation Audit"
echo "============================================================"
echo ""

# ─── Check 1: Stale bug memories ───────────────────────────────────
echo -e "${CYAN}=== Check 1: Stale bug/issue memory files ===${RESET}"
STALE_BUGS=$(grep -rl "CRITICAL\|not fixed\|needs fix\|pending fix\|all.*fail" "$MEMORY_DIR"/project_*.md 2>/dev/null || true)
if [ -n "$STALE_BUGS" ]; then
    while IFS= read -r f; do
        basename=$(basename "$f")
        # Extract the description line
        desc=$(grep "^description:" "$f" 2>/dev/null | head -1 | sed 's/description: //')
        if echo "$desc" | grep -qi "RESOLVED\|SHIPPED\|DONE\|FIXED"; then
            continue
        fi
        echo -e "  ${YELLOW}REVIEW:${RESET} $basename"
        echo "    $desc"
        ISSUES=$((ISSUES+1))
    done <<< "$STALE_BUGS"
    [ "$ISSUES" -eq 0 ] && echo -e "  ${GREEN}All bug memories have RESOLVED/SHIPPED status${RESET}"
else
    echo -e "  ${GREEN}No stale bug memories found${RESET}"
fi
echo ""

# ─── Check 2: Naming convention drift ─────────────────────────────
echo -e "${CYAN}=== Check 2: Naming convention drift ===${RESET}"
cd "$REPO_ROOT"

# rc-core references (deprecated name)
RC_CORE_HITS=$(grep -rn "rc-core" scripts/ --include="*.js" --include="*.sh" --include="*.bat" 2>/dev/null | grep -v "node_modules\|\.git\|worktree\|memory-reconciliation" || true)
if [ -n "$RC_CORE_HITS" ]; then
    echo -e "  ${RED}FOUND: 'rc-core' references (should be 'racecontrol'):${RESET}"
    echo "$RC_CORE_HITS" | while IFS= read -r line; do echo "    $line"; done
    ISSUES=$((ISSUES+1))
else
    echo -e "  ${GREEN}No stale 'rc-core' references in scripts${RESET}"
fi

# Check for hardcoded James IP in deploy scripts (should use $JAMES_IP variable)
HARDCODED_IP=$(grep -rn "192\.168\.31\.27" scripts/ --include="*.sh" 2>/dev/null | grep -v "JAMES_IP\|#.*IP\|comment\|pod-map\|network-map" | head -5 || true)
if [ -n "$HARDCODED_IP" ]; then
    echo -e "  ${YELLOW}WARN: Hardcoded James IP (.27) in scripts (use \$JAMES_IP):${RESET}"
    echo "$HARDCODED_IP" | while IFS= read -r line; do echo "    $line"; done
fi
echo ""

# ─── Check 3: Deploy drift ────────────────────────────────────────
echo -e "${CYAN}=== Check 3: Deploy state vs code ===${RESET}"
HEAD_SHORT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
echo "  Local HEAD: $HEAD_SHORT"

# Server build
SERVER_BUILD=$(curl -s --connect-timeout 3 http://192.168.31.23:8080/api/v1/health 2>/dev/null | python3 -c 'import sys,json; print(json.load(sys.stdin).get("build_id",""))' 2>/dev/null || echo "")
if [ -n "$SERVER_BUILD" ]; then
    echo "  Server build: $SERVER_BUILD"
    # Check for undeployed racecontrol changes
    UNDEPLOYED=$(git log "${SERVER_BUILD}..HEAD" --oneline -- crates/racecontrol/ 2>/dev/null | wc -l)
    if [ "$UNDEPLOYED" -gt 0 ]; then
        echo -e "  ${YELLOW}WARN: $UNDEPLOYED undeployed racecontrol commits since server build${RESET}"
        git log "${SERVER_BUILD}..HEAD" --oneline -- crates/racecontrol/ 2>/dev/null | head -5 | while IFS= read -r line; do echo "    $line"; done
        ISSUES=$((ISSUES+1))
    else
        echo -e "  ${GREEN}Server is up to date${RESET}"
    fi
else
    echo -e "  ${YELLOW}Server unreachable (venue may be closed)${RESET}"
fi
echo ""

# ─── Check 4: Memory vs MEMORY.md index ───────────────────────────
echo -e "${CYAN}=== Check 4: Orphaned memory files ===${RESET}"
ORPHANS=0
for f in "$MEMORY_DIR"/*.md; do
    basename=$(basename "$f")
    [ "$basename" = "MEMORY.md" ] && continue
    [ "$basename" = "SOLUTIONS-INDEX.md" ] && continue
    if ! grep -q "$basename" "$MEMORY_DIR/MEMORY.md" 2>/dev/null; then
        echo -e "  ${YELLOW}ORPHAN: $basename (not in MEMORY.md index)${RESET}"
        ORPHANS=$((ORPHANS+1))
    fi
done
if [ "$ORPHANS" -eq 0 ]; then
    echo -e "  ${GREEN}All memory files indexed in MEMORY.md${RESET}"
fi
echo ""

# ─── Summary ──────────────────────────────────────────────────────
echo "============================================================"
if [ "$ISSUES" -gt 0 ]; then
    echo -e "${YELLOW}FOUND $ISSUES issue(s) requiring attention${RESET}"
    echo "Review each YELLOW/RED item above and update memory or code."
else
    echo -e "${GREEN}ALL CLEAR — memory, code, and deploy are in sync${RESET}"
fi
echo "============================================================"
