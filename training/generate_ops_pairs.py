#!/usr/bin/env python3
"""Generate training pairs from operational knowledge — network, billing, hardware, games, customer journey."""

import json
import random
from pathlib import Path

OUTPUT_PATH = Path(__file__).parent / "data" / "ops_pairs.json"

PODS = [
    {"num": 1, "ip": "192.168.31.89", "mac": "30-56-0F-05-45-88"},
    {"num": 2, "ip": "192.168.31.33", "mac": "30-56-0F-05-46-53"},
    {"num": 3, "ip": "192.168.31.28", "mac": "30-56-0F-05-44-B3"},
    {"num": 4, "ip": "192.168.31.88", "mac": "30-56-0F-05-45-25"},
    {"num": 5, "ip": "192.168.31.86", "mac": "30-56-0F-05-44-B7"},
    {"num": 6, "ip": "192.168.31.87", "mac": "30-56-0F-05-45-6E"},
    {"num": 7, "ip": "192.168.31.38", "mac": "30-56-0F-05-44-B4"},
    {"num": 8, "ip": "192.168.31.91", "mac": "30-56-0F-05-46-C5"},
]


def generate_network_pairs() -> list[dict]:
    """Network and infrastructure knowledge pairs."""
    pairs = []

    # Pod IP queries
    for pod in PODS:
        pairs.append({
            "instruction": f"What is the IP address of pod {pod['num']}?",
            "input": "",
            "output": f"Pod {pod['num']} is at 192.168.31.{pod['ip'].split('.')[-1]} (MAC: {pod['mac']}). "
                      f"All pods are on subnet 192.168.31.x. Pod-agent runs on port 8090.",
            "source": "ops/network",
        })

    # General network
    pairs.extend([
        {
            "instruction": "What is the network layout of Racing Point?",
            "input": "",
            "output": (
                "Racing Point has 8 sim racing pods on subnet 192.168.31.x:\n"
                "- Pod 1: .89, Pod 2: .33, Pod 3: .28, Pod 4: .88\n"
                "- Pod 5: .86, Pod 6: .87, Pod 7: .38, Pod 8: .91\n"
                "- Server (Racing-Point-Server): .51 (64GB RAM, runs rc-core on port 8080)\n"
                "- James (AI assistant): .27\n"
                "- Router: .1\n\n"
                "Each pod runs rc-agent + pod-agent (port 8090). "
                "Lock screen on port 18923, debug on 18924."
            ),
            "source": "ops/network",
        },
        {
            "instruction": "How do I check if a pod is reachable?",
            "input": "",
            "output": (
                "Check pod connectivity:\n"
                "1. Ping: `ping 192.168.31.<pod_ip>`\n"
                "2. Pod-agent health: `curl -s http://192.168.31.<pod_ip>:8090/health`\n"
                "3. rc-core pod list: `curl -s http://localhost:8080/api/v1/pods`\n\n"
                "If unreachable:\n"
                "- Pod may be powered off\n"
                "- DHCP IP may have changed — scan subnet: `nmap -p 8090 192.168.31.0/24`\n"
                "- Firewall may be blocking — check Windows Firewall on the pod\n"
                "- Network cable disconnected — check physical connection"
            ),
            "source": "ops/network",
        },
        {
            "instruction": "What ports does the RaceControl system use?",
            "input": "",
            "output": (
                "RaceControl port map:\n"
                "- 8080: rc-core REST API + WebSocket (server .51)\n"
                "- 8090: pod-agent REST API (each pod)\n"
                "- 9999: UDP heartbeat (rc-agent <-> rc-core)\n"
                "- 18923: Lock screen HTTP server (rc-agent, localhost only)\n"
                "- 18924: Debug HTTP server (rc-agent)\n"
                "- 18925: Overlay HTTP server (rc-agent)\n"
                "- 3300: Kiosk Next.js app (server .51)\n\n"
                "Game telemetry UDP ports:\n"
                "- 9996: Assetto Corsa\n"
                "- 20777: F1 25\n"
                "- 5300: Forza Motorsport\n"
                "- 6789: iRacing\n"
                "- 5555: Le Mans Ultimate"
            ),
            "source": "ops/network",
        },
        {
            "instruction": "What is the server's role at Racing Point?",
            "input": "",
            "output": (
                "Racing-Point-Server at 192.168.31.23 (64GB RAM):\n"
                "- Runs rc-core (Rust/Axum) on port 8080 — manages billing, pods, games, drivers\n"
                "- Runs kiosk Next.js app on port 3300 — staff terminal\n"
                "- Hosts AC multiplayer server — preset RP_OPTIMAL (100% grip)\n"
                "- SQLite database: racecontrol.db (billing, drivers, laps, AI training)\n"
                "- Cloud sync: pushes data to app.racingpoint.cloud every 30s\n"
                "- rc-core is authoritative for billing and laps. Cloud is authoritative for drivers and pricing."
            ),
            "source": "ops/network",
        },
        {
            "instruction": "How does DHCP work with the pods?",
            "input": "",
            "output": (
                "Pods get IPs via DHCP from router at 192.168.31.1. Each pod has a DHCP reservation "
                "based on MAC address to maintain consistent IPs. However, DHCP drift can occur:\n\n"
                "- After power outage, pods may boot before router, getting fallback IPs\n"
                "- Router firmware update may reset DHCP reservations\n"
                "- New NIC or USB adapter may have different MAC\n\n"
                "If a pod gets a new IP:\n"
                "1. Scan subnet: `nmap -p 8090 192.168.31.0/24`\n"
                "2. Identify pod by hostname or MAC: `arp -a`\n"
                "3. Fix DHCP reservation on router admin (192.168.31.1)"
            ),
            "source": "ops/network",
        },
    ])

    return pairs


def generate_billing_pairs() -> list[dict]:
    """Billing system knowledge pairs."""
    pairs = [
        {
            "instruction": "What are the billing tiers at Racing Point?",
            "input": "",
            "output": (
                "Racing Point billing tiers:\n"
                "- **Free Trial**: 5 minutes, free (hidden in kiosk after first use per driver)\n"
                "- **Half Hour**: 30 minutes, ₹700\n"
                "- **Full Hour**: 60 minutes, ₹900\n\n"
                "Idle threshold: 10 seconds (driving detector monitors wheel/pedal input + game telemetry). "
                "The PWA shows 'credits' not rupees. Wallet system allows prepaid balance."
            ),
            "source": "ops/billing",
        },
        {
            "instruction": "How does billing work in RaceControl?",
            "input": "",
            "output": (
                "Billing flow:\n"
                "1. Customer authenticates at pod via PIN (4-digit) or QR scan + PWA confirmation\n"
                "2. Staff selects pricing tier on kiosk terminal\n"
                "3. rc-core creates billing_session in SQLite, starts timer\n"
                "4. rc-core sends BillingStarted to rc-agent via WebSocket\n"
                "5. rc-agent shows countdown overlay on pod screen\n"
                "6. BillingTick sent every second with remaining time\n"
                "7. On expiry or manual stop: SessionEnded sent, overlay dismissed\n\n"
                "Driving time tracked separately from billing time (idle_threshold = 10s). "
                "Sessions survive rc-core restart — recovered from DB."
            ),
            "source": "ops/billing",
        },
        {
            "instruction": "How do I manually end a billing session?",
            "input": "",
            "output": (
                "To manually end a billing session:\n\n"
                "Via API: `POST /api/v1/billing/{session-uuid}/stop`\n\n"
                "Via kiosk: Click the active session on the pod card, then 'End Session'.\n\n"
                "If the in-memory timer is lost (rc-core restart), the API has a DB fallback (f74a5f9) — "
                "it queries for orphaned active sessions and force-ends them.\n\n"
                "To find the session UUID:\n"
                "- `curl http://localhost:8080/api/v1/billing/active` — lists active in-memory timers\n"
                "- `curl http://localhost:8080/api/v1/billing/sessions?limit=5` — recent sessions from DB"
            ),
            "source": "ops/billing",
        },
        {
            "instruction": "What happens when a game crashes during an active billing session?",
            "input": "",
            "output": (
                "When a game crashes during active billing:\n\n"
                "1. rc-agent detects game process exit (2s check interval)\n"
                "2. Sends GameStateUpdate(Error) to rc-core\n"
                "3. Arms 30s crash recovery timer\n"
                "4. If AI debugger enabled, spawns AI crash analysis\n"
                "5. Waits 30s for rc-core to send SessionEnded\n"
                "6. If no SessionEnded after 30s: force-resets pod (safe state enforcement)\n"
                "7. Safe state: kills all game processes, shows blank lock screen\n\n"
                "The billing session continues — staff can restart the game or end the session. "
                "Driving seconds are paused when no game is running."
            ),
            "source": "ops/billing",
        },
        {
            "instruction": "How does the wallet system work?",
            "input": "",
            "output": (
                "Wallet system:\n"
                "- Drivers can have prepaid balance (wallet_balance_paise in drivers table)\n"
                "- Balance displayed on lock screen between sessions\n"
                "- Cloud authoritative for driver data — wallet sync resolves by phone number when local/cloud IDs differ (d99687c)\n"
                "- PWA shows 'credits' not rupees\n"
                "- Wallet balance pushed to agent via WebSocket (71d8a2b)\n"
                "- Kiosk search shows balance and phone number"
            ),
            "source": "ops/billing",
        },
        {
            "instruction": "How does the free trial work?",
            "input": "",
            "output": (
                "Free trial flow:\n"
                "- 5 minutes, free of charge\n"
                "- Available as a pricing tier in the kiosk\n"
                "- Hidden in kiosk after driver's first use (beb0cd1) — prevents abuse\n"
                "- `unlimited_trials` flag on driver record for test drivers\n"
                "- Trial detection: queries billing_sessions for existing trial usage by driver_id\n"
                "- Staff can still manually select trial tier if needed from admin"
            ),
            "source": "ops/billing",
        },
    ]
    return pairs


def generate_hardware_pairs() -> list[dict]:
    """Hardware knowledge pairs."""
    pairs = [
        {
            "instruction": "What wheelbases does Racing Point use?",
            "input": "",
            "output": (
                "Racing Point uses Conspit Ares 8Nm wheelbases across all 8 pods.\n\n"
                "- USB interface: OpenFFBoard (VID:0x1209 PID:0xFFB0)\n"
                "- Software: ConspitLink2.0 (runs on each pod, must stay alive)\n"
                "- Path: `C:\\Program Files (x86)\\Conspit Link 2.0\\ConspitLink2.0.exe`\n"
                "- Force feedback: 8 Newton-meters\n"
                "- rc-agent monitors via hidapi (HID USB reports)\n"
                "- ConspitLink2.0 managed by rc-agent's 10s watchdog (auto-restarts if crashed)\n"
                "- ensure_conspit_link_running() in ac_launcher.rs handles crash recovery"
            ),
            "source": "ops/hardware",
        },
        {
            "instruction": "What gaming PCs are in the pods?",
            "input": "",
            "output": (
                "Each of the 8 pods has a Windows 11 gaming PC:\n"
                "- All on subnet 192.168.31.x\n"
                "- Running rc-agent (Rust binary) + pod-agent (port 8090)\n"
                "- Edge browser in kiosk mode for lock screen\n"
                "- Conspit Ares 8Nm wheelbase via USB\n"
                "- Games installed: Assetto Corsa, F1 25, iRacing, Le Mans Ultimate, Forza\n\n"
                "James's PC (192.168.31.27): RTX 4070 GPU, runs Ollama for AI inference"
            ),
            "source": "ops/hardware",
        },
        {
            "instruction": "How does the driving detector work?",
            "input": "",
            "output": (
                "The driving detector in rc-agent monitors driver activity from two sources:\n\n"
                "1. **HID USB** (hidapi): Reads OpenFFBoard HID reports from the wheelbase. "
                "Detects steering movement and pedal input. Reports hid_connected and hid_active.\n\n"
                "2. **UDP telemetry**: Game-specific telemetry on per-sim UDP ports. "
                "AC on 9996, F1 on 20777, Forza on 5300, iRacing on 6789, LMU on 5555.\n\n"
                "States: Active (input detected), Idle (no input for 10s), NoDevice (wheelbase disconnected).\n"
                "Idle threshold: 10 seconds (configurable in rc-agent.toml).\n"
                "DrivingState is sent to rc-core via heartbeat and used for billing idle tracking."
            ),
            "source": "ops/hardware",
        },
        {
            "instruction": "What are the security cameras at Racing Point?",
            "input": "",
            "output": (
                "13 Dahua cameras (4MP), auth: admin / Admin@123, RTSP subtype=1\n\n"
                "Key cameras:\n"
                "- NVR: 192.168.31.18\n"
                "- Entrance: 192.168.31.8\n"
                "- Reception: 192.168.31.15, 192.168.31.154\n\n"
                "People tracker on port 8095: FastAPI + YOLOv8, 3 cameras, entry/exit counting."
            ),
            "source": "ops/hardware",
        },
    ]
    return pairs


def generate_game_pairs() -> list[dict]:
    """Game-specific knowledge pairs."""
    games = [
        ("Assetto Corsa", "acs.exe", 9996, "Steam/direct", "Content Manager or direct acs.exe launch"),
        ("F1 25", "F1_25.exe", 20777, "Steam", "Steam launch"),
        ("iRacing", "iRacingSim64DX11.exe", 6789, "iRacing launcher", "iRacing UI launch"),
        ("Le Mans Ultimate", "LMU.exe", 5555, "Steam", "Steam launch"),
        ("Forza Motorsport", "ForzaMotorsport.exe", 5300, "Microsoft Store", "Xbox app launch"),
    ]

    pairs = []
    for name, exe, port, platform, launch in games:
        pairs.append({
            "instruction": f"How does {name} work at Racing Point?",
            "input": "",
            "output": (
                f"{name} details:\n"
                f"- Executable: {exe}\n"
                f"- Telemetry UDP port: {port}\n"
                f"- Platform: {platform}\n"
                f"- Launch method: {launch}\n"
                f"- rc-agent monitors process via game_process.rs (2s check interval)\n"
                f"- Driving detector reads telemetry on UDP {port}\n"
                f"- On crash: GameStateUpdate(Error) sent to rc-core, AI debugger triggered"
            ),
            "source": "ops/games",
        })

    # AC-specific extras
    pairs.extend([
        {
            "instruction": "How is Assetto Corsa launched at Racing Point?",
            "input": "",
            "output": (
                "Assetto Corsa launch process:\n\n"
                "**Single-player**: Direct acs.exe launch\n"
                "1. Write race.ini: AUTOSPAWN=1, track, car, AI opponents\n"
                "2. Write assists.ini: difficulty preset (Beginner/Intermediate/Expert)\n"
                "3. Write apps preset: enable required CSP apps\n"
                "4. CSP gui.ini: FORCE_START=1 + HIDE_MAIN_MENU=1\n"
                "5. Launch acs.exe directly (NOT through Steam)\n"
                "6. Wait for process to load, bring to foreground\n"
                "7. ConspitLink2.0 auto-restarts via watchdog (no manual step needed)\n\n"
                "**Multiplayer**: Content Manager --race flag\n"
                "- AC Server on Racing-Point-Server (.51), preset RP_OPTIMAL (100% grip)\n"
                "- Commit 8064559: CM integration + multiplayer support"
            ),
            "source": "ops/games",
        },
        {
            "instruction": "What AC content is available? How many tracks and cars?",
            "input": "",
            "output": (
                "AC catalog at Racing Point:\n"
                "- 36 tracks available\n"
                "- 325 cars available\n"
                "- Custom experience booking through kiosk\n"
                "- Difficulty presets: Beginner (ABS+TC on), Intermediate (ABS only), Expert (all off)\n"
                "- Track and car selection via kiosk UI, transmitted to rc-agent for race.ini generation"
            ),
            "source": "ops/games",
        },
    ])

    return pairs


def generate_customer_journey_pairs() -> list[dict]:
    """Customer journey and authentication pairs."""
    pairs = [
        {
            "instruction": "How does customer authentication work at Racing Point?",
            "input": "",
            "output": (
                "Two authentication methods:\n\n"
                "**PIN Auth** (primary):\n"
                "1. Staff enters customer phone at reception kiosk\n"
                "2. System generates 4-digit PIN, displays on kiosk\n"
                "3. Customer enters PIN on pod lock screen (numpad)\n"
                "4. rc-agent sends PIN to rc-core for validation\n"
                "5. On success: lock screen shows active session UI\n\n"
                "**QR + PWA** (alternative):\n"
                "1. Customer scans QR code displayed on pod\n"
                "2. Opens PWA in mobile browser\n"
                "3. Confirms session on mobile\n"
                "4. rc-core validates and sends BillingStarted to rc-agent\n\n"
                "Employee debug PIN: Daily rotating 4-digit PIN, Content Manager access blocked for customers."
            ),
            "source": "ops/customer",
        },
        {
            "instruction": "What is the lock screen and how does it work?",
            "input": "",
            "output": (
                "Lock screen is an Edge browser in kiosk mode controlled by rc-agent:\n\n"
                "- HTTP server on port 18923 (localhost only)\n"
                "- States: Hidden, ScreenBlanked (PIN numpad), PinEntry, QrDisplay, "
                "ActiveSession (countdown), SessionSummary, BetweenSessions, AwaitingAssistance, Disconnected\n"
                "- Auto-reload every 3s on blank/disconnected states\n"
                "- Overlay on port 18925 shows billing timer during games\n\n"
                "Manual control: `POST /api/v1/pods/pod_X/screen {\"blank\": true/false}`\n\n"
                "Lock screen prevents customers from accessing desktop/other apps. "
                "Kiosk process whitelist enforces authorized apps only."
            ),
            "source": "ops/customer",
        },
        {
            "instruction": "What happens after a customer's session ends?",
            "input": "",
            "output": (
                "Post-session flow:\n"
                "1. Timer expires or staff ends session via kiosk\n"
                "2. rc-core sends SessionEnded to rc-agent\n"
                "3. Lock screen shows SessionSummary: driver name, total laps, best lap, driving time\n"
                "4. 15-second auto-blank timer starts\n"
                "5. After 15s (if no new session started): screen goes blank (ScreenBlanked)\n"
                "6. enforce_safe_state() kills game processes, minimizes background windows\n"
                "7. Pod status set to Idle, ready for next customer\n\n"
                "If customer has wallet balance, BetweenSessions state shows balance."
            ),
            "source": "ops/customer",
        },
        {
            "instruction": "How does the cloud sync work?",
            "input": "",
            "output": (
                "Cloud sync module in crates/rc-core/src/cloud_sync.rs:\n\n"
                "- Syncs every 30 seconds with app.racingpoint.cloud (72.60.101.58, Bono's VPS)\n"
                "- **Cloud authoritative**: drivers, pricing tiers\n"
                "- **Local authoritative**: billing sessions, laps, game state\n\n"
                "ID mismatch handling:\n"
                "- Local and cloud have different UUIDs for drivers\n"
                "- sync_push resolves by matching phone number or email\n"
                "- Wallet balance sync resolves driver by phone when IDs differ (d99687c)\n\n"
                "PWA shows 'credits' not rupees."
            ),
            "source": "ops/customer",
        },
    ]
    return pairs


def generate_architecture_pairs() -> list[dict]:
    """System architecture knowledge pairs."""
    pairs = [
        {
            "instruction": "What is the RaceControl system architecture?",
            "input": "",
            "output": (
                "RaceControl is a Rust monorepo with 3 crates:\n\n"
                "1. **rc-common**: Shared types, protocol definitions, used by both core and agent\n"
                "2. **rc-core** (port 8080): Central server on Racing-Point-Server (.51)\n"
                "   - Axum REST API + WebSocket server\n"
                "   - Billing management, pod tracking, game launching\n"
                "   - AI service (Ollama → Claude CLI → Anthropic fallback)\n"
                "   - Cloud sync, SQLite database\n"
                "3. **rc-agent**: Runs on each gaming pod\n"
                "   - Lock screen, kiosk enforcement, driving detector\n"
                "   - Game process monitoring, AC launcher\n"
                "   - AI debugger for crash analysis\n"
                "   - UDP heartbeat to rc-core\n\n"
                "Communication: WebSocket (commands) + UDP (heartbeat). Agent registers on connect, "
                "core sends billing commands. Heartbeat every 5s."
            ),
            "source": "ops/architecture",
        },
        {
            "instruction": "What are the protected processes on pods?",
            "input": "",
            "output": (
                "Protected processes that should NEVER be killed on pods:\n"
                "- rc-agent.exe — our agent software\n"
                "- pod-agent.exe — remote management agent\n"
                "- ConspitLink2.0.exe — wheelbase software\n"
                "- explorer.exe — Windows shell\n"
                "- dwm.exe — Desktop Window Manager\n"
                "- csrss.exe — Windows subsystem\n\n"
                "The kiosk process whitelist in rc-agent also allows:\n"
                "- msedge.exe (lock screen browser)\n"
                "- acs.exe (Assetto Corsa)\n"
                "- Game executables for all supported sims\n"
                "- TextInputHost.exe, RuntimeBroker.exe (Windows system)"
            ),
            "source": "ops/architecture",
        },
    ]
    return pairs


if __name__ == "__main__":
    all_pairs = []

    network = generate_network_pairs()
    print(f"Network/infra pairs: {len(network)}")
    all_pairs.extend(network)

    billing = generate_billing_pairs()
    print(f"Billing pairs: {len(billing)}")
    all_pairs.extend(billing)

    hardware = generate_hardware_pairs()
    print(f"Hardware pairs: {len(hardware)}")
    all_pairs.extend(hardware)

    games = generate_game_pairs()
    print(f"Game-specific pairs: {len(games)}")
    all_pairs.extend(games)

    customer = generate_customer_journey_pairs()
    print(f"Customer journey pairs: {len(customer)}")
    all_pairs.extend(customer)

    arch = generate_architecture_pairs()
    print(f"Architecture pairs: {len(arch)}")
    all_pairs.extend(arch)

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    with open(OUTPUT_PATH, "w", encoding="utf-8") as f:
        json.dump(all_pairs, f, indent=2, ensure_ascii=False)

    print(f"\nTotal ops pairs: {len(all_pairs)} -> {OUTPUT_PATH}")
