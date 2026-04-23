#!/usr/bin/env bash
# process-ownership.sh — static audit for multi-owner process MANAGEMENT.
#
# Failure mode this catches:
#   Multiple independent code paths in rc-agent SPAWN or KILL the same
#   external process (steam.exe, ConspitLink, etc.) producing
#   "process opens over and over" chaos visible to customers.
#
# Detection is narrow on purpose: we look for CALL SITES that actively
# manage a process, not static lists (NEVER_KILL, known-launcher arrays).
#
# Managed patterns detected:
#   - spawn_safe("NAME.exe")        — rc_common spawn helper
#   - spawn_safe( NAME_VAR )        — skipped (can't static-resolve)
#   - Command::new("NAME.exe")      — std process
#   - "/IM", "NAME.exe"             — taskkill argv literal
#   - "/IM NAME.exe"                — taskkill inline (rare)
#
# Usage:
#   bash scripts/audit/process-ownership.sh          # report + exit status
#   bash scripts/audit/process-ownership.sh --json   # machine-readable
#
# Exit codes:
#   0 — no multi-owner violation
#   1 — violation found; see stdout
#   2 — invocation error

set -euo pipefail

while [ ! -f Cargo.toml ] || ! grep -q '^\[workspace\]' Cargo.toml 2>/dev/null; do
    if [ "$PWD" = "/" ]; then
        echo "[process-ownership] ERROR: not inside a cargo workspace" >&2
        exit 2
    fi
    cd ..
done

JSON_MODE=0
STRICT=0
for arg in "$@"; do
    case "$arg" in
        --json) JSON_MODE=1 ;;
        --strict) STRICT=1 ;;
    esac
done

# Allowlist — document each entry with WHY multi-owner is legitimate.
#
# rc-agent.exe, rc-sentry.exe — mutual recovery between sentry + watchdog
#                               + self_monitor. Intentional coordination.
# WerFault.exe, WerFaultSecure.exe — dialog-dismissal from multiple crash
#                                    recovery entry points. Safe.
# acs.exe — AC game; start/stop logic lives in ac_launcher. Minor cross-
#           module touch is tolerable.
# cmd.exe — shell infrastructure. rc-common/exec.rs is the generic runner,
#           rc-sentry/session1_spawn.rs uses it for Session 1 spawning.
#           Not a customer-facing app.
# Content Manager.exe — inline "Content Manager.exe" with space; rg regex
#                       doesn't capture the space, won't match anyway.
ALLOWED_MULTI_OWNER="
rc-agent.exe
rc-sentry.exe
WerFault.exe
WerFaultSecure.exe
acs.exe
cmd.exe
"

# KNOWN VIOLATIONS as of 2026-04-22 — do NOT silently whitelist.
# Instead, surface them so someone fixes them.
#
# msedge.exe: ai_debugger.rs + event_loop.rs
# acmanager.exe: ai_debugger.rs + game_doctor.rs
#
# Both are structural duplicates of the Steam-management overlap bug.
# Follow-up PR should consolidate each to a single owner.

# Extract (process_name, file) pairs from active management call sites.
# grep -PHo with Perl regex for the three canonical patterns.
# Each output line: "<file>:<line>:<match>" — we parse name from the literal.

RAW=$(grep -RIn --include='*.rs' -E \
    'spawn_safe\("[A-Za-z0-9._-]+\.exe"\)|Command::new\("[A-Za-z0-9._-]+\.exe"\)|"/IM",\s*"[A-Za-z0-9._-]+\.exe"|"/IM\s+[A-Za-z0-9._-]+\.exe"' \
    crates/ 2>/dev/null || true)

if [ -z "$RAW" ]; then
    if [ "$JSON_MODE" -eq 1 ]; then
        echo '{"violations":[],"total_processes_referenced":0}'
    else
        echo "[process-ownership] no active spawn/kill call sites found under crates/."
    fi
    exit 0
fi

# Parse (name, file). Skip test files + comment lines.
PAIRS=$(echo "$RAW" | awk -F: '
    {
        # Reassemble: file=$1, line=$2, rest=$3...
        file=$1
        lineno=$2
        content=""
        for (i=3; i<=NF; i++) content=content ":" $i
        sub(/^:/, "", content)
        # Skip tests + comments.
        if (file ~ /\/tests\//) next
        if (file ~ /_tests\.rs$/) next
        if (file ~ /tests\.rs$/) next
        trimmed=content
        sub(/^[[:space:]]+/, "", trimmed)
        if (trimmed ~ /^\/\//) next
        if (trimmed ~ /^\*/) next
        # Extract every "NAME.exe" literal on this line.
        while (match(content, /"([A-Za-z0-9._-]+\.exe)"/, arr)) {
            print arr[1] "\t" file
            content = substr(content, RSTART + RLENGTH)
        }
    }
' | sort -u)

if [ -z "$PAIRS" ]; then
    if [ "$JSON_MODE" -eq 1 ]; then
        echo '{"violations":[],"total_processes_referenced":0}'
    else
        echo "[process-ownership] no active management call sites after filtering."
    fi
    exit 0
fi

COUNTS=$(echo "$PAIRS" | awk -F'\t' '{print $1}' | sort | uniq -c | sort -rn)
TOTAL_NAMES=$(echo "$COUNTS" | wc -l | tr -d ' ')

VIOLATIONS=$(echo "$COUNTS" | awk -v allow="$ALLOWED_MULTI_OWNER" '
    BEGIN {
        n=split(allow,a,"\n")
        for (i=1;i<=n;i++) if (a[i]!="") allowmap[tolower(a[i])]=1
    }
    $1 > 1 && !(tolower($2) in allowmap) { print $2 }
')

if [ "$JSON_MODE" -eq 1 ]; then
    printf '{"violations":['
    first=1
    if [ -n "$VIOLATIONS" ]; then
        while IFS= read -r v; do
            [ -z "$v" ] && continue
            if [ $first -eq 0 ]; then printf ','; fi
            first=0
            owners=$(echo "$PAIRS" | awk -F'\t' -v p="$v" '$1==p {print $2}' | awk 'BEGIN{ORS=","}{printf "\"" $0 "\""}' | sed 's/,$//')
            printf '{"process":"%s","owners":[%s]}' "$v" "$owners"
        done <<< "$VIOLATIONS"
    fi
    printf '],"total_processes_referenced":%d}\n' "$TOTAL_NAMES"
else
    echo "[process-ownership] audit: ${TOTAL_NAMES} distinct .exe names managed under crates/"
    if [ -z "$VIOLATIONS" ]; then
        echo "[process-ownership] OK — no multi-owner spawn/kill violations"
        exit 0
    fi
    vio_count=$(echo "$VIOLATIONS" | wc -l | tr -d ' ')
    echo "[process-ownership] VIOLATIONS (${vio_count}):"
    echo
    while IFS= read -r v; do
        [ -z "$v" ] && continue
        echo "  $v managed by:"
        echo "$PAIRS" | awk -F'\t' -v p="$v" '$1==p {print "    " $2}'
        echo
    done <<< "$VIOLATIONS"
    echo "If legitimate co-ownership, add to ALLOWED_MULTI_OWNER with justification."
    if [ $STRICT -eq 0 ]; then
        echo
        echo "(Report-only mode — not failing. Use --strict in CI to enforce.)"
    fi
fi

if [ -n "$VIOLATIONS" ] && [ $STRICT -eq 1 ]; then
    exit 1
fi
exit 0
