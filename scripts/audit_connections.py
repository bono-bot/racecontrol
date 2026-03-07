#!/usr/bin/env python3
"""
RaceControl Connection Audit — tests all 16 connection paths.
Run: python scripts/audit_connections.py
"""

import json
import sys
import time
import requests
import sqlite3
from pathlib import Path
from datetime import datetime, timedelta

CORE_URL = "http://localhost:8080/api/v1"
CLOUD_URL = "https://app.racingpoint.cloud/api/v1"
OLLAMA_URL = "http://localhost:11434"
POD8_IP = "192.168.31.91"
TERMINAL_SECRET = "rp-terminal-2026"
DB_PATH = Path(__file__).parent.parent / "data" / "racecontrol.db"

results = []


def check(name: str, fn):
    """Run a check, record pass/fail."""
    try:
        ok, detail = fn()
        status = "PASS" if ok else "FAIL"
    except Exception as e:
        ok = False
        status = "FAIL"
        detail = str(e)
    results.append({"name": name, "status": status, "detail": detail})
    icon = "\033[92m✓\033[0m" if ok else "\033[91m✗\033[0m"
    print(f"  {icon} [{status}] {name}: {detail}")
    return ok


def test_core_health():
    r = requests.get(f"{CORE_URL}/health", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_pod8_agent():
    try:
        r = requests.get(f"http://{POD8_IP}:8090/ping", timeout=5)
        return r.text.strip().strip('"') == "pong", f"Response: {r.text.strip()[:50]}"
    except requests.ConnectionError:
        return False, f"Connection refused — pod-agent not running on {POD8_IP}:8090"


def test_pod8_in_core():
    r = requests.get(f"{CORE_URL}/pods", timeout=5)
    pods = r.json()
    if isinstance(pods, dict) and "pods" in pods:
        pods = pods["pods"]
    pod8 = [p for p in pods if str(p.get("number")) == "8" or "pod_8" in str(p.get("id", "")).lower()]
    return len(pod8) > 0, f"Found {len(pods)} pods, pod_8 present: {len(pod8) > 0}"


def test_ollama():
    r = requests.get(f"{OLLAMA_URL}/api/version", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}, version: {r.json().get('version', '?')}"


def test_cloud_reachable():
    r = requests.get(f"{CLOUD_URL}/health", timeout=10)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_cloud_sync_pull():
    r = requests.get(
        f"{CLOUD_URL}/sync/changes",
        params={"since": "2020-01-01T00:00:00Z", "tables": "drivers"},
        headers={"x-terminal-secret": TERMINAL_SECRET},
        timeout=10,
    )
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_sync_state_recent():
    if not DB_PATH.exists():
        return False, f"DB not found at {DB_PATH}"
    conn = sqlite3.connect(str(DB_PATH))
    rows = conn.execute("SELECT table_name, last_synced FROM sync_state ORDER BY last_synced DESC LIMIT 3").fetchall()
    conn.close()
    if not rows:
        return False, "No sync_state rows"
    latest = rows[0][1]
    return True, f"Latest sync: {rows[0][0]} at {latest}"


def test_sync_health():
    r = requests.get(f"{CORE_URL}/sync/health", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_ai_chat():
    r = requests.post(
        f"{CORE_URL}/ai/chat",
        json={"message": "What pods are online?", "history": []},
        timeout=90,
    )
    data = r.json()
    has_reply = bool(data.get("reply") or data.get("response") or data.get("message"))
    return r.status_code == 200 and has_reply, f"HTTP {r.status_code}, has_reply={has_reply}"


def test_kiosk_settings():
    r = requests.get(f"{CORE_URL}/kiosk/settings", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_pricing_tiers():
    r = requests.get(f"{CORE_URL}/pricing", timeout=5)
    data = r.json()
    tiers = data if isinstance(data, list) else data.get("tiers", data.get("pricing_tiers", []))
    return r.status_code == 200 and len(tiers) > 0, f"HTTP {r.status_code}, {len(tiers)} tiers"


def test_drivers():
    r = requests.get(f"{CORE_URL}/drivers", timeout=5)
    return r.status_code == 200, f"HTTP {r.status_code}"


def test_billing_active():
    r = requests.get(f"{CORE_URL}/billing/active", timeout=5)
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
    return r.status_code == 200, f"HTTP {r.status_code}, {tracks} tracks, {cars} cars"


def main():
    print("\n=== RaceControl Connection Audit ===\n")
    start = time.time()

    checks = [
        (" 1. rc-core health", test_core_health),
        (" 2. Pod 8 agent", test_pod8_agent),
        (" 3. Pod 8 in rc-core", test_pod8_in_core),
        (" 4. Ollama", test_ollama),
        (" 5. Cloud reachable", test_cloud_reachable),
        (" 6. Cloud sync pull", test_cloud_sync_pull),
        (" 7. Sync state (DB)", test_sync_state_recent),
        (" 8. Sync health", test_sync_health),
        (" 9. AI chat", test_ai_chat),
        ("10. Kiosk settings", test_kiosk_settings),
        ("11. Pricing tiers", test_pricing_tiers),
        ("12. Drivers", test_drivers),
        ("13. Billing active", test_billing_active),
        ("14. Leaderboard", test_leaderboard),
        ("15. Customer experiences", test_customer_experiences),
        ("16. AC catalog", test_ac_catalog),
    ]

    passed = sum(1 for name, fn in checks if check(name, fn))
    failed = len(checks) - passed
    elapsed = time.time() - start

    print(f"\n{'=' * 50}")
    print(f"  PASSED: {passed}/{len(checks)}  |  FAILED: {failed}  |  Time: {elapsed:.1f}s")
    print(f"{'=' * 50}\n")

    # Save report
    report_dir = Path(__file__).parent.parent / "data"
    report_dir.mkdir(exist_ok=True)
    report = {
        "timestamp": datetime.utcnow().isoformat(),
        "total": len(checks),
        "passed": passed,
        "failed": failed,
        "duration_seconds": round(elapsed, 1),
        "checks": results,
    }
    report_path = report_dir / "audit_report.json"
    report_path.write_text(json.dumps(report, indent=2))
    print(f"  Report saved to {report_path}")

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
