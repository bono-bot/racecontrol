#!/bin/bash
# Phase 1: Cloud Infrastructure Verification Script
# Run from any machine with internet access (for DNS/TLS checks)
# VPS-only checks are marked and require SSH access

set -euo pipefail

VPS="72.60.101.58"
DOMAIN="racingpoint.cloud"
SUBS="app admin dashboard api"
PASS=0
FAIL=0

pass() { echo "  PASS: $1"; ((PASS++)); }
fail() { echo "  FAIL: $1"; ((FAIL++)); }

echo "=== INFRA-01: DNS Resolution ==="
for sub in $SUBS; do
  fqdn="$sub.$DOMAIN"
  ip=$(dig +short "$fqdn" 2>/dev/null | tail -1)
  if [ "$ip" = "$VPS" ]; then
    pass "$fqdn -> $ip"
  else
    fail "$fqdn -> got '$ip', expected $VPS"
  fi
done

echo ""
echo "=== INFRA-02: TLS Certificates ==="
for sub in $SUBS; do
  fqdn="$sub.$DOMAIN"
  issuer=$(echo | openssl s_client -connect "$fqdn:443" -servername "$fqdn" 2>/dev/null | openssl x509 -noout -issuer 2>/dev/null || echo "CONNECT_FAILED")
  if echo "$issuer" | grep -qiE "let.s.encrypt|R3|R10|R11|E5|E6"; then
    pass "$fqdn: $issuer"
  elif echo "$issuer" | grep -qi "STAGING\|FAKE"; then
    fail "$fqdn: staging cert (switch to production): $issuer"
  else
    fail "$fqdn: $issuer"
  fi
done

echo ""
echo "=== INFRA-02: Security Headers ==="
for sub in $SUBS; do
  fqdn="$sub.$DOMAIN"
  hsts=$(curl -sI "https://$fqdn" 2>/dev/null | grep -i "strict-transport-security" || true)
  if [ -n "$hsts" ]; then
    pass "$fqdn: HSTS present"
  else
    fail "$fqdn: HSTS header missing"
  fi
done

echo ""
echo "=== INFRA-02: HTTP -> HTTPS Redirect ==="
for sub in $SUBS; do
  fqdn="$sub.$DOMAIN"
  status=$(curl -sI -o /dev/null -w "%{http_code}" "http://$fqdn" 2>/dev/null || echo "000")
  if [ "$status" = "301" ] || [ "$status" = "308" ]; then
    pass "$fqdn: HTTP redirects ($status)"
  else
    fail "$fqdn: HTTP status $status (expected 301 or 308)"
  fi
done

echo ""
echo "=== INFRA-03: Container Status (run on VPS) ==="
echo "  [manual] ssh root@$VPS 'docker compose -f /opt/racingpoint/compose.yml ps'"
echo "  Expected: caddy, pwa, admin, dashboard all 'Up (healthy)'"

echo ""
echo "=== INFRA-03: Memory Limits (run on VPS) ==="
echo "  [manual] ssh root@$VPS 'docker stats --no-stream --format \"{{.Name}}: {{.MemUsage}} / {{.MemLimit}}\"'"
echo "  Expected: caddy ~128M limit, pwa/admin/dashboard ~512M limit"

echo ""
echo "=== INFRA-06: Firewall (run on VPS) ==="
echo "  [manual] ssh root@$VPS 'sudo ufw status verbose'"
echo "  Expected: Default deny incoming, allow 22/tcp, 80/tcp, 443/tcp"

echo ""
echo "=== INFRA-07: Swap (run on VPS) ==="
echo "  [manual] ssh root@$VPS 'free -h | grep Swap'"
echo "  Expected: Swap total ~2.0G"

echo ""
echo "================================"
echo "Results: $PASS passed, $FAIL failed"
if [ "$FAIL" -gt 0 ]; then
  echo "STATUS: INCOMPLETE"
  exit 1
else
  echo "STATUS: ALL REMOTE CHECKS PASSED"
  echo "(VPS-local checks marked [manual] still need verification)"
  exit 0
fi
