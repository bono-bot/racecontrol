#!/bin/bash
# conspitlink-cull.sh — Enforce singleton ConspitLink2.0.exe across the fleet.
#
# The standing rule (CLAUDE.md Regression Prevention section) mandates one
# ConspitLink instance per pod. start-rcagent.bat kills-all + starts-one at
# boot, but the process respawns mid-session (at least on Pod 3 during AC
# launches) and accumulates. 2026-04-22 incident: Pod 3 had 8 instances.
#
# This culler runs every 5 min on James and enforces singleton state via
# rc-sentry exec. If count > 1 on any pod, kill all and start one.
#
# Schedule on James every 5 min:
#   schtasks /Create /TN "ConspitLink-Cull" /TR "bat" /SC MINUTE /MO 5 /F
#
# Exit code = pods that were culled in this run (0 = all already singleton).

set -u

SENTRY_KEY="${SENTRY_KEY:-478a3688339737fb5945f9b89d8bb533f2569fe0b1fea46b504656eee455b9ab}"
STAGING_DIR="${STAGING_DIR:-C:/Users/bono/racingpoint/deploy-staging}"
STAGING_URL="${STAGING_URL:-http://192.168.31.27:18889}"
STATE_LOG="${STATE_LOG:-C:/Users/bono/racingpoint/racecontrol/logs/conspitlink-cull.jsonl}"
NOW_UTC="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"

mkdir -p "$(dirname "$STATE_LOG")" 2>/dev/null

# Inline the pod-side cull script in staging so it's always fresh / survives re-clone.
cat > "$STAGING_DIR/conspitlink-cull.ps1" <<'PS1_END'
$exe = 'C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe'
$before = Get-Process -Name 'ConspitLink2.0' -EA SilentlyContinue
$beforeCount = if ($before) { $before.Count } else { 0 }
if ($beforeCount -le 1) {
    Write-Host "before=$beforeCount action=none"
    exit 0
}
& taskkill.exe /F /IM ConspitLink2.0.exe 1>$null 2>$null
Start-Sleep -Seconds 2
if (Test-Path $exe) {
    Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe) -WindowStyle Hidden
    Start-Sleep -Seconds 2
}
$after = Get-Process -Name 'ConspitLink2.0' -EA SilentlyContinue
$afterCount = if ($after) { $after.Count } else { 0 }
Write-Host "before=$beforeCount action=cull after=$afterCount"
PS1_END

declare -A POD_IPS=(
  [pod_1]=192.168.31.89
  [pod_2]=192.168.31.33
  [pod_3]=192.168.31.28
  [pod_4]=192.168.31.88
  [pod_5]=192.168.31.86
  [pod_6]=192.168.31.87
  [pod_7]=192.168.31.38
  [pod_8]=192.168.31.91
)

CULLED=0

for pod in pod_1 pod_2 pod_3 pod_4 pod_5 pod_6 pod_7 pod_8; do
  ip="${POD_IPS[$pod]}"

  # Ensure script is on pod
  curl -s --max-time 5 -X POST "http://$ip:8091/exec" \
    -H "X-Service-Key: $SENTRY_KEY" \
    -H "Content-Type: application/json" \
    -d "{\"cmd\":\"curl.exe -s -o C:\\\\RacingPoint\\\\conspitlink-cull.ps1 $STAGING_URL/conspitlink-cull.ps1\"}" \
    > /dev/null 2>&1

  # Run cull
  res="$(curl -s --max-time 15 -X POST "http://$ip:8091/exec" \
    -H "X-Service-Key: $SENTRY_KEY" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"powershell -ExecutionPolicy Bypass -NoProfile -File C:\\RacingPoint\\conspitlink-cull.ps1"}' 2>/dev/null)"

  stdout="$(echo "$res" | python3 -c "import sys,json; print(json.load(sys.stdin).get('stdout','').strip())" 2>/dev/null)"

  if echo "$stdout" | grep -q 'action=cull'; then
    CULLED=$((CULLED + 1))
    echo "[$NOW_UTC] $pod: $stdout"
  fi

  # Always log state
  echo "{\"ts\":\"$NOW_UTC\",\"pod\":\"$pod\",\"result\":\"$stdout\"}" >> "$STATE_LOG"
done

if [ "$CULLED" -gt 0 ]; then
  echo "[$NOW_UTC] conspitlink-cull: culled $CULLED pod(s)"
fi
exit "$CULLED"
