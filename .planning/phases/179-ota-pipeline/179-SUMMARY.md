# Phase 179: OTA Pipeline — SUMMARY

**Status:** COMPLETE
**Commit:** `95f8f67e`
**Tests:** 31 passing (ota_pipeline module), 500+ total unaffected

## What Was Built

### Plan 01: Foundation Types (ota_pipeline.rs)
- `ReleaseManifest` struct with TOML parsing (release info, binary SHA256 hashes, compatibility matrix, deploy config)
- `PipelineState` enum: 8 variants (Idle → Building → Staging → Canary → StagedRollout → HealthChecking → Completed/RollingBack)
- `DeployRecord` for deploy-state.json persistence — atomic write (tmp + rename), survives server restart
- `compute_sha256()` + `compute_sha256_file()` streaming SHA256 utilities
- Wave constants: WAVE_1=[8], WAVE_2=[1,2,3,4], WAVE_3=[5,6,7]
- Example `release-manifest.toml` in scripts/deploy/

### Plan 02: Health Gate + rc-agent SHA256
- `health_check_pod()` — pure function verifying WS connected, HTTP reachable, binary SHA256 match, error spike threshold
- `PipelineError` enum with structured context for rollback decisions
- `HealthFailure` struct for per-pod failure reporting
- rc-agent: sha2 dependency added, `BINARY_SHA256` OnceLock static, `init_binary_sha256()` called at startup
- rc-agent /health endpoint now returns `binary_sha256` field

### Plan 03: Rollback + Sentinels + pod_healer
- `rollback_wave()` — writes do-rollback.bat via rc-agent /write, executes via rc-sentry :8091/exec (NOT rc-agent)
- `set_ota_sentinel()` / `clear_ota_sentinel()` — ota-in-progress.flag on each pod
- `set_kill_switch()` — sentry-flags.json with kill_watchdog_restart via /write
- pod_healer: OTA sentinel check (CHECK 2b) added before WoL, mirrors MAINTENANCE_MODE pattern
- deploy.rs: `ROLLBACK_SCRIPT_CONTENT` made `pub const` for reuse

### Plan 04: API + Integration
- `POST /api/v1/ota/deploy` — accepts TOML manifest, spawns background pipeline
- `GET /api/v1/ota/status` — returns current pipeline state from deploy-state.json
- `check_interrupted_pipeline()` called at racecontrol startup — detects non-terminal states
- Routes registered in staff_routes (requires JWT auth)

## Requirements Coverage
| Req | Description | Status |
|-----|-------------|--------|
| OTA-01 | Release manifest locks SHA256 + versions | DONE |
| OTA-02 | Canary Pod 8 first | DONE |
| OTA-03 | Active billing session deferral | DONE (has_active_billing_session) |
| OTA-04 | Auto-rollback on health gate failure | DONE |
| OTA-05 | Health gate: WS + HTTP + SHA256 + error spike | DONE |
| OTA-06 | Staged waves: 8 → 1-4 → 5-7 | DONE |
| OTA-07 | rc-agent-prev.exe preserved (SWAP_SCRIPT) | DONE |
| OTA-08 | Pipeline state persisted to deploy-state.json | DONE |
| OTA-09 | OTA sentinel + kill_watchdog_restart coordination | DONE |
| OTA-10 | SHA256 content hash for binary identity | DONE |
| SYNC-02 | check-health.sh foundation | PARTIAL (script not yet extended) |
| SYNC-05 | Compatibility matrix in manifest | DONE |

## Files Modified
- `crates/racecontrol/src/ota_pipeline.rs` (NEW — 730 lines)
- `crates/racecontrol/src/lib.rs` (+1 line: pub mod)
- `crates/racecontrol/src/deploy.rs` (const → pub const)
- `crates/racecontrol/src/api/routes.rs` (+OTA routes + handlers)
- `crates/racecontrol/src/main.rs` (+check_interrupted_pipeline call)
- `crates/racecontrol/src/pod_healer.rs` (+OTA sentinel CHECK 2b)
- `crates/rc-agent/Cargo.toml` (+sha2 dependency)
- `crates/rc-agent/src/main.rs` (+init_binary_sha256 call)
- `crates/rc-agent/src/remote_ops.rs` (+BINARY_SHA256 OnceLock + /health field)
- `scripts/deploy/release-manifest.toml` (NEW — example manifest)
