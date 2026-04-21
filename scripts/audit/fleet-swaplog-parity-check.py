#!/usr/bin/env python3
"""
Fleet SWAPLOG-parity checker — Day 3 board-state surface.

Detects the `deploy-server.sh` SWAPLOG-append-broken class: racecontrol is
redeployed to server .23 but no row is appended to SWAPLOG.md, so the
between-session source-of-truth for "what is running where" silently
drifts from reality. By 2026-04-20 the append path has missed 10
consecutive deploys (per LOGBOOK).

Detection strategy:
  1. Parse SWAPLOG.md for the most recent *racecontrol* row. SWAPLOG is
     mixed — rc-agent (~26MB), racecontrol (~55-65MB), kiosk-zip (~11MB),
     web-zip (~17MB) — we classify by size_bytes + reason text.
  2. Try to fetch `/api/v1/health` from server .23 (LAN) or localhost
     (server-side run). If unreachable, output SKIPPED with reason —
     do NOT fail, because cloud/CI environments cannot reach venue LAN.
  3. Compare live `build_id` (short git hash) against SWAPLOG row's
     commit column (strip `-dirty`). Mismatch = red.

Exit codes:
  0 — green (SWAPLOG last racecontrol row matches live build_id,
      OR endpoint unreachable and classified as skipped).
  1 — red (endpoint returned a build_id that does not match SWAPLOG tail).
  2 — parse error (SWAPLOG unreadable or no racecontrol rows found).

Designed for:
  - run-all-checkers.sh (orchestrator) registration
  - Pre-commit hook on deploy-server.sh changes
  - Bono pm2 workflow-monitor daemon every 5 min

Dependencies: python3 stdlib only. `urllib.request` used for the probe so
no `requests` install is needed.
"""

from __future__ import annotations

import json
import os
import re
import socket
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Optional


REPO_ROOT = Path(__file__).resolve().parents[2]
SWAPLOG_PATH = REPO_ROOT / "SWAPLOG.md"

# size_bytes ranges used to classify SWAPLOG rows. Conservative ranges
# derived from 2026-04 deploy history; update if binary footprint grows.
RACECONTROL_SIZE_MIN = 50_000_000
RACECONTROL_SIZE_MAX = 80_000_000

# Health probe: try LAN IP first, then localhost.
HEALTH_URLS = [
    "http://192.168.31.23:8080/api/v1/health",
    "http://localhost:8080/api/v1/health",
]
HEALTH_TIMEOUT_SEC = 3.0

# SWAPLOG row format (pipe-delimited markdown table):
#   | timestamp_ist | commit_hash | size_bytes | sha256_short | triggered_by | reason |
ROW_RE = re.compile(
    r"^\s*\|\s*(?P<ts>[^|]+?)\s*\|\s*(?P<commit>[^|]+?)\s*\|\s*(?P<size>[^|]+?)\s*\|\s*(?P<sha>[^|]*?)\s*\|\s*(?P<by>[^|]*?)\s*\|\s*(?P<reason>.+?)\s*\|\s*$"
)


def parse_size(cell: str) -> Optional[int]:
    """SWAPLOG size cell may be '60727808', '26727424 (rc-agent) / 60727808 (racecontrol)',
    '11589879 (zip)', or '(not recorded)'. Extract the first int or None."""
    if "(not recorded)" in cell:
        return None
    # Take the largest number that appears in the cell, since mixed-binary rows
    # put the rc-agent size FIRST and the racecontrol size SECOND — we want
    # the racecontrol binary size for our racecontrol-row classification.
    nums = [int(n) for n in re.findall(r"\d{5,}", cell)]
    if not nums:
        return None
    return max(nums)


def is_racecontrol_row(row: dict) -> bool:
    size_cell = row["size"]
    size = parse_size(size_cell)
    reason = row["reason"].lower()
    commit = row["commit"]

    # Exclude rows whose commit cell is a non-racecontrol binary flag
    # e.g. "a13942f2-dirty (rc-agent, Pod 6 catchup)" — commit cell is
    # annotated with "(rc-agent" clearly.
    if "rc-agent" in commit.lower():
        return False
    if "kiosk" in commit.lower():
        return False
    if "web :3200" in commit.lower() or "web only" in commit.lower():
        return False

    # Exclude frontend-only deploys that mention "racecontrol" only as the
    # git-repo context (not a binary swap). Pattern: size cell marked
    # "N/A (frontend rebuild, ...)" OR commit cell annotated with
    # "PR #N merge" OR reason contains "frontend rebuild, no rust binary".
    # These rows document a cloud admin/kiosk/web rebuild against a racecontrol
    # merge commit — server .23 racecontrol binary is UNCHANGED.
    size_lower = size_cell.lower()
    commit_lower = commit.lower()
    if "frontend rebuild" in size_lower or "frontend rebuild" in reason:
        return False
    if size_lower.startswith("n/a") and (
        "frontend" in size_lower or "no rust binary" in size_lower
    ):
        return False
    if "pr #" in commit_lower and "merge" in commit_lower:
        # Merge commits into main that didn't change the .23 binary — the
        # SWAPLOG row is ship-documentation, not a server swap record.
        return False

    # Size-based positive: racecontrol binaries are 50-80 MB.
    if size is not None and RACECONTROL_SIZE_MIN <= size <= RACECONTROL_SIZE_MAX:
        return True

    # Reason-based positive: explicit racecontrol markers.
    if "racecontrol" in reason and "rc-agent only" not in reason:
        return True

    return False


def extract_commit(commit_cell: str) -> str:
    """Strip annotations from the commit cell. Keep `-dirty` so dirty-state
    drift is visible to the reader; strip for the equality check only."""
    # First token before whitespace or parenthesis.
    m = re.match(r"([0-9a-f]{7,40}(?:-dirty)?)", commit_cell.strip())
    return m.group(1) if m else commit_cell.strip()


def parse_swaplog() -> list[dict]:
    text = SWAPLOG_PATH.read_text(encoding="utf-8", errors="replace")
    rows = []
    in_entries = False
    for line in text.splitlines():
        if line.strip().startswith("## Entries"):
            in_entries = True
            continue
        if not in_entries:
            continue
        m = ROW_RE.match(line)
        if not m:
            continue
        # Skip header rows (timestamp_ist literal + dashes separator).
        ts_cell = m.group("ts").strip()
        if ts_cell in ("timestamp_ist", "---"):
            continue
        rows.append(m.groupdict())
    return rows


def probe_health() -> Optional[dict]:
    """Return {'build_id': '...', 'url': '...'} on success, None on fail."""
    for url in HEALTH_URLS:
        try:
            with urllib.request.urlopen(url, timeout=HEALTH_TIMEOUT_SEC) as r:
                raw = r.read().decode("utf-8", errors="replace")
            doc = json.loads(raw)
            bid = doc.get("build_id")
            if bid:
                return {"build_id": bid, "url": url}
        except (urllib.error.URLError, socket.timeout, ConnectionError, OSError, json.JSONDecodeError):
            continue
    return None


def main() -> int:
    if not SWAPLOG_PATH.exists():
        print("PARSE_ERROR: SWAPLOG.md not found at", SWAPLOG_PATH)
        return 2

    rows = parse_swaplog()
    if not rows:
        print("PARSE_ERROR: no SWAPLOG rows parsed (table format drift?)")
        return 2

    racecontrol_rows = [r for r in rows if is_racecontrol_row(r)]
    if not racecontrol_rows:
        print(
            "PARSE_ERROR: no racecontrol rows in SWAPLOG. "
            "Classifier may need tuning — check size_bytes ranges."
        )
        return 2

    last = racecontrol_rows[-1]
    swaplog_commit = extract_commit(last["commit"])
    swaplog_commit_clean = swaplog_commit.replace("-dirty", "")

    health = probe_health()
    if health is None:
        # Endpoint unreachable from this host (cloud/CI without LAN route).
        # Classify as skipped — NOT a failure. Emit context for observability.
        print(
            f"SKIPPED: /api/v1/health unreachable from this host. "
            f"SWAPLOG last racecontrol row = {swaplog_commit} "
            f"({last['ts']}). Network probe is venue-only."
        )
        return 0

    live_build = health["build_id"]
    # Allow -dirty mismatch so a clean live build equal to a dirty SWAPLOG row
    # does not false-red (or vice versa). This is a heuristic; strict mode
    # would compare verbatim.
    if swaplog_commit_clean.startswith(live_build) or live_build.startswith(swaplog_commit_clean):
        print(
            f"OK: SWAPLOG racecontrol tail = {swaplog_commit}, "
            f"live build_id = {live_build} @ {health['url']}"
        )
        return 0

    print(
        "RED: SWAPLOG racecontrol tail does not match live build_id.\n"
        f"  - SWAPLOG row  : {last['ts']} commit={swaplog_commit}\n"
        f"  - Live health  : build_id={live_build} via {health['url']}\n"
        f"  - Implication  : deploy-server.sh append path silently dropped the "
        f"most recent server-.23 swap (expected: one row per successful swap)."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
