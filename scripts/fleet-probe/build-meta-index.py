#!/usr/bin/env python3
# scripts/fleet-probe/build-meta-index.py -- Phase 448 Plan 07
# Reads per-target manifests in a directory, writes _meta.json (SCHEMA-03 shape).
# Usage: python3 scripts/fleet-probe/build-meta-index.py <manifest_dir> [--orchestrator-start-epoch N]
# Exit codes:
#   0 -- success, _meta.json written
#   1 -- parse errors on one or more manifest files (still writes _meta.json with what parsed)
#   2 -- manifest_dir missing or not provided
import sys
import json
import time
import datetime
from pathlib import Path

# TARGET_ORDER: locked per RESEARCH section 3 + CONTEXT.md sequential/parallel split.
# Sequential cluster first, then pods in numeric order, then remaining sequential.
TARGET_ORDER = [
    "server_23",
    "pod_1",
    "pod_2",
    "pod_3",
    "pod_4",
    "pod_5",
    "pod_6",
    "pod_7",
    "pod_8",
    "pos_130",
    "james_27",
    "bono_vps",
    "cloud_admin",
    "cloud_racecontrol",
    "relay_james",
]


def iso_ist_now():
    """Return ISO-8601 timestamp in IST (+05:30). Avoids TZ env (silently fails on Git Bash).
    Uses UTC epoch arithmetic (not TZ override) per CLAUDE.md timezone rule."""
    utc_epoch = time.time()
    ist_epoch = utc_epoch + 19800
    # Use timedelta arithmetic to avoid deprecated utcfromtimestamp() warning in Python 3.12+
    base = datetime.datetime(1970, 1, 1) + datetime.timedelta(seconds=ist_epoch)
    return base.strftime("%Y-%m-%dT%H:%M:%S+05:30")


def main():
    if len(sys.argv) < 2:
        print("Usage: build-meta-index.py <manifest_dir> [--orchestrator-start-epoch N]", file=sys.stderr)
        sys.exit(2)

    mdir = Path(sys.argv[1])
    if not mdir.is_dir():
        print("manifest dir not found: {}".format(mdir), file=sys.stderr)
        sys.exit(2)

    # Parse optional --orchestrator-start-epoch N
    start_epoch = None
    i = 2
    while i < len(sys.argv):
        if sys.argv[i] == "--orchestrator-start-epoch" and i + 1 < len(sys.argv):
            try:
                start_epoch = float(sys.argv[i + 1])
            except (ValueError, IndexError):
                pass
            i += 2
        else:
            i += 1

    # Read per-target manifest files (skip _meta.json itself)
    per_target = {}
    parse_errors = 0
    for f in sorted(mdir.glob("*.json")):
        if f.name == "_meta.json":
            continue
        try:
            with open(f, "r", encoding="utf-8") as fh:
                m = json.load(fh)
        except Exception as e:
            print("skip unreadable manifest {}: {}".format(f.name, e), file=sys.stderr)
            parse_errors += 1
            continue
        tid = m.get("target_id") or f.stem
        per_target[tid] = {
            "target_id": tid,
            "role": m.get("role", "unknown"),
            "probe_status": m.get("probe_status", "unknown"),
            "manifest_file": f.name,
        }

    # Build ordered targets list (TARGET_ORDER first, then any extras in alphabetical order)
    ordered = [per_target[t] for t in TARGET_ORDER if t in per_target]
    for tid in sorted(per_target.keys()):
        if tid not in TARGET_ORDER:
            ordered.append(per_target[tid])

    # Compute status_summary counts
    summary = {"ok": 0, "partial": 0, "probe_failed": 0}
    for entry in ordered:
        st = entry["probe_status"]
        if st in summary:
            summary[st] += 1

    now_ist = iso_ist_now()
    duration = round(time.time() - start_epoch, 2) if start_epoch is not None else 0.0

    meta = {
        "schema_version": "1.0",
        "probe_run_id": now_ist,
        "probed_at_ist": now_ist,
        "probe_duration_sec": duration,
        "orchestrator": "scripts/fleet-probe/probe-all.sh",
        "orchestrator_version": "phase-448-v1",
        "target_count": len(ordered),
        "targets": ordered,
        "status_summary": summary,
    }

    out = mdir / "_meta.json"
    with open(out, "w", encoding="utf-8") as fh:
        json.dump(meta, fh, indent=2)
    print("wrote {} target_count={} summary={}".format(out, len(ordered), summary))

    if parse_errors > 0:
        sys.exit(1)
    sys.exit(0)


if __name__ == "__main__":
    main()
