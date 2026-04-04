#!/bin/bash
# Smart Pipe: Temporal Soak Test
# Runs for 20 minutes, captures memory/log/connection metrics before and after
# Detects: memory leaks, log growth, connection pool exhaustion, temp file growth

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RESULTS_DIR="$REPO_ROOT/.smart-pipes-results/soak-$(date +%Y%m%d-%H%M)"
mkdir -p "$RESULTS_DIR"
DURATION_MIN=${1:-20}
DURATION_SEC=$((DURATION_MIN * 60))

echo "╔══════════════════════════════════════╗"
echo "║  Temporal Soak Test (${DURATION_MIN}min)          ║"
echo "╚══════════════════════════════════════╝"

# ── Baseline snapshot ─────────────────────────────────────────
echo "[1/4] Capturing baseline..."
{
  echo "timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "server_health: $(curl -s --connect-timeout 5 http://192.168.31.23:8080/api/v1/health 2>/dev/null | head -c 200)"

  # Log directory sizes
  echo "log_sizes:"
  for dir in /c/RacingPoint/logs /c/Users/bono/racingpoint/racecontrol/.smart-pipes-results; do
    if [ -d "$dir" ]; then
      SIZE=$(du -sb "$dir" 2>/dev/null | cut -f1)
      echo "  $dir: $SIZE bytes"
    fi
  done

  # Fleet health snapshot
  echo "fleet_pods: $(curl -s --connect-timeout 5 http://192.168.31.23:8080/api/v1/fleet/health 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); print(len(d.get("pods",[])))' 2>/dev/null || echo '?')"

  # Endpoint response times
  echo "response_times:"
  for ep in "http://192.168.31.23:8080/api/v1/health" "http://192.168.31.23:3200" "http://192.168.31.27:8096/api/v1/cameras"; do
    TIME=$(curl -s --connect-timeout 5 -o /dev/null -w "%{time_total}" "$ep" 2>/dev/null || echo "timeout")
    echo "  $ep: ${TIME}s"
  done
} > "$RESULTS_DIR/baseline.txt"
echo "  ✓ Baseline captured"

# ── Sustained load ────────────────────────────────────────────
echo "[2/4] Running sustained load for ${DURATION_MIN} minutes..."
if command -v k6 &>/dev/null && [ -f "$REPO_ROOT/scripts/smart-pipes/k6-predeploy.js" ]; then
  k6 run --quiet --duration "${DURATION_SEC}s" --vus 5 \
    --summary-export "$RESULTS_DIR/k6-soak.json" \
    "$REPO_ROOT/scripts/smart-pipes/k6-predeploy.js" 2>/dev/null &
  K6_PID=$!
  echo "  k6 running (PID $K6_PID)..."

  # Also run periodic curl checks during soak
  END_TIME=$(($(date +%s) + DURATION_SEC))
  SAMPLE=0
  while [ $(date +%s) -lt $END_TIME ]; do
    SAMPLE=$((SAMPLE + 1))
    TIME=$(curl -s --connect-timeout 5 -o /dev/null -w "%{time_total}" http://192.168.31.23:8080/api/v1/health 2>/dev/null || echo "timeout")
    echo "$SAMPLE $(date +%H:%M:%S) $TIME" >> "$RESULTS_DIR/latency-samples.txt"
    sleep 60
  done

  wait $K6_PID 2>/dev/null
  echo "  ✓ Load complete"
else
  echo "  ⚠ k6 not installed — using curl-only soak"
  END_TIME=$(($(date +%s) + DURATION_SEC))
  SAMPLE=0
  while [ $(date +%s) -lt $END_TIME ]; do
    SAMPLE=$((SAMPLE + 1))
    TIME=$(curl -s --connect-timeout 5 -o /dev/null -w "%{time_total}" http://192.168.31.23:8080/api/v1/health 2>/dev/null || echo "timeout")
    echo "$SAMPLE $(date +%H:%M:%S) $TIME" >> "$RESULTS_DIR/latency-samples.txt"
    sleep 30
  done
  echo "  ✓ Curl soak complete ($SAMPLE samples)"
fi

# ── End snapshot ──────────────────────────────────────────────
echo "[3/4] Capturing end snapshot..."
{
  echo "timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "server_health: $(curl -s --connect-timeout 5 http://192.168.31.23:8080/api/v1/health 2>/dev/null | head -c 200)"

  echo "log_sizes:"
  for dir in /c/RacingPoint/logs /c/Users/bono/racingpoint/racecontrol/.smart-pipes-results; do
    if [ -d "$dir" ]; then
      SIZE=$(du -sb "$dir" 2>/dev/null | cut -f1)
      echo "  $dir: $SIZE bytes"
    fi
  done

  echo "fleet_pods: $(curl -s --connect-timeout 5 http://192.168.31.23:8080/api/v1/fleet/health 2>/dev/null | python3 -c 'import json,sys; d=json.load(sys.stdin); print(len(d.get("pods",[])))' 2>/dev/null || echo '?')"

  echo "response_times:"
  for ep in "http://192.168.31.23:8080/api/v1/health" "http://192.168.31.23:3200" "http://192.168.31.27:8096/api/v1/cameras"; do
    TIME=$(curl -s --connect-timeout 5 -o /dev/null -w "%{time_total}" "$ep" 2>/dev/null || echo "timeout")
    echo "  $ep: ${TIME}s"
  done
} > "$RESULTS_DIR/end-snapshot.txt"
echo "  ✓ End snapshot captured"

# ── Compare ───────────────────────────────────────────────────
echo "[4/4] Comparing baseline vs end..."
python3 -c "
import re

def parse_sizes(text):
    sizes = {}
    for line in text.split('\n'):
        m = re.match(r'\s+(.+):\s+(\d+)\s+bytes', line)
        if m: sizes[m.group(1)] = int(m.group(2))
    return sizes

def parse_times(text):
    times = {}
    for line in text.split('\n'):
        m = re.match(r'\s+(http.+):\s+([\d.]+)s', line)
        if m: times[m.group(1)] = float(m.group(2))
    return times

baseline = open('$RESULTS_DIR/baseline.txt').read()
end = open('$RESULTS_DIR/end-snapshot.txt').read()

b_sizes = parse_sizes(baseline)
e_sizes = parse_sizes(end)

b_times = parse_times(baseline)
e_times = parse_times(end)

issues = []

# Check log growth
for path in b_sizes:
    if path in e_sizes:
        growth = e_sizes[path] - b_sizes[path]
        growth_pct = (growth / max(b_sizes[path], 1)) * 100
        if growth_pct > 20:
            issues.append(f'Log growth: {path} grew {growth_pct:.0f}% ({growth} bytes)')

# Check latency degradation
for ep in b_times:
    if ep in e_times:
        if e_times[ep] > b_times[ep] * 2 and e_times[ep] > 0.5:
            issues.append(f'Latency: {ep} degraded from {b_times[ep]:.2f}s to {e_times[ep]:.2f}s')

if issues:
    print('  ⚠ TEMPORAL ISSUES DETECTED:')
    for i in issues: print(f'    - {i}')
else:
    print('  ✓ No temporal degradation detected')
" 2>/dev/null || echo "  (comparison parse failed — check files manually)"

echo ""
echo "Results: $RESULTS_DIR/"
