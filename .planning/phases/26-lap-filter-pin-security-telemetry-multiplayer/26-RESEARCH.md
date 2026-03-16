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
already reads `lap.valid` and gates on it at line 28-29 — LAP-01 is largely already wired.
The gap is: (a) LAP-02 needs a per-track minimum lap time configurable in the track catalog
and a `review_required` column in the DB, (b) LAP-03 needs a `session_type` field added to
`LapData` (currently absent — struct has no such field), and (c) persist_lap needs to
check both the min-time floor and store session_type in the DB insert.

**PIN domain (PIN-01, PIN-02):** The current `validate_pin()` checks the employee debug
PIN first (early return via `validate_employee_pin()`), then the customer token path. No
failure counter of any kind exists yet. PIN-01 requires adding separate in-memory counters
(customer and staff), and PIN-02 requires the staff counter to be in a completely different
namespace so customer exhaustion never blocks the employee path. Counters live in AppState
server-side — the agent only forwards raw PIN strings.

**TELEM-01:** `AgentMessage::TelemetryGap` is already routed to
`bot_coordinator::handle_telemetry_gap()` in ws/mod.rs (line 511-514). The stub logs only.
The requirement adds: (a) check that the pod's game state is `GameState::Running` before
firing the email, and (b) add a TelemetryGap send site in `failure_monitor.rs` — currently
no detection arm for 60s UDP silence exists there.

**MULTI-01:** `AgentMessage::MultiplayerFailure` is already routed in ws/mod.rs (line
520-521) but only logs. The requirement adds `handle_multiplayer_failure()` in
bot_coordinator.rs. Key finding: there is no `EngageLockScreen` variant in
`CoreToAgentMessage`. The correct lock-screen command for MULTI-01 is `SessionEnded` —
this shows the session summary (idle state) which is a locked state. Alternatively,
`ClearLockScreen` puts the pod in the StartupConnecting state; `BlankScreen` shows black.
The MULTI-01 detection signal on rc-agent side: AcStatus transition from Live → non-Live
with billing_active=true is the correct trigger.

**Primary recommendation:** Follow the same TDD wave structure as Phases 24-25. Wave 0
adds RED test stubs, Wave 1a implements lap/PIN work (independent of TELEM/MULTI), Wave 1b
implements TELEM-01 and MULTI-01, and the wiring wave connects everything. LAP+PIN work is
fully independent of TELEM+MULTI and can be planned in parallel waves.

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LAP-01 | `is_valid` flag wired from AC and F1 25 sim adapters into `persist_lap` | AC adapter already sets `valid: is_valid != 0` (assetto_corsa.rs line 308); F1 adapter already sets `valid: !self.current_lap_invalid` (inferred from current_lap_invalid field); persist_lap gates on `!lap.valid` (lap_tracker.rs line 28) — gap is completing the end-to-end test coverage |
| LAP-02 | Per-track minimum lap time configurable in track catalog (Monza, Silverstone, Spa as initial set) | catalog.rs TrackEntry has id/name/category/country only — needs `min_lap_time_ms: Option<u32>` field; DB needs `review_required` column via ALTER TABLE; persist_lap checks floor and sets flag |
| LAP-03 | Laps classified as hotlap vs practice based on session type reported by sim adapter | `SessionType` enum has Practice/Qualifying/Race/Hotlap; `LapData` struct lacks `session_type` field (verified: not in types.rs LapData); F1 adapter already resolves session_type from packet 1; AC adapter hardcodes Practice in session_info() — both must populate it at lap completion |
| PIN-01 | Customer and staff PIN failure counters tracked separately | No counters exist in AppState or auth/mod.rs — confirmed by source read; server-side HashMap in AppState is the correct location; keyed by pod_id |
| PIN-02 | Staff PIN is never locked out by customer PIN failure accumulation | validate_pin() already separates staff path (early return) — counters just need separate HashMaps; customer counter has a configurable ceiling; staff counter has NO ceiling |
| TELEM-01 | Bot detects UDP silence >60s during active billing + Live game state and alerts staff via email | handle_telemetry_gap() stub at bot_coordinator.rs line 93-103; needs GameState::Running guard via state.pods; send site (failure_monitor.rs) currently has no 60s UDP silence arm — must add with telem_gap_fired flag |
| MULTI-01 | Bot detects AC multiplayer server disconnect mid-race and triggers lock screen → end billing → log event | MultiplayerFailure arm at ws/mod.rs line 520-521 is log-only stub; handle_multiplayer_failure() needed in bot_coordinator.rs; no EngageLockScreen command exists — use SessionEnded or BlankScreen to achieve locked state; rc-agent detection signal: AcStatus Live→non-Live with billing_active |
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
├── lap_tracker.rs          # add review_required logic + session_type column in persist_lap
├── catalog.rs              # add min_lap_time_ms to TrackEntry + helper fn
├── auth/mod.rs             # add per-pod customer + staff failure counter HashMaps in AppState
├── state.rs                # add customer_pin_failures + staff_pin_failures to AppState
├── db/mod.rs               # migrations: review_required + session_type columns on laps
crates/rc-common/src/
├── types.rs                # add session_type: SessionType field to LapData struct
crates/rc-agent/src/
├── failure_monitor.rs      # add TELEM-01 detection arm (60s UDP silence send)
├── sims/assetto_corsa.rs   # populate session_type on LapData construction
├── sims/f1_25.rs           # populate session_type on LapData construction
```

No new files required — all changes are additive in existing files. The rc-common change
(LapData field addition) is a cross-crate change that will force all LapData construction
sites to be updated simultaneously.

### Pattern 1: Lap Minimum Time Floor (LAP-02)

**What:** Track catalog holds `min_lap_time_ms: Option<u32>`. `persist_lap()` queries this
after inserting the lap row, and sets `review_required = 1` in the DB if
`lap_time_ms < min_lap_time_ms`. Does NOT delete or skip the lap. The lap is stored with
`valid = true` (game says OK) but `review_required = true` (below track floor).

**DB migration:**
```sql
ALTER TABLE laps ADD COLUMN review_required INTEGER NOT NULL DEFAULT 0;
ALTER TABLE laps ADD COLUMN session_type TEXT NOT NULL DEFAULT 'practice';
```

**TrackEntry change:**
```rust
// catalog.rs
#[derive(Debug, Clone, Serialize)]
pub struct TrackEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub country: &'static str,
    pub min_lap_time_ms: Option<u32>,  // NEW for LAP-02
}
```

**Initial set (Monza, Silverstone, Spa) — conservative floors for Racing Point hardware:**
```rust
TrackEntry { id: "monza",          ..., min_lap_time_ms: Some(80_000)  }, // 1:20 minimum
TrackEntry { id: "ks_silverstone", ..., min_lap_time_ms: Some(90_000)  }, // 1:30 minimum
TrackEntry { id: "spa",            ..., min_lap_time_ms: Some(120_000) }, // 2:00 minimum
// All other tracks: min_lap_time_ms: None
```

**persist_lap addition:**
```rust
// After INSERT — check minimum floor for LAP-02
let min_ms: Option<u32> = get_min_lap_time_ms_for_track(&lap.track);  // catalog lookup
if let Some(min) = min_ms {
    if lap.lap_time_ms < min {
        let _ = sqlx::query("UPDATE laps SET review_required = 1 WHERE id = ?")
            .bind(&lap.id).execute(&state.db).await;
    }
}
```

Note: `get_min_lap_time_ms_for_track` scans the static FEATURED_TRACKS array by track id.
This is synchronous (static data) — no DB query needed for the lookup.

### Pattern 2: Session Type Classification (LAP-03)

**What:** `LapData` gains a `session_type: SessionType` field. Both sim adapters must
populate it at lap completion.

**LapData field addition in rc-common:**
```rust
// rc-common/src/types.rs — LapData struct
pub struct LapData {
    // ... existing fields ...
    pub valid: bool,
    pub session_type: SessionType,  // NEW: Practice/Qualifying/Race/Hotlap
    pub created_at: DateTime<Utc>,
}
```

**IMPORTANT: SessionType does not derive Default.** This means adding the field without a
default forces all construction sites to compile-error until explicitly set. This is the
safety net — exploit it. Do NOT give session_type a default.

**AC adapter:** AC's `session_info()` returns `SessionType::Practice` always. The adapter
should capture the session type from `session_info()` at connection time and use it when
constructing LapData in `read_telemetry()`.

**F1 adapter:** Already resolves session_type from packet 1 byte 6 (`self.session_type`).
At lap completion in `poll_lap_completed()`, the `last_completed_lap` construction must
include `session_type: resolved_session_type`.

### Pattern 3: Separate PIN Failure Counters (PIN-01, PIN-02)

**What:** Two separate `HashMap<String, u32>` in AppState keyed by pod_id. The counters
live server-side because `validate_pin()` is the authoritative resolution point.

**AppState additions in state.rs:**
```rust
pub customer_pin_failures: Arc<tokio::sync::RwLock<HashMap<String, u32>>>,
pub staff_pin_failures: Arc<tokio::sync::RwLock<HashMap<String, u32>>>,
```

**Counter logic in validate_pin():**
- Customer path: increment `customer_pin_failures[pod_id]` on any failure from the token
  lookup path. Lockout when >= configured threshold (e.g. 5).
- Staff path (`validate_employee_pin()`): increment `staff_pin_failures[pod_id]` on wrong
  daily PIN. NO lockout ceiling for staff (PIN-02 absolute guarantee).
- On successful customer PIN: reset `customer_pin_failures[pod_id]` to 0.
- Staff counter and customer counter NEVER cross-reference each other.

**Lockout response:** When customer counter >= threshold, return the same
`INVALID_PIN_MESSAGE` (do not reveal a lockout to the customer). Send `PinFailed` to agent.
Staff path continues to work regardless.

### Pattern 4: TELEM-01 Game-State Guard

**What:** `handle_telemetry_gap()` in bot_coordinator.rs gates on whether the pod's game
state is `GameState::Running` before firing the email alert. The pod state is available via
`state.pods.read().await.get(pod_id)`.

**Key finding:** `PodInfo.game_state: Option<GameState>` uses `GameState` (not `AcStatus`).
`GameState::Running` means the game executable is running (process up). This is the correct
guard — the AC adapter sets AcStatus::Live when the car is on track, but GameState::Running
tells us the process is up. Both conditions together mean the driver is in an active session.

**Guard logic in handle_telemetry_gap():**
```rust
// Guard: game process must be Running (not Launching, Stopping, Error, Idle)
let game_state = state.pods.read().await
    .get(pod_id)
    .and_then(|p| p.game_state);
if !matches!(game_state, Some(GameState::Running)) {
    tracing::debug!(
        "[bot-coord] TelemetryGap pod={} ignored — game not Running ({:?})",
        pod_id, game_state
    );
    return;
}

// Guard: billing must be active
let billing_active = state.billing.active_timers.read().await.contains_key(pod_id);
if !billing_active {
    return;
}

// Proceed with email alert
let subject = format!("Racing Point Alert: Pod {} UDP telemetry gap {}s", pod_id, gap_seconds);
let body = format!(
    "Pod {} has not sent UDP telemetry for {}s while billing is active.\n\nGame is running. Please check if AC has crashed silently.",
    pod_id, gap_seconds
);
state.email_alerter.write().await.send_alert(pod_id, &subject, &body).await;
```

**Send site in failure_monitor.rs:**
```
TELEM-01: billing_active + game_pid.is_some() + last_udp_secs_ago >= 60s
→ send AgentMessage::TelemetryGap { pod_id, sim_type: SimType::AssettoCorsa, gap_seconds }
```

Use a `telem_gap_fired: bool` task-local flag (like `launch_timeout_fired`). Reset when
`last_udp_secs_ago < 60` (data resumed) or `game_pid` becomes None.

```rust
const TELEM_GAP_SECS: u64 = 60;
// task-local: let mut telem_gap_fired = false;

if state.billing_active && state.game_pid.is_some() {
    let udp_silent_60 = state.last_udp_secs_ago
        .map(|s| s >= TELEM_GAP_SECS)
        .unwrap_or(false);
    if udp_silent_60 && !telem_gap_fired {
        telem_gap_fired = true;
        let gap = state.last_udp_secs_ago.unwrap_or(TELEM_GAP_SECS);
        let msg = AgentMessage::TelemetryGap {
            pod_id: pod_id.clone(),
            sim_type: SimType::AssettoCorsa,  // TODO: read sim_type from FailureMonitorState
            gap_seconds: gap as u32,
        };
        let _ = agent_msg_tx.try_send(msg);
    }
    if !udp_silent_60 {
        telem_gap_fired = false;  // reset when data resumes
    }
} else {
    telem_gap_fired = false;  // reset when billing or game stops
}
```

**Note on sim_type:** FailureMonitorState does not have a sim_type field. For Phase 26,
hardcoding `SimType::AssettoCorsa` is acceptable since AC is the primary sim. A follow-up
can add sim_type to FailureMonitorState if needed.

### Pattern 5: MULTI-01 Teardown Order

**What:** `handle_multiplayer_failure()` in bot_coordinator.rs executes three steps in order:
1. Engage lock screen (pod becomes locked immediately)
2. End billing session
3. Log the event

**Key finding on lock screen command:** There is NO `EngageLockScreen` variant in
`CoreToAgentMessage`. The available commands are: `ShowPinLockScreen` (requires token_id),
`ShowQrLockScreen` (requires token), `ClearLockScreen` (closes browser — shows
StartupConnecting), `BlankScreen` (black screen), `SessionEnded` (shows session summary
screen — effectively idle+locked).

**Recommended approach:** Send `SessionEnded` with the active billing session data
(driver_name, total_laps, best_lap, driving_seconds). This shows the session summary screen
which is a clean idle state. Alternatively, send `BlankScreen` for an immediate black
screen. Both are acceptable — `SessionEnded` is the better UX as it shows the driver their
stats before exiting.

**MULTI-01 send site (rc-agent side):** The detection signal is AcStatus transitioning from
Live → non-Live (Off or Pause) while `billing_active = true` AND the session was a
multiplayer session. In `failure_monitor.rs`, this can be detected by tracking the previous
AcStatus. However, since `failure_monitor.rs` does not currently track AcStatus, the
simpler approach is to detect it in `main.rs` in the `GameStatusUpdate` handler where
`last_ac_status` is already tracked.

```rust
// In handle_multiplayer_failure():
// Step 1: lock the pod
let billing_data = state.billing.active_timers.read().await
    .get(pod_id).map(|t| (t.session_id.clone(), t.driver_id.clone()));
if let Some((session_id, _)) = &billing_data {
    // Get summary data for SessionEnded
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(pod_id) {
        let _ = sender.send(CoreToAgentMessage::SessionEnded {
            billing_session_id: session_id.clone(),
            driver_name: "".to_string(),  // OK to be empty for disconnect scenario
            total_laps: 0,
            best_lap_ms: None,
            driving_seconds: 0,
        }).await;
    }
    drop(agent_senders);
}

// Step 2: end billing
if let Some((session_id, _)) = billing_data {
    end_billing_session_public(state, &session_id, BillingSessionStatus::EndedEarly).await;
}

// Step 3: log event
tracing::warn!("[bot-coord] MULTI-01 pod={} multiplayer disconnect — pod teardown complete", pod_id);
```

### Anti-Patterns to Avoid

- **Hard-deleting invalid laps:** Per STATE.md decision, never hard-delete. `review_required=true` only.
- **Sharing PIN counters:** Customer and staff counters must NEVER be in the same HashMap. A combined counter would violate PIN-02.
- **TELEM-01 alerting during menu:** Check `GameState::Running` guard. GameState::Idle/Stopping/Error means game is not active — no alert.
- **MULTI-01 without lock screen first:** The ordered teardown (lock → billing end → log) is non-negotiable per requirements.
- **review_required used as leaderboard filter:** `review_required` is a staff-review flag. Public leaderboard filters on `valid = 1` only. Do NOT add `AND review_required = 0` to leaderboard queries.
- **Using EngageLockScreen (does not exist):** CoreToAgentMessage has no such variant. Use SessionEnded or BlankScreen.
- **Skipping the LapData compile-error safety net:** Adding session_type without a Default forces all sites to set it explicitly. Do not add `#[serde(default)]` or `Option<SessionType>` — keep it mandatory.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Email rate limiting | Custom cooldown struct | Existing `EmailAlerter` in `email_alerts.rs` | Already has per-pod 30min + venue-wide 5min cooldown; `send_alert()` is the right call |
| Session end sequence | Custom billing teardown | Existing `end_billing_session_public()` | Owns StopGame → SessionEnded → wallet debit → broadcast — do NOT replicate |
| Lap validity | Custom physics analysis | Game-reported `is_valid` flag | Authoritative per STATE.md decisions |
| Track minimum lookup | DB query per lap | Static scan of FEATURED_TRACKS array in catalog.rs | Array is ~36 tracks, scan is O(n) and synchronous — no DB round-trip needed |
| PIN cryptography | Custom HMAC | Existing `generate_daily_pin()` / `todays_debug_pin()` | Already uses HMAC-SHA256 variant; never roll crypto |
| Recovery guard | Custom mutex | Existing `is_pod_in_recovery()` from pod_healer.rs | Prevents concurrent fix races — always check before acting in bot_coordinator |

---

## Common Pitfalls

### Pitfall 1: AC IS_VALID_LAP Offset Uncertainty

**What goes wrong:** `graphics::IS_VALID_LAP = 180` is marked in assetto_corsa.rs as
"approximate — may need correction". The comment references true offset as "1408+" in the
extended struct. The field currently used in the AC adapter may always return 0.

**Why it happens:** AC shared memory struct is community-documented and version-dependent.
CSP (Custom Shaders Patch) extends the struct layout.

**How to avoid:** Phase 26 is about verifying the end-to-end wire for LAP-01, not changing
the offset. The existing behavior is: AC sets `valid: is_valid != 0`. If offset 180 returns
0 always, all AC laps would be marked invalid=false (stored but skipped by persist_lap).
Verify on Pod 8 by checking rc-agent logs — if "AC lap completed: valid=false" appears for
clearly-valid laps, the offset is wrong. Do NOT change the offset in this phase; file an
open question.

**Warning signs:** All AC laps marked valid=false in leaderboard or logs.

### Pitfall 2: LapData session_type is a cross-crate breaking change

**What goes wrong:** Adding `session_type: SessionType` to `LapData` in rc-common forces
both consuming crates (rc-agent, racecontrol) to update all LapData construction sites.
There are two construction sites in rc-agent (AC adapter, F1 adapter) and any test code
that constructs LapData directly.

**Why it happens:** rc-common changes propagate to all crates. The compiler will catch every
site with a hard error.

**How to avoid:** Plan the rc-common change and all construction-site updates in the same
task. Do NOT add `#[serde(default)]` to suppress the error — let the compiler enforce all
sites are updated. Also add the DB column migration at the same time.

### Pitfall 3: TelemetryGap double-fire without flag

**What goes wrong:** failure_monitor runs every 5s. If `last_udp_secs_ago >= 60` persists
across multiple ticks, it will send multiple TelemetryGap messages. EmailAlerter rate-limits
per pod but the per-pod cooldown is 30 minutes — repeated messages within 30 min will be
silently dropped but it's still wasteful.

**How to avoid:** Add `telem_gap_fired: bool` as a task-local variable in
`failure_monitor::spawn()` (same pattern as `launch_timeout_fired`). Reset when UDP data
resumes or game stops.

### Pitfall 4: PIN counters and pod reconnect

**What goes wrong:** If a customer enters 5 wrong PINs, gets locked out, then the server
restarts — the in-memory counter resets and the lockout disappears. This is acceptable (per
requirements, counters are not persisted), but staff should know this behavior.

**Warning signs:** "The counter reset" reports after server restart. Expected behavior —
document in commit message.

### Pitfall 5: MULTI-01 detection site: failure_monitor vs main.rs

**What goes wrong:** Adding AcStatus tracking to failure_monitor.rs requires adding a new
field to FailureMonitorState AND new send_modify() update sites in main.rs (many sites
already reset last_ac_status). This is more work than detecting in the existing
GameStatusUpdate handler in main.rs.

**How to avoid:** For Phase 26, detect multiplayer disconnect in `main.rs` within the
`GameStatusUpdate` arm — where `last_ac_status` is already tracked. When
`status == AcStatus::Off` and `billing_active == true` and the billing session has a
`group_session_id` (multiplayer), send `AgentMessage::MultiplayerFailure`.

This keeps FailureMonitorState lean and avoids adding new state update sites.

### Pitfall 6: MULTI-01 ordering: session_ended before billing_end

**What goes wrong:** Calling `end_billing_session_public()` first sends `StopGame` and
`SessionEnded` to the agent. If `handle_multiplayer_failure()` also sends `SessionEnded`
first, there would be two SessionEnded messages. The correct order is: (1) send explicit
lock screen message OR let billing end send it, NOT both.

**How to avoid:** Use `end_billing_session_public()` to handle the session end —
it already sends `SessionEnded` to the agent as part of its teardown. Do NOT send a
separate `SessionEnded` before calling it. For step 1 (lock first), use `BlankScreen` to
immediately black out the pod, then call `end_billing_session_public()` which sends
`SessionEnded` with stats. This achieves: lock (blank) → billing end (with SessionEnded to
agent) → log event.

---

## Code Examples

### persist_lap — current gate (already works for LAP-01)

```rust
// lap_tracker.rs — existing code at lines 27-30, LAP-01 is already wired
pub async fn persist_lap(state: &Arc<AppState>, lap: &LapData) -> bool {
    if lap.lap_time_ms == 0 || !lap.valid {
        return false;  // Invalid lap: not persisted at all, never reaches leaderboard
    }
    // ...
}
```

**LAP-01 analysis:** The wire is complete for the happy path. The remaining work:
- Wave 0 RED tests to confirm the end-to-end path compiles and the invalid-lap skip is tested
- LAP-02: add review_required flag for laps that pass game validity but fail the floor
- LAP-03: add session_type field — requires rc-common change first

### handle_multiplayer_failure — correct lock-screen approach

```rust
// bot_coordinator.rs — MULTI-01 implementation
// Use BlankScreen first (immediate lock), then end billing (sends SessionEnded to agent)
pub async fn handle_multiplayer_failure(
    state: &Arc<AppState>,
    pod_id: &str,
    reason: &PodFailureReason,
    session_id: Option<&str>,
) {
    // Step 1: Blank screen immediately (pod is locked)
    {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender.send(CoreToAgentMessage::BlankScreen).await;
        }
    }

    // Step 2: End billing — end_billing_session_public sends SessionEnded to agent
    let session_id_resolved = state.billing.active_timers.read().await
        .get(pod_id).map(|t| t.session_id.clone());
    if let Some(sid) = session_id_resolved {
        let _ = end_billing_session_public(state, &sid, BillingSessionStatus::EndedEarly).await;
    }

    // Step 3: Log event
    tracing::warn!(
        "[bot-coord] MULTI-01 pod={} reason={:?} session={:?} — teardown complete",
        pod_id, reason, session_id
    );
}
```

### Leaderboard query — confirm review_required is NOT filtered

```sql
-- Existing leaderboard query pattern (lap_tracker.rs personal_bests / track_records)
-- review_required must NOT appear in WHERE clause for public leaderboard
-- Only valid = 1 filters apply
SELECT * FROM laps WHERE valid = 1 AND track = ? ORDER BY lap_time_ms ASC LIMIT 100;
-- NOT: WHERE valid = 1 AND review_required = 0  <-- wrong, do not add this
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Stubs in bot_coordinator.rs (Phase 25) | Full implementations in Phase 26 | TELEM-01 and MULTI-01 become operational |
| No lap minimum floor | Per-track min_lap_time_ms in catalog | LAP-02: impossible laps flagged for staff review |
| Shared PIN validation path | Separate customer/staff counters | PIN-01, PIN-02: staff can always unlock |
| LapData without session_type | LapData.session_type from sim adapter | LAP-03: lap type classification preserved |
| handle_telemetry_gap logs only | Emails staff with game-state guard | TELEM-01: real alert when game is running |

**Deprecated/outdated:**
- `handle_telemetry_gap` stub body — the log-only stub is replaced by full alert logic
- `handle_multiplayer_failure` in ws/mod.rs log stub — replaced by bot_coordinator handler

---

## Wave Structure Recommendation

Phase 26 has two independent work streams that can be planned in parallel waves:

**Stream A: Lap + PIN (server-side, rc-common, rc-agent sims)**
- Wave 0: RED tests for LAP-01 end-to-end, LAP-02 review_required, LAP-03 session_type, PIN-01/PIN-02 counter separation
- Wave 1a: LapData session_type field (rc-common) + DB migrations + catalog min_lap_time_ms + persist_lap logic + sim adapter updates + AppState counter fields + validate_pin counter logic

**Stream B: Telemetry + Multiplayer (failure_monitor + bot_coordinator)**
- Wave 0: RED tests for TELEM-01 send site (failure_monitor), TELEM-01 handler (bot_coordinator), MULTI-01 handler (bot_coordinator)
- Wave 1b: failure_monitor TELEM-01 detection arm + handle_telemetry_gap implementation + handle_multiplayer_failure implementation + ws/mod.rs stub promotion

**Wave 2 (wiring):** Both streams merged, full suite green. Deploy to Pod 8, verify.

The dependency ordering: Stream A's rc-common change (LapData.session_type) breaks
compilation in rc-agent, so both streams cannot run at the exact same time during Wave 1.
Stream A must land first. Stream B has no dependency on Stream A changes.

**Recommended plan count:** 4 plans (Wave 0 for each stream, Wave 1a, Wave 1b+wiring).

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test (cargo test) |
| Config file | none — workspace Cargo.toml |
| Quick run command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |
| Full suite command | same — no separate integration suite |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LAP-01 | persist_lap skips lap when valid=false | unit | `cargo test -p racecontrol-crate -- lap_tracker` | ✅ lap_tracker.rs (0 tests currently — Wave 0 adds them) |
| LAP-01 | AC adapter sets valid from IS_VALID_LAP field | unit | `cargo test -p rc-agent-crate -- assetto_corsa` | ❌ Wave 0 |
| LAP-02 | Lap below track minimum sets review_required=true | unit | `cargo test -p racecontrol-crate -- lap_tracker` | ❌ Wave 0 |
| LAP-02 | Lap above track minimum does NOT set review_required | unit | `cargo test -p racecontrol-crate -- lap_tracker` | ❌ Wave 0 |
| LAP-03 | LapData session_type populated by AC adapter | unit | `cargo test -p rc-agent-crate -- sims` | ❌ Wave 0 |
| LAP-03 | LapData session_type populated by F1 adapter | unit | `cargo test -p rc-agent-crate -- sims` | ❌ Wave 0 |
| PIN-01 | Customer counter increments on wrong PIN | unit | `cargo test -p racecontrol-crate -- auth` | ❌ Wave 0 |
| PIN-01 | Customer counter does not affect staff counter | unit | `cargo test -p racecontrol-crate -- auth` | ❌ Wave 0 |
| PIN-02 | Staff PIN succeeds even when customer counter is at max | unit | `cargo test -p racecontrol-crate -- auth` | ❌ Wave 0 |
| TELEM-01 | failure_monitor sends TelemetryGap after 60s UDP silence | unit | `cargo test -p rc-agent-crate -- failure_monitor` | ❌ Wave 0 |
| TELEM-01 | failure_monitor does NOT send if billing inactive | unit | `cargo test -p rc-agent-crate -- failure_monitor` | ❌ Wave 0 |
| TELEM-01 | handle_telemetry_gap sends email when game Running + billing active | unit | `cargo test -p racecontrol-crate -- bot_coordinator` | ❌ Wave 0 |
| TELEM-01 | handle_telemetry_gap skips email when game not Running | unit | `cargo test -p racecontrol-crate -- bot_coordinator` | ❌ Wave 0 |
| MULTI-01 | handle_multiplayer_failure is called by ws/mod.rs arm | unit | `cargo test -p racecontrol-crate -- bot_coordinator` | ❌ Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Per wave merge:** same
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `lap_tracker.rs` — tests: valid=false skip, review_required floor check, session_type stored
- [ ] `auth/mod.rs` — tests: customer counter, staff counter independence, PIN-02 staff always succeeds
- [ ] `bot_coordinator.rs` — tests: handle_telemetry_gap game-state guard, handle_multiplayer_failure routing
- [ ] `failure_monitor.rs` — tests: TELEM-01 60s UDP silence detection condition
- [ ] DB migration: `ALTER TABLE laps ADD COLUMN review_required INTEGER NOT NULL DEFAULT 0`
- [ ] DB migration: `ALTER TABLE laps ADD COLUMN session_type TEXT NOT NULL DEFAULT 'practice'`
- [ ] AppState: `customer_pin_failures` and `staff_pin_failures` fields added to state.rs

---

## Open Questions

1. **AC IS_VALID_LAP offset correctness (LOW risk — existing behavior)**
   - What we know: offset 180 is marked "approximate" in source; community docs suggest ~1408+ for CSP extended struct
   - What's unclear: whether offset 180 currently works in practice or returns 0 always
   - Recommendation: Verify on Pod 8 during Wave 1 by deliberately cutting a track (going off-track) and checking rc-agent logs for `valid=false`. Do not change the offset in this phase unless testing confirms it's broken — that investigation is out of scope.

2. **Exact minimum lap times for LAP-02 tracks**
   - What we know: Monza, Silverstone, Spa are required. Conservative floors suggested: 80s/90s/120s.
   - What's unclear: appropriate floors for Racing Point hardware (RTX 4070, Conspit Ares 8Nm)
   - Recommendation: Use the conservative placeholder values above. Add a TODO comment asking Uday to confirm before deploy. The test just checks the flag logic, not the specific values.

3. **MULTI-01 detection: where in rc-agent to put it**
   - What we know: main.rs already tracks `last_ac_status` (variable exists with many reset sites). failure_monitor.rs does not track AcStatus.
   - What's unclear: should detection live in main.rs GameStatusUpdate handler, or add ac_status to FailureMonitorState?
   - Recommendation: Use main.rs GameStatusUpdate handler for Phase 26 — it's simpler and avoids adding new state update sites. Only multiplayer sessions (those with a billing session linked to a group_session_id) should trigger MULTI-01.

4. **Customer PIN counter lockout threshold**
   - What we know: requirements say customer counter has a limit (implied by PIN-02 contrast), staff has none
   - What's unclear: what the threshold is (3? 5? 10?)
   - Recommendation: Default to 5 attempts. Make it a const in auth/mod.rs. Staff will need to reset by starting a new booking from kiosk.

---

## Sources

### Primary (HIGH confidence)

- Direct codebase reading: `crates/rc-agent/src/sims/assetto_corsa.rs` — IS_VALID_LAP handling at line 275, LapData construction at line 295-319
- Direct codebase reading: `crates/rc-agent/src/sims/f1_25.rs` — current_lap_invalid at line 54, session_type resolution at lines 539-544
- Direct codebase reading: `crates/racecontrol/src/lap_tracker.rs` — persist_lap() gate at lines 27-30, INSERT at lines 63-83
- Direct codebase reading: `crates/racecontrol/src/bot_coordinator.rs` — stub locations at lines 92-103
- Direct codebase reading: `crates/racecontrol/src/auth/mod.rs` — validate_pin() at lines 385-528, no counters confirmed
- Direct codebase reading: `crates/racecontrol/src/catalog.rs` — TrackEntry struct at lines 11-17, no min_lap_time_ms
- Direct codebase reading: `crates/rc-agent/src/failure_monitor.rs` — FailureMonitorState fields confirmed, no TELEM-01 arm
- Direct codebase reading: `crates/rc-agent/src/billing_guard.rs` — pattern reference for telem_gap_fired flag design
- Direct codebase reading: `crates/rc-common/src/types.rs` — LapData struct at lines 207-223, no session_type field
- Direct codebase reading: `crates/rc-common/src/protocol.rs` — CoreToAgentMessage variants at lines 152-320, no EngageLockScreen
- Direct codebase reading: `crates/rc-common/src/protocol.rs` — TelemetryGap at lines 117-122, MultiplayerFailure at lines 141-145
- Direct codebase reading: `crates/racecontrol/src/db/mod.rs` — laps table schema at lines 85-100, existing ALTER TABLE migrations at lines 1959-1968
- Direct codebase reading: `.planning/STATE.md` — locked decisions (lap filter, PIN counters, multiplayer scope)
- Direct codebase reading: `.planning/REQUIREMENTS.md` — LAP-01 through MULTI-01 definitions

### Secondary (MEDIUM confidence)

- AC shared memory reference (community-documented): offset 180 for IS_VALID_LAP is approximate; true CSP extended-struct offset is ~1408+

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all infrastructure exists
- Architecture: HIGH — all insertion points confirmed by direct source reading; no guessing
- Pitfalls: HIGH — IS_VALID_LAP offset documented in source; PIN counter separation explicit in requirements; lock screen command gap confirmed by reading CoreToAgentMessage enum; all other pitfalls are pattern-based from Phase 24-25 experience

**Research date:** 2026-03-16
**Valid until:** 2026-04-16 (stable codebase, no external API dependencies)
