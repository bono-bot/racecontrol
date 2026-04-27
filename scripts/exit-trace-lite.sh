#!/bin/bash
# PACT-20260425-036 — racecontrol exit-trace-lite wrapper.
#
# Captures exit code + signal + last 100 stderr lines on each racecontrol
# exit cycle. Writes to /var/log/racecontrol/exit-trace-lite-<ts>.log.
#
# Avoids PACT-016 R2 orphan bug: NO pipeline (no `tee`, no process
# substitution). `wait $CHILD` waits on the racecontrol PID directly —
# `&` + `$!` capture the binary's PID, not a pipeline subprocess.
#
# Trade-off: pm2's err log will NOT see racecontrol's real-time stderr
# (it's redirected to a file). Last 100 lines surface in the trace file
# on each exit. This is a diagnostic-only wrapper — accept the trade-off
# while restart-storm cause is opaque; revert ecosystem.config.cjs to
# direct-binary path once root cause is identified.

set -u
TRACE_DIR=/var/log/racecontrol
mkdir -p "$TRACE_DIR"
TS=$(date -u +%Y%m%dT%H%M%SZ)
ERR_FILE="$TRACE_DIR/run-stderr-$TS.log"
TRACE_FILE="$TRACE_DIR/exit-trace-lite-$TS.log"

BINARY=/root/racecontrol/target/release/racecontrol

echo "[exit-trace-lite] starting $BINARY ts=$TS"

# Direct-redirect stderr (no pipeline => no orphan risk).
"$BINARY" 2>"$ERR_FILE" &
CHILD=$!
echo "[exit-trace-lite] child PID=$CHILD stderr=$ERR_FILE"

# Forward catchable signals to child. SIGKILL is uncatchable;
# pm2 sends SIGKILL after kill_timeout=30s if SIGTERM doesn't suffice.
trap 'kill -TERM "$CHILD" 2>/dev/null' SIGTERM SIGINT SIGHUP

wait "$CHILD"
EXIT=$?

# Append a structured exit-trace
{
  echo "==EXIT TRACE LITE=="
  echo "ts=$(date -u +%FT%T.%3NZ)"
  echo "exit_code=$EXIT"
  if [ "$EXIT" -gt 128 ]; then
    echo "signal=$((EXIT - 128))"
  else
    echo "signal=none"
  fi
  echo "child_pid=$CHILD"
  echo "stderr_file=$ERR_FILE"
  echo "==stderr_tail (last 100 lines)=="
  tail -n 100 "$ERR_FILE" 2>/dev/null
  echo "==END=="
} > "$TRACE_FILE"

# Echo summary to wrapper stdout so pm2's out log captures the exit signature
echo "[exit-trace-lite] EXIT $EXIT — trace=$TRACE_FILE"

# Compress stderr file if >1MB to avoid disk fill
if [ -f "$ERR_FILE" ] && [ "$(stat -c%s "$ERR_FILE" 2>/dev/null || echo 0)" -gt 1048576 ]; then
  gzip -f "$ERR_FILE" 2>/dev/null
fi

# Cleanup older than 7 days
find "$TRACE_DIR" -name 'exit-trace-lite-*.log' -mtime +7 -delete 2>/dev/null
find "$TRACE_DIR" -name 'run-stderr-*.log*'    -mtime +7 -delete 2>/dev/null

exit "$EXIT"
