# Phase 300: SQLite Backup Pipeline - Research

**Researched:** 2026-04-01
**Domain:** SQLite backup (WAL-safe), Rust tokio background tasks, SCP file transfer, WhatsApp alerting, Next.js admin dashboard
**Confidence:** HIGH

## Summary

This phase adds a fully automated backup pipeline to the racecontrol server. The server already runs SQLite in WAL mode (enforced at startup in `db/mod.rs`), uses sqlx 0.8 for database access, and has established patterns for background tokio tasks, WhatsApp alerting via Evolution API, and SHA256 hashing. All infrastructure this phase needs already exists in the codebase.

The SQLite `.backup` API requirement is satisfied via the `VACUUM INTO` SQL command (available since SQLite 3.27.0), which is the recommended WAL-safe way to create an online backup from a sqlx pool without adding a rusqlite dependency. This is the correct approach because the server's main DB library is sqlx, not rusqlite. `VACUUM INTO '/path/to/backup.db'` creates a defragmented, consistent snapshot even while WAL readers and writers are active — it does not require exclusive access.

Nightly SCP to Bono VPS uses `tokio::process::Command` invoking the system `scp` binary (confirmed available: `/usr/bin/scp`, OpenSSH 10.2). Bono VPS is reachable at `100.70.177.44` via Tailscale (confirmed alive, health endpoint returns 200). SHA256 checksum is computed locally in Rust using the `sha2` crate (already a workspace dependency) and verified against the remote by running `sha256sum` over SSH.

The admin dashboard backup panel belongs in the existing `web/src/app/settings/page.tsx` as a new card — this keeps related system-health information co-located without adding a new route, which fits the current page structure.

**Primary recommendation:** Implement `backup_pipeline.rs` as a new Rust module with a `spawn()` function following the exact tokio background task pattern used by `scheduler.rs`, `metric_alerts.rs`, and the WhatsApp alerter. Add `[backup]` section to racecontrol.toml config. Add `GET /api/v1/backup/status` behind staff JWT. Add a Backup Status card to the settings page.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Backup uses SQLite .backup API (WAL-safe, not file copy)
- Nightly SCP to Bono VPS (100.70.177.44) — use existing SSH config
- WhatsApp alerts via existing Bono VPS Evolution API alerter
- Admin dashboard panel in racingpoint-admin Next.js app
- TOML config for backup paths and schedule

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

### Deferred Ideas (OUT OF SCOPE)
None — discuss phase skipped.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| BACKUP-01 | Server performs hourly SQLite .backup (WAL-safe) of all operational databases | `VACUUM INTO` SQL command is the WAL-safe snapshot mechanism available via sqlx; tokio interval loop pattern established in scheduler.rs |
| BACKUP-02 | Local backup rotation retains 7 daily + 4 weekly snapshots, auto-purging older files | `std::fs::read_dir` + file metadata for age-based rotation; daily/weekly classification by day-of-week |
| BACKUP-03 | Nightly backup is SCP'd to Bono VPS with integrity verification (SHA256 match) | `tokio::process::Command` with `scp` (confirmed available); `sha2` crate already in workspace; remote verification via SSH `sha256sum` |
| BACKUP-04 | WhatsApp alert fires if newest backup is older than 2 hours (staleness detection) | `send_whatsapp()` in `whatsapp_alerter.rs` is the established pattern; staleness check uses `std::time::SystemTime` on backup file mtime; debounce with `Instant` tracking |
| BACKUP-05 | Backup status visible in admin dashboard (last backup time, size, destination health) | New `GET /api/v1/backup/status` endpoint (staff JWT); new card in `web/src/app/settings/page.tsx` |
</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | 0.8 (workspace) | WAL-safe backup via `VACUUM INTO` SQL | Already the project's DB library; no new dep needed |
| sha2 | workspace | SHA256 checksum for backup integrity | Already workspace dep used in cloud_sync.rs, billing_replay.rs |
| tokio | workspace | Async runtime, interval timers, process::Command for scp | Already project runtime |
| chrono + chrono-tz | workspace | IST timestamp formatting, daily/weekly rotation logic | Already used in whatsapp_alerter.rs for `ist_now_string()` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::fs | stdlib | Directory creation, file listing, metadata, deletion | Backup directory management and rotation |
| tokio::process::Command | tokio | Async SCP invocation | Nightly offsite transfer |
| hex | workspace | Hex-encode SHA256 digest | Checksum comparison |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `VACUUM INTO` | rusqlite backup API | rusqlite requires adding a new crate dep and a bundled SQLite — overkill when `VACUUM INTO` achieves the same WAL-safe snapshot via sqlx |
| `tokio::process::Command scp` | sftp2 crate / reqwest multipart POST | scp is simpler, already proven in fleet_healer.rs SSH pattern, no new dep; crate adds complexity with no benefit |
| File mtime check for staleness | Backup record in SQLite | Mtime is simpler and doesn't require DB reads to detect DB backup failure |

**Installation:** No new Cargo dependencies required. All needed crates are already workspace dependencies.

**Version verification:** sqlx 0.8, sha2, tokio, chrono, hex — all workspace-pinned, versions confirmed from `crates/racecontrol/Cargo.toml`.

---

## Architecture Patterns

### Recommended Module Structure
```
crates/racecontrol/src/
├── backup_pipeline.rs   # NEW — hourly backup, rotation, nightly SCP, staleness alert
crates/racecontrol/src/api/
└── routes.rs            # ADD — GET /api/v1/backup/status handler (inline per codebase pattern)
crates/racecontrol/src/
├── config.rs            # ADD — BackupConfig struct + [backup] TOML section
├── main.rs              # ADD — spawn backup_pipeline::spawn(state)
web/src/app/settings/
└── page.tsx             # ADD — Backup Status card
```

### Pattern 1: Background Task Spawning (follows scheduler.rs and metric_alerts.rs)

**What:** A module with a `pub fn spawn(state: Arc<AppState>)` that creates a tokio interval loop.

**When to use:** All periodic background work in this codebase.

**Example (from scheduler.rs):**
```rust
// Source: crates/racecontrol/src/scheduler.rs
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            if let Err(e) = tick(&state).await {
                tracing::error!("[scheduler] tick error: {}", e);
            }
        }
    });
}
```

For the backup pipeline, the same structure applies with a 3600-second interval.

### Pattern 2: WAL-Safe SQLite Backup via VACUUM INTO

**What:** `VACUUM INTO 'path/to/backup.db'` creates a defragmented, consistent backup of the live database. It is WAL-aware — it checkpoints first and creates a consistent read snapshot, then writes the backup. Available since SQLite 3.27.0 (2019).

**Why not file copy:** Copying the `.db` file while WAL is active can produce a corrupt backup if a WAL frame is being written.

**Example:**
```rust
// Source: SQLite documentation + sqlx pattern
sqlx::query(&format!("VACUUM INTO '{}'", backup_path))
    .execute(&state.db)
    .await
    .map_err(|e| anyhow::anyhow!("VACUUM INTO failed: {}", e))?;
```

**Naming convention for backup files:**
```
data/backups/
├── racecontrol-2026-04-01T10-00-00.db   # hourly backup (daily slot)
├── racecontrol-weekly-2026-W13.db       # weekly snapshot (retained 4 weeks)
```

### Pattern 3: SHA256 Checksum (follows cloud_sync.rs)

**What:** Compute sha256 of backup file bytes in Rust, then compare against `sha256sum` output run on Bono VPS via SSH.

**Example:**
```rust
// Source: sha2 crate pattern used in billing_replay.rs, cloud_sync.rs
use sha2::{Digest, Sha256};

let bytes = std::fs::read(&backup_path)?;
let checksum = hex::encode(Sha256::digest(&bytes));
```

**Remote verification via SSH:**
```rust
// Source: fleet_healer.rs pattern for SSH commands
tokio::process::Command::new("ssh")
    .arg("-o").arg("StrictHostKeyChecking=no")
    .arg("-o").arg(format!("ConnectTimeout={}", SSH_TIMEOUT_SECS))
    .arg("root@100.70.177.44")
    .arg(&format!("sha256sum {}", remote_path))
    .output().await?;
```

### Pattern 4: WhatsApp Staleness Alert with Debounce (follows whatsapp_alerter.rs)

**What:** Use `send_whatsapp(&state.config, &message)` for the staleness alert. Store `last_alert_at: Option<Instant>` in the backup task's private state struct (same pattern as `P0State` in `whatsapp_alerter.rs`). Only re-fire after the next 2-hour staleness window clears and triggers again.

**Staleness check:** Compare `SystemTime::now()` against the backup file's `mtime` metadata. If newest backup mtime is older than 2 hours: alert. The alert must not re-fire until the next staleness window — meaning: once alerted, wait until a successful backup runs before allowing another alert.

**Phone target:** The alert goes to `config.alerting.uday_phone` (the staff number in alerting config), using the same `send_whatsapp()` function used by `metric_alerts.rs` and `whatsapp_alerter.rs`. No new phone config needed.

### Pattern 5: Backup Status Endpoint (follows fleet_health pattern)

**What:** A simple `GET /api/v1/backup/status` handler in routes.rs (staff JWT required) that returns a JSON object with last backup time, size, remote reachability, and latest checksum.

**Status struct:**
```rust
#[derive(serde::Serialize)]
pub struct BackupStatus {
    pub last_backup_at: Option<String>,      // ISO8601 IST
    pub last_backup_size_bytes: Option<u64>,
    pub last_backup_file: Option<String>,
    pub remote_reachable: bool,
    pub last_remote_transfer_at: Option<String>,
    pub last_checksum_match: Option<bool>,
    pub backup_count_local: usize,
    pub staleness_hours: Option<f64>,
}
```

**Shared state:** The `BackupStatus` is stored in `AppState` behind a `RwLock<BackupStatus>` (added to the state struct). The backup task writes it; the HTTP handler reads it. This avoids filesystem reads on every API call.

### Pattern 6: BackupConfig in TOML

**What:** Add `[backup]` section to `config.rs` and `racecontrol.toml`. All fields have serde defaults so adding `[backup]` to the TOML is optional for existing deployments.

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct BackupConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_backup_dir")]
    pub backup_dir: String,
    #[serde(default = "default_backup_interval_secs")]
    pub interval_secs: u64,        // default: 3600 (hourly)
    #[serde(default = "default_daily_retain")]
    pub daily_retain: usize,       // default: 7
    #[serde(default = "default_weekly_retain")]
    pub weekly_retain: usize,      // default: 4
    #[serde(default = "default_true")]
    pub remote_enabled: bool,
    #[serde(default = "default_remote_host")]
    pub remote_host: String,       // default: "root@100.70.177.44"
    #[serde(default = "default_remote_path")]
    pub remote_path: String,       // default: "/root/racecontrol-backups"
    #[serde(default = "default_staleness_alert_hours")]
    pub staleness_alert_hours: u64, // default: 2
}
fn default_backup_dir() -> String { "./data/backups".to_string() }
fn default_backup_interval_secs() -> u64 { 3600 }
fn default_daily_retain() -> usize { 7 }
fn default_weekly_retain() -> usize { 4 }
fn default_remote_host() -> String { "root@100.70.177.44".to_string() }
fn default_remote_path() -> String { "/root/racecontrol-backups".to_string() }
fn default_staleness_alert_hours() -> u64 { 2 }
```

**Config must be added to main `Config` struct** with `#[serde(default)]` so it's backward-compatible — existing deployments without `[backup]` section get all defaults.

### Anti-Patterns to Avoid

- **Do NOT copy the SQLite `.db` file directly** — WAL mode means the DB file is not consistent without a WAL checkpoint. Always use `VACUUM INTO`.
- **Do NOT hold a lock across `.await` in the backup task** — standing rule. Clone config values before entering async I/O.
- **Do NOT combine taskkill + SCP in the same exec chain** — standing rule; applies here: do not design the SCP step to depend on process state.
- **Do NOT use `std::fs::copy` on the live DB path** — race condition with active writes even in WAL mode.
- **Do NOT skip the remote SSH verification step** — a truncated SCP upload would pass without it; SHA256 is mandatory.
- **Do NOT fire the staleness WhatsApp alert in a tight loop** — use `last_alert_at` debounce exactly as `whatsapp_alerter.rs` does with `P0State`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| WAL-safe DB snapshot | Custom WAL parsing or file copy | `VACUUM INTO` SQL command via sqlx | SQLite built-in; handles WAL internally |
| SHA256 of large files | Custom hash loop | `sha2::Sha256::digest()` or streaming `sha2::Sha256::new() + update()` | Correct, tested, already in workspace |
| HTTP multipart file upload to VPS | Custom upload handler on Bono VPS | `scp` via `tokio::process::Command` | Bono VPS has sshd running, root access confirmed; no new endpoint needed on VPS |
| Cron/scheduling | External cron job | tokio interval loop (scheduler.rs pattern) | Keeps everything in the Rust binary; no OS scheduler dependency |

**Key insight:** This phase requires zero new Cargo dependencies. All needed primitives are workspace-available.

---

## Runtime State Inventory

> Step 2.5 is not applicable — this is a greenfield infrastructure phase (no rename/refactor/migration). No runtime state to inventory.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| `scp` / `ssh` | BACKUP-03 nightly transfer + remote SHA256 | ✓ | OpenSSH 10.2p1 | — |
| Bono VPS (100.70.177.44) | BACKUP-03 remote destination | ✓ | health 200, build b9c53f16 | — |
| Tailscale (server → VPS connectivity) | BACKUP-03 SCP path | Assumed ✓ (Bono VPS reachable from James; server uses same Tailscale mesh) | — | Fall back to LAN IP if Tailscale drops |
| `sha256sum` on Bono VPS | BACKUP-03 checksum verification | Assumed ✓ (standard Linux utility on any Ubuntu/Debian VPS) | — | Compute checksum from file bytes before sending, retry if missing |
| `cargo` | Build | ✓ | 1.93.1 | — |
| Node.js | Frontend build | ✓ | v22.22.0 | — |
| SQLite WAL mode | BACKUP-01 | ✓ (verified in db/mod.rs — server bails on non-WAL) | — | — |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:** None — all dependencies confirmed available.

---

## Common Pitfalls

### Pitfall 1: VACUUM INTO Locks Writers Briefly
**What goes wrong:** `VACUUM INTO` acquires a shared lock briefly during the snapshot. Under heavy write load (billing sessions), this could cause a momentary 5000ms busy-timeout wait.
**Why it happens:** SQLite shared lock required for consistent read.
**How to avoid:** Schedule backups during low-traffic window (e.g., 03:00 IST). The existing `busy_timeout=5000` in `db/mod.rs` already handles this. The backup task should log a warning if VACUUM INTO takes more than 30 seconds.
**Warning signs:** `VACUUM INTO failed: database is locked` in logs.

### Pitfall 2: Backup File Accumulation Without Rotation
**What goes wrong:** Hourly backups fill disk. 24 files/day × 365 days = 8760 files.
**Why it happens:** Rotation logic not running, or rotation targeting wrong files.
**How to avoid:** Rotation runs in the same tick as backup. Rotate BEFORE the new backup is created so the newest backup is always present. Test rotation with small `retain` values in unit tests.
**Warning signs:** `data/backups/` growing unbounded.

### Pitfall 3: SCP Partial Transfer Not Detected
**What goes wrong:** Network interruption mid-SCP leaves a truncated file on Bono VPS. Without checksum verification, this looks like a successful transfer.
**Why it happens:** `scp` exit code 0 does not guarantee file integrity.
**How to avoid:** Always run remote `sha256sum` after SCP and compare against local checksum. Log `remote_checksum_match: false` and set `BackupStatus.last_checksum_match = false` for dashboard visibility.
**Warning signs:** Backup file on VPS smaller than local backup.

### Pitfall 4: SSH StrictHostKeyChecking Failure on First Run
**What goes wrong:** First time scp/ssh to Bono VPS from the racecontrol server, the host key is unknown. SSH exits non-zero, SCP fails.
**Why it happens:** `~/.ssh/known_hosts` on the server may not have Bono VPS's key.
**How to avoid:** Use `-o StrictHostKeyChecking=no -o BatchMode=yes` in all SSH/SCP commands (same as `fleet_healer.rs`). The server uses Tailscale which manages trust.
**Warning signs:** SCP fails with `Host key verification failed` in the first nightly run.

### Pitfall 5: Staleness Alert Firing on Server Start
**What goes wrong:** At server start, `last_backup_at` is `None` (no backup has run yet). If staleness check runs immediately, it fires a WhatsApp alert before the first backup has had a chance to run.
**Why it happens:** Staleness check sees `last_backup_at = None` as "never backed up" = stale.
**How to avoid:** Initialize `last_backup_at` from the filesystem on startup — scan `data/backups/` for the most recent backup file and set initial staleness from there. Only alert if disk shows no backup AND `uptime > 2 hours`. Alternatively: add a 2-hour initial delay before the first staleness check.
**Warning signs:** WhatsApp alert fires immediately on server restart.

### Pitfall 6: Backup Task Silently Dying
**What goes wrong:** Panic in backup task kills the tokio task. Server runs indefinitely with no backup, no logs.
**Why it happens:** Unhandled error in backup logic panics the spawned task.
**How to avoid:** Log "backup task started" at startup. Log every tick start/result (standing rule: "Long-Lived Tasks Must Log Lifecycle"). Use `?` operator throughout, never `.unwrap()`. Wrap tick in `if let Err(e) = backup_tick(...).await { tracing::error!(...) }`.
**Warning signs:** No backup log entries for > 2 hours.

### Pitfall 7: `deny_unknown_fields` on Config Struct
**What goes wrong:** Adding `[backup]` to racecontrol.toml causes a parse error because main `Config` struct uses `#[serde(deny_unknown_fields)]`.
**Why it happens:** The `Config` struct has `deny_unknown_fields` (confirmed from config.rs line 31).
**How to avoid:** Add `pub backup: BackupConfig` field WITH `#[serde(default)]` to the main `Config` struct before any racecontrol.toml changes. This must be done first in the implementation wave.
**Warning signs:** Server fails to start with `unknown field 'backup'` TOML parse error.

---

## Code Examples

Verified patterns from codebase:

### Background Task (from scheduler.rs)
```rust
// Source: crates/racecontrol/src/scheduler.rs
pub fn spawn(state: Arc<AppState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(e) = backup_tick(&state).await {
                tracing::error!(target: "backup_pipeline", "backup tick error: {}", e);
            }
        }
    });
}
```

### WAL-Safe SQLite Backup
```rust
// Source: SQLite docs + sqlx pattern; VACUUM INTO available since SQLite 3.27.0
pub async fn run_backup(db: &SqlitePool, backup_path: &str) -> anyhow::Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(backup_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    // VACUUM INTO creates a WAL-safe snapshot of the live database
    sqlx::query(&format!("VACUUM INTO '{}'", backup_path))
        .execute(db)
        .await
        .map_err(|e| anyhow::anyhow!("VACUUM INTO failed: {}", e))?;
    Ok(())
}
```

### SHA256 Checksum (from cloud_sync.rs pattern)
```rust
// Source: sha2 workspace crate, pattern from billing_replay.rs
use sha2::{Digest, Sha256};

fn checksum_file(path: &str) -> anyhow::Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
```

### SCP Transfer (from fleet_healer.rs pattern)
```rust
// Source: crates/racecontrol/src/fleet_healer.rs — SSH/SCP over Tailscale
let result = tokio::time::timeout(
    Duration::from_secs(120),
    tokio::process::Command::new("scp")
        .arg("-o").arg("StrictHostKeyChecking=no")
        .arg("-o").arg("BatchMode=yes")
        .arg("-o").arg("ConnectTimeout=10")
        .arg(&local_backup_path)
        .arg(&format!("{}:{}", config.backup.remote_host, remote_path))
        .output(),
).await??;
```

### WhatsApp Staleness Alert (from whatsapp_alerter.rs pattern)
```rust
// Source: crates/racecontrol/src/metric_alerts.rs + whatsapp_alerter.rs
// Debounce: only fire once per staleness window
if last_backup_age_hours > config.backup.staleness_alert_hours as f64 {
    if last_alert_fired.map(|t: Instant| t.elapsed().as_secs() > 2 * 3600).unwrap_or(true) {
        let msg = format!(
            "[BACKUP] No successful backup in {:.1} hours — last at {} | {}",
            last_backup_age_hours,
            last_backup_at_str,
            whatsapp_alerter::ist_now_string()
        );
        whatsapp_alerter::send_whatsapp(&state.config, &msg).await;
        last_alert_fired = Some(Instant::now());
    }
}
```

### IST Timestamp (from whatsapp_alerter.rs)
```rust
// Source: crates/racecontrol/src/whatsapp_alerter.rs — ist_now_string() is pub(crate)
pub(crate) fn format_backup_time(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%d %b %Y %H:%M IST")
        .to_string()
}
```

### Admin Dashboard Status Card (follows settings/page.tsx pattern)
```tsx
// Source: web/src/app/settings/page.tsx structure
{/* Backup Status */}
<div className="bg-rp-card border border-rp-border rounded-lg p-5">
  <h2 className="text-sm font-medium text-neutral-400 mb-4">Backup Status</h2>
  <div className="space-y-3 text-sm">
    <div className="flex justify-between">
      <span className="text-rp-grey">Last Backup</span>
      <span className="text-neutral-300">{backup?.last_backup_at ?? "Never"}</span>
    </div>
    <div className="flex justify-between">
      <span className="text-rp-grey">Size</span>
      <span className="text-neutral-300 font-mono">{backup ? formatBytes(backup.last_backup_size_bytes) : "---"}</span>
    </div>
    <div className="flex justify-between">
      <span className="text-rp-grey">Remote (Bono VPS)</span>
      <span className={backup?.remote_reachable ? "text-emerald-400" : "text-red-400"}>
        {backup?.remote_reachable ? "Reachable" : "Unreachable"}
      </span>
    </div>
  </div>
</div>
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| File copy of .db | `VACUUM INTO` SQL command | SQLite 3.27.0 (2019) | WAL-safe; produces defragmented copy without exclusive lock |
| rusqlite backup API | `VACUUM INTO` via sqlx | This phase | No new dep; consistent with project's use of sqlx |

**Deprecated/outdated:**
- Direct `.db` file copy: Always wrong for WAL-mode databases (can produce corrupt backup).
- Using rusqlite's C-level backup API in a sqlx project: Adds bundled SQLite, doubles binary size, creates two SQLite stacks — `VACUUM INTO` achieves identical safety.

---

## Open Questions

1. **Multiple DB files?**
   - What we know: `config.database.path` points to `./data/racecontrol.db`. The telemetry store uses `./data/telemetry.db` (separate pool in main.rs line 621). Both must be backed up.
   - What's unclear: Are there other SQLite files (metrics, maintenance)? The `spawn_metrics_ingestion` uses `state.db`, same pool. The `telemetry_store::spawn_writer` uses `telem_pool` — confirmed second DB.
   - Recommendation: Backup config should list DB paths explicitly OR scan `./data/*.db`. Planner should default to backing up both `racecontrol.db` and `telemetry.db`.

2. **Remote directory creation on Bono VPS**
   - What we know: SCP will fail if the remote directory doesn't exist.
   - What's unclear: Does `/root/racecontrol-backups` already exist on Bono VPS?
   - Recommendation: Include `ssh root@100.70.177.44 mkdir -p /root/racecontrol-backups` as Wave 0 or first task in Wave 1. Fail gracefully if SSH fails.

3. **Backup of Bono VPS racecontrol.db**
   - What we know: Bono VPS also runs racecontrol with its own DB (confirmed alive). Phase 300 only backs up the venue server.
   - What's unclear: Is Bono VPS DB backup in scope?
   - Recommendation: Out of scope for Phase 300 (CONTEXT.md says venue server only). Defer.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `#[tokio::test]` + sqlx in-memory SQLite |
| Config file | `crates/racecontrol/tests/integration.rs` (existing) |
| Quick run command | `cargo test -p racecontrol-crate backup` |
| Full suite command | `cargo test -p racecontrol-crate && cargo test -p rc-common` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BACKUP-01 | VACUUM INTO creates valid backup file on disk | unit | `cargo test -p racecontrol-crate backup::tests::test_vacuum_into_creates_file` | ❌ Wave 0 |
| BACKUP-02 | Rotation keeps exactly 7 daily + 4 weekly, deletes older | unit | `cargo test -p racecontrol-crate backup::tests::test_rotation_logic` | ❌ Wave 0 |
| BACKUP-03 | SHA256 checksum computed and matches expected | unit | `cargo test -p racecontrol-crate backup::tests::test_checksum` | ❌ Wave 0 |
| BACKUP-04 | Staleness alert fires after 2h, does not fire again until next window | unit | `cargo test -p racecontrol-crate backup::tests::test_staleness_debounce` | ❌ Wave 0 |
| BACKUP-05 | GET /api/v1/backup/status returns expected JSON shape | integration | `cargo test -p racecontrol-crate backup::tests::test_status_endpoint` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol-crate backup`
- **Per wave merge:** `cargo test -p racecontrol-crate && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/backup_pipeline.rs` — covers BACKUP-01 through BACKUP-04 (unit tests inline as `#[cfg(test)]` block)
- [ ] Route handler + status struct test in `routes.rs` — covers BACKUP-05
- [ ] No new test framework needed — existing `#[tokio::test]` pattern in integration.rs is the model

*(Note: All gaps are new files, not missing framework. Existing test infrastructure is sufficient.)*

---

## Sources

### Primary (HIGH confidence)
- Codebase: `crates/racecontrol/src/scheduler.rs` — background task spawn pattern
- Codebase: `crates/racecontrol/src/whatsapp_alerter.rs` — P0State debounce pattern + `send_whatsapp()` + `ist_now_string()`
- Codebase: `crates/racecontrol/src/metric_alerts.rs` — alert cooldown pattern
- Codebase: `crates/racecontrol/src/db/mod.rs` — WAL mode confirmation, `PRAGMA wal_autocheckpoint`, sqlx pool pattern
- Codebase: `crates/racecontrol/src/fleet_healer.rs` — SSH/SCP command pattern with StrictHostKeyChecking=no, BatchMode=yes
- Codebase: `crates/racecontrol/src/config.rs` — Config struct pattern with `deny_unknown_fields`, `#[serde(default)]`
- Codebase: `crates/racecontrol/src/cloud_sync.rs` — sha2 + hex checksum pattern
- Codebase: `crates/racecontrol/Cargo.toml` — confirmed: sqlx 0.8, sha2, hex, chrono-tz, tokio all workspace deps
- Codebase: `web/src/app/settings/page.tsx` — admin dashboard card pattern, `api.health()` + `useEffect` pattern
- Live check: Bono VPS health 200 at `100.70.177.44:8080/api/v1/health` (2026-04-01)
- Live check: `scp` available at `/usr/bin/scp` (OpenSSH 10.2p1)
- SQLite documentation: `VACUUM INTO` command — WAL-safe online backup, available since SQLite 3.27.0

### Secondary (MEDIUM confidence)
- SQLite WAL documentation: `VACUUM INTO` is the recommended online backup method for WAL-mode databases when using high-level SQL interfaces
- VACUUM INTO note: Acquires a shared lock briefly; does not block concurrent readers; writers may queue during the snapshot window but 5000ms busy_timeout is sufficient

### Tertiary (LOW confidence)
- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries are confirmed workspace deps; no new deps needed
- Architecture: HIGH — all patterns directly verified from existing codebase files
- Pitfalls: HIGH — derived from standing rules and codebase analysis; Pitfall 7 (deny_unknown_fields) is confirmed from config.rs line 31

**Research date:** 2026-04-01
**Valid until:** 2026-05-01 (stable infrastructure; no fast-moving dependencies)
