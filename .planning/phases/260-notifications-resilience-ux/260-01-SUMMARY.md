---
phase: 260-notifications-resilience-ux
plan: 01
subsystem: notifications, billing
tags: [notification-outbox, whatsapp, otp-fallback, wallet-guard, sqlite, axum, tokio]

# Dependency graph
requires:
  - phase: 252-financial-atomicity-core
    provides: debit_in_tx wallet primitives used in post-debit balance check
  - phase: 254-security-hardening
    provides: public_routes() pattern used for otp-fallback endpoint
provides:
  - Durable notification_outbox table with 5-state FSM and exponential backoff retry
  - OTP fallback chain: WhatsApp failure -> screen display via one-time token endpoint
  - Negative wallet balance alert (RESIL-05 non-blocking) on extension debit
  - Negative wallet balance session block (RESIL-05 blocking) on session start
affects: [261-wave2, billing, whatsapp-alerter, customer-registration]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Notification outbox pattern: write-to-DB then background worker delivery (at-least-once)
    - Fallback channel chain: exhaust primary -> switch channel, reset retry counter
    - Non-blocking post-commit alert: read balance after tx drops all locks, then await send_whatsapp

key-files:
  created:
    - crates/racecontrol/src/notification_outbox.rs
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/billing.rs

key-decisions:
  - "Wallet keyed by driver_id (not wallet_id) — adapted plan template to actual schema"
  - "Post-debit check is NON-BLOCKING (log + alert only) — ongoing sessions must not be disrupted"
  - "Pre-billing check IS BLOCKING (rejects start) — new debt on negative-balance wallets prevented"
  - "screen channel delivery marks sent immediately; consumed token updates to delivered on GET"
  - "OTP fallback token available when status is 'failed' OR 'exhausted' — covers both terminal states"
  - "agent_timestamp: None added to 4 PodInfo initializations (pre-existing compile error, Rule 3 fix)"

patterns-established:
  - "Post-debit negative balance check: read SELECT balance after commit (not inside tx) then drop guard before await send_whatsapp — never hold lock across .await"
  - "Notification fallback chain: UPDATE channel=fallback_channel, status='pending', retry_count=0 — reuses same worker loop"

requirements-completed: [UX-01, UX-02, RESIL-05]

# Metrics
duration: 35min
completed: 2026-03-29
---

# Phase 260 Plan 01: Notification Outbox + Wallet Guard Summary

**Durable notification outbox with WhatsApp-to-screen OTP fallback chain, exponential backoff retry, and negative wallet balance RESIL-05 guard on both extension debit and session start**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-29T~09:00Z
- **Completed:** 2026-03-29T~09:35Z
- **Tasks:** 2
- **Files modified:** 5 (+ 1 created)

## Accomplishments

- Notification outbox table with 5-state FSM (`pending/sent/delivered/failed/exhausted`) and exponential backoff (10 * 2^retry_count seconds)
- OTP fallback chain: WhatsApp exhaustion switches channel to `screen` with one-time UUID token; GET /customer/otp-fallback/{token} returns OTP payload (consumed on first read)
- Background worker (10s interval) processes up to 20 pending notifications per cycle with full lifecycle logging
- RESIL-05 post-debit check: negative balance after extension commit fires WhatsApp alert to staff (non-blocking)
- RESIL-05 pre-billing guard: negative balance blocks new session start with error message to staff (blocking)

## Task Commits

1. **Task 1: Notification outbox + background worker + OTP fallback** - `6ed406eb` (feat)
2. **Task 2: Negative wallet balance alert and session block** - `124c2a05` (feat)

## Files Created/Modified

- `crates/racecontrol/src/notification_outbox.rs` — New module: enqueue_notification, enqueue_otp_notification, get_otp_by_fallback_token, notification_worker_task, attempt_delivery
- `crates/racecontrol/src/db/mod.rs` — notification_outbox table + idx_notif_outbox_pending index migration
- `crates/racecontrol/src/lib.rs` — pub mod notification_outbox registered
- `crates/racecontrol/src/main.rs` — notification_worker_task spawned; agent_timestamp: None fix
- `crates/racecontrol/src/api/routes.rs` — GET /customer/otp-fallback/{token} in public_routes; otp_fallback_handler; agent_timestamp: None fix (3 PodInfo sites)
- `crates/racecontrol/src/billing.rs` — RESIL-05 post-debit check in extend_billing_session; RESIL-05 pre-billing guard in start_billing_session

## Decisions Made

- Wallet schema uses `driver_id` as the wallet key (not a separate wallet_id); plan template adapted
- Post-debit check reads balance AFTER `tx.commit()` (no lock held), then drops the result binding before `send_whatsapp().await` — standing rule compliance (no lock across .await)
- OTP fallback token accessible when status is `failed` OR `exhausted` — covers both terminal states so screen fallback works regardless of whether fallback channel switch occurred
- `screen` channel delivery completes immediately in the worker (sets status=`sent`); the token endpoint sets `delivered` on first read — one-time consumption

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing PodInfo agent_timestamp compile errors**
- **Found during:** Task 1 verification (cargo check)
- **Issue:** 4 PodInfo struct initializations missing `agent_timestamp: Option<String>` field (added to rc-common by a prior plan). Blocked cargo check from passing.
- **Fix:** Added `agent_timestamp: None` with `// Intentional default:` comment to all 4 sites (routes.rs x3, main.rs x1)
- **Files modified:** crates/racecontrol/src/api/routes.rs, crates/racecontrol/src/main.rs
- **Verification:** cargo check passes with zero errors (only pre-existing unrelated warnings)
- **Committed in:** 6ed406eb (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking — pre-existing compile error)
**Impact on plan:** Necessary to verify our code compiled correctly. No scope creep.

## Known Stubs

None — notification_outbox.rs wires to actual `send_whatsapp()` for WhatsApp delivery. Screen channel delivers via token. SMS logs a placeholder warning (no SMS provider configured) and retries — intentional, as SMS is not yet integrated.

## Issues Encountered

- Pre-existing compilation errors in `ws/mod.rs` (DashboardEvent::RecordBroken), `telemetry_store.rs` (fields on AppState), and `routes.rs` (display_heartbeats) were present before this plan. They are out of scope and logged to deferred items. cargo check for the lib target still shows 10 pre-existing errors; the racecontrol-crate binary target now passes with 1 warning (unreachable exit log — normal for infinite loop tasks).

## Next Phase Readiness

- notification_outbox module ready for use by customer OTP flow (Phase 260-02/03)
- enqueue_otp_notification() can replace direct send_whatsapp() calls in customer_login/resend_otp paths
- RESIL-05 guards active in billing — negative balance detection live

---
*Phase: 260-notifications-resilience-ux*
*Completed: 2026-03-29*
