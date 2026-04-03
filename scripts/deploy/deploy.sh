#!/usr/bin/env bash
# deploy.sh — unified deploy script for Racing Point services
# Usage: bash deploy.sh <service>
# Services: racecontrol | rc-sentry | kiosk | web | comms-link | cloud
# Runs from James (.27). Calls check-health.sh after deploy.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER="192.168.31.23"
SERVICE="${1:-}"

if [ -z "$SERVICE" ]; then
  echo "Usage: bash deploy.sh <service>"
  echo "Services: racecontrol | rc-sentry | kiosk | web | comms-link | cloud"
  exit 1
fi

run_health_check() {
  echo ""
  echo "--- Post-deploy health check ---"
  bash "${SCRIPT_DIR}/check-health.sh" || {
    echo "DEPLOY INCOMPLETE — health check failed after deploying ${SERVICE}"
    exit 1
  }
}

case "$SERVICE" in
  racecontrol)
    echo "=== Deploying racecontrol to server ${SERVER} ==="
    # Use staged binary if available; rebuild only as fallback
    if [ -f "${SCRIPT_DIR}/racecontrol.exe" ]; then
      echo "Using staged binary: ${SCRIPT_DIR}/racecontrol.exe"
    else
      echo "WARNING: No staged binary in deploy-staging/. Rebuilding from source..."
      cd "$(dirname "${SCRIPT_DIR}")/racecontrol" 2>/dev/null || cd /c/Users/bono/racingpoint/racecontrol
      cargo build --release --bin racecontrol
      cp target/release/racecontrol.exe "${SCRIPT_DIR}/racecontrol.exe"
    fi
    # SCP to server
    scp "${SCRIPT_DIR}/racecontrol.exe" "ADMIN@${SERVER}:C:/RacingPoint/racecontrol-new.exe"
    # Rename swap (standing rule: rename, don't overwrite — Windows locks running .exe)
    ssh "ADMIN@${SERVER}" "cd /d C:\\RacingPoint && del racecontrol-old.exe 2>nul & ren racecontrol.exe racecontrol-old.exe & ren racecontrol-new.exe racecontrol.exe & taskkill /F /IM racecontrol.exe & schtasks /Run /TN StartRCTemp"
    sleep 5
    run_health_check
    ;;
  kiosk)
    echo "=== Deploying kiosk on server ${SERVER} ==="
    ssh "ADMIN@${SERVER}" "cd C:\\RacingPoint\\kiosk && git pull"
    ssh "ADMIN@${SERVER}" "schtasks /Run /TN StartKiosk 2>nul || pm2 restart kiosk"
    sleep 5
    run_health_check
    ;;
  web)
    echo "=== Deploying web dashboard on server ${SERVER} ==="
    ssh "ADMIN@${SERVER}" "cd C:\\RacingPoint\\web && git pull"
    ssh "ADMIN@${SERVER}" "schtasks /Run /TN StartWebDashboard 2>nul || pm2 restart web"
    sleep 5
    run_health_check
    ;;
  comms-link)
    echo "=== Deploying comms-link relay (local) ==="
    cd /c/Users/bono/racingpoint/comms-link
    git pull
    pm2 restart comms-link-bono || pm2 restart all
    sleep 3
    run_health_check
    ;;
  rc-sentry)
    echo "=== Deploying rc-sentry to server + all pods ==="
    # Use staged binary if available; rebuild only as fallback
    if [ -f "${SCRIPT_DIR}/rc-sentry.exe" ]; then
      echo "Using staged binary: ${SCRIPT_DIR}/rc-sentry.exe"
    else
      echo "WARNING: No staged binary in deploy-staging/. Rebuilding from source..."
      cd "$(dirname "${SCRIPT_DIR}")/racecontrol" 2>/dev/null || cd /c/Users/bono/racingpoint/racecontrol
      cargo build --release --bin rc-sentry
      cp target/release/rc-sentry.exe "${SCRIPT_DIR}/rc-sentry.exe"
    fi
    # Ensure server naming convention copy exists
    cp "${SCRIPT_DIR}/rc-sentry.exe" "${SCRIPT_DIR}/rc-server-sentry.exe"

    # Deploy to server (renamed to rc-server-sentry.exe)
    echo "--- Server deploy ---"
    scp "${SCRIPT_DIR}/rc-server-sentry.exe" "ADMIN@${SERVER}:C:/RacingPoint/rc-server-sentry-new.exe"
    ssh "ADMIN@${SERVER}" "cd /d C:\\RacingPoint && del rc-server-sentry-old.exe 2>nul & ren rc-server-sentry.exe rc-server-sentry-old.exe & ren rc-server-sentry-new.exe rc-server-sentry.exe & taskkill /F /IM rc-server-sentry.exe & schtasks /Run /TN StartServerSentry"

    # Deploy to pods via HTTP download + restart
    # Standing rule: start HTTP server, pods curl from James :9998
    echo "--- Pod fleet deploy (via HTTP :9998) ---"
    echo "Starting HTTP server for pod download..."
    cd "${SCRIPT_DIR}"
    python -m http.server 9998 &
    HTTP_PID=$!
    sleep 2

    # Pod IPs from network map
    # Compute SHA256 of staged binary for post-download verification
    SENTRY_SHA256=$(sha256sum "${SCRIPT_DIR}/rc-sentry.exe" | cut -d' ' -f1)
    echo "  Expected SHA256: ${SENTRY_SHA256}"

    POD_IPS="192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91"
    for IP in $POD_IPS; do
      echo "  Deploying to ${IP}..."
      curl -s -X POST "http://${IP}:8090/exec" \
        -H "Content-Type: application/json" \
        -d "{\"cmd\":\"curl.exe -s -o C:\\\\RacingPoint\\\\rc-sentry-new.exe http://192.168.31.27:9998/rc-sentry.exe && certutil -hashfile C:\\\\RacingPoint\\\\rc-sentry-new.exe SHA256 | findstr /V \\\"hash\\\" | findstr /V \\\"CertUtil\\\" > C:\\\\RacingPoint\\\\rc-sentry-sha.txt && findstr /C:\\\"${SENTRY_SHA256}\\\" C:\\\\RacingPoint\\\\rc-sentry-sha.txt >nul && del C:\\\\RacingPoint\\\\rc-sentry-old.exe 2>nul && ren C:\\\\RacingPoint\\\\rc-sentry.exe rc-sentry-old.exe && ren C:\\\\RacingPoint\\\\rc-sentry-new.exe rc-sentry.exe && taskkill /F /IM rc-sentry.exe && start \\\"\\\" C:\\\\RacingPoint\\\\rc-sentry.exe\",\"timeout_ms\":30000}" \
        2>/dev/null | grep -o '"exit_code":[0-9]*' || echo "  FAILED: ${IP} (download or SHA256 mismatch)"
    done

    # Cleanup HTTP server
    kill $HTTP_PID 2>/dev/null
    sleep 5
    run_health_check
    ;;
  cloud)
    echo "=== Deploying racecontrol to Cloud (Bono VPS) ==="
    CLOUD_SCRIPT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)/scripts/deploy-cloud.sh"
    bash "$CLOUD_SCRIPT"
    ;;
  *)
    echo "Unknown service: ${SERVICE}"
    echo "Services: racecontrol | rc-sentry | kiosk | web | comms-link | cloud"
    exit 1
    ;;
esac
