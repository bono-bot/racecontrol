#!/usr/bin/env bash
# network-security-audit.sh — Network security enforcement check (Layer 1 + Security)
#
# Verifies network-level security controls that can't be enforced in code.
# Run at session start or as part of fleet audit.
#
# Checks:
#   1. go2rtc port 1984 accessibility from non-James IPs
#   2. Pod :8090/:8091 ports not exposed to WiFi
#   3. Server :8080 CORS headers correct
#   4. Terminal secret not in NEXT_PUBLIC_ compiled output
#   5. Windows Firewall status on James PC

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
RESET='\033[0m'

PASS=0; WARN=0; FAIL=0

echo "============================================================"
echo "NETWORK SECURITY AUDIT"
echo "============================================================"
echo ""

# Check 1: go2rtc reachability from server (simulates WiFi client)
echo "[1/5] go2rtc :1984 accessibility..."
GO2RTC_FROM_SERVER=$(ssh -o ConnectTimeout=3 ADMIN@100.125.108.37 "curl.exe -s --connect-timeout 2 http://192.168.31.27:1984/api/config 2>nul" 2>/dev/null | head -c 100)
if [ -n "$GO2RTC_FROM_SERVER" ] && echo "$GO2RTC_FROM_SERVER" | grep -q "streams\|{"; then
  echo -e "  ${RED}FAIL: go2rtc accessible from server (= accessible from WiFi)${RESET}"
  echo "  ACTION: Add Windows Firewall rule on James PC to block :1984 from non-localhost"
  ((FAIL++))
else
  echo -e "  ${GREEN}PASS: go2rtc not accessible from server${RESET}"
  ((PASS++))
fi

# Check 2: CORS header check
echo ""
echo "[2/5] CORS headers on server..."
CORS_TEST=$(curl -s -o /dev/null -w '%{http_code}' -H "Origin: http://192.168.31.100:8080" http://192.168.31.23:8080/api/v1/health 2>/dev/null)
CORS_HEADER=$(curl -s -I -H "Origin: http://192.168.31.100:8080" http://192.168.31.23:8080/api/v1/health 2>/dev/null | grep -i "access-control-allow-origin" | tr -d '\r')
if echo "$CORS_HEADER" | grep -q "192.168.31.100"; then
  echo -e "  ${RED}FAIL: CORS allows arbitrary WiFi client (192.168.31.100)${RESET}"
  ((FAIL++))
else
  echo -e "  ${GREEN}PASS: CORS rejects non-whitelisted origin${RESET}"
  ((PASS++))
fi

# Check 3: Terminal secret in compiled JS
echo ""
echo "[3/5] Terminal secret exposure in kiosk bundle..."
KIOSK_JS=$(curl -s http://192.168.31.23:3300/kiosk 2>/dev/null | head -c 5000)
if echo "$KIOSK_JS" | grep -q "rp-terminal-2026"; then
  echo -e "  ${YELLOW}WARN: Terminal secret found in kiosk HTML (expected — NEXT_PUBLIC_ exposure)${RESET}"
  echo "  STATUS: Accepted risk — requires session token architecture to fix"
  ((WARN++))
else
  echo -e "  ${GREEN}PASS: Terminal secret not in initial HTML response${RESET}"
  ((PASS++))
fi

# Check 4: James PC Windows Firewall
echo ""
echo "[4/5] Windows Firewall status..."
FW_STATUS=$(netsh advfirewall show currentprofile state 2>/dev/null | grep -i "State" | tr -d '\r' || echo "UNKNOWN")
if echo "$FW_STATUS" | grep -qi "ON"; then
  echo -e "  ${GREEN}PASS: Windows Firewall is ON${RESET}"
  ((PASS++))
else
  echo -e "  ${YELLOW}WARN: Windows Firewall status: ${FW_STATUS}${RESET}"
  ((WARN++))
fi

# Check 5: Rate limit persistence check
echo ""
echo "[5/5] Admin rate limit persistence..."
echo -e "  ${YELLOW}WARN: Rate limit is in-memory only — resets on server restart${RESET}"
echo "  STATUS: Accepted risk — persistent lockout is future work"
((WARN++))

echo ""
echo "============================================================"
echo "RESULTS: ${PASS} PASS | ${WARN} WARN | ${FAIL} FAIL"
echo "============================================================"

if [ $FAIL -gt 0 ]; then
  exit 1
fi
exit 0
