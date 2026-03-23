# Feature Research: v22.0 Feature Management & OTA Pipeline

**Domain:** Feature flag registry, OTA update pipeline, fleet config distribution, standing rules codification
**Researched:** 2026-03-23
**Confidence:** HIGH (Martin Fowler feature toggles canonical reference, Memfault OTA best practices, Unleash architecture docs, OpenFeature spec, policy-as-code literature; cross-checked against existing racecontrol codebase constraints)

---

## Context: What Already Exists (Do Not Re-Build)

| Already in production | What v22.0 builds on top of |
|-----------------------|------------------------------|
| TOML static config (racecontrol.toml, rc-agent.toml, rc-sentry-ai.toml) — requires restart on change | Runtime config push replaces manual TOML edits |
| Manual binary deploy: scp + taskkill + restart, canary (Pod 8 first) | OTA pipeline formalizes this pattern with health gates and auto-rollback |
| RCAGENT_SELF_RESTART sentinel for graceful pod agent restart | OTA pipeline hooks into this — no new restart mechanism needed |
| Fleet health endpoint: GET /api/v1/fleet/health with build_id, uptime, version per pod | Health gate evaluates this endpoint post-deploy |
| Admin dashboard (Next.js) for staff operations | Feature flag UI is a new section in this existing dashboard |
| Existing WebSocket connection between server and each rc-agent | Config push and feature flag delivery uses this — no new transport |
| rc-common shared types/protocol crate | Flag registry types, release manifest, config push message types go here |
| 41+ standing rules in CLAUDE.md | Automated pipeline gates replace manual verification of each rule |
| comms-link relay for James↔Bono AI coordination | Cloud services OTA goes through Bono; relay used for cross-machine coordination |

---

## The Three-Way Distinction (Critical)

These three concepts are commonly conflated. They are not the same system and must not be collapsed into one.

| Concept | What changes | Delivery | Restart required | Rollback mechanism |
|---------|-------------|----------|------------------|--------------------|
| **Feature flag** | Which code path is taken at runtime | WebSocket push, persisted in server DB | No | Set flag back to old value |
| **Runtime config** | Parameters (rates, timeouts, thresholds) | WebSocket push, persisted in TOML + DB | No (hot-reload via Arc<RwLock>) | Push previous config version |
| **OTA binary update** | The compiled binary itself | Download + sentinel restart | Yes (RCAGENT_SELF_RESTART) | Swap back previous binary file |

Conflating these leads to: feature flags that require restarts (wrong), config changes that accidentally deploy new code (wrong), or OTA pipelines that try to toggle individual features (over-engineered).

---

## Feature Landscape

### Table Stakes: Feature Flag System

Features that any production feature flag system must have. Missing these makes the system untrustworthy for operators.

| # | Feature | Why Expected | Complexity | Dependency |
|---|---------|--------------|------------|------------|
| FF-1 | **Central flag registry with named boolean flags** | Every fleet management system (Unleash, LaunchDarkly, FeatBit) has a named registry. Without a registry, flags are scattered ad-hoc strings with no single source of truth. Admin dashboard and rc-agent must both read from the same registry. | LOW | New table `feature_flags(name, enabled, pod_id_override, updated_at)` in racecontrol SQLite DB |
| FF-2 | **Fleet-wide default + per-pod override** | A flag disabled fleet-wide must still be activatable on Pod 8 (canary) only. This is the core canary testing pattern. Without per-pod override, every flag change is fleet-wide and there is no safe testing path. | MEDIUM | Override row has `pod_id` column (NULL = all pods). rc-agent receives its own pod_id-specific flag set on connect. |
| FF-3 | **Flag delivery over existing WebSocket (no new ports)** | Explicit constraint from milestone: feature toggles must work over existing WebSocket agent connection. This is also good engineering — one protocol, one reconnect logic, no firewall changes. | MEDIUM | New WebSocket message type `FlagSync { flags: HashMap<String, bool> }` in rc-common. Sent on connect and on change. |
| FF-4 | **In-memory flag read on pod (no server round-trip per request)** | Flag evaluation must be synchronous in rc-agent hot paths (game launch, billing guard). A network call per flag evaluation would add latency on every game event. Cache flags locally in `AppState`, update via WS message. | LOW | `Arc<RwLock<HashMap<String, bool>>>` in rc-agent AppState. Already exists as a pattern (billing rate cache refreshes every 60s). |
| FF-5 | **Offline pod fallback to last-known flags** | Pods must operate when server is unreachable. Last-received flags must persist in rc-agent.toml or local state file and be read on startup. Missing = pod stalls on unknown flag state at startup. | MEDIUM | Persist flags to `C:\RacingPoint\flags-cache.json` on every WS sync. Read on startup before server connects. |
| FF-6 | **Admin UI to toggle flags per-pod or fleet-wide** | Operators must not edit the database directly to change flags. The admin dashboard already exists — add a Flags section with toggle switches and per-pod scope selector. | MEDIUM | New Next.js page section in racingpoint-admin. Calls new REST endpoints: `GET /api/v1/flags`, `POST /api/v1/flags/{name}`. |
| FF-7 | **Flag change propagates immediately (no deploy, no restart)** | This is the entire value of feature flags. If changing a flag requires a restart, it is just a config file by another name. Propagation must be push-based over WebSocket within seconds of admin toggle. | MEDIUM | Server broadcasts `FlagSync` message to all connected pods on every flag write. |
| FF-8 | **Kill switch capability (disable flag overrides flag hierarchy)** | Kill switches are the emergency brake. A kill switch must be evaluatable even if the normal flag evaluation path has a bug. Pattern: kill switches are checked before all other flag logic. | LOW | Convention: flags named `kill_*` always take priority. Checked first in flag evaluation logic. |

### Table Stakes: OTA Pipeline

Features that any production OTA system must have. Without these, OTA is less reliable than the current manual deploy.

| # | Feature | Why Expected | Complexity | Dependency |
|---|---------|--------------|------------|------------|
| OTA-1 | **Atomic release manifest (binaries + config + frontend as one versioned bundle)** | Deploying binary v1.2 with config expecting v1.1 fields causes runtime errors. A release manifest locks binary version + config schema version + frontend build_id into one artifact. Roll back the manifest = roll back all three. | HIGH | New `release-manifest.toml` format: `{ version, git_sha, rc_agent_binary_hash, racecontrol_binary_hash, config_schema_version, frontend_build_id, timestamp }`. Checked into `deploy-staging/` per release. |
| OTA-2 | **Canary-first to Pod 8 (already the pattern — formalize it)** | Pod 8 canary is already the established rule in CLAUDE.md. OTA formalizes it: pipeline always deploys to Pod 8 first, waits for health gate pass before continuing. | LOW | Pipeline config: `canary_pods = [8]`, `canary_wait_secs = 60`, `canary_health_threshold = 1.0` (100% pod health required). |
| OTA-3 | **Health gate after each pod wave (not just Pod 8)** | Staged rollout patterns (Memfault, Google Play, mobile OTA) all require success thresholds before expanding. Health gate checks: WS connected, HTTP reachable, version == expected, build_id == manifest, no error spike. | MEDIUM | `check-health.sh` already exists from v21.0. OTA pipeline calls it between waves. New: version/build_id assertion against manifest. |
| OTA-4 | **Auto-rollback on health gate failure** | Manual rollback during an incident is slow and error-prone. If health gate fails within a configurable window, the pipeline must automatically push the previous manifest to affected pods. This is the safety net that makes OTA trustworthy. | HIGH | A/B binary slot pattern: keep `rc-agent-prev.exe` alongside `rc-agent.exe`. Rollback = swap and RCAGENT_SELF_RESTART. Rollback manifest = previous release-manifest.toml. |
| OTA-5 | **Billing session preservation during rollback** | Explicit constraint from milestone: rollback must never lose active session data. A rollback during an active billing session must preserve the session (in-memory state or DB flush before restart). | HIGH | Drain pattern: rc-agent defers binary swap until current session ends, or checkpoints session state to DB before swapping. |
| OTA-6 | **Staged wave rollout (canary → half fleet → full fleet)** | Rolling all 8 pods simultaneously is the current worst-case. Staged rollout lets a bad deploy affect 1 pod, not 8. Standard pattern: wave 1 = canary (Pod 8), wave 2 = 4 pods, wave 3 = remaining pods. | MEDIUM | Pipeline script drives waves. After canary passes, prompt (or auto after timeout) before wave 2. After wave 2 passes, proceed to wave 3. |
| OTA-7 | **Previous binary preserved for one-command rollback** | If auto-rollback fails or was not triggered, operators need a manual escape hatch. `rc-agent-prev.exe` on each pod = one command to revert without re-downloading. | LOW | Already partially true (old binary remains until next deploy). Formalize: `rc-agent-prev.exe` is ALWAYS preserved, never overwritten by the swap step. |
| OTA-8 | **Deploy state machine (not a linear bash script)** | OTA for a fleet has states: idle, building, staging, canary, staged-rollout, health-checking, completed, rolling-back. A state machine prevents partial-state bugs (half the fleet on new version, half on old, pipeline crashes). | HIGH | Rust or Node.js state machine in `deploy-staging/`. Persists state to `deploy-state.json`. Can resume interrupted deploys. |

### Table Stakes: Config Push

| # | Feature | Why Expected | Complexity | Dependency |
|---|---------|--------------|------------|------------|
| CP-1 | **Server-to-pod config push over WebSocket (no manual TOML editing)** | Editing TOML on 8 pods via scp is the status quo. Any config change to billing rates, game limits, process guard entries requires manual fleet touch. Config push is the replacement. | MEDIUM | New WS message `ConfigUpdate { version: u32, fields: HashMap<String, Value> }`. rc-agent applies to in-memory config without restart. |
| CP-2 | **Offline queue: config updates persist until pod reconnects** | A pod that is offline when a config change is pushed must receive the change when it reconnects. Without queuing, offline pods silently diverge from the configured state. | MEDIUM | Server maintains pending config queue per pod_id. On WS reconnect, server sends all queued updates. rc-agent acks each with sequence number. |
| CP-3 | **Hot-reload for runtime config without binary restart** | The milestone explicitly requires hot-reload where possible. Pattern: `Arc<RwLock<Config>>` in rc-agent AppState. WS handler acquires write lock, updates fields, releases. Next request reads updated value. | MEDIUM | Fields that can hot-reload: billing rates, game session limits, process guard whitelist entries, debug verbosity. Fields that cannot: port bindings, WS server address (require restart — document this explicitly). |
| CP-4 | **Config schema version + conflict resolution** | If server pushes config v3 to a pod still running rc-agent binary v2 (which only understands config v2), unknown fields should be ignored, not cause a panic. Config version mismatch must be logged and reported to admin dashboard. | MEDIUM | `config_schema_version` field in release manifest. rc-agent only applies config fields it knows about. Unknown fields: warn + ignore. |
| CP-5 | **Config push audit log** | Who changed what and when. Required for operational accountability. If a config change caused an incident, the audit trail shows what changed 5 minutes before. | LOW | Append-only log table `config_changes(timestamp, field, old_value, new_value, pushed_by, pods_acked)` in racecontrol DB. |
| CP-6 | **Admin validation before push (schema check, range check)** | Wrong config pushed to pods must be caught before reaching pods. Server validates config change against schema before accepting it. Example: billing rate of 0 or negative must be rejected. | LOW | Input validation in REST endpoint handler. Return 400 with field-level error messages. Never push invalid config to pods. |

### Table Stakes: Cargo Feature Gates

| # | Feature | Why Expected | Complexity | Dependency |
|---|---------|--------------|------------|------------|
| CF-1 | **Cargo feature flags for major modules in rc-agent** | Modules like telemetry, AI debugger, process guard, and camera AI are large and optional. Compile-time gates allow building a minimal rc-agent binary for pods that don't need all features (e.g., a pod without a camera for AI). Reduces binary size and attack surface. | MEDIUM | Add `[features]` section to `crates/rc-agent/Cargo.toml`. Feature names: `telemetry`, `ai-debugger`, `process-guard`, `camera-ai`. Default features include everything needed for production. |
| CF-2 | **Default features = production build (no manual flag selection required)** | Martin Fowler's toggle types: release toggles should be opt-in via compile flag, not require operators to remember which flags to pass. Default features = the build that goes to pods. Non-default = optional for testing. | LOW | `[features] default = ["telemetry", "process-guard"]`. AI debugger and camera AI are opt-in. |
| CF-3 | **Feature-gated modules must compile cleanly with and without their feature** | A feature gate that causes compile errors when disabled is useless. CI must verify the non-default build compiles. | LOW | CI build matrix: `cargo build --release` (default) + `cargo build --release --no-default-features` (minimal). |

### Table Stakes: Standing Rules Codification

| # | Feature | Why Expected | Complexity | Dependency |
|---|---------|--------------|------------|------------|
| SR-1 | **All 41+ CLAUDE.md standing rules classified by enforcement type** | Rules have different enforcement strategies: some are static analysis (no unwrap in Rust), some are runtime checks (health gate passes), some are pre-deploy scripts (cargo test green). Without classification, automation is ad-hoc and incomplete. | MEDIUM | Classification schema: STATIC (linter/compiler), TEST (cargo test / npm test), DEPLOY-GATE (script check before release), RUNTIME-MONITOR (post-deploy check), CONVENTION (documented but not automated). |
| SR-2 | **Pre-deploy gate script: runs before any binary leaves staging** | Replaces manual Ultimate Rule pre-flight. Script checks: cargo test green, no unwrap in diff, static CRT config present, LOGBOOK updated, bat files clean ASCII. Blocks deploy if any check fails. | MEDIUM | `deploy-staging/gate-check.sh`. Called before wave 1 starts. Exit code 0 = proceed, non-zero = abort. |
| SR-3 | **Post-deploy verification gate: runs after each wave** | After pods receive new binary, automated checks verify: build_id matches manifest, health endpoints pass, billing session roundtrip works, no error spike in logs. This is the automated replacement for manual E2E. | HIGH | Extends existing `check-health.sh` with version assertions. New `e2e-quick.sh` for post-wave verification (subset of full 231-test E2E). |
| SR-4 | **Pipeline blocks until ALL gates pass — no human bypass** | Explicit constraint from milestone. Pipeline state machine must reject manual "skip gate" commands. The only valid progressions are: gate passes naturally, or rollback is triggered. | MEDIUM | State machine has no `force_continue` transition from a failed health check state. The only exit from `health_check_failed` is `trigger_rollback`. |
| SR-5 | **Standing rules for the OTA system itself (new rules)** | The OTA system introduces new failure modes: stuck deploy states, manifest version drift, rollback loops. New standing rules must cover: always preserve prev binary, never deploy without manifest, rollback window must be defined, billing sessions drain before swap. | LOW | New standing rules section in CLAUDE.md: `### OTA Pipeline`. Document at build time. |

---

## Differentiators

Features that make this OTA/flag system notably better than generic solutions for this specific venue context.

| # | Feature | Value Proposition | Complexity | Notes |
|---|---------|-------------------|------------|-------|
| D-1 | **Billing-aware deploy drain** | Generic OTA systems ignore application state. Draining billing sessions before binary swap ensures no customer is mid-session during a pod restart. No competitor (Memfault, Mender) does this automatically — it requires application-level hooks. | HIGH | rc-agent exposes `PendingRestartState`. When `RCAGENT_SELF_RESTART` is queued, rc-agent finishes current billing session first (max drain timeout: 15 min), then swaps. Admin dashboard shows "draining" pod status. |
| D-2 | **Per-pod feature flag visibility in fleet health dashboard** | Standard feature flag UIs show aggregate state (flag X is on). Racing Point's admin dashboard should show per-pod flag state alongside per-pod health. Operator sees immediately if Pod 3 is running a different flag set than Pod 8. | MEDIUM | Admin dashboard fleet health table gains a "Flags" column showing flag divergence (grey = same as server, orange = drift detected). |
| D-3 | **Config drift detection (server vs pod comparison)** | After a config push, if a pod is offline and misses the push, its config drifts from the server's intended state. Drift detection runs every 5 min and surfaces diverged pods in the admin dashboard with the specific fields that differ. | MEDIUM | rc-agent sends `ConfigChecksum(hash_of_current_config)` on every health ping. Server compares against expected config hash for that pod. Mismatch = show drift warning in dashboard. |
| D-4 | **Atomic release rollback across binary + config + frontend** | Most OTA systems roll back the binary only. Config and frontend may still be on the new version. Rolling back the manifest reverts all three atomically — binary, config schema version, and frontend build_id — so all components stay synchronized. | HIGH | Rollback pushes previous `release-manifest.toml`. Pipeline reads it and redeploys each component from the previous version. |
| D-5 | **Anti-cheat safe mode flag** | The v15.0 anti-cheat milestone needs a runtime flag to disable risky subsystems when EAC/iRacing AC is running. This is a perfect feature flag use case: `disable_process_guard_eac`, `disable_hooks_eac`. Without the flag system, safe mode requires a binary with different compile flags — a much heavier change. | LOW | These are the first real-world consumers of the feature flag system. They prove the system works before adding more complex flags. Document as the canonical usage example. |

---

## Anti-Features

Features that seem useful but create significant problems in this specific context.

| # | Anti-Feature | Why Requested | Why Problematic | Correct Alternative |
|---|-------------|---------------|-----------------|---------------------|
| AF-1 | **A/B testing on pods (50% get feature, 50% don't)** | Classic feature flag use case. Sounds powerful. | Customer-facing A/B testing in a sim racing venue is meaningless — customers don't use the same pod twice, sample sizes are too small, and inconsistent pod behavior confuses staff ("why does Pod 3 work differently?"). | Use per-pod override for canary testing (Pod 8 only), never random-split customer traffic across pods. |
| AF-2 | **Feature flag SDK pulled from external service (LaunchDarkly, Unleash cloud)** | Industry-standard approach. Immediate access to a full-featured UI. | Venue has unreliable internet (on-site LAN). Flag delivery over external internet = flag evaluation fails when internet is down, which is exactly when reliable operation is most needed. Also adds external dependency to a safety-critical system. | Self-hosted registry in racecontrol server (already on LAN). |
| AF-3 | **Hot-reload of port bindings, WS server address, or DB path** | "If we're hot-reloading config, let's hot-reload everything." | These fields require OS-level resource acquisition (open sockets, file handles). Changing them at runtime without restart is fragile and can orphan connections. No production Axum/Tokio app hot-reloads these fields reliably. | Document explicitly which fields hot-reload and which require restart. Config schema marks each field as `hot_reload: true/false`. |
| AF-4 | **Binary self-update on pods (rc-agent downloads and applies its own update)** | Simplifies deploy orchestration — no central pipeline needed. | A binary that replaces itself is inherently fragile: if the download is corrupt, the binary that runs next is corrupt. No rollback capability because the old binary was replaced. RCAGENT_SELF_RESTART already does this dangerously enough — adding download responsibility makes it worse. | Keep orchestration on the server/James side. rc-agent only executes the swap after the binary has been downloaded and verified by the pipeline. |
| AF-5 | **"Feature flags" that actually control billing rates or session limits** | Billing rates and session limits are config values that change operationally. Using a boolean feature flag for them (flag ON = ₹900/hr, flag OFF = ₹700/hr) is a category error. | Config values have ranges, types, and validation rules. Boolean feature flags have none of these. A billing rate encoded as a flag state cannot be validated, audited, or reasoned about. | Config push system handles billing rates, session limits, game profiles. Feature flags handle only boolean enable/disable decisions. |
| AF-6 | **Standing rules codified as runtime assertions that can fail a customer session** | "If the standing rule is violated, block the action." | Standing rules are development-time and deploy-time constraints — not runtime customer-facing guards. Making them runtime checks inside rc-agent would add overhead to every session event and could cause a customer's session to fail because of a policy violation that has nothing to do with their experience. | Enforce standing rules at CI, pre-deploy gate, and post-deploy verification only. Never at runtime in the customer-facing path. |

---

## Feature Dependencies

```
[Cargo feature gates: CF-1, CF-2, CF-3]
    — builds the binary that OTA deploys —
    └──required for──> [OTA binary release: OTA-1]

[Feature flag registry: FF-1, FF-2]
    └──required for──> [Flag delivery WS: FF-3]
                           └──required for──> [In-memory flag cache: FF-4]
                                                  └──required for──> [Offline fallback: FF-5]

[FF-1 registry]
    └──required for──> [Admin flag UI: FF-6]

[FF-3, FF-4, FF-5]
    └──enables──> [Anti-cheat safe mode flag: D-5]

[Config push: CP-1]
    └──required for──> [Offline queue: CP-2]
                           └──required for──> [Config drift detection: D-3]

[OTA-1 release manifest]
    └──required for──> [OTA-3 health gate]
                           └──required for──> [OTA-2 canary]
                                                  └──required for──> [OTA-6 staged waves]
                                                                         └──requires──> [OTA-4 auto-rollback]

[OTA-4 auto-rollback]
    └──requires──> [OTA-5 billing drain]
    └──requires──> [OTA-7 prev binary preserved]

[SR-2 pre-deploy gate]
    └──blocks──> [OTA wave 1]

[SR-3 post-deploy gate]
    └──blocks──> [each subsequent OTA wave]
    └──blocks──> [SR-4 pipeline no-bypass rule]

[OTA-8 state machine]
    └──orchestrates──> [SR-2, OTA-2, OTA-3, OTA-6, OTA-4]
```

### Dependency Notes

- **Cargo feature gates must come first:** The OTA pipeline deploys binaries. Those binaries must already have the correct `[features]` sections defined before the pipeline is built. CF-1 through CF-3 are Phase 1 work.
- **Feature flag registry before flag delivery:** The WS message type for flag sync (FF-3) must serialize from the registry (FF-1). Build the registry first, then wire the transport.
- **Config push before drift detection:** Drift detection (D-3) compares current pod config against the server's expected config. The server can only know the "expected" config once it has a config push system (CP-1) that records what was sent.
- **OTA health gate depends on manifest:** The health gate (OTA-3) must know what version to assert against. That information lives in the manifest (OTA-1). The manifest must be generated before any deploy wave runs.
- **Billing drain must be solved before any pod-touching OTA deploy:** OTA-5 is a soft blocker on all pod deploys. Even if auto-rollback is not yet built, the drain signal must exist before deploying to pods that may have active sessions.
- **Pre-deploy gate (SR-2) gates everything:** No OTA wave can start without the pre-deploy gate passing. SR-2 is the entry condition to the entire pipeline.

---

## MVP Definition for v22.0

### Phase 1: Foundation (Build First)

Minimum needed before any OTA or flag delivery can work.

- [ ] **Cargo feature gates in rc-agent** (CF-1, CF-2, CF-3) — binary produced by pipeline must have correct feature structure
- [ ] **Feature flag registry in racecontrol DB** (FF-1) — named flags, fleet default, per-pod override
- [ ] **release-manifest.toml format defined** (OTA-1) — locks binary + config + frontend together by version
- [ ] **Pre-deploy gate script** (SR-2) — automates Ultimate Rule pre-flight

### Phase 2: Flag Delivery

- [ ] **Flag sync over existing WebSocket** (FF-3) — new WS message type in rc-common
- [ ] **In-memory flag cache in rc-agent** (FF-4) — Arc<RwLock<HashMap>> in AppState
- [ ] **Offline flag fallback** (FF-5) — persist to flags-cache.json on pod
- [ ] **Admin UI for flag management** (FF-6, FF-7) — new section in racingpoint-admin

### Phase 3: Config Push

- [ ] **Config push over WebSocket** (CP-1) — new ConfigUpdate WS message type
- [ ] **Offline queue for config updates** (CP-2) — server-side pending queue per pod
- [ ] **Hot-reload in rc-agent** (CP-3) — arc/rwlock config swap, documented reload/no-reload split
- [ ] **Config push audit log** (CP-5) — append-only DB table

### Phase 4: OTA Pipeline

- [ ] **OTA state machine** (OTA-8) — orchestrates the whole pipeline
- [ ] **Canary wave to Pod 8** (OTA-2) — formalize existing practice
- [ ] **Health gate check** (OTA-3) — version + build_id + health assertions per wave
- [ ] **Staged wave rollout** (OTA-6) — pod 8 → 4 pods → remaining
- [ ] **Prev binary preservation** (OTA-7) — rc-agent-prev.exe always kept
- [ ] **Auto-rollback on gate failure** (OTA-4) — swap to prev + RCAGENT_SELF_RESTART
- [ ] **Billing drain before swap** (OTA-5) — PendingRestart state in rc-agent
- [ ] **Post-deploy verification gate** (SR-3) — quick E2E after each wave

### Add After Validation (v22.x)

- [ ] **Config drift detection** (D-3) — configChecksum on health ping, after config push is stable
- [ ] **Per-pod flag visibility in fleet dashboard** (D-2) — after flag system is in use for one milestone
- [ ] **Atomic rollback: binary + config + frontend** (D-4) — after individual rollbacks are working
- [ ] **Billing-aware drain UX in admin** (D-1) — after OTA is stable in production

### Defer to v23+

- [ ] **Anti-cheat safe mode flags** — depends on v15.0 AntiCheat milestone being planned
- [ ] **Full 231-test E2E as post-wave gate** (SR-3 extended) — current quick check sufficient for v22.0; full suite adds 10+ min per wave

---

## Feature Prioritization Matrix

| Feature | Operational Value | Implementation Cost | Priority |
|---------|------------------|---------------------|----------|
| Cargo feature gates (CF-1–3) | HIGH — enables OTA + clean module boundaries | LOW — Cargo.toml change + cfg() attributes | P1 |
| Flag registry + per-pod override (FF-1, FF-2) | HIGH — unblocks all flag features | LOW — DB table + Rust struct | P1 |
| Flag WS delivery (FF-3, FF-4, FF-5) | HIGH — flags useless without delivery | MEDIUM — new WS message type | P1 |
| Pre-deploy gate script (SR-2) | HIGH — replaces manual Ultimate Rule | MEDIUM — script aggregating existing checks | P1 |
| Release manifest format (OTA-1) | HIGH — required for health gate | LOW — TOML format definition | P1 |
| Config push (CP-1, CP-2, CP-3) | HIGH — eliminates manual TOML editing | MEDIUM — WS message + Arc<RwLock> | P1 |
| OTA state machine (OTA-8) | HIGH — pipeline reliability depends on it | HIGH — state machine design + persistence | P1 |
| Canary + health gate + staged waves (OTA-2, OTA-3, OTA-6) | HIGH — fleet safety | MEDIUM — script + existing health endpoints | P1 |
| Auto-rollback + billing drain (OTA-4, OTA-5) | HIGH — safety net for bad deploys | HIGH — prev binary + drain state in rc-agent | P1 |
| Admin flag UI (FF-6, FF-7) | MEDIUM — ops visibility | MEDIUM — Next.js UI + REST endpoints | P2 |
| Post-deploy verification gate (SR-3) | HIGH — automated E2E subset | MEDIUM — extends check-health.sh | P2 |
| Config audit log (CP-5) | MEDIUM — ops accountability | LOW — append-only DB table | P2 |
| Config drift detection (D-3) | MEDIUM — surfaces offline divergence | MEDIUM — checksum on health ping | P2 |
| Kill switch convention (FF-8) | LOW — covered by normal flag disable | LOW — naming convention only | P3 |
| Per-pod flag visibility in dashboard (D-2) | LOW — nice-to-have visibility | MEDIUM — dashboard column | P3 |
| Atomic multi-component rollback (D-4) | MEDIUM — full consistency | HIGH — coordinates three rollbacks | P3 |

**Priority key:**
- P1: Required for v22.0 to deliver its stated goal
- P2: Should have — adds reliability or visibility, add in later v22.0 phases
- P3: Nice to have — defer to v22.x or v23+

---

## Standing Rules Enforcement Classification

All 41+ standing rules must be assigned to an enforcement category. The categories and example rule mappings:

| Category | Description | When enforced | Tooling |
|----------|-------------|---------------|---------|
| **STATIC** | Compiler or linter catches violation at build time | Every cargo build | `cargo clippy -- -D warnings`, `tsc --noEmit`, `grep -r '\.unwrap()' src/` |
| **TEST** | Automated test suite catches violation | Every cargo test / npm test | Existing test suite (cargo test -p rc-common etc.) |
| **DEPLOY-GATE** | Pre-deploy script blocks pipeline on violation | Before wave 1 starts | `deploy-staging/gate-check.sh` (SR-2) |
| **POST-WAVE** | Post-deploy verification script catches regression | After each deploy wave | `check-health.sh` + `e2e-quick.sh` (SR-3) |
| **RUNTIME-MONITOR** | Ongoing check after deploy, alerts on violation | Continuous post-ship | Admin dashboard anomaly indicators |
| **CONVENTION** | Human convention — documented but not automatable | Developer awareness | CLAUDE.md documentation |

### Example Rule Classification

| Rule | Category | Automated check |
|------|----------|-----------------|
| No `.unwrap()` in production Rust | STATIC | `cargo clippy -- -D clippy::unwrap_used` |
| No `any` in TypeScript | STATIC | `tsc --strict --noEmit` |
| Static CRT `.cargo/config.toml` | DEPLOY-GATE | `grep crt-static .cargo/config.toml` |
| Cargo test green before deploy | DEPLOY-GATE | `cargo test -p rc-common -p rc-agent -p racecontrol` |
| `.bat` files clean ASCII + CRLF | DEPLOY-GATE | `file deploy-staging/*.bat | grep -v CRLF && exit 1` |
| LOGBOOK updated after commit | DEPLOY-GATE | `git diff HEAD~1 LOGBOOK.md | grep -c '^+' must be > 0` |
| build_id matches expected after deploy | POST-WAVE | `curl /health | jq .build_id == manifest.git_sha` |
| Pod 8 canary before other pods | DEPLOY-GATE (pipeline enforced) | State machine enforces wave order — no bypass |
| Rollback plan exists before deploy | DEPLOY-GATE | Manifest includes `prev_manifest_path` field — pipeline rejects if absent |
| Cascade updates (cross-process) | CONVENTION | CLAUDE.md — not automatable |
| RCAGENT_SELF_RESTART not combined with taskkill | CONVENTION | CLAUDE.md — not automatable |

---

## Sources

- [Feature Toggles (aka Feature Flags) — Martin Fowler](https://martinfowler.com/articles/feature-toggles.html) — canonical reference, toggle type taxonomy (release, ops, experiment, permission), HIGH confidence
- [OTA Update Checklist for Embedded Devices — Memfault](https://memfault.com/blog/ota-update-checklist-for-embedded-devices/) — canary, health gates, rollback patterns, HIGH confidence
- [11 Principles for Feature Flag Systems — Unleash](https://docs.getunleash.io/topics/feature-flags/feature-flag-best-practices) — per-device targeting, offline fallback, HIGH confidence
- [OpenFeature Specification](https://openfeature.dev/) — standard flag evaluation API, MEDIUM confidence
- [Cargo Features Reference](https://doc.rust-lang.org/cargo/reference/features.html) — compile-time feature gates, HIGH confidence
- [Policy as Code in CI/CD Pipelines — Platform Engineering](https://platformengineering.org/blog/policy-as-code) — automated enforcement gates pattern, MEDIUM confidence
- [WebSocket Reconnection: State Sync and Recovery — WebSocket.org](https://websocket.org/guides/reconnection/) — sequence number replay for offline queuing, MEDIUM confidence
- [Tokio shared state patterns](https://tokio.rs/tokio/tutorial/shared-state) — Arc<RwLock> for hot-reload config, HIGH confidence (official docs)
- Existing racecontrol codebase: `RCAGENT_SELF_RESTART`, `billing_guard`, `pod_healer`, `check-health.sh` — constraints derived from production code, HIGH confidence

---

*Feature research for: v22.0 Feature Management & OTA Pipeline*
*Researched: 2026-03-23*
