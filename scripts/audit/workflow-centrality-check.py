#!/usr/bin/env python3
"""
Workflow-centrality checker — Day 5 of the workflow-mapping plan.

Compares the live workflow-graph.json (produced by workflow-graph-generate.py)
against a committed baseline. Flags drift in:

  - God state (most-connected state per FSM)
  - God event (most-frequent event per FSM)
  - State set (added/removed states)
  - Event set (added/removed events)
  - Per-state degree changes above threshold
  - Terminal state set

Rationale: when someone adds/removes a transition, the structural centrality
of the FSM changes. Most changes are intentional (Phase 414 grew
WaitingForGame from 2 → 5 out-edges). Some are silent regressions. The
baseline lets reviewers acknowledge intended shifts (by committing a new
baseline in the same PR) and catches unacknowledged ones.

Usage:
    python3 scripts/audit/workflow-centrality-check.py

Exit codes:
    0 — no drift, OR baseline missing and seeded (first run)
    1 — drift detected (prints gaps list to stdout)

Baseline file: .planning/workflow-graph.baseline.json
  - First run with no baseline: creates baseline from current graph, exits 0
  - Subsequent runs: diffs current vs baseline

Pre-commit flow:
  1. run-all-checkers.sh invokes workflow-graph-generate.py
  2. This checker compares against committed baseline
  3. If drift is acceptable (new FSM, refactored transitions), commit both:
     .planning/workflow-graph.json AND .planning/workflow-graph.baseline.json
  4. If drift is a bug, commit is blocked

Read-only over source; writes only to .planning/ (and only seeds baseline
when absent).
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
CURRENT_PATH = REPO_ROOT / ".planning" / "workflow-graph.json"
BASELINE_PATH = REPO_ROOT / ".planning" / "workflow-graph.baseline.json"

# Degree-drift threshold — ignore changes smaller than this for noise-reduction.
# Set to 1 because any change in a small-state graph (11 states) is meaningful.
DEGREE_DRIFT_THRESHOLD = 1


def load_graph(path: Path) -> dict | None:
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        sys.stderr.write(f"ERROR: failed to parse {path}: {e}\n")
        return None


def fsm_by_name(doc: dict) -> dict:
    return {f["name"]: f for f in doc.get("fsms", [])}


def compare_fsm(current: dict, baseline: dict, gaps: list[str]) -> None:
    """Append human-readable gap lines to `gaps` if the FSM drifted."""
    name = current["name"]
    cur_c = current["centrality"]
    base_c = baseline["centrality"]

    # God state / god event shift
    if cur_c["god_state"] != base_c["god_state"]:
        gaps.append(
            f"  - [{name}] god_state changed: "
            f"{base_c['god_state']} -> {cur_c['god_state']}"
        )

    if cur_c["god_event"] != base_c["god_event"]:
        gaps.append(
            f"  - [{name}] god_event changed: "
            f"{base_c['god_event']} -> {cur_c['god_event']}"
        )

    # State set changes
    cur_states = set(current["states"])
    base_states = set(baseline["states"])
    added_states = cur_states - base_states
    removed_states = base_states - cur_states
    for s in sorted(added_states):
        gaps.append(f"  - [{name}] state added: {s}")
    for s in sorted(removed_states):
        gaps.append(f"  - [{name}] state removed: {s}")

    # Event set changes
    cur_events = set(current["events"])
    base_events = set(baseline["events"])
    added_events = cur_events - base_events
    removed_events = base_events - cur_events
    for e in sorted(added_events):
        gaps.append(f"  - [{name}] event added: {e}")
    for e in sorted(removed_events):
        gaps.append(f"  - [{name}] event removed: {e}")

    # Per-state degree drift (only for states present in both versions)
    common_states = cur_states & base_states
    cur_deg = cur_c["state_degree"]
    base_deg = base_c["state_degree"]
    for s in sorted(common_states):
        cur_total = cur_deg[s]["total"]
        base_total = base_deg[s]["total"]
        if abs(cur_total - base_total) >= DEGREE_DRIFT_THRESHOLD:
            gaps.append(
                f"  - [{name}] {s} degree: "
                f"{base_total} -> {cur_total} "
                f"(in {base_deg[s]['in']}->{cur_deg[s]['in']}, "
                f"out {base_deg[s]['out']}->{cur_deg[s]['out']})"
            )

    # Terminal-state changes (these are often safety-critical)
    cur_term = set(cur_c["terminal_states"])
    base_term = set(base_c["terminal_states"])
    for t in sorted(cur_term - base_term):
        gaps.append(f"  - [{name}] terminal state added: {t}")
    for t in sorted(base_term - cur_term):
        gaps.append(
            f"  - [{name}] terminal state removed (now has outbound transitions): {t}"
        )


def main() -> int:
    current = load_graph(CURRENT_PATH)
    if current is None:
        sys.stderr.write(
            f"ERROR: {CURRENT_PATH.relative_to(REPO_ROOT).as_posix()} missing. "
            "Run workflow-graph-generate.py first.\n"
        )
        return 2

    baseline = load_graph(BASELINE_PATH)

    if baseline is None:
        # Seed the baseline on first run.
        BASELINE_PATH.write_text(
            json.dumps(current, indent=2) + "\n", encoding="utf-8"
        )
        sys.stderr.write(
            f"workflow-centrality-check: baseline seeded at "
            f"{BASELINE_PATH.relative_to(REPO_ROOT).as_posix()} "
            f"(first run — commit the baseline to lock current centrality).\n"
        )
        return 0

    current_fsms = fsm_by_name(current)
    baseline_fsms = fsm_by_name(baseline)

    gaps: list[str] = []

    # FSM-level adds/removes
    cur_names = set(current_fsms.keys())
    base_names = set(baseline_fsms.keys())
    for name in sorted(cur_names - base_names):
        gaps.append(f"  - FSM added: {name} (commit baseline to acknowledge)")
    for name in sorted(base_names - cur_names):
        gaps.append(f"  - FSM removed: {name} (was in baseline, missing now)")

    # Per-FSM drift
    for name in sorted(cur_names & base_names):
        compare_fsm(current_fsms[name], baseline_fsms[name], gaps)

    if not gaps:
        sys.stderr.write(
            "workflow-centrality-check: no drift — FSM centrality matches baseline.\n"
        )
        return 0

    # RED. Print human-readable gap list with the `  - ` prefix the
    # orchestrator's summary counter relies on.
    print("workflow-centrality drift detected:")
    for line in gaps:
        print(line)
    print()
    print(
        "If intentional (e.g. new FSM row, refactored transitions): regenerate "
        "the baseline and commit both files:"
    )
    print("  cp .planning/workflow-graph.json .planning/workflow-graph.baseline.json")
    print("  git add .planning/workflow-graph.json .planning/workflow-graph.baseline.json")
    return 1


if __name__ == "__main__":
    sys.exit(main())
