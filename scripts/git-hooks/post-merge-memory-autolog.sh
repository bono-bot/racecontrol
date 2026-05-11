#!/usr/bin/env bash
# RP Playbook Upgrade — RISK-2 (memory hygiene auto-append) v2 — JSONL-only
#
# v1 (committed 0492b602) wrote to comms-link/MEMORY.md + V2-MASTER-STATE.md
# directly. That left comms-link working-tree dirty after every PACT merge,
# colliding with auto-flush guards on inbox-append.js (PACT-026 atomic-publish).
#
# v2 (this file) writes ONLY to comms-link/data/memory-autolog.jsonl as an
# append-only audit ledger. No MEMORY.md or V2-MASTER-STATE.md mutation.
# Promotion from JSONL -> MEMORY.md is a separate human-gated step.
# This eliminates the working-tree-dirt class entirely.
#
# Trigger: git post-merge (fires on any merge commit, including PR-merge into
#   local main).
# Behavior: if the merge-commit message contains PACT-NNN OR the source
#   branch name matches pact-*, append one JSONL row to memory-autolog.jsonl.
# Idempotency: check if {pact_id, merge_hash} pair already exists in last
#   100 lines of the ledger.
# Atomicity: append via >> is single-syscall on POSIX/Windows for lines
#   under PIPE_BUF (4KB); each row is well under that. No read-modify-write.
# Permanence: ledger is committed manually as part of normal comms-link
#   workflow; the hook itself doesn't commit or push anything.
# Bypass: BYPASS_MEMORY_AUTOLOG=1 env var skips the hook entirely.

set -euo pipefail

# ---- BYPASS env override -----------------------------------------------------
if [ "${BYPASS_MEMORY_AUTOLOG:-0}" = "1" ]; then
  exit 0
fi

# ---- locate canonical files --------------------------------------------------
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || true)"
if [ -z "$REPO_ROOT" ]; then
  exit 0   # not in a git repo — silently skip
fi

# Resolve comms-link as sibling of the repo (matches james + bono layouts)
COMMS_LINK="$REPO_ROOT/../comms-link"
[ -d "$COMMS_LINK" ] || COMMS_LINK="$REPO_ROOT/comms-link"
if [ ! -d "$COMMS_LINK" ]; then
  exit 0   # comms-link not findable — silent skip
fi

DATA_DIR="$COMMS_LINK/data"
LEDGER="$DATA_DIR/memory-autolog.jsonl"

# ---- detect PACT context -----------------------------------------------------
HEAD_HASH="$(git rev-parse --short HEAD 2>/dev/null || echo "")"
[ -n "$HEAD_HASH" ] || exit 0
HEAD_MSG_FIRST_LINE="$(git log -1 --format=%s 2>/dev/null || echo "")"
HEAD_MSG_FULL="$(git log -1 --format=%B 2>/dev/null || echo "")"

# PACT-ID lookup: prefer PACT-YYYYMMDD-NNN; fall back to PACT-NNN
PACT_ID="$(printf '%s' "$HEAD_MSG_FULL" | grep -oE 'PACT-[0-9]{8}-[0-9]{3}' | head -1 || true)"
if [ -z "$PACT_ID" ]; then
  PACT_ID="$(printf '%s' "$HEAD_MSG_FULL" | grep -oE 'PACT-[0-9]{3}' | head -1 || true)"
fi

# Source branch detection: from merge-commit second parent if present
SOURCE_BRANCH=""
if git rev-parse --verify HEAD^2 >/dev/null 2>&1; then
  SOURCE_BRANCH="$(git log -1 --format=%P HEAD | awk '{print $2}' | xargs -I{} git name-rev --name-only --refs='refs/heads/*' {} 2>/dev/null | grep -oE 'pact-[0-9-]+[a-z0-9-]*' | head -1 || true)"
fi
if [ -z "$SOURCE_BRANCH" ]; then
  SOURCE_BRANCH="$(printf '%s' "$HEAD_MSG_FIRST_LINE" | grep -oE 'pact-[0-9-]+[a-z0-9-]*' | head -1 || true)"
fi

# only fire if this looks like a PACT-related merge
if [ -z "$PACT_ID" ] && [ -z "$SOURCE_BRANCH" ]; then
  exit 0
fi

# ---- compose entry -----------------------------------------------------------
TS_UTC="$(date -u +'%Y-%m-%dT%H:%M:%S.000Z')"
SUMMARY="$(printf '%s' "$HEAD_MSG_FIRST_LINE" | cut -c1-160 | tr -d '\r' | sed 's/\\/\\\\/g; s/"/\\"/g')"
ORIGIN_REPO="$(basename "$REPO_ROOT")"
PACT_FIELD="${PACT_ID:-null}"
BRANCH_FIELD="${SOURCE_BRANCH:-null}"

# Build JSON line carefully (no jq dependency)
LINE="{\"ts\":\"$TS_UTC\",\"pact_id\":\"$PACT_FIELD\",\"source_branch\":\"$BRANCH_FIELD\",\"merge_hash\":\"$HEAD_HASH\",\"origin_repo\":\"$ORIGIN_REPO\",\"summary\":\"$SUMMARY\"}"

# ---- idempotency check (last 100 lines for {pact_id, merge_hash} pair) ------
mkdir -p "$DATA_DIR"
if [ -f "$LEDGER" ]; then
  if [ -n "$PACT_ID" ]; then
    if tail -100 "$LEDGER" 2>/dev/null | grep -F "\"pact_id\":\"$PACT_ID\"" | grep -qF "\"merge_hash\":\"$HEAD_HASH\""; then
      exit 0
    fi
  fi
  if [ -n "$SOURCE_BRANCH" ]; then
    if tail -100 "$LEDGER" 2>/dev/null | grep -F "\"source_branch\":\"$SOURCE_BRANCH\"" | grep -qF "\"merge_hash\":\"$HEAD_HASH\""; then
      exit 0
    fi
  fi
fi

# ---- atomic append (single >> syscall, line < PIPE_BUF) --------------------
printf '%s\n' "$LINE" >> "$LEDGER"

# Stay silent on success — hook output noise pollutes git CLI
exit 0
