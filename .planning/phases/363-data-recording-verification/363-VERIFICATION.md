---
phase: 363-data-recording-verification
verified: 2026-04-09T21:36:23Z
status: gaps_found
score: 4/5 must-haves verified
gaps:
  - truth: "ROADMAP.md plan-level checkbox for 363-01-PLAN is absent"
    status: partial
    reason: "ROADMAP.md Phase 363 Plans section lists 3/3 complete but only enumerates 363-02-PLAN and 363-03-PLAN checkboxes. 363-01-PLAN is missing entirely. CLAUDE.md standing rule 'ROADMAP plan checkbox sync on completion' requires every completed plan to have its checkbox updated in the same commit."
    artifacts:
      - path: ".planning/ROADMAP.md"
        issue: "363-01-PLAN [x] checkbox never added — violated CLAUDE.md 'ROADMAP plan checkbox sync on completion'"
    missing:
      - "Add '- [x] 363-01-PLAN — DB schema + session_audit module + coverage histogram + cloud sync' under Phase 363 Plans section in .planning/ROADMAP.md"
human_verification:
  - test: "End-to-end refund trace"
    expected: "Customer topup Rs.700, 30min booking, session ended at 15min, wallet shows refund of Rs.325 (not Rs.350 — see Known Deviations). Balance = initial_balance - 375 paise."
    why_human: "Requires live racecontrol + live pod session + wallet DB state — not automatable with cargo test"
  - test: "Restart-safety of grace window"
    expected: "Start session, trigger grace window (session-end event fires), restart racecontrol binary, verify hydrate_active_timers_from_db log line appears and session is still present in active_timers"
    why_human: "Requires killing and restarting the racecontrol process mid-grace-window; not automatable"
  - test: "Feature flag kill switch live test"
    expected: "Set phase363_session_audit=0 in feature_flags table, end a session, verify billing_sessions.lap_count_flag remains UNVERIFIED and telemetry_coverage_pct remains NULL"
    why_human: "Requires live DB manipulation + session end event; in-process test already covers this but live confirm needed per 363-VALIDATION.md"
  - test: "CSV fallback 30s budget on actual pod"
    expected: "Disconnect pod WS mid-session, accumulate CSV laps, end session, observe push attempt in rc-agent log within 30s, confirm csv_fallback_received_at set in server DB"
    why_human: "Requires live pod + network disruption test"
---

# Phase 363: Data Recording Verification — Verification Report

**Phase Goal:** Lap audit + telemetry completeness + CSV auto-sync + 5s billing grace window. Closes all 3 P0s (P0-01 billing lap-reject race, P0-03 CSV fallback not auto-synced, P1-03 session-to-laps reconciliation).
**Verified:** 2026-04-09T21:36:23Z (approx. 03:06 IST 2026-04-10)
**Status:** gaps_found
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | GLD-C-01: Session-end audit writes lap_count_flag to billing_sessions | VERIFIED | `run_session_audit()` in `session_audit.rs:213-231` performs the UPDATE. Called from `post_session_hooks()` in `billing.rs:4842-4847` via `tokio::spawn` fire-and-forget. 10 unit tests green. |
| 2 | GLD-C-02: Sessions with <80% telemetry coverage marked suspect=true | VERIFIED | `compute_suspect()` in `session_audit.rs:95-122` sets suspect=true when coverage<80%. `telemetry_seconds_covered` HashSet updated in `ws/mod.rs:879-884` via `try_write()`. `suspect_reasons` JSON array populated. |
| 3 | GLD-C-03: CSV fallback POSTs to `/api/v1/sessions/{id}/telemetry-fallback` within 30s | VERIFIED | Route registered in `service_routes()` at `api/routes.rs:736-739` (NOT public_routes). `SessionEnded` in `ws_handler.rs:415-441` spawns `push_csv_fallback` detached. Read-before-clear ordering confirmed in `csv_lap_fallback.rs:155-209`. 4 csv_fallback_tests green. |
| 4 | GLD-C-04: 5s grace window defers billing finalize after session end | VERIFIED | `tick_all_timers()` in `billing.rs:1632-1643` sets `lap_reject_grace_until = now() + 5s` on timer expiry instead of immediately finalizing. Deferred finalizes executed after lock release at line 1679. `hydrate_active_timers_from_db()` called at `main.rs:768`. 3 grace window tests green. |
| 5 | F-05 regression lock: CAS UPDATE does not overwrite wallet_debit_paise | VERIFIED | CAS UPDATE at `billing.rs:8830-8832` SET clause contains only `status`, `driving_seconds`, `ended_at`, `end_reason`. `wallet_debit_paise` absent from SET. `test_end_billing_session_early_end_refund_amount` test locks this at the SQL level. |

**Score: 5/5 truths verified** (automated code checks — 1 structural gap in ROADMAP metadata, detailed below)

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/session_audit.rs` | New module: LapCountFlag, expected_laps, compute_lap_flag, coverage_pct, compute_suspect, run_session_audit | VERIFIED | File exists. All 6 symbols present. 10 tests. Lines 1-520. |
| `crates/racecontrol/src/db/mod.rs` | 8 ALTER TABLE migrations + lap_rejections CREATE TABLE + phase363_session_audit INSERT OR IGNORE | VERIFIED | Lines 3959-4021. All 8 columns confirmed: lap_count_expected (3963), lap_count_actual (3965), lap_count_flag (3969), telemetry_coverage_pct (3972), suspect (3975), suspect_reasons (3978), csv_fallback_received_at (3982), lap_reject_grace_until (3986). lap_rejections at 3994-4006 uses `session_id` column (D-12 compliant). Feature flag seeded at 4013-4019. |
| `crates/racecontrol/src/billing.rs` | BillingTimer.lap_reject_grace_until + pending_end_status; tick defers finalize; hydrate_active_timers_from_db; NO wallet_debit_paise in CAS SET | VERIFIED | Fields at lines 426-430. Default None at 486-487. tick defers at 1636-1643. hydrate_active_timers_from_db at 5897. CAS UPDATE SET clause at 8831 excludes wallet_debit_paise. |
| `crates/racecontrol/src/api/routes.rs` | POST /api/v1/sessions/{id}/telemetry-fallback in service_routes(), NOT public_routes | VERIFIED | Route at line 736, inside service_routes() function starting at line 687. DefaultBodyLimit(50MB) applied. X-Service-Key inline auth in handler. 5 tests confirm auth behavior. |
| `crates/racecontrol/src/cloud_sync.rs` | All 8 new columns in billing_sessions push payload | VERIFIED | All 8 columns at lines 669-676: lap_count_expected, lap_count_actual, lap_count_flag (COALESCE), telemetry_coverage_pct, suspect (COALESCE), suspect_reasons, csv_fallback_received_at, lap_reject_grace_until. test_billing_session_push_columns_phase363 asserts all 8 keys. |
| `crates/racecontrol/src/ws/mod.rs` | try_write() for coverage bucket, guard dropped before .await | VERIFIED | Line 879: `if let Ok(mut timers) = state.billing.active_timers.try_write()`. Guard drops at line 884 before any await. No lock held across async boundary. |
| `crates/rc-agent/src/csv_lap_fallback.rs` | push_csv_fallback with read-before-clear: buffer → POST → remove_file only on 200 | VERIFIED | Read at line 155, POST at 186-191, remove_file at line 204 — only inside `Ok(r) if r.status().is_success()` branch. File never cleared on error (test_no_clear_on_failure confirms). |
| `crates/rc-agent/Cargo.toml` | reqwest with multipart feature | VERIFIED | Line 53: `reqwest = { version = "0.12", features = ["json", "multipart"], optional = true }`. http-client feature enables it at line 102. |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `end_billing_session()` | `post_session_hooks()` | `tokio::spawn` at billing.rs:4347-4356 | WIRED | `seconds_covered_at_end` captured before `timers.remove()` and passed as parameter |
| `post_session_hooks()` | `run_session_audit()` | direct `await` at billing.rs:4842-4847 | WIRED | Error logged but not fatal — session continues if audit fails |
| `run_session_audit()` | `billing_sessions` UPDATE | sqlx UPDATE at session_audit.rs:213-231 | WIRED | Writes all 6 audit columns |
| `ws/mod.rs Telemetry handler` | `BillingTimer.telemetry_seconds_covered` | `try_write()` at ws/mod.rs:879 | WIRED | Non-blocking; lock dropped immediately at line 884 |
| `tick_all_timers()` | grace window DB persist | sqlx UPDATE `lap_reject_grace_until` at billing.rs:1668-1675 | WIRED | After lock released; fire-and-forget |
| `tick_all_timers()` | `end_billing_session()` deferred | deferred_finalizes Vec at billing.rs:1679 | WIRED | Finalize called after lock drop |
| `main.rs` startup | `hydrate_active_timers_from_db()` | direct call at main.rs:768 | WIRED | Error logged non-fatally |
| `ws_handler.rs SessionEnded` | `push_csv_fallback()` | `tokio::spawn` at ws_handler.rs:431 | WIRED | `#[cfg(feature = "http-client")]` gated; URL derived from config.core.url |
| `push_csv_fallback()` | `POST /api/v1/sessions/{id}/telemetry-fallback` | reqwest multipart POST at csv_lap_fallback.rs:186-191 | WIRED | X-Service-Key from RCAGENT_SERVICE_KEY env var |
| `telemetry_fallback_handler` | `billing_sessions.csv_fallback_received_at` | sqlx UPDATE at routes.rs:22645-22667 | WIRED | Sets `datetime('now')` on confirmed write |

---

## Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `session_audit.rs:run_session_audit()` | `actual_count` (lap count) | `SELECT COUNT(*) FROM laps WHERE session_id = ?` at line 185 | Yes — live DB query | FLOWING |
| `session_audit.rs:run_session_audit()` | `allocated_seconds` | `SELECT allocated_seconds FROM billing_sessions WHERE id = ?` at line 162 | Yes — live DB query | FLOWING |
| `ws/mod.rs Telemetry` | `telemetry_seconds_covered.insert(elapsed)` | `frame.pod_id` from live WS packet at line 882 | Yes — real telemetry seconds | FLOWING |
| `billing.rs:end_billing_session()` | `seconds_covered_at_end` | `timer.telemetry_seconds_covered.len()` captured before `timers.remove()` | Yes — live count from HashSet | FLOWING |
| `csv_lap_fallback.rs` | `body_bytes` | `tokio::fs::read(csv_path)` at line 155 | Yes — reads real file on disk | FLOWING |
| `cloud_sync.rs` | billing_sessions push payload | `json_object(... lap_count_expected, ...)` from live DB rows | Yes — COALESCE defaults for NULL | FLOWING |

---

## Behavioral Spot-Checks

Step 7b: SKIPPED for live runtime behaviors (server not running during verification). Cargo test suite (25 tests) executed by orchestrator — raw output provided in prompt and trusted. Static code analysis confirms all wiring.

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| GLD-C-01 | 363-01 | Per-session lap audit with >10% gap flag | SATISFIED | `session_audit.rs:run_session_audit()` writes lap_count_flag; 10 tests including test_run_audit_integration |
| GLD-C-02 | 363-01 | Telemetry completeness <80% marks suspect=true | SATISFIED | `compute_suspect()` threshold at session_audit.rs:110; coverage histogram in BillingTimer |
| GLD-C-03 | 363-02 | CSV fallback auto-sync within 30s of session end | SATISFIED | push_csv_fallback() in rc-agent wired to SessionEnded; server endpoint in service_routes(); 4 unit tests |
| GLD-C-04 | 363-03 | Billing 5s grace window for lap-reject race | SATISFIED | Flag-based deferral in tick_all_timers(); DB persist for restart-safety; hydrate on startup; 3 grace window tests |
| F-05 regression | 363-03 | Refund calc does not use overwritten wallet_debit_paise | SATISFIED | CAS UPDATE SET clause excludes wallet_debit_paise; test_end_billing_session_early_end_refund_amount locks SQL invariant |

---

## CLAUDE.md Compliance Check

| Rule | Status | Evidence |
|------|--------|----------|
| Never hold a lock across .await | PASS | ws/mod.rs:884 guard dropped before any await. billing.rs tick_all_timers collects into Vecs under lock, drops lock, then awaits DB updates. session_audit.rs:144-150 snapshots flag and drops guard before DB awaits. hydrate_active_timers_from_db: guard taken then dropped before await (billing.rs:5897+). |
| No .unwrap() in production Rust | PASS | session_audit.rs uses `.unwrap_or(0)` at line 191 (safe fallback) and `.unwrap_or_else(...)` at line 210. billing.rs grace window uses `let _ =` pattern for fire-and-forget DB updates. csv_lap_fallback.rs uses `?` and `anyhow` throughout. |
| Every ::default() reviewed | PASS | All 3 BillingTimer construction sites annotated with `// Intentional default:` comments at billing.rs:486-487, 2750-2751, 3617-3618. |
| DB migrations cover ALL consumers | PASS | cloud_sync.rs updated in same commit as migration (363-01 commit e4784c51 + 0b4e356c). All 8 columns in sync payload. |
| Deploy parity called out | PARTIAL — see Deploy Parity section below | |
| ROADMAP plan checkbox sync on completion | FAIL | 363-01-PLAN checkbox missing from ROADMAP.md (see Gap below) |
| Financial flow E2E | NEEDS HUMAN | Rs.325 refund formula documented and tested at unit level; live E2E trace deferred per 363-VALIDATION.md |

### Deploy Parity Assessment

- 363-01-SUMMARY.md: Mentions cloud parity required (git pull + cargo build + pm2 restart racecontrol on Bono VPS) — flagged as prerequisite, not executed in summary.
- 363-02-SUMMARY.md: Mentions "Bono VPS (cloud parity): racecontrol binary" as a deploy action — listed as pending.
- 363-03-SUMMARY.md: No deploy section present at all. The 363-03 plan touched only billing.rs and main.rs (server-side Rust), so a deploy section was required per DMP. It is absent.

ASSESSMENT: All three SUMMARYs flag or imply Bono VPS parity is needed but none confirm it was executed. Per CLAUDE.md "Code complete != deployed" and DMP rules, the binary has NOT been confirmed deployed to venue server (.23), pods 1-8 (rc-agent for GLD-C-03), or Bono VPS. This is a deploy tracking gap, not a code gap — the code itself is correct and complete. Flagged for the orchestrator's DMP checklist.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/racecontrol/src/billing.rs` | 8836 | `end_reason = ?` bound to `"final_cost_paise:35000"` in F-05 test | Info | Test-only value (in `#[cfg(test)]` block). Hard-coded to match the plan's original assumed Rs.350 formula but test comment corrects this. Not production code — no impact. |
| `crates/racecontrol/src/session_audit.rs` | 191 | `unwrap_or(0)` on COUNT(*) fetch | Info | Safe fallback — COUNT(*) never returns NULL in SQLite; this is defensive. Acceptable per CLAUDE.md "no .unwrap()" rule (this uses unwrap_or, not unwrap). |

No blockers or warnings found. No TODO/FIXME/placeholder comments in Phase 363 files. No hardcoded empty data flowing to rendering paths.

---

## Known Deviations

### Refund Formula: Rs.325 vs Rs.350

- **Origin:** 363-CONTEXT.md D-11 and CLAUDE.md F-05 section reference "Rs.162.50 loss per early-end on a 30-min session" — implying a Rs.350 expected refund on a Rs.700/30min session ended at 15min (simple proportional: 15/30 * 700 = Rs.350).
- **Actual behavior:** `compute_refund()` calls `compute_refund_with_rates()` which uses `best_rate_for_minutes(15, 2500, 75000, 90000)` = 15 * 2500 = 37500 paise (Rs.375 for 15min at per-minute rate). Refund = 70000 - 37500 = 32500 paise = **Rs.325**.
- **Impact on F-05:** None — F-05 is a read-after-write bug on `wallet_debit_paise`. The formula itself is orthogonal. The bug was that the UPDATE at line 2213 overwrote wallet_debit_paise before the refund read at line 2255. That structural bug is fixed — the CAS UPDATE excludes wallet_debit_paise from its SET clause.
- **Impact on CLAUDE.md "Rs.162.50 loss" claim:** The CLAUDE.md Standing Rule references "Rs.162.50 per early-ended 30min session" — this was calculated assuming the Rs.350 refund formula. With the actual formula, the loss would have been Rs.700 - Rs.325 - Rs.375 = Rs.0 for normal path, OR if wallet_debit_paise was overwritten: Rs.325 - Rs.162.50 = Rs.162.50 (same delta actually). The Rs.162.50 figure in CLAUDE.md appears to refer to a different calculation. The milestone audit team should recalculate the actual historical loss based on the confirmed formula.
- **Test assertion:** `test_f05_refund_uses_original_debit` asserts 32500 (Rs.325), not 35000. Deviation documented in 363-03-SUMMARY.md deviations section — CONFIRMED PRESENT.

---

## Gaps Summary

### Gap 1: ROADMAP.md 363-01-PLAN checkbox absent (metadata violation, not a code gap)

The ROADMAP.md Phase 363 Plans section reads:

```
3/3 plans complete
- [x] 363-02-PLAN — CSV fallback auto-sync path
- [x] 363-03-PLAN — Billing 5s grace window + lap-reject race fix
```

The 363-01-PLAN entry is completely absent. The "3/3 plans complete" count is correct but the 363-01 checkbox line was never added. This violates CLAUDE.md Standing Rule "ROADMAP plan checkbox sync on completion": "When a plan's SUMMARY.md is committed, the SAME commit MUST update the corresponding - [ ] checkbox in ROADMAP.md to [x]."

**Severity:** Low — the code delivered by 363-01 is fully correct and wired. This is a metadata tracking gap, not a functional gap. However, future audits that trust ROADMAP plan-level checkboxes (as explicitly warned against in CLAUDE.md) would show 2/3 plans with no evidence of 363-01's existence.

**Fix:** Add `- [x] 363-01-PLAN — DB schema + session_audit module + coverage histogram + cloud sync` under Phase 363 Plans section in `.planning/ROADMAP.md`. One-line commit.

### Gap 2: Deploy not confirmed (DMP gap, not a code gap)

None of the three SUMMARY.md files confirm binary deployment to venue server (.23), pods 1-8, or Bono VPS. 363-03-SUMMARY.md has no deploy section at all (required per DMP). The code is complete; deployment is a separate action the orchestrator must track.

**Fix (orchestrator action):**
1. Build `racecontrol` binary, deploy to server .23 (runs migration, activates GLD-C-01/02/04)
2. Build `rc-agent` binary, deploy to pods 1-8 (activates GLD-C-03 push_csv_fallback on SessionEnded; requires `RCAGENT_SERVICE_KEY` env var)
3. Deploy `racecontrol` to Bono VPS (git pull + cargo build --release + pm2 restart racecontrol) — cloud parity rule

---

## Verification Conclusion

All 5 observable truths are verified at the code level:
- GLD-C-01 lap audit writes to billing_sessions.lap_count_flag at session end
- GLD-C-02 telemetry coverage marks sessions suspect with reasons
- GLD-C-03 CSV fallback endpoint is gated behind service_routes() (not public), rc-agent SessionEnded wires the push with read-before-clear ordering
- GLD-C-04 grace window defers finalization with DB persistence and startup hydration
- F-05 regression locked by SQL invariant test

Two non-code gaps exist:
1. ROADMAP.md 363-01-PLAN checkbox missing (one-line fix)
2. Binary deployment not confirmed (orchestrator DMP action)

Four items require human verification with a live binary (end-to-end refund trace, restart-safety test, feature flag live test, CSV fallback live timing).

---

## VERIFICATION FAILED — structured gap list in frontmatter

The code delivers all 4 GLD-C-0X requirements and the F-05 regression lock. However:

1. **ROADMAP metadata gap** — `363-01-PLAN` checkbox absent from `.planning/ROADMAP.md`. One-line fix: add `- [x] 363-01-PLAN — DB schema + session_audit module + coverage histogram + cloud sync` under Phase 363 Plans.

2. **Deploy not confirmed** — No SUMMARY confirms binary deployment to server (.23), pods 1-8, or Bono VPS. 363-03-SUMMARY.md has no deploy section. DMP requires deploy confirmation before phase is marked complete. Orchestrator must execute: build racecontrol + rc-agent, deploy to server + all 8 pods + Bono VPS, verify build_id on health endpoint.

---

_Verified: 2026-04-09T21:36:23Z_
_Verifier: Claude (gsd-verifier)_
