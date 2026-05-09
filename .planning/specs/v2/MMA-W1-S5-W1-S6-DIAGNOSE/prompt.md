You are reviewing two Root Cause Analysis (RCA) documents for code changes in a Rust auth/middleware module of a racing-simulator SaaS (RacingPoint, V2 architecture).

CONTEXT: Captain Uday Singh (project owner) requires multi-model adversarial review (MMA Step 1 DIAGNOSE) before code authoring per "V1-dependent V2 sections require RCA + past-bug review BEFORE proceeding" doctrine. Both RCAs are at racecontrol commit bda06dc8 on branch feat/v2-wave-1-w1-s1-billing-service. Captain G33 has ALREADY ratified Q-DECISIONs in both RCAs (ACCEPT-ALL-DEFAULTS), so DON'T critique the Q-DECISION outcomes — focus on what's MISSING.

YOUR JOB: identify what the RCA author MISSED. Don't validate what's there — find what's NOT there.
1. Missed root causes / inherited issues / risk surfaces
2. Misclassified items (something marked "NOT-APPLICABLE" that's actually live risk; "RESOLVED" when not; etc.)
3. Implementation hazards the RCA didn't surface (concurrency, race conditions, error paths, observability gaps, cleanup paths)
4. Cross-cutting concerns (security, deployment, rollback, audit, data integrity)
5. Cross-RCA interactions (does W1-S5's sliding-window-refresh interact with W1-S6's PIN-rotate path?)

OUTPUT FORMAT — strict JSON only, no markdown, no preamble:
{
  "w1_s5": {
    "missed": [{"severity":"P0|P1|P2|P3","title":"...","reasoning":"...","suggested_action":"..."}],
    "misclassified": [{"item":"...","current_label":"...","correct_label":"...","reasoning":"..."}],
    "completeness_score": 1-5
  },
  "w1_s6": { same shape as w1_s5 },
  "cross_rca_interactions": [{"severity":"...","title":"...","reasoning":"..."}],
  "blocker_disposition": "PASS|REVISE|BLOCK",
  "blocker_reasoning": "..."
}

Severity guide: P0=must fix before merge, P1=should fix this PR, P2=fix in follow-up, P3=informational. Cap output ~3000 tokens. JSON only.

==========================================
W1-S5 RCA (sliding-window idle-timeout — V2 substrate REPLACES V1-era K5 fixed-window):
==========================================
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

==========================================
W1-S6 RCA (PIN-LOCKOUT auto-rotate + helpdesk@ email + per-staff-id rate-limit — NEW V2 file calling INTO 5 V1 modules):
==========================================
# W1-S6 RCA — PIN-LOCKOUT auto-rotate + helpdesk@ email + staff-id rate-limit

**Doctrine basis:** `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (Captain BILATERAL directive committed at comms-link `8768b628` 2026-05-09 ~09:28 IST). Sibling-of W1-S5 RCA (`15490644` 2026-05-09 ~10:18 IST).

**Author:** james · **Date:** 2026-05-09 ~10:33 IST · **Branch context:** `feat/v2-wave-1-w1-s1-billing-service` HEAD `15490644` (W1-S5 RCA at HEAD; W1-S6 RCA appends here)

**Status:** DRAFT-CAPTAIN-DISPOSITIONED-PRE-AMPLIFIER — Captain G33 batch disposition 2026-05-09 ~11:23 IST CLOSED Q-S6-1..7 ACCEPT-ALL-DEFAULTS + MMA Step 1 budget APPROVED up to $10 (W1-S5 + W1-S6 batched OR separate) + per-PR Captain merge auth STANDS at PR-open. **Captain dispositioned ahead of bono AMPLIFIER** — unusual ordering; if bono AMPLIFIER returns with substantive amendments, those will re-surface for Captain in next disposition cycle. 24h Captain correction-window 2026-05-10 ~11:23 IST.

**Gate sequence (POST Captain G33 batch disposition):**
- (1) bono AMPLIFIER — PENDING (msg=35812 ship triggered request; Captain pre-empted with disposition)
- (2) Captain G33 batch Q-S6-1..7 ✓ CLOSED — disposition 11:23 IST ACCEPT-ALL-DEFAULTS (this update)
- (3) MMA Step 1 DIAGNOSE on RCA — budget APPROVED up to $10; execution PENDING (Captain user decision on timing)
- (4) W1-S6 H1 PLAN — PENDING (gated on (3))
- Per-PR Captain merge auth at W1-S6 PR-open STANDS independently.

**Amendment log:**
- Captain G33 batch disposition 2026-05-09 ~11:23 IST: Q-S6-1..7 ACCEPT-ALL-DEFAULTS as enumerated in §5 Q-DECISION table — all 7 marked CLOSED below with disposition refs. MMA budget approved. Ahead-of-AMPLIFIER ordering noted (does NOT void bono AMPLIFIER welcome — substantive amendments still surface for next Captain cycle).

**Foundational-boundary classification:** YES — auth boundary per doctrine §"MMA escalation". Same gates as W1-S5: MMA Step 1 DIAGNOSE on the RCA itself + per-PR Captain merge auth at PR-open.

**Scope-narrow vs W1-S5:** W1-S6 is NEW V2 file calling INTO 5 V1 modules (vs W1-S5 = modify-existing V2 substrate that REPLACES V1-era K5 fixed-window). Same boundary class; different topology.

---

## Captain dispositions already in place (reduces Q-DECISION surface)

The Captain dispositioned this work substantially in §S-82 + supplementary segments:

| Disposition | Source | Status |
|---|---|---|
| 5 wrong PIN attempts → auto-rotate + helpdesk@ + WhatsApp Captain freeze at ≤3 resets/staff/hr cap | §S-82 Q1 + bono-suggested ≤3/hr cap (line 8024) | Q1 RATIFIED; ≤3/hr cap = bono SUGGESTION pending Captain explicit (Q-S6-3 below) |
| helpdesk@racingpoint.in is the secondary auth channel for 5-wrong reset case | §S-82 Q1.a | RATIFIED |
| Counter resets on first correct PIN within session | §S-82 Q1.c | RATIFIED |
| Every PIN-reset writes audit-log row | §S-82 Q1.d | RATIFIED |
| WhatsApp daily routine PIN delivery (W1-S7 scope) is SEPARATE from helpdesk@ within-day reset (W1-S6 scope) | §S-82 Q1.b + Q1.g | RATIFIED |
| SMTP transport: A2.c extend EmailAlerter shell-out (`crates/racecontrol/src/email_alerts.rs`) | comms-link `89b84fc5` 2026-05-09 ~10:05 IST | RATIFIED |
| Email body schema: `staff_name | employee_id | new_pin | pos_terminal_id | timestamp_ist | refund_attempt_context` | §S-82 Q1 implementation deps line 8023 | bono SUGGESTION; pending Captain explicit (Q-S6-5 below) |
| helpdesk@ monitoring policy + Q1.g.B Google Workspace forward (business-hours read + off-hours Captain mobile) | §S-147.1 W1-S6 Q-DECISION batch | PENDING Captain (single-paragraph batch shipped via msg=35801) |

The remaining Q-DECISIONs are implementation-shape questions, not policy questions. See §5.

---

## §1 — Boundary map

### V1↔V2 surface inventory

W1-S6 introduces NEW V2 file `crates/racecontrol/src/auth/staff_auth.rs` (per PHASE-1-WAVE-1-PLAN.md §1.1 row 33; NF-james-6 confirmed file-not-yet-exists). This V2 code calls INTO 5 V1 modules:

| V1 module | Path | Lines | What W1-S6 calls | Risk class at boundary |
|---|---|---|---|---|
| Email alerter (A2.c target) | `crates/racecontrol/src/email_alerts.rs` | 383 | `EmailAlerter::send_alert(pod_id, ...)` extension OR new `send_pin_rotation(staff_id, new_pin, context)` sibling | DIRECT-CRITICAL — V1 cooldowns are per-pod + venue-wide (1800s + 300s); W1-S6 needs per-staff-id semantics; key-collision risk if extending vs adding sibling |
| Audit log primitive | `crates/racecontrol/src/accounting_audit.rs` (PACT-091) + `accounting.rs:15` re-exports `log_admin_action` | 121 | `accounting::log_admin_action(state, action_type="staff_pin_auto_reset", ...)` (mirror W1-S3 refund_3band_a/b/c pattern) | INDIRECT — same column W1-S3 uses; PACT-091 `audit_log.action_type` column required (already on main) |
| Staff PIN persistence | `crates/racecontrol/src/api/staff_pin_sync.rs` | 235 | `change_staff_pin_safe()` — Phase 347-01 orchestrated PIN change; cloud-authoritative write path | DIRECT-CRITICAL — W1-S6 must write new auto-rotated PIN through this path, NOT a parallel write; cloud_authority_guard governs venue→cloud forwarding |
| IP-keyed rate-limit (V1 abstraction) | `crates/racecontrol/src/auth/rate_limit.rs` | 125 | NOT REUSABLE — tower_governor is `PeerIpKeyExtractor` (per-IP); POS .130 shares IP across all staff. W1-S6 needs PER-STAFF-ID semantics. | NEW-PRIMITIVE-NEEDED — flag for §3 disposition |
| Cookie/session helpers | `crates/racecontrol/src/auth/admin.rs` | 471 | NOT directly called by W1-S6 PIN-rotate logic itself; consumed indirectly via `auth/middleware.rs::require_staff_jwt` for the privileged action gate | NOT-APPLICABLE-DIRECTLY |

### Cross-organ data flow at the boundary

1. **Inbound POST to privileged action** (e.g., refund route) carries Bearer JWT with `StaffClaims`.
2. **`require_staff_jwt`** middleware extracts claims; PIN-attempt counter is NOT in JWT — it's persisted server-side keyed by `staff_id`.
3. **Wrong-PIN handler** (likely in existing `validate_pin_format` flow or new `pin_lockout::check_attempt`) increments per-staff-id counter.
4. **At 5 wrong attempts:** new W1-S6 path fires:
   - Generate new PIN (CSPRNG; format-validated per `validate_pin_format`)
   - Persist via `change_staff_pin_safe(state, staff_id, new_pin)` — V1 path
   - Increment per-staff-id reset-counter (NEW V2 abstraction)
   - If reset-counter > 3/hr → freeze account + WhatsApp Captain `917981264279`
   - Else: dispatch helpdesk@ email via `EmailAlerter` extension (A2.c RATIFIED)
   - Write audit-log row via `log_admin_action(action_type="staff_pin_auto_reset", ...)`
5. **Email transport:** `EmailAlerter` shells out to `comms-link/shared/send-email.js` (Strategy 1 sendmail / Strategy 2 raw SMTP `localhost:25`)
6. **POS/staff response:** 401 with "PIN auto-rotated; check helpdesk@ email" body OR redirect to Forgot-PIN page (Captain Q1 disposition)

### Schema / state surfaces

- **`audit_log.action_type` column** (PACT-091; already on main per W1-S3 handoff Coupling) — accepts `"staff_pin_auto_reset"` per Captain Q1.d RATIFIED. NEW value, no schema migration.
- **Per-staff-id attempt-counter + reset-counter state** — NEW state. Two options: in-memory `tokio::sync::RwLock<HashMap<StaffId, AttemptState>>` (simpler, lost on restart) OR DB-backed (`staff_pin_attempts` table, durable, cloud-syncable). See Q-S6-2 below.
- **Staff PIN storage** — existing infra at `change_staff_pin_safe`. NEW PINs follow same persistence + cloud-sync flow.
- **Email cooldown HashMap** in `EmailAlerter::last_sent_per_pod` — currently per-pod-key. W1-S6 needs per-staff-id key semantics (separate HashMap or generalize to `last_sent_per_key`). See Q-S6-1 below.

### Configuration surfaces

- **Captain-reserve PARAMETERs** (per §S-82 Q1 implementation deps):
  - `auth.helpdesk_email_recipient = "helpdesk@racingpoint.in"` (NEW config key)
  - `auth.pin_reset_rate_limit_per_hour = 3` (bono-default per line 8024; pending Captain explicit Q-S6-3)
  - `auth.pin_lockout_attempts = 5` (Captain RATIFIED §S-82 Q1)
  - `auth.captain_freeze_whatsapp_number = "917981264279"` (existing)
  - `auth.helpdesk_monitoring_hours = "business-hours"` OR `auth.helpdesk_off_hours_forward_to_captain = true` (gates on Captain Q1.g.B disposition per §S-147.1)
- **Email script_path** — existing `EmailAlerter` field; reuse as-is (A2.c RATIFIED extends not replaces)

---

## §2 — Inherited-issue catalogue

| ID | Source | Issue | Scope at this boundary |
|---|---|---|---|
| EA-1 | `email_alerts.rs:9-30` | `last_sent_per_pod: HashMap<String, DateTime<Utc>>` is unbounded — never pruned. Long-running process accumulates entries for every key. | DIRECT — W1-S6 adds per-staff-id keys; with N staff over time, unbounded growth. Mitigation: TTL cleanup pass or LRU. |
| EA-2 | `email_alerts.rs:69-83` | Per-pod 1800s + venue-wide 300s cooldowns ASSUME alert-class semantics (one alert per failure window). | DIRECT — W1-S6 PIN-rotate is event-class not alert-class. A staff member legitimately rotating during 5-wrong attempt SHOULD always get the email regardless of cooldown. Cooldown semantics CONFLICT with W1-S6 use case. |
| EA-3 | `email_alerts.rs` script_path shell-out | Strategy 1 (sendmail -t -i) requires sendmail in PATH; Strategy 2 (SMTP localhost:25) requires local SMTP daemon. NEITHER verified at Server .23 / Bono VPS today. | DIRECT — A2.c RATIFIED but transport-substrate not verified. §12.3 PHASE-1-WAVE-1-PLAN.md flags Session 5 entry probe required: "Verify helpdesk@racingpoint.in mailbox provisioning + Google Workspace SMTP config status". |
| EA-4 | `email_alerts.rs` no DKIM/SPF setup verified for `racingpoint.in` domain | Sender reputation risk: PIN-rotate emails from raw SMTP could land in spam/quarantine; helpdesk@ never receives. | DIRECT — affects Q1 customer-service-priority axis (CR-3 in `project_v2_customer_workflows_consolidated_20260503.md`). Probe required at Session 5 entry. |
| RL-1 | `auth/rate_limit.rs:1-22` | `tower_governor::PeerIpKeyExtractor` keys on socket peer IP. POS .130 is single shared IP across all staff at venue. | DIRECT-CRITICAL — V1 IP-keyed rate-limit is FUNDAMENTALLY UNUSABLE for per-staff-id ≤3 resets/hr semantic. W1-S6 needs NEW abstraction. |
| RL-2 | `auth/rate_limit.rs:14-15` SEC-RESIL-03 burst=20 | "8 pods + kiosk can submit 9+ concurrent PIN validations" — ALL FROM SAME IP. Burst=20 prevents legitimate concurrent-PIN-validate from being rate-limited; same IP-key root cause. | INDIRECT — confirms RL-1 root cause; also informs why per-staff-id key is the right abstraction for V2. |
| SP-1 | `staff_pin_sync.rs:21+` `change_staff_pin_safe` | Cloud-authoritative write path: venue → cloud forward. Cloud sync race conditions during high-volume rotate-storms (e.g., 8 staff get locked-out in same minute). | INDIRECT — W1-S6 amplifies cloud-write traffic; existing SP infra has handled multi-second concurrent writes per `cloud_authority_guard`. Probe: confirm SP throughput envelope. |
| SP-2 | `staff_pin_sync.rs::validate_pin_format` | V1 PIN format constraints (length, character set). Auto-generated PIN must pass these constraints. | DIRECT — W1-S6 CSPRNG must respect `validate_pin_format`; reuse existing function in PIN-gen path. |
| AL-1 | (no prior anchor) | `audit_log.action_type` write amplification: every PIN-rotate event = 1 audit row (LOW volume; ≤3/staff/hr × N staff = bounded). | NOT-APPLICABLE — write volume is bounded; not a concern. |
| AL-2 | `audit_log.action` CHECK constraint (`'create','update','delete'`) per `migrate_policy.rs` schema | W1-S6 must use `log_admin_action` (writes `action_type` column, action='create' fixed) NOT `log_audit` (which is constrained CRUD). Per W1-S3 NF-james-8 axis-distinction. | DIRECT — same disposition as W1-S3+S4; reuse pattern. |
| WS-1 | racingpoint-whatsapp-bot existing infra | Captain-freeze WhatsApp dispatch to `917981264279` — uses existing Evolution API instance "Racing Point Reception". A3 RATIFIED for W1-S7+S8 daily-PIN; same transport for W1-S6 freeze-event. | INDIRECT — reuse A3 transport; cross-pilot bono-substrate dependency for actual send. |

---

## §3 — Past-bug disposition

| Item | Disposition | Evidence / forward action |
|---|---|---|
| EA-1 unbounded `last_sent_per_pod` HashMap | **PATCHED-ONLY (open RCA item)** — no prior bug ticket; growth is slow but unbounded. | W1-S6 design choice: extend EmailAlerter with TTL-purge pass OR sibling staff-id HashMap with TTL purge. Recommendation: sibling HashMap + TTL purge (smaller blast radius). |
| EA-2 cooldown semantics conflict | **NOT-APPLICABLE-TO-V2** — V1 cooldowns are correct for V1 use case (alert flooding). V2 PIN-rotate is event-class; bypass cooldown entirely. | W1-S6 design: NEW `send_pin_rotation_email` path that does NOT consult cooldown HashMap; OR `EmailAlerter::send_event_email(...)` sibling that bypasses. |
| EA-3 transport unverified | **UNRESOLVED — open RCA item; Session 5 entry probe required** | Per §12.3 PHASE-1-WAVE-1-PLAN.md: probe sendmail availability at Server .23, raw SMTP at Bono VPS, OR pivot to Google Workspace API. Q-S6-4 below. |
| EA-4 DKIM/SPF | **UNRESOLVED — open RCA item; Session 5 entry probe required** | Probe `dig +short TXT racingpoint.in` for SPF; check DNS for DKIM selector. If absent: helpdesk@ delivery is at risk. Captain may need to disposition: ship with risk + monitor first delivery OR delay W1-S6 until DKIM/SPF up. Q-S6-4 below. |
| RL-1 IP-keyed not staff-keyed | **NOT-APPLICABLE-TO-V2 (as a "bug")** — V1 IP-keyed serves V1 use case (per-IP burst). W1-S6 needs DIFFERENT abstraction; cannot extend V1, must add NEW. | W1-S6 design: new module `crates/racecontrol/src/auth/staff_rate_limit.rs` OR inline in `staff_auth.rs`. NEW per-staff-id sliding-window primitive (1hr window, 3 resets cap). Q-S6-2 below. |
| SP-1 cloud-sync race | **ROOT-CAUSED-AND-FIXED** via Phase 347-01 orchestration (`change_staff_pin_safe`) — venue→cloud forward + verify pattern. | Reuse as-is. |
| SP-2 PIN format validation | **ROOT-CAUSED-AND-FIXED** — `validate_pin_format` exists. | Reuse in CSPRNG PIN-gen path. |
| AL-1 audit write amplification | **NOT-APPLICABLE** — bounded volume. | No mitigation needed. |
| AL-2 action vs action_type column | **ROOT-CAUSED-AND-FIXED** — W1-S3 established the `log_admin_action` pattern; W1-S6 reuses. | Reuse pattern verbatim. |
| WS-1 WhatsApp transport | **ROOT-CAUSED-AND-FIXED** — A3 RATIFIED for W1-S7+S8 transport; W1-S6 freeze-event uses same path. | Reuse via cross-pilot bono substrate at W1-S6 ship time. |

**Open RCA items requiring W1-S6 design choice:**
1. EA-1 HashMap unbounded growth — TTL-purge or LRU
2. EA-2 cooldown bypass — sibling path or new method
3. EA-3 transport substrate verification at Session 5 entry probe
4. EA-4 DKIM/SPF for `racingpoint.in` — Session 5 entry probe; potentially Captain Q-DECISION on ship-with-risk-vs-delay
5. RL-1 NEW per-staff-id rate-limit primitive — module placement + state durability

---

## §4 — V2-alignment delta

### What V2 doctrine says the boundary should look like

| V2 anchor | Statement | Current alignment |
|---|---|---|
| `project_v2_master_state.md` §S-82 Q1+Q1.a..h | Captain dispositioned 5-wrong + helpdesk@ + WhatsApp daily routine + counter-reset + audit-log + bono defaults Q1.e-h | NOT-YET-IMPLEMENTED — W1-S6 closes this gap |
| `project_v2_customer_workflows_consolidated_20260503.md` CR-3 customer-service-priority | "Customer service is the priority. Failure handling is first-class — not afterthought." | DIRECT-ALIGNMENT — W1-S6 prevents staff lockout cascading into customer-service-failure-mode |
| `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (THIS doctrine) | RCA before action | THIS DOCUMENT is the RCA |
| §S-147.1 W1-S6 Q-DECISION batch (Q1.g.B Google Workspace forward) | Captain disposition awaited on helpdesk@ business-hours vs off-hours forward to Captain mobile | PENDING Captain (bundled in single-paragraph batch shipped via msg=35801) |
| `feedback_kaizen_discipline_dont_complicate.md` | Smallest invariant for observed requirement | RISK — W1-S6 introduces NEW staff-id rate-limit primitive that V1 didn't need; risk of overscope if generalized prematurely |
| comms-link `89b84fc5` Captain RATIFY A2.c | "extend EmailAlerter shell-out" — reuse V1 transport infra | DIRECT-ALIGNMENT — A2.c CONCUR-RATIFIED disposition matches W1-S6 design |
| `feedback_emergent_directed_spend_protocol.md` Rule 4 (specify-codebase-identity) | Don't substitute mental model for environment | OK — every claim in this RCA cites a path/line/commit |
| §AMEND-3.II D12 (Foundation/Strategy/Config separation) | Strategy classes for substitutable behavior | NOT-APPLICABLE — W1-S6 has no strategy variation; single PIN-rotate flow |

### Named gaps

**Gap-1:** No staff-id-keyed rate-limit primitive exists in V1. W1-S6 introduces NEW abstraction. Risk: precedent for other per-staff-id rate-limits (refund-rate / launch-rate / etc.) — should the abstraction be reusable from inception, or kaizen-narrow to PIN-reset only? Q-S6-2.

**Gap-2:** EmailAlerter cooldown semantics (per-pod-key + per-venue-key with 1800s/300s windows) conflict with W1-S6 event-class semantics (always-deliver per PIN-rotate event, bounded by rate-limit-not-cooldown). W1-S6 closes this by sibling-path or method-on-EmailAlerter that bypasses cooldown. Q-S6-1.

**Gap-3:** SMTP transport substrate (sendmail / raw SMTP / Google Workspace API) unverified at Server .23 + Bono VPS; DKIM/SPF for `racingpoint.in` unverified. W1-S6 ship is gated on Session 5 entry probe + potentially Captain Q-DECISION on ship-with-risk if DKIM/SPF unready. Q-S6-4.

**Gap-4:** helpdesk@racingpoint.in monitoring policy + Q1.g.B Google Workspace business-hours-vs-off-hours forward to Captain mobile is a Captain disposition pending. W1-S6 implementation is mostly transport-policy-agnostic (just send the email; Captain dispositions where the email lands), but the email body schema may include "Captain on-call" instructions if Q1.g.B = forward-to-mobile.

---

## §5 — V2-framed proposal

**V2 doctrine alignment:** This change introduces W1-S6 PIN-LOCKOUT auto-rotate as a V2-aligned customer-service-first auth-resilience primitive (CR-3 + §S-82 Q1). It REUSES V1 transport infra (EmailAlerter shell-out per A2.c RATIFIED) without inheriting V1 IP-keyed rate-limit (introduces NEW per-staff-id primitive).

### Implementation sketch (kaizen-min)

1. **NEW module `crates/racecontrol/src/auth/staff_auth.rs`** (~80-100 LOC production):
   - `pub struct PinLockoutTracker { attempts: Mutex<HashMap<StaffId, AttemptState>>, resets: Mutex<HashMap<StaffId, ResetState>> }`
   - `pub fn record_attempt(staff_id, success: bool) -> AttemptOutcome { Continue, LockoutTriggered }`
   - `pub async fn execute_lockout(state, staff_id) -> Result<LockoutOutcome>` — orchestrates: gen new PIN → persist via `change_staff_pin_safe` → check reset-rate-limit → dispatch email or freeze+WhatsApp → log audit
   - `pub fn reset_attempts(staff_id)` (Captain Q1.c — counter resets on first correct PIN)
   - Format: ~40 LOC for PinLockoutTracker + ~40 LOC for execute_lockout
   - **Composes with**: `staff_pin_sync::change_staff_pin_safe` (PIN write) + `accounting::log_admin_action` (audit) + `EmailAlerter` extension (email) + `validate_pin_format` (PIN format)

2. **Extend `EmailAlerter` with `send_pin_rotation` method** (~20 LOC):
   - Sibling to `send_alert`; bypasses per-pod + venue-wide cooldowns (event-class not alert-class)
   - Body schema: `staff_name | employee_id | new_pin | pos_terminal_id | timestamp_ist | refund_attempt_context`
   - Uses same script_path shell-out (A2.c RATIFIED)

3. **NEW per-staff-id rate-limit primitive** (~30 LOC; Gap-1):
   - **Option A (recommended)**: inline in `staff_auth.rs` as `ResetState { count: u32, window_start: DateTime<Utc> }` — kaizen-min, scoped to PIN-reset use case only
   - **Option B**: NEW module `crates/racecontrol/src/auth/staff_rate_limit.rs` with generic `StaffRateLimiter<Key>` for future per-staff-id rate-limits (refund-rate, launch-rate)
   - Disposition Q-S6-2 below; recommend Option A

4. **WhatsApp Captain freeze dispatch** (~15 LOC; cross-pilot via bono substrate):
   - On 4th reset attempt within 1hr → freeze account flag + WhatsApp `917981264279` "Staff <name> account FROZEN: 4+ PIN-resets/hr"
   - Reuses A3 RATIFIED Evolution API "Racing Point Reception" instance (W1-S7+S8 sub-LEAD bono); racecontrol-side calls into existing `whatsapp_send` infra (TODO: confirm path)

5. **Audit log integration** (~10 LOC):
   - `log_admin_action(state, action_type="staff_pin_auto_reset", staff_id, ...)` — mirror W1-S3 pattern
   - JSON payload: `{ old_pin_hash, new_pin_hash, attempt_count_at_lockout, reset_count_in_window, helpdesk_email_dispatched, whatsapp_captain_dispatched }`

6. **Tests** (~200-250 LOC):
   - Unit: PinLockoutTracker — 1st attempt / 4 wrong attempts / 5th wrong triggers lockout / counter-reset on success / reset-rate-limit boundary (3rd within 1hr OK; 4th triggers freeze)
   - Unit: EmailAlerter::send_pin_rotation cooldown bypass — proves event-class semantic
   - Unit: PIN-gen via CSPRNG passes `validate_pin_format` — N=100 samples
   - Integration: full flow staff PIN attempt → 5 wrong → execute_lockout → audit_log row written + EmailAlerter::send_pin_rotation invoked (test double for actual SMTP) + new PIN persisted via change_staff_pin_safe (real cloud_authority_guard test or test double)
   - Integration: 4th reset within 1hr → freeze + WhatsApp test double invoked (NOT real send)
   - Estimated: 8-10 unit + 4-6 integration tests

7. **DEPLOY PARITY scope**:
   - racecontrol binary: REBUILD + redeploy Server .23 + Bono VPS racecontrol
   - rc-agent: NO change
   - POS Web app: NO code change (cookie/redirect behavior is same)
   - Admin/Kiosk: NO change
   - Comms-link: NO change (existing send-email.js reused)
   - send-email.js (bono-side): VERIFY availability at Server .23 + Bono VPS at Session 5 entry probe (Q-S6-4)
   - SWAPLOG row + LOGBOOK row required at deploy time

8. **Memory-file updates triggered by W1-S6 ship**:
   - `project_v2_master_state.md` → §S-N entry naming W1-S6 ship + Q1 closure status
   - `MEMORY.md` → index entry with ⭐ marker
   - `LOGBOOK.md` row at racecontrol root
   - `feedback_v1_dependent_v2_root_cause_before_proceeding.md` → empirical anchor #2 (RCA-rule applied successfully second time, sibling W1-S5 anchor #1)

### Estimated size

- Production code: ~155-175 LOC (PinLockoutTracker + EmailAlerter extension + rate-limit primitive + WhatsApp dispatch + audit integration)
- Test code: ~200-250 LOC (8-10 unit + 4-6 integration)
- Documentation: 4 memory files + LOGBOOK + V2-MASTER-STATE row
- Risk surface: foundational auth boundary; MMA Step 1 DIAGNOSE required (per doctrine)
- Estimated session length: ~2-3hr code + ~30min memory + ~30min MMA Step 1 + Captain auth wait

### Open Captain Q-DECISIONs surfaced by this RCA

| ID | Question | Disposition |
|---|---|---|
| ~~Q-S6-1~~ | ~~EmailAlerter extension shape — sibling method vs generalized~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** Sibling method `send_pin_rotation` (bypass cooldowns); kaizen-smallest scoped to W1-S6 |
| ~~Q-S6-2~~ | ~~Per-staff-id rate-limit placement — inline OR NEW module~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** Inline in staff_auth.rs (kaizen-min); only 1 use case in V2.0 |
| ~~Q-S6-3~~ | ~~≤3 resets/staff/hr cap~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** 3 resets/staff/hr (bono SUGGESTION ratified by Captain) |
| ~~Q-S6-4~~ | ~~SMTP+DKIM/SPF Session 5 entry probe disposition~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** Probe-and-decide at Session 5 entry; if DKIM/SPF absent, surface as Captain Q-DECISION BEFORE code (gate IN-WORKFLOW; ship-with-risk NOT pre-authorized) |
| ~~Q-S6-5~~ | ~~Email body schema enumeration~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** Ship enumerated fields per §S-82 line 8023 (`staff_name|employee_id|new_pin|pos_terminal_id|timestamp_ist|refund_attempt_context`) |
| ~~Q-S6-6~~ | ~~Lockout state durability — in-memory OR DB-backed~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** In-memory HashMap (kaizen-min; restart-after-5-wrong acceptable per CR-3 customer-service-priority — slight forgiveness toward staff). DB-backed deferred to V2.1 if abuse pattern emerges |
| ~~Q-S6-7~~ (depends on §S-147.1 disposition) | ~~Email body includes "after-hours: Captain mobile" OR transparent forwarding~~ | **✓ CLOSED — Captain G33 ~11:23 IST: ACCEPT-DEFAULT.** Transparent Workspace forwarding (staff sees same body; Workspace routing handles delivery to Captain mobile if Q1.g.B disposition routes that way) |

---

## NOT TESTED (RCA AUTHORING phase — pre-implementation)

This is an authoring artifact, not a runtime fix. Items NOT exercised:

- **The proposed code change** — implementation is W1-S6 Session 5 work; this RCA is the gate-precursor only
- **MMA Step 1 DIAGNOSE on this RCA** — gated on Captain budget approval (~$2-5 OpenRouter); 5-model consensus on root causes per doctrine §"MMA escalation"
- **bono substantive AMPLIFIER** — bilateral doctrine; bono review of this RCA pending (composes-with W1-S5 RCA bono notify `387988bb` pending)
- **Captain G33 ratification of Q-S6-1..7** — disposition-needed before W1-S6 implementation can proceed
- **Captain disposition on Q1.g.B per §S-147.1 batch** — Q-S6-7 is downstream of Q1.g.B
- **Per-PR Captain merge auth at W1-S6 PR-open** — gate STANDS for the actual W1-S6 PR (not this RCA artifact PR)
- **Session 5 entry probe** — sendmail availability at Server .23 / SMTP localhost:25 at Bono VPS / DKIM+SPF for `racingpoint.in` / Google Workspace API auth scope
- **POS browser behavior on auto-rotate redirect to Forgot-PIN page** — Wave 1 Session 7 E2E scope per PHASE-1-WAVE-1-PLAN.md §5.4
- **Production-shape concurrent staff-PIN-attempt under contention** — separate workstream
- **Cross-pilot bono substrate availability for Captain-freeze WhatsApp dispatch** — coordinate with bono at Session 5 ship-time; A3 RATIFIED transport but actual call-site path racecontrol-side TBD
- **Memory-file Universal Sync** for the bono mirror of this RCA — same disposition as W1-S5 (probably NO; planning artifact not project-scope feedback rule)

---

## Read trail

- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (doctrine; commit `8768b628` 2026-05-09 ~09:28 IST)
- `racecontrol/.planning/specs/v2/W1-S5-RCA.md` (sibling RCA; structural template)
- `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` row 33 + §2 Session 5 + §5.2 Integration tests + §12.3 A2 deferred Session 5 (W1-S6 specs)
- `comms-link/V2-MASTER-STATE.md` §S-82 Q1+Q1.a..h dispositions (lines 7987-8083) + §S-147.1 W1-S6 Q-DECISION batch
- comms-link `89b84fc5` 2026-05-09 ~10:05 IST (Captain RATIFY A2.c — extend EmailAlerter shell-out)
- `crates/racecontrol/src/email_alerts.rs` (V1 EmailAlerter; A2.c extension target)
- `crates/racecontrol/src/auth/rate_limit.rs` (V1 IP-keyed; NOT REUSABLE)
- `crates/racecontrol/src/api/staff_pin_sync.rs` (V1 PIN persistence; reuse `change_staff_pin_safe`)
- `crates/racecontrol/src/accounting_audit.rs` + `accounting.rs` re-exports (V1 audit-log; reuse `log_admin_action` per W1-S3 NF-james-8 pattern)
- `crates/racecontrol/src/db/migrate_policy.rs:51-61` (`audit_log` schema + `action_type` column PACT-091)
- `comms-link/shared/send-email.js` (V1 transport; A2.c shell-out target — Session 5 entry probe required for Strategy 1/2 substrate)
- `racecontrol/CLAUDE.md` Standing Rules + Doctrine Conventions (Substrate-Pointer Convention applies)

---

— james / 2026-05-09 ~10:33 IST · W1-S6 RCA DRAFT authored under standing autonomy "Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously" (Captain Option Bravo class-level V2-aligned auth re-affirmation 2026-05-09 ~10:07 IST + extended) · gates on Captain G33 review of Q-S6-1..7 + bono AMPLIFIER + MMA Step 1 DIAGNOSE before W1-S6 H1 PLAN can be filed · per-PR Captain merge auth gate STANDS at W1-S6 PR-open (foundational auth boundary) · sibling-of W1-S5 RCA `15490644` (same doctrine, second empirical application)
