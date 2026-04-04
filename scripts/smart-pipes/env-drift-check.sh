#!/bin/bash
# Smart Pipe: Environment Drift Detection
# Compares fingerprints across James, Server, and Cloud
# Reports any version/config mismatches

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RESULTS_DIR="$REPO_ROOT/.smart-pipes-results/drift-$(date +%Y%m%d)"
mkdir -p "$RESULTS_DIR"

echo "╔══════════════════════════════════════╗"
echo "║  Environment Drift Check             ║"
echo "╚══════════════════════════════════════╝"

# Collect James fingerprint (local)
echo "[1/3] James .27 fingerprint..."
{
  echo "machine: james"
  echo "git_hash: $(cd $REPO_ROOT && git rev-parse --short HEAD)"
  echo "rustc: $(rustc --version 2>/dev/null)"
  echo "node: $(node -v 2>/dev/null)"
  echo "cargo_lock: $(sha256sum $REPO_ROOT/Cargo.lock 2>/dev/null | cut -d' ' -f1)"
} > "$RESULTS_DIR/james.txt"
echo "  ✓ James captured"

# Collect Server fingerprint (SSH)
echo "[2/3] Server .23 fingerprint..."
ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no ADMIN@100.125.108.37 "
  echo machine: server
  echo git_hash: \$(cd /c/Users/ADMIN/racingpoint/racecontrol 2>/dev/null && git rev-parse --short HEAD 2>/dev/null || echo unknown)
  echo racecontrol_build: \$(curl -s http://localhost:8080/api/v1/health 2>/dev/null | python3 -c 'import json,sys; print(json.load(sys.stdin).get(\"build_id\",\"unknown\"))' 2>/dev/null || echo unknown)
  echo node: \$(node -v 2>/dev/null || echo none)
" > "$RESULTS_DIR/server.txt" 2>/dev/null && echo "  ✓ Server captured" || echo "  ⚠ Server unreachable"

# Collect Bono VPS fingerprint (relay)
echo "[3/3] Bono VPS fingerprint..."
BONO_RESULT=$(curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"command":"node_version","reason":"env-drift-check"}' 2>/dev/null)
BONO_NODE=$(echo "$BONO_RESULT" | python3 -c "import json,sys; print(json.load(sys.stdin).get('result',{}).get('stdout','unknown').strip())" 2>/dev/null || echo "unknown")

BONO_GIT=$(curl -s -X POST http://localhost:8766/relay/exec/run \
  -H "Content-Type: application/json" \
  -d '{"command":"git_log","reason":"env-drift-check"}' 2>/dev/null | python3 -c "import json,sys; r=json.load(sys.stdin).get('result',{}).get('stdout',''); print(r[:8] if r else 'unknown')" 2>/dev/null || echo "unknown")

{
  echo "machine: bono-vps"
  echo "git_hash: $BONO_GIT"
  echo "node: $BONO_NODE"
} > "$RESULTS_DIR/bono.txt"
echo "  ✓ Bono captured"

# Compare
echo ""
echo "═══════════════════════════════════════"
echo "  DRIFT COMPARISON"
echo "───────────────────────────────────────"
python3 -c "
import os

machines = {}
results_dir = '$RESULTS_DIR'
for f in ['james.txt', 'server.txt', 'bono.txt']:
    path = os.path.join(results_dir, f)
    if os.path.exists(path):
        data = {}
        for line in open(path):
            if ':' in line:
                k, v = line.split(':', 1)
                data[k.strip()] = v.strip()
        machines[data.get('machine', f)] = data

if len(machines) < 2:
    print('  ⚠ Could not reach enough machines for comparison')
else:
    # Compare git hashes
    hashes = {m: d.get('git_hash', '?') for m, d in machines.items()}
    unique_hashes = set(hashes.values()) - {'unknown', '?'}
    if len(unique_hashes) > 1:
        print(f'  ⚠ GIT DRIFT: {hashes}')
    elif len(unique_hashes) == 1:
        print(f'  ✓ Git hash aligned: {unique_hashes.pop()}')
    else:
        print(f'  ⚠ Git hashes unknown: {hashes}')

    # Compare node versions
    nodes = {m: d.get('node', '?') for m, d in machines.items()}
    unique_nodes = set(nodes.values()) - {'unknown', '?', 'none'}
    if len(unique_nodes) > 1:
        print(f'  ⚠ NODE DRIFT: {nodes}')
    elif len(unique_nodes) == 1:
        print(f'  ✓ Node version aligned: {unique_nodes.pop()}')

    print()
    for m, d in machines.items():
        print(f'  {m}: {d}')
" 2>/dev/null || echo "  (comparison failed)"

echo "═══════════════════════════════════════"
echo "Results: $RESULTS_DIR/"
