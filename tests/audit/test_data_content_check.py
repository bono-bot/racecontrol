"""Unit tests for scripts/audit/data-content-check.py (F4).

Coverage:
  * load_rules happy path — real rule pack at
    .planning/functional-layer/data-content-rules.yaml parses without error,
    has exactly 4 rules, IDs R1..R4, expected status flags.
  * load_rules sad paths — missing file, malformed YAML surface as
    RuntimeError with a readable message.
  * handle_python_ini_racecontrol_section — three fixture paths:
      (a) valid fixture -> status=ok
      (b) missing [RACECONTROL] -> status=violation
      (c) missing required key -> status=violation
  * handle_python_ini_racecontrol_section — no --python-ini supplied
    returns status=stubbed (not a crash).
  * handle_python_ini_racecontrol_section — nonexistent path returns
    status=violation with file-not-found message (not a crash).
  * handle_stubbed — returns status=stubbed, echoes the todo field from
    the rule dict.
  * dispatch_rule — unknown handler name + status=implemented returns
    status=error, not an exception.
  * dispatch_rule — handler that raises is caught and reported as
    status=error with traceback in note.
  * data_content_check — end-to-end on real rules file with no options:
    returns aggregated dict with 1 per-rule result per rule, all stubbed,
    exit_code_for maps to 0.
  * data_content_check — with --only filter matching zero rules raises
    RuntimeError (distinct from "rule ok with no violations").
  * exit_code_for — covers ok / violation / error paths.
"""
from __future__ import annotations

from pathlib import Path

import pytest


def test_load_rules_happy_path(checker, rules_path):
    rules = checker.load_rules(rules_path)
    assert isinstance(rules, list)
    assert len(rules) == 4, f"expected 4 rules, got {len(rules)}"
    ids = [r["id"] for r in rules]
    assert ids == ["R1", "R2", "R3", "R4"]

    r1 = next(r for r in rules if r["id"] == "R1")
    assert r1["name"] == "python_ini_racecontrol_section"
    assert r1["status"] == "implemented"
    assert r1["required_section"].strip("[]") == "RACECONTROL"
    assert "ACTIVE" in r1["required_keys"]

    stubbed_rules = [r for r in rules if r["status"] == "stubbed"]
    assert len(stubbed_rules) == 3
    for r in stubbed_rules:
        assert "todo" in r, f"{r['id']} marked stubbed but has no todo"


def test_load_rules_missing_file(checker, tmp_path):
    missing = tmp_path / "does-not-exist.yaml"
    with pytest.raises(RuntimeError, match="rules file not found"):
        checker.load_rules(missing)


def test_load_rules_malformed_yaml(checker, tmp_path):
    bad = tmp_path / "bad.yaml"
    bad.write_text("rules: [not: balanced: bracket\n", encoding="utf-8")
    with pytest.raises(RuntimeError, match="not valid YAML"):
        checker.load_rules(bad)


def test_load_rules_missing_top_level_rules_key(checker, tmp_path):
    bad = tmp_path / "no_rules.yaml"
    bad.write_text("version: 1\nunrelated: []\n", encoding="utf-8")
    with pytest.raises(RuntimeError, match="missing top-level 'rules' list"):
        checker.load_rules(bad)


# ----- R1 handler: fixture coverage --------------------------------------


def _r1_rule_dict():
    return {
        "id": "R1",
        "name": "python_ini_racecontrol_section",
        "status": "implemented",
        "required_section": "[RACECONTROL]",
        "required_keys": ["ACTIVE"],
    }


def test_r1_valid_fixture(checker, fixtures_dir):
    result = checker.handle_python_ini_racecontrol_section(
        _r1_rule_dict(),
        {"python_ini": str(fixtures_dir / "python_ini_valid.ini")},
    )
    assert result["status"] == "ok"
    assert result["violations"] == []


def test_r1_missing_section(checker, fixtures_dir):
    result = checker.handle_python_ini_racecontrol_section(
        _r1_rule_dict(),
        {"python_ini": str(fixtures_dir / "python_ini_missing_section.ini")},
    )
    assert result["status"] == "violation"
    assert len(result["violations"]) == 1
    v = result["violations"][0]
    assert "section missing" in v["actual"]
    assert "Zero Laps P0" in v["message"]


def test_r1_missing_key(checker, fixtures_dir):
    result = checker.handle_python_ini_racecontrol_section(
        _r1_rule_dict(),
        {"python_ini": str(fixtures_dir / "python_ini_missing_key.ini")},
    )
    assert result["status"] == "violation"
    assert len(result["violations"]) == 1
    v = result["violations"][0]
    assert "missing required key" in v["message"].lower()
    assert "active" in v["message"].lower()


def test_r1_no_python_ini_path_returns_stubbed(checker):
    result = checker.handle_python_ini_racecontrol_section(_r1_rule_dict(), {})
    assert result["status"] == "stubbed"
    assert "needs" in result["note"].lower() or "--python-ini" in result["note"]


def test_r1_nonexistent_path_returns_violation(checker, tmp_path):
    result = checker.handle_python_ini_racecontrol_section(
        _r1_rule_dict(),
        {"python_ini": str(tmp_path / "nope.ini")},
    )
    assert result["status"] == "violation"
    assert "not found" in result["violations"][0]["message"].lower()


# ----- Stub handler ------------------------------------------------------


def test_handle_stubbed_echoes_todo(checker):
    rule = {
        "id": "R99",
        "name": "fake_stubbed_rule",
        "status": "stubbed",
        "todo": "waiting on ops auth for SSH access",
    }
    result = checker.handle_stubbed(rule, {})
    assert result["status"] == "stubbed"
    assert result["violations"] == []
    assert "waiting on ops auth" in result["note"]


def test_handle_stubbed_without_todo(checker):
    rule = {"id": "R99", "name": "no_todo_rule", "status": "stubbed"}
    result = checker.handle_stubbed(rule, {})
    assert result["status"] == "stubbed"
    # Should not crash — falls back to a descriptive default.
    assert result["note"] != ""


# ----- Dispatch + error handling -----------------------------------------


def test_dispatch_unknown_handler_implemented_returns_error(checker):
    rule = {
        "id": "RX",
        "name": "no_such_handler_registered",
        "status": "implemented",
    }
    result = checker.dispatch_rule(rule, {})
    assert result["status"] == "error"
    assert "no handler registered" in result["note"].lower()


def test_dispatch_handler_exception_caught(checker, monkeypatch):
    def boom(rule, opts):
        raise ValueError("synthetic handler failure")

    monkeypatch.setitem(
        checker.HANDLERS,
        "python_ini_racecontrol_section",
        boom,
    )
    rule = {
        "id": "R1",
        "name": "python_ini_racecontrol_section",
        "status": "implemented",
    }
    result = checker.dispatch_rule(rule, {})
    assert result["status"] == "error"
    assert "ValueError" in result["note"]
    assert "synthetic handler failure" in result["note"]


def test_dispatch_stubbed_routes_to_stub(checker):
    rule = {
        "id": "R2",
        "name": "mesh_service_key_pod_server_match",
        "status": "stubbed",
        "todo": "ops auth",
    }
    result = checker.dispatch_rule(rule, {})
    assert result["status"] == "stubbed"


# ----- End-to-end driver -------------------------------------------------


def test_data_content_check_end_to_end_default(checker, rules_path):
    agg = checker.data_content_check(
        rules_path=rules_path,
        options={},  # no python_ini -> R1 stubbed
        only=None,
    )
    assert agg["n_rules"] == 4
    assert agg["n_violation"] == 0
    assert agg["n_error"] == 0
    assert agg["n_stubbed"] == 4  # R1 stubbed-no-path + R2/R3/R4 explicitly stubbed
    assert agg["n_ok"] == 0
    assert checker.exit_code_for(agg) == 0
    assert len(agg["results"]) == 4
    assert {r["rule_id"] for r in agg["results"]} == {"R1", "R2", "R3", "R4"}


def test_data_content_check_with_valid_python_ini(checker, rules_path, fixtures_dir):
    agg = checker.data_content_check(
        rules_path=rules_path,
        options={"python_ini": str(fixtures_dir / "python_ini_valid.ini")},
        only="R1",
    )
    assert agg["n_rules"] == 1
    assert agg["n_ok"] == 1
    assert agg["n_violation"] == 0
    assert checker.exit_code_for(agg) == 0


def test_data_content_check_with_violating_python_ini(checker, rules_path, fixtures_dir):
    agg = checker.data_content_check(
        rules_path=rules_path,
        options={"python_ini": str(fixtures_dir / "python_ini_missing_section.ini")},
        only="R1",
    )
    assert agg["n_rules"] == 1
    assert agg["n_violation"] == 1
    assert checker.exit_code_for(agg) == 1


def test_data_content_check_only_no_match_raises(checker, rules_path):
    with pytest.raises(RuntimeError, match="matched zero rules"):
        checker.data_content_check(
            rules_path=rules_path,
            options={},
            only="R_DOES_NOT_EXIST",
        )


# ----- Exit-code mapping -------------------------------------------------


def test_exit_code_for_all_ok(checker):
    agg = {"n_ok": 3, "n_violation": 0, "n_stubbed": 0, "n_error": 0}
    assert checker.exit_code_for(agg) == 0


def test_exit_code_for_violation(checker):
    agg = {"n_ok": 2, "n_violation": 1, "n_stubbed": 0, "n_error": 0}
    assert checker.exit_code_for(agg) == 1


def test_exit_code_for_error_trumps_violation(checker):
    agg = {"n_ok": 0, "n_violation": 1, "n_stubbed": 0, "n_error": 1}
    assert checker.exit_code_for(agg) == 2


def test_exit_code_for_all_stubbed_is_ok(checker):
    agg = {"n_ok": 0, "n_violation": 0, "n_stubbed": 4, "n_error": 0}
    assert checker.exit_code_for(agg) == 0
