# MMA Step 2 PLAN — Wave A.2.1 — surgical amendment to Wave A.2 (substrate-flaw closure)

**Scope**: surgical amendment to `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2.md` addressing the substantive substrate flaws surfaced by Step 4 VERIFY adversarial panel (§S-161). NOT a re-author. Closes the substrate-correctness gap that drove panel weighted mean to 3.988 / 4.0 gate.

**Authored**: 2026-05-09 ~20:30 IST · **Authored-by**: james (Claude Opus 4.7 1M)
**Substrate-class**: surgical amendment — closes substrate-flaw subset of Step 4 VERIFY FAIL
**Status**: SHIPPED — Captain Option C hybrid (~20:28 IST verbatim) authorized this amendment without re-VERIFY ($0 path)

**Supersedes**:
- §X of `MMA-STEP-2-W1S5-W1S6-W3-PLAN-A2.md` (Wave A.2, racecontrol `208b6e8e`) — Wave A.2 status flipped to **REFERENCE-ONLY-AUGMENTED-BY-A.2.1** (preserved for design-history-trail; Wave A.2 + this Wave A.2.1 amendment compose as the canonical Step 2 substrate)

**Captain authorization chain**:
- Captain G33 v5 batch shipped verbatim 2026-05-09 ~19:57 IST (item #1 EXPLICIT-FIRE-AUTH for Step 4 VERIFY · item #11 HALT contingency)
- Step 4 VERIFY shipped FAIL `MMA-STEP-4-VERIFY-W1S5-W1S6-W3-CONSENSUS.md` 2026-05-09 ~20:09 IST (racecontrol `ad7082c9` + comms-link `1e80c35e` §S-161; panel mean 3.988 / 4.0 gate; 1 BLOCKING + 2 MAJOR + 3 P0/P1 correctness flaws + 3 novel P1)
- Captain G33 v6 disposition Option C hybrid verbatim 2026-05-09 ~20:28 IST: split G33 v5 #4 + #6 to cascade #7 detail-PLAN level + Wave A.2.1 surgical amendment for substantive flaws + (default) accept surgical patch without re-VERIFY

**V2 doctrine alignment**: §S-146 V1↔V2 RCA rule SECOND end-to-end pipeline application Step 2 supplementary closure · §AMEND-3.II D12 Foundation/Strategy/Config separation · §S-158 V2 Audit-Log Doctrine · F-05 anti-pattern codification (NEW-Q-1 RATIFIED).

---

## §1 — Scope of this amendment

The Step 4 VERIFY panel surfaced two classes of finding:

1. **Temporal-gap items** (G33 v5 #4 + #6 — Q-W1-S5-NEW-1 12h cap + Q-W1-S6-NEW-2 retry queue) — Wave A.2 PLAN at 12:30 IST predates G33 v5 ratification at ~19:57 IST. **Captain Option C disposition: belong at cascade #7 detail-PLAN level, NOT META-PLAN level.** This amendment does NOT incorporate them — they live at `W1-S5-PLAN.md` + `W1-S6-PLAN.md`.

2. **Substantive substrate flaws** (NON-temporal — the real META-PLAN gaps) — this amendment incorporates.

**In scope (this §A.2.1):**
- F-CONS-17 multi-host clock skew tolerance
- F-CONS-15 lockout-bypass TOCTOU atomic-fix
- F-CONS-16 single-flight mutex distributed-coord disposition
- F-CONS-5 HashMap pruning (V1 inheritance)
- F-CONS-2 F-05 regression test scope extension to W1-S5 refresh path
- Novel P1 refresh-mutex deadlock under PIN-LOCKOUT revocation — proof obligation
- F-AMEND-CONS-7 cashier-default unit test (manager/superadmin role preservation)
- F-AMEND-SING-5 observability `refresh_lock_wait_time` histogram

**Out of scope (this §A.2.1; deferred to cascade #7 detail PLANs):**
- Q-W1-S5-NEW-1 12h cap implementation → `W1-S5-PLAN.md`
- Q-W1-S6-NEW-2 email retry queue → `W1-S6-PLAN.md`
- Per-PR file:line + function-signature specs → cascade #7 detail PLANs
- PR-A code authoring → cascade #8 (PR-open auth required at commit time per G33 v5 #9)

---

## §2 — F-CONS-17 multi-host clock skew tolerance (P1, NON-temporal)

**Source-of-truth:** `MMA-STEP-1-W1S5-W1S6-W3-CONSENSUS.md` F-CONS-17 — multi-host clock skew under sliding-window auth refresh. Step 4 VERIFY [deepseek-r1] flagged as MAJOR divergence: PLAN omitted skew-tolerance enforcement.

**Amendment to PLAN (Wave A.2 §6 W1-S5 PR-C scope addendum):**

PR-C-W1-S5-auth-refresh MUST enforce a clock-skew tolerance window when validating sliding-window refresh tokens across multi-host deployments:

- Spec: token validation accepts `iat` / `exp` claims within ±5s of server clock (configurable: `auth.clock_skew_tolerance_secs` env-var, default `5`, hard-capped `30`)
- Server-side: NTP-sync invariant — `racecontrol` server `.23` runs Windows Time Service against canonical pool; pod fleet syncs against `.23` (existing infra; no new component)
- Validation: `chrono::Utc::now()` vs claim `iat` / `exp` with `chrono::Duration::seconds(skew_tolerance)` tolerance
- Test coverage (cascade #7 W1-S5-PLAN.md detail): `clock_skew_within_tolerance` + `clock_skew_beyond_tolerance` + `clock_skew_negative_drift` + `clock_skew_at_boundary`
- Anti-precedent comment at validation site: `// F-CONS-17: clock skew tolerance — DO NOT remove without re-evaluating multi-host deployment topology`

**V2 doctrine alignment:** §AMEND-3.II D12 Foundation/Strategy/Config separation — clock-skew is config (env-var), not strategy (validation pure-function), not foundation (auth gateway).

---

## §3 — F-CONS-15 lockout-bypass TOCTOU atomic-fix (P0, NON-temporal)

**Source-of-truth:** Step 1 amended CONSENSUS F-CONS-15 (4/4 strong-consensus); Step 4 VERIFY [deepseek-r1] flagged as P0 correctness flaw — PLAN's lockout-bypass fix has TOCTOU between predicate-check and JWT revocation.

**Amendment to PLAN (Wave A.2 §F-CONS-15 PR-A scope, cross-coupling W1-S6 → W1-S5):**

The lockout-state predicate check + JWT revocation MUST be atomic. Wave A.2 specified publisher/consumer but did NOT close the TOCTOU window between consumer-read and revocation-write.

**Spec:**
- `LockoutCheckGuard` type — RAII guard around the (read-predicate, revoke-token) pair
- Implementation: single-flight mutex (per-staff-id keyed) acquired BEFORE predicate read; held THROUGH revocation; released AFTER revocation completes (success OR error)
- The mutex protects the WINDOW between predicate-read and revocation-write — NOT the per-call refresh path (that's F-CONS-16's separate single-flight mutex)
- Two distinct mutex layers required: (a) per-staff-id refresh single-flight mutex (F-CONS-16); (b) per-staff-id lockout-check-and-revoke single-flight mutex (this F-CONS-15 fix). Lock-acquisition order: (b) THEN (a) — never inverse
- Code-comment at acquisition site: `// F-CONS-15: predicate-check + revoke must be atomic; lock order is lockout-check (b) THEN refresh (a); inversion = deadlock`
- Test coverage (cascade #7 W1-S5-PLAN.md detail): `lockout_toctou_concurrent_check_and_revoke` + `lockout_lock_order_verified` + `lockout_lock_release_on_revoke_error`

**Composes-with:** §4 F-CONS-16 distributed-coord disposition (mutex layer (a)); §7 novel deadlock proof.

---

## §4 — F-CONS-16 single-flight mutex distributed-coord disposition (P1, NON-temporal)

**Source-of-truth:** Step 1 amended CONSENSUS F-CONS-16 (4/4 strong-consensus); Step 4 VERIFY [deepseek-r1] flagged as P1: mutex lacks distributed-lock idempotency keys; breaks under multi-instance deployments.

**Amendment to PLAN (Wave A.2 §F-CONS-16 PR-C scope addendum):**

V2.0 production topology: **single-host racecontrol** (Server `.23`) + **single cloud failover** (Bono VPS, only-on-cutover not multi-active). NO multi-active racecontrol deployment in V2.0 scope.

**Disposition: explicit single-node scoping** (not distributed-lock keys).

**Spec:**
- Single-flight mutex implemented via `tokio::sync::Mutex<HashMap<StaffId, Arc<tokio::sync::Mutex<()>>>>` — process-local, pid-scoped
- Code-comment at mutex declaration: `// F-CONS-16: single-flight scoped to single-host racecontrol (V2.0 topology = .23 active OR Bono VPS failover, never both); distributed-coord NOT required. Re-evaluate at multi-active scope expansion.`
- Anti-precedent comment: `// F-CONS-16 ANTI-PRECEDENT: do not introduce multi-active racecontrol topology without distributed-lock keys for this mutex`
- Forward-compatibility hook: `auth.refresh_mutex_distributed_keys` env-var (default `false`); when `true`, `RefreshMutex` must use `redis::Cluster` distributed locks (NOT implemented in V2.0; placeholder for V2.1+)
- Test coverage (cascade #7 W1-S5-PLAN.md detail): single-flight tests as specified in Wave A.2 §6; no distributed-lock tests required for V2.0

**Forward-trigger:** if V2.1+ scope adds multi-active racecontrol, re-fire MMA Step 1 DIAGNOSE on multi-active auth boundary BEFORE flipping `distributed_keys` to `true`.

**Composes-with:** §3 F-CONS-15 atomic TOCTOU fix (lock order); §7 novel deadlock proof.

---

## §5 — F-CONS-5 HashMap pruning (V1 inheritance, PARTIAL)

**Source-of-truth:** Step 1 canonical CONSENSUS F-CONS-5 — V1-era refresh-token tracking HashMap accumulates; not pruned on revocation/expiry. Step 4 VERIFY [deepseek-r1] flagged as PARTIAL coverage: PLAN's F-CONS-5 disposition relies on F-CONS-7 bypass without HashMap pruning, leaving V1 resource leak.

**Amendment to PLAN (Wave A.2 §F-CONS-5 PR-C scope addendum):**

W1-S5 refresh path inherits V1 `RefreshTokenMap: HashMap<StaffId, Vec<RefreshTokenEntry>>` storage. V2 contract: bounded growth.

**Spec:**
- On revocation (F-CONS-15 path): `RefreshTokenMap.entry(staff_id).and_modify(|v| v.retain(|e| e.token_id != revoked_id))`; if entry empty post-retain, `RefreshTokenMap.remove(&staff_id)`
- On expiry (TTL): background task `auth_refresh_token_pruner` runs every 60s; iterates `RefreshTokenMap`; removes entries where `now > exp + clock_skew_tolerance`; removes empty staff-id entries
- Bounded growth metric: `auth_refresh_token_map_size` gauge metric (Prometheus)
- Anti-precedent comment: `// F-CONS-5 V1-inheritance: bounded growth required; pruning on revoke + TTL background task; do not skip either path`
- Test coverage (cascade #7 W1-S5-PLAN.md detail): `pruning_on_revoke_removes_token` + `pruning_on_revoke_removes_empty_staff_entry` + `pruning_background_task_removes_expired` + `bounded_growth_under_load_simulation`

**Composes-with:** §3 F-CONS-15 atomic TOCTOU fix (revocation path triggers pruning).

---

## §6 — F-CONS-2 F-05 regression test scope extension (P1, NON-temporal)

**Source-of-truth:** Step 4 VERIFY [deepseek-r1 + grok-code 2 panels] flagged as P1: PLAN's `f05_anti_pattern_regression_check` test only covers capture path (W3 wallet HRC), not W1-S5 refresh path. F-05 anti-pattern (UPDATE-then-SELECT same column in same scope) applies to ANY scope with read-after-write semantics.

**Amendment to PLAN (Wave A.2 §F-CONS-2 + NEW-Q-1 scope addendum):**

`f05_anti_pattern_regression_check` test MUST cover ALL three RCA-touched paths:

**Spec:**
- Test (a) capture path (W3 wallet HRC) — already in Wave A.2 scope
- Test (b) **NEW** W1-S5 refresh path — UPDATE `refresh_tokens.last_used_at`; assert SELECT immediately after returns the original `last_used_at` value if a snapshot-read-before-write was taken (F-05 violation = SELECT returns the just-written value)
- Test (c) **NEW** W1-S6 PIN-LOCKOUT path — UPDATE `staff.lockout_count`; same assertion shape
- Test fixture: in-memory SQLite DB with PRAGMA matching production
- Code-comment at each test site: `// F-CONS-2 F-05 anti-pattern regression check; scope = <path-name>; do NOT remove without `

**NEW-Q-1 sub-PACT codification (Captain G33 v5 #7 RATIFIED):**
- PACT-DRAFT `e4c64ce2` (§S-156) F-05 anti-pattern lint: standing-rule status on land
- Lint check (clippy custom lint OR `cargo-careful` integration; deferred to V2.1 if not feasible at PR-A time): flags `UPDATE x SET col = ?; SELECT col FROM x WHERE ...` patterns in same function scope
- Pre-commit hook: greps `// F-05 OK:` comment-tag at any UPDATE-then-SELECT site (explicit acknowledgment required)

**Composes-with:** Wave A.2 §F-CONS-2 + NEW-Q-1 + W3-WALLET-HRC-RCA.md §5 F-05 anti-pattern guard.

---

## §7 — Novel P1 refresh-mutex deadlock proof (deepseek-r1)

**Source-of-truth:** Step 4 VERIFY [deepseek-r1] novel P1: "W1-S5 refresh mutex deadlock risk under PIN-LOCKOUT revocation — lockout revocation may attempt to re-enter mutex during active refresh, causing deadlock. Unaddressed in concurrency design."

**Amendment to PLAN (Wave A.2 §6 W1-S5 PR-C scope addendum):**

Two single-flight mutex layers exist (per §3 F-CONS-15 lock order):
- (a) per-staff-id refresh mutex (F-CONS-16)
- (b) per-staff-id lockout-check-and-revoke mutex (F-CONS-15)

Deadlock risk: if a refresh path holding (a) calls into lockout-revocation which acquires (b), the (b) acquisition site MUST NOT block on (a) — i.e., (a) and (b) must not be circularly dependent.

**Spec:**
- Lock order: ALWAYS (b) THEN (a). Never (a) THEN (b).
- The refresh path enters (a) AFTER lockout-check-and-revoke completes — i.e., (b) is acquired and released BEFORE (a) is acquired
- Refresh-path control flow:
  ```
  1. ACQUIRE (b) per-staff lockout-check-and-revoke mutex
  2. CHECK lockout predicate
  3. IF locked-out: revoke active tokens; RELEASE (b); RETURN 401
  4. RELEASE (b)
  5. ACQUIRE (a) per-staff refresh single-flight mutex
  6. PERFORM refresh
  7. RELEASE (a)
  ```
- Lockout-revocation initiated from W1-S6 PIN-LOCKOUT path:
  - Acquires (b) only; never (a)
  - Cannot deadlock with refresh because refresh holds (b) ONLY in steps 1-4 (no inner (a) acquisition)
- Static-analysis: `clippy` lock-order check OR manual code-review checklist for any code path that acquires both mutexes (NEW Standing Rule candidate post-V2.0-RATIFY)
- Code-comment at refresh-path entry: `// F-CONS-15+novel-deadlock proof: lock order = (b) lockout-check THEN (a) refresh; (b) released before (a) acquired; lockout-revocation from W1-S6 acquires (b) only`
- Test coverage (cascade #7 W1-S5-PLAN.md detail): `concurrent_refresh_and_lockout_no_deadlock` (load-simulation: 100 concurrent refreshes + 50 concurrent lockout-revocations across 10 staff_ids; no test exceeds 5s; no deadlock detection)

**Composes-with:** §3 F-CONS-15 + §4 F-CONS-16; cascade #7 W1-S6-PLAN.md (lockout-revocation publisher) + W1-S5-PLAN.md (refresh consumer).

---

## §8 — F-AMEND-CONS-7 cashier-default unit test (Wave A.2 already specified; reaffirmed)

**Source-of-truth:** Step 1 amended CONSENSUS F-AMEND-CONS-7; Wave A.2 §5 item 4 already specifies. Step 4 VERIFY [deepseek-r1] flagged as P1 correctness flaw — confirmed Wave A.2 ALREADY incorporates; no amendment needed; reaffirmed for cascade #7 inheritance.

**Spec (reaffirmation):**
- Unit test `cashier_default_role_preservation_on_refresh` covers: cashier role preserved on refresh; manager role preserved on refresh; superadmin role preserved on refresh; downgrade detected and rejected
- `create_staff_jwt_with_role(claims.role)` — role pulled from EXISTING claim, not hardcoded default

---

## §9 — F-AMEND-SING-5 observability addition (Wave A.2 already specified; reaffirmed)

`refresh_lock_wait_time` histogram metric — already in Wave A.2 §5 item 13; reaffirmed.

**Additional metrics this amendment introduces:**
- `auth_refresh_token_map_size` gauge (F-CONS-5 bounded-growth metric)
- `auth_clock_skew_rejections_total` counter (F-CONS-17 — rejected tokens beyond skew tolerance)
- `auth_lockout_check_revoke_duration` histogram (F-CONS-15 — atomic check+revoke critical-section latency)

---

## §10 — Q-DECISION compliance map (G33 v5 ratifications)

| Ratification | Wave A.2 status | Wave A.2.1 status | Cascade #7 detail-PLAN status |
|--------------|----------------|--------------------|-------------------------------|
| Q-W1-CROSS-1 (security-class explicit) | INCORPORATED | reaffirmed | implementation in W1-S5-PLAN.md + W1-S6-PLAN.md (cross-PR coupling) |
| Q-W1-CROSS-2 (default-a sequencing) | INCORPORATED | reaffirmed | PR-A opens W1-S6 FIRST per W1-S6-PLAN.md |
| Q-W1-S5-NEW-1 (12h cap option-b) | TEMPORAL-GAP | **deferred to cascade #7** per Captain Option C | implementation in W1-S5-PLAN.md |
| Q-S5-NEW-2 (force-expire JWT default) | INCORPORATED | reaffirmed | implementation in W1-S5-PLAN.md |
| Q-W1-S6-NEW-2 (retry queue default-Y caveats) | TEMPORAL-GAP | **deferred to cascade #7** per Captain Option C | implementation in W1-S6-PLAN.md |
| NEW-Q-1 (F-05 lint codification) | INCORPORATED | extended scope per §6 | `f05_anti_pattern_regression_check` in PR-A test coverage |
| NEW-Q-2 (V2 Audit-Log Doctrine §S-158) | INCORPORATED | reaffirmed | `audit_log.action_type` CHECK constraint discipline applied to all V1↔V2 boundary code |

**Captain Option C doctrine clarification (this §10 codifies):**

G33 v5 #4 (Q-W1-S5-NEW-1 12h cap) + #6 (Q-W1-S6-NEW-2 retry queue) ratifications BELONG at cascade #7 detail-PLAN level (W1-S5-PLAN.md + W1-S6-PLAN.md), NOT at Wave A.2 META-PLAN level. The META-PLAN scope is PR breakdown + cross-RCA architecture; detail-PLAN scope is per-Q-DECISION implementation specs. This is doctrine clarification by Captain Option C verbatim disposition, NOT scope inversion.

**Future precedent (kaizen forward):** when Captain G33 ratifications postdate a META-PLAN, default-disposition is detail-PLAN incorporation (NOT META-PLAN amendment), unless the ratification changes META-architecture (PR breakdown / sequencing / cross-RCA topology) in which case META-PLAN amendment IS required. This Wave A.2.1 amendment is the empirical anchor for that precedent.

---

## §11 — Cascade transition

| Cascade item | Pre-this-A.2.1 | Post-this-A.2.1 |
|--------------|----------------|-----------------|
| #6.5 Step 4 VERIFY adversarial panel | SHIPPED-FAIL @ §S-161 | unchanged (single fire) |
| #7 W1-S5-PLAN.md + W1-S6-PLAN.md + W3-PLAN.md authoring | HALT awaiting G33 v6 | **NOW UNBLOCKED — proceed under Wave A.2 + this Wave A.2.1 substrate** |
| #8 PR-A opens W1-S6 FIRST | HALT (downstream) | gates on #7; PR-open auth required at commit time per G33 v5 #9 |
| Captain Option C ratification | n/a | RATIFIED 2026-05-09 ~20:28 IST verbatim |

---

## §12 — Substrate scope reaffirmation

This Wave A.2.1 amendment is **scope-bounded** to the substantive substrate-flaw closure list in §1. It does NOT:
- Re-author Wave A.2 (Wave A.2 substrate stands; this amendment composes WITH it)
- Re-fire Step 4 VERIFY (Captain Option C default = accept surgical patch without re-VERIFY)
- Open PR-A (cascade #8 gate; PR-open auth required)
- Resolve PACT-024 Q1-Q5 wallet-concurrency-idempotency disposition (separate bono-LEAD path; deferred per Drift-Pilot first-mover doctrine)

---

## §13 — NOT TESTED

- Captain G33 v6 explicit re-VERIFY override (Captain said "Option C hybrid" verbatim WITHOUT explicit re-VERIFY; default = accept without re-VERIFY; if Captain later requests re-VERIFY, ~$0.10 OpenRouter spend gates on next G33 EXPLICIT-FIRE-AUTH)
- bono AMPLIFIER absorption of Wave A.2.1 (deferred to next bilateral cycle per W1-S5 RCA precedent; bono picks up via comms-link git_pull + V2-MASTER-STATE §S-162 ledger which will be authored in next-step substrate-ship)
- F-05 lint feasibility study (custom clippy lint OR `cargo-careful` integration) — deferred to V2.1 if not feasible at PR-A time; pre-commit grep-based hook covers V2.0 baseline
- Distributed-lock forward-compatibility hook activation — `auth.refresh_mutex_distributed_keys` env-var stays `false` in V2.0; activation triggers MMA Step 1 DIAGNOSE on multi-active topology (not in V2.0 scope)
- Multi-host racecontrol topology (V2.1+ scope) — F-CONS-16 disposition is V2.0-bounded
- Deadlock proof empirical anchor — load-simulation test specified in §7 but NOT yet authored or run; gates on cascade #7 W1-S5-PLAN.md test specification
- Whether nemotron-3-super-120b-a12b would have detected these substrate flaws with format-honoring output (§S-161 hypothetical; 4/4 panel quorum already corroborated; nemotron re-fire OUT OF SCOPE)

---

— james / 2026-05-09 ~20:30 IST · Wave A.2.1 surgical amendment · Captain Option C hybrid (~20:28 IST) verbatim authorization · 8 substantive substrate-flaw closures (§2–§9) + Q-DECISION compliance map (§10) + doctrine clarification (G33 v5 #4+#6 belong at cascade #7 detail-PLAN level, NOT META-PLAN) + cascade #7 NOW UNBLOCKED · $0 spend (synthesis-only) · cumulative MMA-day james-side $0.505/$5+$10 unchanged · 0 G9 self-caught this turn
