#!/usr/bin/env bash
# Phase 414 — Venue Financial E2E Verification Helper
#
# Usage:
#   bash scripts/verify-414-e2e.sh --session <uuid>
#   bash scripts/verify-414-e2e.sh --driver <driver_id> --since "2026-04-18 15:00:00"
#
# What it does:
#   Queries racecontrol.db on server .23 via SSH for the billing_session row,
#   billing_events timeline, and wallet_transactions deltas. Emits a filled
#   row operators can paste into .planning/phases/414-.../414-FINANCIAL-E2E.md
#   "Actual" column.
#
# Why it exists:
#   The 4 venue E2E tests each need cross-table verification (session status,
#   elapsed_seconds, total_debited_paise, wallet before/after, event timeline
#   for GameStopped/IdleWarning/idle_auto_end). Doing this by hand per test
#   is error-prone. This script outputs the single authoritative table row.
#
# Evidence the script produces:
#   - Session row: id, status, elapsed_seconds, total_debited_paise, started_at, ended_at
#   - Wallet delta: starting_balance - ending_balance (in paise AND credits)
#   - Event timeline: event_type + timestamps (spot GameStopped, IdleWarning, idle_auto_end_queued)
#   - Guard check: did idle_auto_end fire exactly once? (MMA P1-A one-shot guard verification)

set -u
set -o pipefail

SERVER_SSH="${SERVER_SSH:-ADMIN@100.125.108.37}"
DB_PATH="${DB_PATH:-C:\\RacingPoint\\racecontrol.db}"
MODE=""
SESSION_ID=""
DRIVER_ID=""
SINCE=""

usage() {
  cat <<EOF
Usage:
  $0 --session <uuid>
  $0 --driver <driver_id> --since "YYYY-MM-DD HH:MM:SS"  (looks up most recent session)

Env overrides:
  SERVER_SSH   default ADMIN@100.125.108.37 (Tailscale)
  DB_PATH      default C:\\RacingPoint\\racecontrol.db
EOF
  exit 2
}

while [ $# -gt 0 ]; do
  case "$1" in
    --session) SESSION_ID="$2"; MODE="session"; shift 2 ;;
    --driver)  DRIVER_ID="$2"; MODE="driver"; shift 2 ;;
    --since)   SINCE="$2"; shift 2 ;;
    -h|--help) usage ;;
    *) echo "unknown arg: $1" >&2; usage ;;
  esac
done

[ -z "$MODE" ] && usage
[ "$MODE" = "driver" ] && [ -z "$SINCE" ] && { echo "--driver requires --since" >&2; usage; }

run_sql() {
  local sql="$1"
  ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$SERVER_SSH" \
    "sqlite3 -header -separator '|' \"$DB_PATH\" \"$sql\"" 2>&1
}

resolve_session_id() {
  if [ "$MODE" = "session" ]; then
    echo "$SESSION_ID"
    return
  fi
  local sql="SELECT id FROM billing_sessions WHERE driver_id='$DRIVER_ID' AND started_at >= '$SINCE' ORDER BY started_at DESC LIMIT 1;"
  run_sql "$sql" | tail -n +2 | head -1
}

SID="$(resolve_session_id)"
if [ -z "$SID" ]; then
  echo "No session found for given inputs." >&2
  exit 1
fi

echo "=== Phase 414 E2E — session $SID ==="
echo

echo "-- billing_sessions row --"
run_sql "SELECT id, driver_id, pod_id, status, allocated_seconds, driving_seconds, elapsed_seconds, total_debited_paise, started_at, ended_at FROM billing_sessions WHERE id='$SID';"
echo

echo "-- billing_events timeline --"
run_sql "SELECT created_at, event_type, driving_seconds_at_event, metadata FROM billing_events WHERE billing_session_id='$SID' ORDER BY created_at ASC;"
echo

echo "-- wallet_transactions linked to this session (reference_id = session_id for debit_session/refund_session) --"
run_sql "SELECT created_at, txn_type, amount_paise, balance_after_paise, notes FROM wallet_transactions WHERE reference_id='$SID' ORDER BY created_at ASC;"
echo

echo "-- wallet delta (session-linked txns only) --"
# starting_balance = balance_after_paise of first txn - amount_paise of that txn (reconstruct pre-txn balance)
# ending_balance = balance_after_paise of last txn
# Note: amount_paise is signed per txn_type convention — debit_session is negative, refund_session positive.
run_sql "SELECT 'starting_balance_paise' AS k, (balance_after_paise - amount_paise) AS v FROM wallet_transactions WHERE reference_id='$SID' ORDER BY created_at ASC LIMIT 1;"
run_sql "SELECT 'ending_balance_paise' AS k, balance_after_paise AS v FROM wallet_transactions WHERE reference_id='$SID' ORDER BY created_at DESC LIMIT 1;"
run_sql "SELECT 'net_debit_paise' AS k, -SUM(amount_paise) AS v FROM wallet_transactions WHERE reference_id='$SID';"
echo

echo "-- MMA P1-A guard check (idle_auto_end should fire AT MOST once per session) --"
# 8a52cc36 added idle_auto_end_queued one-shot guard. Count should be 0 (session ended early)
# or 1 (session hit 15-min idle auto-end). > 1 = guard regressed.
run_sql "SELECT event_type, COUNT(*) AS count FROM billing_events WHERE billing_session_id='$SID' AND event_type LIKE '%idle_auto_end%' GROUP BY event_type;"
echo

echo "-- Phase 414 new transitions audit --"
# Active <-> WaitingForGame swaps (cumulative-snap-across-games feature)
run_sql "SELECT event_type, COUNT(*) AS count FROM billing_events WHERE billing_session_id='$SID' AND (event_type LIKE '%GameStopped%' OR event_type LIKE '%WaitingForGame%' OR event_type LIKE '%IdleWarning%') GROUP BY event_type;"
echo

echo "=== Done. Paste the rows above into 414-FINANCIAL-E2E.md 'Actual' column. ==="
