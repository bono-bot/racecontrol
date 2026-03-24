#!/usr/bin/env bash
# deploy-all-pods.sh — Deploy rc-agent to all online pods via pod-agent :8090
# Usage: bash deploy-all-pods.sh
# Run from James's machine with HTTP server already running on :9998

set -e

JAMES_IP="192.168.31.27"
HTTP_PORT="9998"
BINARY_URL="http://$JAMES_IP:$HTTP_PORT/rc-agent.exe"
DEST="C:\\RacingPoint\\rc-agent.exe"
DEST_NEW="C:\\RacingPoint\\rc-agent-new.exe"
DEST_OLD="C:\\RacingPoint\\rc-agent-old.exe"
START_BAT="C:\\RacingPoint\\start-rcagent.bat"

declare -A PODS=(
  ["pod_1"]="192.168.31.89"
  ["pod_2"]="192.168.31.33"
  ["pod_3"]="192.168.31.28"
  ["pod_4"]="192.168.31.88"
  ["pod_5"]="192.168.31.86"
  ["pod_6"]="192.168.31.87"
  ["pod_7"]="192.168.31.38"
  ["pod_8"]="192.168.31.91"
)

# PowerShell command: download → rename old → move new → kill old PID → start
# Using -EncodedCommand to avoid escaping issues
make_ps_cmd() {
  local url="$1"
  cat <<EOF
Invoke-WebRequest '$url' -OutFile '$DEST_NEW';
if (Test-Path '$DEST') { Rename-Item '$DEST' '$DEST_OLD' -Force };
Move-Item '$DEST_NEW' '$DEST' -Force;
Get-Process rc-agent -ErrorAction SilentlyContinue | Stop-Process -Force;
Start-Sleep 2;
Remove-Item '$DEST_OLD' -Force -ErrorAction SilentlyContinue;
Start-Process '$START_BAT'
EOF
}

deploy_pod() {
  local name="$1"
  local ip="$2"
  echo ""
  echo "=== Deploying to $name ($ip) ==="

  # Check pod-agent is reachable
  if ! curl -s --max-time 3 "http://$ip:8090/health" > /dev/null 2>&1; then
    echo "  SKIP: $name ($ip) — pod-agent not reachable"
    return
  fi

  # Build base64-encoded PS command
  local ps_raw
  ps_raw=$(make_ps_cmd "$BINARY_URL")
  # UTF-16LE base64 for PowerShell -EncodedCommand
  local ps_enc
  ps_enc=$(printf '%s' "$ps_raw" | iconv -f UTF-8 -t UTF-16LE | base64 -w 0)

  # Write deploy JSON
  local tmpjson="/tmp/deploy-$name.json"
  printf '{"cmd":"powershell -EncodedCommand %s","timeout":120}' "$ps_enc" > "$tmpjson"

  # Send deploy command
  local resp
  resp=$(curl -s --max-time 130 -X POST "http://$ip:8090/exec" \
    -H "Content-Type: application/json" \
    -d @"$tmpjson" 2>&1)

  echo "  Response: $resp"

  # Verify: check new process started
  sleep 3
  local verify
  verify=$(curl -s --max-time 5 -X POST "http://$ip:8090/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"powershell -Command \"(Get-Process rc-agent -ErrorAction SilentlyContinue).Id\"","timeout":10}' 2>&1)
  echo "  Verify (rc-agent PID): $verify"
}

echo "=== RC-Agent Fleet Deploy ==="
echo "Binary: $BINARY_URL"
echo "Pods: ${!PODS[@]}"
echo ""

for name in pod_1 pod_2 pod_3 pod_4 pod_5 pod_6 pod_7 pod_8; do
  ip="${PODS[$name]}"
  deploy_pod "$name" "$ip"
done

echo ""
echo "=== Deploy complete ==="
