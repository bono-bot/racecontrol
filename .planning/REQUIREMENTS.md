# Requirements: Racing Point Operations — v22.0 Feature Management & OTA Pipeline

**Defined:** 2026-03-23
**Core Value:** Seamlessly integrate, remove, and update features across the entire RaceControl fleet without reinstalling programs — with every standing rule enforced as an automated gate at every step.

## v22.0 Requirements

### Feature Flags

- [ ] **FF-01**: Server maintains a central named boolean feature flag registry backed by SQLite with fleet-wide defaults
- [ ] **FF-02**: Operator can set per-pod flag overrides (e.g., enable AC EVO telemetry on Pod 8 only for canary testing)
- [ ] **FF-03**: Flag changes are delivered to pods over the existing WebSocket connection as typed messages — no new ports or protocols
- [ ] **FF-04**: rc-agent caches flags in-memory (Arc<RwLock>) for synchronous reads in hot paths (game launch, billing guard) — no server round-trip per flag check
- [ ] **FF-05**: rc-agent persists last-received flags to flags-cache.json and reads them on startup before server connects — pods operate with last-known flags when offline
- [ ] **FF-06**: Admin dashboard has a Feature Flags section with toggle switches and per-pod scope selector
- [ ] **FF-07**: Flag changes propagate to all connected pods within seconds of admin toggle — no deploy or restart required
- [ ] **FF-08**: Kill switch flags (named kill_*) are evaluated before all other flag logic and take priority over normal flag hierarchy

### Config Push

- [ ] **CP-01**: Server pushes config changes (billing rates, game limits, process guard entries, debug verbosity) to pods over WebSocket as typed ConfigPush messages — never through fleet exec endpoint
- [ ] **CP-02**: Server maintains a pending config queue per pod — offline pods receive queued updates on WebSocket reconnect with sequence-number-based ack
- [ ] **CP-03**: rc-agent hot-reloads supported config fields (billing rates, game limits, process guard whitelist, debug verbosity) without binary restart using arc-swap — fields requiring restart (port bindings, WS URL) are documented and excluded
- [ ] **CP-04**: Config push includes schema version — rc-agent ignores unknown fields from newer schema versions and logs a warning instead of panicking
- [ ] **CP-05**: All config changes are recorded in an append-only audit log table (timestamp, field, old_value, new_value, pushed_by, pods_acked)
- [ ] **CP-06**: Server validates config changes against schema before accepting — invalid values (negative billing rate, empty allowlist) return 400 with field-level errors and are never pushed to pods

### OTA Pipeline

- [ ] **OTA-01**: Releases are defined by an atomic manifest (release-manifest.toml) locking binary SHA256 hash, config schema version, frontend build_id, git commit, and timestamp as one versioned bundle
- [ ] **OTA-02**: OTA pipeline always deploys to canary Pod 8 first and waits for health gate pass before proceeding to other pods
- [ ] **OTA-03**: Health gate runs after each pod wave — checks WS connected, HTTP reachable, binary SHA256 matches manifest, no error spike in logs
- [ ] **OTA-04**: Pipeline auto-rolls back affected pods on health gate failure — swaps to previous binary (rc-agent-prev.exe) and triggers RCAGENT_SELF_RESTART
- [ ] **OTA-05**: OTA pipeline gates all destructive operations on billing session state — pods with active sessions defer binary swap until session ends or checkpoint session state to DB before swap
- [ ] **OTA-06**: Staged wave rollout: wave 1 = canary (Pod 8), wave 2 = 4 pods, wave 3 = remaining pods — each wave waits for health gate before proceeding
- [ ] **OTA-07**: Previous binary (rc-agent-prev.exe) is always preserved on each pod — never overwritten by the swap step — enabling one-command manual rollback
- [ ] **OTA-08**: OTA pipeline is implemented as a state machine (idle, building, staging, canary, staged-rollout, health-checking, completed, rolling-back) with state persisted to deploy-state.json — can resume interrupted deploys
- [ ] **OTA-09**: Recovery systems (rc-sentry, pod_monitor, WoL) are coordinated via sentinel file (ota-in-progress.flag) — all recovery systems check this file before triggering restarts during OTA
- [ ] **OTA-10**: Binary identity uses SHA256 content hash (not git commit hash) — prevents false redeploy triggers from docs-only commits

### Cargo Feature Gates

- [x] **CF-01**: rc-agent Cargo.toml has feature flags for major optional modules (ai-debugger, process-guard) — modules can be compiled out entirely. Telemetry excluded: too deeply woven into billing/game state machine to gate.
- [x] **CF-02**: Default features = full production build — no manual flag selection required for standard pod deploy
- [x] **CF-03**: CI verifies both default and minimal (--no-default-features) builds compile cleanly for both rc-agent and rc-sentry
- [x] **CF-04**: rc-sentry Cargo.toml has feature flags for optional modules (watchdog, tier1-fixes, ai-diagnosis) — bare binary with all features off is a remote-exec-only tool on port 8091

### Protocol Forward-Compatibility

- [x] **PFC-01**: AgentMessage and CoreToAgentMessage enums have an Unknown catch-all variant with #[serde(other)] — older binaries silently ignore new message types instead of crashing on deserialization. Must be deployed to all pods BEFORE any new message variants are added.

### Standing Rules Codification

- [ ] **SR-01**: All 41+ CLAUDE.md standing rules are classified into enforcement types: AUTO (linter/compiler/script), HUMAN-CONFIRM (pipeline pauses for operator checklist), INFORMATIONAL (documented but not gated)
- [ ] **SR-02**: Pre-deploy gate script (gate-check.sh) runs before any binary leaves staging — checks: cargo test green, no unwrap in diff, static CRT config present, LOGBOOK updated, bat files clean ASCII — blocks deploy on failure
- [ ] **SR-03**: Post-deploy verification gate runs after each wave — checks: build_id matches manifest, fleet health passes, billing session roundtrip works, no error spike — blocks next wave on failure
- [ ] **SR-04**: Pipeline has no force-continue or skip-gate commands — the only exit from a failed health check is rollback
- [ ] **SR-05**: New standing rules are added to CLAUDE.md covering: always preserve prev binary, never deploy without manifest, rollback window defined, billing sessions drain before swap, OTA sentinel file protocol, config push never through fleet exec
- [ ] **SR-06**: HUMAN-CONFIRM rules (visual verification, customer audit, anomaly investigation) cause pipeline to PAUSE with a named operator checklist — pipeline resumes only on explicit operator confirmation
- [ ] **SR-07**: Standing rules sync to Bono after any modification — both AIs operate under identical rules

### Cross-Project Sync (builds on v21.0)

- [ ] **SYNC-01**: Feature flag and config push APIs are documented in the OpenAPI 3.0 spec (extends v21.0 Phase 173 contract) with shared TypeScript types in packages/shared-types/
- [ ] **SYNC-02**: OTA pipeline uses the unified deploy scripts (deploy.sh, check-health.sh) from v21.0 as its health gate foundation — extends, not replaces
- [ ] **SYNC-03**: New WebSocket message types (FlagSync, ConfigPush, OtaDownload, etc.) are added to rc-common AND to the shared TypeScript types package — contract tests verify both sides match
- [ ] **SYNC-04**: Feature flag and config push changes cascade to ALL affected components (racecontrol, rc-agent, kiosk, admin dashboard, API gateway) per the cross-process update standing rule
- [ ] **SYNC-05**: OTA release manifest includes version compatibility matrix — which rc-agent version works with which racecontrol version, which kiosk build, which config schema
- [ ] **SYNC-06**: Standing rules gate script (gate-check.sh) extends the v21.0 E2E test framework (run-all.sh) — not a separate test system

### Cross-Milestone Integration

- [ ] **XMIL-01**: v6.0 Salt Fleet Management phases 36-40 reviewed — config distribution aspects superseded by v22.0 config push are marked, Salt scope narrowed to remote exec only or deprecated
- [ ] **XMIL-02**: v10.0 Phase 62 (Fleet Config Distribution) marked superseded by v22.0 CP-01 to CP-06 — no duplicate config push system exists
- [ ] **XMIL-03**: v13.0 Multi-Game Launcher incomplete phases updated to use Cargo feature gates (CF-01) for game telemetry modules and feature flags (FF-01) for per-pod game enablement
- [ ] **XMIL-04**: v15.0 Phase 111 (Code Signing + Canary) updated to use OTA-10 (SHA256 binary identity) and OTA-02 (canary Pod 8) — no duplicate canary infrastructure
- [ ] **XMIL-05**: v17.0 Phase 127 (CI/CD Pipeline) updated to use OTA-08 (deploy state machine) — cloud and local deploy share the same pipeline architecture
- [ ] **XMIL-06**: All future phases across all milestones include standing rules gate dependency — no phase ships without gate-check.sh

## v22.x Requirements (Deferred)

### Advanced OTA
- **OTA-D1**: Atomic multi-component rollback across binary + config + frontend simultaneously
- **OTA-D2**: Full 231-test E2E as post-wave gate (quick subset sufficient for v22.0)
- **OTA-D3**: Anti-cheat safe mode flags (depends on v15.0 AntiCheat milestone)

### Advanced Config
- **CP-D1**: Config drift auto-heal (detect diverged pods and auto-push correct config)
- **CP-D2**: A/B config testing (different config values on different pod groups)

### Advanced Flags
- **FF-D1**: Percentage rollout (enable flag on N% of pods)
- **FF-D2**: Time-based flag scheduling (enable flag at specific time, disable after)

## Out of Scope

| Feature | Reason |
|---------|--------|
| External feature flag service (LaunchDarkly, Unleash) | LAN-only venue, 8 pods — external SaaS adds latency, cost, and network dependency for no benefit |
| A/B testing framework | 8 pods is too small a sample; per-pod override covers the canary use case |
| Blue-green deployment (duplicate fleet) | Only 8 physical pods; no hardware to double |
| Kubernetes/container orchestration | Windows pods running bare-metal Rust binaries — containerization adds complexity without value |
| Mobile OTA (app store) | No mobile apps in scope |
| Feature flag SDK (OpenFeature) | Over-engineered for 8-pod LAN fleet; custom ~100-line FeatureRegistry in rc-common is the right abstraction |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CF-01 | Phase 176 | Complete |
| CF-02 | Phase 176 | Complete |
| CF-03 | Phase 176 | Complete |
| CF-04 | Phase 176 | Complete |
| PFC-01 | Phase 176 | Complete |
| FF-01 | Phase 177 | Pending |
| FF-02 | Phase 177 | Pending |
| FF-03 | Phase 177 | Pending |
| CP-01 | Phase 177 | Pending |
| CP-02 | Phase 177 | Pending |
| CP-04 | Phase 177 | Pending |
| CP-05 | Phase 177 | Pending |
| CP-06 | Phase 177 | Pending |
| SYNC-01 | Phase 177 | Pending |
| FF-04 | Phase 178 | Pending |
| FF-05 | Phase 178 | Pending |
| FF-07 | Phase 178 | Pending |
| FF-08 | Phase 178 | Pending |
| CP-03 | Phase 178 | Pending |
| SYNC-03 | Phase 178 | Pending |
| OTA-01 | Phase 179 | Pending |
| OTA-02 | Phase 179 | Pending |
| OTA-03 | Phase 179 | Pending |
| OTA-04 | Phase 179 | Pending |
| OTA-05 | Phase 179 | Pending |
| OTA-06 | Phase 179 | Pending |
| OTA-07 | Phase 179 | Pending |
| OTA-08 | Phase 179 | Pending |
| OTA-09 | Phase 179 | Pending |
| OTA-10 | Phase 179 | Pending |
| SYNC-02 | Phase 179 | Pending |
| SYNC-05 | Phase 179 | Pending |
| FF-06 | Phase 180 | Pending |
| SYNC-04 | Phase 180 | Pending |
| SR-01 | Phase 181 | Pending |
| SR-02 | Phase 181 | Pending |
| SR-03 | Phase 181 | Pending |
| SR-04 | Phase 181 | Pending |
| SR-05 | Phase 181 | Pending |
| SR-06 | Phase 181 | Pending |
| SR-07 | Phase 181 | Pending |
| SYNC-06 | Phase 181 | Pending |
| XMIL-01 | Phase 182 | Pending |
| XMIL-02 | Phase 182 | Pending |
| XMIL-03 | Phase 182 | Pending |
| XMIL-04 | Phase 182 | Pending |
| XMIL-05 | Phase 182 | Pending |
| XMIL-06 | Phase 182 | Pending |

**Coverage:**
- v22.0 requirements: 48 total
- Mapped to phases: 48
- Unmapped: 0

---
*Requirements defined: 2026-03-23*
*Last updated: 2026-03-24 after adding rc-sentry Cargo feature gates (CF-04)*
