#!/bin/bash
# Smart Pipe: Nightly Quality Audit
# Part of CGP 4.1 Smart Pipes architecture
# Runs via scheduled task or manually: bash scripts/smart-pipes/nightly-audit.sh

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RESULTS_DIR="$REPO_ROOT/.smart-pipes-results/nightly-$(date +%Y%m%d)"
mkdir -p "$RESULTS_DIR"

echo "╔══════════════════════════════════════╗"
echo "║  Nightly Quality Audit               ║"
echo "║  $(date '+%Y-%m-%d %H:%M IST')       ║"
echo "╚══════════════════════════════════════╝"
echo ""

# ── 1. Full cargo audit ──────────────────────────────────────
echo "[1/5] Full Rust dependency audit..."
cd "$REPO_ROOT"
cargo audit --json > "$RESULTS_DIR/cargo-audit.json" 2>/dev/null || true
echo "  Done"

# ── 2. Full npm audit (all apps) ─────────────────────────────
echo "[2/5] Full npm audit..."
for APP_DIR in web kiosk apps/admin; do
  if [ -d "$REPO_ROOT/$APP_DIR" ] && [ -f "$REPO_ROOT/$APP_DIR/package.json" ]; then
    cd "$REPO_ROOT/$APP_DIR"
    npm audit --json > "$RESULTS_DIR/npm-audit-$(basename $APP_DIR).json" 2>/dev/null || true
  fi
done
cd "$REPO_ROOT"
echo "  Done"

# ── 3. Full semgrep scan ─────────────────────────────────────
echo "[3/5] Full SAST scan..."
if command -v semgrep &>/dev/null; then
  semgrep scan --config auto --json --max-target-bytes 1000000 -o "$RESULTS_DIR/semgrep-full.json" "$REPO_ROOT" 2>/dev/null || true
  echo "  Done"
else
  echo "  semgrep not installed — skipping"
fi

# ── 4. Endpoint liveness sweep ────────────────────────────────
echo "[4/5] Endpoint liveness sweep..."
{
  echo "{"
  echo "  \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
  echo "  \"endpoints\": ["

  ENDPOINTS=(
    "http://192.168.31.23:8080/api/v1/health"
    "http://192.168.31.23:3200"
    "http://192.168.31.23:3201"
    "http://192.168.31.23:3300"
    "http://192.168.31.27:8096/api/v1/cameras"
    "http://192.168.31.27:1984/api/streams"
    "http://localhost:8766/relay/health"
  )

  FIRST=true
  for ep in "${ENDPOINTS[@]}"; do
    STATUS=$(curl -s --connect-timeout 5 -o /dev/null -w "%{http_code}" "$ep" 2>/dev/null || echo "000")
    SIZE=$(curl -s --connect-timeout 5 -o /dev/null -w "%{size_download}" "$ep" 2>/dev/null || echo "0")
    if [ "$FIRST" = true ]; then FIRST=false; else echo ","; fi
    printf '    {"url": "%s", "status": %s, "size": %s}' "$ep" "$STATUS" "$SIZE"
  done

  echo ""
  echo "  ]"
  echo "}"
} > "$RESULTS_DIR/endpoint-sweep.json"
echo "  Done"

# ── 5. Fleet health snapshot ─────────────────────────────────
echo "[5/5] Fleet health snapshot..."
curl -s --connect-timeout 5 http://192.168.31.23:8080/api/v1/fleet/health > "$RESULTS_DIR/fleet-health.json" 2>/dev/null || echo "{}" > "$RESULTS_DIR/fleet-health.json"
echo "  Done"

# ── Summary ──────────────────────────────────────────────────
echo ""
echo "═══════════════════════════════════════"
echo "  Results saved to: $RESULTS_DIR/"
echo ""

# Parse and summarize findings
python3 -c "
import json, os, glob

results_dir = '$RESULTS_DIR'
issues = []

# Cargo audit
try:
    d = json.load(open(os.path.join(results_dir, 'cargo-audit.json')))
    count = d.get('vulnerabilities', {}).get('count', 0)
    if count > 0: issues.append(f'Rust CVEs: {count}')
except: pass

# npm audits
for f in glob.glob(os.path.join(results_dir, 'npm-audit-*.json')):
    try:
        d = json.load(open(f))
        v = d.get('metadata', {}).get('vulnerabilities', {})
        high = v.get('high', 0) + v.get('critical', 0)
        if high > 0: issues.append(f'npm ({os.path.basename(f)}): {high} high/critical')
    except: pass

# Semgrep
try:
    d = json.load(open(os.path.join(results_dir, 'semgrep-full.json')))
    count = len(d.get('results', []))
    if count > 0: issues.append(f'SAST findings: {count}')
except: pass

# Endpoints
try:
    d = json.load(open(os.path.join(results_dir, 'endpoint-sweep.json')))
    down = [e['url'] for e in d.get('endpoints', []) if e.get('status', 0) != 200]
    if down:
        urls = ', '.join(down[:3])
        issues.append(f'Endpoints down: {len(down)} ({urls})')
except: pass

if issues:
    print('  FINDINGS:')
    for i in issues: print(f'    - {i}')
else:
    print('  ALL CLEAN -- no issues found')
" 2>/dev/null || echo "  (summary parse failed -- check JSON files)"

echo "═══════════════════════════════════════"
