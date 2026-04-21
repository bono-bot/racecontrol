#!/usr/bin/env bash
# scripts/graphify-refresh-bono-mirror.sh — P1.5/P6.2 helper.
#
# Pulls Bono-side graphify graph.json files into ../bono-mirror/ on James.
# Run AFTER `graphify_bono_all` completes on Bono (via relay) so the mirror
# contains fresh extraction output. The 8 Bono-unique repos have no James-side
# counterpart; their graphs are the only way Tier 2 unify.mjs can see them.
#
# Usage:
#   bash scripts/graphify-refresh-bono-mirror.sh            # pull all 8
#   bash scripts/graphify-refresh-bono-mirror.sh <repo>     # pull one
#
# Environment:
#   BONO_HOST=root@100.70.177.44   # SSH target (Tailscale)
#
# Exit codes: 0 on success, non-zero if any scp fails.

set -euo pipefail

BONO_HOST="${BONO_HOST:-root@100.70.177.44}"
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || cd "$(dirname "$0")/.." && pwd)"
MIRROR="$REPO_ROOT/../bono-mirror"

# (remote-name, local-alias) — local-alias distinguishes James-side vs Bono-side
# when the repo name is shared (whatsapp-bot, discord-bot, admin, mcp-*).
REPOS=(
  "racingpoint-whatsapp-bot racingpoint-whatsapp-bot-bono"
  "racingpoint-discord-bot  racingpoint-discord-bot-bono"
  "racingpoint-hiring-bot   racingpoint-hiring-bot"
  "racingpoint-cloud-dashboard racingpoint-cloud-dashboard"
  "racingpoint-google       racingpoint-google"
  "racingpoint-dashboard    racingpoint-dashboard"
  "racingpoint-api-gateway  racingpoint-api-gateway"
  "meta-corpus              meta-corpus"
)

FILTER="${1:-}"
FAILED=0

for pair in "${REPOS[@]}"; do
  remote="${pair%% *}"
  local="${pair##* }"
  if [ -n "$FILTER" ] && [ "$FILTER" != "$remote" ] && [ "$FILTER" != "$local" ]; then
    continue
  fi
  dest="$MIRROR/$local/graphify-out"
  mkdir -p "$dest"
  if scp -o ConnectTimeout=6 -o StrictHostKeyChecking=no \
        "$BONO_HOST:/root/$remote/graphify-out/graph.json" \
        "$dest/graph.json" >/dev/null 2>&1; then
    size=$(stat -c%s "$dest/graph.json" 2>/dev/null || wc -c < "$dest/graph.json")
    printf 'OK   %-40s %10d bytes\n' "$local" "$size"
  else
    printf 'FAIL %-40s (scp from /root/%s/)\n' "$local" "$remote"
    FAILED=$((FAILED + 1))
  fi
done

if [ "$FAILED" -gt 0 ]; then
  echo ""
  echo "$FAILED repo(s) failed to refresh — unify.mjs will skip them silently."
  exit 1
fi
echo ""
echo "Bono mirror refreshed. Now run: node scripts/graphify-post/unify.mjs"
