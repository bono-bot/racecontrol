# Project Research Summary

**Project:** v12.1 E2E Process Guard
**Domain:** Windows process monitoring, whitelist enforcement, and auto-start auditing across a mixed fleet (8 sim pods + 1 server + 1 operations workstation)
**Researched:** 2026-03-21 IST
**Confidence:** HIGH

## Executive Summary

The v12.1 E2E Process Guard is a continuous whitelist enforcement daemon that closes the gap exposed by the March 2026 incident: Steam, a leaderboard kiosk Edge instance, and a voice assistant watchdog all survived manual audits because those audits searched for known-bad processes rather than inverting the whitelist. The correct model is deny-by-default — `running_processes - whitelist = violations` — and this research confirms every capability required is already in the codebase. No new foundational work is needed: `sysinfo 0.33`, `winreg 0.55` (already in rc-agent via `self_heal.rs`), and `rc-common::exec` cover the core enforcement surface. Two new crates are required: `netstat2 0.11` for port-to-PID mapping and `walkdir 2` for Startup folder enumeration.

The recommended approach is three parallel guard deployments: a background tokio task module inside rc-agent covering all 8 pods, a new `process_guard.rs` module inside racecontrol for the server, and a standalone `rc-process-guard.exe` binary for James (.27) that reports via HTTP rather than WebSocket (standing rule: never run pod binaries on James). The central whitelist lives in `racecontrol.toml` with per-machine override sections, fetched by each agent on WS connect and pushed via a new `CoreToAgentMessage::UpdateProcessWhitelist` variant for mid-session updates. The monitoring surface covers four vectors: running processes, listening ports, HKCU/HKLM Run keys, and the Startup folder — with Scheduled Tasks as a Phase 2 addition.

The dominant risks are operational, not algorithmic. The three most dangerous pitfalls all have codebase precedents that prevent them: PID reuse races (require name + creation-time triple before `TerminateProcess`), self-kill (unconditional self-PID exclusion before any whitelist logic), and watchdog restart storms (storm detection after 3 kills in 60 seconds triggers auto-start source lookup rather than continued process killing). The kill grace period — require two consecutive scan cycles before acting — is the single most effective mitigation across multiple pitfalls and must be a first-class design primitive, not a later addition.

---

## Key Findings

### Recommended Stack

All enforcement primitives are already in the repo. The stack research found zero breaking upgrades needed and identified exactly two new Windows-only crates. The `sysinfo 0.33` API must not be upgraded to 0.38 during this milestone — the 0.33 → 0.38 migration includes breaking changes to `System` initialization that would require changes across `kiosk.rs`, `game_process.rs`, and `self_test.rs`.

**Core technologies:**
- `sysinfo 0.33` — process enumeration and kill via `Process::kill(Signal::Kill)` — already in both rc-agent and racecontrol
- `winreg 0.55` — HKCU/HKLM Run key enumeration and `delete_value()` — already in rc-agent via `self_heal.rs`; add to racecontrol
- `netstat2 0.11` — TCP/UDP listening sockets with owning PID via `GetExtendedTcpTable` — NEW, Windows-only dep
- `walkdir 2` — Startup folder enumeration — NEW (or use `std::fs::read_dir` if avoiding deps; folders are flat)
- `tokio 1` — `tokio::time::interval` for scan loop; `tokio::task::spawn_blocking` for WinAPI calls inside async
- `winapi 0.3` — fallback `TerminateProcess` when sysinfo kill returns false; already in rc-agent
- `rc-common::exec` — `schtasks /delete` and `schtasks /query /fo CSV` shell-outs; no new crate needed

**Critical version note:** Do NOT add `windows = "0.58"` — conflicts with existing `winapi 0.3`. Do NOT add `wmi` — COM overhead plus large transitive dep. Do NOT upgrade `sysinfo`.

### Expected Features

The feature set is divided into a clear MVP (solve the triggering incident and prevent reoccurrence) and a validation-dependent backlog.

**Must have (v12.1 core — all P1):**
- `[process_guard]` section in racecontrol.toml with global whitelist and per-machine override tables (TS-1) — deny-by-default schema
- Continuous process scan in rc-agent `process_guard.rs` module via sysinfo, 30s default poll interval (TS-2, TS-12)
- Auto-kill violating processes via sysinfo kill + winapi fallback (TS-3) — with two-cycle grace period and self-exclusion
- HKCU + HKLM Run key audit at startup and every 5 minutes, remove non-whitelisted entries (TS-4)
- Startup folder audit at startup and every 5 minutes, remove non-whitelisted shortcuts (TS-5)
- Pod binary guard: CRITICAL alert if `racecontrol.exe` found on a pod or `rc-agent.exe` found on James (TS-8)
- `AgentMessage::ProcessViolation` WS variant to server on every kill or auto-start removal (TS-9)
- Append-only audit log at `C:\RacingPoint\process-guard.log` with 512KB rotation (TS-10)
- Server stores per-pod violation list in `FleetHealthStore` (TS-11)

**Should have (add after MVP stable):**
- Scheduled Task audit via `schtasks /query /fo CSV` (TS-6) — third auto-start vector; deferred to avoid scope creep
- Port audit via netstat2 (TS-7) — secondary enforcement layer
- Category-tagged whitelist entries (D-1) — refactor after real-world false positives identified
- Severity tiers: KILL / ESCALATE / MONITOR (D-3) — add when violation history justifies tier decisions

**Defer to v12.2+:**
- Repeat offender email escalation (D-4) — data-driven trigger, needs violation log history first
- Fleet health violation count in `/api/v1/fleet/health` (D-5) — nice metric, not blocking
- James standalone guard binary as separate `rc-process-guard` crate (D-6) — requires rc-common refactor; higher scope
- Server-side process guard module (D-7) — lower risk surface, lower priority
- Wildcard/prefix matching in whitelist (D-2) — premature optimization

**Anti-features (do not build):**
- ETW/WMI real-time process event subscription — COM overhead, elevated privilege, marginal gain over 30s polling
- SHA-256 hash verification per process — operational overhead across 11 machines per Windows Update
- Database schema for violation history — in-memory counters + append log is sufficient
- WMI `SELECT * FROM Win32_Process` — 100-500ms CPU spike per query during active gaming sessions

### Architecture Approach

The guard integrates as a background tokio task inside the existing agent binary — no new Windows Service, no new deployment unit for pods. The guard daemon runs parallel to (not inside) the existing WS event loop, connected via an `mpsc::Sender<AgentMessage>` channel that the event loop already drains for other outbound messages. This pattern is identical to `self_monitor.rs`.

**Major components:**
1. `rc-agent/src/process_guard.rs` (NEW) — pod guard module: `spawn()`, `run_scan()`, `enforce()`, `audit_autostart()`, `audit_ports()`; connects to server via existing WS AgentMessage channel
2. `racecontrol/src/process_guard.rs` (NEW) — server guard module: `ProcessGuardStore`, `WhitelistConfig`, three new HTTP endpoints (`/api/v1/guard/whitelist/{id}`, `/api/v1/guard/violations`, `/api/v1/guard/audit`)
3. `rc-common/src/protocol.rs` (MODIFIED) — three new messages: `AgentMessage::ProcessViolation`, `AgentMessage::ProcessGuardStatus`, `CoreToAgentMessage::UpdateProcessWhitelist`
4. `rc-common/src/types.rs` (MODIFIED) — `MachineWhitelist`, `ViolationType`, `ProcessViolation` structs
5. `racecontrol.toml` (MODIFIED) — `[process_guard]` section with global whitelist and `[process_guard.overrides.james]`, `[process_guard.overrides.pod]`, `[process_guard.overrides.server]` sections

**Key architectural decisions:**
- Whitelist fetched on WS connect (not polled separately) — avoids timer complexity and races with server-push updates
- All WinAPI blocking calls (`TerminateProcess`, registry reads, `QueryFullProcessImageName`) wrapped in `tokio::task::spawn_blocking`
- James workstation covered by rc-process-guard standalone binary (HTTP reporter) not rc-agent — standing rule #2
- Do not merge guard into `kiosk.rs` — different lifecycle (always-on vs session-scoped), different whitelist source, different machine coverage

### Critical Pitfalls

1. **Keyword-scoped audit instead of whitelist inversion** — the exact cause of the triggering incident. Guard logic must be `running - whitelist = violations`, never a blocklist. Deny-by-default from day one in the schema.

2. **PID reuse race kills the wrong process** — between snapshot and `TerminateProcess()` call, PID may be reused by rc-agent itself. Prevention: open handle, verify process name + creation time via `GetProcessTimes()`, call `TerminateProcess()` only if both match. Kill by PID alone is incorrect.

3. **Self-kill — guard terminates itself or its parent** — `rc-agent.exe` not on the whitelist (typo or case mismatch) causes the guard to kill its own containing process. Prevention: `std::process::id()` and `std::env::current_exe()` excluded unconditionally before any whitelist lookup. All name comparisons use `eq_ignore_ascii_case()`.

4. **Watchdog restart storm survives kill** — the voice assistant watchdog was an infinite restart loop. Process-level kill without auto-start removal is futile. Prevention: kill sequence must check auto-start sources first. Storm detection: if same process killed 3 times in 60 seconds, suppress further kills and escalate to "auto-start audit required."

5. **Auto-start entry removed without per-machine context** — the leaderboard Run key was on a pod instead of the server. Removing it silently breaks the leaderboard with no recovery path. Prevention: per-machine whitelist sections in TOML; auto-start enforcement defaults to LOG → ALERT → REMOVE (not immediate remove); backup file written before any removal.

6. **Killing transient system processes during Windows Update** — `MpCmdRun.exe`, `TiWorker.exe`, `msiexec.exe` are short-lived, legitimate, and catastrophic to kill mid-run. Prevention: two-cycle grace period (must be flagged on two consecutive scans before kill); system path prefix rule (processes under `C:\Windows\System32\` default to ALERT-only); never kill children of `TrustedInstaller.exe`.

---

## Implications for Roadmap

Based on the combined research, the build order is dictated by compile-time dependencies: rc-common changes must exist before either racecontrol or rc-agent can import the new types. Server endpoints must be live before pods can fetch the whitelist. Pod canary on Pod 8 before fleet rollout is the standing deploy rule.

### Phase 1: Protocol Foundation (rc-common)

**Rationale:** `rc-common` is a shared library dependency of both racecontrol and rc-agent. New protocol types and message variants must compile cleanly before either binary can reference them. This phase has no runtime deployment — library only. Zero risk to production.
**Delivers:** `MachineWhitelist` struct, `ViolationType` enum, `ProcessViolation` struct in `rc-common/src/types.rs`. Three new message variants in `rc-common/src/protocol.rs`: `AgentMessage::ProcessViolation`, `AgentMessage::ProcessGuardStatus`, `CoreToAgentMessage::UpdateProcessWhitelist`.
**Addresses:** Foundational types for all subsequent phases.
**Avoids:** Compiler dependency failures that would block parallel server + agent work.

### Phase 2: Whitelist Schema + Config

**Rationale:** The whitelist schema is the most consequential design decision in the entire milestone. Getting deny-by-default right, per-machine sections right, and the `allowed_path_prefix` / `startup_delay_s` / `allowed_machines` fields right prevents six of the twelve documented pitfalls before a single line of enforcement code is written. Schema changes here determine whether the guard is correct by construction or correct by luck.
**Delivers:** `ProcessGuardConfig` struct in rc-agent `config.rs` and racecontrol `config.rs`. `[process_guard]` section in `racecontrol.toml` with global whitelist, per-machine override tables, `violation_action = "kill_and_report"` initially set to `"report_only"` for safe rollout. Config version hash field for sync verification. `disable_process_guard` boolean in `AgentConfig` for day-one rollback.
**Addresses:** TS-1 (central whitelist schema), pitfalls 1, 5, 7, 8, 9, 11, 12.
**Avoids:** Pitfall 1 (deny-by-default schema), Pitfall 8 (allowed_machines per entry), Pitfall 11 (startup_delay_s), Pitfall 12 (allowed_path_prefix).

### Phase 3: Server Side (racecontrol)

**Rationale:** HTTP endpoints must be live before pods attempt to fetch the whitelist during their first WS connect post-deploy. Deploy server first, smoke-test whitelist endpoint with curl, then build agents. Server can receive violations before the violation sender exists — safe to deploy early.
**Delivers:** `guard_store: Arc<RwLock<ProcessGuardStore>>` in `racecontrol/src/state.rs`. `racecontrol/src/process_guard.rs` with whitelist merge logic and HTTP handlers. New WS handler arm for `AgentMessage::ProcessViolation`. Routes registered in `racecontrol/src/main.rs`. Build and deploy to server .23.
**Uses:** `winreg 0.55`, `netstat2 0.11`, `walkdir 2` added to racecontrol Cargo.toml.
**Avoids:** Pitfall 5 (server records violation context for per-machine audit trail).

### Phase 4: Pod Agent Module (rc-agent)

**Rationale:** rc-agent needs server endpoints live (Phase 3) for whitelist fetch on WS connect. The guard module is the core enforcement engine — process scan, auto-kill, Run key audit, Startup folder audit. Canary on Pod 8 per standing deploy rules before fleet rollout. Initial deployment with `violation_action = "report_only"` in config to tune whitelist before enabling kills.
**Delivers:** `rc-agent/src/process_guard.rs` with `spawn()`, `run_scan()`, `enforce()`, `audit_autostart()`, `audit_ports()`. `guard_whitelist: Arc<RwLock<MachineWhitelist>>` in `AppState`. Whitelist fetch on WS connect in `main.rs`. `CoreToAgentMessage::UpdateProcessWhitelist` handler in `ws_handler.rs`. Guard background task spawned from `main.rs`. Two-cycle grace period, self-exclusion, storm detection, PID+name+creation-time triple for all kills.
**Addresses:** TS-2, TS-3, TS-4, TS-5, TS-8, TS-9, TS-10, TS-11, TS-12.
**Avoids:** Pitfalls 2, 3, 4, 6, 9 (grace period, self-exclusion, storm detection, PID-triple verification).

### Phase 5: Port Audit + Scheduled Tasks (Polish)

**Rationale:** Port audit and Scheduled Task audit are secondary enforcement layers. Port audit catches what process name matching misses (dev vs prod server via port conflict, crypto miners on non-standard ports). Scheduled Tasks is the third auto-start vector and the most dangerous (can run as SYSTEM). Both deferred from MVP to avoid scope creep — process + Run key + Startup folder coverage addresses the three known incidents.
**Delivers:** `audit_ports()` implementation using `netstat2::get_sockets_info()` in `process_guard.rs`. Scheduled Task audit via `schtasks /query /fo CSV` shell-out. Port whitelist section added to `racecontrol.toml`. Violation reporting for both new vectors via existing `ProcessViolation` message.
**Addresses:** TS-6 (Scheduled Tasks), TS-7 (port audit), pitfall 7 (dev+prod server simultaneously).
**Avoids:** Pitfall 7 (port audit catches node.exe dev server on pod even if process name matches).

### Phase Ordering Rationale

- Phase 1 first: compiler hard dependency. No rc-agent or racecontrol code can reference new types until rc-common compiles.
- Phase 2 before any enforcement code: the schema determines correctness. Retrofitting per-machine fields or path verification after enforcement is live creates a window where the guard is running with an incomplete model.
- Phase 3 before Phase 4: pods need to fetch the whitelist on WS connect. The endpoint must exist before agents deploy.
- Phase 4 deploys with `report_only` mode: allows whitelist tuning against real fleet process inventory before kills are enabled. Prevents false-kill incidents during rollout.
- Phase 5 last: secondary enforcement vectors that do not block the core guard from functioning.

### Research Flags

Phases needing deeper research or verification during planning:
- **Phase 4 (Pod canary):** Benchmark test required on an active gaming pod. Scan cycle must complete in under 20ms. This must be measured, not assumed. If WMI was inadvertently used anywhere, it will manifest here.
- **Phase 4 (Whitelist population):** The initial `racecontrol.toml` global_whitelist and pod override sections need to be populated from the actual running process inventory on all 8 pods. Suggest a one-time audit script (`sysinfo` dump) run before Phase 4 ships to capture the full legitimate process set.

Phases with well-established patterns (skip additional research):
- **Phase 1:** Protocol extension is a solved pattern — 15+ existing `AgentMessage` variants in `rc-common/src/protocol.rs` to follow.
- **Phase 2:** TOML config schema follows the identical pattern as every other `[section]` in `racecontrol.toml`.
- **Phase 3:** HTTP endpoint registration and `ProcessGuardStore` pattern are identical to `FleetHealthStore` in `fleet_health.rs`.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All dep versions verified against actual Cargo.toml files in the repo. Two new crates (netstat2, walkdir) confirmed on crates.io. sysinfo 0.33 API confirmed in existing source files. |
| Features | HIGH | Feature set sourced from direct codebase audit of kiosk.rs, self_heal.rs, fleet_health.rs, protocol.rs, and the trigger incident record. MVP is a direct response to known failures — no speculative requirements. |
| Architecture | HIGH | Build order derived from compiler dependency graph. All integration points (event_loop.rs, AppState, ws_handler.rs) read directly. Anti-patterns confirmed against existing codebase. |
| Pitfalls | HIGH | 12 pitfalls documented. All three trigger incidents (Steam, leaderboard Edge, watchdog.cmd) are confirmed real events. WMI performance pitfall confirmed in Microsoft Learn docs. PID reuse race is well-documented OS behavior. |

**Overall confidence:** HIGH

### Gaps to Address

- **Initial whitelist population:** The research defines the schema but does not enumerate every legitimate process on all 8 pods. A one-time inventory run (sysinfo dump via fleet exec endpoint) is needed before Phase 4 goes to production. Without it, the whitelist will have false positives that generate kills on legitimate processes.

- **James whitelist scope:** `CLAUDE.md` and `STATE-v12.1.md` confirm James runs Ollama, node, python, comms-link, deploy tooling, and VS Code. The `[process_guard.overrides.james]` section needs to enumerate all of these explicitly. The rc-process-guard binary on James will use this section. Verify completeness before enabling enforcement on James.

- **schtasks CSV output parsing:** Scheduled Task audit (Phase 5) relies on `schtasks /query /fo CSV` output parsing. The CSV column layout is stable since Windows 7 but the exact task names for the venue's own scheduled tasks (Kiosk, WebDashboard) need to be confirmed from the server .23 before populating the allowed list.

- **Port whitelist for pods:** Pod UDP telemetry ports (9996, 20777, 5300, 6789, 5555) and rc-agent HTTP (8090) need to be in the pod port whitelist. Confirm all ports against `CLAUDE.md` service table before Phase 5 ships.

---

## Sources

### Primary (HIGH confidence — direct source inspection)

- `crates/rc-agent/src/kiosk.rs` — `ALLOWED_PROCESSES` (60+ entries), `sysinfo` usage, `ProcessApprovalRequest` flow
- `crates/rc-agent/src/self_heal.rs` — `winreg` Run key read/write patterns
- `crates/rc-agent/src/self_monitor.rs` — background daemon pattern, log rotation constants
- `crates/rc-agent/src/config.rs` — `AgentConfig` schema, TOML section design
- `crates/rc-agent/src/game_process.rs` — `taskkill` invocation, process name constants
- `crates/racecontrol/src/fleet_health.rs` — `FleetHealthStore` extension points
- `crates/rc-common/src/protocol.rs` — all existing `AgentMessage` and `CoreToAgentMessage` variants
- `crates/rc-agent/Cargo.toml` — sysinfo 0.33, winapi 0.3 features confirmed
- `crates/racecontrol/Cargo.toml` — sysinfo 0.33 confirmed, no existing windows-only dep block
- `Cargo.toml` (workspace) — tokio, tracing, serde, toml, anyhow versions confirmed
- `.planning/PROJECT.md` — v12.1 milestone context, trigger incident
- `CLAUDE.md` / `STATE-v12.1.md` — standing rule #2, incident origin, static IP assignments

### Secondary (HIGH confidence — official documentation)

- [winreg on crates.io](https://crates.io/crates/winreg) — 0.55.0 confirmed latest (2025-01-12)
- [netstat2 on crates.io](https://crates.io/crates/netstat2) — 0.11.2 current; `GetExtendedTcpTable` confirmed
- [TerminateProcess Win32 docs](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess) — fallback kill path
- [WMI performance troubleshooting](https://learn.microsoft.com/en-us/troubleshoot/windows-server/system-management-components/scenario-guide-troubleshoot-wmi-performance-issues) — Microsoft Learn
- [MITRE ATT&CK T1547.001](https://attack.mitre.org/techniques/T1547.001/) — Startup folder + Run key paths

### Tertiary (MEDIUM confidence — corroborating)

- [PID reuse race condition](https://access.redhat.com/solutions/30695) — Red Hat; Linux origin but PID reuse is OS-agnostic
- [Windows Update svchost/wuauserv crash confirmation](https://www.windowslatest.com/2025/04/30/microsoft-confirms-windows-11-24h2-0x80240069-svchost-exe_wuauserv-crashes/) — Windows Latest

---

*Research completed: 2026-03-21 IST*
*Ready for roadmap: yes*
