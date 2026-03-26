#!/usr/bin/env bash
# scripts/intelligence/self-patch.sh — LEARN-07/08/09: Self-Patch Loop
#
# Processes 'queued_for_selfpatch' proposals by modifying detector and healing
# scripts, verifying the patch, and committing. Auto-reverts if verification fails.
#
# Only processes ONE proposal per invocation (limits blast radius per run).
# Scoped to scripts/detectors/ and scripts/healing/ only — never modifies
# auto-detect.sh, cascade.sh, audit/lib/, or any file outside scripts/.
#
# Exports: self_patch_loop, _self_patch_enabled
#
# Dependencies (inherited from auto-detect.sh context OR set by caller):
#   REPO_ROOT       — repo root path
#   COMMS_PSK       — comms-link pre-shared key
#   COMMS_URL       — comms-link WebSocket URL
#
# Toggle: set self_patch_enabled=true in audit/results/auto-detect-config.json
# Default: DISABLED (self_patch_enabled=false — requires explicit enable)

set -uo pipefail
# NO set -e — all errors return codes, never hard-exit

# ─── Source guard ─────────────────────────────────────────────────────────────
[[ "${_SELF_PATCH_SOURCED:-}" == "1" ]] && return 0
_SELF_PATCH_SOURCED=1

# ─── Constants ────────────────────────────────────────────────────────────────
_SELF_PATCH_REPO_ROOT="${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

AUTO_DETECT_CONFIG="${_SELF_PATCH_REPO_ROOT}/audit/results/auto-detect-config.json"
PROPOSALS_DIR="${_SELF_PATCH_REPO_ROOT}/audit/results/proposals"
SUGGESTIONS_JSONL="${_SELF_PATCH_REPO_ROOT}/audit/results/suggestions.jsonl"
COMMS_LINK_DIR="${COMMS_LINK_DIR:-${_SELF_PATCH_REPO_ROOT}/../comms-link}"

# Scope restriction: ONLY these directories may be patched
ALLOWED_PATCH_DIRS=(
  "${_SELF_PATCH_REPO_ROOT}/scripts/detectors"
  "${_SELF_PATCH_REPO_ROOT}/scripts/healing"
)

# ─── Logging helpers ──────────────────────────────────────────────────────────
_sp_log() {
  local level="$1"; shift
  local ts
  ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')
  echo "[${ts}] [${level}] [self-patch] $*" >&2
}

# ─── _self_patch_enabled ──────────────────────────────────────────────────────
# Returns 0 if self-patching is enabled, 1 otherwise.
# Default: DISABLED (self_patch_enabled=false in config or config absent).
# Override: NO_SELFPATCH=true forces disabled regardless of config.
#
# Mirror of _auto_fix_enabled() pattern but with default=false (LEARN-09).
_self_patch_enabled() {
  if [[ "${NO_SELFPATCH:-false}" == "true" ]]; then return 1; fi
  # Missing config = disabled (default false — requires explicit enable)
  if [[ ! -f "$AUTO_DETECT_CONFIG" ]]; then return 1; fi
  local val
  val=$(jq -r '.self_patch_enabled // false' "$AUTO_DETECT_CONFIG" 2>/dev/null || echo "false")
  if [[ "$val" == "true" ]]; then return 0; fi
  return 1
}
export -f _self_patch_enabled

# ─── _sp_notify_bono_dual_channel ─────────────────────────────────────────────
# Sends dual-channel notification to Bono: WS + INBOX.md.
# Non-fatal — notification failure does not affect return code.
_sp_notify_bono_dual_channel() {
  local message="$1"

  # Channel 1: WS via send-message.js
  if [[ -n "${COMMS_PSK:-}" && -n "${COMMS_URL:-}" && -d "$COMMS_LINK_DIR" ]]; then
    (
      cd "$COMMS_LINK_DIR" 2>/dev/null || exit 0
      timeout 15 env COMMS_PSK="${COMMS_PSK}" COMMS_URL="${COMMS_URL}" \
        node send-message.js "$message" 2>/dev/null || true
    ) || true
  else
    _sp_log WARN "COMMS_PSK/COMMS_URL not set or comms-link missing — skipping WS notification"
  fi

  # Channel 2: INBOX.md + git push
  local inbox_file="${COMMS_LINK_DIR}/INBOX.md"
  if [[ -f "$inbox_file" ]]; then
    (
      local ts_header
      ts_header=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')
      {
        printf '\n## %s -- from james (self-patch)\n' "$ts_header"
        printf '%s\n' "$message"
      } >> "$inbox_file" 2>/dev/null || exit 0
      cd "$COMMS_LINK_DIR" 2>/dev/null || exit 0
      git add INBOX.md 2>/dev/null || exit 0
      git commit -m "notify: self-patch Bono notification" 2>/dev/null || exit 0
      git push 2>/dev/null || true
    ) || true
  else
    _sp_log WARN "INBOX.md not found at ${inbox_file} — skipping INBOX notification"
  fi

  return 0
}

# ─── _sp_update_proposal_status ───────────────────────────────────────────────
# Updates .status field in a proposal JSON file in-place using jq.
# Args: file_path new_status
_sp_update_proposal_status() {
  local file="$1"
  local new_status="$2"

  local tmp
  tmp=$(mktemp) || return 1
  if jq --arg s "$new_status" '.status = $s' "$file" > "$tmp" 2>/dev/null; then
    mv "$tmp" "$file" 2>/dev/null && return 0
  fi
  rm -f "$tmp"
  return 1
}

# ─── _sp_log_suggestions_jsonl ────────────────────────────────────────────────
# Appends a structured entry to suggestions.jsonl.
# Args: entry_type proposal_id bug_type pod_ip status [extra_key extra_val ...]
_sp_log_suggestions_jsonl() {
  local entry_type="$1"
  local proposal_id="$2"
  local bug_type="$3"
  local pod_ip="$4"
  local status="$5"
  # Optional: target_file, reason
  local target_file="${6:-}"
  local reason="${7:-}"

  local run_ts
  run_ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')

  mkdir -p "$(dirname "$SUGGESTIONS_JSONL")" 2>/dev/null || true

  local entry
  entry=$(jq -n \
    --arg run_ts        "$run_ts"       \
    --arg entry_type    "$entry_type"   \
    --arg proposal_id   "$proposal_id"  \
    --arg bug_type      "$bug_type"     \
    --arg pod_ip        "$pod_ip"       \
    --arg status        "$status"       \
    --arg target_file   "$target_file"  \
    --arg reason        "$reason"       \
    '{run_ts:$run_ts,entry_type:$entry_type,proposal_id:$proposal_id,bug_type:$bug_type,pod_ip:$pod_ip,status:$status,target_file:$target_file,reason:$reason}' \
    2>/dev/null || true)

  if [[ -n "$entry" ]]; then
    printf '%s\n' "$entry" >> "$SUGGESTIONS_JSONL" 2>/dev/null || true
  fi
}

# ─── _sp_is_in_allowed_dirs ───────────────────────────────────────────────────
# Returns 0 if the given file is under one of ALLOWED_PATCH_DIRS, 1 otherwise.
# Uses realpath for canonical comparison to prevent path traversal tricks.
_sp_is_in_allowed_dirs() {
  local target="$1"

  local real_target
  real_target=$(realpath "$target" 2>/dev/null || echo "$target")

  local allowed_dir
  for allowed_dir in "${ALLOWED_PATCH_DIRS[@]}"; do
    local real_allowed
    real_allowed=$(realpath "$allowed_dir" 2>/dev/null || echo "$allowed_dir")
    if [[ "$real_target" == "$real_allowed"/* ]]; then
      return 0
    fi
  done

  return 1
}

# ─── self_patch_loop ──────────────────────────────────────────────────────────
# Main entry point. Processes ONE queued_for_selfpatch proposal per invocation.
#
# CE Methodology (from CLAUDE.md Debugging Methodology):
#   Step 1 — Document symptom (bug_type + evidence from proposal)
#   Step 2/3 — Identify patch target in ALLOWED_PATCH_DIRS
#   Step 4a — Apply patch (threshold variables only, sed in-place)
#   Step 4b — Verify patch (bash -n syntax check + threshold presence)
#   Step 5 — Commit+push on success, revert+log on failure
#
# Returns 0 always (errors are non-fatal, logged to suggestions.jsonl)
self_patch_loop() {
  # ── Toggle gate: LEARN-09 — default disabled ────────────────────────────────
  if ! _self_patch_enabled; then
    _sp_log INFO "[LEARN-09] self-patch disabled — skipping (set self_patch_enabled=true in auto-detect-config.json to enable)"
    return 0
  fi

  # ── Find queued proposals ───────────────────────────────────────────────────
  if [[ ! -d "$PROPOSALS_DIR" ]]; then
    _sp_log INFO "[LEARN-07] self_patch_loop: proposals directory absent — no queued proposals"
    return 0
  fi

  # Scan for first proposal with status == queued_for_selfpatch
  local proposal_file=""
  local found_file
  for found_file in "$PROPOSALS_DIR"/*.json; do
    [[ -f "$found_file" ]] || continue
    local pstatus
    pstatus=$(jq -r '.status // ""' "$found_file" 2>/dev/null || echo "")
    if [[ "$pstatus" == "queued_for_selfpatch" ]]; then
      proposal_file="$found_file"
      break
    fi
  done

  if [[ -z "$proposal_file" ]]; then
    _sp_log INFO "[LEARN-07] self_patch_loop: no queued proposals"
    return 0
  fi

  # ── Extract proposal fields ─────────────────────────────────────────────────
  local proposal_id bug_type pod_ip evidence category
  proposal_id=$(jq -r '.id // "unknown"'       "$proposal_file" 2>/dev/null || echo "unknown")
  bug_type=$(jq -r '.bug_type // "unknown"'    "$proposal_file" 2>/dev/null || echo "unknown")
  pod_ip=$(jq -r '.pod_ip // "unknown"'        "$proposal_file" 2>/dev/null || echo "unknown")
  evidence=$(jq -r '.evidence // ""'           "$proposal_file" 2>/dev/null || echo "")
  category=$(jq -r '.category // "unknown"'   "$proposal_file" 2>/dev/null || echo "unknown")

  _sp_log INFO "[LEARN-07] processing proposal: ${proposal_id} (${category}) for ${bug_type} on ${pod_ip}"

  # ── CE Step 1 — Document symptom ────────────────────────────────────────────
  _sp_log_suggestions_jsonl "SELFPATCH_ATTEMPT" "$proposal_id" "$bug_type" "$pod_ip" "started"

  # ── CE Step 2/3 — Identify patch target in allowed dirs ─────────────────────
  local target_file=""

  # Search ALLOWED_PATCH_DIRS for files mentioning bug_type
  local search_result
  search_result=$(grep -rl "$bug_type" "${ALLOWED_PATCH_DIRS[@]}" 2>/dev/null | head -1 || true)
  if [[ -n "$search_result" ]]; then
    target_file="$search_result"
  fi

  # If no exact match, try closest related term (strip underscores, partial match)
  if [[ -z "$target_file" ]]; then
    local bug_slug="${bug_type//_/-}"
    search_result=$(grep -rl "$bug_slug" "${ALLOWED_PATCH_DIRS[@]}" 2>/dev/null | head -1 || true)
    if [[ -n "$search_result" ]]; then
      target_file="$search_result"
    fi
  fi

  # Also try direct name match: detect-${bug_type_slug}.sh
  if [[ -z "$target_file" ]]; then
    local direct_slug="${bug_type//_/-}"
    local direct_path="${_SELF_PATCH_REPO_ROOT}/scripts/detectors/detect-${direct_slug}.sh"
    if [[ -f "$direct_path" ]]; then
      target_file="$direct_path"
    fi
  fi

  if [[ -z "$target_file" ]]; then
    _sp_log WARN "[LEARN-08] self_patch_loop: no target file found for bug_type=${bug_type} — skipping proposal ${proposal_id}"
    _sp_update_proposal_status "$proposal_file" "no_patch_pattern" || true
    return 0
  fi

  # ── SAFETY CHECK: verify target is under ALLOWED_PATCH_DIRS ─────────────────
  if ! _sp_is_in_allowed_dirs "$target_file"; then
    _sp_log WARN "[LEARN-08] patch target outside allowed scope: ${target_file} — skipping proposal ${proposal_id}"
    _sp_update_proposal_status "$proposal_file" "patch_scope_rejected" || true
    return 0
  fi

  _sp_log INFO "[LEARN-08] patch target confirmed in allowed scope: ${target_file}"

  # ── CE Step 4a — Find patchable threshold variable ──────────────────────────
  # Only patch threshold/count variables (not new code generation)
  local threshold_line var_name old_threshold new_threshold
  threshold_line=$(grep -inE '(max_[a-z_]+=|[a-z_]*threshold[a-z_]*=)[0-9]+' "$target_file" 2>/dev/null | head -1 || true)

  if [[ -z "$threshold_line" ]]; then
    _sp_log WARN "[LEARN-08] self_patch_loop: no patchable threshold pattern found in ${target_file} — skipping proposal ${proposal_id}"
    _sp_update_proposal_status "$proposal_file" "no_patch_pattern" || true
    return 0
  fi

  old_threshold=$(echo "$threshold_line" | grep -oE '[0-9]+$' | head -1 || echo "")
  if [[ -z "$old_threshold" || ! "$old_threshold" =~ ^[0-9]+$ ]]; then
    _sp_log WARN "[LEARN-08] self_patch_loop: could not parse threshold value from line: ${threshold_line}"
    _sp_update_proposal_status "$proposal_file" "no_patch_pattern" || true
    return 0
  fi

  # Increment by 20% (round up to ensure actual increase for small base values)
  new_threshold=$(awk "BEGIN { t = $old_threshold * 1.2; printf \"%d\", (t == int(t)) ? t : int(t) + 1 }" 2>/dev/null || echo "$((old_threshold + 1))")

  var_name=$(echo "$threshold_line" | grep -oE '[a-zA-Z_]+=' | head -1 | tr -d '=')
  if [[ -z "$var_name" ]]; then
    _sp_log WARN "[LEARN-08] self_patch_loop: could not extract variable name from: ${threshold_line}"
    _sp_update_proposal_status "$proposal_file" "no_patch_pattern" || true
    return 0
  fi

  _sp_log INFO "[LEARN-07] applying patch: ${var_name}=${old_threshold} -> ${new_threshold} in ${target_file}"

  # ── Record pre-patch git hash for revert reference ───────────────────────────
  local pre_patch_hash
  pre_patch_hash=$(git -C "$_SELF_PATCH_REPO_ROOT" rev-parse HEAD 2>/dev/null || echo "unknown")

  # ── Apply sed in-place change ────────────────────────────────────────────────
  sed -i "s/\(${var_name}=\)[0-9]*/\1${new_threshold}/" "$target_file" 2>/dev/null || {
    _sp_log WARN "[LEARN-08] sed failed on ${target_file} — aborting patch"
    _sp_update_proposal_status "$proposal_file" "no_patch_pattern" || true
    return 0
  }

  # ── CE Step 4b — Verify the patch ───────────────────────────────────────────
  # Minimum: bash -n syntax check + verify new threshold value present in file
  local verify_ok=0
  if ! bash -n "$target_file" 2>/dev/null; then
    _sp_log WARN "[LEARN-08] syntax check (bash -n) failed after patch"
    verify_ok=1
  elif ! grep -q "${var_name}=${new_threshold}" "$target_file" 2>/dev/null; then
    _sp_log WARN "[LEARN-08] threshold verification failed — ${var_name}=${new_threshold} not found in ${target_file}"
    verify_ok=1
  fi

  # ── REVERT if verification failed ────────────────────────────────────────────
  if [[ "$verify_ok" -ne 0 ]]; then
    _sp_log WARN "[LEARN-08] self-patch REVERTED for ${bug_type}: verification failed after modifying ${target_file}"

    # Restore file from git
    git -C "$_SELF_PATCH_REPO_ROOT" checkout -- "$target_file" 2>/dev/null || {
      _sp_log WARN "[LEARN-08] git checkout revert also failed for ${target_file} — pre_patch_hash: ${pre_patch_hash}"
    }

    # Log SELFPATCH_REVERTED to suggestions.jsonl
    local revert_ts
    revert_ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')
    local revert_entry
    revert_entry=$(jq -n \
      --arg run_ts        "$revert_ts"           \
      --arg entry_type    "SELFPATCH_REVERTED"   \
      --arg proposal_id   "$proposal_id"         \
      --arg bug_type      "$bug_type"            \
      --arg pod_ip        "$pod_ip"              \
      --arg reason        "verification_failed"  \
      --arg target_file   "$target_file"         \
      --arg pre_patch_hash "$pre_patch_hash"     \
      '{run_ts:$run_ts,entry_type:$entry_type,proposal_id:$proposal_id,bug_type:$bug_type,pod_ip:$pod_ip,reason:$reason,target_file:$target_file,pre_patch_hash:$pre_patch_hash}' \
      2>/dev/null || true)
    if [[ -n "$revert_entry" ]]; then
      printf '%s\n' "$revert_entry" >> "$SUGGESTIONS_JSONL" 2>/dev/null || true
    fi

    # Update proposal status to patch_reverted
    _sp_update_proposal_status "$proposal_file" "patch_reverted" || true

    # Notify Bono: self-patch reverted
    _sp_notify_bono_dual_channel "Self-patch REVERTED for ${bug_type}: verification failed after modifying ${target_file}. Proposal: ${proposal_id}"

    return 0
  fi

  # ── CE Step 5 — Commit and notify on success ─────────────────────────────────
  git -C "$_SELF_PATCH_REPO_ROOT" add "$target_file" 2>/dev/null || {
    _sp_log WARN "[LEARN-07] git add failed for ${target_file}"
    return 0
  }

  local commit_msg="selfpatch(215): apply threshold tune for ${bug_type} — proposal ${proposal_id}"
  if git -C "$_SELF_PATCH_REPO_ROOT" commit -m "$commit_msg" 2>/dev/null; then
    # Push to remote
    git -C "$_SELF_PATCH_REPO_ROOT" push 2>/dev/null || \
      _sp_log WARN "[LEARN-07] git push failed after self-patch — changes committed locally"
  else
    _sp_log WARN "[LEARN-07] git commit failed (nothing staged or commit error) — continuing"
  fi

  # Update proposal status to patch_applied
  _sp_update_proposal_status "$proposal_file" "patch_applied" || true

  # Log SELFPATCH_APPLIED to suggestions.jsonl
  local applied_ts
  applied_ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')
  local applied_entry
  applied_entry=$(jq -n \
    --arg run_ts      "$applied_ts"        \
    --arg entry_type  "SELFPATCH_APPLIED"  \
    --arg proposal_id "$proposal_id"       \
    --arg bug_type    "$bug_type"          \
    --arg pod_ip      "$pod_ip"            \
    --arg status      "applied"            \
    --arg target_file "$target_file"       \
    --argjson old_threshold "$old_threshold" \
    --argjson new_threshold "$new_threshold" \
    '{run_ts:$run_ts,entry_type:$entry_type,proposal_id:$proposal_id,bug_type:$bug_type,pod_ip:$pod_ip,status:$status,target_file:$target_file,old_threshold:$old_threshold,new_threshold:$new_threshold}' \
    2>/dev/null || true)
  if [[ -n "$applied_entry" ]]; then
    printf '%s\n' "$applied_entry" >> "$SUGGESTIONS_JSONL" 2>/dev/null || true
  fi

  # Notify Bono dual-channel: self-patch applied
  _sp_notify_bono_dual_channel "Self-patch applied: ${bug_type} threshold adjusted (${var_name}: ${old_threshold} -> ${new_threshold}) in ${target_file}. Proposal: ${proposal_id}"

  _sp_log INFO "[LEARN-07] self-patch applied: ${proposal_id} — ${var_name}=${old_threshold} -> ${new_threshold} in ${target_file}"

  # Process only ONE proposal per loop invocation (blast radius limit)
  return 0
}

export -f self_patch_loop
