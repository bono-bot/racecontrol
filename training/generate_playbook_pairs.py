#!/usr/bin/env python3
"""Generate training pairs from the debugging playbook and seeded debug_playbooks table."""

import json
import random
import sqlite3
from pathlib import Path

OUTPUT_PATH = Path(__file__).parent / "data" / "playbook_pairs.json"
DB_PATH = Path(r"C:\Users\bono\racingpoint\racecontrol\racecontrol.db")

PODS = [
    {"num": 1, "ip": "192.168.31.89"},
    {"num": 2, "ip": "192.168.31.33"},
    {"num": 3, "ip": "192.168.31.28"},
    {"num": 4, "ip": "192.168.31.88"},
    {"num": 5, "ip": "192.168.31.86"},
    {"num": 6, "ip": "192.168.31.87"},
    {"num": 7, "ip": "192.168.31.38"},
    {"num": 8, "ip": "192.168.31.91"},
]

SIMS = ["Assetto Corsa", "F1 25", "iRacing", "Le Mans Ultimate", "Forza Motorsport"]
SIM_EXES = {
    "Assetto Corsa": "acs.exe",
    "F1 25": "F1_25.exe",
    "iRacing": "iRacingSim64DX11.exe",
    "Le Mans Ultimate": "LMU.exe",
    "Forza Motorsport": "ForzaMotorsport.exe",
}

# Playbook-derived training pairs with varied scenarios
PLAYBOOK_TEMPLATES = [
    # Category 1: Blank screen during active session
    {
        "questions": [
            "Pod {num} screen is blank but the customer has an active session. What's wrong?",
            "Customer on pod {num} says screen went black during their session.",
            "Blank screen on pod {num} during active billing. How to fix?",
            "Pod {num} at {ip} shows black screen even though billing is running.",
        ],
        "answer": (
            "Blank screen during active session has several possible causes:\n\n"
            "1. **rc-agent restarted during session**: racecontrol now re-sends BillingStarted on agent reconnect (fix 273db1c). "
            "Check racecontrol logs for 'Resynced billing session'.\n\n"
            "2. **Stale rc-agent holding port 18923**: Run `netstat -ano | findstr 18923` on the pod to check. "
            "Kill ALL rc-agent processes with `taskkill /F /IM rc-agent.exe`, wait 3s, restart.\n\n"
            "3. **Browser not relaunching**: Fixed in 05ef1d6 — browser now relaunches on every BillingStarted event.\n\n"
            "4. **WebSocket disconnected**: Check if rc-agent connected to racecontrol. "
            "Quick fix: `POST /api/v1/pods/pod_{num}/screen` with `{{\"blank\": false}}`.\n\n"
            "Debug checklist:\n"
            "- `curl http://localhost:8080/api/v1/billing/active` — is session active?\n"
            "- On pod: `tasklist | findstr rc-agent` — how many instances?\n"
            "- On pod: `curl.exe -s http://127.0.0.1:18923/` — what page is showing?"
        ),
    },
    # Category 2: Zombie rc-agent processes
    {
        "questions": [
            "Pod {num} has multiple rc-agent processes running. How to clean up?",
            "I see 5 rc-agent instances on pod {num}. Is that normal?",
            "Zombie rc-agent processes on pod {num} at {ip}.",
            "Pod {num} rc-agent is behaving weird, might have duplicates.",
        ],
        "answer": (
            "Multiple rc-agent instances is a zombie process issue. Since commit 305638b, "
            "the named mutex `Global\\RacingPoint_RCAgent_SingleInstance` prevents this, but old binaries may still be running.\n\n"
            "Cleanup procedure:\n"
            "1. `taskkill /F /IM rc-agent.exe` — kill ALL instances\n"
            "2. Wait 3 seconds for ports to free\n"
            "3. `start \"rc-agent\" /D C:/RacingPoint C:/RacingPoint/rc-agent.exe`\n"
            "4. Wait 5 seconds\n"
            "5. `tasklist | findstr rc-agent` — verify exactly 1 instance\n\n"
            "If running an old binary without mutex protection, deploy the latest rc-agent."
        ),
    },
    # Category 3: AddrInUse crash
    {
        "questions": [
            "rc-agent on pod {num} crashes with 'AddrInUse' error on port 18923.",
            "Pod {num} rc-agent panics: Os {{ code: 10048, kind: AddrInUse }}.",
            "Can't start rc-agent on pod {num}, port 18923 already in use.",
        ],
        "answer": (
            "AddrInUse on port 18923 means a stale rc-agent process is still holding the lock screen port.\n\n"
            "Diagnosis: `netstat -ano | findstr 18923` — find the PID holding the port.\n\n"
            "Fix:\n"
            "1. `taskkill /F /PID <stale_pid>` — kill the stale process\n"
            "2. Wait 3 seconds for the port to free (TIME_WAIT state)\n"
            "3. Restart: `start \"rc-agent\" /D C:/RacingPoint C:/RacingPoint/rc-agent.exe`\n\n"
            "The SO_REUSEADDR fix (ed946db) reduces TIME_WAIT issues, and the mutex guard (305638b) prevents future zombies."
        ),
    },
    # Category 4: Kiosk login lost
    {
        "questions": [
            "Staff terminal kiosk lost login state. Staff has to re-login.",
            "Kiosk at reception keeps showing login screen.",
            "Staff login on kiosk disappears after page refresh.",
        ],
        "answer": (
            "Staff login persistence was fixed in commit 417dd06. Login state is now stored in sessionStorage:\n"
            "- `kiosk_staff_id` and `kiosk_staff_name` saved on login\n"
            "- Restored from sessionStorage on component mount\n"
            "- Cleared on explicit 'Sign Out' or tab close\n\n"
            "If still happening, check:\n"
            "1. Is the kiosk running the latest build? (`npm run build` in kiosk/ directory)\n"
            "2. Is Edge in private/InPrivate mode? sessionStorage doesn't persist there.\n"
            "3. Is the browser crashing and restarting? Check Windows Event Viewer."
        ),
    },
    # Category 5: Deploy failures
    {
        "questions": [
            "Deploy to pod {num} failed with SIZE MISMATCH.",
            "rc-agent deploy to pod {num} shows curl exit=23.",
            "Can't deploy rc-agent to pod {num}, file is locked.",
            "Deploy script says pod {num} at {ip} is unreachable.",
        ],
        "answer": (
            "Common deploy failures and fixes:\n\n"
            "**SIZE MISMATCH**: certutil was used instead of curl.exe. NEVER use certutil — it adds metadata. "
            "Use `curl.exe -o` for downloads.\n\n"
            "**curl exit=23 (write error)**: Disk full or permissions issue on the pod. Check disk space.\n\n"
            "**File locked**: Old rc-agent still running. Kill it first: `taskkill /F /IM rc-agent.exe`, "
            "wait 3s, then download.\n\n"
            "**Pod unreachable**: Pod may be powered off or DHCP IP changed. Scan subnet port 8090 to find new IP.\n\n"
            "Deploy checklist:\n"
            "1. Build: `cargo build -p rc-agent-crate --release`\n"
            "2. Update EXPECTED_SIZE in deploy script\n"
            "3. Start HTTP server: `python3 -m http.server 8888`\n"
            "4. Run: `python3 deploy-rc-agent.py`"
        ),
    },
    # Category 6: racecontrol binary locked
    {
        "questions": [
            "cargo build for racecontrol fails with 'Access is denied'.",
            "Can't rebuild racecontrol.exe, file is locked.",
            "Build error: Access denied on target/release/racecontrol.exe.",
        ],
        "answer": (
            "racecontrol (racecontrol.exe) is still running and Windows locks the file.\n\n"
            "Fix:\n"
            "1. Stop racecontrol: `powershell -Command \"Stop-Process -Name racecontrol -Force\"`\n"
            "2. Wait 3 seconds\n"
            "3. Rebuild: `cargo build -p racecontrol-crate --release`\n"
            "4. Restart: `./target/release/racecontrol.exe &`\n\n"
            "Warning: Stopping racecontrol drops all WebSocket connections. All rc-agents disconnect briefly. "
            "Active billing sessions survive — recovered from DB on restart."
        ),
    },
    # Category 7: Stale overlay timer
    {
        "questions": [
            "Pod {num} shows overlay timer from a previous session that ended.",
            "Overlay countdown on pod {num} keeps running but kiosk says pod is idle.",
            "Stale billing overlay on pod {num} after racecontrol restart.",
        ],
        "answer": (
            "Stale overlay timer after racecontrol restart — fixed in f74a5f9.\n\n"
            "Root cause: Two bugs:\n"
            "1. `recover_active_sessions()` didn't update pod state (billing_session_id, current_driver, status)\n"
            "2. `end_billing_session()` only checked in-memory timers, not DB\n\n"
            "Debug checklist:\n"
            "- `curl http://localhost:8080/api/v1/billing/active` — in-memory timer exists?\n"
            "- `curl http://localhost:8080/api/v1/billing/sessions?limit=5` — is session 'active' in DB?\n"
            "- On pod: `curl http://127.0.0.1:18925/data` — overlay active=true/false?\n"
            "- If orphaned: `POST /api/v1/billing/{{session-uuid}}/stop` — force-end via DB fallback"
        ),
    },
    # Category 8: WebSocket flapping
    {
        "questions": [
            "Pod {num} shows rapid 'Pod Online' / 'Pod Disconnected' cycling in the activity log.",
            "WebSocket flapping on pod {num} — connects and disconnects repeatedly.",
            "Pod {num} keeps reconnecting every few seconds.",
        ],
        "answer": (
            "WebSocket flapping is caused by a UDP heartbeat race condition — fixed in a0edc5a.\n\n"
            "What happens:\n"
            "1. Agent connects via WebSocket, sends Register\n"
            "2. Before Register completes, UDP heartbeat arrives at core\n"
            "3. Core sees !ws_connected → sends force_reconnect\n"
            "4. Agent drops connection → reconnects → same thing → loop\n\n"
            "Fix: Agent-side 10s grace period — ignores ForceReconnect within 10 seconds of successful WebSocket connect.\n\n"
            "If still happening, the pod may be running an old rc-agent binary without the grace period fix. "
            "Deploy the latest rc-agent."
        ),
    },
    # Category 9: Wheelbase disconnect
    {
        "questions": [
            "Wheelbase disconnected on pod {num}. Customer can't steer.",
            "Pod {num} wheelbase not detected. OpenFFBoard USB device missing.",
            "Conspit Ares wheel not working on pod {num}.",
        ],
        "answer": (
            "Wheelbase disconnect on pod {num} — Conspit Ares 8Nm (OpenFFBoard VID:0x1209 PID:0xFFB0).\n\n"
            "Troubleshooting:\n"
            "1. Check USB connection — try different USB port\n"
            "2. Power-cycle the wheelbase (turn off/on the power strip under the rig)\n"
            "3. Check Device Manager for the OpenFFBoard device\n"
            "4. ConspitLink2.0 auto-restarts via rc-agent's 10s watchdog — no manual restart needed\n"
            "5. If USB hub issues, try connecting wheelbase directly to motherboard USB\n\n"
            "Note: rc-agent monitors HID via hidapi. The driving detector reports `NoDevice` state when wheelbase disconnects."
        ),
    },
    # Category 10: Pod status shows idle during active billing
    {
        "questions": [
            "Kiosk shows pod {num} as idle but customer is playing with active billing.",
            "Pod {num} status is 'idle' in dashboard, but billing is running.",
            "Pod state mismatch: {num} shows idle, billing is active.",
        ],
        "answer": (
            "Pod status showing idle during active billing — fixed in a0edc5a. Multiple root causes:\n\n"
            "1. **Heartbeat overwrites billing state**: Heartbeat handler used insert() which replaced entire pod. "
            "Now uses merge — preserves core-managed fields (status, current_driver, billing_session_id).\n\n"
            "2. **start_billing_session missing fields**: Only set billing_session_id, now also sets status=InSession and current_driver.\n\n"
            "3. **End billing missing cleanup**: Now all end paths clear driver and set status=Idle.\n\n"
            "4. **Register handler missing restore**: After billing resync, pod state is updated.\n\n"
            "If running latest code and still seeing this, check racecontrol logs for heartbeat merge behavior."
        ),
    },
    # Category 11: Windows Defender false positive
    {
        "questions": [
            "Windows Defender flagged a file as trojan on pod {num}.",
            "Defender quarantined rc-agent.exe on pod {num}.",
            "Got a Trojan:Win32/Ceprolad.A alert on pod {num}.",
        ],
        "answer": (
            "This is a Windows Defender false positive caused by using certutil for file downloads.\n\n"
            "NEVER use certutil for binary downloads — it's flagged as Trojan:Win32/Ceprolad.A. "
            "Always use `curl.exe` (built into Windows 11).\n\n"
            "To verify it's a false positive:\n"
            "```powershell\n"
            "Get-MpThreatDetection | Select DetectionID, ThreatID, ProcessName\n"
            "Get-MpThreat | Select ThreatName, IsActive, DidThreatExecute\n"
            "```\n\n"
            "To restore quarantined file and add exclusion:\n"
            "```powershell\n"
            "Add-MpPreference -ExclusionPath 'C:\\RacingPoint'\n"
            "```"
        ),
    },
    # Category 12: Content Manager launch issues
    {
        "questions": [
            "Assetto Corsa won't launch via Content Manager on pod {num}.",
            "CM launch fails on pod {num}. Game doesn't start.",
            "Content Manager shows error when trying to launch AC on pod {num}.",
        ],
        "answer": (
            "AC launch via Content Manager issues (commit 8064559):\n\n"
            "1. **CM blocked for customers**: Employee debug PIN controls CM access. "
            "Customers should use the kiosk to start AC, which launches acs.exe directly.\n\n"
            "2. **Launch method**: Single-player uses direct acs.exe launch. "
            "Multiplayer uses Content Manager `--race` flag.\n\n"
            "3. **Launch requirements**:\n"
            "   - race.ini: AUTOSPAWN=1\n"
            "   - CSP gui.ini: FORCE_START=1 + HIDE_MAIN_MENU=1\n"
            "   - AC Server on Racing-Point-Server (.51): preset RP_OPTIMAL (100% grip)\n\n"
            "4. ConspitLink2.0 auto-restarts via rc-agent watchdog if it crashes.\n\n"
            "5. Check `game_launch_events` table for launch history and errors."
        ),
    },
]


def generate_playbook_pairs() -> list[dict]:
    """Generate varied training pairs from playbook templates."""
    pairs = []

    for template in PLAYBOOK_TEMPLATES:
        # Generate 2-4 variations per template with different pods
        selected_pods = random.sample(PODS, min(len(template["questions"]), len(PODS)))

        for i, question_template in enumerate(template["questions"]):
            pod = selected_pods[i % len(selected_pods)]
            question = question_template.format(num=pod["num"], ip=pod["ip"])
            answer = template["answer"].format(num=pod["num"], ip=pod["ip"])

            pairs.append({
                "instruction": question,
                "input": "",
                "output": answer,
                "source": "playbook",
            })

    return pairs


def export_db_playbooks(db_path: Path) -> list[dict]:
    """Export debug_playbooks table entries as training pairs."""
    if not db_path.exists():
        return []

    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    cur.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='debug_playbooks'")
    if not cur.fetchone():
        conn.close()
        return []

    cur.execute("SELECT category, symptom, diagnosis_steps, fix_steps FROM debug_playbooks")
    rows = cur.fetchall()
    conn.close()

    pairs = []
    for row in rows:
        pod = random.choice(PODS)
        instruction = f"I'm seeing this issue on pod {pod['num']}: {row['symptom']}"
        output_parts = []
        if row["diagnosis_steps"]:
            output_parts.append(f"Diagnosis:\n{row['diagnosis_steps']}")
        if row["fix_steps"]:
            output_parts.append(f"Fix:\n{row['fix_steps']}")

        if output_parts:
            pairs.append({
                "instruction": instruction,
                "input": "",
                "output": "\n\n".join(output_parts),
                "source": f"db_playbook/{row['category']}",
            })

    return pairs


if __name__ == "__main__":
    pairs = generate_playbook_pairs()
    print(f"Generated {len(pairs)} pairs from playbook templates")

    db_pairs = export_db_playbooks(DB_PATH)
    print(f"Exported {len(db_pairs)} pairs from debug_playbooks table")

    all_pairs = pairs + db_pairs
    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with open(OUTPUT_PATH, "w", encoding="utf-8") as f:
        json.dump(all_pairs, f, indent=2, ensure_ascii=False)

    print(f"Total playbook pairs: {len(all_pairs)} -> {OUTPUT_PATH}")
