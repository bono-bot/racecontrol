#!/usr/bin/env bash
# session-end-console-sync.sh -- SessionEnd Console data auto-sync (replaces the nightly cron).
#
# On session end, regenerate the two Console snapshots (OFFLINE) and commit + push
# ONLY those two generated JSON files on the console branch. Mirrors the accepted
# partner-memory-push.js session-end auto-push pattern.
#
# Safe by construction:
#   - scoped: commits ONLY the two data/*.json (uses `git commit -- <paths>`, never -A;
#     immune to a dirty index / pre-staged changes).
#   - branch-pinned: never commits onto an unrelated branch, never pushes main.
#   - no-force, only-if-changed, fail-open: never blocks session termination.
#
# Manual run : bash /root/racecontrol/scripts/session-end-console-sync.sh
# Kill-switch: CONSOLE_AUTOSYNC=0    Branch override: CONSOLE_SYNC_BRANCH=<branch>
# Exit code  : 0 ALWAYS.
# ASCII-only (per feedback_ascii_only_script_constraint).
set -uo pipefail

log() { echo "[console-sync] $*"; }

# --- 1. guards / kill-switch ---
[ "${CONSOLE_AUTOSYNC:-1}" = "0" ] && { log "disabled (CONSOLE_AUTOSYNC=0)"; exit 0; }
[ "$(uname -s 2>/dev/null)" = "Linux" ] || { log "skip: non-Linux host"; exit 0; }

RC=/root/racecontrol
REPO=/root/rp-v2-apps
APP="$REPO/apps/racecontrol-console"
BRANCH="${CONSOLE_SYNC_BRANCH:-feat/console-frontend-renders}"
P1=apps/racecontrol-console/data/launch-snapshot.json
P2=apps/racecontrol-console/data/dev-registry.json

[ -d "$APP" ] || { log "skip: app dir missing ($APP)"; exit 0; }

# --- 2. offline regen (best-effort each; NO network harvest, NO rebuild, NO restart) ---
# Order matters: gen-dev-registry then inject-auto-evidence (else the [IDE] evidence
# is dropped), then gen-launch-snapshot. The live page reads the JSON per request.
( cd "$APP" && python3 scripts/gen-dev-registry.py )    >/dev/null 2>&1 || log "warn: gen-dev-registry failed"
python3 "$RC/scripts/inject-auto-evidence.py"           >/dev/null 2>&1 || log "warn: inject-auto-evidence failed"
( cd "$APP" && python3 scripts/gen-launch-snapshot.py ) >/dev/null 2>&1 || log "warn: gen-launch-snapshot failed"

# --- 3. branch-pin + clean-state guard ---
cur="$(git -C "$REPO" branch --show-current 2>/dev/null)"
if [ "$cur" != "$BRANCH" ]; then log "skip: on '$cur' (not console branch '$BRANCH')"; exit 0; fi
if [ -d "$REPO/.git/rebase-merge" ] || [ -d "$REPO/.git/rebase-apply" ] || [ -f "$REPO/.git/MERGE_HEAD" ]; then
  log "skip: repo mid-rebase/merge"; exit 0
fi

# --- 4. only-if-changed (compare the two files against HEAD) ---
if git -C "$REPO" diff --quiet HEAD -- "$P1" "$P2" 2>/dev/null; then
  log "no data change"; exit 0
fi

# --- 5. commit ONLY the two paths (partial commit; ignores anything else in the index) ---
ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
if ! git -C "$REPO" commit -q -m "chore(console): session-end data refresh $ts" -- "$P1" "$P2" 2>/dev/null; then
  log "warn: commit failed"; exit 0
fi
sha="$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null)"

# defensive: the commit must contain ONLY the two data files
extra="$(git -C "$REPO" show --name-only --format= HEAD 2>/dev/null | grep -vE "^(apps/racecontrol-console/data/(launch-snapshot|dev-registry)\.json)?$" || true)"
if [ -n "$extra" ]; then
  log "ABORT: commit $sha touched unexpected paths -> $(echo "$extra" | tr '\n' ' '); NOT pushing"
  exit 0
fi

# --- 6. push (best-effort, no force) ---
if git -C "$REPO" push origin "$BRANCH" >/dev/null 2>&1; then
  log "pushed $sha -> origin/$BRANCH"
else
  log "push failed (committed $sha) -- resolve manually: git -C $REPO push origin $BRANCH"
fi

exit 0
