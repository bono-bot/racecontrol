# Phase 94 Plan 01 Summary: Backend Pricing & Commitment Ladder

**Status:** Complete
**Commit:** c12fecec

## What was built

1. **DB migration** — `commitment_ladder` column on `drivers` table (TEXT, CHECK constraint: trial/single/package/member, default 'trial')

2. **GET /api/v1/pricing/display** (public) — Returns active pricing tiers with dynamic prices via `compute_dynamic_price()`. Response includes `base_price_paise`, `dynamic_price_paise`, and `has_discount` flag.

3. **GET /api/v1/pricing/social-proof** (public) — Returns `drivers_this_week` (COUNT DISTINCT driver_id) and `sessions_today` (COUNT) from `billing_sessions` with completed/ended_early status. Real counts only, never fabricated.

4. **evaluate_commitment_ladder()** — Added as Step 8 in `post_session_hooks`. Counts non-trial completed sessions per driver, updates ladder position, queues WhatsApp nudge at escalation thresholds (2 sessions → package nudge, 5 sessions → membership nudge) with 7-day dedup via `nudge_queue`.

## Verification
- `cargo check -p racecontrol-crate` passes
- Both endpoints registered in `public_routes()` (no auth required)
- No `.unwrap()` in new code — all `unwrap_or()` pattern
