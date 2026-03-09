#!/usr/bin/env python3
"""Export existing AI training data from SQLite to Alpaca JSON format."""

import json
import sqlite3
import sys
from pathlib import Path

DB_PATH = Path(r"C:\Users\bono\racingpoint\racecontrol\racecontrol.db")
OUTPUT_DIR = Path(__file__).parent / "data"

def export_training_pairs(db_path: Path, output_path: Path) -> int:
    """Export ai_training_pairs with quality_score > 0."""
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    # Check if table exists
    cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='ai_training_pairs'")
    if not cur.fetchone():
        print("ai_training_pairs table not found, skipping")
        conn.close()
        output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(output_path, "w", encoding="utf-8") as f:
            json.dump([], f)
        return 0

    cur.execute("""
        SELECT query_text, response_text, source, model
        FROM ai_training_pairs
        WHERE quality_score > 0
        ORDER BY use_count DESC, created_at DESC
    """)
    rows = cur.fetchall()
    conn.close()

    pairs = []
    for row in rows:
        pairs.append({
            "instruction": row["query_text"].strip(),
            "input": "",
            "output": row["response_text"].strip(),
            "source": f"training_pair/{row['source']}/{row['model']}",
        })

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(pairs, f, indent=2, ensure_ascii=False)

    print(f"Exported {len(pairs)} training pairs -> {output_path}")
    return len(pairs)


def export_suggestions(db_path: Path, output_path: Path) -> int:
    """Export non-dismissed AI suggestions as training pairs."""
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    # Check if ai_suggestions table exists
    cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='ai_suggestions'")
    if not cur.fetchone():
        print("ai_suggestions table not found, skipping")
        conn.close()
        return 0

    cur.execute("""
        SELECT pod_id, sim_type, error_context, suggestion, model
        FROM ai_suggestions
        WHERE dismissed = 0
        ORDER BY created_at DESC
    """)
    rows = cur.fetchall()
    conn.close()

    pairs = []
    for row in rows:
        instruction = (
            f"A {row['sim_type']} game has encountered an issue on {row['pod_id']}. "
            f"Error context: {row['error_context']}. "
            f"What should I do to fix this?"
        )
        pairs.append({
            "instruction": instruction,
            "input": "",
            "output": row["suggestion"].strip(),
            "source": f"ai_suggestion/{row['model']}",
        })

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(pairs, f, indent=2, ensure_ascii=False)

    print(f"Exported {len(pairs)} AI suggestions -> {output_path}")
    return len(pairs)


if __name__ == "__main__":
    if not DB_PATH.exists():
        print(f"ERROR: Database not found at {DB_PATH}", file=sys.stderr)
        sys.exit(1)

    total = 0
    total += export_training_pairs(DB_PATH, OUTPUT_DIR / "existing_pairs.json")
    total += export_suggestions(DB_PATH, OUTPUT_DIR / "existing_suggestions.json")
    print(f"\nTotal exported: {total} pairs")
