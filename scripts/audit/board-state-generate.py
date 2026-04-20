#!/usr/bin/env python3
"""
Board-state generator — Phase 1 of the workflow-mapping plan.

Extracts the "position" of the codebase as a machine-readable JSON document:
every table, every column, every FK edge. Consumed by downstream tools
(dpdp-coverage-check.py, cascade-hook.py).

v1 scope: SQLite schema from crates/racecontrol/src/db/migrate_*.rs.
Future scope: serde struct boundaries, WS protocol surface, deploy targets.

Usage:
    python scripts/audit/board-state-generate.py
        -> writes .planning/board-state.json

This is read-only over source; safe to run anywhere.
"""

from __future__ import annotations

import json
import re
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MIGRATE_DIR = REPO_ROOT / "crates" / "racecontrol" / "src" / "db"
OUT_PATH = REPO_ROOT / ".planning" / "board-state.json"


# --- SQL extraction regex ---------------------------------------------------
# We parse Rust string literals that carry CREATE TABLE / ALTER TABLE SQL.
# This is intentionally lenient — the generator flags parse failures but does
# not block. Downstream checkers must tolerate the generator being incomplete.

# Match: CREATE TABLE IF NOT EXISTS foo ( ... )
CREATE_TABLE_RE = re.compile(
    r'"CREATE TABLE(?:\s+IF NOT EXISTS)?\s+(\w+)\s*\((.*?)\)(?:\s*WITHOUT ROWID)?\s*"',
    re.DOTALL | re.IGNORECASE,
)

# Column-level match inside the parenthesised body.
# Captures: col_name, col_type, optional REFERENCES table(col) tail.
COLUMN_RE = re.compile(
    r"""
    (?:^|,)\s*
    (?P<col>\w+)\s+
    (?P<typ>\w+)
    (?P<tail>[^,]*)
    """,
    re.VERBOSE | re.MULTILINE,
)

REFERENCES_RE = re.compile(
    r"REFERENCES\s+(\w+)\s*\(\s*(\w+)\s*\)",
    re.IGNORECASE,
)

ALTER_ADD_COLUMN_RE = re.compile(
    r'"ALTER TABLE\s+(\w+)\s+ADD COLUMN\s+(\w+)\s+(\w+)([^"]*)"',
    re.IGNORECASE,
)


def ist_now() -> str:
    return (datetime.now(timezone.utc) + timedelta(hours=5, minutes=30)).strftime(
        "%Y-%m-%d %H:%M IST"
    )


def parse_migration(path: Path) -> list[dict]:
    """Return a list of schema events (table + column records) extracted from one file."""
    text = path.read_text(encoding="utf-8", errors="replace")
    events: list[dict] = []

    # CREATE TABLE statements
    for m in CREATE_TABLE_RE.finditer(text):
        table = m.group(1)
        body = m.group(2)
        line = text[: m.start()].count("\n") + 1
        columns: list[dict] = []
        # Strip table-level constraints (PRIMARY KEY (...), UNIQUE(...), FOREIGN KEY...)
        # by matching only lines that look like "col_name TYPE ...".
        for col_match in COLUMN_RE.finditer(body):
            col = col_match.group("col")
            typ = col_match.group("typ").upper()
            tail = col_match.group("tail")
            # Filter false positives — table-level constraints start with keywords.
            if col.upper() in {
                "PRIMARY",
                "FOREIGN",
                "UNIQUE",
                "CHECK",
                "CONSTRAINT",
                "INDEX",
            }:
                continue
            # Filter keywords that serve as type hints when real type is absent.
            if typ not in {
                "TEXT",
                "INTEGER",
                "REAL",
                "BLOB",
                "NUMERIC",
                "BOOLEAN",
            }:
                continue
            nullable = "NOT NULL" not in tail.upper()
            pk = "PRIMARY KEY" in tail.upper()
            unique = "UNIQUE" in tail.upper()
            ref_match = REFERENCES_RE.search(tail)
            col_rec: dict = {
                "name": col,
                "type": typ,
                "nullable": nullable,
                "pk": pk,
                "unique": unique,
            }
            if ref_match:
                col_rec["fk"] = {
                    "table": ref_match.group(1),
                    "column": ref_match.group(2),
                }
            columns.append(col_rec)
        events.append(
            {
                "kind": "create_table",
                "table": table,
                "columns": columns,
                "source": f"{path.relative_to(REPO_ROOT).as_posix()}:{line}",
            }
        )

    # ALTER TABLE ADD COLUMN statements (with or without REFERENCES tail)
    for m in ALTER_ADD_COLUMN_RE.finditer(text):
        table = m.group(1)
        col = m.group(2)
        typ = m.group(3).upper()
        tail = m.group(4) or ""
        line = text[: m.start()].count("\n") + 1
        ref_match = REFERENCES_RE.search(tail)
        col_rec: dict = {
            "name": col,
            "type": typ,
            "nullable": "NOT NULL" not in tail.upper(),
            "pk": False,
            "unique": "UNIQUE" in tail.upper(),
        }
        if ref_match:
            col_rec["fk"] = {
                "table": ref_match.group(1),
                "column": ref_match.group(2),
            }
            # SQLite caveat: ALTER TABLE ADD COLUMN ... REFERENCES does NOT
            # enforce the FK at runtime. Flag it so downstream tools can warn.
            col_rec["fk_enforced"] = False
        events.append(
            {
                "kind": "alter_add_column",
                "table": table,
                "column": col_rec,
                "source": f"{path.relative_to(REPO_ROOT).as_posix()}:{line}",
            }
        )

    return events


def build_board(events: list[dict]) -> dict:
    """Fold schema events into a single board-state document."""
    tables: dict[str, dict] = {}
    fk_edges: list[dict] = []

    for ev in events:
        if ev["kind"] == "create_table":
            t = ev["table"]
            tables.setdefault(
                t,
                {"name": t, "columns": [], "sources": []},
            )
            tables[t]["sources"].append(ev["source"])
            for col in ev["columns"]:
                # Avoid duplicate columns (rare; same migration rerun).
                if not any(c["name"] == col["name"] for c in tables[t]["columns"]):
                    tables[t]["columns"].append(col)
                if "fk" in col:
                    fk_edges.append(
                        {
                            "src_table": t,
                            "src_column": col["name"],
                            "dst_table": col["fk"]["table"],
                            "dst_column": col["fk"]["column"],
                            "source": ev["source"],
                            "enforced": col.get("fk_enforced", True),
                        }
                    )
        elif ev["kind"] == "alter_add_column":
            t = ev["table"]
            col = ev["column"]
            tables.setdefault(t, {"name": t, "columns": [], "sources": []})
            tables[t]["sources"].append(ev["source"])
            if not any(c["name"] == col["name"] for c in tables[t]["columns"]):
                tables[t]["columns"].append(col)
            if "fk" in col:
                fk_edges.append(
                    {
                        "src_table": t,
                        "src_column": col["name"],
                        "dst_table": col["fk"]["table"],
                        "dst_column": col["fk"]["column"],
                        "source": ev["source"],
                        "enforced": col.get("fk_enforced", True),
                    }
                )

    return {
        "generated_at": ist_now(),
        "generator_version": 1,
        "source_root": "crates/racecontrol/src/db",
        "tables": sorted(tables.values(), key=lambda t: t["name"]),
        "fk_edges": sorted(
            fk_edges,
            key=lambda e: (e["src_table"], e["src_column"], e["dst_table"]),
        ),
        "counts": {
            "tables": len(tables),
            "fk_edges": len(fk_edges),
            "driver_fk_edges": sum(1 for e in fk_edges if e["dst_table"] == "drivers"),
            "laps_fk_edges": sum(1 for e in fk_edges if e["dst_table"] == "laps"),
            "billing_sessions_fk_edges": sum(
                1 for e in fk_edges if e["dst_table"] == "billing_sessions"
            ),
            "unenforced_fk_edges": sum(1 for e in fk_edges if not e.get("enforced", True)),
        },
    }


def main() -> int:
    if not MIGRATE_DIR.is_dir():
        print(f"ERROR: migration dir not found: {MIGRATE_DIR}", file=sys.stderr)
        return 2
    paths = sorted(MIGRATE_DIR.glob("migrate_*.rs"))
    if not paths:
        print(f"ERROR: no migrate_*.rs files under {MIGRATE_DIR}", file=sys.stderr)
        return 2
    all_events: list[dict] = []
    for p in paths:
        all_events.extend(parse_migration(p))
    board = build_board(all_events)
    OUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUT_PATH.write_text(json.dumps(board, indent=2) + "\n", encoding="utf-8")
    print(
        "board-state written: "
        f"{board['counts']['tables']} tables, "
        f"{board['counts']['fk_edges']} FK edges "
        f"({board['counts']['driver_fk_edges']} to drivers, "
        f"{board['counts']['billing_sessions_fk_edges']} to billing_sessions, "
        f"{board['counts']['laps_fk_edges']} to laps, "
        f"{board['counts']['unenforced_fk_edges']} unenforced) "
        f"-> {OUT_PATH.relative_to(REPO_ROOT)}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
