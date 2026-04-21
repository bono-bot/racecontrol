#!/usr/bin/env python3
"""
DPDP §12 right-of-erasure — Source-of-truth parser.

Reads crates/racecontrol/src/api/customer_legal.rs and emits the authoritative
lists as machine-readable output for consumption by E2E tests and audit
scripts. ONE PARSER, ONE SOT — eliminates the cleanup-test-drivers.mjs drift
(which hardcoded 21 tables while customer_legal.rs has 42).

Sister to scripts/audit/dpdp-coverage-check.py (structural check) — this
parser is the data feed for the Day 4 runtime contract.

Usage:
    python3 tests/e2e/api/dpdp_erase_list.py --erase
        one line per table: "table_name col1 col2 ..."
    python3 tests/e2e/api/dpdp_erase_list.py --pointer
        one line per entry: "table_name column_name"
    python3 tests/e2e/api/dpdp_erase_list.py --transitive
        one line per entry: "table_name"
    python3 tests/e2e/api/dpdp_erase_list.py --json
        single JSON doc: {"erase":[...], "pointer":[...], "transitive":[...]}

Read-only over source; safe to run anywhere.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[3]
LEGAL_PATH = (
    REPO_ROOT / "crates" / "racecontrol" / "src" / "api" / "customer_legal.rs"
)


def parse_erase_tables(src: str) -> list[dict]:
    """Parse ERASE_TABLES = &[("table", &["col1", "col2"]), ...]."""
    # Type annotation `&[(&str, &[&str])]` contains nested brackets —
    # use non-greedy match from `ERASE_TABLES` to the opening `= &[`,
    # then capture body until `\n];`.
    block_re = re.compile(
        r"ERASE_TABLES.*?=\s*&\[(.*?)\n\]\s*;",
        re.DOTALL,
    )
    m = block_re.search(src)
    if not m:
        raise ValueError("ERASE_TABLES block not found")
    body = m.group(1)

    row_re = re.compile(
        r'\(\s*"([^"]+)"\s*,\s*&\[\s*((?:"[^"]+"\s*,?\s*)+)\s*\]\s*\)',
        re.DOTALL,
    )
    col_re = re.compile(r'"([^"]+)"')

    rows = []
    for row_m in row_re.finditer(body):
        table = row_m.group(1)
        cols = col_re.findall(row_m.group(2))
        rows.append({"table": table, "fk_columns": cols})
    return rows


def parse_pointer_tables(src: str) -> list[dict]:
    """Parse POINTER_TABLES = &[("table", "column"), ...]."""
    block_re = re.compile(
        r"POINTER_TABLES.*?=\s*&\[(.*?)\n\]\s*;",
        re.DOTALL,
    )
    m = block_re.search(src)
    if not m:
        raise ValueError("POINTER_TABLES block not found")
    body = m.group(1)

    row_re = re.compile(r'\(\s*"([^"]+)"\s*,\s*"([^"]+)"\s*\)')
    return [
        {"table": row_m.group(1), "column": row_m.group(2)}
        for row_m in row_re.finditer(body)
    ]


def parse_transitive_erase(src: str) -> list[dict]:
    """Parse TRANSITIVE_ERASE_SQL = &[("table", "DELETE ..."), ...]."""
    block_re = re.compile(
        r"TRANSITIVE_ERASE_SQL.*?=\s*&\[(.*?)\n\]\s*;",
        re.DOTALL,
    )
    m = block_re.search(src)
    if not m:
        raise ValueError("TRANSITIVE_ERASE_SQL block not found")
    body = m.group(1)

    # Each row is (table_name, "DELETE FROM ... WHERE col IN (SELECT ...)").
    # We extract only the table name; the SQL is in the Rust handler, not
    # reproduced here (the runtime test verifies row-count = 0, not the SQL shape).
    row_re = re.compile(r'\(\s*"([^"]+)"\s*,')
    return [{"table": row_m.group(1)} for row_m in row_re.finditer(body)]


def main() -> int:
    parser = argparse.ArgumentParser(description="DPDP erase-list parser")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--erase", action="store_true", help="emit ERASE_TABLES")
    mode.add_argument("--pointer", action="store_true", help="emit POINTER_TABLES")
    mode.add_argument("--transitive", action="store_true", help="emit TRANSITIVE_ERASE_SQL")
    mode.add_argument("--json", action="store_true", help="emit all three as JSON")
    args = parser.parse_args()

    if not LEGAL_PATH.exists():
        sys.stderr.write(f"ERROR: {LEGAL_PATH} not found\n")
        return 2

    src = LEGAL_PATH.read_text(encoding="utf-8")

    try:
        erase = parse_erase_tables(src)
        pointer = parse_pointer_tables(src)
        transitive = parse_transitive_erase(src)
    except ValueError as e:
        sys.stderr.write(f"ERROR: {e}\n")
        return 2

    if args.erase:
        for row in erase:
            print(row["table"], *row["fk_columns"])
    elif args.pointer:
        for row in pointer:
            print(row["table"], row["column"])
    elif args.transitive:
        for row in transitive:
            print(row["table"])
    elif args.json:
        print(json.dumps({
            "erase": erase,
            "pointer": pointer,
            "transitive": transitive,
        }, indent=2))

    return 0


if __name__ == "__main__":
    sys.exit(main())
