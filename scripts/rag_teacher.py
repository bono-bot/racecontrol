#!/usr/bin/env python3
"""
RAG Teacher — generates ~80 domain Q&A pairs for Ollama training.

Uses Claude CLI as the teacher to generate high-quality, codebase-aware answers.
Inserts into ai_training_pairs in the local SQLite DB.

Run: python scripts/rag_teacher.py [--dry-run] [--api]
  --dry-run : Print pairs without inserting into DB
  --api     : Use racecontrol /ai/training/import endpoint instead of direct DB
"""

import hashlib
import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

DB_PATH = Path(__file__).parent.parent / "data" / "racecontrol.db"
CRATE_ROOT = Path(__file__).parent.parent / "crates"
CORE_URL = "http://localhost:8080/api/v1"

# Stop words — mirrors extract_keywords() in ai.rs
STOP_WORDS = {
    "the", "is", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for",
    "of", "with", "by", "from", "as", "it", "this", "that", "are", "was", "be",
    "has", "had", "not", "no", "do", "does", "did", "will", "would", "could",
    "should", "may", "can", "been", "being", "have", "were", "they", "them",
    "their", "its", "you", "your", "we", "our", "i", "my", "me", "he", "she",
    "his", "her", "what", "which", "who", "when", "where", "how", "all", "each",
    "every", "both", "few", "more", "most", "other", "some", "such", "than",
    "too", "very", "just", "about", "above", "after", "again", "also", "any",
    "because", "before", "between", "here", "there", "into", "only", "over",
    "same", "so", "then", "these", "those", "through", "under", "up", "out",
}


def extract_keywords(text: str) -> str:
    """Port of ai.rs extract_keywords()."""
    words = re.split(r"[^a-zA-Z0-9_.]", text.lower())
    return " ".join(w for w in words if len(w) >= 2 and w not in STOP_WORDS)


def query_hash(question: str) -> str:
    """MD5 hash for dedup — simpler than Rust's DefaultHasher but serves same purpose."""
    return hashlib.md5(question.encode()).hexdigest()


def read_source_snippet(path: str, max_lines: int = 80) -> str:
    """Read first N lines of a source file for context."""
    full = CRATE_ROOT / path
    if not full.exists():
        return ""
    lines = full.read_text(encoding="utf-8", errors="ignore").splitlines()[:max_lines]
    return "\n".join(lines)


def ask_claude(question: str, context: str = "") -> str:
    """Call Claude CLI to generate an answer."""
    prompt = f"""You are James, the AI operations assistant for RacingPoint eSports & Cafe.
Answer the following question based on the RaceControl codebase context provided.
Be specific, technical, and concise (under 200 words). Reference actual function names, ports, and config values.

{f"CODE CONTEXT:{chr(10)}{context}{chr(10)}{chr(10)}" if context else ""}QUESTION: {question}

ANSWER:"""

    try:
        result = subprocess.run(
            ["claude", "-p", "--output-format", "text"],
            input=prompt,
            capture_output=True,
            text=True,
            timeout=60,
        )
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
    except (subprocess.TimeoutExpired, FileNotFoundError) as e:
        print(f"  Claude CLI failed: {e}")

    return ""


# ─── Training Data Categories ───────────────────────────────────────────────

TRAINING_PAIRS = [
    # ── Billing (15) ──
    {
        "category": "billing",
        "question": "How does a billing session start in RaceControl?",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "What are the pricing tiers and how much do they cost?",
        "context_file": None,
        "static_answer": "RaceControl has 3 pricing tiers: Free Trial (5 minutes, free), 30-Minute Session (₹700), and 60-Minute Session (₹900). Pricing is stored in the pricing_tiers table with price_paise (multiply by 100). The idle detection threshold is 10 seconds — if no steering/throttle input, the timer pauses after 10s.",
    },
    {
        "category": "billing",
        "question": "How does the billing timer tick and sync to the database?",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "What happens when a billing session ends? Walk me through the flow.",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "How does idle detection work during a billing session?",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "How does wallet debit work when a customer starts a session?",
        "context_file": "racecontrol/src/wallet.rs",
    },
    {
        "category": "billing",
        "question": "Can a billing session be paused and resumed?",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "How does billing recover after racecontrol restarts?",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "What is the billing session lifecycle from start to end?",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "How does extending a billing session work?",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "What billing events are logged and where?",
        "context_file": "racecontrol/src/billing.rs",
    },
    {
        "category": "billing",
        "question": "How does the BillingTick message reach the pod lock screen?",
        "context_file": None,
        "static_answer": "Every second, billing::tick_all_timers() decrements active timers. For each active session, it sends CoreToAgentMessage::BillingTick { remaining_seconds, allocated_seconds, driver_name } to the agent via the agent_senders channel. The rc-agent lock screen receives this and updates the countdown display. Simultaneously, DashboardEvent::BillingTick is broadcast for web dashboards.",
    },
    {
        "category": "billing",
        "question": "How does dynamic pricing work in RaceControl?",
        "context_file": "racecontrol/src/api/routes.rs",
    },
    {
        "category": "billing",
        "question": "What is the daily revenue report endpoint and what does it return?",
        "context_file": "racecontrol/src/api/routes.rs",
    },
    {
        "category": "billing",
        "question": "How does the auto-end feature work when a session timer runs out?",
        "context_file": "racecontrol/src/billing.rs",
    },
    # ── Pods (15) ──
    {
        "category": "pods",
        "question": "What are the pod statuses and what does each mean?",
        "context_file": None,
        "static_answer": "Pods have 5 statuses: offline (not responding to heartbeats), idle (connected but no active session), in_session (customer actively using the pod), error (pod reported a problem, needs healing), and disabled (manually taken offline by staff). Status is stored in-memory in state.pods and persisted to the pods table.",
    },
    {
        "category": "pods",
        "question": "How does pod discovery work via mDNS?",
        "context_file": "racecontrol/src/main.rs",
    },
    {
        "category": "pods",
        "question": "How does Wake-on-LAN (WoL) work for pods?",
        "context_file": "racecontrol/src/wol.rs",
    },
    {
        "category": "pods",
        "question": "What does the pod monitor do and how often does it run?",
        "context_file": "racecontrol/src/pod_monitor.rs",
    },
    {
        "category": "pods",
        "question": "How does the pod healer work? What does it auto-fix?",
        "context_file": "racecontrol/src/pod_healer.rs",
    },
    {
        "category": "pods",
        "question": "What is the pod registration flow when a new pod connects?",
        "context_file": None,
        "static_answer": "When rc-agent starts on a pod, it connects via WebSocket to ws://<core_ip>:8080/ws/agent and sends AgentMessage::Register(PodInfo) with pod_id, number, IP, MAC, and initial status. racecontrol's ws/mod.rs handles this — inserts/updates the pods table, adds to state.pods HashMap, creates an agent_senders channel entry, and broadcasts DashboardEvent::PodUpdate to web dashboards.",
    },
    {
        "category": "pods",
        "question": "What is the pod network topology? List all pod IPs.",
        "context_file": None,
        "static_answer": "8 sim racing pods on subnet 192.168.31.x: Pod 1 (.89), Pod 2 (.33), Pod 3 (.28), Pod 4 (.88), Pod 5 (.86), Pod 6 (.87), Pod 7 (.38), Pod 8 (.91). All share MAC prefix 30-56-0F. The racecontrol server is at 192.168.31.27. Router/gateway is 192.168.31.1. Each pod runs Windows 11 with 32GB RAM and 16 CPUs. pod-agent runs on port 8090, rc-agent on the WebSocket connection to core.",
    },
    {
        "category": "pods",
        "question": "How does DHCP drift cause pods to go offline?",
        "context_file": None,
        "static_answer": "Pods get IPs via DHCP from the router. After reboot, a pod may get a different IP. racecontrol still tries the old IP and marks the pod as offline. Fix: scan subnet for port 8090 responses, or set DHCP reservations in the router (192.168.31.1) binding each pod's MAC to a fixed IP. TCP scan: try socket.connect((ip, 8090)) across 192.168.31.2-254.",
    },
    {
        "category": "pods",
        "question": "How does pod-agent differ from rc-agent?",
        "context_file": None,
        "static_answer": "pod-agent (port 8090, Rust/Axum) is a lightweight remote management tool — handles file operations, command execution, and health checks via REST API. rc-agent (WebSocket to core:8080) is the full sim racing lifecycle manager — lock screen, telemetry capture, billing display, game process monitoring, USB wheelbase detection. pod-agent is deployed on all machines including the server; rc-agent only runs on sim pods.",
    },
    {
        "category": "pods",
        "question": "What happens when a pod crashes or becomes unresponsive?",
        "context_file": "racecontrol/src/pod_healer.rs",
    },
    {
        "category": "pods",
        "question": "How do I shutdown a pod remotely?",
        "context_file": "racecontrol/src/api/routes.rs",
    },
    {
        "category": "pods",
        "question": "What is the error_aggregator and how does it detect patterns?",
        "context_file": "racecontrol/src/error_aggregator.rs",
    },
    {
        "category": "pods",
        "question": "How does the smart scheduler decide when to wake/shutdown pods?",
        "context_file": "racecontrol/src/scheduler.rs",
    },
    {
        "category": "pods",
        "question": "What are the watchdog scripts and how do they auto-restart agents?",
        "context_file": None,
        "static_answer": "watchdog-rc-agent.cmd and watchdog-pod-agent.cmd are infinite-loop batch scripts in pod-scripts/. They start the respective .exe, wait for exit, then restart after 5 seconds. install-watchdogs.cmd copies these scripts to C:\\RacingPoint\\ on each pod and creates Windows scheduled tasks (ONSTART, SYSTEM privilege) via setup-autostart.cmd. This ensures agents survive crashes without manual intervention.",
    },
    {
        "category": "pods",
        "question": "What is the pod reservation system?",
        "context_file": "racecontrol/src/pod_reservation.rs",
    },
    # ── Games (10) ──
    {
        "category": "games",
        "question": "How does Assetto Corsa launch on a pod? What's the full sequence?",
        "context_file": None,
        "static_answer": "AC launch sequence: 1) Kill existing acs.exe via pod-agent /exec. 2) Write race.ini with AUTOSPAWN=1 for auto-join. 3) Set CSP gui.ini FORCE_START=1 and HIDE_MAIN_MENU=1. 4) Launch acs.exe directly (NOT AssettoCorsa.exe, NOT Steam/CM) from the assettocorsa folder. 5) Wait 8 seconds for AC to initialize. 6) Restart ConspitLink2.0.exe — wheel display telemetry only works if started AFTER AC. Script: python pod_ac_launch.py <pod_ip> [car] [track] [driver].",
    },
    {
        "category": "games",
        "question": "What games does RaceControl support and what are their UDP ports?",
        "context_file": None,
        "static_answer": "5 games supported: Assetto Corsa (UDP 9996), F1 25 (UDP 20777), Forza Motorsport (UDP 5300), iRacing (UDP 6789), Le Mans Ultimate (UDP 5555). The SimType enum in rc-common/types.rs defines these. rc-agent listens on the corresponding UDP port based on which game is launched.",
    },
    {
        "category": "games",
        "question": "Why does ConspitLink need to restart after AC launches?",
        "context_file": None,
        "static_answer": "ConspitLink 2.0 connects to AC's shared memory API to display telemetry on the Conspit Ares wheel display. If ConspitLink is running before AC starts, it fails to detect AC's shared memory and shows no data. Restarting ConspitLink after AC has initialized (wait 8s) forces it to reconnect. This is a known quirk of the shared memory initialization order.",
    },
    {
        "category": "games",
        "question": "How does game health monitoring work?",
        "context_file": "racecontrol/src/game_launcher.rs",
    },
    {
        "category": "games",
        "question": "How does the AC dedicated server work for multiplayer?",
        "context_file": "racecontrol/src/ac_server.rs",
    },
    {
        "category": "games",
        "question": "What is the game launcher flow from API call to game running?",
        "context_file": "racecontrol/src/game_launcher.rs",
    },
    {
        "category": "games",
        "question": "How does telemetry data flow from game to dashboard?",
        "context_file": None,
        "static_answer": "Game → UDP packet → rc-agent sim adapter (e.g., assetto_corsa.rs parses AC shared memory/UDP) → builds TelemetryFrame → sends AgentMessage::Telemetry(frame) over WebSocket to racecontrol → core broadcasts DashboardEvent::Telemetry(frame) to all connected dashboards. TelemetryFrame has 130+ fields: speed, RPM, gear, throttle, brake, steering, lap time, position, tire data, etc.",
    },
    {
        "category": "games",
        "question": "What are the AC server presets and what is RP_OPTIMAL?",
        "context_file": None,
        "static_answer": "AC server presets are saved configurations for LAN multiplayer sessions. Stored in ac_presets table with config_json. RP_OPTIMAL is the recommended preset: SESSION_START=100% grip, RANDOMNESS=0, SESSION_TRANSFER=100%, LAP_GAIN=0, clear weather, no wind, SUN_ANGLE=16. Other presets: SERVER_01, SERVER_02, SERVER_05 (bad grip — 80%). The AC server runs on Racing-Point-Server (192.168.31.23).",
    },
    {
        "category": "games",
        "question": "How does the camera controller work for spectator mode?",
        "context_file": "racecontrol/src/ac_camera.rs",
    },
    {
        "category": "games",
        "question": "What processes are protected and should never be killed?",
        "context_file": None,
        "static_answer": "Protected processes (kiosk whitelist in rc-agent): rc-agent.exe, pod-agent.exe, ConspitLink2.0.exe, explorer.exe, dwm.exe, csrss.exe, svchost.exe, SearchHost.exe, ShellExperienceHost.exe, RuntimeBroker.exe. The kiosk module monitors running processes and only allows whitelisted ones plus the active game. Non-whitelisted processes are terminated to prevent customers from accessing the desktop.",
    },
    # ── Cloud Sync (10) ──
    {
        "category": "cloud_sync",
        "question": "How does cloud sync work between racecontrol and the cloud?",
        "context_file": "racecontrol/src/cloud_sync.rs",
    },
    {
        "category": "cloud_sync",
        "question": "What tables are synced from cloud and what is the merge strategy?",
        "context_file": None,
        "static_answer": "Cloud-authoritative tables: drivers, wallets, pricing_tiers, kiosk_experiences, kiosk_settings. racecontrol polls GET /sync/changes?since=<last_sync>&tables=<list> every 30 seconds with x-terminal-secret header. Upsert strategy: drivers/pricing/settings → cloud wins (INSERT OR REPLACE). Wallets → MAX(credits) from cloud, preserve local debits. Local-authoritative: billing_sessions, laps, game_state — never overwritten by cloud.",
    },
    {
        "category": "cloud_sync",
        "question": "What is the sync_state table and how is it used?",
        "context_file": "racecontrol/src/cloud_sync.rs",
    },
    {
        "category": "cloud_sync",
        "question": "How does the remote terminal allow cloud commands to execute locally?",
        "context_file": "racecontrol/src/remote_terminal.rs",
    },
    {
        "category": "cloud_sync",
        "question": "What is the action queue and how does it reduce booking latency?",
        "context_file": None,
        "static_answer": "The action queue is a cloud→racecontrol path that polls every 3 seconds (vs 30s for sync). racecontrol calls GET /actions/pending and processes CloudAction variants: BookingCreated, WalletTopUp, BookingCancelled, QrConfirmed, SettingsChanged, Notification. After processing, it ACKs via POST /actions/{id}/ack. This reduces PWA booking latency from 30s to ~3s without requiring port forwarding or tunnels.",
    },
    {
        "category": "cloud_sync",
        "question": "What is the cloud API URL and what authentication does it use?",
        "context_file": None,
        "static_answer": "Cloud API: https://app.racingpoint.cloud/api/v1 (Bono's VPS at 72.60.101.58). Authentication: x-terminal-secret header with value from racecontrol.toml [cloud].terminal_secret. The terminal_pin is for the web UI authentication only (Uday's PIN). API gateway (Express.js, port 3100) proxies requests to racecontrol on the cloud side.",
    },
    {
        "category": "cloud_sync",
        "question": "How does the wallet merge strategy prevent double-spending?",
        "context_file": "racecontrol/src/cloud_sync.rs",
    },
    {
        "category": "cloud_sync",
        "question": "What happens if cloud sync fails or the internet goes down?",
        "context_file": None,
        "static_answer": "racecontrol is designed for offline resilience. If cloud sync fails, it logs a warning and retries next interval (30s). All billing, pod management, game launching, and lock screen functionality works offline — these are local-authoritative. Only new customer registrations and wallet top-ups from the PWA are delayed. The sync_state table tracks last successful sync per table, so when internet returns, only changes since the last sync are pulled.",
    },
    {
        "category": "cloud_sync",
        "question": "What is the terminal_secret and how is it used across services?",
        "context_file": None,
        "static_answer": "terminal_secret ('rp-terminal-2026') is a shared secret between racecontrol (local) and the cloud API gateway. It's sent as x-terminal-secret header on all cloud API calls: sync/changes, terminal/commands/pending, actions/pending. The cloud validates this header to ensure requests come from an authorized racecontrol instance, not random internet traffic. Set in racecontrol.toml [cloud].terminal_secret.",
    },
    {
        "category": "cloud_sync",
        "question": "How does the sync push work to send local data to the cloud?",
        "context_file": "racecontrol/src/cloud_sync.rs",
    },
    # ── Auth (10) ──
    {
        "category": "auth",
        "question": "How does the PIN authentication flow work for customers?",
        "context_file": "racecontrol/src/auth/mod.rs",
    },
    {
        "category": "auth",
        "question": "How does QR code authentication work for the PWA?",
        "context_file": "racecontrol/src/auth/mod.rs",
    },
    {
        "category": "auth",
        "question": "How does OTP verification work for customer login?",
        "context_file": "racecontrol/src/auth/mod.rs",
    },
    {
        "category": "auth",
        "question": "How does the lock screen work on sim pods?",
        "context_file": None,
        "static_answer": "rc-agent serves an HTML lock screen via a local HTTP server (port 18923). When a customer is assigned to a pod, core sends ShowPinLockScreen or ShowQrLockScreen message. The lock screen displays a PIN entry keypad or QR code. Customer enters PIN → agent sends AgentMessage::PinEntered → core validates via auth module → if correct, billing starts and lock screen clears. The kiosk module prevents ALT+TAB, taskbar access, and non-whitelisted processes.",
    },
    {
        "category": "auth",
        "question": "What is the employee debug PIN and how does it work?",
        "context_file": None,
        "static_answer": "The employee debug PIN is a daily rotating 4-digit code generated from the current date + a salt. Staff can enter this PIN on any pod's lock screen to enter debug/maintenance mode. In debug mode: Content Manager is allowed (normally blocked for customers), billing doesn't start, and the pod shows a staff maintenance screen. GET /api/v1/employee/daily-pin returns today's PIN (staff-only endpoint).",
    },
    {
        "category": "auth",
        "question": "How does JWT authentication work for the customer PWA?",
        "context_file": "racecontrol/src/auth/mod.rs",
    },
    {
        "category": "auth",
        "question": "What are auth_tokens and their lifecycle?",
        "context_file": None,
        "static_answer": "auth_tokens table tracks customer→pod assignments. Lifecycle: 1) Staff assigns customer via /auth/assign → creates token with status='pending'. 2) Token has type 'pin' or 'qr', contains driver_id, pod_id, pricing_tier_id. 3) Customer validates (enters PIN or scans QR) → status='consuming'. 4) Billing starts → status='consumed'. 5) If not validated within pin_expiry_secs (default 600s), auth::expire_stale_tokens() sets status='expired'.",
    },
    {
        "category": "auth",
        "question": "How does the start-now staff override work?",
        "context_file": "racecontrol/src/api/routes.rs",
    },
    {
        "category": "auth",
        "question": "How does terminal PIN authentication work for the admin web UI?",
        "context_file": None,
        "static_answer": "The terminal web UI at james.racingpoint.cloud uses a PIN stored in racecontrol.toml [cloud].terminal_pin. POST /terminal/auth validates the PIN and returns a 24-hour session token. This token is stored in terminal_sessions HashMap and checked on subsequent terminal commands. Only Uday knows this PIN (261121). The token expires after 24 hours.",
    },
    {
        "category": "auth",
        "question": "How are rate limits enforced on OTP requests?",
        "context_file": "racecontrol/src/auth/mod.rs",
    },
    # ── Troubleshooting (15) ──
    {
        "category": "troubleshooting",
        "question": "A pod shows as offline but it's powered on. What should I check?",
        "context_file": None,
        "static_answer": "Checklist: 1) DHCP drift — pod got new IP after reboot. Scan subnet: try port 8090 on 192.168.31.2-254. 2) Windows Firewall — Domain profile may be ON even if Private/Public are OFF. Fix: netsh advfirewall set domainprofile state off. 3) rc-agent not running — check Task Manager. Watchdog should auto-restart, but verify scheduled task exists. 4) WebSocket connection failed — check racecontrol logs for connection errors. 5) Network cable unplugged or switch port issue.",
    },
    {
        "category": "troubleshooting",
        "question": "Pod-agent responds to /ping but file commands fail. Why?",
        "context_file": None,
        "static_answer": "Known CWD bug: pod-agent was installed from a USB drive (F:\\pod-agent). After USB removal, the working directory F:\\ is invalid. dir and file commands fail because they're relative to CWD. Fixes: 1) Use /files?path=C:\\RacingPoint endpoint (works regardless of CWD). 2) Prefix /exec commands with 'cd /d C:\\RacingPoint &'. 3) Reinstall pod-agent with proper CWD. 4) Use 'where /r C:\\ filename.exe' to locate files.",
    },
    {
        "category": "troubleshooting",
        "question": "What are CLOSE_WAIT zombie sockets and how do I fix them?",
        "context_file": None,
        "static_answer": "CLOSE_WAIT happens when a TCP connection's remote end closed but the local process didn't close its socket. Common in rc-agent after long uptime or network blips. Symptoms: new WebSocket connections fail, port appears 'in use'. Fix: identify the PID with 'netstat -ano | findstr CLOSE_WAIT', then kill the stale process (unless it's protected). Long-term: rc-agent uses connection timeouts and periodic reconnect logic to prevent accumulation.",
    },
    {
        "category": "troubleshooting",
        "question": "Windows Firewall is blocking pod connections. How do I fix it?",
        "context_file": None,
        "static_answer": "Windows Firewall has 3 profiles: Domain, Private, Public. Even if Private and Public are OFF, the Domain profile can be ON (happens when PC joins a domain or detects a domain controller). Fix: 'netsh advfirewall set domainprofile state off' then restart pod-agent. Or add specific port rules: 'netsh advfirewall firewall add rule name=\"pod-agent\" dir=in action=allow protocol=TCP localport=8090'. Check all profiles: 'netsh advfirewall show allprofiles'.",
    },
    {
        "category": "troubleshooting",
        "question": "A game crashed on a pod. How does RaceControl handle it?",
        "context_file": "racecontrol/src/game_launcher.rs",
    },
    {
        "category": "troubleshooting",
        "question": "How do I debug a billing session that's stuck?",
        "context_file": None,
        "static_answer": "Check: 1) GET /billing/active — is the session listed? Status should be 'active'. 2) Check racecontrol logs for BillingTick errors. 3) Verify the pod's agent_sender channel exists in state — if the WebSocket dropped, ticks won't reach the pod. 4) Direct DB check: SELECT * FROM billing_sessions WHERE status = 'active'. 5) Force-end: POST /billing/{id}/stop. 6) If timer is stuck in memory but DB says ended, restart racecontrol — recover_active_sessions() will re-sync.",
    },
    {
        "category": "troubleshooting",
        "question": "USB wheelbase disconnected during a session. What happens?",
        "context_file": None,
        "static_answer": "rc-agent monitors the Conspit Ares wheelbase via HID API (VID:0x1209 PID:0xFFB0). If USB disconnects: 1) DrivingState changes to 'idle' (no FFB input detected). 2) If idle exceeds 10s threshold, billing pauses (if auto-pause enabled). 3) The pod healer detects the missing wheelbase and can attempt USB reset. 4) Game may or may not crash — AC handles it gracefully (switches to keyboard), F1 usually crashes. Customer should be notified to re-plug the USB cable.",
    },
    {
        "category": "troubleshooting",
        "question": "Content Manager keeps launching instead of acs.exe. How to fix?",
        "context_file": None,
        "static_answer": "Content Manager (CM) is blocked for customers via the kiosk process whitelist — only acs.exe is allowed. If CM launches: 1) Check if debug mode is active (employee PIN was entered). 2) Verify kiosk.rs whitelist doesn't include ContentManager.exe. 3) Check file associations — .acpreview files may point to CM. 4) CSP's gui.ini must have FORCE_START=1 and HIDE_MAIN_MENU=1 to bypass CM's launcher. 5) Launch acs.exe directly, never AssettoCorsa.exe (which can trigger CM).",
    },
    {
        "category": "troubleshooting",
        "question": "Duplicate game processes are running on a pod. How to fix?",
        "context_file": None,
        "static_answer": "Duplicate processes happen when a previous game didn't fully exit before a new launch. rc-agent's game_process module tracks PIDs, but if rc-agent itself restarted, it loses track. Fix: 1) Use pod-agent /exec to kill all: 'taskkill /f /im acs.exe'. 2) Then relaunch normally via /games/launch. 3) game_launcher.rs check_game_health() (every 5s) should detect and clean up duplicates. 4) For persistent issues, check if watchdog is accidentally restarting the game.",
    },
    {
        "category": "troubleshooting",
        "question": "Cloud sync is not pulling new data. How to diagnose?",
        "context_file": "racecontrol/src/cloud_sync.rs",
    },
    {
        "category": "troubleshooting",
        "question": "The lock screen is not showing on a pod after customer assignment.",
        "context_file": None,
        "static_answer": "Check: 1) Is rc-agent running on the pod? (Check WebSocket connection in racecontrol logs). 2) Is the agent_senders channel populated for this pod_id? (Core must have received a Register message). 3) Was ShowPinLockScreen/ShowQrLockScreen sent? Check racecontrol logs. 4) Is Edge kiosk mode working? The lock screen runs in msedge --kiosk mode on port 18923. 5) Is the port 18923 accessible? (firewall). 6) Check rc-agent logs for lock screen errors.",
    },
    {
        "category": "troubleshooting",
        "question": "How do I reset a pod to a clean state?",
        "context_file": None,
        "static_answer": "Reset sequence: 1) POST /billing/{id}/stop — end any active billing. 2) POST /games/stop — kill running game. 3) POST /auth/cancel — cancel pending auth tokens for the pod. 4) Core sends ClearLockScreen → agent clears lock screen. 5) Core sends BlankScreen if screen_blanking is enabled. 6) Optionally: POST /pods/{id}/shutdown for full power cycle (requires WoL to wake back up). 7) Pod returns to 'idle' status, ready for next customer.",
    },
    {
        "category": "troubleshooting",
        "question": "racecontrol won't start. What are common causes?",
        "context_file": None,
        "static_answer": "Common causes: 1) Port 8080 already in use — another racecontrol instance or process. Fix: netstat -ano | findstr 8080, then kill the PID. 2) SQLite DB locked — another process has data/racecontrol.db open. 3) Invalid racecontrol.toml — TOML parse error. Check syntax. 4) Missing data/ directory — create it. 5) Rust panic in initialization — check stderr for backtrace. 6) Missing DLLs (Windows) — ensure MSVC redistributable is installed.",
    },
    {
        "category": "troubleshooting",
        "question": "A pod's wheel display shows no telemetry data.",
        "context_file": None,
        "static_answer": "The Conspit Ares wheel display gets telemetry via ConspitLink2.0.exe reading AC's shared memory. Checklist: 1) Is ConspitLink2.0.exe running? (Check Task Manager). 2) Was it started AFTER AC? It must be restarted after AC launches. 3) Is AC actually running? (not stuck on loading screen). 4) Is the USB cable properly connected? (VID:0x1209 PID:0xFFB0). 5) Try killing ConspitLink2.0.exe and restarting it. 6) Check if AC's shared memory is enabled in the AC config.",
    },
    {
        "category": "troubleshooting",
        "question": "How do I check the health of all systems at once?",
        "context_file": None,
        "static_answer": "Run the connection audit: python scripts/audit_connections.py — tests 16 connection paths (core health, pod agent, Ollama, cloud, sync, AI chat, pricing, billing, leaderboard, etc.). For deeper checks: GET /api/v1/health (core), GET /api/v1/sync/health (cloud sync timestamps), GET /api/v1/pods (all pod statuses), GET /api/v1/billing/active (active sessions). The error_aggregator module also tracks patterns — check /api/v1/ai/suggestions for automated diagnostics.",
    },
    # ── Business (5) ──
    {
        "category": "business",
        "question": "What are the operating hours and pricing at RacingPoint?",
        "context_file": None,
        "static_answer": "RacingPoint eSports & Cafe is in Bandlaguda, Hyderabad. Pricing: 5-minute free trial (first-timers), 30 minutes for ₹700, 60 minutes for ₹900. The venue has 8 sim racing pods with Conspit Ares 8Nm wheelbases. Games: Assetto Corsa, F1 25, Forza, iRacing, Le Mans Ultimate. Customers can book via the PWA (app.racingpoint.cloud) or walk in.",
    },
    {
        "category": "business",
        "question": "How does the customer journey work from walk-in to racing?",
        "context_file": None,
        "static_answer": "Walk-in flow: 1) Customer arrives → staff creates/finds driver profile. 2) Staff assigns customer to a pod with pricing tier via kiosk terminal (POST /auth/assign). 3) Pod lock screen shows PIN entry. 4) Customer enters 4-digit PIN at the pod. 5) racecontrol validates → billing starts → lock screen clears → game launches. 6) Customer races with countdown timer visible on pod. 7) Timer ends → session summary shows laps/best time → pod returns to idle. PWA flow adds: customer registers online → books → QR code at pod → auto-validates.",
    },
    {
        "category": "business",
        "question": "How does the wallet and credits system work?",
        "context_file": "racecontrol/src/wallet.rs",
    },
    {
        "category": "business",
        "question": "What experiences are available for customers to book?",
        "context_file": None,
        "static_answer": "Custom experiences are stored in kiosk_experiences table with: name, description, sim_type, track, car, difficulty (novice/intermediate/expert/custom), price_paise, and image_url. Examples: 'Nurburgring Challenge' (AC, Nordschleife, Porsche GT3), 'F1 Grand Prix' (F1 25, Monaco). Staff can create/edit via the kiosk terminal (GET/POST /kiosk/experiences). The AC catalog has ~36 tracks and ~325 cars. Customers see active experiences on the PWA at /customer/experiences.",
    },
    {
        "category": "business",
        "question": "How does the leaderboard work?",
        "context_file": None,
        "static_answer": "Leaderboards track best lap times per track+car combination. When AgentMessage::LapCompleted arrives, racecontrol updates: 1) personal_bests table (driver's best per track/car). 2) track_records table (global best per track/car). 3) Broadcasts DashboardEvent::LeaderboardUpdate. Public API: GET /api/v1/public/leaderboard returns top entries. The customer PWA shows leaderboards so players can compare times. Fastest lap of the day is highlighted in the AI chat context.",
    },
]


def insert_pair_db(conn, question: str, answer: str, source: str, quality: int, category: str):
    """Insert a training pair directly into SQLite."""
    qhash = query_hash(question)
    keywords = extract_keywords(question)
    pair_id = __import__("uuid").uuid4().hex[:8]

    # Check for existing
    existing = conn.execute(
        "SELECT id FROM ai_training_pairs WHERE query_hash = ?", (qhash,)
    ).fetchone()
    if existing:
        print(f"  [SKIP] Duplicate: {question[:60]}...")
        return False

    conn.execute(
        """INSERT INTO ai_training_pairs
           (id, query_hash, query_text, query_keywords, response_text, source, model, quality_score, use_count, created_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, datetime('now'))""",
        (pair_id, qhash, question, keywords, answer, source, "claude-cli", quality, ),
    )
    return True


def insert_pair_api(question: str, answer: str, source: str, quality: int):
    """Insert via racecontrol API endpoint."""
    import requests
    r = requests.post(
        f"{CORE_URL}/ai/training/import",
        json=[{"query": question, "response": answer, "source": source, "quality_score": quality}],
        timeout=10,
    )
    return r.status_code == 200


def main():
    dry_run = "--dry-run" in sys.argv
    use_api = "--api" in sys.argv

    print(f"\n=== RAG Teacher — Generating Training Data ===")
    print(f"  Pairs: {len(TRAINING_PAIRS)}")
    print(f"  Mode: {'DRY RUN' if dry_run else 'API' if use_api else 'Direct DB'}\n")

    conn = None
    if not dry_run and not use_api:
        if not DB_PATH.exists():
            print(f"ERROR: Database not found at {DB_PATH}")
            sys.exit(1)
        conn = sqlite3.connect(str(DB_PATH))

    inserted = 0
    skipped = 0
    errors = 0
    categories = {}

    for i, pair in enumerate(TRAINING_PAIRS, 1):
        q = pair["question"]
        cat = pair["category"]
        categories[cat] = categories.get(cat, 0) + 1

        print(f"[{i}/{len(TRAINING_PAIRS)}] ({cat}) {q[:70]}...")

        # Use static answer if provided, otherwise ask Claude
        if "static_answer" in pair:
            answer = pair["static_answer"]
        else:
            # Read source context
            ctx = ""
            if pair.get("context_file"):
                ctx = read_source_snippet(pair["context_file"])

            answer = ask_claude(q, ctx)
            if not answer:
                print(f"  [ERROR] No answer from Claude CLI")
                errors += 1
                continue

            # Rate-limit Claude CLI calls
            time.sleep(1)

        if dry_run:
            print(f"  Answer: {answer[:120]}...")
            inserted += 1
            continue

        if use_api:
            ok = insert_pair_api(q, answer, "rag_teacher", 2)
        else:
            ok = insert_pair_db(conn, q, answer, "rag_teacher", 2, cat)

        if ok:
            inserted += 1
        else:
            skipped += 1

    if conn:
        conn.commit()
        conn.close()

    print(f"\n{'=' * 50}")
    print(f"  Inserted: {inserted}")
    print(f"  Skipped (duplicate): {skipped}")
    print(f"  Errors: {errors}")
    print(f"\n  By category:")
    for cat, count in sorted(categories.items()):
        print(f"    {cat}: {count}")
    print(f"{'=' * 50}\n")


if __name__ == "__main__":
    main()
