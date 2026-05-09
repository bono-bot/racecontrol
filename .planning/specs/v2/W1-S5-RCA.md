# W1-S5 RCA — Sliding-window idle-timeout in `auth/middleware.rs`

**Doctrine basis:** `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (Captain BILATERAL directive committed at comms-link `8768b628` 2026-05-09 ~09:28 IST)

**Author:** james · **Date:** 2026-05-09 ~10:15 IST · **Branch context:** `feat/v2-wave-1-w1-s1-billing-service` HEAD `3b44f051`

**Status:** DRAFT-v2-CAPTAIN-DISPOSITIONED — Captain G33 batch disposition 2026-05-09 ~11:23 IST CLOSED Q-S5-1..7 ACCEPT-ALL-DEFAULTS + MMA Step 1 budget APPROVED up to $10 OpenRouter (W1-S5 + W1-S6 batched OR separate) + per-PR Captain merge auth STANDS at PR-open + PROMOTE-N=2 → N=3 §S-142.3 PATH-TYPO sub-class codification APPROVED at next charter cycle. 24h Captain correction-window 2026-05-10 ~11:23 IST.

**Gate sequence (POST Captain G33 batch disposition):**
- (1) bono AMPLIFIER ✓ COMPLETE — msg=35808 (10:34 IST) absorbed in DRAFT-v2 `cb9ea94f`
- (2a) Captain Q-RECONCILE-1 ✓ CLOSED — msg=35809 EXPLICIT-RATIFY = AUTHORIZED (10:45 IST)
- (2b) Captain G33 batch Q-S5-1..7 ✓ CLOSED — disposition 11:23 IST ACCEPT-ALL-DEFAULTS (this update)
- (3) MMA Step 1 DIAGNOSE on RCA-as-amended — budget APPROVED up to $10; execution PENDING (Captain user decision on timing)
- (4) W1-S5 H1 PLAN — PENDING (gated on (3))
- Per-PR Captain merge auth at W1-S5 PR-open STANDS independently.

**Amendment log (this version):**
- Captain G33 batch disposition 2026-05-09 ~11:23 IST: Q-S5-1..7 ACCEPT-ALL-DEFAULTS as enumerated in §5 Q-DECISION table — all 7 marked CLOSED below with disposition refs. MMA budget approved. PROMOTE-N=3 codification approved at next charter cycle.
- bono AMPLIFIER msg=35808 ABSORBED 2026-05-09 ~10:34 IST (CONCUR + 3 NITs + 1 CAVEAT + 1 FLAG + 1 RATIONALE-EXTENSION applied as amendments below); Captain Q-RECONCILE-1 EXPLICIT-RATIFY = AUTHORIZED via msg=35809 ~10:45 IST.

**Amendment log (this version):**
- NIT-1 (bono msg=35808 §A axis-1): §1 row 8 path corrected — `config.rs::AuthConfig::idle_timeout_secs` → `config/services.rs:105+136+:442`. PATH-TYPO sub-class N=2 evidence (PROMOTE-N=2 candidacy invoked per bono §C). **Verify-Before-Generate FIRED during this amendment**: bono msg=35808 §A axis-2 cited `default_idle_timeout()` at `:151` — james `git cat-file -e` + grep verification at amendment-time showed actual line is **442**, not 151. Bono's `:151` cite was itself a PATH-TYPO instance pre-correction; james VBG caught + corrected to `:442` before propagating into this RCA. Empirical doctrine bar advance: §S-121 v0.2 source-verify gate works bilateral-symmetric — both pilots' cite-discipline gets caught by structural fix; redundancy preserves substrate-correctness against single-pilot path-discipline failure (per §S-143.6 sub-observation). PROMOTE-N=2 candidacy now has N=3 empirical evidence anchors (1 james W1-S5 RCA original + 1 bono msg=35808 NIT-1 catch + 1 bono msg=35808 self-instance `:151` james-caught).
- CAVEAT-1 (bono msg=35808 §A axis-2): PHASE-1-WAVE-1-PLAN.md row 21+33 cites "7-min" but V2.1 PIN + middleware.rs:124 + §S-82 Q3 + config/services.rs:442 (`default_idle_timeout`) all say 30-min — NEW inherited-issue row PLAN-1 added to §2; NEW past-bug disposition row added to §3 (plan-author typo class; W1-S5 ship is natural place to amend the plan if (a))
- NIT-2 (bono msg=35808 §A axis-3): §3 JWT secret rotation grace window row split into past-disposition (NOT-APPLICABLE-TO-V2 as a "bug") + forward-looking-RCA-item (OPEN-FORWARD-LOOKING)
- FLAG-1 (bono msg=35808 §A axis-4): Q-S5-6 added to Captain Q-DECISION table — response-mutating-middleware-layer precedent: ratified pattern (future RCAs cite) OR explicit one-off-with-no-future-composition?
- RATIONALE-EXTENSION (bono msg=35808 §A axis-6): Q-S5-3 note added — at sliding-window-vs-fixed-window decision site, add `// Trait DEFERRED-TO-N3 per kaizen; if 3rd timeout-strategy variant lands, abstract here per §AMEND-3.II D12 Foundation/Strategy/Config separation.` comment for kaizen-deferral audit trail
- Q-RECONCILE-1 status (Captain msg=35809 ~10:45 IST): EXPLICIT-RATIFY = AUTHORIZED. V2.1 sliding-window pull-forward to W1-S5 EXPLICITLY AUTHORIZED. W1-S5 ship MAY amend V2.1 PIN status to RETIRED-PULLED-FORWARD-TO-W1-S5 + middleware.rs:103-110 comment (commit-hash anchor `<W1-S5-ship-commit>` placeholder until ship-time). Captain text verbatim: *"Q-RECONCILE-1 close-loop: Captain DEFAULT-YES upgrade to EXPLICIT-RATIFY"*. 24h Captain correction-window 2026-05-10 ~10:45 IST.

**Foundational-boundary classification:** YES — auth boundary per doctrine §"MMA escalation". Triggers: MMA Step 1 DIAGNOSE on the RCA itself + per-PR Captain merge auth at PR-open.

---

## Reconciliation note (read first)

There is a doctrine reconciliation point that must be Captain-dispositioned before this RCA is closed:

| Source | Says |
|---|---|
| `crates/racecontrol/src/auth/middleware.rs:103-110` (PR #64 substrate, MERGED `991b5411`) | "sliding-window refresh on activity (token re-issuance) is **scope-pinned to V2.1** — see memory file `project_v2_1_sliding_window_idle_timeout_pact_pin.md`" |
| `project_v2_1_sliding_window_idle_timeout_pact_pin.md` §"V2.1 milestone trigger conditions" | PACT fires on FIRST of: V2.0 launch readiness Wave 6 / staff burn-in >10% / Captain explicit request / 2026-06-30 + 60d soak |
| `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` row 33 (authored 2026-05-08 ~19:05 IST, AFTER PR #64 merge + V2.1 pin) | W1-S5 = "Idle-timeout 30min sliding-window — Auth middleware extension — extends Wave 0 K5 7-min fixed-window for staff-elevated session" — Session 4 of Wave 1, NOT Wave 6 |

**Captain disposition (UPDATED 2026-05-09 ~10:45 IST per msg=35809):** Q-RECONCILE-1 = **EXPLICIT-RATIFY = AUTHORIZED**. V2.1 sliding-window pull-forward to W1-S5 EXPLICITLY AUTHORIZED. Captain text verbatim relayed via bono: *"Q-RECONCILE-1 close-loop: Captain DEFAULT-YES upgrade to EXPLICIT-RATIFY"*. 24h Captain correction-window 2026-05-10 ~10:45 IST.

**W1-S5 ship action items (now ratified):**
- amend `briefings/james/memory/project_v2_1_sliding_window_idle_timeout_pact_pin.md` status: ACTIVE-V2.1 → RETIRED-PULLED-FORWARD-TO-W1-S5 (commit-hash anchor `<W1-S5-ship-commit>` placeholder until ship-time)
- amend `crates/racecontrol/src/auth/middleware.rs:103-110` comment block: remove "scope-pinned to V2.1" language; replace with "implemented per W1-S5 [commit-hash] sliding-window per Captain §S-82 Q3 disposition + Q-RECONCILE-1 EXPLICIT-RATIFY 2026-05-09 ~10:45 IST"

Original prior-version interpretation (preserved for audit trail): Wave-1 plan pulled V2.1 PACT forward via trigger #1 ("V2.0 launch readiness Wave 6") loose interpretation; Captain implicit authorization via rolling autonomy + Bravo dispositions. Bono AMPLIFIER msg=35808 §B flagged this as substantive doctrine reinterpretation NOT pre-authorized by V2.1 pin's plain-reading + recommended UPGRADE to Captain explicit ratify. Captain dispositioned upgrade ~10:45 IST.

---

## §1 — Boundary map

### V1↔V2 surface inventory

| Path | Lines | V1-era? | V2-era? | Touched by W1-S5? |
|---|---|---|---|---|
| `crates/racecontrol/src/auth/middleware.rs` | 1-348 (whole file) | YES — `extract_staff_claims` + `require_staff_jwt` + `require_role` + `create_staff_jwt*` predate V2 (Phase 306 era + PACT-018 era) | PARTIAL — `is_idle_expired` added by PACT-20260506-001 §AMEND-1.F + Captain §S-82 Q3 disposition (2026-05-07) is V2-era | YES — sliding-window REPLACES `is_idle_expired` semantics + ADDS post-handler token re-issuance + cookie write |
| `crates/racecontrol/src/auth/middleware.rs:79-96` | `extract_staff_claims` JWT decode + secret-rotation grace | V1 | — | NO (read-only call site) |
| `crates/racecontrol/src/auth/middleware.rs:99-101` | VALID_ROLES enforcement | V1 | — | NO |
| `crates/racecontrol/src/auth/middleware.rs:102-114` | K5 fixed-window idle-timeout block + V2.1 PACT pointer comment | — | V2 (PR #64 substrate, MERGED `991b5411`) | YES — REPLACED by sliding-window check + token re-issuance hook |
| `crates/racecontrol/src/auth/middleware.rs:119-134` | `is_idle_expired(claims, idle_timeout_secs) -> bool` | — | V2 (same PR #64 substrate) | YES — function signature changes (sliding-window needs `last_activity` not `iat`) OR function deprecated in favor of new sliding-window check |
| `crates/racecontrol/src/auth/middleware.rs:144-161` | `require_staff_jwt` middleware | V1 (predates V2 idle-timeout) | — | YES — must be wrapped or chained with post-handler token re-issuance layer |
| `crates/racecontrol/src/auth/middleware.rs:251-273` | `create_staff_jwt_with_role` JWT mint | V1 (Phase 306-era PIN cookie pattern) | — | YES — sliding-window re-issuance reuses this OR extracts a "refresh helper" sibling that doesn't re-validate PIN |
| `crates/racecontrol/src/auth/middleware_tests.rs:298-366` | Idle-timeout test block (Captain §S-82 Q3 sliding-window header comment, fixed-window test impl) | — | V2 (PR #64 substrate) | YES — sliding-window introduces 3-5 new test cases per V2.1 PACT pin §"Test coverage" |
| `crates/racecontrol/src/config/services.rs:105` (struct AuthConfig) + `:136` (field idle_timeout_secs) + `:151` (default_idle_timeout = 1800s) | (referenced from middleware.rs:111 via `state.config.auth.idle_timeout_secs`) | V1 (config schema) | — | NO (value only; new sliding-window may add `idle_refresh_window_secs` sibling) [PATH corrected per bono NIT-1 msg=35808 — was `config.rs::AuthConfig::idle_timeout_secs`; PATH-TYPO sub-class N=2 evidence] |
| `crates/racecontrol/src/auth/admin.rs` | Cookie helpers + Set-Cookie utilities (PIN issue path uses these) | V1 (PACT-018 era) | — | YES — sliding-window response-mutating layer must use the same cookie helpers (httpOnly + Secure + SameSite=Strict) per V2.1 PACT pin §2 |

### Cross-organ data flow at the boundary

1. **Inbound staff request** carries Bearer JWT (HTTP `Authorization` header) — V1 contract.
2. **`require_staff_jwt`** calls `extract_staff_claims` which calls `is_idle_expired` — V2 K5 check (current).
3. **`StaffClaims`** inserted into request extensions — V1 axum extension pattern.
4. **Handler runs** — out of scope for W1-S5.
5. **Response returned** — currently unmodified by middleware (no Set-Cookie write per V2.1 PACT pin §2 finding).
6. **POS browser** reads response — currently no idle-timeout-related cookie state per V2.1 PACT pin §5 cross-pilot finding.

W1-S5 introduces step 5.5: post-handler middleware that re-issues a fresh JWT (new `iat`) and writes a Set-Cookie response header.

### Schema / state surfaces

- **JWT contract** (`StaffClaims` struct) — `iat` semantics shift from "issuance time, never updated" → "last activity time, refreshed on each authenticated request." V1-era cross-system clients that snapshot `iat` for any purpose other than idle-expiry computation are at risk. Need to grep callers.
- **Cookie state** — POS browser currently has staff-PIN cookie (httpOnly). New idle-refresh cookie may collide or compose with PIN cookie. Need to re-read `crates/racecontrol/src/auth/admin.rs` cookie helpers and confirm no cookie-name collision.
- **Audit log** — sliding-window re-issuance fires every authenticated request. If we audit-log every re-issuance, we 100x the audit_log row count for a busy POS. Likely SHOULD NOT audit-log routine re-issuances; SHOULD audit-log idle-expiry rejections.

### Configuration surfaces

- `AuthConfig::idle_timeout_secs` (struct at `config/services.rs:105`; field at `:136`; default at `:151` returns 1800s = 30min) — per Captain §S-82 Q3. Sliding-window keeps the value, changes the SEMANTIC.
- POTENTIAL NEW config `AuthConfig::idle_refresh_grace_secs` — overlap window where the OLD token (pre-refresh `iat`) stays valid even after a new token has been issued. V2.1 PACT pin §"Test coverage" item 3 names this as test scope; the config to make it tunable may or may not ship in W1-S5.

---

## §2 — Inherited-issue catalogue

Issues at this boundary, drawn from V1 failure-mode investigation + commit-log + LOGBOOK + ledger anchors.

| ID | Source | Issue | Scope at this boundary |
|---|---|---|---|
| K5 | `racecontrol/.planning/specs/v2/PHASE-1-WIREUP-DEPLOY-MANIFEST.md` §3 K5 row | "Captain §S-82 Q3 specified sliding-window; PR #64 shipped fixed-window-from-iat" | DIRECT — W1-S5 is the K5 closure |
| Token re-issuance has no cookie write path today | V2.1 PACT pin §"Implementation outline" item 2 | "currently no per-request `Set-Cookie` write on staff routes" | DIRECT — W1-S5 must add response-mutating layer |
| Cross-pilot impact on TS POS frontend | V2.1 PACT pin §"Implementation outline" item 5 | "TS POS frontend needs to handle Set-Cookie response gracefully" | DIRECT — POS browser is the primary staff JWT consumer; W1-S5 must verify cookie handling |
| `require_role` ordering invariant | `crates/racecontrol/src/auth/middleware.rs:177-178` ("Must be used AFTER `require_staff_jwt` in the middleware chain") | If sliding-window adds a NEW middleware layer, the chain ordering changes | INDIRECT — sliding-window post-handler layer must NOT disturb the require_staff_jwt → require_role → handler order |
| JWT secret rotation grace period (`extract_staff_claims:84-95`) | V1-era `jwt_secret_previous` fallback | If sliding-window re-issuance always uses CURRENT secret, mid-rotation tokens get refreshed onto the new secret implicitly — SHOULD be OK but needs RCA validation | INDIRECT |
| `create_staff_jwt` issues "cashier" by default | `middleware.rs:244-246` | If sliding-window refresh helper is naïvely extracted from `create_staff_jwt`, it may downgrade non-cashier roles to cashier on every refresh | DIRECT-CRITICAL — must use `create_staff_jwt_with_role(claims.role)` not `create_staff_jwt(...)` |
| `iat as i64` cast at line 131 | `middleware.rs:131` | If `iat` is ever > i64::MAX (year 2262+) the cast wraps. Not practical concern but flagged. | NOT-APPLICABLE |
| Clock-skew handling | `middleware.rs:128-134` (saturating_sub) | Current implementation tolerates iat-in-future (returns 0 elapsed). Sliding-window must preserve this — re-issuance with iat=now should not retroactively expire prior requests | INDIRECT |
| audit_log write amplification | (no prior anchor — surfaced in §1 above) | Re-issuance every authenticated request could 100x audit_log INSERTs if we audit each refresh | NEW-FROM-RCA |
| W1-S3+S4 substrate dependency | this session's substrate `0386db62` + `59432d4b` (refund 3-band routing) | Refund handlers run BEHIND `require_staff_jwt` middleware. If sliding-window re-issuance changes Extension extraction shape, refund tests may break | INDIRECT |
| §S-61 V1 failure-mode investigation 14-mapped catalogue | `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` | No V1 auth-middleware failure mode mapped specifically to idle-timeout (V1 didn't have idle-timeout — that's a V2 invention). Adjacent failure: V1 broadcast-storm class touches WS auth, not HTTP middleware | NOT-APPLICABLE-DIRECTLY |
| **PLAN-1** [added per bono CAVEAT-1 msg=35808 §A axis-2] | `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` rows 21+33 | Plan cites Wave-0 K5 idle-timeout as "**7-min** fixed-window for staff-elevated session"; V2.1 PIN file + middleware.rs:124 doc comment ("default 30 min sliding window via `idle_timeout_secs = 1800`") + Captain §S-82 Q3 verbatim ("Q3 idle-timeout = 30 min sliding-window") + `config/services.rs:442` `default_idle_timeout()` returns 1800s all say **30-min**. Internal inconsistency in PLAN — either (a) plan-author typo class fix-in-same-W1-S5-ship, OR (b) different fine-grained timeout silent-disposition (e.g. distinct Wave 0 staff-elevated-action 7-min timer separate from 30-min general idle), OR (c) commit-history reconciliation needed | DIRECT (W1-S5 ship is natural place to amend the plan if (a)) |

---

## §3 — Past-bug disposition

| Past bug at boundary | Disposition | Evidence |
|---|---|---|
| K5 (fixed-window where Captain specified sliding) | **PATCHED-ONLY** — Path A shipped fixed-window for V2.0; sliding-window pinned to V2.1 PACT (now being pulled forward via W1-S5) | PR #64 merge `991b5411` 2026-05-08 13:54 IST; Captain Path A disposition 2026-05-08 ~13:50 IST; V2.1 pin memory file authored same day |
| Cookie-write-path absence | **UNRESOLVED — open RCA item** | V2.1 PACT pin §2 finding; no commit closes this |
| Cross-pilot POS browser cookie handling | **UNRESOLVED — open RCA item** | V2.1 PACT pin §5; no test coverage today |
| `require_role` ordering invariant documentation | **ROOT-CAUSED-AND-FIXED** (documented at `middleware.rs:177-178`) — needs verification that sliding-window post-handler layer doesn't violate it | Doc comment is the existing protection |
| JWT secret rotation grace window (PAST-bug aspect) | **NOT-APPLICABLE-TO-V2** as a "bug" — V1 grace-period behavior is acceptable; no prior incident | `extract_staff_claims:84-95` V1 era; no prior bug ticket |
| JWT secret rotation grace + sliding-window interaction (FORWARD-looking-RCA-item per NIT-2 split msg=35808 §A axis-3) | **OPEN-FORWARD-LOOKING — open RCA item from §2** — sliding-window introduces NEW concern: tokens refreshed during rotation grace would always end up on current secret. Need W1-S5 design choice: (a) preserve grace-secret refresh (read original token's secret, refresh with same secret), OR (b) explicitly accept side-effect (always-current-secret refresh; document in code comment that mid-rotation refresh implicitly migrates to new secret). Bono recommends explicit-document-the-choice approach | Forward-looking; no prior bug evidence (V1 had no refresh path) |
| `create_staff_jwt` cashier-default downgrade risk | **UNRESOLVED — open RCA item from §2** | No prior bug; flagged here as W1-S5 implementation hazard |
| audit_log write amplification | **NEW** — surfaced in §1 / §2 | No prior bug; needs W1-S5 design choice |
| W1-S3+S4 substrate dependency | **ROOT-CAUSED-AND-FIXED** (W1-S3+S4 use Extension extractor; sliding-window doesn't change that) — needs regression test in W1-S5 to prove | Session 3 substrate test pass at `cargo test -p racecontrol-crate --test refund_routing` (14/14 pass per Session 3 handoff §"Verification") |
| **PLAN-1 plan-author "7-min" inconsistency** [added per bono CAVEAT-1 msg=35808 §A axis-2] | **OPEN-PLAN-AUTHOR-DISPOSITION** — three candidate resolutions: (a) plan-author typo class — amend PHASE-1-WAVE-1-PLAN row 21+33 in same W1-S5 ship (most likely; passes Occam test given 4 other anchors all say 30-min); (b) distinct fine-grained timeout silent-disposition (separate Wave 0 staff-elevated-action 7-min timer vs 30-min general idle — would need PR #64 commit-archaeology to confirm); (c) commit-history reconciliation needed | bono empirical anchor: 4 "30-min" anchors (V2.1 PIN, middleware.rs:124, §S-82 Q3 verbatim, config/services.rs:442) vs 1 "7-min" anchor (PLAN row 21+33). Composes-with W1-S5 ship — natural amendment opportunity if (a) ratified |

**Open RCA items to resolve in W1-S5 design (per doctrine §"Disposition each past bug"):**

1. Cookie-write-path absence
2. Cross-pilot POS browser cookie handling
3. `create_staff_jwt` cashier-default downgrade risk
4. audit_log write amplification (design choice: log refreshes or not)
5. JWT secret rotation grace + sliding-window interaction (decide-and-document — bono recommends explicit-document-the-choice)
6. **PLAN-1 "7-min" inconsistency** [added per bono CAVEAT-1] — disposition (a)/(b)/(c); if (a), W1-S5 ship amends PHASE-1-WAVE-1-PLAN row 21+33 to "30-min" in same commit

---

## §4 — V2-alignment delta

### What V2 doctrine says the boundary should look like

| V2 anchor | Statement | Current alignment |
|---|---|---|
| `project_v2_master_state.md` §S-82 Q3 | Captain disposition: "Q3 idle-timeout = 30 min sliding-window" | NOT ALIGNED — current is fixed-window |
| `project_v2_1_sliding_window_idle_timeout_pact_pin.md` (V2.1 PACT) | Implementation outline for sliding-window | ALIGNED-FUTURE (the PACT itself is the alignment plan) |
| `feedback_v2_doctrine_alignment_drift_g9_pact_20260503_002.md` (V2-MASTER-STATE canonical-source ledger) | All V2 state changes go through ledger | NEEDS-LEDGER-ROW — W1-S5 disposition should land in V2-MASTER-STATE §S-N |
| `project_v2_customer_workflows_consolidated_20260503.md` | 5 base + 6 missed customer scenarios — none directly touch idle-timeout (staff-side concern) | NOT-CUSTOMER-VISIBLE per V2 customer doctrine |
| `feedback_kaizen_discipline_dont_complicate.md` | Smallest invariant for observed requirement | RISK — sliding-window adds a NEW response-mutating middleware layer + cookie write path that didn't previously exist; risk of overscope |
| `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (THIS doctrine) | RCA before action | THIS DOCUMENT is the RCA; satisfies the gate once Captain reviews |
| `feedback_emergent_directed_spend_protocol.md` Rule 4 (specify-codebase-identity) | Don't substitute mental model for environment | OK — every claim in this RCA cites a path/line/commit |
| §AMEND-3.II D12 (Foundation/Strategy/Config separation) | Strategy classes for substitutable behavior | COULD-APPLY — sliding-window vs fixed-window could be a `IdleTimeoutStrategy` trait. Likely OVERSCOPE for W1-S5 (only 2 strategies, no third planned); flagged for Captain decision |

### Named gap

**Gap-1:** middleware.rs implements fixed-window; doctrine + V2.1 PACT pin specify sliding-window. W1-S5 closes this gap.

**Gap-2:** No response-mutating middleware layer exists for staff routes. V2 doctrine doesn't forbid it; V1 didn't need it; W1-S5 introduces it. Risk: sets precedent for other response-mutating concerns (CSRF token rotation, audit-trail headers) that should be considered before adding more response middleware ad-hoc.

**Gap-3:** No `IdleTimeoutStrategy` trait. §AMEND-3.II D12 would suggest one. KAIZEN says don't add it for 2-strategy case. Captain decision.

**Gap-4:** V2.1 PACT pin file says sliding-window is V2.1 scope. Wave-1 plan says W1-S5 (V2.0 timeline). The pin file is now stale; W1-S5 ship MUST update the pin to RETIRED-PULLED-FORWARD-TO-W1-S5 + amend middleware.rs:103-110 comment.

---

## §5 — V2-framed proposal

**V2 doctrine alignment:** This change moves the idle-timeout boundary from V1-era-style stateless-fixed-window-from-iat → V2 sliding-window-refresh-on-activity per §S-82 Q3 + V2.1 PACT pin substrate. It pulls the V2.1 PACT forward into Wave 1 launch-readiness.

### Implementation sketch (kaizen-min, minus the §AMEND-3.II D12 strategy trait per Gap-3)

1. **Add `IdleTimeoutStatus` return enum** to `is_idle_expired` (refactor from `bool` → `enum { Fresh, RefreshSoon, Expired }`). The middle case fires when token age > some threshold but < idle_timeout_secs; signals "re-issue this token."
   - File: `auth/middleware.rs:119-134`
   - Lines changed: ~10

2. **Refactor `extract_staff_claims`** to return `(StaffClaims, IdleTimeoutStatus)` instead of `Result<StaffClaims, ()>`. RefreshSoon path returns Ok with the status flag set; Expired path returns Err as today.
   - File: `auth/middleware.rs:66-117`
   - Lines changed: ~5

3. **Modify `require_staff_jwt`** to:
   - On RefreshSoon: insert StaffClaims into extensions AS TODAY, AND register a response-mutating callback to mint a fresh token + write Set-Cookie header AFTER the handler runs.
   - On Fresh: insert StaffClaims AS TODAY, no response mutation.
   - On Expired: 401 AS TODAY.
   - File: `auth/middleware.rs:144-161`
   - Lines changed: ~20

4. **Extract `mint_refreshed_jwt(claims) -> String` helper** that:
   - Uses `claims.role` (NOT default cashier) — closes Gap-1 of inherited-issue catalogue
   - Uses CURRENT secret (acceptable per §3 disposition; document the choice)
   - Sets new `iat = now`, `exp = now + 24h` (preserve current 24h exp window)
   - Returns the encoded JWT string
   - New helper, ~10 lines

5. **Cookie write path:**
   - Use existing cookie helpers in `auth/admin.rs` (V1 PACT-018-era pattern)
   - Cookie name: `staff_jwt` or new `staff_idle_refresh` — Captain Q-DECISION
   - Flags: httpOnly + Secure + SameSite=Strict per V2.1 PACT pin §2
   - Lines: ~5

6. **Audit log — DESIGN CHOICE per §3 open item 4:**
   - Recommendation: DO NOT log routine re-issuance (would 100x audit_log volume on busy POS). DO log idle-expiry rejections (already implicit in 401 path; explicit log preferred).
   - If Captain requires routine-refresh logging, surface as Q-DECISION and accept the volume.

7. **Tests** (per V2.1 PACT pin §"Test coverage" + new requirements from §3 open items):
   - 30-min activity simulation with 5-min request gaps → expect fresh `iat` after each request, no expiration (V2.1 pin §1)
   - 31-min idle gap → expect expiration on next request (V2.1 pin §2)
   - Token at `iat=now-29min` arriving → re-issued to `iat=now`; old token still valid until natural expiry (V2.1 pin §3 — overlap window test)
   - Manager + superadmin role NOT downgraded on refresh (closes §3 open item 3)
   - Cookie write path: response includes Set-Cookie with httpOnly + Secure + SameSite=Strict
   - W1-S3+S4 refund routing tests still pass (regression coverage)
   - `require_role` ordering invariant preserved
   - JWT secret rotation grace + refresh interaction test (closes §3 open item 5)

   Estimated count: 8-10 new tests + 14 existing refund_routing tests still pass.

8. **Cross-pilot impact (POS browser):**
   - Send a NOTIFY to bono via INBOX before merge: "Set-Cookie response header now arrives on every authenticated staff request; verify POS browser handles it as no-op write (browser auto-stores httpOnly cookies)."
   - Out of W1-S5 scope to MODIFY POS code; in scope to DOCUMENT the contract change.

9. **DEPLOY PARITY scope** (per CLAUDE.md DMP-MANDATORY rule):
   - racecontrol binary: REBUILD + redeploy to Server .23 + Bono VPS racecontrol
   - rc-agent: NO change
   - POS Web app: NO code change (browser cookie behavior is auto)
   - Admin/Kiosk: NO change
   - Comms-link: NO change
   - SWAPLOG row + LOGBOOK row required at deploy time

10. **Memory-file updates triggered by W1-S5 ship:**
    - `project_v2_1_sliding_window_idle_timeout_pact_pin.md` → status RETIRED-PULLED-FORWARD-TO-W1-S5 + commit-hash anchor
    - `project_v2_master_state.md` → §S-N entry naming W1-S5 ship + Q3 sliding-window CLOSED status
    - `MEMORY.md` → index entry with ⭐ marker
    - `LOGBOOK.md` row at racecontrol root
    - amend `middleware.rs:103-110` comment to remove "V2.1 PACT" language

### Estimated size

- Production code: ~50 LOC (item 1: 10 + item 2: 5 + item 3: 20 + item 4: 10 + item 5: 5)
- Test code: ~150-200 LOC (8-10 new tests at 15-25 LOC each)
- Documentation: 5 memory files + LOGBOOK + middleware.rs comment + V2-MASTER-STATE row
- Risk surface: foundational auth boundary; MMA Step 1 DIAGNOSE required (per doctrine)
- Estimated session length: ~2-3 hours for code + ~30 min for memory + ~30 min for MMA Step 1 + Captain auth wait

### Open Captain Q-DECISIONs surfaced by this RCA

| ID | Question | Default if Captain doesn't disposition |
|---|---|---|
| ~~Q-RECONCILE-1~~ | ~~Confirm W1-S5 IS the V2.1 sliding-window PACT executed early; authorize amending middleware.rs comment + V2.1 pin~~ | **✓ CLOSED — Captain EXPLICIT-RATIFY = AUTHORIZED 2026-05-09 ~10:45 IST per msg=35809.** V2.1 pin amendment to RETIRED-PULLED-FORWARD-TO-W1-S5 + middleware.rs:103-110 comment amendment both AUTHORIZED at W1-S5 ship time |
| ~~Q-S5-1~~ | ~~Cookie name: `staff_jwt` (overwrite) vs `staff_idle_refresh` (sibling)~~ | **✓ CLOSED — Captain G33 batch disposition 2026-05-09 ~11:23 IST: ACCEPT-DEFAULT.** `staff_jwt` overwrite (kaizen-smallest) ratified |
| ~~Q-S5-2~~ | ~~Audit log routine re-issuance?~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** NO routine logging on refresh; YES on idle-expiry 401 |
| ~~Q-S5-3~~ | ~~Adopt `IdleTimeoutStrategy` trait per §AMEND-3.II D12 OR keep direct logic per kaizen?~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT with RATIONALE-EXTENSION.** Keep direct logic; W1-S5 implementation MUST add `// Trait DEFERRED-TO-N3 per kaizen; if 3rd timeout-strategy variant lands, abstract here per §AMEND-3.II D12 Foundation/Strategy/Config separation.` comment at sliding-window-vs-fixed-window decision site (per bono RATIONALE-EXTENSION msg=35808 §A axis-6) |
| ~~Q-S5-4~~ | ~~JWT secret rotation: refresh always uses CURRENT secret?~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** Always CURRENT secret + document choice in code comment |
| ~~Q-S5-5~~ | ~~`idle_refresh_grace_secs` config knob — ship now or defer?~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** Defer config knob; hardcode grace window in `IdleTimeoutStatus::RefreshSoon` threshold; expose as config follow-up if observed need |
| ~~Q-S5-6~~ [added per bono FLAG-1 msg=35808 §A axis-4] | ~~Response-mutating-middleware-layer (Gap-2) — ratified pattern OR explicit one-off?~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** Explicit one-off with named anti-precedent (kaizen-narrow); future CSRF rotation / audit-trail headers / SameSite-policy-rotation must justify their own composition NOT inherit by precedent. W1-S5 implementation MUST add anti-precedent comment at response-mutating-middleware site naming this disposition |
| ~~Q-S5-7~~ [added per bono CAVEAT-1 msg=35808 §A axis-2 — see PLAN-1 in §2/§3] | ~~PHASE-1-WAVE-1-PLAN.md row 21+33 "7-min" vs 4 anchors saying 30-min — (a)/(b)/(c)~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT (a) plan-author typo class.** W1-S5 implementation ship MUST amend PHASE-1-WAVE-1-PLAN row 21+33 7-min→30-min in same commit |

---

## NOT TESTED (RCA AUTHORING phase — pre-implementation)

This is an authoring artifact, not a runtime fix. Items NOT exercised:

- **The proposed code change** — implementation is W1-S5 Session 4 work; this RCA is the gate-precursor only
- **MMA Step 1 DIAGNOSE on this RCA** — gated on Captain budget approval (~$2-5 OpenRouter); 5-model consensus on root causes per doctrine §"MMA escalation"
- **bono substantive AMPLIFIER** — bilateral doctrine; bono review of this RCA pending
- **Captain G33 ratification of Q-RECONCILE-1 + Q-S5-1..Q-S5-5** — disposition-needed before W1-S5 implementation can proceed
- **Per-PR Captain merge auth at PR-open** — gate STANDS for the actual W1-S5 PR (not this RCA artifact PR; though the RCA itself may live on a separate planning-doc branch or directly on wave-1 branch — pending Captain disposition)
- **POS browser real-cookie-handling test** — Wave 1 Session 7 E2E scope per PHASE-1-WAVE-1-PLAN.md §5.4
- **Production-shape concurrent staff request load (re-issuance under contention)** — separate workstream
- **Memory-file Universal Sync** for the bono mirror of this RCA — TBD whether RCA artifacts trigger Universal-Sync (probably NO, since they are project planning docs not project-scope feedback rules; but flag for Captain confirmation)

---

## Read trail

- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (doctrine; commit `8768b628` 2026-05-09 ~09:28 IST)
- `project_v2_1_sliding_window_idle_timeout_pact_pin.md` (V2.1 PACT pin; authored 2026-05-08 ~13:55 IST)
- `crates/racecontrol/src/auth/middleware.rs:103-114` (current K5 fixed-window)
- `crates/racecontrol/src/auth/middleware.rs:119-134` (`is_idle_expired`)
- `crates/racecontrol/src/auth/middleware_tests.rs:298-366` (idle-timeout test block)
- `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` row 33 (W1-S5 spec; authored 2026-05-08 ~19:05 IST)
- `racecontrol/.planning/specs/v2/PHASE-1-WIREUP-DEPLOY-MANIFEST.md` §3 K5 row (PR #64 substrate disclosure)
- PR #64 merge `991b5411` 2026-05-08 13:54 IST (fixed-window-from-iat ship)
- Captain §S-82 Q3 disposition 2026-05-07 ~05:30 IST (verbatim "30 min sliding-window")
- Captain Path A disposition 2026-05-08 ~13:50 IST (verbatim "merge PR #64 as fixed-window; file V2.1 sliding-window PACT post-launch")

---

— james / 2026-05-09 ~10:15 IST · W1-S5 RCA DRAFT authored under standing autonomy "Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously" 2026-05-09 ~10:07 IST · gates on Captain G33 review of Q-RECONCILE-1 + Q-S5-1..5 + bono AMPLIFIER + MMA Step 1 DIAGNOSE before W1-S5 H1 PLAN can be filed · per-PR Captain merge auth gate STANDS at W1-S5 PR-open (foundational auth boundary)
