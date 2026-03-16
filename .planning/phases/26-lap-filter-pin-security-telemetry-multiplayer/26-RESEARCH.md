# Phase 26: Lap Filter, PIN Security, Telemetry + Multiplayer - Research

**Researched:** 2026-03-16
**Domain:** Rust/Axum — sim adapter wiring, SQLite schema extension, failure_monitor pattern, email alert gating, bot_coordinator stub promotion
**Confidence:** HIGH

---

## Summary

Phase 26 completes the v5.0 RC Bot Expansion by wiring seven requirements across four
orthogonal domains. All infrastructure is already in place from Phases 23-25: the protocol
variants exist in rc-common, the stub handlers exist in ws/mod.rs and bot_coordinator.rs,
and the email alerting infrastructure (EmailAlerter) is already wired into AppState.

**LAP domain (LAP-01, LAP-02, LAP-03):** The AC adapter already populates `valid:
is_valid != 0` and the F1 adapter already carries `current_lap_invalid`. `persist_lap()`
already reads `lap.valid` and gates on it. The gap is: (a) LAP-02 needs a per-track
minimum lap time configurable in the track catalog, (b) LAP-03 needs a `session_type`
field on LapData (currently absent from the struct), and (c) persist_lap needs a
`review_required` flag in the DB insert path for minimum-lap-time violations.

**PIN domain (PIN-01, PIN-02):** The current `validate_pin()` checks the employee debug
PIN first (early return), then the customer token path. No failure counter of any kind
exists yet. PIN-01 requires adding separate in-memory counters (customer and staff), and
PIN-02 requires the staff counter to be in a different namespace so customer exhaustion
never blocks the employee path. The per-pod counter must live in AppState (server-side),
since the agent only forwards raw PIN strings.

**TELEM-01:** `AgentMessage::TelemetryGap` is already routed to
`bot_coordinator::handle_telemetry_gap()` in ws/mod.rs. The stub logs only. The
requirement adds: check that the pod's game state is `AcStatus::Live` (not menu/idle)
before firing the email. The send site (failure_monitor or billing_guard sending the
TelemetryGap message) does not yet exist in rc-agent — that also needs to be added.

**MULTI-01:** `AgentMessage::MultiplayerFailure` is already routed in ws/mod.rs but only
logs. The requirement adds: a new `handle_multiplayer_failure()` function in
bot_coordinator.rs that sends `CoreToAgentMessage::ShowLockScreen`, calls
`end_billing_session_public()`, and logs the event — in that exact order.

**Primary recommendation:** Follow the same TDD wave structure as Phases 24-25. Wave 0
adds RED test stubs for all seven requirements, Wave 1 implements the production code,
and the wiring wave connects everything.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LAP-01 | `is_valid` flag wired from AC and F1 25 sim adapters into `persist_lap` | AC adapter already sets `valid: is_valid != 0`; F1 adapter already sets `valid: !self.current_lap_invalid`; persist_lap already gates on `lap.valid` — gap is LAP-02 review_required field and LAP-03 session_type |
| LAP-02 | Per-track minimum lap time configurable in track catalog (Monza, Silverstone, Spa as initial set) | catalog.rs has `TrackEntry` struct with id/name/category/country — needs `min_lap_time_ms: Option<u32>` field; persist_lap checks this against `lap.lap_time_ms` |
| LAP-03 | Laps classified as hotlap vs practice based on session type reported by sim adapter | `SessionType` enum already has Practice/Qualifying/Race/Hotlap variants; `LapData` struct lacks `session_type` field — must add |
| PIN-01 | Customer and staff PIN failure counters tracked separately | No counters exist yet; server-side HashMap in AppState is the correct location; per-pod customer counter keyed on pod_id |
| PIN-02 | Staff PIN is never locked out by customer PIN failure accumulation | Employee debug PIN path in validate_pin() is a separate early-return — its counter must live in a different HashMap; customer exhaustion must never touch the staff path |
| TELEM-01 | Bot detects UDP silence >60s during active billing + Live game state and alerts staff via email | handle_telemetry_gap() stub exists in bot_coordinator.rs; needs game-state guard (AcStatus::Live check via pod state); send site (rc-agent) needs TelemetryGap emission in failure_monitor.rs |
| MULTI-01 | Bot detects AC multiplayer server disconnect mid-race and triggers lock screen → end billing → log event | MultiplayerFailure arm in ws/mod.rs is a log stub; needs handle_multiplayer_failure() in bot_coordinator.rs; rc-agent needs multiplayer disconnect detection sending MultiplayerFailure |
</phase_requirements>

---

## Standard Stack

### Core (all pre-existing — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rc-common | workspace | Protocol types, PodFailureReason, AgentMessage | Cross-crate dependency foundation |
| sqlx | workspace | SQLite queries for lap insert + min_lap_time lookup | Already in use everywhere |
| tokio | workspace | Async runtime, watch channel for FailureMonitorState | Already in use |
| serde | workspace | Serialization of AgentMessage variants | Already in use |

No new Cargo.toml dependencies required for this phase.

---

## Architecture Patterns

### Recommended Project Structure (Phase 26 additions)

```
crates/racecontrol/src/
├── bot_coordinator.rs      # promote stubs: handle_telemetry_gap + handle_multiplayer_failure
├── lap_tracker.rs          # add review_required logic in persist_lap
├── catalog.rs              # add min_lap_time_ms to TrackEntry + helper fn
├── auth/mod.rs             # add per-pod customer + staff failure counter HashMaps

crates/rc-agent/src/
├── failure_monitor.rs      # add TelemetryGap send + MULTI-01 AC disconnect detection
├── sims/assetto_corsa.rs   # verify IS_VALID_LAP offset correctness (audit only)
```

No new files required — all changes are additive in existing files.

### Pattern 1: Lap Minimum Time Floor (LAP-02)

**What:** Track catalog holds `min_lap_time_ms: Option<u32>`. `persist_lap()` queries this
after inserting the lap row, and sets `review_required = 1` in the DB if
`lap_time_ms < min_lap_time_ms`. Does NOT delete or skip the lap. The lap is stored with
`valid = true` (game says OK) but `review_required = true` (below track floor).

**When to use:** Only for laps that pass game validity but are statistically impossible.
The existing `suspect` flag (< 20s, sector sum check) covers the ultra-fast case; LAP-02
is the per-track configurable version for more realistic minimum floors.

**Tracking in DB:**
```sql
-- Migration: add review_required column to laps table
ALTER TABLE laps ADD COLUMN review_required INTEGER NOT NULL DEFAULT 0;
```

```rust
// In persist_lap(), after INSERT:
let min_ms: Option<u32> = get_min_lap_time_ms_for_track(&state.db, &lap.track).await;
if let Some(min) = min_ms {
    if lap.lap_time_ms < min {
        let _ = sqlx::query(
            "UPDATE laps SET review_required = 1 WHERE id = ?",
        )
        .bind(&lap.id)
        .execute(&state.db)
        .await;
    }
}
```

### Pattern 2: Session Type Classification (LAP-03)

**What:** `LapData` gains a `session_type: SessionType` field. The AC adapter always
returns `SessionType::Practice` from its `session_info()` call — that same value should
be captured when a lap is completed and stored in the lap. Both AC and F1 adapters report
the session type; AC's `session_info()` currently hardcodes `SessionType::Practice`, which
is the correct starting point.

**LapData field addition:**
```rust
// rc-common/src/types.rs — LapData struct
pub session_type: SessionType,  // NEW: Practice/Qualifying/Race/Hotlap
```

The leaderboard public API filters on `valid = 1` only — the session_type classification
does not change leaderboard visibility, it enriches the data for future analytics.

### Pattern 3: Separate PIN Failure Counters (PIN-01, PIN-02)

**What:** Two separate `HashMap<String, u32>` in AppState keyed by pod_id — one for
customer failures, one for staff failures. The counters live server-side (racecontrol)
because validate_pin() is the authoritative resolution point. The agent only forwards raw
PIN strings.

**AppState additions:**
```rust
// state.rs — AppState
pub customer_pin_failures: Arc<tokio::sync::RwLock<HashMap<String, u32>>>,
pub staff_pin_failures: Arc<tokio::sync::RwLock<HashMap<String, u32>>>,
```

**Counter logic in validate_pin():**
```rust
// Customer path: increment customer counter on failure
// Staff path (employee debug PIN): increment staff counter on failure — SEPARATE
// Staff counter NEVER touches customer counter; customer counter NEVER touches staff counter
```

**Lockout thresholds:** The requirements state "staff PIN is never locked out" (PIN-02).
This means: customer_pin_failures may accumulate; staff counter is checked against a
separate, higher threshold (or no threshold — requirements say "never locked out").
Implementation: customer counter can gate on a configurable threshold (e.g. 5 attempts);
staff counter has NO lockout ceiling (PIN-02 is absolute).

### Pattern 4: TELEM-01 Game-State Guard

**What:** `handle_telemetry_gap()` in bot_coordinator.rs must gate on whether the pod's
game state is Live (AC is in active session) before firing the email alert. The pod state
is available via `state.pods.read().await.get(pod_id)`.

**Guard logic:**
```rust
// In handle_telemetry_gap():
let game_state = state.pods.read().await
    .get(pod_id)
    .and_then(|p| p.game_state);

// Only alert if game is actively running (not menu, not idle)
if !matches!(game_state, Some(GameState::Running)) {
    tracing::debug!("[bot-coord] TelemetryGap skipped — game not Running (state={:?})", game_state);
    return;
}
// ... proceed with email alert via state.email_alerter
```

**Send site (rc-agent side):** failure_monitor.rs needs a new detection arm:
```
TELEM-01: billing_active + game_pid.is_some() + last_udp_secs_ago >= 60s
→ send AgentMessage::TelemetryGap { pod_id, sim_type, gap_seconds }
```
Use a `telem_gap_fired` flag (like `launch_timeout_fired`) to suppress repeated sends.
Reset when `last_udp_secs_ago < 60` (data resumed) or `game_pid` becomes None.

### Pattern 5: MULTI-01 Teardown Order

**What:** `handle_multiplayer_failure()` in bot_coordinator.rs executes three steps in
order:
1. Send `CoreToAgentMessage::ShowPinLockScreen` (or `ClearLockScreen` + `ShowPinEntry` —
   actually just send a direct command to re-engage the lock screen)
2. Call `end_billing_session_public()` with `BillingSessionStatus::EndedEarly`
3. Log the event to activity_log

This is the same pattern as `recover_stuck_session()` — reuse that function's structure.
The lock screen re-engagement happens before billing end to ensure the pod is locked before
the session closes (prevents customer from continuing to drive without billing).

**Send site (rc-agent side):** The existing multiplayer.rs in rc-agent monitors AC server
connectivity. The detection needs to send `AgentMessage::MultiplayerFailure` when the AC
server connection drops mid-session. The agent already has the ac_server module context.

### Anti-Patterns to Avoid

- **Hard-deleting invalid laps:** Per STATE.md decision, never hard-delete. `review_required=true` only.
- **Sharing PIN counters:** Customer and staff counters must NEVER be in the same HashMap. A combined counter would violate PIN-02.
- **TELEM-01 alerting during menu:** Check `GameState::Running` guard. AC's STATUS=OFF/PAUSE means the customer is in menus — no alert.
- **MULTI-01 without lock screen first:** The ordered teardown (lock → billing end → log) is non-negotiable per requirements. Billing end before lock screen = customer can see billing end without being locked out.
- **review_required used as hard filter:** `review_required` is a staff-review flag. Public leaderboard filters on `valid = 1` only. Adding `AND review_required = 0` to the public leaderboard query is NOT correct.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Email rate limiting | Custom cooldown struct | Existing `EmailAlerter` in `email_alerts.rs` | Already has per-pod 30min + venue-wide 5min cooldown; `send_alert()` is the right call |
| Session end sequence | Custom billing teardown | Existing `end_billing_session_public()` | Owns StopGame → SessionEnded → wallet debit → broadcast — do NOT replicate |
| Lap validity | Custom physics analysis | Game-reported `is_valid` flag | Authoritative per STATE.md decisions |
| PIN cryptography | Custom HMAC | Existing `generate_daily_pin()` / `todays_debug_pin()` | Already uses HMAC-SHA256 variant; never roll crypto |
| Recovery guard | Custom mutex | Existing `is_pod_in_recovery()` from rc-common | Prevents concurrent fix races — always check before acting |

---

## Common Pitfalls

### Pitfall 1: AC IS_VALID_LAP Offset Uncertainty

**What goes wrong:** `graphics::IS_VALID_LAP = 180` is marked in the source as "approximate — may need correction". The comment at offset `396: currentSectorIndex repeated?` and `1408+: isValidLap` suggests the true offset may be deeper in the struct.

**Why it happens:** The AC shared memory struct is community-documented and version-dependent. CSP extends the struct.

**How to avoid:** The current code at offset 180 already sets `valid: is_valid != 0` and the field is wired through to `persist_lap()`. LAP-01 is about confirming the wire is complete end-to-end, not about changing the offset. The offset question is a separate investigation. Keep the existing behavior; don't change the offset in Phase 26.

**Warning signs:** All laps being marked valid=false when they should be valid would indicate offset mismatch — diagnose separately.

### Pitfall 2: LapData session_type requires migration in two places

**What goes wrong:** Adding `session_type` to `LapData` struct requires: (a) the struct change in rc-common, (b) the DB INSERT in lap_tracker.rs, (c) a DB migration adding the column, AND (d) all test fixtures that construct `LapData` need `session_type` populated.

**Why it happens:** LapData is constructed in two sim adapters (AC and F1) — both must set `session_type`. If either is left with `..Default::default()`, the field silently defaults.

**How to avoid:** `SessionType` does not implement `Default`. Make `session_type` non-optional in `LapData`. This forces all construction sites to explicitly set it. The compile error is the safety net.

### Pitfall 3: TelemetryGap double-fire without flag

**What goes wrong:** failure_monitor runs every 5s. If `last_udp_secs_ago >= 60` persists for multiple ticks, it will send multiple TelemetryGap messages, triggering multiple emails (rate-limiter in EmailAlerter will catch most, but it's wasteful and may circumvent venue-wide cooldown).

**Why it happens:** Same pattern as `launch_timeout_fired` — the detection condition persists once triggered.

**How to avoid:** Add `telem_gap_fired: bool` to the failure_monitor task-local state (not FailureMonitorState — same rationale as `launch_timeout_fired`). Reset when UDP data resumes.

### Pitfall 4: PIN counters reset on reconnect

**What goes wrong:** If the counters live only in AppState RwLock maps and a pod reconnects, the pod_id key stays in the map (pod_id doesn't change on reconnect). This is fine. But if the server restarts, all counters reset to 0. This is acceptable per requirements — the counters are in-memory, not persisted.

**Warning signs:** Staff complaining that the lockout threshold resets unexpectedly — answer is "yes, in-memory counters reset on server restart". This is by design for PIN-01.

### Pitfall 5: MULTI-01 detect-on-agent vs detect-on-server

**What goes wrong:** The AC server is on the server (.23), not on the pods. The pods connect to it via LAN. The rc-agent on the pod can detect the AC server disconnect by monitoring the UDP/TCP connection loss. The racecontrol server can also detect it via `ac_server.rs`. Both could send the failure signal.

**Why it happens:** Phase 26's MULTI-01 scope says "bot detects" — this is the rc-agent sending AgentMessage::MultiplayerFailure. The server's ac_server.rs handles server lifecycle (AML-02). MULTI-01 is the pod-side disconnect detection.

**How to avoid:** rc-agent failure_monitor.rs is the correct send site, consistent with other failure patterns. Check for loss of AC server connectivity by monitoring UDP gap for AC-specific game state or by detecting when the game returns to menu (AcStatus goes from Live to non-Live mid-session with billing active).

---

## Code Examples

### persist_lap — current gate (already works for LAP-01)

```rust
// lap_tracker.rs — existing code, LAP-01 is already wired
pub async fn persist_lap(state: &Arc<AppState>, lap: &LapData) -> bool {
    if lap.lap_time_ms == 0 || !lap.valid {
        return false;  // Invalid lap: not persisted, not on leaderboard
    }
    // ...
}
```

**LAP-01 wire confirmation:** AC adapter sets `valid: is_valid != 0`. F1 adapter sets `valid: !self.current_lap_invalid`. persist_lap gates on `!lap.valid`. The wire is complete. LAP-01 is largely already implemented — the Wave 0 RED tests just verify the end-to-end path.

### TrackEntry min_lap_time_ms addition (LAP-02)

```rust
// catalog.rs
#[derive(Debug, Clone, Serialize)]
pub struct TrackEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub country: &'static str,
    pub min_lap_time_ms: Option<u32>,  // NEW
}

// Initial set (Monza, Silverstone, Spa):
TrackEntry { id: "monza",         ..., min_lap_time_ms: Some(80_000) },  // 1:20 minimum
TrackEntry { id: "ks_silverstone", ..., min_lap_time_ms: Some(90_000) }, // 1:30 minimum
TrackEntry { id: "spa",           ..., min_lap_time_ms: Some(120_000) }, // 2:00 minimum
// All other tracks: min_lap_time_ms: None
```

**Note:** Exact minimum values are for planner to confirm with Uday. These are placeholder floors for planning.

### handle_telemetry_gap — promoted from stub (TELEM-01)

```rust
// bot_coordinator.rs
pub async fn handle_telemetry_gap(
    state: &Arc<AppState>,
    pod_id: &str,
    gap_seconds: u64,
) {
    // Guard: only alert if game is actively Running
    let game_state = state.pods.read().await
        .get(pod_id)
        .and_then(|p| p.game_state);
    if !matches!(game_state, Some(GameState::Running)) {
        tracing::debug!("[bot-coord] TelemetryGap ignored — pod {} game not Running ({:?})", pod_id, game_state);
        return;
    }

    // Guard: only alert if billing is active
    let billing_active = state.billing.active_timers.read().await.contains_key(pod_id);
    if !billing_active {
        return;
    }

    let subject = format!("Racing Point Alert: Pod {} UDP telemetry gap {}s", pod_id, gap_seconds);
    let body = format!(
        "Pod {} has not sent UDP telemetry for {}s while billing is active.\n\nGame is running. Please check if AC has crashed silently.",
        pod_id, gap_seconds
    );
    state.email_alerter.write().await.send_alert(pod_id, &subject, &body).await;
}
```

### TELEM-01 send site in failure_monitor.rs

```rust
// New detection arm in failure_monitor spawn loop:
const TELEM_GAP_SECS: u64 = 60;
// task-local: let mut telem_gap_fired = false;

// TELEM-01: UDP silence 60s while game running + billing active
if state.billing_active && state.game_pid.is_some() {
    let udp_silent_60 = state.last_udp_secs_ago
        .map(|s| s >= TELEM_GAP_SECS)
        .unwrap_or(false);
    if udp_silent_60 && !telem_gap_fired {
        telem_gap_fired = true;
        let gap = state.last_udp_secs_ago.unwrap_or(TELEM_GAP_SECS);
        let msg = AgentMessage::TelemetryGap {
            pod_id: pod_id.clone(),
            sim_type: SimType::AssettoCorsa, // TODO: read from state
            gap_seconds: gap as u32,
        };
        let _ = agent_msg_tx.try_send(msg);
    }
    if !udp_silent_60 {
        telem_gap_fired = false; // reset when data resumes
    }
}
```

### handle_multiplayer_failure (MULTI-01)

```rust
// bot_coordinator.rs — new function
pub async fn handle_multiplayer_failure(
    state: &Arc<AppState>,
    pod_id: &str,
    reason: &PodFailureReason,
    session_id: Option<&str>,
) {
    tracing::warn!(
        "[bot-coord] MultiplayerFailure pod={} reason={:?} session={:?}",
        pod_id, reason, session_id
    );

    // Step 1: Engage lock screen (pod becomes locked immediately)
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(pod_id) {
        let _ = sender.send(CoreToAgentMessage::EngageLockScreen).await;
    }
    drop(agent_senders);

    // Step 2: End billing session
    let session_id_resolved = state.billing.active_timers.read().await
        .get(pod_id).map(|t| t.session_id.clone());
    if let Some(sid) = session_id_resolved {
        end_billing_session_public(state, &sid, BillingSessionStatus::EndedEarly).await;
    }

    // Step 3: Log event
    crate::activity_log::log_pod_activity(
        state, pod_id, "multiplayer",
        "Multiplayer Disconnect",
        &format!("reason={:?} session={:?}", reason, session_id),
        "bot"
    );
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Stubs in bot_coordinator.rs (Phase 25) | Full implementations in Phase 26 | TELEM-01 and MULTI-01 become operational |
| No lap minimum floor | Per-track min_lap_time_ms in catalog | LAP-02: fraudulent/impossible laps flagged for review |
| Shared PIN validation path | Separate customer/staff counters | PIN-01, PIN-02: staff can always unlock |
| LapData without session_type | LapData.session_type from sim adapter | LAP-03: lap type classification preserved |

**Deprecated/outdated:**
- `handle_telemetry_gap` stub body — the log-only stub is replaced by full alert logic

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test (cargo test) |
| Config file | none — workspace Cargo.toml |
| Quick run command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| Full suite command | same as above (no separate integration suite) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LAP-01 | `persist_lap` skips lap when `valid=false` | unit | `cargo test -p racecontrol-crate -- lap_tracker` | ✅ (persist_lap tests in lap_tracker.rs) |
| LAP-01 | AC adapter sets `valid` from IS_VALID_LAP | unit | `cargo test -p rc-agent-crate -- assetto_corsa` | ✅ |
| LAP-02 | Lap below track minimum sets `review_required=true` in DB | unit | `cargo test -p racecontrol-crate -- lap_tracker` | ❌ Wave 0 |
| LAP-02 | Lap above track minimum does NOT set `review_required` | unit | `cargo test -p racecontrol-crate -- lap_tracker` | ❌ Wave 0 |
| LAP-03 | LapData includes session_type from sim adapter | unit | `cargo test -p rc-agent-crate -- sims` | ❌ Wave 0 |
| PIN-01 | Customer counter increments on wrong PIN | unit | `cargo test -p racecontrol-crate -- auth` | ❌ Wave 0 |
| PIN-01 | Customer counter does not touch staff counter | unit | `cargo test -p racecontrol-crate -- auth` | ❌ Wave 0 |
| PIN-02 | Staff PIN succeeds even when customer counter is maxed | unit | `cargo test -p racecontrol-crate -- auth` | ❌ Wave 0 |
| TELEM-01 | handle_telemetry_gap sends email when game Running + billing active | unit | `cargo test -p racecontrol-crate -- bot_coordinator` | ❌ Wave 0 |
| TELEM-01 | handle_telemetry_gap skips email when game not Running | unit | `cargo test -p racecontrol-crate -- bot_coordinator` | ❌ Wave 0 |
| TELEM-01 | failure_monitor sends TelemetryGap after 60s UDP silence | unit | `cargo test -p rc-agent-crate -- failure_monitor` | ❌ Wave 0 |
| MULTI-01 | handle_multiplayer_failure routes to correct handler | unit | `cargo test -p racecontrol-crate -- bot_coordinator` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Per wave merge:** same
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `lap_tracker.rs` — tests for LAP-02 `review_required` flag + LAP-03 `session_type`
- [ ] `auth/mod.rs` — tests for PIN-01/PIN-02 separate counter behavior
- [ ] `bot_coordinator.rs` — tests for TELEM-01 game-state guard + MULTI-01 routing
- [ ] `failure_monitor.rs` — test for TELEM-01 60s UDP silence detection send site
- [ ] DB migration: `ALTER TABLE laps ADD COLUMN review_required INTEGER NOT NULL DEFAULT 0`
- [ ] DB migration: `ALTER TABLE laps ADD COLUMN session_type TEXT NOT NULL DEFAULT 'practice'`

---

## Open Questions

1. **AC IS_VALID_LAP offset correctness**
   - What we know: offset 180 is marked "approximate" in a comment; the true offset per the AC shared memory reference is "1408+"
   - What's unclear: whether offset 180 currently works in practice or always returns 0 (making all laps appear valid)
   - Recommendation: During Wave 1, verify on Pod 8 by deliberately cutting a track and checking the valid flag in rc-agent logs. Do not change the offset in this phase unless testing confirms it's broken.

2. **Exact minimum lap times for LAP-02 tracks**
   - What we know: Monza, Silverstone, Spa are the initial set per requirements
   - What's unclear: what values are appropriate for the Racing Point hardware (RTX 4070, Conspit Ares)
   - Recommendation: Use conservative values (80s Monza, 90s Silverstone, 120s Spa) as placeholders. Ask Uday to confirm before deploy.

3. **EngageLockScreen vs ShowPinLockScreen for MULTI-01**
   - What we know: CoreToAgentMessage has ShowPinLockScreen but the protocol.rs preview shows a `ClearLockScreen` variant
   - What's unclear: whether there's a standalone "lock the pod immediately without showing a new booking screen" command vs requiring a token
   - Recommendation: Check if CoreToAgentMessage has an `EngageLockScreen` variant without a token. If not, the MULTI-01 implementation should just show the idle/disconnected state, which is effectively a locked state. `ClearLockScreen` closes the browser; the lock screen HTTP server serves a default page. Investigate lock_screen.rs `show_screen_blanked()` as the right call.

4. **MULTI-01 detection on rc-agent: what signal triggers it?**
   - What we know: AC reports STATUS via `read_ac_status()` — AcStatus::Off/Live/Pause. A mid-race disconnect would likely show AcStatus::Off or a shift back to non-Live.
   - What's unclear: Is the AcStatus transition sufficient, or does the pod need to detect the AC server TCP connection dropping separately?
   - Recommendation: Use AcStatus transition from Live → non-Live with `billing_active=true` as the multiplayer disconnect signal. This is already read in main.rs and reported via GameStatusUpdate. The MULTI-01 detection can live in failure_monitor or in the main.rs GameStatusUpdate handler.

---

## Sources

### Primary (HIGH confidence)

- Direct codebase reading: `crates/rc-agent/src/sims/assetto_corsa.rs` — IS_VALID_LAP handling, LapData construction
- Direct codebase reading: `crates/rc-agent/src/sims/f1_25.rs` — `current_lap_invalid` wiring
- Direct codebase reading: `crates/racecontrol/src/lap_tracker.rs` — `persist_lap()` gate at line 28-29
- Direct codebase reading: `crates/racecontrol/src/bot_coordinator.rs` — stub locations confirmed
- Direct codebase reading: `crates/racecontrol/src/auth/mod.rs` — validate_pin() structure, employee debug PIN path
- Direct codebase reading: `crates/racecontrol/src/email_alerts.rs` — EmailAlerter interface
- Direct codebase reading: `crates/racecontrol/src/ws/mod.rs` — TelemetryGap and MultiplayerFailure arm locations (lines 511-522)
- Direct codebase reading: `crates/rc-agent/src/failure_monitor.rs` — detection pattern, FailureMonitorState fields
- Direct codebase reading: `crates/rc-common/src/types.rs` — PodFailureReason, SessionType, LapData struct
- Direct codebase reading: `crates/rc-common/src/protocol.rs` — AgentMessage::TelemetryGap, LapFlagged, MultiplayerFailure variants
- Direct codebase reading: `.planning/STATE.md` — locked decisions (lap filter, PIN counters, multiplayer scope)
- Direct codebase reading: `.planning/REQUIREMENTS.md` — LAP-01 through MULTI-01 definitions

### Secondary (MEDIUM confidence)

- AC shared memory reference (community-documented): offset 180 for IS_VALID_LAP is approximate; true extended-struct offset is ~1408+

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all infrastructure exists
- Architecture: HIGH — all insertion points confirmed by direct source reading
- Pitfalls: HIGH — IS_VALID_LAP offset uncertainty is documented in source; PIN counter separation is explicit in requirements; all other pitfalls are pattern-based from Phase 24-25 experience

**Research date:** 2026-03-16
**Valid until:** 2026-04-16 (stable codebase, no external API dependencies)
