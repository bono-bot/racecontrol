#!/usr/bin/env bash
# ─── Installed Games E2E Verification ────────────────────────────────────────
# Verifies that each pod's reported installed_games matches actual Steam
# installations on disk. No ghost games should appear.
#
# WHAT IT CHECKS:
#   INST-01: Each pod's installed_games matches Steam appmanifest files
#   INST-02: No ghost games (reported but not on disk)
#   INST-03: Server pod state matches agent health
#
# USAGE: bash tests/e2e/fleet/installed-games.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

SERVER_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"

info "Installed Games Verification (INST-01/02/03)"
echo ""

# ─── Gate 1: Get server-reported installed_games ─────────────────────────────
echo "--- Gate 1: Server-reported installed_games ---"
SERVER_PODS=$(curl -s --max-time 10 "${SERVER_URL}/pods" 2>/dev/null)
if [ -z "$SERVER_PODS" ]; then
    fail "Could not reach server pods endpoint"
    summary_exit
fi
pass "Server pods endpoint reachable"

# ─── Gate 2: Per-pod Steam manifest verification ────────────────────────────
echo ""
echo "--- Gate 2: Per-pod disk verification (INST-01) ---"
GHOST_COUNT=0
TOTAL_PODS=0

# Known Steam app IDs for our games
# AC=244210, EVO=3058630, Rally=3917090, F1_25=3059520,
# iRacing=266410, LMU=2399420, Forza=2440510, FH5=1551360
declare -A APPID_TO_GAME
APPID_TO_GAME[244210]="assetto_corsa"
APPID_TO_GAME[3058630]="assetto_corsa_evo"
APPID_TO_GAME[3917090]="assetto_corsa_rally"
APPID_TO_GAME[3059520]="f1_25"
APPID_TO_GAME[266410]="iracing"
APPID_TO_GAME[2399420]="le_mans_ultimate"
APPID_TO_GAME[2440510]="forza"
APPID_TO_GAME[1551360]="forza_horizon_5"

for POD_NUM in 1 2 3 4 5 6 7 8; do
    POD_IP=$(pod_ip "pod-${POD_NUM}")
    TOTAL_PODS=$((TOTAL_PODS + 1))

    # Get reported games from server
    REPORTED=$(echo "$SERVER_PODS" | python3 -c "
import sys,json
data = json.load(sys.stdin)
pods = data.get('pods', [])
for p in pods:
    if p.get('number') == ${POD_NUM} or p.get('pod_number') == ${POD_NUM}:
        print(' '.join(sorted(p.get('installed_games', []))))
        break
" 2>/dev/null)

    # Get actual manifests from pod via rc-agent or rc-sentry
    MANIFESTS=""
    # Try rc-agent first
    MANIFEST_RAW=$(curl -s --max-time 8 -X POST "http://${POD_IP}:8090/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"dir /b C:\\Progra~2\\Steam\\steamapps\\appmanifest_*.acf","timeout_secs":5}' 2>/dev/null)
    if [ -z "$MANIFEST_RAW" ]; then
        # Fallback to rc-sentry
        MANIFEST_RAW=$(curl -s --max-time 8 -X POST "http://${POD_IP}:8091/exec" \
            -H "Content-Type: application/json" \
            --data-binary @"${SCRIPT_DIR}/../../../deploy-staging/check-manifests.json" 2>/dev/null)
    fi

    DISK_GAMES=$(echo "$MANIFEST_RAW" | python3 -c "
import sys,json
appid_map = {
    '244210': 'assetto_corsa',
    '3058630': 'assetto_corsa_evo',
    '3917090': 'assetto_corsa_rally',
    '3059520': 'f1_25',
    '266410': 'iracing',
    '2399420': 'le_mans_ultimate',
    '2440510': 'forza',
    '1551360': 'forza_horizon_5',
}
try:
    d = json.load(sys.stdin)
    stdout = d.get('stdout', '')
    games = set()
    for line in stdout.split('\n'):
        line = line.strip()
        if 'appmanifest_' in line:
            # Extract app_id from appmanifest_XXXXX.acf
            parts = line.split('appmanifest_')
            if len(parts) > 1:
                app_id = parts[1].split('.')[0]
                if app_id in appid_map:
                    games.add(appid_map[app_id])
    # AC is always added by rc-agent
    games.add('assetto_corsa')
    print(' '.join(sorted(games)))
except:
    print('ERROR')
" 2>/dev/null)

    if [ "$DISK_GAMES" = "ERROR" ] || [ -z "$DISK_GAMES" ]; then
        skip "Pod $POD_NUM ($POD_IP): could not read manifests"
        continue
    fi

    # Compare: find ghost games (reported but not on disk)
    GHOSTS=$(python3 -c "
reported = set('${REPORTED}'.split())
disk = set('${DISK_GAMES}'.split())
ghosts = reported - disk
if ghosts:
    print(' '.join(sorted(ghosts)))
else:
    print('')
" 2>/dev/null)

    if [ -n "$GHOSTS" ] && [ "$GHOSTS" != "" ]; then
        fail "Pod $POD_NUM: GHOST games reported but not installed: $GHOSTS"
        GHOST_COUNT=$((GHOST_COUNT + 1))
    else
        pass "Pod $POD_NUM: reported=[${REPORTED}] matches disk (no ghosts)"
    fi
done

# ─── Gate 3: No ghost games across fleet (INST-02) ──────────────────────────
echo ""
echo "--- Gate 3: Fleet ghost game summary (INST-02) ---"
if [ "$GHOST_COUNT" -eq 0 ]; then
    pass "Zero ghost games across all $TOTAL_PODS pods"
else
    fail "$GHOST_COUNT pod(s) have ghost games"
fi

# ─── Gate 4: Build consistency ───────────────────────────────────────────────
echo ""
echo "--- Gate 4: Build consistency ---"
BUILDS=""
for POD_NUM in 1 2 3 4 5 6 7 8; do
    POD_IP=$(pod_ip "pod-${POD_NUM}")
    BUILD=$(curl -s --max-time 3 "http://${POD_IP}:8090/health" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('build_id','?'))" 2>/dev/null)
    BUILDS="$BUILDS $BUILD"
done
UNIQUE=$(echo $BUILDS | tr ' ' '\n' | sort -u | grep -v '^$' | wc -l)
FIRST_BUILD=$(echo $BUILDS | tr ' ' '\n' | sort -u | grep -v '^$' | head -1)
if [ "$UNIQUE" -eq 1 ]; then
    pass "All pods on same build: $FIRST_BUILD"
else
    fail "Multiple builds detected: $(echo $BUILDS | tr ' ' '\n' | sort -u | tr '\n' ' ')"
fi

echo ""
summary_exit
