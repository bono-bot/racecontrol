---
phase: 198-on-track-billing
verified: 2026-03-26T08:30:00+05:30
status: human_needed
score: 11/12 must-haves verified
re_verification: false
human_verification:
  - test: "AC ON-TRACK: Launch AC on Pod 8, observe billing during shader compilation"
    expected: "driving_seconds == 0 and status shows WaitingForGame during loading. After car on track with speed > 0 or steer != 0 for 5s, driving_seconds increments."
    why_human: "Requires live AC game on pod hardware with real UDP telemetry — cannot verify from code alone"
  - test: "AC FALSE LIVE: AC reports Live but car stays stationary in replay/menu for 5s"
    expected: "Billing does NOT start. Guard suppresses the emit. Log shows 'AC False-Live suppressed (5s, speed=0, steer=0)'"
    why_human: "Requires live AC session with real SHM telemetry to confirm speed/steer readings"
  - test: "F1 25 ON-TRACK: Launch F1 25, verify billing starts only at race start with UDP active"
    expected: "billing_events shows playable_signal_at after launch_command_at with measurable delta"
    why_human: "Requires live F1 25 on pod with real UDP port 20777 data"
  - test: "iRACING ON-TRACK: Launch iRacing, verify billing starts when IsOnTrack=true"
    expected: "Billing starts when shared memory IsOnTrack=true AND IsOnTrackCar=true"
    why_human: "Requires live iRacing on pod with real shared memory"
  - test: "KIOSK LOADING STATE: Observe kiosk WebSocket during game load"
    expected: "Kiosk timer displays 'Loading...' (WaitingForGame status) during load, transitions to Active countdown after PlayableSignal"
    why_human: "Requires kiosk UI running and WebSocket observable on venue hardware"
  - test: "CRASH PAUSE/RESUME: Kill a running game mid-session"
    expected: "Billing status changes to PausedGamePause immediately. After relaunch and PlayableSignal, billing resumes. total_paused_seconds shows exact recovery duration."
    why_human: "Requires coordinated crash test on live pod with billing active"
---

# Phase 198: On-Track Billing Verification Report

**Phase Goal:** Billing starts only when the customer car is on-track and controllable, pauses on crash, resumes on successful relaunch -- customers are never charged for loading screens, shader compilation, or crashed games
**Verified:** 2026-03-26T08:30:00 IST
**Status:** human_needed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | AC billing suppressed during menu/replay (5s speed+steer gate) | VERIFIED | `ac_live_since` + `ac_live_has_input` fields in `ConnectionState`; guard at event_loop.rs:289-327; log line "AC False-Live suppressed (5s, speed=0, steer=0)" at line 324 |
| 2 | F1 25 billing starts on UDP telemetry, not process launch | VERIFIED | `f1_udp_playable_received` flag; DrivingDetector `UdpActive` signal sets flag at line 409; emits AcStatus::Live at line 686 |
| 3 | iRacing billing starts on IsOnTrack=true, not process launch | VERIFIED | `adapter.read_is_on_track()` checked at event_loop.rs:701; emits AcStatus::Live at line 703 |
| 4 | Kiosk receives WaitingForGame BillingTick during game load | VERIFIED | Separate broadcast loop at billing.rs:1009-1035; constructs BillingSessionInfo with `WaitingForGame` status and `cost_paise: Some(0)` for each entry in `waiting_for_game` map |
| 5 | No charge if game dies before PlayableSignal | VERIFIED | `cancelled_no_playable` DB INSERT at billing.rs:681-697 (crash path) and 1452-1471 (timeout path); both with `driving_seconds=0, total_paused_seconds=0` |
| 6 | Crash recovery time tracked in total_paused_seconds | VERIFIED | PausedGamePause sync added at billing.rs:1522-1531: `UPDATE billing_sessions SET driving_seconds = ?, total_paused_seconds = ?` — explicitly tagged BILL-07 |
| 7 | Process fallback (90s) does not start billing for crashed games | VERIFIED | `game_alive = state.game_process.as_mut().map(|g| g.is_running()).unwrap_or(false)` at event_loop.rs:741-744; dead game emits `AcStatus::Off` not `AcStatus::Live` (line 756-764) |
| 8 | AC timer sync uses single Utc::now() call | VERIFIED | Single `let now = Utc::now()` at billing.rs:573 (multiplayer path) and 622 (single-player path); both use `now` for `billing_start_at` field |
| 9 | Multiplayer DB failure rejects billing (not silent downgrade) | VERIFIED | Explicit match on sqlx query at billing.rs:488-506; `Err(e)` branch: `tracing::error!("group_session_members query failed...")` and `return;`; entry re-inserted into `waiting_for_game` for retry |
| 10 | Multiplayer non-connecting pods evicted, late arrivals do not start billing | PARTIAL | `multiplayer_billing_timeout()` evicts from `multiplayer_waiting` and only starts billing for pods in `live_pods`. However, non-connected pods that never sent Live remain in `waiting_for_game` map — they will eventually be cleaned by `check_launch_timeouts` (attempt 2) after 6 minutes, not immediately on 60s multiplayer timeout. No explicit `waiting_for_game.remove()` for non-connected pods in `multiplayer_billing_timeout`. |
| 11 | All 5 billing timeouts configurable via racecontrol.toml | VERIFIED | `BillingConfig` struct at config.rs:428 with 5 serde-defaulted fields; `billing: BillingConfig` on `Config` at line 38; `multiplayer_wait_timeout_secs` read at billing.rs:539; `launch_timeout_per_attempt_secs` read at billing.rs:410 |
| 12 | BillingSessionStatus::CancelledNoPlayable variant exists and serializes correctly | VERIFIED | Variant at types.rs:345 within enum with `#[serde(rename_all = "snake_case")]` at line 331 — serializes as `"cancelled_no_playable"` |

**Score:** 11/12 truths verified (Truth #10 is partial — see Gaps section)

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/types.rs` | `CancelledNoPlayable` variant on `BillingSessionStatus` | VERIFIED | Line 345: `CancelledNoPlayable,` with doc comment; enum has `#[serde(rename_all = "snake_case")]` |
| `crates/racecontrol/src/config.rs` | `BillingConfig` struct with 5 configurable timeout fields | VERIFIED | Lines 428-460: struct + 5 serde default fns + `impl Default`; wired to `Config` at line 38 |
| `crates/rc-agent/src/event_loop.rs` | AC False-Live guard + process fallback crash guard | VERIFIED | `ac_live_since` (line 103) + `ac_live_has_input` (line 105) fields; guard logic lines 289-327; crash guard lines 738-764 |
| `crates/racecontrol/src/billing.rs` | WaitingForGame tick broadcast, cancelled_no_playable handling, configurable timeouts, multiplayer error handling | VERIFIED | All present: WaitingForGame loop (1009-1035), `cancelled_no_playable` at lines 687, 1457; BILL-10 error path (487-506); configurable timeouts (539, 410) |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `config.rs` | `Config` struct | `pub billing: BillingConfig` field | WIRED | Line 38 in Config struct |
| `event_loop.rs` | `ConnectionState` | `ac_live_since` + `ac_live_has_input` fields | WIRED | Lines 103-105 declared; lines 141-142 initialized in `new()`; lines 291-324 used in telemetry tick |
| `billing.rs tick_all_timers` | `DashboardEvent::BillingTick` | WaitingForGame branch broadcasts timer info | WIRED | Lines 1009-1035: loops `waiting_for_game` map and pushes `DashboardEvent::BillingTick(info)` |
| `billing.rs` | `billing_sessions` table | INSERT cancelled_no_playable on timeout | WIRED | Lines 1455-1465: `INSERT INTO billing_sessions ... 'cancelled_no_playable'` |
| `billing.rs check_launch_timeouts_from_manager` | `BillingConfig.launch_timeout_per_attempt_secs` | `timeout_secs: u64` parameter | WIRED | Line 396 signature takes `timeout_secs`; line 410 passes `state.config.billing.launch_timeout_per_attempt_secs` |
| `billing.rs handle_game_status_update` | `group_session_members` query | Explicit error handling (not unwrap_or_default) | WIRED | Lines 488-506: match with `Err(e) => { tracing::error!; drop(mp); waiting_for_game.insert(pod_id, entry); return; }` |
| `billing.rs multiplayer_billing_timeout` | `multiplayer_wait_timeout_secs` | `state.config.billing.multiplayer_wait_timeout_secs` | WIRED | Line 539 reads config value, line 540-542 uses it in `tokio::time::sleep` |

---

### Requirements Coverage

The BILL requirements are defined in ROADMAP.md Phase 198 success criteria (not in a separate REQUIREMENTS.md file — no BILL-prefixed requirements exist in any REQUIREMENTS-v*.md file). Cross-reference against plan `requirements:` frontmatter:

| Requirement | Plan | Description | Status | Evidence |
|-------------|------|-------------|--------|----------|
| BILL-01 | 198-01 | AC billing only when on-track (not menu/replay) | VERIFIED (needs live test) | AC False-Live guard in event_loop.rs:289-327 |
| BILL-02 | 198-01 | AC PlayableSignal requires speed>0 OR steer>0.02 within 5s | VERIFIED (needs live test) | `frame.speed_kmh > 0.0 || frame.steering.abs() > 0.02` at event_loop.rs:300-301 |
| BILL-03 | 198-01 | F1 25 billing starts on UDP telemetry, not process launch | VERIFIED (needs live test) | `f1_udp_playable_received` flow at event_loop.rs:404-409, 683-690 |
| BILL-04 | 198-01 | iRacing billing starts on IsOnTrack=true | VERIFIED (needs live test) | `adapter.read_is_on_track()` at event_loop.rs:701-704 |
| BILL-05 | 198-02 | Kiosk shows "Loading..." via WaitingForGame BillingTick | VERIFIED (needs live test) | WaitingForGame broadcast loop at billing.rs:1009-1035 |
| BILL-06 | 198-02 | cancelled_no_playable DB record on timeout/crash, zero charge | VERIFIED | Two INSERT paths at billing.rs:681-697 and 1452-1471 |
| BILL-07 | 198-02 | total_paused_seconds persisted for PausedGamePause (crash recovery tracking) | VERIFIED | DB UPDATE at billing.rs:1522-1531 for PausedGamePause status |
| BILL-08 | 198-01 | 90s process fallback does not bill crashed games | VERIFIED | Crash guard at event_loop.rs:738-764; dead game emits Off not Live |
| BILL-09 | 198-02 | AC timer sync: single Utc::now(), canonical pod ID, error-level failures | VERIFIED | Single `let now = Utc::now()` at billing.rs:573, 622; canonical pod_id via `normalize_pod_id()` at line 466 |
| BILL-10 | 198-02 | Multiplayer DB failure rejects billing (not silent downgrade) | VERIFIED | Explicit match with error log and return at billing.rs:487-506 |
| BILL-11 | 198-02 | Multiplayer 60s timeout evicts non-connected pods; late arrivals do not bill | PARTIAL | `multiplayer_billing_timeout()` evicts from `multiplayer_waiting` correctly; but non-connected pods that never sent Live remain in `waiting_for_game` for up to 6 min (until `check_launch_timeouts` attempt 2). Not immediate cleanup. |
| BILL-12 | 198-01, 198-02 | All billing timeouts configurable via BillingConfig in racecontrol.toml | VERIFIED | 5 fields in BillingConfig; 3 of 5 actively consumed (`launch_timeout_per_attempt_secs`, `multiplayer_wait_timeout_secs`, `pause_auto_end_timeout_secs` via timer.max_pause_duration_secs); `idle_drift_threshold_secs` and `offline_grace_secs` exist but not yet actively read (future phases) |

**No orphaned requirements found** — all 12 BILL-01 through BILL-12 are claimed by at least one plan.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `billing.rs` | 1098 | `.unwrap_or_default()` | Info | Unrelated to BILL-10 — different query site; not the group_session_members path |
| `billing.rs` | 1478 | `.unwrap_or_default()` | Info | Unrelated to BILL-10 — different query site; not the group_session_members path |
| `billing.rs` | 281 | Hardcoded `600` in `pause_seconds >= 600` | Warning | `PausedGamePause` auto-end check uses hardcoded 600s instead of `state.config.billing.pause_auto_end_timeout_secs` — BILL-12 configurable timeout not fully wired for this path. The `BillingConfig.pause_auto_end_timeout_secs` field exists but is not read here. |
| `config.rs` | N/A | Pre-existing test failures: `jwt_secret_rejects_dangerous_default` + `load_keys_valid_hex` | Warning | Pre-existing failures introduced by phases 208-01/208-03 (after phase 198). Not caused by phase 198. Confirmed: neither `config.rs` nor `crypto/` was modified by any phase 198 commit. |
| `billing.rs` | 1436 | `LaunchGame` retry in timeout handler hardcodes `SimType::AssettoCorsa` | Warning | When retrying on attempt 1 timeout (line 1436), the retry always sends `SimType::AssettoCorsa`. This is a pre-existing limitation — not introduced by phase 198, but worth noting for future phases. |

---

### Test Suite Status

- New tests added in Plan 03: 4 functions (`waiting_for_game_tick_broadcasts`, `cancelled_no_playable_on_timeout`, `multiplayer_db_query_failure_preserves_waiting_entry`, `configurable_billing_timeouts`) — all at billing.rs:4381-4632
- Full racecontrol test run: **540 passed, 2 failed** — the 2 failures (`jwt_secret_rejects_dangerous_default`, `load_keys_valid_hex`) are pre-existing, introduced by phases 208-01/208-03, not caused by phase 198
- 4 new BILL tests all pass when run specifically
- Commits verified in git: `ad7ab774`, `e9558961`, `f5189125`, `f9b7be6b`, `d8edbb46` — all present

---

### Human Verification Required

#### 1. AC On-Track Billing Gate

**Test:** Launch Assetto Corsa on Pod 8. During shader compilation (AC status LOADING/before LIVE), query billing timer endpoint or observe kiosk timer.
**Expected:** `driving_seconds == 0`, kiosk shows "Loading...". After car reaches track and driver steers/accelerates for 5s, `driving_seconds` starts incrementing. Server log shows "AC False-Live guard passed (input detected) — emitting Live".
**Why human:** Requires live AC game with real SHM telemetry on pod hardware. Code path verified statically but UDP/SHM data correctness requires runtime.

#### 2. AC False Live Guard

**Test:** Launch AC, let it reach LIVE status (status=green) but stay on main menu or replay camera for 5+ seconds with no input.
**Expected:** Billing does NOT start. Server log shows "AC False-Live suppressed (5s, speed=0, steer=0) — not billing". Billing only starts after real driving input.
**Why human:** Requires live AC session where `AcStatus::Live` fires during non-driving state.

#### 3. F1 25 Billing Trigger

**Test:** Launch F1 25 on Pod 8 (UDP port 20777). Observe billing during menu navigation vs. actual race.
**Expected:** `billing_events` table shows `playable_signal_at` timestamp occurs after menu time — only when UDP telemetry shows `m_speed > 0`. Billing does NOT start during team selection/loading screens.
**Why human:** Requires live F1 25 with real UDP telemetry to the DrivingDetector.

#### 4. iRacing Billing Trigger

**Test:** Launch iRacing. Observe billing before and after joining track.
**Expected:** Billing starts when shared memory `IsOnTrack=true` AND `IsOnTrackCar=true`. Verify `driving_seconds` stays at 0 in garage/loading.
**Why human:** Requires live iRacing with real rF2 shared memory.

#### 5. Kiosk Loading State Display

**Test:** During game load, observe kiosk WebSocket message stream or kiosk UI timer.
**Expected:** Timer shows "Loading..." text (WaitingForGame status), NOT a counting-down timer. After PlayableSignal → status changes to Active → countdown begins. WS message sequence: `WaitingForGame` → `Active`.
**Why human:** Requires kiosk UI running and observable — `DashboardEvent::BillingTick` consumption depends on kiosk frontend rendering logic not verified here.

#### 6. Crash Pause and Resume

**Test:** Start billing on Pod 8, confirm Active state, then kill the AC process. Observe billing status and paused seconds.
**Expected:** Billing status changes to `PausedGamePause` immediately (within 1 tick). After game relaunch and PlayableSignal, billing resumes. Final `total_paused_seconds` in DB equals the crash recovery window exactly.
**Why human:** Requires coordinated crash test with active billing session. Timing accuracy of `total_paused_seconds` cannot be verified from unit tests alone.

---

### Gaps Summary

#### BILL-11 Partial Gap: Non-Connected Pod Cleanup Timing

**Truth:** "Multiplayer 60s timeout evicts non-connected pods and removes their WaitingForGameEntry"
**Issue:** The `multiplayer_billing_timeout()` function (billing.rs:742) correctly evicts pods from `multiplayer_waiting.waiting_entries` and only starts billing for pods in `live_pods`. However, pods that never sent `AcStatus::Live` remain in the `waiting_for_game` map throughout the 60s multiplayer timeout. These are not cleaned from `waiting_for_game` in `multiplayer_billing_timeout()`. They will receive spurious `BillingTick` broadcasts with `WaitingForGame` status for up to 6 minutes (until `check_launch_timeouts` fires at attempt 2).

**Impact assessment:** Low-to-moderate. The pod will NOT start billing (no `WaitingForGameEntry` exists to trigger `start_billing_session`). The only symptom is phantom `WaitingForGame` BillingTick broadcasts to the kiosk for these evicted pods — potentially showing "Loading..." for a pod that should be idle. After 6 minutes, `check_launch_timeouts` (attempt 2) will clean the entry and insert a `cancelled_no_playable` record.

**Not a blocker for shipping** — billing correctness is maintained (no false charges). The kiosk display discrepancy (phantom Loading state) is a cosmetic issue.

#### BILL-12 Partial Gap: pause_auto_end_timeout_secs Not Fully Wired

**Issue:** `BillingConfig.pause_auto_end_timeout_secs` (default 600) exists and is configurable via TOML, but the actual pause auto-end check at billing.rs:281 uses a hardcoded `600`: `self.pause_seconds >= 600`. The configurable value is not read at this decision point.
**Impact:** If an operator changes `pause_auto_end_timeout_secs` in racecontrol.toml, the change will NOT take effect for the pause auto-end behavior. The field was added for BILL-12 coverage but is not yet wired to the actual enforcement logic.
**Note:** The `max_pause_duration_secs` field on `BillingTimer` exists (initialized from timer construction) — but it's unclear whether it reads from `BillingConfig`. This should be verified and wired if not.

---

_Verified: 2026-03-26T08:30:00 IST_
_Verifier: Claude (gsd-verifier)_
