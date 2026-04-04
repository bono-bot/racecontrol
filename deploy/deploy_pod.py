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
  3. Write new per-pod config via /write endpoint (or PowerShell via server fallback)
  4. If --binary-url or not --config-only: download new rc-agent.exe via /exec curl
  5. Send RCAGENT_SELF_RESTART sentinel — rc-agent relaunches itself via Rust (connection close = success)

The /write endpoint overwrites atomically, but we still delete first to ensure
no stale content remains if the write fails partway through (defense in depth).

Fallback: if pod-agent HTTP (:8090) is unreachable, all exec/write calls are
routed through the racecontrol core server at CORE_URL/api/v1/pods/{pod_id}/exec.
"""

import argparse
import base64
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
CORE_URL = "http://192.168.31.23:8080"

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


def probe_pod_agent(pod_ip, timeout=4):
    """Return True if pod-agent HTTP is directly reachable on :8090."""
    url = "http://{}:{}/ping".format(pod_ip, POD_AGENT_PORT)
    req = urllib.request.Request(url, method="GET")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            resp.read()
            return True
    except Exception:
        return False


def pod_exec_direct(pod_ip, cmd, timeout_ms=10000):
    """Execute a shell command via direct pod-agent HTTP (:8090)."""
    url = "http://{}:{}/exec".format(pod_ip, POD_AGENT_PORT)
    return post_json(url, {"cmd": cmd, "timeout_ms": timeout_ms}, timeout=timeout_ms // 1000 + 5)


def pod_exec_via_server(pod_number, cmd, timeout_ms=10000):
    """Execute a shell command via racecontrol core server WebSocket proxy."""
    pod_id = "pod_{}".format(pod_number)
    url = "{}/api/v1/pods/{}/exec".format(CORE_URL, pod_id)
    return post_json(url, {"cmd": cmd, "timeout_ms": timeout_ms}, timeout=timeout_ms // 1000 + 5)


def pod_write_direct(pod_ip, path, content):
    """Write a file on the pod via direct pod-agent /write endpoint."""
    url = "http://{}:{}/write".format(pod_ip, POD_AGENT_PORT)
    return post_json(url, {"path": path, "content": content}, timeout=30)


def ps_encoded_cmd(ps_script):
    """Encode a PowerShell script as UTF-16LE base64 for use with -EncodedCommand.

    This bypasses all cmd.exe quoting and dollar-sign issues entirely.
    The ps_script should use double-backslash for Windows paths (e.g. C:\\\\RacingPoint\\\\).
    """
    ps_bytes = ps_script.encode("utf-16-le")
    return "powershell -NonInteractive -EncodedCommand {}".format(
        base64.b64encode(ps_bytes).decode("ascii")
    )


def pod_write_via_server(pod_number, path, content):
    """Write a file on the pod via racecontrol server exec + PowerShell -EncodedCommand.

    Uses [IO.File]::WriteAllText so multi-line content is written atomically.
    The script is encoded as UTF-16LE base64 to avoid ALL cmd.exe quoting issues.
    Content is base64-encoded inside the PS script to handle arbitrary bytes.
    """
    content_b64 = base64.b64encode(content.encode("utf-8")).decode("ascii")
    # Double the backslashes for the PowerShell path (Python string → actual path)
    ps_path = path.replace("\\", "\\\\")
    ps_script = "[IO.File]::WriteAllText('{}', [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('{}')), [Text.Encoding]::UTF8)".format(
        ps_path, content_b64
    )
    cmd = ps_encoded_cmd(ps_script)
    result = pod_exec_via_server(pod_number, cmd, timeout_ms=15000)
    if result.get("success", False):
        return {"bytes": len(content), "path": path}
    return result



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

    # Probe direct pod-agent connectivity first.
    # If :8090 is not reachable, fall back to racecontrol server exec proxy.
    print("\n[0/5] Probing pod-agent HTTP on :{}...".format(POD_AGENT_PORT))
    use_server = not probe_pod_agent(pod_ip)
    if use_server:
        print("      :8090 unreachable — using racecontrol server exec proxy ({}/api/v1/pods/pod_{}/exec)".format(
            CORE_URL, pod_number))
    else:
        print("      :8090 reachable — using direct pod-agent HTTP.")

    # Build the swap script once — used by both the server proxy path and the
    # direct path. Rename-then-copy keeps the old binary running until kill.
    # NOTE: Do NOT Start-Process rc-agent.exe here — it would run in Session 0 (no GUI).
    # RCWatchdog service detects the dead agent and restarts it in Session 1 via
    # WTSQueryUserToken + CreateProcessAsUser. Just kill rc-agent; watchdog handles restart.
    if config_only:
        ps_script = (
            "Start-Sleep -Milliseconds 500\r\n"
            "Stop-Process -Name rc-agent -Force -ErrorAction SilentlyContinue\r\n"
            "Start-Sleep -Seconds 2\r\n"
            "# RCWatchdog auto-restarts rc-agent in Session 1\r\n"
        )
    else:
        ps_script = (
            "Rename-Item -Path 'C:\\RacingPoint\\rc-agent.exe' -NewName 'rc-agent-old.exe' -ErrorAction SilentlyContinue\r\n"
            "Copy-Item -Path 'C:\\RacingPoint\\rc-agent-new.exe' -Destination 'C:\\RacingPoint\\rc-agent.exe' -Force\r\n"
            "Remove-Item 'C:\\RacingPoint\\rc-agent-new.exe' -ErrorAction SilentlyContinue\r\n"
            "Start-Sleep -Milliseconds 500\r\n"
            "Stop-Process -Name rc-agent -Force -ErrorAction SilentlyContinue\r\n"
            "Start-Sleep -Seconds 2\r\n"
            "Remove-Item 'C:\\RacingPoint\\rc-agent-old.exe' -ErrorAction SilentlyContinue\r\n"
            "# RCWatchdog auto-restarts rc-agent in Session 1\r\n"
        )

    if use_server:
        # ── SERVER FALLBACK PATH ──────────────────────────────────────────────
        # Port :8090 is unreachable from James's PC.
        # All operations go through racecontrol server WebSocket exec proxy.
        #
        # Key constraint: killing rc-agent disconnects the WebSocket, so we
        # CANNOT kill-then-restart as separate calls — the second call would fail.
        # Strategy:
        #   1/5  - No-op (don't kill old binary yet; WebSocket must stay alive)
        #   2/5  - Delete old config via server exec
        #   3/5  - Write new config via PowerShell base64 decode (server exec)
        #   4/5  - Download new binary to staging path rc-agent-new.exe (server exec)
        #   5/5  - Atomic swap: single PowerShell script that kills, renames, starts
        #          (all in one exec call — WebSocket dies after kill, but the script
        #           is already running in a detached process so the swap completes)

        print("\n[1/5] Skipping pre-kill (server fallback: WebSocket must stay live).")

        # Step 2: Delete old config
        print("\n[2/5] Deleting old rc-agent.toml...")
        result = pod_exec_via_server(pod_number, "del /Q C:\\RacingPoint\\rc-agent.toml", timeout_ms=5000)
        if result.get("success", False):
            print("      Deleted.")
        else:
            print("      Not found or already gone (OK): {}".format(result.get("stderr", "")[:80]))

        # Step 3: Write new config via PowerShell base64 decode
        print("\n[3/5] Writing new rc-agent.toml...")
        config_content = generate_config(pod_number)
        result = pod_write_via_server(pod_number, "C:\\RacingPoint\\rc-agent.toml", config_content)
        if "error" in result and "bytes" not in result:
            print("[FAIL] Config write failed: {}".format(result))
            return False
        print("      Written: {} bytes to {}".format(result.get("bytes", "?"), result.get("path", "?")))

        # Step 4: Download new binary to staging path (keep old binary running)
        if not config_only:
            staging_path = "C:\\RacingPoint\\rc-agent-new.exe"
            print("\n[4/5] Downloading rc-agent-new.exe from {}...".format(binary_url))
            dl_cmd = "curl.exe -s -f -o {} {}".format(staging_path, binary_url)
            result = pod_exec_via_server(pod_number, dl_cmd, timeout_ms=120000)
            if result.get("success", False):
                print("      Download complete.")
            else:
                print("[FAIL] Download failed: {}".format(result.get("stderr", "")[:200]))
                return False
        else:
            print("\n[4/5] Skipping binary download (--config-only).")

        # Step 5: Atomic kill + replace + start via detached PowerShell script.
        # Two-phase to avoid cmd-line length / quoting issues with long inline scripts:
        #   Phase A: write swap-agent.ps1 to disk via base64 decode (server exec)
        #   Phase B: launch swap-agent.ps1 detached (Start-Process) — the script
        #            outlives rc-agent, kills it, swaps binary, starts new instance.
        # rc-agent.exe exits → WebSocket drops → old instance gone.
        # The detached PowerShell process then completes the copy + start independently.
        print("\n[5/5] Writing swap script + launching detached PowerShell...")
        swap_script_path = "C:\\RacingPoint\\swap-agent.ps1"
        # Phase A: write the script file
        write_result = pod_write_via_server(pod_number, swap_script_path, ps_script)
        if "error" in write_result and "bytes" not in write_result:
            print("      [WARN] Failed to write swap script: {}".format(write_result))
            print("      Attempting direct inline restart as fallback...")
        else:
            print("      Swap script written ({} bytes).".format(write_result.get("bytes", "?")))

        # Phase B: launch the script detached — use -EncodedCommand to avoid quoting issues.
        # The outer PS script uses Start-Process to spawn swap-agent.ps1 detached.
        launch_ps = "Start-Process powershell -ArgumentList '-NonInteractive -ExecutionPolicy Bypass -File C:\\\\RacingPoint\\\\swap-agent.ps1' -WindowStyle Hidden"
        detach_cmd = ps_encoded_cmd(launch_ps)
        restart_result = pod_exec_via_server(pod_number, detach_cmd, timeout_ms=10000)
        # The WebSocket may close mid-exec as rc-agent exits — that's expected.
        error = restart_result.get("error", "")
        relaunch_ok = (
            restart_result.get("success", False)
            or "timed out" in error.lower()
            or "connection" in error.lower()
            or "reset" in error.lower()
            or "eof" in error.lower()
            or "ws command timed out" in error.lower()
        )
        if relaunch_ok:
            print("      Detached PowerShell swap launched — rc-agent will restart in ~5s.")
        else:
            print("      Note: unexpected response: {}".format(restart_result))

    else:
        # ── DIRECT PATH ──────────────────────────────────────────────────────
        # Port :8090 is directly reachable — use pod-agent HTTP for all steps.
        # Keep rc-agent alive through steps 1-4 (it's serving :8090).
        # Swap happens atomically in step 5 via detached PowerShell script.

        print("\n[1/5] Skipping pre-kill (rc-agent serves :8090 — must stay alive for steps 2-4).")

        # Step 2: Delete old config (ignore failure — may not exist)
        print("\n[2/5] Deleting old rc-agent.toml...")
        result = pod_exec_direct(pod_ip, "del /Q C:\\RacingPoint\\rc-agent.toml", timeout_ms=5000)
        if result.get("success", False):
            print("      Deleted.")
        else:
            print("      Not found or already gone (OK): {}".format(result.get("stderr", "")[:80]))

        # Step 3: Write new config via /write
        print("\n[3/5] Writing new rc-agent.toml...")
        config_content = generate_config(pod_number)
        result = pod_write_direct(pod_ip, "C:\\RacingPoint\\rc-agent.toml", config_content)
        if "error" in result and "status" not in result:
            print("[FAIL] Config write failed: {}".format(result))
            return False
        print("      Written: {} bytes to {}".format(result.get("bytes", "?"), result.get("path", "?")))

        # Step 4: Download new binary to staging path (keep old binary running)
        if not config_only:
            staging_path = "C:\\RacingPoint\\rc-agent-new.exe"
            print("\n[4/5] Downloading rc-agent-new.exe from {}...".format(binary_url))
            dl_cmd = "curl.exe -s -f -o {} {}".format(staging_path, binary_url)
            result = pod_exec_direct(pod_ip, dl_cmd, timeout_ms=120000)
            if result.get("success", False):
                print("      Download complete.")
            else:
                print("[FAIL] Download failed: {}".format(result.get("stderr", "")[:200]))
                return False
        else:
            print("\n[4/5] Skipping binary download (--config-only).")

        # Step 5: Write swap script + launch detached — same rename-then-copy approach as WS path.
        print("\n[5/5] Writing swap script + launching detached PowerShell...")
        swap_script_path = "C:\\RacingPoint\\swap-agent.ps1"
        write_result = pod_write_direct(pod_ip, swap_script_path, ps_script)
        if "error" in write_result and "bytes" not in write_result:
            print("      [WARN] Failed to write swap script: {}".format(write_result))
        else:
            print("      Swap script written ({} bytes).".format(write_result.get("bytes", "?")))

        launch_ps = "Start-Process powershell -ArgumentList '-NonInteractive -ExecutionPolicy Bypass -File C:\\\\RacingPoint\\\\swap-agent.ps1' -WindowStyle Hidden"
        detach_cmd = ps_encoded_cmd(launch_ps)
        restart_result = pod_exec_direct(pod_ip, detach_cmd, timeout_ms=10000)
        error = restart_result.get("error", "")
        relaunch_ok = (
            restart_result.get("success", False)
            or "timed out" in error.lower()
            or "connection" in error.lower()
            or "reset" in error.lower()
            or "eof" in error.lower()
        )
        if relaunch_ok:
            print("      Detached PowerShell swap launched -> rc-agent will restart in ~5s.")
        else:
            print("      Note: unexpected response: {}".format(restart_result))

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
