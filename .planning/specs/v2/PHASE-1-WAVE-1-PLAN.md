# V2 Phase 1 Wave 1 — Static Billing Engine — PLAN.md

**Slug:** phase-1-wave-1-static-billing-engine
**Class:** substrate (V2 critical-path; first wave that ships a working economic transaction through V2-DB)
**Status:** SKELETON-PLAN-DRAFT — bono-authored 2026-05-08 ~19:05 IST; james substantively extends per implementation discoveries when next session opens
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
