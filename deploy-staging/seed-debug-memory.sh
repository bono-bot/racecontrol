#!/bin/bash
# deploy-staging/seed-debug-memory.sh
# Pre-seeds debug-memory.json on all 8 pods with 7 deterministic fix patterns.
# These patterns enable instant fix replay (<100ms) from first boot for the most
# common crash scenarios, bypassing LLM round-trip on known issues.
#
# Usage: bash deploy-staging/seed-debug-memory.sh
#
# Requires: rc-agent :8090 running on each pod (uses /write endpoint)
# Fallback: /exec with PowerShell if /write returns non-"written" status

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_ROOT=$(cd "$SCRIPT_DIR/.." && pwd)
source "$REPO_ROOT/tests/e2e/lib/common.sh"
source "$REPO_ROOT/tests/e2e/lib/pod-map.sh"

info "Debug Memory Seed — writing 7 deterministic fix patterns to all pods"
echo ""

# ── 7 pre-seeded DebugIncident entries (DebugMemory struct, ai_debugger.rs) ──
# pattern_key format: "{SimType:?}:{exit_code}" as produced by pattern_key() fn
# success_count=1 satisfies instant_fix() threshold so memory is used from boot
#
# Each entry on its own line so grep -c "pattern_key" returns 7 (one per pattern).
INCIDENT_1='{"pattern_key":"AssettoCorsa:-1","fix_type":"fix_kill_stale_game","ai_suggestion":"Game crashed with exit code -1. This is typically a shader cache corruption or mod conflict. Recommend: relaunch game. Kill stale acs.exe process and relaunch.","success_count":1,"last_seen":"2026-03-19T00:00:00Z"}'
INCIDENT_2='{"pattern_key":"AssettoCorsa:unknown","fix_type":"fix_frozen_game","ai_suggestion":"Game appears frozen (not responding). Kill the game frozen process and relaunch. FFB will be zeroed first for safety.","success_count":1,"last_seen":"2026-03-19T00:00:00Z"}'
INCIDENT_3='{"pattern_key":"F125:3221225477","fix_type":"fix_kill_stale_game","ai_suggestion":"F1 25 crashed with access violation (0xC0000005). DirectX shader cache may be corrupted. Relaunch game after clearing stale process.","success_count":1,"last_seen":"2026-03-19T00:00:00Z"}'
INCIDENT_4='{"pattern_key":"AssettoCorsa:1","fix_type":"kill_error_dialogs","ai_suggestion":"WerFault error dialog detected blocking the screen. Kill error dialogs (WerFault.exe, WerFaultSecure.exe) to clear the display.","success_count":1,"last_seen":"2026-03-19T00:00:00Z"}'
INCIDENT_5='{"pattern_key":"AssettoCorsa:socket","fix_type":"clear_stale_sockets","ai_suggestion":"CLOSE_WAIT zombie sockets detected on :8090. Stale socket cleanup needed. Clear CLOSE_WAIT connections on ports 8090, 18923, 18924, 18925.","success_count":1,"last_seen":"2026-03-19T00:00:00Z"}'
INCIDENT_6='{"pattern_key":"AssettoCorsa:disk","fix_type":"clean_temp","ai_suggestion":"Low disk space detected. Clean temp files to free disk space. Remove temp directory contents.","success_count":1,"last_seen":"2026-03-19T00:00:00Z"}'
INCIDENT_7='{"pattern_key":"AssettoCorsa:hid","fix_type":"fix_usb_reconnect","ai_suggestion":"Wheelbase HID device disconnected. USB reset needed. Zero FFB on wheelbase reconnect to clear stale state.","success_count":1,"last_seen":"2026-03-19T00:00:00Z"}'

# Assemble into DebugMemory JSON (incidents array)
DEBUG_MEMORY_JSON=$(python3 -c "
import json, sys

incidents = [json.loads(x) for x in sys.argv[1:]]
memory = {'incidents': incidents}
print(json.dumps(memory, separators=(',', ':')))
" \
    "$INCIDENT_1" "$INCIDENT_2" "$INCIDENT_3" "$INCIDENT_4" \
    "$INCIDENT_5" "$INCIDENT_6" "$INCIDENT_7")

if [ -z "$DEBUG_MEMORY_JSON" ]; then
    echo "ERROR: Failed to assemble debug-memory JSON (python3 error)"
    exit 1
fi

TARGET_PATH='C:\\RacingPoint\\debug-memory.json'

for POD_NUM in $(seq 1 8); do
    POD_ID="pod-${POD_NUM}"
    pod_ip "$POD_ID" > /dev/null 2>&1 || { skip "${POD_ID}: no IP mapping"; continue; }
    POD_IP=$(pod_ip "$POD_ID")

    # Check reachability (same pattern as close-wait.sh)
    PING_RESP=$(curl -s --connect-timeout 1 --max-time 2 "http://${POD_IP}:8090/ping" 2>/dev/null)
    if [ "$PING_RESP" != "pong" ]; then
        skip "${POD_ID} (${POD_IP}): rc-agent not reachable on :8090"
        continue
    fi

    # Build JSON payload for /write endpoint: {"path": "...", "content": "..."}
    # Use python3 to produce clean JSON with proper escaping of the content string
    WRITE_PAYLOAD=$(python3 -c "
import json, sys
payload = {
    'path': sys.argv[1],
    'content': sys.argv[2]
}
print(json.dumps(payload))
" "$TARGET_PATH" "$DEBUG_MEMORY_JSON" 2>/dev/null)

    if [ -z "$WRITE_PAYLOAD" ]; then
        fail "${POD_ID} (${POD_IP}): failed to build write payload (python3 error)"
        continue
    fi

    # Primary: POST to /write endpoint
    WRITE_RESP=$(curl -s --connect-timeout 3 --max-time 10 \
        -X POST "http://${POD_IP}:8090/write" \
        -H "Content-Type: application/json" \
        -d "$WRITE_PAYLOAD" \
        2>/dev/null)

    if echo "$WRITE_RESP" | grep -q '"status":"written"'; then
        BYTES=$(echo "$WRITE_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('bytes',0))" 2>/dev/null)
        pass "${POD_ID} (${POD_IP}): debug-memory.json written (${BYTES} bytes)"
        continue
    fi

    # Fallback: /exec with PowerShell to write the file
    info "${POD_ID} (${POD_IP}): /write returned '${WRITE_RESP}', trying /exec fallback"

    # Escape single quotes for PowerShell string (replace ' with '')
    PS_CONTENT=$(echo "$DEBUG_MEMORY_JSON" | sed "s/'/''/g")
    PS_CMD="Set-Content -Path 'C:\\RacingPoint\\debug-memory.json' -Value '${PS_CONTENT}' -Encoding UTF8"

    EXEC_PAYLOAD=$(python3 -c "
import json, sys
payload = {
    'cmd': 'powershell -NonInteractive -Command \"' + sys.argv[1] + '\"',
    'timeout_ms': 5000
}
print(json.dumps(payload))
" "$PS_CMD" 2>/dev/null)

    EXEC_RESP=$(curl -s --connect-timeout 3 --max-time 10 \
        -X POST "http://${POD_IP}:8090/exec" \
        -H "Content-Type: application/json" \
        -d "$EXEC_PAYLOAD" \
        2>/dev/null)

    if echo "$EXEC_RESP" | grep -qE '"exit_code"\s*:\s*0'; then
        pass "${POD_ID} (${POD_IP}): debug-memory.json written via /exec fallback"
    else
        fail "${POD_ID} (${POD_IP}): both /write and /exec failed — response: ${EXEC_RESP}"
    fi
done

echo ""
summary_exit
