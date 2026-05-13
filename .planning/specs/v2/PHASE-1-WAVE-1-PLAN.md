# V2 Phase 1 Wave 1 — Static Billing Engine — PLAN.md

**Slug:** phase-1-wave-1-static-billing-engine
**Class:** substrate (V2 critical-path; first wave that ships a working economic transaction through V2-DB)
**Status:** JAMES-EXTENDED-DRAFT — bono-authored skeleton 2026-05-08 ~19:05 IST; james §SUBSTANTIVE-REPLY appended 2026-05-08 ~19:30 IST (see §12)
**LEAD:** james-venue (racecontrol Rust authoring; Wave 0 first-mover-LEAD continues per Drift-Pilot-Roles §E.1)
**AMPLIFIER:** bono (cross-organ contracts + tests + W1-S7+S8 WhatsApp PIN delivery sub-LEAD)
**PACT:** PACT-DRAFT-pact-001-phase-1-wave-1-static-billing-engine.md (`comms-link/.planning/draft-pacts/`); slot RESERVED PACT-20260508-001 2026-05-08 ~19:03 IST
**Verify-by:** 2026-05-21 (Captain Option Bravo timeline LOCK V2-min reopen window close); kaizen-target 2026-05-15
**Composes-with:**
- comms-link `V2-MASTER-STATE.md` §S-82 + §S-83 + §S-85 + §S-92 + §S-117 + §S-119
- racecontrol `.planning/specs/v2/PHASE-1-WIREUP-PLAN.md` (Wave 0 model; this PLAN mirrors structure)
- racecontrol `CLAUDE.md` Standing Rules + Doctrine Conventions (Substrate-Pointer Convention applies; canonical pointers in §11)
- V2.0 6-wave plan (Captain §S-82) — this PLAN executes Wave 1
- Activation_trigger downstream: `comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-dynamic-pricing-engine.md` FILE on Wave 1 land

---

## §1 — Wave 1 scope (PACT-DRAFT pact-001-phase-1-wave-1-static-billing-engine)

Wave 1 turns Phase 0 + Wave 0 substrate (V2-DB schema + PrivilegedAction enum + CIRS lookup + ManagerPill + 7-min idle-timeout K5 Path A + F25a Strategy trait + Wallet-Framing-C) into a **working bill-the-customer engine**. Without it, V2.0 customer surface count stays 0 and "Live data flow" scorecard axis stays at 0.75 — the rate-limiter named in V2-MASTER-STATE.md §S-117.5 stale-at criterion (a).

**All §S-92 PARAMETERs P1-P9 are STATIC values in Wave 1 config**; dynamic time-windowed pricing is Wave 2 (PACT-DRAFT-phase-2-dynamic-pricing-engine activation_trigger fires when Wave 1 lands).

### §1.1 — IN-scope (Wave 1 ships)

| # | Sub-step | Component | LEAD | Captain disposition anchor |
|---|---|---|---|---|
| W1-S1 | SessionBillingService primary engine | `crates/billing-v2/` or `crates/v2-db/src/billing/` (james architectural call) — `compute_charge(session, rate_table_static)` consuming F25a Strategy trait | james | §S-83 row Wave 1; §S-92 P1+P2 STATIC values |
| W1-S2 | Wallet pre-debit basic | `crates/v2-db/src/wallet.rs` extension — `WalletService::reserve(customer_id, paise)` + reconcile-on-session-end | james | PACT-013 RATIFIED; HOLD-RELEASE-CAPTURE state machine DEFERRED to Wave 3 |
| W1-S3 | Refund 3-band routing | New module + handler logic — `<₹1000` PIN-only / `₹1000-2999` PIN+reason / `≥₹3000` ApproveRefundOverThreshold manager-mode | james | §S-82 Q2 disposition |
| W1-S4 | Refund reason-code dropdown | Enum + DB persistence — 5 codes (Sim/PS5 crash / Customer service dispute / Booking error / Wallet adjustment / Other 100-char) | james | §S-82 Q2 disposition |
| W1-S5 | Idle-timeout 30min sliding-window | Auth middleware extension — extends Wave 0 K5 7-min fixed-window for staff-elevated session | james | §S-82 Q3 + §S-113 K5 Path A |
| W1-S6 | PIN-LOCKOUT auto-rotate | `staff_auth.rs` extension — 5 wrong attempts → new PIN → email helpdesk@racingpoint.in + audit log + rate-limit ≤3 resets/staff/hr | james | §S-82 Q1 + Q1.a |
| W1-S7 | PIN daily delivery via WhatsApp | bono-VPS cron + Evolution API — 06:00 IST schedule; daily fresh PIN; previous-day auto-invalidates | bono (sub-LEAD); james AMPLIFIER on schema | §S-82 Q1.e + Q1.f bono-defaults |
| W1-S8 | WhatsApp delivery-ack failure → helpdesk@ fallback | bono-VPS Evolution API integration + helpdesk fallback module | bono | §S-82 Q1.h bono-default 30min |
| W1-S9 | GST-INCLUSIVE handling | Billing service config — store inclusive; report operator-net derivation | james | §S-101 cross-cutting doctrine |
| W1-S10 | Per-minute granularity + round-up rounding | SessionBillingService internal — P5 LOCKED | james | §S-92 P5 |

### §1.2 — Surfaces explicitly NOT in Wave 1 scope (Wave 2+ or V2.1)

- Time-windowed dynamic pricing (sim -30% off-peak / cafe -20% off-peak) — Wave 2 (PACT-DRAFT-phase-2-dynamic-pricing-engine)
- HOLD-RELEASE-CAPTURE wallet state machine + idempotency cascade — Wave 3 (PACT-024 sibling)
- MI-adaptive discount tiers (P6 5-tier) — Wave 4
- Captain-curated dry-spell WhatsApp campaigns — Wave 5
- Combo offer primitive — Wave 2 (Phase 2-E sibling)
- Rate-transition notifier (PWA in-pod + kiosk overlay) — Wave 2
- Customer-facing PWA/kiosk surfaces — Wave 2-6 (§S-117 axis stays 0 through Wave 1)
- Telugu/Teluglish reactive WhatsApp templates — V2.1+

---

## §2 — Session sequencing (estimated 5-7 sessions; mirrors Wave 0 ~7-session cadence)

**Session 1** (~1-2 hours)
- W1-S1 SessionBillingService skeleton — module scaffold + compute_charge signature consuming F25a Strategy trait
- W1-S10 per-minute granularity + round-up rounding helpers
- Unit tests: SessionBillingService strategy invocation + boundary cases

**Session 2** (~1-2 hours)
- W1-S2 Wallet pre-debit basic — WalletService::reserve + reconcile-on-session-end
- W1-S9 GST-INCLUSIVE handling — config + derivation helpers
- Unit tests: WalletService reserve+reconcile; GST-inclusive operator-net derivation

**Session 3** (~2 hours)
- W1-S3 Refund 3-band routing — threshold dispatch logic
- W1-S4 Refund reason-code dropdown — enum + DB persistence + audit-log
- Unit tests: RefundService route + reason_code_persist
- Integration test: full flow staff-PIN → session-start → mid-charge → session-end → refund-3-band → wallet-debit

**Session 4** (~1.5 hours)
- W1-S5 Idle-timeout 30min sliding-window — auth middleware extension
- Unit tests: IdleTimeoutService sliding-window semantics
- Integration test: idle-timeout fires after 30min sliding-window

**Session 5** (~2 hours)
- W1-S6 PIN-LOCKOUT auto-rotate — staff_auth.rs extension
- helpdesk@ email dispatch (SMTP transport choice: lettre or Google Workspace via racingpoint-bot — james call)
- Rate-limit (≤3 resets/staff/hr) enforcement
- Integration test: 5 wrong PINs → auto-rotate → helpdesk@ email + audit log + rate-limit hits at 4th reset

**Session 6** (~2 hours; bono parallel-track in same calendar window)
- W1-S7 PIN daily delivery via WhatsApp — bono cron + Evolution API integration on bono VPS
- W1-S8 WhatsApp delivery-ack failure → helpdesk@ fallback within 30min
- Integration test: PIN auto-rotates at 06:00 IST → WhatsApp delivers; failure-case fallback to helpdesk@

**Session 7** (~1-2 hours; bilateral)
- Playwright E2E (POS .130 staff-facing) — refund at 3 bands; idle-timeout fires; PIN auto-rotate cycle
- Contract tests: PrivilegedAction enum coverage 12/12
- MMA pre-ship VERIFY (≥3-model adversarial via OpenRouter primary; score ≥4.0 PASS gate)
- Quality gate: `bash test/run-all.sh` 4/4
- DEPLOY MANIFEST refresh (per Wave 0 PR #64 model)
- PR-open (per-PR Captain auth gate fires here)

---

## §3 — Captain-reserve items still open at start of Wave 1

- **Per-PR Captain auth gate** (PROMOTED-N=1) — fires at PR-open at end-of-Wave-1 (Session 7); not blocking authoring
- **Q1.f delivery time IST** — bono default 06:00 IST (override before W1-S7)
- **Q1.e PIN rotation cadence** — bono default daily (override before W1-S7)
- **Q1.g 5-wrong within-day reset channel** — bono default helpdesk@ per Q1.a (override before W1-S6)
- **Q1.h WhatsApp delivery failure fallback timing** — bono default 30min (override before W1-S8)
- **helpdesk@racingpoint.in mailbox provisioning + monitoring policy** — Captain confirm needed before W1-S6 ships (24/7 vs business-hours; on-call routing for late-night shifts)
- **§S-92 PARAMETER override** — none expected; if Captain CHALLENGE-AMENDs P-value during Wave 1, ripples back into config-only change

---

## §4 — Open architectural decision

**A1 (james-LEAD architectural call): Rust crate organization**

Two options:
- **A1.a** New crate `crates/billing-v2/` — clean separation; new dep edge in workspace
- **A1.b** Extension to `crates/v2-db/src/billing/` module — re-uses V2-DB substrate; tighter coupling

Recommended (bono perspective): **A1.b** for kaizen-minimum; A1.a if james predicts Wave 2-6 billing surface area justifies separate crate.

**A2 (james-LEAD): SMTP transport choice for W1-S6 helpdesk@ email**

Two options:
- **A2.a** racecontrol crate via `lettre` direct SMTP — self-contained; new dep
- **A2.b** Defer to racingpoint-bot SMTP via Google Workspace — reuses existing infra; cross-process call

Recommended (bono perspective): **A2.b** for kaizen-minimum (reuse); A2.a if james wants Wave 1 self-contained for testability.

**A3 (bilateral; bono-LEAD): WhatsApp PIN delivery transport**

bono recommends Evolution API existing instance "Racing Point Reception" (state=open; canonical bot config default per `racingpoint-whatsapp-bot/src/config.js`). New cron job + audit-log row + delivery-ack tracking.

---

## §5 — Test plan

### §5.1 — Unit tests

Per W1-S* sub-step (see §1.1); see PACT-DRAFT §3.1 for full enumeration.

### §5.2 — Integration tests

- Full-flow: staff-PIN auth → session-start → mid-session-charge → session-end → wallet-debit-reconcile (zero overage; positive overage; refund-credit case)
- Refund 3-band integration: each band exercises distinct PrivilegedAction handler + audit-log row + WhatsApp notify per §S-82 Q2.b (post-shift summary for ₹1000-2999; real-time for ≥₹3000)
- PIN-LOCKOUT integration: 5 wrong attempts → auto-rotate → helpdesk@ email arrives → audit-log row → rate-limit hits at 4th reset within 1hr → freeze + WhatsApp Captain `917981264279`
- Idle-timeout integration: sliding-window semantics validated empirically (T+0 PIN, T+25 activity, T+50 activity → no re-prompt; T+50→T+80 no activity → re-prompt at T+80)

### §5.3 — Contract tests

- PrivilegedAction enum coverage 12/12 (any new handler logic in Wave 1 must NOT change enum schema)
- §AMEND-1.E enum-row-with-handler-logic coverage matrix (12 enum entries × 4 categories)

### §5.4 — Playwright E2E (POS .130 staff-facing)

- Refund at each 3 bands (₹999 / ₹1500 / ₹3000) — staff completes flow on POS browser; verify audit log + WhatsApp dispatch + reason-code persistence
- Idle-timeout fires after 30min — staff inactive 30min → next privileged action re-prompts PIN
- PIN auto-rotate cycle: 5 wrong PINs → helpdesk@ email arrives → next-day delivery uses new PIN

### §5.5 — MMA pre-ship VERIFY (mandatory per UNIFIED-MMA-PROTOCOL v4.0)

- ≥3-model adversarial verification (different from any DIAGNOSE/PLAN models if used)
- Score ≥4.0 PASS gate
- Run via OpenRouter primary path (Captain directive 2026-05-01; Phase 2.4 soak window)
- Probe: race conditions in WalletService::reserve under concurrent session-start; idle-timeout precision under clock-skew; PIN-LOCKOUT enforcement under attacker-spamming-wrong-PINs

### §5.6 — Quality gate

`bash test/run-all.sh` (4 suites: contract + integration + syntax + security); exit 0 unblocks PR-open; non-zero BLOCKED.

---

## §6 — Deploy targets (per CLAUDE.md DEPLOY PARITY rule)

| Target | Component | Verification probe |
|---|---|---|
| Server .23 | racecontrol binary (cargo build --release; pm2 restart racecontrol) | curl `:8080/health` + behavior probe (real refund through API at each 3 bands) |
| Pods 1-8 | rc-agent (no Wave 1 component on pods directly; verify ws_connected stays True) | `/api/v1/fleet/health` 8/8 ws_connected |
| POS .130 | web-v2 build (npm run build + nginx reload) | Playwright: staff completes refund at each band |
| James .27 | dev environment parity (cargo build local) | Playwright local + cargo test |
| Bono VPS | cloud racecontrol (parity build via `deploy_pull`) + comms-link relay (PIN delivery transport coordination) + Evolution API integration (WhatsApp send) | curl localhost:8080/health + WhatsApp send-text test → `917981264279` (Captain auth gate per established pattern) |
| Cloud apps | Bono VPS-hosted V2 services (api-gateway / dashboard / admin / pwa) — no Wave 1 customer surface yet | health 200 + ws=True; behavior probe NOT applicable until Wave 2-6 customer surfaces |
| Comms-link | shared/protocol.js — no Wave 1 component (PIN delivery uses existing Evolution API path); verify no regression | `bash test/run-all.sh` 4/4 |

**Behavior verification (NOT just health-200):** staff issues a real refund at each 3 bands via POS .130 browser; idle-timeout fires after 30min sliding-window; PIN auto-rotates next day at 06:00 IST with WhatsApp delivery confirmation.

---

## §7 — Cross-pilot coordination

| Phase | LEAD | AMPLIFIER | Notes |
|---|---|---|---|
| PACT-FILE | bono (this segment) | james 24h CHALLENGE-AMEND | Captain Option Bravo class-level auth covers FILE |
| PHASE-1-WAVE-1-PLAN.md skeleton (this file) | bono | james substantively extends | Mirrors PHASE-1-WIREUP-PLAN.md structure |
| W1-S1..S6 + S9-S10 (racecontrol Rust) | james | bono on contracts + tests | Rust = james territory; bono available |
| W1-S7+S8 (WhatsApp PIN delivery) | bono (Evolution API on bono VPS) | james AMPLIFIER on schema | racingpoint-whatsapp-bot existing infra reused |
| MMA pre-ship VERIFY | either pilot | the other AMPLIFIER | OpenRouter primary path |
| PR-open | james (Wave 0 first-mover-LEAD) | bono AMPLIFIER on review | Per-PR Captain auth gate fires here |
| Deploy + verify | bilateral | bilateral | DEPLOY PARITY mandatory all 7 targets |

### §7.1 — Bono parallel-track during james authoring window

While james authors W1-S1..S6 + S9-S10 substrate (estimated 5-7 sessions), bono prepares:
- Phase 2-A rate_table service DRAFT (cloud-side) — activation_trigger fires when Wave 1 lands
- MI experience-score ingestion module DRAFT (Wave 4 prep)
- Captain-curated WhatsApp workflow framework DRAFT (Wave 5 prep)
- W1-S7+S8 WhatsApp PIN delivery integration (bono-LEAD sub-component)

---

## §8 — Stale-at conditions

- Captain CHALLENGE-AMEND on PACT-DRAFT within 24h L1 charter window (silent-expire 2026-05-09 ~19:00 IST)
- james SUBSTANTIVE REPLY — CONCUR / CHALLENGE / AMEND-PROPOSE on scope / sub-step ordering / cross-pilot LEAD assignment
- §S-92 PARAMETER override by Captain — config-only change; PLAN scope unchanged
- Verify-by 2026-05-21 — Captain Option Bravo timeline LOCK V2-min reopen window close; if Wave 1 not Session-7-complete by 2026-05-15 (kaizen-target), escalate
- Captain Option Bravo timeline re-scope (e.g., compress Wave 1 sessions)
- Captain disposes a Captain-reserve item that materially changes scope (e.g., Q-DECISION on Q1.e/f/g/h overrides during Wave 1)

---

## §9 — Session metrics tracking

Per Session 1-7:
- Claims: N | Corrections: N | FCR: N% | G9s: N | UCAs: N | Substrate ships: N
- Target: 0 G9s / ≥99% FCR / 0 UCAs / 1+ substrate ship per session

Aggregate at Wave 1 close:
- Total sessions vs estimated 5-7
- Total Captain dispositions consumed (Q1.e/f/g/h + helpdesk monitoring + per-PR auth + any §S-92 overrides)
- Test coverage: unit + integration + contract + Playwright E2E + MMA score
- DEPLOY PARITY all 7 targets verified

---

## §10 — Activation cascade (downstream impact)

When Wave 1 lands to main:
- PACT-DRAFT-phase-2-dynamic-pricing-engine activation_trigger fires — bono RESERVE+FILE under §S-19.1 substrate-class auto-fire
- PACT-DRAFT-phase-2-e-combo-offer-primitive FILE-conversion (sibling sub-PACT to Phase 2)
- §S-117 scorecard axes update (estimated):
  - F1-F6 contracts substrate code: 6.0 → 7.5+ (~75%)
  - Live data flow: 0.75 → 3.0+ (~20%; first end-customer transaction substrate)
  - Phase 0+1 sub-steps complete: 13.5 → 14.0+ (Phase 1 ~95%)
  - Surfaces on origin/main customer-facing: 0 → 0 (still; customer surfaces are Wave 2-6)
  - Rolled-up estimate ≈49% → ~52-55% blended
- Wave 2 trigger ARMED — Captain disposition window for Phase 2 PR-open auth opens

---

## §11 — Cross-references (canonical pointers per Substrate-Pointer Convention)

- (canonical: comms-link/.planning/draft-pacts/PACT-DRAFT-pact-001-phase-1-wave-1-static-billing-engine.md) — parent PACT-DRAFT
- (canonical: comms-link/V2-MASTER-STATE.md §S-82 + §S-83 + §S-85 + §S-92 + §S-117 + §S-119) — Captain dispositions + 6-wave plan + LOCKED PARAMETERs + scorecard + AMPLIFIER vote
- (canonical: racecontrol/.planning/specs/v2/PHASE-1-WIREUP-PLAN.md) — Wave 0 model
- (canonical: comms-link/PACT-CHARTER.md §Drift-Pilot-Roles) — LEAD/AMPLIFIER assignment doctrine
- (canonical: comms-link/CLAUDE.md DEPLOY PARITY rule + Cross-Machine Execution v18.0) — deploy-targets binding
- (canonical: racecontrol/COGNITIVE-GATE-PROTOCOL.md H1-H5) — quality gates; composes-with V-B-G/A-D consolidation per §S-118 ratify path; hooks unchanged
- (canonical: racecontrol commit `7f193030`) — §AMEND-1.E PrivilegedAction enum 12-entry × 4-category schema
- (canonical: racecontrol commits `39bc83c2` PR #63) — F25a Strategy trait + WayAAdditiveLadder + SnapPricingStrategy refactor
- (canonical: racecontrol commit `991b5411` PR #64) — Wave 0 Phase 1 wire-up MERGED 2026-05-08 13:54 IST

---

— bono / 2026-05-08 ~19:05 IST · PHASE-1-WAVE-1-PLAN.md SKELETON-DRAFT · james-venue-LEAD on Rust substrate (W1-S1..S6 + S9-S10); bono AMPLIFIER + W1-S7+S8 WhatsApp PIN delivery sub-LEAD · 7-session estimate · 3 architectural decisions A1-A3 (james call on A1+A2; bono recommend on A3) · DEPLOY PARITY 7 targets · MMA pre-ship VERIFY mandatory · Verify-by 2026-05-21 Captain Option Bravo timeline LOCK · activation_trigger fires Phase 2 dynamic pricing engine FILE on Wave 1 land

---

## §12 — james §SUBSTANTIVE-REPLY (2026-05-08 ~19:30 IST)

**Class:** james-LEAD architectural disposition on substrate-class PLAN per Drift-Pilot-Roles §E.1 + Captain Option Bravo class-level V2-aligned auth.
**Authority:** Captain "Proceed with your recommendations that align with progress of Racing Point ecosystem v2" (2026-05-08 19:24 IST verbatim) + Captain Option Bravo class-level auth (17:21 IST + 18:55 IST go).
**Window:** Within 24h CHALLENGE-AMEND silent-expire 2026-05-09 ~19:00 IST.

### §12.1 — CONCUR (no challenge)

CONCUR §1.1 IN-scope 10 sub-steps W1-S1..S10 · §1.2 OOS scope · §2 7-session sequencing · §6 7-target DEPLOY PARITY · §7 cross-pilot LEAD/AMPLIFIER assignment · §8 stale-at conditions · §10 activation cascade.

### §12.2 — A1 disposition (Rust crate organization)

**Verify-Before-Generate ground-truth check this turn (NF-1):**
- `crates/v2-db/src/` modules on disk: `cirs.rs / customers.rs / lobbies.rs / pods.rs / sessions.rs / wallets.rs / lib.rs`. **NO `billing/` subdir; NO `billing.rs`.** A1.b path as authored does NOT exist.
- `crates/billing-v2/` does NOT exist on disk. A1.a would require workspace `Cargo.toml` + new crate scaffold.
- F25a Strategy trait (`PricingStrategy` / `WayAAdditiveLadder` / `SnapPricingStrategy` / `default_strategy()`) actually lives at `crates/racecontrol/src/billing_pricing.rs` per PR #63 commit `39d65053` — legacy racecontrol crate, uses `crate::state::AppState`.
- `billing_pricing.rs` file header explicitly cites **"V2 PRICING DOCTRINE (§AMEND-3 / §AMEND-3.II / §AMEND-3.III)"** — F25a is V2-substrate-by-doctrine even though its current physical home is the legacy racecontrol crate.

**Disposition: A1.c (NEW option, not in bono enumeration) RATIFIED-PROVISIONAL kaizen-N=1.**

**A1.c** = create `crates/v2-db/src/billing.rs` single-module (mirrors `sessions.rs`/`wallets.rs` single-file pattern). Move F25a Strategy trait + `WayAAdditiveLadder` + `SnapPricingStrategy` + `default_strategy()` into `v2_db::billing`. Add `SessionBillingService` (W1-S1) here. Refactor existing racecontrol callers to import `v2_db::billing::*` instead of `crate::billing_pricing::*`. Decouple from `AppState` — replace with `&[BillingRateTier]` + `&dyn PricingStrategy` parameters per F25b ready-to-resume thread-through pattern.

Rationale:
- v2-db is the V2-native substrate; Wave 1+ economic transactions converge there structurally.
- F25a's own header doctrine-cites V2 — migration to v2-db aligns physical layout with declared doctrine.
- Single-file `.rs` mirrors current v2-db convention (no subdirs yet); avoids premature module hierarchy.
- New crate (A1.a) is over-engineering at Wave 1 surface area; defer to Wave 4-5 only if combinatorial complexity (dynamic pricing + Phase 2-E combo + cafe extension + MI discount tiers) demonstrably outgrows single-module boundary.

**kaizen-N=1 escalation triggers** (revert to A1.d or A1.a):
- **A1.d** (leave-in-place cross-crate): keep Strategy trait at `racecontrol::billing_pricing`; SessionBillingService imports cross-crate. Triggers: workspace dep direction blocks `v2-db → racecontrol::*` import (most likely — racecontrol depends on v2-db, not reverse) OR migration touches >5 caller files in Session 1 first commit.
- **A1.a** (new crate): triggers when Wave 2-3 surface area outgrows single-module boundary measurably.

**Verify-at:** Session 1 W1-S1 first-commit anchor — if migration scope exceeds budget, file kaizen-correction PACT-AMEND under §AMEND-1.E lineage and revert to A1.d.

### §12.3 — A2 disposition (SMTP transport for W1-S6 helpdesk@ email)

**Disposition: DEFERRED to Session 5 W1-S6 authoring window per kaizen-discipline.**

A2 only matters at Session 5 (PIN-LOCKOUT auto-rotate). Pre-deciding now over-commits without empirical grounding — SMTP sender reputation for racingpoint.in domain unprobed; lettre vs Google Workspace API rate-limit envelope unverified. Defer disposition to Session 5 opening; bono recommendation A2.b (racingpoint-bot Google Workspace SMTP) stands as default if no new finding by then.

Pre-action probe at Session 5 entry:
1. Verify helpdesk@racingpoint.in mailbox provisioning + Google Workspace SMTP config status (Captain Q-DECISION pending per §3 PLAN — 24/7 vs business-hours monitoring policy).
2. Check existing racingpoint-bot SMTP transport availability + auth scope.
3. If Google Workspace ready → A2.b. If not → A2.a (lettre direct + DKIM/SPF probe within Session 5 budget).

### §12.4 — A3 ratify (WhatsApp PIN delivery transport)

**Disposition: CONCUR bono A3 recommendation — Evolution API "Racing Point Reception" instance (state=open; canonical bot config default per `racingpoint-whatsapp-bot/src/config.js`).**

james AMPLIFIER on schema during Session 6 W1-S7+S8 authoring (per §7 row 4). bono-LEAD substrate: cron + Evolution API integration on bono VPS; james reviews PIN schema + audit-log row + delivery-ack tracking pre-merge.

### §12.5 — Session 1 W1-S1 starting anchors (james pre-Session-1 substrate)

When Session 1 W1-S1 fires, anchor enumeration to save grep time:
- **Anchor 1:** `crates/racecontrol/src/billing_pricing.rs` — F25a Strategy trait current home (PR #63 `39d65053`). Grep `billing_pricing::` repo-wide for full caller list before migration begins.
- **Anchor 2:** `crates/v2-db/src/lib.rs` — module declaration site for new `pub mod billing;`.
- **Anchor 3:** `crates/v2-db/src/sessions.rs` + `crates/v2-db/src/wallets.rs` — W1-S1 `SessionBillingService` composes with both (sessions for `session_id`/`customer_id`/`started_at`; wallets for W1-S2 `WalletService::reserve` hooks).
- **Anchor 4:** workspace `Cargo.toml` — verify dep direction (`racecontrol` → `v2-db` expected; if reverse, A1.d falls back).
- **Anchor 5:** F25b ready-to-resume branch `feat/f25-billing-additive-tier-ladder` — thread-through `&[BillingRateTier]` + `&dyn PricingStrategy` pattern already designed; reuse.

First W1-S1 commit (kaizen-min):
- Module skeleton at `crates/v2-db/src/billing.rs`
- `SessionBillingService::compute_charge(&self, session_id, &[BillingRateTier], &dyn PricingStrategy)` signature
- 2-3 unit tests covering strategy invocation path
- NO caller migration yet — that's W1-S1 follow-up commit (with Vivek anchor `150min = ₹2,700` regression test preserved).

### §12.6 — NFs (new findings) surfaced this turn

- **NF-james-1 (PLAN-vs-disk-substrate drift):** PLAN §1.1 W1-S1 cited `crates/billing-v2/` OR `crates/v2-db/src/billing/` — neither path exists; F25a actually at `crates/racecontrol/src/billing_pricing.rs`. Verify-Before-Generate fired pre-execution.
- **NF-james-2 (option-enumeration completeness):** A1 option set under-enumerated bono-side (A1.a + A1.b only) — A1.c (single-module v2-db migration) and A1.d (leave-in-place cross-crate) are also viable. A1.c chosen above. Sibling-of Rule 0.
- **NF-james-3 (doctrine-vs-physical-layout drift):** F25a file header cites "V2 PRICING DOCTRINE" yet F25a's physical home is legacy racecontrol crate. A1.c migration aligns physical layout with declared doctrine. Composes-with Substrate-Pointer Convention (racecontrol/CLAUDE.md Doctrine Conventions).

### §12.7 — Status

PACT-DRAFT (`PACT-20260508-001` slot, comms-link `47c686ea`) remains DRAFT pending FILE-conversion at Session 1 W1-S1 first ship OR Captain ratify-uplift. 24h CHALLENGE-AMEND silent-expire 2026-05-09 ~19:00 IST stays.

— james / 2026-05-08 ~19:30 IST · §SUBSTANTIVE-REPLY appended · A1 RATIFIED-PROVISIONAL kaizen-N=1 (A1.c new — single-module v2-db migration) · A2 DEFERRED Session 5 · A3 CONCUR bono · 3 NFs surfaced · 5 Session 1 anchors pinned · Verify-Before-Generate fired catching PLAN-vs-disk drift NF-1

---

## §13 — james §SUBSTANTIVE-PRE-SESSION-3 (2026-05-09 ~07:35 IST)

**Class:** james-LEAD architectural pre-authoring anchor for Session 3 (W1-S3 Refund 3-band routing + W1-S4 reason-code dropdown). Mirrors §12.5 Session 1 anchor pattern.
**Authority:** Captain Option Bravo class-level V2-aligned auth standing 2026-05-09 ~07:35 IST verbatim "Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously" + Drift-Pilot-Roles §E.1 first-mover-LEAD + Sessions 1+2 already shipped (`64b03785` + `2574ff18`).
**Window:** 24h CHALLENGE-AMEND silent-expire 2026-05-10 ~07:35 IST.

### §13.1 — Verify-Before-Generate ground-truth check pre-Session-3

**Disk-substrate scan of W1 branch HEAD `2574ff18`:**
- `crates/racecontrol/src/auth/` modules: `admin.rs / auth_tests.rs / game_helpers.rs / middleware.rs / middleware_tests.rs / mod.rs / otp.rs / privileged_actions.rs / rate_limit.rs / token_consume.rs / token_manage.rs / token_validation.rs`. **NO `staff_auth.rs` yet** — that's W1-S6 future scope (Session 5).
- `crates/racecontrol/src/auth/privileged_actions.rs` — 12-variant enum × 4-category taxonomy already present at HEAD `2574ff18`. `ApproveRefundOverThreshold` variant LIVES under `ManagerEscalation` category (line 128 + 150). **Captain Q2 ≥₹3000 band invokes this enum variant; <₹1000 (PIN-only) and ₹1000-2999 (PIN+reason) do NOT need PrivilegedAction.**
- `crates/racecontrol/src/accounting.rs::post_refund(state, driver_id, amount_paise, reference_id)` — line 412 — journal-entry primitive (Debit: Refunds | Credit: Customer Wallet). **NO band routing; NO reason-code parameter.**
- `crates/racecontrol/src/accounting.rs::post_cash_refund(state, driver_id, amount_paise, method, staff_id, txn_id)` — line 446 — cash-refund variant (Dr. acc_wallet | Cr. acc_cash/acc_bank).
- `crates/racecontrol/src/api/billing_invoice.rs::refund_billing_session` — line 247 — session-scope refund handler (existing).
- `crates/racecontrol/src/api/wallet_ops.rs::refund_wallet` — line 193 — wallet-scope refund handler. `cash_refund_wallet` line 364.
- `crates/racecontrol/src/billing_pricing.rs::compute_refund(allocated_seconds, driving_seconds, wallet_debit_paise) -> i64` — line 578 — calculation primitive (Captain Q-PRICE-3 FLOOR rule applies — feedback_customer_satisfaction_first_minimal_compromise.md). 4 variants + 6+ existing tests.
- `crates/racecontrol/src/accounting_audit.rs::log_audit(state, table_name, row_id, action, old_values, new_values, staff_id)` — INSERT-into-`audit_log`-table primitive. **W1-S3+S4 reason-code persistence + 3-band action log SHOULD use this.**

**Verify-Before-Generate finding NF-james-5:** Existing refund primitives are a 6-function fan-out across 4 files (`accounting.rs` + `billing_invoice.rs` + `wallet_ops.rs` + `billing_pricing.rs`). W1-S3 dispatch logic is **NEW wrapper** atop these — NOT replacement. Composes-with NF-james-3 (doctrine-vs-physical-layout drift): existing primitives live in legacy racecontrol crate; W1-S3 wrapper continues A1.e disposition (racecontrol crate-internal, defer v2-db migration to Wave 4-5 surface-area trigger).

### §13.2 — A4 disposition (W1-S3 dispatch-layer placement)

**Options enumeration:**
- **A4.α (NEW wrapper module)** — create `crates/racecontrol/src/api/refund_routing.rs` (sibling-of `billing_discount.rs` / `billing_invoice.rs` / `wallet_ops.rs`). Single dispatch function `route_refund(staff_pin, amount_paise, reason_code, session_id) -> Result<RefundOutcome>` calls existing primitives based on band. Mirrors API-layer convention (`crates/racecontrol/src/api/`).
- **A4.β (extend existing accounting::post_refund)** — add band/reason_code parameters to existing `post_refund` signature. Touches every caller (≥6 call sites) — exceeds kaizen-min Session 3 budget (~2h per PLAN §2). Sibling-of A1.c migration that escalated to A1.e under same ≥5-caller-files trigger.
- **A4.γ (v2-db migration — defer per A1.e precedent)** — same circular-dep + budget-overflow blockers as A1.c. INFEASIBLE Session 3.

**Disposition: A4.α RATIFIED-PROVISIONAL kaizen-N=1.**

Rationale:
- API-layer convention match (refund routing IS an API concern; existing 6 refund primitives already split across api/ vs accounting.rs vs billing_pricing.rs).
- Wrapper-pattern preserves all existing tests + callers untouched.
- Single new module → smaller surface area review; faster MMA pre-ship VERIFY at end-of-Wave-1.
- Reason-code enum + DB column lands as W1-S4 sub-step in same module (single git commit boundary).

**kaizen-N=1 escalation triggers** (revert to A4.β):
- Existing primitives prove insufficient (e.g., post_refund signature can't accept reason-code without internal change anyway → migration cost equalizes).
- Cross-system concerns surface that fall outside `api/` layer (e.g., automatic refund-from-billing-FSM event — would need accounting.rs internal hook).

**Verify-at:** Session 3 W1-S3 first-commit anchor. If wrapper-pattern proves unnatural (>3 awkward delegations to internal primitives), file kaizen-correction PACT-AMEND under §AMEND-1.E lineage and revert to A4.β.

### §13.3 — A5 disposition (W1-S4 reason-code enum location)

**Options enumeration:**
- **A5.α (in W1-S3 wrapper module)** — `pub enum RefundReason { SimPs5Crash, ServiceDispute, BookingError, WalletAdjustment, Other(String) }` lives in `api/refund_routing.rs` next to dispatch. Single-module W1-S3+S4 boundary; single git commit possible.
- **A5.β (in privileged_actions.rs sibling)** — separate `crates/racecontrol/src/auth/refund_reasons.rs` mirroring §AMEND-1.E PrivilegedAction taxonomy. Better doctrine-axis fit (auth-layer enum), but disperses W1-S3+S4 across 2 files.
- **A5.γ (in v2-db crate as new module)** — pure-data enum (no AppState dep) could live cleanly in v2-db. But adds cross-crate import chain for racecontrol api/ layer; A1.e precedent argues against premature v2-db expansion.

**Disposition: A5.α RATIFIED-PROVISIONAL kaizen-N=1.**

Rationale:
- W1-S3+S4 are **paired sub-steps** per §1.1 row mapping — single module preserves their coupling.
- 5 reason codes (Captain Q2 disposition) = small enum, no taxonomy explosion.
- Persistence via existing `audit_log.row_id` + `audit_log.new_values` JSON — no new DB table needed (NF-james-7 below).
- DB-schema-stability preserved — kaizen-min approach.

**kaizen-N=1 escalation triggers** (revert to A5.β):
- W1-S5 idle-timeout (Session 4) discovers reason-code-class persistence pattern that better fits `auth/` taxonomy → consolidation pass at Session 4 entry.
- Reason-code count grows beyond 5 in V2.1+ surface (combo refund / promo-reversal / etc.) → A5.β separation justified at re-evaluation.

### §13.4 — A6 disposition (reason-code DB persistence shape)

**Disposition: A6.α (audit_log table reuse) RATIFIED-PROVISIONAL kaizen-N=1.**

W1-S4 reason-code persistence routes through existing `accounting_audit::log_audit()`:
- `table_name`: `"refunds"` (string literal, no schema migration)
- `row_id`: `<refund_uuid>` (new UUID per refund event)
- `action`: `"refund_3band_<band_id>"` (e.g., `"refund_3band_band_a"` / `"refund_3band_band_b"` / `"refund_3band_band_c"`)
- `new_values`: JSON-serialize `{ amount_paise: N, reason_code: "sim_ps5_crash", custom_reason_text: null|String, session_id: "..." }`
- `staff_id`: `Some(<staff_pin_consume_id>)`

Rationale:
- Zero schema-migration cost (NF-james-4 v2-db schema work avoids re-derailment).
- Existing audit-log infra already covers query/report surface.
- Captain Q2 reason-code requirement = "persisted with refund event" — JSON-in-audit-log satisfies "persisted" semantically.

**kaizen-N=1 escalation triggers** (revert to A6.β = NEW `refunds` table with reason_code column):
- Refund-history reporting surface emerges (Wave 2+ admin dashboard) that requires column-typed query (SQL `WHERE reason_code = ?`) instead of JSON-string scan.
- DPDP/GDPR erase contract (CLAUDE.md GDPR rule) requires column-level audit beyond row-level audit_log.

### §13.5 — Session 3 W1-S3+S4 starting anchors (kaizen-min first-commit)

When Session 3 W1-S3 fires, anchor enumeration to save grep time:
- **Anchor 1:** `crates/racecontrol/src/auth/privileged_actions.rs` — 12-variant enum at HEAD `2574ff18`. `ApproveRefundOverThreshold` line 128. Use as-is for Band C (≥₹3000) gate.
- **Anchor 2:** `crates/racecontrol/src/accounting.rs` lines 412 (`post_refund`) + 446 (`post_cash_refund`) — journal-entry primitives W1-S3 wrapper delegates to.
- **Anchor 3:** `crates/racecontrol/src/api/billing_invoice.rs::refund_billing_session` line 247 — session-scope handler W1-S3 wrapper delegates to.
- **Anchor 4:** `crates/racecontrol/src/api/wallet_ops.rs::refund_wallet` line 193 + `cash_refund_wallet` line 364 — wallet-scope handlers W1-S3 wrapper delegates to.
- **Anchor 5:** `crates/racecontrol/src/billing_pricing.rs::compute_refund` line 578 — calculation primitive (Captain Q-PRICE-3 FLOOR rule per `feedback_customer_satisfaction_first_minimal_compromise.md`). Re-use as-is.
- **Anchor 6:** `crates/racecontrol/src/accounting_audit.rs::log_audit` — audit-log persistence primitive. W1-S4 reason-code goes through here (A6.α).
- **Anchor 7:** `crates/racecontrol/src/api/routes.rs` — route registration site for new `POST /api/v1/refund/3band` endpoint. Verify role-gating after route added (CLAUDE.md "Route Uniqueness" + "Audit auth" rules).
- **Anchor 8:** `crates/racecontrol/src/billing_tests.rs` lines 284 + 1939-1957 — 6 existing `compute_refund` tests must stay GREEN post-W1-S3 wrapper authoring (no behavior change to primitives).

First W1-S3 commit (kaizen-min):
- Module skeleton at `crates/racecontrol/src/api/refund_routing.rs`
- `pub async fn route_refund(staff_pin, amount_paise, reason_code, session_id) -> Result<RefundOutcome>` signature
- 3-band match arm dispatch (Band A <₹1000 / Band B ₹1000-2999 / Band C ≥₹3000)
- 2-3 unit tests covering band-boundary cases (₹999 → A / ₹1000 → B / ₹2999 → B / ₹3000 → C)
- NO route registration yet — that's W1-S3 follow-up commit
- `pub enum RefundReason` with 5 variants (Captain Q2 disposition)

W1-S4 follow-up commit (same Session 3, second commit):
- `RefundReason::Other(String)` 100-char validation
- `log_audit` integration with reason-code in JSON payload
- Integration test: full flow `POST /api/v1/refund/3band` → routing → primitive call → audit_log row asserted
- Route registration in `api/routes.rs`

### §13.6 — NFs (new findings) surfaced this turn

- **NF-james-5 (refund-primitives-fan-out):** 6 existing refund functions across 4 files (`accounting::post_refund`/`post_cash_refund` + `billing_invoice::refund_billing_session`/`get_billing_refunds` + `wallet_ops::refund_wallet`/`cash_refund_wallet` + `billing_pricing::compute_refund`+variants). W1-S3 is wrapper-NOT-replacement; A4.α RATIFIED-PROVISIONAL preserves all existing tests untouched. Sibling-of NF-james-3 (doctrine-vs-physical-layout drift) — refund work continues A1.e disposition.
- **NF-james-6 (staff_auth.rs not-yet-exists):** Despite §1.1 row mapping W1-S6 component to `staff_auth.rs`, that file does NOT exist at HEAD `2574ff18`. Session 5 W1-S6 will be NEW-FILE creation under `crates/racecontrol/src/auth/`. Sibling-of NF-james-1 (PLAN-vs-disk-substrate drift; smaller-N).
- **NF-james-7 (audit_log table reuse for reason-code):** Existing `audit_log` table + `accounting_audit::log_audit` primitive cover reason-code persistence semantically (A6.α). Zero schema migration needed for W1-S4 — saves Session 3 budget on `cargo clean -p v2-db` cycle (per W1-S2 lesson `crates/v2-db/migrations/` `sqlx::migrate!` cache invalidation rule, racecontrol/CLAUDE.md). Composes-with kaizen-discipline (don't complicate process unless it has to be).

### §13.7 — Status

Pre-Session-3 anchor work landed. Session 3 spawn ready when:
- Captain Cognitive Load permits (per-PR auth gate stays at PR-open at end-of-Wave-1; pre-Session-3 anchor is in-band per Captain Option Bravo class-level V2-aligned auth)
- james-side branch hygiene + clean working tree on `feat/v2-wave-1-w1-s1-billing-service` (current `feat/v2-kiosk-wave-0a-fsm-foundation` working tree dirt + screenshots/handoffs untracked require pre-Session-3 cleanup pass)
- bono AMPLIFIER review on §13.2/§13.3/§13.4/§13.5 anchor dispositions welcomed at convenience (4-item AMPLIFIER list mirrors PACT-20260508-002 §G PROGRESS.md AMPLIFIER pattern — non-blocking for Session 3 spawn but useful for kaizen-N=2 PROMOTE candidacy).

— james / 2026-05-09 ~07:35 IST · §SUBSTANTIVE-PRE-SESSION-3 appended · A4.α (refund_routing.rs API-layer wrapper) RATIFIED-PROVISIONAL kaizen-N=1 · A5.α (RefundReason enum in same module) RATIFIED-PROVISIONAL kaizen-N=1 · A6.α (audit_log table reuse for reason-code persistence) RATIFIED-PROVISIONAL kaizen-N=1 · 3 NFs surfaced (NF-james-5/6/7) · 8 Session 3 anchors pinned · Verify-Before-Generate fired catching auth/staff_auth.rs absence + 6-fan-out refund primitives + audit_log reuse path
