# Phase 1: State Wiring & Config Hardening - Research

**Researched:** 2026-03-13
**Domain:** Rust/Axum AppState initialization, TOML config validation, HTTP API error semantics, Windows bat deploy scripting
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Config failure behavior**
- rc-agent shows a branded error on the lock screen when config is invalid ("Configuration Error — contact staff") — no file paths or system details exposed
- Validate critical fields only: server URL (valid URL), pod number (1-8), billing rates (must be > 0), game paths. Optional fields use defaults.
- Validate field values, not just presence — catches typos like rate_30min = 0 that would give free sessions
- Fail immediately on invalid config — no retry delay. Watchdog/HKLM Run key handles restarts, so retry in the agent is redundant.

**Deploy config cleanup**
- Overwrite old config without backup — source of truth is deploy-staging on James (.27), not what's on the pod
- One shared config template with only `pod_number` as the per-pod field — everything else identical across all 8 pods
- Deploy process: delete old racecontrol.toml → write new one → start rc-agent

**pod-agent error reporting**
- /exec returns JSON with { success, exit_code, stdout, stderr } — enough for James to diagnose without SSHing into the pod
- Proper HTTP status codes: 200 success, 400 bad request (missing cmd field), 500 command execution failure
- 30s default timeout with override via { cmd: "...", timeout: 60 } in request body — prevents hung commands blocking the endpoint
- pod-agent binds to LAN only (192.168.31.x, not 0.0.0.0) — no auth needed, router NAT blocks external access. Note: pods DO have internet access for online games (iRacing, LMU).

**AppState wiring**
- Pre-populate pod_backoffs entries for all 8 pods at rc-core startup — pod_monitor never encounters a missing entry
- Send a test email on first boot only (flag file after first success) — verifies Gmail OAuth works without spamming Uday's inbox on every restart
- Backoff step durations (30s→2m→10m→30m) hardcoded — no config file tuning for now
- No network ping check at startup — pods check in via WebSocket when they come online, "offline" is the default state

### Claude's Discretion
- Exact lock screen error message wording and styling
- Which config fields count as "optional" vs "critical" beyond the explicitly listed ones
- How pod_backoffs entries are keyed (pod_id string vs pod_number)
- Error log format for config validation failures

### Deferred Ideas (OUT OF SCOPE)
- Multi-venue support — config portability for new venues, venue_id field, centralized config management. Future project, not this phase.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| WD-02 | pod_monitor and pod_healer share backoff state via AppState — no concurrent restarts | AppState.pod_backoffs already defined as `RwLock<HashMap<String, EscalatingBackoff>>`. pod_monitor already uses `entry().or_insert_with()` pattern. Gap: entries not pre-populated at startup — pod_monitor creates them on first stale detection. Need initialization loop in AppState::new(). |
| DEPLOY-01 | rc-agent validates all required config fields at startup, exits non-zero on invalid config | load_config() in rc-agent/src/main.rs returns Ok(default) when no config file found — this is the primary bug. Validation must run post-parse and call std::process::exit(1) with a branded lock screen error before returning. |
| DEPLOY-03 | pod-agent /exec returns clear success/failure status (not HTTP 200 for everything) | exec_command() in pod-agent/src/main.rs currently returns Ok(Json(...)) for ALL cases including timeout and spawn failure — HTTP 200 always. Must add `success` field and correct HTTP 500 on non-zero exit / spawn error. |
| DEPLOY-04 | Deploy wipes old config files from pods before writing new config — no stale config remnants | install.bat already deletes rc-agent.toml (line 55: `del /Q %DEST%\rc-agent.toml`). Gap is in remote deploy via pod-agent /exec: the JSON payload used by pod_monitor restart cmd does NOT delete old config. Need a deploy helper script or explicit del step in remote deploy payloads. |
</phase_requirements>

---

## Summary

Phase 1 is pure integration and hardening — every component already exists, nothing needs to be designed from scratch. The work is four focused changes across three codebases (rc-agent, pod-agent, rc-core).

The most impactful change is rc-agent config validation. Currently `load_config()` silently falls back to a default config when no config file is found — a pod with a missing or corrupt `rc-agent.toml` will start with pod_number=1, connecting as "Pod 01" regardless of which physical pod it's on. Billing rates are not present in rc-agent's config (billing lives in rc-core), so the "rate must be > 0" validation listed in CONTEXT.md applies to rc-core's `racecontrol.toml`, not rc-agent. The critical rc-agent fields are: server URL (must be a valid WebSocket URL), pod number (1-8), and pod name (non-empty).

The pod-agent /exec fix is straightforward but high-value: the current implementation returns HTTP 200 for spawn failures and timeouts with no `success` field, meaning rc-core's pod_monitor cannot distinguish a successful restart from a failed one. Adding a `success: bool` field and returning HTTP 500 on non-zero exit codes makes the existing pod_monitor log messages accurate.

AppState.pod_backoffs pre-population is the smallest change — a single initialization loop in `AppState::new()` that inserts `EscalatingBackoff::new()` for all 8 pods by their pod_id string key ("pod_1" through "pod_8"). The structure and types already exist and are tested.

**Primary recommendation:** Implement changes in this order — pod-agent /exec (isolated, no tests to break), rc-agent config validation (high blast radius, test carefully), AppState pre-population (lowest risk), deploy script cleanup (verify on Pod 8 last).

---

## Standard Stack

### Core (already in use — no new dependencies)

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| toml | 0.8 | TOML deserialization | Already in workspace, used by both rc-agent and rc-core |
| anyhow | 1 | Error propagation in Rust | Already used in rc-agent main() return type |
| axum | (via rc-core Cargo.toml) | HTTP routing in pod-agent | pod-agent is a separate Cargo workspace at /c/Users/bono/racingpoint/pod-agent/ |
| url | (not yet added) | URL validation in rc-agent | May need to add for server URL format check — or use a simple regex/prefix check |

### Validation approach for rc-agent

The `AgentConfig` struct uses `#[derive(Deserialize)]` via serde. Post-parse validation is the correct pattern — do NOT add serde validation attributes (they give poor error messages and can't do cross-field checks). Instead, add a `validate()` method or standalone function called immediately after `toml::from_str` succeeds.

For server URL validation, the simplest approach is a prefix check (`starts_with("ws://") || starts_with("wss://")`) rather than pulling in the `url` crate. This avoids a new dependency and is sufficient to catch the "forgot the url prefix" class of typo.

**Installation:** No new dependencies needed for any of the four tasks.

---

## Architecture Patterns

### Pattern 1: Post-parse validation function in rc-agent

**What:** A standalone `validate_config(config: &AgentConfig) -> Result<(), String>` function called in `load_config()` after successful deserialization.

**When to use:** After `toml::from_str` succeeds. Validation errors are returned as `Err(String)` with a human-readable message. The caller (main) converts validation errors to a lock screen display + `std::process::exit(1)`.

**Critical fields to validate (rc-agent):**
- `pod.number`: must be 1-8 (inclusive). If 0 or > 8, reject.
- `pod.name`: must be non-empty after trim.
- `core.url`: must start with `ws://` or `wss://`. Empty is also invalid.
- No config file found at all: fail immediately, do NOT use defaults.

**Billing rates note:** Billing rates (`rate_30min`, `rate_60min`) live in `racecontrol.toml` (rc-core's config), NOT in `rc-agent.toml`. rc-agent has no billing fields. The "must be > 0" rate validation belongs in rc-core's `Config::load()` path, not rc-agent's. CONTEXT.md says "billing rates" under rc-agent validation — this is likely referring to rc-core's startup validation. Confirmed: rc-core's `VenueConfig` has no billing rate fields either; billing rates live in the DB (kiosk_settings table). The Config validation in rc-core may not need rate validation at all, or it's out of scope for Phase 1.

**Example pattern:**

```rust
// In rc-agent/src/main.rs

fn validate_config(config: &AgentConfig) -> Result<(), String> {
    if config.pod.number < 1 || config.pod.number > 8 {
        return Err(format!(
            "pod.number must be 1-8, got {}",
            config.pod.number
        ));
    }
    if config.pod.name.trim().is_empty() {
        return Err("pod.name must not be empty".to_string());
    }
    let url = config.core.url.trim();
    if !url.starts_with("ws://") && !url.starts_with("wss://") {
        return Err(format!(
            "core.url must start with ws:// or wss://, got: {}",
            url
        ));
    }
    Ok(())
}

fn load_config() -> Result<AgentConfig> {
    let paths = ["rc-agent.toml", "/etc/racecontrol/rc-agent.toml"];
    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            let config: AgentConfig = toml::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Config parse error in {}: {}", path, e))?;
            validate_config(&config)
                .map_err(|e| anyhow::anyhow!("Config validation error: {}", e))?;
            tracing::info!("Loaded config from {}", path);
            return Ok(config);
        }
    }
    // NO fallback to defaults — fail hard
    Err(anyhow::anyhow!(
        "No config file found. Create rc-agent.toml in C:\\RacingPoint\\"
    ))
}
```

**CRITICAL:** The current `load_config()` uses `?` operator but main.rs returns `Result<()>`. A config validation error propagated via `?` will print the error to stderr and exit with non-zero — this is exactly the DEPLOY-01 requirement. No special `std::process::exit(1)` call is needed if `main()` propagates the error.

**Lock screen error display:** The config error must be shown on the branded lock screen, not just stderr. The challenge is that `LockScreenManager` is initialized AFTER `load_config()` in the current startup sequence. Options:
1. Initialize a minimal lock screen before config load, show error there. (Complex — browser/HTTP server setup needed before config.)
2. Show error via lock screen's existing `Disconnected` state or a new `ConfigError` state, but this requires refactoring the startup order.
3. Start with the lock screen server initialized early, showing a "starting..." state, then update to error if config fails.

The simplest approach that matches the CONTEXT.md requirement: Add a new `LockScreenState::ConfigError { message: String }` variant, initialize the lock screen server before config validation, show the error, then loop forever (watchdog will restart). The message shown to customers is "Configuration Error — contact staff" (not the technical details).

### Pattern 2: AppState pod_backoffs pre-population

**What:** In `AppState::new()`, after `pod_backoffs: RwLock::new(HashMap::new())`, immediately populate with entries for all 8 pods.

**Keying:** pod_id strings "pod_1" through "pod_8" — consistent with how pod_monitor generates keys via `format!("pod_{}", config.pod.number)` in rc-agent, and how pods are identified throughout the system.

**Example:**

```rust
// In rc-core/src/state.rs — AppState::new()

let mut initial_backoffs = HashMap::new();
for pod_num in 1..=8u32 {
    initial_backoffs.insert(
        format!("pod_{}", pod_num),
        EscalatingBackoff::new(),
    );
}
// ...
Self {
    // ...
    pod_backoffs: RwLock::new(initial_backoffs),
    // ...
}
```

Note: the config has `pods.count` (defaults to 16) but the user decision is to hardcode 8. Use the literal range `1..=8` rather than reading from config — config says "Claude's discretion" for now.

### Pattern 3: pod-agent /exec honest HTTP status codes

**What:** The existing `exec_command` handler in pod-agent returns `Ok(Json(ExecResponse {...}))` (HTTP 200) for ALL outcomes including spawn errors and timeouts. The fix adds a `success` field and uses `Err((StatusCode, Json<ExecResponse>))` for failures.

**Current signature:**
```rust
async fn exec_command(Json(req): Json<ExecRequest>)
    -> Result<Json<ExecResponse>, (StatusCode, Json<ExecResponse>)>
```

The return type already supports error responses — it's just not using them for failure cases.

**New ExecResponse:**
```rust
#[derive(Serialize)]
struct ExecResponse {
    success: bool,       // ADD THIS
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}
```

**HTTP mapping:**
- Semaphore exhausted: 429 (already correct)
- Spawn failure (e.g., `cmd` not found): 500 + `success: false`
- Timeout: 500 + `success: false` + exit_code: Some(124) (keep existing sentinel)
- Non-zero exit code: 500 + `success: false`
- Zero exit code: 200 + `success: true`

**IMPORTANT:** The existing `timeout_ms` field in `ExecRequest` is used internally but the user wants `timeout` (in seconds) as the external field per CONTEXT.md (`{ cmd: "...", timeout: 60 }`). Clarify: CONTEXT.md says `timeout: 60` — this is in seconds, but the internal timeout uses milliseconds. The ExecRequest needs `timeout_ms: Option<u64>` for backward compat OR rename to `timeout_secs: Option<u64>`. Keep `timeout_ms` for backward compat — existing callers (pod_monitor.rs restart cmd JSON) use `timeout_ms: 10000`.

**Binding to LAN only:** pod-agent currently binds to `0.0.0.0:8090`. The decision is to bind to the pod's LAN IP (192.168.31.x). This requires detecting the local IP at startup and binding to it. The existing `local_ip()` function returns the 192.168.31.x IP via ipconfig parsing. Use this to construct the bind address. Fallback to 0.0.0.0 if detection fails (degraded but still functional).

### Pattern 4: Deploy config cleanup in remote deploy

**What:** The install.bat already deletes rc-agent.toml before writing new (line 55). The gap is in remote deploy via pod-agent /exec payloads used from James's machine.

**Current remote deploy flow (from MEMORY.md):**
1. Copy binary to deploy-staging, start HTTP server on port 9998
2. POST JSON to pod-agent /exec with a cmd string that downloads and starts rc-agent

**The fix:** Add explicit config file deletion and write steps to the remote deploy sequence. Two approaches:
- Option A: Use pod-agent `/write` endpoint to push config content directly (no delete needed since `/write` overwrites).
- Option B: Add `del /Q C:\RacingPoint\rc-agent.toml` to the deploy cmd string before writing new config.

Option A (using `/write`) is cleaner — avoids shell quoting issues and is already implemented in pod-agent. The deploy sequence becomes:
1. POST /write with new config content → config updated
2. POST /exec with restart cmd → rc-agent restarted with new config

The deploy-staging area also needs a template TOML file with only `pod_number` variable — one template, populated per-pod at deploy time from James's machine.

### Anti-Patterns to Avoid

- **Don't add serde validation attributes:** `#[serde(deserialize_with = "...")]` for validation gives opaque error messages. Use a separate `validate()` step.
- **Don't panic on config error in rc-core:** rc-core's `Config::load_or_default()` already swallows errors and uses defaults. If rc-core gets stricter validation, use `warn!` not `error!` for non-critical fields, and only fail for database path (required to start).
- **Don't add `success` field to timeout response as `true`:** Current code returns `Ok(Json(...))` for timeouts — this is the bug. Timeout must be HTTP 500 + `success: false`.
- **Don't bind pod-agent to 0.0.0.0 when LAN binding is requested:** But also don't fail startup if IP detection fails — graceful fallback.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| URL validation | Custom parser | Simple `starts_with` prefix check | Full URL parsing (`url` crate) is overkill for a WebSocket URL sanity check |
| Config file write atomicity | Custom temp+rename | Direct `fs::write` is fine | Pods are single-user, no concurrent config writers, `pod-agent /write` already uses direct write |
| Backoff initialization | Complex dynamic discovery | Hardcoded range `1..=8` | Dynamic pod discovery happens via WebSocket registration, backoff init is just a safety net |
| HTTP status selection | Complex mapping logic | Direct `StatusCode` variants | axum already has `StatusCode::INTERNAL_SERVER_ERROR`, `StatusCode::BAD_REQUEST`, etc. |

**Key insight:** Every piece of infrastructure for this phase is already implemented and tested. The risk is in the integration points (startup order for lock screen, keying consistency between rc-agent and rc-core, backward compat in pod-agent response format).

---

## Common Pitfalls

### Pitfall 1: Lock screen not ready when config error occurs

**What goes wrong:** `LockScreenManager` is created after `load_config()` succeeds. If config fails, there's no lock screen to display the error.

**Why it happens:** Current startup sequence: tracing init → config load → main() proceeds. The lock screen is initialized much later in the agent loop.

**How to avoid:** Either (a) initialize the lock screen HTTP server before config validation and use it to show errors, or (b) show the error via a simple `msgbox` / console output and let the watchdog restart cycle handle display. The user wants "branded lock screen" — option (a) is required.

**Implementation approach:** Extract lock screen HTTP server start into a function callable before config load. Show `LockScreenState::ConfigError` (new variant). Then `std::process::exit(1)` (watchdog restarts, config error persists until staff fixes the file).

**Warning signs:** If you see the lock screen test pass but config error test shows blank screen, the server isn't starting early enough.

### Pitfall 2: pod_backoffs key mismatch between rc-agent and rc-core

**What goes wrong:** rc-agent generates `pod_id = format!("pod_{}", config.pod.number)` e.g. `"pod_3"`. rc-core pod_monitor generates the same key via `pod.id` from PodInfo. If pre-population uses a different format (e.g., `"pod-3"` or `"Pod 3"`), `entry().or_insert_with()` will still create a new entry but the pre-populated one is wasted — no bug, but a wasted initialization.

**Why it happens:** The key format is not enforced by a type system — it's just a String.

**How to avoid:** Verify that `format!("pod_{}", pod_num)` matches what rc-agent sends in its WebSocket registration message. From rc-agent/src/main.rs line 188: `let pod_id = format!("pod_{}", config.pod.number)` — confirmed, use underscore not hyphen.

### Pitfall 3: pod-agent response format breaking existing callers

**What goes wrong:** rc-core's pod_monitor.rs checks `resp.status().is_success()` (line 226). If exec returns HTTP 500 for non-zero exit, the existing code path correctly falls into the `Ok(resp)` (non-success) arm and logs a warning. BUT — existing code does NOT parse the response body JSON, so adding `success: bool` does not break any callers.

**Why it happens:** pod_monitor.rs deserializes nothing from the exec response — it only checks the HTTP status code. So the response body change is safe.

**How to avoid:** The change is backward safe for rc-core callers. Verify by checking ALL callers of pod-agent /exec before shipping.

**Callers of /exec:**
- `rc-core/src/pod_monitor.rs` — checks `resp.status().is_success()` only (safe)
- `rc-core/src/pod_healer.rs` — check if it also calls /exec (need to verify)
- `deploy-cmd.json` on James's machine — manual deploy tool, doesn't parse response structure

### Pitfall 4: rc-agent exits zero when config is invalid (wrong exit code)

**What goes wrong:** If load_config() returns Err(e) and main() uses `?`, Rust's default behavior is to print the Debug representation of the error and exit with code 1. This IS non-zero. BUT — if the error is swallowed somewhere (e.g., `if let Err(e) = load_config() { ... return Ok(()) }`), exit code will be 0 and watchdog won't know something went wrong.

**How to avoid:** Use `?` propagation from load_config() → main(). Do NOT catch the error in main and return Ok(()). The `Result<()>` return from `main()` converts Err to exit code 1 automatically.

### Pitfall 5: install.bat config write uses delayed expansion but pod-agent /write does not

**What goes wrong:** install.bat uses `>` redirect with `!POD!` for the pod number (delayed expansion). This works in .bat but is irrelevant for pod-agent /write endpoint which takes raw JSON content.

**How to avoid:** For remote deploy, generate the config content on James's machine (Python or bash), then POST it to pod-agent /write. No shell expansion needed on the pod side.

---

## Code Examples

Verified patterns from codebase inspection:

### Existing pod-agent /exec return pattern (CURRENT - to be fixed)

```rust
// pod-agent/src/main.rs - current exec_command (simplified)
match result {
    Ok(Ok(out)) => Ok(Json(ExecResponse {  // HTTP 200 always
        exit_code: out.status.code(),
        stdout: ...,
        stderr: ...,
    })),
    Ok(Err(e)) => Ok(Json(ExecResponse {   // BUG: spawn error → still 200
        exit_code: None,
        stderr: format!("Failed to execute: {}", e),
        ..
    })),
    Err(_) => Ok(Json(ExecResponse {       // BUG: timeout → still 200
        exit_code: Some(124),
        stderr: format!("Command timed out after {}ms", timeout_ms),
        ..
    })),
}
```

### Fixed pod-agent /exec return pattern

```rust
match result {
    Ok(Ok(out)) => {
        let success = out.status.success();
        let resp = Json(ExecResponse {
            success,
            exit_code: out.status.code(),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        });
        if success {
            Ok(resp)  // HTTP 200
        } else {
            Err((StatusCode::INTERNAL_SERVER_ERROR, resp))  // HTTP 500
        }
    }
    Ok(Err(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ExecResponse {
        success: false,
        exit_code: None,
        stdout: String::new(),
        stderr: format!("Failed to execute: {}", e),
    }))),
    Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ExecResponse {
        success: false,
        exit_code: Some(124),
        stdout: String::new(),
        stderr: format!("Command timed out after {}ms", timeout_ms),
    }))),
}
```

### AppState pod_backoffs pre-population

```rust
// rc-core/src/state.rs — in AppState::new()
let mut initial_backoffs = HashMap::new();
for pod_num in 1u32..=8 {
    initial_backoffs.insert(
        format!("pod_{}", pod_num),
        EscalatingBackoff::new(),
    );
}
// Then in the struct initializer:
pod_backoffs: RwLock::new(initial_backoffs),
```

### rc-agent config validation skeleton

```rust
// rc-agent/src/main.rs

struct ConfigError(String);

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn validate_config(config: &AgentConfig) -> Result<()> {
    let mut errors = Vec::new();

    if config.pod.number < 1 || config.pod.number > 8 {
        errors.push(format!("pod.number must be 1-8, got {}", config.pod.number));
    }
    if config.pod.name.trim().is_empty() {
        errors.push("pod.name must not be empty".to_string());
    }
    let url = config.core.url.trim();
    if !url.starts_with("ws://") && !url.starts_with("wss://") {
        errors.push(format!("core.url must be a WebSocket URL (ws:// or wss://), got: {}", url));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("Config validation failed:\n  - {}", errors.join("\n  - ")))
    }
}
```

### First-boot email test (flag file pattern)

```rust
// rc-core/src/main.rs — after AppState initialization

const FIRST_BOOT_FLAG: &str = "./data/email_verified.flag";

async fn maybe_send_first_boot_email(state: &Arc<AppState>) {
    if std::path::Path::new(FIRST_BOOT_FLAG).exists() {
        return;  // Already verified in a previous boot
    }
    let subject = "RaceControl Started — Email Alerts Active";
    let body = format!(
        "RaceControl rc-core started successfully.\n\
         Email alerting is configured and working.\n\
         Venue: {}\n\
         Time: {}",
        state.config.venue.name,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );
    // Use "system" as pod_id for the rate limit key (bypasses per-pod cooldown)
    state.email_alerter.write().await
        .send_alert("system", &subject, &body).await;
    // Write flag regardless of success — avoid spamming on email misconfiguration
    let _ = std::fs::write(FIRST_BOOT_FLAG, "");
}
```

---

## State of the Art

| Old Approach | Current Approach | Status |
|--------------|------------------|--------|
| Fixed restart cooldown (restart_cooldown_secs config) | EscalatingBackoff (30s→2m→10m→30m) already implemented | EscalatingBackoff in rc-common, used in pod_monitor — but not pre-populated in AppState |
| pod-agent /exec returns HTTP 200 for everything | Should return 500 for non-zero exit, 400 for bad request | Fix needed — current code has the right return type, just uses Ok() everywhere |
| rc-agent silently starts with default config | Should fail fast with non-zero exit on invalid/missing config | Fix needed — load_config() currently returns Ok(default) |
| Config file not explicitly deleted on remote deploy | install.bat deletes it, remote deploy path does not | Gap in remote deploy path (deploy-cmd.json pattern) |

---

## Open Questions

1. **Billing rates in rc-agent config**
   - What we know: CONTEXT.md mentions "billing rates (must be > 0)" as a critical field. rc-agent's `AgentConfig` has NO billing rate fields — billing is rc-core's domain.
   - What's unclear: Are billing rates supposed to be added to rc-agent.toml, or is this CONTEXT.md referring to rc-core validation? Or does it mean validating that the configured sim type is valid (not "0")?
   - Recommendation: Treat as rc-core validation only (not rc-agent). If billing rates were meant for rc-agent, they don't exist in the current struct — adding them would be new design, not Phase 1 hardening. Confirm with Uday if needed.

2. **Lock screen state before config load**
   - What we know: The current startup sequence initializes LockScreenManager inside the main agent loop, after config is loaded.
   - What's unclear: Can we safely initialize the lock screen HTTP server (port 18923) before config load without the config values? The server doesn't require config — it just serves HTML. The lock screen manager only needs the event channel.
   - Recommendation: Initialize `LockScreenManager` and start the HTTP server at the very top of main(), before `load_config()`. This enables branded error display on config failure.

3. **pod-agent LAN binding — fallback behavior**
   - What we know: The decision is to bind to 192.168.31.x only. The `local_ip()` function parses ipconfig output looking for a 192.168.31.x address.
   - What's unclear: If local_ip() fails (e.g., pod is not yet on the network), should pod-agent refuse to start or bind to 0.0.0.0 as fallback?
   - Recommendation: Bind to 0.0.0.0 as fallback if 192.168.31.x detection fails. Log a warning. This preserves availability during network hiccups.

4. **First-boot email "system" pod_id and rate limits**
   - What we know: EmailAlerter.should_send() checks per-pod cooldown (30min) and venue-wide cooldown (5min). Using "system" as pod_id for the startup test email won't conflict with real pod IDs.
   - What's unclear: Will the venue-wide 5-minute cooldown block a legitimate pod alert immediately after the startup email?
   - Recommendation: Use a dedicated `send_unconditional()` method that bypasses rate limiting for the first-boot test (or temporarily disable rate limiting just for this call). Alternatively, just call `send_alert()` and accept that it might be blocked if a pod alert fires within 5 minutes of startup — the test email is just a nice-to-have verification.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test runner (`cargo test`) |
| Config file | `.cargo/config.toml` (workspace-level, for CRT static linking) |
| Quick run command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |
| Full suite command | Same as quick — no separate integration test profile needed |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WD-02 | pod_backoffs pre-populated for pods 1-8 in AppState::new() | unit | `cargo test -p rc-core -- state::tests` | ❌ Wave 0 |
| WD-02 | pod_monitor reads existing backoff entry (no `entry().or_insert_with()` needed for known pods) | unit | `cargo test -p rc-core -- pod_monitor::tests` | ❌ Wave 0 |
| DEPLOY-01 | validate_config rejects pod.number = 0 | unit | `cargo test -p rc-agent -- validate_config` | ❌ Wave 0 |
| DEPLOY-01 | validate_config rejects pod.number = 9 | unit | `cargo test -p rc-agent -- validate_config` | ❌ Wave 0 |
| DEPLOY-01 | validate_config rejects empty pod.name | unit | `cargo test -p rc-agent -- validate_config` | ❌ Wave 0 |
| DEPLOY-01 | validate_config rejects invalid URL (no ws:// prefix) | unit | `cargo test -p rc-agent -- validate_config` | ❌ Wave 0 |
| DEPLOY-01 | validate_config accepts valid config | unit | `cargo test -p rc-agent -- validate_config` | ❌ Wave 0 |
| DEPLOY-01 | load_config() returns Err when no file found (not Ok(default)) | unit | `cargo test -p rc-agent -- load_config_no_file` | ❌ Wave 0 |
| DEPLOY-03 | /exec returns HTTP 200 + success:true for exit code 0 | unit (pod-agent) | `cargo test -p pod-agent` (separate workspace) | ❌ Wave 0 |
| DEPLOY-03 | /exec returns HTTP 500 + success:false for non-zero exit | unit (pod-agent) | `cargo test -p pod-agent` | ❌ Wave 0 |
| DEPLOY-03 | /exec returns HTTP 500 + success:false for timeout | unit (pod-agent) | `cargo test -p pod-agent` | ❌ Wave 0 |
| DEPLOY-04 | Remote deploy payload includes config delete step | manual smoke | Deploy to Pod 8, verify no stale racecontrol.toml | Manual |

### Sampling Rate

- **Per task commit:** `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Per wave merge:** Same as above + deploy to Pod 8 + verify pod reports correct pod_number via rc-core dashboard
- **Phase gate:** All unit tests green + Pod 8 smoke verify before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/rc-agent/src/main.rs` — add `#[cfg(test)] mod tests { ... }` block with `validate_config_*` unit tests
- [ ] `crates/rc-core/src/state.rs` — add `#[cfg(test)] mod tests { ... }` block for AppState::new() backoff pre-population
- [ ] `pod-agent/src/main.rs` (separate repo at `/c/Users/bono/racingpoint/pod-agent/`) — add unit tests for exec_command response codes
- [ ] No new framework installs needed — `cargo test` already works

*(pod-agent is a separate Cargo workspace at `/c/Users/bono/racingpoint/pod-agent/` — not part of the racecontrol monorepo. Tests there use `cargo test` from that directory.)*

---

## Sources

### Primary (HIGH confidence)

- Direct codebase inspection — all findings verified against actual source code
  - `crates/rc-agent/src/main.rs` — load_config(), validate_config gap, AgentConfig struct
  - `crates/rc-core/src/state.rs` — AppState::new(), pod_backoffs initialization
  - `crates/rc-core/src/pod_monitor.rs` — backoff usage, entry().or_insert_with() pattern
  - `crates/rc-core/src/email_alerts.rs` — EmailAlerter API, rate limit behavior
  - `crates/rc-common/src/watchdog.rs` — EscalatingBackoff API, tested
  - `pod-agent/src/main.rs` — /exec handler, current HTTP 200 for everything bug
  - `deploy-staging/install.bat` — existing config cleanup in pendrive deploy path

### Secondary (MEDIUM confidence)

- CONTEXT.md decisions — user's explicit implementation choices, taken as authoritative for this research

### Tertiary (LOW confidence)

- "billing rates in rc-agent" interpretation — this may refer to rc-core validation, not rc-agent. Flagged as open question.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all verified against actual Cargo.toml and source files
- Architecture: HIGH — all patterns derived from existing working code in the codebase
- Pitfalls: HIGH — discovered through direct code inspection, not speculation
- Open questions: Flagged honestly — 4 ambiguities identified

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable Rust ecosystem, project-specific — changes with code changes)
