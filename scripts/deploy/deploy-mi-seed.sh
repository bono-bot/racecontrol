#!/usr/bin/env bash
# deploy-mi-seed.sh — MI gap closure P1
#
# Seeds permanent "deploy gap" entries into the MI fleet KB so that Tier 0
# can short-circuit on known deploy issues without wasting AI credits.
#
# Run once at install time, or whenever new deploy gaps are added.
#
# Usage:
#   bash scripts/deploy/deploy-mi-seed.sh
#   SERVER_URL=http://192.168.31.23:8080 bash scripts/deploy/deploy-mi-seed.sh
#
# Requires: STAFF_TOKEN env var OR SERVICE_KEY env var (for /audit-seed-service)
#
# Background: MI's diagnostic engine cannot detect deploy state — it only sees
# code (via multi-model-audit.js) or runtime process state. Deploy issues like
# stale frontends, drifted bat files, or missing env vars are structurally
# invisible. By seeding them into audit_known_issues, MI's Tier 0 will recognize
# the symptom patterns and escalate with the correct fix instead of running
# expensive Tier 4 model diagnosis.

set -euo pipefail

SERVER_URL="${SERVER_URL:-http://192.168.31.23:8080}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# IST timestamp (UTC+5:30 — Git Bash TZ=Asia/Kolkata silently fails on Windows)
IST_DATE="$(python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d'))" 2>/dev/null || date -u +%Y-%m-%d)"

PAYLOAD_FILE="$(mktemp)"
trap 'rm -f "$PAYLOAD_FILE"' EXIT

cat > "$PAYLOAD_FILE" <<EOF
{
  "findings": [
    {
      "problem_key": "deploy_gap:frontend_stale",
      "severity": "P1",
      "symptom_patterns": [
        "stale js chunks",
        "old build_id frontend",
        "next.js 404 _next/static",
        "page loads unstyled",
        "websocket 800 reconnects per minute",
        "dashboard ws churn"
      ],
      "root_cause": "Next.js frontend (kiosk/web/admin) was not rebuilt after server deploy. New server sends WS messages the old JS cannot parse. Health endpoint shows OK because HTTP layer works, but WebSocket fails on every message.",
      "fix_action": "Rebuild ALL 3 frontends after any server deploy: cd kiosk && pnpm build && tar -czf kiosk.tar.gz .next public package.json && scp to server.23 && extract & restart schtask. Repeat for web/ and admin/. Verify dashboard_ws_churn.connects_per_min < 10 in /fleet/health.",
      "fix_status": "pending",
      "affects": ["server_23", "pos_20", "cloud_bono_vps"],
      "audit_date": "$IST_DATE",
      "escalation_message": "[DEPLOY GAP: frontend_stale] Server deploy completed but Next.js frontends not rebuilt. WS message format drift likely. Run rebuild-frontends.sh and verify dashboard_ws_churn metric."
    },
    {
      "problem_key": "deploy_gap:bat_file_drift",
      "severity": "P1",
      "symptom_patterns": [
        "stale staged binary",
        "rc-agent process multiplication",
        "conspitlink 11 instances",
        "powershell watchdog 16 instances",
        "binary swap reverted",
        "bat file missing kill lines"
      ],
      "root_cause": "Pod's start-rcagent.bat is older than the repo version. Old bats lack: (a) staged binary cleanup lines (rc-agent-*.exe orphans), (b) ConspitLink singleton kill, (c) PowerShell cleanup. Result: deploy reverts because old bat swaps stale binary back, processes multiply, performance degrades.",
      "fix_action": "Compute SHA256 of deployed C:\\\\RacingPoint\\\\start-rcagent.bat, compare against scripts/deploy/start-rcagent.bat in repo. If mismatch, SCP the repo version to the pod. Reboot to apply.",
      "fix_status": "pending",
      "affects": ["pod_1", "pod_2", "pod_3", "pod_4", "pod_5", "pod_6", "pod_7", "pod_8"],
      "audit_date": "$IST_DATE",
      "escalation_message": "[DEPLOY GAP: bat_file_drift] Pod bat file does not match repo. Deploy will regress on next restart. SCP fresh bat from scripts/deploy/start-rcagent.bat and reboot."
    },
    {
      "problem_key": "deploy_gap:cloud_parity",
      "severity": "P1",
      "symptom_patterns": [
        "cloud build_id behind",
        "racingpoint.cloud stale",
        "bono vps old binary",
        "cloud frontend not rebuilt",
        "cloud admin shows old data"
      ],
      "root_cause": "Local server deploy (.23) succeeded but cloud (Bono VPS) was not updated. Customers on racingpoint.cloud see stale UI. Deploy parity rule violated: every local deploy MUST also deploy to cloud.",
      "fix_action": "git push from local repo. On Bono: curl -s -X POST http://localhost:8766/relay/exec/run -d '{\"command\":\"git_pull\"}'. Rebuild cloud racecontrol binary and pm2 restart. Verify build_id matches venue at /api/v1/health on both endpoints.",
      "fix_status": "pending",
      "affects": ["cloud_bono_vps"],
      "audit_date": "$IST_DATE",
      "escalation_message": "[DEPLOY GAP: cloud_parity] Cloud build is behind venue. Cloud customers see stale UI. Run git push + Bono git_pull + cloud rebuild."
    },
    {
      "problem_key": "deploy_gap:env_var_missing",
      "severity": "P1",
      "symptom_patterns": [
        "next_public_ws_url localhost",
        "websocket connects to localhost",
        "remote browser cannot connect ws",
        "kiosk works on server only",
        "pos cannot reach websocket"
      ],
      "root_cause": "NEXT_PUBLIC_ env vars are baked into Next.js builds at compile time. If not set, NEXT_PUBLIC_WS_URL falls back to localhost — works on the server browser, fails on every remote browser (POS, spectator, staff phones).",
      "fix_action": "Verify .env.production.local has: NEXT_PUBLIC_API_URL=http://192.168.31.23:8080, NEXT_PUBLIC_WS_URL=ws://192.168.31.23:8080. Rebuild affected frontend (kiosk/web/admin). Test from a non-server machine — curl proves HTML loads, only browser proves WS works.",
      "fix_status": "pending",
      "affects": ["server_23"],
      "audit_date": "$IST_DATE",
      "escalation_message": "[DEPLOY GAP: env_var_missing] NEXT_PUBLIC_ env var defaulted to localhost. Frontend builds must be rebuilt with proper LAN URL. Verify .env.production.local."
    },
    {
      "problem_key": "deploy_gap:config_not_synced",
      "severity": "P1",
      "symptom_patterns": [
        "pod toml drift",
        "pod config differs",
        "missing game from pod toml",
        "game launch fails track not found",
        "pod config sha256 mismatch"
      ],
      "root_cause": "Per-pod rc-agent.toml files in deploy/configs/ have drifted from what is actually deployed on each pod. Manual edits via SSH are not version-controlled. Result: game launches fail because pod config does not include the requested game.",
      "fix_action": "For each pod 1-8: SCP the deploy/configs/rc-agent-pod{N}.toml to C:\\\\RacingPoint\\\\rc-agent.toml. Reboot pod. Verify game catalog includes all expected games via /api/v1/games/catalog?pod=N.",
      "fix_status": "pending",
      "affects": ["pod_1", "pod_2", "pod_3", "pod_4", "pod_5", "pod_6", "pod_7", "pod_8"],
      "audit_date": "$IST_DATE",
      "escalation_message": "[DEPLOY GAP: config_not_synced] Pod TOML configs drifted from repo. SCP from deploy/configs/ to each pod and reboot."
    },
    {
      "problem_key": "deploy_gap:dynamic_audit",
      "severity": "P2",
      "symptom_patterns": [
        "deploy audit todo",
        "deploy manifest gap",
        "code complete not deployed"
      ],
      "root_cause": "Phase marked 'code complete' but deploy-audit.sh shows TODO items. Per Deploy Manifest Protocol (DMP), every phase must have a deploy: section and all DMP actions must be executed before the phase can ship.",
      "fix_action": "Run: bash scripts/deploy/deploy-audit.sh <old_hash> <new_hash>. Execute every [TODO] action listed. Re-run audit until exit code is 0 and no TODO items remain. Only then mark phase complete.",
      "fix_status": "pending",
      "affects": ["server_23", "pod_1", "pod_2", "pod_3", "pod_4", "pod_5", "pod_6", "pod_7", "pod_8", "pos_20", "cloud_bono_vps"],
      "audit_date": "$IST_DATE",
      "escalation_message": "[DEPLOY GAP: deploy_audit_todo] Run deploy-audit.sh and execute pending actions before claiming phase complete."
    }
  ]
}
EOF

echo "Seeding $(jq '.findings | length' "$PAYLOAD_FILE") deploy-gap entries to MI fleet KB at $SERVER_URL ..."

# Use service-key auth if available, fall back to staff token
if [[ -n "${RCAGENT_SERVICE_KEY:-}" ]]; then
  AUTH_HEADER="X-Service-Key: $RCAGENT_SERVICE_KEY"
  ENDPOINT="$SERVER_URL/api/v1/mesh/audit-seed-service"
elif [[ -n "${STAFF_TOKEN:-}" ]]; then
  AUTH_HEADER="Authorization: Bearer $STAFF_TOKEN"
  ENDPOINT="$SERVER_URL/api/v1/mesh/audit-seed"
else
  echo "ERROR: set RCAGENT_SERVICE_KEY or STAFF_TOKEN to authenticate against MI seed endpoint" >&2
  exit 1
fi

response="$(curl -s -w "\n%{http_code}" -X POST "$ENDPOINT" \
  -H "Content-Type: application/json" \
  -H "$AUTH_HEADER" \
  -d @"$PAYLOAD_FILE")"

http_code="$(echo "$response" | tail -n1)"
body="$(echo "$response" | sed '$d')"

if [[ "$http_code" == "200" ]]; then
  echo "OK ($http_code): $body"
  exit 0
else
  echo "FAIL ($http_code): $body" >&2
  exit 1
fi
