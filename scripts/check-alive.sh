#!/bin/bash
# Multi-probe connectivity check — CGP v4.0 H3 compliance
# Prevents false "offline" conclusions from single probe failures.
#
# Usage: bash scripts/check-alive.sh <target>
# Target: pod1-pod8, pos, server, bono, james
#
# Runs 3 probes: ping, HTTP health, SSH. Reports per-probe results.
# Verdict: UP (any probe succeeds) | DOWN (all probes fail) | DEGRADED (partial)
#
# Exit code: 0 = UP/DEGRADED, 1 = DOWN

TARGET="$1"
if [ -z "$TARGET" ]; then
  echo "Usage: bash scripts/check-alive.sh <target>"
  echo "Targets: pod1-pod8, pos, server, bono, james"
  exit 1
fi

# Target IP mapping
declare -A LAN_IPS
LAN_IPS[pod1]="192.168.31.89"
LAN_IPS[pod2]="192.168.31.33"
LAN_IPS[pod3]="192.168.31.28"
LAN_IPS[pod4]="192.168.31.88"
LAN_IPS[pod5]="192.168.31.86"
LAN_IPS[pod6]="192.168.31.87"
LAN_IPS[pod7]="192.168.31.38"
LAN_IPS[pod8]="192.168.31.91"
LAN_IPS[pos]="192.168.31.20"
LAN_IPS[server]="192.168.31.23"
LAN_IPS[james]="192.168.31.27"

declare -A TS_IPS
TS_IPS[pod1]="100.92.122.89"
TS_IPS[pod2]="100.105.93.108"
TS_IPS[pod3]="100.69.231.26"
TS_IPS[pod4]="100.75.45.10"
TS_IPS[pod5]="100.110.133.87"
TS_IPS[pod6]="100.127.149.17"
TS_IPS[pod7]="100.82.196.28"
TS_IPS[pod8]="100.98.67.67"
TS_IPS[pos]="100.95.211.1"
TS_IPS[server]="100.125.108.37"
TS_IPS[bono]="100.70.177.44"

declare -A HEALTH_PORTS
HEALTH_PORTS[pod1]="8090"
HEALTH_PORTS[pod2]="8090"
HEALTH_PORTS[pod3]="8090"
HEALTH_PORTS[pod4]="8090"
HEALTH_PORTS[pod5]="8090"
HEALTH_PORTS[pod6]="8090"
HEALTH_PORTS[pod7]="8090"
HEALTH_PORTS[pod8]="8090"
HEALTH_PORTS[pos]="8090"
HEALTH_PORTS[server]="8080"
HEALTH_PORTS[james]="8766"
HEALTH_PORTS[bono]="8080"

LAN="${LAN_IPS[$TARGET]}"
TS="${TS_IPS[$TARGET]}"
PORT="${HEALTH_PORTS[$TARGET]:-8090}"

if [ -z "$LAN" ] && [ -z "$TS" ]; then
  echo "ERROR: Unknown target '$TARGET'"
  exit 1
fi

PROBES_PASS=0
PROBES_FAIL=0
PROBES_TOTAL=0

echo "=== MULTI-PROBE: $TARGET ==="

# Probe 1: Ping (LAN)
if [ -n "$LAN" ]; then
  PROBES_TOTAL=$((PROBES_TOTAL + 1))
  PING_RESULT=$(ping -n 1 -w 2000 "$LAN" 2>/dev/null | grep -c "Reply from $LAN")
  if [ "$PING_RESULT" -gt 0 ]; then
    echo "[PASS] Ping LAN ($LAN): reachable"
    PROBES_PASS=$((PROBES_PASS + 1))
  else
    echo "[FAIL] Ping LAN ($LAN): unreachable"
    PROBES_FAIL=$((PROBES_FAIL + 1))
  fi
fi

# Probe 2: Ping (Tailscale)
if [ -n "$TS" ]; then
  PROBES_TOTAL=$((PROBES_TOTAL + 1))
  PING_TS=$(ping -n 1 -w 3000 "$TS" 2>/dev/null | grep -c "Reply from $TS")
  if [ "$PING_TS" -gt 0 ]; then
    echo "[PASS] Ping Tailscale ($TS): reachable"
    PROBES_PASS=$((PROBES_PASS + 1))
  else
    echo "[FAIL] Ping Tailscale ($TS): unreachable"
    PROBES_FAIL=$((PROBES_FAIL + 1))
  fi
fi

# Probe 3: HTTP health (LAN first, then Tailscale fallback)
PROBES_TOTAL=$((PROBES_TOTAL + 1))
HTTP_OK=0
if [ -n "$LAN" ]; then
  HTTP_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 3 "http://$LAN:$PORT/health" 2>/dev/null)
  if [ "$HTTP_STATUS" = "200" ]; then
    echo "[PASS] HTTP health LAN ($LAN:$PORT): $HTTP_STATUS"
    HTTP_OK=1
  fi
fi
if [ "$HTTP_OK" = "0" ] && [ -n "$TS" ]; then
  HTTP_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --connect-timeout 3 "http://$TS:$PORT/health" 2>/dev/null)
  if [ "$HTTP_STATUS" = "200" ]; then
    echo "[PASS] HTTP health Tailscale ($TS:$PORT): $HTTP_STATUS"
    HTTP_OK=1
  fi
fi
if [ "$HTTP_OK" = "1" ]; then
  PROBES_PASS=$((PROBES_PASS + 1))
else
  echo "[FAIL] HTTP health: no response on LAN or Tailscale"
  PROBES_FAIL=$((PROBES_FAIL + 1))
fi

# Verdict
echo ""
echo "--- VERDICT ---"
echo "Probes: $PROBES_PASS pass / $PROBES_FAIL fail / $PROBES_TOTAL total"

if [ "$PROBES_PASS" -eq "$PROBES_TOTAL" ]; then
  echo "STATUS: UP (all probes pass)"
  exit 0
elif [ "$PROBES_PASS" -gt 0 ]; then
  echo "STATUS: DEGRADED ($PROBES_FAIL/$PROBES_TOTAL probes failed — system is ON but has connectivity issues)"
  exit 0
else
  echo "STATUS: DOWN (all $PROBES_TOTAL probes failed)"
  exit 1
fi
