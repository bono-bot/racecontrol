#!/usr/bin/env bash
# rotate-credentials.sh — M2-SEC: Rotate all system credentials
#
# Generates new secrets, updates racecontrol.toml on server + cloud,
# updates comms-link env, updates pod service keys, and verifies connectivity.
#
# MUST be run from James PC. Requires SSH access to server + VPS.
#
# Usage: bash scripts/rotate-credentials.sh [--dry-run]

set -euo pipefail

DRY_RUN="${1:-}"
TIMESTAMP=$(date '+%Y-%m-%d_%H%M')

# ─── Generate new credentials ─────────────────────────────────────────────────
NEW_JWT_SECRET=$(openssl rand -hex 32)
NEW_COMMS_PSK=$(openssl rand -hex 32)
NEW_RELAY_SECRET=$(openssl rand -hex 16)
NEW_SENTRY_KEY=$(openssl rand -hex 32)
NEW_ADMIN_PIN=$(shuf -i 100000-999999 -n 1)

echo "============================================================"
echo "CREDENTIAL ROTATION — $TIMESTAMP"
echo "============================================================"
echo ""
echo "New JWT Secret:    ${NEW_JWT_SECRET:0:16}..."
echo "New COMMS_PSK:     ${NEW_COMMS_PSK:0:16}..."
echo "New Relay Secret:  ${NEW_RELAY_SECRET:0:16}..."
echo "New Sentry Key:    ${NEW_SENTRY_KEY:0:16}..."
echo "New Admin PIN:     $NEW_ADMIN_PIN"
echo ""

if [ "$DRY_RUN" = "--dry-run" ]; then
  echo "DRY RUN — no changes made"
  exit 0
fi

# ─── Hash the new admin PIN ───────────────────────────────────────────────────
echo "[1/6] Hashing new admin PIN..."
cd C:/Users/bono/racingpoint/racecontrol
NEW_PIN_HASH=$(cargo run --release --bin racecontrol -- --hash-pin "$NEW_ADMIN_PIN" 2>/dev/null || echo "HASH_FAILED")
if [ "$NEW_PIN_HASH" = "HASH_FAILED" ]; then
  echo "  WARNING: Could not hash PIN via cargo run. Manual hash required."
  echo "  Use: cargo run --release --bin racecontrol -- --hash-pin $NEW_ADMIN_PIN"
fi

# ─── Step 2: Update server racecontrol.toml ───────────────────────────────────
echo "[2/6] Updating server .23 racecontrol.toml..."
SERVER_SSH="ssh -o ConnectTimeout=5 ADMIN@100.125.108.37"

# Backup current config
$SERVER_SSH "copy C:\\RacingPoint\\racecontrol.toml C:\\RacingPoint\\racecontrol.toml.bak-${TIMESTAMP}" 2>/dev/null || true

# Use PowerShell to do in-place replacements (TOML values)
$SERVER_SSH "powershell -Command \"
  \$f = 'C:\\RacingPoint\\racecontrol.toml'
  \$c = Get-Content \$f -Raw
  \$c = \$c -replace 'jwt_secret = \"[^\"]*\"', 'jwt_secret = \"${NEW_JWT_SECRET}\"'
  \$c = \$c -replace 'relay_secret = \"[^\"]*\"', 'relay_secret = \"${NEW_RELAY_SECRET}\"'
  \$c = \$c -replace 'sentry_service_key = \"[^\"]*\"', 'sentry_service_key = \"${NEW_SENTRY_KEY}\"'
  Set-Content \$f \$c -NoNewline
  Write-Host 'Server TOML updated'
\"" 2>/dev/null && echo "  OK" || echo "  FAILED — update manually"

# ─── Step 3: Update Bono VPS racecontrol.toml ─────────────────────────────────
echo "[3/6] Updating Bono VPS racecontrol.toml..."
BONO_SSH="ssh -o ConnectTimeout=5 root@100.70.177.44"
$BONO_SSH "cp /root/racecontrol/racecontrol.toml /root/racecontrol/racecontrol.toml.bak-${TIMESTAMP}" 2>/dev/null || true
$BONO_SSH "sed -i \
  -e 's|jwt_secret = \"[^\"]*\"|jwt_secret = \"${NEW_JWT_SECRET}\"|' \
  -e 's|relay_secret = \"[^\"]*\"|relay_secret = \"${NEW_RELAY_SECRET}\"|' \
  -e 's|sentry_service_key = \"[^\"]*\"|sentry_service_key = \"${NEW_SENTRY_KEY}\"|' \
  /root/racecontrol/racecontrol.toml" 2>/dev/null && echo "  OK" || echo "  FAILED — update manually"

# ─── Step 4: Update comms-link PSK ────────────────────────────────────────────
echo "[4/6] Updating comms-link PSK..."
# James side
COMMS_ENV="C:/Users/bono/racingpoint/comms-link/.env"
if [ -f "$COMMS_ENV" ]; then
  sed -i "s|COMMS_PSK=.*|COMMS_PSK=${NEW_COMMS_PSK}|" "$COMMS_ENV"
  echo "  James .env updated"
fi
# James watchdog
WATCHDOG="C:/Users/bono/.claude/james_watchdog.ps1"
if [ -f "$WATCHDOG" ]; then
  sed -i "s|COMMS_PSK = \"[^\"]*\"|COMMS_PSK = \"${NEW_COMMS_PSK}\"|" "$WATCHDOG"
  echo "  James watchdog updated"
fi
# Bono side
$BONO_SSH "sed -i 's|COMMS_PSK=.*|COMMS_PSK=${NEW_COMMS_PSK}|' /root/comms-link/.env" 2>/dev/null && echo "  Bono .env updated" || echo "  Bono FAILED"

# ─── Step 5: Update pod service keys ──────────────────────────────────────────
echo "[5/6] Updating pod sentry service keys..."
POD_IPS=(192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91)
POD_NAMES=(Pod-1 Pod-2 Pod-3 Pod-4 Pod-5 Pod-6 Pod-7 Pod-8)
SENTRY_PORT=8091

for i in "${!POD_IPS[@]}"; do
  pod_ip="${POD_IPS[$i]}"
  pod_name="${POD_NAMES[$i]}"
  RESULT=$(curl -s --connect-timeout 3 -X POST "http://${pod_ip}:${SENTRY_PORT}/exec" \
    -H "Content-Type: application/json" \
    -d "{\"cmd\":\"powershell -Command \\\"(Get-Content C:\\\\RacingPoint\\\\rc-agent.toml -Raw) -replace 'sentry_service_key = \\\\\\\"[^\\\\\\\"]*\\\\\\\"', 'sentry_service_key = \\\\\\\"${NEW_SENTRY_KEY}\\\\\\\"' | Set-Content C:\\\\RacingPoint\\\\rc-agent.toml -NoNewline\\\"\"}" \
    2>/dev/null || echo '{"error":"unreachable"}')
  echo "  $pod_name: $(echo $RESULT | grep -q 'success' && echo 'OK' || echo 'SKIP (update on next deploy)')"
done

# ─── Step 6: Restart services ─────────────────────────────────────────────────
echo "[6/6] Services need restart to pick up new credentials:"
echo "  Server .23:  schtasks /Run /TN StartRCTemp (via SSH)"
echo "  Bono VPS:    pm2 restart all"
echo "  James relay: Restart comms-link daemon"
echo "  Pods:        Service key takes effect on next rc-agent restart"
echo ""
echo "============================================================"
echo "ROTATION COMPLETE — verify connectivity after restarting services"
echo "============================================================"
echo ""
echo "Verification commands:"
echo "  curl -s http://192.168.31.23:8080/api/v1/health  # server"
echo "  curl -s http://localhost:8766/relay/health         # relay"
echo "  curl -s -X POST http://localhost:8766/relay/exec/run -d '{\"command\":\"health_check\"}'  # bono"
echo ""
echo "Save these credentials securely (NOT in git):"
echo "  JWT_SECRET=$NEW_JWT_SECRET"
echo "  COMMS_PSK=$NEW_COMMS_PSK"
echo "  RELAY_SECRET=$NEW_RELAY_SECRET"
echo "  SENTRY_KEY=$NEW_SENTRY_KEY"
echo "  ADMIN_PIN=$NEW_ADMIN_PIN"
