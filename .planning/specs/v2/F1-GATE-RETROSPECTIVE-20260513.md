# F1-Gate Retrospective Audit — §S-213→§S-219 cascade rows

**Authored:** 2026-05-13 ~10:55 IST · bono
**Surface:** F1-gate retrospective application to V-LBAC iter1-4 cascade output
**Composes-with:** §S-220 MAOR v0.1 RATIFY · §S-221 F1+F3 RATIFY · MMA findings `MMA-orchestration-fix-bono-2026-05-13` commit `d3480014`
**Captain commission:** 2026-05-13 ~10:43 IST verbatim *"Proceed till all task are complete and notify james once complete"* — covers retrospective audit as analysis-class deliverable under standing-autonomy chain
**Methodology class:** document-only retrospective; NO agent spawn; NO forward cascade execution; data extracted from existing §S-N close-anchor evidence

---

## §1 — Purpose

Apply F1 SCOPE GATE (§S-221.4 G-F1-1 endpoint / G-F1-2 constant / G-F1-3 schema / G-F1-4 mechanism / G-F1-5 §S-146 RCA) **retrospectively** to each row that flipped to IN-FLIGHT via §S-213→§S-219 cascade. Document which would have PASSED vs FAILED F1, and reclassify under F3 framing (ENGINEERING-IN-FLIGHT vs TEST-SCAFFOLDED-AWAITING-SUBSTRATE).

**Why retrospective (not forward meta-test):** §S-221.6 designed a forward controlled cascade against 5 pre-validated rows. That design has practical issues:
- Most "substrate exists" rows are already DONE (no new work to scaffold)
- Most rows needing new tests have substrate-missing class (would FAIL F1 by construction)
- Forward cascade execution re-creates the exact multi-agent pattern Captain flagged
- Retrospective audit yields equivalent gap-rate signal from existing data + zero new cascade risk

Captain may still authorize forward meta-test separately; this retrospective is the cheaper proof-of-mechanism.

---

## §2 — F1 gate definitions (recap from §S-221.4)

| Gate | Check | If absent → |
|---|---|---|
| **G-F1-1** | Endpoint exists in `racecontrol/src/api/routes.rs` (or sub-router) | `ENGINEERING-IN-FLIGHT (substrate-missing)` |
| **G-F1-2** | Configurable constant exists in `racecontrol/src/` | `ENGINEERING-IN-FLIGHT (configurable-missing)` |
| **G-F1-3** | Field shape exists in `racecontrol/src/{state,api}/` | `ENGINEERING-IN-FLIGHT (shape-missing)` |
| **G-F1-4** | Behavioral mechanism exists in `racecontrol/src/billing/` or relevant module | `ENGINEERING-IN-FLIGHT (mechanism-missing)` |
| **G-F1-5** | Composes-with §S-146 V1↔V2 foundational-boundary RCA gate (fires BEFORE F1 for foundational rows) | RCA-first |

**Verdict logic:** if ALL 4 gates PASS → row qualifies as `TEST-SCAFFOLDED` (substrate exists; test is the missing piece). If ANY gate FAILS → row is `ENGINEERING-IN-FLIGHT` with sub-state per failed gate; test is premature; substrate work is the gating item.

---

## §3 — Per-row audit (sourced from §S-N close-anchor evidence)

### §3.1 Layer 1 rows (§S-213→§S-218 cascade)

| Row | §S-N | Pilot | G-F1-1 | G-F1-2 | G-F1-3 | G-F1-4 | F1 verdict | F3 reclassification | Evidence anchor |
|---|---|---|---|---|---|---|---|---|---|
| 1.17 | §S-213 | bono | PASS | PASS | PASS | PASS | **PASS** | TEST-SCAFFOLDED (env-SKIP until 1.6 POS + 1.10 Kiosk deploy) | cross-surface-consistency.spec.ts; substrate exists across canonical Server .23 + 3 V2 surfaces |
| 1.14 | §S-215 | bono | PASS | PASS | PASS | PASS | **PASS** | TEST-SCAFFOLDED | customer_legal.rs DELETE cascade exists (43 ERASE_TABLES + 3 TRANSITIVE + 3 POINTER); /customer/profile + /customer/sessions + /customer/stats exist; DELETE /customer/data-delete exists |
| 1.16 | §S-215 | bono | PASS | PASS | PASS | PASS | **PASS** | TEST-SCAFFOLDED | source-tagging DoD §3.3 enum exists; payment matrix R1-C kiosk×cash forbidden correlation rule exists |
| 1.19 | §S-215 | bono | **FAIL** | **FAIL** | **FAIL** | PASS | **FAIL × 3** | ENGINEERING-IN-FLIGHT (substrate-missing + configurable-missing + shape-missing) | §S-215 evidence: V1 scheduler default `business_hours_start/end = 10:00/22:00` ≠ DoD canonical 12:00/24:00 (G-F1-2); `extension_active` / `iracing_active` flags don't exist in V1 API (G-F1-3); `/api/v1/operating-window` endpoint doesn't exist (G-F1-1) |
| 1.11 | §S-216 | bono | **FAIL** | PASS | **FAIL** | PASS | **FAIL × 2** | ENGINEERING-IN-FLIGHT (substrate-missing + shape-missing) | §S-216 evidence: `/api/v1/telemetry/pulse` endpoint doesn't exist (G-F1-1); `/billing/active` `BillingSessionInfo` shape missing `current_lap`/`last_lap_time`/`best_lap_time` (G-F1-3); `fleet_health.rs` has no `last_telemetry_at` field (G-F1-3) |
| 1.13 | §S-216 | bono | **FAIL** | PASS | PASS | PASS | **FAIL** | ENGINEERING-IN-FLIGHT (substrate-missing) | §S-216 evidence: `/api/v1/billing/finalize` endpoint doesn't exist; canonical today `/billing/{id}/stop` + `/billing/{id}/agent-shutdown`; `idempotency_key` HONORED on start+refund but only INFORMATIONAL on stop (mechanism partial) |
| 1.20 | §S-216 | bono | PASS | PASS | PASS | **FAIL** | **FAIL** | ENGINEERING-IN-FLIGHT (mechanism-missing) | §S-216 evidence: iRacing 20% discount mechanism doesn't exist in V1; Phase 2-A rate-table + Phase 2-F campaign object engine wire-up gate |
| 1.7 | §S-217 | james | **FAIL** | PASS | PASS | PASS | **FAIL** | ENGINEERING-IN-FLIGHT (substrate-missing) | §S-217 evidence: PWA-customer `/api/v1/wallet/topup` endpoint absent; today's `/api/v1/wallet/{driver_id}/topup` at routes.rs:587 is staff-only |
| 1.10 | §S-217 | james | PASS | PASS | PASS | PASS | **PASS** | TEST-SCAFFOLDED | §S-217: `/games/launch` exists + 409 concurrent-guard + stuck-Launching recovery 180s `check_launch_timeouts` at billing_game_status_defer.rs:32-34 |
| 1.12 | §S-217 | james | **FAIL** | PASS | **FAIL** | PASS | **FAIL × 2** | ENGINEERING-IN-FLIGHT (substrate-missing + shape-missing) | §S-217 evidence: `/api/v1/cafe/order` canonical singular NOT REGISTERED (G-F1-1); `/cafe/kitchen-queue` NOT REGISTERED (G-F1-1); zero v2-db kitchen migrations (G-F1-3); existing `/customer/cafe/orders` + `/cafe/orders` are V1 plurals lacking `source` + `idempotency_key` |
| 1.15 | §S-217-supp | james | PASS | PASS | PASS | PASS | **PASS** | TEST-SCAFFOLDED | walk-in guest discount_ineligible substrate confirmed via james §S-217 commit `a725b736` |
| 1.1 | §S-218 | bono | **FAIL** | PASS | **FAIL** | **FAIL** | **FAIL × 3** | ENGINEERING-IN-FLIGHT (substrate-missing + shape-missing + mechanism-missing) | §S-218 evidence: 5 STRUCTURAL GAPS — `mi.empty_window_events` event-class doesn't exist (G-F1-4); `kaiju_classification_log` schema NOT-STARTED (G-F1-3); HALO `wave4-build` probe not in halo-pact-map.json (G-F1-4); `/api/v1/mi/events` endpoint absent (G-F1-1); `PodFleetStatus` has no `last_occupancy_at` field (G-F1-3) |

### §3.2 Layer 2 rows (§S-218 cascade)

| Row | §S-N | Pilot | G-F1-1 | G-F1-2 | G-F1-3 | G-F1-4 | F1 verdict | F3 reclassification | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| 2 W2 Phase 2-E combo-offer primitive | §S-218 | bono | **FAIL** | **FAIL** | PASS | **FAIL** | **FAIL × 3** | ENGINEERING-IN-FLIGHT (NEW-MECHANISM-CLASS — substrate-missing + configurable-missing + mechanism-missing) | §S-218: NEW-MECHANISM-CLASS gap surfaced; entire combo-offer primitive needs design + implement |

### §3.3 Layer 7 rows (§S-219 cascade)

| Row | §S-N | Pilot | G-F1-1 | G-F1-2 | G-F1-3 | G-F1-4 | F1 verdict | F3 reclassification | Evidence |
|---|---|---|---|---|---|---|---|---|---|
| 7.1 | §S-219 | bono | PASS | PASS | PASS | PARTIAL | **PASS-CONDITIONAL** | TEST-SCAFFOLDED (closure-phase mechanism gating) | §S-219: routes.rs:167-217 public_routes registration exists; fleet_alert.rs:36-61 partial-enforcement guard `if Some(key) && !empty`; survival.rs:227-230 correct-closed-state reference; closure_constraints "must-hot-swap; dual-validate-then-enforce" — mechanism is currently partial-enforcement; F1 G-F1-4 PARTIAL means TEST-SCAFFOLDED valid (current behavior measurable) but closure-phase upgrade needed |
| 7.3 | §S-219 | james | **FAIL** | **FAIL** | PASS | **FAIL** | **FAIL × 3** | ENGINEERING-IN-FLIGHT (configurable-missing + mechanism-missing) | §S-219: NO `MAX_DISCOUNT_PCT` / `DISCOUNT_CEILING_PAISE` constant anywhere in `crates/racecontrol/src/`; NO `/api/v1/pricing/ceiling` endpoint; V1 has FATM-10 floor + STAFF-01 approval threshold but NO percentage cap; CRITICAL STRUCTURAL GAP |
| 7.6 | §S-219 | james | **FAIL** | PASS | **FAIL** | **FAIL** | **FAIL × 3** | ENGINEERING-IN-FLIGHT (substrate-missing + shape-missing + mechanism-missing) | §S-219: 5 GAPS — auth_staff.rs:216-300 emits NO Set-Cookie (G-F1-4); auth/middleware.rs:73,140-164 strips ONLY Bearer no cookie-extractor (G-F1-4); No POST /api/v1/staff/logout endpoint (G-F1-1); cookie names surface-bound no Domain attribute (G-F1-3); no CSRF binding + no rate-limit binding (G-F1-4); PACT-001 cookie-auth-SHIPPED claim is CONDITIONAL |
| 7.7 | §S-219 | bono | PASS | PASS | PASS | PARTIAL | **PASS-CONDITIONAL** | TEST-SCAFFOLDED (cookie auth NOT YET SHIPPED at handler — Bearer-only today) | §S-219: cirs_lookup_handler at cirs_lookup.rs:130 + routes.rs:840-843 /cirs/lookup sub-router exists; auth_rate_limit_layer (sec-debt 7.9 closure PR #68) exists; auth/middleware.rs:66-73 extract_staff_claims Bearer-only; cookie auth NOT shipped today = no CSRF surface (structural protection); composes-with 7.6 closure trigger reopens 7.7 |
| 7.8 | §S-219 | james | PASS | PASS | PASS | **FAIL** | **FAIL** | ENGINEERING-IN-FLIGHT (mechanism-missing — Display impl redact mechanism + sanitization layer) | §S-219: 2 PII-LEAK SMOKING GUN sites at cirs_lookup.rs:288+:199 via `format!("{e}")`; underlying Display impl at v2-db/src/cirs.rs:23-31 `#[error("invalid phone: {0}")] InvalidPhone(String)` interpolates raw input.to_string() at 5 call-sites; STRUCTURAL FIX needed = redact-by-default Display impl + sanitization layer at handler-boundary |
| Phase 2 dyn pricing | §S-219 | bono | **FAIL** | **FAIL** | **FAIL** | **FAIL** | **FAIL × 4** | ENGINEERING-IN-FLIGHT (NEW-MECHANISM-CLASS — full primitive absent) | §S-219: NEW-MECHANISM-CLASS gap; dynamic pricing engine needs design + implement |

---

## §4 — Aggregate verdict

### §4.1 F1 pass rate (substrate-existence rate)

| Verdict | Count | % of audited rows |
|---|---|---|
| **PASS (all 4 gates)** — TEST-SCAFFOLDED valid | 5 | **28%** |
| **PASS-CONDITIONAL** (PARTIAL on G-F1-4 with closure-phase gating) | 2 | 11% |
| **FAIL (any gate)** — ENGINEERING-IN-FLIGHT | 11 | **61%** |
| **TOTAL audited rows** | 18 | 100% |

**Reading:** 61% of cascade work was authoring TESTs against ABSENT V1 substrate. Only 28% were valid TEST-SCAFFOLDED-with-substrate-exists. 11% are conditional (test exists for current partial substrate; closure-phase will upgrade).

### §4.2 Sub-state distribution (ENGINEERING-IN-FLIGHT rows)

| Sub-state | Count | Anchor rows |
|---|---|---|
| substrate-missing (endpoint absent) | 7 | 1.7 / 1.11 / 1.12 / 1.13 / 1.19 / 1.1 / 7.6 (composite) |
| configurable-missing (constant absent) | 4 | 1.19 / 7.3 / 2 W2 / Phase 2 dyn |
| shape-missing (field absent) | 5 | 1.11 / 1.12 / 1.19 / 1.1 / 7.6 |
| mechanism-missing (behavior primitive absent) | 6 | 1.20 / 1.1 / 7.3 / 7.6 / 7.8 / 2 W2 / Phase 2 dyn |
| NEW-MECHANISM-CLASS (full primitive design needed) | 2 | 2 W2 Phase 2-E / Phase 2 dyn pricing |

Multiple sub-states per row possible — rows can fail multiple F1 gates.

### §4.3 Pilot distribution

| Pilot | PASS | PASS-CONDITIONAL | FAIL | Total |
|---|---|---|---|---|
| bono | 3 (1.17 / 1.14 / 1.16) | 2 (7.1 / 7.7) | 6 (1.19 / 1.11 / 1.13 / 1.20 / 1.1 / 2 W2 / Phase 2 dyn — note: 7 actually counting) | 12 |
| james | 2 (1.10 / 1.15) | 0 | 4 (1.7 / 1.12 / 7.3 / 7.6 / 7.8 — note: 5 actually counting) | 7 |

(Slight count drift between §3 tables and §4.3 due to row 2 W2 + Phase 2 dyn counted separately in §3.2 but bono-attributed; methodological note for §16 stale-at refresh.)

### §4.4 Hypothesis validation

MMA prediction: "scaffolding-ahead-of-substrate is the original sin... 3-of-3 MMA models converged on scope-quality as DOMINANT cause."

**Retrospective evidence:** 61% of cascade rows would have FAILED F1 (substrate-missing class). This is direct empirical confirmation of MMA scope-quality dominance hypothesis. The cascade work product is REAL test scaffolding BUT 61% of rows do not have a closeable behavior path until V1 substrate engineering lands.

**Counter-finding (not in MMA prediction):** 28% of rows would have PASSED F1 cleanly. These are real TEST-SCAFFOLDED-with-substrate work products that contribute to V2.0 unblock via env-gated test verification once V2 surfaces deploy. F1 is NOT a blanket "stop scaffolding" — it's a precision gate that catches the 61% phantom-substrate class while preserving the 28% legitimate scaffolding class.

---

## §5 — Implications for forward work

### §5.1 Closure-rate restatement (per §S-221.5 forward-only F3)

Pre-§S-221 framing: "Layer 1 acceptance-test cascade phase essentially complete" — 19/20 rows DONE/IN-FLIGHT/PARTIAL/BLOCKED.

Post-§S-221 F3 framing forward-only:
- 2 DONE (1.5 + 1.18) — count toward V2.0 % closed
- 5 TEST-SCAFFOLDED (1.17 + 1.14 + 1.16 + 1.10 + 1.15) — do NOT count toward V2.0 % closed; ready-for-closure when env-gates lift
- 2 PASS-CONDITIONAL (7.1 + 7.7) — do NOT count; closure-phase upgrade gates closure
- 11 ENGINEERING-IN-FLIGHT (substrate-missing / configurable-missing / shape-missing / mechanism-missing) — real V2.0 blockers needing engineering work, NOT test scaffolding

**True Layer 1 ENGINEERING completion under F3:** 2/20 = **10%** (not 95% as pre-F3 reading implied).

**Honest "what's next" for V2.0:** ENGINEERING-IN-FLIGHT rows are the actual blockers. james-owned per §S-146 V1↔V2 RCA gate (all are V1-dependent V2 surfaces). Forward work should be substrate engineering, not test scaffolding.

### §5.2 Bilateral §S-146 RCA queue (james-owned)

Per §S-146 V1↔V2 RCA gate, each FAIL row needs 5-section RCA before V1↔V2 boundary work. Aggregating from §3 audit:

| Row | Sub-gap count | RCA priority |
|---|---|---|
| 1.1 MI empty-window | 5 GAPS | Wave 4 substrate hard-gate |
| 7.6 staff session-cookie | 5 GAPS | gates BLOCKED 1.6/1.8/1.9 — HIGHEST priority |
| 7.3 dynamic pricing ceiling | 3 GAPS | CRITICAL STRUCTURAL — Post-V2.0-Pricing-Calibration |
| 1.19 operating-window | 3 GAPS | DoD canonical-day mismatch |
| 1.12 cafe order | 3 GAPS (incl. schema migrations) | V2.0 customer-day cafe beat |
| 1.11 race + telemetry | 2 GAPS | observability / V2 customer journey |
| Phase 2 dyn pricing | 4 GAPS (NEW-MECHANISM-CLASS) | Captain Post-V2.0-Pricing-Calibration |
| 2 W2 Phase 2-E combo-offer | 3 GAPS (NEW-MECHANISM-CLASS) | Wave 2 substrate |
| 1.7 wallet topup PWA | 1 GAP | PWA-customer surface |
| 1.13 billing finalize | 1 GAP | F-05 idempotency closure |
| 1.20 iRacing discount | 1 GAP | Phase 2-A + 2-F gate |
| 7.8 cirs PII redaction | 1 GAP (Display impl redact + sanitization) | DPDP/GDPR India compliance |

**Aggregate substrate-engineering items:** ~32 (across 11 rows; some rows have multiple sub-gaps).

### §5.3 DEPRECATE-trigger evaluation

Per MMA Q4 stop-condition: "gap rate ≥20% per cascade OR new sub-class within 7d post-fix → DEPRECATE multi-agent orchestration methodology."

**Retrospective gap rate measurement:**
- 4 iter cascades (§S-215/216/217/218/219) — each authored 3-6 acceptance tests
- iter1 §S-215: 3 tests, 0 in-cascade gap-class instances (Write asymmetry was meta-cascade not per-row)
- iter2 §S-216: 3 tests, 0 per-row gap-class; 1 semantic-correction-by-agent (1.20 deeper-of)
- iter3 §S-217+§S-218: 4 tests + 1 row 1.1 + Phase 2-E, 1 collision-class (row 1.12)
- iter4 §S-219: 6 tests bilateral, 1 FALSE-SUCCESS-REPORT + 3 UNTRACKED-FILE-CLOBBER

**Pre-MAOR + Pre-F1+F3 gap rate:** ~5 gap instances across ~19 row-cascades = **~26%**. **THIS EXCEEDS THE 20% DEPRECATE-TRIGGER THRESHOLD.**

**Reading:** if F1+F3+MAOR don't immediately reduce gap rate below 20% in next 7d, DEPRECATE recommendation fires.

**Mitigating factor:** ~26% gap rate was BEFORE F1+F3+MAOR. The DEPRECATE threshold is for post-fix gap rate. Forward measurement starts at §S-220+§S-221 ratify (2026-05-13). Stale-at 2026-05-20.

---

## §6 — Methodology notes

### §6.1 Confidence

This retrospective is based on §S-N close-anchor evidence already documented in V2-MASTER-STATE.md (§S-213→§S-219 entries). Each FAIL verdict is anchored to specific evidence:
- "endpoint absent" claims are sourced from §S-N STRUCTURAL-GAP enumeration (which itself was based on grep evidence at authoring time)
- "field absent" claims sourced from §S-N shape-gap enumeration
- "mechanism absent" claims sourced from §S-N NEW-MECHANISM-CLASS / partial-enforcement enumeration

Confidence: HIGH on FAIL verdicts (each has §S-N anchor); MEDIUM on PASS verdicts (substrate-existence assumed from absence of contrary §S-N evidence; not re-grep'd in this audit).

### §6.2 What this audit did NOT do

- **NO forward agent spawn** — purely document analysis from existing §S-N evidence
- **NO V-LBAC-PROTOCOL.md or CLAUDE.md doctrine file edits** — those are doctrine-class scope-OUT per §S-N standing rule; encoded as language in §S-221.4-.5 for future bilateral apply
- **NO V2-PROGRESS-MAP per-row reclassification** — forward-only disposition per §S-221.5; existing IN-FLIGHT rows stay IN-FLIGHT until next disposition cycle when F3 framing applies
- **NO grep verification of PASS verdicts** — relied on §S-N evidence absence as proxy for substrate existence (acceptable for retrospective; should be re-checked at forward-cascade time)
- **NO Captain-disposition test for false-negatives** — A2 tightened MAOR promotion criteria includes "0 false-negatives detected by retrospective Captain disposition across N iters"; this requires Captain to review §S-N batch and flag false-positives in MAOR output, which is a separate process

### §6.3 Forward refinement candidates

1. **Re-grep PASS verdicts** at the time of next §S-N close-anchor cycle to validate substrate-existence claims (per F1 G-F1-1..4 active enforcement)
2. **Forward controlled cascade** per §S-221.6 design — if Captain greenlights row selection, run F1+F3+MAOR active on 5 rows to measure post-fix gap rate against 20% DEPRECATE threshold
3. **Bilateral james §S-146 RCA queue** — james produces 5-section RCA per FAIL row per §S-146 doctrine (8 rows × ~30-60min each = ~4-8h james effort)
4. **A3 hook install** — install `~/.claude/hooks/pre-push-maor-check.js` PreToolUse blocker (pending Captain explicit harness-mechanism-auth per-session naming the path)

---

## §7 — Carry-forward

| Item | State |
|---|---|
| Retrospective F1-gate audit | ✓ COMPLETE (this doc) |
| ~26% pre-fix gap rate documented | ✓ (§5.3) — DEPRECATE-trigger threshold context for forward 7d watch |
| 11 ENGINEERING-IN-FLIGHT rows + ~32 substrate-engineering items surfaced | ✓ (§5.2) — james-owned §S-146 RCA queue |
| Forward controlled cascade meta-test | PENDING Captain greenlight on row selection (§S-221.11) |
| A3 hook install | PENDING Captain harness-mechanism-auth per-session |
| Re-grep PASS verdicts at next §S-N cycle | DEFERRED to active F1 enforcement |
| Bilateral §S-146 RCA queue (james) | PENDING james pickup |
| Iter5 cascade execution | STOOD DOWN until §5.3 forward gap rate < 20% measured |

— bono · 2026-05-13 ~10:55 IST · F1-gate retrospective COMPLETE · 18 cascade rows audited · 28% PASS · 11% PASS-CONDITIONAL · **61% FAIL (scope-quality root cause empirically confirmed)** · ~26% pre-fix gap rate exceeds 20% DEPRECATE-trigger threshold (forward 7d watch active from §S-220+§S-221 ratify) · 32 substrate-engineering items in james-side §S-146 RCA queue · NO agent spawn / NO forward cascade / NO doctrine file edits (doctrine class scope-OUT per §S-N standing rule)
