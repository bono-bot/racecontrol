---
artifact: §S-146 V1↔V2 RCA · Phase γ scope (extends cluster RCA bad164b6)
boundary: wallet-ledger transaction-boundary + idempotency-persistence + audit-log atomicity
status: AUTHORED-AWAITING-MMA-STEP-1-AND-CAPTAIN-DECISION-QUEUE
authored: 2026-05-14 IST (~03:00 IST)
author: bono
boundary-class: foundational-CLUSTER (wallet + billing + accounting simultaneous)
parent-cluster-rca: ./RCA-2026-05-13-wallet-ledger-discount-cluster.md (`bad164b6`)
parent-cluster-rca-§S-anchor: §S-263 CLOSE-ANCHOR (comms-link `c89bc0f0`-vicinity)
phase: γ (post-Phase-β-merge; D-CLUSTER-3 PR #72 + D-CLUSTER-8 PR #73 + D-CLUSTER-9 PR #74 all LANDED 2026-05-13)
items-in-scope: I-13 concurrency race · I-15 idempotency-key persistence · I-17 audit-log atomicity 2PC
items-deferred-from-phase-beta: per parent RCA §3 dispositions (I-9/I-13/I-15/I-17 = deferred to Phase γ; I-9 = duplicate-naming with I-17 in cluster RCA pointer file)
captain-decisions-needed: MMA-budget Phase γ · PR-cardinality (single-bundle vs 2-PR vs 3-PR) · per-PR merge auth · ordering (small-first vs big-first)
mma-step-1: PENDING-Captain-budget-auth (~$0.10-0.50 OpenRouter; covered by Captain $45 release 2026-05-13 ~20:18 IST IF Q3-cleared)
companion-rca: parent cluster RCA above (covers atoms 1-5; this RCA covers atoms 6-8)
stale-at: 2026-06-14
---

# §S-146 V1↔V2 RCA — Phase γ Wallet Substrate (I-13 · I-15 · I-17)

This RCA extends the parent cluster RCA (`bad164b6`) to the three Phase γ deferred items. **All three items inherit the parent's boundary-class (foundational-CLUSTER per §S-146)** and follow the same gate discipline: 5-section RCA → MMA Step 1 → substrate → MAOR → per-PR Captain auth.

The parent RCA used "I-9" for audit-log atomicity (RCA §2 line 60); the wallet-files pointer RCA used "I-17" for the same issue (introduced as forward-amendment via MMA Step 1 results). This RCA adopts the wallet-files naming (I-13/I-15/I-17) for forward-compat with §S-263 + handoff vocabulary; both names refer to the same item.

---

## 1. Boundary map

### I-13 — Wallet credit + session start RMW race

**Surface:** the boundary between `wallet::credit_in_tx`/`credit` (admin-triggered) and `billing_start.rs::start_billing` (customer-triggered session creation). Each individual function is atomic per-call via `sqlx::Transaction`, but the read-modify-write boundary spans the two.

| File:line | Function | Atomicity scope | Race vector |
|---|---|---|---|
| `crates/racecontrol/src/wallet.rs:114-193` | `credit_in_tx` | UPDATE wallets + SELECT new_balance + INSERT wallet_transactions — all inside `tx` | none internal; commits cleanly |
| `crates/racecontrol/src/wallet.rs:198-265` | `credit` | begins its own tx, calls credit_in_tx, commits | none internal |
| `crates/racecontrol/src/api/billing_start.rs:~250-285` | `start_billing` tx begin | begins tx AFTER reading wallet balance + computing discount | **TOCTOU window** — wallet balance + discount computation happen OUTSIDE the tx |
| `crates/racecontrol/src/api/billing_start.rs:296-311` | `debit_in_tx` call | inside tx; uses pre-computed `final_price_paise` based on stale balance read | **race surface** — stale-data debit |
| `crates/racecontrol/src/api/billing_start.rs:316+` | INSERT billing session | inside tx | composes with stale debit |

**Concrete race sequence:**

1. T0: `start_billing` reads wallet balance B0 = 1000 (outside tx)
2. T0: `start_billing` computes `final_price_paise = max(0, original_price - applied_discount)` based on B0
3. T1: Concurrent `credit_in_tx` (admin top-up) commits +500 → balance = 1500
4. T2: `start_billing` begins tx, calls `debit_in_tx` with stale `final_price_paise`
5. Result: debit succeeds (balance ≥ final_price_paise; trivially true since B0 < B1500), but the discount/charge computation was based on stale balance state — if the discount formula depends on balance (e.g., "100% discount for first-time top-up bonus accrual" type rules in future Phase 2-A/F substrate), the outcome is incorrect

**Today's mitigating factor (per parent RCA §3 I-9 disposition):** `MAX_DISCOUNT_PCT_DEFAULT = 0.50` is **const-in-code** at `pricing/discount_ceiling.rs:25` and `max_discount_pct(&state)` returns the const directly. No per-session config drift can affect the ceiling. The race is therefore **bounded** by the const — concurrent credit cannot weaken the ceiling. But the race **remains** for any future Phase-2-A rate-table-driven discount or Phase-2-F campaign-driven dynamic pricing that reads balance during computation.

### I-15 — Idempotency-key persistence gap (Atom 2 retry path)

**Surface:** `crates/racecontrol/src/api/wallet_gateway.rs:227-272` — the Path B `payment_gateway_webhook` idempotency check.

| File:line | Operation | Atomicity | Race vector |
|---|---|---|---|
| `wallet_gateway.rs:227-235` | SELECT amount_paise FROM wallet_transactions WHERE idempotency_key = ? | NOT inside any tx; pre-flight check | **TOCTOU window** — two concurrent webhook deliveries can both pass the SELECT |
| `wallet_gateway.rs:250` | `state.db.begin()` — begin credit tx | tx scope starts here | composes with race |
| `wallet_gateway.rs:261-272` | `wallet::credit_in_tx(...)` with `idempotency_key = Some(&req.transaction_id)` | inside tx; INSERTs wallet_transactions row with idempotency_key column | duplicate inserts possible |

**Concrete double-credit sequence:**

1. T0: Webhook delivery #1 arrives with `idempotency_key = "txn_abc"`; SELECT returns None (no prior row)
2. T0+ε: Webhook delivery #2 (retry / network duplicate) arrives with same `idempotency_key = "txn_abc"`; SELECT also returns None (delivery #1 hasn't committed yet)
3. T1: Both proceed to `credit_in_tx`; both UPDATE wallets balance + INSERT wallet_transactions row
4. T2: Both commit
5. Result: balance updated 2x; either two wallet_transactions rows (if no UNIQUE INDEX on idempotency_key) → **double credit**, OR one INSERT fails with constraint error (if UNIQUE INDEX exists) but the UPDATE wallets balance was applied twice in the failing tx — **inconsistent state if rollback doesn't fully undo**

**Verification needed:** Is there a UNIQUE INDEX on `wallet_transactions.idempotency_key`? Parent cluster RCA D-CLUSTER-1 ratified V1 ALTER + drop narrow CHECK; need to verify whether that ALTER added UNIQUE on idempotency_key. Atom 7 spec must include "verify or add UNIQUE INDEX wallet_transactions_idempotency_key_unique" as a sub-step.

### I-17 — Audit-log atomicity (no 2PC for clamp + wallet_transactions write)

**Surface:** `billing_discount.rs:181-220` discount UPDATE path + `wallet.rs::credit_in_tx` + `accounting::log_admin_action`.

| File:line | Operation | Atomicity scope | Failure-mode |
|---|---|---|---|
| `billing_discount.rs:148-170` (Atom 1 §S-253/254) | `clamp_discount_paise()` pure-function | none (no I/O) | n/a |
| `billing_discount.rs:181-190` | UPDATE billing_sessions SET discount_paise = ? | committed to its own tx (composes with Phase β atoms 2-5 dispositions per parent §S-263.3) | inside-tx commit |
| `billing_start.rs:Atom-5-call-site` + `billing_discount.rs:Atom-5-call-site` | `accounting::log_admin_action(action_type="discount_clamped", ...)` | **fire-and-forget per signature** (parent §S-263.3) | best-effort durability; if tokio task panics or network partition → audit_log row missing while wallet_transactions row exists |

**Concrete failure-mode:**

1. T0: clamp computes `Clamped { original=0.80, clamped=0.50, cap_source="MAX_DISCOUNT_PCT" }`
2. T1: UPDATE billing_sessions commits with `discount_paise = clamped_amount`
3. T1+ε: `accounting::log_admin_action(action_type="discount_clamped", original=0.80, clamped=0.50)` fired in spawned tokio task
4. T2: Tokio runtime shutdown / process kill / network partition / DB pool exhaustion → audit log INSERT never completes
5. Result: wallet_transactions reflects the clamped discount (correct economic outcome) but audit_log has no entry recording the clamp event → forensic gap; auditors cannot reconstruct what was clamped and when, breaking compliance attestation for any future Captain Post-V2.0-Pricing-Calibration retrospective

**Atomicity gap rigor:** the gap is "best-effort durability under failure"; happy-path operation (no panic, no partition) is correct. The 2PC pattern (or outbox / saga) closes the gap by making the audit-log write part of the same atomic commit as the wallet write.

---

## 2. Inherited-issue catalogue (extension)

All three items are forward-amendments from MMA Step 1 DIAGNOSE on parent cluster RCA (2026-05-13 ~18:30 IST, results at `/tmp/mma-cluster-results/`). Per wallet-files RCA pointer line 35: "5 additional class candidates (I-13..I-17 forward-amendment if Captain ratifies); I-14/I-15/I-16 mitigations incorporated directly into Atom 2 + Atom 3 implementations this Phase β. I-13 + I-17 documented as known gaps with mitigation strategy in Phase γ."

The handoff vocabulary "I-13/I-17" is shorthand for the three Phase γ items (I-13 + I-15 + I-17). I-14 (HMAC seal scope incomplete header coverage) was closed via Atom 2 substrate landing. I-16 (Path E cap formula cost<=0 edge) was closed via Atom 3 substrate landing.

| # | V1 / V2 failure class | MMA Step 1 vote | Phase β disposition | Phase γ disposition |
|---|---|---|---|---|
| **I-13** | Concurrency race (wallet credit + session start RMW) | 3/3 BLOCKING (DeepSeek R1 + Qwen3 Coder + Kimi K2.5) | DEFERRED — Phase γ wider transaction-boundary refactor required; `MAX_DISCOUNT_PCT` const-in-code provides bounded mitigation for ceiling specifically | **THIS RCA Atom 6** — close the RMW via tx-boundary refactor |
| **I-15** | Idempotency-key persistence gap on Atom 2 retry path | 2/3 BLOCKING (DeepSeek R1 + Qwen3 Coder) | PARTIALLY-MITIGATED — Atom 2 added in-memory SELECT pre-check at wallet_gateway.rs:227-235; TOCTOU race remains | **THIS RCA Atom 7** — close TOCTOU via INSERT-then-handle-conflict or SELECT-INSIDE-tx + UNIQUE INDEX |
| **I-17** | Audit-log stamp atomicity (no 2PC for clamp + wallet_transactions) | 2/3 PRE-MERGE (Qwen3 Coder + Kimi K2.5) | DEFERRED — `accounting::log_admin_action` fire-and-forget per signature; best-effort durability; full 2PC out of Phase 1 | **THIS RCA Atom 8** — close 2PC gap via outbox pattern OR same-tx audit write |

---

## 3. Past-bug review

| # | Item | Disposition | Cite |
|---|---|---|---|
| **I-13 RMW race** | **UNRESOLVED-FROM-PHASE-β** — explicitly deferred per parent RCA §3 line 77 disposition + §S-263.3 + §S-264.4 C2 AMPLIFIER caveat. NOW pre-MERGE-AUTH BLOCKER for Phase γ atoms 6-8 PR. **Phase γ Atom 6** = close the RMW boundary. | parent cluster RCA `bad164b6` §3 I-9 + §S-263.3 |
| **I-15 idempotency persistence** | **PARTIALLY-PATCHED-AT-Φβ** — Atom 2 landed in-memory pre-check (good for happy-path; race remains under contention). NOW pre-MERGE-AUTH BLOCKER if Phase γ MMA Step 1 reaffirms 2/3 BLOCKING severity. **Phase γ Atom 7** = harden via INSERT-then-handle-conflict path OR composite-key idempotency table. | wallet_gateway.rs:227-272 |
| **I-17 audit-log atomicity** | **UNRESOLVED-FROM-PHASE-β** — explicitly deferred per parent RCA §3 line 79 + Atom 5 acceptance of "fire-and-forget" as best-effort. NOW pre-MERGE-AUTH guidance for Phase γ; Captain decides whether 2PC is pre-merge-required or follow-up (depends on Captain's compliance-attestation requirements). **Phase γ Atom 8** = close 2PC via outbox OR same-tx audit insertion. | parent cluster RCA `bad164b6` §3 I-9 (audit-log naming conflict; see header) + §S-263.3 |

**Cross-reference to V1 failure classes** (per `feedback_v1_process_mess_audit_for_v2_blockers.md` categories A-J): I-13 = Category C (concurrency-class). I-15 = Category D (idempotency-class). I-17 = Category E (audit-trail-durability class). All three are V1-inherited; none introduced by V2 substrate work.

---

## 4. V2-alignment delta

V2 doctrine for Phase γ:

> All wallet-state mutations that are observed by customer-facing or auditor-facing surfaces MUST be (a) atomic under concurrency (no read-stale-then-write), (b) idempotent under retry (no double-credit / double-debit), (c) durable under partial-failure (audit-trail row guaranteed to exist iff economic-outcome row exists).

These three invariants close I-13 (atomicity), I-15 (idempotency), I-17 (durability).

**Phase γ atom plan:**

### Atom 6 — Wallet RMW transaction-boundary refactor (closes I-13)

**Goal:** Move the balance read INTO the same transaction as the debit + session insert at `billing_start.rs::start_billing`. SQLite-specific: use `BEGIN IMMEDIATE` to acquire reserved lock before reads; this prevents concurrent credit_in_tx from interleaving.

**Scope estimate:** ~150-300 LOC including:
- Refactor `billing_start.rs::start_billing` to begin tx EARLIER (before balance read) with `BEGIN IMMEDIATE`
- Move balance-read + discount-computation INSIDE tx
- Compose with existing `debit_in_tx` + `billing_session INSERT`
- Add concurrency tests (spawn N=8 parallel credit + start_billing tasks, assert serializability)
- Verify no `.await` between balance read and debit (per racecontrol/CLAUDE.md "Never hold a lock across .await")

**Risk:** moving operations inside tx increases tx duration → lock contention under load. Mitigation: keep tx short by extracting all non-DB computation outside.

**Composes-with:** §S-264.4 C2 AMPLIFIER framing (Atom 3 credit_in_tx consolidation reduces-but-doesn't-close the race; Atom 6 closes it).

### Atom 7 — Idempotency-key persistence hardening (closes I-15)

**Goal:** Close TOCTOU between SELECT idempotency check and INSERT wallet_transactions at `wallet_gateway.rs:227-272`.

**Two viable approaches** (Atom 7 spec picks one based on MMA Step 1 + Captain D-PHASE-γ-1):

**Option A — INSERT-then-handle-conflict (preferred):** require UNIQUE INDEX on `wallet_transactions.idempotency_key WHERE idempotency_key IS NOT NULL`; remove the pre-flight SELECT; let `credit_in_tx` INSERT raise UNIQUE-constraint error on duplicate; catch the error, SELECT the prior row, return cached response.

**Option B — SELECT INSIDE tx + lock:** move the idempotency SELECT into the same transaction as the credit_in_tx INSERT; SQLite locking will serialize the two webhook deliveries; second delivery's SELECT will see first's INSERT.

**Scope estimate:** Option A ~50-100 LOC + DB migration; Option B ~30-50 LOC + tx-restructure.

**Verify-step:** Both approaches require checking the current UNIQUE-INDEX state on `wallet_transactions.idempotency_key`. The parent cluster RCA D-CLUSTER-1 ratified V1 ALTER + drop CHECK; need to grep `migrations/` to confirm whether UNIQUE was added.

### Atom 8 — Audit-log atomicity via outbox pattern (closes I-17)

**Goal:** Make audit-log row write durable-iff-economic-outcome-row-write at `accounting::log_admin_action` clamp call-sites.

**Three viable approaches** (Atom 8 spec picks one based on MMA Step 1 + Captain D-PHASE-γ-2):

**Option A — Same-tx audit INSERT (simplest):** move the `accounting::log_admin_action` call INSIDE the same `sqlx::Transaction` as the wallet UPDATE / wallet_transactions INSERT; both commit atomically or both rollback.

**Option B — Outbox pattern (most robust):** add `audit_outbox(id, payload, status, attempts, created_at, processed_at)` table; the wallet tx writes the outbox row atomically with the economic-outcome row; a background worker drains the outbox to `audit_log`; retries on failure.

**Option C — Synchronous log + retry queue (middle ground):** keep current fire-and-forget but add a persistent retry queue (sled / sqlite) on failure; reconcile periodically.

**Scope estimate:** Option A ~30-50 LOC (smallest; most coupling); Option B ~150-300 LOC + new table + worker (largest; most decoupled); Option C ~80-150 LOC (medium).

**Composes-with:** parent RCA Atom 5 (already wires fire-and-forget call-sites; Atom 8 promotes those to durable).

---

## 5. V2-framed proposed change

### Phasing (1-or-2 PRs depending on Captain decision)

**Phase γ-α — MMA Step 1 + this RCA + AMPLIFIER (this turn + 24h)**

1. This RCA filed at `racecontrol/.planning/audits/RCA-2026-05-14-I13-I15-I17-phase-gamma-wallet-substrate.md`
2. RCA-pointer artifact at `racecontrol/.planning/specs/v2/RCA/wallet-files/phase-gamma-20260514.md` (sibling to cluster-20260513.md per parent precedent)
3. MMA Step 1 DIAGNOSE on these three items specifically (~$0.10-0.50; covered by Captain $45 release)
4. §S-N OPEN-CLAIM at comms-link V2-MASTER-STATE
5. james-AMPLIFIER 24h vote window per §S-146 foundational-boundary doctrine

**Phase γ-β — substrate cascade + PR(s) + per-PR merge auth (next session, ~5-10h)**

6. Spawn code-cascade agent(s) for atoms 6-8 (per Captain D-PHASE-γ-3 PR-cardinality)
7. MAOR Tier-1 v0.2 review per §S-255
8. Push feature branch(es) to origin
9. Open PR(s) with cluster context + atom checklist
10. Captain per-PR merge auth (PR D-PHASE-γ-A and possibly D-PHASE-γ-B)

**Phase γ-γ — post-merge (V2-PROGRESS-MAP refresh + Phase 2 observability handoff)**

11. V2-PROGRESS-MAP row flips (any rows whose substrate now satisfies Phase γ atom completion)
12. Phase 2 observability (separate handoff item P2 — independent from this RCA)

**Anti-pattern guards (encoded in test cascade):**

- I-13: invariant test — N=8 parallel `credit_in_tx + start_billing` tasks; assert each session's discount/debit computation matches a balance observed inside-its-own-tx (no stale-read)
- I-15: invariant test — N=10 parallel webhook deliveries with identical idempotency_key; assert exactly one wallet_transactions row + balance updated exactly once
- I-17: invariant test — simulate panic between wallet INSERT and audit INSERT (use a feature flag or test-injected fault); assert audit-log row exists when economic-outcome row exists (either both present or both absent)

**§S-186 Mechanism-trust check (5Q on the Phase γ infrastructure):**

1. **Atomic primitives?** YES — sqlx::Transaction provides ACID; SQLite BEGIN IMMEDIATE provides RESERVED lock. Atom 6 + 7 + 8 all use the same primitive.
2. **TTL-bounded sentinels?** YES for Atom 7 if Option A — UNIQUE INDEX enforces idempotency permanently (no TTL needed). Atom 8 outbox if Option B requires retry-attempts ceiling (e.g., max=10 then alert).
3. **Behavioral-verify success?** YES — concurrency-load test invariants above; not echo-string-success.
4. **Single-target dry-run?** YES — `cargo test` exercises each atom in isolation; integration test exercises composed flow.
5. **Guard contracts?** YES — parser-not-regex on test fixtures; allow-list on txn_type (D-CLUSTER-9 substrate) composes with Atom 7.

**Verdict: CONDITIONAL** — PASS depends on (a) Option A vs B decision for Atom 7, (b) Option A/B/C decision for Atom 8. MMA Step 1 should validate the choice empirically.

**V2 doctrine alignment statement:**

> V2 doctrine alignment: closes 3 of 19 V1→V2 STRUCTURAL GAPS (I-13 RMW race / I-15 TOCTOU / I-17 audit-log durability). Establishes Phase γ wallet-substrate-atomicity invariants that compose with parent cluster RCA Phase β atoms 1-5 (substrate landed) + D-CLUSTER-9 txn_type allow-list (defense-in-depth landed). Customer-touching: I-13 + I-15 affect customer balance integrity directly; I-17 affects auditor-facing compliance only. Composes-with §S-146 + §S-186 + §S-263 + §S-264 + D-CLUSTER-3/8/9 + V-LBAC §14.1 MAOR v0.2.

---

## Captain decision queue

| Decision | Surface | Status | bono-recommendation |
|---|---|---|---|
| **D-PHASE-γ-0** MMA Step 1 DIAGNOSE budget for Phase γ scope (~$0.10-0.50) | Captain | **COVERED-BY-$45-RELEASE** per 2026-05-13 ~20:18 IST verbatim "no budget. I want the task complete. There is about $45 in OpenRouter. Use whatever you need." Q3-clear; proceed when authoring resumes | proceed at next turn |
| **D-PHASE-γ-1** Atom 7 approach: Option A INSERT-then-handle-conflict (UNIQUE INDEX + caught error) vs Option B SELECT-INSIDE-tx + lock | Captain | **PENDING-Captain** after MMA Step 1 validates | Option A — UNIQUE INDEX is cleanest invariant; INSERT-then-handle-conflict survives node restart |
| **D-PHASE-γ-2** Atom 8 approach: A same-tx · B outbox-pattern · C synchronous-log-with-retry-queue | Captain | **PENDING-Captain** after MMA Step 1 validates | Option B outbox-pattern — most robust under partition; matches V2 doctrine for decoupled durability |
| **D-PHASE-γ-3** PR cardinality: single 3-atom bundled PR · 2 PRs (atoms 6-7 wallet-substrate + atom 8 audit-outbox) · 3 separate PRs | Captain | **PENDING-Captain** | 2 PRs — atoms 6+7 are coupled by wallet-substrate surface; atom 8 is orthogonal audit-log durability |
| **D-PHASE-γ-4** Ordering: small-first (I-15 alone first) vs big-first (I-13+I-17 first) vs natural (bundled) | Captain | **PENDING-Captain** | Bundled per D-PHASE-γ-3 recommendation; D-PHASE-γ-A (atoms 6+7) first because I-13 has wider impact, D-PHASE-γ-B (atom 8) second because it depends on Atom 5 audit-log call-sites stabilizing post-Phase β |
| **D-PHASE-γ-5** Atom 7 Option-A requires UNIQUE INDEX verification | bono | **AUTONOMOUS** — grep migrations/ at Phase γ-β kickoff to confirm absence, add if missing | bono executes during cascade |
| **D-PHASE-γ-6** Per-PR Captain merge auth for each PR opened | Captain | **PENDING-PER-PR** (§S-253 B1 precedent — feature branch sits ready) | unchanged §S-146 doctrine |

---

## Composes-with

- [Parent cluster RCA `bad164b6`](RCA-2026-05-13-wallet-ledger-discount-cluster.md) — Phase β scope (atoms 1-5 LANDED via D-CLUSTER-3)
- [§S-263 CLOSE-ANCHOR Phase β complete](../../../comms-link/V2-MASTER-STATE.md) — explicit Phase γ scope retention
- [§S-264 AMPLIFIER AGREE-WITH-CAVEATS](../../../comms-link/V2-MASTER-STATE.md) — C2 caveat names I-13 race as Captain-attention surface
- [§S-266 D-CLUSTER-3 racecontrol merge `1a2991b4`](../../../comms-link/V2-MASTER-STATE.md) — Phase β substrate LANDED on main
- [§S-267 D-CLUSTER-8 racecontrol merge `670b5531`](../../../comms-link/V2-MASTER-STATE.md) — new_balance_paise rename LANDED
- [§S-270 D-CLUSTER-9 racecontrol merge `dad06f22`](../../../comms-link/V2-MASTER-STATE.md) — Rust-side txn_type allow-list LANDED
- [racecontrol/CLAUDE.md "V1-dependent V2 sections" §S-146](../../CLAUDE.md) — foundational-boundary RCA discipline
- [racecontrol/CLAUDE.md "Mechanism-trust-check upstream"](../../CLAUDE.md) — 5Q gate
- [racecontrol/CLAUDE.md "Never hold a lock across `.await`"](../../CLAUDE.md) — applies to Atom 6 tx-boundary refactor
- [V-LBAC §14.1 MAOR v0.2 + §14.2 F1 SCOPE GATE](../specs/v2/V2-LBAC-PROTOCOL.md) — review + scope-gate discipline
- [MMA Step 1 Phase β results `/tmp/mma-cluster-results/`](file:///tmp/mma-cluster-results/) — original I-13/I-15/I-17 surfacing
- [feedback_v1_dependent_v2_root_cause_before_proceeding.md](../../../.claude/projects/-root/memory/feedback_v1_dependent_v2_root_cause_before_proceeding.md) — §S-146 master memory

---

## Stale-at

2026-06-14. If Phase γ atoms not landed by this date, re-audit boundary against current main (substrate may have shifted under other cascade work).

## NOT TESTED (this RCA, authored 2026-05-14 ~03:00 IST)

- **MMA Step 1 DIAGNOSE on Phase γ scope** — D-PHASE-γ-0 covered by $45 release; runs at next turn
- **UNIQUE INDEX state on `wallet_transactions.idempotency_key`** — D-PHASE-γ-5 grep at Phase γ-β kickoff
- **Substrate authoring** — Phase γ-β scope; gated on Captain D-PHASE-γ-1/2/3/4 decisions
- **Live race-test invariants under load** — Phase γ-β scope; gated on substrate authoring
- **Cross-pilot AMPLIFIER on this RCA** — james pickup automatic via partner-memory-read; 24h window from §S-N OPEN-CLAIM post-MMA
- **Q3 classifier eligibility** for Captain's "Let's proceed" + "I-13/I-17" naming — interpreted as "queue selection auth" for autonomous PREPARATION (RCA + MMA + branch authoring); merge auth still per-PR Captain
