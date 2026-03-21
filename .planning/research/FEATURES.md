# Feature Research: RC Sentry AI Debugger

**Domain:** Crash-diagnosing external watchdog for a Windows process (rc-agent) on sim racing pods, with anti-cheat-safe detection, log parsing, pattern memory, LLM triage, and escalation decisions.
**Researched:** 2026-03-21
**Confidence:** HIGH (codebase audit of rc-sentry, rc-agent, ai_debugger, self_monitor; confirmed EAC detection mechanisms from official sources; confirmed iRacing EOS anti-cheat approach; existing 4-tier debug order is direct prior art)

---

## Context: What Already Exists

Before listing features, it matters what the new sentry debugger can build on versus what it must build fresh:

| Already in rc-agent (dies with the patient) | Must move to rc-sentry (external survivor) |
|---------------------------------------------|---------------------------------------------|
| `ai_debugger.rs` — 14 auto-fix patterns, DebugMemory, Ollama query | None of it survives rc-agent crash |
| `self_monitor.rs` — CLOSE_WAIT detection, WS dead detection, relaunch_self() | The relaunch_self() pattern is reusable |
| `debug-memory.json` on disk at C:\RacingPoint\ | File survives crash — sentry can read it |
| `rc-bot-events.log` on disk | File survives crash — sentry can read it |
| `/health` on :8090 (rc-agent's own health endpoint) | Port goes away when rc-agent dies |
| `PodStateSnapshot` — runtime context at crash time | Lost with rc-agent unless serialized to disk |

The sentry already has `/health`, `/processes`, `/files`, `/exec` endpoints and the `rc-common::exec::run_cmd_sync` primitive. It is a pure-std no-async binary running on :8091. Adding a crash analysis loop means adding a background thread (no async needed — the pattern is already std::thread based).

---

## Feature Landscape

### Table Stakes (Operations Expects These)

Features the sentry AI debugger must have. Missing any means rc-agent still restarts blindly — the milestone goal is not met.

| # | Feature | Why Expected | Complexity | Existing Dependency |
|---|---------|--------------|------------|---------------------|
| TS-1 | **Health endpoint polling for rc-agent crash detection** | Without detecting the crash, nothing else can happen. Poll `localhost:8090/health` every 5s. Non-response for 2+ consecutive polls = crashed. | LOW | HTTP client in std (no reqwest — no async in sentry). `std::net::TcpStream::connect_timeout` + basic GET. Pattern: rc-sentry is already pure-std for connections. |
| TS-2 | **Post-crash log reading: startup_log, stderr capture, panic output** | The WHY of a crash lives in the log files. rc-agent already writes startup errors to disk (HEAL-01 through HEAL-04 in v4.0). Without reading these, every restart is still blind. | LOW | `std::fs::read_to_string` on known paths: `C:\RacingPoint\startup_errors.log`, `C:\RacingPoint\rc-agent-stderr.log`, `C:\RacingPoint\rc-bot-events.log`. All paths are constants in the project. `/files` sentry endpoint already exists but internal read is simpler. |
| TS-3 | **Tier 1 deterministic fixes before restart** | rc-agent's own `ai_debugger.rs` Tier 1 has 14 patterns. The most critical ones (stale sockets, zombie processes, config repair, shader cache) must be replicated in sentry so they run even when rc-agent is dead. | MEDIUM | Patterns replicated from `ai_debugger.rs`. Implementation: shell-outs via `rc_common::exec::run_cmd_sync` (already used in sentry's `/exec` handler). Billing gate does NOT apply here — rc-agent is dead, no session in progress. |
| TS-4 | **Crash pattern memory: read debug-memory.json for instant fix replay** | debug-memory.json on disk survives the crash. If rc-agent solved this exact pattern before (success_count > 0), apply the known fix immediately without querying Ollama. | LOW | `std::fs::read_to_string(C:\RacingPoint\debug-memory.json)` + `serde_json` parse. The `DebugMemory` struct and `pattern_key` logic in `ai_debugger.rs` can be extracted to `rc-common` or duplicated with minimal code (it's just JSON read + key lookup). |
| TS-5 | **Tier 3 Ollama query for unknown crash patterns** | Patterns not in debug-memory.json and not matched by Tier 1 keywords need LLM triage. Ollama runs on James (.27:11434), survives pod crashes. Query with crash log excerpt and receive RESTART / FIX / ALERT. | MEDIUM | `std::net::TcpStream` raw HTTP POST to `192.168.31.27:11434/api/generate` — no async needed. Blocking 30s timeout via `set_read_timeout`. Same Ollama model already serving rc-agent queries. |
| TS-6 | **Crash diagnostics reported to server via fleet API** | Server needs to know WHY pod was restarted, not just that it was. The `/api/v1/fleet/health` endpoint and fleet dashboard show pod state. A `CrashDiagnostic` event to the server closes the observability loop. | LOW | HTTP POST to `192.168.31.23:8080` — plain std TCP, same approach as Ollama query. Server logs the event. No new server endpoint needed if it maps to existing `AgentMessage` channel via rc-agent (but rc-agent is dead). Sentry posts directly via HTTP. |
| TS-7 | **Escalation decision: restart vs alert staff vs block pod** | Not every crash should result in a restart loop. A pod crashing 5 times in 10 minutes signals a hardware or config problem that staff must physically address. Three outcomes: restart (most crashes), alert (repeat crashes), block with maintenance screen. | MEDIUM | Three-state FSM in the sentry crash loop: `restart_count` + `last_crash_at` in memory. If `restart_count >= 3 in 10 min` → email alert + `/exec` to show maintenance screen (the lock_screen HTML approach already used). |
| TS-8 | **Anti-cheat-safe detection: HTTP health polling only, no process inspection** | F1 25 uses Easy Anti-Cheat (EAC). EAC scans for handles to the game process, OpenProcess calls, and memory access from external processes. Health polling via TCP to :8090 does not touch the game process — it is completely invisible to EAC. The sentry already avoids process inspection in its `/processes` endpoint (it uses sysinfo for reporting, not game debugging). | LOW | This is a constraint, not a feature to build, but it must be explicitly honored: the crash detection loop MUST NOT call OpenProcess/TerminateProcess on game PIDs, MUST NOT call `EnumProcessModules`, MUST NOT attach debug events. Only allowed: HTTP polling localhost:8090, reading log files from disk, running shell commands (netstat, taskkill by name — not by PID attachment). |
| TS-9 | **Write crash diagnostic to crash-sentry.log on disk** | Every crash event (detected, logs read, fixes applied, outcome) must be persisted locally. This log survives the sentry's own restarts and enables post-mortem by Uday or James. | LOW | Append to `C:\RacingPoint\crash-sentry.log` with 512KB rotation. Same `log_event()` pattern from `self_monitor.rs` — replicate in sentry. Structured line: `[epoch_secs] CRASH_DETECTED | fixes=stale_socket,restart | outcome=restart | ollama=RESTART`. |

### Differentiators (Beyond the Mandatory Floor)

Features that make the crash-diagnosing watchdog significantly better without being required for the core mission.

| # | Feature | Value Proposition | Complexity | Notes |
|---|---------|-------------------|------------|-------|
| D-1 | **Write debug-memory.json from sentry after successful fix** | Today, only rc-agent writes debug-memory.json (after a successful fix). If sentry fixes a crash pattern, it should record that fix in debug-memory.json so future crashes (whether handled by sentry or rc-agent) can use it. Closes the learning loop. | LOW | `std::fs::write` with atomic rename pattern (already in `ai_debugger.rs`). Sentry reads the same JSON format. |
| D-2 | **Capture rc-agent stderr at launch: redirect output to file before rc-agent starts** | Currently rc-agent is started from `start-rcagent.bat`. If that bat file pipes stderr to a file, sentry has structured error output to parse. The sentry can check if the bat file already does this and add the redirect if not. | MEDIUM | Modifying the bat file is an operational deploy step, not a code change. Sentry can check if `rc-agent-stderr.log` exists and is non-empty as a signal of previous crash details. Document the required bat file change. |
| D-3 | **Hysteresis on crash detection: require 2+ consecutive polling failures before acting** | Single poll failure can be network blip (Windows Defender scan locks port briefly). Requiring 2 consecutive failures (within 10s total) eliminates false-positive restarts that interrupt customer sessions. | LOW | Counter variable in the poll loop: `consecutive_failures`. Reset to 0 on success. Trigger analysis only when `>= 2`. |
| D-4 | **Pass last known PodStateSnapshot to Ollama query** | rc-agent serializes a `PodStateSnapshot` to disk on panic (if the panic hook writes it). If this file exists at crash time, pass it as context to Ollama. Better context = more accurate fix suggestion. | MEDIUM | Requires rc-agent panic hook to write `C:\RacingPoint\last-crash-state.json`. Sentry reads this file if it exists and appends to the Ollama prompt. Richer context (billing_active, game_pid, ws_connected at time of crash). |
| D-5 | **Startup error classification: distinguish config crash vs runtime crash vs OOM crash** | Log parsing can classify the crash type before Ollama is consulted. Config parse errors have distinctive messages ("missing field", "invalid TOML"). OOM crashes have distinct Windows Event Log entries. This pre-classification narrows the Tier 1 fix selection before wasting Ollama budget on already-known patterns. | MEDIUM | Keyword matching on log content: config errors → fix_config(), OOM → fix_memory_pressure(), port conflict → fix_stale_socket(). Narrows the search space and may eliminate the Ollama call entirely. |
| D-6 | **Report diagnostic to fleet health dashboard with crash_reason field** | Extend `PodFleetStatus` with `last_crash_reason: Option<String>` and `restart_count_24h: u32`. Staff kiosk can show "Pod 3: restarted 2x today — port conflict" without reading logs. | MEDIUM | HTTP POST from sentry to server. New fields on `PodFleetStatus` struct in rc-common. |
| D-7 | **Configurable via rc-agent.toml section** | The crash analysis poll interval, Ollama URL, escalation threshold, and log paths should be configurable without rebuilding sentry. Sentry reads rc-agent.toml at startup (it shares the same `C:\RacingPoint\` working directory). | LOW | `serde_json` / `toml` parse in sentry's main. Reasonable defaults if config absent. One TOML section: `[sentry]` with `poll_interval_ms`, `ollama_url`, `max_restarts_per_window`, `restart_window_secs`. |

### Anti-Features (Do Not Build)

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Process inspection of game PID (OpenProcess, debug APIs, CreateRemoteThread)** | EAC kernel driver scans for handles to the game process from external processes. Any OpenProcess call on F1 25 or iRacing EOS risks a ban or game crash. iRacing EOS runs a sandbox preventing external program access. This is a hard constraint. | Health poll localhost:8090 only. Shell commands (taskkill by name) are safe because they use the kernel's standard process management API, not the game's process memory. |
| **Minidump analysis (WinDbg, DbgHelp.dll)** | Minidump parsing requires loading DbgHelp.dll with the correct symbol paths. Symbols are game-specific, change every patch, and the game EXE minidumps are not accessible (EAC guards the game process memory). rc-agent minidumps are accessible but setting up symbol servers adds significant operational complexity for marginal gain over log parsing. | Read rc-agent's own text log output and panic messages. Rust panics produce readable stack traces to stderr if `RUST_BACKTRACE=1` is set. Text log parsing is sufficient. |
| **Real-time Windows Event Log subscription** | Subscribing to Windows Event Log for application crashes (EventID 1000/1001) via `EvtSubscribe` requires COM initialization and significant Win32 plumbing. The sentry deliberately avoids async and complex win32 to stay minimal. | Poll a known log file path every 5s. Already covers 99% of the crash information. Event Log polling is not worth the complexity for the marginal case where a crash leaves no disk log. |
| **Restart loop without exponential backoff** | Rapid restart loops during persistent crashes consume CPU, corrupt state, and may trigger Windows service recovery limits. A sentry that restarts rc-agent 20 times in 2 minutes is worse than one that blocks the pod and alerts staff. | Escalating backoff after each restart: 3s, 5s, 10s, 30s. Block pod and alert staff after 3 restarts in 10 minutes. Already validated pattern from rc-agent's own `EscalatingBackoff` in rc-common. |
| **Ollama for every crash without Tier 1 first** | Sending every log to Ollama skips deterministic fixes that always work (stale socket = netsh delete, zombie = taskkill). Ollama adds 5-30s latency. Most crashes have known patterns that resolve in <1s with the right command. | 4-tier debug order: Tier 1 deterministic → Tier 2 memory → Tier 3 Ollama. Only reach Tier 3 if Tiers 1+2 yield no applicable fix. This is the established pattern from `ai_debugger.rs`. |
| **Game-crash debugging in sentry** | rc-agent is alive when a game crashes (it detects and handles game crashes itself). Sentry only activates when rc-agent is dead. Adding game-crash debugging to sentry creates a confusing dual-ownership of the same problem. | Game crash debugging stays in rc-agent's `ai_debugger.rs`. Sentry handles only rc-agent crashes. Scope boundary: sentry = rc-agent dead, rc-agent = game dead. |
| **Cross-pod crash correlation in sentry** | Detecting "3 pods crashed at the same time = server problem" is a valid observation but belongs to racecontrol's `pod_monitor.rs` + `bot_coordinator.rs` on the server, which has fleet-wide visibility. The sentry is a per-pod tool with no cross-pod communication path. | Server-side pod_monitor already detects fleet-wide health drops. Feed sentry crash events to the server (TS-6) and let the server do fleet-level correlation. |

---

## Feature Dependencies

```
TS-1 (health poll: is rc-agent dead?)
    └──triggers──> TS-2 (read crash logs from disk)
                       └──feeds──> D-5 (classify crash type: config/OOM/port)
                                       └──narrows──> TS-3 (Tier 1 deterministic fixes)
                                                         └──if no fix──> TS-4 (debug-memory instant replay)
                                                                             └──if no match──> TS-5 (Ollama Tier 3)
                                                                                                   └──outcome feeds──> TS-7 (escalation FSM)

TS-7 (escalation: restart vs alert vs block)
    └──restart outcome──> rc-agent.exe launched via existing relaunch_self() pattern
    └──alert outcome──> TS-6 (report to server) + email alert
    └──block outcome──> /exec to show maintenance lock screen on pod

D-3 (hysteresis 2+ failures)
    └──gates──> TS-1 (prevents false-positive crash detection)

D-1 (write debug-memory.json after fix)
    └──requires──> TS-3 (a fix was applied and succeeded)
    └──requires──> TS-4 (pattern key derived from crash content)

D-4 (pass PodStateSnapshot to Ollama)
    └──requires──> rc-agent panic hook writing last-crash-state.json (rc-agent code change)
    └──enhances──> TS-5 (richer Ollama context)

TS-9 (crash-sentry.log write)
    └──written at──> every stage: crash detected, logs read, fix applied, outcome
```

### Dependency Notes

- **TS-1 is the trigger for everything**: Nothing activates until health poll detects rc-agent is down. This is the critical path — poll interval and hysteresis (D-3) must be set correctly before any other feature matters.
- **TS-3 Tier 1 fixes require careful port between rc-agent and sentry**: The billing gate from `ai_debugger.rs` ("don't kill game if billing active") does NOT apply in sentry — rc-agent is already dead, so billing has already been interrupted. The sentry fixes run unconditionally.
- **TS-5 Ollama is a pure-std blocking HTTP call in sentry**: Unlike rc-agent which uses reqwest (async), sentry must do this via raw TCP or a blocking HTTP client. The constraint is sentry's no-async architecture. Implementation: `std::net::TcpStream` + manual HTTP/1.1 POST. Max 30s timeout via `set_read_timeout`.
- **D-4 requires a rc-agent code change (panic hook writes JSON)**: This is a cross-binary dependency. The sentry can still work without it (just queries Ollama without snapshot context). D-4 is enhancement, not blocking.
- **Anti-cheat constraint (TS-8) is architectural**: It gates how TS-1 is implemented (HTTP poll only) and how TS-3 fixes are applied (taskkill by name, not by PID attachment). This constraint flows through every feature that touches processes.

---

## MVP Definition

### Launch With (v11.2 core)

The minimum sentry that solves the problem: rc-agent crashes → sentry reads why → sentry fixes or reports → rc-agent restarts with context.

- [ ] **TS-1** — Health poll loop on :8090 every 5s, 2 consecutive failures = crash detected (D-3 hysteresis included from day 1 — trivial to add and prevents false positives during customer sessions).
- [ ] **TS-2** — Read startup_log + stderr capture + rc-bot-events.log from known disk paths. Truncate to last 8KB for Ollama prompt budget.
- [ ] **TS-3** — Tier 1 deterministic fixes: stale sockets (`netsh int ip delete arpcache`, close port-in-use), zombie processes (`taskkill /F /IM rc-agent.exe`), config repair (copy backup toml if main is corrupt/empty), shader cache clear (known NVIDIA DXCache path).
- [ ] **TS-4** — Read debug-memory.json, extract `pattern_key` from crash log text, apply cached fix if match found with `success_count > 0`.
- [ ] **TS-5** — Ollama query via blocking std TCP POST to James :11434. 30s timeout. Prompt includes crash log excerpt + rc-agent version + crash count in window. Parse RESTART / FIX_X / ALERT from response.
- [ ] **TS-7** — Escalation FSM: track restart_count + timestamps. After 3 restarts in 10 minutes: email alert via `send_email.js` shell-out + `/exec` to show maintenance screen. Reset counter after 30 minutes of stable operation.
- [ ] **TS-9** — Append to `C:\RacingPoint\crash-sentry.log` at every state transition. 512KB rotation.

### Add After Validation (v11.2 polish)

- [ ] **TS-6** — Report crash diagnostic to server fleet API once server-side handler is confirmed working.
- [ ] **D-1** — Write successful fix back to debug-memory.json (closes the learning loop with rc-agent's memory).
- [ ] **D-5** — Log classification (config vs OOM vs port) to narrow Tier 1 before Ollama.
- [ ] **D-7** — Sentry configurable via `[sentry]` section in rc-agent.toml.

### Future Consideration (v11.3+)

- [ ] **D-4** — PodStateSnapshot at crash time (requires rc-agent panic hook change — separate phase).
- [ ] **D-6** — Crash reason field in fleet health dashboard (requires racecontrol schema change).
- [ ] **D-2** — Structured stderr capture from rc-agent (requires bat file change + ops deploy).

---

## Feature Prioritization Matrix

| Feature | Operational Value | Implementation Cost | Priority |
|---------|-------------------|---------------------|----------|
| TS-1 Health poll (crash detection) | HIGH — without this, nothing works | LOW — std TcpStream | P1 |
| TS-2 Post-crash log read | HIGH — tells us WHY | LOW — std::fs::read | P1 |
| TS-3 Tier 1 deterministic fixes | HIGH — resolves 70%+ of crashes instantly | MEDIUM — port patterns from ai_debugger | P1 |
| TS-4 Pattern memory (debug-memory.json) | HIGH — instant fix replay, no Ollama needed | LOW — JSON read + key lookup | P1 |
| TS-7 Escalation FSM | HIGH — prevents restart loops, protects customers | MEDIUM — state machine + email | P1 |
| TS-9 crash-sentry.log | HIGH — operational record, post-mortem | LOW — append + rotate | P1 |
| D-3 Hysteresis (2 failures) | HIGH — prevents false restart during game session | LOW — counter only | P1 |
| TS-5 Ollama Tier 3 | MEDIUM — needed for unknown patterns | MEDIUM — blocking std TCP HTTP | P1 |
| TS-6 Report to server | MEDIUM — visibility | LOW — HTTP POST | P2 |
| D-1 Write debug-memory.json | MEDIUM — closes learning loop | LOW — JSON write | P2 |
| D-5 Crash classification | MEDIUM — reduces Ollama calls | MEDIUM — keyword matching | P2 |
| D-7 TOML config | LOW-MEDIUM — operational flexibility | LOW — toml parse | P2 |
| D-4 PodStateSnapshot at crash | MEDIUM — better Ollama context | HIGH — requires rc-agent change | P3 |
| D-6 Fleet health crash_reason field | LOW — nice dashboard metric | MEDIUM — server schema | P3 |
| D-2 stderr capture from bat | LOW — marginal log improvement | MEDIUM — ops deploy step | P3 |

---

## Anti-Cheat Safety Summary

This is the highest-risk constraint for this milestone. Concrete safe vs unsafe list:

| Action | EAC/EOS Safe? | Reason |
|--------|--------------|--------|
| HTTP GET to `localhost:8090/health` | YES | TCP to rc-agent's own port, no game process contact |
| `std::fs::read_to_string` on log files | YES | File I/O, not process memory access |
| `taskkill /F /IM rc-agent.exe` (by name) | YES | Standard Windows process management by name |
| `netstat -ano` shell-out | YES | Network statistics query, no process memory |
| `netsh` socket cleanup | YES | Network stack command, no process memory |
| HTTP POST to Ollama `192.168.31.27:11434` | YES | Local network call to James, no game contact |
| `sysinfo::System::processes()` (read-only list) | YES | Reads process list via standard API — no handle to game process, no memory read |
| `OpenProcess(game_pid)` | NO | EAC scans for open handles to game process |
| `CreateRemoteThread(game_pid, ...)` | NO | Immediate EAC flag |
| `VirtualQueryEx(game_pid, ...)` | NO | Memory inspection of game process |
| Attaching WinDbg or debug events | NO | EAC detects debug events on game process |
| `taskkill /F /PID <game_pid>` (by game PID) | GRAY — avoid | PID-based kill requires OpenProcess internally; prefer stopping billing and letting rc-agent handle game teardown when it restarts |

**Verdict**: The sentry's health poll + log read + deterministic shell commands approach is entirely EAC-safe. The constraint requires discipline about PID-based operations on game processes specifically, not rc-agent processes.

---

## Relationship to Existing Modules

| Feature | Ports From | Extends | Creates |
|---------|-----------|---------|---------|
| TS-1 Health poll | `self_monitor.rs` query pattern | `rc-sentry/src/main.rs` — new background thread | `crash_watcher` thread in sentry |
| TS-2 Log read | `ai_debugger.rs` — log path constants | rc-sentry main | `read_crash_logs()` fn |
| TS-3 Tier 1 fixes | `ai_debugger.rs` — fix_stale_socket, fix_zombie_game, fix_config, fix_shader_cache | `rc_common::exec::run_cmd_sync` | `tier1_fixes()` fn in sentry |
| TS-4 Pattern memory | `ai_debugger.rs` — DebugMemory struct, instant_fix() | Duplicate or extract to rc-common | `pattern_memory.rs` or inline in sentry |
| TS-5 Ollama query | `self_monitor.rs` — query_ollama() (async) | Blocking rewrite for sentry's std context | `query_ollama_blocking()` fn |
| TS-7 Escalation FSM | `ai_debugger.rs` — restart count patterns | `EscalatingBackoff` in rc-common | `escalation_state` struct in crash watcher |
| TS-9 Crash log | `self_monitor.rs` — `log_event()`, `EVENT_LOG` constant | New log file path | `log_crash_event()` fn |
| D-1 Memory write | `ai_debugger.rs` — `DebugMemory::record_fix()`, `save()` | Shares debug-memory.json | |
| TS-8 Anti-cheat constraint | Architecture constraint — no new code | Affects how TS-1, TS-3 are implemented | |

---

## Sources

- **Codebase audit (HIGH confidence):**
  - `crates/rc-sentry/src/main.rs` — confirmed pure-std, no-async architecture; 6 endpoints; `run_cmd_sync` usage; thread-per-connection model
  - `crates/rc-agent/src/ai_debugger.rs` — 14 Tier 1 fix patterns, DebugMemory struct, 4-tier debug order, pattern_key logic, billing gate rule
  - `crates/rc-agent/src/self_monitor.rs` — health monitoring pattern, relaunch_self(), log_event(), CLOSE_WAIT detection, escalation logic
  - `.planning/PROJECT.md` — v11.2 target feature list, constraints (no new crates, extend rc-sentry only, anti-cheat safe)

- **EAC detection mechanisms (MEDIUM confidence — WebSearch + arxiv paper):**
  - EAC scans for open handles to the game process, memory read/write access, suspicious threads in kernel/user mode, and hooking techniques. Source: arxiv.org/html/2408.00500v1 (academic analysis of kernel anti-cheat systems)
  - HTTP polling to a separate localhost port (rc-agent :8090) does not create handles to the game process and is invisible to EAC's detection surface.
  - EAC's Windows 11 24H2 compatibility issue was resolved in June 2025 (KB5063060) — pods must have this update installed for F1 25 to be stable regardless of sentry design.

- **iRacing EOS anti-cheat (MEDIUM confidence — WebSearch + iRacing support docs):**
  - iRacing transitioned from Kamu/EAC to Epic EOS anti-cheat. EOS runs iRacing inside a sandbox preventing external program access to the simulation memory. External telemetry tools that use UDP (not process memory) coexist with EOS safely. HTTP health polling falls in the same safe category.

- **Watchdog escalation patterns (HIGH confidence — embedded systems best practice + Akka supervisor pattern):**
  - Check-in pattern: supervisor monitors health signals; absence triggers escalation. Source: interrupt.memfault.com/blog/firmware-watchdog-best-practices
  - Exponential backoff with restart ceiling: stop restarting after N failures in window, escalate to staff. Source: xebia.com/blog/exponential-backoff-with-akka-actors/
  - Both patterns are already implemented in rc-common (EscalatingBackoff) and rc-agent (self_monitor.rs) — the sentry reuses, not reinvents.

---

*Feature research for: v11.2 RC Sentry AI Debugger (rc-sentry crash watcher for rc-agent on sim racing pods)*
*Researched: 2026-03-21*
