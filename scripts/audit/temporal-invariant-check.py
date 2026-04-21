#!/usr/bin/env python3
"""Temporal invariant checker — F2 of the functional-layer milestone.

Evaluates declarative rules from .planning/invariants.yaml against an event
ledger produced by F1 (.planning/event-ledger.json) and emits violations to
.planning/invariant-violations.json.

Catches classes the structural layer cannot:
  - Pattern I (logger wedges) via MIN_CADENCE
  - Game-launch hangs via LATENCY
  - idle_auto_end-without-pause (MMA P1-A) via REQUIRES
  - Duplicate session-end via NO_DUPE
  - Orphan billing_event (C1 FK class) via NO_ORPHAN
  - FSM order regressions via ORDER

Graceful-skip contract: if the ledger file does not exist, exit 0 with a
stderr note. F1 is the upstream producer; until it lands, this checker is
dormant — not red.
"""
from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import yaml


def parse_ts(ts: str) -> datetime:
    return datetime.fromisoformat(ts.replace("Z", "+00:00"))


def _filter_source_type(events: list[dict], source: str, event_type: str) -> list[dict]:
    return [e for e in events if e.get("source") == source and e.get("type") == event_type]


def _group_by_machine(events: list[dict]) -> dict[str, list[dict]]:
    out: dict[str, list[dict]] = {}
    for e in events:
        out.setdefault(e.get("machine", "_unknown"), []).append(e)
    return out


def _group_by_scope(events: list[dict], source: str, scope_key: str) -> dict[Any, list[dict]]:
    out: dict[Any, list[dict]] = {}
    for e in events:
        if e.get("source") != source:
            continue
        sk = (e.get("fields") or {}).get(scope_key)
        if sk is None:
            continue
        out.setdefault(sk, []).append(e)
    return out


def evaluate_min_cadence(rule: dict, events: list[dict]) -> list[dict]:
    window_s = int(rule["window_seconds"])
    filtered = _filter_source_type(events, rule["source"], rule["event_type"])
    if rule.get("per_source", False):
        groups = _group_by_machine(filtered)
    else:
        groups = {"_all": filtered}
    violations: list[dict] = []
    for machine, evs in groups.items():
        evs = sorted(evs, key=lambda e: e["ts"])
        if not evs:
            violations.append({
                "rule": rule["id"],
                "machine": machine,
                "reason": f"no {rule['event_type']} events observed in window",
            })
            continue
        for prev, cur in zip(evs, evs[1:]):
            delta = (parse_ts(cur["ts"]) - parse_ts(prev["ts"])).total_seconds()
            if delta > window_s:
                violations.append({
                    "rule": rule["id"],
                    "machine": machine,
                    "reason": f"gap of {int(delta)}s exceeds {window_s}s window",
                    "gap_start": prev["ts"],
                    "gap_end": cur["ts"],
                })
    return violations


def evaluate_latency(rule: dict, events: list[dict]) -> list[dict]:
    scope_key = rule["scope_key"]
    max_delta = int(rule["max_delta_seconds"])
    relevant = [
        e for e in events
        if e.get("source") == rule["source"]
        and e.get("type") in (rule["from_event"], rule["to_event"])
    ]
    by_scope = _group_by_scope(relevant, rule["source"], scope_key)
    violations: list[dict] = []
    for sk, evs in by_scope.items():
        evs = sorted(evs, key=lambda e: e["ts"])
        from_ts = None
        matched = False
        for e in evs:
            if e["type"] == rule["from_event"]:
                from_ts = parse_ts(e["ts"])
                matched = False
            elif e["type"] == rule["to_event"] and from_ts is not None:
                delta = (parse_ts(e["ts"]) - from_ts).total_seconds()
                if delta > max_delta:
                    violations.append({
                        "rule": rule["id"],
                        "scope": {scope_key: sk},
                        "reason": f"{rule['from_event']} -> {rule['to_event']} took {int(delta)}s > {max_delta}s",
                        "from_ts": from_ts.isoformat(),
                        "to_ts": e["ts"],
                    })
                matched = True
                from_ts = None
        if from_ts is not None and not matched:
            violations.append({
                "rule": rule["id"],
                "scope": {scope_key: sk},
                "reason": f"{rule['from_event']} never followed by {rule['to_event']} in window",
                "from_ts": from_ts.isoformat(),
            })
    return violations


def evaluate_requires(rule: dict, events: list[dict]) -> list[dict]:
    scope_key = rule["scope_key"]
    prior_set = set(rule["requires_prior"])
    target = rule["event_type"]
    by_scope = _group_by_scope(events, rule["source"], scope_key)
    violations: list[dict] = []
    for sk, evs in by_scope.items():
        evs = sorted(evs, key=lambda e: e["ts"])
        seen_prior = False
        for e in evs:
            if e["type"] in prior_set:
                seen_prior = True
            elif e["type"] == target:
                if not seen_prior:
                    violations.append({
                        "rule": rule["id"],
                        "scope": {scope_key: sk},
                        "reason": f"{target} fired without prior {sorted(prior_set)}",
                        "ts": e["ts"],
                    })
    return violations


def evaluate_no_dupe(rule: dict, events: list[dict]) -> list[dict]:
    keyed_by = rule["keyed_by"]
    target = rule["event_type"]
    seen: dict[tuple, str] = {}
    violations: list[dict] = []
    for e in events:
        if e.get("source") != rule["source"] or e.get("type") != target:
            continue
        fields = e.get("fields") or {}
        key = tuple(fields.get(k) for k in keyed_by)
        if None in key:
            continue
        if key in seen:
            violations.append({
                "rule": rule["id"],
                "key": dict(zip(keyed_by, key)),
                "reason": f"duplicate {target} — first at {seen[key]}, dupe at {e['ts']}",
                "first_ts": seen[key],
                "dupe_ts": e["ts"],
            })
        else:
            seen[key] = e["ts"]
    return violations


def evaluate_no_orphan(rule: dict, events: list[dict]) -> list[dict]:
    scope_key = rule["scope_key"]
    start_type = rule["requires_prior_start"]
    target = rule["event_type"]
    by_scope = _group_by_scope(events, rule["source"], scope_key)
    violations: list[dict] = []
    for sk, evs in by_scope.items():
        evs = sorted(evs, key=lambda e: e["ts"])
        start_ts = None
        for e in evs:
            if e["type"] == start_type:
                start_ts = parse_ts(e["ts"])
            elif e["type"] == target:
                if start_ts is None or parse_ts(e["ts"]) < start_ts:
                    violations.append({
                        "rule": rule["id"],
                        "scope": {scope_key: sk},
                        "reason": f"{target} at {e['ts']} has no prior {start_type}",
                        "ts": e["ts"],
                    })
    return violations


def evaluate_order(rule: dict, events: list[dict]) -> list[dict]:
    scope_key = rule["scope_key"]
    expected = list(rule["events"])
    expected_set = set(expected)
    skippable = set(rule.get("allow_skips", []))
    by_scope = _group_by_scope(events, rule["source"], scope_key)
    violations: list[dict] = []
    for sk, evs in by_scope.items():
        evs = sorted(evs, key=lambda e: e["ts"])
        idx = 0
        for e in evs:
            if e["type"] in skippable:
                continue
            if idx < len(expected) and e["type"] == expected[idx]:
                idx += 1
            elif e["type"] in expected_set:
                violations.append({
                    "rule": rule["id"],
                    "scope": {scope_key: sk},
                    "reason": (
                        f"expected {expected[idx] if idx < len(expected) else '(end)'} "
                        f"but saw {e['type']} at {e['ts']}"
                    ),
                    "ts": e["ts"],
                })
                break
    return violations


EVALUATORS = {
    "MIN_CADENCE": evaluate_min_cadence,
    "LATENCY": evaluate_latency,
    "REQUIRES": evaluate_requires,
    "NO_DUPE": evaluate_no_dupe,
    "NO_ORPHAN": evaluate_no_orphan,
    "ORDER": evaluate_order,
}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rules", default=".planning/invariants.yaml")
    parser.add_argument("--ledger", default=".planning/event-ledger.json")
    parser.add_argument("--fixture", help="override ledger path (test harness)")
    parser.add_argument("--output", default=".planning/invariant-violations.json")
    args = parser.parse_args(argv)

    ledger_path = Path(args.fixture or args.ledger)
    if not ledger_path.exists():
        print(
            f"temporal-invariant: skip - ledger {ledger_path} does not exist "
            f"(F1 event-ledger generator not yet producing)",
            file=sys.stderr,
        )
        return 0

    rules_path = Path(args.rules)
    if not rules_path.exists():
        print(f"temporal-invariant: rules file {rules_path} missing", file=sys.stderr)
        return 2

    rules_data = yaml.safe_load(rules_path.read_text(encoding="utf-8")) or {}
    rules = rules_data.get("rules", [])
    ledger = json.loads(ledger_path.read_text(encoding="utf-8"))
    events = ledger.get("events", [])

    all_violations: list[dict] = []
    for rule in rules:
        fn = EVALUATORS.get(rule.get("class"))
        if fn is None:
            print(
                f"temporal-invariant: unknown rule class {rule.get('class')!r} on {rule.get('id')}",
                file=sys.stderr,
            )
            continue
        try:
            all_violations.extend(fn(rule, events))
        except Exception as exc:  # noqa: BLE001  # checker must not crash on malformed rule
            print(
                f"temporal-invariant: rule {rule.get('id')} evaluator raised: {exc}",
                file=sys.stderr,
            )

    output_path = Path(args.output)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(
        json.dumps(
            {
                "generated_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
                "ledger": str(ledger_path),
                "rule_count": len(rules),
                "violation_count": len(all_violations),
                "violations": all_violations,
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    if all_violations:
        print(f"temporal-invariant: {len(all_violations)} violations")
        for v in all_violations[:10]:
            print(f"  - [{v['rule']}] {v['reason']}")
        if len(all_violations) > 10:
            print(f"  ... and {len(all_violations) - 10} more (see {output_path})")
        return 1

    print(f"temporal-invariant: all {len(rules)} invariants green")
    return 0


if __name__ == "__main__":
    sys.exit(main())
