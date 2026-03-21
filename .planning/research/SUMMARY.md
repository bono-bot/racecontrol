# Project Research Summary

**Project:** v11.2 RC Sentry AI Debugger
**Domain:** Crash diagnostics as external watchdog in an existing Rust binary (rc-sentry) on Windows sim racing pods
**Researched:** 2026-03-21 IST
**Confidence:** HIGH

## Executive Summary

The v11.2 RC Sentry AI Debugger solves a fundamental gap: rc-agent's crash diagnostics die with rc-agent. When the agent crashes, every restart is blind. The fix is to move the crash analysis loop into rc-sentry — the external survivor binary already running on port 8091. rc-sentry runs pure `std` (no tokio, no async) and this architecture must be preserved. The watchdog is a single background `std::thread` that polls `localhost:8090/health` every 5 seconds; crash detection drives a 4-tier fix sequence (deterministic → pattern memory → Ollama → escalation) before restarting rc-agent. Zero new crate dependencies are needed beyond `chrono` and `anyhow`, both already workspace-locked.

The recommended approach follows the existing codebase patterns precisely: port `DebugMemory` and fix patterns from `rc-agent/src/ai_debugger.rs`, duplicate the struct definitions into `rc-sentry/src/debug_memory.rs` (copy, not shared dep — avoids pulling tokio into rc-sentry), and implement all external HTTP calls (health poll, Ollama query, fleet report) as raw `std::net::TcpStream` calls. The anti-cheat constraint (F1 25 EAC and iRacing EOS) is the highest-stakes risk: health endpoint polling via TCP is completely safe; any process inspection API (`OpenProcess`, `CreateToolhelp32Snapshot`, `VirtualQueryEx`) is a potential ban trigger. The entire detection mechanism must use HTTP polling exclusively.

The critical operational risks are: restart loops (solved with the existing `EscalatingBackoff` from rc-common), false-positive crash detection from transient load spikes (solved with N=3 consecutive poll failures before declaring crashed), and stale TCP sockets blocking rc-agent restart after kill (solved with a post-kill port readiness check before restart). All three mitigations are first-class design requirements, not post-launch improvements.

---

## Key Findings

### Recommended Stack

rc-sentry is a pure-std no-tokio binary and must stay that way. The "no new crate dependencies" constraint is satisfied: all required capabilities use either existing workspace crates or `std`. Two workspace deps (`chrono`, `anyhow`) are added to `crates/rc-sentry/Cargo.toml` — both already locked in `Cargo.lock` from other workspace crates. The Ollama HTTP call and fleet report are raw `std::net::TcpStream` posts, not reqwest. The watchdog loop is `std::thread::spawn` with `std::time::Duration::from_secs(5)` sleep, not tokio.

**Core technologies:**
- `std::thread` + `std::net::TcpStream` — watchdog loop and all HTTP calls — zero async overhead, matches existing rc-sentry threading model
- `serde` / `serde_json` (workspace) — crash log parsing and debug-memory.json read/write — already in rc-sentry
- `chrono` (workspace) — RFC3339 timestamps in `DebugIncident.last_seen` — NEW to rc-sentry Cargo.toml, already workspace-locked
- `anyhow` (workspace) — error propagation in watchdog and log analysis — NEW to rc-sentry Cargo.toml, already workspace-locked
- `rc-common::exec::run_cmd_sync` — Tier 1 fix shell commands (taskkill, netsh, etc.) — already used in rc-sentry's `/exec` handler
- `std::process::Command` — restart rc-agent via `start-rcagent.bat` — anti-cheat safe (command execution, not process inspection)

**What NOT to add:** tokio (requires async runtime init, breaks std-thread model), reqwest (bundles mini tokio runtime, adds 800KB), ollama-rs (wraps reqwest/tokio), any WinAPI process inspection function.

### Expected Features

The feature set divides cleanly into a fully-specified MVP and two deferred layers. All MVP features are interdependent — TS-1 is the trigger for everything; nothing else activates without crash detection.

**Must have (v11.2 core — all P1):**
- TS-1: Health endpoint polling (`localhost:8090/health` every 5s, 2+ consecutive failures = crash) — anti-cheat safe
- TS-2: Post-crash log reading (startup_log, stderr capture, rc-bot-events.log from known disk paths)
- TS-3: Tier 1 deterministic fixes before restart (kill zombie rc-agent, clean stale sockets, repair config, clear shader cache)
- TS-4: Crash pattern memory — read `debug-memory.json` for instant fix replay (skip Ollama if match with success_count > 0)
- TS-5: Tier 3 Ollama query via blocking std TCP POST to James `.27:11434` — 30s timeout, graceful fail if unreachable
- TS-7: Escalation FSM — track restart count; after 3 restarts in 10 min, alert staff + show maintenance screen instead of retrying
- TS-9: Append to `C:\RacingPoint\crash-sentry.log` at every state transition, 512KB rotation
- D-3: Hysteresis (N=3 consecutive poll failures before fix sequence) — prevents false restarts during shader compilation spikes

**Should have (v11.2 polish — P2):**
- TS-6: Report crash diagnostic to server fleet API (`POST /api/v1/sentry/crash`) — closes observability loop
- D-1: Write successful fix back to debug-memory.json — closes the learning loop with rc-agent's pattern memory
- D-5: Log classification (config crash vs OOM vs port conflict) before Tier 1 — reduces Ollama calls
- D-7: Configurable via `[sentry]` section in rc-agent.toml (poll interval, Ollama URL, escalation threshold)

**Defer to v11.3+ (P3):**
- D-4: PodStateSnapshot at crash time (requires rc-agent panic hook change — separate phase)
- D-6: Crash reason field in fleet health dashboard (requires racecontrol schema change)
- D-2: Structured stderr capture from bat file (ops deploy step, not code change)

**Anti-features (do not build):**
- Process inspection of game PIDs (OpenProcess, debug APIs) — EAC ban risk
- Minidump analysis — requires symbol servers, marginal gain over log parsing
- Windows Event Log subscription — COM overhead, not worth complexity
- Restart loop without exponential backoff — worse than blocking the pod and alerting staff
- Game-crash debugging in sentry — rc-agent is alive for those; dual ownership is confusing

### Architecture Approach

rc-sentry gains a single background `std::thread` (watchdog_thread) spawned at `main()` startup alongside the existing TCP accept loop. The watchdog thread runs the health poll FSM, drives the crash analysis pipeline, and coordinates fix tiers. Five new source files are created in `crates/rc-sentry/src/`. Two existing files in rc-common and two in racecontrol are extended. No new crate, no new binary, no tokio added to rc-sentry.

**Major components:**
1. `watchdog_thread` (rc-sentry/main.rs) — FSM driver: Healthy → Suspect(n) → Crashed → fix tiers → restart → verify
2. `crash_logs.rs` (NEW) — pure file reads of startup_log, stderr, rc-bot-events.log; returns `CrashContext`
3. `debug_memory.rs` (NEW) — DebugMemory load/save/query (copied struct from rc-agent, same JSON format, no shared dep)
4. `tier1_fixes.rs` (NEW) — deterministic fix functions behind `#[cfg(test)]` guards; kill zombie, clean sockets, repair config, clear shader cache
5. `ollama.rs` (NEW) — blocking HTTP POST to James `.27:11434`, 30s timeout, fails gracefully
6. `fleet_reporter.rs` (NEW) — blocking HTTP POST to racecontrol, 10s timeout, fire-and-forget
7. `rc-common/src/types.rs` (MODIFIED) — `SentryCrashReport` struct, `CrashDiagResult` enum
8. `racecontrol/src/fleet_health.rs` + `api/routes.rs` (MODIFIED) — `POST /api/v1/sentry/crash` endpoint, `last_sentry_crash` field in `FleetHealthStore`
9. `start-rcagent.bat` (MODIFIED) — add `2>> C:\RacingPoint\rc-agent-stderr.log` and `set RUST_BACKTRACE=1` — prerequisite for log parsing

**Build order:** rc-common types first, then rc-sentry watchdog modules (testable in isolation), then racecontrol endpoint, then bat file change, then Pod 8 canary integration test.

### Critical Pitfalls

1. **Anti-cheat ban from process inspection** — EAC scans for handles to the game process; `OpenProcess()`, `CreateToolhelp32Snapshot(TH32CS_SNAPMODULE)`, and `VirtualQueryEx()` on game PIDs can trigger flags. Prevention: health endpoint polling only — never add any supplementary crash detection that uses WinAPI on game PIDs. No dead code paths to these APIs should exist in the binary.

2. **Restart loop without backoff** — 10 restarts/minute on persistent crash hammers Ollama, corrupts billing state, creates ghost billing records. Prevention: use existing `EscalatingBackoff` from rc-common (already there from v1.0). Block pod and alert staff after 3 restarts in 5 minutes. Reset counter only after 2 minutes of stable health.

3. **Log timing race — zombie holds file handles** — crash logs may be inaccessible if the zombie rc-agent process still holds `GENERIC_WRITE` on the log file. Prevention: Tier 1 sequence must be kill zombie first → wait 500ms → read logs → apply other fixes → restart. Not read-then-kill.

4. **Pattern memory JSON corruption from concurrent write** — direct `std::fs::write()` truncates before writing; crash during write = zero-byte file. Prevention: write to `debug-memory.json.tmp` then `std::fs::rename()` (atomic on same Windows volume via `MoveFileExW`). Load into in-memory struct on startup; write-through on mutation.

5. **Ollama cold start blocks restart sequence** — model unloads after idle period (default 5 min `OLLAMA_KEEP_ALIVE`); cold start is 10-30s on RTX 4070. Prevention: Ollama query must never block the restart path. Hard 30s `set_read_timeout` on TcpStream; if timeout hits, proceed with restart and record Ollama as `None`. Restart latency target: under 10 seconds regardless of Ollama state.

6. **Stale TCP socket blocks port 8090 after kill** — taskkill releases the process but port stays in TIME_WAIT (Windows default 2 min). rc-agent fails to bind on restart and sentry counts a second crash. Prevention: after zombie kill, poll `netstat -ano` in a loop (max 10s) for port 8090 to clear before launching restart.

7. **Panic output silently discarded** — `start-rcagent.bat` does not redirect stderr; panic output is lost. Prevention: bat file update is a prerequisite task, not a follow-up. Add `set RUST_BACKTRACE=1` and `2>> C:\RacingPoint\rc-agent-stderr.log` before any log analysis code is tested.

8. **Single poll timeout triggers false crash during game load** — shader compilation and audio driver stalls can cause 3-8s unresponsive windows. Prevention: require N=3 consecutive poll failures (15s silence) before any Tier 1 action. Defined before implementation, not tuned later.

---

## Implications for Roadmap

Based on the combined research, the build order is determined by two constraints: compile-time (rc-common types before either binary imports them) and operational (racecontrol endpoint live before sentry tries to POST reports). The canary-first deploy rule (Pod 8 first) applies to the final integration phase.

### Phase 1: Types Foundation (rc-common)

**Rationale:** `SentryCrashReport` and `CrashDiagResult` must compile in rc-common before racecontrol or rc-sentry can reference them. Library-only change, zero production risk.
**Delivers:** `SentryCrashReport` struct and `CrashDiagResult` enum in `rc-common/src/types.rs`.
**Addresses:** Compile-time unblocking for all subsequent phases.
**Avoids:** Dependency failures that would require replanning.

### Phase 2: rc-sentry Watchdog Core

**Rationale:** All new rc-sentry modules are self-contained and testable without a running rc-agent. `crash_logs.rs` and `debug_memory.rs` can be unit tested with temp files. `tier1_fixes.rs` uses `#[cfg(test)]` guards so tests never fire real taskkill. The full watchdog pipeline (health poll FSM + fix tiers + restart + logging) can be built and tested before the server endpoint exists.
**Delivers:** `crash_logs.rs`, `debug_memory.rs`, `tier1_fixes.rs`, `ollama.rs`, `fleet_reporter.rs` modules. `watchdog_loop()` function and `WatchdogState` FSM in `main.rs`. `WATCHDOG_SHUTDOWN` AtomicBool. `crash-sentry.log` writes.
**Implements:** TS-1, TS-2, TS-3, TS-4, TS-5, TS-7, TS-9, D-3 (hysteresis)
**Avoids:** Pitfalls 2 (backoff), 3 (log timing), 4 (atomic JSON write), 5 (Ollama timeout), 6 (port readiness), 8 (N-consecutive threshold)

### Phase 3: Server Endpoint (racecontrol)

**Rationale:** The fleet reporter in rc-sentry tries to POST to `192.168.31.23:8080/api/v1/sentry/crash`. This endpoint must exist and be deployed before the sentry is tested end-to-end. Deploy server first, smoke-test with curl, then proceed to integration testing.
**Delivers:** `FleetHealthStore::last_sentry_crash` field. `POST /api/v1/sentry/crash` handler in `fleet_health.rs`. Route registered in `api/routes.rs`. WS broadcast of `DashboardEvent::PodCrashDiagnostic` to staff kiosk. WhatsApp alert via `whatsapp_alerter.rs` when `restart_verified = false`.
**Uses:** `SentryCrashReport` from rc-common (Phase 1).
**Avoids:** Pitfall related to fire-and-forget blocking (10s timeout, never blocks restart).

### Phase 4: bat File + Stderr Capture

**Rationale:** `start-rcagent.bat` must redirect stderr before any end-to-end crash log test can produce meaningful output. One-line change to `self_heal.rs` `START_SCRIPT_CONTENT` ensures self-heal doesn't revert it. Low risk, high value — deploy to Pod 8 and verify `C:\RacingPoint\rc-agent-stderr.log` appears.
**Delivers:** `set RUST_BACKTRACE=1` and `2>> C:\RacingPoint\rc-agent-stderr.log` in bat file. Updated `self_heal.rs` constant to match.
**Addresses:** Pitfall 7 (panic output capture).
**Avoids:** Log analysis always falling through to Ollama because stderr is empty.

### Phase 5: Pod 8 Canary Integration Test + Fleet Rollout

**Rationale:** Deploy updated rc-sentry to Pod 8. Simulate crash via `/exec` (`taskkill /F /IM rc-agent.exe`). Observe full pipeline: 15s crash declaration, log reads, Tier 1 fixes, restart, health verification, fleet report received at server. Verify no EAC-triggering API calls. Roll to remaining 7 pods after canary validates.
**Delivers:** Confirmed working crash detection, diagnosis, and restart pipeline on all 8 pods.
**Addresses:** TS-6 (fleet report verified), all table stakes features end-to-end.
**Avoids:** Pitfall 1 (verify no game process handles in logs during F1 25 session).

### Phase Ordering Rationale

- rc-common first: compiler hard dependency. Both racecontrol and rc-sentry import `SentryCrashReport`.
- rc-sentry watchdog before server endpoint: sentry modules are independently testable; building them first allows the hardest integration work (Ollama, Tier 1 fixes, FSM) to be tested in isolation.
- Server endpoint before integration test: `fleet_reporter.rs` will silently fail if server endpoint doesn't exist, masking a required feature during testing.
- bat file before integration test: without stderr redirect, crash log analysis produces empty results and every test crashes through to Ollama unnecessarily.
- Pod 8 canary last: standing deploy rule; canary validates before fleet exposure.

### Research Flags

Phases needing deeper research or verification during planning:
- **Phase 2 (Tier 1 fixes):** The exact socket cleanup commands for stale port 8090 in TIME_WAIT need verification against the specific Windows 11 version on pods. `netsh int ip delete arpcache` vs `SetTcpEntry()` approach — confirm which works reliably via test on Pod 8 before baking into `tier1_fixes.rs`.
- **Phase 2 (debug-memory.json schema):** The `pattern_key` derivation in rc-sentry differs from rc-agent's (log content hash vs SimType/exit_code). Confirm the key derivation produces stable, collision-resistant keys for log-based patterns before the memory write path is implemented.

Phases with well-established patterns (skip additional research):
- **Phase 1 (rc-common types):** Direct pattern from existing `SentryCrashReport` analogs in `rc-common/src/types.rs`.
- **Phase 3 (racecontrol endpoint):** Identical pattern to `FleetHealthStore` extension in `fleet_health.rs` — 15+ prior examples.
- **Phase 4 (bat file):** One-line change; `self_heal.rs` constant update is a single-file edit with precedent from v4.0 self-heal work.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All dep versions verified against actual `Cargo.toml` files. No new crate lock entries needed — chrono and anyhow already workspace-locked. Implementation patterns verified against existing `ai_debugger.rs` and `self_monitor.rs`. |
| Features | HIGH | Feature set sourced from direct codebase audit of rc-sentry, rc-agent, ai_debugger.rs, self_monitor.rs, and PROJECT.md. Anti-cheat constraints confirmed from EAC/iRacing official sources. MVP is a direct port of proven patterns. |
| Architecture | HIGH | Build order derived from compiler dependency graph. All integration points read from actual source files. Anti-patterns confirmed against existing codebase. 5-phase build order matches the ARCHITECTURE.md's recommended sequence. |
| Pitfalls | HIGH | 9 pitfalls documented with specific prevention steps and phase assignments. EAC kernel scan behavior confirmed from Microsoft Learn and iRacing official docs. Race conditions and timing issues sourced from rust-lang/rust GitHub issues. |

**Overall confidence:** HIGH

### Gaps to Address

- **Tier 1 socket cleanup commands:** The specific `netsh` or `SetTcpEntry` approach for clearing TIME_WAIT on port 8090 needs a live test on Windows 11 22H2/24H2 before baking into `tier1_fixes.rs`. Commands that work on older Windows builds may behave differently under 24H2 network stack changes.

- **Ollama model pre-warm strategy:** `OLLAMA_KEEP_ALIVE` defaults to 5 minutes. If the venue has long idle periods between crashes, every Ollama query will be a cold start. Consider documenting a pre-warm approach (James can set `OLLAMA_KEEP_ALIVE=-1` to keep model loaded indefinitely) as an operational note.

- **`start-rcagent.bat` current state:** ARCHITECTURE.md notes that `start-rcagent.bat` currently does not redirect stderr. Verify the exact current file content before writing the updated `self_heal.rs` constant — the bat file may have changed since last documented.

- **debug-memory.json pattern_key for sentry context:** rc-agent keys on `SimType/exit_code`; rc-sentry must key on log content patterns (no SimType available post-crash). The exact hash/key derivation algorithm should be decided in Phase 2 spec before implementation to ensure memory hits are possible across sentry and agent for the same crash pattern.

---

## Sources

### Primary (HIGH confidence — direct source inspection)

- `crates/rc-sentry/src/main.rs` — confirmed pure-std, thread-per-connection, 6 endpoints, 4-slot concurrency cap
- `crates/rc-agent/src/ai_debugger.rs` — DebugMemory struct, 14 Tier 1 fix patterns, 4-tier debug order, billing gate rule, pattern_key logic
- `crates/rc-agent/src/self_monitor.rs` — health monitoring pattern, relaunch_self(), log_event(), CLOSE_WAIT detection, escalation logic
- `crates/rc-agent/src/startup_log.rs` — log paths, write_phase(), detect_crash_recovery()
- `crates/rc-agent/src/self_heal.rs` — START_SCRIPT_CONTENT, repair patterns
- `crates/rc-common/src/protocol.rs` — AgentMessage variants, StartupReport fields
- `crates/racecontrol/src/fleet_health.rs` — FleetHealthStore, existing crash_recovery field
- `crates/rc-sentry/Cargo.toml` — confirmed current deps: serde, serde_json, toml, tracing, sysinfo 0.33
- `Cargo.toml` (workspace) — confirmed chrono, anyhow, serde_json as workspace deps
- `.planning/PROJECT.md` — v11.2 target features, anti-cheat constraints, milestone state

### Secondary (HIGH confidence — official documentation)

- [EasyAntiCheat kernel driver — Microsoft Learn](https://learn.microsoft.com/en-us/answers/questions/3962392/easy-anti-cheat-driver-incompatible-with-kernel-mo) — confirmed EAC kernel-mode scope
- [iRacing anti-cheat migration to EOS EAC](https://support.iracing.com/support/solutions/articles/31000173103-anticheat-not-installed-uninstalling-eac-and-installing-eos-) — confirmed iRacing uses EOS variant
- [Rust panic hook stderr on Windows](https://github.com/rust-lang/rust/issues/25643) — confirmed stderr behavior on Windows subsystem binaries
- [RUST_BACKTRACE — Rust Book](https://doc.rust-lang.org/book/ch09-01-unrecoverable-errors-with-panic.html) — confirmed requirement for stack traces in panic output
- [reqwest timeout configuration](https://webscraping.ai/faq/reqwest/how-can-i-set-a-request-in-reqwest) — connect_timeout vs total timeout distinction

### Secondary (MEDIUM confidence)

- [EAC continuous kernel scanning behavior](https://forums.ea.com/discussions/apex-legends-technical-issues-en/bug-report-%E2%80%93-easyanticheat-eos-sys-kernel-scan-loop/13034346) — EA Forums
- [Anti-cheat false positives: ASUS Aura Sync flagged](https://gamespace.com/all-articles/news/easy-anti-cheat-download-install/) — confirms legitimate background apps can trigger EAC
- [Ollama KEEP_ALIVE model loading latency](https://markaicode.com/troubleshooting-ollama-tool-execution-timeouts/) — 30-90s cold start confirmed
- WebSearch: reqwest 0.12.x confirmed as latest stable 0.12 series
- WebSearch: Ollama `/api/generate` REST endpoint confirmed stable (stream: false for blocking response)
- WebSearch: Watchdog escalation patterns (interrupt.memfault.com firmware watchdog best practices)

---

*Research completed: 2026-03-21 IST*
*Ready for roadmap: yes*
