---
phase: 03-billing-synchronization
verified: 2026-03-14T12:00:00Z
status: gaps_found
score: 10/12 must-haves verified
gaps:
  - truth: "BillingTick carries elapsed_seconds, cost_paise, rate_per_min_paise, paused, minutes_to_value_tier as Optional fields alongside legacy remaining_seconds/allocated_seconds"
    status: partial
    reason: "The struct fields exist in protocol.rs with correct serde, but tick_all_timers() sends ALL new Optional fields as None to agents. The agent_ticks tuple only collects (pod_id, remaining, allocated, driver_name) — no cost data. So agents always receive legacy-format BillingTick and can never trigger update_billing_v2(). The overlay taxi meter is structurally complete but will never activate from core-sent ticks."
    artifacts:
      - path: "crates/rc-core/src/billing.rs"
        issue: "Lines 513-525: agent_ticks.push stores only 4 fields (pod_id, remaining, allocated, driver_name). The BillingTick send (line 515) hardcodes elapsed_seconds: None, cost_paise: None, rate_per_min_paise: None, paused: None, minutes_to_value_tier: None for every active session tick."
    missing:
      - "In tick_all_timers(): change agent_ticks type from Vec<(String, u32, u32, String)> to Vec<(String, u32, u32, String, u32, i64, i64, bool, Option<u32>)> or a struct to carry elapsed/cost/rate/paused/minutes_to_value_tier"
      - "Populate elapsed_seconds, cost_paise, rate_per_min_paise from timer.current_cost() when building agent_ticks"
      - "Set paused=true when timer.status == PausedGamePause and send a BillingTick to agent so overlay shows PAUSED badge"
      - "Set minutes_to_value_tier from timer.current_cost().minutes_to_next_tier"
  - truth: "Overlay shows 'PAUSED' badge and freezes the elapsed timer when AC STATUS=PAUSE"
    status: partial
    reason: "The overlay rendering code for PAUSED badge exists and is correct. However, the core tick loop skips PausedGamePause timers entirely (line 412: 'if timer.status != BillingSessionStatus::Active { continue; }'). No BillingTick is sent to the agent while in PausedGamePause state, so the agent's overlay.update_billing_v2(paused=true) is never called from core. The agent only knows about pause if it reads AcStatus::Pause directly — which it does via GameStatusUpdate — but the overlay paused state is only set by update_billing_v2(), not by the GameStatusUpdate handler."
    artifacts:
      - path: "crates/rc-core/src/billing.rs"
        issue: "tick_all_timers() skips PausedGamePause with 'continue' at line 412 without sending a BillingTick to the agent for that state"
      - path: "crates/rc-agent/src/main.rs"
        issue: "GameStatusUpdate handler (STATUS polling) sends GameStatusUpdate to core but does not update the overlay's paused state directly — only update_billing_v2 sets overlay.paused"
    missing:
      - "In tick_all_timers(): add explicit handling for PausedGamePause timers that sends BillingTick to agent with paused=true and current elapsed/cost values (similar to how PausedDisconnect sends dashboard events)"
      - "OR: in main.rs, when GameStatusUpdate sends AcStatus::Pause to core, also directly call overlay.update_billing_v2 with paused=true to keep agent overlay in sync without waiting for core tick"
human_verification:
  - test: "Deploy to Pod 8, launch AC, validate billing only starts on STATUS=LIVE"
    expected: "Billing session not created at PIN entry; created only when AC STATUS reaches 2 (LIVE)"
    why_human: "Requires AC runtime and shared memory on pod hardware"
  - test: "Drive on Pod 8 for 2+ minutes, observe overlay"
    expected: "Overlay shows elapsed time counting up and running cost (Rs.X format); NOT countdown"
    why_human: "Requires deployed binaries with BillingTick gap fix applied first; visual verification"
  - test: "Press ESC during AC session, observe overlay"
    expected: "PAUSED badge appears, timer freezes, cost freezes"
    why_human: "Requires AC runtime; also depends on BillingTick gap fix being applied"
  - test: "Drive for 25+ minutes, observe rate upgrade prompt"
    expected: "Text appears near 25 min: 'Drive X more min for Rs.15/min!'"
    why_human: "Requires sustained AC session; also depends on BillingTick gap fix"
  - test: "Drive past 30 minutes, observe VALUE RATE UNLOCKED celebration"
    expected: "Green 'VALUE RATE UNLOCKED!' text appears for ~10 seconds at tier crossing"
    why_human: "Requires sustained AC session; also depends on BillingTick gap fix"
---

# Phase 3: Billing Synchronization Verification Report

**Phase Goal:** Customers are billed only for time spent actually driving on-track, not loading screens or DirectX initialization
**Verified:** 2026-03-14
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | AcStatus enum exists with Off/Replay/Live/Pause variants and round-trips through serde | VERIFIED | `types.rs:275-284` — all 4 variants present. `ac_status_serde_roundtrip_all_variants` test passes. |
| 2 | GameStatusUpdate message variant exists in AgentMessage for agent->core STATUS reporting | VERIFIED | `protocol.rs:57` — `GameStatusUpdate { pod_id: String, ac_status: AcStatus }`. Tests `test_game_status_update_roundtrip` and `test_game_status_update_all_ac_statuses` pass. |
| 3 | BillingTick carries elapsed_seconds, cost_paise, rate_per_min_paise, paused, minutes_to_value_tier as Optional fields | PARTIAL | Fields defined correctly in `protocol.rs:134-148` with serde(default). BUT `tick_all_timers()` sends all as None. Agent overlay will never receive count-up data. |
| 4 | compute_session_cost returns correct amounts for both tiers with retroactive crossing at 30 min | VERIFIED | `billing.rs:75-98`. Tests: `cost_zero_seconds`, `cost_15_minutes_standard_tier`, `cost_30_minutes_retroactive_value_tier` (45000 paise), `cost_45_minutes_value_tier` all pass. |
| 5 | BillingTimer counts UP via elapsed_seconds instead of DOWN via remaining_seconds | VERIFIED | `billing.rs:134,180-182` — `elapsed_seconds` field, `tick()` increments it. `timer_countup_active_increments_elapsed` test passes. |
| 6 | BillingTimer.tick() freezes elapsed during PausedGamePause and increments pause_seconds | VERIFIED | `billing.rs:184-187`. `timer_paused_game_pause_freezes_elapsed_increments_pause` test passes. |
| 7 | 10-minute pause timeout and 3-hour hard max cap auto-end the session | VERIFIED | `billing.rs:186` (pause_seconds >= 600), `billing.rs:182` (elapsed >= max_session_seconds). Tests `timer_pause_timeout_triggers_end` and `timer_hard_max_cap_triggers_end` pass. |
| 8 | Overlay shows elapsed time + running cost in taxi meter format while driving | PARTIAL | Rendering code exists in `overlay.rs:291-314`. But depends on `update_billing_v2` being called, which requires core to send non-None BillingTick fields. Core does not send them. |
| 9 | Overlay shows 'PAUSED' badge and freezes the elapsed timer when AC STATUS=PAUSE | PARTIAL | Rendering code at `overlay.rs:270-289`. `test_update_billing_v2_paused` passes. But core tick loop skips PausedGamePause timers — no BillingTick with paused=true is ever sent to agent from core. |
| 10 | read_ac_status() on SimAdapter trait returns AcStatus from shared memory | VERIFIED | `sims/mod.rs:34` — default None impl. `assetto_corsa.rs:418-428` — Windows real read, non-Windows None stub. Test `test_ac_status_read_non_windows` passes. |
| 11 | Agent polls AC STATUS and sends GameStatusUpdate to core on transitions | VERIFIED | `main.rs:590-609` — debounce logic, sends `AgentMessage::GameStatusUpdate` after 1-second stability. LaunchState updated to Live on STATUS=LIVE. |
| 12 | rc-core WebSocket handler has a match arm for AgentMessage::GameStatusUpdate that starts billing on AcStatus::Live | VERIFIED | `ws/mod.rs:383-387` — dispatches to `billing::handle_game_status_update()`. `game_status_live_on_paused_game_pause_resumes_billing` and related tests pass. |

**Score:** 10/12 truths verified (2 partial)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/rc-common/src/types.rs` | AcStatus enum, WaitingForGame+PausedGamePause variants, Optional fields on BillingSessionInfo | VERIFIED | All present: AcStatus lines 275-284, BillingSessionStatus lines 288-302, BillingSessionInfo lines 304-332 with Optional fields. |
| `crates/rc-common/src/protocol.rs` | GameStatusUpdate agent message, Optional fields on BillingTick | VERIFIED | GameStatusUpdate at line 57, BillingTick with 5 Optional fields at lines 129-148. Tests pass. |
| `crates/rc-core/src/billing.rs` | compute_session_cost(), BillingTimer count-up, WaitingForGameEntry, handle_game_status_update, defer_billing_start, check_launch_timeouts | VERIFIED (with gap) | All present and correct. GAP: tick_all_timers() does not populate new Optional fields in BillingTick sent to agents. |
| `crates/rc-agent/src/sims/mod.rs` | SimAdapter::read_ac_status() trait method | VERIFIED | Line 34: `fn read_ac_status(&self) -> Option<AcStatus> { None }` |
| `crates/rc-agent/src/sims/assetto_corsa.rs` | AssettoCorsaAdapter::read_ac_status() from shared memory | VERIFIED | Lines 418-433: Windows reads graphics::STATUS, non-Windows returns None. Test passes. |
| `crates/rc-agent/src/overlay.rs` | Taxi meter OverlayData fields, activate_v2, update_billing_v2, format_cost, PAUSED badge rendering, WAITING FOR GAME, 30-min celebration | VERIFIED (rendering only) | All rendering and data model correct. GAP: will not render taxi meter unless core sends non-None BillingTick Optional fields. |
| `crates/rc-agent/src/main.rs` | AC STATUS polling with debounce, GameStatusUpdate sending, LaunchState machine, BillingTick v2 handler | VERIFIED | LaunchState machine at lines 173-183. Status polling at 590-609. BillingTick v2 handler at 963-976. |
| `crates/rc-core/src/ws/mod.rs` | AgentMessage::GameStatusUpdate match arm | VERIFIED | Lines 383-387: matches and dispatches to billing::handle_game_status_update(). |
| `crates/rc-core/src/auth/mod.rs` | All 4 auth call sites using defer_billing_start | VERIFIED | 4 call sites confirmed at lines 425, 544, 650, 1218. All set pod state to WaitingForGame. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `billing.rs` | `types.rs` | Uses AcStatus, PausedGamePause, WaitingForGame | WIRED | `rc_common::types::AcStatus` imported. PausedGamePause used in tick() and handle_game_status_update(). |
| `billing.rs` | `protocol.rs` | BillingTick includes elapsed/cost fields in struct | PARTIAL | Struct has fields. But tick_all_timers() sends them all as None. |
| `ws/mod.rs` | `billing.rs` | GameStatusUpdate arm calls handle_game_status_update() | WIRED | Line 386: `billing::handle_game_status_update(&state, pod_id, *ac_status, &cmd_tx).await` |
| `assetto_corsa.rs` | `types.rs` | read_ac_status() returns AcStatus | WIRED | `AcStatus` imported at line 4. Windows impl maps raw i32 to AcStatus variants. |
| `main.rs` | `protocol.rs` | Sends AgentMessage::GameStatusUpdate, reads BillingTick Optional fields | WIRED | GameStatusUpdate sent at line 596-600. BillingTick destructured with all Optional fields at lines 963-976. |
| `main.rs` | `overlay.rs` | Calls update_billing_v2 when Optional fields present | PARTIAL | Call exists at line 966, but only reached if elapsed_seconds/cost_paise/rate_per_min_paise are Some — which core never sends. |
| `auth/mod.rs` | `billing.rs` | Auth no longer calls start_billing_session directly | WIRED | All 4 sites use defer_billing_start(). WaitingForGame state set post-auth. |

### Requirements Coverage

| Requirement | Source Plans | Description | Status | Evidence |
|-------------|-------------|-------------|--------|---------|
| BILL-01 | 03-01, 03-02, 03-03 | Billing timer starts when AC STATUS=LIVE, not at game process launch | SATISFIED | defer_billing_start() + handle_game_status_update(Live) pattern fully implemented. 3-min launch timeout with retry on both agent (LaunchState) and core (check_launch_timeouts) sides. Tests: `game_status_live_on_paused_game_pause_resumes_billing`, `launch_timeout_detected_after_180s`, `launch_timeout_attempt_2_cancels_with_no_charge` all pass. |
| BILL-02 | 03-01, 03-03 | DirectX initialization delay does not count as billable time | SATISFIED | WaitingForGame status in BillingTimer.tick() returns false with no increment (line 188). Billing only starts when GameStatusUpdate(Live) is received. Test: `timer_waiting_for_game_no_increments` passes. |
| BILL-06 | 03-02 | Session time remaining displayed as overlay during gameplay | PARTIAL | Overlay rendering is implemented (taxi meter, PAUSED badge, WAITING FOR GAME, 30-min celebration). However core does not send elapsed/cost data in BillingTick — agent always falls back to legacy countdown display. Customer sees countdown (remaining_seconds) not taxi meter (elapsed + cost) in practice. |

No orphaned requirements — BILL-01, BILL-02, BILL-06 are the only Phase 3 requirements per REQUIREMENTS.md traceability table.

### Anti-Patterns Found

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| `crates/rc-core/src/billing.rs` lines 513-525 | BillingTick sent to agents with all new Optional fields hardcoded as None | BLOCKER | Overlay taxi meter never activates. BILL-06 display of elapsed time + cost is broken end-to-end. |
| `crates/rc-core/src/billing.rs` lines 411-414 | PausedGamePause timers silently skipped in tick loop — no BillingTick sent to agent | BLOCKER | Agent overlay PAUSED badge cannot be triggered by core. No tick-based paused state update flows to overlay. |
| `crates/rc-core/src/auth/mod.rs` line 241 | `link_reservation_to_billing` function is never used (compiler warning) | INFO | Unused function left by auth decoupling. Not a functional issue but indicates incomplete cleanup. |

### Human Verification Required

#### 1. Billing Starts on LIVE — Not at Auth

**Test:** Deploy rc-core and rc-agent to Pod 8. Authenticate a session (PIN entry). Watch rc-core logs and billing dashboard.
**Expected:** No billing session row created at PIN entry. Billing session created only when AC game reaches STATUS=2 (LIVE).
**Why human:** Requires AC runtime + shared memory on pod hardware. Cannot verify in unit tests.

#### 2. Taxi Meter Overlay Display

**Test:** After BillingTick gap fix is applied — Deploy to Pod 8, start a session, drive in AC.
**Expected:** Overlay shows elapsed time counting up (e.g. "15:23") and running cost in Rs. format (e.g. "Rs.350"), not countdown.
**Why human:** Requires deployed fix + AC runtime + visual inspection.

#### 3. PAUSED Badge on ESC

**Test:** After BillingTick gap fix — During an active AC session, press ESC.
**Expected:** Overlay shows "PAUSED" badge in red background, timer freezes, cost freezes.
**Why human:** Requires AC runtime + visual inspection.

#### 4. Rate Upgrade Prompt at 25 Minutes

**Test:** Drive for 25+ minutes.
**Expected:** Small green text appears: "Drive X more min for Rs.15/min!" where X counts down from 5.
**Why human:** Requires sustained session. Depends on BillingTick gap fix.

#### 5. VALUE RATE UNLOCKED Celebration at 30 Minutes

**Test:** Drive through the 30-minute threshold.
**Expected:** Green "VALUE RATE UNLOCKED!" text appears for ~10 seconds. Rate drops retroactively: cost resets to 30 * 1500 paise.
**Why human:** Requires sustained session. Depends on BillingTick gap fix.

### Gaps Summary

Two related gaps share the same root cause: the `tick_all_timers()` function in `billing.rs` does not populate the new Optional fields when sending `BillingTick` messages to agents.

**Root cause:** `agent_ticks` is a `Vec<(String, u32, u32, String)>` tuple collecting only legacy fields. It does not capture `elapsed_seconds`, `cost_paise`, `rate_per_min_paise`, `paused`, or `minutes_to_value_tier` from the timer. The send at lines 515-524 hardcodes all new fields as `None`.

**Consequence 1 (BILL-06):** The agent's `BillingTick` handler checks `if let (Some(elapsed), Some(cost), Some(rate)) = ...` before calling `update_billing_v2()`. Since these are always None, the overlay always falls back to legacy `update_billing(remaining_seconds)` — the taxi meter UI never activates.

**Consequence 2 (PAUSED badge):** PausedGamePause timers are skipped entirely by the `continue` at line 412 — no BillingTick at all is sent during game-pause state. The agent overlay cannot show the PAUSED badge based on core ticks. (Note: the agent *could* be modified to set paused state directly from GameStatusUpdate on its own side, but that code path is also not wired.)

**Fix scope:** Single function in `billing.rs`: change `agent_ticks` to carry cost data and populate Optional fields in the BillingTick send. Also add a PausedGamePause tick path that sends `paused=true` to the agent. Estimated 20-30 lines of targeted change.

**What IS fully working:**
- Core billing lifecycle (deferred start, LIVE trigger, pause/resume) — functionally complete and tested
- BILL-01 and BILL-02 are satisfied — customers are not charged for loading time
- All 110 relevant tests pass (68 rc-common + 28 rc-core billing + 14 rc-agent)
- The overlay rendering code is correct and would work once it receives data

---

_Verified: 2026-03-14_
_Verifier: Claude (gsd-verifier)_
