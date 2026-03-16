#!/usr/bin/env python3
"""Test domain knowledge of racing-point-ops model vs base llama3.1:8b."""

import json
import subprocess
import sys
import time

MODELS = ["racing-point-ops", "llama3.1:8b"]

# Each test has a query and expected keywords that should appear in the response
TESTS = [
    {
        "query": "What is the IP address of pod 3?",
        "expected": ["192.168.31.28", "pod 3"],
        "category": "network",
    },
    {
        "query": "What wheelbases do we use at Racing Point?",
        "expected": ["Conspit", "Ares", "8Nm", "OpenFFBoard"],
        "category": "hardware",
    },
    {
        "query": "What are the billing tiers and prices?",
        "expected": ["700", "900", "30", "60", "free trial"],
        "category": "billing",
    },
    {
        "query": "Assetto Corsa crashed on pod 5 with exit code -1073741819. What should I do?",
        "expected": ["GPU", "driver", "acs.exe"],
        "category": "crash_diagnosis",
    },
    {
        "query": "How do I deploy rc-agent to the pods?",
        "expected": ["deploy", "cargo build", "rc-agent", "8888"],
        "category": "operations",
    },
    {
        "query": "Pod 7 has multiple rc-agent processes. How to fix?",
        "expected": ["taskkill", "rc-agent", "mutex"],
        "category": "debug",
    },
    {
        "query": "What port does racecontrol run on?",
        "expected": ["8080", "server", ".51"],
        "category": "network",
    },
    {
        "query": "The wheelbase disconnected on pod 2 during a game. What should I check?",
        "expected": ["USB", "ConspitLink", "OpenFFBoard"],
        "category": "hardware",
    },
    {
        "query": "How does the lock screen work?",
        "expected": ["18923", "Edge", "kiosk", "blank"],
        "category": "architecture",
    },
    {
        "query": "What games are available and what are their telemetry ports?",
        "expected": ["9996", "20777", "Assetto", "F1"],
        "category": "games",
    },
]


def query_model(model: str, prompt: str, timeout: int = 60) -> tuple[str, float]:
    """Query an Ollama model and return (response, latency_seconds)."""
    start = time.time()
    try:
        result = subprocess.run(
            ["ollama", "run", model, prompt],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        latency = time.time() - start
        if result.returncode == 0:
            return result.stdout.strip(), latency
        else:
            return f"ERROR: {result.stderr.strip()}", latency
    except subprocess.TimeoutExpired:
        return "TIMEOUT", time.time() - start


def check_keywords(response: str, expected: list[str]) -> tuple[int, int, list[str]]:
    """Check how many expected keywords are in the response. Returns (found, total, missing)."""
    response_lower = response.lower()
    found = 0
    missing = []
    for kw in expected:
        if kw.lower() in response_lower:
            found += 1
        else:
            missing.append(kw)
    return found, len(expected), missing


def main():
    print("=" * 70)
    print("Racing Point Domain Knowledge Evaluation")
    print("=" * 70)

    # Check which models are available
    result = subprocess.run(["ollama", "list"], capture_output=True, text=True)
    available = result.stdout if result.returncode == 0 else ""
    models_to_test = [m for m in MODELS if m in available]

    if not models_to_test:
        print("ERROR: No models available to test.", file=sys.stderr)
        print(f"Available models:\n{available}", file=sys.stderr)
        sys.exit(1)

    print(f"Testing models: {', '.join(models_to_test)}\n")

    results = {model: {"total_score": 0, "total_possible": 0, "latencies": []} for model in models_to_test}

    for i, test in enumerate(TESTS, 1):
        print(f"\n{'─' * 70}")
        print(f"Test {i}/{len(TESTS)} [{test['category']}]")
        print(f"Q: {test['query']}")
        print(f"Expected keywords: {test['expected']}")

        for model in models_to_test:
            response, latency = query_model(model, test["query"])
            found, total, missing = check_keywords(response, test["expected"])
            score_pct = (found / total * 100) if total > 0 else 0

            results[model]["total_score"] += found
            results[model]["total_possible"] += total
            results[model]["latencies"].append(latency)

            # Truncate response for display
            display = response[:200] + "..." if len(response) > 200 else response

            status = "PASS" if found == total else "PARTIAL" if found > 0 else "FAIL"
            print(f"\n  [{model}] {status} ({found}/{total} keywords, {latency:.1f}s)")
            print(f"  Response: {display}")
            if missing:
                print(f"  Missing: {missing}")

    # Summary
    print(f"\n{'=' * 70}")
    print("SUMMARY")
    print(f"{'=' * 70}")

    for model in models_to_test:
        r = results[model]
        pct = (r["total_score"] / r["total_possible"] * 100) if r["total_possible"] > 0 else 0
        avg_latency = sum(r["latencies"]) / len(r["latencies"]) if r["latencies"] else 0
        print(f"\n  {model}:")
        print(f"    Score: {r['total_score']}/{r['total_possible']} ({pct:.0f}%)")
        print(f"    Avg latency: {avg_latency:.1f}s")

    # Save results
    output_path = Path(__file__).parent / "data" / "eval_results.json"
    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    main()
