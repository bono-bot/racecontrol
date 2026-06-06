#!/usr/bin/env bash
# Idempotently install the ide-sync post-commit block into bono-lane repos.
#
# The block (scripts/git-hooks/post-commit-ide-sync.sh) is APPENDED to the live
# .git/hooks/post-commit (preserving graphify/other blocks) between
# `# ide-sync-start` / `# ide-sync-end` markers. Re-running strips the old block
# and re-appends — safe to run any number of times.
#
# Note: .git/hooks are local + untracked (permanence = this tracked installer).
# The harvester scans all three repos regardless; installing the trigger in
# racecontrol (where this work is committed) is sufficient — comms-link is a
# bonus trigger. Doctrine: IDE-OPERATING-MODEL.md §5.
set -uo pipefail

SNIPPET=/root/racecontrol/scripts/git-hooks/post-commit-ide-sync.sh
START='# ide-sync-start'
END='# ide-sync-end'
TARGETS=(/root/racecontrol/.git/hooks/post-commit /root/comms-link/.git/hooks/post-commit)

[ -f "$SNIPPET" ] || { echo "ERROR: snippet $SNIPPET missing"; exit 1; }

for hook in "${TARGETS[@]}"; do
    dir=$(dirname "$hook")
    if [ ! -d "$dir" ]; then echo "skip: $dir is not a hooks dir"; continue; fi
    if [ ! -f "$hook" ]; then printf '#!/bin/sh\n' > "$hook"; fi

    tmp=$(mktemp)
    # strip any existing ide-sync block (inclusive of markers), keep everything else
    awk -v s="$START" -v e="$END" '
        $0==s {skip=1}
        skip==0 {print}
        $0==e {skip=0; next}
    ' "$hook" > "$tmp"
    printf '\n' >> "$tmp"
    cat "$SNIPPET" >> "$tmp"
    cat "$tmp" > "$hook"
    rm -f "$tmp"
    chmod +x "$hook"

    n=$(grep -c "$START" "$hook" 2>/dev/null || echo 0)
    echo "installed -> $hook   (ide-sync-start count: $n; must be 1)"
done
echo "Done."
