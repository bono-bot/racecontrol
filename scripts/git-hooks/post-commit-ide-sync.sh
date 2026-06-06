# ide-sync-start
# Auto-harvest `Development:` trailers -> Console initiative evidence.
# Fail-open + fully detached: never blocks or slows a commit. Idempotently
# (re)installed by scripts/install-ide-sync-hooks.sh into .git/hooks/post-commit.
# Source of truth: scripts/git-hooks/post-commit-ide-sync.sh
# Doctrine: .planning/specs/racecontrol-layer/IDE-OPERATING-MODEL.md §5
if command -v python3 >/dev/null 2>&1 && [ -f /root/racecontrol/scripts/sync-ide-initiatives.py ]; then
    ( python3 /root/racecontrol/scripts/sync-ide-initiatives.py >/tmp/ide-sync-last.log 2>&1 </dev/null & ) >/dev/null 2>&1
fi
# ide-sync-end
