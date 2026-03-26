#!/usr/bin/env bash
# audit/phases/tier1/phase03.sh -- Phase 03: Network & Tailscale
# Tier: 1 (Infrastructure Foundation)
# What: Tailscale connected, LAN reachable, Bono VPS reachable.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase03() {
  local phase="03" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # James Tailscale status
  local ts_output; ts_output=$(tailscale status 2>/dev/null || echo "OFFLINE")
  if printf '%s' "$ts_output" | grep -q "100\."; then
    status="PASS"; severity="P3"; message="Tailscale active on James"
  else
    status="WARN"; severity="P2"; message="Tailscale not responding or offline on James"
  fi
  emit_result "$phase" "$tier" "james-tailscale" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Pods LAN ping to server .23
  local ip host
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"
    response=$(safe_remote_exec "$ip" "8090" \
      'ping -n 1 192.168.31.23' \
      "$DEFAULT_TIMEOUT")
    local stdout; stdout=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
    if printf '%s' "$stdout" | grep -qiE "Reply from|bytes="; then
      status="PASS"; severity="P3"; message="Pod can reach server .23 via LAN"
    elif [[ -z "$stdout" ]]; then
      status="WARN"; severity="P2"; message="Pod offline -- cannot verify LAN connectivity"
    else
      status="FAIL"; severity="P2"; message="Pod cannot ping server .23: ${stdout:0:80}"
    fi
    if [[ "$venue_state" = "closed" ]] && [[ "$status" = "FAIL" || "$status" = "WARN" ]]; then
      status="QUIET"; severity="P3"
    fi
    emit_result "$phase" "$tier" "${host}-lan" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  # Server .23 -> Bono VPS via Tailscale
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'curl.exe -s -m 5 http://100.70.177.44:8080/api/v1/health' \
    15)
  local cloud_resp; cloud_resp=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
  if printf '%s' "$cloud_resp" | grep -q "build_id"; then
    status="PASS"; severity="P3"; message="Server .23 -> Bono VPS Tailscale path working"
  elif [[ -z "$cloud_resp" ]]; then
    status="WARN"; severity="P2"; message="Server .23 -> Bono VPS unreachable (cloud degraded, not critical)"
  else
    status="WARN"; severity="P2"; message="Server .23 -> Bono VPS response unexpected: ${cloud_resp:0:60}"
  fi
  emit_result "$phase" "$tier" "server-to-bono" "$status" "$severity" "$message" "$mode" "$venue_state"

  # POS PC reachable — try known LAN IPs then Tailscale fallback
  # POS gets varying DHCP leases (.20, .130, .135) until router reservation is set
  local pos_found="" pos_ip=""
  for try_ip in 192.168.31.20 192.168.31.130 192.168.31.135; do
    response=$(http_get "http://${try_ip}:8090/health" 3)
    if printf '%s' "$response" | grep -q "build_id"; then
      pos_found="$response"; pos_ip="$try_ip"; break
    fi
  done
  if [[ -z "$pos_found" ]]; then
    # Tailscale fallback
    response=$(http_get "http://100.95.211.1:8090/health" 5)
    if printf '%s' "$response" | grep -q "build_id"; then
      pos_found="$response"; pos_ip="100.95.211.1 (Tailscale)"
    fi
  fi
  if [[ -n "$pos_found" ]]; then
    local pos_build; pos_build=$(printf '%s' "$pos_found" | jq -r '.build_id // "unknown"' 2>/dev/null)
    status="PASS"; severity="P3"; message="POS PC rc-agent reachable at ${pos_ip} (build: ${pos_build})"
    export POS_IP="$pos_ip"
  else
    status="WARN"; severity="P2"; message="POS PC unreachable (tried .20/.130/.135 + Tailscale)"
  fi
  emit_result "$phase" "$tier" "pos-pc" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase03
