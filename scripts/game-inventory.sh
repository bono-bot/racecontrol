#!/bin/bash
# game-inventory.sh — Fleet-wide Steam game inventory.
# Authoritative replacement for hardcoded-path audits (Pod 3 F1 25 false-negative 2026-04-22).
#
# Output: one JSON per pod with summary + game list. Merged via jq into a matrix.
# Usage:
#   bash game-inventory.sh                 # all 8 pods
#   bash game-inventory.sh 3 4             # specific pods

set -u

SENTRY_KEY="${SENTRY_KEY:-478a3688339737fb5945f9b89d8bb533f2569fe0b1fea46b504656eee455b9ab}"
STAGING_DIR="${STAGING_DIR:-C:/Users/bono/racingpoint/deploy-staging}"
STAGING_URL="${STAGING_URL:-http://192.168.31.27:18889}"
OUT_DIR="${OUT_DIR:-C:/Users/bono/racingpoint/racecontrol-monitors/logs/game-inventory}"
NOW="$(date -u +'%Y%m%dT%H%M%SZ')"
mkdir -p "$OUT_DIR"

declare -A PODS=(
  [1]="192.168.31.89"
  [2]="192.168.31.33"
  [3]="192.168.31.28"
  [4]="192.168.31.88"
  [5]="192.168.31.86"
  [6]="192.168.31.87"
  [7]="192.168.31.38"
  [8]="192.168.31.91"
)

# Inline ps1 into staging so this script is self-contained and survives fresh clones
cp "$(dirname "$0")/game-inventory.ps1" "$STAGING_DIR/game-inventory.ps1"

if [ $# -eq 0 ]; then
  TARGETS=(1 2 3 4 5 6 7 8)
else
  TARGETS=("$@")
fi

for pod in "${TARGETS[@]}"; do
  ip="${PODS[$pod]:-}"
  [ -z "$ip" ] && { echo "Pod $pod unknown"; continue; }
  printf '\n==== POD %s (%s) ====\n' "$pod" "$ip"
  # Push script
  curl -s --max-time 10 -X POST "http://$ip:8091/exec" \
    -H "X-Service-Key: $SENTRY_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"cmd\":\"curl.exe -s -o C:\\\\RacingPoint\\\\game-inventory.ps1 $STAGING_URL/game-inventory.ps1\"}" \
    > /dev/null || { echo "  fetch failed"; continue; }
  # Execute and capture
  res="$(curl -s --max-time 30 -X POST "http://$ip:8091/exec" \
    -H "X-Service-Key: $SENTRY_KEY" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"powershell -ExecutionPolicy Bypass -NoProfile -File C:\\RacingPoint\\game-inventory.ps1"}' 2>/dev/null)"
  if [ -z "$res" ]; then echo "  no response"; continue; fi
  # Save raw response + extracted stdout
  echo "$res" > "$OUT_DIR/pod${pod}-${NOW}.raw.json"
  echo "$res" | python -c "import sys,json; d=json.load(sys.stdin); print(d.get('stdout',''))" > "$OUT_DIR/pod${pod}-${NOW}.jsonl" 2>/dev/null
  # Print compact summary to stdout
  echo "$res" | python -c "
import sys, json
d = json.load(sys.stdin)
stdout = d.get('stdout','').strip().splitlines()
if not stdout:
    print('  (no output)'); sys.exit()
summary = json.loads(stdout[0])
print(f\"  host={summary.get('host')} user={summary.get('user')} libraries={len(summary.get('steam_libraries',[]))} active_user={summary.get('steam_active_user')} games={summary.get('steam_game_count')}\")
for line in stdout[1:]:
    try:
        g = json.loads(line)
        mark = 'OK' if g.get('on_disk') else '--'
        print(f\"    [{mark}] {g.get('appid'):>8} {g.get('size_gb'):>6}GB  {g.get('name')[:60]}\")
    except Exception: pass
" 2>&1
done

echo ""
echo "raw outputs: $OUT_DIR"
