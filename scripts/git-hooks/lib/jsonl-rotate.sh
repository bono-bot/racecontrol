#!/bin/bash
# scripts/git-hooks/lib/jsonl-rotate.sh — Shared JSONL rotation helper.
#
# Provides rotate_jsonl_if_needed for any append-only JSONL ledger that needs
# growth bounds. Triggers rotation on EITHER (a) file size > 10 MB or
# (b) oldest entry timestamp > 90 days old.
#
# Authored as Phase 1 substrate of PACT-20260505-004 (P6 State-Divergence Pre-Commit
# Hook) per bono AMPLIFIER §AMEND-1 CAVEAT-A absorption: state-divergence-audit.jsonl
# needs a rotation/size cap from day 1; same helper reusable by RISK-2 memory-autolog
# ledger and any future audit JSONL family.
#
# Usage (from another hook script):
#   source "$(dirname "$0")/lib/jsonl-rotate.sh"
#   rotate_jsonl_if_needed "/path/to/data/foo.jsonl"
#
# Behaviour:
#   - File doesn't exist: return 0 (no-op; caller will create it)
#   - File < 10 MB AND oldest entry < 90d: return 0 (no rotation)
#   - Otherwise: gzip-archive to <file>-YYYY-MM-DD.jsonl.gz, truncate file to 0 lines
#
# Cross-platform: POSIX bash + standard gzip + date. Works on Linux + Git Bash.
#
# Bypass: ROTATE_JSONL_DISABLE=1 env var skips rotation (test scaffolding).

# Guard against double-source.
[ "${_JSONL_ROTATE_LOADED:-}" = "1" ] && return 0
_JSONL_ROTATE_LOADED=1

# 10 MB in bytes
_JSONL_ROTATE_SIZE_THRESHOLD_BYTES=$((10 * 1024 * 1024))

# 90 days in seconds
_JSONL_ROTATE_AGE_THRESHOLD_SECONDS=$((90 * 24 * 60 * 60))

rotate_jsonl_if_needed() {
  local file="$1"
  [ -z "$file" ] && { echo "[jsonl-rotate] usage: rotate_jsonl_if_needed <file>" >&2; return 2; }
  [ "${ROTATE_JSONL_DISABLE:-}" = "1" ] && return 0
  [ ! -f "$file" ] && return 0

  # Size check (cross-platform: stat -c on Linux, stat -f on BSD/Darwin; wc -c as fallback).
  local size_bytes
  size_bytes=$(stat -c '%s' "$file" 2>/dev/null) \
    || size_bytes=$(stat -f '%z' "$file" 2>/dev/null) \
    || size_bytes=$(wc -c < "$file" 2>/dev/null | tr -d ' ')

  local needs_rotate=0
  if [ -n "$size_bytes" ] && [ "$size_bytes" -gt "$_JSONL_ROTATE_SIZE_THRESHOLD_BYTES" ]; then
    needs_rotate=1
  fi

  # Age check: parse first line's "ts" field as ISO-8601, compare to now-90d.
  if [ "$needs_rotate" -eq 0 ] && [ -s "$file" ]; then
    local first_ts
    first_ts=$(head -n 1 "$file" 2>/dev/null | sed -n 's/.*"ts"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
    if [ -n "$first_ts" ]; then
      # date -d for GNU, date -j -f for BSD; both wrapped in subshell to silence errors.
      local first_epoch now_epoch
      first_epoch=$(date -d "$first_ts" '+%s' 2>/dev/null) \
        || first_epoch=$(date -j -f '%Y-%m-%dT%H:%M:%S' "${first_ts%.*}" '+%s' 2>/dev/null) \
        || first_epoch=""
      if [ -n "$first_epoch" ]; then
        now_epoch=$(date '+%s')
        local age=$((now_epoch - first_epoch))
        if [ "$age" -gt "$_JSONL_ROTATE_AGE_THRESHOLD_SECONDS" ]; then
          needs_rotate=1
        fi
      fi
    fi
  fi

  [ "$needs_rotate" -eq 0 ] && return 0

  # Rotate: gzip-archive sibling, truncate file.
  local archive
  archive="${file%.jsonl}-$(date '+%Y-%m-%d').jsonl.gz"
  if gzip -c "$file" > "$archive" 2>/dev/null; then
    : > "$file"
    [ "${STATE_DIVERGENCE_VERBOSE:-${JSONL_ROTATE_VERBOSE:-}}" = "1" ] \
      && echo "[jsonl-rotate] rotated $file -> $archive (size=$size_bytes)" >&2
    return 0
  else
    echo "[jsonl-rotate] WARN: gzip failed for $file; archive not created; ledger left intact" >&2
    return 1
  fi
}
