#!/bin/bash
set -euo pipefail

# restart-dependents.sh — PM2 restart ordering with health checks between tiers
#
# Restarts PM2 services in correct dependency order (tier 0 first, tier 3 last)
# with health checks between tiers to ensure upstream services are ready.
#
# Usage:
#   bash scripts/restart-dependents.sh racecontrol racingpoint-dashboard
#   bash scripts/restart-dependents.sh --all
#   bash scripts/restart-dependents.sh --dry-run --all
#   bash scripts/restart-dependents.sh --dry-run racecontrol racingpoint-admin

# ANSI colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# Tier definitions (from DEPENDENCIES.json pm2_restart_order)
TIER_0=("racecontrol")
TIER_1=("racingpoint-api-gateway" "racecontrol-pwa")
TIER_2=("racingpoint-admin" "racingpoint-dashboard" "racingpoint-bot" "racingpoint-discord-bot")
TIER_3=("racingpoint-hiring" "racingpoint-website" "racingpoint-website-api" "bono-failsafe")

ALL_SERVICES=("${TIER_0[@]}" "${TIER_1[@]}" "${TIER_2[@]}" "${TIER_3[@]}")

# Parse flags
DRY_RUN=false
RESTART_ALL=false
REQUESTED_SERVICES=()

for arg in "$@"; do
  case "$arg" in
    --dry-run)
      DRY_RUN=true
      ;;
    --all)
      RESTART_ALL=true
      ;;
    *)
      REQUESTED_SERVICES+=("$arg")
      ;;
  esac
done

# If --all, use all services
if [ "$RESTART_ALL" = true ]; then
  REQUESTED_SERVICES=("${ALL_SERVICES[@]}")
fi

# Validate we have something to restart
if [ ${#REQUESTED_SERVICES[@]} -eq 0 ]; then
  echo -e "${RED}ERROR: No services specified. Use --all or provide service names.${RESET}"
  echo ""
  echo "Usage:"
  echo "  bash scripts/restart-dependents.sh <service1> <service2> ..."
  echo "  bash scripts/restart-dependents.sh --all"
  echo "  bash scripts/restart-dependents.sh --dry-run --all"
  echo ""
  echo "Available services:"
  echo "  Tier 0: ${TIER_0[*]}"
  echo "  Tier 1: ${TIER_1[*]}"
  echo "  Tier 2: ${TIER_2[*]}"
  echo "  Tier 3: ${TIER_3[*]}"
  exit 1
fi

# Helper: check if a value is in an array
contains() {
  local needle="$1"
  shift
  for item in "$@"; do
    if [ "$item" = "$needle" ]; then
      return 0
    fi
  done
  return 1
}

# Build per-tier restart lists
RESTART_T0=()
RESTART_T1=()
RESTART_T2=()
RESTART_T3=()

for svc in "${REQUESTED_SERVICES[@]}"; do
  if contains "$svc" "${TIER_0[@]}"; then
    RESTART_T0+=("$svc")
  elif contains "$svc" "${TIER_1[@]}"; then
    RESTART_T1+=("$svc")
  elif contains "$svc" "${TIER_2[@]}"; then
    RESTART_T2+=("$svc")
  elif contains "$svc" "${TIER_3[@]}"; then
    RESTART_T3+=("$svc")
  else
    echo -e "${YELLOW}WARNING: Unknown service '${svc}' — skipping${RESET}"
  fi
done

TOTAL_RESTARTED=0
TIERS_USED=0

# Helper: restart services in a tier
restart_tier() {
  local tier_num=$1
  shift
  local services=("$@")

  if [ ${#services[@]} -eq 0 ]; then
    return
  fi

  TIERS_USED=$((TIERS_USED + 1))
  echo -e "${BOLD}${CYAN}--- Tier ${tier_num} ---${RESET}"

  for svc in "${services[@]}"; do
    if [ "$DRY_RUN" = true ]; then
      echo -e "  ${YELLOW}[DRY RUN]${RESET} Would restart: ${BOLD}${svc}${RESET}"
    else
      echo -e "  Restarting: ${BOLD}${svc}${RESET}"
      if pm2 restart "$svc" --update-env > /dev/null 2>&1; then
        echo -e "  ${GREEN}OK${RESET}: ${svc} restarted"
      else
        echo -e "  ${YELLOW}WARN${RESET}: ${svc} may not be running in PM2 — skipped"
      fi
    fi
    TOTAL_RESTARTED=$((TOTAL_RESTARTED + 1))
  done
}

# Helper: health check with retries
check_health() {
  local url=$1
  local label=$2
  local retries=${3:-3}
  local delay=${4:-2}

  if [ "$DRY_RUN" = true ]; then
    echo -e "  ${YELLOW}[DRY RUN]${RESET} Would health-check: ${label} (${url})"
    return 0
  fi

  echo -e "  Checking health: ${label} (${url})"
  for i in $(seq 1 "$retries"); do
    if curl -sf --max-time 10 "$url" > /dev/null 2>&1; then
      echo -e "  ${GREEN}HEALTHY${RESET}: ${label}"
      return 0
    fi
    if [ "$i" -lt "$retries" ]; then
      echo -e "  ${YELLOW}Retry ${i}/${retries}${RESET} — waiting ${delay}s..."
      sleep "$delay"
    fi
  done

  echo -e "  ${RED}ERROR${RESET}: ${label} failed health check after ${retries} retries"
  return 1
}

echo ""
echo -e "${BOLD}PM2 Coordinated Restart${RESET}"
if [ "$DRY_RUN" = true ]; then
  echo -e "${YELLOW}(DRY RUN — no actual restarts)${RESET}"
fi
echo ""

# Tier 0: Core services
restart_tier 0 "${RESTART_T0[@]}"
if [ ${#RESTART_T0[@]} -gt 0 ]; then
  if ! check_health "http://localhost:8080/api/v1/health" "rc-core" 3 2; then
    echo -e "${RED}FATAL: rc-core health check failed — aborting restart sequence${RESET}"
    exit 1
  fi
  echo ""
fi

# Tier 1: Gateway and PWA
restart_tier 1 "${RESTART_T1[@]}"
if [ ${#RESTART_T1[@]} -gt 0 ]; then
  sleep 3 2>/dev/null || true
  if contains "racingpoint-api-gateway" "${RESTART_T1[@]}"; then
    check_health "http://localhost:3100/api/health" "api-gateway" 3 2 || true
  fi
  echo ""
fi

# Tier 2: Frontend services and bots
restart_tier 2 "${RESTART_T2[@]}"
if [ ${#RESTART_T2[@]} -gt 0 ]; then
  sleep 2 2>/dev/null || true
  echo ""
fi

# Tier 3: Auxiliary services
restart_tier 3 "${RESTART_T3[@]}"
if [ ${#RESTART_T3[@]} -gt 0 ]; then
  echo ""
fi

# Summary
echo -e "${BOLD}${GREEN}Done.${RESET} Restarted ${TOTAL_RESTARTED} service(s) across ${TIERS_USED} tier(s)."
