---
artifact: §S-146 V1↔V2 RCA
row: V2-PROGRESS-MAP §2 W2 Phase 2-A — rate_table service substrate (Layer 10.12)
status: AUTHORED-AWAITING-CAPTAIN-DISPOSITION
authored: 2026-05-13 IST
author: james
boundary-class: foundational (wallet/billing rate-resolution; load-bearing for ALL session debits via Phase 2 dynamic-pricing engine + sibling Phase 2-E combo + RCA-row-7.3 ceiling + RCA-row-1.20 deeper-of)
mma-step-1: PENDING-Captain-auth (foundational-boundary escalation; cross-organ bono cloud authority + james venue consumer)
parent-cascade: §S-248 9-RCA batch follow-up — explicitly named prerequisite for RCA #9 dynamic-pricing engine (`RCA-2026-05-13-phase-2-dynamic-pricing-engine.md` §4.E + §5 Phase 1) + RCA #8 combo-offer (`RCA-2026-05-13-phase-2-e-combo-offer-primitive.md` §1 + §5 Phase 1)
gaps-closed: 1 (G-Phase-2-A-1 NEW-MECHANISM-CLASS rate_table service: rate_windows table + /v2/rates/* endpoints + cache layer + seed config from §S-91 §RATE-TABLE + event emission to MI + billing calculator)
customer-day-beat: 14:00 pricing-floor lookup (canonical day §4 — every session-start + every per-minute boundary tick queries `effective_rate(surface, timestamp)`); also feeds 14:10-14:50 dynamic pricing on customer-day-beat
captain-decisions-needed: composes-with bono PACT-DRAFT-phase-2-a-rate-table-service.md Q-A1..Q-A4 (4 open questions at FILE-time with bono defaults proposed)
companion-pact: `comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-a-rate-table-service.md` (bono-LEAD engineering spec; DRAFT-AWAITING-WAVE-1-LAND; activation_trigger = Wave 1 SessionBillingService + refund routing + idle-timeout + PIN-LOCKOUT ship + verify-by 2026-05-21)
---

# §S-146 V1↔V2 RCA — Phase 2-A rate_table service substrate

## 1. Boundary map

Phase 2-A NEW-MECHANISM-CLASS: cloud-side authoritative `rate_table` service answering `effective_rate(surface, timestamp)` queries for every V2 rate-eligible surface (sim / ps5 / cafe-as-relevant). This is the load-bearing substrate that Phase 2 dynamic-pricing engine (RCA #9 in §S-248 batch) reads via `crates/racecontrol/src/pricing/rate_table.rs` (planned per RCA #9 §4.A module structure) and all 4 sibling discount/composition RCAs (1.20 deeper-of · 7.3 ceiling · Phase 2-E combo · Phase 2 engine) compose into.

**G-Phase-2-A-1: rate_table cluster substrate (NEW-MECHANISM-CLASS)**

§S-219.11 finding propagation: RCA-row-Phase-2 (RCA #9) names Phase 2-A as DEPENDS-ON; RCA-row-Phase-2-E (RCA #8) §1 names Phase 2-A as ship-sequence prerequisite. This RCA is the §S-146 V1↔V2 gate for the Phase 2-A substrate-PR (engineering plan lives in companion PACT-DRAFT; bono-LEAD).

**Grep verification** (current main as of 2026-05-13 ~15:42 IST commit `d8e493d6`):
- `rg "rate_table\b|rate_windows\b|resolve_rate\b" crates/` → **0 hits**
- `rg "effective_rate\b" crates/` → **0 hits**
- `rg "/v2/rates" crates/racecontrol/src/api/` → **0 hits**

V1 adjacent recommendation-only substrate exists but does NOT provide rate resolution:
- `crates/racecontrol/src/dynamic_pricing.rs:7-18` — `PricingRecommendation` struct + `recommend_pricing()` fn; **recommendation generation**, NOT applied engine; produces advice for staff, not authoritative rate
- `crates/racecontrol/src/pricing_bridge.rs:2` — comment-only stub *"Prices computed by dynamic_pricing.rs are proposed → approved → applied to all channels"* — bridge not implemented
- `crates/racecontrol/src/scheduler_analytics.rs:66-95` — `peak_hours` / `off_peak_hours` analytics + `pricing_suggestion` string; analytics view layer, NOT engine
- `crates/racecontrol/src/maintenance_checks.rs:78` — `is_peak_hours()` predicate-only, used for maintenance gating not pricing
- `crates/racecontrol/src/pricing_engine.rs` — V1 stub; reviewed at grep — implements V1 SnapPricing class-A pricing strategy per F25a doctrine (CLAUDE.md F25a); NOT the V2 effective-rate engine

**Canonical seed config sources** (Captain-LOCKED — RCA design must preserve):
- §S-91 §RATE-TABLE — pricing & capacity binding rule; V2 modules MUST reference §RATE-TABLE (NOT private copies)
- §S-92 P1 sim -30% off-peak weekday 11:00-16:00 + P2 cafe -20% off-peak weekday 11:00-16:00
- §S-101 GST-INCLUSIVE V2.0 doctrine — cross-cutting (sim/PS5/cafe/wallet); rate_value_paise semantically GST-inclusive
- §S-109 10-SKU coffee menu CANONICAL bundleable universe at MRP
- §S-104 Class A coffee doctrine — `is_promoted=true` is MESSAGING-layer flag, NOT price-discount flag
- §S-108 Q-RATE-1 (cache invalidation = push `config_push`) + Q-RATE-2 (window boundary edge = live-rate-per-minute continuous) CONCUR-AGREE
- Joint #4 dynamic pricing locked spec — broadcast-only / non-personalized / deeper-of-two; service signature `effective_rate(surface, [surface_sku,] timestamp)` excludes customer parameter
- Joint #2 Billing 2-second tolerance — rate-window boundary ticks respect ±2s drift

**Contract test scaffolded:** `racecontrol/tests/contract/phase-2-a-rate-table.spec.ts` — **NOT YET AUTHORED** (§S-249 prereq cascade did not scaffold its own test; substrate-PR ships the test alongside). Per §S-221 F1 SCOPE GATE: env-gated SKIP-with-reason `V1_NO_RATE_TABLE_PRIMITIVE` until substrate ships.

## 2. Inherited-issue catalogue

| # | V1 class | Surface | Source |
|---|---|---|---|
| I-1 | **dynamic_pricing.rs recommendation-only** — V1 generates recommendations; never applies them; pricing_bridge.rs is comment-only stub | grep `dynamic_pricing.rs:7-18` + `pricing_bridge.rs:2` | §S-219.11 + RCA #9 §2 I-1 |
| I-2 | **No rate-resolution authority for cross-channel pricing** — kiosk/PWA/POS each derive prices from their own client-side config; race condition risk when admin updates one channel and forgets others | inferred from `pricing_engine.rs` SnapPricing scope (F25a) | F25a doctrine + Joint #4 |
| I-3 | **F25a SnapPricing strategy** — V1 SnapPricing class-A is substrate; V2 must preserve as known strategy with HISTORICAL block per `racecontrol/CLAUDE.md` F25a doctrine; behavior-parity test required at engine integration | `pricing_engine.rs` + F25a CLAUDE.md entry | RCA #9 §2 I-4 |
| I-4 | **Hard-coded prices in scheduler_analytics + maintenance_checks** — analytics modules treat ₹900/₹500 as constants for staff display; V2 must source these from rate_windows seed not hard-code | `scheduler_analytics.rs:66-95` + `maintenance_checks.rs:78` | grep |
| I-5 | **Captain Q-2-1..6 pricing-engine doctrine + 4 bono AMPLIFIER-ASKs** — §S-218 + bono Phase 2-D msg=36346 + Phase 2-C msg=36347 + Phase 2-G msg=36349 + Phase 2-F msg=36341 — open dispositions for pricing-engine surrounding policy | doctrine class — Q-2-1 already in §S-248 batch RCA #7 ceiling-value | §S-211 outbound queue |
| I-6 | **§S-91 §RATE-TABLE canonical NOT enforced at code layer** — V1 modules may load prices from local config files instead of §RATE-TABLE seed; risk of canonical-substrate drift if V2 rate_table seed is permitted to deviate at init | `dynamic_pricing.rs` + scattered pricing constants | §S-91 binding rule |
| I-7 | **Wave 1 W1-S6 billing-calculator parallel work (bono-LED Phase 2-B)** — `comms-link/.planning/draft-pacts/PHASE-2-B-BONO-CLOUD-SURFACES-REFERENCE.md` (commit abbf52a8) ships 3 surfaces + 4 wallet-client ops + 1 WS subscription; rate_table service consumer interface must align with this | Phase 2-B consolidated index | bono Phase 2-B reference |
| I-8 | **sqlx::migrate cache invalidation discipline (Captain doctrine 2026-05-08 ~22:01 IST)** — adding new .sql files in `crates/v2-db/migrations/` requires `cargo clean -p v2-db` before test run; rate_windows migration triggers this for v2-db crate | doctrine class — RCA-row-1.13 + RCA #9 noted | Captain doctrine 22:01 IST |
| I-9 | **SQLite ALTER TABLE RENAME FK rewriting (Captain doctrine 2026-05-08 ~22:01 IST)** — any future migration recreate-tables `rate_windows` will rewrite ALL sibling-table FK clauses (`session_rate_snapshots.rate_id`, `campaigns.rate_window_id`); sibling-rebuild required in same migration | doctrine class | Captain doctrine 22:01 IST + PACT-DRAFT-2-A §4 FK-rewriting awareness |
| I-10 | **Cross-organ boundary contract — bono cloud authority on rate_table; james venue consumer at Phase 2-B billing-calculator** — wrong-side state write risks if james venue accidentally writes rate_windows directly instead of querying via Phase 2-A REST | §S-49.3 surfaces table | PACT-DRAFT-2-A §12 authority basis |

## 3. Past-bug review

| # | Issue | Disposition |
|---|---|---|
| I-1 | dynamic_pricing.rs recommendation-only | **PATCHED-ONLY** — V1 module retained for recommendation generation; V2 rate_table service is parallel substrate that APPLIES rates; no V1 retirement needed |
| I-2 | No rate-resolution authority cross-channel | **NOT-APPLICABLE-TO-V2** — V2 rate_table service IS the cross-channel authority; broadcast-only Joint #4 ensures all channels receive same rate per (surface, time) tuple |
| I-3 | F25a SnapPricing | **ROOT-CAUSED-AND-FIXED 2026-03-28** — V2 engine (RCA #9) preserves SnapPricing strategy with HISTORICAL block; behavior-parity test enforced at engine integration; rate_table service supplies the BASE rate that SnapPricing then transforms |
| I-4 | Hard-coded prices in scheduler_analytics + maintenance_checks | **PATCHED-BY-DESIGN** — V2 substrate-PR refactors scheduler_analytics + maintenance_checks to consume rate_windows via Phase 2-A REST (or direct DB read if same-host); analytics-only client-side caching retained |
| I-5 | Captain Q-2-1..6 + bono AMPLIFIER-ASKs | **DEPENDS-ON** — engine substrate-PR drafts gate on Captain answers; sibling decision queue handled per-Q; Q-2-1 ceiling-value RCA-row-7.3 explicitly composes here |
| I-6 | §S-91 binding not code-enforced | **PATCHED-BY-DESIGN** — V2 substrate-PR encodes §S-91 seed via migration `policy_tag` prefix `S91-` (per PACT-DRAFT-2-A §6 seed config); init path REJECTS missing/altered seed via PRAGMA check |
| I-7 | Phase 2-B billing-calculator parallel | **COMPOSE-WITH** — bilateral sync at PR-author time; rate_table service emits `rate_resolved` event to billing calculator (Phase 2-B consumer); consumer interface alignment verified at FILE-time |
| I-8 | sqlx::migrate cache invalidation discipline | **NOT-A-BUG** — established discipline; PACT-DRAFT-2-A §13 step 9 mandates `cargo clean -p v2-db` in FILE-conversion checklist |
| I-9 | SQLite RENAME FK rewriting discipline | **NOT-A-BUG** — established discipline; PACT-DRAFT-2-A §4 FK-rewriting awareness + §13 step 10 `grep -rn "REFERENCES rate_windows"` audit pattern |
| I-10 | Cross-organ wrong-side write risk | **PATCHED-BY-DESIGN** — bono cloud is sole writer of rate_windows + emits read-only `effective_rate(surface, time)` API; james venue NEVER writes rate_windows directly; pod-side surfaces are read-only consumers via REST |

## 4. V2-alignment delta

V2 Phase 2-A rate_table service substrate per PACT-DRAFT-phase-2-a-rate-table-service.md (bono-LEAD; canonical engineering spec):

**A. Service deployment shape (PACT-DRAFT §3 Q1-b)**

Module in api-gateway, route prefix `/v2/rates/*`, single deployable. Bono cloud authority. Composes with existing api-gateway auth/security middleware.

**B. Storage schema (PACT-DRAFT §4 Q2-c — V2-DB extension)**

```sql
-- Migration: crates/v2-db/migrations/<ts>_phase_2_a_rate_table.sql
CREATE TABLE rate_windows (
  window_id        TEXT PRIMARY KEY,
  surface          TEXT NOT NULL,                    -- 'sim' | 'ps5' | 'cafe'
  surface_sku      TEXT,                             -- NULL for sim/ps5; cafe SKU id per §S-109
  starts_at        TEXT NOT NULL,                    -- ISO8601 UTC (Q3-a)
  ends_at          TEXT NOT NULL,                    -- ISO8601 UTC (Q3-a)
  rate_value_paise INTEGER NOT NULL,                 -- GST-INCLUSIVE per §S-101
  rate_kind        TEXT NOT NULL CHECK (rate_kind IN ('standard','discount','premium','combo')),
  policy_tag       TEXT,                             -- e.g. 'P1-sim-offpeak-30pct' / 'S91-sim-standard-v1'
  is_promoted      INTEGER NOT NULL DEFAULT 0,       -- §S-104 messaging-flag (NOT price-discount)
  cafe_class       TEXT CHECK (cafe_class IN ('A','B','C')),  -- §S-95 class column; NULL for non-cafe
  created_by       TEXT NOT NULL,
  created_at       TEXT NOT NULL,                    -- UTC
  superseded_by    TEXT,                             -- self-FK soft-delete
  FOREIGN KEY (superseded_by) REFERENCES rate_windows(window_id)
);
CREATE INDEX rate_windows_active ON rate_windows(surface, starts_at, ends_at);
CREATE INDEX rate_windows_surface_sku ON rate_windows(surface, surface_sku) WHERE surface_sku IS NOT NULL;
```

**C. REST API (PACT-DRAFT §2)**

- `GET  /v2/rates/effective` — `?surface=sim&at=ISO8601` → `{rate_value_paise, rate_kind, policy_tag, window_id}`
- `GET  /v2/rates/upcoming` — `?surface=sim&from=ISO8601&to=ISO8601` → array of upcoming windows (booking-flow display per Wallet-Framing-C split-rate break)
- `POST /v2/rates/admin/window` — admin add/supersede (staff:manager+); writes new row + emits `config_push` cache-flush event
- `GET  /v2/rates/admin/health` — `{seed_loaded: bool, cache_warm: bool, active_window_count, last_invalidation_at}`

**D. Cache layer (PACT-DRAFT §5 Q4-c hybrid + §S-108)**

Push-invalidation primary (`config_push` event broadcast on admin write; ack tracked) + 60s TTL backstop. Cache key = `(surface, [surface_sku,] minute_bucket)`. Q-RATE-2 live-rate-per-minute continuous: billing calculator (Phase 2-B consumer) re-queries each minute boundary; Joint #2 ±2s tolerance respected.

Performance budget: <5ms p99 cache-warmed; <50ms p99 cache-miss. Benchmark gate at FILE-time + ship-time.

**E. Event emission (PACT-DRAFT §7)**

Per-rate-resolution event published to:
1. **MI ingestion (Wave 4 consumer)** — `rate_resolved` event `{session_id, surface, surface_sku?, resolved_at_utc, rate_value_paise, rate_kind, policy_tag, window_id}`
2. **Billing calculator (Phase 2-B consumer, james venue)** — same event delivered via comms-link relay (push-mode for live-rate; pull-mode for retry/recovery); idempotency via `(session_id, observed_at)` composite key

**F. Seed config (PACT-DRAFT §6 — §S-91 + §S-92 + §S-109 LOCKED Captain substrate)**

Migration runtime seed:
- sim standard 90000 paise/hr · sim discount 63000 paise/hr weekday 11:00-16:00 IST
- ps5 standard 50000 paise/hr · ps5 extra-controller flat 20000 paise/session (§S-93)
- 10 cafe SKUs per §S-109 standard prices · 4-SKU push-set with `is_promoted=true` (no price discount)
- cafe -20% weekday 11:00-16:00 IST window per §S-92 P2

3 cafe sheet confirmations OPEN at FILE (Flat White / Iced Latte / Hazelnut Iced Latte V2.1+ scope-pin).

**G. Bono Phase 2-B consumer interface alignment (PACT-DRAFT §S-110 risk note)**

Engine `effective_rate()` outputs align with Phase 2-B §1 3-surface contract (commit abbf52a8 PHASE-2-B-BONO-CLOUD-SURFACES-REFERENCE.md). Bono billing-calculator (W1-S6) reads `rate_value_paise` for per-tick debits. Split-rate contract per Phase 2-B §4 honored.

**Named gap (R-Phase-2-A):** V2 rate_table service is the NEW-MECHANISM-CLASS load-bearing substrate that 5 downstream RCAs (RCA #1 1.19 operating-window · RCA #2 1.20 iRacing-discount · RCA #7 7.3 ceiling · RCA #8 Phase-2-E combo · RCA #9 Phase 2 dyn-pricing engine) all DEPEND-ON. V1 dynamic_pricing.rs recommendation-only retained; F25a SnapPricing strategy preserved with HISTORICAL block consumed via engine layer atop rate_table base rate.

## 5. V2-framed proposed change

**Phasing (4 sub-phases; ~250-350 LOC; bono cloud-LEAD substrate-PR per PACT-DRAFT-2-A §12 authority basis; james AMPLIFIER on cross-organ Phase 2-B interface):**

**Phase 1 — Schema + 2 read endpoints + seed migration** (~150 LOC)
- v2-db migration: `rate_windows` table + 3 indices + FK-rewriting awareness audit
- `crates/api-gateway/src/v2_rates/` module (NEW) — query handlers `effective` + `upcoming`
- Seed migration loads §S-91 + §S-92 + §S-109 substrate
- Routes register `GET /v2/rates/effective` + `GET /v2/rates/upcoming`
- Unit tests cover §S-91 standard rates + §S-92 P1+P2 off-peak windows + §S-101 GST-inclusive semantics + §S-104 `is_promoted=true` messaging-flag NOT discount

**Phase 2 — Admin write endpoints + push-invalidation** (~80 LOC)
- `POST /v2/rates/admin/window` (staff:manager+ auth)
- `GET  /v2/rates/admin/health`
- `config_push` cache-flush event broadcast on admin write; ack-tracked via comms-link WS substrate
- 60s TTL backstop on cache layer (in-process LRU per api-gateway worker)
- Unit tests cover Q-RATE-1 push invalidation + Q-RATE-2 live-rate-per-minute boundary behavior

**Phase 3 — Event emission to MI + Phase 2-B billing calculator** (~70 LOC)
- `rate_resolved` event emitted on every `effective` resolution (sampled at session-start + per-minute-boundary by consumer)
- Comms-link relay push-mode for james billing-calculator
- MI ingestion path direct write (Wave 4 same-host consumer)
- Idempotency via `(session_id, observed_at)` composite key
- Contract tests against bono Phase 2-B billing-calculator surface (commit abbf52a8 reference)

**Phase 4 — Performance benchmark + behavior-parity** (~50 LOC harness)
- Benchmark harness for <5ms p99 cache-warmed / <50ms p99 cache-miss
- F25a SnapPricing strategy consumed at engine layer (RCA #9 integration) — behavior-parity test ensures Phase 2-A base rate × SnapPricing transform = V1 historical fixture

**Anti-pattern guard:**
- Test asserts `effective_rate(surface, sku, time)` signature explicitly excludes customer parameter (Joint #4 broadcast-only)
- Test asserts `rate_value_paise` semantically GST-inclusive (§S-101) — revenue-recognition layer splits at consumption boundary not rate boundary
- Test asserts §S-91 binding seed loads correctly (PRAGMA check fails-closed if seed missing)
- Test asserts F25a SnapPricing behavior-parity preserved at engine integration
- Test asserts `is_promoted=true` rows do NOT alter rate_value_paise (§S-104 messaging-flag discipline)
- Test asserts SQLite RENAME audit (`grep -rn "REFERENCES rate_windows" crates/*/migrations/`) returns clean post-migration
- Test asserts cache-flush event fires within Joint #2 ±2s tolerance window

**Mechanism-trust check (§S-186 5-Q):**
- (1) atomic primitives? **YES** — `effective_rate` is pure function read; admin write + config_push is single atomic /exec sequence
- (2) TTL-bounded sentinels? **YES** — 60s TTL backstop on cache; admin write triggers immediate invalidation via config_push
- (3) behavioral-verify success? **YES** — health endpoint reports `seed_loaded` + `cache_warm` + `active_window_count` + `last_invalidation_at`; benchmark gate verifies p99
- (4) single-target dry-run? **YES** — staging endpoint + unit tests + Phase 3 contract tests against Phase 2-B consumer before fleet rollout
- (5) guard contracts? **YES** — bono cloud sole writer; james venue read-only consumer via REST; api-gateway auth middleware on admin endpoints; PACT-DRAFT-2-A §6.5 cross-organ contract explicit
- **Verdict: PASS** (V2-aligned; substrate-PR can proceed once dependencies clear)

**Mechanism-trust dependencies blocking ship:**
1. Wave 1 SessionBillingService + refund routing + idle-timeout + PIN-LOCKOUT LAND (PACT-DRAFT-2-A §11 activation_trigger; verify-by 2026-05-21)
2. PACT-DRAFT-phase-2-a-rate-table-service.md slot-RESERVE + FILE (currently DRAFT-AWAITING-WAVE-1-LAND)
3. Captain answers Q-A1..Q-A4 (bono defaults proposed): migration ordering · surface_sku FK vs TEXT · policy_tag versioning · cache-flush channel topology — all bono-default acceptance acceptable per §S-204 G33 RATIFY pattern when james AMPLIFIER concurs
4. 3 cafe sheet confirmations (Flat White / Iced Latte price / Hazelnut Iced Latte V2.1+ scope-pin) — gate on next Playwright + bono Google session sheet-read cycle

**V2 doctrine alignment statement:**
> V2 doctrine alignment: closes 1 of 19 V1→V2 STRUCTURAL GAPS — G-Phase-2-A-1 NEW-MECHANISM-CLASS rate_table service substrate. Establishes V2 cloud-side authoritative rate-resolution primitive per V2-PROGRESS-MAP §2 W2 Phase 2-A (Layer 10.12 LIVE-BLOCKING). Encodes §S-91 §RATE-TABLE canonical binding rule + §S-92 P1+P2 off-peak windows + §S-101 GST-INCLUSIVE doctrine + §S-104 Class A messaging-flag discipline + §S-108 cache semantics + §S-109 10-SKU cafe menu + Joint #4 broadcast-only signature + Joint #2 ±2s tolerance. Composes-with downstream RCA #1 (operating-window extension consumer) · RCA #2 (iRacing-discount deeper-of input) · RCA #7 (ceiling clamp atop deeper-of) · RCA #8 (Phase 2-E combo offer cafe-class consumer) · RCA #9 (Phase 2 dynamic-pricing engine load-bearing base). Bono Phase 2-B billing-calculator consumer interface (commit abbf52a8 PHASE-2-B-BONO-CLOUD-SURFACES-REFERENCE.md) aligned at PR-author bilateral sync. F25a SnapPricing strategy preserved with HISTORICAL block at engine integration (RCA #9 §4.A strategy module).

## Captain decision queue

| Decision | Status |
|---|---|
| **D-Phase-2-A-1** PACT-DRAFT-2-A slot-RESERVE + FILE | DRAFT-AWAITING-WAVE-1-LAND (PACT-DRAFT §11 activation_trigger; verify-by 2026-05-21) |
| **D-Phase-2-A-2** Substrate-PR Phase 1 schema + read endpoints | AUTHORED-PENDING (this RCA = §S-146 V1↔V2 gate prereq satisfied) |
| **D-Phase-2-A-3** MMA Step 1 DIAGNOSE (foundational) | bono OpenRouter; AWAITING-Captain-budget-auth (~$1 share of §S-248.D-2 ~$3-5 batch) |
| **D-Phase-2-A-4** Captain Q-A1..Q-A4 disposition (bono defaults proposed) | bono-LEAD recommendations posted; james AMPLIFIER concurrence pending FILE |
| **D-Phase-2-A-5** 3 cafe sheet confirmations | DEFERRED to next Playwright + bono Google session sheet-read cycle |
| **D-Phase-2-A-6** Per-PR Captain merge auth on substrate-PR (foundational boundary) | AWAITING substrate-PR draft |

## Composes-with

- [⭐⭐ V1-dep V2 RCA doctrine](feedback_v1_dependent_v2_root_cause_before_proceeding.md)
- [⭐ Mechanism-trust-check upstream of fix RCA §S-172](feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md)
- RCA-2026-05-13-row-1.19-operating-window-extension — operating-window consumer of rate_windows
- RCA-2026-05-13-row-1.20-iracing-discount-mechanism — game-context → rate-table join consumer
- RCA-2026-05-13-row-7.3-max-discount-pct-ceiling — ceiling clamp atop resolved rate
- RCA-2026-05-13-phase-2-e-combo-offer-primitive — cafe-class column consumer
- RCA-2026-05-13-phase-2-dynamic-pricing-engine — load-bearing engine read consumer
- **Companion PACT-DRAFT-phase-2-a-rate-table-service.md** (bono-LEAD engineering spec; sibling-of parent Phase 2 dynamic-pricing-engine PACT)
- PHASE-2-B-BONO-CLOUD-SURFACES-REFERENCE.md (commit abbf52a8) — billing-calculator consumer interface alignment
- §S-91 §RATE-TABLE canonical binding (LOCKED 2026-05-08 segment-D)
- §S-92 P1+P2 off-peak windows (LOCKED 2026-05-08 segment-E)
- §S-101 GST-INCLUSIVE doctrine (LOCKED 2026-05-08 segment-F)
- §S-109 10-SKU coffee menu (LOCKED 2026-05-08 segment-H)
- §S-108 Q-RATE-1 + Q-RATE-2 (CONCUR-AGREE 2026-05-08)
- Captain sqlx::migrate cache invalidation doctrine (RATIFIED 2026-05-08 ~22:01 IST)
- Captain SQLite ALTER TABLE RENAME FK rewriting doctrine (RATIFIED 2026-05-08 ~22:01 IST)
- §S-248 9-RCA batch parent cascade (this RCA = §S-249.1 follow-up)

## Stale-at

2026-08-13 (90 days). Re-read against current code state before substrate-PR derivation — PACT-DRAFT-2-A may have advanced past DRAFT-AWAITING-WAVE-1-LAND state, or §S-91/§S-92/§S-101/§S-109 substrate may have CHALLENGE-AMENDed.
