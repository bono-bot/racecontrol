# Pitfalls Research

**Domain:** Multi-layer autonomous healing — adding 3-layer survival system to existing Windows fleet management (v31.0)
**Researched:** 2026-03-30
**Confidence:** HIGH — all pitfalls drawn from documented incidents in this exact codebase (CLAUDE.md standing rules, MEMORY.md, git history, existing code in `crates/rc-watchdog/`, `crates/rc-agent/`, `crates/racecontrol/`). No hypothetical pitfalls.

---

## Critical Pitfalls

### Pitfall 1: Recovery System Fight — Five Independent Healers on the Same Patient

**What goes wrong:**
v31.0 adds Layer 1 (Smart Watchdog MMA), Layer 2 (Fleet Healer SSH), and Layer 3 (External Guardian) on top of the EXISTING recovery systems that have not been removed: `rc-sentry` watchdog (breadcrumb file at `C:\RacingPoint\sentry-restart-breadcrumb.txt`), `RCWatchdog` Windows service (5s poll, `restart_grace_active` + `sentry_breadcrumb_active` deconfliction), and `server pod_monitor/WoL`. With 5 independent healers:

- Layer 1 Smart Watchdog detects crash-loop → triggers MMA → recommends rollback → starts rolling back
- Simultaneously Layer 2 Fleet Healer SSHes in to collect diagnostics → sees the binary swap in progress
- rc-sentry also wakes up → writes its breadcrumb → tries to restart via existing path
- Layer 3 External Guardian sees server not reporting healthy pods → escalates
- The MMA cycle from Layer 1 runs for 30-60s → by the time it completes, rc-sentry has already restarted the agent in a corrupted state (new binary half-written due to OTA_DEPLOYING not being checked)

This is not theoretical. It already happened during v17.1: "Self-restart + watchdog + WoL created an infinite restart loop that took 45 minutes to diagnose." The new layers multiply the competing actors from 3 to 5+.

**Why it happens:**
Each new layer is added incrementally to fix a new failure mode, with deconfliction added as an afterthought. The existing breadcrumb mechanism (`sentry-restart-breadcrumb.txt`) only deconflicts between rc-sentry and RCWatchdog. It is invisible to Layer 1 MMA, Layer 2 SSH healer, and Layer 3 Guardian.

**How to avoid:**
Implement a single `HEAL_IN_PROGRESS` sentinel file at `C:\RacingPoint\HEAL_IN_PROGRESS` before any autonomous healing action at ANY layer. Every recovery system (existing and new) must check this file before acting. Contents: JSON with `{"layer": 1, "started_at": "ISO8601", "action": "mma_diagnosis", "ttl_secs": 120}`. TTL is mandatory — sentinel expires automatically if healing crashes mid-way.

```rust
// In rc-watchdog, before MMA diagnosis:
fn try_acquire_heal_lock(ttl_secs: u64) -> bool {
    let path = Path::new(r"C:\RacingPoint\HEAL_IN_PROGRESS");
    // Check existing lock first
    if let Ok(contents) = fs::read_to_string(&path) {
        if let Ok(lock) = serde_json::from_str::<HealLock>(&contents) {
            if lock.started_at.elapsed_secs() < lock.ttl_secs {
                return false; // Another layer is healing
            }
        }
    }
    // Write our lock
    fs::write(&path, serde_json::to_string(&HealLock {
        layer: 1,
        started_at: Utc::now(),
        ttl_secs,
    }).unwrap_or_default()).is_ok()
}
```

Also extend the existing `OTA_DEPLOYING` sentinel check — all three new layers must skip ALL healing actions when `OTA_DEPLOYING` is present. The existing rc-watchdog service already respects `MAINTENANCE_MODE`; the new MMA-triggered actions must too.

**Warning signs:**
- `restart_count` incrementing faster than possible (2+ restarts per 10s window)
- `HEAL_IN_PROGRESS` file exists with age > TTL (healing crashed, stale lock)
- Layer 2 SSH diagnostics return "binary not found" — Layer 1 was mid-swap when Layer 2 ran
- Server fleet health shows pod flip-flopping between `ws_connected: true` and `ws_connected: false` every 5-15s

**Phase to address:** Phase 1 (Smart Watchdog core) — the sentinel protocol must be defined BEFORE any healing logic is written. Every subsequent phase references it.

---

### Pitfall 2: MAINTENANCE_MODE Has No Timeout — Smart Watchdog MMA Triggers It Then Locks Itself Out

**What goes wrong:**
The existing `MAINTENANCE_MODE` file blocks ALL restarts permanently (no TTL). The Smart Watchdog MMA loop detects a crash-loop (>3 restarts in 10 min), runs MMA diagnosis, then recommends "block further restarts while we analyze." If the watchdog writes `MAINTENANCE_MODE` as part of the analysis pause, it then cannot restart the agent after the fix is identified — the fix is correct but the sentinel blocks execution of the fix indefinitely.

This is not a new risk — v17.1 explicitly addressed it: "MAINTENANCE_MODE sentinel written after 3 restarts in 10 min, but has no auto-clear mechanism, no TTL, no timeout." However, v31.0 adds a new actor (Layer 1 MMA) that will interact with this sentinel in a new way: the MMA cycle itself takes 30-120s, meaning a pod can be in "analyzing" state much longer than the existing 10-min MAINTENANCE_MODE window anticipates.

**Why it happens:**
The MMA loop is slow by design (multi-model consensus). The sentinel was designed for human-in-the-loop operation where a human clears it. With autonomous operation, no human clears it.

**How to avoid:**
The v17.1 fix added a 30-minute auto-clear TTL to MAINTENANCE_MODE. v31.0 must ensure:

1. MMA diagnosis uses a SEPARATE sentinel (`MMA_DIAGNOSING`) with its own TTL (= MMA_TIMEOUT + 30s buffer), distinct from MAINTENANCE_MODE.
2. MAINTENANCE_MODE is NEVER written by the Smart Watchdog during autonomous MMA — it is only written by rc-agent itself after crash-loop detection. The watchdog reads it (to know healing is blocked) but does not write it.
3. If MMA recommends a fix and MAINTENANCE_MODE is present, the Smart Watchdog calls the server's new direct-report endpoint to have the server send a CLEAR_SENTINEL command to the pod via rc-sentry (bypassing the dead rc-agent).

```rust
// In rc-watchdog MMA completion:
fn apply_mma_fix(fix: &MmaFix) {
    // Check for blocking sentinels first
    if Path::new(r"C:\RacingPoint\MAINTENANCE_MODE").exists() {
        // Cannot act locally — escalate to server to clear via rc-sentry
        self.report_to_server(WatchdogReport {
            action_blocked_by: Some("MAINTENANCE_MODE".into()),
            recommended_fix: fix.clone(),
            ..
        });
        return;
    }
    // Proceed with fix
}
```

**Warning signs:**
- Pod stays in `ws_connected: false` indefinitely after MMA reports "fix identified"
- `MMA_DIAGNOSING` file age > 3 minutes (MMA stalled or crashed)
- `MAINTENANCE_MODE` present AND `HEAL_IN_PROGRESS` present simultaneously (two blocking sentinels)
- Server watchdog report endpoint receives `action_blocked_by: MAINTENANCE_MODE` repeatedly

**Phase to address:** Phase 1 (Smart Watchdog core) — sentinel inventory and interaction protocol must be defined before MMA integration.

---

### Pitfall 3: Windows Service Cannot Make HTTP Calls to OpenRouter Without Proxy Config

**What goes wrong:**
The Smart Watchdog runs as a Windows service (`RCWatchdog`, `NT AUTHORITY\SYSTEM`). The SYSTEM account on pod machines does NOT have WinHTTP proxy settings configured. The `reqwest` client in the existing `openrouter.rs` uses the system default trust store and proxy settings. When the SYSTEM account makes an outbound HTTPS request to `https://openrouter.ai`, three failure modes occur:

1. **Proxy redirect:** Venue WiFi has a captive portal or transparent proxy; SYSTEM doesn't get the `INTERNET_DEFAULT_PROXY` settings that the user account has
2. **Certificate validation:** SYSTEM's certificate store may not have the intermediate CA chain for OpenRouter's TLS cert, causing `certificate verify failed`
3. **TLS timeouts:** SYSTEM-context HTTP is subject to different timeout behavior — the `PER_ATTEMPT_TIMEOUT_SECS = 30` in `openrouter.rs` may not apply correctly from a service

The rc-agent already calls OpenRouter successfully but rc-agent runs in Session 1 (user context), not as SYSTEM. This distinction will cause the Smart Watchdog to fail on its first real MMA call in production even though it works in testing (where testing is done interactively).

**Why it happens:**
Service context vs user context HTTP differences are only visible at runtime. `cargo test` and local development run in user context. The service only runs on the pod hardware. The failure does not appear until a real crash-loop triggers MMA.

**How to avoid:**
```rust
// In rc-watchdog openrouter client initialization:
fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        // Explicit timeout — don't rely on system defaults from SYSTEM context
        .timeout(Duration::from_secs(45))
        // Load system certs AND bundled Mozilla root certs (fallback for SYSTEM store gaps)
        .tls_built_in_root_certs(true)
        // Disable proxy for direct API calls from service context
        // (venue proxy only applies to user-context browsing)
        .no_proxy()
        .build()
        .expect("reqwest client construction failed")
}
```

Test this BEFORE any MMA feature goes to production: deploy a service-context version that calls `https://openrouter.ai/api/v1/models` and verify the response in the watchdog log. This test call should be in the watchdog startup sequence.

**Warning signs:**
- Watchdog log shows `WARN openrouter — attempt 1/4 error: certificate verify failed`
- MMA diagnosis never completes, pod stays in crash-loop
- OpenRouter calls succeed when tested from rc-agent (Session 1) but fail from watchdog (SYSTEM)
- `reqwest::Error { kind: Connect }` from service context but not from user context

**Phase to address:** Phase 1 (Smart Watchdog core) — verify SYSTEM-context HTTP in a canary deploy before wiring MMA. A startup connectivity check (`POST /api/v1/models`) must pass before MMA is enabled.

---

### Pitfall 4: Rollback Loop — `rc-agent-prev.exe` Is Also Broken

**What goes wrong:**
The Smart Watchdog detects a crash in <30s, rolls back to `rc-agent-prev.exe`. If `rc-agent-prev.exe` is ALSO broken (corrupted download, same bug, or binary from before a required DB migration), the rollback itself crashes in <30s. The watchdog detects this, decides to "roll forward" — but the new binary is already marked bad. Net result: the watchdog alternates between two broken binaries, incrementing `restart_count` until MAINTENANCE_MODE fires.

This is different from the existing crash-loop detection because the crash-loop counter does not distinguish between "new binary is bad" and "both binaries are bad." After rollback, `restart_count` should reset (new state), but the existing counter is session-scoped and increments regardless.

**Why it happens:**
The deploy sequence creates `rc-agent-prev.exe` by renaming the outgoing binary. If two successive deploys both ship bad binaries, both `rc-agent.exe` AND `rc-agent-prev.exe` are bad. The rollback mechanism has no concept of "how many rollback depth levels" exist.

**How to avoid:**
1. Maintain a `rollback-state.json` at `C:\RacingPoint\rollback-state.json`:
   ```json
   {
     "current_hash": "abc123",
     "prev_hash": "def456",
     "rollback_attempted_at": null,
     "rollback_succeeded": null,
     "rollback_depth": 0
   }
   ```
2. After rollback, reset the crash-loop counter to 0 (new binary, new chance). If rollback binary also crashes in <30s, set `rollback_depth: 1` and do NOT attempt another rollback.
3. At `rollback_depth: 1` (both binaries bad), escalate to Layer 2 (server fleet healer) via the direct-report endpoint — do NOT loop. The watchdog sends: `{"action": "both_binaries_bad", "current_hash": "...", "prev_hash": "..."}`.
4. Layer 2 response: push a known-good binary from the server's staging area via its own download channel.

```rust
// In rc-watchdog rollback logic:
fn handle_crash_loop(state: &mut WatchdogState) {
    if state.rollback_depth == 0 && prev_binary_exists() {
        attempt_rollback();
        state.rollback_depth += 1;
        state.crash_count = 0; // Reset for rollback binary
    } else {
        // rollback_depth >= 1: both binaries bad
        // Do NOT write MAINTENANCE_MODE — escalate to server instead
        report_to_server(WatchdogReport {
            escalation_reason: "both_binaries_bad",
            ..
        });
        // Stop restarting — wait for server to push binary
        state.healing_paused = true;
    }
}
```

**Warning signs:**
- `restart_count` > 10 in watchdog report (alternating between two binaries)
- Log shows alternating "rolling back to prev" and "rolling forward to current"
- `rc-agent-prev.exe` crash time matches `rc-agent.exe` crash time (same code path failing)
- Pod dark for >15 minutes despite watchdog active (both binaries bad, paused)

**Phase to address:** Phase 1 (Smart Watchdog rollback logic) — depth tracking and "both bad" escalation path must be designed before rollback is implemented.

---

### Pitfall 5: Split-Brain Between James (Layer 3) and Bono (Layer 3)

**What goes wrong:**
Both James (.27) and Bono VPS are defined as "External Guardian" (Layer 3). Both watch the server. Both can trigger restart via SSH/schtasks. If the server is slow to respond (high CPU, network jitter), both guardians independently conclude the server is down and simultaneously:

1. James sends `schtasks /Run /TN StartRCOnBoot` via Tailscale SSH to the server
2. Bono sends the same command via its own SSH connection 30 seconds later
3. Two racecontrol instances attempt to bind port 8080 simultaneously → `os error 10048 (address in use)` → both crash
4. Both guardians now see the server as down and escalate again

This is the "16 orphan watchdog instances" incident from 2026-03-24 but at the inter-AI level instead of the intra-machine level.

**Why it happens:**
Distributed guardians without a coordination protocol independently observe the same symptom and independently apply the same fix. The fix itself (starting racecontrol) requires a "confirm kill" step that takes 15s. If both guardians start within that 15s window, they both "win" and create a conflict.

**How to avoid:**
One guardian owns server restarts. The other is in "standby" mode — it only acts if the primary guardian is itself unreachable. Concrete assignment: **Bono VPS is primary for server-level recovery** (24/7 always-on). James is secondary, only activates if Bono's VPS goes dark.

Implementation:
```
# Bono Layer 3 Guardian checks:
1. Is James's relay alive? (curl http://James:8766/relay/health)
2. If YES: Is James already acting on this? (check shared GUARDIAN_ACTING sentinel in comms-link)
3. If James acting: skip, let James finish
4. If neither acting: Bono acquires GUARDIAN_ACTING sentinel (written to comms-link INBOX.md)
5. Bono performs recovery
6. Bono clears GUARDIAN_ACTING sentinel
```

The `GUARDIAN_ACTING` sentinel must be in the shared comms-link channel (INBOX.md commit or a dedicated sentinel endpoint), NOT a local file on either machine. A local file only coordinates with the local process — it does nothing to coordinate between two different machines.

**Warning signs:**
- Server log shows `os error 10048` within 60 seconds of a restart attempt
- Both James and Bono WhatsApp notifications show "server restarted" at nearly identical timestamps
- `start-racecontrol.bat` logs show two simultaneous executions
- Server health endpoint alternates between available and `connection refused`

**Phase to address:** Phase 5 (External Guardian / Layer 3) — guardian coordination protocol must be the FIRST thing defined before either guardian's recovery logic is written.

---

### Pitfall 6: SSH Into Dark Pods — Fleet Healer SSH Concurrency Causes `MaxSessions` Exhaustion

**What goes wrong:**
Layer 2 (Server Fleet Healer) SSHes into dark pods for diagnostics. The existing OpenSSH on Windows pods defaults to `MaxSessions 10` and `MaxStartups 10:30:100`. With 8 pods potentially all dark simultaneously, the Fleet Healer might spawn parallel SSH connections. Additionally:

1. SSH to Windows pods uses a password or key (`ssh User@<pod_ip>`). The known-good path is Tailscale SSH (`ssh User@<tailscale_ip>`). But Tailscale on pods may also be down (if it's a deep crash where the pod can't connect to Tailscale coordination server).
2. The Fleet Healer is inside `racecontrol.exe` running on the server. `std::process::Command::new("ssh")` in a Rust async context blocks the calling thread. Spawning 8 concurrent `ssh` processes from Axum's async runtime causes thread pool starvation.
3. `ssh` in a non-interactive context (no TTY) may hang waiting for a password prompt if key auth fails, blocking indefinitely.

**Why it happens:**
Fleet healing was designed in the v26.0 MESHED-INTELLIGENCE spec but the SSH implementation details were deferred. The `pod_healer.rs` currently uses `rc-agent /exec` for diagnostics — it assumes rc-agent is alive. For the v31.0 use case (dark pods where rc-agent is dead), SSH is needed but not yet implemented.

**How to avoid:**
```rust
// In fleet_healer SSH diagnostics:
async fn ssh_diagnose_pod(pod_ip: &str, tailscale_ip: &str) -> Result<PodDiagnostics> {
    // Use tokio::process (non-blocking), not std::process
    let mut cmd = tokio::process::Command::new("ssh");
    cmd.args([
        "-o", "StrictHostKeyChecking=no",
        "-o", "ConnectTimeout=10",      // Fail fast — don't hang on dead pod
        "-o", "BatchMode=yes",          // No interactive prompts — key auth only
        "-o", "ServerAliveInterval=5",
        "-o", "ServerAliveCountMax=2",
        &format!("User@{}", tailscale_ip),
        "tasklist /FI \"IMAGENAME eq rc-agent.exe\" && dir C:\\RacingPoint\\"
    ]);

    // EXPLICIT timeout: 20s max per SSH command
    tokio::time::timeout(Duration::from_secs(20), cmd.output()).await
        .map_err(|_| anyhow!("SSH to {} timed out", tailscale_ip))?
}

// Limit concurrency — max 2 SSH connections at a time
static SSH_SEMAPHORE: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(2));
```

For Tailscale-down scenarios: maintain a second attempt path using LAN IP with `ping -n 1 -w 500` first, then SSH. If neither path reaches the pod, report as `unreachable` and use WoL if MAC address is known — do not hang.

**Warning signs:**
- Fleet healer cycle takes >2 minutes (multiple SSH hangs)
- `ssh` processes accumulate in `tasklist` on the server
- Fleet health check endpoint (:8080/api/v1/fleet/health) becomes slow (blocked on SSH in healer loop)
- Pod shows `unreachable` in fleet health but is actually up (Tailscale IP vs LAN IP confusion)

**Phase to address:** Phase 3 (Server Fleet Healer SSH diagnostics) — concurrency limit and timeout must be in the initial implementation, not added later.

---

### Pitfall 7: MMA Budget Overrun in Service Context — No User to Approve

**What goes wrong:**
The existing `budget_tracker.rs` in rc-agent has a `$10/day/pod` hard cap. When the cap hits, the existing code falls back to the mechanical (local Ollama) path. In v31.0, the Smart Watchdog adds its OWN MMA calls that also bill to the same OpenRouter API key. In a crash-loop scenario:

- Pod enters crash loop → watchdog MMA fires (5 models, ~$4/run)
- MMA recommends rollback → rollback binary also crashes → watchdog MMA fires AGAIN
- If this happens at 2am: 3 iterations = $12 before the existing rc-agent budget tracker even sees it (the budget tracker is inside rc-agent, which is dead during the crash loop)

The watchdog operates OUTSIDE the rc-agent process, so the existing `budget_tracker.rs` state is inaccessible to the watchdog. Two independent callers share one API key with no shared budget tracking.

**Why it happens:**
The v26.0 budget tracker was designed for rc-agent only. The watchdog was not an AI caller at that time. v31.0 makes the watchdog an AI caller, but the budget tracker doesn't know about it.

**How to avoid:**
1. The watchdog must maintain its OWN budget file at `C:\RacingPoint\watchdog-budget.json` (separate from rc-agent's budget state), with `$5/day` hard cap (half the pod budget).
2. Before any MMA call, the watchdog reads BOTH budget files (its own + rc-agent's, if accessible) and aborts MMA if combined daily spend > $8.
3. Budget file is shared via the server's direct-report endpoint — the watchdog sends its spend in every report so the server can aggregate fleet-wide AI spend.

```rust
struct WatchdogBudget {
    daily_spend_usd: f32,
    last_reset: NaiveDate,
    hard_cap_usd: f32, // = 5.0
}

fn can_run_mma(&self) -> bool {
    let today = Utc::now().date_naive();
    if self.last_reset < today {
        return true; // New day, reset
    }
    self.daily_spend_usd < self.hard_cap_usd
}
```

**Warning signs:**
- OpenRouter API key returns 402 (payment required) across ALL layers simultaneously
- Watchdog log shows MMA calls at high frequency (>3/hour)
- No budget file exists at `C:\RacingPoint\watchdog-budget.json` (budget never persisted)
- rc-agent budget tracker shows $0 spend but OpenRouter bills show high usage (watchdog spend uncounted)

**Phase to address:** Phase 1 (Smart Watchdog MMA integration) — budget file must exist before the first MMA call. Never add MMA without a spend cap.

---

### Pitfall 8: Binary Manifest TOCTOU — Check Happens on Disk, Launch Happens Seconds Later

**What goes wrong:**
The Smart Watchdog validates the SHA256 of `rc-agent.exe` against a manifest before launching. This is correct. However:

1. The check happens at T=0: hash matches manifest → OK
2. Between T=0 and T=2s (when the watchdog calls `spawn_in_session1()`), an interrupted download or partial OTA write overwrites `rc-agent.exe` with a partially-written file
3. The watchdog launches the partially-written binary
4. The binary crashes instantly (PE header corrupted) → crash-loop starts

The TOCTOU window is especially large if the watchdog is throttling between the check and the launch (e.g., waiting for MAINTENANCE_MODE to clear, running MMA, etc.).

A secondary case: the manifest distribution path. The server serves the manifest at an endpoint like `/api/v1/manifest`. If the manifest is fetched AFTER the binary is downloaded (instead of before), the binary has already been written by the time the hash is checked against a fresh manifest — a corrupted download could match an older manifest entry if the server is serving a cached response.

**Why it happens:**
Manifest checks are added as a pre-condition check, not as a "load-and-lock" operation. The binary is treated as immutable between check and launch, but it is not.

**How to avoid:**
```rust
fn validate_and_prepare_binary(path: &Path) -> Result<ValidatedBinary> {
    // 1. Open file with FILE_FLAG_SEQUENTIAL_SCAN (no write sharing)
    // 2. Hash the open file descriptor (not the path)
    // 3. Keep file handle open until spawn — no window for replacement
    // Actually in Windows, rename() is used for atomic swap (delete prev, rename new)
    // So the correct check is: hash THEN immediately rename to a temp name for launch

    let hash = sha256_file(path)?;
    let manifest = fetch_manifest_from_server()?;
    if manifest.get_hash_for(path) != Some(&hash) {
        return Err(anyhow!("Binary hash mismatch — abort launch"));
    }
    // Atomically rename to a "validated" copy that OTA cannot overwrite
    let validated_path = path.with_extension("validated.exe");
    fs::rename(path, &validated_path)?; // Atomic on same volume
    // ... launch from validated_path
    Ok(ValidatedBinary { path: validated_path })
}
```

For manifest distribution: fetch the manifest FIRST from the server (with auth), then download and verify the binary against the fetched manifest. Never check a downloaded binary against a manifest fetched after the download.

**Warning signs:**
- `rc-agent.exe` file size changes between watchdog check and launch
- `OTA_DEPLOYING` file is absent but binary content is inconsistent with manifest hash
- Crash at T < 5s with exit code -1073741795 (0xC000007B — invalid image format)
- Multiple hash-check failures in rapid succession (active OTA in progress)

**Phase to address:** Phase 1 (Smart Watchdog binary validation) — the check-then-launch must be made atomic before rollback logic is added.

---

### Pitfall 9: Layer 2 Fleet Healer SSH Runs During Active Customer Sessions

**What goes wrong:**
Layer 2 SSH diagnostics are triggered when a pod appears "dark" to the server (no WS connection). However, a pod can appear dark to the server while STILL HAVING AN ACTIVE BILLING SESSION if:

- The WebSocket connection dropped (brief network glitch) but rc-agent is still running
- The billing timer is persisting to SQLite on the pod (heartbeat every 60s)
- A customer is mid-session

If the Fleet Healer SSHes in and starts running diagnostics (`tasklist`, `netstat`, reading log files), it competes for I/O with the billing heartbeat. Worse, if the Fleet Healer decides to push a new binary and restart rc-agent, it kills an active billing session — the customer loses their remaining session time and the venue loses the revenue.

The existing `pod_healer.rs` already has this check:
```rust
const PROTECTED_PROCESSES: &[&str] = &["rc-agent.exe", "acs.exe", ...];
```
But this protection is at the "kill process" level. The Layer 2 SSH healer operates at a lower level (direct SSH commands) and bypasses this protection entirely.

**Why it happens:**
Layer 2 SSH is designed for "dark pods" — the assumption is that if SSH is needed, rc-agent is dead. But a pod can be dark to the server without rc-agent being dead (WS disconnected ≠ rc-agent dead).

**How to avoid:**
Before any Layer 2 SSH action that modifies the pod (binary push, process kill, restart):
1. Attempt to reach the pod via HTTP directly: `curl http://<pod_ip>:8090/health` — if this succeeds, rc-agent IS alive, WS merely disconnected. Switch to WS reconnect path, not SSH intervention.
2. Read `C:\RacingPoint\billing_active.sentinel` via SSH before any disruptive action — if this file exists and is <120s old, a billing session is active. SSH diagnostics only, no restarts.
3. The billing session drain from the OTA pipeline (`has_active_billing_session()`) must also be called from the Layer 2 healer before any binary push.

**Warning signs:**
- Pod shows `ws_connected: false` but `http_reachable: true` — this is WS glitch, NOT dead pod
- Fleet Healer SSH log shows "session drain: 0" but billing DB has an active session (check times)
- Customer complains about session ending unexpectedly during Fleet Healer cycle
- `billing_active.sentinel` present on pod at time of SSH healer action

**Phase to address:** Phase 3 (Server Fleet Healer) — "dark pod" must have three definitions: WS-only dark (WS down, HTTP up), partially dark (WS down, HTTP up but unhealthy), and truly dark (WS down, HTTP unreachable). Each requires a different healer response.

---

### Pitfall 10: OpenRouter API Rate Limits and 503s During a Fleet Crash Storm

**What goes wrong:**
A firmware update, power event, or network issue takes down all 8 pods simultaneously. All 8 Smart Watchdog instances independently detect the crash and independently trigger MMA diagnosis. OpenRouter receives 8 parallel requests for 5-model consensus, each spawning 5 API calls = 40 concurrent API calls from one key. OpenRouter rate limits at the API key level (not per-IP). Result:

- 6 of 8 watchdog MMA calls get 429 errors
- The existing `MAX_RETRIES: 4` + exponential backoff in `openrouter.rs` retries with up to 10s delays
- All 8 watchdogs are now in retry loops simultaneously, retrying at nearly synchronized intervals (thundering herd)
- The retries themselves cause more 429s
- No pod gets an MMA result for 5-10 minutes

The existing `TIER4_SEMAPHORE: Semaphore::new(2)` in `openrouter.rs` limits concurrency within ONE rc-agent process. It does NOT limit concurrency across 8 pod watchdogs.

**Why it happens:**
The semaphore in `openrouter.rs` is a static within one process. Cross-process coordination requires a different mechanism.

**How to avoid:**
1. Stagger watchdog MMA triggers by pod number. Pod 1 waits 0s, Pod 2 waits 15s, Pod 3 waits 30s, etc. After the first MMA result, Layer 2 (Fleet Healer) detects the fleet-wide pattern and can provide the same root cause to all pods without running 8 separate MMA cycles.
2. Fleet-wide pattern detection (already in the v31.0 spec: "same failure on 3+ pods = systemic issue") should SHORT-CIRCUIT individual pod MMA. The Fleet Healer runs ONE MMA on the pattern, distributes the result to all affected pods.
3. In the watchdog retry logic, add full jitter: `delay = rand(0, BASE_DELAY_MS * 2^attempt)` (not just `BASE_DELAY_MS * 2^attempt`). The existing `openrouter.rs` uses fixed base delay multiplied by attempt — add randomization to spread retries.

```rust
// Add to rc-watchdog openrouter client:
fn backoff_with_jitter(attempt: u32) -> Duration {
    let base = BASE_DELAY_MS * (1u64 << attempt.min(5));
    let jitter = rand::random::<u64>() % base;
    Duration::from_millis((base + jitter).min(MAX_DELAY_MS))
}
```

**Warning signs:**
- 8 pods all enter crash-loop within 60s of each other (fleet-wide event)
- OpenRouter API logs show >20 requests from same key in <10s
- Watchdog logs show "429 Too Many Requests — attempt N/4" on most pods
- Fleet Healer detects fleet-wide pattern but continues running pod-by-pod MMA anyway

**Phase to address:** Phase 2 (Unified MMA Protocol) — staggering and fleet-pattern short-circuit must be in the MMA spec before implementation.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Using `tasklist` polling to detect crashes (current rc-watchdog) | Simple, no extra dependencies | Cannot determine WHY the crash happened (exit code, exception type) | Acceptable for restart detection; NEVER as input for MMA diagnosis |
| Hardcoding `C:\RacingPoint` path in watchdog | Simpler code | All machines must use this exact path | Acceptable for pods (all configured this way); NOT acceptable for server or James |
| Single OpenRouter key for all layers | One thing to manage | Budget overrun between layers invisible; key rotation affects everything | Only acceptable with cross-layer spend tracking (Pitfall 7) |
| Writing MAINTENANCE_MODE from watchdog | Stops restart storms | Watchdog locks itself out of applying the fix | Never — watchdog must NOT write MAINTENANCE_MODE |
| Using sentry breadcrumb file for all deconfliction | Already exists | Does not scale to 3+ recovery layers | Never for v31.0 — extend to HEAL_IN_PROGRESS sentinel |
| Running MMA on every single crash (not just crash-loops) | More diagnosis data | $4/crash × 8 pods × 3 crashes/day = $96/day | Never — MMA only on confirmed crash-loops (>3 restarts / 10 min) |

---

## Integration Gotchas

Common mistakes when connecting the new layers to existing systems.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Watchdog → Server direct-report endpoint | Adding endpoint to `public_routes` (no auth) | Behind `require_service_key` middleware; watchdog sends `RCSENTRY_SERVICE_KEY` header |
| Fleet Healer SSH → Windows pods | Using password auth (may prompt) | Key-based auth only; `BatchMode=yes` SSH flag; `rc-watchdog` key pre-authorized on all pods |
| External Guardian → Server schtasks | Calling `schtasks /Run /TN StartRCOnBoot` directly | Use `deploy-server.sh` logic: disable watchdog → confirmed kill → swap → start → verify |
| Layer 1 MMA → OpenRouter | Using rc-agent's `openrouter.rs` unchanged | Separate client with SYSTEM-context certificate handling and no-proxy setting |
| Layer 2 SSH → Pod exec | Running arbitrary commands | Whitelist: `tasklist`, `dir`, `netstat -an`, `type C:\RacingPoint\*.log` — never arbitrary shell |
| Manifest server → Pods | Serving unsigned manifest over HTTP | Manifest must be signed with HMAC-SHA256 using the service key; watchdog verifies before trusting |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Synchronous SSH in async healer | Fleet health endpoint becomes slow during pod-down events | `tokio::process::Command` + `tokio::time::timeout(20s)` | When >2 pods go dark simultaneously |
| MMA on every watchdog cycle | $100+/day API bills | MMA gated behind crash-loop threshold (>3 restarts/10min) | Immediately if trigger threshold is too low |
| Loading full pod diagnostics into MMA prompt | Token limits exceeded for complex cases | Cap diagnostic context at 4000 tokens; summarize log tail | When pod has 100MB of crash logs |
| Fleet healer collecting ALL pod logs via SSH | Server memory pressure from 8× multi-MB log transfers | Collect last 50 lines only; stream via SSH, don't buffer | When pods have verbose logging enabled |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| OpenRouter key in watchdog registry or toml | Key exposed to any local admin process | Environment variable only, set via service properties; NEVER in config file |
| Watchdog direct-report endpoint without auth | Any LAN client can inject false crash reports | `RCSENTRY_SERVICE_KEY` header required on all new Layer 1 endpoints |
| SSH private key stored on server in plain text | Compromised server = fleet access | Key stored in `C:\RacingPoint\fleet-ssh-key` with ACL limiting to ADMIN user only |
| Guardian executing arbitrary server commands via SSH | Compromise of guardian machine = full server access | Guardian whitelist: `schtasks`, `netstat`, `dir C:\RacingPoint\`; no arbitrary shell |
| Manifest served over unauthenticated HTTP | MITM can swap manifest, watchdog accepts corrupted binary | HMAC-SHA256 signed manifest; watchdog verifies signature with pre-shared key |

---

## "Looks Done But Isn't" Checklist

- [ ] **HEAL_IN_PROGRESS sentinel:** All 5 recovery systems check it before acting — verify by grepping `HEAL_IN_PROGRESS` appears in: `rc-watchdog/src/service.rs`, `rc-sentry/src/`, `racecontrol/src/pod_healer.rs`, `racecontrol/src/fleet_health.rs`, and the External Guardian script.
- [ ] **Rollback depth tracking:** `rollback-state.json` exists and `rollback_depth` field is checked before the second rollback attempt — verify by grepping `rollback_depth` in watchdog code.
- [ ] **Budget file bootstrapping:** `watchdog-budget.json` is created on first start if missing (do not crash on missing file) — verify watchdog starts cleanly on a fresh pod with no budget file.
- [ ] **SYSTEM-context HTTP test:** Watchdog startup runs `GET https://openrouter.ai/api/v1/models` as SYSTEM and logs the result before MMA is enabled — verify this test appears in watchdog boot log.
- [ ] **Session awareness in Layer 2:** Fleet Healer checks `billing_active.sentinel` before ANY binary push — verify by grepping `billing_active` in `racecontrol/src/` fleet healer code.
- [ ] **Guardian coordination:** `GUARDIAN_ACTING` is written to comms-link (shared channel) not to a local file — verify by grepping `GUARDIAN_ACTING` in both James and Bono guardian scripts.
- [ ] **MMA stagger:** Watchdog MMA trigger is staggered by pod number — verify by reading pod-ID-based delay calculation in watchdog code.
- [ ] **Manifest signature verification:** Watchdog rejects manifests without valid HMAC-SHA256 — verify test exists for malformed manifest handling.

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Recovery system fight | MEDIUM | (1) `del C:\RacingPoint\HEAL_IN_PROGRESS` on affected pod, (2) restart rc-watchdog service, (3) manually clear breadcrumb file |
| MAINTENANCE_MODE lockout | LOW | `del C:\RacingPoint\MAINTENANCE_MODE` via rc-sentry exec; `schtasks /Run /TN StartRCAgent` via SSH |
| Both binaries bad | HIGH | SSH to pod → `scp` known-good binary from server → `ren rc-agent.exe rc-agent-prev.exe && ren rc-agent-good.exe rc-agent.exe && schtasks /Run /TN StartRCAgent` |
| Split-brain double-restart | MEDIUM | SSH to server → `taskkill /F /IM racecontrol.exe` → wait 5s → `schtasks /Run /TN StartRCDirect` → verify one instance in tasklist |
| OpenRouter rate limit storm | LOW | Wait 60s for backoff to clear; manually trigger `del C:\RacingPoint\MMA_DIAGNOSING` on affected pods to unblock |
| SSH session exhaustion | LOW | Restart OpenSSH service on affected pods via WoL + scheduled task |
| Budget overrun | MEDIUM | Delete `watchdog-budget.json` ONLY if day has rolled over; otherwise wait for midnight reset; check OpenRouter dashboard for anomalous spend |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Recovery system fight (Pitfall 1) | Phase 1 — Smart Watchdog core | `grep HEAL_IN_PROGRESS` in all recovery system source files |
| MAINTENANCE_MODE lockout (Pitfall 2) | Phase 1 — Smart Watchdog core | Test: trigger crash-loop → verify MMA completes → verify fix applied without MAINTENANCE_MODE blocking |
| SYSTEM-context HTTP failure (Pitfall 3) | Phase 1 — Smart Watchdog core | Deploy watchdog canary on Pod 8 → verify OpenRouter HTTP succeeds from service log |
| Rollback loop (Pitfall 4) | Phase 1 — Smart Watchdog rollback | Test: deploy two bad binaries in sequence → verify watchdog escalates at depth=1, not loops |
| Split-brain guardians (Pitfall 5) | Phase 5 — External Guardian | Test: simulate server down with both guardians active → verify only one restart occurs |
| SSH concurrency exhaustion (Pitfall 6) | Phase 3 — Fleet Healer SSH | Load test: 8 dark pods → verify SSH semaphore prevents >2 concurrent connections |
| Budget overrun in service (Pitfall 7) | Phase 2 — Unified MMA Protocol | Verify `watchdog-budget.json` created on first start; verify MMA blocked after cap |
| Binary manifest TOCTOU (Pitfall 8) | Phase 1 — Smart Watchdog binary validation | Test: corrupt `rc-agent.exe` between check and launch → verify watchdog detects mismatch |
| Active session disruption by Layer 2 (Pitfall 9) | Phase 3 — Fleet Healer | Test: create active billing session → verify fleet healer does NOT push binary or restart |
| OpenRouter thundering herd (Pitfall 10) | Phase 2 — Unified MMA Protocol | Test: trigger fleet-wide crash → verify pod-staggered MMA calls and fleet-pattern short-circuit |

---

## Sources

- CLAUDE.md standing rules — all incidents marked "Why:" — direct evidence from this codebase
- MEMORY.md — shipped milestones v17.1, v26.0, v27.0, v28.0 incident history
- PROJECT.md v31.0 milestone definition — architecture and constraints
- `crates/rc-watchdog/src/service.rs` — current deconfliction implementation (breadcrumb file, grace window)
- `crates/rc-agent/src/openrouter.rs` — existing MMA client (semaphore, retry logic, SYSTEM context gap)
- `crates/racecontrol/src/pod_healer.rs` — existing fleet healer (protected processes, WoL interaction)
- `crates/racecontrol/src/fleet_health.rs` — crash-loop detection implementation
- `.planning/research/PITFALLS-v17.1-watchdog-ai.md` — prior watchdog AI pitfalls (Session 0/1, spawn verification)
- 2026-03-24 incident: 16 orphan PowerShell watchdog instances (split-brain pattern at intra-machine scale)
- 2026-03-26 incident: MAINTENANCE_MODE blocked 3 pods for 1.5h without alert
- 2026-03-29 incident: both racecontrol instances attempted to bind port 8080 simultaneously

---
*Pitfalls research for: v31.0 Autonomous Survival System — 3-Layer MI Independence*
*Researched: 2026-03-30*
