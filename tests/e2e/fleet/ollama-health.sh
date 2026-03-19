#!/bin/bash
# tests/e2e/fleet/ollama-health.sh
# Verifies Ollama + rp-debug model health on all 8 pods.
# Gate 1: rp-debug model present (curl /api/tags)
# Gate 2: rp-debug responds to prompt in <5s (curl /api/generate)
#
# Usage: bash tests/e2e/fleet/ollama-health.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

RESPONSE_TIMEOUT_MS=5000

info "Ollama Fleet Health Check (Phase 47: Local LLM Deployment)"
echo ""

for POD_NUM in $(seq 1 8); do
    POD_ID="pod-${POD_NUM}"
    POD_IP=$(pod_ip "$POD_ID")

    if [ -z "$POD_IP" ]; then
        skip "${POD_ID}: no IP mapping"
        continue
    fi

    # Check if pod is reachable first (1s timeout)
    PING_RESP=$(curl -s --connect-timeout 1 --max-time 2 "http://${POD_IP}:8090/ping" 2>/dev/null)
    if [ "$PING_RESP" != "pong" ]; then
        skip "${POD_ID} (${POD_IP}): rc-agent not reachable on :8090"
        continue
    fi

    # --- Gate 1: rp-debug model presence ---
    # Execute via :8090/exec to check Ollama's model list on localhost:11434
    TAGS_RESP=$(curl -s --connect-timeout 3 --max-time 10 \
        -X POST "http://${POD_IP}:8090/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"curl -s http://127.0.0.1:11434/api/tags","timeout_ms":5000}' \
        2>/dev/null)

    if [ -z "$TAGS_RESP" ]; then
        fail "${POD_ID} (${POD_IP}): Gate 1 — exec endpoint did not respond for /api/tags"
        continue
    fi

    # Extract stdout from exec response and check for rp-debug
    TAGS_STDOUT=$(echo "$TAGS_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('stdout',''))" 2>/dev/null)

    if echo "$TAGS_STDOUT" | grep -q "rp-debug"; then
        pass "${POD_ID} (${POD_IP}): Gate 1 — rp-debug model present in /api/tags"
    else
        fail "${POD_ID} (${POD_IP}): Gate 1 — rp-debug model NOT found in /api/tags"
        # Still attempt Gate 2 even if Gate 1 fails — more diagnostic info
    fi

    # --- Gate 2: rp-debug response time <5s ---
    # Measure wall clock time for the full round-trip:
    # James's machine -> pod :8090 -> pod's Ollama localhost:11434 -> response back
    # If the total round-trip completes in <5s, the LLM is healthy and responsive.
    START_MS=$(date +%s%3N)
    EXEC_RESP=$(curl -s --connect-timeout 3 --max-time 15 \
        -X POST "http://${POD_IP}:8090/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"curl -s --max-time 8 -X POST http://127.0.0.1:11434/api/generate -d \"{\\\"model\\\":\\\"rp-debug\\\",\\\"prompt\\\":\\\"diagnose: test ping\\\",\\\"stream\\\":false}\"","timeout_ms":10000}' \
        2>/dev/null)
    END_MS=$(date +%s%3N)
    ELAPSED_MS=$((END_MS - START_MS))

    if [ -z "$EXEC_RESP" ]; then
        fail "${POD_ID} (${POD_IP}): Gate 2 — exec endpoint did not respond for /api/generate"
        continue
    fi

    # Extract stdout from exec response
    GEN_STDOUT=$(echo "$EXEC_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('stdout',''))" 2>/dev/null)

    # Check that the generate response contains a non-empty "response" field
    HAS_RESPONSE=$(echo "$GEN_STDOUT" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
    resp = d.get('response', '')
    print('yes' if resp else 'empty')
except:
    print('parse_error')
" 2>/dev/null)

    if [ "$HAS_RESPONSE" != "yes" ]; then
        fail "${POD_ID} (${POD_IP}): Gate 2 — /api/generate returned no response field (got: ${HAS_RESPONSE})"
        continue
    fi

    # Check timing: full round-trip must be <5000ms
    if [ "$ELAPSED_MS" -lt "$RESPONSE_TIMEOUT_MS" ]; then
        pass "${POD_ID} (${POD_IP}): Gate 2 — rp-debug responded in ${ELAPSED_MS}ms (< ${RESPONSE_TIMEOUT_MS}ms)"
    else
        fail "${POD_ID} (${POD_IP}): Gate 2 — rp-debug response too slow: ${ELAPSED_MS}ms (>= ${RESPONSE_TIMEOUT_MS}ms threshold)"
    fi
done

echo ""
summary_exit
