#!/bin/bash
# scripts/git-hooks/pre-commit-state-divergence.sh — PACT-20260505-004 P6 hook.
#
# Phase 1 substrate ship of the P6 State-Divergence Pre-Commit Hook
# (PACT-20260505-004 FILED 2026-05-05 + bono AMPLIFIER §AMEND-1 ABSORB-IN-FULL
# 2026-05-06 ~00:05 IST + Captain G33-GUIDE-CONFIRM PENDING).
#
# Substrate-substitutes the deferred session-start git-fetch hook (the original
# fix for the H5-A shallow-state-read G9 class anchored 2026-05-05 N=3
# same-session). Catches the same staleness class at COMMIT time instead of
# session-start time — higher leverage because a stale assertion in
# conversation is a G9 (cost: correction round + memory file) but a stale
# commit is shipped substrate (cost: ratchet on origin/main, drift across
# worktrees, partner-pull confusion).
#
# Defense-in-depth pairing: at session-start, the deferred fetch hook (post-
# 2026-06-04) catches in-conversation stale reads. At edit-time, PACT-005
# conflict-marker hook + PACT-20260505-005 CFSG (constraint-forward-simulation
# gate) catch shallow constraint citations. At commit-time, this hook catches
# stale-state-shipped. At event-time, PACT-cross-pilot-bilateral-sync-substrate
# (SIFL) catches partner-pilot in-flight state divergence. Per CAVEAT-D §AMEND-1:
# if a session-start fetch ran 30s ago and this commit-time fetch is also
# fetching, it is REDUNDANT BY DESIGN — different trigger surfaces catch
# different shallow-read patterns. Do not short-circuit.
#
# This hook is a STANDALONE script. It is NOT installed as `.git/hooks/pre-commit`
# (that file is the existing Phase 445 drift guard). Activation is opt-in via
# `git config core.hooksPath scripts/git-hooks` plus a wrapper that chains both
# pre-commits. Phase 3 of PACT-004 covers activation; this Phase 1 ships the
# script + test harness only.
#
# Behaviour summary:
#   1. Fast-skip via allowlist (commit-msg [skip-state-check] OR all-paths in
#      .tmp/ / node_modules/ / target/) -> exit 0
#   2. BYPASS env var (BYPASS_STATE_DIVERGENCE_CHECK=1) -> audit + exit 0
#   3. git fetch origin main with 3s timeout, fail-open on flake (audit-record)
#   4. Compare local HEAD vs origin/main; block if behind by K>=2 commits (default)
#   5. Read comms-link/V2-MASTER-STATE.md; extract highest §S-N integer header
#   6. If commit message references §S-M and M > local-highest-§S-N -> block
#   7. Audit-log every check to data/state-divergence-audit.jsonl (with rotation)
#
# Bypass channels:
#   - Commit message contains [skip-state-check]   (substrate-ship + opt-in)
#   - All staged paths under .tmp/, node_modules/, target/   (build artifacts)
#   - BYPASS_STATE_DIVERGENCE_CHECK=1 env var   (rare, audited)
#   - git --no-verify   (last-resort; not detected by hook)
#
# Configuration:
#   - STATE_DIVERGENCE_K_BEHIND=N    (default 2; commits-behind threshold)
#   - STATE_DIVERGENCE_VERBOSE=1     (or git config state-divergence.verbose true)
#   - ROTATE_JSONL_DISABLE=1         (test scaffolding; skips rotation in test harness)
#
# Exit codes:
#   0 = pass (allow commit)
#   1 = block (state-divergence detected)
#   2 = config error (e.g. missing dependency)
#
# Cross-platform: POSIX bash + git + standard coreutils. Tested on Linux bash
# (bono VPS) and Windows Git Bash (james .27). Date math uses POSIX-stable form.
#
# Author: james (PACT-20260505-004 §AMEND-1 ABSORB-IN-FULL absorbed)
# Co-author: bono (AMPLIFIER §AMEND-1 5 CAVEATs A-D + 3 NEW FINDINGs)
# Captain auth (Phase 1 ship): pending G33-GUIDE-CONFIRM as of this commit

set -u

# -- Resolve helper paths ---------------------------------------------------

HOOK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# Resolve REPO_ROOT from the active git repo (where the commit is happening),
# NOT from the script location. The hook may be invoked via core.hooksPath
# from a different worktree than the script lives in.
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
[ -z "$REPO_ROOT" ] && REPO_ROOT="$(cd "$HOOK_DIR/../.." && pwd)"
LIB_DIR="$HOOK_DIR/lib"
AUDIT_LOG="$REPO_ROOT/data/state-divergence-audit.jsonl"

# Verbose flag: env var OR git config.
VERBOSE="${STATE_DIVERGENCE_VERBOSE:-}"
if [ -z "$VERBOSE" ]; then
  VERBOSE="$(git config --get state-divergence.verbose 2>/dev/null || true)"
  [ "$VERBOSE" = "true" ] && VERBOSE=1 || VERBOSE=""
fi

# Configurable K-behind threshold (default 2).
K_BEHIND="${STATE_DIVERGENCE_K_BEHIND:-2}"

# Source the rotation helper if present.
if [ -f "$LIB_DIR/jsonl-rotate.sh" ]; then
  # shellcheck source=lib/jsonl-rotate.sh
  . "$LIB_DIR/jsonl-rotate.sh"
fi

# -- Helpers ----------------------------------------------------------------

log_verbose() {
  [ -n "$VERBOSE" ] && echo "[state-divergence] $*" >&2 || true
}

log_warn() {
  echo "[state-divergence] WARN: $*" >&2
}

# Append one JSONL row to audit log. Best-effort; rotation is silent on failure.
audit_append() {
  local outcome="$1"
  local detail="$2"
  local data_dir
  data_dir="$(dirname "$AUDIT_LOG")"
  [ -d "$data_dir" ] || mkdir -p "$data_dir" 2>/dev/null || true

  if declare -F rotate_jsonl_if_needed >/dev/null 2>&1; then
    rotate_jsonl_if_needed "$AUDIT_LOG" || true
  fi

  local ts
  ts="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  local local_head origin_head branch
  local_head="$(git rev-parse HEAD 2>/dev/null || echo 'unknown')"
  origin_head="$(git rev-parse origin/main 2>/dev/null || echo 'unknown')"
  branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo 'unknown')"

  # Escape detail for JSON: collapse newlines + escape quotes/backslashes.
  local detail_esc
  detail_esc="$(printf '%s' "$detail" | sed 's/\\/\\\\/g; s/"/\\"/g; s/	/\\t/g' | tr '\n' ' ')"

  printf '{"ts":"%s","outcome":"%s","local_head":"%s","origin_head":"%s","branch":"%s","detail":"%s"}\n' \
    "$ts" "$outcome" "$local_head" "$origin_head" "$branch" "$detail_esc" \
    >> "$AUDIT_LOG" 2>/dev/null || true
}

# Read commit message from staged commit (Git provides it via env or .git/COMMIT_EDITMSG).
read_commit_msg() {
  if [ -n "${GIT_COMMIT_MSG_FILE:-}" ] && [ -f "$GIT_COMMIT_MSG_FILE" ]; then
    cat "$GIT_COMMIT_MSG_FILE"
    return
  fi
  local msg_file="$REPO_ROOT/.git/COMMIT_EDITMSG"
  [ -f "$msg_file" ] && cat "$msg_file" || echo ""
}

# Check if all staged paths are under allowlist roots.
all_paths_allowlisted() {
  local staged
  staged="$(git diff --cached --name-only 2>/dev/null)"
  [ -z "$staged" ] && return 1
  while IFS= read -r path; do
    [ -z "$path" ] && continue
    case "$path" in
      .tmp/*|node_modules/*|target/*|*/.tmp/*|*/node_modules/*|*/target/*) ;;
      *) return 1 ;;
    esac
  done <<< "$staged"
  return 0
}

# Locate comms-link sibling repo. Returns absolute path on stdout, or empty.
resolve_comms_link() {
  local candidates=(
    "$REPO_ROOT/../comms-link"
    "$REPO_ROOT/../../comms-link"
    "${COMMS_LINK_REPO:-}"
  )
  local c
  for c in "${candidates[@]}"; do
    [ -z "$c" ] && continue
    if [ -f "$c/V2-MASTER-STATE.md" ]; then
      (cd "$c" && pwd)
      return 0
    fi
  done
  return 1
}

# Extract highest §S-N header integer from a file (anchored regex per CAVEAT-B).
# Stdout: integer or empty. POSIX bracket class for cross-platform.
extract_highest_s_n() {
  local file="$1"
  [ -f "$file" ] || { echo ""; return 1; }
  # Anchored: ^## §S-(N)<spaces>—  per CAVEAT-B (em-dash required).
  grep -E '^## §S-[0-9]+[[:space:]]+—' "$file" 2>/dev/null \
    | sed -E 's/^## §S-([0-9]+)[[:space:]]+—.*/\1/' \
    | sort -n | tail -n 1
}

# Extract any §S-M references from a commit message body.
# Stdout: one integer per line (deduped + sorted descending).
extract_s_n_refs_from_msg() {
  local msg="$1"
  [ -z "$msg" ] && return 0
  # Find §S-N tokens; word-boundary on the right side.
  printf '%s' "$msg" | grep -oE '§S-[0-9]+' \
    | sed -E 's/§S-([0-9]+)/\1/' \
    | sort -nu -r
}

# -- Main hook --------------------------------------------------------------

# 1. Allowlist: commit-msg marker
COMMIT_MSG="$(read_commit_msg)"
if printf '%s' "$COMMIT_MSG" | grep -qF '[skip-state-check]'; then
  log_verbose "skip-state-check marker present in commit message; skipping"
  audit_append "skip-marker" "[skip-state-check] in commit msg"
  exit 0
fi

# 1b. Allowlist: all paths under build-artifact roots
if all_paths_allowlisted; then
  log_verbose "all staged paths under allowlist roots (.tmp/ / node_modules/ / target/); skipping"
  audit_append "skip-allowlist" "all paths build-artifact"
  exit 0
fi

# 2. BYPASS env var
if [ "${BYPASS_STATE_DIVERGENCE_CHECK:-}" = "1" ]; then
  log_warn "BYPASS_STATE_DIVERGENCE_CHECK=1 set; skipping check (audited)"
  audit_append "bypass-env" "BYPASS_STATE_DIVERGENCE_CHECK=1"
  exit 0
fi

# Edge: detached HEAD or non-git context. Silent no-op.
if ! git rev-parse --abbrev-ref HEAD >/dev/null 2>&1; then
  log_verbose "detached HEAD or non-git context; skipping"
  audit_append "skip-detached" "no current branch"
  exit 0
fi

# 3. git fetch origin main with 3s timeout, fail-open
fetch_outcome="ok"
fetch_err=""
if ! fetch_err="$(timeout 3 git fetch --quiet origin main 2>&1)"; then
  fetch_outcome="flake"
  log_warn "git fetch origin main timed out or failed (3s budget); proceeding fail-open"
  log_verbose "fetch_err: $fetch_err"
fi

# 4. Compare local HEAD vs origin/main behind-count
behind_count=0
if git rev-parse --verify origin/main >/dev/null 2>&1; then
  behind_count=$(git rev-list --count HEAD..origin/main 2>/dev/null || echo 0)
fi

if [ "$behind_count" -ge "$K_BEHIND" ]; then
  audit_append "block-behind" "behind=$behind_count threshold=$K_BEHIND fetch=$fetch_outcome"
  cat <<EOF >&2
[state-divergence] BLOCK: local branch is $behind_count commits behind origin/main (K_BEHIND=$K_BEHIND).
                  Stale state may corrupt downstream pulls.

  Recovery:
    git fetch origin main
    git rebase origin/main          # or git merge --ff-only origin/main
    # then retry the commit

  Override (rare; audited):
    BYPASS_STATE_DIVERGENCE_CHECK=1 git commit ...
    OR add [skip-state-check] to commit message

  Per-repo K override:
    STATE_DIVERGENCE_K_BEHIND=N git commit ...

  Audit log: $AUDIT_LOG
EOF
  exit 1
fi

# 5. Resolve comms-link sibling
COMMS_LINK_PATH=""
COMMS_LINK_PATH="$(resolve_comms_link 2>/dev/null || true)"

if [ -z "$COMMS_LINK_PATH" ]; then
  # Per CAVEAT-C: silent skip on missing sibling, with opt-in verbose surface.
  log_verbose "comms-link sibling not found at any candidate path; skipping §S-N validation"
  audit_append "pass-no-sibling" "behind=$behind_count fetch=$fetch_outcome"
  exit 0
fi

V2_STATE_FILE="$COMMS_LINK_PATH/V2-MASTER-STATE.md"
if [ ! -f "$V2_STATE_FILE" ]; then
  log_verbose "V2-MASTER-STATE.md not found at $V2_STATE_FILE; skipping §S-N validation"
  audit_append "pass-no-state-file" "behind=$behind_count fetch=$fetch_outcome path=$V2_STATE_FILE"
  exit 0
fi

# 6. Validate commit-msg §S-M references against local highest §S-N
local_highest="$(extract_highest_s_n "$V2_STATE_FILE")"
if [ -z "$local_highest" ]; then
  log_verbose "no §S-N headers parsed from $V2_STATE_FILE; skipping reference validation"
  audit_append "pass-no-headers" "behind=$behind_count file=$V2_STATE_FILE"
  exit 0
fi

# Collect §S-M refs from commit msg
mapfile -t refs < <(extract_s_n_refs_from_msg "$COMMIT_MSG")
forward_refs=()
for ref in "${refs[@]:-}"; do
  [ -z "$ref" ] && continue
  if [ "$ref" -gt "$local_highest" ]; then
    forward_refs+=("$ref")
  fi
done

if [ "${#forward_refs[@]}" -gt 0 ]; then
  audit_append "block-forward-ref" "behind=$behind_count refs=${forward_refs[*]} local_highest=$local_highest"
  cat <<EOF >&2
[state-divergence] BLOCK: commit message references §S-${forward_refs[*]} but local
                  V2-MASTER-STATE.md highest header is §S-$local_highest.

  This usually means either:
    (a) you cited a future §S-N (typo / wrong number)
    (b) you have stale local state (origin has §S-${forward_refs[*]} that you haven't pulled)
    (c) you are authoring §S-${forward_refs[*]} in this commit but haven't appended the header yet

  Recovery (most common — case b):
    git fetch origin main
    git rebase origin/main
    # then retry the commit

  Recovery (case c — authoring forward §S-N):
    Add the §S-${forward_refs[*]} header to V2-MASTER-STATE.md before commit
    OR add [skip-state-check] to commit msg if intentional out-of-sequence

  Override (rare; audited):
    BYPASS_STATE_DIVERGENCE_CHECK=1 git commit ...

  Audit log: $AUDIT_LOG
  Comms-link: $COMMS_LINK_PATH
EOF
  exit 1
fi

# All checks passed.
audit_append "pass" "behind=$behind_count local_highest=§S-$local_highest fetch=$fetch_outcome refs_count=${#refs[@]}"
log_verbose "pass (behind=$behind_count, local_highest=§S-$local_highest, refs=${#refs[@]}, fetch=$fetch_outcome)"
exit 0
