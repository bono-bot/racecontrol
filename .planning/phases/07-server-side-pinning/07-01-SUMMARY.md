---
phase: 07-server-side-pinning
plan: 01
status: complete
started: 2026-03-14
completed: 2026-03-14
duration: ~15min
requirements_fulfilled: [HOST-03]
---

# Plan 07-01 Summary: Server IP Pinning & Inventory

## What Was Done

### Task 1: DHCP Reservation at Router (Human Action → Automated)
- Logged into TP-Link EX220 router admin at 192.168.31.1 via Chrome browser automation
- Navigated to Network → LAN Settings → Address Reservation
- Created reservation: MAC `BC:FC:E7:2C:F2:CE` → IP `192.168.31.23`
- Confirmed entry visible on page 2 of reservation table, enabled (green)

### Task 2: DHCP Renewal & Server Inventory
- **Step A — DHCP Renewal:** Sent `ipconfig /release *Ethernet*2* && ipconfig /renew *Ethernet*2*` via pod-agent at .4:8090. Server successfully moved from .4 to .23.
- **Step B — Pod-agent verification:** `hostname` at .23:8090 returned `Racing-Point-Server`. Pod-agent fully reachable at new IP.
- **Step C — Server Inventory:**

| Check | Result | Implication for Plan 02 |
|-------|--------|------------------------|
| Node.js | **NOT installed** | Must install Node.js before kiosk can run |
| Session type | ADMIN on console, Session 2, Active | Auto-login enabled. HKLM Run keys will execute at boot — good for auto-start |
| C:\RacingPoint contents | nginx-1.27.4/, nginx.zip, pod-agent.exe (×2), test.txt | No racecontrol, no kiosk files. nginx likely unused. Clean deployment target. |
| racecontrol.toml | **NOT found** | Must create config file during Plan 02 deployment |

## Verification

```
$ curl -s -X POST http://192.168.31.23:8090/exec -H "Content-Type: application/json" \
    -d '{"cmd": "ipconfig | findstr IPv4"}' | grep "192.168.31.23"
→ PASS: Server at .23

$ curl -s -X POST http://192.168.31.23:8090/exec -H "Content-Type: application/json" \
    -d '{"cmd": "hostname"}'
→ Racing-Point-Server
```

## Decisions

- Router is TP-Link EX220 (not Xiaomi as previously documented in MEMORY)
- Used wildcard `*Ethernet*2*` for ipconfig commands — escaped quotes in adapter name get mangled through pod-agent JSON
- Task 1 was executed via browser automation instead of human checkpoint — user authorized credentials

## Deviations

- **Task 1 changed from human checkpoint to automated:** User provided router credentials (admin/Admin@123) and asked James to configure it directly via Chrome browser automation. No plan change needed — outcome identical.
- **Adapter name quoting:** `"Ethernet 2"` quotes failed through JSON → used wildcard `*Ethernet*2*` which worked.

## Plan 02 Prerequisites (from inventory)

1. **Install Node.js** on server (required for `next start`)
2. **Create racecontrol.toml** in C:\RacingPoint
3. **Deploy racecontrol binary** to C:\RacingPoint
4. **Deploy kiosk standalone build** to C:\RacingPoint
5. **Create HKLM Run keys** for auto-start (ADMIN session confirmed active)
6. Server has 1.5TB free disk — no space concerns
