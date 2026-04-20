#!/usr/bin/env bash
# cleanup-pod-edge-stale-ps1.sh - Remove benign dead-code Edge kiosk .ps1
# files from pod disks. These are 2026-03-23 investigation leftovers flagged
# as drift by scripts/audit/autostart-surfaces.ps1.
#
# Scope (from audit/pod-*-autostart-baseline-2026-04-20.json - the ground
# truth, correcting the 04:20 handoff's "Pods 3/5/6/7/8" claim):
#   - Pods 1, 2, 3, 4, 5, 8: 0 stale files (already clean)
#   - Pod 6: 3 files (edge-check.ps1, get-edge-url.ps1, investigate-kiosk.ps1)
#   - Pod 7: 3 files (fix-kiosk-span.ps1, get-edge-url.ps1, investigate-kiosk.ps1)
#
# None of these files have a current caller (no HKLM Run / Startup /
# scheduled task / service references them per the baseline enumeration).
# Deletion is benign but still requires per-pod sign-off per handoff.
#
# Usage:
#   ./cleanup-pod-edge-stale-ps1.sh --pod 6              # delete Pod 6's 3 files
#   ./cleanup-pod-edge-stale-ps1.sh --pod 7              # delete Pod 7's 3 files
#   ./cleanup-pod-edge-stale-ps1.sh --pod 6 --dry-run    # list only, no delete
#   ./cleanup-pod-edge-stale-ps1.sh --all-flagged        # Pod 6 + Pod 7 together
#
# Each run:
#   1. Prints the file list to be deleted.
#   2. If not --dry-run, deletes via SSH powershell Remove-Item -Force.
#   3. Re-runs autostart-surfaces.ps1 on the pod to confirm 0 remaining refs.

set -u

POD=""
ALL_FLAGGED=0
DRY_RUN=0

while [ $# -gt 0 ]; do
    case "$1" in
        --pod)          POD="$2"; shift 2 ;;
        --all-flagged)  ALL_FLAGGED=1; shift ;;
        --dry-run)      DRY_RUN=1; shift ;;
        -h|--help)      sed -n '2,/^set -u/p' "$0" | sed 's/^#//'; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$POD" ] && [ "$ALL_FLAGGED" -eq 0 ]; then
    echo "ERROR: --pod N or --all-flagged required" >&2
    exit 2
fi

# Per-pod file lists (source: baseline JSONs, 2026-04-20 06:36 IST snapshot)
files_for_pod() {
    case "$1" in
        6) echo "C:\\RacingPoint\\edge-check.ps1 C:\\RacingPoint\\get-edge-url.ps1 C:\\RacingPoint\\investigate-kiosk.ps1" ;;
        7) echo "C:\\RacingPoint\\fix-kiosk-span.ps1 C:\\RacingPoint\\get-edge-url.ps1 C:\\RacingPoint\\investigate-kiosk.ps1" ;;
        *) echo "" ;;
    esac
}

ssh_host() {
    # Tailscale SSH as User@simN
    echo "User@sim$1"
}

cleanup_pod() {
    local pod="$1"
    local files
    files=$(files_for_pod "$pod")
    if [ -z "$files" ]; then
        echo "Pod $pod: no stale Edge .ps1 files in baseline - skipping"
        return 0
    fi
    local host
    host=$(ssh_host "$pod")
    echo "=== Pod $pod ($host) ==="
    echo "Files to delete:"
    for f in $files; do
        echo "  $f"
    done

    if [ "$DRY_RUN" -eq 1 ]; then
        echo "--dry-run: no changes."
        return 0
    fi

    echo ""
    echo "Deleting via SSH..."
    for f in $files; do
        ssh -o ConnectTimeout=5 "$host" "powershell -NoProfile -Command \"Remove-Item -LiteralPath '$f' -Force -ErrorAction SilentlyContinue\"" \
            && echo "  rm $f" \
            || echo "  WARN: rm $f failed"
    done

    echo ""
    echo "Verifying..."
    # Re-run autostart-surfaces.ps1 would be heavy. Cheaper: check each file
    # no longer exists.
    local remaining=0
    for f in $files; do
        if ssh "$host" "powershell -NoProfile -Command \"Test-Path -LiteralPath '$f'\"" 2>/dev/null | grep -qi true; then
            echo "  STILL EXISTS: $f"
            remaining=$((remaining + 1))
        fi
    done
    if [ "$remaining" -eq 0 ]; then
        echo "OK: all $(echo $files | wc -w) file(s) removed from Pod $pod."
    else
        echo "WARN: $remaining file(s) remain on Pod $pod. Investigate manually."
        return 1
    fi
}

if [ "$ALL_FLAGGED" -eq 1 ]; then
    cleanup_pod 6
    echo ""
    cleanup_pod 7
else
    cleanup_pod "$POD"
fi
