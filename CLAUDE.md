# Racing Point eSports — Project Context

## Project Identity

- **Repo:** racecontrol — Rust/Axum + Next.js monorepo (`C:\Users\bono\racingpoint\racecontrol`)
- **James Vowles** — on-site operations AI, james@racingpoint.in, GitHub: james-racingpoint
- **Bono** — partner AI on VPS (srv1422716.hstgr.cloud), bono@racingpoint.in
- **Uday Singh** — boss, usingh@racingpoint.in. Goal: automate so he can be with his daughter.
- **Timezone:** Always IST (UTC+5:30) for all timestamps

---

## Network Map

| Device | IP | MAC | Notes |
|--------|----|-----|-------|
| Pod 1 | 192.168.31.89 | 30-56-0F-05-45-88 | |
| Pod 2 | 192.168.31.33 | 30-56-0F-05-46-53 | |
| Pod 3 | 192.168.31.28 | 30-56-0F-05-44-B3 | |
| Pod 4 | 192.168.31.88 | 30-56-0F-05-45-25 | |
| Pod 5 | 192.168.31.86 | 30-56-0F-05-44-B7 | |
| Pod 6 | 192.168.31.87 | 30-56-0F-05-45-6E | |
| Pod 7 | 192.168.31.38 | 30-56-0F-05-44-B4 | |
| Pod 8 | 192.168.31.91 | 30-56-0F-05-46-C5 | |
| Server | 192.168.31.23 | 10-FF-E0-80-B1-A7 | Racing-Point-Server, 64GB RAM, DHCP (needs reservation) |
| James | 192.168.31.27 | D8-BB-C1-CD-B3-CF | RTX 4070, static IP, Ollama :11434 |
| POS PC | 192.168.31.20 | 10-4A-7D-5B-C4-DA | WiFi, Tailscale: pos1/100.95.211.1 |
| Spectator | 192.168.31.200 | 00-E0-4C-77-77-DF | WiFi, DeskIn: 712 906 402 |
| Router | 192.168.31.1 | | |
| NVR | 192.168.31.18 | | Dahua 13x cameras |

---

## Crate Names and Binary Naming

| Crate dir | Cargo name | Binary | Role |
|-----------|-----------|--------|------|
| `crates/racecontrol/` | `racecontrol` | `racecontrol.exe` | Server, port 8080 |
| `crates/rc-agent/` | `rc-agent` | `rc-agent.exe` | Pod agent, port 8090 |
| `crates/rc-common/` | `rc-common` | (lib only) | Shared types |

- NEVER call the server "rc-core" in conversation. Crate dir name only.
- Server config: `C:\RacingPoint\racecontrol.toml` (NOT `C:\RaceControl\`)
- Server starts via `start-racecontrol.bat` → HKLM Run key on server
- Pods start via `start-rcagent.bat` → HKLM `Run\RCAgent` key on each pod
- Cargo PATH: `export PATH="$PATH:/c/Users/bono/.cargo/bin"`
- Build commands:
  - `cargo build --release --bin rc-agent`
  - `cargo build --release --bin racecontrol`
  - Tests: `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`

---

## Server Services

| Service | Port | Location | Start |
|---------|------|----------|-------|
| racecontrol | 8080 | Server .23 | `start-racecontrol.bat` (HKLM Run) |
| kiosk | 3300 | Server .23 | Scheduled task |
| web dashboard | 3200 | Server .23 | Scheduled task |
| rc-agent remote_ops | 8090 | All pods | `start-rcagent.bat` (HKLM Run) |
| webterm | 9999 | James .27 | `python C:/Users/bono/racingpoint/deploy-staging/webterm.py` |
| Ollama | 11434 | James .27 | qwen3:0.6b — venue-only |
| Cloud API | 443 | 72.60.101.58 | app.racingpoint.cloud (Bono's VPS) |

---

## Fleet Endpoints

- `GET http://192.168.31.23:8080/api/v1/fleet/health` — array of PodFleetStatus
  - Fields: `pod_number`, `ws_connected`, `http_reachable`, `version`, `build_id`, `uptime_secs`, `last_seen`
  - Filter by `pod_number` field (NOT array index)
- `POST http://192.168.31.23:8080/api/v1/fleet/exec` — remote exec via rc-agent :8090
- Cloud sync: pull/push every 30s. Cloud authoritative: drivers, pricing. Local authoritative: billing, laps, game state.

---

## Deployment Rules

1. **Verification sequence (NO EXCEPTIONS):** kill → delete → download → size check → start → connect
2. **NEVER run pod binaries on James's PC** (rc-agent.exe, pod-agent.exe, ConspitLink.exe) — crashes workstation
3. **Test before upload** = `cargo test` + size check + deploy to Pod 8 first, verify, then other pods
4. **Clean old binaries** before downloading new. Never leave stale binaries.
5. **Latest builds take priority** — check comms-link logbook to confirm newest
6. **.bat files:** clean ASCII + CRLF. Use bash heredoc + `sed 's/$/\r/'`. Never Write tool directly (adds UTF-8 BOM = breaks cmd.exe).
7. **Deploy staging:** `C:\Users\bono\racingpoint\deploy-staging\`
8. **Pendrive:** `D:\pod-deploy\install.bat <pod_number>` (v5) — run as admin on the pod
9. **Static CRT:** `.cargo/config.toml` enables +crt-static — no vcruntime140.dll dependency

---

## 4-Tier Debug Order

| Tier | Method | When | Action |
|------|--------|------|--------|
| 1 | **Deterministic** | Always first | Stale sockets, game cleanup, temp files, WerFault — apply without LLM |
| 2 | **Memory** | After Tier 1 fails | Check LOGBOOK.md + commit history for identical past incident |
| 3 | **Local Ollama** | After Tier 2 fails | Query qwen3:0.6b at James .27:11434 |
| 4 | **Cloud Claude** | Last resort | Escalate — NOT auto-triggered |

---

## Standing Process Rules

1. **Refactor Second** — characterization tests first, verify green, then refactor. No exceptions.
2. **Cross-Process Updates** — changing a feature? Update ALL: rc-agent, racecontrol, PWA, Admin, Gateway, Dashboard.
3. **No Fake Data** — use `TEST_ONLY`, `0000000000`, or leave empty. Never real-looking identifiers.
4. **Prompt Quality Check** — missing clarity/specificity/actionability/scope → ask one focused question before acting.
5. **Learn From Past Fixes** — check LOGBOOK + commit history before re-investigating.
6. **Bono comms:** append to `C:\Users\bono\racingpoint\comms-link\INBOX.md` → `git add INBOX.md && git commit && git push`. Entry format: `## YYYY-MM-DD HH:MM IST — from james`
7. **Auto-push rule:** always `git push` after every commit. No exceptions.
8. **Bono deploy updates:** `git push` → comms-link WS message → INBOX.md entry is an **atomic sequence**. Do all three before marking tasks complete, starting new work, or responding to the user. Every push, every commit — even cleanup/docs/logbook. No mental ranking of "important" vs "minor" commits.
9. **LOGBOOK:** after every commit, append `| timestamp IST | James | hash | summary |` to `LOGBOOK.md`
10. **Cross-Process Recovery Awareness** — independent recovery systems (self_monitor, rc-sentry watchdog, server pod_monitor/WoL, scheduler wake) can fight each other. When adding or modifying any auto-recovery, auto-restart, or auto-wake logic, verify it won't cascade with the others. Specifically:
    - A graceful self-restart must be distinguishable from a real crash (use sentinel files or IPC).
    - Escalation (e.g. MAINTENANCE_MODE) must know *why* restarts are happening, not just count them. Server-down restarts ≠ pod crashes.
    - WoL auto-wake will revive pods that entered MAINTENANCE_MODE, creating infinite loops. Any "pod offline" recovery must check whether the pod was deliberately taken offline.
    - Always test recovery paths against **server downtime**, not just pod failures.
11. **Allowlist Auth** — the `/api/v1/config/kiosk-allowlist` endpoint requires auth. rc-agent currently calls it without auth → 401 → pods run on hardcoded local allowlist only. Fix when touching kiosk or auth code.
12. **Bono VPS exec (v18.0 — DEFAULT):** Use comms-link relay, not SSH. Single command: `curl -s -X POST http://localhost:8766/relay/exec/run -H "Content-Type: application/json" -d '{"command":"git_pull"}'`. Chain: `curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[...]}'`. SSH (`ssh root@100.70.177.44`) only when relay is down.
13. **"Shipped" Means "Works For The User"** — A milestone is NOT shipped until every user-facing endpoint is verified working at runtime. The verification checklist MUST include:
    - Binary built, deployed, and **running** (not just compiled).
    - All runtime dependencies present (DLLs, models, config files, directories).
    - Every API endpoint returns correct data (not just HTTP 200 — check response content).
    - Every UI page renders and is interactive (open in browser, verify visually with screenshot).
    - Hardware integrations tested with live data (cameras, GPU inference, network devices).
    - `cargo check` and unit tests are necessary but NOT sufficient. They prove structure, not function.
    - Never report "all green" based on compilation alone. 9 deployment failures were hidden behind "Phase Complete ✓" because no runtime test was performed.
14. **Long-Lived Tasks Must Log Lifecycle** — Any `tokio::spawn` or `std::thread::spawn` that runs a loop must log: (a) when it starts, (b) when it processes its first item, (c) when it exits. Silent task death is unacceptable. Errors in new pipelines use `warn`/`error` level, not `debug`. Downgrade only after the pipeline is proven working with live data.
15. **Standing Rules Sync** — after modifying CLAUDE.md standing rules, always sync to Bono via comms-link so both AIs operate under the same rules.

---

## Billing and Rates

- 30min / ₹700 | 60min / ₹900 | 5min free trial | 10s idle threshold
- PWA shows "credits" (not rupees)
- Wheelbases: Conspit Ares 8Nm — OpenFFBoard VID:0x1209 PID:0xFFB0
- UDP telemetry ports: 9996 (AC) | 20777 (F1) | 5300 (Forza) | 6789 (iRacing) | 5555 (LMU)

---

## Brand Identity

- Racing Red: `#E10600` | Asphalt Black: `#1A1A1A` | Gunmetal Grey: `#5A5A5A`
- Card: `#222222` | Border: `#333333`
- Fonts: Montserrat (body), Enthocentric (headers)
- OLD orange `#FF4400` is DEPRECATED — do not use

---

## Security Cameras

- 13x Dahua 4MP. Auth: `admin` / `Admin@123`, RTSP `subtype=1`
- NVR: .18 | Entrance: .8 | Reception: .15, .154
- People tracker: port 8095, FastAPI + YOLOv8, entry/exit counting

---

## Key File Paths

| Path | Purpose |
|------|---------|
| `C:\RacingPoint\racecontrol.toml` | Server config (on server .23, user: ADMIN) |
| `C:\RacingPoint\start-racecontrol.bat` | Server start script (HKLM Run) |
| `C:\RacingPoint\start-rcagent.bat` | Pod agent start script (HKLM Run on each pod) |
| `C:\Users\bono\racingpoint\deploy-staging\` | Build staging area + HTTP server (James) |
| `C:\Users\bono\racingpoint\deploy-staging\webterm.py` | Web terminal (Uday's phone → :9999) |
| `C:\Users\bono\racingpoint\comms-link\INBOX.md` | James→Bono comms channel |
| `D:\pod-deploy\` | Pendrive deploy kit (install.bat v5) |
| `LOGBOOK.md` | Incident + commit log at repo root |
| `.cargo\config.toml` | Static CRT build config |

---

## Development Rules

- No `.unwrap()` in production Rust. No `any` in TypeScript. Idempotent SQL migrations.
- Static CRT: `.cargo/config.toml` `+crt-static` — eliminates vcruntime140.dll on pods
- Git config (per-repo): `user.name="James Vowles"`, `user.email="james@racingpoint.in"`
- Cascade updates: when changing a process, update ALL linked references (training data, playbooks, prompts, docs)
- LSP: rust-analyzer enabled in settings.json
- Next.js hydration: never read `sessionStorage`/`localStorage` in useState initializer — use `useEffect` + hydrated flag
- `.bat` files: NEVER use parentheses in if/else blocks — use `goto` labels. Test with `cmd /c` before deploying.
- Git Bash JSON: write JSON payloads to file with Write tool, then `curl -d @file` (bash string escaping mangles `\\`)
- `start` command: always use `/D C:\RacingPoint` to set CWD (rc-agent uses relative `rc-agent.toml`)

---

## Current Blockers

- v6.0 blocked on BIOS AMD-V (SVM Mode disabled on server Ryzen 7 5800X) — does not affect v9.0
- Gmail OAuth tokens expired — MCP Gmail needs re-authorization before Phase 52
- Pod 6 UAC prompt (2026-03-16) — unknown install request, under investigation
- USB mass storage lockdown pending (Group Policy)
- Server DHCP reservation needed: MAC 10-FF-E0-80-B1-A7 → 192.168.31.23
