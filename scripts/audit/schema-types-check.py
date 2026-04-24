#!/usr/bin/env python3
"""
schema-types-check — D8 cross-boundary type parity checker.

Validates that every REQUIRED field of a Rust struct (a serde Deserialize
target at a cross-boundary, e.g. rc-agent's HTTP /ac/launch endpoint) is
referenced literally (word-boundary match) in at least one of the
configured TypeScript files (kiosk / web / admin payload builders).

Historical class this catches (configured as pair `ac_launch_params_kiosk`):
  2026-03-26 — kiosk sent `ai_difficulty: "easy"` + `ai_count: 5`; rc-agent's
  AcLaunchParams required `ai_level: u32` + `ai_cars: Vec<AiCarSlot>`. Serde
  tolerated the missing fields as defaults; game launched with AI always at
  Semi-Pro / zero opponents. deny_unknown_fields now catches extras at
  runtime, but no layer catches MISSING required fields at commit time.

Detection strategy (by design, simple + regex):
  - Read rust_file, locate `pub struct <name> {` ... matching `}`.
  - Enumerate `pub <field>: <type>` lines. Field is OPTIONAL if the
    preceding line(s) within the struct have `#[serde(default...)]` OR
    the type is `Option<...>`. Otherwise REQUIRED.
  - Skip `#[allow(dead_code)]` / `#[serde(skip)]` attributes.
  - For each REQUIRED field, grep each ts_file for the literal field name
    with word-boundary. If no TS file has any reference, the field is RED.
  - OPTIONAL fields are reported as "note: optional, not checked" in -v
    verbose mode but never fail the checker.

What this does NOT do (deliberate scope cap for D8 Phase 1):
  - Full AST parse (requires syn / tree-sitter-rust). Regex is sufficient
    for the serde-annotated cross-boundary structs we care about.
  - Type-shape parity (u32 vs string). deny_unknown_fields catches shape
    drift at runtime; D8 adds the missing-required layer on top.
  - Nested struct follow-through. Each struct is checked in isolation; if
    `AcLaunchParams.aids: Option<AcAids>` has its own required fields,
    AcAids needs its own pair entry.
  - TS side "used-but-undeclared" (i.e. kiosk builds a field that doesn't
    exist on the Rust side). Runtime deny_unknown_fields is authoritative
    for that direction.

Exit codes:
  0 — no drift: every required Rust field found in at least one TS file
      (across all configured pairs).
  1 — drift detected: one or more required fields missing in TS.
  2 — checker internal error (config missing, struct not found, parse
      error, rust_file not found, etc.).

Dependencies: Python 3 stdlib. No third-party requirements.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Dict, List, Tuple


# Minimal TOML loader — stdlib tomllib (3.11+) or fallback.
try:
    import tomllib  # type: ignore[import-not-found]
except ModuleNotFoundError:  # pragma: no cover — 3.10-safety
    tomllib = None

# ---------------------------------------------------------------------------


def parse_rust_struct_fields(
    rust_source: str, struct_name: str
) -> Tuple[List[str], List[str]]:
    """Return (required_fields, optional_fields) for the named struct.

    Regex-based. Only looks at `pub <name>:` lines. Field is OPTIONAL if:
      (a) preceding non-blank line within struct body is
          `#[serde(default...)]`, OR
      (b) type expression starts with `Option<`.
    """
    # Locate struct body: `pub struct <name>` optionally with generics/where,
    # then the next `{` and its matching `}`.
    m = re.search(
        r"pub\s+struct\s+" + re.escape(struct_name) + r"\b[^{]*\{",
        rust_source,
    )
    if not m:
        raise ValueError(
            "struct '{}' not found in rust source".format(struct_name)
        )
    start = m.end()
    # Match braces for the struct body (handles nested braces in types like
    # `HashMap<K, V>`; Rust doesn't nest `{}` inside type annotations at the
    # field-declaration level though, so depth++ on `{`, depth-- on `}`).
    depth = 1
    i = start
    while i < len(rust_source) and depth > 0:
        c = rust_source[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                break
        i += 1
    if depth != 0:
        raise ValueError(
            "struct '{}' body unterminated".format(struct_name)
        )
    body = rust_source[start:i]

    # Walk lines; track the most recent attribute block.
    required: List[str] = []
    optional: List[str] = []
    recent_attrs: List[str] = []
    for raw in body.splitlines():
        line = raw.strip()
        if not line or line.startswith("//"):
            # Blank / comment does NOT reset attrs — multi-attr fields
            # commonly have blank lines between attrs in formatted code.
            continue
        if line.startswith("#["):
            recent_attrs.append(line)
            continue
        fm = re.match(r"pub\s+(\w+)\s*:\s*(.+?)\s*,?\s*$", line)
        if not fm:
            # Non-field line inside struct body (rare — e.g. nested items
            # in some macros). Reset attrs so they don't leak to next field.
            recent_attrs = []
            continue
        field_name = fm.group(1)
        field_type = fm.group(2).rstrip(",")
        attrs_joined = " ".join(recent_attrs)
        recent_attrs = []

        # Consider optional if serde-default or Option<...>.
        is_optional = False
        if re.search(r"#\[serde\(.*\bdefault\b.*\)\]", attrs_joined):
            is_optional = True
        if field_type.startswith("Option<"):
            is_optional = True

        # Skip-serialize doesn't count as optional for the cross-boundary
        # sender; it only affects serialization output.

        if is_optional:
            optional.append(field_name)
        else:
            required.append(field_name)
    return required, optional


def ts_has_field_reference(ts_source: str, field_name: str) -> bool:
    """True if `field_name` appears as a word-boundary-matched token in
    the TS source. This tolerates any quoting style (bare property, string
    key, object shorthand) but rejects substring matches (e.g. `ai_count`
    does NOT match `ai_count_range`)."""
    pat = r"\b" + re.escape(field_name) + r"\b"
    return re.search(pat, ts_source) is not None


def check_pair(
    pair: Dict, repo_root: Path, verbose: bool
) -> Tuple[List[str], str, int]:
    """Check one pair. Returns (missing_fields, pair_id, status_code).

    status_code: 0 = green, 1 = red, 2 = error.
    """
    pid = pair.get("id", "<unnamed>")
    rust_file = repo_root / pair["rust_file"]
    struct_name = pair["rust_struct"]
    ts_files = [repo_root / p for p in pair["ts_files"]]
    severity = pair.get("severity", "required")

    if not rust_file.exists():
        return (
            ["ERROR: rust_file not found: {}".format(rust_file)],
            pid,
            2,
        )
    rust_source = rust_file.read_text(encoding="utf-8")
    try:
        required, optional = parse_rust_struct_fields(
            rust_source, struct_name
        )
    except Exception as e:  # noqa: BLE001
        return (
            [
                "ERROR: struct parse failed: {}: {}".format(
                    struct_name, e
                )
            ],
            pid,
            2,
        )

    # Read all TS files into one concatenated blob for grep.
    ts_blob_parts: List[str] = []
    for tp in ts_files:
        if not tp.exists():
            return (
                [
                    "ERROR: ts_file not found: {}".format(tp),
                ],
                pid,
                2,
            )
        ts_blob_parts.append(tp.read_text(encoding="utf-8"))
    ts_blob = "\n".join(ts_blob_parts)

    missing: List[str] = []
    for field in required:
        if not ts_has_field_reference(ts_blob, field):
            missing.append(field)

    if verbose:
        # Printed before RED summary so operators see context.
        print(
            "[{}] struct={} required={} optional={}".format(
                pid, struct_name, len(required), len(optional)
            )
        )
        for field in required:
            present = ts_has_field_reference(ts_blob, field)
            print(
                "  {} {} ({} ts file{})".format(
                    "ok" if present else "MISS",
                    field,
                    len(ts_files),
                    "" if len(ts_files) == 1 else "s",
                )
            )

    if missing and severity == "required":
        return missing, pid, 1
    return [], pid, 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description="D8 cross-boundary types parity checker"
    )
    parser.add_argument(
        "--config",
        default=".planning/functional-layer/schema-types-pairs.toml",
        help="pairs config TOML (repo-root-relative)",
    )
    parser.add_argument(
        "--repo-root",
        default=".",
        help="repo root (defaults to cwd)",
    )
    parser.add_argument(
        "-v",
        "--verbose",
        action="store_true",
        help="print per-field check status",
    )
    args = parser.parse_args()

    if tomllib is None:
        print(
            "schema-types-check: ERROR — Python 3.11+ required (tomllib)"
        )
        return 2

    repo_root = Path(args.repo_root).resolve()
    config_path = repo_root / args.config
    if not config_path.exists():
        print(
            "schema-types-check: ERROR - config file not found: {}".format(
                config_path
            )
        )
        return 2

    with config_path.open("rb") as f:
        config = tomllib.load(f)
    pairs = config.get("pair", [])
    if not pairs:
        print(
            "schema-types-check: ERROR - no [[pair]] entries in config"
        )
        return 2

    overall = 0
    red_lines: List[str] = []
    for pair in pairs:
        missing, pid, status = check_pair(pair, repo_root, args.verbose)
        if status == 2:
            overall = 2
            for m in missing:
                red_lines.append("  - [{}] {}".format(pid, m))
        elif status == 1:
            if overall == 0:
                overall = 1
            for field in missing:
                red_lines.append(
                    "  - [{}] required field missing in ts: {}".format(
                        pid, field
                    )
                )

    if overall == 0:
        print(
            "schema-types-check: no drift - {} pair(s) green".format(
                len(pairs)
            )
        )
        return 0
    if overall == 1:
        print("schema-types-check: DRIFT detected:")
        for line in red_lines:
            print(line)
        return 1
    # overall == 2
    print("schema-types-check: internal error:")
    for line in red_lines:
        print(line)
    return 2


if __name__ == "__main__":
    sys.exit(main())
