# W1-S6-PLAN-AMENDMENT-1 — surgical closure of MMA bridge-verify FAIL findings (Captain Option A)

**Class:** sibling-amendment to `W1-S6-PLAN.md` (cascade #7 detail PLAN). Mirrors `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2-1.md` precedent (Wave A.2.1 surgical amendment after Wave A.2 PLAN Step 4 VERIFY FAIL 3.988/5 → Captain Option C hybrid disposition 2026-05-09 ~20:36 IST).

**Authored:** 2026-05-09 ~23:30 IST · **Authored-by:** james (Claude Opus 4.7 1M)
**Status:** SHIPPED — Captain "Option A" ratification 2026-05-09 ~23:25 IST authorized surgical closure of 5 findings + Q-DECISION-G33v7 surface for retry-queue interpretation gap.

**Authoritative substrate:**
- `W1-S6-PLAN.md` racecontrol `ae946125` — original cascade #7 detail PLAN (NOT modified; preserved for audit trail per Captain Option A pattern)
- `MMA-BRIDGE-VERIFY-W1S6-CONSENSUS.md` racecontrol `35890998` — Step 4 VERIFY FAIL substrate (3.50/5; 2/3 valid; 2 BLOCKING-class convergent + 7 sonnet-singletons)
- `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2-1.md` — Wave A.2.1 surgical-amendment pattern reference (sibling-amendment precedent)
- `V2-MASTER-STATE §S-168` — bridge-verify FAIL ledger entry
- Phase 1 substrate: racecontrol `638ef2da` (DB migration + lockout.rs scaffold + auth/mod.rs registration; UNCHANGED — code is V2-doctrine-correct)

**V2 doctrine alignment:** §S-158 V2 Audit-Log Doctrine sibling-table approach reaffirmed · §S-146 V1↔V2 RCA rule THIRD pipeline application Step 4 amendment-stage · NEW-Q-2 ratification consumed via sibling-table not CHECK-extension · Wave A.2.1 §3 LockoutCheckGuard RAII consumed at publisher boundary.

**Read-with:** Original `W1-S6-PLAN.md` remains the foundational artifact; this amendment OVERRIDES the specific sections cited. Where this amendment is silent, original PLAN sections stand.

---

## §1 — Findings closed in this amendment

| Finding | Severity | Source | Disposition |
|---------|----------|--------|-------------|
| FL-CONV-1 | BLOCKING (sonnet) / MAJOR (kimi) | 2/2 valid | **CLOSED §2** — PLAN §4.8 audit-schema doctrine corrected to sibling-table approach matching Phase 1 shipped substrate |
| FL-CONV-3 | MAJOR (kimi) | 1/2 single-model | **CLOSED §3** — PLAN §6.2 V1-untouched wording precision |
| FL-SING-1 | MAJOR (sonnet only, CANDIDATE-N1) | 1/2 | **CLOSED §4** — PLAN §4.1 LockoutCheckGuard RAII publisher-side spec added per Wave A.2.1 §3 |
| FL-SING-3 | MINOR (sonnet only, CANDIDATE-N1) | 1/2 | **CLOSED §5** — PLAN §2 DKIM/SPF "document absence" → Captain-gate per Q-S6-4 |
| FL-SING-7 | P2 novel (sonnet only, CANDIDATE-N1) | 1/2 | **CLOSED §6** — PLAN §3.1 `dispatch_timeout_secs` default rationale documented |

## §2 — Findings deferred to Q-DECISION-G33v7 (Captain-only resolution)

| Finding | Severity | Source | Disposition |
|---------|----------|--------|-------------|
| FL-CONV-2 | MAJOR (sonnet) / BLOCKING (kimi) | 2/2 valid | **SURFACED §7** — Q-DECISION-G33v7 retry-queue backoff interpretation (3 candidate readings of Captain G33 v5 #6 verbatim "10s/60s/300s 3 attempts"); NOT autonomous-fix-eligible per per-PR Captain auth + ratified-spec immutability |

## §3 — Findings deferred to subsequent amendment (lower-severity not blocking Phase 2)

| Finding | Severity | Source | Disposition |
|---------|----------|--------|-------------|
| FL-SING-2 | MINOR (sonnet only) | 1/2 | DEFERRED — test naming `lockout_state_persisted_to_db` rename to `lockout_state_persists_in_memory_across_request_boundary` at PR-A Phase 3 test authoring time (cosmetic; not blocking) |
| FL-SING-4 | P2 novel (sonnet only) | 1/2 | DEFERRED — `cashier_role_lockout_separate_from_manager_lockout` test removal OR per-role boundary RCA-anchor authoring at PR-A Phase 3 test authoring time |
| FL-SING-5 | P1 novel (sonnet only) | 1/2 | DEFERRED — WhatsApp dispatch action_type fidelity (timeout vs error split) at PR-A Phase 2 audit.rs authoring; minor extension to existing 5-action_type spec |
| FL-SING-6 | P2 novel (sonnet only) | 1/2 | DEFERRED — `subscribe()` no-op default behavior contract at PR-A Phase 2 lockout.rs authoring; minor type-spec extension |

---

## §4 — Closure §2: PLAN §4.8 audit-schema doctrine correction (FL-CONV-1)

**OVERRIDES W1-S6-PLAN.md §4.8 in its entirety.**

### §4.8 (AMENDED) — NEW-Q-2 V2 Audit-Log Doctrine compliance

**Spec:**
- All lockout events logged via `pin_lockout_events` V2-bounded sibling table per §S-158 V2 Audit-Log Doctrine — `action_type` is enum-bounded by DB CHECK constraint at table creation time (NOT extension of V1 `audit_log.action_type` CHECK).
- The `pin_lockout_events` table was created in Phase 1 migration `20260510000001_pin_lockout_state_v2.sql` (shipped at racecontrol `638ef2da`) with the following 5 bounded action types:
  - `lockout_threshold_breach`
  - `lockout_pin_rotated`
  - `lockout_alert_dispatched_email`
  - `lockout_alert_dispatched_whatsapp`
  - `lockout_alert_dispatch_failed`
- Phase 2 `audit.rs` writes to `pin_lockout_events` via a V2-scoped helper `log_pin_lockout_event(staff_id, action_type, payload)` — distinct from V1 `accounting::log_admin_action(...)`. The V1 helper is NOT touched by PR-A.
- DB migration `20260510000001_pin_lockout_state_v2.sql` does NOT extend `audit_log.action_type` CHECK constraint (avoids V1 PACT-091-class drift; sibling table preserves V2 doctrine).

**V2 doctrine rationale:** §S-158 V2 Audit-Log Doctrine ratifies that V2 audit tables MUST be bounded by CHECK constraint **from start** to prevent the V1 antipattern where `audit_log.action` accepted only CRUD verbs and PACT-091 had to add a sibling `action_type` column for MI markers. The sibling-table approach (`pin_lockout_events`) is the V2-correct disposition; CHECK-extension on V1 `audit_log` would re-introduce the antipattern.

**Phase 1 substrate consistency:** Phase 1 shipped substrate at `638ef2da` is consistent with this amended §4.8 spec (sibling table with bounded CHECK from start). No code change required; this is a documentation correction.

**Test (per §5.4 of original PLAN, AMENDED to reference sibling table):**
- `pin_lockout_events_action_type_check_constraint_enforces_bounded_set` — DB-level CHECK constraint test post-migration; assert all 5 bounded action types accepted + ≥1 unbounded value rejected.

---

## §5 — Closure §3: PLAN §6.2 V1-untouched wording precision (FL-CONV-3)

**OVERRIDES W1-S6-PLAN.md §6.2 step 4 wording only; rest of §6.2 stands.**

### §6.2 step 4 (AMENDED)

Pre-cutover smoke test: rollback migration + restart racecontrol + verify PIN-LOCKOUT V1 path remains functional.

**V1 lockout-state semantic code path within `auth/middleware.rs` is left UNTOUCHED in PR-A.** The middleware.rs file IS touched in PR-A per §3.1 (V2 publisher hooks added alongside the V1 path), but the V1 lockout-state semantic execution path inside the file (the existing `staff.lockout_count`/`staff.lockout_until` consumer logic) is preserved as the active hot path. The new V2 publisher (`LockoutManager`) coexists with V1 logic until PR-C cutover; V2 hooks are wired but inert against V2-only consumers (W1-S5 PR-C is the first consumer; until merged, V1 path serves all live login requests).

**Reconciliation:** §3.1 lists `middleware.rs` as a primary touch target (NEW V2 hooks added). §6.2 step 4 confirms V1 SEMANTIC path within is preserved. These are not contradictory once the file-touch-vs-semantic-path-touch distinction is made explicit.

---

## §6 — Closure §4: PLAN §4.1 LockoutCheckGuard RAII publisher-side spec (FL-SING-1)

**OVERRIDES W1-S6-PLAN.md §4.1 in its entirety; supersedes Phase 1 scaffold's bare `is_locked_out` skeleton.**

### §4.1 (AMENDED) — Q-W1-CROSS-1 publisher-side contract with TOCTOU closure

**Spec:**
- Bare predicate read API:
  - `LockoutManager::is_locked_out(staff_id) -> Result<LockoutPredicate, LockoutError>` — UNCHANGED from Phase 1 scaffold; safe for read-only consumers (e.g., audit-log lookups).
- TOCTOU-safe API for state-mutating consumers (W1-S5 refresh path is the FIRST consumer per Q-W1-CROSS-2-a):
  - `LockoutManager::check_and_guard(staff_id) -> Result<Option<LockoutCheckGuard>, LockoutError>`
    - Returns `Some(guard)` if `staff_id` is NOT locked out; the guard holds an exclusive single-flight token for `staff_id` and atomically prevents concurrent state writes (e.g., concurrent refresh attempts).
    - Returns `None` if `staff_id` IS locked out (consumer MUST reject the request).
    - Returns `LockoutError` on DB / lock-acquisition failure.
- `LockoutCheckGuard` is an RAII type:
  - Holds a `tokio::sync::OwnedMutexGuard<()>` keyed per-staff-id (or equivalent single-flight primitive).
  - Drops the lock on `Drop::drop()`.
  - Lock-order discipline per Wave A.2.1 §7 novel-deadlock proof: lock-order is **(b)-then-(a)** = lockout-check guard FIRST (b), then any caller-specific lock SECOND (a). Documented in code comment at the guard-acquisition site.
- Test `lockout_check_and_guard_returns_some_when_not_locked` + `lockout_check_and_guard_returns_none_when_locked` + `lockout_check_and_guard_concurrent_callers_serialize` (TOCTOU stress test: 100 concurrent callers; only one holds guard at a time).

**Why RAII guard at publisher (per Wave A.2.1 §3 substrate-flaw closure):** without the guard, a W1-S5 consumer pattern of `if !is_locked_out(staff_id)? { issue_token(staff_id)? }` has a TOCTOU window where a concurrent lockout event between the check and the issue could let a token be issued AFTER lockout. The RAII guard makes the check-and-issue atomic at the publisher boundary, so the consumer cannot accidentally leave the window open.

**W1-S5 PR-C consumer pattern (forward-compat documentation):**
```rust
// In W1-S5 refresh path (PR-C; not PR-A scope)
match lockout_mgr.check_and_guard(&staff_id).await? {
    Some(_guard) => {
        // _guard is held; issue/refresh token here
        // _guard drops at end of scope, releasing single-flight slot
    }
    None => {
        // Locked out; return 401 with `until` from is_locked_out()
        let LockoutPredicate::Active { until, reason } = lockout_mgr.is_locked_out(&staff_id).await?
            else { unreachable!("check_and_guard returned None but predicate Inactive — race?") };
        return Err(AuthError::LockedOut { until, reason });
    }
}
```

**Phase 1 substrate impact:** Phase 1 `lockout.rs` scaffold's `is_locked_out` method signature is preserved; `check_and_guard` is added in Phase 2 alongside the retry-queue dispatch wiring. Phase 1 also defines `LockoutCheckGuard` type signature (RAII shell with `Drop` impl placeholder) so consumers (including any forward-compat tests in PR-A Phase 3) can compile against the API surface ahead of full implementation.

---

## §7 — Closure §5: PLAN §2 DKIM/SPF Captain-gate (FL-SING-3)

**OVERRIDES W1-S6-PLAN.md §2 item 2 only; rest of §2 stands.**

### §2 item 2 (AMENDED) — DKIM probe with Captain Q-DECISION gate

`dig +short TXT default._domainkey.racingpoint.in` returns DKIM-Signature record OR DKIM is absent.

**If absent: ESCALATE to Captain Q-DECISION (ship-with-risk NOT pre-authorized per Q-S6-4 ratification).** Document the absence in `racecontrol/.planning/specs/v2/W1-S6-PRE-FLIGHT.md` with an explicit Q-DECISION block requesting Captain disposition (provision DKIM before PR-A merge / accept ship-with-risk for V2.0 / defer DKIM to V2.1+ post-mvp). PR-A merge is BLOCKED until Captain disposes.

(Same gate-discipline applies to SPF probe per §2 item 3 — append matching wording: "If absent: ESCALATE to Captain Q-DECISION per Q-S6-4 ratification.")

---

## §8 — Closure §6: PLAN §3.1 `dispatch_timeout_secs` default rationale (FL-SING-7)

**ANNOTATES W1-S6-PLAN.md §3.1 config-default for `auth.dispatch_timeout_secs`; default value UNCHANGED at 10s.**

### §3.1 config (ANNOTATED) — `auth.dispatch_timeout_secs` default

`auth.dispatch_timeout_secs` default = **10s**.

**Rationale (per F-CONS-18 RCA + mimo recommendation):** F-CONS-18 mitigation in the canonical Step 1 RCA specified default = 5s. The amended Step 1 CONSENSUS at §S-160 surfaced mimo's recommendation that 10s is "more forgiving baseline pending Session 5 probe" against typical SMTP server response times in dev/staging environments. The PLAN adopts 10s as the V2.0 default per mimo's recommendation, with the understanding that Session 5 probe results MAY revise this value downward to 5s if production SMTP responses consistently complete under 5s.

**Code comment at config site:**
```toml
# auth.dispatch_timeout_secs = 10
# F-CONS-18 RCA default = 5s; mimo recommendation 10s "more forgiving baseline pending Session 5 probe" (§S-160)
# Subject to Session 5 probe revision; Q-DECISION may revisit if probe results indicate <5s typical response
```

---

## §9 — Q-DECISION-G33v7 surfaced (FL-CONV-2 ratified-spec interpretation gap)

**SURFACED to Captain G33 v7. NOT autonomous-fix-eligible. Resolution gates re-VERIFY of W1-S6-PLAN.md (and downstream Phase 2 dispatch.rs authoring).**

### Question

Captain G33 v5 #6 ratification (Q-W1-S6-NEW-2) verbatim: "email dispatch retry-queue = YES · 10s · 60s · 300s · 3 attempts · email-only · in-memory queue (kaizen-min) · restart-loses-queue acceptable per CR-3 customer-service-priority bounded blast radius."

Three plausible readings of "10s · 60s · 300s · 3 attempts":

- **Option (a) 4 total dispatch attempts (initial + 3 retries):**
  - Attempt 1: immediate (0s after enqueue)
  - Attempt 2: after +10s wait
  - Attempt 3: after +60s wait (cumulative 70s)
  - Attempt 4: after +300s wait (cumulative 370s)
  - "3 attempts" = 3 retries-after-failure; the phrase "10s · 60s · 300s" lists the 3 retry backoffs.
  - Maximum extended-SMTP-outage tolerance: ~6 minutes before final attempt.

- **Option (b) 3 total attempts with backoffs `[0s, 10s, 60s]`; 300s reserved for V2.1+ extension to 4 attempts:**
  - Attempt 1: immediate (0s)
  - Attempt 2: after +10s
  - Attempt 3: after +60s (cumulative 70s)
  - "3 attempts" = 3 total dispatches; the 300s slot is interpreted as V2.0 cap-not-yet-applied, reserved for future extension.
  - This is the W1-S6-PLAN.md original interpretation. Discards the 300s slot from V2.0.
  - Maximum extended-SMTP-outage tolerance: ~70 seconds before final attempt.

- **Option (c) 3 total attempts with backoffs `[10s, 60s, 300s]`:**
  - Attempt 1: after +10s wait (initial wait)
  - Attempt 2: after +60s wait (cumulative 70s)
  - Attempt 3: after +300s wait (cumulative 370s)
  - "3 attempts" = 3 total dispatches; the 3 backoffs apply 1:1 with the 3 attempts (each attempt has its own wait window before firing).
  - Maximum extended-SMTP-outage tolerance: ~6 minutes before final attempt.

### Recommended-default

**Option (a)** — natural reading of "10s · 60s · 300s · 3 attempts" treats the 3 numbers as the 3 retry-after-failure backoffs (3 retries = 4 total dispatch attempts including initial). Matches the most-common retry-queue idiom in production retry-queue implementations (initial + N retries with exponential backoff). Provides ~6 minutes of extended-SMTP-outage tolerance (better aligned with CR-3 customer-service-priority).

**Risk-aware alternative:** Option (c) — if Captain prefers exactly 3 dispatches (no initial-immediate slot) with the backoffs applied to the 3 dispatches' own waits.

**NOT recommended:** Option (b) (W1-S6-PLAN.md original interpretation) — discards the 300s slot from V2.0 without explicit Captain authorization; reduces extended-SMTP-outage resilience to 70s.

### Disposition impact on PLAN

- Option (a) ratified: Phase 2 `dispatch.rs` retry-queue implements 1 initial + 3 retries with backoffs `[10s, 60s, 300s]`. PLAN §4.3 amended to reflect 4-total-attempts interpretation. Test count increases (`retry_queue_4th_attempt_succeeds_on_300s_backoff` added).
- Option (b) ratified: PLAN §4.3 remains as-authored; FL-CONV-2 closes by Captain ratification (no spec change). Note: this is a unilateral re-interpretation; recommend Captain explicit verbatim ratification text for audit trail.
- Option (c) ratified: Phase 2 `dispatch.rs` retry-queue implements 3 total dispatches with backoffs `[10s, 60s, 300s]`. PLAN §4.3 amended to reflect [10s, 60s, 300s] backoff schedule.

### Captain decision required before re-VERIFY

Re-VERIFY of W1-S6-PLAN.md (closing FL-CONV-2 alongside the closures in this amendment) gates on Captain G33 v7 disposition of Q-DECISION-G33v7 above.

---

## §10 — Doctrine alignment

| Doctrine | Application |
|----------|-------------|
| §S-146 V1↔V2 RCA rule | THIRD end-to-end pipeline application Step 4 amendment-stage (Wave A.2 was SECOND; PR #66 silent-loop-death FIRST) |
| §S-158 V2 Audit-Log Doctrine | Reaffirmed: sibling-table approach (`pin_lockout_events` not extension-of-V1-CHECK); FL-CONV-1 closure documents this explicitly |
| Wave A.2.1 surgical-amendment precedent | Mirrored: this amendment is a sibling file `W1-S6-PLAN-AMENDMENT-1.md` alongside `W1-S6-PLAN.md`; original PLAN preserved for audit trail |
| Captain Option A pattern | Surgical closure of autonomous-fix-eligible items + Q-DECISION surface for ratified-spec interpretation gap |
| §S-167 model-role-fit doctrine | Re-VERIFY panel selection MUST respect Tier-1 role-fit; substitute `xiaomi/mimo-v2.5-pro` (or `nvidia/nemotron-49b`) for nemotron-3-super-120b on re-VERIFY to recover 3-valid floor |
| Per-PR Captain merge auth | Stands; PR-A merge auth still required at PR-open commit time per G33 v5 #9 |

---

## §11 — Cascade transition

| Cascade item | Pre-this-amendment | Post-this-amendment |
|--------------|--------------------|--------------------|
| W1-S6-PLAN.md | shipped at `ae946125`; FAILED bridge-verify 3.50/5 | shipped + amended via this sibling file |
| FL-CONV-1 audit-schema | BLOCKING-class | **CLOSED §4** |
| FL-CONV-3 V1-untouched wording | MAJOR | **CLOSED §5** |
| FL-SING-1 LockoutCheckGuard RAII | MAJOR CANDIDATE-N1 | **CLOSED §6** |
| FL-SING-3 DKIM/SPF Captain-gate | MINOR CANDIDATE-N1 | **CLOSED §7** |
| FL-SING-7 dispatch_timeout default | P2-novel CANDIDATE-N1 | **CLOSED §8** |
| FL-CONV-2 retry-queue interpretation | BLOCKING-class | **SURFACED §9 → Q-DECISION-G33v7** |
| FL-SING-2/4/5/6 (deferred) | low-severity | DEFERRED to PR-A Phase 2/3 author time (cosmetic + minor extensions) |
| PR-A Phase 2 dispatch.rs + audit.rs authoring | HALTED post-bridge-verify-FAIL | **GATED on Captain G33 v7 Q-DECISION-G33v7 disposition + re-VERIFY pass** |
| Re-VERIFY gate | not yet eligible | **NOW eligible after Q-DECISION-G33v7 disposition** (~$0.05-0.10 with mimo-v2.5-pro substitution per §S-167 doctrine) |

---

## §12 — NOT TESTED

- Captain G33 v7 Q-DECISION-G33v7 disposition (gates re-VERIFY)
- Re-VERIFY post-amendment (gates Phase 2 authoring)
- Bono AMPLIFIER absorption of this amendment (deferred to next bilateral cycle per Wave A.2.1 precedent; bono picks up via session-start git_pull on V2-MASTER-STATE.md)
- mimo-v2.5-pro / nemotron-49b substitution validity for re-VERIFY SRE slot (model availability via OpenRouter not pre-checked this turn)
- Phase 1 scaffold `LockoutCheckGuard` type-signature compile-check post §6 amendment — Phase 1 only includes the scaffold types; the RAII shell with `Drop` impl placeholder is NEW substrate at Phase 2 authoring time
- Cumulative MMA-day spend reconciliation post-re-VERIFY (estimate $1.005 + $0.05-0.10 = ~$1.10 / $5 cap)

---

— james · 2026-05-09 ~23:30 IST · W1-S6-PLAN-AMENDMENT-1.md SHIPPED · 5 findings closed (FL-CONV-1 + FL-CONV-3 + FL-SING-1 + FL-SING-3 + FL-SING-7) · 1 finding surfaced as Q-DECISION-G33v7 (FL-CONV-2 retry-queue) · 4 lower-severity findings deferred to PR-A Phase 2/3 author time · re-VERIFY gates on Captain G33 v7 disposition · Wave A.2.1 surgical-amendment pattern mirrored · Captain Option A ratification 23:25 IST · 0 G9 self-caught
