#!/usr/bin/env bash
# Manual deploy: refresh the live Console /initiatives board from IDE work.
#
# Cadence (Captain 2026-06-06): auto-harvest on commit (git hook) keeps the
# auto-evidence current; THIS is the manual deploy half that pushes it live.
# It is the ONLY sanctioned path to refresh the board — running the bare
# generator without the injector would drop the IDE evidence.
#
# Steps: catch-up harvest (+PR) -> regenerate registry -> inject IDE evidence
#        -> regenerate launch-snapshot (home cards) -> refresh the service
#        -> verify health.
# Doctrine: .planning/specs/racecontrol-layer/IDE-OPERATING-MODEL.md §3/§5
set -uo pipefail

RC=/root/racecontrol
CONSOLE_APP=/root/rp-v2-apps/apps/racecontrol-console
HEALTH=http://127.0.0.1:3220/api/health

echo "== [1/6] harvest Development: trailers (with PR lookup) =="
python3 "$RC/scripts/sync-ide-initiatives.py" --with-pr || echo "WARN: harvest returned non-zero (continuing)"

echo
echo "== [2/6] regenerate dev-registry.json from curated YAML =="
( cd "$CONSOLE_APP" && python3 scripts/gen-dev-registry.py ) || { echo "ERROR: generator failed — aborting (board unchanged)"; exit 1; }

echo
echo "== [3/6] inject harvested IDE evidence into the generated registry =="
python3 "$RC/scripts/inject-auto-evidence.py" || { echo "ERROR: injector failed — aborting"; exit 1; }

echo
echo "== [4/6] regenerate launch-snapshot.json (home dashboard cards) =="
( cd "$CONSOLE_APP" && python3 scripts/gen-launch-snapshot.py ) || echo "WARN: launch-snapshot generator failed (home cards unchanged; board still refreshed)"

echo
echo "== [5/6] refresh the live console (pm2 restart — JSON is read at runtime) =="
if command -v pm2 >/dev/null 2>&1; then
    pm2 restart racecontrol-console --update-env >/dev/null 2>&1 && echo "pm2 restart racecontrol-console: ok" \
        || echo "WARN: pm2 restart failed — check 'pm2 list'"
else
    echo "WARN: pm2 not found — refresh the console manually"
fi

echo
echo "== [6/6] verify health (single probe; behavioural check is loading /initiatives) =="
sleep 3
curl -s --max-time 8 "$HEALTH" || echo "(no health response yet — re-probe in a few seconds)"
echo
echo "Done. Behavioural verify: open https://console.racecontrol.in/initiatives/<id> and confirm the [IDE ...] evidence rows."
