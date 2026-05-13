# V2 Phase 2 Wave 2 — Dynamic Pricing + Cafe Extension + Combo Primitive + Rate-Transition UI — PLAN.md

**Slug:** phase-2-wave-2-dynamic-pricing-engine
**Class:** substrate (V2 critical-path; first wave that exposes time-windowed dynamic pricing + cafe surface + combo primitive + cross-organ rate-transition surfaces to customer)
**Status:** DRAFT-AWAITING-WAVE-1-LAND + CAPTAIN-G33-Q-DECISIONS — authored 2026-05-09 ~10:55 IST · scaffolding-only · NO implementation until Wave 1 PR merges + Captain dispositions Q-DECISIONs in §3
**LEAD:** bilateral — bono-cloud-LEAD on rate_table / cafe / combo schemas; james-venue-LEAD on billing calculator + session_addons + Kiosk countdown overlay (per WAVE-2-DESIGN-NOTES §3 lead split)
**AMPLIFIER:** bilateral — each pilot AMPLIFIES on the other's hemisphere; per-PR Captain merge auth gate fires at PR-open
**PACT:** PACT-DRAFT-phase-2-dynamic-pricing-engine (`comms-link/.planning/draft-pacts/`; commit `3da732a7`); PACT-DRAFT-phase-2-e-combo (commit `de642a12`); body update absorbs WAVE-2-DESIGN-NOTES + §S-92 P1-P4 + this PLAN at Wave 1 land-trigger
**Verify-by:** 2026-06-15 (kaizen-target) · 2026-06-30 + 60d soak gates V2.1 sliding-window pull-forward exit-condition (composes-with W1-S5 RCA)
**Composes-with:**
- racecontrol `.planning/specs/v2/WAVE-2-DESIGN-NOTES-20260508.md` (266-line design substrate; this PLAN executes its §2 scope split)
- racecontrol `.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` (Wave 1 model; this PLAN mirrors structure)
- comms-link `V2-MASTER-STATE.md` §S-82 + §S-83 + §S-84 + §S-91 §RATE-TABLE + §S-92 P1-P4 + §S-93 + §S-94 + §S-95 + §S-98 + §S-99
- comms-link `.planning/draft-pacts/PACT-DRAFT-phase-2-dynamic-pricing-engine.md` (`3da732a7`)
- comms-link `.planning/draft-pacts/PACT-DRAFT-phase-2-e-combo.md` (`de642a12`)
- racecontrol `CLAUDE.md` Standing Rules + Doctrine Conventions (Substrate-Pointer Convention; canonical pointers in §11)
- V2.0 6-wave plan (Captain §S-82) — this PLAN executes Wave 2
- Wallet-Framing-C LOCKED 2026-05-03 (Phase 2-A cafe surface stays separate from sim+PS5 wallet redemption)
- §AMEND-3.II live-rate doctrine (no snapshot — Phase 2-B reads tiers fresh on each call)
- §AMEND-4 kaizen-discipline (no speculative scaffolding for V2.1+ scope)
- W1-S5 RCA `15490644` (sliding-window idle-timeout pulled forward into Wave 1; if W1-S5 exits to V2.1, Wave 2 unaffected — orthogonal boundary)

---

## §1 — Wave 2 scope

Wave 2 turns Wave 1's static-rate billing engine into a **dynamic time-windowed pricing engine + cafe-extension + combo primitive + cross-organ rate-transition surfaces**. Wave 1 ships the substrate with §S-92 P1-P9 PARAMETERs as STATIC config; Wave 2 makes P1 (sim off-peak) + P2 (cafe off-peak) + P3 (combo offer) + P4 (rate-transition notification) DYNAMIC and customer-visible.

Activation trigger: PACT-DRAFT-phase-2-dynamic-pricing-engine FILE on Wave 1 land (per Wave 1 PLAN §1 + design memo §5).

### §1.1 — IN-scope (Wave 2 ships)

| # | Sub-step | Component | LEAD | Captain disposition anchor |
|---|---|---|---|---|
| W2-S1 | rate_table service + schema | New service: rate lookup keyed on `(surface, sku_or_class, window, customer_segment_optional) → effective_paise` | bono | §S-91 §RATE-TABLE; §S-92 P1+P2; design memo §2 Phase 2-A |
| W2-S2 | off_peak_windows table + initial Tue-Wed 1pm-4pm window | Schema: `window_id / surface / day_of_week / start_hh / end_hh / discount_pct` | bono | §S-92 P1; §S-84 dry-spell scenario |
| W2-S3 | Beverages catalog + `is_coffee_or_related` + NULLABLE cost_inr | Schema extension; cafe-extension surface | bono | §S-95 coffee-led doctrine; §S-89 disp #6 + §S-93 (Q-OE-7e/7f) |
| W2-S4 | Billing calculator: Way A flip + Strategy threading | F25b: `default_strategy()` `&SNAP_STRATEGY` → `&WAY_A_STRATEGY`; thread `&[BillingRateTier]` + `&dyn PricingStrategy` through 5 caller files (compute_session_cost / snap_debit_amount / 3 refund fns) | james | §AMEND-3.II live-rate; design memo §2 Phase 2-B |
| W2-S5 | session_addons schema + PS5 extra controller (per-session unit) | New schema: `addon_id / session_id / unit (per_session/per_hour/per_minute) / amount_paise / eligibility_window` | james | Q-OE-PS5-1=(a) §S-93 |
| W2-S6 | Per-minute round-up rounding helper | `round_up_to_minute(elapsed_seconds)` shared with Phase 2-C (Wave 3) | james | §S-92 P5 |
| W2-S7 | combo_offers + combo_offer_items schema + redemption flow | Schema authoring (bono); billing-side application atomic at session-create OR cart-add (james) | bono schema · james billing-side | §S-92 P3 explicit-combo-id only; §S-95 coffee-paired first-class type |
| W2-S8 | PWA countdown banner (in-pod) | "Off-peak ends in 12 min — sim ₹630/hr" banner; <15min-before / <15min-end transition states | bono | §S-92 P4 PWA in-pod surface; design memo §2 Phase 2-D |
| W2-S9 | Kiosk countdown overlay (web-v2 component) | Overlay during transition window (last 5 min of off-peak) with confirm-or-cancel CTA; consumes `OffPeakWindowEvent { window_id, transition_state, seconds_remaining }` via WS | james | §S-92 P4 kiosk overlay surface |
| W2-S10 | Viability tracking event emission | `BillingSessionEnded { customer_id, surface, gross_paise, discount_paise, net_paise, ts }` to canonical RP store | james | §S-98 racecontrol DB primary canonical; Wave 6 break-even ₹15,400/day rolling 30-day gate |

### §1.2 — Surfaces explicitly NOT in Wave 2 scope (Wave 3+ or V2.1)

- Wallet HOLD-RELEASE-CAPTURE state machine + idempotency cascade — Wave 3 (Phase 2-C; PACT-024 sibling)
- MI-adaptive discount tiers (P6 5-tier) — Wave 4
- Captain-curated dry-spell WhatsApp campaigns — Wave 5
- Per-category combo bundling (combo_offers without explicit combo_id) — V2.1+ (P3 explicit-only at V2.0)
- Coffee-equipment investment scope (machine upgrade / barista training / bean sourcing) — V2.1+ (Q-OE-COFFEE-3)
- External substrate integration (post-§S-98 SRL VMS removal reopen) — V2.1+
- Telugu/Teluglish reactive WhatsApp templates — V2.1+ (per §S-99 + Captain 2026-05-08 ~06:13 IST scope-pin)
- Customer-segment-dependent rate lookup — V2.1+ (rate_table schema accommodates `customer_segment_optional` field for forward-compat; segment dispatch deferred)

---

## §2 — Session sequencing (estimated 6-8 sessions; mirrors Wave 1 ~7-session cadence)

**Session 1** (~2 hours; bilateral)
- W2-S1 rate_table service skeleton (bono) + W2-S2 off_peak_windows schema (bono)
- W2-S4 F25b billing calculator default_strategy flip (james — separate PR; pre-soaked from Wave 1 backlog)
- Unit tests: rate lookup boundary; SNAP→WAY_A strategy flip preserves Vivek anchor

**Session 2** (~1.5 hours; bono parallel-track + james parallel-track)
- W2-S3 beverages_catalog cafe-extension (bono)
- W2-S5 session_addons schema (james)
- Unit tests: beverages_catalog `is_coffee_or_related` flag persistence; session_addons unit-type round-trip

**Session 3** (~2 hours; james-LEAD)
- W2-S6 per-minute round-up rounding helper (extracted; shared with Phase 2-C Wave 3 sibling)
- W2-S10 viability-tracking BillingSessionEnded event emission (canonical RP store wiring)
- Unit tests: round_up_to_minute boundary cases; emit-and-receive integration

**Session 4** (~2 hours; bono-LEAD)
- W2-S7a combo_offers + combo_offer_items schema (bono)
- gates on Q-OE-COFFEE-1 Captain disposition (coffee SKU roster — gates coffee-paired combo authoring)
- Unit tests: combo schema migrations; coffee-paired flag persistence

**Session 5** (~2 hours; james-LEAD billing-side; consumes Session 4 schema)
- W2-S7b combo redemption flow (billing-side atomic application of bundle_price_paise)
- Q-COMBO-1 disposition wired (combo wins over off-peak per bono recommendation; Captain ratify check)
- Integration test: combo at session-create + active off-peak window = combo price applies (no double-discount)

**Session 6** (~2 hours; bilateral)
- W2-S8 PWA countdown banner (bono cloud-LEAD; PWA lives on cloud per V2-skeleton)
- W2-S9 Kiosk countdown overlay (james venue-LEAD; web-v2 component on bono VPS but UI authored james-side)
- Surface contract: rate_table service emits `OffPeakWindowEvent` via WS — wired both consumers
- Playwright E2E: PWA + Kiosk both render countdown correctly during transition window

**Session 7** (~1.5 hours; bilateral integration)
- Full-flow integration: session-start during off-peak → live-rate-per-minute applied (Q-BILL-1 boundary case) → session crosses off-peak boundary → rate transitions correctly per minute
- PS5 extra controller add-on at session-create (Q-BILL-2 disposition: session-create timing per bono recommendation)
- Cafe order during off-peak: -20% discount badge surfaces on POS .130; receipt shows pre-discount + discount + final
- Contract tests: rate_table cache invalidation strategy (Q-RATE-1) — pull-with-5min-refetch per BOOT-02 default
- Off-peak window boundary edge cases (Q-RATE-2): per-minute live-rate continuous resolution

**Session 8** (~1.5 hours; bilateral)
- Quality gate: `bash test/run-all.sh` 4/4 + Wave 2 specific contract tests
- MMA pre-ship VERIFY (≥3-model adversarial via OpenRouter primary; score ≥4.0 PASS gate; mandatory per CLAUDE.md cross-system bridge rule — Wave 2 introduces NEW POS/PWA/Kiosk surfaces consuming new WS event type)
- DEPLOY MANIFEST refresh (per Wave 1 model): rate_table service / billing calculator / web-v2 build / PWA build / WS contract additions
- Visual verification per CLAUDE.md Ultimate Rule #4: customer-visible countdown on PWA + Kiosk + cafe discount badge on POS
- PR-open per phase (per-PR Captain merge auth gate fires here for EACH of: Phase 2-A / Phase 2-B / Phase 2-D / Phase 2-E)

---

## §3 — Captain-reserve items still open at start of Wave 2

| ID | Question | Default if Captain doesn't disposition | Gates which sub-step |
|---|---|---|---|
| Q-OE-COFFEE-1 | Coffee SKU roster — specific SKUs in V2.0 menu (espresso / cappuccino / latte / cold coffee / mocha / flat white / americano / others?) | bono-default 10-SKU broader-roster + Hazelnut V2.1+ (per §S-95 + segment-G/H absorption) | W2-S7 (Phase 2-E coffee-paired combo authoring) |
| Q-RATE-1 | rate_table cache invalidation strategy — push (config_push) vs pull (5-min refetch per BOOT-02)? | bono-recommendation: pull (5-min refetch sibling to allowlist + flags BOOT-02 pattern) | W2-S1 |
| Q-RATE-2 | off-peak window edge cases — session that crosses window boundary, what rate applies? | bono-recommendation: live-rate-per-minute (rate at minute N's wall-clock determines minute N's cost) | W2-S1 + W2-S4 |
| Q-BILL-1 | session that spans off-peak window boundary — which rate applies per minute? | bono-recommendation: live-rate-per-minute (unambiguous per Q-RATE-2 sibling) | W2-S4 |
| Q-BILL-2 | PS5 add-on charged at session-create vs session-end? | bono-recommendation: session-create (atomicity; treat as immediate top-up debit) | W2-S5 |
| Q-COMBO-1 | combo redemption with active off-peak — combo price stands or off-peak applies on top? | bono-recommendation: combo wins (otherwise off-peak undercut paradox) | W2-S5 + W2-S7 |
| Q-OE-COFFEE-2 | 60/40 coffee-led vs other messaging weight (Wave 5 marketing scope) | bono-default 60-40 | NOT-Wave-2 (Wave 5; flagged here for cross-wave context) |
| Refined ₹4.62L breakdown current actuals | Captain provides operating-expense baseline current numbers | bono-default §S-92 P9 ₹4.62L/month placeholder | NOT-Wave-2 directly (Wave 6 viability anchor; flagged here for cross-wave context) |

**Per-PR Captain merge auth gate** (PROMOTED bilateral N=1+ from Wave 1) — fires at PR-open for EACH Wave 2 phase PR (2-A / 2-B / 2-D / 2-E); not blocking authoring; not transitive across PRs

---

## §4 — Open architectural decisions

**A1 (bono-LEAD): rate_table service deployment surface**

Two options:
- **A1.a** New service binary on bono VPS with HTTP API consumed by racecontrol billing calculator
- **A1.b** Module within racecontrol crate; `rate_table::lookup(...)` Rust function call (no network hop)

Recommended (kaizen-min): **A1.b** for Wave 2; A1.a if Wave 4 MI experience-score consumer materializes that requires rate_table read from cloud-side Rust service. Defer A1.a until that consumer is concrete.

**A2 (james-LEAD): combo redemption integration point**

Two options:
- **A2.a** Apply combo at session-create (cart-style — staff selects combo SKU; bundle_price atomic debit)
- **A2.b** Apply combo at session-end reconcile (post-hoc detection — sum components, replace with bundle_price if combo_id matches)

Recommended: **A2.a** for V2.0 (deterministic; staff explicit selection; matches P3 explicit-combo-id-only). A2.b is V2.1+ if auto-detection becomes valuable.

**A3 (cross-pilot): WS event protocol for OffPeakWindowEvent**

bono recommends extending existing FlagSync WS channel pattern (sibling to `ConfigPushQueue` Phase 177); james AMPLIFIER on protocol additions before Session 6 PWA + Kiosk consumer wiring.

**A4 (cross-pilot): viability tracking event sink (W2-S10)**

Options:
- **A4.a** Direct INSERT into racecontrol DB `billing_session_summary` table (canonical per §S-98)
- **A4.b** Emit through comms-link bus → bono cloud sink for Wave 6 dashboard surface

Recommended: **A4.a** for Wave 2 (kaizen-min; primary canonical = racecontrol DB); A4.b is Wave 6 dashboard surface scope.

---

## §5 — Test plan

### §5.1 — Unit tests

Per W2-S* sub-step (see §1.1); see PACT-DRAFT-phase-2-dynamic-pricing-engine §3 + PACT-DRAFT-phase-2-e-combo §3 for full enumeration when those PACTs FILE.

### §5.2 — Integration tests

- **Off-peak window crossing:** session starts at 12:50 IST (standard rate), crosses 13:00 IST off-peak window start → minute 1-10 at ₹900/hr → minutes 11+ at ₹630/hr; reconcile produces correct final paise
- **Cafe order during off-peak:** POS .130 staff scans cafe item → -20% discount badge surfaces → receipt prints pre-discount + discount + final correctly
- **Combo at session-create + active off-peak:** combo_id selected → bundle_price applied atomically → off-peak does NOT undercut combo (Q-COMBO-1 disposition validated)
- **PS5 extra controller add-on:** session-create → addon `unit=per_session amount_paise=20000` debits at create time (Q-BILL-2 disposition validated)
- **Per-minute live-rate continuous resolution:** session that starts 12:55 (5 min standard) + crosses 13:00 (off-peak start) + ends 13:30 (25 min off-peak) — total = (5min × ₹900/60) + (25min × ₹630/60); each minute resolves at its own wall-clock rate

### §5.3 — Contract tests

- rate_table service `(surface, sku_or_class, window, customer_segment_optional)` lookup returns `effective_paise` deterministically; null customer_segment falls through to base rate
- combo_offers schema: explicit-combo-id present, NULL allowed for `is_coffee_paired` (default false), `bundle_price_paise` NOT NULL
- session_addons unit type enum: `per_session | per_hour | per_minute` round-trip
- OffPeakWindowEvent WS contract: `window_id, transition_state ∈ {starting, active, ending}, seconds_remaining` shape preserved cross-pilot

### §5.4 — Playwright E2E (POS .130 + PWA in-pod + Kiosk)

- POS staff scans cafe item during Tue 13:30 IST (active off-peak Tue-Wed 1pm-4pm) → -20% badge surfaces → receipt prints correct math
- PWA in-pod: customer at minute 47 of off-peak window sees countdown "Off-peak ends in 12:53" → updates per-second
- Kiosk overlay: customer in pod during last 5min of off-peak window sees confirm-or-cancel CTA → confirm extends session at off-peak rate (Q-COMBO-1 sibling: rate at moment-of-confirm)
- Combo at POS: staff selects "Sim 2hr + coffee + cookie" combo → bundle_price applies; receipt shows combo line item not 3 separate items

### §5.5 — MMA pre-ship VERIFY (mandatory per UNIFIED-MMA-PROTOCOL v4.0 + cross-system bridge rule)

- ≥3-model adversarial verification (different from any DIAGNOSE/PLAN models if used)
- Score ≥4.0 PASS gate
- Run via OpenRouter primary path (Captain directive 2026-05-01)
- Probe surfaces: rate_table cache invalidation under config-change concurrent-session race; OffPeakWindowEvent WS event emission under partial-disconnect; combo redemption under concurrent staff selection (two staff select different combos for same session); per-minute live-rate boundary precision under clock-skew

### §5.6 — Quality gate

`bash test/run-all.sh` (4 suites: contract + integration + syntax + security); exit 0 unblocks PR-open; non-zero BLOCKED.

---

## §6 — Deploy targets (per CLAUDE.md DEPLOY PARITY rule)

| Target | Component | Verification probe |
|---|---|---|
| Server .23 | racecontrol binary (cargo build --release; deploy-server.sh) | curl `:8080/health` + behavior probe (real session crossing off-peak boundary; cafe order during off-peak; combo redemption) |
| Pods 1-8 | rc-agent (no Wave 2 component on pods directly; verify ws_connected stays True; OffPeakWindowEvent WS subscription test) | `/api/v1/fleet/health` 8/8 ws_connected + WS event receive log |
| POS .130 | web-v2 build (npm run build + nginx reload) — cafe discount badge surface; combo selection UI | Playwright: cafe order during off-peak shows badge; combo selection applies bundle_price |
| James .27 | dev environment parity (cargo build local; web-v2 dev server) | Playwright local + cargo test |
| Bono VPS | cloud racecontrol parity (deploy_pull) + PWA build (countdown banner component) + admin dashboard rate_table editor (forward-compat) | curl `:8080/health` + WhatsApp send-text test (no Wave 2 WhatsApp component but verify regression-free); PWA Playwright countdown render |
| Cloud apps | PWA on Bono VPS (countdown banner Wave 2 customer surface) | health 200 + ws=True + countdown renders during transition window |
| Comms-link | shared/protocol.js — OffPeakWindowEvent WS event type addition | `bash test/run-all.sh` 4/4 |

**Behavior verification (NOT just health-200):** customer in pod during off-peak window sees countdown banner update per-second; staff scans cafe item during off-peak and sees discount badge; combo redeemed at POS produces single bundle line item on receipt.

---

## §7 — Cross-pilot coordination

| Phase | LEAD | AMPLIFIER | Notes |
|---|---|---|---|
| PHASE-2-WAVE-2-PLAN.md scaffold (this file) | james (this file authoring) | bono substantively extends post-AMPLIFIER | Mirrors Wave 1 PLAN structure |
| W2-S1+S2+S3 (rate_table + cafe-extension schemas) | bono cloud | james AMPLIFIER on contracts + tests | Cloud-side service per §3 lead split |
| W2-S4 (F25b Way A flip) | james venue | bono AMPLIFIER | Rust strategy trait threading; Vivek anchor preservation |
| W2-S5+S6 (session_addons + per-minute helper) | james venue | bono AMPLIFIER on schema | Adjacent to billing_session table |
| W2-S7 (combo schema + redemption) | bono schema · james billing-side | bilateral | Schema bono-side; redemption application james-side |
| W2-S8 (PWA countdown banner) | bono cloud | james AMPLIFIER on UI/UX | PWA on cloud per V2-skeleton |
| W2-S9 (Kiosk countdown overlay) | james venue | bono AMPLIFIER on WS event consumer wiring | web-v2 component on Bono VPS but UI authored james-side |
| W2-S10 (viability-tracking event emission) | james venue | bono AMPLIFIER on event sink wiring | Phase 2-B emits; Wave 6 dashboard consumer |
| MMA pre-ship VERIFY | either pilot | the other AMPLIFIER | OpenRouter primary path |
| PR-open | per-phase LEAD | the other pilot AMPLIFIER on review | Per-PR Captain auth gate fires here for EACH phase PR |
| Deploy + verify | bilateral | bilateral | DEPLOY PARITY mandatory all 7 targets |

### §7.1 — bono parallel-track during james authoring window (and vice versa)

While james authors W2-S4+S5+S6+S9+S10 substrate, bono parallel-tracks:
- W2-S1+S2+S3 rate_table + off_peak_windows + beverages_catalog schema authoring
- W2-S7a combo_offers + combo_offer_items schema authoring (gates on Q-OE-COFFEE-1)
- W2-S8 PWA countdown banner component
- MI experience-score ingestion module continued DRAFT (Wave 4 prep — not Wave 2 ship)
- Captain-curated WhatsApp workflow framework continued DRAFT (Wave 5 prep — not Wave 2 ship)

While bono authors W2-S1+S2+S3+S7a+S8, james parallel-tracks:
- W2-S4 F25b Way A flip (separate workstream — pre-soaked from Wave 1 backlog; can proceed in parallel)
- W2-S5+S6 session_addons + per-minute helper
- W2-S9 Kiosk countdown overlay (gates on bono WS event protocol per A3)
- W2-S10 viability-tracking event emission

---

## §8 — Stale-at conditions

- Wave 1 has NOT landed (PACT-001 Phase 1 wire-up PR merge) — Wave 2 PR-open BLOCKED until Wave 1 ships per V2.0 6-wave plan §S-82
- Captain CHALLENGE-AMEND on PACT-DRAFT-phase-2-dynamic-pricing-engine OR PACT-DRAFT-phase-2-e-combo within their 24h L1 charter windows (silent-expire computed from FILE-conversion timestamps)
- Captain CHALLENGE-AMEND on §S-92 P1-P4 / §S-93 / §S-94 / §S-95 within 24h windows (silent-expire 2026-05-09 ~04:22 IST onwards — most have expired; but late CHALLENGE-AMEND would ripple)
- Captain disposes any Q-DECISION in §3 (Q-OE-COFFEE-1 / Q-RATE-1+2 / Q-BILL-1+2 / Q-COMBO-1) — disposition either confirms bono-recommendation default (no PLAN ripple) or contradicts (sub-step impl ripple)
- Verify-by 2026-06-15 (kaizen-target) — if Wave 2 not Session-8-complete by 2026-06-15, escalate
- bono substantively extends this PLAN (§7 row 1) — mirrors Wave 1 PLAN bono→james extend pattern; james-AMPLIFIER absorption may add §SUBSTANTIVE-REPLY section
- W1-S5 RCA outcome shifts (e.g., Captain Q-RECONCILE-1 disposes "defer sliding-window to true V2.1") — Wave 2 unaffected (orthogonal auth boundary), but flag for cross-wave doctrine alignment
- Captain G33-AUTH on per-PR merge auth gate (per-phase basis)

---

## §9 — Session metrics tracking

Per CGP v4.3 + Wave 1 PLAN §9 model:
- **Claims** — N (track per session)
- **Corrections** — N (track per session)
- **FCR** — % (false claim rate)
- **G9s** — N (target: 0)
- **UCAs** — N (Unenumerated Coverage Assertions; target: 0)
- **MMA budget** — track per Step 1/2/4 invocation (default $5/session unless Captain approves)

---

## §10 — Scaffolding-only scope (this PLAN as authored)

This PLAN is a **scaffolding artifact** — it documents Wave 2 scope, sequencing, leads, tests, and deploy targets but does NOT initiate any source-code change. Per CGP H1 + V1-dependent V2 RCA doctrine + Captain per-PR merge auth gate:

- **NO source code authoring** until: (1) Wave 1 PR merges (activation_trigger) + (2) Captain dispositions §3 Q-DECISIONs + (3) per-phase per-PR Captain auth at PR-open
- **NO PACT-FILE conversions** of the 2 PACT-DRAFTs (`3da732a7` + `de642a12`) until Captain G33-CONFIRM-FILE
- **NO new branches** — this PLAN lives on `feat/v2-wave-1-w1-s1-billing-service` (precedent: WAVE-2-DESIGN-NOTES + W1-S5-RCA both landed here as planning artifacts)
- **NO scaffolding of cross-pilot bono hemisphere components** (rate_table service, beverages_catalog schema, combo_offers schema, PWA countdown banner) — bono is LEAD on those per §3 lead split; james-side scaffolding here is documentation-only
- **NO MMA Step 1 DIAGNOSE invocation** — Wave 2 doesn't yet have substrate to diagnose; MMA pre-ship VERIFY is Session 8 scope

This PLAN unblocks Wave 2 SESSION-1 START as soon as Wave 1 lands AND Captain dispositions §3 Q-DECISIONs.

---

## §11 — Canonical sources (Substrate-Pointer Convention per CLAUDE.md Doctrine Conventions)

- **§S-92 PARAMETERs P1-P9** — `comms-link/V2-MASTER-STATE.md` §S-92 (Captain explicit 2026-05-08 ~04:22 IST)
- **§RATE-TABLE** — `comms-link/V2-MASTER-STATE.md` §S-91 (canonical rate table)
- **Wave 2 design substrate** — `racecontrol/.planning/specs/v2/WAVE-2-DESIGN-NOTES-20260508.md` (266-line memo)
- **PACT-DRAFT bodies** — `comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-dynamic-pricing-engine.md` (`3da732a7`) + `PACT-DRAFT-phase-2-e-combo.md` (`de642a12`)
- **Wallet-Framing-C** — memory `project_v2_wallet_framing_c_locked_20260503.md`
- **§AMEND-3.II live-rate doctrine** — F25a substrate; canonical at PR #63 merge
- **6-wave plan** — `comms-link/V2-MASTER-STATE.md` §S-82 + §S-83 + §S-85
- **Wave 1 PLAN** — `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md`
- **W1-S5 RCA** — `racecontrol/.planning/specs/v2/W1-S5-RCA.md` (orthogonal-boundary cross-wave check)
- **CLAUDE.md DEPLOY PARITY + DMP** — `racecontrol/CLAUDE.md` Standing Rules

---

## §12 — Verification gates this PLAN asserts

Per CGP H3 — what this PLAN CAN claim vs what is NOT TESTED:

**This PLAN asserts:**
- Wave 2 scope is documented per §S-92 P1-P4 + §S-93 + §S-94 + §S-95 + §S-98 (every locked PARAMETER cited verbatim from canonical substrate)
- Lead split is documented per WAVE-2-DESIGN-NOTES §3 (resolves cross-pilot ambiguity before Wave 2 PR-open)
- Captain-reserve Q-DECISIONs are enumerated for Wave 2 PR-open (8 items in §3 with bono-recommendation defaults)
- Composes-with W1-S5 RCA (sliding-window pull-forward) + Wave 1 PLAN + design memo + 2 PACT-DRAFTs
- Scaffolding-only scope (§10) — NO code, NO PACT-FILE, NO new branch

**NOT TESTED (this PLAN does not assert):**
- bono concurs with this PLAN (gates on bono AMPLIFIER vote OR Wave 2 PR-open per-PR)
- Schema details survive MMA Cross-System Bridge audit (mandatory at Phase 2-A PR-open per CLAUDE.md)
- Cargo-test of any Wave 2 sub-step (gates on Wave 1 land + Session 1 START)
- Captain dispositions on §3 Q-DECISIONs (default-AGREE windows not yet applicable; PLAN assumes bono-recommendation defaults until Captain disposes)
- Q-OE-COFFEE-1 coffee SKU roster (gates W2-S7)
- DEPLOY PARITY readiness — per-phase DEPLOY MANIFEST will be authored at PR-open (not in this PLAN scope)
- PR-open count — Wave 2 may decompose into 4 PRs (Phase 2-A / 2-B / 2-D / 2-E) OR fewer per james architectural call at Session 1 START

---

— james-venue / 2026-05-09 ~10:55 IST · Wave 2 PLAN scaffolding · DRAFT-AWAITING-WAVE-1-LAND + CAPTAIN-G33-Q-DECISIONS · mirrors Wave 1 PLAN structure · references WAVE-2-DESIGN-NOTES + 2 PACT-DRAFTs + §S-92 P1-P4 LOCKED PARAMETERs · scaffolding-only scope per §10 · NO code · NO PACT-FILE · NO new branch · per-PR Captain merge auth gate STANDS at each Wave 2 phase PR-open
