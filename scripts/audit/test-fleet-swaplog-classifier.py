#!/usr/bin/env python3
"""
Unit tests for fleet-swaplog-parity-check.py's `is_racecontrol_row` classifier.

Covers the 2026-04-21 Phase 445 false-positive class (frontend-only SWAPLOG
row misclassified as a racecontrol binary swap) + adjacent edge cases:

  - Pure racecontrol binary row (old pattern)
  - rc-agent-only row
  - Kiosk zip row
  - Web-only row
  - Mixed rc-agent + racecontrol row (historical deploy pattern)
  - Phase 445 frontend-only row (size="N/A (frontend rebuild, no rust binary)")
  - PR-merge row (commit annotated with "PR #N merge")
  - Edge: short hash, -dirty suffix, whitespace variance

Run:
    python3 scripts/audit/test-fleet-swaplog-classifier.py
Exit 0 = all pass.
Exit 1 = one or more assertions failed.
"""

from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

# Import the classifier via spec_loader since the filename has a hyphen.
MODULE_PATH = Path(__file__).parent / "fleet-swaplog-parity-check.py"
spec = importlib.util.spec_from_file_location("swaplog_check", MODULE_PATH)
mod = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mod)

is_racecontrol_row = mod.is_racecontrol_row
parse_size = mod.parse_size
extract_commit = mod.extract_commit


def mkrow(commit: str, size: str, reason: str, ts: str = "2026-04-21 22:07 IST", by: str = "james", sha: str = "-") -> dict:
    return {"ts": ts, "commit": commit, "size": size, "sha": sha, "by": by, "reason": reason}


TESTS = []


def t(name: str, row: dict, expect: bool):
    TESTS.append((name, row, expect))


# -------- Positive: legitimate racecontrol binary swaps --------
t(
    "pure racecontrol binary row",
    mkrow(
        commit="f9c6678b",
        size="60632064",
        reason="fix(legal) DPDP erase 9-gap closure — customer_data_delete FK coverage. Deploy via deploy-staging/deploy-server.sh f9c6678b.",
    ),
    expect=True,
)
t(
    "racecontrol with -dirty suffix",
    mkrow(
        commit="a97c7491-dirty",
        size="60987136",
        reason="racecontrol dirty build for hot-patch validation",
    ),
    expect=True,
)
t(
    "racecontrol by reason text only, size not recorded",
    mkrow(
        commit="abc1234",
        size="(not recorded)",
        reason="racecontrol redeploy + rc-agent co-ship",
    ),
    expect=True,
)
t(
    "mixed rc-agent + racecontrol row — size picks the larger (racecontrol)",
    mkrow(
        commit="0c0c8134",
        size="26727424 (rc-agent) / 60727808 (racecontrol)",
        reason="Pod 1-8 rc-agent + server .23 racecontrol co-deploy",
    ),
    expect=True,
)

# -------- Negative: non-racecontrol rows --------
t(
    "rc-agent-only row (commit annotated)",
    mkrow(
        commit="a13942f2-dirty (rc-agent, Pod 6 catchup)",
        size="26727424",
        reason="rc-agent only — Pod 6 catchup deploy",
    ),
    expect=False,
)
t(
    "kiosk zip row (commit annotated)",
    mkrow(
        commit="1257a1b5 (venue kiosk :3300)",
        size="11589879 (zip)",
        reason="fix(kiosk): remove dead multiplayer scaffolding",
    ),
    expect=False,
)
t(
    "web-only row (commit annotated web :3200)",
    mkrow(
        commit="ba84c0f6 (web :3200)",
        size="17234567",
        reason="web only dashboard update",
    ),
    expect=False,
)

# -------- Negative: frontend-only deploys (Phase 445 class) --------
t(
    "Phase 445 frontend-only row",
    mkrow(
        commit="d52a8a72 (racecontrol main, PR #9 merge)",
        size="N/A (frontend rebuild, no rust binary)",
        reason="feat(phase-445) Typed API Contract SHIPPED. Cloud admin rebuild on Bono VPS.",
    ),
    expect=False,
)
t(
    "frontend-only with lowercase N/A prefix",
    mkrow(
        commit="abc1234",
        size="n/a (frontend rebuild, admin rebuild only)",
        reason="feat(admin) some frontend-only work — racecontrol code base unchanged at runtime",
    ),
    expect=False,
)
t(
    "frontend-only flagged in reason not size",
    mkrow(
        commit="abc1234",
        size="(not recorded)",
        reason="admin frontend rebuild, no rust binary swap — Phase 500 placeholder",
    ),
    expect=False,
)
t(
    "PR-merge row for a racecontrol frontend-only ship",
    mkrow(
        commit="deadbeef (PR #42 merge)",
        size="(not recorded)",
        reason="feat(phase-500) — kiosk migration to generated types",
    ),
    expect=False,
)


def main() -> int:
    failed = []
    for name, row, expect in TESTS:
        actual = is_racecontrol_row(row)
        ok = actual == expect
        print(f"  [{'OK' if ok else 'FAIL'}] {name}: is_racecontrol_row={actual} (expected {expect})")
        if not ok:
            failed.append(name)

    print()
    print(f"Total: {len(TESTS)} | Passed: {len(TESTS) - len(failed)} | Failed: {len(failed)}")
    if failed:
        print(f"Failed tests: {failed}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
