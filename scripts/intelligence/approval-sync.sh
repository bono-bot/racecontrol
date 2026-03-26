#!/usr/bin/env bash
# scripts/intelligence/approval-sync.sh — LEARN-05: Approval Sync Layer
#
# Applies approved suggestions from the proposal inbox to their correct target files.
# Handles 6 proposal categories: threshold_tune, new_autofix_candidate, standing_rule_gap,
# cascade_coverage_gap, new_audit_check, self_patch.
#
# Every applied change is:
#   1. Written to its target file (detector / config / registry / suppress.json)
#   2. git add + git commit + git push
#   3. Bono notified via dual-channel (WS + INBOX.md)
#
# Exports: approve_suggestion, apply_approved_suggestion
#
# Dependencies (inherited from auto-detect.sh context OR set by caller):
#   REPO_ROOT       — repo root path
#   COMMS_PSK       — comms-link pre-shared key
#   COMMS_URL       — comms-link WebSocket URL
#
# Usage:
#   source scripts/intelligence/approval-sync.sh
#   approve_suggestion "<proposal_id>"              # pending → approved → applies
#   apply_approved_suggestion "<proposal_id>"       # re-apply if already approved

set -uo pipefail
# NO set -e — all errors return codes, never hard-exit

# ─── Source guard ─────────────────────────────────────────────────────────────
[[ "${_APPROVAL_SYNC_SOURCED:-}" == "1" ]] && return 0
_APPROVAL_SYNC_SOURCED=1

# ─── Constants ────────────────────────────────────────────────────────────────
_APPROVAL_SYNC_REPO_ROOT="${REPO_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)}"

PROPOSALS_DIR="${_APPROVAL_SYNC_REPO_ROOT}/audit/results/proposals"
SUPPRESS_JSON="${_APPROVAL_SYNC_REPO_ROOT}/audit/results/suppress.json"
AUTO_DETECT_CONFIG="${_APPROVAL_SYNC_REPO_ROOT}/audit/results/auto-detect-config.json"
STANDING_RULES_REGISTRY="${_APPROVAL_SYNC_REPO_ROOT}/standing-rules-registry.json"
DETECTORS_DIR="${_APPROVAL_SYNC_REPO_ROOT}/scripts/detectors"
COMMS_LINK_DIR="${COMMS_LINK_DIR:-${_APPROVAL_SYNC_REPO_ROOT}/../comms-link}"

# ─── Logging helpers ──────────────────────────────────────────────────────────
_as_log() {
  local level="$1"; shift
  local ts
  ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')
  echo "[${ts}] [${level}] [approval-sync] $*" >&2
}

# ─── _find_proposal_file ──────────────────────────────────────────────────────
# Resolves proposal_id to a file path.
# Tries exact match first, then glob *${id}*.json.
# Returns file path via stdout. Returns 1 if not found.
_find_proposal_file() {
  local proposal_id="$1"

  # Exact match
  if [[ -f "${PROPOSALS_DIR}/${proposal_id}.json" ]]; then
    echo "${PROPOSALS_DIR}/${proposal_id}.json"
    return 0
  fi

  # Glob match (partial ID or different prefix)
  local found
  found=$(ls "${PROPOSALS_DIR}"/*"${proposal_id}"*.json 2>/dev/null | head -1 || true)
  if [[ -n "$found" && -f "$found" ]]; then
    echo "$found"
    return 0
  fi

  return 1
}

# ─── _update_proposal_status ──────────────────────────────────────────────────
# Updates .status field in a proposal JSON file in-place using jq.
# Args: file_path new_status
# Returns 0 on success, 1 on failure.
_update_proposal_status() {
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

# ─── _notify_bono_dual_channel ────────────────────────────────────────────────
# Sends dual-channel notification to Bono: WS + INBOX.md.
# Non-fatal — notification failure does not affect return code.
# Args: message
_notify_bono_dual_channel() {
  local message="$1"

  # Channel 1: WS via send-message.js
  if [[ -n "${COMMS_PSK:-}" && -n "${COMMS_URL:-}" && -d "$COMMS_LINK_DIR" ]]; then
    (
      cd "$COMMS_LINK_DIR" 2>/dev/null || exit 0
      timeout 15 env COMMS_PSK="${COMMS_PSK}" COMMS_URL="${COMMS_URL}" \
        node send-message.js "$message" 2>/dev/null || true
    ) || true
  else
    _as_log WARN "COMMS_PSK/COMMS_URL not set or comms-link missing — skipping WS notification"
  fi

  # Channel 2: INBOX.md + git push
  local inbox_file="${COMMS_LINK_DIR}/INBOX.md"
  if [[ -f "$inbox_file" ]]; then
    (
      local ts_header
      ts_header=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')
      {
        printf '\n## %s -- from james (approval-sync)\n' "$ts_header"
        printf '%s\n' "$message"
      } >> "$inbox_file" 2>/dev/null || exit 0
      cd "$COMMS_LINK_DIR" 2>/dev/null || exit 0
      git add INBOX.md 2>/dev/null || exit 0
      git commit -m "notify: approval-sync Bono notification" 2>/dev/null || exit 0
      git push 2>/dev/null || true
    ) || true
  else
    _as_log WARN "INBOX.md not found at ${inbox_file} — skipping INBOX notification"
  fi

  return 0
}

# ─── _apply_threshold_tune ────────────────────────────────────────────────────
# Finds the detector script for bug_type, increments threshold by 20%.
# Also updates threshold_overrides in auto-detect-config.json.
# Returns 0 on success, 1 on failure.
_apply_threshold_tune() {
  local bug_type="$1"

  # Find the detector script that handles this bug_type
  # Naming convention: detect-${bug_type_slug}.sh (with hyphens, not underscores)
  local bug_slug="${bug_type//_/-}"
  local detector_file=""

  # Try direct match first
  if [[ -f "${DETECTORS_DIR}/detect-${bug_slug}.sh" ]]; then
    detector_file="${DETECTORS_DIR}/detect-${bug_slug}.sh"
  else
    # Search for any detector mentioning bug_type in its content (loose match)
    local found_detector
    found_detector=$(grep -rl "$bug_type" "${DETECTORS_DIR}"/*.sh 2>/dev/null | head -1 || true)
    if [[ -n "$found_detector" ]]; then
      detector_file="$found_detector"
    fi
  fi

  local new_threshold=4  # default if no detector found

  if [[ -n "$detector_file" ]]; then
    # Find threshold variable: max_restarts=N, THRESHOLD=N, _THRESHOLD=N, threshold=N
    local threshold_line
    threshold_line=$(grep -inE '(max_[a-z_]+=|[a-z_]*threshold[a-z_]*=)[0-9]+' "$detector_file" 2>/dev/null | head -1 || true)

    if [[ -n "$threshold_line" ]]; then
      local old_threshold
      old_threshold=$(echo "$threshold_line" | grep -oE '[0-9]+$' | head -1 || echo "")

      if [[ -n "$old_threshold" && "$old_threshold" =~ ^[0-9]+$ ]]; then
        # Increment by 20%, round to nearest integer
        new_threshold=$(awk "BEGIN { t = $old_threshold * 1.2; printf \"%d\", (t == int(t)) ? t : int(t) + 1 }" 2>/dev/null || echo "$((old_threshold + 1))")

        # Apply sed update in place — replace first occurrence of the threshold line
        local var_name
        var_name=$(echo "$threshold_line" | grep -oE '[a-zA-Z_]+=' | head -1 | tr -d '=')

        if [[ -n "$var_name" ]]; then
          # Use sed with backup to update the variable value
          sed -i "s/\(${var_name}=\)[0-9]*/\1${new_threshold}/" "$detector_file" 2>/dev/null || true
          _as_log INFO "threshold_tune: ${detector_file} — ${var_name} ${old_threshold} -> ${new_threshold}"
        fi
      fi
    fi
  else
    _as_log WARN "threshold_tune: no detector script found for bug_type=${bug_type} — updating config only"
  fi

  # Update auto-detect-config.json: add/update threshold_overrides.${bug_type}
  if [[ ! -f "$AUTO_DETECT_CONFIG" ]]; then
    echo '{"auto_fix_enabled":true,"wol_enabled":false}' > "$AUTO_DETECT_CONFIG" 2>/dev/null || true
  fi

  local tmp_config
  tmp_config=$(mktemp) || return 1

  jq --arg bt "$bug_type" --argjson val "$new_threshold" \
    '.threshold_overrides = (.threshold_overrides // {}) | .threshold_overrides[$bt] = $val' \
    "$AUTO_DETECT_CONFIG" > "$tmp_config" 2>/dev/null

  if [[ $? -eq 0 ]]; then
    mv "$tmp_config" "$AUTO_DETECT_CONFIG" 2>/dev/null || { rm -f "$tmp_config"; return 1; }
  else
    rm -f "$tmp_config"
    return 1
  fi

  # Stage files for commit
  if [[ -n "$detector_file" ]]; then
    git -C "${_APPROVAL_SYNC_REPO_ROOT}" add "$detector_file" 2>/dev/null || true
  fi
  git -C "${_APPROVAL_SYNC_REPO_ROOT}" add "$AUTO_DETECT_CONFIG" 2>/dev/null || true

  return 0
}

# ─── _apply_new_autofix_candidate ─────────────────────────────────────────────
# Appends bug_type to auto-detect-config.json "approved_auto_fixes" array.
# Creates array if absent.
# Returns 0 on success, 1 on failure.
_apply_new_autofix_candidate() {
  local bug_type="$1"

  if [[ ! -f "$AUTO_DETECT_CONFIG" ]]; then
    echo '{"auto_fix_enabled":true,"wol_enabled":false}' > "$AUTO_DETECT_CONFIG" 2>/dev/null || true
  fi

  local tmp_config
  tmp_config=$(mktemp) || return 1

  # Add bug_type to approved_auto_fixes array (deduplicated)
  jq --arg bt "$bug_type" \
    '.approved_auto_fixes = ((.approved_auto_fixes // []) | if index($bt) then . else . + [$bt] end)' \
    "$AUTO_DETECT_CONFIG" > "$tmp_config" 2>/dev/null

  if [[ $? -eq 0 ]]; then
    mv "$tmp_config" "$AUTO_DETECT_CONFIG" 2>/dev/null || { rm -f "$tmp_config"; return 1; }
  else
    rm -f "$tmp_config"
    return 1
  fi

  git -C "${_APPROVAL_SYNC_REPO_ROOT}" add "$AUTO_DETECT_CONFIG" 2>/dev/null || true
  return 0
}

# ─── _apply_standing_rule_gap ─────────────────────────────────────────────────
# Appends an entry to standing-rules-registry.json.
# Creates the registry file as [] if it does not exist.
# Returns 0 on success, 1 on failure.
_apply_standing_rule_gap() {
  local bug_type="$1"
  local pod_ip="$2"
  local evidence="$3"

  # Create registry if absent
  if [[ ! -f "$STANDING_RULES_REGISTRY" ]]; then
    echo '[]' > "$STANDING_RULES_REGISTRY" 2>/dev/null || return 1
  fi

  # Generate next SR ID
  local current_count
  current_count=$(jq 'length' "$STANDING_RULES_REGISTRY" 2>/dev/null || echo "0")
  local next_num=$(( current_count + 1 ))
  local new_id
  new_id=$(printf "SR-LEARNED-%03d" "$next_num")

  local added_ts
  added_ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')

  local tmp_registry
  tmp_registry=$(mktemp) || return 1

  jq --arg id        "$new_id"    \
     --arg rule      "Address recurring ${bug_type} on ${pod_ip}" \
     --arg why       "$evidence"  \
     --arg added_ts  "$added_ts"  \
     '. + [{
        "id": $id,
        "category": "process",
        "rule": $rule,
        "why": $why,
        "added_ts": $added_ts,
        "source": "suggestion_engine"
      }]' \
    "$STANDING_RULES_REGISTRY" > "$tmp_registry" 2>/dev/null

  if [[ $? -eq 0 ]]; then
    mv "$tmp_registry" "$STANDING_RULES_REGISTRY" 2>/dev/null || { rm -f "$tmp_registry"; return 1; }
  else
    rm -f "$tmp_registry"
    return 1
  fi

  git -C "${_APPROVAL_SYNC_REPO_ROOT}" add "$STANDING_RULES_REGISTRY" 2>/dev/null || true
  return 0
}

# ─── _apply_cascade_coverage_gap ──────────────────────────────────────────────
# Appends a pending_cascade_review entry to suppress.json (audit/results/suppress.json).
# Creates suppress.json as [] if absent.
# Returns 0 on success, 1 on failure.
_apply_cascade_coverage_gap() {
  local bug_type="$1"
  local pod_ip="$2"

  # Initialize suppress.json if absent
  if [[ ! -f "$SUPPRESS_JSON" ]]; then
    echo '[]' > "$SUPPRESS_JSON" 2>/dev/null || return 1
  fi

  # Compute expires_at = 7 days from now (ISO date)
  local expires_at
  expires_at=$(TZ=Asia/Kolkata date -d "+7 days" '+%Y-%m-%d' 2>/dev/null \
    || TZ=Asia/Kolkata date -v+7d '+%Y-%m-%d' 2>/dev/null \
    || date -d "+7 days" '+%Y-%m-%d' 2>/dev/null \
    || echo "")

  local created_ts
  created_ts=$(TZ=Asia/Kolkata date '+%Y-%m-%d %H:%M IST' 2>/dev/null || date '+%Y-%m-%d %H:%M')

  local tmp_suppress
  tmp_suppress=$(mktemp) || return 1

  jq --arg issue_type   "$bug_type"                           \
     --arg pod_ip       "$pod_ip"                             \
     --arg reason       "cascade_coverage_gap_pending_review" \
     --arg expires_at   "${expires_at:-}"                     \
     --arg created_by   "suggestion_engine"                   \
     --arg created_ts   "$created_ts"                         \
     '. + [{
        "issue_type": $issue_type,
        "pod_ip": $pod_ip,
        "reason": $reason,
        "expires_at": $expires_at,
        "created_by": $created_by,
        "created_ts": $created_ts
      }]' \
    "$SUPPRESS_JSON" > "$tmp_suppress" 2>/dev/null

  if [[ $? -eq 0 ]]; then
    mv "$tmp_suppress" "$SUPPRESS_JSON" 2>/dev/null || { rm -f "$tmp_suppress"; return 1; }
  else
    rm -f "$tmp_suppress"
    return 1
  fi

  git -C "${_APPROVAL_SYNC_REPO_ROOT}" add "$SUPPRESS_JSON" 2>/dev/null || true
  return 0
}

# ─── apply_approved_suggestion ────────────────────────────────────────────────
# Reads an approved proposal by ID, dispatches to correct apply function,
# commits + pushes changes, notifies Bono, updates status to "applied".
#
# Args: proposal_id
# Returns 0 on success, 1 on failure (partial failures logged, status → apply_failed)
apply_approved_suggestion() {
  local proposal_id="$1"

  if [[ -z "$proposal_id" ]]; then
    _as_log WARN "apply_approved_suggestion: called with empty proposal_id"
    return 1
  fi

  # Find proposal file
  local proposal_file
  if ! proposal_file=$(_find_proposal_file "$proposal_id"); then
    _as_log WARN "apply_approved_suggestion: proposal file not found for id=${proposal_id}"
    return 1
  fi

  # Extract proposal fields
  local category bug_type pod_ip evidence confidence
  category=$(jq -r '.category // "unknown"'  "$proposal_file" 2>/dev/null || echo "unknown")
  bug_type=$(jq -r '.bug_type // "unknown"'  "$proposal_file" 2>/dev/null || echo "unknown")
  pod_ip=$(jq -r '.pod_ip // "unknown"'      "$proposal_file" 2>/dev/null || echo "unknown")
  evidence=$(jq -r '.evidence // ""'          "$proposal_file" 2>/dev/null || echo "")
  confidence=$(jq -r '.confidence // "0.00"' "$proposal_file" 2>/dev/null || echo "0.00")

  _as_log INFO "[LEARN-05] applying: ${proposal_id} (${category}) for ${bug_type} on ${pod_ip}"

  mkdir -p "${_APPROVAL_SYNC_REPO_ROOT}/audit/results" 2>/dev/null || true

  local apply_ok=1  # 0 = success, 1 = failure

  case "$category" in

    threshold_tune)
      if _apply_threshold_tune "$bug_type"; then
        apply_ok=0
      else
        _as_log WARN "threshold_tune: apply failed for bug_type=${bug_type}"
      fi
      ;;

    new_autofix_candidate)
      if _apply_new_autofix_candidate "$bug_type"; then
        apply_ok=0
      else
        _as_log WARN "new_autofix_candidate: apply failed for bug_type=${bug_type}"
      fi
      ;;

    standing_rule_gap)
      if _apply_standing_rule_gap "$bug_type" "$pod_ip" "$evidence"; then
        apply_ok=0
      else
        _as_log WARN "standing_rule_gap: apply failed for bug_type=${bug_type}"
      fi
      ;;

    cascade_coverage_gap)
      if _apply_cascade_coverage_gap "$bug_type" "$pod_ip"; then
        apply_ok=0
      else
        _as_log WARN "cascade_coverage_gap: apply failed for bug_type=${bug_type}"
      fi
      ;;

    new_audit_check|self_patch)
      # These require the self-patch loop (LEARN-07, covered in 215-04).
      # Queue for self-patch loop — do not silently drop.
      _as_log INFO "[LEARN-05] category=${category} — queued for self-patch loop"
      _update_proposal_status "$proposal_file" "queued_for_selfpatch" || true
      return 0
      ;;

    *)
      _as_log WARN "apply_approved_suggestion: unknown category=${category} for proposal ${proposal_id}"
      _update_proposal_status "$proposal_file" "apply_failed" || true
      return 1
      ;;
  esac

  # Handle apply failure
  if [[ "$apply_ok" -ne 0 ]]; then
    _update_proposal_status "$proposal_file" "apply_failed" || true
    return 1
  fi

  # Stage and commit the applied changes
  git -C "${_APPROVAL_SYNC_REPO_ROOT}" add "$proposal_file" 2>/dev/null || true

  local commit_msg="learn(215): apply approved suggestion ${proposal_id} — ${category} for ${bug_type}"
  if git -C "${_APPROVAL_SYNC_REPO_ROOT}" commit -m "$commit_msg" 2>/dev/null; then
    # Push to remote
    git -C "${_APPROVAL_SYNC_REPO_ROOT}" push 2>/dev/null || \
      _as_log WARN "git push failed after apply — changes committed locally"
  else
    _as_log WARN "git commit failed (nothing staged or commit error) — continuing"
  fi

  # Update proposal status to "applied"
  _update_proposal_status "$proposal_file" "applied" || \
    _as_log WARN "failed to update proposal status to applied"

  # Commit status update
  git -C "${_APPROVAL_SYNC_REPO_ROOT}" add "$proposal_file" 2>/dev/null || true
  git -C "${_APPROVAL_SYNC_REPO_ROOT}" commit -m "learn(215): mark proposal ${proposal_id} as applied" 2>/dev/null || true
  git -C "${_APPROVAL_SYNC_REPO_ROOT}" push 2>/dev/null || true

  # Notify Bono via dual-channel
  local notify_msg="Suggestion applied — ${category}: ${bug_type} on ${pod_ip}. Evidence: ${evidence}. Confidence: ${confidence}."
  _notify_bono_dual_channel "$notify_msg"

  _as_log INFO "[LEARN-05] applied: ${proposal_id} (${category})"
  return 0
}

export -f apply_approved_suggestion

# ─── approve_suggestion ───────────────────────────────────────────────────────
# Validates a pending proposal by ID, sets status to "approved", then calls
# apply_approved_suggestion to execute the change.
#
# Args: proposal_id
# Returns 0 on success (applied or queued), 1 on validation failure or apply failure
approve_suggestion() {
  local proposal_id="$1"

  if [[ -z "$proposal_id" ]]; then
    _as_log WARN "approve_suggestion: called with empty proposal_id"
    return 1
  fi

  # Find proposal file
  local proposal_file
  if ! proposal_file=$(_find_proposal_file "$proposal_id"); then
    _as_log WARN "approve_suggestion: proposal not found for id=${proposal_id}"
    return 1
  fi

  # Validate status must be "pending"
  local current_status
  current_status=$(jq -r '.status // "unknown"' "$proposal_file" 2>/dev/null || echo "unknown")

  if [[ "$current_status" != "pending" ]]; then
    _as_log WARN "approve_suggestion: proposal ${proposal_id} is already '${current_status}' — skipping"
    return 1
  fi

  # Update status to "approved"
  if ! _update_proposal_status "$proposal_file" "approved"; then
    _as_log WARN "approve_suggestion: failed to update status to approved for ${proposal_id}"
    return 1
  fi

  _as_log INFO "approve_suggestion: ${proposal_id} status → approved"

  # Apply the approved suggestion
  apply_approved_suggestion "$proposal_id"
  return $?
}

export -f approve_suggestion
