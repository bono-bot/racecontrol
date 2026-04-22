#!/usr/bin/env bash
# loop.sh — run the behavioral suite on a 15-min cadence, logging every run.
#
# Hard stop at 10:00 IST to avoid interfering with venue-open hours.
# Run one final cycle at stop time, then exit.
#
# Usage:
#   bash tests/e2e/behavioral/loop.sh &
#
# Log: logs/overnight.jsonl (summary line per run, appends to summary.jsonl)
# Env:
#   INTERVAL_SECS  default 900 (15 min)
#   STOP_IST_HOUR  default 10  (stop at 10:00 IST)
#   MAX_ITERATIONS default 0   (0 = unlimited)

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INTERVAL_SECS="${INTERVAL_SECS:-900}"
STOP_IST_HOUR="${STOP_IST_HOUR:-10}"
MAX_ITERATIONS="${MAX_ITERATIONS:-0}"
LOG_DIR="${LOG_DIR:-${ROOT}/logs}"
mkdir -p "$LOG_DIR"

OVERNIGHT_LOG="${LOG_DIR}/overnight.jsonl"
PID_FILE="${LOG_DIR}/loop.pid"

echo $$ > "$PID_FILE"

ist_now_hour() {
    python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).hour)" 2>/dev/null || echo 0
}

ist_iso() {
    python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%dT%H:%M:%S+05:30'))" 2>/dev/null || date -u -Iseconds
}

iteration=0
while true; do
    iteration=$((iteration + 1))
    hour=$(ist_now_hour)

    # Hard stop check: stop if we've entered venue-open hours.
    if [ "$hour" -ge "$STOP_IST_HOUR" ] && [ "$hour" -lt 18 ]; then
        python3 -c "
import json
print(json.dumps({'ts':'$(ist_iso)', 'event':'hard_stop', 'reason':'venue-open hours', 'hour_ist':$hour, 'iterations':$iteration}))
" >> "$OVERNIGHT_LOG"
        echo "[loop] hard stop at IST ${hour}:00 — venue-open hours."
        break
    fi

    if [ "$MAX_ITERATIONS" -gt 0 ] && [ "$iteration" -gt "$MAX_ITERATIONS" ]; then
        echo "[loop] MAX_ITERATIONS reached."
        break
    fi

    python3 -c "
import json
print(json.dumps({'ts':'$(ist_iso)', 'event':'cycle_start', 'iteration':$iteration}))
" >> "$OVERNIGHT_LOG"

    bash "${ROOT}/run-all.sh" > "${LOG_DIR}/cycle-${iteration}.stdout" 2>&1
    rc=$?

    python3 -c "
import json
print(json.dumps({'ts':'$(ist_iso)', 'event':'cycle_end', 'iteration':$iteration, 'exit_code':$rc}))
" >> "$OVERNIGHT_LOG"

    echo "[loop] iteration $iteration finished rc=$rc at $(ist_iso). Sleeping ${INTERVAL_SECS}s."
    sleep "$INTERVAL_SECS"
done

rm -f "$PID_FILE"
echo "[loop] exit clean."
