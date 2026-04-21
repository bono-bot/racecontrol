"""Smoke tests for instant-arithmetic-check.py.

Run with:
    python3 -m unittest scripts/audit/test_instant_arithmetic_check.py

Covers:
  * scan_hits finds `Instant::now() - Duration::` patterns
  * Inline `// INSTANT_SUB_SAFE: ...` on the same line suppresses
  * Inline `// INSTANT_SUB_SAFE: ...` on the previous line suppresses
  * `//` / `///` / `*` commented lines are not flagged
  * baseline round-trip (write then load gives same set)
  * new hit outside baseline is detected

Uses stdlib only — no pytest, no conftest.py, no test-framework coupling
to the F4 `tests/audit/` tree (so this file can ship on main before or
after F4 merges without friction).
"""
from __future__ import annotations

import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT_PATH = Path(__file__).resolve().parent / "instant-arithmetic-check.py"


def _load_checker():
    spec = importlib.util.spec_from_file_location(
        "instant_arithmetic_check",
        SCRIPT_PATH,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"could not load {SCRIPT_PATH}")
    mod = importlib.util.module_from_spec(spec)
    sys.modules["instant_arithmetic_check"] = mod
    spec.loader.exec_module(mod)
    return mod


CHECKER = _load_checker()


class ScanHitsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="iac-"))
        (self.tmp / "src").mkdir()
        self.rs = self.tmp / "src" / "fake.rs"

    def tearDown(self):
        for f in self.tmp.rglob("*"):
            if f.is_file():
                f.unlink()
        for d in sorted(self.tmp.rglob("*"), reverse=True):
            if d.is_dir():
                d.rmdir()
        self.tmp.rmdir()

    def _scan(self):
        return CHECKER.scan_hits(self.tmp)

    def test_flags_raw_pattern(self):
        self.rs.write_text(
            "fn f() {\n"
            "    let past = Instant::now() - Duration::from_secs(60);\n"
            "}\n",
            encoding="utf-8",
        )
        hits = self._scan()
        self.assertEqual(len(hits), 1)
        self.assertIn("Duration::from_secs(60)", hits[0]["content"])

    def test_same_line_exempt_marker_suppresses(self):
        self.rs.write_text(
            "fn f() {\n"
            "    let past = Instant::now() - Duration::from_secs(60); "
            "// INSTANT_SUB_SAFE: test helper, long-uptime only\n"
            "}\n",
            encoding="utf-8",
        )
        hits = self._scan()
        self.assertEqual(hits, [])

    def test_previous_line_exempt_marker_suppresses(self):
        self.rs.write_text(
            "fn f() {\n"
            "    // INSTANT_SUB_SAFE: explained on the previous line\n"
            "    let past = Instant::now() - Duration::from_secs(60);\n"
            "}\n",
            encoding="utf-8",
        )
        hits = self._scan()
        self.assertEqual(hits, [])

    def test_commented_out_line_not_flagged(self):
        self.rs.write_text(
            "fn f() {\n"
            "    // let past = Instant::now() - Duration::from_secs(60);\n"
            "    /// Doc-comment: Instant::now() - Duration::from_secs(60) would panic.\n"
            "    let past = Instant::now() - Duration::from_secs(30);\n"
            "}\n",
            encoding="utf-8",
        )
        hits = self._scan()
        self.assertEqual(len(hits), 1)
        self.assertIn("30", hits[0]["content"])

    def test_target_directory_skipped(self):
        target = self.tmp / "target" / "debug"
        target.mkdir(parents=True)
        (target / "cached.rs").write_text(
            "let x = Instant::now() - Duration::from_secs(60);\n",
            encoding="utf-8",
        )
        self.rs.write_text("// clean file\n", encoding="utf-8")
        hits = self._scan()
        self.assertEqual(hits, [])


class BaselineRoundTripTest(unittest.TestCase):
    def test_write_and_load_round_trip(self):
        with tempfile.TemporaryDirectory(prefix="iac-baseline-") as d:
            path = Path(d) / "baseline.json"
            hits = [
                {"file": "crates/x/src/a.rs", "line": 10, "hash": "abc123", "content": "let a = Instant::now() - Duration::from_secs(5);"},
                {"file": "crates/y/src/b.rs", "line": 20, "hash": "def456", "content": "let b = Instant::now() - Duration::from_secs(9);"},
            ]
            CHECKER.write_baseline(hits, path)
            loaded = CHECKER.load_baseline(path)
            self.assertEqual(
                loaded,
                {("crates/x/src/a.rs", "abc123"), ("crates/y/src/b.rs", "def456")},
            )
            doc = json.loads(path.read_text(encoding="utf-8"))
            self.assertEqual(doc["version"], 1)
            self.assertEqual(len(doc["entries"]), 2)

    def test_load_baseline_missing_file_returns_empty(self):
        with tempfile.TemporaryDirectory(prefix="iac-baseline-") as d:
            path = Path(d) / "nope.json"
            self.assertEqual(CHECKER.load_baseline(path), set())


class NewHitDetectionTest(unittest.TestCase):
    def test_new_hit_outside_baseline_detected(self):
        # Baseline contains a known hit; scan produces that plus one new.
        baseline = {("crates/x/src/a.rs", "abc123")}
        hits = [
            {"file": "crates/x/src/a.rs", "line": 10, "hash": "abc123", "content": "known"},
            {"file": "crates/z/src/c.rs", "line": 30, "hash": "zzz999", "content": "new"},
        ]
        new = [h for h in hits if (h["file"], h["hash"]) not in baseline]
        self.assertEqual(len(new), 1)
        self.assertEqual(new[0]["file"], "crates/z/src/c.rs")

    def test_content_change_counts_as_new_even_same_file(self):
        # Same file, different line content -> different hash -> new hit.
        baseline = {("crates/x/src/a.rs", "old_hash")}
        hits = [
            {"file": "crates/x/src/a.rs", "line": 10, "hash": "new_hash", "content": "modified"},
        ]
        new = [h for h in hits if (h["file"], h["hash"]) not in baseline]
        self.assertEqual(len(new), 1)


if __name__ == "__main__":
    unittest.main()
