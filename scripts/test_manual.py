#!/usr/bin/env python3
"""
RaceControl Manual Test Suite — walks through each subsystem sequentially.
Run: python scripts/test_manual.py [--verbose]
"""

import json
import sys
import time
import requests

CORE_URL = "http://localhost:8080/api/v1"
VERBOSE = "--verbose" in sys.argv

results = []


def test(name: str, fn):
    """Run a test, record pass/fail."""
    try:
        ok, detail = fn()
        status = "PASS" if ok else "FAIL"
    except Exception as e:
        ok = False
        status = "FAIL"
        detail = str(e)[:200]
    results.append({"name": name, "status": status, "detail": detail})
    icon = "\033[92m PASS\033[0m" if ok else "\033[91m FAIL\033[0m"
    print(f"  [{icon}] {name}")
    if VERBOSE or not ok:
        print(f"         {detail}")
    return ok


# ─── 1. Health & Config ─────────────────────────────────────────────────────

def test_health():
    r = requests.get(f"{CORE_URL}/health", timeout=5)
    data = r.json()
    return r.status_code == 200, f"HTTP {r.status_code} — {json.dumps(data)[:100]}"


def test_venue():
    r = requests.get(f"http://localhost:8080/", timeout=5)
    data = r.json()
    return data.get("status") == "running", f"status={data.get('status')}, name={data.get('name')}"


# ─── 2. Drivers ─────────────────────────────────────────────────────────────

def test_list_drivers():
    r = requests.get(f"{CORE_URL}/drivers", timeout=5)
    data = r.json()
    drivers = data if isinstance(data, list) else data.get("drivers", [])
    return r.status_code == 200, f"HTTP {r.status_code}, {len(drivers)} drivers"


# ─── 3. Pods ────────────────────────────────────────────────────────────────

def test_list_pods():
    r = requests.get(f"{CORE_URL}/pods", timeout=5)
    data = r.json()
    pods = data if isinstance(data, list) else data.get("pods", [])
    return r.status_code == 200, f"HTTP {r.status_code}, {len(pods)} pods"


# ─── 4. Pricing ─────────────────────────────────────────────────────────────

def test_pricing_tiers():
    r = requests.get(f"{CORE_URL}/pricing", timeout=5)
    data = r.json()
    tiers = data if isinstance(data, list) else data.get("tiers", data.get("pricing_tiers", []))
    has_tiers = len(tiers) > 0
    return r.status_code == 200 and has_tiers, f"HTTP {r.status_code}, {len(tiers)} tiers"


# ─── 5. Billing ─────────────────────────────────────────────────────────────

def test_billing_active():
    r = requests.get(f"{CORE_URL}/billing/active", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_billing_report():
    r = requests.get(f"{CORE_URL}/billing/report/daily", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


# ─── 6. Games ───────────────────────────────────────────────────────────────

def test_games_active():
    r = requests.get(f"{CORE_URL}/games/active", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


# ─── 7. Cloud Sync ──────────────────────────────────────────────────────────

def test_sync_health():
    r = requests.get(f"{CORE_URL}/sync/health", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


# ─── 8. AI Chat ─────────────────────────────────────────────────────────────

def test_ai_chat():
    r = requests.post(
        f"{CORE_URL}/ai/chat",
        json={"message": "What is RaceControl?", "history": []},
        timeout=90,
    )
    data = r.json()
    has_reply = bool(data.get("reply") or data.get("response"))
    return r.status_code == 200 and has_reply, f"HTTP {r.status_code}, has_reply={has_reply}"


def test_ai_training_stats():
    r = requests.get(f"{CORE_URL}/ai/training/stats", timeout=5)
    data = r.json()
    return r.status_code == 200, f"HTTP {r.status_code}, total={data.get('total', '?')}"


# ─── 9. Kiosk ───────────────────────────────────────────────────────────────

def test_kiosk_experiences():
    r = requests.get(f"{CORE_URL}/kiosk/experiences", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_kiosk_settings():
    r = requests.get(f"{CORE_URL}/kiosk/settings", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


# ─── 10. Leaderboard ────────────────────────────────────────────────────────

def test_leaderboard():
    r = requests.get(f"{CORE_URL}/public/leaderboard", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


# ─── 11. Customer API ───────────────────────────────────────────────────────

def test_customer_experiences():
    r = requests.get(f"{CORE_URL}/customer/experiences", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_customer_ac_catalog():
    r = requests.get(f"{CORE_URL}/customer/ac/catalog", timeout=5)
    data = r.json()
    tracks = len(data.get("tracks", []))
    cars = len(data.get("cars", []))
    return r.status_code == 200, f"HTTP {r.status_code}, {tracks} tracks, {cars} cars"


# ─── Run All ────────────────────────────────────────────────────────────────

def main():
    print("\n=== RaceControl Manual Test Suite ===\n")
    start = time.time()

    sections = [
        ("1. Health & Config", [
            ("GET /health", test_health),
            ("GET / (venue)", test_venue),
        ]),
        ("2. Drivers", [
            ("GET /drivers", test_list_drivers),
        ]),
        ("3. Pods", [
            ("GET /pods", test_list_pods),
        ]),
        ("4. Pricing", [
            ("GET /pricing", test_pricing_tiers),
        ]),
        ("5. Billing", [
            ("GET /billing/active", test_billing_active),
            ("GET /billing/report/daily", test_billing_report),
        ]),
        ("6. Games", [
            ("GET /games/active", test_games_active),
        ]),
        ("7. Cloud Sync", [
            ("GET /sync/health", test_sync_health),
        ]),
        ("8. AI", [
            ("POST /ai/chat", test_ai_chat),
            ("GET /ai/training/stats", test_ai_training_stats),
        ]),
        ("9. Kiosk", [
            ("GET /kiosk/experiences", test_kiosk_experiences),
            ("GET /kiosk/settings", test_kiosk_settings),
        ]),
        ("10. Leaderboard", [
            ("GET /public/leaderboard", test_leaderboard),
        ]),
        ("11. Customer API", [
            ("GET /customer/experiences", test_customer_experiences),
            ("GET /customer/ac/catalog", test_customer_ac_catalog),
        ]),
    ]

    passed = 0
    total = 0
    for section_name, tests in sections:
        print(f"\n  [{section_name}]")
        for name, fn in tests:
            total += 1
            if test(name, fn):
                passed += 1

    elapsed = time.time() - start
    failed = total - passed

    print(f"\n{'=' * 50}")
    print(f"  PASSED: {passed}/{total}  |  FAILED: {failed}  |  Time: {elapsed:.1f}s")
    print(f"{'=' * 50}\n")

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
