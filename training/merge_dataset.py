#!/usr/bin/env python3
"""Merge all generated training pairs, dedup, shuffle, and split into train/eval."""

import hashlib
import json
import random
from pathlib import Path

DATA_DIR = Path(__file__).parent / "data"
OUTPUT_TRAIN = DATA_DIR / "train.json"
OUTPUT_EVAL = DATA_DIR / "eval.json"

# Files to merge (order doesn't matter, we shuffle later)
INPUT_FILES = [
    "existing_pairs.json",
    "existing_suggestions.json",
    "playbook_pairs.json",
    "ops_pairs.json",
    "crash_pairs.json",
]

EVAL_RATIO = 0.10  # 10% for evaluation


def instruction_hash(instruction: str) -> str:
    """Hash instruction text for deduplication."""
    normalized = instruction.strip().lower()
    return hashlib.sha256(normalized.encode("utf-8")).hexdigest()[:16]


def merge_and_split():
    all_pairs = []
    stats = {}

    for filename in INPUT_FILES:
        filepath = DATA_DIR / filename
        if not filepath.exists():
            print(f"  SKIP (not found): {filename}")
            continue

        with open(filepath, "r", encoding="utf-8") as f:
            pairs = json.load(f)

        stats[filename] = len(pairs)
        all_pairs.extend(pairs)
        print(f"  Loaded {len(pairs):>4} pairs from {filename}")

    print(f"\n  Total before dedup: {len(all_pairs)}")

    # Deduplicate by instruction hash
    seen = set()
    unique_pairs = []
    dupes = 0
    for pair in all_pairs:
        h = instruction_hash(pair["instruction"])
        if h not in seen:
            seen.add(h)
            unique_pairs.append(pair)
        else:
            dupes += 1

    print(f"  Removed {dupes} duplicates")
    print(f"  Total after dedup: {len(unique_pairs)}")

    # Clean up: remove source field (not needed for training), ensure Alpaca format
    clean_pairs = []
    for pair in unique_pairs:
        clean_pairs.append({
            "instruction": pair["instruction"],
            "input": pair.get("input", ""),
            "output": pair["output"],
        })

    # Shuffle deterministically
    random.seed(42)
    random.shuffle(clean_pairs)

    # Split
    eval_count = max(1, int(len(clean_pairs) * EVAL_RATIO))
    eval_pairs = clean_pairs[:eval_count]
    train_pairs = clean_pairs[eval_count:]

    # Save
    DATA_DIR.mkdir(parents=True, exist_ok=True)

    with open(OUTPUT_TRAIN, "w", encoding="utf-8") as f:
        json.dump(train_pairs, f, indent=2, ensure_ascii=False)

    with open(OUTPUT_EVAL, "w", encoding="utf-8") as f:
        json.dump(eval_pairs, f, indent=2, ensure_ascii=False)

    print(f"\n  Train: {len(train_pairs)} pairs -> {OUTPUT_TRAIN}")
    print(f"  Eval:  {len(eval_pairs)} pairs -> {OUTPUT_EVAL}")
    print(f"\n  Source breakdown:")
    for filename, count in stats.items():
        print(f"    {filename}: {count}")


if __name__ == "__main__":
    print("Merging training datasets...\n")
    merge_and_split()
    print("\nDone!")
