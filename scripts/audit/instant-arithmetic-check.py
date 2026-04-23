#!/usr/bin/env python3
"""Ratcheting lint for `Instant::now() - Duration::...` patterns.

Motivation
==========
Subtracting a Duration from `Instant::now()` panics with
"overflow when subtracting duration from instant" on any platform where
`Instant` cannot represent pre-boot times. On Windows, `Instant` is backed
by QueryPerformanceCounter anchored at system start; on Linux it's backed
by CLOCK_MONOTONIC anchored at boot. Fresh CI runners typically have <2min
uptime — any test subtracting more than that from `Instant::now()` is a
time bomb that passes on long-uptime dev machines and detonates on CI.

This class has recurred twice so far:
  * 2026-04-09  session_enforcer.rs:296  (original, backlog 999.1)
  * 2026-04-21  event_loop.rs:2832       (SN-01 test, fixed in PR #2)

Memory-only documentation (feedback_instant_arithmetic_on_ci.md) did not
prevent the 2026-04-21 recurrence. This checker is the structural gate
that should.

Behavior
========
* Scans all `*.rs` files under `crates/` (skipping `target/`).
* Reports any line matching `Instant::now() - Duration::` that is not
  exempted.
* Compares the hit set against a persisted baseline of known patterns.
* Exits 1 if new (file, content_hash) pairs exist that aren't baselined.
* Exits 0 when every current hit is baselined (ratchets down over time
  as the codebase is refactored).

Exempting a legitimate use
==========================
Add `// INSTANT_SUB_SAFE: <reason>` on the same line as the expression
or on the immediately preceding line. Use sparingly — the preferred
refactors are `Instant::checked_sub()`, `saturating_duration_since()`, or
threading `Duration` through the API instead of `Instant`.

Adding a legitimately baselined pattern
=======================================
When an audit determines a flagged pattern is acceptable (e.g. low-risk
production code that will not run on short-uptime CI), update the
baseline:

    python3 scripts/audit/instant-arithmetic-check.py --update-baseline

The baseline lives at `scripts/audit/instant-arithmetic-baseline.json`.
It is committed to the repo and acts as the ratchet — new hits must
either be refactored or explicitly accepted into the baseline via this
command (and PR review).

Exit codes
==========
  0 — no new violations (all hits are baselined)
  1 — at least one new violation
  2 — checker internal error
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
CRATES_DIR = REPO_ROOT / "crates"
BASELINE_PATH = REPO_ROOT / "scripts" / "audit" / "instant-arithmetic-baseline.json"
PATTERN = re.compile(r"Instant::now\(\)\s*-\s*Duration::")
EXEMPT_MARKER = "INSTANT_SUB_SAFE"


def _line_is_comment(line: str) -> bool:
    s = line.lstrip()
    return s.startswith("//") or s.startswith("///") or s.startswith("*")


def scan_hits(root: Path = CRATES_DIR):
    """Yield {file, line, hash, content} for each active match."""
    hits = []
    if not root.is_dir():
        raise RuntimeError(f"crates dir not found: {root}")
    for rs in sorted(root.rglob("*.rs")):
        if "target" in rs.parts:
            continue
        try:
            text = rs.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for m in PATTERN.finditer(text):
            line_start = text.rfind("\n", 0, m.start()) + 1
            line_end = text.find("\n", m.end())
            if line_end == -1:
                line_end = len(text)
            line = text[line_start:line_end]
            if _line_is_comment(line):
                continue
            if EXEMPT_MARKER in line:
                continue
            # Check preceding line for an inline exempt marker.
            prev_start = text.rfind("\n", 0, line_start - 1) + 1
            prev_line = text[prev_start:line_start].rstrip("\n")
            if EXEMPT_MARKER in prev_line:
                continue
            try:
                rel = str(rs.relative_to(REPO_ROOT)).replace("\\", "/")
            except ValueError:
                rel = str(rs.relative_to(root)).replace("\\", "/")
            content = line.strip()
            content_hash = hashlib.sha256(content.encode("utf-8")).hexdigest()[:12]
            line_num = text[: m.start()].count("\n") + 1
            hits.append(
                {
                    "file": rel,
                    "line": line_num,
                    "hash": content_hash,
                    "content": content,
                }
            )
    return hits


def load_baseline(path: Path = BASELINE_PATH):
    if not path.exists():
        return set()
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        raise RuntimeError(f"baseline file {path} unreadable: {e}") from e
    entries = doc.get("entries", [])
    return {(e["file"], e["hash"]) for e in entries}


def write_baseline(hits, path: Path = BASELINE_PATH):
    entries = [
        {"file": h["file"], "hash": h["hash"], "content": h["content"]}
        for h in sorted(hits, key=lambda h: (h["file"], h["hash"]))
    ]
    doc = {
        "version": 1,
        "description": (
            "Baseline of known `Instant::now() - Duration::` patterns. "
            "The instant-arithmetic-check.py checker fails CI if a new "
            "(file, content_hash) pair appears that isn't in this list. "
            "See feedback_instant_arithmetic_on_ci.md for the class and "
            "PR #2 (c957350e) for the recurrence that motivated the lint."
        ),
        "entries": entries,
    }
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _print_new_hits(new_hits):
    print(f"instant-arithmetic: RED ({len(new_hits)} new violation(s))")
    for h in new_hits:
        print(f"  - {h['file']}:{h['line']}")
        print(f"      {h['content']}")
    print()
    print("Fix options (prefer top to bottom):")
    print("  1. Refactor the API to take Duration instead of Instant,")
    print("     then construct instants from the Duration in the caller.")
    print("  2. Use `Instant::checked_sub(duration)` and match on None.")
    print("  3. Use `now.saturating_duration_since(earlier)` instead of")
    print("     `now - duration` constructions.")
    print(f"  4. If the use is genuinely safe, add a `// {EXEMPT_MARKER}: "
          "<reason>` comment on or above the line.")
    print("  5. If the pattern is intentional infrastructure that should")
    print("     be tolerated, update the baseline:")
    print("       python3 scripts/audit/instant-arithmetic-check.py --update-baseline")
    print()
    print("Context: feedback_instant_arithmetic_on_ci.md")
    print("  Recurrence history:")
    print("    2026-04-09  session_enforcer.rs:296  (original)")
    print("    2026-04-21  event_loop.rs:2832        (SN-01 test, c957350e)")


def main(argv=None) -> int:
    p = argparse.ArgumentParser(
        prog="instant-arithmetic-check",
        description=(
            "Ratcheting lint: `Instant::now() - Duration::...` patterns. "
            "Fails when a new (file, content_hash) pair appears that is "
            "not in scripts/audit/instant-arithmetic-baseline.json."
        ),
    )
    p.add_argument(
        "--update-baseline",
        action="store_true",
        help="Rewrite the baseline file from the current tree's hits. "
             "Intended for reviewer-approved acceptance of new patterns.",
    )
    args = p.parse_args(argv)

    try:
        hits = scan_hits()
    except RuntimeError as e:
        print(f"instant-arithmetic-check: ERROR — {e}", file=sys.stderr)
        return 2

    if args.update_baseline:
        write_baseline(hits)
        print(f"instant-arithmetic: baseline rewritten — {len(hits)} entries")
        return 0

    try:
        baseline = load_baseline()
    except RuntimeError as e:
        print(f"instant-arithmetic-check: ERROR — {e}", file=sys.stderr)
        return 2

    new_hits = [h for h in hits if (h["file"], h["hash"]) not in baseline]
    if new_hits:
        _print_new_hits(new_hits)
        return 1

    n_baseline = len(baseline)
    n_current = len(hits)
    if n_current < n_baseline:
        print(
            f"instant-arithmetic: green "
            f"({n_current}/{n_baseline} baseline entries remain — "
            f"consider refreshing with --update-baseline)"
        )
    else:
        print(
            f"instant-arithmetic: green "
            f"({n_current} known patterns, 0 new)"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
