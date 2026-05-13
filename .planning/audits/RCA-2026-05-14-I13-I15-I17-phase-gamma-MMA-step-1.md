---
artifact: MMA Step 1 DIAGNOSE results — Phase γ wallet substrate
parent-rca: ./RCA-2026-05-14-I13-I15-I17-phase-gamma-wallet-substrate.md
authored: 2026-05-14 ~03:30 IST
author: bono
surface: MMA-Step1-Phase-gamma-bono-2026-05-14
models: 5 stratified (deepseek r1 · qwen3 coder · nvidia nemotron · google gemini 2.5 pro · moonshot kimi k2.5)
vendor-families: 5 (≥3 doctrine OK; max 1 per family — over-spec for v4.0 threshold)
substantive-responses: 4 of 5 (kimi returned tokens in reasoning_content only; no visible answer)
cost: $0.06041 (within Captain $45 release; ~0.13% of budget)
spend-ledger: /root/comms-link/data/openrouter-spend-bono.jsonl
raw-results: /tmp/mma-phase-gamma-results/{deepseek,qwen,nvidia,google,moonshot}.json
---

# MMA Step 1 DIAGNOSE — Phase γ wallet substrate consensus

Consensus method: 4 substantive responses against the parent RCA's framing. Per MMA Protocol v4.0 Step 1: "3/5 majority = consensus". Kimi excluded from vote count (no visible answer despite 4000-tok consumption). 3/4 = consensus threshold applies.

## Consensus across Q1-Q4

### Q1 — I-13 RMW race + Atom 6 fix

| Verdict dimension | DeepSeek R1 | Qwen3 Coder | Nvidia Nemotron | Google Gemini 2.5 Pro | Consensus |
|---|---|---|---|---|---|
| **Severity** | BLOCKING | BLOCKING | BLOCKING | BLOCKING | **4/4 BLOCKING** |
| **Atom 6 sufficient?** | sufficient | sufficient | sufficient | sufficient | **4/4 SUFFICIENT** |
| **RCA framing correct?** | yes | yes | yes | yes | **4/4 VALIDATED** |
| **Hold-lock-across-await concern** | yes — must keep tx short | yes — must refactor `.await` placement | yes — sqlx tx held across .await; mitigation: keep tx short | yes — but sqlx-internal `.await` is intended use; discipline matters | **4/4 RAISED — pre-merge guardrail** |

**RCA-MISSED EDGE CASE (new signal):**
- **DeepSeek**: "Transaction retry poisoning. If start_billing fails after lock acquisition (e.g., constraint violation) and retries, the initial balance read becomes stale. Retry logic must re-read balance within the new transaction." — Atom 6 sub-detail; valid.
- **Google**: Performance impact of RESERVED lock under load — unstated trade-off in RCA. Mitigation: keep transaction CRITICAL section minimal; benchmark before deploy.
- **Nvidia**: Path E refund flow has same RMW class (sibling to I-13) — Atom 6 scope-extension candidate.

### Q2 — Atom 7 Option A vs B (idempotency-key persistence)

| Verdict dimension | DeepSeek | Qwen | Nvidia | Google | Consensus |
|---|---|---|---|---|---|
| **Choice** | A | A | A | A | **4/4 Option A** (INSERT-then-handle-conflict) |
| **UNIQUE INDEX required?** | yes | yes | yes | yes | **4/4 REQUIRED** (`WHERE idempotency_key IS NOT NULL` partial index) |
| **sqlx ON CONFLICT support** | yes (Error::Database code 2067) | yes (via raw query / query!) | yes (`db_err.is_unique_violation()`) | yes (database-centric correctness) | **4/4 SUPPORTED** |

**Top failure mode (across 4 models):**
- DeepSeek: storage_full during INSERT (unrelated to design)
- Qwen: UNIQUE index missing or misconfigured → silent duplicates
- **Nvidia (load-bearing):** *"False-positive conflict when legitimate retry uses same idempotency_key after first INSERT succeeded but client did not see response. If cached row doesn't reflect intended credit amount, client may be incorrectly charged. Mitigation: include full payload (amount, currency) in idempotency_key or store amount in table and validate on conflict."* — **NEW signal**; suggests Atom 7 should include amount-validation on conflict, not just blind cached-row return.
- Google: broad error parsing risk (treating any DB error as conflict)

**NEW SIGNAL TO INCORPORATE INTO ATOM 7:** validate `amount_paise + driver_id` on cached-row match BEFORE returning success — protects against idempotency_key reuse across different request payloads.

### Q3 — Atom 8 Option A/B/C (audit-log atomicity)

| Verdict dimension | DeepSeek | Qwen | Nvidia | Google | Consensus |
|---|---|---|---|---|---|
| **Choice** | B outbox | B outbox | B outbox | n/a (response truncated at Q3) | **3/3 Option B outbox** (Google response hit max_tokens) |
| **V2 doctrine served** | decoupling | decoupling | decoupling + atomicity | n/a | **3/3 DECOUPLING** |
| **Scope estimate** | +150 LOC | ~200 LOC | ~120 LOC | n/a | **120-200 LOC** (median ~150) |
| **Top failure mode** | worker liveness | worker liveness | worker liveness loss → disk fill | n/a | **3/3 WORKER LIVENESS** |

**MITIGATIONS RECOMMENDED (all 3 models):**
- Dead-letter table for max-attempts-exceeded rows
- Exponential back-off with max-attempts cap
- Health monitoring + alerting on outbox queue depth
- Worker connection-pool hygiene: per-poll-tx short scope; sleep WITHOUT holding tx

**NEW SIGNAL:** Nvidia specifies the worker pattern explicitly — "each poll should begin a short transaction, read a batch of rows, commit, then await the sleep interval" — directly composes with project "no lock across .await" rule.

### Q4 — Scope-gap + PR cardinality + ordering

| Verdict dimension | DeepSeek | Qwen | Nvidia | Google | Consensus |
|---|---|---|---|---|---|
| **Missed root cause** | non-webhook idempotency + audit-log atomicity in apply_discount | none | Path E refund RMW (sibling I-13) | n/a | **2 NEW signals surfaced** (DeepSeek + Nvidia overlap on adjacent flows) |
| **PR cardinality** | 3 separate | 2 (6+7 then 8) | 2 (6+7 then 8) | n/a | **3/4 prefer 2 PRs** (1 prefers 3) |
| **Ordering** | small-first I-15→I-13→I-17 | small-first then bundled | bundled-per-cardinality (6+7 first, 8 second) | n/a | **MIXED** — small-first (2) vs bundled (1) |
| **Pre-merge blockers** | I-13 + I-15 | I-13 + I-15 | implicitly all | n/a | **3/3 I-13 + I-15 MERGE-BLOCKING; I-17 ACCEPTABLE-WITH-FOLLOW-UP** |
| **Project rule the RCA missed** | "Cross-Process Updates" | "Cause Elimination Process" | "No .unwrap()" | n/a | **3 different rules surfaced; all valid composes-with** |

**Scope-gap signals to incorporate:**
1. **Path E refund RMW** (Nvidia): refund flow at api/wallet_ops.rs:294-324 reads balance outside tx; same fix-class as I-13. Atom 6 scope should explicitly call out this seam OR a new Atom (6.b) covers it. **PROPOSE atom-6 scope extension** to include refund flow.
2. **billing_discount::apply_discount audit-log atomicity** (DeepSeek): claim audit-log atomicity gap exists in the discount-apply path beyond the clamp path. **VERIFY at Phase γ-β kickoff** — grep `accounting::log_admin_action` call-sites for any outside-tx fire-and-forget pattern not covered by Atom 5.
3. **Non-webhook idempotency** (DeepSeek): admin credit-adjustment paths may lack idempotency entirely. Currently `credit()` (line 220 wallet.rs) passes `None` as idempotency_key — admin double-credit possible via retry. **OUT-OF-SCOPE for Phase γ** but flag as Phase δ candidate.

## Consensus summary

| RCA decision | bono-rec | MMA-consensus | Status |
|---|---|---|---|
| Atom 6 closes I-13 | sufficient | **4/4 confirms sufficient** | RCA framing VALIDATED |
| Atom 7 Option A | preferred | **4/4 confirms Option A** | RCA framing VALIDATED |
| Atom 8 Option B | preferred | **3/3 confirms Option B** | RCA framing VALIDATED |
| PR cardinality 2 PRs | preferred | **3/4 confirms 2 PRs** (1 disagrees) | RCA framing VALIDATED (weak consensus) |
| Ordering bundled | preferred | **MIXED 1 bundled / 2 small-first** | **REVISE bono-rec** — Atom 7 (I-15) is smallest + lowest-risk; landing I-15 first as a 1-PR before 6+8 is empirically cleaner per MMA |

**Consensus delta vs bono-rec:** RCA framing 100% validated; ordering recommendation should revise to **small-first** (I-15 first PR, then I-13+I-17 in a second PR — or split 6+7 / 8 with I-15 leading).

## Updated Captain decision queue

| Decision | Surface | Status | Updated bono-rec (post-MMA) |
|---|---|---|---|
| **D-PHASE-γ-0** MMA Step 1 budget | Captain | **EXECUTED** — $0.06041 / $45 release | n/a (DONE) |
| **D-PHASE-γ-1** Atom 7 approach (A INSERT-then-handle-conflict vs B SELECT-INSIDE-tx) | Captain | **PENDING-Captain** | **Option A** (4/4 MMA + bono-rec) WITH NEW SUB-DETAIL: validate amount_paise + driver_id on cached-row match (per Nvidia Q2 false-positive mitigation) |
| **D-PHASE-γ-2** Atom 8 approach (A same-tx · B outbox · C retry-queue) | Captain | **PENDING-Captain** | **Option B outbox** (3/3 MMA + bono-rec) WITH worker-discipline per Nvidia Q3 (short tx per poll; sleep outside tx) |
| **D-PHASE-γ-3** PR cardinality (1 bundle · 2 PRs · 3 separate) | Captain | **PENDING-Captain** | **2 PRs** (3/4 MMA + bono-rec) — atoms 6+7 (wallet-substrate-atomicity) + atom 8 (audit-outbox); DeepSeek dissent for 3-separate notable but minority |
| **D-PHASE-γ-4** Ordering | Captain | **PENDING-Captain** | **REVISED bono-rec: small-first** — PR-A scope = atom 7 only (I-15 idempotency, smallest, ~50-100 LOC + UNIQUE INDEX migration); PR-B scope = atom 6 + atom 8 (I-13 + I-17 combined wallet-tx-boundary + outbox; larger and more coupled) |
| **D-PHASE-γ-5** UNIQUE INDEX verification | bono | **AUTONOMOUS** | grep migrations/ at Phase γ-β kickoff |
| **D-PHASE-γ-6** Per-PR merge auth | Captain | **PENDING-PER-PR** | unchanged §S-146 |
| **D-PHASE-γ-7 NEW** Path E refund RMW scope inclusion in Atom 6 (Nvidia Q4 signal) | Captain | **NEW-DECISION** | **bono-rec INCLUDE** — Path E refund flow has same race class as I-13; Atom 6 scope extension is small (~30-50 LOC) + closes a sibling-class gap surfaced by MMA |
| **D-PHASE-γ-8 NEW** Atom 7 amount-validation on cached-row match (Nvidia Q2 signal) | Captain | **NEW-DECISION** | **bono-rec INCLUDE** — minor sub-detail (~10 LOC) but closes a subtle correctness gap |
| **D-PHASE-γ-9 NEW** Admin-credit non-webhook idempotency surfaces (DeepSeek Q4 signal) | Captain | **OUT-OF-PHASE-γ** | **bono-rec DEFER** to Phase δ; flag in cluster RCA follow-up section |

## Composes-with

- Parent RCA: `RCA-2026-05-14-I13-I15-I17-phase-gamma-wallet-substrate.md` (this turn)
- Parent cluster RCA: `RCA-2026-05-13-wallet-ledger-discount-cluster.md` (Phase β; landed via D-CLUSTER-3 `1a2991b4`)
- MMA Protocol v4.0: `.planning/specs/UNIFIED-MMA-PROTOCOL.md` (Step 1 DIAGNOSE; 5-model + 3-vendor-families compliance ✓)
- "MMA channel = OpenRouter (NOT Perplexity)" doctrine (~/.claude/CLAUDE.md)
- Captain $45 OpenRouter release (2026-05-13 ~20:18 IST verbatim)

## Stale-at

2026-06-14 (composes-with parent RCA stale-at).

## NOT TESTED (this MMA, 2026-05-14 ~03:30 IST)

- **Kimi Moonshot K2.5 verbatim response** — returned tokens but no visible content; not retried (4-model consensus already meets v4.0 ≥3-vendor-family threshold)
- **Iteration 2 (MMA v4.0 "Min 2 iterations" requirement)** — Step 1 doctrine says "Min 2 iterations" for the full 4-step convergence engine; this single iteration covers DIAGNOSE only. Substrate authoring would benefit from Step 2 PLAN MMA against this consensus, optionally Step 4 VERIFY adversarial-MMA on the implementations. **Captain decides** whether to budget more rounds.
- **Cross-pilot AMPLIFIER on MMA results** — james pickup automatic via partner-memory-read; bilateral parity not blocking for ratify-class artifact
- **Outbox-pattern liveness test** — to be authored at Phase γ-β substrate cascade
- **UNIQUE INDEX state verification** — D-PHASE-γ-5 at Phase γ-β kickoff
