# Phase 3: Billing Synchronization - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Billing starts when the customer is actually on-track (AC shared memory STATUS=LIVE), not at game launch or PIN auth. Open-ended per-minute billing with retroactive rate tiers. Billing pauses when AC is paused (STATUS=PAUSE) and auto-ends after 10 min paused. Elapsed time + running cost shown as overlay while driving. Game launch failure handling with auto-retry.

</domain>

<decisions>
## Implementation Decisions

### Billing Model — Open-Ended Per-Minute
- **No pre-booked duration** — customer starts driving, billing runs until they stop
- **Per-minute billing** — charged by the minute based on total session duration
- **Two-tier retroactive pricing:**
  - Under 30 min: ₹23.3/min (~₹700/30min equivalent)
  - 30 min and above: ₹15/min (₹900/hr equivalent)
- **Retroactive** — when crossing the 30-min threshold, the cheaper rate applies to the ENTIRE session
  - Example: 15 min = 15 × ₹23.3 = ₹350
  - Example: 45 min = 45 × ₹15 = ₹675 (cheaper than old ₹700 for just 30 min!)
- **Hard max cap** — maximum session length (e.g. 3 hours) to prevent all-day camping

### Billing Trigger
- Billing starts ONLY when AC shared memory `STATUS=2` (LIVE) — car is on-track
- No billable time accrues during game startup, loading screens, or DirectX initialization
- Timer counts **UP** (elapsed), not down — there is no pre-set duration

### Pause Behavior
- Billing **pauses** when AC STATUS=PAUSE (customer hits ESC / pause menu)
- Billing **does NOT pause** for pit lane — only AC system pause
- After **10 minutes** continuously paused, session **auto-ends** (frees pod for next customer)
- Overlay shows **'PAUSED' badge** when billing is paused, elapsed timer freezes visually
- When customer unpauses, billing resumes immediately and PAUSED badge disappears

### Session End Triggers
- **Customer ends via PWA** — taps "End Session" on phone when done
- **Staff ends via kiosk** — staff can end from reception
- **Next booking forces end** — if pod is booked by another customer, 5-min warning then auto-end
- **Hard max cap** — session auto-ends after maximum duration (e.g. 3 hours)
- **Pause timeout** — auto-ends after 10 min continuously paused
- Game process killed cleanly on session end (FFB zeroing is Phase 4 scope)

### Overlay — Taxi Meter Display
- Overlay shows **elapsed time + running cost** (e.g. "15:23 — ₹350")
- At **~25 min**, show **rate upgrade prompt**: "Drive 5 more minutes to unlock ₹15/min rate!" — gamifies the experience, encourages longer play
- When rate tier crosses at 30 min, brief celebration/confirmation that rate dropped
- If next booking approaching, overlay shows warning (e.g. "Pod booked in 10 min")

### Loading Screen Experience
- Overlay appears **immediately at game launch** with "0:00 — ₹0" frozen
- Text shows **"WAITING FOR GAME"** under the frozen display
- When AC reaches STATUS=LIVE, elapsed timer starts counting up and "WAITING FOR GAME" switches to track/car info
- Overlay already partially works via `game_live` flag — extend with explicit text

### Game Launch Failure
- **3-minute timeout** — if AC never reaches STATUS=LIVE within 3 min of launch, declare failure
- **Auto-retry once** — kill AC process, relaunch with same settings automatically
- If second attempt also fails after 3 min, **cancel session entirely (no charge)**
- Customer never made it on-track, so no billing — clean customer experience
- Session can be restarted fresh by customer or staff

### Session Extension (simplified)
- With open-ended billing, there's no explicit "extension" — the session just keeps running
- Customer can extend beyond what they originally planned by simply continuing to drive
- The only limit is the hard max cap or an upcoming booking

### Claude's Discretion
- Exact mechanism for agent→core billing trigger communication (WebSocket message, HTTP call, etc.)
- How to track pause duration (timer in agent vs polling STATUS)
- Whether auto-retry reuses the same billing session ID or creates a new one
- Internal state machine transitions (Pending → Live → Paused → Live → Ended)
- How the per-minute cost calculation integrates with existing `pricing_tiers` table
- Exact hard max cap duration (suggest 3 hours)
- Rate upgrade prompt timing and display duration

</decisions>

<specifics>
## Specific Ideas

- Taxi meter UX: elapsed time + running cost displayed together, like a cab ride. Customer always knows what they're spending.
- Rate upgrade gamification: "Drive 5 more minutes to unlock ₹15/min!" near the 30-min mark. Creates a positive moment that encourages longer sessions and repeat visits.
- Retroactive pricing is a psychological win — customer crossing 30 min feels rewarded, not punished. The "unlock" moment creates positive association with the venue.
- Booking/scheduling system: 15-min booking slots, Rs.100 non-refundable booking fee (deductible from session), advance booking with pod blocking, booking schedule shown on lock screen ("{Nickname} + last 4 digits of phone has booked pod for 8:00 PM"). This is a separate capability, noted below.
- Lock screen should show next booking for the pod so staff knows when the pod is needed

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AssettoCorsaAdapter` (sims/assetto_corsa.rs): Already reads AC shared memory — STATUS field at `graphics::STATUS` offset 4. Values: 0=OFF, 1=REPLAY, 2=LIVE, 3=PAUSE
- `OverlayData.game_live` (overlay.rs:45): Already tracks whether game is live. Currently set from telemetry (speed/RPM), should switch to STATUS=LIVE from shared memory
- `OverlayData.remaining_seconds` / `allocated_seconds` (overlay.rs:39-40): Timer currently counts DOWN — needs refactoring to count UP for elapsed time
- `SessionTimerSection` (overlay.rs:218): Renders timer with color states — needs update for elapsed + cost display
- `DrivingDetector` (driving_detector.rs): Hysteresis-based idle detection with 10s threshold — could inform pause detection
- `billing.rs` in rc-core: Full billing lifecycle — `start_billing_session()`, `end_billing_session()`, `extend_billing_session()` exist. Per-minute calculation and retroactive tiers are new.
- `compute_dynamic_price()` (billing.rs): Already handles time-of-day pricing rules via `pricing_rules` table — pattern for tier-based rate calculation
- `udp_heartbeat.rs`: `billing_active` AtomicBool already tracked in agent status

### Established Patterns
- Agent↔Core communication: WebSocket messages via `rc_common::protocol` (CoreToAgentMessage / AgentToCoreMessage)
- Billing events: `DashboardCommand::StartBilling` already exists in protocol
- Overlay update: `overlay.update_billing(remaining_seconds)` called from billing tick handler — needs update for elapsed model
- State broadcast: Agent sends pod status updates including `billing_active` via UDP heartbeat

### Integration Points
- `auth/mod.rs`: Currently calls `start_billing_session()` at PIN auth — this needs to be deferred until STATUS=LIVE
- `main.rs`: Game launch flow — launches AC, then needs to monitor STATUS for billing trigger
- `overlay.rs:735`: `game_live` detection currently uses telemetry heuristic — should also check STATUS=LIVE
- `billing.rs`: Timer tick logic — needs per-minute elapsed tracking instead of countdown, plus tier-aware cost calculation
- `pricing_tiers` table: May need restructuring or companion table for per-minute rate tiers

</code_context>

<deferred>
## Deferred Ideas

- **Booking/scheduling system** — 15-min slots, advance booking, pod blocking, non-refundable booking fee, booking schedule on lock screen. This is a separate capability (new phase or milestone).
- **Mid-session crash recovery** — If AC crashes mid-drive (not during loading), should billing pause? How to handle reconnect? Separate from initial launch failure.
- **Multi-pod synchronized billing** — Phase 9 handles multiplayer billing sync
- **Time-of-day rate multipliers** — Peak hours (weekend evenings) could have higher per-minute rates. `pricing_rules` table already supports this but not wired to per-minute model.
- **Loyalty/subscription pricing** — Frequent customers could unlock a flat lower per-minute rate. Future feature.

</deferred>

---

*Phase: 03-billing-synchronization*
*Context gathered: 2026-03-13*
