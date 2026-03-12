#!/usr/bin/env python3
"""
deploy_pod.py — Remote deploy helper for RaceControl rc-agent pods.

Usage:
  python deploy_pod.py <pod_number>            Deploy config + binary + restart
  python deploy_pod.py <pod_number> --config-only  Config + restart, no binary
  python deploy_pod.py <pod_number> --binary-url URL  Custom binary URL
  python deploy_pod.py all                     Deploy to all 8 pods

Steps performed:
  1. Kill rc-agent.exe on pod (ignore failure — may not be running)
  2. Delete old rc-agent.toml (ignore failure — may not exist)
  3. Write new per-pod config via /write endpoint
  4. If --binary-url or not --config-only: download new rc-agent.exe via /exec curl
  5. Start rc-agent.exe (timeout expected — runs indefinitely)

The /write endpoint overwrites atomically, but we still delete first to ensure
no stale content remains if the write fails partway through (defense in depth).
"""

import argparse
import json
import os
import sys
import urllib.request
import urllib.error

# ─── Pod Network Map ──────────────────────────────────────────────────────────

POD_IPS = {
    1: "192.168.31.89",
    2: "192.168.31.33",
    3: "192.168.31.28",
    4: "192.168.31.88",
    5: "192.168.31.86",
    6: "192.168.31.87",
    7: "192.168.31.38",
    8: "192.168.31.91",
}

POD_AGENT_PORT = 8090
DEFAULT_BINARY_URL = "http://192.168.31.27:9998/rc-agent.exe"

# ─── HTTP helpers ─────────────────────────────────────────────────────────────


def post_json(url, data, timeout=30):
    """POST JSON payload to URL. Returns parsed response dict."""
    payload = json.dumps(data).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = resp.read().decode("utf-8", errors="replace")
            return json.loads(body)
    except urllib.error.HTTPError as e:
        body = e.read().decode("utf-8", errors="replace")
        try:
            return json.loads(body)
        except Exception:
            return {"error": "HTTP {}: {}".format(e.code, body[:200])}
    except Exception as e:
        return {"error": str(e)}


def pod_exec(pod_ip, cmd, timeout_ms=10000):
    """Execute a shell command on the pod via /exec endpoint."""
    url = "http://{}:{}/exec".format(pod_ip, POD_AGENT_PORT)
    return post_json(url, {"cmd": cmd, "timeout_ms": timeout_ms}, timeout=timeout_ms // 1000 + 5)


def pod_write(pod_ip, path, content):
    """Write a file on the pod via /write endpoint."""
    url = "http://{}:{}/write".format(pod_ip, POD_AGENT_PORT)
    return post_json(url, {"path": path, "content": content}, timeout=30)


# ─── Config generation ────────────────────────────────────────────────────────


def load_template():
    """Load rc-agent.template.toml from the same directory as this script."""
    script_dir = os.path.dirname(os.path.abspath(__file__))
    template_path = os.path.join(script_dir, "rc-agent.template.toml")
    if not os.path.exists(template_path):
        print("[ERROR] Template not found: {}".format(template_path))
        sys.exit(1)
    with open(template_path, "r", encoding="utf-8") as f:
        return f.read()


def generate_config(pod_number):
    """Render the template for a specific pod number."""
    template = load_template()
    pod_name = "Pod {:02d}".format(pod_number)
    config = template.replace("{pod_number}", str(pod_number))
    config = config.replace("{pod_name}", pod_name)
    return config


# ─── Deploy logic ─────────────────────────────────────────────────────────────


def deploy_pod(pod_number, config_only=False, binary_url=None):
    """Deploy rc-agent to a single pod. Returns True on success."""
    if pod_number not in POD_IPS:
        print("[ERROR] Unknown pod number: {}. Valid: {}".format(pod_number, sorted(POD_IPS.keys())))
        sys.exit(1)

    pod_ip = POD_IPS[pod_number]
    binary_url = binary_url or DEFAULT_BINARY_URL

    print("\n{}".format("=" * 60))
    print("Deploying to Pod {} ({})".format(pod_number, pod_ip))
    print("Mode: {}".format("config-only" if config_only else "full deploy"))
    print("=" * 60)

    # Step 1: Kill rc-agent.exe (ignore failure — may not be running)
    print("\n[1/5] Killing rc-agent.exe...")
    result = pod_exec(pod_ip, "taskkill /F /IM rc-agent.exe", timeout_ms=5000)
    if result.get("success", False):
        print("      rc-agent.exe killed.")
    else:
        print("      Not running or already stopped (OK): {}".format(result.get("stderr", "")[:80]))

    # Step 2: Delete old config (ignore failure — may not exist)
    print("\n[2/5] Deleting old rc-agent.toml...")
    result = pod_exec(pod_ip, "del /Q C:\\RacingPoint\\rc-agent.toml", timeout_ms=5000)
    if result.get("success", False):
        print("      Deleted.")
    else:
        print("      Not found or already gone (OK): {}".format(result.get("stderr", "")[:80]))

    # Step 3: Write new config via /write
    print("\n[3/5] Writing new rc-agent.toml...")
    config_content = generate_config(pod_number)
    result = pod_write(pod_ip, "C:\\RacingPoint\\rc-agent.toml", config_content)
    if "error" in result and "status" not in result:
        print("[FAIL] Config write failed: {}".format(result))
        return False
    print("      Written: {} bytes to {}".format(result.get("bytes", "?"), result.get("path", "?")))

    # Step 4: Download new binary (if not config-only)
    if not config_only:
        print("\n[4/5] Downloading rc-agent.exe from {}...".format(binary_url))
        dl_cmd = "curl.exe -s -f -o C:\\RacingPoint\\rc-agent.exe {}".format(binary_url)
        result = pod_exec(pod_ip, dl_cmd, timeout_ms=120000)
        if result.get("success", False):
            print("      Download complete.")
        else:
            print("[FAIL] Download failed: {}".format(result.get("stderr", "")[:200]))
            return False
    else:
        print("\n[4/5] Skipping binary download (--config-only).")

    # Step 5: Start rc-agent (timeout expected — it runs indefinitely)
    print("\n[5/5] Starting rc-agent.exe...")
    start_cmd = "cd /d C:\\RacingPoint && start /b rc-agent.exe"
    result = pod_exec(pod_ip, start_cmd, timeout_ms=5000)
    stderr = result.get("stderr", "")
    if result.get("success", False) or "timed out" in stderr.lower():
        print("      rc-agent started (or start initiated).")
    else:
        exit_code = result.get("exit_code")
        print("      Note: exit_code={}, stderr={}".format(exit_code, stderr[:100]))

    print("\nPod {} deployment complete.".format(pod_number))
    return True


# ─── Entry point ─────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(
        description="Deploy rc-agent to one or all RaceControl pods via pod-agent HTTP API.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "Examples:\n"
            "  python deploy_pod.py 8                        Deploy to Pod 8 (config + binary + restart)\n"
            "  python deploy_pod.py 8 --config-only          Update config only, restart rc-agent\n"
            "  python deploy_pod.py 8 --binary-url http://192.168.31.27:9998/rc-agent.exe\n"
            "  python deploy_pod.py all                      Deploy to all 8 pods\n"
            "  python deploy_pod.py all --config-only        Update config on all pods\n"
        ),
    )
    parser.add_argument(
        "pod",
        help='Pod number (1-8) or "all" to deploy to all pods',
    )
    parser.add_argument(
        "--config-only",
        action="store_true",
        help="Skip binary download; only update config and restart rc-agent",
    )
    parser.add_argument(
        "--binary-url",
        default=None,
        metavar="URL",
        help="URL to download rc-agent.exe (default: {})".format(DEFAULT_BINARY_URL),
    )

    args = parser.parse_args()

    if args.pod.lower() == "all":
        print("Deploying to all {} pods...".format(len(POD_IPS)))
        failed = []
        for pod_num in sorted(POD_IPS.keys()):
            ok = deploy_pod(pod_num, config_only=args.config_only, binary_url=args.binary_url)
            if not ok:
                failed.append(pod_num)
        if failed:
            print("\n[SUMMARY] FAILED pods: {}".format(failed))
            sys.exit(1)
        else:
            print("\n[SUMMARY] All {} pods deployed successfully.".format(len(POD_IPS)))
    else:
        try:
            pod_number = int(args.pod)
        except ValueError:
            parser.error("Invalid pod number: '{}'. Use 1-8 or 'all'.".format(args.pod))
            return
        ok = deploy_pod(pod_number, config_only=args.config_only, binary_url=args.binary_url)
        sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
