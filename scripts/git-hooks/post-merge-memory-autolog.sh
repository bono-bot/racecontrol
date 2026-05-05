#!/usr/bin/env bash
# RP Playbook Upgrade — RISK-2 (MEMORY.md hygiene auto-append)
# MMA Iter4 5/5 consensus: ship-first; blast_radius=1-2; rollback = remove file or unset core.hooksPath
#
# Trigger: git post-merge (fires on any merge commit, including PR-merge into local main)
# Behavior: if the merged branch matches `pact-*` OR the merge-commit message contains `PACT-NNN`,
#   append a one-line summary to MEMORY.md and increment a session counter in V2-MASTER-STATE.md.
# Idempotency: hook checks last MEMORY.md line against the candidate line; skips on dup.
# Permanence: lives in repo at scripts/git-hooks/; activated via `git config core.hooksPath scripts/git-hooks`.
# NOT activated by default — opt-in only, per Iter4 captain_consent_required=NO with explicit rollback.

set -euo pipefail

# ---- locate canonical files ---------------------------------------------------
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$REPO_ROOT" ]; then
  exit 0   # not in a git repo — silently skip
fi

# Canonical MEMORY.md and V2-MASTER-STATE.md live in comms-link, not racecontrol.
# Resolve relative to repo root; if comms-link sibling not present, exit 0.
COMMS_LINK="$REPO_ROOT/../comms-link"
[ -d "$COMMS_LINK" ] || COMMS_LINK="$REPO_ROOT/comms-link"
[ -d "$COMMS_LINK" ] || { exit 0; }

MEMORY="$COMMS_LINK/MEMORY.md"
VMS="$COMMS_LINK/V2-MASTER-STATE.md"
[ -f "$MEMORY" ] || { exit 0; }

# ---- detect PACT context ------------------------------------------------------
HEAD_MSG="$(git log -1 --format=%B 2>/dev/null || echo "")"
HEAD_HASH="$(git rev-parse --short HEAD 2>/dev/null || echo "?")"
SOURCE_BRANCH="$(git log -1 --format=%s | grep -oE 'pact-[0-9-]+[a-z0-9-]*' | head -1 || true)"
PACT_ID="$(printf '%s' "$HEAD_MSG" | grep -oE 'PACT-[0-9]{8}-[0-9]{3}|PACT-[0-9]{3}' | head -1 || true)"

# only auto-log if this looks like a PACT merge
if [ -z "$PACT_ID" ] && [ -z "$SOURCE_BRANCH" ]; then
  exit 0
fi

# ---- compose one-line summary -------------------------------------------------
TS_IST="$(python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d %H:%M IST'))" 2>/dev/null || date -u +'%Y-%m-%d %H:%M UTC')"
SUMMARY_TITLE="$(printf '%s' "$HEAD_MSG" | head -1 | cut -c1-100)"
PACT_LABEL="${PACT_ID:-$SOURCE_BRANCH}"

LINE="- [auto] $TS_IST — $PACT_LABEL merged at $HEAD_HASH — $SUMMARY_TITLE"

# ---- idempotency check --------------------------------------------------------
LAST="$(tail -1 "$MEMORY" 2>/dev/null || echo "")"
if [ "$LAST" = "$LINE" ]; then
  exit 0   # already logged
fi
# also check if PACT_ID + hash pair already present anywhere in last 50 lines
if [ -n "$PACT_ID" ] && tail -50 "$MEMORY" 2>/dev/null | grep -qF "$PACT_ID merged at $HEAD_HASH"; then
  exit 0
fi

# ---- append (atomic via temp file) -------------------------------------------
TMP="$(mktemp)"
cat "$MEMORY" > "$TMP"
printf '%s\n' "$LINE" >> "$TMP"
mv "$TMP" "$MEMORY"

# ---- increment V2-MASTER-STATE §S-N (best effort, non-fatal on failure) ------
if [ -f "$VMS" ]; then
  CURRENT_S="$(grep -oE '^## §S-[0-9]+' "$VMS" 2>/dev/null | tail -1 | grep -oE '[0-9]+' || echo "0")"
  NEXT_S=$((CURRENT_S + 1))
  {
    echo ""
    echo "## §S-$NEXT_S — auto-append on PACT close"
    echo ""
    echo "- $LINE"
    echo "- source: post-merge-memory-autolog hook"
    echo ""
  } >> "$VMS" 2>/dev/null || true
fi

echo "[memory-autolog] appended $PACT_LABEL @ $HEAD_HASH to MEMORY.md" >&2
exit 0
