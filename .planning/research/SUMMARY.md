# Project Research Summary

**Project:** Racing Point Operations — v10.0 Connectivity & Redundancy
**Domain:** Venue ops infrastructure — DHCP stability, health monitoring, config sync, auto-failover, and failback for a Rust/Axum + rc-agent sim racing venue
**Researched:** 2026-03-20 IST
**Confidence:** HIGH

## Executive Summary

v10.0 Connectivity & Redundancy solves a single operational problem: the venue has no automated recovery when the local server (.23) goes down. The current architecture has all pods hardwired to a single WebSocket URL, no external health observer, and a server IP that drifts nightly due to an unconfigured DHCP reservation. The root cause of most weekly ops incidents — IP drift, manual recovery, Uday having to wake James — is addressable with targeted infrastructure changes and one new Rust crate (`sha2`). The Bono VPS already runs a compatible racecontrol instance with Tailscale mesh connectivity to all pods, making this a wiring problem, not a greenfield build.

The recommended approach is strictly dependency-ordered across six phases: (1) fix the DHCP reservation so the health monitor watches a stable address, (2) implement config sync so the cloud failover target has current venue settings, (3) add the `SwitchController` message to rc-agent so pods can be redirected at runtime without a restart, (4) build the failover controller server-side, (5) deploy the standalone `server-monitor` binary on James's workstation to automate detection and triggering, and (6) handle failback data reconciliation. Each phase delivers testable, independently verifiable functionality. Nothing is built against an untested dependency.

The critical risks are not architectural — they are operational. Tailscale SSH server does not work on Windows (confirmed via GitHub issue #14942), so the remote exec path must use rc-agent remote_ops :8090 over Tailscale IP instead. The health monitor threshold must be calibrated high enough to survive normal AC game launches (3-4s CPU spikes) without triggering false-positive failovers that disrupt live sessions. Split-brain billing — two servers simultaneously billing the same session — is the highest-severity failure mode and requires pod-side health confirmation (not just James's view) before any failover is declared.

## Key Findings

### Recommended Stack

See `.planning/research/STACK.md` for full details, code patterns, and alternative rationale.

Only one net-new Cargo dependency is added for the entire milestone: `sha2 = "0.10"` for config file change detection. Every other capability is satisfied by existing crates (`reqwest`, `tokio::sync::watch`, `tokio::process::Command`, `serde_json`) or by infrastructure configuration (DHCP reservation, OpenSSH if available). The constraint "no new language runtimes" is satisfied — everything stays Rust + the existing Node.js `send_email.js` pattern.

**Core technologies:**
- `sha2 0.10.8` (RustCrypto): config file hash for change detection before pushing to Bono — the sole new dependency
- `tokio::sync::watch` (existing): broadcast failover URL changes from the health probe task to the WS reconnect loop in rc-agent — lock-free, zero new crate
- `reqwest 0.12` (existing): HTTP health probes every 5-10s from James .27 to server .23; reused in both `server-monitor` and `failover_monitor`
- `tokio::process::Command` (existing): shell-out to `send_email.js` for failover alerts; same pattern as existing `watchdog.rs`
- TP-Link router DHCP reservation (web UI): pin server .23 to MAC `10:FF:E0:80:B1:A7` — zero code, one-time router config
- OpenSSH or rc-agent :8090 (existing): remote exec path to server for automated restart; OpenSSH is preferred but has a documented component store failure on this machine — rc-agent :8090 is the fallback

### Expected Features

See `.planning/research/FEATURES.md` for the full prioritization matrix and dependency graph.

**Must have (v10.0 MVP — launch together):**
- DHCP reservation for server .23 (MAC 10-FF-E0-80-B1-A7) — prevents IP drift that invalidates all other features; 30-minute router UI task
- Server health monitor on James .27 — HTTP poll `/health` every 5s, DOWN after 3 consecutive failures, UP after 2 consecutive successes; uses `reqwest`, no new deps
- Config sync (racecontrol.toml → Bono VPS) — push sanitized venue config on startup and on file change so cloud failover target knows pod definitions and billing rates
- `core.fallback_url` in rc-agent TOML + `SwitchController` AgentMessage — pods can switch WS target at runtime without restart; deploy to Pod 8 canary first
- Failover trigger + Uday email notification — when health monitor declares .23 DOWN and Bono VPS is healthy, signal pods to switch; email Uday
- Failback trigger + Uday email notification — when .23 recovers, signal pods back to primary WS URL; email Uday

**Should have (v10.x Phase 2 — add after MVP validated):**
- Tailscale SSH to server (via OpenSSH MSI if component store repair succeeds) — remote restart capability for James without physical access
- Split-brain prevention — Bono enters read-only billing mode when .23 sends "I am primary" signal; requires remote exec to server
- Grace period for active sessions — snapshot lap/session state before URL switch to prevent lost laps during mid-session failover
- WhatsApp failover alerts (Evolution API, existing) — mirror email alerts; faster notification for Uday

**Defer (v10.x+):**
- Failover health dashboard — admin panel widget; email notifications cover the operational need at MVP
- Automatic recovery test drill — weekly simulated outage; too risky until MVP is proven stable in production
- Island mode (no connectivity) — pods accept local sessions without billing when both .23 and cloud are unreachable; significant complexity, rare scenario

**Explicitly avoid:**
- DNS-based failover — Windows DNS caching makes propagation 90-120s minimum; direct IP switching in rc-agent gives <15s
- Active-active dual-server — requires distributed consensus on top of SQLite; the existing UUID mismatch in cloud_sync already proves this is architecturally mismatched
- Tailscale SSH server on Windows — not supported (Tailscale GitHub #14942); use Tailscale for IP routing only, exec via rc-agent :8090 or OpenSSH
- Continuous config sync every 30s — config changes weekly at most; push event-driven on startup and file mtime change, not every loop tick
- Full racecontrol.toml sync to cloud — must strip all credentials (JWT secret, API keys, db path) before transmission; `ConfigSnapshot` struct is the correct abstraction

### Architecture Approach

See `.planning/research/ARCHITECTURE.md` for full data flow diagrams, build order, and anti-pattern analysis.

The architecture adds four new components to the existing system with minimal modification to existing files. The core pattern is: an external health observer (James .27) watches the server from outside the failure domain; pods receive `SwitchController` AgentMessages over their existing WS channel to redirect their connections at runtime; Bono's VPS receives config snapshots and pod connections during failover using the existing ws/agent WebSocket protocol with no changes to the shared rc-common protocol beyond one new enum variant.

**Major components:**
1. `server-monitor` (new Rust binary on James .27) — polls `.23:8080/health` every 10s, drives FSM (Healthy→Failover→Recovering→Healthy), triggers failover/failback commands, notifies Uday via email/WhatsApp
2. `failover_controller.rs` (new module in racecontrol) — broadcasts `SwitchController` to all WS-connected pods; rc-sentry :8091 fallback for disconnected pods; registered as `POST /api/v1/admin/failover` and `POST /api/v1/admin/failback`
3. `SwitchController` variant in `CoreToAgentMessage` (rc-common) — `{ new_url: String, reason: String, write_override: bool }` — pods update their in-memory active URL and write a durable override file (`C:\RacingPoint\rc-agent-active-url.txt`) that survives rc-agent restarts
4. `ConfigSnapshot` + config export endpoint (racecontrol on .23) — sanitized venue config (pod definitions, billing rates, no secrets) pushed to Bono on startup and on mtime change via extended `bono_relay.rs`

**Key build order constraint:** DHCP reservation must be verified stable (server survives overnight reboot at .23) before any Rust code is written for phases 2+. The health monitor watching a drifting IP is worse than no health monitor.

### Critical Pitfalls

See `.planning/research/PITFALLS.md` for full recovery strategies, integration gotchas, and the "Looks Done But Isn't" verification checklist.

1. **Tailscale SSH server not supported on Windows** — Tailscale SSH is Linux/macOS only (GitHub #14942). The v10.0 plan references "remote exec via Tailscale SSH" — this must be re-planned. Use rc-agent remote_ops :8090 over Tailscale IP for automated exec, or OpenSSH MSI (not `Add-WindowsCapability` which already failed) for interactive shell. Test on the actual server before designing any phase around SSH. No planning assumption should depend on `tailscale ssh` working on .23.

2. **DHCP reservation assigned to the old MAC** — The server's NIC changed from `BC:FC:E7:2C:F2:CE` to `10:FF:E0:80:B1:A7` on 2026-03-17. Any existing DHCP reservation uses the old MAC and will never match. Verify the current MAC on the server with `ipconfig /all` before touching the router. After creating the reservation, force `ipconfig /release && ipconfig /renew` and then reboot to confirm .23 sticks. Also configure a static IP on the NIC as belt-and-suspenders — DHCP reservation alone has known consumer router bugs where active leases are not overridden.

3. **Split-brain dual billing during failover** — If James's network path to .23 fails (switch port fault, cable issue) but pods can still reach .23 directly, James declares a false outage and commands all pods to switch to cloud. Both servers bill the same active sessions. This is the highest-severity failure mode: billing is double-charged or credits are lost. Mitigation: require at least one pod's own LAN probe to confirm .23 unreachability before triggering fleet-wide failover. James's health check is a necessary but not sufficient condition.

4. **False-positive health check during AC game launch** — AC session spawn is CPU-intensive (3-4s). If the health check timeout is 2s and the threshold is 2 failures, a normal game launch triggers failover. Minimum outage duration for trigger must be 60s continuous; health probe timeout must be 5-10s, not 2s. The existing `WS_DEAD_SECS = 300` in `self_monitor.rs` already accounts for slow boots — health monitor thresholds must be at least as tolerant.

5. **rc-agent dual-connect during failback** — If failback opens a new WS to .23 before the cloud WS is explicitly closed, both servers send commands simultaneously. This causes double-execution of billing END events and phantom lock screen engagements. The connection state machine in rc-agent must have an explicit `DISCONNECTING` state with awaited close before any new connection is opened.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Infrastructure Foundation
**Rationale:** All of v10.0 depends on the server having a stable IP and James having a remote exec path. Neither requires Rust code. These are prerequisites that can be validated in isolation before any development begins. DHCP drift is the root cause of the current weekly ops burden — fixing it first eliminates a failure mode that would corrupt all subsequent testing.
**Delivers:** Server .23 permanently assigned to IP 192.168.31.23 (DHCP reservation + static NIC fallback); remote exec path confirmed (rc-agent :8090 over Tailscale IP, and/or OpenSSH if component store repair succeeds)
**Addresses:** Table stakes "stable server IP" and "remote exec" features from FEATURES.md
**Avoids:** DHCP wrong-MAC pitfall (Pitfall 2), Tailscale SSH on Windows pitfall (Pitfall 1)
**Verification gate:** Reboot server overnight, confirm it gets .23 in the morning. Run `curl http://192.168.31.23:8080/health` from James .27. Phase does not proceed to code until this passes.

### Phase 2: Config Sync
**Rationale:** Config sync is purely additive (push-only from .23 to Bono VPS) with zero failover risk. Building it first validates the James↔Bono communication channel under zero operational pressure. If config sync has problems (auth failures, schema mismatches, Bono-side storage), better to discover them here than during an actual failover event.
**Delivers:** `ConfigSnapshot` type in rc-common; `GET /api/v1/config/export` endpoint on racecontrol; `config_pusher.rs` task that hashes racecontrol.toml (sha2) and pushes to Bono on startup and on mtime change; Bono VPS stores snapshot; cloud racecontrol knows pod count, pod numbers, billing rates
**Uses:** `sha2 0.10.8` (sole new Cargo dependency for all of v10.0), `reqwest` (existing), `bono_relay.rs` extension (existing)
**Implements:** config export endpoint and config_pusher.rs architecture components
**Avoids:** Config sync overwrites outage changes pitfall (Pitfall 6 — add `config_updated_at` timestamp from the start); syncing secrets to cloud (ConfigSnapshot struct excludes credentials by design)

### Phase 3: Pod SwitchController
**Rationale:** The `SwitchController` AgentMessage is the core mechanism that makes failover zero-downtime for active sessions. It touches rc-agent's WS reconnect loop — the most sensitive code in the system. Must be built and validated in isolation (Pod 8 canary) before any automated trigger can use it. If this phase reveals unexpected complexity in the reconnect loop, it does not affect config sync or health monitoring phases.
**Delivers:** `SwitchController` variant in `CoreToAgentMessage`; rc-agent reconnect loop reads from `Arc<RwLock<String>>` (not startup-cached URL); `SwitchController` handler writes durable override file (`rc-agent-active-url.txt`); `self_monitor.rs` suppresses relaunch for 5min after switch; Pod 8 canary validated
**Uses:** `tokio::sync::RwLock` (existing), `CoreToAgentMessage` enum extension (rc-common)
**Implements:** in-memory URL switch with TOML durability pattern (Pattern 1 from ARCHITECTURE.md)
**Avoids:** rc-agent dual-connect pitfall (Pitfall 5 — explicit DISCONNECTING state before new connection); self_monitor relaunch loop risk (WS dead→relaunch→reconnect to dead .23→loop)

### Phase 4: Failover Controller
**Rationale:** The failover controller server-side provides manual trigger capability before any automated detection is connected. Manual testing (trigger failover from James's terminal, verify all 8 pods switch to cloud WS) is a necessary confidence-building step before automation. A manual trigger that fails reveals integration bugs in a controlled way; automated false-positive triggers cause customer disruption.
**Delivers:** `failover_controller.rs` module in racecontrol; `POST /api/v1/admin/failover` and `POST /api/v1/admin/failback` endpoints (X-Admin-Secret auth); broadcast SwitchController to all connected pods; rc-sentry :8091 fallback for pods not WS-connected; manual failover test confirmed end-to-end
**Implements:** Failover controller architecture component, incremental pod switching pattern (Pattern 4 — Pod 8 first, 30s delay, then fleet)
**Avoids:** Split-brain during manual test (only one operator initiating, no simultaneous cloud and local billing)

### Phase 5: Health Monitor Automation
**Rationale:** The `server-monitor` binary on James .27 is the final step that makes failover fully automatic. It depends on Phases 3 and 4 both being proven correct — the trigger mechanism must be reliable before it is automated. First automated test: kill racecontrol on .23 and confirm server-monitor triggers failover within 90s without operator intervention.
**Delivers:** New `crates/server-monitor` Rust binary; 5s health probe loop; FSM (Healthy→Failover→Recovering→Healthy); calls Bono VPS `POST /api/v1/admin/failover` when .23 DOWN + Bono UP confirmed; email notification via `send_email.js`; state persisted to `~/.local/server-monitor.state.json` for crash recovery
**Uses:** `reqwest` (existing), `tokio::time::interval` (existing), `send_email.js` shell-out pattern (existing)
**Avoids:** False-positive failover pitfall (Pitfall 4 — minimum 60s continuous outage, 5-10s probe timeout, time-gated suppression 01:00-03:00 for Windows Update window); health check pitfall "heartbeat piggybacked on WS" (FEATURES.md anti-feature)

### Phase 6: Failback and Data Reconciliation
**Rationale:** Failback is more complex than failover because it requires data integrity — sessions that ran on cloud during the outage must merge back to local without double-billing or phantom session states. The sync-before-accept gate (server refuses rc-agent connections until cloud_sync pull completes) is the most important correctness constraint in the entire milestone.
**Delivers:** Sync-before-accept startup gate in racecontrol (WS listener opens only after cloud_sync pull completes); session state reconciliation on first pod reconnect after failback (compare in-memory state vs synced DB); cloud sessions marked as "failover-mode" for merge tracking; Uday notification on failback; all-pod-reconnected confirmation check
**Avoids:** Failback stale session state pitfall (Pitfall 7); cloud_sync passive sync timing gap for sessions created during outage; dual billing on failed reconciliation

### Phase Ordering Rationale

- Phase 1 is a hard prerequisite for all subsequent phases. A drifting server IP makes health monitoring non-deterministic and failover testing unreliable. Nothing else starts until Phase 1 verification passes.
- Phases 2 and 3 can be developed in parallel by different sessions — they have no shared code. Phase 2 touches racecontrol and bono_relay; Phase 3 touches rc-agent and rc-common. They share only the `ConfigSnapshot` type in rc-common which can be stubbed.
- Phase 4 requires Phase 3 complete (pods must support SwitchController before the controller can broadcast it).
- Phase 5 requires Phase 4 complete (failover endpoint must exist before server-monitor calls it).
- Phase 6 requires Phase 5 complete (failback is triggered by server-monitor, and reconciliation logic must handle sessions created during an automated failover scenario).
- This ordering follows the architecture build order from ARCHITECTURE.md exactly, which was derived from code dependency analysis.

### Research Flags

Phases needing deeper research or explicit verification during planning:

- **Phase 1 (Remote Exec):** OpenSSH on server .23 has a documented component store failure. Before any planning assumes OpenSSH, run `winget install Microsoft.OpenSSH.Beta` on the server and confirm it succeeds. If it fails, the remote exec path is rc-agent :8090 (already deployed on server per MEMORY.md). The plan must be explicit about which path it depends on.
- **Phase 3 (SwitchController):** rc-agent reconnect loop is complex (`reconnect_delay_for_attempt()` backoff, `self_monitor.rs` relaunch interaction). The architecture research identified a specific loop risk (relaunch→reconnect to dead .23→WS dead→relaunch loop). The implementation plan must explicitly address this interaction and include a test that confirms the loop does not occur.
- **Phase 6 (Failback Reconciliation):** The session sync direction "cloud → local for sessions created during failover" is not currently implemented in `cloud_sync.rs`. The planning session must determine whether this extends the existing SYNC_TABLES mechanism or requires a new dedicated reconciliation endpoint on Bono's VPS. This is the highest architectural uncertainty in the milestone.

Phases with standard patterns (skip research-phase):

- **Phase 1 (DHCP):** TP-Link DHCP reservation procedure is documented in the official FAQ. MAC verification via `ipconfig /all` is standard. No research needed — this is a router UI change with a clear verification step.
- **Phase 2 (Config Sync):** `sha2` integration is trivial (compute hash, POST if different). Bono relay extension follows the existing `X-Relay-Secret` auth pattern exactly. No research needed.
- **Phase 4 (Failover Controller):** Axum route registration and WebSocket broadcast to connected pods follows existing patterns in racecontrol (see `pod_monitor.rs`, `main.rs` router setup). No research needed.
- **Phase 5 (Server Monitor):** Probe loop with hysteresis is a standard async Rust pattern already used in `cloud_sync.rs` (`RELAY_DOWN_THRESHOLD=3`, `RELAY_UP_THRESHOLD=2`). The FSM is documented in full in ARCHITECTURE.md. No research needed beyond validating the failover endpoint address before writing the trigger call.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Only new dependency is `sha2` (393M downloads, RustCrypto ecosystem). All other capabilities use existing crates verified in the codebase. Single uncertainty: OpenSSH component store repair on server — but rc-agent :8090 is a confirmed fallback. |
| Features | HIGH | Feature set derived from concrete operational gaps (DHCP drift history, no health observer, no fallback URL). MVP scope is minimal and tightly bounded. The FEATURES.md dependency graph is internally consistent. |
| Architecture | HIGH | All integration points verified by reading actual source files (bono_relay.rs, cloud_sync.rs, self_monitor.rs, rc-agent/main.rs, rc-common/protocol.rs). The loop risk in Phase 3 is identified and documented — not a gap, but a design constraint to address. |
| Pitfalls | HIGH | Every critical pitfall is derived from a documented past failure on this exact hardware or a confirmed external source (Tailscale GitHub issue). No hypothetical risks — all concrete. |

**Overall confidence:** HIGH

### Gaps to Address

- **OpenSSH availability on server .23:** `Add-WindowsCapability` already failed once (MEMORY.md). The alternative (`winget install Microsoft.OpenSSH.Beta` or offline MSI) has not been tested. Treat remote exec via OpenSSH as unconfirmed until verified on the server. Phase 1 must include a "confirm exec path" step with an explicit go/no-go on OpenSSH before any automated recovery design assumes it is available.
- **Pod Tailscale IPs for failover URL:** The failover WS URL for Bono's VPS uses Bono's Tailscale IP (100.x.x.x). This IP must be confirmed before updating any rc-agent TOML config — Tailscale IPs are stable per device but must be looked up from the Tailscale admin console or `tailscale ip` on Bono's VPS.
- **Failback session reconciliation scope:** `cloud_sync.rs` SYNC_TABLES includes billing_rates, drivers, wallets — but NOT session events (sessions that started and ended on cloud during failover). The implementation must define whether to extend SYNC_TABLES or add a dedicated reconciliation push from Bono. This decision affects both the cloud_sync schema and the Bono-side API surface.
- **Split-brain prevention mechanism:** The research recommends pod-side health confirmation before fleet-wide failover. The exact implementation (pod reports its own LAN probe result to server-monitor, or server-monitor polls each pod's :8090 as a secondary signal) was not specified. This must be resolved during Phase 5 planning before writing the failover trigger logic.

## Sources

### Primary (HIGH confidence)
- Existing codebase read directly (2026-03-20 IST): `bono_relay.rs`, `cloud_sync.rs`, `self_monitor.rs`, `config.rs`, `state.rs`, `rc-agent/main.rs`, `rc-common/protocol.rs`, `rc-sentry/main.rs`, `pod_monitor.rs`
- [Tailscale SSH GitHub issue #14942](https://github.com/tailscale/tailscale/issues/14942) — Tailscale SSH server not supported on Windows (confirmed by Tailscale team)
- [sha2 crates.io](https://crates.io/crates/sha2) — 0.10.8 current stable, RustCrypto ecosystem, 393M downloads
- [tokio::sync::watch docs.rs](https://docs.rs/tokio/latest/tokio/sync/watch/index.html) — lock-free broadcast pattern for frequently-read, rarely-written values
- [Microsoft Learn: OpenSSH for Windows](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_install_firstuse) — `Add-WindowsCapability` install path, alternative `winget` option
- [TP-Link DHCP Address Reservation FAQ](https://www.tp-link.com/us/support/faq/182/) — reservation procedure and active lease behavior
- [Tailscale High Availability Docs](https://tailscale.com/kb/1115/high-availability) — failover timing ~15s, threshold calibration
- [AWS ELB Health Check Concepts](https://docs.aws.amazon.com/elasticloadbalancing/latest/network/target-group-health-checks.html) — interval/threshold patterns for health check design
- MEMORY.md — server MAC change history, DHCP drift timeline, OpenSSH component store failure, Tailscale Phase 27 status, VPS address

### Secondary (MEDIUM confidence)
- [IBM DNS Failover Limitations](https://www.ibm.com/think/topics/dns-failover) — Windows DNS caching makes DNS-based failover unsuitable for LAN
- [StarWind Split-Brain Prevention](https://www.starwindsoftware.com/blog/whats-split-brain-and-how-to-avoid-it/) — heartbeat-based split-brain detection patterns
- [OneUptime Rust Health Check Endpoints (2026)](https://oneuptime.com/blog/post/2026-01-25-health-check-endpoints-dependencies-rust/view) — dependency-aware /health endpoint patterns
- [HAProxy Health Check Tutorial](https://www.haproxy.com/documentation/haproxy-configuration-tutorials/reliability/health-checks/) — 3-5s intervals, 2-3 failure threshold calibration

### Tertiary (LOW confidence)
- TP-Link Community forums — DHCP reservation ignored for active leases (consumer router behavior; not officially documented but confirmed in practice)

---
*Research completed: 2026-03-20 IST*
*Ready for roadmap: yes*
