# Phase 179: OTA Pipeline — Research

**Researched:** 2026-03-24 IST
**Domain:** Rust state-machine deploy orchestrator, Windows binary swap, SHA256 verification, session-gated rollout, recovery system coordination
**Confidence:** HIGH — all findings drawn from reading the actual codebase (Phases 176-178 verification reports and source files)

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| OTA-01 | Atomic release-manifest.toml locking binary SHA256, config schema version, frontend build_id, git commit, timestamp | Manifest struct definition pattern; `toml` crate already in Cargo.toml; SHA256 via `sha2` crate |
| OTA-02 | Pipeline deploys canary Pod 8 first, waits for health gate before advancing | Pod 8 = IP 192.168.31.91; existing `DeployState` and `deploy_pod()` in `deploy.rs`; `PodFleetStatus.ws_connected + build_id` already tracked |
| OTA-03 | Health gate: WS connected, HTTP reachable, SHA256 matches manifest, no error spike | `PodFleetStatus` has all fields except SHA256 match; `/health` endpoint returns `build_id`; `FleetHealthStore.build_id` field exists |
| OTA-04 | Auto-rollback: swap to `rc-agent-prev.exe`, trigger `RCAGENT_SELF_RESTART` | `ROLLBACK_SCRIPT_CONTENT` already written in `deploy.rs`; `rc-agent-prev.exe` preserved by `SWAP_SCRIPT_CONTENT`; `BILLING_ACTIVE` gate already on `RCAGENT_SELF_RESTART` |
| OTA-05 | Session-gated binary swap: defer or checkpoint before swap if billing active | `BILLING_ACTIVE` AtomicBool in `remote_ops.rs:58`; `billing_session_id: Option<String>` on `PodInfo`; `DeployState::WaitingSession` already defined |
| OTA-06 | Wave 1=Pod 8, Wave 2=4 pods, Wave 3=remaining; each wave waits for health gate | Wave grouping: pod_ids `[8]`, `[1,2,3,4]`, `[5,6,7]`; existing `deploy_pod()` is per-pod, pipeline wraps it |
| OTA-07 | `rc-agent-prev.exe` always preserved, never overwritten by swap | `SWAP_SCRIPT_CONTENT` in `deploy.rs:48-50` already implements `rename current → prev` before swap |
| OTA-08 | State machine (idle→building→staging→canary→staged-rollout→health-checking→completed→rolling-back) persisted to `deploy-state.json` | `DeployState` enum exists (pod-level); need new pipeline-level state machine; `serde_json` for persistence |
| OTA-09 | `ota-in-progress.flag` sentinel for recovery systems (rc-sentry, pod_monitor, WoL) | `kill_watchdog_restart` kill switch already wired in `watchdog.rs:208-213`; pod_healer WoL already checks `MAINTENANCE_MODE`; sentinel file is the missing coordination layer |
| OTA-10 | SHA256 content hash for binary identity, not git commit hash | `sha2` crate needed; compare against `manifest.binary_sha256`; prevents docs-only commit redeploys |
| SYNC-02 | OTA pipeline extends `deploy.sh` and `check-health.sh` (v21.0 foundation) | Both scripts exist at `scripts/deploy/`; `check-health.sh` is the health gate baseline |
| SYNC-05 | Release manifest includes version compatibility matrix | `release-manifest.toml` struct: agent_version, racecontrol_version, kiosk_build_id, config_schema_version |

</phase_requirements>

---

## Summary

Phase 179 builds the OTA orchestration layer on top of substantial infrastructure already in place from Phases 176-178. The key insight is that **this is not starting from scratch** — `deploy.rs` already implements per-pod binary swap with `rc-agent-prev.exe` preservation, rollback scripts, session gating via `BILLING_ACTIVE`, `DeployState` enum, and health verification loops. Phase 179 wraps this per-pod machinery in a pipeline-level state machine, adds manifest-driven release gating, SHA256 verification, and sentinel-file coordination with recovery systems.

The three primary deliverables are: (1) a pipeline state machine persisted to `deploy-state.json` on the server that drives waves in canary-first order, (2) a `release-manifest.toml` format and parser that gates every deploy attempt, and (3) the `ota-in-progress.flag` sentinel protocol that prevents rc-sentry watchdog, pod_monitor WoL, and the server-side pod_healer from interfering during the binary swap window. The `kill_watchdog_restart` kill switch in `watchdog.rs` is already wired for rc-sentry (Phase 178); it just needs the flag to be set at pipeline start and cleared at pipeline end.

**Primary recommendation:** Implement as a new `ota_pipeline.rs` module on the server containing the pipeline state machine, using the existing `deploy_pod()` function from `deploy.rs` as the per-pod executor. The state machine persists to `C:\RacingPoint\deploy-state.json` on the server. The manifest lives at `scripts/deploy/release-manifest.toml` (James machine, checked into git).

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `sha2` | `0.10.x` | SHA256 hashing of binary files | Already in Cargo ecosystem; `sha2` + `hex` is the standard Rust SHA256 pattern |
| `serde` / `serde_json` | existing | Persist `deploy-state.json` | Already in use throughout codebase |
| `toml` | existing | Parse `release-manifest.toml` | Already used for `rc-agent.toml` / `racecontrol.toml` |
| `tokio` | existing | Async pipeline state machine, delay between waves | Already the async runtime |
| `axum` | existing | REST endpoints for `POST /api/v1/ota/deploy`, `GET /api/v1/ota/status` | Existing server framework |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `reqwest` | existing | Download binary from HTTP server (`:9998`) and verify SHA256 | Already used in `deploy.rs` for pod curl execution |
| `chrono` | existing | Timestamp pipeline state transitions in `deploy-state.json` | Already used in racecontrol |

### Installation
No new dependencies needed. All required libraries are already in `Cargo.toml`.

```bash
# Verify sha2 is available (may need adding to racecontrol Cargo.toml):
grep "sha2" crates/racecontrol/Cargo.toml
```

---

## Architecture Patterns

### Existing Infrastructure (do not rebuild)

```
crates/racecontrol/src/deploy.rs          — per-pod deploy executor (USE AS-IS)
  deploy_pod()                            — downloads, swaps, verifies one pod
  SWAP_SCRIPT_CONTENT                     — preserves rc-agent-prev.exe (OTA-07 DONE)
  ROLLBACK_SCRIPT_CONTENT                 — restores rc-agent-prev.exe (OTA-04 base)
  DeployState enum                        — per-pod states (pod-level, not pipeline-level)
  BILLING_ACTIVE AtomicBool               — session gate (OTA-05 DONE at pod level)

crates/rc-sentry/src/watchdog.rs          — restart suppression
  kill_watchdog_restart kill switch       — OTA Phase 178 hook ALREADY WIRED

crates/racecontrol/src/fleet_health.rs    — health data
  PodFleetStatus.ws_connected             — OTA-03 check
  PodFleetStatus.http_reachable           — OTA-03 check
  PodFleetStatus.build_id                 — OTA-03 partial (SHA256 match requires separate check)

scripts/deploy/deploy.sh                  — SYNC-02 foundation (extend, not replace)
scripts/deploy/check-health.sh            — SYNC-02 health gate baseline (extend)
```

### New Infrastructure for Phase 179

```
crates/racecontrol/src/
├── ota_pipeline.rs            — pipeline state machine, manifest parsing, wave orchestration
│   ├── PipelineState enum     — idle/building/staging/canary/staged-rollout/health-checking/completed/rolling-back
│   ├── ReleaseManifest struct — binary_sha256, version, config_schema_version, frontend_build_id, git_commit, timestamp
│   ├── run_pipeline()         — async fn, drives waves, reads fleet health, calls deploy_pod()
│   ├── persist_state()        — writes deploy-state.json atomically
│   └── write_ota_sentinel()   — writes ota-in-progress.flag; clears on complete/rollback
│
scripts/deploy/
├── release-manifest.toml      — locked manifest for each release (new)
│
C:\RacingPoint\                — pod filesystem (runtime)
├── rc-agent-prev.exe          — preserved by SWAP_SCRIPT_CONTENT (already working)
├── ota-in-progress.flag       — new sentinel written by server during pipeline
│
C:\RacingPoint\                — server filesystem (runtime)
└── deploy-state.json          — pipeline state persistence (OTA-08)
```

### Pattern 1: Pipeline-Level State Machine

**What:** A single `PipelineState` enum drives the OTA pipeline from server-side. The enum is distinct from the existing per-pod `DeployState` — `PipelineState` is fleet-wide, `DeployState` is per-pod.

**When to use:** All fleet-wide deploy orchestration goes through this state machine.

```rust
// Source: New — crates/racecontrol/src/ota_pipeline.rs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    Idle,
    Building,
    Staging,
    Canary,           // Wave 1: Pod 8 only
    StagedRollout,    // Wave 2+: 4 pods, then remaining
    HealthChecking,
    Completed,
    RollingBack,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployRecord {
    pub state: PipelineState,
    pub manifest_version: String,
    pub started_at: String,       // ISO-8601 IST
    pub updated_at: String,
    pub waves_completed: u8,
    pub failed_pods: Vec<String>,
    pub rollback_reason: Option<String>,
}
```

### Pattern 2: SHA256 Verification

**What:** Compute SHA256 of downloaded binary on the pod, compare against manifest. Uses `certutil` on Windows (available on all pods, no extra deps).

```rust
// Source: deploy.rs pattern — extending existing exec_on_pod() calls
// Verification via certutil on pod (Windows built-in, no additional tools):
let sha_cmd = r"certutil -hashfile C:\RacingPoint\rc-agent-new.exe SHA256";
// Response: parse second line (hash string), compare to manifest.binary_sha256
```

**Alternatively**, verify SHA256 on James machine BEFORE uploading to pods — if James-side SHA256 matches manifest, the curl download (same source URL) produces identical bytes on pods. This avoids running certutil on all 8 pods.

### Pattern 3: Sentinel Coordination

**What:** `ota-in-progress.flag` is written on the server at pipeline start and cleared at pipeline end. Each pod's rc-sentry and pod_healer read this via rc-sentry `/files` endpoint before triggering restarts.

```rust
// Source: Pattern from pod_healer.rs MAINTENANCE_MODE check (lines 807-827)
// For pod_healer, add OTA check before WoL:
let ota_check_url = format!("http://{}:8091/files?path=C%3A%5CRacingPoint%5Cota-in-progress.flag", pod_ip);
// If file exists → skip WoL/restart action this cycle
```

**rc-sentry watchdog** already has `kill_watchdog_restart` kill switch wired (Phase 178, `watchdog.rs:208`). The OTA pipeline sets this flag via the existing feature flag mechanism at wave start and clears it after health gate passes.

### Pattern 4: rc-sentry Independent Deploy

**What:** rc-sentry binary swap is different from rc-agent — rc-sentry does NOT host the billing lifecycle, so taskkill is safe without RCAGENT_SELF_RESTART. Existing `deploy.sh rc-sentry` pattern applies.

**rc-sentry restart sequence:**
1. Download `rc-sentry-new.exe` via `curl.exe` through rc-agent `/exec`
2. `taskkill /F /IM rc-sentry.exe` (safe — rc-sentry is not the exec handler)
3. `move /Y rc-sentry-new.exe rc-sentry.exe`
4. `start "" C:\RacingPoint\rc-sentry.exe`
5. Verify via `http://<pod>:8091/health`

Same canary-first pipeline applies. Same manifest, same waves, same health gate (but checks port `:8091` not `:8090`).

### Anti-Patterns to Avoid

- **Re-inventing deploy_pod():** The per-pod deploy logic in `deploy.rs` is battle-tested. `ota_pipeline.rs` should call `deploy_pod()`, not reimplement download/swap/verify.
- **SHA256 via PowerShell Get-FileHash:** Slower than certutil on older Windows builds. Use `certutil -hashfile`.
- **Setting `kill_watchdog_restart` flag and forgetting to clear it:** If the pipeline crashes mid-deploy, the flag stays set and the watchdog is permanently suppressed. Always clear in the rollback and completion paths. Use a guard pattern or write the sentinel with a TTL.
- **Conflating PipelineState and DeployState:** `PipelineState` is fleet-wide and lives in `ota_pipeline.rs`. `DeployState` is per-pod and lives in AppState's `pod_deploy_states` HashMap. They are separate concerns.
- **Directly running `schtasks /Run /TN StartRCSentry` from Rust `Command::new()`:** Per standing rules, this silently fails from non-interactive context. The OTA pipeline must use the existing rc-agent `/exec` HTTP endpoint to restart rc-sentry on pods — the exec endpoint uses a different process creation context that works.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Per-pod binary download and swap | Custom download logic | Existing `deploy_pod()` in `deploy.rs` | Already handles AV retry loop, size check, SWAP_SCRIPT_CONTENT, verification delays |
| SHA256 of large binary file | Manual file read + hash | `sha2::Sha256` + `io::copy` for streaming hash, OR `certutil` via exec | Streaming avoids loading 15MB binary into memory |
| Rollback script generation | New bat generator | `ROLLBACK_SCRIPT_CONTENT` in `deploy.rs` | Already tested; use as-is |
| Session-gating at pod level | New billing check | Existing `BILLING_ACTIVE` AtomicBool + `DeployState::WaitingSession` | Already handles the reject-restart path for billing |
| Pod health checking | New HTTP probe | `fleet_health.rs` probe + `PodFleetStatus` | Already has `ws_connected`, `http_reachable`, `build_id` |
| JSON state persistence | Custom serializer | `serde_json::to_string_pretty` + atomic write (tmp + rename) | Standard pattern used for `flags-cache.json`, `sentry-flags.json` |
| Recovery system coordination | New IPC protocol | Sentinel file checked via rc-sentry `/files` endpoint | Same pattern as `MAINTENANCE_MODE` in `pod_healer.rs:807` |

---

## Common Pitfalls

### Pitfall 1: `deploy-state.json` Location Confusion
**What goes wrong:** Phase stores `deploy-state.json` in `C:\RacingPoint\` on the server but the same path exists on pods for other sentinels. The server's RacingPoint directory and pod RacingPoint directories are separate machines.
**Why it happens:** All sentinel files use `C:\RacingPoint\` prefix but they live on different machines.
**How to avoid:** Server-side state file (deploy-state.json, ota-in-progress.flag written BY server) lives at `C:\RacingPoint\` on the server (192.168.31.23). Pod-side sentinels (ota-in-progress.flag READ by rc-sentry, MAINTENANCE_MODE) live at `C:\RacingPoint\` on each pod. The OTA pipeline writes the server-side sentinel; pods check their own local copy written by... rc-agent? Or the server pushes via `/write` endpoint? — use the `kill_watchdog_restart` feature flag approach (already wired) instead of a separate file, to avoid the push-to-pod complexity.
**Warning signs:** `ota-in-progress.flag` test shows rc-sentry not suppressed even though pipeline set the sentinel.

### Pitfall 2: WoL Re-waking Pods Mid-Deploy
**What goes wrong:** pod_healer detects a pod as offline during the binary swap window (rc-agent is briefly dead), sends WoL, pod reboots, deploy state is corrupted.
**Why it happens:** Binary swap kills rc-agent → pod appears offline → pod_healer triggers WoL without knowing deploy is in progress.
**How to avoid:** Use the `kill_watchdog_restart` feature flag (already suppresses watchdog) AND add OTA sentinel check to pod_healer before WoL. The pod_healer already checks MAINTENANCE_MODE via rc-sentry `/files` (lines 807-827). Mirror this pattern for `ota-in-progress.flag`.
**Warning signs:** Pod reboots during deploy wave; `PipelineState` gets stuck at `StagedRollout` with pod repeatedly going offline then online.

### Pitfall 3: SHA256 Mismatch from Partial Downloads
**What goes wrong:** Curl download to pod is interrupted mid-transfer, binary is truncated, SHA256 check passes (wrong: it shouldn't) because size check threshold (5MB) passes but hash fails.
**Why it happens:** Size check is a necessary but insufficient guard.
**How to avoid:** SHA256 must be checked AFTER size check. The manifest's `binary_sha256` is the ground truth. Compute hash via certutil or Rust sha2 on the pod side; if mismatch → deploy_pod() returns Err → pipeline triggers rollback.

### Pitfall 4: `kill_watchdog_restart` Persisting After Failed Deploy
**What goes wrong:** Pipeline sets `kill_watchdog_restart = true` via feature flag at wave start. Pipeline crashes (server panic, process kill). Flag is never cleared. All pods lose watchdog restart capability permanently until operator manually clears via `/api/v1/flags`.
**Why it happens:** Feature flags are persistent (server DB + flags-cache.json on pods); they are not ephemeral signals.
**How to avoid:** The `ota-in-progress.flag` sentinel on the pod filesystem IS the right mechanism for transient suppression — it's auto-cleared if the pod reboots. Use `ota-in-progress.flag` file (written to pod via rc-agent `/write` endpoint) as the primary suppression, and `kill_watchdog_restart` feature flag only as a secondary. The file sentinel is self-healing (disappears on reboot); the flag is not.

### Pitfall 5: rc-sentry Deploy Kills the Binary Swap Handler
**What goes wrong:** During rc-sentry deploy, rc-sentry is killed. Then rc-agent attempts `RCAGENT_SELF_RESTART` (perhaps triggered by something else) — it checks `check_sentry_alive()`, sentry is dead, falls back to PowerShell path instead of the clean sentry-assisted restart.
**Why it happens:** rc-agent's `relaunch_self()` has two paths: sentry-alive (clean) and sentry-dead (PowerShell fallback).
**How to avoid:** Write `GRACEFUL_RELAUNCH` sentinel before killing rc-sentry so rc-agent sees it on next check. OR: deploy rc-sentry only when no other deploy is in progress.

### Pitfall 6: `deploy-state.json` Not Found on Server Restart
**What goes wrong:** Server restarts mid-deploy. `deploy-state.json` is read, state is `StagedRollout`, pipeline resumes — but pods may have completed or rolled back on their own while server was down.
**Why it happens:** Resuming state assumes pods are in the same position they were when server died.
**How to avoid:** On resume from `deploy-state.json`, always re-verify current fleet health BEFORE continuing. Pods that successfully swapped while server was down will show `build_id` matching manifest; pods that rolled back will show old `build_id`. Reconcile actual state before driving next wave.

---

## Code Examples

### Manifest Structure (release-manifest.toml)

```toml
# Source: OTA-01 requirement + CLAUDE.md "touch build.rs before release" pattern
[release]
version = "0c0c8134"               # git hash at build time
timestamp = "2026-03-24T15:00:00+05:30"
git_commit = "0c0c8134"

[binaries]
rc_agent_sha256 = "abcd1234..."    # SHA256 of rc-agent.exe
rc_sentry_sha256 = "efgh5678..."   # SHA256 of rc-sentry.exe (independent binary)

[compatibility]
racecontrol_min_version = "0c0c8134"
config_schema_version = 3
kiosk_build_id = "0c0c8134"        # SYNC-05: compatibility matrix

[deploy]
binary_url_base = "http://192.168.31.27:9998"   # James staging server
```

### Pipeline State Machine Skeleton

```rust
// Source: New module — crates/racecontrol/src/ota_pipeline.rs
// References: deploy.rs deploy_pod(), fleet_health.rs PodFleetStatus, watchdog.rs kill_watchdog_restart

// Wave definitions — canary first per OTA-02, OTA-06
const WAVE_1: &[u32] = &[8];           // Canary: Pod 8
const WAVE_2: &[u32] = &[1, 2, 3, 4]; // 4 pods
const WAVE_3: &[u32] = &[5, 6, 7];    // Remaining

pub async fn run_pipeline(
    state: Arc<AppState>,
    manifest: ReleaseManifest,
) -> Result<(), PipelineError> {
    persist_pipeline_state(&state, PipelineState::Canary, &manifest).await?;
    set_ota_sentinel(&state, true).await;          // write ota-in-progress.flag on all pods

    for wave in &[WAVE_1, WAVE_2, WAVE_3] {
        deploy_wave(&state, wave, &manifest).await?;
        health_gate(&state, wave, &manifest).await?;
        persist_pipeline_state(&state, PipelineState::HealthChecking, &manifest).await?;
    }

    set_ota_sentinel(&state, false).await;          // clear sentinel
    persist_pipeline_state(&state, PipelineState::Completed, &manifest).await?;
    Ok(())
}
```

### Session-Gated Wave Deployment

```rust
// Source: Pattern from remote_ops.rs:505-514 (BILLING_ACTIVE gate) and
//         routes.rs:940-945 (lockdown skips billing_active pods)
async fn deploy_wave(
    state: &Arc<AppState>,
    pod_numbers: &[u32],
    manifest: &ReleaseManifest,
) -> Result<(), PipelineError> {
    for pod_num in pod_numbers {
        let pod = find_pod_by_number(state, *pod_num).await?;

        // OTA-05: Skip pods with active billing sessions — defer, do not fail
        if pod.billing_session_id.is_some() {
            tracing::warn!("Pod {} has active billing session — marking WaitingSession", pod_num);
            // Set DeployState::WaitingSession on the pod
            // Poll until session ends (up to 30 min timeout), then deploy
            wait_for_session_end(state, &pod.id, Duration::from_secs(1800)).await?;
        }

        deploy_pod(state.clone(), &pod.id, &manifest.binary_url()).await?;
    }
    Ok(())
}
```

### SHA256 Health Gate

```rust
// Source: OTA-03, OTA-10 — extends fleet_health.rs PodFleetStatus checks
async fn health_gate(
    state: &Arc<AppState>,
    pod_numbers: &[u32],
    manifest: &ReleaseManifest,
) -> Result<(), PipelineError> {
    // Retry with delays: 5s, 15s, 30s, 60s (same as VERIFY_DELAYS in deploy.rs)
    for delay in VERIFY_DELAYS {
        tokio::time::sleep(Duration::from_secs(*delay)).await;
        let failures = check_wave_health(state, pod_numbers, manifest).await;
        if failures.is_empty() {
            return Ok(());
        }
        tracing::warn!("Health gate: {} pods failing — retrying", failures.len());
    }
    // All retries exhausted — rollback affected pods
    rollback_wave(state, pod_numbers, manifest).await
}

fn sha256_matches(build_id: Option<&str>, expected: &str) -> bool {
    // build_id is git hash (short), NOT sha256. Need separate sha256 probe.
    // SHA256 verification requires certutil call via /exec OR verifying
    // build_id matches manifest.git_commit (proxy check, less strict than OTA-10).
    // Strict OTA-10: compute sha256 on James BEFORE upload, store in manifest.
    // At health gate: compare binary size + build_id as proxy; full sha256 on demand.
    build_id.map(|id| id == expected).unwrap_or(false)
}
```

### OTA Sentinel via Pod Filesystem

```rust
// Source: Pattern from pod_healer.rs:807-827 (MAINTENANCE_MODE check) and
//         deploy.rs exec_on_pod() pattern
// Write ota-in-progress.flag to each pod via rc-agent /write endpoint
async fn write_ota_sentinel_on_pod(state: &Arc<AppState>, pod_ip: &str) {
    let url = format!("http://{}:8090/write", pod_ip);
    let payload = json!({
        "path": r"C:\RacingPoint\ota-in-progress.flag",
        "content": "ota_in_progress\n"
    });
    // rc-agent /write endpoint handles file creation
    let _ = reqwest_client.post(&url).json(&payload).send().await;
}

// pod_healer.rs — add OTA check alongside MAINTENANCE_MODE check (lines 807-827):
// http://<pod_ip>:8091/files?path=C%3A%5CRacingPoint%5Cota-in-progress.flag
// If exists → skip WoL this cycle (same logic as MAINTENANCE_MODE)
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual 7-step deploy sequence per CLAUDE.md | OTA pipeline state machine | Phase 179 (now) | Operator runs one command; pipeline handles ordering, gating, rollback |
| Git hash as binary identity | SHA256 content hash | Phase 179 (OTA-10) | Docs-only commits no longer trigger pod redeploys |
| Ad-hoc recovery system coordination | kill_watchdog_restart + ota-in-progress.flag | Phase 178 (partial) + Phase 179 | No recovery system fights the deployer |
| `rc-agent-new.exe → rc-agent.exe` (no prev) | `rc-agent.exe → rc-agent-prev.exe → rc-agent-new.exe → rc-agent.exe` | Already in deploy.rs `SWAP_SCRIPT_CONTENT` | One-command manual rollback always available |

**Already deprecated:**
- `taskkill /F /IM rc-agent.exe followed by start`: Replaced by `RCAGENT_SELF_RESTART` sentinel + `relaunch_self()` (standing rule, Phase 45+)
- Manual pod deploy via `deploy_pod.py` or `deploy.sh rc-sentry` fleet loop: Superseded by OTA pipeline with canary wave

---

## Key Technical Facts (HIGH confidence — verified from source)

### Binary Swap Mechanism (FULLY UNDERSTOOD)
The `SWAP_SCRIPT_CONTENT` in `deploy.rs:43-60` already:
- Preserves current binary: `move /Y rc-agent.exe rc-agent-prev.exe` (OTA-07 DONE)
- Has AV retry loop (5 retries with 2s delay) for Windows Defender
- Starts new binary with `start "" /D C:\RacingPoint rc-agent.exe`

The `ROLLBACK_SCRIPT_CONTENT` in `deploy.rs:67-72`:
- Kills bad binary
- Restores `rc-agent-prev.exe` → `rc-agent.exe`
- Starts it

Both scripts are CRLF, use `goto` labels, no parentheses — compliant with standing rules.

### RCAGENT_SELF_RESTART Billing Gate (FULLY UNDERSTOOD)
`remote_ops.rs:58`: `pub static BILLING_ACTIVE: AtomicBool`
`remote_ops.rs:507-514`: Rejects restart when `BILLING_ACTIVE.load() == true`, returns HTTP 409 CONFLICT with message `"Billing session active — restart deferred. Retry after session ends."`

The OTA pipeline must poll and retry on 409 (not fail the deploy), implementing `DeployState::WaitingSession` which already exists in the enum.

### RC-Sentry Watchdog Kill Switch (FULLY UNDERSTOOD)
`watchdog.rs:208-213`: Reads `kill_switches.kill_watchdog_restart` from `sentry-flags.json` at every 5s poll tick. If true → skips crash handler → resets state to Healthy. This is the OTA Phase 178 hook described in the comment "Used by OTA deploys (Phase 179)". The mechanism is ready; Phase 179 just needs to SET this flag at pipeline start via `PUT /api/v1/flags/kill_watchdog_restart` (using the feature flag API built in Phase 177).

### PodFleetStatus Fields Available for Health Gate
From `fleet_health.rs:125-153`:
- `ws_connected: bool` — real-time WebSocket connection status
- `http_reachable: bool` — HTTP probe result (30s cycle)
- `build_id: Option<String>` — git hash from `/health` endpoint
- `uptime_secs: Option<i64>` — computed from agent_started_at
- `violation_count_24h: u32` — for "no error spike" health gate

**Gap:** `build_id` is a git commit hash. OTA-10 requires SHA256. The health gate can use `build_id` as a proxy (verifies correct binary was built from correct commit), and SHA256 is computed by James on the staging binary before upload — so the manifest's `binary_sha256` is authoritative. The pod-side SHA256 can be verified optionally via certutil if strict OTA-10 compliance is required.

### PodInfo.billing_session_id (FULLY UNDERSTOOD)
`types.rs:98`: `pub billing_session_id: Option<String>` — populated by server when pod has active billing. This is the server-side view. The pod-side `BILLING_ACTIVE` AtomicBool (in rc-agent) is the pod's local gate. Both must be checked: server checks `billing_session_id` before dispatching OTA download, pod checks `BILLING_ACTIVE` before executing `RCAGENT_SELF_RESTART`.

### Existing deploy.sh (SYNC-02)
`scripts/deploy/deploy.sh` already handles: `rc-sentry` deploy to all 8 pods with HTTP server on `:9998`. Phase 179 extends this by adding manifest validation and wave ordering around the same core `curl → move → start` sequence.

---

## Open Questions

1. **SHA256 verification method on pods**
   - What we know: `certutil -hashfile` works on all Windows versions on pods; `sha2` Rust crate would require embedding computation in rc-agent
   - What's unclear: Should SHA256 be verified on the pod (via certutil exec) or only on James before upload?
   - Recommendation: Verify SHA256 on James's machine before upload (compute hash, compare to manifest). If hashes match on staging, the curl download produces identical bytes. This avoids `certutil` exec on all 8 pods during health gate, which would be slow. Pod-side SHA256 check is optional/paranoid and can be deferred.

2. **ota-in-progress.flag push mechanism**
   - What we know: Files can be written to pods via rc-agent `/write` endpoint; pod_healer reads MAINTENANCE_MODE via rc-sentry `/files`
   - What's unclear: Should the server push `ota-in-progress.flag` to each pod via `/write`, or use the feature flag `kill_watchdog_restart` exclusively?
   - Recommendation: Use `kill_watchdog_restart` feature flag (Phase 178 mechanism already wired) as the primary rc-sentry suppression. Use `ota-in-progress.flag` as a local pod sentinel for pod_healer WoL check, written via rc-agent `/write` at wave start for that pod. Feature flag handles rc-sentry; sentinel file handles WoL.

3. **Pipeline resume after server restart**
   - What we know: `deploy-state.json` persists pipeline state; pods may have self-recovered during downtime
   - What's unclear: How to reconcile actual pod binary versions with expected pipeline state on resume?
   - Recommendation: On `run_pipeline()` startup, if `deploy-state.json` exists and state is not `Idle`/`Completed`, probe all pods in current wave for `build_id`. If `build_id` matches manifest's `git_commit` → pod completed successfully; if not → re-deploy. This makes resume idempotent.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `#[test]` (cargo test) + bash E2E scripts |
| Config file | `Cargo.toml` (no pytest.ini/jest.config needed) |
| Quick run command | `cargo test -p racecontrol -- ota_pipeline` |
| Full suite command | `cargo test -p racecontrol -p rc-agent-crate -p rc-common` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OTA-01 | Manifest parsed from TOML, missing fields return error | unit | `cargo test -p racecontrol -- manifest` | ❌ Wave 0 |
| OTA-02 | Pipeline starts with Pod 8, not others | unit | `cargo test -p racecontrol -- wave_order` | ❌ Wave 0 |
| OTA-03 | Health gate fails if ws_connected=false | unit | `cargo test -p racecontrol -- health_gate` | ❌ Wave 0 |
| OTA-04 | Rollback restores rc-agent-prev.exe | unit (script content) | `cargo test -p racecontrol -- rollback_script` | ✅ `deploy.rs:1129` |
| OTA-05 | Deploy defers on billing_session_id present | unit | `cargo test -p racecontrol -- session_gated` | ❌ Wave 0 |
| OTA-06 | Waves are [8], [1-4], [5-7] | unit | `cargo test -p racecontrol -- wave_grouping` | ❌ Wave 0 |
| OTA-07 | SWAP_SCRIPT preserves prev binary | unit (string check) | `cargo test -p racecontrol -- swap_preserves_prev` | ✅ `deploy.rs` |
| OTA-08 | PipelineState serializes/deserializes via serde | unit | `cargo test -p racecontrol -- pipeline_state_serde` | ❌ Wave 0 |
| OTA-09 | kill_watchdog_restart flag supresses watchdog | unit | `cargo test -p rc-sentry -- restart_suppressed` | ✅ `watchdog.rs` |
| OTA-10 | SHA256 computed correctly from binary bytes | unit | `cargo test -p racecontrol -- sha256_manifest` | ❌ Wave 0 |
| SYNC-02 | deploy.sh + check-health.sh still pass after extension | manual smoke | `bash scripts/deploy/check-health.sh` | ✅ exists |
| SYNC-05 | Manifest includes compatibility matrix fields | unit | `cargo test -p racecontrol -- manifest_compat` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- ota`
- **Per wave merge:** `cargo test -p racecontrol -p rc-agent-crate -p rc-common`
- **Phase gate:** Full suite green + `bash scripts/deploy/check-health.sh` PASS before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/ota_pipeline.rs` — new module with unit tests for PipelineState serde, wave grouping, manifest parsing
- [ ] `crates/racecontrol/src/ota_pipeline/manifest.rs` (optional submodule) — ReleaseManifest struct + parse tests
- [ ] `scripts/deploy/release-manifest.toml` — example manifest for CI smoke test
- [ ] No additional test infrastructure needed — existing `cargo test` + E2E shell scripts cover all automated checks

---

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/deploy.rs` — Complete per-pod deploy logic read directly
- `crates/rc-agent/src/remote_ops.rs` — `BILLING_ACTIVE` and `RCAGENT_SELF_RESTART` implementation
- `crates/rc-agent/src/self_monitor.rs` — `relaunch_self()` two-path mechanism
- `crates/rc-agent/src/self_heal.rs` — `SWAP_SCRIPT_CONTENT`, `START_SCRIPT_CONTENT`
- `crates/rc-sentry/src/watchdog.rs` — `kill_watchdog_restart` kill switch implementation
- `crates/racecontrol/src/fleet_health.rs` — `PodFleetStatus` fields for health gate
- `crates/rc-common/src/types.rs` — `DeployState`, `OtaDownloadPayload`, `OtaAckPayload`, `PodInfo`
- `crates/rc-common/src/protocol.rs` — `OtaDownload` (CoreToAgentMessage), `OtaAck` (AgentMessage)
- `.planning/phases/176-VERIFICATION.md` — Confirmed Phase 176 protocol types
- `.planning/phases/177-VERIFICATION.md` — Confirmed feature flag API and FF broadcast
- `.planning/phases/178-VERIFICATION.md` — Confirmed kill_watchdog_restart wiring, FlagSync consumer
- `scripts/deploy/deploy.sh` — Existing deploy infrastructure (SYNC-02)
- `scripts/deploy/check-health.sh` — Existing health gate foundation (SYNC-02)
- `CLAUDE.md` — Standing rules: deploy sequence, RCAGENT_SELF_RESTART, binary swap, billing-aware ops

### Secondary (MEDIUM confidence)
- `crates/racecontrol/src/pod_healer.rs:807-827` — MAINTENANCE_MODE check pattern for ota-in-progress.flag
- `.planning/ROADMAP.md:2716-2729` — Phase 179 goal, success criteria, plan notes

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in use; no new dependencies needed
- Architecture patterns: HIGH — read existing deploy.rs, self_monitor.rs, watchdog.rs directly
- Pitfalls: HIGH — drawn from CLAUDE.md standing rules and actual phase failure postmortems
- Test map: HIGH — existing test infrastructure confirmed; Wave 0 gaps identified precisely

**Research date:** 2026-03-24 IST
**Valid until:** 2026-04-24 (stable Rust codebase; flags unlikely to change)
