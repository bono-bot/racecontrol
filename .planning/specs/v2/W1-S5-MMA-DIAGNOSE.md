# W1-S5 MMA Step 1 DIAGNOSE — Sliding-window idle-timeout RCA

**Author:** james · **Date:** 2026-05-09 ~12:00 IST · **Branch:** `feat/v2-wave-1-w1-s1-billing-service`

**Trigger:** Captain G33 batch disposition 2026-05-09 ~11:23 IST (commit `bda06dc8`) approved MMA Step 1 budget up to $10 OpenRouter for W1-S5 + W1-S6 RCAs (batched OR separate). This is the W1-S5 invocation, run separately so the consensus is artifact-specific.

**Doctrine basis:** `feedback_v1_dependent_v2_root_cause_before_proceeding.md` doctrine §"MMA escalation" (foundational auth boundary triggers MMA Step 1 DIAGNOSE on the RCA itself before H1 PLAN). UNIFIED-MMA-PROTOCOL.md §"4-Step Convergence Engine" Step 1.

**Input artifact:** `.planning/specs/v2/W1-S5-RCA.md` at branch HEAD `bda06dc8` (290 lines · 32,397 chars · ~8,099 input tokens).

**Outcome:** **APPROVE-WITH-AMENDMENTS** (5/5 unanimous) · defensibility score avg **4.2/5** (4, 4, 5, 4, 4) · **8 consensus findings (≥3/5)** require amendment-into-RCA-v3 OR explicit Captain deferral before W1-S5 H1 PLAN can be filed.

---

## §1 — Run log

### Panel selection (per CLAUDE.md vendor-diversity rules)

| Slot | Model ID | Short | Vendor | Role | Constraints satisfied |
|---|---|---|---|---|---|
| 1 | `deepseek/deepseek-r1-0528` | deepseek-r1 | deepseek | reasoner | ≥1 reasoner ✓ |
| 2 | `deepseek/deepseek-chat-v3-0324` | deepseek-v3 | deepseek | code_expert | ≥1 code_expert ✓ |
| 3 | `qwen/qwen3-coder` | qwen3-coder | qwen | code_expert | second code_expert (different vendor) ✓ |
| 4 | `xiaomi/mimo-v2-pro` | mimo-v2-pro | xiaomi | sre | ≥1 sre ✓ |
| 5 | `google/gemini-2.5-flash` | gemini-flash | google | generalist | ≥1 generalist ✓ |

- **Vendor families:** deepseek (×2), qwen, xiaomi, google → **4 families** (≥3 required ✓)
- **Max-per-vendor:** deepseek=2 (≤2 cap ✓)
- **Required roles:** reasoner ✓ + code_expert ✓ + sre ✓ + generalist ✓

### Budget log (per CGP Standing Rule §S-49 + MMA Standing Rules MMA-21)

| Model | prompt_tokens | completion_tokens | $ in (rate) | $ out (rate) | Total $ |
|---|---:|---:|---:|---:|---:|
| deepseek-r1 | 10,406 | 3,355 | $0.0047 ($0.45/M) | $0.0072 ($2.15/M) | **$0.0119** |
| deepseek-v3 | 10,404 | 542 | $0.0021 ($0.20/M) | $0.0004 ($0.77/M) | **$0.0025** |
| qwen3-coder | 10,525 | 938 | $0.0023 ($0.22/M) | $0.0009 ($1.00/M) | **$0.0033** |
| mimo-v2-pro | 10,766 | 1,681 | $0.0108 ($1.00/M) | $0.0050 ($3.00/M) | **$0.0158** |
| gemini-flash | 11,170 | 1,655 | $0.0017 ($0.15/M) | $0.0010 ($0.60/M) | **$0.0027** |
| **Total** | **53,271** | **8,171** | — | — | **$0.0361** |

- Budget cap (Captain G33 ~11:23 IST): **$10.00**
- Headroom remaining: **$9.96** (99.6% unused; W1-S6 DIAGNOSE shares the same envelope)
- Wall-clock: 176,472 ms (~3 min) · longest single call: deepseek-r1 (176s) · shortest: gemini-flash (12s)
- Iterations: **1** (consensus reached on first pass; no Step 1 backtrack needed)

### Per-model raw outputs (for audit trail)

- `.tmp/mma-w1-s5/responses/deepseek-r1.md` (3,253 chars)
- `.tmp/mma-w1-s5/responses/deepseek-v3.md` (2,373 chars)
- `.tmp/mma-w1-s5/responses/qwen3-coder.md` (3,938 chars)
- `.tmp/mma-w1-s5/responses/mimo-v2-pro.md` (4,970 chars)
- `.tmp/mma-w1-s5/responses/gemini-flash.md` (6,913 chars)
- `.tmp/mma-w1-s5/summary.json` (machine-readable run summary)

These are uncommitted scratch outputs at the time of writing. Citation lines below quote them; the structured findings section is the authoritative DIAGNOSE record.

---

## §2 — Per-model verdict matrix

| Model | Recommendation | Defensibility | HC concerns | Top concern |
|---|---|---:|---:|---|
| deepseek-r1 | APPROVE-WITH-AMENDMENTS | 4/5 | 3 | Post-handler callback breaks under future middleware composition |
| deepseek-v3 | APPROVE-WITH-AMENDMENTS | 4/5 | 2 | JWT replay attack possible during refresh grace window |
| qwen3-coder | APPROVE-WITH-AMENDMENTS | 5/5 | 3 | `mint_refreshed_jwt` may bypass `VALID_ROLES` re-check |
| mimo-v2-pro | APPROVE-WITH-AMENDMENTS | 4/5 | 5 | `iat` semantics shift may break V1 clients snapshotting `iat` |
| gemini-flash | APPROVE-WITH-AMENDMENTS | 4/5 | 3 | `IdleTimeoutStatus` `RefreshSoon` threshold undefined |

**Verdict consensus:** **5/5 APPROVE-WITH-AMENDMENTS** · NO model said REJECT · NO model said unconditional APPROVE.

**Score consensus:** mean 4.2 / median 4 / range 4–5 · per protocol Step 1 PASS threshold ≥3.0 of 5; Step 4 VERIFY threshold ≥4.0 — this DIAGNOSE clears Step 1 with margin and projects above the Step 4 threshold.

---

## §3 — Consensus findings (3/5-majority, action-required)

These 8 findings were raised by ≥3/5 models. Per UNIFIED-MMA-PROTOCOL.md §"Consensus voting", 3/5 = consensus. Each must either (a) land as an amendment in RCA-v3 with explicit code/test addition, or (b) be Captain-dispositioned as out-of-W1-S5-scope with a deferral target.

### MMA-1 · Cookie overwrite/collision risk between PIN cookie and idle-refresh cookie · **5/5 UNANIMOUS** · severity P1

| Model | Citation |
|---|---|
| deepseek-r1 | C-3 "Cookie helper reuse may inherit incorrect attributes" — `auth/admin.rs` cookie path/domain consistency audit needed |
| deepseek-v3 | C-2 "Cookie collision risk between PIN cookie and idle-refresh cookie" — `auth/admin.rs` cookie helpers |
| qwen3-coder | C-2 "Cookie name reuse (`staff_jwt`) risks invalidating active PIN cookies" — overlapping httpOnly namespace |
| mimo-v2-pro | C-2 "Cookie overwrite approach (Q-S5-1 `staff_jwt`) may cause POS browser issues if PIN cookie and idle-refresh cookie have different lifetimes/security attributes" |
| gemini-flash | C-3 "Confirm overwriting `staff_jwt` doesn't cause race conditions or unexpected behavior if the client expects a stable JWT value for a short period" |

**Why it matters:** Q-S5-1 dispositioned `staff_jwt` overwrite (kaizen-smallest), but the PIN-issuance path also writes a `staff_jwt`-named cookie (per RCA §1 schema-state-surfaces). If the two paths emit different `httpOnly` / `Secure` / `SameSite` / `Path` / `Max-Age` attributes, browsers will treat them as distinct cookies (RFC 6265 §5.3) — potentially producing two `staff_jwt` cookies in the jar, or silently dropping one based on attribute precedence. Either outcome breaks staff auth in subtle ways no health-check catches.

**Recommended amendment for RCA-v3:**
- Audit `auth/admin.rs` cookie helper(s) used by PIN issuance path; capture EXACT attribute set (httpOnly, Secure, SameSite, Path, Domain, Max-Age).
- Add §5 sub-item to W1-S5 implementation requiring sliding-window refresh use the IDENTICAL helper or assert attribute equality at compile time.
- Add integration test (in `middleware_tests.rs`) confirming PIN-issuance + sliding-window refresh produce attribute-byte-identical `Set-Cookie` headers.

**Q-S5-MMA-1 (Captain disposition required):** Mandate attribute-equality test + helper-reuse, OR accept risk with explicit POST-merge POS browser verification?

---

### MMA-2 · Concurrent token-refresh race-condition test missing · **5/5 UNANIMOUS** · severity P1

| Model | Citation |
|---|---|
| deepseek-r1 | Gap 3 "Middleware chain breakage" — extensions survive error paths assumption · UA-1 |
| deepseek-v3 | §5 item 7 weak point "Missing test for concurrent token refresh race conditions. Add test case simulating parallel requests with `iat` near expiry threshold" |
| qwen3-coder | Gap 2 "Missing test for concurrent request token refresh under load — concurrent authenticated requests may race to refresh the same token, potentially issuing multiple valid tokens with different `iat`" |
| mimo-v2-pro | Gap 3 "Two simultaneous requests from same staff session could both trigger re-issuance, causing duplicate Set-Cookie headers and potential client-side cookie thrashing" + §5 item 7 amendment "test with 2+ simultaneous requests from same session" |
| gemini-flash | UA-2 "Overwriting `staff_jwt` cookie on every refresh + race conditions for client mid-sequence" |

**Why it matters:** POS terminals issue ≥2 concurrent authenticated requests routinely (e.g. dashboard auto-refresh + staff-action click). Two parallel handlers both seeing `IdleTimeoutStatus::RefreshSoon` will both mint+set new tokens, returning two `Set-Cookie` headers in the same response or in racing responses. Browsers serialize cookie writes, but OBSERVED jar state becomes non-deterministic. Already-shipped W1-S3+S4 refund routing tests (substrate `0386db62`+`59432d4b`) demonstrate concurrent N=20 + N=200 contention patterns are real on this codebase — sliding-window must add an analogous concurrent-refresh test before merge.

**Recommended amendment for RCA-v3:**
- Add §5 item 7 test 9: "concurrent_refresh_idempotency" — N=20 parallel `route_refund` (or any staff-authenticated route) targeting a session at `iat=now-29min`. Assert: only ONE `Set-Cookie` materially differs from inputs in the response set, OR all N responses carry IDENTICAL `Set-Cookie` values (depending on refresh-locking strategy).
- Decide refresh-locking strategy in implementation: per-session lock OR last-writer-wins (with timestamp tiebreak).

**Q-S5-MMA-2 (Captain disposition required):** Per-session lock vs last-writer-wins for concurrent refresh? (Default suggestion: last-writer-wins with timestamp tiebreak — kaizen-min, no new lock infrastructure.)

---

### MMA-3 · JWT secret rotation + sliding-window refresh interaction needs explicit test + audit-trail entry · **4/5 STRONG MAJORITY** · severity P1

| Model | Citation |
|---|---|
| deepseek-r1 | Cross-pilot hazard 3 "During JWT secret rotation, new instances may reject tokens minted by old instances mid-deployment. Requires zero-downtime rotation protocol" |
| deepseek-v3 | UA-3 "JWT secret rotation won't occur during refresh grace window — test rotation during active sliding-window session — may force staff re-auth" |
| qwen3-coder | UA-2 "JWT secret rotation grace interaction is safe under refresh — integration test during active secret rotation window — could cause auth storms or token invalidation if misaligned" |
| mimo-v2-pro | C-5 "JWT secret rotation interaction (Q-S5-4) silently migrates tokens to new secret without audit trail" |

**Why it matters:** Q-S5-4 dispositioned "always CURRENT secret + document choice in code comment", but the RCA's §3 row 5 acknowledged the rotation-grace interaction is OPEN-FORWARD-LOOKING. With sliding-window refresh, every authenticated request during rotation grace silently migrates a customer-staff pair onto the new secret — INVISIBLY. If the new secret is later rolled back (incident response), the in-flight migrated tokens become unverifiable; staff are silently logged out.

**Recommended amendment for RCA-v3:**
- §5 item 4 amendment: Add `tracing::debug!` log when `mint_refreshed_jwt` is called during rotation grace (i.e. when `extract_staff_claims` resolved via `jwt_secret_previous` fallback at lines 84-95).
- Add §5 item 7 test 10: "refresh_during_rotation_uses_current_secret_and_logs" — set up two-secret state, present a token signed with previous secret, expect refresh to mint with current secret AND emit a debug log row tagged `refresh_secret_migration=true`.

**Q-S5-MMA-3 (Captain disposition required):** Add debug-log on secret-migration-during-refresh? (Default suggestion: YES, tagged `refresh_secret_migration=true` for grep-ability during incident response.)

---

### MMA-4 · `IdleTimeoutStatus::RefreshSoon` threshold needs explicit definition · **3/5** · severity P1

| Model | Citation |
|---|---|
| qwen3-coder | §5 item 1 "lacks explicit error mapping for `RefreshSoon` in downstream consumers" |
| mimo-v2-pro | Gap 4 + §5 item 1 amendment "Specify threshold as fraction of `idle_timeout_secs` (e.g., 1/6 = 5 minutes for 30-min timeout)" |
| gemini-flash | C-1 "Explicitly define `RefreshSoon` threshold (e.g., `idle_timeout_secs / 2` or `idle_timeout_secs - idle_refresh_grace_secs`) in the enum or its associated logic" |

**Why it matters:** Q-S5-5 dispositioned "defer config knob; hardcode grace window in `IdleTimeoutStatus::RefreshSoon` threshold". But the RCA does not specify what hardcoded value the implementation should use. Without a named threshold, three implementers will pick three different values; production behavior becomes implementation-dependent.

**Recommended amendment for RCA-v3:**
- §5 item 1 explicit text: `RefreshSoon` fires when `idle_age > (idle_timeout_secs - REFRESH_GRACE_SECS)` where `REFRESH_GRACE_SECS = 300` (5-minute hardcoded grace). Encode as `const REFRESH_GRACE_SECS: u64 = 300;` at top of `auth/middleware.rs` with comment naming Q-S5-5 + Q-S5-MMA-4 disposition.
- Captain may pick a different value; must be named.

**Q-S5-MMA-4 (Captain disposition required):** Hardcoded `REFRESH_GRACE_SECS` value? (Default suggestion: 300s — V2.1 PACT-pin §"Test coverage" reference; aligned with mimo-v2-pro's 1/6-of-30min recommendation.)

---

### MMA-5 · Clock-skew amplification under sliding-window · **3/5** · severity P2

| Model | Citation |
|---|---|
| deepseek-r1 | C-2 + Gap 1 "Distributed clock skew — even 500ms clock drift could cause token rejection during refresh; `middleware.rs:131`'s `SystemTime` is vulnerable" |
| mimo-v2-pro | Gap 2 "Sliding-window refresh on each request may amplify clock skew between servers in multi-instance deployments if `iat` is set to local `now()`" |
| gemini-flash | Cross-pilot hazard 3 "with frequent re-issuance setting `iat = now`, any significant clock skew between the server minting the token and the server validating it could lead to tokens being considered 'future' and thus invalid" |

**Why it matters:** Today's deployment is single-server (Server .23 only on venue path; Bono VPS for cloud). Multi-instance is not on V2.0 timeline per project_v2_master_state.md. BUT: Bono VPS is already a DEPLOY-PARITY target (per CLAUDE.md "DEPLOY PARITY (UNIVERSAL — NO EXCEPTIONS)"). If a staff session is initiated on venue and a subsequent request hits cloud (or vice versa via Tailscale), clock skew >5s between the two clocks would cause spurious idle-expiry. Current `saturating_sub` at `middleware.rs:128-134` mitigates `iat-in-future` but does NOT mitigate `iat-in-past-of-validator`.

**Recommended amendment for RCA-v3:**
- §5 item 7 test 11: "refresh_tolerates_5s_clock_skew" — present a token with `iat = now + 5s` (validator clock 5s slow), expect VALID and refresh proceeds; present `iat = now + 30s` (skew exceeds tolerance), expect 401 with explicit error.
- §5 item 4 amendment: in `mint_refreshed_jwt`, capture `now()` ONCE and pass through; do not call `now()` twice in the refresh path (eliminates same-request internal-skew class).

**Q-S5-MMA-5 (Captain disposition required):** Add 5s-skew tolerance test? (Default suggestion: YES, scope-limited to single test asserting current `saturating_sub` behavior; do not introduce new tolerance config.)

---

### MMA-6 · JWT replay / overlap-window threat surface · **3/5** · severity P1

| Model | Citation |
|---|---|
| deepseek-r1 | Gap 2 "Token replay attacks — overlap window (§5 item 7 test 3) allows old tokens to remain valid; an attacker could replay a captured token during refresh grace period" |
| deepseek-v3 | C-1 "JWT replay attack possible during refresh grace window — add nonce validation in `mint_refreshed_jwt`" + amendment to add `jti` UUID v7 |
| qwen3-coder | Gap 2 "concurrent authenticated requests may race to refresh the same token, potentially issuing multiple valid tokens with different `iat`" |

**Why it matters:** RCA §"Test coverage" V2.1 PACT pin §3 explicitly designs the overlap window — old token remains valid AFTER a new token is minted, until natural expiry of the old token's `exp`. This IS a deliberate UX choice (no in-flight requests fail mid-refresh), but it widens the attacker's replay window. Without `jti` (JWT ID), there is no per-token revocation primitive — both the captured-old and the issued-new are bearer-equivalent.

**Recommended amendment for RCA-v3:**
- DEFAULT POSITION: V2.0 ships overlap-window-without-jti (acceptable per V2.1 PACT pin §3 design). V2.1+ adds `jti` + revocation list as a separate PACT.
- Document threat surface explicitly in code comment at `mint_refreshed_jwt` site: "overlap window accepts replay risk for in-flight UX continuity per V2.1 PACT pin §3; revocation primitive deferred to V2.1+ PACT".
- Add §3 row in past-bug disposition: "JWT replay during overlap window — DESIGN-ACCEPTED for V2.0 per V2.1 PACT pin §3 + Q-S5-MMA-6; defer revocation primitive to V2.1+".

**Q-S5-MMA-6 (Captain disposition required):** Accept overlap-window replay risk for V2.0 with explicit code comment + V2.1+ revocation deferral, OR ship `jti`+revocation in W1-S5 (overscope per kaizen)?  (Default suggestion: ACCEPT for V2.0 with explicit deferral PACT; matches Q-S5-3 kaizen-deferral pattern.)

---

### MMA-7 · Monitoring/observability gap — no metrics for refresh failures or volume · **3/5** · severity P2

| Model | Citation |
|---|---|
| deepseek-r1 | Cross-pilot hazard 2 "No metrics for refresh failures. Silent failures could accumulate until mass logout event. Add Prometheus counter for `jwt_refresh_errors`" |
| mimo-v2-pro | Cross-pilot hazard 2 "No metrics for re-issuance rate, token size, or refresh failures; operational blindness in production" |
| gemini-flash | Cross-pilot hazard 2 "Re-issuing a JWT on every authenticated request could significantly increase the load on the JWT minting logic, especially under high concurrent POS traffic" |

**Why it matters:** Without metrics, the only way to know sliding-window is misbehaving is staff complaints. Mass logout events have happened on this codebase (per CLAUDE.md "Crash Loop Detection" → "MAINTENANCE_MODE silent pod killer" pattern); same anti-pattern applies here.

**Recommended amendment for RCA-v3:**
- §5 item 6 amendment: emit `tracing::info!` (NOT audit_log; that's already dispositioned NO under Q-S5-2) on each refresh, with structured fields `staff_id`, `idle_age_secs`, `refresh_grace_remaining_secs`. Standard log filter excludes by default; ops can grep on incident.
- Add §5 item 11: "metrics emit a counter `staff_jwt_refresh_total` and `staff_jwt_refresh_errors_total` on the existing `/metrics` endpoint (if `prometheus` crate is wired) OR via `tracing` events for downstream metric extraction".

**Q-S5-MMA-7 (Captain disposition required):** Emit refresh-event tracing logs (standard filter excludes; ops grep on incident)? (Default suggestion: YES — refresh-event tracing logs only; defer prometheus counter to a follow-up PACT if `/metrics` not already wired.)

---

### MMA-8 · Middleware composition / response-mutating-layer ordering guarantees · **3/5** · severity P2

| Model | Citation |
|---|---|
| deepseek-r1 | C-1 "Post-handler callback registration may break with future middleware composition — add explicit ordering constraints in middleware.rs comments" |
| qwen3-coder | UA-3 "No middleware ordering conflict with future auth layers — integration test with mock CSRF middleware — may silently break if chained incorrectly post-refactor" |
| gemini-flash | Gap 3 "While the plan states the post-handler layer must not disturb the order, it doesn't explicitly detail HOW this is guaranteed" |

**Why it matters:** Q-S5-6 dispositioned "explicit one-off with named anti-precedent" for response-mutating-middleware-layer (Gap-2 in RCA-v2). The disposition mandates an anti-precedent comment. The MMA panel observes this comment is necessary-but-not-sufficient: a comment can be ignored by future contributors. A CI-time check (test that asserts NO route registers a response-mutating layer except the named one) is a stronger guard.

**Recommended amendment for RCA-v3:**
- §5 item 3 amendment: in `middleware.rs`, name the response-mutating mechanism as a single function `apply_idle_refresh_to_response` (or similar) with the anti-precedent comment block. Add `#[deprecated]` attribute? — probably overkill; rely on the comment.
- §5 item 7 test 12: "no_other_response_mutating_middleware_in_router" — grep test that asserts no other call to `Response::map` mutates response state OUTSIDE of `apply_idle_refresh_to_response`. (This is the route-uniqueness-test pattern from `route_uniqueness_tests::no_duplicate_route_registrations`.)

**Q-S5-MMA-8 (Captain disposition required):** Add CI-grep-test as guard for response-mutating-layer pattern preservation? (Default suggestion: YES; mirrors existing route-uniqueness-test pattern.)

---

## §4 — Minority signals (2/5 or 1/5, flagged but not consensus)

These were raised by <3/5 models and do NOT block H1 PLAN per protocol. Captured for record-keeping. Captain may elect to elevate any to amendment; otherwise they remain documented for future RCAs that touch the same surfaces.

| ID | Concern | Models | Severity-as-stated | Disposition default |
|---|---|---:|---|---|
| MN-1 | `iat` semantics shift breaks V1 clients snapshotting `iat` for non-idle-expiry purposes | 2/5 (mimo, gemini) | P1 | DEFER — grep `iat` usage outside idle-expiry as part of W1-S5 implementation; if any non-idle-expiry use exists, surface as Q-DECISION at H1 PLAN |
| MN-2 | Token size bloat / cookie size limits / CPU load of frequent crypto | 2/5 (mimo, gemini) | P2 | DEFER — measure post-deploy; W1-S5 test 11 (concurrent N=20+) implicitly characterizes load |
| MN-3 | `jti` (JWT ID) regenerate-vs-preserve on refresh | 2/5 (deepseek-v3, gemini) | P2 | RESOLVED VIA MMA-6 default — no `jti` in V2.0; V2.1+ PACT |
| MN-4 | Audit-log sampling for re-issuances (1% sample of debug log) | 2/5 (mimo, qwen3-coder) | P2 | RESOLVED VIA MMA-7 default — tracing log not audit_log; standard filter excludes |
| MN-5 | Anti-precedent comment exact text + placement specifics | 1/5 (mimo) | P2 | ACCEPT VIA MMA-8 default — function-naming + comment block + CI-grep test together close it |
| MN-6 | Rollback playbook for sliding-window not specified | 1/5 (mimo) | P2 | ACCEPT — DEPLOY MANIFEST §3 will name rollback step at W1-S5 ship time per CLAUDE.md DMP rule |

---

## §5 — Verification of RCA's existing strengths (where models did NOT raise concerns)

DIAGNOSE quality is signal-to-noise. Where models said the RCA is solid, recording for posterity:

- §1 boundary map: 0/5 models flagged the file/line citations as wrong. (PATH-TYPO sub-class N=3 evidence pattern in RCA §1 axis-2 self-correction held up under 5-model adversarial review.)
- §2 inherited-issue catalogue: 0/5 models said an item was missing. (PLAN-1 plan-author-typo class addition was unanimously NOT challenged.)
- §3 past-bug disposition: 0/5 models said a row was mis-dispositioned. NIT-2 split (NOT-APPLICABLE-TO-V2 + OPEN-FORWARD-LOOKING) survived review.
- §4 V2-alignment delta: 0/5 models challenged the 4 named gaps.
- §5 estimated size (50 LOC prod + 150-200 LOC tests): 0/5 models said the estimate is wrong. With 12 new tests (8 original + 4 from MMA), final estimate likely ~250-300 LOC tests.
- Captain G33 dispositions Q-S5-1..7: 0/5 models challenged any of the 7 closed Q-DECISIONs. The MMA panel flagged AMENDMENTS to the proposal, not REVERSALS of the dispositions.

---

## §6 — New Q-DECISIONs queued for Captain (Q-S5-MMA-1..8)

| ID | Source | Question | Default-AGREE if Captain doesn't disposition |
|---|---|---|---|
| Q-S5-MMA-1 | MMA-1 (cookie collision 5/5) | Mandate attribute-equality test + `auth/admin.rs` helper-reuse for sliding-window refresh? | YES — add helper-reuse + attribute-equality integration test |
| Q-S5-MMA-2 | MMA-2 (concurrent refresh 5/5) | Per-session lock vs last-writer-wins for concurrent refresh? | last-writer-wins with timestamp tiebreak (kaizen-min) |
| Q-S5-MMA-3 | MMA-3 (secret rotation 4/5) | Add debug-log on secret-migration-during-refresh? | YES — `tracing::debug!` tagged `refresh_secret_migration=true` |
| Q-S5-MMA-4 | MMA-4 (RefreshSoon threshold 3/5) | Hardcoded `REFRESH_GRACE_SECS` value? | 300s (5-minute grace; V2.1 PACT-pin reference; matches mimo 1/6-of-30min) |
| Q-S5-MMA-5 | MMA-5 (clock skew 3/5) | Add 5s-skew tolerance test? | YES — single test asserting current `saturating_sub` behavior; no new tolerance config |
| Q-S5-MMA-6 | MMA-6 (replay window 3/5) | Accept overlap-window replay risk for V2.0 with explicit deferral, OR ship `jti`+revocation now? | ACCEPT for V2.0; defer `jti`+revocation to V2.1+ PACT (kaizen) |
| Q-S5-MMA-7 | MMA-7 (monitoring gap 3/5) | Emit refresh-event tracing logs (standard filter excludes; ops grep on incident)? | YES — tracing logs only; defer prometheus counter to follow-up PACT |
| Q-S5-MMA-8 | MMA-8 (middleware composition 3/5) | Add CI-grep-test as guard for response-mutating-layer pattern preservation? | YES — mirrors existing route-uniqueness-test pattern |

**Captain disposition path:** ACCEPT-ALL-DEFAULTS (mirrors `bda06dc8` G33 batch pattern) is the kaizen-min route — 24h CHALLENGE-AMEND window. EXPLICIT-AMEND on any specific Q-S5-MMA-N also valid. EXPLICIT-DEFER (move into V2.1+ PACT) is valid for any item Captain judges out-of-W1-S5-scope; the W1-S5 ship would then require an explicit deferral commit-comment + V2.1+ PACT-DRAFT in the same wave.

---

## §7 — Updated gate sequence

| Gate | Status | Updated this artifact |
|---|---|---|
| (1) bono AMPLIFIER on RCA-v2 | ✓ COMPLETE | msg=35808 (10:34 IST) absorbed in DRAFT-v2 `cb9ea94f` |
| (2a) Captain Q-RECONCILE-1 | ✓ CLOSED | msg=35809 EXPLICIT-RATIFY = AUTHORIZED (10:45 IST) |
| (2b) Captain G33 batch Q-S5-1..7 | ✓ CLOSED | `bda06dc8` ACCEPT-ALL-DEFAULTS (11:23 IST) |
| **(3) MMA Step 1 DIAGNOSE** | **✓ COMPLETE (this artifact)** | **APPROVE-WITH-AMENDMENTS unanimous; 8 consensus + 6 minority findings; $0.0361 of $10 spent** |
| (3a) Captain Q-S5-MMA-1..8 disposition | ⏳ PENDING | Captain default-AGREE 24h CHALLENGE-AMEND window (silent-AGREE 2026-05-10 ~12:00 IST) |
| (3b) bono AMPLIFIER on this DIAGNOSE | ⏳ PENDING | Optional but recommended; bilateral discipline applies |
| (4) RCA-v3 amendment ship (absorbing dispositioned Q-S5-MMA-N) | ⏳ PENDING | Gated on (3a) disposition |
| (5) W1-S5 H1 PLAN | ⏳ PENDING | Gated on (4) |
| (6) W1-S5 implementation + tests | ⏳ PENDING | Gated on (5) + per-PR Captain merge auth at PR-open |

---

## §8 — Composes-with

- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (doctrine; commit `8768b628`)
- `.planning/specs/v2/W1-S5-RCA.md` (input artifact at branch HEAD `bda06dc8`)
- `.planning/specs/UNIFIED-MMA-PROTOCOL.md` (Step 1 spec, 5-model consensus rules)
- `bda06dc8` (Captain G33 batch disposition + MMA $10 budget approval)
- `LOGBOOK.md` (this run's row)
- `.tmp/mma-w1-s5/responses/*.md` (5 raw model outputs, audit trail)

---

## §9 — NOT TESTED (DIAGNOSE phase, pre-implementation)

This is a Step 1 DIAGNOSE artifact — synthesis of model consensus, not a runtime fix. Items NOT exercised:

- **The proposed code change** — implementation is W1-S5 Session 4 work; this DIAGNOSE is the gate-precursor only
- **Step 4 VERIFY (3-model adversarial)** — runs after EXECUTE, not after Step 1 DIAGNOSE; threshold ≥4.0/5
- **bono substantive AMPLIFIER on this DIAGNOSE** — bilateral doctrine applies; bono review gate is OPTIONAL at MMA Step 1 (per UNIFIED-MMA-PROTOCOL.md the Step 1 consensus stands on its own) but RECOMMENDED for V1-dependent V2 doctrine sibling-discipline
- **Captain G33 disposition of Q-S5-MMA-1..8** — must precede RCA-v3 amendment ship
- **Iteration 2** — protocol min is 2 iterations on disagreement; this run reached consensus on iteration 1 (5/5 unanimous verdict + 8 ≥3/5 findings); iteration 2 would be required only if verdicts had split
- **Per-PR Captain merge auth at W1-S5 PR-open** — gate STANDS independently of MMA outcome

---

— james / 2026-05-09 ~12:00 IST · MMA Step 1 DIAGNOSE on W1-S5 RCA authored under standing autonomy "Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously" 2026-05-09 ~11:45 IST · gates on Captain G33 disposition of Q-S5-MMA-1..8 + (optional) bono AMPLIFIER + RCA-v3 ship before W1-S5 H1 PLAN can be filed · per-PR Captain merge auth gate STANDS at W1-S5 PR-open (foundational auth boundary)
