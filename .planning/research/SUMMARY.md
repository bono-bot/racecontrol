# Project Research Summary

**Project:** v22.0 Feature Management & OTA Pipeline
**Domain:** Runtime feature flags, OTA binary/config delivery, server-to-pod config push, standing rules codification — additions to existing Rust/Axum + Next.js fleet ops platform
**Researched:** 2026-03-23
**Confidence:** HIGH

## Executive Summary

v22.0 adds four related systems on top of an already-operational Rust/Axum + Next.js fleet: a runtime feature flag registry, a server-to-pod config push channel, an OTA binary pipeline, and automated standing rules gates. All four research files converge on the same architectural decision: build everything on top of the existing WebSocket connection between racecontrol and rc-agent, using the existing `CoreToAgentMessage` enum, `AppState` pattern, and `pending_deploys` queuing mechanism. No new ports, no new protocols, and only 3 new crates (`arc-swap`, `notify`, `self-replace`) are needed on top of the current workspace.

The critical design constraint that shapes every decision is that this is an 8-pod LAN venue where active billing sessions must never be interrupted. This rules out naive fleet-wide restarts, makes rollback a session-gated operation, and means the config push system must treat the WebSocket typed message path as the ONLY route for config data — never the fleet exec endpoint (which routes through cmd.exe and will corrupt any value containing spaces, backslashes, or dollar signs). The billing-session drain pattern, recovery system coordination sentinel, and binary content-addressed identity (SHA256 over git hash) are the three highest-priority design decisions that must be locked before any OTA pipeline code is written.

The build order is firmly constrained by dependency: rc-common protocol additions first (7 new message variants), then server-side registry and agent-side consumer in parallel, then the OTA pipeline layer on top, then the admin dashboard UI, then standing rules gate wiring. The Cargo feature gate design must also be settled early because the OTA topology (how many distinct binaries the fleet supports) is set by this decision and cannot be changed after canary infrastructure is built. The clear recommendation: one production binary tier for all pods, with per-pod behavioral differences expressed through the runtime flag registry, not through separate compile-time builds.

## Key Findings

### Recommended Stack

The platform is already Rust 1.93.1 / Axum 0.8 / tokio (full features) / SQLite with WAL / reqwest / sha2 + hex — all of which are directly reused. Only 3 new crates are added. `arc-swap 1.9` (workspace-wide) replaces `Arc<RwLock<Config>>` for lock-free reads in hot paths where multiple WebSocket handlers contend on config access. `notify 8.2` (racecontrol only) watches `racecontrol.toml` for local edits on the server using Windows `ReadDirectoryChangesW`. `self-replace 1.5` (rc-agent only) handles the Windows-safe binary self-swap on pods — it defers deletion of the running exe via a `.__selfdelete__.exe` mechanism that the OS allows.

**Core technologies:**
- `arc-swap 1.9`: Lock-free config hot-swap — eliminates RwLock contention across concurrent WS handlers; 143M downloads, the standard Rust answer to this problem
- `notify 8.2`: File watcher for server-side TOML reload — reactive, zero CPU spin, Windows ReadDirectoryChangesW backend; used by rust-analyzer and cargo-watch
- `self-replace 1.5`: Binary self-swap on rc-agent — handles Windows running-exe constraint atomically with deferred cleanup
- Custom `FeatureRegistry` (rc-common, ~100 lines): Per-pod override map, SQLite-backed, WebSocket-deliverable — external crates (LaunchDarkly, `features` crate) are categorically wrong for this use case
- `tokio::sync::broadcast` (already in workspace): Config push fan-out to all 8 pod WebSocket handlers simultaneously; mpsc would require one sender per pod
- Cargo `[features]` (builtin): Compile-time module exclusion for major optional subsystems; zero runtime cost

### Expected Features

**Must have (table stakes):**
- Named boolean flag registry with fleet-wide default and per-pod override — the core canary testing primitive (FF-1, FF-2)
- Flag delivery over existing WebSocket, in-memory cache in rc-agent, offline fallback to `flags-cache.json` (FF-3, FF-4, FF-5)
- Admin UI toggle with immediate push propagation — the entire value proposition of feature flags (FF-6, FF-7)
- Atomic release manifest locking binary + config + frontend as one versioned bundle (OTA-1)
- Canary-first (Pod 8), health gate after each pod wave, staged 1-then-4-then-3 rollout (OTA-2, OTA-3, OTA-6)
- Auto-rollback on health gate failure with billing session drain before swap (OTA-4, OTA-5)
- Deploy state machine with `deploy-state.json` persistence — a linear bash script cannot handle partial fleet state (OTA-8)
- Config push over WebSocket, offline queue per pod, hot-reload for supported subsystems only (CP-1, CP-2, CP-3)
- Pre-deploy gate script replacing manual Ultimate Rule pre-flight (SR-2)
- Cargo feature gates for major rc-agent modules with single-binary-tier policy (CF-1, CF-2, CF-3)

**Should have (differentiators for this venue context):**
- Billing-aware drain UX in admin dashboard showing "draining" pod status (D-1)
- Config drift detection via configChecksum on health ping, surfacing diverged pods in dashboard (D-3)
- Per-pod flag visibility in fleet health table as a flag divergence column (D-2)
- Post-deploy quick E2E gate after each wave (SR-3)
- Config push audit log append-only DB table (CP-5)

**Defer to v22.x or later:**
- Atomic multi-component rollback across binary + config + frontend simultaneously (D-4)
- Anti-cheat safe mode flags (D-5) — depends on v15.0 AntiCheat milestone being planned
- Full 231-test E2E as post-wave gate — current quick subset is sufficient; full suite adds 10+ min per wave

### Architecture Approach

All v22.0 additions layer onto the existing WebSocket gateway without introducing new transports or ports. AppState gains 3 new `RwLock` fields (`feature_flags`, `pending_config_pushes`, `ota_release_state`). SQLite gains 4 new tables (`feature_flags`, `config_push_log`, `ota_releases`, `ota_pod_status`). The rc-common protocol gains 7 new message variants on existing enums using additive `#[serde(tag = "type")]` — unknown variants are ignored by older agents, ensuring backward compatibility. The admin dashboard gains 2 new pages. All changes are additive; no existing message types, tables, or pages are modified.

Config storage uses a three-tier TOML + SQLite hybrid (per-pod override wins over global override wins over static TOML). TOML remains the install-time baseline and is never hot-reloaded for port bindings, DB path, or WS URL. SQLite overlay is for operational config that staff toggle from the dashboard. If the DB is wiped, pods fall back to TOML — no brick risk.

**Major components:**
1. `feature_registry.rs` (racecontrol) — SQLite-backed flag store, per-pod/global scope, REST CRUD, WS push broadcast
2. `ota_pipeline.rs` (racecontrol) — release manifest ingestion, session-gated canary, staged rollout, health gate, auto-rollback
3. `config_push.rs` (racecontrol) — per-pod config queue, reconnect replay, ack tracking, version counter
4. `standing_rules_gate.rs` (racecontrol) — executable codification of 41+ CLAUDE.md rules in 5 enforcement tiers
5. `rc-agent/feature_flags.rs` — in-memory `Arc<RwLock<HashMap<String, Value>>>` updated via WS push
6. `rc-agent/event_loop.rs` extensions — handlers for ConfigPush, FeatureFlagsUpdate, OtaDownload messages
7. Admin dashboard: feature toggle page + OTA release trigger page (Next.js, REST-backed)

### Critical Pitfalls

1. **OTA restarts an active billing session** — Add `session_state: Idle | Active | Ending` to `PodFleetStatus`. Gate ALL destructive OTA actions on `session_state == Idle`. This must be the first gate checked, not a post-launch hardening item. Do not re-queue a pod the moment its session ends — schedule for the next deployment window.

2. **Recovery systems fight the OTA restarter** — rc-sentry, pod_monitor, self_monitor, and WoL all independently respond to "pod offline." During a 15-second binary swap they will all fire simultaneously. Write sentinel file `C:\RacingPoint\ota-in-progress.flag` before any pod deploy step. rc-sentry must check this before restarting. pod_monitor must suppress WoL when `DeployInProgress` is set in AppState. Verify `build_id` matches manifest after every deploy — if the watchdog won, the old binary is running.

3. **git hash as binary identity triggers unnecessary redeployments** — docs-only commits advance the git hash, making all 8 pods appear outdated. Use `sha256sum rc-agent.exe | cut -c1-16` as binary identity. Store both in the manifest. Gate CI builds on path-filtered changes (`crates/rc-agent/`, `crates/rc-common/`). This failure mode is documented in CLAUDE.md but was not applied to pipeline design.

4. **Config push overwrites manual TOML emergency fixes** — During outage recovery, staff edits TOML on pods. Server comes back and silently pushes the broken server config. Track config with a monotonic version counter. If a pod's counter is greater than the server's stored version, alert instead of overwrite. Default to read-only (report drift) during the first week of operation.

5. **Cargo feature gates create per-pod binary variants** — Using compile-time flags for per-pod runtime differences forces the OTA system to track N binary variants. Policy: one production binary tier for all pods; per-pod behavioral differences go through the runtime flag registry. Document this policy before writing any feature gate code.

6. **cmd.exe quoting corrupts config values** — 4 production incidents to date. Config push must NEVER route through the fleet exec endpoint. WebSocket typed `ConfigPush` message is the only acceptable path. Integration test with values containing spaces (`Pod 3`), backslashes (`C:\RacingPoint\`), and dollar signs before shipping.

7. **Human-observable standing rules auto-passed by health checks** — v17.0 flicker incident: four deploy rounds declared "fixed" based on health endpoint while screens flickered visibly. Classify every standing rule as AUTO, HUMAN-CONFIRM, or INFORMATIONAL before writing any automation. Visual verification rules must be HUMAN-CONFIRM — pipeline PAUSES, issues named checklist, requires explicit `CONFIRM <rule-id>`.

## Implications for Roadmap

Based on combined research, the build order is firmly dependency-constrained. Features and architecture research converge on the same 6-phase sequence.

### Phase 1: Protocol Foundation + Cargo Gates
**Rationale:** rc-common changes block everything downstream. Cargo feature gate decisions lock in OTA topology and cannot be changed after canary infrastructure is built on top of them. Both must land before any server or agent code references new types. The standing rules classification (AUTO/HUMAN-CONFIRM/INFORMATIONAL) should also be done in this phase as a planning artifact.
**Delivers:** 7 new WS message variants on existing enums; 4 new types in rc-common; `[features]` sections in rc-agent and rc-sentry-ai Cargo.toml; single-binary-tier policy documented
**Addresses:** CF-1, CF-2, CF-3
**Avoids:** Pitfall 10 (Cargo feature gate topology debt) — policy must be set here before OTA infrastructure is built on top of it

### Phase 2: Server-Side Registry + Config Foundation
**Rationale:** REST endpoints must exist before the admin dashboard can be built or tested. Both Phase 2 (server) and Phase 3 (agent) depend on Phase 1 but not on each other — they can proceed in parallel if two developers are available; single developer should do server first to get testable endpoints.
**Delivers:** 4 new SQLite tables; `feature_registry.rs`, `config_push.rs`, AppState extensions; REST endpoints for flags, config push, OTA releases; `notify` file watcher for `racecontrol.toml`; config version counter (monotonic integer); `registry_initialized` flag on server startup
**Uses:** `arc-swap` (RaceControlConfig hot-swap); `notify` (TOML file watch)
**Addresses:** FF-1, FF-2 (registry); CP-1, CP-5 (config push + audit log)
**Avoids:** Pitfall 4 (config push overwrites manual TOML) — version counter model; Pitfall 9 (empty allowlist push during server startup) — `registry_initialized` guard

### Phase 3: Agent-Side Consumer
**Rationale:** rc-agent must handle all new WS message variants before the OTA pipeline can send them. The hot-reload scope (which config fields require restart vs. are safe to hot-swap) must be documented and enforced here.
**Delivers:** `RuntimeConfigOverlay` merge logic in `rc-agent/config.rs`; in-memory `FeatureFlagMap`; event_loop handlers for ConfigPush, FeatureFlagsUpdate, OtaDownload; `arc-swap` integration; `self-replace` OTA swap path; `flags-cache.json` offline fallback; sentinel file write before binary swap
**Uses:** `arc-swap` (AgentConfig hot-swap); `self-replace` (Windows binary self-replacement)
**Addresses:** FF-3, FF-4, FF-5 (flag delivery chain); CP-3 (hot-reload); OTA download handler
**Avoids:** Pitfall 7 (cmd.exe quoting) — config data must ONLY flow via WS typed message; Pitfall 2 (recovery systems fight OTA) — sentinel file written before swap, build_id verified after

### Phase 4: OTA Pipeline
**Rationale:** Requires Phase 2 (AppState OTA fields) and Phase 3 (OtaDownload handler in rc-agent). The state machine, session gate, recovery coordination, and rollback must be designed together — none can be "added later" without encountering a production incident first.
**Delivers:** `ota_pipeline.rs` state machine with `deploy-state.json` persistence; session-gated canary (Pod 8 first); staged 1-then-4-then-3 waves; `DeployInProgress` AppState suppression for pod_monitor and WoL; content-addressed binary identity (SHA256); auto-rollback with session-gated per-pod sequencing; `complete_by` rollout deadline in manifest; `self-replace` + `RCAGENT_SELF_RESTART` integration; `deploy.rs` reuse for swap-script mechanism; pre-deploy gate script (SR-2) called before wave 1
**Addresses:** OTA-1 through OTA-8; SR-2
**Avoids:** Pitfalls 1, 2, 3, 5, 6 simultaneously — all are OTA pipeline design decisions that must be made here

### Phase 5: Admin Dashboard UI
**Rationale:** Can begin as soon as Phase 2 REST endpoints are available. OTA trigger UI can use stub endpoints during Phase 4 development. Frontend is fully independent of Phase 3. This delivers operator visibility and control.
**Delivers:** Feature toggle page (per-pod and fleet-wide flag toggles); OTA release trigger page with wave progress display and rollback button; standing rules gate status display; config drift indicator in fleet health table; "draining" pod status for billing-aware OTA
**Implements:** Admin dashboard component (Next.js, REST-backed)
**Addresses:** FF-6, FF-7 (admin UI); D-1 (drain UX); D-2 (per-pod flag visibility)
**Avoids:** No critical pitfalls introduced — purely additive frontend work

### Phase 6: Standing Rules Codification + Gate Wiring
**Rationale:** `standing_rules_gate.rs` must wire into the OTA pipeline (Phase 4). The classification of all 41+ rules as AUTO/HUMAN-CONFIRM/INFORMATIONAL must be done before any automation is written — Pitfall 8 is triggered by mapping human-observable rules to machine checks under schedule pressure.
**Delivers:** `standing_rules_gate.rs` with Tier A/B/C/D/E enforcement; pipeline integration at pre-canary and pre-batch checkpoints; HUMAN-CONFIRM rules as pipeline PAUSE states with named operator checklists; clippy lints in `.cargo/config.toml` (`-D clippy::unwrap_used`); `deploy-staging/gate-check.sh` shell-based checks; CLAUDE.md updated with new `### OTA Pipeline` standing rules section; auth enforcement on feature flag REST endpoints
**Addresses:** SR-1 through SR-5; Pitfall 11 (auth-unprotected flag endpoint)
**Avoids:** Pitfall 8 (human-observable rules auto-passed) — classification done before automation is written

### Phase Ordering Rationale

- Phases 1 through 3 are dependency-ordered by the rc-common protocol — any code referencing new message variants must wait for Phase 1
- Phases 2 and 3 are parallelizable after Phase 1 — server registry and agent consumer both depend on protocol but not on each other
- Phase 4 (OTA pipeline) cannot start until both Phase 2 (AppState OTA state fields) and Phase 3 (OtaDownload handler) are complete — the canary step sends the message and waits for the ack
- Phase 5 (admin UI) can begin as soon as Phase 2 REST endpoints are available, independently of Phases 3 and 4
- Phase 6 (standing rules gate) wires into Phase 4's pipeline, so it comes last; but the rule classification task should be done in Phase 1 as a planning document

### Research Flags

Phases needing deeper research during planning:
- **Phase 4 (OTA Pipeline):** The billing session drain state machine, recovery system sentinel coordination (cross-repo change touching rc-sentry), and rollback session-gated sequencing are all novel to this codebase. Recommend a planning session with `deploy.rs`, `pod_monitor`, and rc-sentry health-check code open. The `complete_by` rollout policy value requires an operator decision from Uday before it is hardcoded.
- **Phase 6 (Standing Rules Gate):** Classifying 41+ rules requires reading each CLAUDE.md rule and making a manual AUTO/HUMAN-CONFIRM/INFORMATIONAL decision. This is a classification task that should not be rushed. Recommend a dedicated classification pass before writing `standing_rules_gate.rs`.

Phases with standard patterns (can skip deeper research):
- **Phase 1 (Protocol Foundation):** Adding enum variants with `#[serde(tag = "type")]` and Cargo feature gates are both from official documentation with established patterns in this codebase.
- **Phase 2 (Server Registry):** SQLite schema additions and RwLock AppState extensions follow the existing `state.rs` pattern exactly. `notify` crate usage is `spawn_blocking` wrapper around `recommended_watcher`.
- **Phase 3 (Agent Consumer):** `arc-swap.store()` and `self-replace::self_replace()` are both simple APIs. The merge logic for `RuntimeConfigOverlay` is standard serde deserialization with `#[serde(default)]`.
- **Phase 5 (Admin UI):** Next.js pages calling REST endpoints — identical pattern to all existing admin dashboard pages.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All 3 new crates verified on docs.rs 2026-03-23; existing workspace dependencies confirmed against live Cargo.toml |
| Features | HIGH | Feature taxonomy from Martin Fowler canonical reference + Memfault OTA checklist; cross-checked against live codebase constraints |
| Architecture | HIGH | All integration points verified against live source: protocol.rs, state.rs, deploy.rs, config.rs, ws/mod.rs |
| Pitfalls | HIGH | Majority derived from live incidents documented in CLAUDE.md and PROJECT.md for this exact codebase — not theoretical |

**Overall confidence:** HIGH

### Gaps to Address

- **Auth pattern on feature flag REST endpoints:** `POST /api/v1/flags` must require auth. The existing auth mechanism in racecontrol (session token vs. PSK vs. admin-only header) needs to be confirmed against live `api/routes.rs` before the endpoint is implemented. Address during Phase 2 planning.
- **rc-sentry sentinel file coordination:** rc-sentry must be modified to check `C:\RacingPoint\ota-in-progress.flag` before triggering a restart. This is a cross-repo change (rc-sentry-ai repository). Confirm rc-sentry's file-read capability and health-check polling interval during Phase 4 planning — the sentinel timeout (currently assumed 60s) must exceed the actual binary swap time on the slowest pod.
- **Rollback session wire protocol compatibility:** Pitfall 6 requires session structs to use `#[serde(default)]` on all new fields so rc-agent N-1 can receive `SessionSync` from server N without data loss. This backward-compat constraint must be applied to any session-related struct changes during Phase 3 and must be reviewed before every future session struct modification.
- **Rollout completion policy value:** Pitfall 5 requires a `complete_by` timestamp in the manifest. The actual policy — hours vs. days, auto-complete vs. alert-and-wait — requires an explicit decision from Uday before it is hardcoded into the pipeline state machine.

## Sources

### Primary (HIGH confidence)
- docs.rs — `arc-swap` 1.9.0, `notify` 8.2.0, `self-replace` 1.5.0 (verified 2026-03-23)
- Official Cargo Book — Features chapter (Rust documentation)
- Tokio official docs — shared state patterns, broadcast channel
- Live racecontrol source: `crates/rc-common/src/protocol.rs`, `crates/racecontrol/src/state.rs`, `crates/racecontrol/src/deploy.rs`, `crates/rc-agent/src/config.rs`, `crates/rc-agent/Cargo.toml`, `crates/racecontrol/src/ws/mod.rs`
- CLAUDE.md standing rules and documented incidents (direct codebase source — same repo)

### Secondary (MEDIUM confidence)
- Martin Fowler — Feature Toggles canonical reference (toggle type taxonomy: release, ops, experiment, permission)
- Memfault — OTA Update Checklist for Embedded Devices (canary, health gates, rollback patterns)
- Unleash — 11 Principles for Feature Flag Systems (per-device targeting, offline fallback)
- WebSocket.org — reconnection state sync and recovery (sequence number replay for offline queuing)

### Tertiary (LOW confidence)
- OpenFeature Specification — standard flag evaluation API (referenced for evaluation API design patterns only, not adopted directly)
- Platform Engineering — Policy as Code in CI/CD Pipelines (gate pattern reference)

---
*Research completed: 2026-03-23 IST*
*Ready for roadmap: yes*
