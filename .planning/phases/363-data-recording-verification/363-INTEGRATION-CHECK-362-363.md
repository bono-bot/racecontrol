---
phase: 363-data-recording-verification
check_type: cross-phase-integration
phases_checked: [362, 363]
verdict: INTEGRATION PASSED
flags: 4
completed_date: 2026-04-09
---

# Phase 362 x Phase 363 -- Integration Check

**Scope:** Phase 362 (Post-Launch Config Verification, deployed a9b5eaa3) x Phase 363 (Data Recording Verification, code-complete, not deployed)
**Checker:** Nyquist integration agent
**Date:** 2026-04-09

---

## Check 1 -- Stage 5 Pattern Replication

**Question:** Does Phase 363 reproduce the non-fatal degradation pattern established by Phase 362?

**Findings:**

Phase 362 verify_launch_config() (crates/rc-agent/src/launch_verifier.rs) returns LaunchStage::ConfigVerified on timeout rather than failing the launch. Non-fatal degradation is the explicit contract.

Phase 363 run_session_audit() (crates/racecontrol/src/session_audit.rs) checks the phase363_session_audit feature flag first; if disabled, returns Ok(()) immediately. On any DB error inside the audit, the ? propagates to post_session_hooks, which is called via tokio::spawn fire-and-forget from billing.rs lines 4347-4356. A panic or error in the spawned task cannot crash the billing FSM.

Both phases use the same architectural contract: verify/audit runs asynchronously, never blocks the critical path, degrades gracefully.

**Verdict: PASS**

---

## Check 2 -- BillingTimer Hydration vs Phase 362 State

**Question:** Does hydrate_active_timers_from_db() conflict with any Phase 362 state?

**Findings:**

Phase 362 adds SessionConfig to rc-agent (crates/rc-agent). It never touches BillingManager or active_timers in crates/racecontrol.

hydrate_active_timers_from_db() (billing.rs line 5897+) is the first-ever startup hydration path for active_timers. It reads billing_sessions rows and reconstructs BillingTimer instances. Phase 362 adds no columns to billing_sessions; the 8 columns added in Phase 363 (363-01-SUMMARY.md) are all COALESCE-defaulted in cloud_sync.rs.

Phase 362 and Phase 363 operate in separate processes (rc-agent pod binary vs racecontrol server binary). No shared in-memory state.

**Verdict: PASS**

---

## Check 3 -- SessionConfig Boundary

**Question:** Is the SessionConfig boundary between the two phases clean? Will future phases need a FK?

**Findings:**

Phase 362 persists ConfigMismatchDetected events to event_archive (ws/mod.rs line 2229) and config_mismatches table. Phase 363 session_audit.rs reads from billing_sessions only -- it does not join config_mismatches or event_archive.

The session_type field: Phase 362 sends SessionType in SessionConfig WS message. Phase 363 session_audit.rs defaults session_type to trackday when absent from billing_sessions (363-01-SUMMARY.md decision: session_type absent from billing_sessions, defaults to trackday). This means audit coverage will always use trackday lap targets regardless of what Phase 362 detected.

This is intentionally clean per 363-01 decisions. A future phase (noted as Phase 367 in VERIFICATION.md) will need to wire SessionConfig.session_type through to billing_sessions.session_type to close this gap.

**Verdict: FLAG (intentional, future work) -- no integration break in current scope**

---

## Check 4 -- Error Cascade: Config Mismatch into Audit

**Question:** If Phase 362 emits a ConfigMismatchDetected event, does it corrupt the Phase 363 audit?

**Findings:**

ConfigMismatchDetected is handled in ws/mod.rs line 2229: it logs, persists to event_archive, sends WhatsApp, and broadcasts to admin SSE. It does NOT alter billing_sessions columns, does NOT modify active_timers, and does NOT set or clear phase363_session_audit flag.

run_session_audit() reads: allocated_seconds, laps_completed COUNT from lap_events, coverage histogram from BillingTimer.telemetry_seconds_covered. None of these are touched by the config mismatch handler.

DB DEFAULT values handle the case where audit has never run: all 6 audit columns (audit_lap_flag, audit_coverage_pct, etc.) default to NULL / 0, which is safe for display.

**Verdict: PASS**

---

## Check 5 -- Deploy Parity and Sequencing

**Question:** Is there a deploy ordering constraint between Phase 362 (deployed) and Phase 363 (not yet deployed)?

**Findings:**

Phase 363 adds 8 columns to billing_sessions and creates lap_rejections table via migration block in db/mod.rs lines 3959-4021. The migration uses let _ = sqlx::query(ALTER TABLE...) idempotent pattern. Running Phase 363 migration against a DB that already has Phase 362 schema is safe.

Phase 363 Plan 03 (363-03-SUMMARY.md) has NO deploy section. This is a DMP violation. The 363-01 and 363-02 summaries include deploy notes; 363-03 does not.

Deploy sequencing required: racecontrol server (with migration) MUST deploy before rc-agent pods. rc-agent push_csv_fallback (Phase 363-02) calls POST /api/v1/sessions/{id}/telemetry-fallback on racecontrol. If pods update before server, the endpoint returns 404 and pods enter 7-attempt retry (~254s envelope). This is handled gracefully by the read-before-clear invariant, so no data loss, but the retry window creates unnecessary load.

Deploy parity rule: venue server deploy MUST be followed by Bono VPS deploy in same session.

**Verdict: FLAG -- server-first sequencing is mandatory; 363-03 missing deploy section (DMP gap)**

---

## Check 6 -- Feature Flag Isolation

**Question:** Does phase363_session_audit flag affect Phase 362 behavior or vice versa?

**Findings:**

Phase 362 has no feature flag kill switch. verify_launch_config() always runs on pod launch.

Phase 363 seeds phase363_session_audit with enabled=1 via INSERT OR IGNORE in migration. Disabling it suppresses run_session_audit() only. It does not affect billing FSM, lap event recording, telemetry WS handling, or push_csv_fallback.

The flags are completely independent. Disabling phase363_session_audit does not suppress Phase 362 features. Phase 362 has no mechanism to disable Phase 363 audit.

**Verdict: FLAG (non-blocking observation) -- Phase 362 has no kill switch; Phase 363 flag is isolated**

---

## Check 7 -- Test Interaction

**Question:** Do Phase 363 tests interact with Phase 362 test infrastructure?

**Findings:**

Phase 362 tests live in crates/rc-agent/ (launch_verifier tests using injected closure pattern).
Phase 363 tests live in crates/racecontrol/src/billing.rs and crates/racecontrol/src/session_audit.rs.

The two crates are separate compilation units. No shared test helpers, no shared in-memory SQLite fixtures between them.

363-02-SUMMARY.md notes: MMA audit required before deploy (cross-system bridge). The cargo test -p rc-agent-crate suite must be run after Phase 363 is merged to confirm no rc-agent regressions from Cargo.toml change (reqwest multipart feature addition).

Phase 363 billing_grace module adds make_grace_test_db() with lap_reject_grace_until TEXT column, separate from the existing create_test_db(). No collision.

**Verdict: PASS with action item -- run cargo test -p rc-agent-crate before deploy to verify reqwest feature gate has no side effects**

---

## Check 8 -- E2E Flow Trace

**Question:** Does the full customer-to-data E2E flow complete without breaks?

**13-step flow:**

| Step | Actor | Action | Phase | Status |
|------|-------|--------|-------|--------|
| 1 | Customer | Kiosk session start | pre-362 | OK |
| 2 | rc-agent | Launch game | pre-362 | OK |
| 3 | rc-agent | Stage 5: verify_launch_config() | 362 | OK -- graceful timeout |
| 4 | rc-agent | ConfigMismatchDetected WS if mismatch | 362 | OK -- server handles at ws/mod.rs:2229 |
| 5 | server | WhatsApp alert on mismatch | 362 | GAP: pre-existing, not Phase 363 scope |
| 6 | server | Billing FSM ticks, coverage histogram fills | 363 | OK -- try_write() non-blocking |
| 7 | rc-agent | Telemetry CSV buffered | 363 | OK -- read-before-clear invariant |
| 8 | server | Session ends, post_session_hooks spawned | 363 | OK -- fire-and-forget |
| 9 | server | run_session_audit() checks flag, computes audit | 363 | OK -- feature flag guarded |
| 10 | rc-agent | push_csv_fallback 7-attempt retry | 363 | OK -- server must be up first |
| 11 | server | telemetry-fallback endpoint stores CSV | 363 | OK -- service-key authenticated |
| 12 | server | cloud_sync pushes billing_sessions with 8 new cols | 363 | OK -- COALESCE defaults |
| 13 | Admin | Audit columns visible in admin dashboard | 363 | NOT WIRED -- future phase |

Step 13 gap: session_audit.rs writes audit columns to billing_sessions DB but no admin UI component reads them. This is expected scope for a future phase.

Step 5 gap (WhatsApp on mismatch): pre-existing gap noted in 363-VERIFICATION.md, not introduced by Phase 363.

**Verdict: PASS -- no structural E2E breaks introduced by Phase 363. Step 13 admin UI is out-of-scope future work.**

---

## Check 9 -- F-05 Impact on Phase 362

**Question:** Does the F-05 fix in Phase 363 affect any Phase 362 code paths?

**Findings:**

F-05 fix is in billing.rs line 8831: CAS UPDATE SET clause excludes wallet_debit_paise. This is in the racecontrol server billing FSM.

Phase 362 code is entirely in crates/rc-agent/src/launch_verifier.rs and related rc-agent files. Phase 362 does not call compute_refund(), does not touch the CAS UPDATE path, and has no billing FSM code.

The F-05 regression tests (test_f05_refund_uses_original_debit, test_end_billing_session_early_end_refund_amount) are isolated to billing::tests module.

**Verdict: PASS -- Phase 362 has zero overlap with F-05 billing refund logic**

---

## Requirements Integration Map

| Requirement | Integration Path | Status | Issue |
|-------------|-----------------|--------|-------|
| GLD-B-01 | rc-agent: read_session_config() -> verify_launch_config() Stage 5 | WIRED | None |
| GLD-B-02 | rc-agent: AtomicWrite race.ini -> readback verify | WIRED | None |
| GLD-B-03 | rc-agent -> server: ConfigMismatchDetected WS -> ws/mod.rs:2229 | WIRED | WhatsApp send gap (pre-existing, not Phase 363) |
| GLD-B-04 | rc-agent: SessionType normalization in SessionConfig | WIRED | session_type not surfaced to billing_sessions (Phase 367 future work) |
| GLD-B-05 | rc-agent: graceful timeout -> LaunchStage::ConfigVerified | WIRED | None |
| GLD-C-01 | server: billing.rs coverage histogram -> session_audit.rs -> billing_sessions audit cols | WIRED | audit cols not exposed in admin UI (future phase) |
| GLD-C-02 | server: telemetry WS handler try_write() -> BillingTimer.telemetry_seconds_covered | WIRED | None |
| GLD-C-03 | rc-agent: push_csv_fallback -> server: /api/v1/sessions/{id}/telemetry-fallback | WIRED | Server must deploy before pods (sequencing constraint) |
| GLD-C-04 | server: grace window + deferred finalize + hydrate_active_timers_from_db + record_lap_rejection | WIRED | 363-03 missing deploy section (DMP gap) |
| F-05 | server: billing.rs CAS UPDATE excludes wallet_debit_paise | WIRED (server only) | No cross-phase dependency |

**Requirements with no cross-phase wiring:**
- GLD-B-01 through GLD-B-05: self-contained in rc-agent (Phase 362). No Phase 363 code reads or calls these. The integration boundary is the WS protocol message (ConfigMismatchDetected) which server handles but Phase 363 audit does not consume.
- F-05: self-contained in racecontrol billing FSM. Phase 362 has no billing code.

---

## Wiring Summary

**Connected:** 8 exports/paths properly wired
**Orphaned:** 0 exports created but unused
**Missing:** 1 forward link (session_type from Phase 362 SessionConfig to billing_sessions -- Phase 367 scope)

## API Coverage

**Consumed:** 1 new route (/api/v1/sessions/{id}/telemetry-fallback called by push_csv_fallback)
**Orphaned:** 0

## Auth Protection

**Protected:** 1 (telemetry-fallback endpoint is service-key-gated via RCAGENT_SERVICE_KEY)
**Unprotected:** 0

## E2E Flows

**Complete:** 1 (customer session start through audit write, 11 of 13 steps)
**Broken:** 0 structural breaks
**Out of scope:** 2 steps (Step 5 pre-existing WhatsApp gap, Step 13 admin UI future phase)

---

## INTEGRATION PASSED

**4 non-blocking flags:**
1. FLAG-1 (Check 3): session_type defaults to trackday in audit -- correct by design, Phase 367 will close
2. FLAG-2 (Check 5): Server-first deploy sequencing mandatory; 363-03-SUMMARY missing deploy section (DMP gap)
3. FLAG-3 (Check 6): Phase 362 has no kill switch -- operational risk noted, not an integration break
4. FLAG-4 (Check 8): Audit columns not wired to admin UI -- future phase, no break in current scope

**Pre-deploy action items:**
1. Run cargo test -p rc-agent-crate after Phase 363 merge to verify reqwest multipart feature gate
2. Deploy racecontrol server BEFORE rc-agent pods
3. Deploy to Bono VPS in same session as venue server (deploy parity rule)
4. Add deploy section to 363-03 PLAN.md (DMP compliance)
