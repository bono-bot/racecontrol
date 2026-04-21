"""Tests for temporal-invariant-check.py (F2 of functional-layer milestone).

Each INV-XX rule has a dedicated failure fixture exercising exactly that rule.
The happy-path fixture must pass all rules simultaneously.

Fixtures live in tests/audit/fixtures/event-ledgers/ and use the v1 ledger
schema (flat events list) that F1 will produce.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "audit" / "temporal-invariant-check.py"
RULES = REPO_ROOT / ".planning" / "invariants.yaml"
FIXTURE_DIR = Path(__file__).parent / "fixtures" / "event-ledgers"


def run_checker(fixture: Path | None, tmp_path: Path) -> tuple[int, str, str, dict | None]:
    output = tmp_path / "violations.json"
    cmd = [sys.executable, str(SCRIPT), "--rules", str(RULES), "--output", str(output)]
    if fixture is not None:
        cmd += ["--fixture", str(fixture)]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    data = json.loads(output.read_text()) if output.exists() else None
    return proc.returncode, proc.stdout, proc.stderr, data


def rule_ids(violations: list[dict]) -> set[str]:
    return {v["rule"] for v in violations}


def test_happy_path_all_green(tmp_path: Path) -> None:
    rc, out, err, data = run_checker(FIXTURE_DIR / "happy-path.json", tmp_path)
    assert rc == 0, f"expected green, got violations:\n{out}\n{err}"
    assert data["violation_count"] == 0


def test_inv01_heartbeat_gap_fires(tmp_path: Path) -> None:
    """Pattern I reproduction — 11min gap in pod_6 heartbeats."""
    rc, out, err, data = run_checker(FIXTURE_DIR / "inv01-heartbeat-gap.json", tmp_path)
    assert rc == 1, f"expected red, got:\n{out}"
    assert "INV-01" in rule_ids(data["violations"])


def test_inv02_latency_exceeded_fires(tmp_path: Path) -> None:
    """StartWaiting -> GameLive exceeded 90s — game-launch hang detection."""
    rc, out, err, data = run_checker(FIXTURE_DIR / "inv02-latency-exceeded.json", tmp_path)
    assert rc == 1
    assert "INV-02" in rule_ids(data["violations"])


def test_inv03_idle_auto_end_without_pause_fires(tmp_path: Path) -> None:
    """MMA P1-A reproduction — idle_auto_end_queued without prior pause event."""
    rc, out, err, data = run_checker(FIXTURE_DIR / "inv03-requires-missing.json", tmp_path)
    assert rc == 1
    assert "INV-03" in rule_ids(data["violations"])


def test_inv04_duplicate_session_end_fires(tmp_path: Path) -> None:
    """billing_session_end firing twice for the same session_id."""
    rc, out, err, data = run_checker(FIXTURE_DIR / "inv04-dupe.json", tmp_path)
    assert rc == 1
    assert "INV-04" in rule_ids(data["violations"])


def test_inv05_orphan_billing_event_fires(tmp_path: Path) -> None:
    """C1 FK orphan class — billing_event with no prior billing_session_start."""
    rc, out, err, data = run_checker(FIXTURE_DIR / "inv05-orphan.json", tmp_path)
    assert rc == 1
    assert "INV-05" in rule_ids(data["violations"])


def test_inv06_fsm_order_violation_fires(tmp_path: Path) -> None:
    """EndedEarly before Active — FSM order regression."""
    rc, out, err, data = run_checker(FIXTURE_DIR / "inv06-order-wrong.json", tmp_path)
    assert rc == 1
    assert "INV-06" in rule_ids(data["violations"])


def test_missing_ledger_graceful_skip(tmp_path: Path) -> None:
    """When F1 has not yet produced a ledger, F2 must skip — not fail.

    This is the null-test reality: without F1 in place, F2's gate provides
    zero signal. The gate stays green by design until the upstream producer
    lands, so pre-commit cascades don't break during the transition.
    """
    rc, out, err, data = run_checker(tmp_path / "nonexistent-ledger.json", tmp_path)
    assert rc == 0
    assert "skip" in err.lower() or "does not exist" in err.lower()


def test_rules_file_parses() -> None:
    """Structural check — invariants.yaml loads with the expected top-level keys."""
    import yaml
    data = yaml.safe_load(RULES.read_text(encoding="utf-8"))
    assert data["version"] == 1
    assert isinstance(data["rules"], list)
    assert len(data["rules"]) >= 6
    rule_classes = {r["class"] for r in data["rules"]}
    assert rule_classes == {"MIN_CADENCE", "LATENCY", "REQUIRES", "NO_DUPE", "NO_ORPHAN", "ORDER"}


@pytest.mark.parametrize("rule_id", ["INV-01", "INV-02", "INV-03", "INV-04", "INV-05", "INV-06"])
def test_every_rule_has_catches_annotation(rule_id: str) -> None:
    """Every rule must cite a historical incident it was designed to catch."""
    import yaml
    data = yaml.safe_load(RULES.read_text(encoding="utf-8"))
    rule = next(r for r in data["rules"] if r["id"] == rule_id)
    assert rule.get("catches"), f"{rule_id} missing `catches` annotation"
