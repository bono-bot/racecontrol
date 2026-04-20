#!/usr/bin/env python3
"""
DPDP §12 right-of-erasure coverage checker.

Compares the FK graph in .planning/board-state.json against
ERASE_TABLES + POINTER_TABLES + TRANSITIVE_ERASE_SQL declared in
crates/racecontrol/src/api/customer_legal.rs.

Fails (exit 1) if any of the following is true:
  - A table with a driver_id FK edge is NEITHER in ERASE_TABLES nor in POINTER_TABLES.
  - A table with an FK to billing_sessions/laps (transitive) is not covered
    by ERASE_TABLES *before* its parent, or by TRANSITIVE_ERASE_SQL.
  - ERASE_TABLES orders a child AFTER its parent (FK constraint would block).

This is the Phase-3-candidate middlegame enforcement: if this script had run
as a pre-commit hook against 4ab2e835^ (the pre-rewrite 22-table version),
it would have flagged the 18 missing tables + 2 missing pointer entries +
1 missing transitive clear that the coverage-matrix audit later surfaced.

Usage:
    python scripts/audit/dpdp-coverage-check.py
        -> exit 0 (green) or 1 (red) with a matrix printed to stdout

Prerequisite: run scripts/audit/board-state-generate.py first.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
BOARD_PATH = REPO_ROOT / ".planning" / "board-state.json"
LEGAL_PATH = (
    REPO_ROOT
    / "crates"
    / "racecontrol"
    / "src"
    / "api"
    / "customer_legal.rs"
)


# --- customer_legal.rs const extraction -------------------------------------
# Parse the three declarative constants out of the source. We extract the
# const by its name, then walk the body and pick up `("table", &["col", ...])`
# or `("table", "sql...")` tuples.

CONST_BLOCK_RE = re.compile(
    r"pub\(crate\) const\s+(\w+)\s*:\s*[^=]+=\s*&\[(.*?)\];",
    re.DOTALL,
)
# Matches: ("table_name", &["col1", "col2", ...])
ERASE_ENTRY_RE = re.compile(
    r'''\(\s*"(?P<table>\w+)"\s*,\s*&\[(?P<cols>[^\]]*)\]\s*\)''',
    re.DOTALL,
)
# Matches: ("table_name", "column_name")  -- POINTER_TABLES shape
POINTER_ENTRY_RE = re.compile(
    r'''\(\s*"(?P<table>\w+)"\s*,\s*"(?P<col>\w+)"\s*\)''',
)
# Matches individual "col" string literals inside the &[...] body
COL_STR_RE = re.compile(r'"(\w+)"')


def extract_const(source: str, name: str) -> str | None:
    for m in CONST_BLOCK_RE.finditer(source):
        if m.group(1) == name:
            return m.group(2)
    return None


def parse_erase_tables(body: str) -> list[tuple[str, list[str]]]:
    out: list[tuple[str, list[str]]] = []
    for m in ERASE_ENTRY_RE.finditer(body):
        table = m.group("table")
        cols = COL_STR_RE.findall(m.group("cols"))
        out.append((table, cols))
    return out


def parse_pointer_tables(body: str) -> list[tuple[str, str]]:
    return [(m.group("table"), m.group("col")) for m in POINTER_ENTRY_RE.finditer(body)]


def parse_transitive_erase(body: str) -> list[tuple[str, str]]:
    """Pull (table_name, sql_blob) pairs. SQL may span multiple lines."""
    # Shape:  ( "table", "DELETE FROM ... WHERE ... IN (SELECT ...)" ),
    # The SQL literal can be split across lines with escaped quotes, so we
    # grab everything between the first two string literals of each tuple.
    out: list[tuple[str, str]] = []
    # Find all ("...", "...") tuples — lenient.
    tuple_re = re.compile(
        r'''\(\s*"(?P<table>\w+)"\s*,\s*"(?P<sql>.+?)"\s*,?\s*\)''',
        re.DOTALL,
    )
    for m in tuple_re.finditer(body):
        out.append((m.group("table"), m.group("sql")))
    return out


# --- Coverage logic ---------------------------------------------------------


def main() -> int:
    if not BOARD_PATH.is_file():
        print(
            f"ERROR: {BOARD_PATH.relative_to(REPO_ROOT)} not found. "
            "Run scripts/audit/board-state-generate.py first.",
            file=sys.stderr,
        )
        return 2
    board = json.loads(BOARD_PATH.read_text(encoding="utf-8"))
    legal_src = LEGAL_PATH.read_text(encoding="utf-8")

    erase_body = extract_const(legal_src, "ERASE_TABLES")
    pointer_body = extract_const(legal_src, "POINTER_TABLES")
    transitive_body = extract_const(legal_src, "TRANSITIVE_ERASE_SQL")
    missing_consts = [
        name
        for name, body in [
            ("ERASE_TABLES", erase_body),
            ("POINTER_TABLES", pointer_body),
            ("TRANSITIVE_ERASE_SQL", transitive_body),
        ]
        if body is None
    ]
    if missing_consts:
        print(
            "ERROR: could not find const(s) in customer_legal.rs: "
            + ", ".join(missing_consts),
            file=sys.stderr,
        )
        return 2

    erase = parse_erase_tables(erase_body)
    pointer = parse_pointer_tables(pointer_body)
    transitive = parse_transitive_erase(transitive_body)

    erase_tables = {t for t, _ in erase}
    erase_order = {t: i for i, (t, _) in enumerate(erase)}
    pointer_tables = {t for t, _ in pointer}
    pointer_cols = {(t, c) for t, c in pointer}
    transitive_tables = {t for t, _ in transitive}

    # Index edges from the board by destination table.
    driver_fks = [e for e in board["fk_edges"] if e["dst_table"] == "drivers"]
    billing_fks = [
        e for e in board["fk_edges"] if e["dst_table"] == "billing_sessions"
    ]
    laps_fks = [e for e in board["fk_edges"] if e["dst_table"] == "laps"]

    gaps: list[str] = []
    rows: list[tuple[str, str, str, str, str, str]] = []

    # --- driver FK coverage ------------------------------------------------
    for e in driver_fks:
        src_t = e["src_table"]
        src_c = e["src_column"]
        enforced = "yes" if e.get("enforced", True) else "NO"
        in_erase = src_t in erase_tables
        in_pointer = (src_t, src_c) in pointer_cols
        status = "OK" if (in_erase or in_pointer) else "GAP"
        rows.append(
            (
                f"{src_t}.{src_c}",
                "drivers(id)",
                "y" if in_erase else "-",
                "y" if in_pointer else "-",
                enforced,
                status,
            )
        )
        if status == "GAP":
            gaps.append(
                f"driver FK {src_t}.{src_c} -> drivers(id) not in ERASE_TABLES or POINTER_TABLES"
            )

    # --- billing_sessions FK coverage (transitive) -------------------------
    for e in billing_fks:
        src_t = e["src_table"]
        src_c = e["src_column"]
        enforced = "yes" if e.get("enforced", True) else "NO"
        in_erase = src_t in erase_tables
        in_transitive = src_t in transitive_tables
        # Ordering check: src must be deleted BEFORE billing_sessions.
        order_ok = True
        if in_erase and "billing_sessions" in erase_order:
            order_ok = erase_order[src_t] < erase_order["billing_sessions"]
        status = "OK"
        if not in_erase and not in_transitive:
            status = "GAP"
            gaps.append(
                f"billing_sessions FK {src_t}.{src_c} not in ERASE_TABLES or TRANSITIVE_ERASE_SQL"
            )
        elif in_erase and not order_ok:
            status = "ORDER"
            gaps.append(
                f"ERASE_TABLES orders {src_t} AFTER billing_sessions; "
                f"FK will block parent DELETE"
            )
        rows.append(
            (
                f"{src_t}.{src_c}",
                "billing_sessions(id)",
                "y" if in_erase else "-",
                "T" if in_transitive else "-",
                enforced,
                status,
            )
        )

    # --- laps FK coverage (transitive) ------------------------------------
    for e in laps_fks:
        src_t = e["src_table"]
        src_c = e["src_column"]
        enforced = "yes" if e.get("enforced", True) else "NO"
        in_erase = src_t in erase_tables
        in_transitive = src_t in transitive_tables
        order_ok = True
        if in_erase and "laps" in erase_order:
            order_ok = erase_order[src_t] < erase_order["laps"]
        status = "OK"
        if not in_erase and not in_transitive:
            status = "GAP"
            gaps.append(
                f"laps FK {src_t}.{src_c} not in ERASE_TABLES or TRANSITIVE_ERASE_SQL"
            )
        elif in_erase and not order_ok:
            status = "ORDER"
            gaps.append(
                f"ERASE_TABLES orders {src_t} AFTER laps; FK will block parent DELETE"
            )
        rows.append(
            (
                f"{src_t}.{src_c}",
                "laps(id)",
                "y" if in_erase else "-",
                "T" if in_transitive else "-",
                enforced,
                status,
            )
        )

    # --- Print matrix -----------------------------------------------------
    width_src = max(10, max(len(r[0]) for r in rows))
    width_dst = max(12, max(len(r[1]) for r in rows))
    hdr = (
        f"{'SRC':<{width_src}}  "
        f"{'DST':<{width_dst}}  "
        f"ERASE  PTR/T  FK-ENFORCED  STATUS"
    )
    print(hdr)
    print("-" * len(hdr))
    for src, dst, erase_flag, ptr_flag, enf, status in rows:
        print(
            f"{src:<{width_src}}  "
            f"{dst:<{width_dst}}  "
            f"{erase_flag:^5}  {ptr_flag:^5}  {enf:^11}  {status}"
        )

    # --- Summary ----------------------------------------------------------
    print()
    print(
        f"edges: {len(driver_fks)} driver, "
        f"{len(billing_fks)} billing_sessions, "
        f"{len(laps_fks)} laps"
    )
    print(
        f"ERASE_TABLES: {len(erase_tables)} tables | "
        f"POINTER_TABLES: {len(pointer_tables)} cols | "
        f"TRANSITIVE_ERASE_SQL: {len(transitive_tables)} tables"
    )
    if gaps:
        print()
        print(f"GAPS ({len(gaps)}):")
        for g in gaps:
            print(f"  - {g}")
        return 1
    print("coverage: complete (no gaps)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
