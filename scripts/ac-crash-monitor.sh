#!/bin/bash
# ac-crash-monitor.sh — Detect "Running -> Error within 5s" pattern on any pod,
# the canonical customer-impacting AC crash signature (exit code 53 class).
#
# Behavioral monitor: catches the symptom regardless of root cause (Steam,
# wheelbase, python plugin, content corruption, etc.). Superseded earlier
# steam-login-monitor.sh — the state-probe approach produced false positives
# (Pod 5 had ActiveUser=0 but AC launched cleanly on 2026-04-22 08:27).
#
# Incident background: 2026-04-22, Pod 3 AC crashes exit-53 within 2s of
# reaching Running. 6 interventions attempted (NTP, python.ini, ConspitLink
# cleanup, USB reseat, Steam login, reboot). Root cause remained ambiguous.
# This monitor focuses on the customer-visible symptom instead.
#
# Schedule on James every 2 min:
#   schtasks /Create /TN "AC-Crash-Monitor" /TR "bat" /SC MINUTE /MO 2 /F
#
# Exit code = pods crashed in this window (0 = clean).

set -u

SERVER="${SERVER:-server}"                    # ~/.ssh/config alias
LOG_BASE="${LOG_BASE:-C:/RacingPoint/logs/racecontrol-.$(date -u +%Y-%m-%d).jsonl}"
COMMS_INBOX="${COMMS_INBOX:-C:/Users/bono/racingpoint/comms-link/INBOX.md}"
STATE_DIR="${STATE_DIR:-C:/Users/bono/racingpoint/racecontrol/logs}"
STATE_FILE="$STATE_DIR/.ac-crash-monitor-last-alert.txt"
NOW_UTC="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
NOW_IST="$(python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d %H:%M IST'))")"
WINDOW_MIN="${WINDOW_MIN:-5}"                 # look-back minutes

mkdir -p "$STATE_DIR" 2>/dev/null

# Fetch log lines from server via SSH (last 500 lines is more than enough for a 5-min window)
RAW_LOG="$(ssh -o ConnectTimeout=10 "$SERVER" "powershell -NoProfile -Command \"Get-Content '$LOG_BASE' -Tail 500\"" 2>/dev/null)"

if [ -z "$RAW_LOG" ]; then
  echo "[$NOW_UTC] ac-crash-monitor: could not read server log (SSH failed or file empty)" >&2
  exit 2
fi

# Detect pattern: Running -> Error within 5s for same pod.
# Extract pod crashes from the log window using python (jsonl parsing + timestamp math).
CRASHES="$(echo "$RAW_LOG" | python3 - "$WINDOW_MIN" <<'PYEOF'
import sys, json, re
from datetime import datetime, timedelta, timezone

WINDOW_MIN = int(sys.argv[1])
cutoff = datetime.now(timezone.utc) - timedelta(minutes=WINDOW_MIN)

# Extract game_state events per pod in time order
running_at = {}  # pod_id -> list of (ts, game)
error_at   = {}  # pod_id -> list of (ts, game)

line_re = re.compile(r'Pod (pod_\d+) game state: (Running|Error) \(([^)]+)\)')

for line in sys.stdin:
    try:
        d = json.loads(line)
    except Exception:
        continue
    ts_str = d.get('timestamp','')
    try:
        ts = datetime.fromisoformat(ts_str.replace('Z','+00:00'))
    except Exception:
        continue
    if ts < cutoff:
        continue
    msg = d.get('fields',{}).get('message','')
    m = line_re.search(msg)
    if not m:
        continue
    pod, state, game = m.group(1), m.group(2), m.group(3)
    if state == 'Running':
        running_at.setdefault(pod, []).append((ts, game))
    elif state == 'Error':
        error_at.setdefault(pod, []).append((ts, game))

# For each pod, find Running->Error pairs within 5s
crashes = []
for pod, runs in running_at.items():
    errors = error_at.get(pod, [])
    for run_ts, run_game in runs:
        for err_ts, err_game in errors:
            if err_ts < run_ts:
                continue
            delta = (err_ts - run_ts).total_seconds()
            if 0 <= delta <= 5 and run_game == err_game:
                crashes.append({
                    'pod': pod,
                    'game': run_game,
                    'running_ts': run_ts.strftime('%Y-%m-%dT%H:%M:%SZ'),
                    'error_ts': err_ts.strftime('%Y-%m-%dT%H:%M:%SZ'),
                    'delta_s': round(delta, 1),
                })
                break

for c in crashes:
    print(f"{c['pod']}|{c['game']}|{c['running_ts']}|{c['error_ts']}|{c['delta_s']}")
PYEOF
)"

if [ -z "$CRASHES" ]; then
  echo "[$NOW_UTC] ac-crash-monitor: no Running->Error<5s events in last $WINDOW_MIN min"
  exit 0
fi

# Build alert body
COUNT="$(echo "$CRASHES" | wc -l | tr -d ' ')"
ALERT_BODY="## $NOW_IST - AC CRASH ALERT (exit-53 signature)\n\n"
ALERT_BODY="${ALERT_BODY}$COUNT launch(es) crashed within 5s of reaching Running state in the last $WINDOW_MIN min:\n\n"
while IFS='|' read -r pod game run_ts err_ts delta; do
  ALERT_BODY="${ALERT_BODY}- **$pod** - $game - Running at $run_ts, Error at $err_ts (+${delta}s)\n"
done <<< "$CRASHES"
ALERT_BODY="${ALERT_BODY}\nPoE ladder: docs/DIAGNOSTIC-PLAYBOOK.md Pod-AC-exit-53 section.\n"

# Dedup by fingerprint so we don't re-alert on the same set
FP="$(echo "$CRASHES" | sort | sha256sum | cut -d' ' -f1)"
PREV_FP=""
[ -f "$STATE_FILE" ] && PREV_FP="$(cat "$STATE_FILE")"

if [ "$FP" = "$PREV_FP" ]; then
  echo "[$NOW_UTC] ac-crash-monitor: $COUNT crash(es) detected, same set as last run (alert suppressed)"
  exit "$COUNT"
fi

echo "$FP" > "$STATE_FILE"
printf "\n%b\n" "$ALERT_BODY" >> "$COMMS_INBOX"
echo "[$NOW_UTC] ac-crash-monitor: $COUNT crash(es) detected, alert appended to $COMMS_INBOX"
echo "$CRASHES" | while IFS='|' read -r pod game run_ts err_ts delta; do
  echo "  $pod $game Running@$run_ts Error@$err_ts +${delta}s"
done

exit "$COUNT"
