# Feature Research: v17.1 Watchdog-to-AI Migration

**Domain:** Intelligent process supervision, AI-driven recovery, crash pattern memory
**Researched:** 2026-03-25
**Confidence:** HIGH (existing codebase well understood; Windows Session 0 behavior confirmed from standing rules incident logs; Erlang OTP supervisor patterns authoritative; spawn verification behavior confirmed from standing rules)

---

## Context: What Already Exists (Do Not Re-Build)

| Already in production | What v17.1 changes |
|-----------------------|--------------------|
| rc-sentry 6-endpoint fallback — blind 5s health poll + restart | Replace with: pattern memory lookup → Tier 1 deterministic fix → Tier 3 Ollama → escalate |
| self_monitor.rs relaunch — PowerShell DETACHED_PROCESS on crash | Preserve this mechanism; add sentinel file coordination so rc-sentry doesn't fight it |
| pod_monitor + pod_healer (server-side) — WoL + restart decisions | Merge context: distinguish crash vs deliberate shutdown via intent registry |
| james_watchdog.ps1 — blind 2min Windows service check on James | Replace with AI debugger + pattern memory; same diagnostic Tier 1–4 pipeline |
| MAINTENANCE_MODE sentinel — blocks ALL restarts after 3 crashes in 10min | Extend to carry REASON for maintenance mode, not just flag presence |
| debug-memory.json (planned in v11.2) — crash pattern persistence | This is the foundation for all AI recovery features; must exist before others |
| Health endpoint polling: localhost:8090/health every 5s in rc-sentry | Keep — but response drives graduated response, not blind restart |

**Critical pre-existing bug (incident trigger):**
`.spawn().is_ok()` returning `Ok` does NOT mean the child started. Confirmed on Windows: `cmd /C start`, `PowerShell Start-Process`, and `schtasks /Run` all return `Ok` from `std::process::Command` in a non-interactive context but silently fail to start the target. "restarted=true" was logged for days while pods stayed dead. This is the root cause the entire v17.1 milestone must resolve at every spawn site.

---

## Feature Landscape

### Table Stakes (Must Have — AI Recovery Is Incomplete Without These)

Features that, if missing, make "AI recovery" indistinguishable from the dumb watchdog it replaces.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Spawn verification** — confirm child process actually started after every restart attempt | Without this, "restarted=true" is a lie. The entire premise of v17.1 is that dumb watchdogs say "restarted" without verifying. AI recovery must never do this. | MEDIUM | Three verification strategies (see Feature Details section). All three needed depending on context. |
| **Crash pattern memory (debug-memory.json)** — persist crash signatures across restarts | Pattern matching only works if history survives restarts. Without persistence, every crash is "new" and the AI never gets faster. | LOW | File exists (v11.2 design). Key: it must survive rc-agent death because rc-sentry is the reader, not rc-agent. Store on shared path both can read. |
| **Tier 1 deterministic fix library** — predefined fixes for known crash patterns before touching Ollama | If you call Ollama for every crash, latency is 2–10s per incident. Deterministic fixes (stale socket cleanup, MAINTENANCE_MODE clear, zombie process kill) should fire in <100ms. | MEDIUM | Already partially built in v11.2. Needs expansion to cover rc-sentry, pod_monitor, and james_watchdog domains. |
| **Recovery intent registry** — machine-readable record of "why a restart is happening" | Four recovery systems exist (self_monitor, rc-sentry, pod_monitor, james_watchdog). Without coordination, they fight. Standing Rule #10 requires this. | MEDIUM | Sentinel file approach: write intent before acting, check intent before acting. See Anti-Pattern: Blind Restart. |
| **Graduated response with cooldown** — Tier 1 → Tier 2 → Tier 3 → block, not repeat-Tier-1 forever | Erlang OTP lesson: if intensity=10, period=1s, you get 10 restarts/second forever filling logs. Graduated response with increasing cooldowns prevents crash storms. | LOW | Map existing MAINTENANCE_MODE (3 restarts in 10min) to Tier 4 (block). Add Tier 2 (2nd fail → wait 30s), Tier 3 (3rd fail → Ollama diagnosis). |
| **Restart count reset on real recovery** | If you don't reset the counter when recovery succeeds, a pod that crashed once and recovered normally is penalized on its next legitimate crash. | LOW | Already exists partially (WD-04 backoff reset). Needs to be shared state in recovery intent registry. |

### Differentiators (AI Recovery vs Dumb Watchdog)

Features that make the recovery genuinely intelligent, not just faster blind restart.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Crash fingerprinting** — normalize log lines to a canonical signature, ignoring timestamps, PIDs, and addresses | Two crashes from the same root cause produce different log lines (different PID, timestamp). Fingerprinting groups them into one known pattern, making pattern memory actually useful. | MEDIUM | Strip: timestamps, PIDs, hex addresses, port numbers. Keep: error class, module path, message text. Hash the normalized string for O(1) lookup in debug-memory.json. |
| **Context-aware restart decision** — distinguish crash vs deliberate shutdown vs maintenance shutdown vs server-down restart | pod_monitor currently does WoL + restart on any "pod offline" signal. A pod in MAINTENANCE_MODE triggered by crash storm should NOT be woken by WoL. A pod offline because staff is rebooting it should not trigger alert. | HIGH | Intent registry + server-side "last known reason" field per pod in AppState. Server stores shutdown reason when it knows it (session end, admin action). Unknown = crash hypothesis. |
| **Ollama Tier 3 diagnosis with structured output** — query local qwen2.5:3b at James .27:11434 for unknown crash patterns | When Tier 1 deterministic fixes don't match, LLM can suggest fixes from crash context. Already deployed to all pods (v8.0). Structured JSON output (action, confidence, reasoning) enables safe automated action. | HIGH | Existing rp-debug modelfile already in place. Key: LLM output must go through safe-action whitelist before execution. Never execute raw LLM output. |
| **Recovery telemetry to server** — push recovery attempt results (pattern matched, fix applied, outcome) as structured events | Staff dashboard currently shows ws_connected and http_reachable per pod. Recovery events add "last recovery: socket cleanup, 14min ago, success" — visible to Uday without requiring SSH access. | MEDIUM | Reuse existing fleet events WebSocket channel. New event type: `RecoveryAttempted { pattern_id, tier, action_taken, outcome, duration_ms }`. |
| **Silent failure detection** — detect processes that appear alive (HTTP 200) but are functionally dead | rc-agent can return 200 on /health while the billing state machine is deadlocked, or while Edge has crashed and not been relaunched. Functional health check: probe deeper than HTTP — check session state, check lock screen visible. | HIGH | Existing idle-state health checks (v17.0: Edge alive + window rect + HTTP). Wire these into the recovery decision: if HTTP ok but functional check fails, trigger recovery even though "healthy". |
| **Pattern memory confidence scoring** — track fix success rate per crash pattern | A fix that worked once might not always work. Track: attempts / successes / last_applied per pattern. After 3 failures of the same fix, escalate even if pattern matches. | LOW | Add fields to debug-memory.json: `attempts: u32`, `successes: u32`, `last_outcome: Outcome`. Confidence = successes / attempts. If confidence < 0.5 after 3+ attempts, skip to Tier 3. |

### Anti-Features (Commonly Requested, Actively Harmful)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Blind restart as fallback** | "Just restart it if nothing else works" | This is exactly the dumb watchdog behavior being replaced. If AI recovery exhaust all tiers without fix, blind restart is the WRONG default — it creates infinite loops (the standing rule #10 incident). | After Tier 3 fails: write MAINTENANCE_MODE with reason, alert staff, STOP. Human decision required. |
| **Process inspection (tasklist, OpenProcess by name)** | "Need to know if process is running" | Triggers Easy Anti-Cheat (F1 25 EAC) and iRacing AC. Standing rule: health endpoint polling only (no process inspection) on pods with active anti-cheat games. | HTTP /health endpoint polling. For non-gaming processes (rc-sentry, james_watchdog), process handle retention after spawn is safe. |
| **Per-pod binary variants for recovery behavior** | "Pod 8 is canary, give it different watchdog config" | Compile-time per-pod variants are explicitly banned (single-binary-tier policy from v22.0). Creates untested combinations. | Runtime feature flags (v22.0 foundation) for graduated rollout. All pods same binary, behavior difference via config. |
| **LLM as first responder** | "AI is smarter, use it first" | qwen2.5:3b at .27 adds 2–10s latency. During a crash storm, this is unacceptable. Also: LLM is probabilistic, deterministic Tier 1 fixes are guaranteed correct for known patterns. | LLM is always Tier 3 — only after deterministic Tier 1 and pattern-memory Tier 2 fail to match. |
| **Unified "super watchdog" binary that does everything** | "Simpler to have one recovery process" | Single point of failure. rc-sentry's value is that it survives rc-agent death. A unified watchdog that crashes with rc-agent provides no recovery capability. | Keep recovery decoupled: rc-sentry (pod-side external), pod_monitor (server-side external), james_watchdog (James-side). Coordinate via shared protocol, not shared binary. |
| **WMI process monitoring** | "Get reliable start/stop events instead of polling" | WMI Win32_ProcessStartTrace has well-known reliability issues — events can be dropped under load. Also: potential anti-cheat trigger on gaming pods. | Keep HTTP /health polling for pod-side. For James's machine (no anti-cheat): WMI is acceptable. For pods: health endpoint + process handle if spawned by rc-sentry. |

---

## Feature Details: Spawn Verification Strategies

Three complementary strategies — choose based on what spawned the process and whether anti-cheat applies.

### Strategy A: Process Handle Retention (preferred when rc-sentry spawns the child)

When rc-sentry directly spawns rc-agent via `std::process::Command`, keep the `Child` handle. Poll `child.try_wait()` after a 2s delay to confirm the child is still running. If `try_wait()` returns `Ok(Some(status))`, the child exited immediately — spawn verification FAILED.

```rust
// After spawning
let mut child = Command::new("rc-agent.exe").spawn()?;
tokio::time::sleep(Duration::from_secs(2)).await;
match child.try_wait() {
    Ok(Some(status)) => { /* child died immediately — spawn failed */ }
    Ok(None) => { /* child still running — spawn succeeded */ }
    Err(e) => { /* can't query — assume failed, log warning */ }
}
```

**Limitation:** Only works when rc-sentry is the direct spawner. Does not work for processes started via `schtasks`, `cmd /C start`, or the HKLM Run key (non-interactive context).

### Strategy B: Health Endpoint Poll After Spawn (always safe, anti-cheat safe)

After triggering a restart via any mechanism (schtasks, RCAGENT_SELF_RESTART sentinel, bat file execution), poll `localhost:8090/health` with a 500ms interval and 10s timeout. If health returns HTTP 200 within the window, spawn verified. If timeout expires without response, spawn verification FAILED.

This is the correct strategy for non-interactive context spawns where process handle retention is impossible.

```rust
// After triggering restart via schtasks or sentinel
let deadline = Instant::now() + Duration::from_secs(10);
loop {
    if let Ok(resp) = http_get("http://localhost:8090/health").await {
        if resp.status == 200 { return SpawnResult::Verified; }
    }
    if Instant::now() > deadline { return SpawnResult::Failed; }
    tokio::time::sleep(Duration::from_millis(500)).await;
}
```

**Limitation:** 10s window may be tight during cold boot or shader compilation. Config: make timeout configurable (default 10s, tunable per recovery scenario).

### Strategy C: Sentinel File Written by Child (for processes that can cooperate)

On startup, the child writes a sentinel file (e.g., `C:\RacingPoint\rc-agent-started.sentinel`) with its PID and timestamp. On restart, rc-sentry deletes the sentinel before triggering spawn, then waits for the file to reappear. Reappearance = spawn verified. No reappearance within timeout = spawn verification FAILED.

**Advantage:** Works even when HTTP isn't up yet (e.g., early in startup before Axum binds the port).
**Limitation:** Requires modifying the child binary to write the sentinel. Not available for third-party processes.

**Recommended approach for v17.1:** Strategy B (health endpoint) as primary. Strategy A (handle retention) as supplement when rc-sentry directly spawns. Strategy C deferred — requires rc-agent changes and adds complexity.

---

## Feature Details: Recovery Intent Registry

The intent registry solves standing rule #10: "recovery systems must not fight each other."

### Intent Registry Design

A shared file at `C:\RacingPoint\recovery-intent.json` (accessible to rc-sentry, readable via HTTP by server pod_monitor):

```json
{
  "pod_id": 3,
  "intent": "restart_rc_agent",
  "initiated_by": "rc-sentry",
  "reason": "health_endpoint_timeout",
  "crash_pattern_id": "socket_stale_0x4f2a",
  "tier": 1,
  "started_at_utc": "2026-03-25T09:14:22Z",
  "expires_at_utc": "2026-03-25T09:16:22Z"
}
```

### Coordination Protocol

Before any recovery action:
1. Read `recovery-intent.json`. If another system has a non-expired intent for the same target → SKIP (don't fight).
2. Write your own intent with a 2-minute expiry.
3. Execute recovery.
4. On completion (success or failure), update intent with `outcome` field.
5. Server pod_monitor polls `GET /api/v1/pod/{id}/recovery-intent` (new endpoint, reads the file via rc-sentry's `/files` endpoint) to decide whether to suppress WoL.

### Intent Expiry

Intents must expire. A stale intent with no outcome (e.g., rc-sentry crashed mid-recovery) must not block recovery forever. 2-minute TTL: if an intent has no outcome AND was started > 2 minutes ago, it is treated as abandoned and a new recovery attempt can begin.

---

## Feature Details: Graduated Response Tiers

Map to existing tier vocabulary (v11.2 established Tier 1–4):

| Tier | Trigger | Action | Max attempts | Cooldown |
|------|---------|--------|-------------|----------|
| **Tier 0** | First health poll miss | Wait 1 poll cycle (5s), re-check | 3 misses | None — not yet a real failure |
| **Tier 1** | 3 consecutive health misses OR crash pattern matched | Deterministic fix (socket cleanup, sentinel clear, zombie kill) + spawn with verification | 2 | 30s between attempts |
| **Tier 2** | Tier 1 fix applied but health still failing after 30s | Check pattern memory — if confidence > 0.7 on a known fix, apply it. Otherwise: write diagnostic context, wait 60s, retry Tier 1 | 1 | 60s |
| **Tier 3** | Tier 2 failed OR unknown crash pattern | Query Ollama (qwen2.5:3b), parse structured output, apply if within safe-action whitelist. Alert staff via WhatsApp/WS. | 1 | 120s |
| **Tier 4** | Tier 3 failed OR 3+ crashes in 10min window | Write MAINTENANCE_MODE with reason + diagnostic context. Stop all restarts. Alert staff. Wait for manual clear. | Permanent until cleared | N/A |

**Tier 0 rationale:** The existing 5s health poll means a single missed poll is noise (network hiccup, GC pause). Three consecutive misses in 15s is a real failure. This prevents the tasklist-returns-empty false-positive that caused the 30s screen flicker incident in v17.0.

**Tier 4 distinction from current MAINTENANCE_MODE:** Current implementation writes the file but carries no reason. Tier 4 must write: `{ "reason": "tier3_exhausted", "last_crash_pattern": "...", "tier3_action_attempted": "...", "tier3_outcome": "failed", "context_log_tail": [...] }`. Staff can read this to know what happened without SSH.

---

## Feature Dependencies

```
Spawn Verification (Strategy B: health poll)
    └──required by──> Graduated Response Tiers (all tiers need to verify restart worked)

Crash Pattern Memory (debug-memory.json)
    └──required by──> Pattern Fingerprinting (nothing to match against without history)
    └──required by──> Confidence Scoring (no history = no score)

Recovery Intent Registry
    └──required by──> Context-Aware Restart Decision (pod_monitor needs to read intent)
    └──required by──> All Tier 1–4 actions (must write intent before acting)

Tier 1 Deterministic Fix Library
    └──required by──> Graduated Response Tiers (Tier 1 is the first responder)

Graduated Response Tiers
    └──enhanced by──> Crash Pattern Memory (Tier 2 pattern lookup)
    └──enhanced by──> Ollama Tier 3 (Tier 3 diagnosis)
    └──enhanced by──> Recovery Telemetry (surfaces outcomes to staff dashboard)

Silent Failure Detection (functional health check)
    └──enhances──> Graduated Response (triggers recovery even when HTTP 200 lies)
    └──depends on──> v17.0 idle-state health checks (already shipped)

Recovery Authority Coordination
    └──conflicts with──> Multiple independent watchdogs acting without coordination
    └──resolved by──> Recovery Intent Registry
```

### Dependency Notes

- **Spawn Verification is pre-requisite for everything else.** Building pattern memory without spawn verification means logging false "restart succeeded" events. Pattern confidence scores would be corrupted from day 1.
- **Recovery Intent Registry must be built before any watchdog is upgraded.** If you upgrade rc-sentry first without the intent registry, and rc-sentry and pod_monitor still fight, you've made the collision worse (smarter fighter, same conflict).
- **Pattern Memory requires crash fingerprinting to be useful.** Without normalization, two identical crashes with different timestamps produce different hashes — no pattern match ever fires.
- **Ollama Tier 3 depends on structured output parsing.** The rp-debug modelfile must be prompted to return `{ "action": "...", "confidence": 0.0–1.0, "reasoning": "..." }`. Raw prose output cannot drive safe automated action.

---

## MVP Definition

### Phase 1 (foundation — deploy first, everything else depends on it)

- [ ] **Spawn verification** (Strategy B: health poll after every restart trigger) — replaces the `.spawn().is_ok()` lie at all rc-sentry restart sites
- [ ] **Recovery intent registry** — sentinel file written before every recovery action, read before acting, expires after 2min
- [ ] **Crash pattern memory persistence** (debug-memory.json with fingerprinting) — survives rc-sentry and rc-agent restarts
- [ ] **Tier 0: false-positive suppression** — require 3 consecutive health misses before triggering recovery (eliminates the tasklist-empty false-positive class)

### Phase 2 (graduated response — replaces blind restart loop)

- [ ] **Tier 1 deterministic fix library** — stale socket cleanup, MAINTENANCE_MODE reason-clear, zombie process kill, shader cache clear
- [ ] **Graduated response Tiers 1–4** wired together with spawn verification and pattern memory
- [ ] **MAINTENANCE_MODE with reason** — extend file format to carry diagnostic context for staff

### Phase 3 (AI diagnosis — adds intelligence above deterministic baseline)

- [ ] **Tier 3 Ollama integration** — structured output parsing, safe-action whitelist, alert staff on escalation
- [ ] **Recovery telemetry to server** — RecoveryAttempted events visible in fleet dashboard
- [ ] **Context-aware pod_monitor** — server reads intent registry before WoL/restart decisions
- [ ] **james_watchdog.ps1 replacement** — apply same Tier 1–4 pipeline to James-side Windows service monitoring

### Defer to v17.2 or later

- [ ] **Confidence scoring** — useful after 2+ weeks of production data, premature before patterns accumulate
- [ ] **Silent failure detection (functional health beyond HTTP)** — high complexity; v17.0 idle-state checks partially address this; full integration is follow-on
- [ ] **WMI event-based monitoring for James** — reduces polling overhead on James's machine but adds complexity and potential anti-cheat risk on pods; evaluate after baseline ships

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Spawn verification (health poll) | HIGH — eliminates silent failures | LOW — 20 lines of retry loop | P1 |
| Recovery intent registry | HIGH — prevents watchdog conflicts | LOW — shared JSON file + TTL | P1 |
| Crash pattern memory (fingerprinting) | HIGH — enables learning | MEDIUM — normalization logic | P1 |
| Tier 0 false-positive suppression | HIGH — prevents flicker class bugs | LOW — counter in rc-sentry state | P1 |
| Tier 1 deterministic fix library | HIGH — covers 80% of real crashes | MEDIUM — catalog known fixes | P1 |
| Graduated response Tiers 1–4 | HIGH — replaces infinite restart loop | MEDIUM — state machine | P1 |
| MAINTENANCE_MODE with reason | HIGH — staff visibility without SSH | LOW — extend file schema | P1 |
| Ollama Tier 3 structured output | MEDIUM — covers unknown patterns | MEDIUM — prompt engineering + parser | P2 |
| Recovery telemetry to server | MEDIUM — Uday visibility | MEDIUM — new WS event type | P2 |
| Context-aware pod_monitor (server) | MEDIUM — prevents WoL conflicts | MEDIUM — new endpoint + reader | P2 |
| james_watchdog.ps1 replacement | MEDIUM — James-side parity | LOW — same pipeline, different target | P2 |
| Confidence scoring | LOW — useful only after data accumulates | LOW — counter fields in memory | P3 |
| Silent failure detection | HIGH value, HIGH cost — complex functional checks | HIGH | P3 |

---

## Windows-Specific Considerations

These issues are confirmed from standing rule incidents. They are not hypothetical.

### Session 0 Isolation (Critical)

Windows Services and processes running in non-interactive context (Session 0) cannot directly launch interactive GUI processes. `std::process::Command::new("rc-agent.exe").spawn()` from within rc-sentry running as a service will return `Ok` but the process may silently fail to start if it needs Session 1 GUI access.

**Confirmed workaround (from standing rules):** Spawn via the HTTP `/exec` endpoint on rc-agent's :8090 port (different creation context). For recovery when rc-agent is dead: use `schtasks /Run /TN StartRCAgent` — this was verified to work when called through the existing HKLM Run + bat file mechanism. Direct Rust `Command::new("schtasks").spawn()` in a service context was confirmed NOT reliable for this purpose.

**Verified recovery path from CLAUDE.md standing rules:**
1. Write to `rcagent-restart-sentinel.txt` (RCAGENT_SELF_RESTART mechanism)
2. `start-rcagent.bat` detects sentinel and relaunches
3. Verify via health endpoint poll (Strategy B above)

Never use: `cmd /C start`, `PowerShell Start-Process`, or `schtasks /Run` directly from Rust's `Command::new()` in non-interactive context — all confirmed to return `Ok` and silently fail.

### MAINTENANCE_MODE Sentinel Is a Silent Pod Killer

Once `C:\RacingPoint\MAINTENANCE_MODE` exists (after 3 restarts in 10min), ALL restarts stop permanently. No timeout, no auto-clear, no staff alert. **Before any restart debugging or recovery action, always check for and clear:** `MAINTENANCE_MODE`, `GRACEFUL_RELAUNCH`, `rcagent-restart-sentinel.txt`.

The Tier 4 extension adds a reason payload but keeps the same blocking semantics. Tier 4 MUST also trigger a staff WhatsApp alert — the current implementation is silent.

### No Anti-Cheat Process Inspection on Gaming Pods

On any pod that may be running F1 25 (EAC), iRacing, or LMU:
- NEVER use `tasklist`, `OpenProcess`, or WMI Win32_ProcessStartTrace to check if a process is running
- ALWAYS use HTTP health endpoint polling
- Recovery triggers must come from health endpoint failures, not from process-absence detection

For James's machine (no anti-cheat): process handle retention (Strategy A) and WMI event queries are acceptable.

### Explorer.exe Must Never Be Restarted on Pods

NVIDIA Surround triple-monitor setup is destroyed by explorer.exe restart and requires full reboot to restore. Any recovery action that could trigger explorer restart (e.g., shell process manipulation) is forbidden on pods.

---

## Existing Components to Modify (Not Rebuild)

| Component | File | Change |
|-----------|------|--------|
| rc-sentry health poll loop | `crates/rc-sentry/src/main.rs` | Add 3-miss Tier 0 counter; replace `restart()` call with graduated response state machine |
| rc-sentry restart function | `crates/rc-sentry/src/main.rs` | After spawn trigger: add Strategy B health poll verification (10s window, 500ms interval) |
| pod_monitor.rs | `crates/racecontrol/src/pod_monitor.rs` | Before WoL: read recovery-intent.json via rc-sentry /files endpoint; skip if active non-expired intent exists |
| debug-memory.json schema | new file in `C:\RacingPoint\` | Add: `fingerprint_hash`, `attempts`, `successes`, `last_outcome`, `tier_at_match` fields |
| MAINTENANCE_MODE file | written by rc-sentry Tier 4 | Change from empty file to JSON: `{ "reason", "pattern_id", "tier_exhausted", "context_log_tail" }` |
| james_watchdog.ps1 | `C:\Users\bono\racingpoint\comms-link\` or standalone | Replace with Rust binary or updated PS1 using same Tier 1–4 protocol |

---

## Sources

- [Erlang OTP Supervisor Behaviour](https://www.erlang.org/doc/system/sup_princ.html) — canonical reference for max_restarts, intensity/period, graduated restart strategy (HIGH confidence)
- [Windows Session 0 Isolation — Launching Interactive Processes from Services](https://learn.microsoft.com/en-us/archive/blogs/winsdk/launching-an-interactive-process-from-windows-service-in-windows-vista-and-later) — WTSQueryUserToken pattern, confirmed why direct spawn fails (HIGH confidence)
- [spawn-interactive-process gist — Windows Session workaround](https://gist.github.com/kenkit/21d134cead62b3380a25d924bd0906d7) — practical WTS token pattern (MEDIUM confidence)
- [Windows Process Liveness Check — DaniWeb](https://www.daniweb.com/programming/software-development/threads/453174/how-to-check-for-process-states-alive-dead-using-winapi) — confirmed process handle retention as authoritative liveness check (MEDIUM confidence)
- [Scheduling Agent Supervisor Pattern — GeeksforGeeks](https://www.geeksforgeeks.org/scheduling-agent-supervisor-pattern-system-design/) — single recovery authority coordination pattern (MEDIUM confidence)
- [AI-Driven Failure Detection and Self-Healing — IRJMETS 2025](https://www.irjmets.com/uploadedfiles/paper//issue_2_february_2025/67154/final/fin_irjmets1738995183.pdf) — 50%+ recovery time reduction with AI-driven pattern matching (MEDIUM confidence — single source)
- Standing Rules in CLAUDE.md — confirmed from 10+ production incidents: spawn verification failure, MAINTENANCE_MODE silent kill, Session 0 spawn failure, anti-cheat process inspection ban, watchdog conflict (HIGH confidence — first-party incident data)

---

*Feature research for: v17.1 Watchdog-to-AI Migration*
*Researched: 2026-03-25*
