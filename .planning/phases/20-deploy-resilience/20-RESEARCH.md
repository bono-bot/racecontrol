# Phase 20: Deploy Resilience - Research

**Researched:** 2026-03-15
**Domain:** Rust/Windows binary self-swap, rollback mechanics, AV exclusion, fleet deploy reporting
**Confidence:** HIGH

---

## Summary

Phase 20 hardens the existing deploy pipeline in `racecontrol/src/deploy.rs`. The current self-swap
pattern (download `rc-agent-new.exe` alongside the live process, then run `do-swap.bat` detached)
works correctly for the happy path but has four unaddressed failure modes: no previous binary is
preserved for rollback, health-check failure at the 60s gate stops at `DeployState::Failed` without
reverting, the staging filename `rc-agent-new.exe` is not covered by Defender exclusions, and the
rolling deploy produces no structured summary of which pods succeeded and which failed.

All four requirements are purely additive to code that already exists and compiles. No new crates
are needed. The changes touch `do-swap.bat` generation (one Rust string constant), `deploy_pod()`
(add rollback branch after the verify loop), `self_heal.rs` (add Defender exclusion check), and
`deploy_rolling()` (collect results and emit a summary log line). The `DeployState` enum needs one
new variant (`RollingBack`) and one clarification to `deploy_step_label`.

The rollback trigger location is racecontrol (not the watchdog). The watchdog restarts any process that
disappears — it has no knowledge of whether the binary is good or bad. Rollback must be orchestrated
by the entity that initiated the deploy: `deploy_pod()` in racecontrol. The watchdog's role remains
unchanged: if rc-agent disappears during rollback it will restart it from whatever `rc-agent.exe`
is present at that moment.

**Primary recommendation:** Implement rollback entirely inside `deploy_pod()` by triggering a
second detached batch script (`do-rollback.bat`) that moves `rc-agent-prev.exe` back to
`rc-agent.exe` and starts it. Add `DeployState::RollingBack` to track this in the dashboard.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DEP-01 | Self-swap preserves previous binary as rc-agent-prev.exe for rollback | `do-swap.bat` generation in `deploy_pod()` must add `move rc-agent.exe rc-agent-prev.exe` before moving new binary into place |
| DEP-02 | deploy.rs verifies pod health and triggers rollback on failure | Health-gate loop already exists (lines 484-518); add rollback branch after `VERIFY_DELAYS` exhausted; requires `DeployState::RollingBack` variant |
| DEP-03 | Defender exclusion covers rc-agent-new.exe staging filename | `self_heal.rs::run()` already checks config/script/registry; add 4th check for Defender `C:\RacingPoint\` directory exclusion |
| DEP-04 | Fleet deploy reports per-pod success/failure summary with retry for failed pods | `deploy_rolling()` already collects per-pod deploy calls sequentially; capture outcome, log structured summary, retry failed pods once |
</phase_requirements>

---

## Standard Stack

### Core (already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.x | Async runtime for deploy_pod tasks | Already used throughout racecontrol |
| serde / serde_json | 1.x | DeployState serialization to dashboard | Already used — DeployState is serde-tagged |
| tracing | 0.1 | Structured logging for fleet summary | Already used — all deploy steps log via `tracing::info!` |
| chrono | 0.4 | Timestamps in failure alerts | Already used in `send_deploy_failure_alert` |

### Supporting (already present)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| winreg (indirect via std::process::Command) | n/a | Registry check for Defender exclusions | self_heal.rs uses Command::new("reg") already; use PowerShell via Command for Defender check |
| reqwest | 0.12 | HTTP health check (lock screen) | Already used in `is_lock_screen_healthy` |

**Installation:** No new dependencies required.

---

## Architecture Patterns

### Current self-swap flow (deploy_pod)

```
download rc-agent-new.exe
size check
write config
generate + run do-swap.bat (detached):
    wait 3s
    taskkill /F /IM rc-agent.exe
    wait 2s
    del /Q rc-agent.exe
    move rc-agent-new.exe rc-agent.exe
    start rc-agent.exe
verify health at 5s, 15s, 30s, 60s
on failure → DeployState::Failed (no rollback today)
```

### Pattern 1: Preserve-then-swap (DEP-01)

**What:** Add one `move` command to `do-swap.bat` between killing the old process and starting the
new one. Before deleting `rc-agent.exe`, move it to `rc-agent-prev.exe`. This is atomic at the
Windows filesystem level — a `move` within the same directory is a rename (no copy).

**When to use:** Every deploy. The previous binary must exist before the new binary runs.

**do-swap.bat string in deploy_pod() — change from:**
```bat
@echo off
timeout /t 3 /nobreak
taskkill /F /IM rc-agent.exe
timeout /t 2 /nobreak
del /Q rc-agent.exe
move rc-agent-new.exe rc-agent.exe
start "" /D C:\RacingPoint rc-agent.exe
```

**to (adds DEP-01 preservation + DEP-02 retry loop for AV race):**
```bat
@echo off
timeout /t 3 /nobreak >nul
taskkill /F /IM rc-agent.exe >nul 2>&1
timeout /t 2 /nobreak >nul
if exist rc-agent-prev.exe del /Q rc-agent-prev.exe >nul 2>&1
if exist rc-agent.exe move /Y rc-agent.exe rc-agent-prev.exe >nul 2>&1
set RETRIES=0
:RETRY
move /Y rc-agent-new.exe rc-agent.exe >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    timeout /t 2 /nobreak >nul
    set /a RETRIES+=1
    if %RETRIES% LSS 5 goto RETRY
    echo SWAP FAILED >> C:\RacingPoint\deploy-error.log
    exit /b 1
)
start "" /D C:\RacingPoint rc-agent.exe
```

Note: The swap_cmd string in `deploy_pod()` at line 477 generates `do-swap.bat` using a long one-liner `echo` pipeline. The cleanest implementation replaces this with a multi-line constant (like `START_SCRIPT_CONTENT` in self_heal.rs) with explicit CRLF line endings.

### Pattern 2: racecontrol-triggered rollback (DEP-02)

**What:** After the VERIFY_DELAYS loop exhausts without health, generate and run a `do-rollback.bat`
via `exec_on_pod`. This script moves `rc-agent-prev.exe` back to `rc-agent.exe` and starts it.

**When to use:** Only when `rc-agent-prev.exe` is confirmed to exist AND health verification failed.

**Design decision** (confirmed in STATE.md): Rollback is automatic, not manual. racecontrol is the
trigger. The watchdog is not involved in rollback decisions.

```rust
// In deploy_pod(), after verify loop exhaustion:
let prev_exists = check_prev_binary_exists(&state, &pod_id, &pod_ip).await;

if prev_exists {
    set_deploy_state(&state, &pod_id, DeployState::RollingBack).await;
    trigger_rollback(&state, &pod_id, &pod_ip).await;
    // wait for rollback health (same VERIFY_DELAYS pattern, shorter — 5s/15s/30s)
    // if rollback health OK → DeployState::RolledBack (or Idle after 10s)
    // if rollback health fails → DeployState::Failed { reason: "rollback also failed" }
}
```

**do-rollback.bat content:**
```bat
@echo off
taskkill /F /IM rc-agent.exe >nul 2>&1
timeout /t 2 /nobreak >nul
if exist rc-agent.exe del /Q rc-agent.exe >nul 2>&1
move /Y rc-agent-prev.exe rc-agent.exe
start "" /D C:\RacingPoint rc-agent.exe
```

**Watchdog interaction:** The watchdog polls via `tasklist` every 5 seconds. During the ~5-7 second
gap while `do-rollback.bat` kills the bad binary and starts the old one, `tasklist` may return no
`rc-agent.exe`. The watchdog has a 15-second restart grace window after any restart — if it
triggered a restart recently it will skip polling. If the watchdog fires independently during
rollback, it starts `rc-agent.exe` — whichever binary is present at that moment. This is safe: if
rollback completed, it starts the good prev binary. If rollback hasn't completed (bat still running)
the watchdog will find no `rc-agent.exe` to start (it was moved to prev), sleep 5s, and retry.
**Conclusion:** No coordination needed between deploy.rs rollback and watchdog. They converge to
the same correct state.

### Pattern 3: DeployState::RollingBack variant (DEP-02)

**What:** Add one variant to `DeployState` in rc-common/src/types.rs.

```rust
/// Rolling back to rc-agent-prev.exe after health verification failure
RollingBack,
```

Also add `deploy_step_label` arm:
```rust
DeployState::RollingBack => "Rolling back to previous binary".to_string(),
```

The `is_active()` impl should include `RollingBack`:
```rust
// RollingBack is active — deploy is still in progress (recovery phase)
// Remove RollingBack from the !matches!() list
```

Serde tag format is already `#[serde(tag = "state", content = "detail")]` with `rename_all =
"snake_case"` — the new variant serializes as `{"state": "rolling_back"}` automatically.

All existing tests for DeployState serialization remain unchanged.

### Pattern 4: Defender exclusion check in self_heal.rs (DEP-03)

**What:** The existing `run()` function in self_heal.rs checks 3 things. Add a 4th check:
verify the `C:\RacingPoint\` directory-wide exclusion is present in Windows Defender.

**Decision** (from STATE.md): Directory-wide `C:\RacingPoint\` exclusion — not per-file. This
covers `rc-agent.exe`, `rc-agent-new.exe`, `rc-agent-prev.exe`, and `do-swap.bat` in one rule.

**Implementation:**
```rust
/// Check if C:\RacingPoint\ is in Defender ExclusionPath.
fn defender_exclusion_exists() -> bool {
    // Use powershell to query; reg query on Defender exclusions requires SYSTEM or admin
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile", "-NonInteractive", "-Command",
        r#"(Get-MpPreference).ExclusionPath -contains 'C:\RacingPoint'"#
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.trim() == "True"
        }
        Err(_) => false,
    }
}

/// Add C:\RacingPoint\ to Defender exclusions.
fn repair_defender_exclusion() -> Result<()> {
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile", "-NonInteractive", "-Command",
        r#"Add-MpPreference -ExclusionPath 'C:\RacingPoint'"#
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| anyhow::anyhow!("powershell failed: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Add-MpPreference failed: {}", stderr.trim());
    }
    Ok(())
}
```

Add `defender_repaired: bool` field to `SelfHealResult`. Add check 4 in `run()` analogous to
the existing 3 checks. This runs at every rc-agent startup — if Defender reset its config after
a Windows Update or policy push, the exclusion is re-applied before any deploy can happen.

**Important:** `Add-MpPreference` requires admin/SYSTEM privileges. rc-agent runs in Session 1
as the logged-in user, which on pod PCs is an admin account (confirmed by install.bat usage).
If the call fails, log a warning and continue — non-fatal, same as registry repair.

### Pattern 5: Fleet deploy summary (DEP-04)

**What:** `deploy_rolling()` already runs pods sequentially. It currently logs per-pod success/
failure but has no aggregated summary at the end. Add result collection and a terminal summary log.

**Implementation in deploy_rolling():**

```rust
// After the sequential loop, before returning Ok(()):
let final_states = deploy_status(&state).await;

let mut succeeded = Vec::new();
let mut failed = Vec::new();
let mut waiting = Vec::new();

for (pod_id, state) in &final_states {
    match state {
        DeployState::Complete | DeployState::Idle => succeeded.push(pod_id.clone()),
        DeployState::Failed { .. } => failed.push(pod_id.clone()),
        DeployState::WaitingSession => waiting.push(pod_id.clone()),
        _ => {} // still in progress (shouldn't happen — all awaited)
    }
}

// Retry failed pods once (DEP-04)
if !failed.is_empty() {
    tracing::warn!("Rolling deploy: retrying {} failed pods: {:?}", failed.len(), failed);
    for pod_id in &failed {
        let pod_ip = { /* resolve from state.pods */ };
        if let Some(ip) = pod_ip {
            deploy_pod(state.clone(), pod_id.clone(), ip, binary_url.clone()).await;
        }
    }
    // Recheck after retry
    let retry_states = deploy_status(&state).await;
    // update succeeded/failed lists from retry_states
}

tracing::info!(
    "Rolling deploy COMPLETE: success={:?} failed={:?} queued_for_session_end={:?}",
    succeeded, failed, waiting
);

// Broadcast fleet summary event to dashboard
let _ = state.dashboard_tx.send(DashboardEvent::FleetDeploySummary {
    succeeded,
    failed,
    waiting,
    timestamp: Utc::now().to_rfc3339(),
});
```

**DashboardEvent addition** (rc-common/src/protocol.rs):
```rust
/// Fleet-wide rolling deploy completed — summary of per-pod outcomes
FleetDeploySummary {
    succeeded: Vec<String>,  // pod_ids that reached Complete/Idle
    failed: Vec<String>,     // pod_ids still in Failed state after retry
    waiting: Vec<String>,    // pod_ids queued (active billing session)
    timestamp: String,
},
```

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Windows atomic rename | Custom copy-verify-delete sequence | `move /Y` within same directory | Same-partition rename is atomic at NTFS level; copy-verify-delete is not and leaves orphan files |
| Defender exclusion check | winreg crate direct registry reads | `powershell -c "(Get-MpPreference).ExclusionPath"` | Defender exclusion registry path (`HKLM\SOFTWARE\Microsoft\Windows Defender\Exclusions\Paths`) requires TrustedInstaller/SYSTEM for reads on some Windows builds; PowerShell cmdlet works as admin user |
| Process kill detection | Polling `tasklist` in a Rust loop | The 2-second `timeout` in bat + existing `is_process_alive()` check | rc-agent cannot kill itself from Rust (it IS rc-agent); bat script is the correct tool for the kill-rename-start sequence |
| Retry with backoff for AV hold | Custom Rust async retry | Retry loop in bat script | The AV hold happens during the `move` in the bat script — Rust code has already exited at that point; only the bat can retry |

---

## Common Pitfalls

### Pitfall 1: do-swap.bat CRLF line endings

**What goes wrong:** The `swap_cmd` at line 477 of deploy.rs generates do-swap.bat using a chained
`echo` pipeline via the Windows `cmd /c` one-liner. This is fragile: the resulting bat file will
have CRLF because cmd.exe writes CRLF natively. However, if this generation is ever moved to a
Rust string constant (as recommended above), the string must use `\r\n` explicitly. LF-only bat
files on Windows silently parse as a single line.

**How to avoid:** Use the same pattern as `START_SCRIPT_CONTENT` in self_heal.rs — define a Rust
`const` with literal `\r\n` and write it via the `/write` endpoint on pod-agent, not via an `echo`
pipeline in a shell command.

**Warning signs:** Pod gets killed (process disappears) but new binary never starts. Port 8090
unreachable. `dir C:\RacingPoint\` shows `rc-agent-new.exe` present and correct size.

### Pitfall 2: Watchdog restarts during rollback window

**What goes wrong:** Between `taskkill rc-agent.exe` and `start rc-agent.exe` in do-rollback.bat
(about 3-5 seconds), `tasklist` shows no rc-agent. If the watchdog happens to poll in that window
and does NOT have an active restart grace, it fires another restart — potentially starting
`rc-agent.exe` (the bad binary that was being rolled back) before `do-rollback.bat` finishes
the `move rc-agent-prev.exe rc-agent.exe` rename.

**How to avoid:** do-rollback.bat must kill, rename, then start — in that strict order, with no
pause between kill and rename. The window where both `rc-agent.exe` is gone AND `rc-agent-prev.exe`
is still present is eliminated by making the rename immediate after kill.

The watchdog grace window (15 seconds after last restart) also helps: if the watchdog recently
restarted rc-agent (e.g., when the bad binary crashed immediately), the grace window covers the
rollback period.

### Pitfall 3: rc-agent-prev.exe absent on first deploy

**What goes wrong:** On a pod that has never received a rolling deploy, `rc-agent-prev.exe` does
not exist. The rollback path must guard against this: check prev exists before attempting rollback,
otherwise `do-rollback.bat` will fail silently.

**How to avoid:** In `deploy_pod()`, before triggering rollback, check via exec:
```
if not exist C:\RacingPoint\rc-agent-prev.exe exit /b 1
```
or use `exec_on_pod` with `dir C:\RacingPoint\rc-agent-prev.exe` and parse the result. If prev
does not exist, log "no previous binary available for rollback" and set `DeployState::Failed`.

### Pitfall 4: is_active() excludes RollingBack from active set

**What goes wrong:** If `DeployState::RollingBack` is added to `DeployState` but the `is_active()`
implementation is not updated to include it, the dashboard and deploy guards will treat a pod
mid-rollback as "idle" — which could allow a second deploy to start on the same pod concurrently.

**How to avoid:** `RollingBack` must NOT be in the `!matches!()` exclusion list in `is_active()`.
The variant should return `true` from `is_active()` (deploy still in progress). Update the test
in `types.rs` that asserts which states are active.

### Pitfall 5: deploy_rolling() calls deploy_pod() await — retry doubles elapsed time

**What goes wrong:** `deploy_rolling()` calls `deploy_pod(pod_id, ...)` with `.await`. Each
deploy can take up to 110 seconds (download + 5+15+30+60 verify delays). A fleet deploy of 8 pods
sequential takes up to 15 minutes. Adding a retry for failed pods could add another 110s per
failed pod.

**How to avoid:** Retry is acceptable at this scale (8 pods). Document the worst-case time
in logs. The retry should happen after all initial deploys complete, not interleaved. The
summary log should include total elapsed time.

### Pitfall 6: Defender exclusion check permissions

**What goes wrong:** `Get-MpPreference` may return an empty `ExclusionPath` even when exclusions
exist, if the process calling it lacks admin rights. rc-agent runs as the logged-in Session 1 user
(who is an admin on pod PCs via `install.bat`). However, the check should be non-fatal: if
PowerShell fails or returns unexpected output, log a warning and continue — don't block startup.

**How to avoid:** The existing self_heal.rs pattern is correct: each repair is wrapped in
`match ... { Ok → log repaired, Err → log error, push to errors vec, continue }`. Follow
the same pattern for the Defender check.

---

## Code Examples

### do-swap.bat constant (replaces inline echo pipeline)

```rust
// Source: modeled on self_heal.rs START_SCRIPT_CONTENT pattern
// Place in deploy.rs as a module-level const
const SWAP_SCRIPT_CONTENT: &str = "@echo off\r\n\
    cd /d C:\\RacingPoint\r\n\
    timeout /t 3 /nobreak >nul\r\n\
    taskkill /F /IM rc-agent.exe >nul 2>&1\r\n\
    timeout /t 2 /nobreak >nul\r\n\
    if exist rc-agent-prev.exe del /Q rc-agent-prev.exe >nul 2>&1\r\n\
    if exist rc-agent.exe move /Y rc-agent.exe rc-agent-prev.exe >nul 2>&1\r\n\
    set RETRIES=0\r\n\
    :RETRY\r\n\
    move /Y rc-agent-new.exe rc-agent.exe >nul 2>&1\r\n\
    if %ERRORLEVEL% NEQ 0 (\r\n\
        timeout /t 2 /nobreak >nul\r\n\
        set /a RETRIES+=1\r\n\
        if %RETRIES% LSS 5 goto RETRY\r\n\
        echo SWAP FAILED > C:\\RacingPoint\\deploy-error.log\r\n\
        exit /b 1\r\n\
    )\r\n\
    start \"\" /D C:\\RacingPoint rc-agent.exe\r\n";
```

### do-rollback.bat constant

```rust
const ROLLBACK_SCRIPT_CONTENT: &str = "@echo off\r\n\
    cd /d C:\\RacingPoint\r\n\
    taskkill /F /IM rc-agent.exe >nul 2>&1\r\n\
    timeout /t 2 /nobreak >nul\r\n\
    if exist rc-agent.exe del /Q rc-agent.exe >nul 2>&1\r\n\
    move /Y rc-agent-prev.exe rc-agent.exe\r\n\
    start \"\" /D C:\\RacingPoint rc-agent.exe\r\n";
```

### Writing bat via /write endpoint (replacing exec_on_pod echo pipeline)

```rust
// Source: existing /write usage in deploy_pod() for config writing (lines 440-470)
// Write bat content via pod-agent HTTP /write endpoint
let write_url = format!("http://{}:{}/write", pod_ip, POD_AGENT_PORT);
let _ = state.http_client
    .post(&write_url)
    .json(&serde_json::json!({
        "path": "C:\\RacingPoint\\do-swap.bat",
        "content": SWAP_SCRIPT_CONTENT
    }))
    .timeout(Duration::from_secs(10))
    .send()
    .await;

// Then run it detached
let _ = exec_on_pod(
    &state, &pod_id, &pod_ip,
    r#"start /min cmd /c C:\RacingPoint\do-swap.bat"#,
    5000
).await;
```

### DeployState::RollingBack in types.rs

```rust
// Source: existing DeployState enum, rc-common/src/types.rs line 674
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", content = "detail")]
#[serde(rename_all = "snake_case")]
pub enum DeployState {
    Idle,
    Killing,
    WaitingDead,
    Downloading { progress_pct: u8 },
    SizeCheck,
    Starting,
    VerifyingHealth,
    Complete,
    Failed { reason: String },
    WaitingSession,
    // NEW:
    RollingBack,  // serializes as {"state": "rolling_back"}
}

impl DeployState {
    pub fn is_active(&self) -> bool {
        !matches!(
            self,
            DeployState::Idle
                | DeployState::Complete
                | DeployState::Failed { .. }
                | DeployState::WaitingSession
            // NOTE: RollingBack is intentionally NOT here — it IS active
        )
    }
}
```

### SelfHealResult with defender_repaired field

```rust
// Source: self_heal.rs SelfHealResult struct (line 37)
#[derive(Debug)]
pub struct SelfHealResult {
    pub config_repaired: bool,
    pub script_repaired: bool,
    pub registry_repaired: bool,
    pub defender_repaired: bool,  // NEW
    pub errors: Vec<String>,
}
```

### Fleet summary tracing pattern

```rust
// After all pods complete (synchronous sequential loop in deploy_rolling):
let summary: Vec<(String, &str)> = all_pod_ids.iter().map(|id| {
    let outcome = match final_states.get(id) {
        Some(DeployState::Complete) | Some(DeployState::Idle) => "ok",
        Some(DeployState::Failed { .. }) => "failed",
        Some(DeployState::WaitingSession) => "queued",
        _ => "unknown",
    };
    (id.clone(), outcome)
}).collect();

tracing::info!(
    "Rolling deploy summary: {}",
    summary.iter()
        .map(|(id, outcome)| format!("{}={}", id, outcome))
        .collect::<Vec<_>>()
        .join(", ")
);
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| echo pipeline one-liner for bat generation | Named const with CRLF (START_SCRIPT_CONTENT) | Phase 18 (self_heal.rs) | Prevents silent CRLF bugs; const is testable |
| Per-file Defender exclusion | Directory-wide C:\RacingPoint\ exclusion | Decision in STATE.md | Covers all staging filenames without enumeration |
| HTTP-only exec path for deploy | HTTP-first, WS-fallback via exec_on_pod() | Phase 17 | Deploys survive HTTP blocking (8090 firewalled) |

**Deprecated/outdated:**
- `do-swap.bat` via inline echo pipeline: works today but fragile; replace with `/write` + const
- Per-pod manual Defender exclusion: was manually applied during install; self-heal check ensures idempotency

---

## Open Questions

1. **Rollback health verify delays**
   - What we know: deploy health uses `&[5, 15, 30, 60]` (110s total) — same delays for rollback would be excessive since the prev binary was known-good
   - What's unclear: should rollback use shorter delays? `&[5, 15, 30]` seems sufficient (50s total)
   - Recommendation: Define a separate `ROLLBACK_VERIFY_DELAYS: &[u64] = &[5, 15, 30]` constant

2. **RolledBack vs Idle terminal state after successful rollback**
   - What we know: after successful deploy, state goes `Complete` then `Idle` after 10s
   - What's unclear: should rollback also show a terminal `RolledBack` state briefly, or just `Failed`?
   - Recommendation: Add `RolledBack` variant that displays for 10s then resets to `Idle`. Dashboard can show it in amber. This keeps the audit trail visible.

3. **FleetDeploySummary broadcast vs log-only**
   - What we know: DEP-04 says "racecontrol logs a per-pod summary" — logging is sufficient for the requirement
   - What's unclear: should `FleetDeploySummary` also be added as a `DashboardEvent` variant?
   - Recommendation: Add the `DashboardEvent::FleetDeploySummary` variant anyway — low cost, Uday can see it on the kiosk dashboard without needing SSH. The planner should scope this as optional (log is the requirement, event is a bonus).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p rc-common && cargo test -p racecontrol-crate` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate && cargo test -p rc-watchdog` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEP-01 | SWAP_SCRIPT_CONTENT const contains `rc-agent-prev.exe` move command | unit | `cargo test -p racecontrol-crate deploy::tests::swap_script_preserves_prev` | Wave 0 |
| DEP-01 | SWAP_SCRIPT_CONTENT has CRLF line endings | unit | `cargo test -p racecontrol-crate deploy::tests::swap_script_crlf` | Wave 0 |
| DEP-02 | `DeployState::RollingBack` serializes as `rolling_back` | unit | `cargo test -p rc-common types::tests::deploy_state_rolling_back_serde` | Wave 0 |
| DEP-02 | `RollingBack` returns true from `is_active()` | unit | `cargo test -p rc-common types::tests::rolling_back_is_active` | Wave 0 |
| DEP-02 | `deploy_step_label` returns correct string for `RollingBack` | unit | `cargo test -p racecontrol-crate deploy::tests::deploy_step_label_rolling_back` | Wave 0 |
| DEP-03 | `SelfHealResult` has `defender_repaired` field | unit | compile check (no runtime test needed) | ✅ (add field) |
| DEP-03 | Self-heal runs defender check (integration) | manual | Check log output on Pod 8 after deploy | N/A |
| DEP-04 | ROLLBACK_SCRIPT_CONTENT contains `rc-agent-prev.exe` | unit | `cargo test -p racecontrol-crate deploy::tests::rollback_script_contains_prev` | Wave 0 |
| DEP-04 | ROLLBACK_SCRIPT_CONTENT has CRLF | unit | `cargo test -p racecontrol-crate deploy::tests::rollback_script_crlf` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-common && cargo test -p racecontrol-crate`
- **Per wave merge:** full suite (`cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate && cargo test -p rc-watchdog`)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

New test functions needed in existing test modules (no new files):

- [ ] `crates/racecontrol/src/deploy.rs` tests — `swap_script_preserves_prev`, `swap_script_crlf`, `rollback_script_contains_prev`, `rollback_script_crlf`, `deploy_step_label_rolling_back`
- [ ] `crates/rc-common/src/types.rs` tests — `deploy_state_rolling_back_serde`, `rolling_back_is_active`

Both test files already exist with existing passing tests. New tests are additive — no existing tests change.

---

## Sources

### Primary (HIGH confidence)

- Direct codebase read: `crates/racecontrol/src/deploy.rs` — complete deploy flow, 848 lines
- Direct codebase read: `crates/rc-common/src/types.rs` — `DeployState` enum at line 674
- Direct codebase read: `crates/rc-agent/src/self_heal.rs` — existing 4-check repair pattern
- Direct codebase read: `crates/rc-watchdog/src/service.rs` — 15s grace window, tasklist polling
- Direct codebase read: `crates/racecontrol/src/state.rs` — `pod_deploy_states`, `pending_deploys` fields

### Secondary (MEDIUM confidence)

- `.planning/STATE.md` decision log — locked decisions for rollback and AV exclusion approach
- `.planning/research/PITFALLS.md` lines 120-153 — AV hold pitfall documented from live outage
- `.planning/research/SUMMARY.md` lines 79-82 — confirmed pitfall and mitigation pattern
- `.planning/phases/20-deploy-resilience/20-VALIDATION.md` — existing test infrastructure audit

### Tertiary (LOW confidence)

- Windows `move` atomic rename behavior within same NTFS volume — standard Windows behavior,
  verified by practice; no official Microsoft docs citation

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies, all patterns from existing code
- Architecture: HIGH — rollback trigger location (racecontrol not watchdog), bat generation pattern, and AV exclusion approach all locked in STATE.md decisions
- Pitfalls: HIGH — CRLF bat, AV hold, and watchdog race all documented from the Mar 15 live outage in PITFALLS.md; rollback-on-first-deploy is logical from code inspection
- DEP-03 Defender check: MEDIUM — `Get-MpPreference` behavior as admin user confirmed by general Windows knowledge; not tested on actual pod environment yet

**Research date:** 2026-03-15
**Valid until:** 2026-04-15 (stable Rust/Windows domain — 30 days)
