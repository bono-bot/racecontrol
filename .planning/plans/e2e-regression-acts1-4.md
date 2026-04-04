# E2E Regression Test — Acts 1-4 (Assetto Corsa Multiplayer)

**Created:** 2026-04-04 03:00 IST
**Server Build:** `c31997c0`
**Test Drivers:** drv_8d1025c4 (Vishal), 3755d087 (Ravi Kumar)

---

## Plan

### Phase 1: Pre-flight
- [x] 1. Server health check — `c31997c0`, status ok, whatsapp ok
- [x] 2. Fleet health — 8 pods + POS all WS+HTTP connected
- [x] 3. Auth — admin JWT obtained via `/auth/admin-login`
- [x] 4. Pricing tiers — trial(free/5min), 30min(Rs.700), 60min(Rs.900), per-minute
- [x] 5. Test drivers identified, wallets checked

### Phase 2: ACT 1 — Wallet + Billing Start
- [x] 6. Wallet topup (Driver 1: Rs.1000 cash) — PASS
- [x] 7. Wallet balance verify — 100000 paise — PASS
- [x] 8. Billing start Pod 1 (Driver 1, 30min) — PASS, session `8bad393e`
- [x] 9. Billing start Pod 2 (Driver 2, 30min) — PASS, session `5c97c4e2`
- [x] 10. Verify wallets debited (Rs.700 each) — PASS
- [x] 11. Verify sessions in `waiting_for_game` — PASS

### Phase 3: ACT 2 — Game Launch + Billing Transition
- [x] 12. Launch AC on Pod 1 — `verified:true`, PID 3920
- [x] 13. Launch AC on Pod 2 — `verified:true`, PID 6160
- [x] 14. Launch AC on Pod 3 — `verified:true`, PID 1560, SSH tasklist confirmed
- [x] 15. verify-action.sh contradiction test — 3/3 PASS
- [x] 16. Check billing transition — **NOT TRIGGERED** (False-Live guard, needs physical input)
- [x] 17. Session cancel after timeout — cancelled at 21:15:19

### Phase 4: ACT 3 — Session End + Receipts
- [x] 18. billing/stop on waiting_for_game — **FAIL** (not found)
- [x] 19. Receipt endpoint — PASS (tier, cost, duration, balance)
- [x] 20. Session summary — PASS (driver, pod, laps, pricing)
- [x] 21. Session events — PASS (created→started→idle→paused→ended)
- [x] 22. Incentive approval — PASS (Rs.50 bonus credited)
- [x] 23. **Refund on cancel — CRITICAL FAIL** (CHECK constraint: `refund_stale_cancel`)

### Phase 5: Edge Cases
- [x] 24. Idempotency replay — PASS
- [x] 25. Insufficient balance — PASS
- [x] 26. Double stop — PASS
- [x] 27. Occupied pod — PASS
- [x] 28. Invalid pod/driver/tier — PASS (3 tests)
- [x] 29. Trial game restriction — PASS (blocked F1 on trial)

### Phase 6: Bug Fixes
- [x] 30. Fix `refund_stale_cancel` → `refund_session` (billing.rs:1736)
- [x] 31. Fix `refund_no_playable` → `refund_session` (billing.rs:1135)
- [x] 32. Fix false-positive logging (match Ok/Err instead of unconditional info)
- [x] 33. Fix attempt 2 timeout: UPDATE existing BILL-13 record + refund (billing.rs:2193)
- [x] 34. Build racecontrol — compiled successfully

### Phase 7: Deploy + Verify
- [ ] 35. Deploy fixed binary to server (.23) **← NEXT**
- [ ] 36. Re-test refund: billing/start → wait 5min → verify wallet refunded
- [ ] 37. Verify server logs show "Refunded" WITHOUT CHECK constraint error
- [ ] 38. Update LOGBOOK.md

### Phase 8: Physical Verification (requires venue hours)
- [ ] 39. Test `waiting_for_game` → `active` transition with driver input
- [ ] 40. Test active session pause/resume
- [ ] 41. Test billing/stop with refund calculation on active session
- [ ] 42. Test session upgrade (30→60min)
- [ ] 43. Visual verification of AC on triple monitors

---

## Bugs Found

| ID | Severity | Description | Status |
|----|----------|-------------|--------|
| BUG-1 | P0 | `refund_stale_cancel` txn_type fails CHECK constraint — wallet refunds lost | FIXED in code |
| BUG-2 | P0 | `refund_no_playable` txn_type same issue | FIXED in code |
| BUG-3 | P1 | Attempt 2 timeout INSERTs duplicate record instead of UPDATE for BILL-13 | FIXED in code |
| GAP-1 | P2 | `billing/stop` can't cancel `waiting_for_game` sessions | Not fixed |
| GAP-2 | P3 | D3D games bypass GDI screen capture for remote verification | By design |

## Dependencies
- Steps 35-38 depend on Phase 6 (fixes compiled)
- Steps 39-43 depend on venue being open + physical presence at pods
