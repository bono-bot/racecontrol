# Phase 1: Billing-Game Lifecycle - Research

**Researched:** 2026-03-15
**Domain:** Rust — rc-core billing/game_launcher wiring, rc-agent CoreToAgentMessage handling, lock screen state machine
**Confidence:** HIGH — all findings sourced directly from codebase inspection

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LIFE-01 | When billing session expires or is manually stopped, the running game is force-closed within 10 seconds | `StopGame` message already exists in protocol and is already sent on expiry (billing.rs:920) — but `BillingStopped` handler in rc-agent does NOT kill the game; only `SessionEnded` does |
| LIFE-02 | Staff cannot launch a game on a pod that has no active billing session | `game_launcher.rs:launch_game()` has NO billing check — only checks for double-launch (`GameState::Launching`) and catalog validity |
| LIFE-03 | After session ends, pod shows a brief session summary (15s) then returns to the idle lock screen automatically | `SessionEnded` handler shows `SessionSummary` state — but per SESS-03 comment in code, the blank timer is deliberately NOT armed. Must add 15s timer. |
| LIFE-04 | Rapid "launch game" requests are deduplicated — only one game launch per active billing session | `game_launcher.rs:launch_game()` only guards against `GameState::Launching`, not `GameState::Running` — double-launch when game is already Running is possible |
</phase_requirements>

---

## Summary

This phase wires the billing lifecycle to the game process lifecycle. The infrastructure is almost complete — `StopGame` exists in the protocol, billing expiry already sends it, and rc-agent already handles it in `SessionEnded`. The gaps are all at connection points between existing systems, not missing systems.

**The core gaps (verified in code):**

1. **`BillingStopped` path does not kill the game** (rc-agent main.rs:1047-1071). When `BillingStopped` fires, it calls `lock_screen.show_active_session("Session Complete!", 0, 0)` and kills the game — but this path is only used in edge cases (launch failure cancel). The real billing expiry goes through `StopGame` + `SessionEnded`. The `BillingStopped` handler currently DOES kill the game and clean up, but it sets the wrong lock screen state (`"Session Complete!"` in `ActiveSession`, not `SessionSummary`).

2. **No billing gate in `game_launcher.rs:launch_game()`** (game_launcher.rs:72-164). The function only checks catalog validity and whether already-launching. Adding `state.billing.active_timers.contains_key(pod_id)` check is a 4-line addition.

3. **`SessionSummary` → idle transition never fires** (main.rs:1111-1113). The code explicitly says `// SESS-03: results stay on screen until next session starts` and the `blank_timer` is NOT armed. LIFE-03 requires a 15-second automatic transition. The timer infrastructure (`blank_timer`, `blank_timer_armed`) already exists in the event loop — it just needs to be armed.

4. **Double-launch guard only blocks `Launching` state** (game_launcher.rs:93-100). Once the game reaches `Running`, a second `LaunchGame` request will pass the guard and start a second acs.exe process.

**Primary recommendation:** All four requirements are small targeted additions to existing code. No new modules, no new protocol messages. Plan 01-01 touches rc-core (billing gate + double-launch guard). Plan 01-02 touches rc-agent (15s idle timer after `SessionEnded`).

---

## Standard Stack

### Core (already in place — do NOT add)
| Component | Location | Purpose |
|-----------|----------|---------|
| `BillingManager` + `BillingTimer` | rc-core/src/billing.rs | Full timer lifecycle, expiry detection |
| `GameManager` + `GameTracker` | rc-core/src/game_launcher.rs | Launch FSM, state tracking |
| `CoreToAgentMessage::StopGame` | rc-common/src/protocol.rs:135 | Already exists, already sent on billing expiry |
| `CoreToAgentMessage::SessionEnded` | rc-common/src/protocol.rs:120-126 | Already exists, rc-agent handles it (kills game, shows summary) |
| `GameProcess::stop()` | rc-agent/src/game_process.rs:225-244 | `taskkill /PID /F` on Windows, clears PID file |
| `LockScreenState::SessionSummary` | rc-agent/src/lock_screen.rs:41-50 | Already exists, `show_session_summary()` method exists |
| `blank_timer` + `blank_timer_armed` | rc-agent/src/main.rs:555-557 | Timer infrastructure already in event loop, dormant |

---

## Architecture Patterns

### Current Billing Expiry Flow (what happens today)

```
billing.rs: tick_all_sessions()
  → timer.tick() → returns true (expired)
  → expired_sessions.push(...)
  → agent_senders.get(pod_id).send(CoreToAgentMessage::StopGame)     ← sent first
  → agent_senders.get(pod_id).send(CoreToAgentMessage::SessionEnded)  ← sent second
```

rc-agent handles `SessionEnded` (main.rs:1072-1113):
```
1. heartbeat_status.billing_active = false
2. overlay.deactivate()
3. Zero FFB (awaited)
4. lock_screen.show_session_summary(...)  ← SessionSummary state
5. game.stop()                            ← kills acs.exe
6. adapter.disconnect()
7. enforce_safe_state()
8. blank_timer NOT armed (SESS-03 comment)
```

rc-agent handles `StopGame` separately (main.rs lines NOT shown — need to check):
- `StopGame` message is received but there is no explicit `StopGame` match arm in the rc-agent `CoreToAgentMessage` match block shown. The agent only has `BillingStopped` and `SessionEnded` arms that kill the game. **Investigate: is StopGame handled in rc-agent?**

### Current `BillingStopped` Flow (manually stopped billing)

rc-agent main.rs:1047-1071 — **kills game and shows wrong lock screen state**:
```
1. overlay.deactivate()
2. Zero FFB (awaited)
3. lock_screen.show_active_session("Session Complete!", 0, 0)  ← WRONG: not SessionSummary
4. game.stop()
5. enforce_safe_state()
```

Note: `BillingStopped` does NOT set `heartbeat_status.billing_active = false`. That is only done in `SessionEnded`. This means the agent thinks billing is still active after `BillingStopped` fires.

### Current Manual Stop Flow in billing.rs

```rust
// billing.rs:1170-1191
let _ = sender.send(CoreToAgentMessage::StopGame).await;
let _ = sender.send(CoreToAgentMessage::HidePauseOverlay { ... }).await;
let _ = sender.send(CoreToAgentMessage::SessionEnded { ... }).await;
```

Manual stop DOES send `StopGame` + `SessionEnded`. The `SessionEnded` handler kills the game. So manual stop actually WORKS for LIFE-01.

### Confirmed Gaps vs Expected Behavior

| Gap | File:Line | What Happens Today | What Must Happen |
|-----|-----------|-------------------|-----------------|
| LIFE-02 billing gate | game_launcher.rs:72 | No check — launches regardless | Check `active_timers.contains_key(pod_id)`, return `Err("No active billing session")` |
| LIFE-03 auto-return to idle | main.rs:1111-1113 | `blank_timer` NOT armed after `SessionEnded` | Arm `blank_timer` for 15s in `SessionEnded` handler |
| LIFE-04 double-launch guard | game_launcher.rs:93-100 | Only blocks `GameState::Launching` | Also block `GameState::Running` |
| LIFE-01 status | billing.rs:920, main.rs:1072 | `StopGame` sent + `SessionEnded` kills game | Already works — but `BillingStopped` path needs `billing_active = false` |

### Anti-Patterns to Avoid

- **Do not add a new `StopGame` handler in rc-agent.** The `SessionEnded` handler already kills the game. `StopGame` is sent first as a belt-and-suspenders signal, but `SessionEnded` is what the agent acts on for cleanup.
- **Do not bypass `SessionEnded`.** The cleanup sequence (FFB → lock screen → game kill → enforce_safe_state) is carefully ordered in the existing handler. Replicate the same order for any new code paths.
- **Do not check `GameState::Launching` only.** The double-launch guard must check both `Launching` AND `Running`.
- **Do not forget `serde(default)`** when adding new fields to protocol messages for rolling deploy compatibility.
- **Do not call `game.stop()` before zeroing FFB.** The established safety ordering is: zero FFB first (awaited) → lock screen → game stop → enforce_safe_state.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Killing game process | Custom process kill | `GameProcess::stop()` in game_process.rs | Already handles child process + PID fallback + PID file cleanup |
| Timer countdown | New timer struct | `blank_timer` + `blank_timer_armed` already in main.rs event loop | Pattern already established for auto-blank |
| Billing presence check | New query | `state.billing.active_timers.read().await.contains_key(pod_id)` | Already used in pod_healer.rs:767 and ws/mod.rs:458 |
| Lock screen transition | New mechanism | `lock_screen.show_session_summary()` + arm `blank_timer` | All infrastructure exists |

---

## Common Pitfalls

### Pitfall 1: StopGame Is Not Handled Explicitly in rc-agent
**What goes wrong:** `CoreToAgentMessage::StopGame` is sent by rc-core but there is no explicit `StopGame` arm in the rc-agent `CoreToAgentMessage` match block (based on code inspection of main.rs). If this is true, the agent silently ignores `StopGame` and only reacts to `SessionEnded`.
**Why it happens:** `SessionEnded` was added later and does everything `StopGame` does plus more. `StopGame` may be treated as redundant.
**How to avoid:** Verify whether a `StopGame` match arm exists in rc-agent. If not, the current behavior still works because `SessionEnded` always follows `StopGame`. Do NOT add a `StopGame` handler that partially duplicates `SessionEnded` cleanup.
**Warning signs:** If `StopGame` is handled AND `SessionEnded` is also handled, game.stop() may be called twice — the second call will fail silently (no process to kill) but is harmless.

### Pitfall 2: billing_active Not Reset in BillingStopped
**What goes wrong:** `BillingStopped` handler (main.rs:1047) does NOT call `heartbeat_status.billing_active.store(false, ...)`. Only `SessionEnded` does. If `BillingStopped` fires without a subsequent `SessionEnded`, the agent permanently thinks billing is active.
**Why it happens:** `BillingStopped` is a legacy path. In normal operation, `SessionEnded` follows.
**How to avoid:** If `BillingStopped` is used as the sole signal for LIFE-01 (it shouldn't be — `SessionEnded` is the right path), add `billing_active = false` to the `BillingStopped` handler.

### Pitfall 3: blank_timer Armed While Billing Still Active
**What goes wrong:** The existing `blank_timer` firing code (main.rs:910-923) already guards against this: `if heartbeat_status.billing_active.load(...)`. If billing is still flagged active when the timer fires, the auto-blank is skipped.
**Why it happens:** `billing_active` must be set to `false` BEFORE arming the blank_timer, or the timer will fire and be silently skipped.
**How to avoid:** In the `SessionEnded` handler, `billing_active = false` is set at line 1079 BEFORE the blank_timer is armed. Keep this order.

### Pitfall 4: Double-Launch Race Window
**What goes wrong:** Between when `launch_game()` reads `GameState::Launching` and sets the new tracker, a concurrent request could slip through.
**Why it happens:** The guard acquires a read lock at line 93, drops it at line 100, then acquires a write lock at line 122. There is a window between read-unlock and write-lock.
**How to avoid:** For LIFE-04, expand the guard to also reject `GameState::Running`. The guard at line 93-100 is already correct structurally — just add `|| matches!(tracker.game_state, GameState::Running)` to the condition.

### Pitfall 5: SessionEnded With No Active Game
**What goes wrong:** `SessionEnded` calls `game.stop()` even if `game_process` is `None` (e.g., game never launched, or already stopped). The code already guards this: `if let Some(ref mut game) = game_process`.
**Why it happens:** N/A — code already handles it correctly.
**How to avoid:** No action needed. The existing pattern is correct.

---

## Code Examples

Verified patterns from codebase:

### Billing Gate Pattern (game_launcher.rs — to add)
```rust
// Source: game_launcher.rs:launch_game() — add after catalog validation, before double-launch check
{
    let timers = state.billing.active_timers.read().await;
    if !timers.contains_key(pod_id) {
        tracing::warn!("Launch rejected for pod {}: no active billing session", pod_id);
        return Err(format!("Pod {} has no active billing session", pod_id));
    }
}
```

### Expanded Double-Launch Guard (game_launcher.rs — to modify)
```rust
// Source: game_launcher.rs:93-100 — current code
{
    let games = state.game_launcher.active_games.read().await;
    if let Some(tracker) = games.get(pod_id) {
        if matches!(tracker.game_state, GameState::Launching) {
            return Err(format!("Pod {} already launching a game", pod_id));
        }
    }
}

// Modified: also block Running state
{
    let games = state.game_launcher.active_games.read().await;
    if let Some(tracker) = games.get(pod_id) {
        if matches!(tracker.game_state, GameState::Launching | GameState::Running) {
            return Err(format!("Pod {} already has a game active", pod_id));
        }
    }
}
```

### Auto-Blank Timer Arm (main.rs — to add in SessionEnded handler)
```rust
// Source: main.rs:910-923 — existing blank_timer fire handler (already correct)
// Add this line in the SessionEnded handler, after show_session_summary():
blank_timer.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(15));
blank_timer_armed = true;
```

### Existing blank_timer Fire Handler (main.rs:910-923 — already correct, no changes needed)
```rust
_ = &mut blank_timer, if blank_timer_armed => {
    blank_timer_armed = false;
    if heartbeat_status.billing_active.load(std::sync::atomic::Ordering::Relaxed) {
        tracing::info!("Skipping auto-blank — billing is active");
    } else {
        tracing::info!("Auto-blanking screen after session summary");
        lock_screen.show_blank_screen();
        // FFB + safe state cleanup ...
    }
}
```

### Existing SessionEnded Handler Structure (main.rs:1072 — reference for ordering)
```rust
CoreToAgentMessage::SessionEnded { ... } => {
    // 1. Set billing_active = false          ← MUST be before blank_timer arm
    heartbeat_status.billing_active.store(false, ...);
    // 2. Cancel crash recovery
    crash_recovery_armed = false;
    // 3. Deactivate overlay
    overlay.deactivate();
    // 4. Reset AC status tracking
    last_ac_status = None; ac_status_stable_since = None; launch_state = LaunchState::Idle;
    // 5. Zero FFB (awaited)
    { let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
    tokio::time::sleep(Duration::from_millis(500)).await;
    // 6. Show session summary
    lock_screen.show_session_summary(...);
    // 7. Kill game
    if let Some(ref mut game) = game_process { let _ = game.stop(); game_process = None; }
    // 8. Disconnect telemetry
    if let Some(ref mut adp) = adapter { adp.disconnect(); }
    // 9. Report FFB status
    // 10. enforce_safe_state
    // → ADD HERE: arm blank_timer for 15s
}
```

---

## State of the Art

| Old Approach | Current Approach | Status | Impact |
|--------------|------------------|--------|--------|
| `blank_timer` armed after SessionSummary | Timer NOT armed (SESS-03 locked it off) | SESS-03 deliberate decision | LIFE-03 REQUIRES re-enabling — but only for 15s, then show `ScreenBlanked` not `PinEntry` |
| No billing gate | No gate exists | Gap | LIFE-02 requires adding the gate |
| Double-launch blocks only Launching | Only `GameState::Launching` blocked | Partial | LIFE-04 requires blocking `Running` too |

**Note on SESS-03 vs LIFE-03:** SESS-03 says "results stay on screen indefinitely". LIFE-03 says "15 seconds then transitions to idle". These conflict. **Resolution per PROJECT.md:** Lock screen visual redesign is out of scope, but lifecycle state transitions are in scope. The 15s timer transitions to `ScreenBlanked` (blank black screen), NOT back to PIN entry. PIN entry only shows when a new auth token is created. This is consistent with the existing `blank_timer` handler that calls `lock_screen.show_blank_screen()`.

---

## Open Questions

1. **Is there an explicit `StopGame` handler in rc-agent?**
   - What we know: `StopGame` is sent, but the main.rs match block excerpt shows only `BillingStopped`, `BillingStarted`, `SessionEnded`, `LaunchGame` arms explicitly.
   - What's unclear: Whether the catch-all `_ => {}` absorbs `StopGame` silently or if there's an arm I didn't see (main.rs was read in sections).
   - Recommendation: The planner should add a task to verify by reading the full CoreToAgentMessage match block in main.rs. If no handler exists, rc-agent already relies on `SessionEnded` for game cleanup — which is correct behavior. If an explicit handler exists, note it to avoid double-kill.

2. **Should `BillingStopped` also kill the game?**
   - What we know: `BillingStopped` is currently sent only for launch failure cancellation (billing.rs:1240). Normal session end goes through `SessionEnded`. The `BillingStopped` handler does kill the game today.
   - What's unclear: Whether LIFE-01 requires `BillingStopped` to also have a clean summary path.
   - Recommendation: Out of scope for Phase 1. `BillingStopped` is an edge-case path; `SessionEnded` is the primary path. Phase 1 should focus on the `SessionEnded` path.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust `cargo test` with `#[cfg(test)]` modules |
| Config file | none (inline in source files) |
| Quick run command | `cargo test -p rc-common && cargo test -p rc-core && cargo test -p rc-agent` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LIFE-01 | Billing expiry sends StopGame + SessionEnded to agent | unit | `cargo test -p rc-core billing::tests::test_session_expiry_sends_stop_game` | ❌ Wave 0 |
| LIFE-02 | LaunchGame rejected when no active billing | unit | `cargo test -p rc-core game_launcher::tests::test_launch_rejected_no_billing` | ❌ Wave 0 |
| LIFE-02 | LaunchGame succeeds when billing is active | unit | `cargo test -p rc-core game_launcher::tests::test_launch_allowed_with_billing` | ❌ Wave 0 |
| LIFE-03 | Protocol: SessionEnded deserializes correctly | unit | `cargo test -p rc-common` (existing serialization tests) | ✅ (partial) |
| LIFE-04 | Double-launch blocked when game Running | unit | `cargo test -p rc-core game_launcher::tests::test_double_launch_blocked_running` | ❌ Wave 0 |
| LIFE-04 | Double-launch blocked when game Launching | unit | `cargo test -p rc-core game_launcher::tests::test_double_launch_blocked_launching` | ❌ Wave 0 (extends existing) |
| LIFE-01,02,03,04 | Full scenario: billing end → game killed → summary → idle | manual | Pod 8 end-to-end test | manual-only |

**Note:** LIFE-01 in rc-agent (game actually killed within 10s) requires a running pod with a real game process — this cannot be unit tested. The unit test validates the message is sent; the 10s SLA is verified manually on Pod 8.

### Sampling Rate
- **Per task commit:** `cargo test -p rc-core game_launcher::tests && cargo test -p rc-common`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-core/src/game_launcher.rs` — add `#[cfg(test)] mod tests` with billing gate and double-launch tests (3 test cases)
- [ ] `crates/rc-core/src/billing.rs` — add test for `tick_all_sessions` sending StopGame on expiry (1 test case, requires mock sender)
- [ ] No new test files needed — all tests inline in existing source files per project convention

*(Existing test infrastructure: 47 tests across 3 crates, all inline `#[cfg(test)]` modules. rc-common has serialization tests. rc-agent has game_process tests. rc-core has billing timer tests.)*

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-core/src/billing.rs` — full file read (session expiry, StopGame dispatch, manual stop flow)
- `crates/rc-core/src/game_launcher.rs` — full file read (launch_game, stop_game, handle_game_state_update, double-launch guard)
- `crates/rc-common/src/protocol.rs` — CoreToAgentMessage enum (lines 96-256)
- `crates/rc-agent/src/main.rs` — main event loop (lines 550-1113, CoreToAgentMessage handlers)
- `crates/rc-agent/src/game_process.rs` — full file read (GameProcess::stop, cleanup_orphaned_games)
- `crates/rc-agent/src/lock_screen.rs` — LockScreenState enum, show_session_summary, state transitions

### Secondary (MEDIUM confidence)
- `.planning/codebase/TESTING.md` — test conventions, existing test count
- `.planning/ac-launch/STATE.md` — accumulated decisions (billing-authoritative rc-core, serde(default) requirement)

### Tertiary (LOW confidence)
- None — all findings directly verified in source.

---

## Metadata

**Confidence breakdown:**
- Billing expiry flow (LIFE-01): HIGH — read actual billing.rs expiry code, confirmed StopGame + SessionEnded sent
- Launch gate absence (LIFE-02): HIGH — read entire launch_game() function, no billing check present
- blank_timer not armed (LIFE-03): HIGH — explicit SESS-03 comment in code + confirmed blank_timer_armed not set
- Double-launch guard gap (LIFE-04): HIGH — read guard at lines 93-100, only checks Launching state
- Agent StopGame handling: MEDIUM — read main.rs in sections; could not confirm presence or absence of explicit StopGame arm

**Research date:** 2026-03-15
**Valid until:** 2026-04-15 (code changes slowly between milestones)
