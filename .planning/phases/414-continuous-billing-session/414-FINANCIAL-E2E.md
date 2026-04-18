# Phase 414 — Venue Financial Flow E2E Test

**Status:** PENDING operator execution at venue.
**Required:** Human at venue in Pod 8 with a test driver wallet.
**Deployed state at test time:**
- Server .23 racecontrol: `68f4d61e` (contains 414-01..414-06-Task1 + BILL-14 + CRASH-CASCADE + SIM-ARGS-FIX commits)
- Pod 8 rc-agent: `5f80fc6a` (pre-414 — billing flow lives in server, so rc-agent version is not gating)
- Kiosk frontend on .23: verify with `curl http://192.168.31.23:3300/kiosk/api/health/deep` before starting
- Cloud: parity not required for this test — venue-only financial flow

**Prerequisites (check before starting each test):**
1. `curl -s http://192.168.31.23:8080/api/v1/health` returns `build_id=68f4d61e` (or newer)
2. `curl -s http://192.168.31.91:8090/health` returns `build_id=5f80fc6a` (or newer) + `ws_connected=true` in fleet health
3. Pod 8 kiosk shows driver-facing screen
4. Top up test driver to known starting balance (suggest ₹1000 = 100000 paise)
5. Note: `test_start_timestamp`, `test_driver_id`, `starting_balance_paise` in each table below

---

## TEST 1 — Cumulative snap across single-game swap (PRIMARY)

**Starting state:** driver=`<fill>`, balance_paise=`<fill>`, session_id=N/A, timestamp_start=`<fill>`

| Step | Action | Expected | Actual |
|------|--------|----------|--------|
| 1 | Start session at kiosk for the test driver | Session created, status=waiting_for_game, elapsed=0 | __ |
| 2 | Launch AC | status=Active, meter ticks up | __ |
| 3 | Drive AC for exactly 10 minutes | elapsed_seconds=600, cost_paise=25000 (₹250) | __ |
| 4 | Exit AC cleanly (Esc → quit) | Within 5s: status=waiting_for_game (NOT EndedEarly), meter shows PAUSED — BETWEEN GAMES, frozen at ₹250 | __ |
| 5 | Wait 2 minutes (no game running) | Meter still ₹250, idle counter increments invisibly server-side | __ |
| 6 | Tap "Continue with another game" → select F1 25 | Game picker opens, F1 25 launches, status=Active, meter resumes ticking from ₹250 | __ |
| 7 | Drive F1 25 for exactly 10 minutes | elapsed_seconds=1200, cost_paise=50000 (₹500) — NOT ₹250+₹250 separately | __ |
| 8 | Tap End Session at kiosk → confirm | ConfirmDialog opens, confirm, session ends as Completed | __ |
| 9 | Check final wallet balance | starting_balance - cost = 100000 - 50000 = 50000 paise (₹500) | __ |
| 10 | Check `billing_sessions` DB row | status=completed, total_debited_paise=50000, elapsed_seconds=1200 | __ |

**Verdict:** __ (PASS / FAIL)  
**Wallet delta:** starting `<fill>` − ending `<fill>` = `<fill>` paise. Expected: 50000. Match: __

---

## TEST 2 — Cumulative snap CROSSES the 30-min threshold

**Starting state:** driver=`<fill>`, balance_paise=`<fill>`, session_id=N/A, timestamp_start=`<fill>`

| Step | Action | Expected | Actual |
|------|--------|----------|--------|
| 1 | Top up driver to known balance | __ | __ |
| 2 | Start session, launch AC, drive 25 minutes | elapsed=1500, cost=62500 (₹625) | __ |
| 3 | Stop AC → wait 5 min → launch F1 25 → drive 5 minutes | elapsed=1800 (30 min), final cost = ₹700 (snap to 30-min pkg, NOT 30 × ₹25 = ₹750) | __ |
| 4 | End session, check wallet delta | starting - 70000 = ending balance | __ |

**Verdict:** __ (PASS / FAIL)  
**Snap math observed:** `<fill>` paise. Expected: 70000 (₹700 snap). Match: __

---

## TEST 3 — IdleWarning + 15-min auto-end (LONG)

**Starting state:** driver=`<fill>`, balance_paise=`<fill>`, session_id=N/A, timestamp_start=`<fill>`

| Step | Action | Expected | Actual |
|------|--------|----------|--------|
| 1 | Start session, launch AC, drive 5 min, then exit AC | status=waiting_for_game, elapsed=300 | __ |
| 2 | Wait 10 min without launching another game | At 10:00, IdleWarningDialog appears with countdown 5:00, balance display, "Tap to continue" + "End session now" CTAs | __ |
| 3 | Wait another 5 min | At 15:00 total idle, session auto-ends, status=Completed | __ |
| 4 | Check wallet delta | Charged for 5 min only (₹125), NOT for the 15 idle minutes | __ |

**Verdict:** __ (PASS / FAIL)  
**Key check:** auto-end fires ONCE (414-06 MMA P1-A: `idle_auto_end_queued` one-shot guard from `8a52cc36`). Confirm in server JSONL: `grep "idle_auto_end" racecontrol-*.jsonl | grep pod_8` returns exactly one entry per session.

---

## TEST 4 — Out-of-credits Branch B

**Starting state:** driver=`<fill>`, balance_paise≈3000 (just above 1 min rate), session_id=N/A

| Step | Action | Expected | Actual |
|------|--------|----------|--------|
| 1 | Start session with driver whose balance is ~₹30 (just above 1 min rate) | __ | __ |
| 2 | Launch AC, drive ~1 minute, exit AC | balance now < ₹25 (rate per min) | __ |
| 3 | Wait 10 min | IdleWarningDialog appears with TITLE "Out of credits" and SOLE CTA "End session" (no Continue option) | __ |
| 4 | Tap End session | session ends, balance correctly debited | __ |

**Verdict:** __ (PASS / FAIL)  
**UI branch observed:** `<A (has credits) / B (out of credits)>`. Expected: B. Match: __

---

## Failure Modes to Watch

- Wallet over-debited (charged twice for same time) — would mean snap-across-swap broken
- Wallet under-debited (charged less than driving time) — would mean cumulative tracking broken
- Game stop ends session prematurely (status=EndedEarly instead of waiting_for_game) — would mean handle_game_off rewrite incomplete
- IdleWarning never fires — broadcast wiring broken (Plan 03)
- Auto-end never fires at 15 min — Plan 04 wiring broken (or MMA P1-A one-shot guard regressed)
- Modal shows wrong copy or wrong branch — Plan 05 frontend bug

## Result Summary

- [ ] TEST 1 — Cumulative snap across swap: __
- [ ] TEST 2 — Snap crosses 30-min threshold: __
- [ ] TEST 3 — IdleWarning + auto-end: __
- [ ] TEST 4 — Out-of-credits Branch B: __

**Overall:** __ (all 4 PASS → approved; any FAIL → block ship, file root-cause and loop back)

**Operator signature:** `<name, YYYY-MM-DD HH:MM IST>`
