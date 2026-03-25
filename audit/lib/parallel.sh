#!/usr/bin/env bash
# audit/lib/parallel.sh -- Parallel pod execution with 4-connection semaphore
#
# Provides file-based semaphore locking with stale-lock detection and
# a parallel_pod_loop dispatcher that staggers pod launches 200ms apart
# to avoid ARP floods. All phase_fn functions write to per-pod result
# files via emit_result -- no stdout interleaving.
#
# Prerequisites: source audit/lib/core.sh first (provides emit_result, http_get,
#                safe_remote_exec, ist_now).
#
# Usage:
#   source "$SCRIPT_DIR/lib/core.sh"
#   source "$SCRIPT_DIR/lib/parallel.sh"
#   parallel_pod_loop my_phase_fn   # my_phase_fn(ip, host)
#
# Standing rules applied:
#   - No set -e (errors encoded in emit_result, not bash exit code)
#   - JSON payloads written to temp file for curl (never inline)
#   - All timestamps in IST via ist_now() from core.sh

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

# ---------------------------------------------------------------------------
# Concurrency constants
# ---------------------------------------------------------------------------
MAX_CONCURRENT=4
STAGGER_MS=0.2

# ---------------------------------------------------------------------------
# FUNCTION 1 — semaphore_acquire
# Acquire a slot (0 to MAX_CONCURRENT-1) using mkdir atomic locking.
# Sets ACQUIRED_SLOT in the calling shell for later semaphore_release call.
# Detects stale locks (process no longer alive) and clears them.
# Blocks (with 0.2s sleep) until a slot is free.
# ---------------------------------------------------------------------------
semaphore_acquire() {
  local sem_dir="${RESULT_DIR}/.sem"
  mkdir -p "$sem_dir"

  while true; do
    local i
    for i in $(seq 0 $((MAX_CONCURRENT - 1))); do
      local lock_dir="${sem_dir}/slot-${i}.lock"

      # Stale lock detection: if lock exists but PID inside is dead, remove it
      if [[ -d "$lock_dir" ]]; then
        local pid_file="${lock_dir}/pid"
        if [[ -f "$pid_file" ]]; then
          local stored_pid
          stored_pid=$(cat "$pid_file" 2>/dev/null || echo "")
          if [[ -n "$stored_pid" ]]; then
            if ! kill -0 "$stored_pid" 2>/dev/null; then
              # Process is gone -- lock is stale, remove it
              rm -rf "$lock_dir"
            fi
          else
            # Empty or unreadable pid file -- treat as stale
            rm -rf "$lock_dir"
          fi
        fi
      fi

      # Attempt atomic acquisition via mkdir
      if mkdir "$lock_dir" 2>/dev/null; then
        # Write our PID into the lock so stale detection works for others
        echo "$$" > "${lock_dir}/pid"
        ACQUIRED_SLOT=$i
        export ACQUIRED_SLOT
        return 0
      fi
    done

    # All slots busy -- wait and retry
    sleep "$STAGGER_MS"
  done
}
export -f semaphore_acquire

# ---------------------------------------------------------------------------
# FUNCTION 2 — semaphore_release (slot_num)
# Release the named slot by removing its lock directory.
# ---------------------------------------------------------------------------
semaphore_release() {
  local slot_num=$1
  local lock_dir="${RESULT_DIR}/.sem/slot-${slot_num}.lock"
  rm -rf "$lock_dir"
}
export -f semaphore_release

# ---------------------------------------------------------------------------
# FUNCTION 3 — parallel_pod_loop (phase_fn)
# Dispatch phase_fn(ip, host) for every pod in $PODS in parallel.
# Enforces MAX_CONCURRENT=4 simultaneous connections via semaphore.
# Staggers launches by STAGGER_MS=0.2 to prevent ARP floods.
# Waits for all background jobs before returning.
# phase_fn must write its own results via emit_result -- no stdout capture.
# ---------------------------------------------------------------------------
parallel_pod_loop() {
  local phase_fn=$1
  local pids=()

  # Ensure semaphore directory exists
  mkdir -p "${RESULT_DIR}/.sem"

  for ip in $PODS; do
    local host
    host="pod-$(echo "$ip" | sed 's/192\.168\.31\.//')"

    # 200ms stagger between launches to avoid ARP flood
    sleep "$STAGGER_MS"

    # Launch background subshell: acquire slot, run phase_fn, release slot
    (
      semaphore_acquire
      local slot=$ACQUIRED_SLOT
      "$phase_fn" "$ip" "$host"
      semaphore_release "$slot"
    ) &

    pids+=($!)
  done

  # Wait for all pod jobs to complete
  wait_all_jobs "${pids[@]}"
}
export -f parallel_pod_loop

# ---------------------------------------------------------------------------
# FUNCTION 4 — wait_all_jobs (pid...)
# Wait for all given PIDs, returning the maximum exit code seen.
# Phase functions always return 0, but this preserves correct semantics
# for any callers that check the return value.
# ---------------------------------------------------------------------------
wait_all_jobs() {
  local pids=("$@")
  local max_rc=0
  local pid
  for pid in "${pids[@]}"; do
    local rc=0
    wait "$pid" || rc=$?
    if [[ $rc -gt $max_rc ]]; then
      max_rc=$rc
    fi
  done
  return $max_rc
}
export -f wait_all_jobs
