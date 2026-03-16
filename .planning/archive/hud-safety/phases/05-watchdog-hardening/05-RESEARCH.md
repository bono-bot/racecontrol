# Phase 5: Watchdog Hardening - Research

**Researched:** 2026-03-12
**Domain:** Pod supervision, process lifecycle management, crash loop prevention, email alerting
**Confidence:** HIGH

## Summary

Phase 5 hardens the existing 3-tier pod supervision stack (watchdog.bat + pod-agent watchdog -> pod_monitor.rs -> pod_healer.rs) by replacing fixed restart cooldowns with escalating backoff (30s -> 2m -> 10m -> 30m), adding post-restart health verification (WebSocket + lock screen checks), and introducing email notifications for persistent pod failures that require Uday's manual intervention.

The codebase already has all the scaffolding needed: `PodRecoveryState` in `pod_monitor.rs` tracks `consecutive_failures`, `pod_healer.rs` has `HealCooldown` with fixed 600s cooldown, both use pod-agent's HTTP `/exec` endpoint for remote commands, and `udp_heartbeat.rs` provides fast liveness detection (6s dead timeout). The core work is (1) replacing constants with escalating state machines, (2) adding a post-restart verification loop after each restart command, and (3) wiring in email notifications via the existing `send_email.js` Node script that already authenticates with Gmail.

**Primary recommendation:** Implement escalating cooldowns as a shared `EscalatingBackoff` struct in `rc-common`, modify `pod_monitor.rs` and `pod_healer.rs` to use it, add a post-restart verification task that polls pod-agent `/exec` and WebSocket sender state, and send email alerts by shelling out to the existing `send_email.js` (not adding a full SMTP crate dependency).

## Standard Stack

### Core (already in project)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.x | Async runtime for timers, spawning verification tasks | Already used everywhere |
| chrono | 0.4 | Timestamps for cooldown tracking | Already in workspace deps |
| tracing | 0.1 | Structured logging for watchdog events | Already used |
| reqwest | 0.12 | HTTP client for pod-agent calls + Gmail API | Already in racecontrol deps |
| serde/serde_json | 1.x | Serialization for health check responses | Already in workspace |

### Supporting (new or modified)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::process | (in tokio) | Shell out to `send_email.js` for Gmail | Email notifications only |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `send_email.js` shell-out | `lettre` 0.11 crate (SMTP) | lettre adds ~3 crate deps, needs Gmail App Password (Google Workspace may not support it), and the project already has working Gmail auth via `send_email.js`. Shell-out is simpler and reuses existing credentials. |
| `send_email.js` shell-out | Gmail API via `reqwest` + OAuth2 | More Rust-native but requires OAuth2 token refresh logic (access token from refresh token), adds ~200 lines of boilerplate. `send_email.js` already handles this. |
| `send_email.js` shell-out | `lettre` with XOAUTH2 | lettre supports XOAUTH2 mechanism but you still need to refresh the access token from the refresh token, which requires a separate HTTP call to Google's token endpoint. Complexity not justified. |

**Installation:**
No new crate dependencies needed. All functionality uses existing deps + the `send_email.js` script already at project root.

## Architecture Patterns

### Recommended Project Structure
```
crates/rc-common/src/
  watchdog.rs            # NEW: EscalatingBackoff struct (shared between core and agent)

crates/racecontrol/src/
  pod_monitor.rs         # MODIFIED: Use EscalatingBackoff, add post-restart verification
  pod_healer.rs          # MODIFIED: Use EscalatingBackoff instead of fixed HEAL_COOLDOWN_SECS
  email_alerts.rs        # NEW: Email notification module (shell-out to send_email.js)
  config.rs              # MODIFIED: Add email alert config fields to WatchdogConfig
```

### Pattern 1: EscalatingBackoff State Machine
**What:** A struct that tracks restart attempts per pod and returns the appropriate cooldown duration based on the attempt count. Resets to initial delay on successful recovery.
**When to use:** Both `pod_monitor.rs` (Tier 2) and `pod_healer.rs` (Tier 3) for their respective cooldowns.
**Example:**
```rust
// In crates/rc-common/src/watchdog.rs
use std::time::Duration;
use chrono::{DateTime, Utc};

/// Escalating backoff: 30s -> 2m -> 10m -> 30m (capped)
pub struct EscalatingBackoff {
    pub attempt: u32,
    pub last_attempt_at: Option<DateTime<Utc>>,
    steps: Vec<Duration>,
}

impl EscalatingBackoff {
    pub fn new() -> Self {
        Self {
            attempt: 0,
            last_attempt_at: None,
            steps: vec![
                Duration::from_secs(30),
                Duration::from_secs(120),
                Duration::from_secs(600),
                Duration::from_secs(1800),
            ],
        }
    }

    /// Returns true if enough time has elapsed since last attempt
    pub fn ready(&self, now: DateTime<Utc>) -> bool {
        match self.last_attempt_at {
            None => true,
            Some(last) => {
                let cooldown = self.current_cooldown();
                let elapsed = (now - last).num_seconds();
                elapsed >= cooldown.as_secs() as i64
            }
        }
    }

    /// Current cooldown duration based on attempt count
    pub fn current_cooldown(&self) -> Duration {
        let idx = (self.attempt as usize).min(self.steps.len() - 1);
        self.steps[idx]
    }

    /// Record an attempt (escalate to next tier)
    pub fn record_attempt(&mut self, now: DateTime<Utc>) {
        self.last_attempt_at = Some(now);
        self.attempt = self.attempt.saturating_add(1);
    }

    /// Reset on successful recovery
    pub fn reset(&mut self) {
        self.attempt = 0;
        self.last_attempt_at = None;
    }

    /// True if we've exhausted all escalation steps (at max cooldown)
    pub fn exhausted(&self) -> bool {
        self.attempt as usize >= self.steps.len()
    }
}
```

### Pattern 2: Post-Restart Health Verification
**What:** After sending a restart command to pod-agent, spawn a verification task that polls for rc-agent health at intervals (5s, 10s, 20s) up to a timeout (60s). Checks: (a) pod-agent `/exec "tasklist | findstr rc-agent"`, (b) WebSocket sender exists in `state.agent_senders`, (c) lock screen port 18923 responsive.
**When to use:** After every restart command in `pod_monitor.rs`.
**Example:**
```rust
// After restart command succeeds in pod_monitor.rs
tokio::spawn(verify_restart(
    state.clone(),
    pod.id.clone(),
    pod.ip_address.clone(),
));

async fn verify_restart(
    state: Arc<AppState>,
    pod_id: String,
    pod_ip: String,
) {
    // Check at 5s, 15s, 30s, 60s after restart
    let check_delays = [5, 15, 30, 60];

    for delay in check_delays {
        tokio::time::sleep(Duration::from_secs(delay)).await;

        // 1. Process running?
        let process_alive = check_process_running(&state, &pod_ip, "rc-agent.exe").await;
        if !process_alive { continue; }

        // 2. WebSocket connected?
        let ws_connected = state.agent_senders.read().await.contains_key(&pod_id);

        // 3. Lock screen responsive?
        let lock_screen_ok = check_lock_screen(&state, &pod_ip).await;

        if ws_connected && lock_screen_ok {
            tracing::info!("Pod {} restart verified: healthy after {}s", pod_id, delay);
            log_pod_activity(&state, &pod_id, "race_engineer",
                "Restart Verified", &format!("Healthy after {}s", delay), "watchdog");
            return;
        }
    }

    // All checks failed
    tracing::error!("Pod {} restart verification FAILED after 60s", pod_id);
    log_pod_activity(&state, &pod_id, "race_engineer",
        "Restart Failed", "Not healthy after 60s — email alert", "watchdog");

    // Trigger email alert
    send_pod_alert(&state, &pod_id, "Restart verification failed after 60s").await;
}
```

### Pattern 3: Email Notification via Shell-Out
**What:** Shell out to the existing `send_email.js` Node script which already has Google OAuth2 credentials configured. Rate-limit to 1 email per pod per 30 minutes to avoid spam.
**When to use:** When a pod hits max escalation (30m cooldown) or when post-restart verification fails.
**Example:**
```rust
// In crates/racecontrol/src/email_alerts.rs
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use tokio::process::Command;

const EMAIL_COOLDOWN_SECS: i64 = 1800; // 30 min between emails per pod
const SEND_EMAIL_SCRIPT: &str = "send_email.js";

pub struct EmailAlerter {
    last_sent: HashMap<String, DateTime<Utc>>,
    recipient: String,
    enabled: bool,
    script_path: String,
}

impl EmailAlerter {
    pub fn new(recipient: String, script_path: String, enabled: bool) -> Self {
        Self {
            last_sent: HashMap::new(),
            recipient,
            enabled,
            script_path,
        }
    }

    pub async fn send_alert(&mut self, pod_id: &str, subject: &str, body: &str) {
        if !self.enabled { return; }

        let now = Utc::now();
        if let Some(last) = self.last_sent.get(pod_id) {
            if (now - *last).num_seconds() < EMAIL_COOLDOWN_SECS {
                tracing::debug!("Email cooldown active for pod {}", pod_id);
                return;
            }
        }

        let result = Command::new("node")
            .arg(&self.script_path)
            .arg(&self.recipient)
            .arg(subject)
            .arg(body)
            .kill_on_drop(true)
            .output()
            .await;

        match result {
            Ok(output) if output.status.success() => {
                tracing::info!("Email alert sent for pod {} to {}", pod_id, self.recipient);
                self.last_sent.insert(pod_id.to_string(), now);
            }
            Ok(output) => {
                tracing::warn!("Email send failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(e) => {
                tracing::warn!("Failed to run email script: {}", e);
            }
        }
    }
}
```

### Anti-Patterns to Avoid
- **Fixed cooldowns across all pods:** Each pod must have independent escalation state. Do NOT use a global cooldown.
- **Restart-without-verify:** Never declare recovery successful just because the restart command returned HTTP 200. The `start /b rc-agent.exe` command always succeeds even if rc-agent crashes on startup.
- **Email on every failure:** Rate-limit strictly. An 8-pod venue with network issues could send 50+ emails in minutes without rate limiting.
- **Blocking the main monitor loop for verification:** Always spawn verification as a separate task. The pod_monitor loop checks all 8 pods every 10s and must not block.
- **Restarting during active billing:** Both pod_monitor and pod_healer already check for active billing. Preserve this guard when adding escalating cooldowns.
- **Session 0 restart:** The existing code uses `start /b rc-agent.exe` from pod-agent (which runs as SYSTEM). This starts rc-agent in Session 0 where GUI (lock screen, overlay) cannot display. The HKLM Run key handles Session 1 startup at login. The pod_monitor restart is a best-effort -- it gets rc-agent running for WebSocket/heartbeat, but the lock screen may not render until next reboot. Post-restart verification should account for this.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Email sending | Custom SMTP client or OAuth2 token refresh | `send_email.js` (existing) | Already authenticated, tested, handles Google OAuth2 token refresh |
| Exponential backoff math | Manual Duration arithmetic | `EscalatingBackoff` struct with step table | Step table is clearer than exponential formula, easy to tune, serializable |
| Process health check | Custom TCP probe | pod-agent `/exec` + `tasklist` / PowerShell HTTP check | pod-agent already has remote exec, lock screen health check pattern exists in pod_healer.rs |
| WebSocket connection verification | Custom TCP probe | `state.agent_senders.read().await.contains_key()` | Already tracks connected agents in-memory |

**Key insight:** The codebase already has 90% of the primitives needed. The watchdog hardening is about connecting existing pieces with better state management, not building new infrastructure.

## Common Pitfalls

### Pitfall 1: Session 0 GUI Blindness
**What goes wrong:** rc-agent restarted by pod-agent (SYSTEM session) runs in Session 0, where Windows GUI APIs (lock screen, overlay) are invisible to the user.
**Why it happens:** pod-agent runs as SYSTEM. `start /b rc-agent.exe` from SYSTEM creates the process in Session 0.
**How to avoid:** Post-restart verification should treat "WebSocket connected but lock screen unresponsive" as a partial recovery, not a failure. Log it as a known limitation. The HKLM Run key will fix it on next reboot.
**Warning signs:** Lock screen health check fails but WebSocket is connected.

### Pitfall 2: Flapping Between Online/Offline
**What goes wrong:** Pod flaps between Online and Offline on every monitor cycle because the restart takes 5-10 seconds and the next check happens during that window.
**Why it happens:** Monitor check interval (10s) is close to restart time (~5s for kill + startup).
**How to avoid:** After sending a restart command, mark the pod as "recovering" and skip the next N check cycles (or use a per-pod "last_restart_attempt" timestamp to suppress checks for 30s). The current code already has `last_restart_attempt` -- the escalating backoff naturally handles this.
**Warning signs:** Activity log shows rapid Online/Offline/Online cycles.

### Pitfall 3: Email Storm on Network Outage
**What goes wrong:** All 8 pods go offline simultaneously (network switch failure, router reboot, power glitch). Each pod triggers an email alert, flooding Uday's inbox.
**Why it happens:** Per-pod email cooldown doesn't prevent 8 simultaneous emails.
**How to avoid:** Add venue-level rate limiting: max 1 email per 5 minutes across all pods. If multiple pods are down, aggregate into a single email listing all affected pods.
**Warning signs:** Multiple pods go offline within the same 30s window.

### Pitfall 4: Stale EscalatingBackoff State
**What goes wrong:** A pod recovers but the backoff state is never reset, so the next genuine failure starts at the 30-minute cooldown.
**Why it happens:** Recovery detection missed or reset logic not triggered.
**How to avoid:** In `check_all_pods()`, when a pod transitions from stale to healthy, call `backoff.reset()`. This path already exists in pod_monitor.rs (the `consecutive_failures = 0` reset block at line 91-101) -- just add `backoff.reset()` there.
**Warning signs:** Pod logs show "restart cooldown not elapsed" even for a fresh failure after a recovery.

### Pitfall 5: Node.js Not Available on Server
**What goes wrong:** The `send_email.js` script works on James's machine but fails on the Racing-Point-Server where racecontrol runs.
**Why it happens:** Node.js may not be installed on the server, or the `send_email.js` path / credential path differs.
**How to avoid:** (a) Verify Node.js is installed on the server. (b) Use an absolute path to the script and credentials. (c) Add a config option `[watchdog] email_script_path = "..."`. (d) Graceful fallback: if the script fails, log a warning and continue -- never let email failure block the watchdog.
**Warning signs:** "Failed to run email script" in logs.

### Pitfall 6: Concurrent Restart Attempts
**What goes wrong:** pod_monitor and pod_healer both try to restart the same pod simultaneously.
**Why it happens:** They run on independent timers (10s and 120s) and don't coordinate.
**How to avoid:** Share the `EscalatingBackoff` state between pod_monitor and pod_healer via `AppState`, or have pod_healer check `last_restart_attempt` from pod_monitor before acting. The simplest approach: pod_healer defers to pod_monitor for restarts and focuses on diagnostics/healing only.
**Warning signs:** Activity log shows "Agent Restarted" from both "race_engineer" (monitor) and "race_engineer" (healer) within seconds.

## Code Examples

### Current Supervision Tiers (for reference)

```
Tier 0: watchdog.bat (SYSTEM scheduled task, 2min interval)
  - Simple process-alive check for rc-agent.exe and pod-agent.exe
  - Just restarts if missing, no health verification
  - Runs on each pod independently

Tier 1: pod-agent watchdog (in pod-agent main.rs, 30s interval)
  - watchdog_ensure_running("rc-agent.exe")
  - Same logic as watchdog.bat but in Rust, runs in-process

Tier 1b: rc-agent watchdog (in rc-agent main.rs, 30s interval)
  - watchdog_ensure_running("pod-agent.exe")
  - Cross-watches pod-agent from rc-agent

Tier 2: pod_monitor.rs (in racecontrol, 10s check interval)
  - Detects heartbeat staleness (30s timeout)
  - Marks pods Offline, attempts restart via pod-agent HTTP
  - Fixed 120s cooldown, tracks consecutive_failures
  - Sends WoL if pod-agent unreachable

Tier 3: pod_healer.rs (in racecontrol, 120s interval)
  - Deep diagnostics: stale sockets, disk, memory, processes
  - Rule-based auto-fix: kill zombies, clear temp, restart rc-agent
  - Fixed 600s cooldown per pod
  - AI escalation for unhandled issues
```

### Existing Restart Command (pod_monitor.rs line 162)
```rust
let restart_cmd = r#"cd /d C:\RacingPoint & taskkill /F /IM rc-agent.exe >nul 2>&1 & timeout /t 2 /nobreak >nul & start /b rc-agent.exe"#;
```

### Existing Recovery State (pod_monitor.rs line 24-29)
```rust
struct PodRecoveryState {
    last_restart_attempt: Option<DateTime<Utc>>,
    last_wol_attempt: Option<DateTime<Utc>>,
    consecutive_failures: u32,
    pod_agent_reachable: bool,
}
```

### Existing Health Check Pattern (pod_healer.rs line 417-429)
```rust
async fn check_rc_agent_health(state: &Arc<AppState>, pod_ip: &str) -> anyhow::Result<bool> {
    let cmd = r#"powershell -NoProfile -Command "try { $r = Invoke-WebRequest -Uri 'http://127.0.0.1:18923/' -TimeoutSec 3 -UseBasicParsing; $r.StatusCode } catch { 0 }""#;
    match exec_on_pod(state, pod_ip, cmd).await {
        Ok(output) => {
            let code: u32 = output.trim().parse().unwrap_or(0);
            Ok(code == 200)
        }
        Err(_) => Ok(true), // if pod-agent exec fails, assume healthy (safe default)
    }
}
```

### send_email.js Interface (existing, at project root)
```bash
# Usage: node send_email.js <to> <subject> <body>
node send_email.js usingh@racingpoint.in "Pod 3 Alert" "Pod 3 has failed 4 restart attempts..."
```
Requires: `C:/Users/bono/.claude/james-google-credentials.json` with Google OAuth2 credentials.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed 120s restart cooldown (pod_monitor) | Will become escalating 30s->2m->10m->30m | Phase 5 | Prevents crash loops where a broken rc-agent restarts every 2 min forever |
| Fixed 600s heal cooldown (pod_healer) | Will use same escalating backoff | Phase 5 | Healer stops hammering a pod that needs manual fix |
| No post-restart verification | Will verify process + WebSocket + lock screen | Phase 5 | Catch cases where rc-agent starts but fails immediately |
| Dashboard-only alerts (AssistanceNeeded) | Will add email alerts for persistent failures | Phase 5 | Uday notified even when not watching dashboard |
| No coordination between tiers | Shared backoff state in AppState | Phase 5 | Prevents simultaneous restart from monitor + healer |

## Proposed Requirements

| ID | Description | Category |
|----|-------------|----------|
| WD-01 | Escalating restart cooldowns: 30s -> 2m -> 10m -> 30m per pod, resets on successful recovery | Crash Loop Prevention |
| WD-02 | Post-restart self-test: verify rc-agent process running, WebSocket reconnected, and lock screen responsive within 60s of restart | Health Verification |
| WD-03 | Email notification to Uday (usingh@racingpoint.in) when a pod hits max escalation or post-restart verification fails | Alerting |
| WD-04 | Email rate limiting: max 1 email per pod per 30 minutes, max 1 venue-wide email per 5 minutes (aggregate multiple pod failures) | Alert Spam Prevention |
| WD-05 | Shared backoff state between pod_monitor and pod_healer to prevent duplicate restart attempts | Coordination |
| WD-06 | Configurable alert settings in racecontrol.toml: email recipient, enable/disable, script path, cooldown durations | Configuration |

## Open Questions

1. **Node.js on the server**
   - What we know: `send_email.js` works on James's machine (.27). racecontrol runs on Racing-Point-Server (.23).
   - What's unclear: Is Node.js installed on .23? Are Google credentials available there?
   - Recommendation: Verify Node.js on server. If not available, two alternatives: (a) install Node.js on server, or (b) have racecontrol call a simple HTTP endpoint on James's machine that triggers the email. Option (a) is preferred.

2. **Healer restart vs monitor restart scope**
   - What we know: pod_healer.rs also restarts rc-agent (action: "restart_rc_agent"). pod_monitor.rs restarts rc-agent. Both act independently.
   - What's unclear: Should pod_healer stop doing restarts entirely and delegate to pod_monitor? Or should they share the same escalating backoff state?
   - Recommendation: Share the `EscalatingBackoff` state in `AppState`. pod_monitor owns restarts. pod_healer focuses on diagnostics and healing actions (kill zombies, clear temp). If healer detects rc-agent is unhealthy but monitor hasn't restarted yet, healer logs the issue and lets monitor handle restart.

3. **Session 0 problem scope**
   - What we know: rc-agent restarted from SYSTEM (pod-agent or watchdog.bat) runs in Session 0 without GUI.
   - What's unclear: Should post-restart verification consider Session 0 restarts as "partial success" or "failure"?
   - Recommendation: Treat as partial success. Log "Restart partial: process running and WebSocket connected but GUI in Session 0 -- will resolve on next reboot". Do NOT trigger email alerts for Session 0 partial recovery.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust, built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p rc-common -- watchdog` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WD-01 | Escalating cooldown steps and reset | unit | `cargo test -p rc-common -- watchdog::tests -x` | Wave 0 |
| WD-02 | Post-restart verification logic | unit | `cargo test -p racecontrol-crate -- pod_monitor::tests -x` | Wave 0 |
| WD-03 | Email alert trigger conditions | unit | `cargo test -p racecontrol-crate -- email_alerts::tests -x` | Wave 0 |
| WD-04 | Rate limiting (per-pod + venue-wide) | unit | `cargo test -p racecontrol-crate -- email_alerts::tests -x` | Wave 0 |
| WD-05 | Shared backoff prevents dual restart | unit | `cargo test -p racecontrol-crate -- pod_monitor::tests -x` | Wave 0 |
| WD-06 | Config parsing with defaults | unit | `cargo test -p racecontrol-crate -- config::tests -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common && cargo test -p racecontrol-crate`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-common/src/watchdog.rs` -- EscalatingBackoff struct + tests (covers WD-01)
- [ ] `crates/racecontrol/src/email_alerts.rs` -- EmailAlerter struct + tests (covers WD-03, WD-04)
- [ ] Test module in `pod_monitor.rs` -- verification logic tests (covers WD-02, WD-05)
- [ ] Test module in `config.rs` -- WatchdogConfig with new email fields (covers WD-06)

## Sources

### Primary (HIGH confidence)
- **Codebase inspection** -- `pod_monitor.rs`, `pod_healer.rs`, `udp_heartbeat.rs` (both crates), `watchdog.bat`, `pod-agent/src/main.rs`, `rc-agent/src/main.rs`, `config.rs`, `activity_log.rs`, `send_email.js`
- **Existing `send_email.js`** -- Working Gmail sender with OAuth2 credentials at project root
- **MEMORY.md** -- Session 0 issue documented, pod network map, deployment rules
- **DashboardEvent::AssistanceNeeded** -- Existing dashboard alert pattern (rc-common/src/protocol.rs)

### Secondary (MEDIUM confidence)
- [lettre docs](https://docs.rs/lettre/0.11.19/lettre/) -- Async SMTP transport API, verified via docs.rs
- [lettre crates.io](https://crates.io/crates/lettre) -- Version 0.11.19, confirmed current

### Tertiary (LOW confidence)
- Gmail App Password availability for Google Workspace accounts -- unverified whether racingpoint.in Google Workspace supports App Passwords (reason for recommending `send_email.js` shell-out instead)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- All libraries already in project, no new deps needed
- Architecture: HIGH -- Patterns derived directly from existing codebase analysis
- Pitfalls: HIGH -- Identified from actual codebase behaviors (Session 0, flapping, healer/monitor overlap)
- Email approach: MEDIUM -- Shell-out to Node.js works but may need server-side verification

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable domain, no rapidly changing dependencies)
