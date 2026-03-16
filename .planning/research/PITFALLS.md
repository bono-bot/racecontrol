# Pitfalls Research

**Domain:** Sim racing venue management — expanding a deterministic auto-fix bot (RC Bot Expansion v5.0)
**Researched:** 2026-03-16
**Confidence:** HIGH (derived from direct codebase reading: ai_debugger.rs, billing.rs, pod_healer.rs, CONCERNS.md, PROJECT.md, plus observed production bugs documented in MEMORY.md)

---

## Critical Pitfalls

### Pitfall 1: Fix Pattern Fires During Active Billing Session Without a Guard

**What goes wrong:**
A new bot pattern (e.g., "kill stale game process", "USB reset", "WebSocket reconnect force") fires while a customer is mid-session. The fix interrupts the game or clears state that the billing timer depends on. The session ends prematurely but the customer was already charged, or the billing timer loses its reference and continues ticking against a dead session.

**Why it happens:**
New fix patterns are wired into `try_auto_fix()` or `heal_pod()` via keyword matching against AI suggestion text. `PodStateSnapshot.billing_active` exists but there is no enforcement that callers check it before executing. The `fix_kill_stale_game()` implementation today does not consult `snapshot.billing_active`. Pattern memory replay (`DebugMemory::instant_fix()`) bypasses any call-site guards entirely.

**How to avoid:**
Every new fix function that can terminate a process, reset a device, or close a socket MUST gate on `!snapshot.billing_active` before executing. The guard must be inside the fix function itself, not only at the call site — because pattern memory replay returns a suggestion string and calls `try_auto_fix()` directly. Add a required test for every new fix: assert that `billing_active: true` causes the fix to return `None` (no-op).

**Warning signs:**
- New fix function added to `try_auto_fix()` without a matching test with `billing_active: true`.
- Pattern memory test covers round-trip but not context gate.
- Fix function signature does not receive `&PodStateSnapshot` (cannot inspect billing state).

**Phase to address:**
Every phase that adds a new bot pattern. Billing active guard must be in the acceptance criteria for each fix.

---

### Pitfall 2: Pattern Memory Replays a Fix That Was Safe in One Context but Dangerous in Another

**What goes wrong:**
`DebugMemory::instant_fix()` keys on `"{SimType}:{exit_code}"` only. It ignores billing state at the time of recording. A fix recorded between sessions (billing_active: false, safe to kill game) replays instantly during an active session (billing_active: true, dangerous to kill game). The replay happens before any billing guard logic because `analyze_crash()` returns early with the cached suggestion and `try_auto_fix()` is called with the replayed text.

**Why it happens:**
`pattern_key()` strips all context except simulator type and exit code. This was correct when the fix set was small and all safe. As the fix set expands to include destructive operations (game kill, USB reset, session end), context must become part of the key.

**How to avoid:**
Extend `pattern_key` to encode billing context: `"{SimType}:{exit_code}:billing={true/false}"`. This splits the memory pool so fixes recorded during idle never replay during active sessions. Alternatively, store `billing_active_when_recorded: bool` in `DebugIncident` and skip replay when the current context does not match.

**Warning signs:**
- `DebugIncident` struct has no billing context field.
- Test `test_record_and_instant_fix_round_trip` does not assert that a fix recorded with `billing_active: false` is suppressed when replayed with `billing_active: true`.
- `debug-memory.json` contains `fix_type: kill_stale_game` entries with no billing context stored.

**Phase to address:**
Phase adding new fix patterns to pattern memory. Must update `DebugIncident` schema before adding any destructive fix type.

---

### Pitfall 3: Billing Timer Orphans After Auto-Fix Kills the Game

**What goes wrong:**
Bot kills the game process (`fix_kill_stale_game`). The game normally sends `AcStatus::Off` via UDP/WebSocket to `handle_game_status_update()`, which calls `end_billing_session()`. But if the WebSocket is also degraded (which is why the crash happened), `AcStatus::Off` is delayed or dropped. The billing timer in `BillingManager::active_timers` keeps ticking via its background loop. The customer is charged for time after the game was already killed by the bot.

**Why it happens:**
`billing.rs` `BillingTimer::tick()` runs in a background task independent of game state. It has no awareness that the game was killed by an automated fix vs. a user-initiated quit. There is no "BotKill" session end path that explicitly calls `end_billing_session()` before the kill.

**How to avoid:**
Any fix that kills a game process must: (1) check if there is an active billing timer for this pod via `state.billing.active_timers`, (2) if yes, call `end_billing_session()` with an explicit reason before killing the process — do not rely on `AcStatus::Off` propagation as the end signal when the kill was initiated by the bot.

**Warning signs:**
- `fix_kill_stale_game()` does not call any billing end function.
- The billing end is expected to come from `handle_game_status_update(AcStatus::Off)` which depends on game process emitting an exit event — unreliable in a crash scenario.
- No test verifies: "if billing is active and game kill fix fires, billing session is ended atomically."

**Phase to address:**
Phase implementing crash/hang detection and game kill fixes.

---

### Pitfall 4: USB Reset Causes Windows to Re-Enumerate the Wheelbase at a Different Device Path

**What goes wrong:**
The bot detects a disconnected wheelbase (OpenFFBoard VID:0x1209 PID:0xFFB0) and issues a USB reset. Windows re-enumerates the device but the new device instance path includes a serial suffix that may differ from before. ConspitLink2.0 (which holds the wheelbase handle) silently reconnects to the stale handle and reports "connected" while FFB is actually dead. The fix is marked `success: true` but the customer is driving with no force feedback.

**Why it happens:**
Windows HID enumeration is not stable across device resets. Device instance paths include enumeration-order or serial-number suffixes that can shift when a USB port is reset. There is no API guarantee that the new path matches the old path after reset.

**How to avoid:**
After any USB reset action, re-enumerate HID devices fresh using VID/PID filter and confirm the wheelbase responds to FFB commands before marking the fix successful. Treat the fix as "pending verification" until a follow-up enumeration confirms. Never mark `success: true` immediately on `DeviceIoControl` returning `Ok`.

**Warning signs:**
- USB fix returns `AutoFixResult { success: true }` without a re-enumeration step.
- `wheelbase_connected: true` in `PodStateSnapshot` after a USB reset but ConspitLink telemetry shows zero FFB output.
- No post-reset verification logic exists in the fix function.

**Phase to address:**
Phase implementing USB hardware self-healing.

---

### Pitfall 5: WerFault Kill Fires During a Legitimate AC Save Dialog

**What goes wrong:**
New bot pattern kills `WerFault.exe` whenever a crash keyword appears in the AI suggestion. In some AC scenarios (end of race, replay save), Windows briefly presents a dialog that the pattern incorrectly identifies as a crash dialog. The bot kills it, corrupting the AC replay save state. Customer's best lap is not recorded because the replay could not be saved.

**Why it happens:**
`fix_kill_error_dialogs()` issues `taskkill /IM WerFault.exe /F` unconditionally. WerFault is the Windows Error Reporting process, but it can appear briefly for non-crash dialogs. The fix has no verification that the dialog is an actual crash report vs. a save or exit prompt. The existing implementation does not check `billing_active` or `driving_state`.

**How to avoid:**
Before killing WerFault, verify: (a) the parent process PID is one of the known game executables (`acs.exe`, `F1_25.exe`), AND (b) the game process is non-responsive (check via `WaitForSingleObject` with zero timeout or check if the game has produced telemetry recently). Only kill WerFault if both conditions hold. When `billing_active: true AND driving_state: Active`, add extra confirmation delay.

**Warning signs:**
- `fix_kill_error_dialogs()` does not consult `snapshot.billing_active` or `snapshot.driving_state`.
- Existing test `test_auto_fix_error_dialogs` uses `billing_active: false, driving_state: None` — no coverage of mid-drive state.

**Phase to address:**
Phase implementing crash/hang detection. Add driving-state guard to the WerFault kill.

---

### Pitfall 6: Telemetry Gap Bot Spams False Alerts When Pods Are Between Sessions

**What goes wrong:**
The telemetry gap bot watches UDP ports (9996 AC, 20777 F1) and alerts when no data arrives for N seconds. Between sessions the pod is on the lock screen — no game is running, no telemetry is expected. The bot alerts "telemetry dropped on Pod 3" during off-peak hours. Staff get flooded with false alerts, learn to ignore them, and miss a real telemetry failure during a session.

**Why it happens:**
A telemetry monitoring task spawned independently of the billing system has no visibility into whether a session is currently active. `billing_active` is only in `PodStateSnapshot` (agent-side) but a server-side monitoring task must query `AppState.billing.active_timers` to know whether telemetry silence is expected.

**How to avoid:**
The telemetry gap check must consult `state.billing.active_timers` before alerting. If no active timer exists for the pod, telemetry silence is expected — suppress the alert. Only alert when `billing_active: true AND elapsed_seconds > threshold AND no UDP data received for N seconds`.

**Warning signs:**
- Telemetry monitoring task spawned in `main.rs` without a reference to `billing.active_timers`.
- Alert log shows telemetry warnings at times when `billing_sessions` table has no active sessions.
- No test for "no alert when pod has no active timer."

**Phase to address:**
Phase implementing telemetry gap detection.

---

### Pitfall 7: Cloud Sync Overwrites Billing Write During Auto-Fix Window

**What goes wrong:**
Bot triggers session end early (`EndedEarly`). `end_billing_session()` debits the wallet and writes the session record. Within the next 30-second cloud sync window, `cloud_sync.rs` pulls wallet data from cloud where the wallet balance is still at the pre-session value (stale). The `upsert_wallet` CRDT merge uses `MAX(updated_at)`. If the cloud record has a newer `updated_at` (clock skew of even 1 second), the cloud's stale balance wins — effectively reversing the deduction. The customer was charged locally but the cloud wallet now shows the full pre-session credit.

**Why it happens:**
CONCERNS.md documents this as P1: "Wallet sync CRDT merge untested. updated_at can be clock-skewed between cloud and venue." Bot-triggered `EndedEarly` makes this worse because it happens asynchronously and unpredictably within the 30-second sync window. The billing write and the cloud sync poll race.

**How to avoid:**
After any bot-triggered session end, ensure the wallet write timestamp is guaranteed to beat the cloud record. One approach: write `updated_at = MAX(current_cloud_updated_at + 1s, Utc::now())` — requires knowing the cloud timestamp. Safer: add a "venue authoritative" flag or a minimum hold-off before the next cloud pull after a billing write. Long-term: migrate wallet sync from balance snapshots to transaction logs (additive, not overwriting).

**Warning signs:**
- After a bot-triggered EndedEarly, compare wallet balance on cloud vs. `billing_sessions` table — mismatch indicates a sync race was lost.
- Cloud sync interval is 30s; bot fixes can occur at any point in that window with no fence.
- `end_billing_session()` does not set any "hold off cloud sync" flag.

**Phase to address:**
Phase implementing billing edge case recovery. Must address wallet write fence before shipping bot-triggered session end.

---

### Pitfall 8: Multiple Bot Tasks Race to Fix the Same Pod Simultaneously

**What goes wrong:**
`pod_healer.rs` runs on its own interval. `pod_monitor.rs` runs independently. New crash bot tasks run on their own intervals. All three see the same degraded pod state simultaneously. `pod_healer` flags the pod for restart; `pod_monitor` triggers the restart; the crash bot kills the game. Three concurrent modifications result in: game killed by crash bot, rc-agent restarted by pod_monitor, healer tries to execute a heal action on an rc-agent that just restarted and whose port 8090 is temporarily unreachable. The pod ends up in a worse state than before, requiring staff intervention.

**Why it happens:**
`pod_healer.rs` already has partial coordination with `pod_monitor.rs` via `pod_watchdog_states` — it skips pods in recovery cycles (lines 151-176). But new bot tasks added to the system are not automatically wired into this coordination. CONCERNS.md identifies pod state race conditions as P1. Adding more concurrent tasks multiplies the risk.

**How to avoid:**
All new bot tasks MUST check `pod_watchdog_states` and `pod_deploy_states` before acting — the same pattern used in `heal_pod()`. Extract this check into a shared `is_pod_in_recovery(state, pod_id) -> bool` utility in `AppState` and require all bot tasks to call it. Make this a code review requirement for every new fix task.

**Warning signs:**
- A new bot fix function calls `execute_on_pod()` or sends a WebSocket command without first calling `is_pod_in_recovery()`.
- Two fix actions appear in the activity log for the same pod within a 5-second window from different subsystems.
- New bot task is spawned in `main.rs` without consulting `pod_watchdog_states`.

**Phase to address:**
Phase 1 — establish `is_pod_in_recovery()` utility before any new bot tasks are added. Enforce as a code review gate for all subsequent phases.

---

### Pitfall 9: Idle Detection Fires During AC Menu Navigation at Session Start

**What goes wrong:**
A new idle billing drift fix uses UDP silence or driving state absence as its idle signal. At session start, the customer spends 15-30 seconds in the AC car selection menu — no telemetry, no driving state. The bot flags this as "idle billing" and ends the session. Customer loses their session having never driven. Or billing is prematurely ended just before the customer enters the track.

**Why it happens:**
`BillingSessionStatus::WaitingForGame` was designed for the gap before `AcStatus::Live`. Once billing starts (Live received), there is another gap before the customer finishes menu navigation and UDP telemetry begins. A naive idle detector that measures "time since last UDP packet" or "time since last DrivingState::Active" will misfire during this gap.

**How to avoid:**
Idle detection must use `DrivingState` (from the sim protocol), not UDP packet presence alone. `DrivingState::Idle` means confirmed idle in-car. No driving state at all means menu — which must not trigger idle action. Only act when `DrivingState::Idle` has been confirmed for longer than the threshold AND billing status is `Active` (not `WaitingForGame`). Minimum threshold must exceed the longest expected menu navigation time (suggest 60 seconds minimum).

**Warning signs:**
- Idle detection logic uses `last_telemetry_received` timestamp as the primary idle signal.
- No test for the `WaitingForGame -> Active -> (menu navigation) -> no-UDP period` sequence.
- Threshold is set to the same 10-second idle value used for hardware detection (too short for menu navigation).

**Phase to address:**
Phase implementing billing edge case recovery.

---

### Pitfall 10: CRLF-Damaged Commands Sent via Remote Exec Look Successful but Do Nothing

**What goes wrong:**
A new bot fix constructs a multi-line batch command string in Rust (Unix `\n` endings) and posts it to the pod-agent `/exec` endpoint or WebSocket remote exec. `cmd.exe` misparses the multi-line string as one long invalid command. The command silently does nothing. `fix_result.success` is `true` because pod-agent returned HTTP 200 on dispatch — but the fix never executed. This is the same class of bug that caused the March 15 outage (CRLF-damaged bat files, MEMORY.md).

**Why it happens:**
Rust string literals use Unix line endings. When a fix constructs a multi-line command for remote execution, the developer naturally uses `\n`. `cmd.exe` splits on `\r\n`, so the multi-line command is treated as a single (invalid) line. The fix function returns success because success is measured at the HTTP layer, not at the command execution layer.

**How to avoid:**
All bot fix functions that construct remote command strings for `cmd.exe` execution MUST use `\r\n` separators, not `\n`. Single-line commands (e.g., `taskkill /IM acs.exe /F`) are safe. Multi-line scripts are the danger zone. Add a unit test for each remote command that asserts `\r\n` presence in the final command string.

**Warning signs:**
- Fix function builds a `String` with `\n` separators and passes it to `execute_on_pod()`.
- Fix returns `success: true` but the targeted process is still running (verified by follow-up query).
- No test validates line endings of remote command payloads.

**Phase to address:**
All phases. CRLF is cross-cutting. Every remote execution path needs a linting check.

---

### Pitfall 11: Lap Filter Bot Rejects Valid Laps Due to LAN Packet Loss Mid-Lap

**What goes wrong:**
The lap filter bot flags laps as invalid when it detects speed discontinuities (track cuts) or missing sector splits. But on this venue's LAN, UDP packets are dropped mid-lap (slow internet noted in PROJECT.md context). A missing telemetry packet looks identical to a speed discontinuity caused by a track cut. Valid hotlaps are rejected. Customers complain their best lap was not recorded. This undermines the leaderboard — the venue's core value proposition.

**Why it happens:**
AC UDP telemetry is fire-and-forget — no retransmission, no gap filling. A missing packet is indistinguishable from a game state discontinuity at the packet analysis layer. Naive validity computation from raw telemetry samples always produces false positives on a lossy network.

**How to avoid:**
Lap validity MUST use the game-reported `isValidLap` field (AC physics packet) as the primary signal — not bot analysis. Bot analysis is secondary: flag for staff review only, never auto-reject. Auto-reject only when the game itself reported the lap invalid, OR the lap time is physically impossible (e.g., <10% of track record). Preserve the lap row with a `review_required` flag rather than deleting it.

**Warning signs:**
- Lap filter implementation recomputes validity from raw `telemetry_samples` rather than reading the game's own validity flag.
- Invalid lap rate is higher on pods with weaker LAN signal (correlation indicates false positives).
- Lap filter deletes rows rather than flagging them with `review_required`.

**Phase to address:**
Phase implementing lap filter bot.

---

### Pitfall 12: Kiosk PIN Bot Locks Out Staff by Sharing a Failure Counter with Customer PINs

**What goes wrong:**
The PIN bot detects repeated PIN validation failures and increments a lockout counter. During a busy Saturday, multiple customers mistype PINs simultaneously across several pods. The bot's lockout counter fires for all pods simultaneously. If the lockout logic does not distinguish customer PINs from staff/debug PINs, staff cannot unlock pods manually. The employee daily rotating debug PIN is also blocked.

**Why it happens:**
`AUTH-01` (PROJECT.md) unified PIN auth infrastructure. A lockout mechanism that counts failures without separating customer vs. staff PIN type will penalize staff attempting to debug. The failure counter is keyed on pod or IP, not on PIN type.

**How to avoid:**
PIN failure counting MUST be scoped by PIN type: customer PINs and staff/debug PINs must have separate failure counters and separate lockout policies. The bot's lockout action must never apply to the employee debug PIN. Add `pin_type: customer | staff | debug` to all PIN validation attempts and filter bot lockout actions to `customer` type only.

**Warning signs:**
- PIN validation failure handler does not extract PIN type before incrementing failure counter.
- No test for "staff PIN succeeds after 5 consecutive customer PIN failures on the same pod."
- Lockout counter is keyed on pod_id only (not pod_id + pin_type).

**Phase to address:**
Phase implementing kiosk PIN bot.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Keyword matching on AI suggestion text for fix dispatch | Simple to add new patterns | Ambiguous; two patterns can fire on the same text; brittle against AI phrasing changes | MVP only — replace with structured fix type codes from AI |
| `success: true` based on `taskkill` exit code 0 | Simple result reporting | taskkill exits 0 even if the process was not found; fix appears successful when it did nothing | Never — verify process absence after kill |
| Single `PROTECTED_PROCESSES` list shared between agent and healer | DRY | Agent and healer have different contexts; healer correctly protects `acs.exe` (never kill game from server), agent should not kill `acs.exe` either but for different reasons | Extract to rc-common with context enum |
| `pattern_key = "{SimType}:{exit_code}"` (no billing context) | Smaller memory footprint | Fix replayed in wrong billing context — Pitfall 2 | Never — extend key to include billing state before adding destructive fix types |
| Remote exec success = HTTP 200 from pod-agent | Simple status check | Pod-agent returns 200 on command dispatch, not on command success; fix may have silently failed | Never for destructive fixes — add post-fix verification step |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Windows HID USB reset | Issue reset, mark connected immediately | Re-enumerate after reset, confirm VID/PID present, confirm FFB response before marking success |
| AC UDP telemetry gap | Treat packet absence as game absence | Distinguish "game running, no UDP" from "game not running"; use `game_pid` presence from snapshot |
| pod-agent `/exec` endpoint | Check HTTP 200 as confirmation of fix success | HTTP 200 = command dispatched; add follow-up `/exec` to verify the expected outcome |
| Cloud sync (billing write) | Assume billing DB write beats cloud pull | Cloud pulls every 30s regardless; bot-triggered billing writes need a fence or tombstone timestamp |
| WerFault.exe | Kill unconditionally when crash keyword seen | Verify parent process is a known game EXE and game is non-responsive before killing |
| WebSocket remote exec | Send multi-line batch with `\n` separators | Always use `\r\n` for cmd.exe compatibility in remote commands |
| AC `isValidLap` field | Recompute validity from raw telemetry | Trust game-reported validity as primary signal; bot analysis is secondary review flag only |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Healer scans all 8 pods every 2 minutes with 3+ remote commands per pod | Acceptable at 8 pods, low traffic | If healer interval shrinks or command count grows, 24+ remote calls per cycle accumulates latency | When healer interval drops below 60s or command count exceeds 5 per pod |
| Pattern memory file write on every `record_fix()` | Instant persistence | JSON serialization + file write on every successful fix; under rapid failure/fix cycles this hammers disk | Any incident that triggers the same fix 10+ times rapidly |
| AI escalation on every healer cycle when issue persists | Catches issues quickly | OpenRouter has rate limits; repeated escalation of same persistent issue burns API quota | Any persistent issue that does not resolve between healer cycles |
| New bot task spawned per pod (not shared) | Simple isolation | 8 tasks × 5 bot types = 40 background tasks competing for tokio executor | At 8 pods this is manageable; design shared tasks with per-pod dispatch from day one |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Bot fix commands constructed from AI suggestion text (untrusted input) | Prompt injection: adversarial AI response crafts a suggestion that triggers a dangerous fix keyword | Validate fix type against a fixed enum before executing; never execute free-form AI text as a command |
| `debug-memory.json` writable by any process on the pod | Attacker modifies file to inject a cached "fix" that triggers arbitrary commands on next replay | Lock file to rc-agent user only; validate JSON schema on load; reject entries with unknown fix_type values |
| `fix_kill_stale_game()` kills by executable name, not PID | Could theoretically kill a legitimate process with the same name that is not the game | Use `game_pid` from snapshot (specific PID) rather than name matching when a PID is available |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Bot silently ends session (EndedEarly) with no customer notification | Customer sees lock screen mid-drive with no explanation; perceives a crash | Bot-triggered session end must display a reason on the lock screen before terminating |
| Telemetry gap alerts fire during off-peak (no sessions) | Staff alert fatigue; real alerts ignored | Gate all bot alerts on session-active state |
| Lap flagged invalid by bot is permanently deleted | Customer's best lap irretrievably lost | Invalid flags must be soft — preserve lap with `review_required: true` for staff review |
| Idle detection ends session during menu navigation | Customer charged for session they never drove | Idle threshold must exceed longest expected menu navigation; use DrivingState not UDP silence |

## "Looks Done But Isn't" Checklist

- [ ] **Billing guard:** Every new fix function has a test with `billing_active: true` confirming it returns `None` (no-op) — grep tests for `billing_active: true`
- [ ] **Post-fix verification:** Fix functions confirm the targeted condition is resolved before returning `success: true` — not just that the command ran
- [ ] **CRLF in remote commands:** Multi-line command strings use `\r\n` — assert `cmd_string.contains("\r\n")` in tests
- [ ] **Pattern memory context:** `DebugIncident` includes billing state at recording time — verify schema field present
- [ ] **USB reconnect verified:** USB fix includes re-enumeration step before marking success — verify in fix implementation
- [ ] **Lap validity primary signal:** Lap filter uses game-reported `isValidLap` as primary signal — verify in filter logic
- [ ] **PIN type separation:** PIN failure counter is scoped to `customer` pin_type — verify lockout does not affect staff PINs
- [ ] **Concurrent fix coordination:** Every new bot task calls `is_pod_in_recovery()` before acting — verify in code review

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Fix fired during active session, session ended prematurely | HIGH — customer trust damage | Staff manually issue credit refund; insert compensating row in billing_sessions |
| Pattern memory replayed wrong fix in billing context | MEDIUM | Delete or correct the entry in `C:\RacingPoint\debug-memory.json` on the pod; re-run correct fix manually |
| USB reset left wheelbase at wrong device path | MEDIUM | Physical replug; ConspitLink2.0 watchdog detects and reconnects; verify FFB before next session |
| Cloud sync overwrote wallet balance | HIGH — financial data integrity | Compare `billing_sessions` table (source of truth) with `wallets` table; compute correct balance; apply manual correction SQL |
| Multiple tasks put pod in inconsistent state | MEDIUM | Staff dashboard "force pod reset"; rc-agent restart via deploy infrastructure |
| Lap wrongly flagged invalid | LOW if soft-flagged | Staff review queue; unset `review_required`; validate lap |
| Kiosk PIN bot locked out staff PIN | HIGH — operational blocker | Direct console access to racecontrol; manually clear lockout counter for staff PIN type |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Fix fires without billing guard | Every phase adding a fix pattern | Each fix has a `billing_active: true` test confirming no-op |
| Pattern memory replays fix in wrong context | Phase adding new destructive fix types | Test: fix recorded with `billing=false` is suppressed when replayed with `billing=true` |
| Billing timer orphans after game kill | Phase: crash/hang bot | Test: bot-triggered game kill also triggers billing end before kill |
| USB reset causes device path shift | Phase: USB hardware self-healing | Test: post-reset enumeration confirms device by VID/PID before marking success |
| WerFault false positive during save | Phase: crash/hang bot | Test: `driving_state=Active` suppresses WerFault kill |
| Telemetry gap false alert when idle | Phase: telemetry gap bot | Test: no alert when `billing_active: false` |
| Cloud sync overwrites billing write | Phase: billing edge cases | Test: wallet balance correct after bot-triggered EndedEarly + simulated cloud pull within 30s |
| Multiple tasks race on same pod | Every phase | Code review: every new bot task calls `is_pod_in_recovery()` |
| Idle detection fires during menu | Phase: billing edge cases | Test: WaitingForGame -> UDP silence -> no idle alert |
| CRLF in remote commands | All phases | Unit test: assert `\r\n` in multi-line command strings |
| Lap filter rejects valid laps | Phase: lap filter bot | Test: laps with UDP gaps but game-reported `isValidLap=true` are accepted |
| PIN bot locks out staff | Phase: kiosk PIN bot | Test: 5 customer PIN failures do not affect staff PIN success on same pod |

## Sources

- `crates/rc-agent/src/ai_debugger.rs` — `try_auto_fix()`, `fix_kill_stale_game()`, `fix_kill_error_dialogs()`, `DebugMemory::instant_fix()`, `PodStateSnapshot`, `PROTECTED_PROCESSES`
- `crates/racecontrol/src/pod_healer.rs` — `heal_pod()` coordination logic, billing active check (lines 223-230), watchdog state skip (lines 151-176), `PROTECTED_PROCESSES` (healer context)
- `crates/racecontrol/src/billing.rs` — `BillingTimer::tick()`, `handle_game_status_update()`, `WaitingForGameEntry`, multiplayer billing coordination, `end_billing_session()` flow
- `.planning/codebase/CONCERNS.md` — P0/P1 issues: no billing transaction wrapping, cloud sync CRDT untested, pod state races, 154 `.ok()` error silences, `billing.rs` zero test coverage
- `.planning/PROJECT.md` — v5.0 requirements, constraint list, known past bugs (CRLF, Session 0 GUI, Edge stacking, stale sockets)
- MEMORY.md — CRLF bug root cause (March 15 outage), ConspitLink watchdog pattern, billing rules, 10-second idle threshold

---
*Pitfalls research for: RC Bot Expansion (v5.0) — sim racing venue auto-fix bot expansion*
*Researched: 2026-03-16*
