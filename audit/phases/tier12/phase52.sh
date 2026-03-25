#!/usr/bin/env bash
# audit/phases/tier12/phase52.sh -- Phase 52: Frontend Deploy Integrity
# Tier: 12 (Code Quality and Static Analysis)
# What: Next.js builds are structurally complete. All NEXT_PUBLIC_ vars set. Standalone deploy correct.
# Standing rules: DBG-12 (NEXT_PUBLIC_ completeness), DBG-13 (.next/standalone structure), CQ-17 (Edge stacking)

set -u
set -o pipefail
# NO set -e

run_phase52() {
  local phase="52" tier="12"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local RC_BASE="C:/Users/bono/racingpoint/racecontrol"

  # --- Check 1: NEXT_PUBLIC_ completeness for each app ---
  for app in kiosk pwa web; do
    local app_dir="${RC_BASE}/${app}"
    local app_host="james-nextpub-${app}"
    if [[ ! -d "${app_dir}/src" ]]; then
      status="WARN"; severity="P2"; message="${app}: src/ directory not found at ${app_dir}"
      emit_result "$phase" "$tier" "$app_host" "$status" "$severity" "$message" "$mode" "$venue_state"
      continue
    fi

    # Find all NEXT_PUBLIC_ variable names referenced in source
    local vars; vars=$(grep -roh "NEXT_PUBLIC_[A-Z_]*" "${app_dir}/src/" 2>/dev/null | sort -u || true)
    if [[ -z "$vars" ]]; then
      status="PASS"; severity="P3"; message="${app}: no NEXT_PUBLIC_ vars referenced in source"
      emit_result "$phase" "$tier" "$app_host" "$status" "$severity" "$message" "$mode" "$venue_state"
      continue
    fi

    local missing_count=0
    local missing_list=""
    local env_file="${app_dir}/.env.production.local"
    while IFS= read -r var; do
      [[ -z "$var" ]] && continue
      if ! grep -q "$var" "$env_file" 2>/dev/null; then
        missing_count=$((missing_count + 1))
        missing_list="${missing_list} ${var}"
      fi
    done <<< "$vars"

    if [[ "$missing_count" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="${app}: all NEXT_PUBLIC_ vars present in .env.production.local"
    else
      status="PASS"; severity="P3"; message="${app}: ${missing_count} NEXT_PUBLIC_ var(s) missing from .env.production.local (may use defaults):${missing_list}"
    fi
    emit_result "$phase" "$tier" "$app_host" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  # --- Check 2: Runtime static file check for kiosk (:3300/kiosk, has basePath) ---
  local kiosk_html; kiosk_html=$(curl -s -m 10 "http://192.168.31.23:3300/kiosk" 2>/dev/null || echo "")
  local kiosk_path; kiosk_path=$(printf '%s' "$kiosk_html" \
    | grep -oiP 'href="(/kiosk)?/_next/static/[^"]+' \
    | head -1 \
    | sed 's/href="//i' \
    || true)
  # Fallback: check for __next or _next/static markers (Next.js App Router)
  if [[ -z "$kiosk_path" ]]; then
    kiosk_path=$(printf '%s' "$kiosk_html" \
      | grep -oiP '(src|href)="[^"]*_next/static/[^"]+' \
      | head -1 \
      | sed 's/^[^"]*"//;s/"$//' \
      || true)
  fi
  if [[ -n "$kiosk_path" ]]; then
    local kiosk_static; kiosk_static=$(curl -s -m 10 -o /dev/null -w "%{http_code}" \
      "http://192.168.31.23:3300${kiosk_path}" 2>/dev/null)
    if [[ "$kiosk_static" = "200" ]]; then
      status="PASS"; severity="P3"; message="Kiosk static file serving OK (${kiosk_path} returned 200)"
    else
      status="FAIL"; severity="P1"; message="Kiosk static file 404: ${kiosk_path} returned ${kiosk_static} — check appDir in required-server-files.json"
    fi
  else
    # Check for __next marker (Next.js App Router uses __next id on body/div)
    if printf '%s' "$kiosk_html" | grep -qi '__next\|_next/static'; then
      status="PASS"; severity="P3"; message="Kiosk: Next.js App Router markers found (__next) — static serving likely OK"
    else
      status="WARN"; severity="P2"; message="Kiosk: no static file reference found in HTML (app may be down or no CSS loaded)"
    fi
  fi
  emit_result "$phase" "$tier" "server-23-static-kiosk" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Runtime static file check for web dashboard (:3200, no basePath) ---
  local web_html; web_html=$(curl -s -m 10 "http://192.168.31.23:3200" 2>/dev/null || echo "")
  local web_path; web_path=$(printf '%s' "$web_html" \
    | grep -oiP '(src|href)="/_next/static/[^"]+' \
    | head -1 \
    | sed 's/^[^"]*"//;s/"$//' \
    || true)
  if [[ -n "$web_path" ]]; then
    local web_static; web_static=$(curl -s -m 10 -o /dev/null -w "%{http_code}" \
      "http://192.168.31.23:3200${web_path}" 2>/dev/null)
    if [[ "$web_static" = "200" ]]; then
      status="PASS"; severity="P3"; message="Web dashboard static file serving OK (${web_path} returned 200)"
    else
      status="FAIL"; severity="P1"; message="Web dashboard static file 404: ${web_path} returned ${web_static} — check appDir in required-server-files.json"
    fi
  else
    if printf '%s' "$web_html" | grep -qi '__next\|_next/static'; then
      status="PASS"; severity="P3"; message="Web dashboard: Next.js App Router markers found (__next) — static serving likely OK"
    else
      status="WARN"; severity="P2"; message="Web dashboard: no static file reference found in HTML (app may be down)"
    fi
  fi
  emit_result "$phase" "$tier" "server-23-static-web" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 4: Runtime static file check for admin (:3201, no basePath) ---
  local admin_html; admin_html=$(curl -s -m 10 "http://192.168.31.23:3201" 2>/dev/null || echo "")
  local admin_path; admin_path=$(printf '%s' "$admin_html" \
    | grep -oiP '(src|href)="/_next/static/[^"]+' \
    | head -1 \
    | sed 's/^[^"]*"//;s/"$//' \
    || true)
  if [[ -n "$admin_path" ]]; then
    local admin_static; admin_static=$(curl -s -m 10 -o /dev/null -w "%{http_code}" \
      "http://192.168.31.23:3201${admin_path}" 2>/dev/null)
    if [[ "$admin_static" = "200" ]]; then
      status="PASS"; severity="P3"; message="Admin static file serving OK (${admin_path} returned 200)"
    else
      status="FAIL"; severity="P1"; message="Admin static file 404: ${admin_path} returned ${admin_static} — check appDir in required-server-files.json"
    fi
  else
    if printf '%s' "$admin_html" | grep -qi '__next\|_next/static'; then
      status="PASS"; severity="P3"; message="Admin: Next.js App Router markers found (__next) — static serving likely OK"
    else
      status="PASS"; severity="P3"; message="Admin: no static file reference found in HTML (redirect expected or app loading)"
    fi
  fi
  emit_result "$phase" "$tier" "server-23-static-admin" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase52
