#!/usr/bin/env python3
"""
Data-content validator — F4 functional-layer checker.

Structural checkers (D1-D5) verify topology: does the file exist, does the
route register, does the schema compile. F4 complements by validating the
*content* of runtime files and DB invariants — the class of drift where
"file exists" is green but the contents are wrong.

Canonical motivating incident: Zero Laps P0 (2026-04-16). Every pod had
`python.ini` on disk (structural check: pass) but 7/8 were missing the
`[RACECONTROL]` section, so the AC telemetry plugin never loaded and zero
laps were ever recorded for weeks.

F4 rules live in `.planning/functional-layer/data-content-rules.yaml`:
  R1 python_ini_racecontrol_section    — implemented (fixture-bound tonight)
  R2 mesh_service_key_pod_server_match — stubbed (needs live pod + server)
  R3 billing_session_total_cost_...    — stubbed (needs DB access)
  R4 kiosk_settings_...cache_match     — stubbed (needs DB + kiosk access)
  R5 SWAPLOG binary hash parity        — NOT listed; owned by
                                         fleet-swaplog-parity-check.py

Exit codes:
  0 — no violations (including all-stubbed-runs: a stubbed rule never fires
      a violation, only a deferred-work marker).
  1 — at least one implemented rule returned one or more violations.
  2 — checker internal error (rules file missing/malformed, fatal handler
      exception, etc.). Distinct from "violation" to avoid masking tool
      bugs as content drift.

Designed for:
  - run-all-checkers.sh orchestrator (same shape as fleet-swaplog-parity)
  - Future: pm2 workflow-monitor / pre-commit cascade once R2-R4 wired

Dependencies:
  - Python 3 stdlib (pathlib, json, argparse, configparser, sys)
  - PyYAML for rules file parsing (install: `pip install pyyaml`). If PyYAML
    is unavailable, the checker exits 2 with a clear message rather than
    silently skipping — a missing rules loader is a structural error, not a
    no-op.
"""

from __future__ import annotations

import argparse
import configparser
import json
import sys
import traceback
from pathlib import Path
from typing import Any, Callable, Optional


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_RULES_PATH = REPO_ROOT / ".planning" / "functional-layer" / "data-content-rules.yaml"
DEFAULT_OUTPUT_PATH = REPO_ROOT / ".planning" / "data-content-violations.json"


# ---------------------------------------------------------------------------
# Rule handlers
#
# Each handler receives (rule: dict, options: dict) and returns a dict shaped
#   {
#     "rule_id": "R1",
#     "name": "...",
#     "status": "ok" | "violation" | "stubbed" | "error",
#     "violations": [ {target, expected, actual, message}, ... ],
#     "note": str (optional — stubbed rules use this for the todo line),
#   }
# ---------------------------------------------------------------------------


def handle_python_ini_racecontrol_section(rule: dict, options: dict) -> dict:
    """R1 — verify [RACECONTROL] section + required keys in a python.ini file.

    Expects:
      rule["required_section"]  e.g. "[RACECONTROL]" (with or without brackets)
      rule["required_keys"]     list of case-insensitive key names

    Options:
      options["python_ini"]  absolute path to a python.ini to check.
                             If not supplied, the rule is marked stubbed
                             with a "needs --python-ini <path>" todo.

    Returns violation when:
      (a) file does not exist at the supplied path
      (b) required section is absent
      (c) required section is present but one or more required keys are absent
    """
    python_ini_path = options.get("python_ini")
    section_raw = rule.get("required_section", "[RACECONTROL]")
    section_name = section_raw.strip().strip("[]")
    required_keys = [k.lower() for k in rule.get("required_keys", []) or []]

    if not python_ini_path:
        return {
            "rule_id": rule["id"],
            "name": rule["name"],
            "status": "stubbed",
            "violations": [],
            "note": (
                "R1 live probe skipped — pass --python-ini <path> to check "
                "a specific pod's copy. Unit tests exercise the handler "
                "against fixtures. Live pod SSH pass is a future-session op."
            ),
        }

    path = Path(python_ini_path)
    if not path.exists():
        return {
            "rule_id": rule["id"],
            "name": rule["name"],
            "status": "violation",
            "violations": [
                {
                    "target": str(path),
                    "expected": f"file present with [{section_name}] section",
                    "actual": "file not found",
                    "message": f"python.ini not found at {path}",
                }
            ],
        }

    # python.ini is an INI-like file but Assetto Corsa writes CRLF + sometimes
    # duplicate-section-name files. configparser handles both if we set
    # strict=False. For section existence checks we also fall back to a
    # literal substring search because AC's writer occasionally omits the
    # trailing newline, which some parsers tolerate and some don't.
    try:
        raw = path.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        return {
            "rule_id": rule["id"],
            "name": rule["name"],
            "status": "violation",
            "violations": [
                {
                    "target": str(path),
                    "expected": "readable file",
                    "actual": f"OSError: {e}",
                    "message": f"could not read {path}: {e}",
                }
            ],
        }

    section_header = f"[{section_name}]"
    if section_header not in raw:
        return {
            "rule_id": rule["id"],
            "name": rule["name"],
            "status": "violation",
            "violations": [
                {
                    "target": str(path),
                    "expected": f"{section_header} section present",
                    "actual": "section missing",
                    "message": (
                        f"python.ini at {path} has no {section_header} "
                        f"section. This is the Zero Laps P0 class — AC will "
                        f"not load the telemetry plugin."
                    ),
                }
            ],
        }

    # Section present — now check keys. Parse with configparser strict=False
    # to tolerate AC quirks.
    cp = configparser.ConfigParser(strict=False, interpolation=None)
    cp.optionxform = str  # preserve case for diagnostic; we compare lowercase below
    try:
        cp.read_string(raw)
    except configparser.Error:
        # Fall back to manual parse — find the section, extract key=value
        # lines until the next section header or EOF.
        section_keys_present: set[str] = set()
        in_section = False
        for line in raw.splitlines():
            stripped = line.strip()
            if stripped.startswith("[") and stripped.endswith("]"):
                in_section = (stripped == section_header)
                continue
            if in_section and "=" in stripped and not stripped.startswith((";", "#")):
                key = stripped.split("=", 1)[0].strip().lower()
                if key:
                    section_keys_present.add(key)
    else:
        section_keys_present = set()
        # configparser lowercases its section names by default — but we set
        # optionxform=str above, so the section name is preserved as-is.
        # Look up the section case-insensitively.
        for name in cp.sections():
            if name.strip().lower() == section_name.lower():
                section_keys_present = {k.lower() for k in cp[name].keys()}
                break

    missing = [k for k in required_keys if k not in section_keys_present]
    if missing:
        return {
            "rule_id": rule["id"],
            "name": rule["name"],
            "status": "violation",
            "violations": [
                {
                    "target": str(path),
                    "expected": f"keys {required_keys} under {section_header}",
                    "actual": f"present: {sorted(section_keys_present)}",
                    "message": (
                        f"python.ini at {path} has {section_header} but is "
                        f"missing required key(s): {missing}"
                    ),
                }
            ],
        }

    return {
        "rule_id": rule["id"],
        "name": rule["name"],
        "status": "ok",
        "violations": [],
        "note": (
            f"{section_header} present with required keys "
            f"{required_keys} at {path}"
        ),
    }


def handle_stubbed(rule: dict, options: dict) -> dict:
    """Default handler for rules with status=stubbed in the YAML.

    Returns a structured stubbed result so the framework runs end-to-end
    without exception. `todo` is surfaced so pm2 / CI / humans see exactly
    what is deferred.
    """
    return {
        "rule_id": rule["id"],
        "name": rule["name"],
        "status": "stubbed",
        "violations": [],
        "note": rule.get("todo", "no todo recorded; see YAML"),
    }


# Dispatch map: rule name -> handler. Only rules marked status=implemented in
# the YAML route to a real handler; status=stubbed routes to handle_stubbed.
HANDLERS: dict[str, Callable[[dict, dict], dict]] = {
    "python_ini_racecontrol_section": handle_python_ini_racecontrol_section,
    # R2-R4 are stubbed in the YAML; they do not need a custom handler here.
}


# ---------------------------------------------------------------------------
# Core driver
# ---------------------------------------------------------------------------


def load_rules(path: Path) -> list[dict]:
    """Load + validate the rules YAML. Raises on structural breakage."""
    try:
        import yaml  # type: ignore[import-untyped]
    except ImportError as e:
        raise RuntimeError(
            "PyYAML not available — install with `pip install pyyaml`. "
            "F4 intentionally uses YAML for the rule pack (human-editable); "
            "fallback-to-JSON would hide the install gap."
        ) from e

    if not path.exists():
        raise RuntimeError(f"rules file not found: {path}")

    try:
        doc = yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as e:
        raise RuntimeError(f"rules file {path} is not valid YAML: {e}") from e

    if not isinstance(doc, dict) or "rules" not in doc:
        raise RuntimeError(
            f"rules file {path} missing top-level 'rules' list"
        )
    rules = doc["rules"]
    if not isinstance(rules, list):
        raise RuntimeError(
            f"rules file {path}: 'rules' must be a list, got {type(rules)}"
        )
    # Skip rules marked status=skipped (e.g. R5 is not listed, but an
    # operator might mark one skipped without removing it).
    return [r for r in rules if r.get("status") != "skipped"]


def dispatch_rule(rule: dict, options: dict) -> dict:
    """Route a rule to its handler, catching handler exceptions."""
    status = rule.get("status", "stubbed")
    name = rule.get("name")
    if status == "stubbed":
        return handle_stubbed(rule, options)
    handler = HANDLERS.get(name)
    if handler is None:
        return {
            "rule_id": rule.get("id", "?"),
            "name": name,
            "status": "error",
            "violations": [],
            "note": (
                f"rule {name!r} marked status={status!r} but no handler "
                f"registered in HANDLERS. Add one or mark it stubbed."
            ),
        }
    try:
        return handler(rule, options)
    except Exception as e:
        return {
            "rule_id": rule.get("id", "?"),
            "name": name,
            "status": "error",
            "violations": [],
            "note": (
                f"handler for {name!r} raised {type(e).__name__}: {e}\n"
                f"{traceback.format_exc()}"
            ),
        }


def data_content_check(
    rules_path: Path = DEFAULT_RULES_PATH,
    options: Optional[dict] = None,
    only: Optional[str] = None,
) -> dict:
    """Run the full checker. Returns an aggregated result dict.

    The returned dict is:
      {
        "rules_file": <str path>,
        "n_rules": int,
        "n_ok": int,
        "n_violation": int,
        "n_stubbed": int,
        "n_error": int,
        "results": [ <per-rule result>, ... ],
      }
    """
    options = options or {}
    rules = load_rules(rules_path)
    if only:
        rules = [r for r in rules if r.get("id") == only or r.get("name") == only]
        if not rules:
            raise RuntimeError(
                f"--only {only!r} matched zero rules in {rules_path}"
            )

    results = [dispatch_rule(r, options) for r in rules]
    agg = {
        "rules_file": str(rules_path),
        "n_rules": len(results),
        "n_ok": sum(1 for r in results if r["status"] == "ok"),
        "n_violation": sum(1 for r in results if r["status"] == "violation"),
        "n_stubbed": sum(1 for r in results if r["status"] == "stubbed"),
        "n_error": sum(1 for r in results if r["status"] == "error"),
        "results": results,
    }
    return agg


def write_output(agg: dict, output_path: Path) -> None:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(agg, indent=2) + "\n", encoding="utf-8")


def exit_code_for(agg: dict) -> int:
    if agg["n_error"] > 0:
        return 2
    if agg["n_violation"] > 0:
        return 1
    return 0


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        prog="data-content-check",
        description=(
            "F4 data-content validator. Runs rules from "
            ".planning/functional-layer/data-content-rules.yaml, writes "
            "aggregated violations to .planning/data-content-violations.json, "
            "exits 0 (clean) / 1 (violation) / 2 (checker error)."
        ),
    )
    p.add_argument(
        "--rules",
        type=Path,
        default=DEFAULT_RULES_PATH,
        help=f"Path to rule pack YAML (default: {DEFAULT_RULES_PATH})",
    )
    p.add_argument(
        "--output",
        type=Path,
        default=DEFAULT_OUTPUT_PATH,
        help=f"Path to write aggregated violations JSON (default: {DEFAULT_OUTPUT_PATH})",
    )
    p.add_argument(
        "--only",
        default=None,
        help="Run only one rule by id (e.g. R1) or name (e.g. python_ini_racecontrol_section)",
    )
    p.add_argument(
        "--python-ini",
        type=Path,
        default=None,
        help=(
            "Path to a python.ini to validate for R1. When omitted, R1 "
            "returns stubbed ('needs --python-ini <path>')."
        ),
    )
    return p.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    args = parse_args(argv)
    options = {
        "python_ini": str(args.python_ini) if args.python_ini else None,
    }
    try:
        agg = data_content_check(
            rules_path=args.rules,
            options=options,
            only=args.only,
        )
    except RuntimeError as e:
        print(f"data-content-check: ERROR — {e}", file=sys.stderr)
        return 2

    try:
        write_output(agg, args.output)
    except OSError as e:
        print(
            f"data-content-check: failed to write output {args.output}: {e}",
            file=sys.stderr,
        )
        # Still print summary + return exit code so orchestrator captures
        # something meaningful. Output write failure is not a content
        # violation but is a checker error.
        print_summary(agg)
        return 2

    print_summary(agg)
    return exit_code_for(agg)


def print_summary(agg: dict) -> None:
    if agg["n_violation"] == 0 and agg["n_error"] == 0:
        tag = "green"
    elif agg["n_violation"] > 0:
        tag = "RED"
    else:
        tag = "ERROR"
    print(
        f"data-content: {tag} "
        f"n_rules={agg['n_rules']} ok={agg['n_ok']} "
        f"stubbed={agg['n_stubbed']} violation={agg['n_violation']} "
        f"error={agg['n_error']}"
    )
    for r in agg["results"]:
        status = r["status"]
        name = r["name"]
        if status == "ok":
            print(f"  - {r['rule_id']} {name}: ok")
        elif status == "stubbed":
            note = r.get("note", "").splitlines()[0] if r.get("note") else ""
            print(f"  - {r['rule_id']} {name}: stubbed — {note}")
        elif status == "violation":
            print(f"  - {r['rule_id']} {name}: VIOLATION ({len(r['violations'])})")
            for v in r["violations"]:
                print(f"      target={v['target']}")
                print(f"      message={v['message']}")
        elif status == "error":
            note_first = r.get("note", "").splitlines()[0] if r.get("note") else ""
            print(f"  - {r['rule_id']} {name}: ERROR — {note_first}")


if __name__ == "__main__":
    sys.exit(main())
