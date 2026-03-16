#!/usr/bin/env python3
"""
RaceControl Integration Test Suite — full lifecycle tests.
Run: python scripts/test_integration.py [--cleanup]
  --cleanup : Delete test data after run (default: keep for inspection)
"""

import json
import sys
import time
import uuid
from pathlib import Path
import requests

CORE_URL = "http://localhost:8080/api/v1"
CLEANUP = "--cleanup" in sys.argv
DATA_DIR = Path(__file__).parent.parent / "data"

results = []
test_driver_id = None
test_billing_id = None


def run_test(name: str, fn):
    """Run a test, record result."""
    start = time.time()
    try:
        ok, detail = fn()
        status = "pass" if ok else "fail"
    except Exception as e:
        ok = False
        status = "fail"
        detail = str(e)[:300]
    elapsed = round(time.time() - start, 2)
    results.append({
        "name": name,
        "status": status,
        "detail": detail,
        "duration_seconds": elapsed,
    })
    icon = "\033[92m PASS\033[0m" if ok else "\033[91m FAIL\033[0m"
    print(f"  [{icon}] {name} ({elapsed}s)")
    if not ok:
        print(f"         {detail}")
    return ok


# ─── Test Functions ──────────────────────────────────────────────────────────

def test_health():
    r = requests.get(f"{CORE_URL}/health", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_create_driver():
    global test_driver_id
    test_driver_id = f"test-driver-{uuid.uuid4().hex[:8]}"
    r = requests.post(
        f"{CORE_URL}/drivers",
        json={
            "id": test_driver_id,
            "name": "Integration Test Driver",
            "phone": "+919999999999",
        },
        timeout=10,
    )
    ok = r.status_code in (200, 201)
    return ok, f"HTTP {r.status_code}, driver_id={test_driver_id}"


def test_verify_driver():
    r = requests.get(f"{CORE_URL}/drivers", timeout=5)
    data = r.json()
    drivers = data if isinstance(data, list) else data.get("drivers", [])
    found = any(d.get("id") == test_driver_id for d in drivers)
    return found, f"Driver {test_driver_id} found={found}"


def test_list_pods():
    r = requests.get(f"{CORE_URL}/pods", timeout=5)
    data = r.json()
    pods = data if isinstance(data, list) else data.get("pods", [])
    return r.status_code == 200 and len(pods) >= 0, f"HTTP {r.status_code}, {len(pods)} pods"


def test_pricing_exists():
    r = requests.get(f"{CORE_URL}/pricing", timeout=5)
    data = r.json()
    tiers = data if isinstance(data, list) else data.get("tiers", data.get("pricing_tiers", []))
    return len(tiers) > 0, f"{len(tiers)} pricing tiers"


def test_billing_active():
    r = requests.get(f"{CORE_URL}/billing/active", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_sync_health():
    r = requests.get(f"{CORE_URL}/sync/health", timeout=5)
    data = r.json()
    return r.status_code == 200, f"HTTP {r.status_code}, sync_data={json.dumps(data)[:150]}"


def test_ai_chat():
    r = requests.post(
        f"{CORE_URL}/ai/chat",
        json={"message": "How many pods are there?", "history": []},
        timeout=90,
    )
    data = r.json()
    has_reply = bool(data.get("reply") or data.get("response"))
    model = data.get("model", "?")
    return r.status_code == 200 and has_reply, f"HTTP {r.status_code}, model={model}"


def test_training_stats():
    r = requests.get(f"{CORE_URL}/ai/training/stats", timeout=5)
    data = r.json()
    total = data.get("total", 0)
    return r.status_code == 200, f"HTTP {r.status_code}, {total} training pairs"


def test_training_import():
    r = requests.post(
        f"{CORE_URL}/ai/training/import",
        json=[{
            "query": "Integration test question — what is 2+2?",
            "response": "The answer is 4. This is a test training pair.",
            "source": "integration_test",
            "quality_score": 1,
        }],
        timeout=10,
    )
    data = r.json()
    imported = data.get("imported", 0)
    return r.status_code == 200 and imported >= 0, f"HTTP {r.status_code}, imported={imported}"


def test_training_pairs_list():
    r = requests.get(f"{CORE_URL}/ai/training/pairs?limit=5", timeout=5)
    data = r.json()
    total = data.get("total", 0)
    pairs = data.get("pairs", [])
    return r.status_code == 200, f"HTTP {r.status_code}, total={total}, returned={len(pairs)}"


def test_kiosk_experiences():
    r = requests.get(f"{CORE_URL}/kiosk/experiences", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_kiosk_settings():
    r = requests.get(f"{CORE_URL}/kiosk/settings", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_leaderboard():
    r = requests.get(f"{CORE_URL}/public/leaderboard", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_customer_experiences():
    r = requests.get(f"{CORE_URL}/customer/experiences", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_ac_catalog():
    r = requests.get(f"{CORE_URL}/customer/ac/catalog", timeout=5)
    data = r.json()
    tracks = len(data.get("tracks", []))
    cars = len(data.get("cars", []))
    return r.status_code == 200, f"{tracks} tracks, {cars} cars"


def test_daily_report():
    r = requests.get(f"{CORE_URL}/billing/report/daily", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_games_active():
    r = requests.get(f"{CORE_URL}/games/active", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_ai_suggestions():
    r = requests.get(f"{CORE_URL}/ai/suggestions", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


# ─── Cleanup ─────────────────────────────────────────────────────────────────

def cleanup():
    """Remove test data."""
    if not test_driver_id:
        return
    print("\n  Cleaning up test data...")
    # Note: racecontrol may not have a DELETE /drivers endpoint — that's fine
    try:
        requests.delete(f"{CORE_URL}/drivers/{test_driver_id}", timeout=5)
    except Exception:
        pass


# ─── Main ────────────────────────────────────────────────────────────────────

def main():
    print("\n=== RaceControl Integration Test Suite ===\n")
    start = time.time()

    tests = [
        ("Health check", test_health),
        ("Create test driver", test_create_driver),
        ("Verify driver in DB", test_verify_driver),
        ("List pods", test_list_pods),
        ("Pricing tiers exist", test_pricing_exists),
        ("Billing active sessions", test_billing_active),
        ("Daily billing report", test_daily_report),
        ("Active games", test_games_active),
        ("Sync health", test_sync_health),
        ("AI chat (Ollama)", test_ai_chat),
        ("AI training stats", test_training_stats),
        ("AI training import", test_training_import),
        ("AI training pairs list", test_training_pairs_list),
        ("AI suggestions", test_ai_suggestions),
        ("Kiosk experiences", test_kiosk_experiences),
        ("Kiosk settings", test_kiosk_settings),
        ("Public leaderboard", test_leaderboard),
        ("Customer experiences", test_customer_experiences),
        ("AC catalog", test_ac_catalog),
    ]

    passed = sum(1 for name, fn in tests if run_test(name, fn))
    failed = len(tests) - passed
    elapsed = round(time.time() - start, 1)

    if CLEANUP:
        cleanup()

    # Generate report
    report = {
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "total": len(tests),
        "passed": passed,
        "failed": failed,
        "duration_seconds": elapsed,
        "tests": results,
        "failures": [r for r in results if r["status"] == "fail"],
    }

    DATA_DIR.mkdir(exist_ok=True)
    report_path = DATA_DIR / "test_report.json"
    report_path.write_text(json.dumps(report, indent=2))

    print(f"\n{'=' * 50}")
    print(f"  PASSED: {passed}/{len(tests)}  |  FAILED: {failed}  |  Time: {elapsed}s")
    print(f"  Report: {report_path}")
    print(f"{'=' * 50}\n")

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
