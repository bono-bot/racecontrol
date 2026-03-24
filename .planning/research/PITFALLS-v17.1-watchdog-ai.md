# Pitfalls Research

**Domain:** AI-driven recovery migration — adding AI healer on top of existing Windows watchdog stack (v17.1)
**Researched:** 2026-03-25
**Confidence:** HIGH — all pitfalls sourced from actual incidents in this codebase and standing rules

---

## Critical Pitfalls

### Pitfall 1: spawn().is_ok() Does Not Mean the Target Started

**What goes wrong:**
`std::process::Command::spawn()` returns `Ok(Child)` if `CreateProcess` was accepted by the OS. This is NOT confirmation that the target binary started, is running, or will survive. rc-sentry's `restart_service()` hit this: it tested `cmd /C start`, `PowerShell Start-Process`, and `schtasks /Run` — all returned Ok, all silently failed. Pods stayed dead for days because `restarted=true` was logged.

**Why it happens:**
The AI recovery layer produces a restart decision, delegates to existing code, logs "restart Ok", and marks the incident resolved. The verification step is skipped because `.spawn().is_ok()` looks like sufficient confirmation.

**How to avoid:**
After every restart attempt, verify the target is actually alive before declaring success:
- Poll `/health` endpoint (the rc-sentry watchdog FSM already does this — use its state, not spawn return value)
- Check process via `tasklist /FI "IMAGENAME eq rc-agent.exe"` and parse for the actual image name substring (not the "no tasks" string)
- Wait for GRACEFUL_RELAUNCH sentinel cleanup — self_monitor writes it before exiting; the new process's startup clears it, confirming startup actually occurred

The AI decision loop must NOT advance to "resolved" state until `poll_health()` returns true after the restart window.

**Warning signs:**
- `restarted=true` in recovery log immediately followed by another crash detection on the same pod within 15s
- `restart_count` incrementing repeatedly but `ws_connected` never becoming true in fleet health
- All three launch methods appear to succeed in logs but pod stays offline

**Phase to address:** Phase 1 (rc-sentry AI healer core) — post-restart verification must be baked in before any AI decision path is wired. Never add AI decision-making without also adding the verification that closes the feedback loop.

---

### Pitfall 2: Non-Interactive Context Cannot Launch Session 1 Processes

**What goes wrong:**
rc-sentry runs as a background service with no desktop attachment. `cmd /C start`, `PowerShell Start-Process`, and `schtasks /Run` all succeed syntactically but fail to place the process in Session 1 where the GUI (lock screen, kiosk) lives. The AI healer pattern memory could record "PowerShell restart worked" from a test that ran interactively, then replay it from the service context where it silently fails.

**Why it happens:**
Pattern memory learns from successful fixes without recording the execution context. A fix verified by a human at a terminal runs in Session 1. The same fix replayed by rc-sentry at runtime runs in Session 0. Same command, different context, different result.

This was proven across four separate test iterations — cmd, PowerShell, START /B, schtasks — all returning Ok but rc-agent never appearing in Session 1.

**How to avoid:**
- Pattern memory (`debug-memory-sentry.json`) must tag each recorded fix with an `execution_context` field (`interactive` vs `service_context`)
- Only replay patterns tagged `service_context` when executing from within rc-sentry
- The ONLY proven path from a non-interactive context: the HTTP `/exec` endpoint (different process creation context), or the `RCWatchdog` Windows Service using `session::spawn_in_session1()` (already in `rc-watchdog/src/session.rs`)
- When migrating rc-sentry to AI healer, route all restart actions through the already-proven session spawn path — never invent a new spawn method

**Warning signs:**
- Restart logs show "spawned Ok" but `tasklist /V` shows rc-agent.exe in Session 0 (System) not Session 1
- AI healer records a fix as successful but the lock screen never appears on pod display
- Pattern memory hit count increases for a pattern but customer-visible symptoms persist

**Phase to address:** Phase 1 (rc-sentry AI healer) — session context must be a first-class parameter in the restart action. The spawn implementation must come from the tested `rc-watchdog` service code, not new code.

---

### Pitfall 3: MAINTENANCE_MODE Blocks All Recovery Silently Forever

**What goes wrong:**
Once `C:\RacingPoint\MAINTENANCE_MODE` is written (triggered after 3 restarts within 10 minutes), ALL restart attempts by ALL recovery systems stop permanently. There is no timeout, no auto-clear, no alert to staff. The AI healer will detect the pod as DOWN, generate diagnoses, issue restart commands — all of which silently skip because the sentinel check happens before restart logic. The AI produces increasingly sophisticated diagnoses for a pod that simply has a sentinel file blocking every attempt.

**Why it happens:**
The AI system generates decisions and logs them, but the execution layer silently short-circuits on the sentinel. The AI sees "restart issued" in its output but never learns the sentinel is blocking execution. The feedback loop is broken at the sentinel check.

**Confirmed incident (2026-03-24):** Pods 5, 6, 7 all had MAINTENANCE_MODE from the same crash storm. Pods were powered on, rc-sentry alive, but rc-agent permanently blocked. Same `last_seen` timestamp on all three was the only clue. Resolution: clear ALL three sentinels + `schtasks /Run /TN StartRCAgent` via rc-sentry exec.

**How to avoid:**
- When a restart is blocked by MAINTENANCE_MODE, emit `RecoveryAction::SkipMaintenanceMode` (already defined in `rc-common::recovery`) so the AI layer surfaces it to staff rather than treating it as a normal retry
- Add MAINTENANCE_MODE detection as a pre-check in the AI decision path: if the sentinel exists AND was written more than 30 minutes ago, emit `AlertStaff` with sentinel age, not `Restart`
- At AI healer startup, verify no pods have blocking sentinels before entering the monitoring loop
- Never record a pattern fix as successful when MAINTENANCE_MODE was active — those are not real fixes

**Warning signs:**
- Recovery log shows repeated `Restart` decisions for the same pod at 2-minute intervals but pod never reconnects
- Fleet health shows pod with a static `last_seen` timestamp while pod is pingable
- Multiple pods show the same `last_seen` timestamp (simultaneous sentinel activation from one crash storm)

**Phase to address:** Phase 1 (rc-sentry AI healer) — sentinel awareness is a blocker for the AI decision loop. Phase 2 (pod_monitor migration) must also inherit this, since pod_monitor WoL can wake a pod straight into a MAINTENANCE_MODE loop.

---

### Pitfall 4: Recovery Authority Conflicts Create Infinite Restart Loops

**What goes wrong:**
Four recovery systems currently act on pod liveness independently:
1. `self_monitor` (inside rc-agent) — relaunches on WS dead 5+ min or CLOSE_WAIT flood
2. `rc-sentry` watchdog — polls `/health` every 5s with 3-poll hysteresis
3. `pod_monitor` / `pod_healer` (on racecontrol server) — WoL on heartbeat timeout
4. `rc-watchdog` Windows service — tasklist poll every 5s

When rc-agent does a graceful self-restart, rc-sentry sees a 15-second health gap and may issue its own restart. The server sees the WebSocket drop and sends WoL. The Windows service sees tasklist empty and fires Session 1 spawn. Four restarts fire within 5 seconds, all trying to write to the same paths and bind the same ports.

The `ProcessOwnership` registry in `rc-common::recovery` exists to prevent this, but it only works if all four systems actually check it before acting. The `rc-watchdog` service and `pod_healer` are currently NOT wired through this registry.

**Confirmed incident:** self_monitor + rc-sentry watchdog + pod_monitor/WoL created an infinite restart loop that took 45 minutes to diagnose. The systems had no coordination.

**How to avoid:**
- Before Phase 1 ships, map all four authority paths against the `ProcessOwnership` registry and confirm each one calls `owner_of()` before acting
- The GRACEFUL_RELAUNCH sentinel is the existing coordination primitive — every recovery system must read this before acting
- WoL from pod_monitor must be gated: if rc-sentry issued a restart in the last 30 seconds (visible in `RECOVERY_LOG_POD`), skip WoL
- v17.1 milestone constraint (from PROJECT.md): "single recovery authority per machine — this milestone enforces it"

**Warning signs:**
- `restart_count` in `WatchdogCrashReport` exceeds 3 in under a minute for the same pod
- Recovery JSONL log shows interleaved entries from `rc_sentry` and `pod_healer` within seconds of each other for the same process
- Pod comes up briefly then immediately drops (second restart fired mid-reconnect)

**Phase to address:** Phase 1 (authority assignment) and Phase 2 (pod_monitor consolidation). Phase 1 must not ship without authority registry wired to all four paths.

---

### Pitfall 5: tasklist /FI Returns Empty on Pods — False-Positive Crashes

**What goes wrong:**
`tasklist /FI "IMAGENAME eq rc-agent.exe"` returns `INFO: No tasks are running...` when rc-agent is absent — but also returns empty or this string intermittently on pods even when rc-agent IS running (timing window between process start and tasklist visibility). The v17.0 browser watchdog was burned by the same pattern: `tasklist /FI` returned empty → watchdog declared Edge dead → killed and relaunched every 30s → 30-second screen flicker on all 8 pods.

The `rc-watchdog` service code (`output_contains_agent()`) is correctly conservative: if tasklist fails, it assumes running. But the AI healer pattern matching could treat a brief tasklist gap as a real crash and learn the wrong fix.

**Why it happens:**
Process list polling has a race window. When no crash signals are present (no panic, no exit code, no last phase), the pattern key is `"unknown"`. The AI may issue a restart for `"unknown"` patterns after Ollama returns "RESTART" for ambiguous symptoms — which it will 60% of the time regardless of context.

**How to avoid:**
- Use the 3-poll hysteresis already in `watchdog.rs` — never act on a single failed poll
- Do NOT add AI diagnosis for `"unknown"` pattern keys without secondary confirmation (log tail must show actual crash evidence)
- The `WatchdogState::Suspect(n)` → `WatchdogState::Crashed` FSM is the correct abstraction; the AI layer should only run AFTER the FSM reaches Crashed, not at Suspect
- Health check response content check (`text.contains("200")`) must be preserved — do not simplify to connection success only

**Warning signs:**
- `pattern_key: "unknown"` with `hit_count > 2` in `debug-memory-sentry.json` — the AI is learning noise
- Pods restart during active billing sessions
- Recovery log shows sub-15-second restart intervals (faster than the 3-poll × 5s = 15s hysteresis window)

**Phase to address:** Phase 1 (rc-sentry healer). Ollama consultation must be gated to only fire when `CrashContext` has at least one non-None field. A completely empty crash context must wait for the next cycle, not trigger AI diagnosis.

---

### Pitfall 6: Pattern Memory Learns Wrong Fixes From Server-Down Restarts

**What goes wrong:**
When racecontrol goes down, rc-agent loses its WebSocket connection and `self_monitor` triggers a relaunch (WS dead 5+ minutes). The crash context has no panic, no exit code, and last_phase is the normal running state. The AI healer records this as a successful fix if rc-agent comes back up — but the real cause (server down) is not in the pattern. Next time the server goes down, the AI issues a restart that does nothing, records it as a fix, and pattern memory accumulates false confidence.

This is also the trigger for MAINTENANCE_MODE if the server stays down long enough to cause 3+ restarts in 10 minutes (Pitfall 3).

**Why it happens:**
Pattern memory learns correlation (restart happened → service came back), not causation. Server-down events look identical to rc-agent crashes from a pod-local observer. The `exit:0` or `unknown` pattern key captures both real crashes and clean restarts caused by external events.

**How to avoid:**
- Before recording a fix as successful, verify the recovery was genuine: did the pod reconnect to the server within 60 seconds of restart, OR did rc-agent restart into a server-still-down state?
- Add `server_reachable: bool` check as part of pattern recording — if server is still unreachable after restart, tag the fix as `inconclusive` not `resolved`
- Server-down scenarios must suppress MAINTENANCE_MODE escalation: rc-sentry should check `racecontrol:8080` reachability before counting server-disconnect-caused restarts toward the maintenance threshold
- The `FailureState` model in `james_monitor.rs` (persists across cycles, distinguishes "still failing" from "recovered") is the right pattern to adopt here

**Warning signs:**
- `debug-memory-sentry.json` has entries with `pattern_key: "unknown"` or `"exit:0"` with high `hit_count`
- After planned server maintenance, all 8 pods show `restart_count: 2-3` in crash reports
- MAINTENANCE_MODE appears on multiple pods simultaneously (same crash storm from server disconnect)

**Phase to address:** Phase 1 (pattern memory design), Phase 2 (pod_monitor integration). Server-down awareness must be in the pattern recording design from day one.

---

### Pitfall 7: AI Diagnosis Without Sufficient Log Context Produces Harmful Actions

**What goes wrong:**
The Ollama query in `self_monitor.rs` sends a short prompt with just the issue symptom string. If the root cause is not in local logs (e.g. network partition, USB hardware fault, game crash that killed the process), the LLM produces a plausible-sounding but wrong diagnosis. The AI applies a fix that does nothing, logs "AI recommended RESTART", and the pattern is recorded as if the restart resolved the issue.

The `james_monitor.rs` pattern is better: it tails log files, collects failure count, and includes all available context. But it still fails when `log_path` is `None` (server-side services: `racecontrol`, `kiosk`, `dashboard` all have `log_path: None`).

**Why it happens:**
LLMs are confidently wrong with minimal context. An AI that says "RESTART" is correct 60% of the time (most transient failures resolve on restart), so pattern memory accumulates "AI was right" records even when the AI had no actual insight. The qwen2.5:3b model on pods (actually qwen3:0.6b deployed per v8.0) is small and optimized for binary classification, not diagnosis.

**How to avoid:**
- Minimum context requirement before Ollama query: the prompt must include at least one of: crash panic message, non-zero exit code, log tail with ERROR/WARN lines, or explicit symptom count > 1
- For services with `log_path: None`, do NOT invoke Ollama — fall back to deterministic Tier 1 fixes only, then alert
- Scope the model's role: binary gate (RESTART/OK) only, not root cause diagnosis. The model is not capable of reliable root cause analysis for this domain
- The 5-strike auto-restart fallback in `self_monitor.rs` must be preserved in the AI migration — Ollama unavailability must not block recovery

**Warning signs:**
- Pattern memory has entries with generic `detail` fields ("restart resolved the issue") rather than specific ones ("cleared stale socket on :8090 before restart")
- The same crash pattern appears with different `fix_type` values (the AI is guessing differently each time)
- Ollama query latency exceeds 15 seconds (the current 30s reqwest timeout helps, but the blocking call in the async context holds the self-monitor loop)

**Phase to address:** Phase 1 (rc-sentry AI healer), Phase 3 (James watchdog migration). Minimum context requirements must be specified in phase requirements, not left to implementation judgment.

---

### Pitfall 8: Deploying AI Healer Bat Files With BOM, Parentheses, or /dev/null

**What goes wrong:**
The AI healer involves deploying or modifying bat files (start-rcsentry.bat, watchdog-rcsentry-ai.bat, start-rcagent.bat). Previous deployments needed 4 attempts due to: UTF-8 BOM breaking cmd.exe, parentheses in if/else blocks causing silent failures, `/dev/null` not existing on Windows (use `nul`), and `timeout` command failing in non-interactive SSH context (use `ping -n N 127.0.0.1`).

**Why it happens:**
Claude Code's Write tool adds UTF-8 BOM. Bash heredoc on Windows needs BOM stripped. Standard shell patterns (`/dev/null`, `timeout`) don't map to Windows equivalents. The non-interactive session restriction (Pitfall 2) affects bat file behavior in ways that only surface at runtime.

**How to avoid:**
- NEVER use the Write tool to create bat files directly — use bash heredoc + `sed 's/$/\r/'` to add CRLF
- NEVER use parentheses in if/else blocks in bat files — use `goto` labels (standing rule)
- Replace `/dev/null` with `nul`, `timeout /T N` with `ping -n N 127.0.0.1 >nul`, and `title RCAGENT` with nothing (title command breaks non-interactive contexts)
- Test every new bat file with `cmd /c` before deploying
- Deploy to Pod 8 canary first and verify with `tasklist` before fleet deploy

**Warning signs:**
- rc-agent does not appear in tasklist after bat deploy
- rc-sentry exec of the bat file returns immediately with no visible effect
- Watchdog service shows "started Ok" but the target binary is not running within 10 seconds

**Phase to address:** Every phase that touches bat files — explicitly call out bat file constraints in phase requirements. The standing rule exists but has been violated in 4 out of 4 bat file deployments that needed multiple attempts.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Record every restart as a "fix" | Simple bookkeeping | Pattern memory fills with false positives; AI replays wrong fixes with high confidence | Never |
| Single debug-memory JSON shared between rc-sentry and rc-agent | One file to check | Both writers collide; already separated (`debug-memory-sentry.json` vs `debug-memory.json`) — names must stay distinct | Acceptable as-is, but do not merge them |
| `spawn().is_ok()` as restart confirmation | Zero extra code | Pod stays dead while recovery system thinks it succeeded | Never |
| Ollama RESTART decision on empty crash context | "Restart never hurts" | Pattern memory learns noise; hardware faults cause endless loops | Never — require at least one non-None CrashContext field |
| Running old dumb watchdog in parallel with AI healer permanently | Clean rollback | Authority conflicts if both act independently | Acceptable as transition only — dumb watchdog should be disabled after 48h AI healer track record |
| Skipping MAINTENANCE_MODE detection in new AI paths | Faster implementation | AI loop spins forever generating useless diagnoses | Never |

---

## Integration Gotchas

Common mistakes when wiring AI recovery into this specific system.

| Integration Point | Common Mistake | Correct Approach |
|-------------------|----------------|------------------|
| rc-sentry + rc-watchdog service | Both check tasklist independently and both restart rc-agent | rc-sentry owns rc-agent recovery (register in `ProcessOwnership`); rc-watchdog defers to rc-sentry, only fires if rc-sentry itself is absent |
| pod_healer + rc-sentry | pod_healer sends WoL when pod disappears; rc-sentry already restarted rc-agent; WoL causes full reboot interrupting active session | Before WoL, check `RECOVERY_LOG_POD` for rc-sentry restart decision within 60s; if present, skip WoL |
| self_monitor + rc-sentry | GRACEFUL_RELAUNCH sentinel signals rc-sentry to skip crash escalation; if sentinel write fails, rc-sentry treats graceful restart as crash | Verify sentinel was written before calling `std::process::exit(0)` in `relaunch_self()`; if write fails, do NOT exit |
| Ollama on pods (qwen3:0.6b) | AI healer assumes Ollama always available | Ollama may be killed by process guard or anti-cheat safe mode (v15.0) | All Ollama paths must have a deterministic Tier 1 fallback |
| Pattern memory + OTA deploy | OTA deploy kills and restarts rc-agent; looks like crash to AI healer; wrong fix recorded | OTA deploys must set `kill_watchdog_restart` flag (v22.0 Phase 178) BEFORE issuing the kill; AI healer must honor this kill switch |
| debug-memory-sentry.json + sentry-flags.json | Both written by different processes to the same dir; corrupt JSON silently falls to empty default | Both files already use atomic write (tmp + rename); do not change to direct write |
| james_monitor + rc-sentry AI | james_monitor checks racecontrol health; rc-sentry checks rc-agent health; both on the same machine should not restart the same target | Each authority's scope is documented in `RecoveryAuthority` enum — JamesMonitor owns James-local services; RcSentry owns rc-agent on each pod |

---

## Performance Traps

Patterns that work at current scale but break under failure conditions.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Blocking Ollama query in sync watchdog thread | Watchdog loop stalls 15-30s; health polling stops; pods falsely detected as crashed during the stall | Never add blocking Ollama calls to rc-sentry's sync watchdog thread (pure std, no async runtime); use the spawn-thread pattern from `self_monitor.rs` | Breaks immediately on first Ollama query in rc-sentry |
| Pattern memory pruning by hit_count only | Serious crash patterns with 1 occurrence get pruned during a crash storm | Add `critical` flag for patterns with panic message present; prune low-hit generic patterns first | Breaks when crash storm fills 50 slots with noise |
| Parallel WoL to all pods on server restart | 8 simultaneous pod reboots; sessions lost; reboots conflict with AI healer restarts | `EscalatingBackoff` in rc-common already rate-limits; ensure AI healer decisions for multiple pods are serialized | Breaks during planned server maintenance if not suppressed |
| Building new reqwest client per health check | Acceptable at 2-min intervals (james_monitor) | Do not reduce james_monitor poll interval below 60s without switching to connection pool | Not a concern at current rates |

---

## "Looks Done But Isn't" Checklist

Things that appear complete in this migration but are missing critical pieces.

- [ ] **Restart action:** Verify `poll_health()` returns true within `RESTART_GRACE_SECS` window after spawn — not just that spawn returned Ok
- [ ] **Pattern memory recording:** Verify `CrashContext` has at least one non-None field before recording a fix — not just that a restart happened
- [ ] **Recovery authority registration:** Verify every new AI healer path calls `ProcessOwnership::register()` at startup — error if already owned
- [ ] **MAINTENANCE_MODE awareness:** Verify AI healer checks the sentinel before every restart attempt — not just once at startup
- [ ] **GRACEFUL_RELAUNCH coordination:** Verify AI healer reads `GRACEFUL_RELAUNCH` before counting a health gap as a crash
- [ ] **Server-down distinguisher:** Verify pattern keys include `server_reachable` flag so server-down restarts are not conflated with rc-agent crashes
- [ ] **Ollama unavailability path:** Verify deterministic Tier 1 fallback fires when Ollama returns Err or times out
- [ ] **Kill switch honor:** Verify `kill_watchdog_restart` flag from `sentry-flags.json` is checked in every restart-issuing code path
- [ ] **Bat file validation:** Verify every new or modified bat file passes `cmd /c` test before deploy to fleet

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Pattern memory corrupted with false positives | LOW | Delete `C:\RacingPoint\debug-memory-sentry.json` and `C:\RacingPoint\debug-memory.json` on affected pod; both reload as empty defaults on next startup |
| All pods in MAINTENANCE_MODE simultaneously | MEDIUM | Via rc-sentry exec on each pod: `del C:\RacingPoint\MAINTENANCE_MODE C:\RacingPoint\GRACEFUL_RELAUNCH C:\RacingPoint\rcagent-restart-sentinel.txt`; then `schtasks /Run /TN StartRCAgent`; always clear ALL three sentinels together |
| Multiple recovery systems fighting | MEDIUM | Stop in order: (1) `sc stop RCWatchdog`, (2) write `MAINTENANCE_MODE` to pause rc-sentry restart loop, (3) manually restart rc-agent once, (4) restart services in correct order; investigate authority coordination gap before re-enabling |
| AI healer stuck in diagnosis loop (Ollama unreachable) | LOW | The 5-strike fallback auto-escalates without Ollama; no manual intervention needed if fallback is preserved |
| pod_healer + AI healer sending duplicate WoL | MEDIUM | Check `RECOVERY_LOG_POD` on server for interleaved decisions; disable pod_healer WoL in `racecontrol.toml`; investigate authority coordination gap before re-enabling |
| Wrong fix replayed from pattern memory | LOW | Delete specific pattern entry from `debug-memory-sentry.json` (JSON array — edit directly); the file uses atomic write so edits are safe |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| spawn().is_ok() false confirmation | Phase 1: rc-sentry AI healer core | Post-restart: `poll_health()` must return true within `RESTART_GRACE_SECS`; test by killing rc-agent manually |
| Non-interactive Session 1 spawn failure | Phase 1: rc-sentry AI healer core | Spawn path must use `session::spawn_in_session1()`; verify with `tasklist /V` showing Session 1, not Session 0 |
| MAINTENANCE_MODE silent block | Phase 1: rc-sentry AI healer core | Manually trigger MAINTENANCE_MODE; verify AI healer emits `AlertStaff` with sentinel age, not `Restart` |
| Recovery authority conflicts | Phase 1 (registry wiring) + Phase 2 (pod_monitor) | Run all four recovery systems simultaneously; kill rc-agent; verify only ONE restart fires per pod |
| tasklist false-positive (v17.0 flicker pattern) | Phase 1: rc-sentry AI healer core | Only trigger AI on CrashContext with non-None fields; verify with empty crash context that Ollama is NOT called |
| Pattern memory learns server-down as crashes | Phase 1 (recording design) + Phase 2 | Force server restart; verify pods do not accumulate crash patterns for server-disconnect events |
| AI diagnosis on empty context | Phase 1 + Phase 3 (James monitor) | Unit test: `CrashContext { all None }` must not reach Ollama; verify Tier 1 fallback fires when Ollama is offline |
| OTA deploy triggers AI restart loop | Phase 1 (honor kill switch) | Run OTA deploy with AI healer active; verify `kill_watchdog_restart` flag prevents any AI restart during binary swap |
| Bat file deployment failures | Every phase touching bat files | Test every new bat file with `cmd /c` before fleet deploy; verify on Pod 8 canary first |

---

## Sources

- Standing rules in `crates/racecontrol/CLAUDE.md` — `.spawn().is_ok()`, non-interactive context, MAINTENANCE_MODE, recovery conflicts, tasklist false positive, cmd.exe quoting, bat file syntax
- Codebase: `crates/rc-sentry/src/watchdog.rs` — actual FSM and hysteresis implementation; `WatchdogState` enum
- Codebase: `crates/rc-agent/src/self_monitor.rs` — GRACEFUL_RELAUNCH sentinel, `relaunch_self()` PowerShell path, CLOSE_WAIT strike logic, WS dead threshold
- Codebase: `crates/rc-watchdog/src/james_monitor.rs` — graduated action pattern (count 1/2/3/4+), `FailureState` persistence, `collect_symptoms()`, Ollama diagnosis gating
- Codebase: `crates/rc-watchdog/src/service.rs` — session 1 spawn, restart grace window, `output_contains_agent()` conservative logic
- Codebase: `crates/rc-common/src/recovery.rs` — `ProcessOwnership` registry, `RecoveryAuthority` enum, `RecoveryAction` enum, `RecoveryLogger` JSONL writer
- Codebase: `crates/rc-sentry/src/debug_memory.rs` — pattern key derivation, `MAX_INCIDENTS` pruning, atomic write pattern
- Memory file (MEMORY.md) — 2026-03-24 audit: Pods 5/6/7 MAINTENANCE_MODE simultaneous incident; v17.0 browser watchdog 30s flicker incident; orphan PowerShell leak from self_monitor; 4 bat deploy attempts needed
- PROJECT.md — v17.1 milestone constraints: "single recovery authority per machine"; pattern memory persistence requirement; v17.0 incident trigger (browser watchdog flicker)

---
*Pitfalls research for: AI-driven recovery migration (v17.1 Watchdog-to-AI Migration)*
*Researched: 2026-03-25*
