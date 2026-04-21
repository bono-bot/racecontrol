#!/usr/bin/env python3
"""
Workflow-graph generator — Day 5 of the workflow-mapping plan.

Extracts FSM transition tables from Rust source and emits a workflow graph
(states, events, edges) plus per-state degree centrality. Complementary to
board-state-generate.py: board-state captures the DB schema "position";
workflow-graph captures the runtime state-machine "position".

Scope (v1):
- crates/racecontrol/src/billing_fsm.rs — TRANSITION_TABLE pattern

Future scope (v2):
- game launcher FSM, AC multiplayer group FSM, any additional state machines
  that follow the `&[(State, Event, State)]` convention

Usage:
    python3 scripts/audit/workflow-graph-generate.py
        -> writes .planning/workflow-graph.json

Pattern parsed (billing_fsm.rs:71-123 as of 2026-04-21):
    const TRANSITION_TABLE: &[(BillingSessionStatus, BillingEvent, BillingSessionStatus)] = &[
        (BillingSessionStatus::Pending, BillingEvent::StartWaiting, BillingSessionStatus::WaitingForGame),
        ...
    ];

Output schema (stable for downstream consumers):
{
  "timestamp_ist": "YYYY-MM-DD HH:MM IST",
  "commit": "<short hash>",
  "fsms": [
    {
      "name": "billing",
      "source_file": "crates/racecontrol/src/billing_fsm.rs",
      "state_type": "BillingSessionStatus",
      "event_type": "BillingEvent",
      "states": ["Pending", "WaitingForGame", ...],
      "events": ["StartWaiting", "GameLive", ...],
      "transitions": [
        {"from": "Pending", "event": "StartWaiting", "to": "WaitingForGame"},
        ...
      ],
      "centrality": {
        "state_degree": {"Active": {"in": 5, "out": 8, "total": 13}, ...},
        "event_frequency": {"End": 6, "Cancel": 5, ...},
        "god_state": "Active",
        "god_event": "End",
        "terminal_states": ["Completed", ...]
      }
    }
  ]
}

Read-only over source; safe to run anywhere.
"""

from __future__ import annotations

import json
import re
import sys
import subprocess
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
OUT_PATH = REPO_ROOT / ".planning" / "workflow-graph.json"

# Whitelist of FSM sources. Each entry: (name, path, state_type, event_type).
# Add rows here as new FSMs are encoded using the TRANSITION_TABLE idiom.
FSM_SOURCES = [
    (
        "billing",
        REPO_ROOT / "crates" / "racecontrol" / "src" / "billing_fsm.rs",
        "BillingSessionStatus",
        "BillingEvent",
    ),
]


def ist_now() -> str:
    return (datetime.now(timezone.utc) + timedelta(hours=5, minutes=30)).strftime(
        "%Y-%m-%d %H:%M IST"
    )


def short_commit() -> str:
    try:
        out = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=REPO_ROOT,
            stderr=subprocess.DEVNULL,
        )
        return out.decode().strip()
    except Exception:
        return "unknown"


def parse_transition_table(
    source_path: Path, state_type: str, event_type: str
) -> dict:
    """Parse a `const TRANSITION_TABLE: &[(State, Event, State)] = &[...];` block."""
    if not source_path.exists():
        raise FileNotFoundError(f"FSM source not found: {source_path}")

    text = source_path.read_text(encoding="utf-8")

    # Locate the TRANSITION_TABLE block. Accept any identifier ending in
    # TRANSITION_TABLE to allow per-FSM naming (e.g., BILLING_TRANSITION_TABLE).
    block_re = re.compile(
        r"const\s+\w*TRANSITION_TABLE\s*:\s*&\[.*?\]\s*=\s*&\[(.*?)\];",
        re.DOTALL,
    )
    block_match = block_re.search(text)
    if not block_match:
        raise ValueError(f"No TRANSITION_TABLE block found in {source_path}")

    body = block_match.group(1)

    # Match tuples of form (State::Variant, Event::Variant, State::Variant).
    # Tolerates whitespace/comments between rows.
    tuple_re = re.compile(
        rf"\(\s*{re.escape(state_type)}::(\w+)\s*,\s*"
        rf"{re.escape(event_type)}::(\w+)\s*,\s*"
        rf"{re.escape(state_type)}::(\w+)\s*\)",
        re.MULTILINE,
    )

    transitions = []
    states: set[str] = set()
    events: set[str] = set()

    for match in tuple_re.finditer(body):
        from_state, event, to_state = match.group(1), match.group(2), match.group(3)
        transitions.append({"from": from_state, "event": event, "to": to_state})
        states.add(from_state)
        states.add(to_state)
        events.add(event)

    if not transitions:
        raise ValueError(
            f"TRANSITION_TABLE block found in {source_path} but no "
            f"({state_type}::X, {event_type}::Y, {state_type}::Z) tuples parsed"
        )

    return {
        "states": sorted(states),
        "events": sorted(events),
        "transitions": transitions,
    }


def compute_centrality(parsed: dict) -> dict:
    """Degree centrality per state + event frequency."""
    in_degree: Counter[str] = Counter()
    out_degree: Counter[str] = Counter()
    event_freq: Counter[str] = Counter()

    for t in parsed["transitions"]:
        out_degree[t["from"]] += 1
        in_degree[t["to"]] += 1
        event_freq[t["event"]] += 1

    state_degree = {}
    for state in parsed["states"]:
        state_degree[state] = {
            "in": in_degree[state],
            "out": out_degree[state],
            "total": in_degree[state] + out_degree[state],
        }

    terminal_states = [s for s in parsed["states"] if out_degree[s] == 0]

    # God state = highest total degree (ties broken alphabetically for stability).
    god_state = max(
        parsed["states"],
        key=lambda s: (state_degree[s]["total"], -ord(s[0]) if s else 0),
    )

    god_event = event_freq.most_common(1)[0][0] if event_freq else None

    return {
        "state_degree": state_degree,
        "event_frequency": dict(event_freq.most_common()),
        "god_state": god_state,
        "god_event": god_event,
        "terminal_states": sorted(terminal_states),
        "transition_count": len(parsed["transitions"]),
        "state_count": len(parsed["states"]),
        "event_count": len(parsed["events"]),
    }


def main() -> int:
    fsms_out = []
    errors = []

    for name, path, state_type, event_type in FSM_SOURCES:
        try:
            parsed = parse_transition_table(path, state_type, event_type)
        except (FileNotFoundError, ValueError) as e:
            errors.append(f"{name}: {e}")
            continue

        centrality = compute_centrality(parsed)

        rel_path = path.relative_to(REPO_ROOT).as_posix()
        fsms_out.append(
            {
                "name": name,
                "source_file": rel_path,
                "state_type": state_type,
                "event_type": event_type,
                "states": parsed["states"],
                "events": parsed["events"],
                "transitions": parsed["transitions"],
                "centrality": centrality,
            }
        )

    doc = {
        "timestamp_ist": ist_now(),
        "commit": short_commit(),
        "fsms": fsms_out,
        "errors": errors,
    }

    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUT_PATH.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8")

    # stderr summary for orchestrator logs (stdout stays clean for piping).
    fsm_count = len(fsms_out)
    err_count = len(errors)
    sys.stderr.write(
        f"workflow-graph-generate: {fsm_count} FSM(s) parsed, "
        f"{err_count} error(s) -> {OUT_PATH.relative_to(REPO_ROOT).as_posix()}\n"
    )

    if errors:
        for e in errors:
            sys.stderr.write(f"  ERROR: {e}\n")
        return 2

    return 0


if __name__ == "__main__":
    sys.exit(main())
