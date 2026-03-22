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

## Standing Rules

### Ultimate Rule

**Before marking ANY milestone or phase as shipped, run all three verification layers:**

```bash
# 1. Quality Gate — automated tests (contract + integration + syntax)
cd C:/Users/bono/racingpoint/comms-link && COMMS_PSK="..." bash test/run-all.sh

# 2. E2E — live round-trip verification
curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"node_version"}'   # single exec
curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[{"command":"node_version"}]}'  # chain
curl -s http://localhost:8766/relay/health   # health + connection mode

# 3. Standing Rules — check compliance (auto-push, Bono synced, watchdog running, rules categorized)
```

**All three must pass. No exceptions. No "I'll verify later."**
_Why: v18.0 shipped with 8 integration bugs that 135 unit tests missed. Every bug was caught only by manual E2E after deploy. This rule ensures automated + live + compliance verification happens BEFORE shipped._

**4. Visual verification for display-affecting deploys:**
Any change that touches lock screen, Edge kiosk, overlay, blanking, or browser launch MUST include a visual check — ask the user "are the screens showing correctly?" BEFORE marking shipped. Build IDs, fleet health, and cargo tests cannot catch flicker, misalignment, or rendering issues. Do NOT declare "PASS" from terminal output alone when the change affects what customers see.
_Why: v17.0 browser watchdog caused screen flicker on all pods (kill+relaunch cycle every 30s, plus location.reload() every 5s). Four deploy rounds declared "fixed" without anyone looking at the screens. The flicker was obvious to anyone in the venue._

### Deploy

- **Remote deploy sequence (rc-agent):** (1) `cargo build --release`, (2) copy to deploy-staging, (3) start HTTP server on :9998, (4) exec download on pod: `curl.exe -s -o C:\RacingPoint\rc-agent-new.exe http://192.168.31.27:9998/rc-agent.exe`, (5) exec `RCAGENT_SELF_RESTART` sentinel — rc-agent calls `relaunch_self()` → `start-rcagent.bat` swaps `rc-agent-new.exe` → `rc-agent.exe` and starts the new binary. (6) verify build_id on `/health`.
  _Why: Previous deploy-update.bat used `taskkill /F` which killed the exec handler mid-command, preventing the restart from executing. `RCAGENT_SELF_RESTART` spawns the new process before exiting — reliable across all pods._
- **NEVER use `taskkill /F /IM rc-agent.exe` followed by `start` in the same exec chain.** The taskkill kills the process serving the exec endpoint — subsequent commands in the chain may never execute. Use `RCAGENT_SELF_RESTART` sentinel instead.
  _Why: Pod 5 went offline for 2+ minutes during v17.0 deploy because taskkill killed rc-agent before the restart command ran. rc-sentry eventually recovered it, but the gap is unacceptable._
- **Server deploy (racecontrol):** SCP binary to server, `taskkill /F /IM racecontrol.exe`, `move /Y racecontrol-new.exe racecontrol.exe`, `schtasks /Run /TN StartRCTemp`. The scheduled task survives SSH disconnect.
  _Why: SSH `start` command dies when SSH session closes. schtasks persists independently._
- **NEVER run pod binaries on James's PC** (rc-agent.exe, pod-agent.exe, ConspitLink.exe) — crashes workstation.
  _Why: Pod binaries assume hardware/ports that don't exist on James's machine; crash is instant._
- **Test before upload** = `cargo test` + size check + deploy to Pod 8 first, verify, then other pods.
  _Why: Pod 8 canary catches runtime failures (DLL missing, wrong CWD, config mismatch) before fleet-wide damage._
- **Deploy staging path:** `C:\Users\bono\racingpoint\deploy-staging\`
  _Why: Consistent staging root prevents "which binary is current" confusion across sessions._
- **Pendrive install:** `D:\pod-deploy\install.bat <pod_number>` (v5) — run as admin on the pod. For pods with RCAGENT_SERVICE_KEY blocking exec.
  _Why: Pendrive path is fixed; using ad-hoc paths leaves install.bat version drift._

### Comms

- **Bono INBOX.md:** Append to `C:\Users\bono\racingpoint\comms-link\INBOX.md` → `git add INBOX.md && git commit && git push`. Entry format: `## YYYY-MM-DD HH:MM IST — from james`. Then also send via WS (send-message.js). Git push alone is insufficient — Bono does not auto-pull.
  _Why: Git-only comms left Bono's context stale on three occasions; WS+git is the required dual channel._
- **Auto-push + notify (atomic sequence):** `git push` → comms-link WS message → INBOX.md entry. Do all three before marking tasks complete, starting new work, or responding to Uday. Every push, every commit — even cleanup/docs/logbook. No ranking of "important" vs "minor" commits.
  _Why: Commits without push leave Bono's context stale and break deploy chains; treating minor commits as optional caused missed notifications._
- **Bono VPS exec (v18.0 — DEFAULT):** Use comms-link relay, not SSH. Single: `curl -s -X POST http://localhost:8766/relay/exec/run -H "Content-Type: application/json" -d '{"command":"git_pull"}'`. Chain: `curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[...]}'`. SSH (`ssh root@100.70.177.44`) only when relay is down.
  _Why: SSH requires Tailscale up and leaves no audit trail; relay is always-on and returns structured results._
- **Standing Rules Sync:** After modifying CLAUDE.md standing rules, always sync to Bono via comms-link so both AIs operate under the same rules.
  _Why: Rules drift between AIs causes inconsistent behavior and contradictory decisions in multi-agent tasks._

### Code Quality

- **No `.unwrap()` in production Rust** — use `?`, `.ok()`, or match.
  _Why: Unwrap panics crash the entire service; production code must degrade gracefully._
- **No `any` in TypeScript** — type everything explicitly.
  _Why: `any` hides real type errors that surface at runtime, not compile time._
- **`.bat` files: clean ASCII + CRLF.** Use bash heredoc + `sed 's/$/\r/'`. Never Write tool directly (adds UTF-8 BOM = breaks cmd.exe). Never use parentheses in if/else — use `goto` labels. Test with `cmd /c` before deploying.
  _Why: BOM and parentheses in .bat files cause silent command failures on Windows; caught after multiple broken deploys._
- **Static CRT:** `.cargo/config.toml` `+crt-static` — no vcruntime140.dll dependency on pods.
  _Why: Pod images don't ship VS redistributables; dynamic CRT causes instant crash-on-launch._
- **Cascade updates:** When changing a process, update ALL linked references (training data, playbooks, prompts, docs, memory). Never change one place and leave stale references.
  _Why: Stale references in playbooks or prompts cause both AIs to apply the old behavior after a fix._
- **Next.js hydration:** Never read `sessionStorage`/`localStorage` in `useState` initializer — use `useEffect` + hydrated flag.
  _Why: SSR reads fail server-side; hydration mismatch breaks the entire page silently._
- **Git Bash JSON:** Write JSON payloads to a file with Write tool, then `curl -d @file`. Bash string escaping mangles backslashes.
  _Why: Inline JSON in Git Bash strips backslashes from Windows paths, corrupting the payload._
- **UI must reflect config truth** — no hardcoded camera lists, names, or layouts. All UI must read from API/config dynamically. If the backend config changes, the UI must update without code changes.
  _Why: v16.1 cameras dashboard was initially built with hardcoded 13-camera arrays. When cameras were added/removed from NVR config, the UI showed stale/phantom tiles. Dynamic fetch from /api/v1/cameras fixed it — this rule prevents regression._

### Process

- **Refactor Second** — characterization tests first, verify green, then refactor. No exceptions.
  _Why: Refactoring without a green test baseline turns every compile error into an unknown regression._
- **Cross-Process Updates** — changing a feature? Update ALL: rc-agent, racecontrol, PWA, Admin, Gateway, Dashboard.
  _Why: Single-crate updates leave other components speaking a different protocol version, causing silent data corruption._
- **No Fake Data** — use `TEST_ONLY`, `0000000000`, or leave empty. Never real-looking identifiers.
  _Why: Realistic-looking fake data (names, IDs, emails) has leaked into production databases twice._
- **Prompt Quality Check** — missing clarity/specificity/actionability/scope → ask one focused question before acting.
  _Why: Acting on ambiguous prompts produces work that must be redone; one question costs less than one wrong implementation._
- **Learn From Past Fixes** — check LOGBOOK + commit history before re-investigating.
  _Why: Re-investigating solved problems wastes session time; LOGBOOK has resolved the same issue in under 2 minutes._
- **LOGBOOK:** After every commit, append `| timestamp IST | James | hash | summary |` to `LOGBOOK.md`.
  _Why: LOGBOOK is Tier 2 debugging — without consistent entries, memory-based debugging fails._

### Debugging

- **Cross-Process Recovery Awareness** — independent recovery systems (self_monitor, rc-sentry watchdog, server pod_monitor/WoL, scheduler wake) can fight each other. When adding or modifying any auto-recovery, auto-restart, or auto-wake logic, verify it won't cascade with the others.
  - A graceful self-restart must be distinguishable from a real crash (use sentinel files or IPC).
  - Escalation (e.g. MAINTENANCE_MODE) must know *why* restarts are happening, not just count them. Server-down restarts ≠ pod crashes.
  - WoL auto-wake will revive pods that entered MAINTENANCE_MODE, creating infinite loops. Any "pod offline" recovery must check whether the pod was deliberately taken offline.
  - Always test recovery paths against **server downtime**, not just pod failures.

  _Why: Self-restart + watchdog + WoL created an infinite restart loop that took 45 minutes to diagnose; the systems had no coordination._
- **Allowlist Auth known issue** — `/api/v1/config/kiosk-allowlist` requires auth. rc-agent calls it without auth → 401 → pods run on hardcoded local allowlist only. Fix when touching kiosk or auth code.
  _Why: Known gap; documenting here prevents it being "fixed" in isolation without updating rc-agent._
- **"Shipped" Means "Works For The User"** — A milestone is NOT shipped until every user-facing endpoint is verified working at runtime:
  - Binary built, deployed, and **running** (not just compiled). All runtime dependencies present (DLLs, models, config files).
  - Every API endpoint returns correct data (not just HTTP 200 — check response content).
  - Every UI page renders and is interactive (open in browser, verify visually with screenshot).
  - Hardware integrations tested with live data (cameras, GPU inference, network devices).
  - **Frontend: verify from the user's browser, not from the server.** `NEXT_PUBLIC_` env vars are baked at build time — rebuild with correct LAN IP.
  - **Frontend: standalone deploy requires `.next/static` copied into `.next/standalone/`.**

  _Why: "Phase Complete" was reported 9 times based on compilation alone — runtime failures were hidden each time._
- **Long-Lived Tasks Must Log Lifecycle** — Any `tokio::spawn` or `std::thread::spawn` loop must log: (a) when it starts, (b) when it processes its first item, (c) when it exits. Errors in new pipelines use `warn`/`error`, not `debug`.
  _Why: Silent task death (panic in spawned thread, channel close) went undetected for hours because no lifecycle logs existed._

### Security

- **Allowlist auth required** on `/api/v1/config/kiosk-allowlist` — see Debugging section for full context.
  _Why: Unauthenticated endpoint would allow any LAN device to overwrite the process allowlist._
- **Process guard safe mode:** Do not disable rc-process-guard during testing sessions — use the allowlist override instead.
  _Why: Disabling the guard entirely during a test left the machine unprotected when the session ended without re-enabling it._

---

## 4-Tier Debug Order

| Tier | Method | When | Action |
|------|--------|------|--------|
| 1 | **Deterministic** | Always first | Stale sockets, game cleanup, temp files, WerFault — apply without LLM |
| 2 | **Memory** | After Tier 1 fails | Check LOGBOOK.md + commit history for identical past incident |
| 3 | **Local Ollama** | After Tier 2 fails | Query qwen3:0.6b at James .27:11434 |
| 4 | **Cloud Claude** | Last resort | Escalate — NOT auto-triggered |

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
