# Stack Research

**Domain:** Feature management, OTA binary/config/frontend delivery, server-to-pod config push, standing rules codification — v22.0 additions to existing Rust/Axum + Next.js fleet ops platform.
**Researched:** 2026-03-23
**Confidence:** HIGH (crates verified against docs.rs; versions confirmed; integration patterns verified against existing Cargo.toml)

---

## Context: What Already Exists (Do Not Re-research)

| Component | Version | Status |
|-----------|---------|--------|
| Rust | 1.93.1 | Installed, stable MSVC toolchain |
| Axum | 0.8 | racecontrol + rc-agent Cargo.toml |
| tokio | 1 (full features) | workspace |
| serde / serde_json | 1 | workspace |
| sha2 | 0.10 | workspace — already used for auth |
| hex | 0.4 | workspace |
| reqwest | 0.12 | racecontrol + rc-agent |
| tokio-tungstenite | 0.26 | both crates |
| sysinfo | 0.33 | both crates |
| tracing / tracing-subscriber | 0.1 / 0.3 | workspace |
| axum-server | 0.8 | racecontrol |
| sqlx (SQLite WAL) | 0.8 | racecontrol |
| toml | 0.8 | workspace |

**Implication:** sha2 + hex are already in scope for checksum verification. reqwest already available for binary downloads. tokio::sync::broadcast already available (tokio full features) for config push fan-out. No new auth or HTTP-client crates needed.

---

## Recommended Stack (New Additions Only)

### 1. Runtime Feature Flag Registry

**Approach: Custom in-memory registry in rc-common, no external crate.**

Do not add the `features` crate (crates.io). It uses a global proc-macro approach that does not support per-pod overrides, cannot be queried over HTTP, and cannot be persisted to SQLite. The existing pattern in this codebase (AppState in Arc<RwLock<...>>) is the right model.

**Implementation:**

```rust
// In rc-common: feature_flags.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlag {
    pub name: String,
    pub enabled: bool,
    pub per_pod: HashMap<u8, bool>,  // pod_number → override
}

pub type FeatureRegistry = Arc<RwLock<HashMap<String, FeatureFlag>>>;
```

- Server (racecontrol): holds the registry in `AppState`, persists to SQLite table `feature_flags` (name, enabled, per_pod_overrides jsonb).
- rc-agent: receives flag state via the existing WebSocket connection on reconnect and on push. Holds local `FeatureRegistry` in `AppState`.
- Admin dashboard: reads/writes via new REST endpoints on racecontrol.

**Why no external crate:** The `features` crate uses `AtomicBool` global statics — it cannot be scoped per-pod, cannot be serialized to the DB, and cannot be pushed over WebSocket. LaunchDarkly-style SDKs are cloud-hosted and overkill for an 8-pod LAN fleet. The custom approach is 100 lines of Rust and integrates natively with AppState.

---

### 2. Config Hot-Reload Without Binary Restart

**Add: `arc-swap` 1.9.0 to workspace.**

```toml
# Cargo.toml [workspace.dependencies]
arc-swap = "1"
```

Used in both `racecontrol` and `rc-agent`. Replace `Arc<RwLock<Config>>` with `Arc<ArcSwap<Config>>` for the runtime-mutable portions of config.

**Why arc-swap over RwLock<Config>:** arc-swap uses a hybrid hazard pointer + generation lock approach. Reads are lock-free — no contention between WebSocket handlers, billing loops, and config push handlers. `RwLock` causes CPU-level contention when many tasks read config concurrently (every WebSocket message). arc-swap has 143M downloads — it is the standard answer to this problem in Rust.

**Pattern for rc-agent config hot-reload:**

```rust
// AppState holds:
pub config: Arc<ArcSwap<AgentConfig>>,

// When server pushes new config via WebSocket:
let new_config = parse_pushed_config(msg)?;
state.config.store(Arc::new(new_config));
// All in-flight handlers pick up new config on next .load() call
```

**Scope of hot-reload (no restart needed):**
- Feature flag overrides
- Billing rate overrides
- Process guard allowlist updates
- Log verbosity changes

**NOT hot-reloadable (require restart):**
- Server bind address/port
- TLS certificate paths
- Database path
- WebSocket server URL (rc-agent)

---

### 3. File-Based Config Watch (James/Server local reload)

**Add: `notify` 8.2.0 to racecontrol only.**

```toml
# crates/racecontrol/Cargo.toml
notify = "8"
```

Used to watch `racecontrol.toml` for local edits on the server (Uday editing the file directly via SSH). On change event, reload the hot-reloadable config fields using arc-swap.store(). Debounce with a 500ms `tokio::time::sleep` to avoid reloading on every editor write.

**Why notify:** 62.7M downloads, used by rust-analyzer and cargo-watch. Version 8.2.0 is current and production-stable. Cross-platform (though server is Windows, so ReadDirectoryChangesW backend). Alternative is polling every N seconds — notify is reactive, lower latency, no CPU spin.

**NOT needed on rc-agent:** rc-agent's config is pushed from the server via WebSocket, not edited locally on pods. Do not add notify to rc-agent — it adds a dependency for a use case that doesn't exist on pods.

---

### 4. OTA Binary Delivery — Server Side

**No new crate. Use existing reqwest + sha2 + hex + tokio::fs.**

The existing deploy flow already has a working pattern: racecontrol serves a staging binary over HTTP :9998 (James's deploy-staging webterm.py). For v22.0, move this into a proper Axum endpoint on racecontrol itself:

```
GET /api/v1/ota/releases                    — list available releases (from manifest)
GET /api/v1/ota/releases/{version}/manifest — release manifest JSON
GET /api/v1/ota/releases/{version}/rc-agent — binary download (streaming)
GET /api/v1/ota/releases/{version}/config   — config bundle
POST /api/v1/ota/deploy                     — trigger deploy to pod(s)
```

Binaries are stored at `C:\RacingPoint\releases\{version}\` on the server. Streaming download via `tokio::fs::File` + `axum::body::Body::from_stream`. Checksum verification uses `sha2` (already in workspace) — compute SHA-256 of downloaded bytes and compare against manifest.

**Manifest format (serde_json, no new crate):**

```json
{
  "version": "22.0.3",
  "build_id": "a1b2c3d",
  "released_at": "2026-03-23T14:30:00+05:30",
  "components": {
    "rc-agent": {
      "sha256": "deadbeef...",
      "size_bytes": 4194304,
      "min_server_version": "22.0.0"
    },
    "frontend": {
      "sha256": "cafebabe...",
      "size_bytes": 2097152
    }
  },
  "rollback_to": "22.0.2",
  "standing_rules_version": "41"
}
```

---

### 5. OTA Binary Self-Replacement on rc-agent (Pods)

**Add: `self-replace` 1.5.0 to rc-agent only.**

```toml
# crates/rc-agent/Cargo.toml
self-replace = "1"
```

Used for the OTA update path where rc-agent downloads a new binary and swaps itself. On Windows, the existing `RCAGENT_SELF_RESTART` sentinel + `start-rcagent.bat` approach works but requires pre-staging the new binary manually. With self-replace, rc-agent can:

1. Download new binary to `rc-agent-new.exe` via reqwest (streaming, no full-memory load)
2. Verify SHA-256 against manifest
3. Call `self_replace::self_replace("rc-agent-new.exe")?`
4. Trigger `RCAGENT_SELF_RESTART` sentinel to restart via bat

**Why self-replace over the current bat swap approach:** The current approach requires James to stage the binary on :9998 and trigger a download. self-replace makes this fully autonomous — rc-agent initiates the download on server instruction, verifies integrity, and swaps atomically. Windows cannot delete a running exe, but self-replace handles this with a `.__selfdelete__.exe` deferred cleanup mechanism. Version 1.5.0 is current (verified docs.rs 2026-03-23).

**Constraint:** self-replace is added ONLY to rc-agent. racecontrol (server) does not self-update — it deploys via the existing SSH + schtasks path which is reliable and monitored by Uday.

---

### 6. Config Push Over Existing WebSocket

**No new crate. Use existing tokio::sync::broadcast in tokio (full features).**

Pattern: racecontrol holds a `broadcast::Sender<ConfigPushMessage>` in `AppState`. When admin changes a flag or config value, it broadcasts to all connected pod WebSocket handlers. Each handler receives the push and calls `arc-swap.store()`.

```rust
// In AppState (racecontrol):
pub config_push_tx: broadcast::Sender<ConfigPushMessage>,

// When admin API writes a flag:
let _ = state.config_push_tx.send(ConfigPushMessage::FeatureFlag {
    flag: "telemetry".into(),
    pod_override: Some((pod_number, enabled)),
    global: None,
});

// In pod WS handler loop:
while let Ok(msg) = config_push_rx.recv().await {
    handle_config_push(msg, &ws_sender).await?;
}
```

**Offline pod handling:** If a pod's WebSocket is disconnected, the broadcast message is lost. On reconnect, rc-agent requests a full config sync via a `ConfigSyncRequest` message. The server responds with current full state. This is already the right pattern — no queue/persist needed, because server always has the authoritative state.

**Why broadcast over mpsc:** broadcast allows one config-push event to fan out to all 8 pod WebSocket handlers simultaneously. mpsc would require one sender per pod. tokio::sync::broadcast is already in scope (tokio full features).

---

### 7. Cargo Feature Gates for Major Modules

**No new crate. Use Cargo's built-in [features] table.**

This is compile-time conditional compilation — not a crate to add. Define feature gates in the relevant Cargo.toml files:

```toml
# crates/rc-agent/Cargo.toml [features]
default = ["telemetry", "process-guard"]
telemetry = []
ai-debugger = ["dep:reqwest"]   # only pulls reqwest when AI debugger enabled
process-guard = []
camera-ai = []                  # for rc-sentry-ai: pulls onnxruntime
keyboard-hook = []              # existing — keep

# crates/racecontrol/Cargo.toml [features]
default = ["cloud-sync", "ota-pipeline"]
cloud-sync = []
ota-pipeline = []
```

**Convention:** Use `#[cfg(feature = "telemetry")]` blocks at the module level. Wrap entire `mod telemetry;` declarations with cfg gates. Do NOT gate individual fields in public structs (breaks downstream deserialization).

**Why this approach:** Cargo features are zero-runtime-cost, compile-time only. The `dep:` prefix syntax (Rust 2021+ edition — confirmed workspace uses edition = "2024") avoids implicit feature exposure. For modules that are always shipped, use runtime feature flags (approach #1 above). Cargo features are for modules that should not exist in the binary at all for certain deployment targets (e.g., camera-ai on pods vs server).

---

### 8. Standing Rules as Automated Checks

**No new crate. Use Rust's built-in test harness + shell scripts.**

Standing rules codification means: each rule gets a test that fails the pipeline when violated. Two tiers:

**Tier 1 — Cargo integration tests** (for Rust-verifiable rules):

```rust
// tests/standing_rules.rs in racecontrol crate
#[test]
fn rule_no_unwrap_in_production() {
    // Run `grep -r "\.unwrap()" src/` — fail if any found outside test modules
    // This is a compile-time check via clippy::unwrap_used lint
}
```

Better: add to `.cargo/config.toml`:
```toml
[target.x86_64-pc-windows-msvc]
rustflags = ["-D", "clippy::unwrap_used", "-D", "clippy::panic"]
```
Then `cargo clippy` enforces no-unwrap as a hard error.

**Tier 2 — Shell script gates** (for deploy/ops rules):

```bash
# scripts/check-standing-rules.sh
# Rule: static CRT
grep -q "crt-static" .cargo/config.toml || { echo "FAIL: static CRT not configured"; exit 1; }

# Rule: no hardcoded IPs in source
grep -rn "192\.168\.31\." crates/ --include="*.rs" | grep -v "test\|example\|comment" && { echo "FAIL: hardcoded IP in source"; exit 1; }

# Rule: LOGBOOK has entry for today
grep -q "$(date +%Y-%m-%d)" LOGBOOK.md || { echo "WARN: no LOGBOOK entry today"; }

# Rule: git status clean before deploy
git status --porcelain | grep -q "." && { echo "FAIL: uncommitted changes"; exit 1; }
```

**Integration with OTA pipeline:** The OTA deploy step runs `check-standing-rules.sh` before initiating a rollout. If any check fails, the pipeline aborts. This is the enforcement gate.

**Why no external test framework:** The existing pattern (comms-link `test/run-all.sh`) is already established. Standing rules are a mix of static analysis (clippy), grep-based source checks, and operational state checks. A dedicated test framework (nextest, etc.) adds complexity without benefit for this use case.

---

## Summary Table — New Additions Only

| Crate/Approach | Version | Add To | Purpose |
|----------------|---------|--------|---------|
| `arc-swap` | 1.9.0 | workspace | Lock-free atomic config hot-swap |
| `notify` | 8.2.0 | racecontrol only | File-watch `racecontrol.toml` for local edits |
| `self-replace` | 1.5.0 | rc-agent only | Binary self-replacement for OTA on pods |
| Custom FeatureRegistry | n/a | rc-common | Runtime per-pod feature flags (SQLite-backed) |
| Cargo `[features]` | builtin | rc-agent, rc-sentry-ai | Compile-time module exclusion |
| `tokio::sync::broadcast` | (already in tokio full) | racecontrol | Config push fan-out to all pods |
| `sha2` + `hex` | (already in workspace) | racecontrol, rc-agent | Release manifest checksum verification |
| Shell script gates | n/a | CI / OTA pipeline | Standing rules enforcement |
| Clippy lints in config | n/a | .cargo/config.toml | no-unwrap/no-panic as hard errors |

**Total new crates: 3** (`arc-swap`, `notify`, `self-replace`)

---

## Cargo.toml Changes

```toml
# Workspace root Cargo.toml — add to [workspace.dependencies]:
arc-swap = "1"

# crates/racecontrol/Cargo.toml — add:
arc-swap = { workspace = true }
notify = "8"

# crates/rc-agent/Cargo.toml — add:
arc-swap = { workspace = true }
self-replace = "1"
```

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| LaunchDarkly / Unleash / Flagsmith SDK | Cloud-hosted, overkill for 8-pod LAN, requires internet | Custom FeatureRegistry in rc-common (100 lines) |
| `features` crate (crates.io) | Global proc-macro statics — cannot do per-pod overrides, cannot serialize to DB, cannot push over WebSocket | Custom HashMap-based registry |
| `self_update` crate | Pulls GitHub API / S3 — wrong distribution model for internal LAN fleet; unnecessary transitive deps | `self-replace` (primitive) + existing reqwest for download |
| `figment` | Abstracts over config sources (env, file, CLI) — this project has one config format (TOML), adding figment just adds a dependency for existing toml + serde | `toml` (already in workspace) + `arc-swap` for hot-swap |
| `hot_reload` / `hot-lib-reloader` | These reload Rust dylibs at runtime — dynamic linking. Conflicts with static CRT constraint. | arc-swap for data hot-swap; Cargo features for compile-time gates |
| `notify` on rc-agent | rc-agent config is pushed from server via WebSocket, not edited locally on pods | tokio::sync::broadcast receive in WS handler |
| A separate config microservice | Would require new port, new process, new auth | WebSocket already exists between server and all pods |
| SaltStack for config distribution | v6.0 is blocked on BIOS AMD-V — cannot be unblocked before v22.0 | WebSocket push (existing infrastructure) |
| Separate OTA server process | Adds operational complexity; racecontrol already serves HTTP | Axum routes on racecontrol :8080 |

---

## Integration Points

### rc-common Changes

Add `feature_flags.rs` module:
- `FeatureFlag` struct (name, enabled, per_pod overrides map)
- `FeatureRegistry` type alias (`Arc<RwLock<HashMap<String, FeatureFlag>>>`)
- `is_flag_enabled(registry, name, pod_number)` helper
- `ConfigPushMessage` enum for WebSocket push messages (FeatureFlag variant, ConfigUpdate variant, OtaAvailable variant)

### racecontrol Changes

- `AppState`: add `feature_registry: FeatureRegistry`, `config_push_tx: broadcast::Sender<ConfigPushMessage>`, `current_config: Arc<ArcSwap<RaceControlConfig>>`
- New SQLite table: `feature_flags` (name TEXT PK, enabled BOOL, per_pod_json TEXT)
- New Axum routes: `/api/v1/features`, `/api/v1/ota/releases`, `/api/v1/ota/deploy`
- Startup: spawn notify watcher for `racecontrol.toml`

### rc-agent Changes

- `AppState`: add `feature_registry: FeatureRegistry`, `current_config: Arc<ArcSwap<AgentConfig>>`
- WS handler: handle `ConfigPushMessage` variants — update registry via arc-swap.store()
- OTA handler: download binary from server, verify SHA-256, call self_replace, trigger RCAGENT_SELF_RESTART
- On reconnect: send `ConfigSyncRequest` to get full current flag state

### Admin Dashboard Changes

- New page: Feature Flags — list all flags, toggle per-service, toggle per-pod
- New page: OTA Releases — list releases, trigger canary deploy, monitor rollout progress, one-click rollback

---

## Version Compatibility

| Package | Version | Compatible With | Notes |
|---------|---------|-----------------|-------|
| arc-swap 1.9.0 | Rust 1.65+ | tokio 1, Axum 0.8 | No special features needed; pure Rust atomic ops |
| notify 8.2.0 | Rust 1.70+ | tokio 1 (needs async feature or spawn_blocking) | Use `notify::recommended_watcher` with `spawn_blocking` wrapper; async watcher via `notify-async` is separate crate but spawn_blocking is simpler |
| self-replace 1.5.0 | Windows + Unix | No async dependency | Synchronous; call from `tokio::task::spawn_blocking` in OTA handler |
| sha2 0.10.9 | Already in workspace | hex 0.4 | No change needed — verify binary chunks with `Sha256::new().chain_update(chunk).finalize()` |

---

## Sources

- crates.io / docs.rs — `arc-swap` 1.9.0: HIGH confidence (docs.rs verified 2026-03-23)
- crates.io / docs.rs — `notify` 8.2.0: HIGH confidence (docs.rs verified 2026-03-23)
- crates.io / docs.rs — `self-replace` 1.5.0: HIGH confidence (docs.rs verified 2026-03-23)
- crates.io / docs.rs — `sha2` 0.10.9: HIGH confidence (docs.rs verified 2026-03-23)
- Cargo Book — Features chapter: HIGH confidence (official Rust documentation)
- Existing racecontrol Cargo.toml (read directly): HIGH confidence (project source 2026-03-23)
- WebSearch — `features` crate (crates.io): MEDIUM confidence (confirmed it uses global proc-macro statics, not per-entity scoping)
- WebSearch — self_update crate: MEDIUM confidence (confirmed it pulls GitHub/S3 backends, wrong fit for LAN fleet)
- arc-swap docs patterns page (docs.rs direct read): HIGH confidence

---

*Stack research for: v22.0 Feature Management & OTA Pipeline — runtime feature flags, config hot-reload, binary OTA, standing rules codification*
*Researched: 2026-03-23 IST*
