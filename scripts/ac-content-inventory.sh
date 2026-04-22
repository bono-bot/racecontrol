#!/bin/bash
# ac-content-inventory.sh — Fleet-wide AC cars+tracks enumeration.
# Usage: bash ac-content-inventory.sh [pod_num ...]  (default: all 8)
set -u

SENTRY_KEY="${SENTRY_KEY:-478a3688339737fb5945f9b89d8bb533f2569fe0b1fea46b504656eee455b9ab}"
STAGING_DIR="${STAGING_DIR:-C:/Users/bono/racingpoint/deploy-staging}"
STAGING_URL="${STAGING_URL:-http://192.168.31.27:18889}"
OUT_DIR="${OUT_DIR:-C:/Users/bono/racingpoint/racecontrol-monitors/logs/ac-content}"
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

cp "$(dirname "$0")/ac-content-inventory.ps1" "$STAGING_DIR/ac-content-inventory.ps1"

if [ $# -eq 0 ]; then TARGETS=(1 2 3 4 5 6 7 8); else TARGETS=("$@"); fi

for pod in "${TARGETS[@]}"; do
  ip="${PODS[$pod]:-}"; [ -z "$ip" ] && continue
  printf '\n==== POD %s (%s) ====\n' "$pod" "$ip"
  curl -s --max-time 10 -X POST "http://$ip:8091/exec" \
    -H "X-Service-Key: $SENTRY_KEY" -H "Content-Type: application/json" \
    -d "{\"cmd\":\"curl.exe -s -o C:\\\\RacingPoint\\\\ac-content-inventory.ps1 $STAGING_URL/ac-content-inventory.ps1\"}" \
    > /dev/null || { echo "  fetch failed"; continue; }
  res="$(curl -s --max-time 60 -X POST "http://$ip:8091/exec" \
    -H "X-Service-Key: $SENTRY_KEY" -H "Content-Type: application/json" \
    -d '{"cmd":"powershell -ExecutionPolicy Bypass -NoProfile -File C:\\RacingPoint\\ac-content-inventory.ps1"}' 2>/dev/null)"
  if [ -z "$res" ]; then echo "  no response"; continue; fi
  echo "$res" > "$OUT_DIR/pod${pod}-${NOW}.raw.json"
  echo "$res" | python -c "import sys,json; d=json.load(sys.stdin); print(d.get('stdout',''))" > "$OUT_DIR/pod${pod}-${NOW}.jsonl" 2>/dev/null
  # Summary line only
  echo "$res" | python -c "
import sys, json
d = json.load(sys.stdin)
lines = d.get('stdout','').strip().splitlines()
if not lines:
    print('  (no output)'); sys.exit()
s = json.loads(lines[0])
if s.get('ac_installed') is False:
    print(f\"  host={s.get('host')} ac=NOT INSTALLED  error={s.get('error')}\"); sys.exit()
print(f\"  host={s.get('host')} ac_path={s.get('ac_path')}\")
print(f\"  cars={s.get('cars_count')} tracks={s.get('tracks_count')} layouts={s.get('tracks_layout_count')}\")
" 2>&1
done

echo ""
echo "Full JSONL per pod saved in: $OUT_DIR"
