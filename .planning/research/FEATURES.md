# Feature Research: E2E Process Guard

**Domain:** Continuous process whitelist enforcement, auto-start auditing, and violation handling across a mixed Windows fleet (8 sim pods + 1 server + 1 operations workstation)
**Researched:** 2026-03-21
**Confidence:** HIGH (based on direct codebase audit — kiosk.rs ALLOWED_PROCESSES already established, process detection patterns proven in game_process.rs, kiosk.rs, self_test.rs; Windows audit surface well-defined)

---

## Feature Landscape

### Table Stakes (Operations Expects These)

Features the process guard must have to do its job. Missing any of these means the guard either misses violations, can't act on them, or creates so much noise that staff disable it.

| # | Feature | Why Expected | Complexity | Existing Dependency |
|---|---------|--------------|------------|---------------------|
| TS-1 | **Central whitelist in racecontrol.toml with per-machine overrides** | Different machines legitimately run different processes: James needs Ollama (11434), pods do not want Steam, server does not run ConspitLink. A flat global list causes false positives on every machine. | LOW | `racecontrol.toml` already has sections for per-feature config. TOML supports nested tables trivially. No new serialization infrastructure needed. |
| TS-2 | **Process whitelist check — running processes vs approved list** | Core function of the guard. Scan running processes (via `sysinfo` crate, already used in `kiosk.rs`) and flag any exe not in the approved list. | LOW | `sysinfo::System` already imported in `kiosk.rs`. `ALLOWED_PROCESSES` static slice already has 60+ validated entries categorized. New guard reuses this foundation — no re-inventorying the Windows process namespace. |
| TS-3 | **Auto-kill on violation** | A guard that only reports violations is easily ignored. Unauthorized processes are either customer attempts to bypass kiosk restrictions or silent installs from Windows Update/software. Auto-kill is the correct default. Staff cannot manually kill processes on 8 pods — it must be automated. | LOW | `taskkill /F /IM <name>` pattern already used in `game_process.rs` and `self_heal.rs`. Wrap in a safe fn with allowlist re-check before kill to prevent accidental kills. |
| TS-4 | **Auto-start audit: HKCU/HKLM Run keys** | The trigger for v12.1 was a missed Steam auto-start. Registry Run keys are the canonical Windows auto-start mechanism and must be audited. Any non-whitelisted entry in `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` or `HKLM\...\Run` must be removed. | MEDIUM | `self_heal.rs` already reads/writes HKLM Run keys (for RCAgent itself). `winreg` crate already in rc-agent's dependency tree. New: enumerate all values in Run keys and diff against whitelist. |
| TS-5 | **Auto-start audit: Startup folder** | `%AppData%\Microsoft\Windows\Start Menu\Programs\Startup` is a second auto-start vector. Steam, Discord, and bundled software installers frequently drop shortcuts here. Must enumerate and remove non-whitelisted entries. | LOW | `std::fs::read_dir` on the Startup folder path. Known Windows path, no external crate needed. |
| TS-6 | **Auto-start audit: Scheduled Tasks** | Scheduled Tasks are the third major auto-start vector and the most dangerous because they can run as SYSTEM. The venue's own legitimate tasks (kiosk, web dashboard) must be whitelisted; all others removed. | MEDIUM | `schtasks /query /fo CSV` → parse output. Already used as shell-out pattern via `rc-common::exec`. Alternatively `com_object`-based ITaskService but shell-out is sufficient and simpler. |
| TS-7 | **Port audit: listening ports vs approved list** | A non-whitelisted listening port indicates an unauthorized server process (game server, remote access tool, crypto miner). Must detect and flag. Acts as a second layer beyond process name matching. | LOW | `netstat -ano` shell-out → parse listening ports. Pattern matches `remote_ops.rs` and `firewall.rs` existing approaches. `sysinfo` also exposes network info. |
| TS-8 | **Pod binary guard: wrong binary on wrong machine** | Standing rule #2 — `rc-agent.exe` must never run on James, `racecontrol.exe` must never run on a pod. This was a source of past crashes. Detect by process name + machine role combination. | LOW | `config.rs` already has `PodConfig.pod_number` to identify machine role. If `racecontrol.exe` is found in process list on a pod, it's an immediate violation. |
| TS-9 | **Violation alert via WebSocket to staff kiosk** | Staff must know about violations without polling. The WS notification channel is the established mechanism for all pod-to-server alerting. A new `ProcessViolation` AgentMessage variant routes to the kiosk dashboard. | LOW | `AgentMessage` enum in `rc-common/src/protocol.rs`. Adding a variant follows the exact same pattern as `ProcessApprovalRequest` (already exists). |
| TS-10 | **Audit log: timestamp, machine, process name, action taken** | Operations log for Uday — can review what was killed, when, why. Critical for compliance (e.g., "did the guard kill a legitimate process?"). Also used to detect persistent violators (process keeps relaunching = deeper problem). | LOW | Append to `C:\RacingPoint\process-guard.log` using the same rotation pattern as `self_monitor.rs` (512KB cap, rotate-on-exceed). Structured lines: `[timestamp] MACHINE: pod-3 | NAME: steam.exe | PID: 4421 | ACTION: killed`. |
| TS-11 | **Violation report to server on kill** | Beyond WS notification, the server should record every kill centrally. Enables cross-machine violation pattern analysis (e.g., Steam keeps reinstalling on pods 2 and 4 but not 6). | LOW | `AgentMessage::ProcessViolation` carries machine, process name, pid, action, timestamp. Server stores in-memory per-pod violation history (not DB — not worth schema cost for this milestone). |
| TS-12 | **Continuous monitoring daemon with configurable poll interval** | The guard must run continuously — not just on startup. Process violations happen at runtime (customer triggers a download, Windows auto-update runs in background). Continuous polling at a configurable interval (default 60s) is the correct model. | LOW | `tokio::time::interval` loop, same structure as `self_monitor.rs` and `kiosk.rs` enforce loop. Interval configurable via `[process_guard]` TOML section. |

### Differentiators (Beyond the Mandatory Floor)

Features that make the guard significantly more useful for operations without being mandatory for it to function.

| # | Feature | Value Proposition | Complexity | Notes |
|---|---------|-------------------|------------|-------|
| D-1 | **Category-tagged whitelist (system, racecontrol, game, peripheral, ollama)** | Instead of a flat list of names, the whitelist entries carry a category tag. This enables per-machine policy by category: pods allow `game` category, James allows `ollama` category, server allows `racecontrol` category. Easier to maintain and audit than per-name machine overrides. | LOW | Add `category` field to whitelist entries. TOML: `[[process_guard.allowed]] name = "ollama.exe" category = "ollama" machines = ["james"]`. The existing `ALLOWED_PROCESSES` in `kiosk.rs` already has natural categories in comments — formalize them. |
| D-2 | **Wildcard/prefix matching in whitelist** | Some processes need pattern matching: NVIDIA spawns `nvcontainer.exe`, `nvdisplay.container.exe`, `nvspcaps64.exe` — all from the same vendor. A `nv*.exe` wildcard is cleaner than enumerating each. Similarly, `acs_*.exe`, `msedge_*.exe`. | LOW | `glob` crate or simple prefix/suffix matching with `*` as wildcard character. The whitelist entry specifies `name = "nv*.exe"`. Complexity is LOW — this is pattern string matching, not regex engine. |
| D-3 | **Severity tiers: kill-immediately vs warn-and-escalate vs monitor-only** | Not all violations warrant immediate kill. A first-time unknown process should be logged and escalated to staff (same `ProcessApprovalRequest` flow as `kiosk.rs`). A known-bad process (e.g., `steam.exe` on a pod) should be killed immediately. Severity tier is per-entry in the blocklist config. | MEDIUM | Three tiers: `KILL` (kill without approval), `ESCALATE` (warn staff, auto-kill after TTL), `MONITOR` (log only, no action). Per-entry in the `[process_guard.blocked]` section or derive from category. |
| D-4 | **Violation trend reporting: escalate on repeat offenders** | If the same process is killed more than N times within a time window, escalate via email rather than just WS notification. Indicates a process is relaunching itself (installer, watchdog, malware). | MEDIUM | Track per-process kill counts in memory. If count > threshold (e.g., 3 kills in 30min), trigger `email_alerts.rs` via the same send_email.js shell-out. No new email infrastructure. |
| D-5 | **Fleet-wide violation summary in /api/v1/fleet/health** | Extend `PodFleetStatus` to include `violation_count_24h` and `last_violation_at`. Uday can see at a glance which pods have been most active for the guard without reading logs. | MEDIUM | Requires server-side aggregation of `ProcessViolation` AgentMessages. Server maintains per-pod violation counter in `FleetHealthStore`. No DB schema change — in-memory only. |
| D-6 | **Guard runs on James workstation as a standalone daemon (not rc-agent module)** | James is not a pod — it doesn't run rc-agent. The process guard for James needs to run as a separate light binary (or as part of racecontrol server ops if James runs a comms-link daemon). The guard logic should be a library crate so it compiles into both rc-agent (pods) and a standalone binary (James). | MEDIUM | Extract guard logic to `rc-common` or a new `rc-guard` helper crate. Standalone binary for James polls local processes and reports violations via Tailscale HTTP to the server. Pod variant runs as an rc-agent module. |
| D-7 | **Guard runs on racecontrol server as a module** | The server (.23) runs racecontrol.exe — it needs its own process guard integrated as a `server_ops.rs`-style module. Violations on the server are higher severity (server compromise affects all 8 pods). | LOW | Add `process_guard::run_server_guard()` called from racecontrol's main event loop. Server guard is simpler — it only needs to audit the server's own processes, not pod processes. |

### Anti-Features (Do Not Build)

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **Real-time process event subscription (ETW/WMI events)** | ETW (Event Tracing for Windows) process creation events give sub-100ms detection. Sounds appealing but requires COM + elevated SYSTEM privileges + significant Win32 plumbing not representable in safe Rust. High complexity for marginal gain over 60s polling. | 60-second polling is sufficient for the threat model: Steam auto-starts, Windows Update agents, customer-triggered downloads. Near-real-time detection of process spawns is not the problem to solve. |
| **Per-process cryptographic hash verification** | Checking SHA-256 of every running binary against a hash database catches process-name spoofing. However, managing hash updates across 8 pods (every Windows Update changes system binary hashes) creates enormous operational overhead. | Trust process name + path. Verify path is in an expected directory (e.g., `C:\RacingPoint\`, `C:\Windows\System32\`). Path-based allow is sufficient and maintainable. |
| **Deep packet inspection on flagged ports** | Examining traffic on non-whitelisted ports to determine if it's "benign" network activity. Network DPI requires kernel-level drivers, is OS-version-specific, and creates far more operational risk than it solves. | Port audit at the socket level (listening ports via netstat). If a non-approved port is listening, kill the owning process. No DPI needed. |
| **Behavioral analysis (CPU/memory heuristics for malware)** | CPU usage anomaly detection to identify cryptominers or runaway processes. This crosses into EDR territory. A sim racing venue has a narrow expected process tree — a simple whitelist is more effective than behavioral analysis. | If a process is not on the whitelist, kill it regardless of behavior. Behavioral analysis adds complexity without better outcomes for this threat model. |
| **Quarantine mode (suspend instead of kill)** | Suspending a violating process rather than killing it to "preserve evidence." Adds process lifecycle complexity, suspended processes still consume memory, and staff cannot use a suspended process to diagnose anything useful. | Kill the process, log the violation with process name + path + PID. The audit log is the "evidence." |
| **Auto-whitelisting via LLM on every scan** | Routing every unknown process through Ollama for classification (the kiosk ALLOWED pattern) is expensive at the process guard level. The kiosk LLM is justified because it's per-customer-session and the stakes are high (false positive = lock out customer). The process guard runs every 60s — LLM on each scan adds latency and Ollama load. | Use LLM only for `ESCALATE`-tier violations (first-time unknowns) when staff approval is needed. `KILL`-tier violations (known-bad names) skip LLM entirely. |
| **DB schema for violation history** | Adding a `process_violations` table to track every kill over time. Adds migration, schema change, sync surface. | In-memory per-machine violation counters in `FleetHealthStore`. Structured log file for long-term history. The log file is queryable (grep, tail) without adding DB overhead. |

---

## Feature Dependencies

```
racecontrol.toml [process_guard] section (TS-1)
    └──loaded by──> ProcessGuardConfig struct (new, in rc-common or rc-agent/config.rs)
                        ├──used by──> rc-agent process_guard module (TS-2, TS-3, TS-4, TS-5, TS-6)
                        └──used by──> racecontrol server guard module (D-7)

kiosk.rs ALLOWED_PROCESSES (existing)
    └──reused by──> process_guard module base whitelist (TS-2)
                        └──extended with──> per-machine overrides from TOML (TS-1)

sysinfo crate (existing in rc-agent)
    └──used by──> TS-2 process scan

winreg crate (existing in rc-agent via self_heal.rs)
    └──used by──> TS-4 Run key audit

rc-common::exec (existing)
    └──used by──> TS-3 kill (taskkill), TS-6 schtasks query, TS-7 netstat

AgentMessage enum (rc-common/protocol.rs)
    └──new variant──> ProcessViolation { pod_id, machine, name, pid, action, timestamp } (TS-9, TS-11)

TS-9 (WS alert) requires TS-2 (process scan produces violation)
TS-10 (audit log) requires TS-3 (action taken is what gets logged)
TS-11 (server record) requires TS-9 (WS channel carries the event)

D-4 (repeat offender email) requires TS-3 (kill count tracked from kill actions)
D-5 (fleet health violation count) requires TS-11 (server receives violations to count)
D-6 (James standalone daemon) requires extracting guard logic from rc-agent to rc-common

TS-8 (pod binary guard) requires TS-1 (machine role known from config) and TS-2 (process scan)
```

### Dependency Notes

- **TS-4, TS-5, TS-6 (auto-start audit) are independent of TS-2 (process scan)**: Audit checks can run at startup + periodic, process scanning runs continuously. They share config and logging infrastructure but not code paths.
- **TS-3 (auto-kill) depends on TS-2 (process scan)**: Cannot kill what you have not scanned.
- **D-6 (James standalone) requires guard logic in rc-common, not rc-agent**: This is an architectural decision that affects phase ordering — the library extraction should happen before building the standalone binary.
- **kiosk.rs already has LLM classification for `ESCALATE`-tier unknowns**: The process guard can reuse `ProcessApprovalRequest` AgentMessage and `kiosk::classify_with_llm()` rather than rebuilding this path.

---

## MVP Definition

### Launch With (v12.1 core)

The minimum guard that solves the triggering incident (Steam missed during manual audit) and the pod binary rule violation risk.

- [ ] **TS-1** — `[process_guard]` section in racecontrol.toml with `allowed` list and per-machine `overrides` table. Schema validated on startup.
- [ ] **TS-2** — Periodic process scan in rc-agent (`process_guard.rs` module) using `sysinfo`. 60s default poll interval.
- [ ] **TS-3** — Auto-kill via `taskkill /F /IM` for processes not on the whitelist. Safe guard: never kill from `["system", "rc-agent.exe", "lsass.exe", "csrss.exe"]` hard-coded safety set regardless of config.
- [ ] **TS-4** — HKCU + HKLM Run key audit at startup and every 5 minutes. Remove non-whitelisted entries.
- [ ] **TS-5** — Startup folder audit at startup and every 5 minutes. Remove non-whitelisted shortcuts.
- [ ] **TS-8** — Pod binary guard: if `racecontrol.exe` found running on a pod, immediate `ProcessViolation` alert (do not kill — log and escalate, restart rc-agent to be safe).
- [ ] **TS-9** — `AgentMessage::ProcessViolation` variant sent to server on every kill or auto-start removal.
- [ ] **TS-10** — Append-only audit log at `C:\RacingPoint\process-guard.log` (512KB rotation).
- [ ] **TS-11** — Server receives `ProcessViolation` messages and stores per-pod violation list in `FleetHealthStore`.
- [ ] **TS-12** — Continuous daemon spawned from rc-agent `event_loop.rs` via `tokio::spawn`.

### Add After Validation (v12.1 polish)

- [ ] **TS-6** — Scheduled Task audit (add after Run key + Startup folder proven stable — avoids scope creep in initial ship).
- [ ] **TS-7** — Port audit (add after process guard stable — port violations are a secondary concern vs process violations).
- [ ] **D-1** — Category-tagged whitelist entries (refactor after MVP ships and real-world false positives are identified).
- [ ] **D-3** — Severity tiers: KILL vs ESCALATE vs MONITOR (add when operations has enough violation history to make informed tier decisions).

### Future Consideration (v12.2+)

- [ ] **D-4** — Repeat offender email escalation (add when violation log shows persistent offenders — data-driven trigger).
- [ ] **D-5** — Fleet health violation count field (add when Uday asks "which pod has the most violations?").
- [ ] **D-6** — James standalone guard binary (requires rc-common refactor, defer until v13.0 infrastructure phase).
- [ ] **D-7** — Server-side process guard module (lower priority — server has no customers, lower violation risk).
- [ ] **D-2** — Wildcard matching in whitelist (add when real-world wildcard patterns are identified — premature optimization otherwise).

---

## Feature Prioritization Matrix

| Feature | Operational Value | Implementation Cost | Priority |
|---------|-------------------|---------------------|----------|
| TS-1 TOML config schema | HIGH — enables everything else | LOW — TOML table struct | P1 |
| TS-2 Process scan | HIGH — core function | LOW — sysinfo already imported | P1 |
| TS-3 Auto-kill | HIGH — guard without kill is just a logger | LOW — taskkill pattern exists | P1 |
| TS-4 Run key audit | HIGH — exact trigger for this milestone | MEDIUM — winreg enumerate + diff | P1 |
| TS-5 Startup folder audit | HIGH — second auto-start vector | LOW — read_dir + remove | P1 |
| TS-8 Pod binary guard | HIGH — standing rule enforcement | LOW — name check in scan | P1 |
| TS-9 WS violation alert | HIGH — staff visibility | LOW — new AgentMessage variant | P1 |
| TS-10 Audit log | HIGH — operational record | LOW — append file + rotate | P1 |
| TS-12 Continuous daemon | HIGH — periodic guard is useless | LOW — interval loop pattern | P1 |
| TS-11 Server violation record | MEDIUM — aggregation value | LOW — FleetHealthStore field | P1 |
| TS-6 Scheduled task audit | MEDIUM — third auto-start vector | MEDIUM — schtasks parse | P2 |
| TS-7 Port audit | MEDIUM — secondary layer | LOW — netstat parse | P2 |
| D-1 Category tags | MEDIUM — maintainability | LOW — TOML schema only | P2 |
| D-3 Severity tiers | MEDIUM — precision | MEDIUM — tier dispatch logic | P2 |
| D-4 Repeat offender email | LOW — rare scenario | MEDIUM — counter + email | P3 |
| D-5 Fleet health count field | LOW — nice dashboard metric | MEDIUM — server aggregation | P3 |
| D-6 James standalone binary | MEDIUM — coverage | HIGH — rc-common refactor | P3 |
| D-7 Server guard module | LOW — low risk surface | MEDIUM — server integration | P3 |
| D-2 Wildcard matching | LOW — premature | LOW | P3 |

---

## Relationship to Existing Modules

This table maps each new feature to the rc-agent/racecontrol modules it touches or extends, to avoid hidden interface churn during implementation.

| Feature | Touches / Extends | Creates New |
|---------|-------------------|-------------|
| TS-1 config schema | `rc-agent/config.rs` — add `ProcessGuardConfig` | `[process_guard]` TOML section |
| TS-2, TS-3 process scan + kill | `kiosk.rs` — reuse `ALLOWED_PROCESSES` and sysinfo System | `process_guard.rs` module |
| TS-4 Run key audit | `self_heal.rs` — reuse winreg patterns | `auto_start.rs` module (or sub-fn in process_guard.rs) |
| TS-5 Startup folder audit | None — pure std::fs | Part of auto_start.rs |
| TS-6 Scheduled task audit | `rc-common/exec.rs` — shell-out to schtasks | Part of auto_start.rs |
| TS-7 Port audit | `firewall.rs` and `remote_ops.rs` — reuse netstat pattern | `port_audit.rs` fn or sub-fn |
| TS-8 Pod binary guard | `process_guard.rs` scan path — special case by name | No new file |
| TS-9 WS alert | `rc-common/protocol.rs` — add `ProcessViolation` variant | AgentMessage variant |
| TS-10 audit log | `self_monitor.rs` — reuse log rotation pattern | `C:\RacingPoint\process-guard.log` |
| TS-11 server record | `fleet_health.rs` — add `violations` to `FleetHealthStore` | Server-side handler for new AgentMessage |
| TS-12 daemon spawn | `event_loop.rs` — add `process_guard::spawn()` call | `spawn()` fn in process_guard.rs |

---

## Sources

- **Codebase audit (HIGH confidence):**
  - `crates/rc-agent/src/kiosk.rs` — `ALLOWED_PROCESSES` (60+ entries, already categorized), `sysinfo` usage, LLM classification path, `ProcessApprovalRequest` flow
  - `crates/rc-agent/src/self_heal.rs` — `winreg` Run key read/write patterns, log append/rotate pattern
  - `crates/rc-agent/src/self_monitor.rs` — interval daemon pattern, log rotation constants
  - `crates/rc-agent/src/config.rs` — `AgentConfig` schema, TOML section design pattern
  - `crates/rc-agent/src/game_process.rs` — `taskkill` invocation pattern, process name constants
  - `crates/racecontrol/src/fleet_health.rs` — `FleetHealthStore` extension points
  - `crates/rc-common/src/protocol.rs` — `AgentMessage` variants, `ProcessApprovalRequest` as template for `ProcessViolation`
- **Domain analysis (HIGH confidence — standard Windows process hardening operations):**
  - Windows auto-start vectors (Run keys, Startup folder, Scheduled Tasks) are well-documented and stable. The `winreg` crate API for enumerating registry values is standard. schtasks CSV output format is stable since Windows 7. This is applied Windows system programming, not novel territory.
- **Operational context (HIGH confidence — direct cause analysis from PROJECT.md):**
  - The v12.1 milestone was triggered by Steam, Leaderboard kiosk, and voice assistant watchdog being missed during a manual audit. The feature set directly addresses these three auto-start vectors.

---

*Feature research for: v12.1 E2E Process Guard (rc-agent pods + racecontrol server + James workstation)*
*Researched: 2026-03-21*
