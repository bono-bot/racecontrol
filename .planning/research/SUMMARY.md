# Project Research Summary

**Project:** RaceControl v4.0 — Pod Fleet Self-Healing
**Domain:** Windows gaming PC fleet management — crash recovery, firewall auto-config, remote exec, self-healing config, fleet observability
**Researched:** 2026-03-15
**Confidence:** HIGH

## Executive Summary

RaceControl v4.0 addresses five root causes uncovered during a 4-hour outage on Mar 15, 2026 where Pods 1, 3, and 4 went offline simultaneously: exec slot exhaustion, missing firewall rules (caused by CRLF-damaged batch files), rc-agent crashes with no auto-restart (HKLM Run key has no crash restart semantics), and no remote diagnostics once HTTP port 8090 was blocked. The research is grounded in direct codebase inspection of the production system and cross-referenced against official Microsoft documentation, making confidence unusually high for a single-day research cycle.

The recommended approach uses a minimal set of additions on top of the existing Rust/Axum/WebSocket infrastructure. The critical architectural decision is the Windows Service strategy: rc-agent must NOT be converted to a native Windows Service because it owns a Session 1 GUI (lock screen overlay). Instead, use NSSM as a crash-restart watchdog that wraps the existing startup bat file, combined with a Task Scheduler task for Session 1 restart-on-failure. The stack adds only three new Rust crates (windows-service or NSSM, winreg 0.55, tokio-util 0.7) and zero new npm packages. All fleet dashboard functionality is achievable by extending the existing kiosk WebSocket hook and adding one new Next.js route.

The highest risks are all in the first phase: the Session 0/Session 1 split is a hard OS boundary that will cause a blank lock screen if mishandled, CRLF batch file generation must be unit-tested from the start, and the WebSocket exec protocol change requires rc-common, rc-core, and rc-agent to all be rebuilt and deployed atomically. Every subsequent phase builds on a foundation that must be verified on Pod 8 (canary) before fleet-wide deployment.

---

## Key Findings

### Recommended Stack

The v4.0 stack adds three Windows-only Rust crates and zero new npm packages. The project constraint ("no new dependencies where existing deps cover it") is satisfied — each addition is justified by a capability gap that no existing dep covers.

**Core technologies (new additions only):**
- `windows-service = "0.8"` (Mullvad VPN, 2.8M downloads): Windows SCM service registration and ServiceMain protocol — only needed if the native service path is chosen. NSSM is the alternative (external binary, no code change to main.rs).
- `winreg = "0.55"`: Registry read/write (HKLM Run key, startup config verification) — 10 lines of safe Rust vs 150 lines of raw winapi FFI for HKEY lifecycle management.
- `tokio-util = "0.7"`: CancellationToken for coordinated async shutdown — required if native Windows Service is implemented; already a standard tokio companion crate.
- `std::process::Command` calling `netsh advfirewall`: Firewall rule management — no new crate needed; eliminates the CRLF-sensitive batch file failure mode entirely.
- Existing kiosk Next.js 16.1.6 + `useKioskSocket` hook: Fleet dashboard — zero new npm packages; new `/fleet` route added to existing kiosk app.

**NSSM vs native ServiceMain:**
ARCHITECTURE.md and FEATURES.md diverge slightly on this choice. ARCHITECTURE.md recommends NSSM (external wrapper, zero rc-agent code change, Session 1 safe). STACK.md recommends the native `windows-service` crate (structured shutdown, error reporting via WebSocket before death). The resolution: if lock screen responsibility is moved to the kiosk (already runs in Session 1), native ServiceMain becomes viable. If the lock screen stays in rc-agent, NSSM or Task Scheduler is mandatory.

### Expected Features

**Must have (P1 — eliminates Mar 15 root causes):**
- Windows Service wrapper with auto-restart on crash — eliminates root cause #4 (HKLM Run key has no crash restart)
- Firewall auto-configuration in Rust on startup — eliminates root causes #2 and #3 (CRLF-damaged batch files)
- Config self-heal at startup: verify toml, bat file (LF check), HKLM Run/Task Scheduler key, firewall rules — eliminates root cause #3 broadly
- Startup error reporting: structured phase names, pre-exit HTTP POST to rc-core, file fallback — eliminates root cause #5 partially
- WebSocket exec (CoreToAgentMessage::Exec with request_id correlation) — eliminates root cause #5 fully (management when HTTP blocked)

**Should have (P2 — operational visibility):**
- Deploy rollback on 60s health gate failure — prevents bad deploys leaving pods offline permanently
- Agent version in heartbeat and fleet dashboard — verifies all 8 pods updated after deploy
- Fleet health dashboard at `/fleet` kiosk route — single-screen ops view for Uday's phone

**Defer to v5.0:**
- WebSocket exec output streaming — 3x complexity of request/response, not stability-critical
- Pod uptime trend heatmap — requires new DB table; defer until fleet is stable
- Exec slot visibility in dashboard — useful but low urgency post-v4.0

**Anti-features (do not implement):**
- Automatic reboot on any crash — destroys active customer sessions; SCM "Restart Computer" action must never be enabled
- Continuous config file monitoring (inotify-style) — creates repair feedback loops; startup-time check is sufficient
- Full ServiceMain in rc-agent without moving lock screen — Session 0 hard boundary causes invisible lock screen
- Centralized log aggregation (ELK, Loki) — overkill for 8 pods; per-pod log files fetched on-demand are sufficient

### Architecture Approach

The architecture layers new capabilities onto the existing two-crate WebSocket protocol (rc-common defines messages; rc-core sends to pods; rc-agent handles and responds) without restructuring the established communication pattern. All five v4.0 concerns map to distinct, low-coupling additions: a new `firewall.rs` module in rc-agent, a new `self_healing.rs` module in rc-agent, two new message variants in rc-common (Exec + ExecResult), a new `fleet_health.rs` background task in rc-core, and a new kiosk `/fleet` page. The most invasive change is deploy.rs gaining a WebSocket exec fallback path when HTTP :8090 is unavailable.

**Major components (new and modified):**
1. `rc-agent/src/firewall.rs` (new) — idempotent netsh wrapper; runs before `remote_ops::start()`; returns `Vec<FirewallAction>` for startup report
2. `rc-agent/src/self_healing.rs` (new) — checks toml, bat CRLF, registry key, AV exclusions on startup; repairs via embedded templates; reports anomalies
3. `rc-common/src/protocol.rs` (modified) — add `CoreToAgentMessage::Exec { request_id, cmd, timeout_ms }`, `AgentMessage::ExecResult { request_id, success, exit_code, stdout, stderr }`, `DashboardEvent::FleetHealth(Vec<PodHealthSnapshot>)`, `DeployState::Rollback`
4. `rc-core/src/state.rs` (modified) — add `pending_ws_execs: RwLock<HashMap<String, oneshot::Sender<ExecResult>>>` field
5. `rc-core/src/deploy.rs` (modified) — dual-path exec (HTTP first, WS fallback); rollback logic on 60s gate failure; save `rc-agent-prev.exe` in do-swap.bat
6. `rc-core/src/fleet_health.rs` (new) — background task; reads AppState every 5s; broadcasts `DashboardEvent::FleetHealth`
7. `kiosk/src/app/fleet/page.tsx` + `FleetGrid.tsx` + `PodHealthCard.tsx` (new) — mobile-first health grid; reuses `useKioskSocket`; shows WS status, deploy state, watchdog state, last error
8. NSSM or Task Scheduler install scripts (new) — deployed via pod-agent HTTP; service wraps `start-rcagent.bat`

### Critical Pitfalls

1. **Session 0 GUI invisible** — Windows Services run in Session 0; rc-agent's lock screen (Edge kiosk + port 18923) is invisible in Session 0. The "Allow service to interact with desktop" checkbox is permanently non-functional on Windows 10 1803+ / Windows 11. Mitigation: use NSSM/Task Scheduler to wrap `start-rcagent.bat` (Session 1 preserved) OR move lock screen to kiosk before converting to native ServiceMain. Verify with `tasklist /v` that Session# = 1 after any service install.

2. **CRLF line endings silently break batch files** — cmd.exe splits on `\r\n`; LF-only bat files are silently misparsed as a single line. This was the direct root cause of the Mar 15 outage. Any Rust code writing a .bat file must use `lines.join("\r\n") + "\r\n"`. A unit test asserting CRLF presence is mandatory in the firewall module.

3. **Exec slot exhaustion blocks all management** — the 4-slot semaphore in remote_ops.rs is shared across all HTTP exec operations. WebSocket exec MUST use an independent semaphore; if both management paths share the semaphore, they fail together. rc-core must also serialize exec requests per pod (per-pod Mutex) to avoid concurrent slot exhaustion.

4. **Windows Defender holds new binary during self-swap** — `rc-agent-new.exe` is a fresh unsigned Rust binary that triggers AV heuristic scanning. The scanner holds a file handle while `do-swap.bat` tries to rename it, causing `ERROR_SHARING_VIOLATION`. Mitigation: retry loop (5 attempts, 2s sleep) in do-swap.bat PLUS AV exclusion verification at startup via winreg.

5. **Firewall rule applied to wrong profile** — `netsh add rule` without `profile=any` applies to the currently active profile (may be Public if DHCP is still negotiating). Later profile reclassification leaves the pod unreachable. Always pass `profile=any` and verify rule application by re-running the show command, not just checking exit code.

---

## Implications for Roadmap

Based on the combined research, the dependency graph is clear: rc-common protocol additions must precede both rc-core and rc-agent changes; firewall auto-config and self-healing are independent and can ship first as isolated modules; the Windows Service install must come after all agent changes are live (service restarts bring up the new agent). The natural phase split is 7 implementation phases matching the ARCHITECTURE.md build order.

### Phase 1: rc-common Protocol Additions (Foundation)
**Rationale:** rc-common is the contract between rc-agent and rc-core. Adding `Exec`, `ExecResult`, `FleetHealth`, `DeployState::Rollback`, and `PodHealthSnapshot` here first means all downstream crates can be updated without breaking the build incrementally. This is zero user-visible work but blocks everything else.
**Delivers:** Updated protocol types; all existing enum variants still serialize identically (characterization tests must pass before proceeding).
**Addresses:** Protocol foundation for WS exec (P1 feature), fleet health broadcast (P2 feature), deploy rollback state (P2 feature)
**Avoids:** Mid-development protocol breaks that force simultaneous rebuilds of both rc-core and rc-agent

### Phase 2: rc-agent Firewall Module
**Rationale:** Isolated, independent, immediately addresses a root cause. No dependency on Phase 1 new variants (firewall.rs uses std::process::Command only). Can be deployed to Pod 8 (canary) for fast verification.
**Delivers:** `firewall.rs` — idempotent netsh ICMP + TCP 8090 rules; runs before remote_ops starts; CRLF-safe Rust string generation; unit test asserting CRLF in any generated bat content.
**Addresses:** FWALL-01 (firewall auto-config), eliminates Mar 15 root causes #2 and #3 directly
**Avoids:** Duplicate firewall rules (Pitfall 8) via idempotency check; wrong profile (Pitfall 5) via `profile=any`; CRLF failures (Pitfall 2) by moving entirely out of batch files

### Phase 3: rc-agent WebSocket Exec Handling
**Rationale:** Depends on Phase 1 (Exec variant must exist in rc-common). rc-agent just needs to receive `CoreToAgentMessage::Exec`, execute the command, and send `AgentMessage::ExecResult` — the rc-core side is not needed yet for the agent to build and test.
**Delivers:** Exec handler in rc-agent main.rs WS receive loop; shared exec helper extracted from remote_ops.rs; independent semaphore for WS exec path.
**Addresses:** WS-EXEC-01 (agent side); eliminates dependency on HTTP :8090 being reachable for basic management
**Avoids:** Shared semaphore failure (Pitfall 3); zombie processes from WS exec (Pitfall 7) via AbortHandle tracking and correlation_id

### Phase 4: rc-agent Self-Healing Config Check
**Rationale:** Standalone, no cross-crate dependencies. Adds important startup idempotency before the crash-restart mechanic (Phase 6) amplifies any damage from corrupted config. Can be deployed independently.
**Delivers:** `self_healing.rs` — checks toml parse, bat CRLF, HKLM Run/Task Scheduler key, AV exclusions; repairs via embedded templates; sends `AgentMessage::StartupReport` once WS connects.
**Addresses:** CFGHEAL-01; eliminates root cause #3 broadly; prevents AV quarantine of new binaries (Pitfall 4)
**Uses:** `winreg = "0.55"` crate for registry read/write; `include_str!()` for embedded bat template

### Phase 5: rc-core WS Exec Path + Deploy.rs Changes
**Rationale:** Depends on Phase 1 (ExecResult variant) and Phase 3 (agent actually handles Exec). Can now be tested end-to-end: send Exec from rc-core, receive ExecResult from agent.
**Delivers:** `pending_ws_execs` in AppState; `exec_on_pod_ws()` helper in deploy.rs; dual-path exec (HTTP first, WS fallback on connection refused); rollback logic at 60s health gate; modified do-swap.bat saving `rc-agent-prev.exe`.
**Addresses:** Root cause #5 (no management when HTTP blocked); deploy resilience; eliminates process death race (Pitfall 9) via poll-until-dead loop in do-swap.bat
**Avoids:** WS-only exec for binary download (Anti-Pattern 2) — download stays HTTP; WatchdogState direct access in deploy.rs (Anti-Pattern 3) — deploy.rs reads only its own state fields

### Phase 6: Windows Service / Auto-Restart Install
**Rationale:** Must come after all agent changes are live. Service restarts bring up a fresh agent — that agent needs firewall auto-config (Phase 2) and self-healing (Phase 4) to work correctly on first crash-restart. Installing the service before Phases 2 and 4 are deployed means crash-restarts create broken agents.
**Delivers:** NSSM or Task Scheduler install scripts; deployed via pod-agent HTTP to all 8 pods sequentially; Pod 8 canary crash-restart test before fleet-wide rollout; HKLM Run key replaced.
**Addresses:** SVC-01 (crash restart), SVC-02 (auto-restart semantics); eliminates root cause #4 (HKLM Run key no crash restart)
**Avoids:** Session 0 lock screen invisible (Pitfall 1) — verify `tasklist /v` shows Session# = 1 on Pod 8 before proceeding; HKLM Run key permanence (Pitfall 6) — replaced by Task Scheduler with restart-on-failure

### Phase 7: Fleet Health Dashboard
**Rationale:** Observability layer — can only display meaningful data after Phases 2-6 make that data available (firewall status, startup anomalies, service status, deploy rollback state). Built last because it is read-only and independent; does not affect any existing functionality.
**Delivers:** `fleet_health.rs` rc-core background task (5s broadcast); `DashboardEvent::FleetHealth` over existing kiosk WS; new `/fleet` kiosk route with `FleetGrid.tsx` + `PodHealthCard.tsx`; mobile-first 2-column layout for Uday's phone.
**Addresses:** DASH-01 (fleet health for Uday's phone); VER-01 (agent version visible in dashboard); real-time WS + HTTP status as independent indicators
**Avoids:** Fleet monitoring overhead (Pitfall 11) — dashboard is a read-only AppState view, no new polling loops; separate Next.js app (Anti-Pattern 5) — extends existing kiosk

### Phase Ordering Rationale

- Phase 1 must be first because rc-common is the shared contract. A protocol change mid-development forces simultaneous rebuilds of both crates. Do it once, cleanly.
- Phases 2 and 4 can be developed in parallel (both are isolated rc-agent modules) but must deploy sequentially to Pod 8 before Phase 3 is layered on.
- Phase 3 before Phase 5 because rc-core's WS exec path depends on the agent actually implementing the handler. Testing without the agent side produces silent timeouts.
- Phase 5 before Phase 6 because deploy rollback requires WebSocket exec to be available as the fallback path when :8090 is blocked post-restart.
- Phase 6 before Phase 7 because the service status field in `PodHealthSnapshot` is only meaningful once the service is actually installed.
- Phase 7 last because it depends on all health data flowing through AppState from the preceding phases.

### Research Flags

Phases likely needing deeper investigation during planning:
- **Phase 6 (Windows Service install strategy):** The NSSM vs Task Scheduler vs native ServiceMain decision has product implications. If the lock screen moves to the kiosk (a desirable simplification), native ServiceMain becomes viable and eliminates NSSM as an external dependency. This architectural choice must be settled before Phase 6 planning begins. Recommend Uday sign-off on whether the Session 1 lock screen stays in rc-agent or moves to kiosk.
- **Phase 5 (Deploy rollback trigger):** The 60s health gate currently emails on failure. Rollback trigger changes this to an automatic remediation action. Uday should confirm: automatic rollback always, or alert + manual confirm? The research recommends automatic, but this is a product decision.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Protocol additions):** serde enum extension with `#[serde(other)]` is a well-documented Rust pattern. Direct codebase inspection confirms exact types needed.
- **Phase 2 (Firewall module):** netsh commands, idempotency pattern, and CRLF requirement are fully specified in STACK.md. No unknowns.
- **Phase 3 (WS exec agent side):** exec logic is a direct copy of the existing `remote_ops.rs` pattern with a correlation ID. Well-documented.
- **Phase 4 (Self-healing config):** File existence checks, TOML parse, registry read via winreg — all standard patterns with direct code examples in STACK.md.
- **Phase 7 (Fleet dashboard):** Existing kiosk `useKioskSocket` pattern is confirmed working. New page is additive. No new libraries.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Verified against actual Cargo.toml files and kiosk/package.json. New crates confirmed on crates.io with exact version numbers. NSSM vs native service debate resolved with clear rationale. Zero new npm packages confirmed. |
| Features | HIGH | All P1 features directly map to Mar 15 incident root causes, which are fully documented. P2 features derived from operational patterns at comparable fleet management scale. Anti-features list explicitly derived from what would make things worse. |
| Architecture | HIGH | Based on direct codebase inspection of rc-agent/main.rs (474+ lines), rc-core/deploy.rs, rc-core/ws/mod.rs, rc-common/protocol.rs, rc-core/state.rs, remote_ops.rs, pod_monitor.rs, and all kiosk pages. All architectural claims are derived from reading actual source files. |
| Pitfalls | HIGH | All 11 pitfalls are either directly observed during Mar 15 (Pitfalls 1, 2, 3, 6, 10) or derived from production codebase analysis of code that nearly caused those failures (Pitfalls 4, 5, 7, 8, 9). Cross-referenced with official Microsoft documentation on Session 0 isolation, netsh profile behavior, and AV scanner behavior. |

**Overall confidence:** HIGH

### Gaps to Address

- **Session 1 lock screen vs kiosk decision (Phase 6):** Whether rc-agent retains the Session 1 lock screen or delegates it to the kiosk changes the Windows Service strategy significantly. Resolve before Phase 6 planning. If moving to kiosk: native ServiceMain viable, no NSSM needed. If staying in rc-agent: NSSM or Task Scheduler hybrid required.
- **Automatic vs manual rollback (Phase 5):** Research recommends automatic rollback on 60s health gate failure. This is a product decision — Uday should confirm before the rollback branch is implemented.
- **AV exclusion scope (Phases 4/6):** Startup self-healing verifies and re-applies AV exclusions via winreg. The scope question (directory-wide `C:\RacingPoint\` vs specific file names) has a security tradeoff that needs a decision before implementation.
- **do-swap.bat CRLF in current deploy.rs:** The current inline echo-chain generation in deploy.rs is CRLF-safe (cmd.exe writes the file). The moment any Rust code writes bat file bytes directly, a unit test must assert CRLF. This is a permanent rule that must be documented in the codebase, not just in research.

---

## Sources

### Primary (HIGH confidence — direct codebase inspection)

- `crates/rc-agent/src/main.rs` (474+ lines) — startup sequence order, HKLM Run key, GUI processes, tokio runtime location
- `crates/rc-agent/src/remote_ops.rs` — EXEC_SEMAPHORE (4 slots), try_acquire pattern, CREATE_NO_WINDOW flag, exec timeout (10s default, 120s for downloads)
- `crates/rc-core/src/deploy.rs` — exec_on_pod(), self-swap do-swap.bat generation (inline echo-chain, CRLF-safe), VERIFY_DELAYS, canary Pod 8 pattern
- `crates/rc-core/src/ws/mod.rs` — handle_agent(), agent_senders pattern, all AgentMessage handling
- `crates/rc-common/src/protocol.rs` — CoreToAgentMessage variants (26 listed), AgentMessage variants (18 listed), DashboardEvent
- `crates/rc-core/src/state.rs` — AppState field map: agent_senders, pod_deploy_states, pod_watchdog_states, pending_deploys
- `kiosk/src/hooks/useKioskSocket.ts` — confirmed WebSocket hook at `/ws/dashboard`, bidirectional, handles all pod events
- `kiosk/package.json` — confirmed Next.js 16.1.6, React 19.2.3, Tailwind 4, zero charting libraries
- `.planning/PROJECT.md` — v4.0 requirements, Mar 15 incident motivation

### Primary (HIGH confidence — official documentation)

- windows-service crate: https://crates.io/crates/windows-service (0.8.0, Mullvad VPN)
- winreg crate: https://crates.io/crates/winreg (0.55.0, released 2025-01-12)
- Session 0 isolation: https://learn.microsoft.com/en-us/windows/win32/services/interactive-services
- UI0Detect removed Windows 10 1803: https://www.coretechnologies.com/blog/windows-services/interact-with-desktop/
- netsh advfirewall docs: https://learn.microsoft.com/en-us/troubleshoot/windows-server/networking/netsh-advfirewall-firewall-control-firewall-behavior
- Writing a Windows Service in Rust (Hamann, Feb 2026): https://davidhamann.de/2026/02/28/writing-a-windows-service-in-rust/
- tokio process docs (kill_on_drop, CREATE_NO_WINDOW): https://docs.rs/tokio/latest/tokio/process/struct.Command.html

### Secondary (MEDIUM confidence — community and vendor sources)

- shawl (Rust service wrapper): https://github.com/mtkennerly/shawl — actively maintained alternative to NSSM
- NSSM vs WinSW vs shawl comparison: https://dev.to/aelassas/servy-vs-nssm-vs-winsw-2k46
- Rust community AV false positives: https://users.rust-lang.org/t/anti-virus-deleting-my-executables/80776
- WebSocket exec pattern (Kubernetes): https://jasonstitt.com/websocket-kubernetes-exec
- Health check implementation patterns: https://aws.amazon.com/builders-library/implementing-health-checks/
- Automated rollback on health check failure: https://oneuptime.com/blog/post/2026-02-09-automated-rollback-health-failures/view

### Direct observation (HIGH confidence — first-hand incident)

- Mar 15, 2026: 4-hour debugging session. Pods 1/3/4 offline. All five root causes confirmed during live debugging: CRLF in do-swap.bat, exec slot exhaustion (pre-CREATE_NO_WINDOW fix), missing firewall rules (CRLF cascade), rc-agent crash with no restart (HKLM Run key), Pod 3 one-way connectivity (WS up, HTTP blocked).

---
*Research completed: 2026-03-15*
*Ready for roadmap: yes*
