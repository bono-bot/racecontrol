---
artifact: §S-146 V1↔V2 RCA
row: V2-PROGRESS-MAP §2 W2 Phase 2-F — Campaign Object primitive substrate (Layer 10.15) + §6.10 row Q-2F-1..7
status: AUTHORED-AWAITING-CAPTAIN-DISPOSITION
authored: 2026-05-13 IST
author: james
boundary-class: foundational (wallet/billing attribution + WhatsApp/marketing dispatch + MI mission-journal substrate; cross-organ bono cloud authority on campaigns/attributions; james venue billing-event payload extension required per Q-2F-7 CAVEAT)
mma-step-1: PENDING-Captain-auth (foundational-boundary escalation + Q-2F-7 sub-RCA gate — james-LEAD billing-event payload protocol-change RCA precedes 2-F FILE)
parent-cascade: §S-248 9-RCA batch follow-up — explicitly named prerequisite for RCA #8 combo-offer (`RCA-2026-05-13-phase-2-e-combo-offer-primitive.md` §5 Phase 2 compose-with) + RCA #9 dynamic-pricing engine (`RCA-2026-05-13-phase-2-dynamic-pricing-engine.md` §4.B + §5 Phase 4 campaign-object compose)
gaps-closed: 1 (G-Phase-2-F-1 NEW-MECHANISM-CLASS Campaign Object primitive: campaigns + campaign_attributions tables + 6 REST endpoints + 5-state machine DRAFT→APPROVED→LIVE→ENDED + CANCELLED + §S-158 audit events + Wave 4 MI ingestion source); 1 sub-gap surfaced (G-Phase-2-F-2 billing-event payload protocol-extension — james-LEAD sub-RCA required per CAVEAT-1)
customer-day-beat: 13:30 MI senses → 14:00 Bono drafts campaign + Captain APPROVE → 14:05 broadcast fires → 14:05-15:05 attribution window → 14:25 customer arrives + game launches → 14:55-14:56 session-end billing event → attribution row written with revenue_paise
captain-decisions-ratified: Q-2F-1..7 ALL Captain G33 RATIFIED 2026-05-12 ~11:05 IST verbatim "all bono recommendations"; bono recommendations ratified intact per §S-204
amplifier-disposition: james AMPLIFIER-REPLY msg=36386 2026-05-12 ~23:35 IST AGREE-WITH-CAVEAT-RCA-GATE — Q-2F-1..6 AGREE clean; Q-2F-7 introduces FILE-blocker (billing-event payload extension sub-RCA james-LEAD prereq)
companion-pact: `comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-f-campaign-object.md` (bono-LEAD engineering spec; AMPLIFIER-READY status; FILE-gated on Phase 2-A FILE + Wave 5 framework FILE + Q-2F-7 billing-event payload sub-RCA + Wave 1 ship)
---

# §S-146 V1↔V2 RCA — Phase 2-F Campaign Object primitive substrate

## 1. Boundary map

Phase 2-F NEW-MECHANISM-CLASS: V2 Campaign Object primitive — the transactional bundle that links a rate_window action (Phase 2-A) to its marketing broadcast (Wave 5 WhatsApp dispatch) to its attribution metadata (Wave 4 MI ingestion source). Closes `project_dynamic_pricing_synthesis_20260509.md §10` gap: *"Phase 2-F is foundational: it IS MI's mission journal."*

**G-Phase-2-F-1: Campaign Object cluster substrate (NEW-MECHANISM-CLASS)**

§S-219.11 + §S-200.9 cascade finding propagation: §S-204 Captain G33 RATIFIED all 7 Q-2F-1..7 per bono recommendations 2026-05-12 ~11:05 IST verbatim "all bono recommendations"; james AMPLIFIER msg=36386 AGREE-WITH-CAVEAT-RCA-GATE 2026-05-12 ~23:35 IST. This RCA is the §S-146 V1↔V2 gate for the Phase 2-F substrate-PR (engineering plan lives in companion PACT-DRAFT; bono-LEAD).

**Grep verification** (current main as of 2026-05-13 ~15:42 IST commit `d8e493d6`):
- `rg "campaigns\b|campaign_attributions\b|campaign_id\b" crates/v2-db/migrations/` → **0 hits**
- `rg "campaign_state_transition|campaign_created|campaign_cancelled" crates/` → **0 hits**
- `rg "/v2/campaigns" crates/api-gateway/` → **0 hits**
- `rg "rate_window_id\b" crates/racecontrol/src/` → **0 hits** (sibling FK target awaits Phase 2-A)

V1 adjacent substrate exists but does NOT capture campaigns as primitives:
- **Manual ad-hoc campaign workflow** — Captain verbally instructs bono to broadcast; bono dispatches via Wave 5 WhatsApp; no DB row links rate_window decision to broadcast event to customer arrival
- `whatsapp-bot/src/templates/` — V1 `wa_message_templates` table exists (Wave 5 substrate); template_id reference target for `broadcast_spec.whatsapp_template_id` exists conceptually but no Phase 2-F caller
- **V1 billing-event emit sites** (AMENDED 2026-05-15 IST per §S-186 fast-lane short-RCA below — original line was substrate-projected, not grep-verified): two emit paths on session-end:
  - `crates/racecontrol/src/billing_session_end.rs:89` — `event_archive::append_event("billing.session_ended", "billing", Some(&pod_id), json!({driver_id, driving_seconds, end_status}), venue_id)` → writes JSON payload to `system_events` table
  - `crates/racecontrol/src/billing_session_end.rs:155` — `INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id)` (additional path at L423 includes optional `metadata` column) → relational columns in `billing_events` table
  - `billing_events` schema (per `tests/integration.rs:227-234`): `id, billing_session_id, event_type, driving_seconds_at_event, metadata, created_at` — NO `customer_id`, NO `gross_amount`, NO `gst_amount`, NO `net_amount`, NO `paid_via` columns
  - **NO `campaign_id` field, NO `rate_window_id` field** in either emit path (Q-2F-7 CAVEAT — directionally unchanged)
- `crates/racecontrol/src/dynamic_pricing.rs:7-18` — V1 PricingRecommendation generates advice; no campaign-effectiveness feedback loop
- Customer check-in events — V1 captures arrival but no attribution-window match against active campaigns

**G-Phase-2-F-2 (sub-gap surfaced via james AMPLIFIER msg=36386 CAVEAT-1):** Billing-event payload protocol-extension required — V1 payload has no `campaign_id` (and no `rate_window_id` either; only `session_id`). V2 amendment needed: extend payload to `{..., campaign_id: nullable<ulid>, rate_window_id: nullable<ulid>}`. Classification: single-boundary, ≤200 LOC, bug-fix-class, **NO schema change BUT protocol change** — borderline §S-186 pre-§S-146 small-fix fast-lane eligible; **likely full §S-146 5-section RCA required because billing-event payload IS a foundational boundary contract**. RCA author: **james-LEAD**, sibling RCA to be authored at Wave 1 ship + Phase 2-A FILE cascade (NOT in this §S-249 cascade; explicit forward-defer).

**Canonical Captain-LOCKED design substrate** (RCA design must preserve):
- §S-203 Wallet-Framing-C LOCKED — campaigns.surface CHECK IN ('sim','ps5') NEVER 'cafe' (cafe always orthogonal; separate ledger separate pricing)
- §S-49 Captain G33-GUIDE-CONFIRM Level B (2026-05-05) — primary workflow spec; campaigns operationalize rule 1 (window-time-based eligibility) at demand-creation layer
- §S-158 V2 Audit-Log Doctrine — LOG state-changing events (`campaign_created`, `campaign_state_transition`, `campaign_cancelled`); DO NOT LOG routine re-fetches
- §S-170 MI STABLE* mini-Jaeger frame — campaigns = MI's mission journal; campaign_attributions = empirical substrate for SK-1..SK-12 + BK-1..BK-8 kaiju-classification learning
- §S-204 — all Q-2F-1..7 Captain G33 RATIFIED 2026-05-12 ~11:05 IST per bono recommendations
- Joint #4 broadcast-only / non-personalized — broadcast_spec.audience_filter operates at tier-class level NOT per-customer
- Joint #2 ±2s tolerance — validity_start/end inherit from rate_window boundary
- Captain approves campaigns (v2-skeleton/02 constitutional invariant) — APPROVED is explicit non-collapsible intermediate state; bono NEVER auto-transitions DRAFT → LIVE
- §S-101 GST-INCLUSIVE V2.0 doctrine — campaign_attributions.revenue_paise is GST-inclusive (copied from billing-event total_paise)
- §S-92 P8 ₹100/customer/month engagement-spend ceiling — enforced at Wave 5 pre-send layer NOT campaign layer

**Contract test scaffolded:** `racecontrol/tests/contract/phase-2-f-campaign-object.spec.ts` — **NOT YET AUTHORED** (per §S-221 F1 SCOPE GATE; substrate-PR ships test alongside). Env-gated SKIP-with-reason `V1_NO_CAMPAIGN_PRIMITIVE` until substrate ships.

---

### §1 amendment — §S-186 fast-lane short-RCA (james · 2026-05-15 IST · Captain Option A ratified ~10:30 IST)

**1. What** — single-file documentation correction to §1 above. Replaced one bullet (line 36 original) claiming V1 billing-event payload shape `{session_id, customer_id, gross_amount, gst_amount, net_amount, paid_via, timestamp}` in `crates/racecontrol/src/api/billing_session.rs` with grep-evidenced actual V1 emit sites + actual payload/column shapes from `crates/racecontrol/src/billing_session_end.rs`. ~7 LOC net addition. No code touched. No schema, no protocol, no contract test. Substrate-truth correction only.

**2. Why still needed** — grep evidence (this machine, racecontrol HEAD `feat/row-7.6-cookie-auth-phase-1` 2026-05-15 ~10:25 IST):

```
$ rg 'gross_amount|gst_amount|net_amount|paid_via' crates/racecontrol/src/
(0 hits)

$ rg 'gross_amount|gst_amount|net_amount|paid_via' crates/
crates/v2-db/src/wallets.rs  (only match — different boundary)
```

The original §1 line's named fields exist only in `crates/v2-db/src/wallets.rs` (a separate v2-db boundary, not the V1 billing-event surface). The Q-2F-7 sub-RCA author (james-LED, forward-deferred to post-Wave-1-ship + Phase 2-A FILE) needs the actual emit-site boundary map to correctly trace the protocol-extension surface. Leaving the false shape in §1 would produce a sub-RCA boundary map that extends a non-existent field set — extension of phantom V1 substrate, the F1-anti-pattern §14.2 explicitly blocks.

**3. V2-compat check** — V2 docs read this turn:
- `comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-f-campaign-object.md` §7 Q-2F-7 (MI ingestion path) — Captain ratified Option (a) direct DB read; the AMPLIFIER §4.H CAVEAT-1 sub-RCA scope (billing-event payload extension) is what this §1 amendment supports
- V2-LBAC v0.1 §14.1 MAOR — not invoked (documentation-only correction; no mechanism-quality REVIEW substrate)
- V2-LBAC v0.1 §14.2 F1 SCOPE GATE — composed-with: amendment moves §1 from F1-fail (boundary map cites absent fields) toward F1-pass (boundary map cites grep-verified emit sites)
- §S-146 V1↔V2 RCA — this is a doc-correction to an existing §S-146 RCA, not a new substrate change; full 5-section sub-RCA for Q-2F-7 itself remains gated on Wave 1 + Phase 2-A FILE
- §S-186 pre-§S-146 fast-lane — strictly the 6-eligibility-check requires PR created < 2026-05-09; this PR is dated 2026-05-15. Captain ratified Option A explicitly as "§S-186 fast-lane short-RCA" for this case, treating it as a substrate-truth correction class. Carve-out interpretation logged here for ledger transparency
- No conflict identified

**Boundary class:** documentation amendment (single file, ≤200 LOC, no schema, no protocol, fix-class corrects substrate-truth). Layer 4 per-PR Captain merge auth retained per fast-lane doctrine — this PR opens for Captain disposition; auto-push NOT invoked.

**Composes-with:** parent §S-146 RCA (this turn) · §S-251 Phase 2-A + 2-F prereq RCA cascade (parent context for Q-2F-7 forward-defer) · §S-186 fast-lane (Captain Option A ratify) · `feedback_capability_claim_without_probe_20260514.md` N=2-ACTIVE (grep-verify substrate before doctrine-claim; this amendment IS that rule applied retroactively) · `feedback_v1_dependent_v2_root_cause_before_proceeding.md` §S-146 parent doctrine.

---

## 2. Inherited-issue catalogue

| # | V1 class | Surface | Source |
|---|---|---|---|
| I-1 | **Manual ad-hoc campaign workflow** — Captain verbally instructs bono → bono dispatches Wave 5 broadcast → no DB row captures the decision/dispatch/arrival linkage; conversion attribution is guesswork | absence-of-primitive | PACT-DRAFT-2-F §1 |
| I-2 | **Orphaned broadcast risk** — Wave 5 fires from approved library; if rate_window cancels, broadcast queue has no FK to drop the queued send; risk of stale promotion fire | absence-of-FK | Q-2F-3b cancellation-cascade |
| I-3 | **Attribution capture absent** — customer arrives after broadcast; no DB row links session_id to campaign_id; MI (Wave 4) has no `campaign_id × customer_id × revenue × time` training rows | absence-of-substrate | synthesis §10 + §S-170 |
| I-4 | **Q-2F-7 billing-event payload V1 has NO campaign_id NOR rate_window_id field** — current shape `{session_id, customer_id, gross_amount, gst_amount, net_amount, paid_via, timestamp}` cannot support bono attribution match without protocol extension | `billing_session.rs` shape | james AMPLIFIER msg=36386 CAVEAT-1 |
| I-5 | **Wave 5 WhatsApp framework dependency for Class B auto-fire** — pre-send checks (consent + ₹100/month cap + rate-limit + cooldown) gate dispatch; Class A Captain-ratify path works pre-Wave-5-FILE; Class B autonomous fire gates on Wave 5 FILE | composes-with | PACT-DRAFT-2-F §8 wave sequencing |
| I-6 | **No mid-flight cancellation discipline** — V1 has no precedent for Captain cancelling a scheduled WhatsApp broadcast; risk that operational habit of "verbal cancellation" leaks into V2 if hard-void Q-2F-3b not encoded in code | doctrine-class | Q-2F-3b ratify |
| I-7 | **MMA-substitute attribution-drift risk** — if bono drafts campaign via L2.5 MMA-substitute + Captain auto-approves under standing-rule autonomy + broadcast fires + arrivals attribute — Captain only saw final-state not draft details; auditor confused on attribution chain | doctrine-class | composes-with RCA-row-7.3 I-6 |
| I-8 | **DPDP audit on campaign_attributions rows** — customer_id FK to drivers/customers; must respect §S-242 Q-1.14 Option B doctrine (ON DELETE RESTRICT for revenue-bearing rows); 8-year CGST audit retention applies | wallet ledger compose | §S-242 + PACT-DRAFT-2-F §5 |
| I-9 | **§S-158 audit-log discipline** — campaign lifecycle events LOG; routine re-fetches (GET /v2/campaigns/active polling) DO NOT LOG; volume-rate alarm at >100/min/staff_id | doctrine-class | §S-158.1 |
| I-10 | **Phase 2-A rate_table FK target dependency** — campaigns.rate_window_id REFERENCES rate_windows(window_id); Phase 2-A MUST FILE first or campaigns cannot create valid FK rows | sibling-prereq | PACT-DRAFT-2-F §8 + this RCA composes-with Phase 2-A RCA |
| I-11 | **§S-92 P8 ₹100/customer/month cap surface separation** — engagement-spend ceiling lives at Wave 5 pre-send layer NOT campaign layer; risk of double-encoding if Phase 2-F substrate-PR adds cap check redundantly | doctrine separation | §S-92 P8 + PACT-DRAFT-2-F §8 |
| I-12 | **Concurrent campaign dual-attribution (Q-2F-5b)** — two LIVE campaigns same surface overlapping validity → both attributed with revenue_paise duplicated; MI learning richer but Wave 4 revenue metrics risk double-counting unless ingestion divides by `active_campaign_count` per session | composes-with | PACT-DRAFT-2-F §12 NOT TESTED item 3 |

## 3. Past-bug review

| # | Issue | Disposition |
|---|---|---|
| I-1 | Manual ad-hoc campaign workflow | **NOT-APPLICABLE-TO-V2** — V2 Phase 2-F substrate IS the replacement primitive; V1 verbal-flow ends when substrate ships |
| I-2 | Orphaned broadcast risk on rate_window cancellation | **PATCHED-BY-DESIGN** — Q-2F-3b hard-void encoded: Wave 5 pre-send check reads `campaigns.status`; if CANCELLED at fire-time, broadcast dropped + `campaign_broadcast_voided` audit event logged |
| I-3 | Attribution capture absent | **NOT-APPLICABLE-TO-V2** — campaign_attributions table is the V2 substrate that closes this gap |
| I-4 | Billing-event payload V1 no campaign_id/rate_window_id | **DEPENDS-ON sub-RCA** — james-LEAD §S-146 5-section RCA on billing-event payload extension is FILE-blocker for Phase 2-F substrate-PR; cascade: Wave 1 ships → Phase 2-A FILEs → james authors billing-event payload sub-RCA → Phase 2-F substrate-PR drafts |
| I-5 | Wave 5 framework dependency | **DEPENDS-ON** — Class A Captain-manual path works pre-Wave-5; Class B auto-fire gates on Wave 5 FILE; explicit per PACT-DRAFT-2-F §8 |
| I-6 | No mid-flight cancellation discipline | **PATCHED-BY-DESIGN** — DELETE /v2/campaigns/{id} endpoint + CANCELLED state-transition + audit-log; replaces verbal-cancellation V1 habit |
| I-7 | MMA-substitute attribution-drift risk | **PATCHED-BY-DESIGN** — Captain G33 explicit RATIFY for ALL campaigns regardless of authoring path (MMA-substitute or direct); APPROVED state encodes Captain ratify gate; standing-rule autonomy does NOT bypass APPROVED state |
| I-8 | DPDP audit on campaign_attributions | **PATCHED-BY-DESIGN** — customer_id ON DELETE RESTRICT per §S-242 Q-1.14 Option B; 8-year CGST audit retention applies; PACT-DRAFT-2-F §3.2 schema encodes |
| I-9 | §S-158 audit-log discipline | **PATCHED-BY-DESIGN** — PACT-DRAFT-2-F §3.3 explicit 3-event enum (campaign_created/state_transition/cancelled); explicit DO-NOT-LOG list for GET /v2/campaigns/active + attribution_window_min polling + Phase 2-D boundary tick reads |
| I-10 | Phase 2-A rate_table FK dependency | **DEPENDS-ON** — sibling §S-249 RCA `RCA-2026-05-13-phase-2-a-rate-table-substrate.md`; ship-sequence: Phase 2-A FILE + ship → Phase 2-F substrate-PR draft |
| I-11 | §S-92 P8 cap surface separation | **PATCHED-BY-DESIGN** — Phase 2-F substrate-PR explicitly DOES NOT encode P8 cap; PACT-DRAFT-2-F §8 wave sequencing relegates cap to Wave 5 pre-send layer |
| I-12 | Concurrent campaign dual-attribution Wave 4 double-count | **UNRESOLVED — DEFERRED to Wave 4 ingestion spec** — PACT-DRAFT-2-F §12 NOT TESTED item 3 explicitly flags; Wave 4 ingestion authors decide whether to divide by `active_campaign_count` or sum unmodified; Phase 2-F substrate ships dual-attribution as designed per Q-2F-5b RATIFIED |

## 4. V2-alignment delta

V2 Phase 2-F Campaign Object primitive per PACT-DRAFT-phase-2-f-campaign-object.md (bono-LEAD; canonical engineering spec; Captain G33 RATIFIED Q-2F-1..7 + james AMPLIFIER AGREE-WITH-CAVEAT-RCA-GATE Q-2F-7):

**A. 2 tables (NEW v2-db migration; Q-2F-1a V2-DB extension)**

```sql
-- Migration: crates/v2-db/migrations/<ts>_phase_2_f_campaigns.sql
CREATE TABLE campaigns (
  campaign_id      TEXT PRIMARY KEY,                                 -- e.g. 'dry-spell-2026-05-11-14h-v1'
  surface          TEXT NOT NULL CHECK (surface IN ('sim','ps5')),  -- Wallet-Framing-C: NEVER 'cafe'
  rate_window_id   TEXT NOT NULL,                                    -- FK → rate_windows.window_id (Phase 2-A)
  broadcast_spec   TEXT NOT NULL,                                    -- JSON; shape per PACT-DRAFT-2-F §6.3
  attribution_id   TEXT NOT NULL,                                    -- opaque correlation tag for attribution batch (UUID)
  validity_start   TEXT NOT NULL,                                    -- ISO8601 UTC; ±2s tolerance per Joint #2
  validity_end     TEXT NOT NULL,                                    -- ISO8601 UTC
  priority         INTEGER NOT NULL DEFAULT 10,                      -- lower = higher priority concurrent
  status           TEXT NOT NULL CHECK (status IN ('DRAFT','APPROVED','LIVE','ENDED','CANCELLED')),
  metadata_json    TEXT,                                             -- optional; Captain notes / promo-code
  created_by       TEXT NOT NULL,                                    -- 'bono' (demand-creation control loop)
  created_at       TEXT NOT NULL,
  FOREIGN KEY (rate_window_id) REFERENCES rate_windows(window_id)
);
CREATE INDEX campaigns_status ON campaigns(status);
CREATE INDEX campaigns_validity ON campaigns(validity_start, validity_end);
CREATE INDEX campaigns_surface_status ON campaigns(surface, status);

CREATE TABLE campaign_attributions (
  id                       INTEGER PRIMARY KEY AUTOINCREMENT,
  campaign_id              TEXT NOT NULL,                          -- FK → campaigns
  customer_id              INTEGER NOT NULL,                       -- FK → customers; ON DELETE RESTRICT per §S-242
  arrived_at_utc           TEXT NOT NULL,
  revenue_paise            INTEGER,                                -- NULL until billing event; GST-inclusive per §S-101
  session_id               TEXT,                                   -- NULL until billing event lands
  attribution_window_min   INTEGER NOT NULL DEFAULT 60,            -- 60min window per ratified Q-2F-default
  attribution_confirmed_at TEXT,                                   -- set when billing event matches
  FOREIGN KEY (campaign_id) REFERENCES campaigns(campaign_id),
  FOREIGN KEY (customer_id) REFERENCES customers(id)
);
CREATE INDEX campaign_attributions_campaign ON campaign_attributions(campaign_id);
CREATE INDEX campaign_attributions_customer ON campaign_attributions(customer_id, arrived_at_utc);
CREATE INDEX campaign_attributions_unconfirmed ON campaign_attributions(campaign_id) WHERE attribution_confirmed_at IS NULL;
```

FK-rewriting awareness (Captain doctrine 2026-05-08 ~22:01 IST): `campaigns.rate_window_id REFERENCES rate_windows(window_id)` + `campaign_attributions.campaign_id REFERENCES campaigns(campaign_id)` + `.customer_id REFERENCES customers(id)`. Any future recreate-table on `rate_windows` requires same-migration rebuild of `campaigns` FK; same for `customers` → `campaign_attributions`. Audit pattern at FILE-time: `grep -rn "REFERENCES campaigns\|REFERENCES rate_windows\|REFERENCES customers" crates/*/migrations/`.

**B. 6 REST endpoints (api-gateway route prefix `/v2/campaigns`)**

| Method | Route | Purpose | Auth |
|---|---|---|---|
| POST | `/v2/campaigns` | Create campaign (DRAFT status) | staff:manager+ |
| POST | `/v2/campaigns/{id}/approve` | Captain approval: DRAFT → APPROVED | staff:captain/admin |
| GET | `/v2/campaigns/active` | List LIVE campaigns (attribution open) | service-key (internal) |
| GET | `/v2/campaigns/{id}` | Single campaign detail | staff:any |
| DELETE | `/v2/campaigns/{id}` | Cancel campaign (any non-ENDED state) | staff:manager+ |
| GET | `/v2/campaigns/{id}/attributions` | Attribution rows | staff:manager+ |

**C. 5-state campaign machine (Q-2F-2a RATIFIED)**

```
DRAFT ──(Captain APPROVE)──► APPROVED ──(broadcast fires)──► LIVE ──(validity_end reached)──► ENDED
  │                                                              │
  └──────────────────(any state)──────────────────────────────►CANCELLED
```

APPROVED is **non-collapsible** intermediate state encoding constitutional Captain-must-approve invariant. Broadcast fires at `broadcast_spec.send_at_utc` (which may be in the future) NOT immediately on approval.

**D. §S-158 audit events (Q-2F-6 RATIFIED)**

Three state-changing action_types LOG (verb_subject snake_case):
- `campaign_created` — POST /v2/campaigns lands
- `campaign_state_transition` — DRAFT→APPROVED / APPROVED→LIVE / LIVE→ENDED / any→CANCELLED
- `campaign_cancelled` — DELETE /v2/campaigns/{id} or CANCELLED transition

DO NOT LOG: GET /v2/campaigns/active re-fetches · attribution_window_min polling · Phase 2-D boundary tick reads.

Additional `campaign_broadcast_voided` action_type fires from Wave 5 pre-send gate (Q-2F-3b) when status=CANCELLED at fire-time — logged at Wave 5 layer not campaigns service.

**E. Cross-organ boundary (PACT-DRAFT-2-F §6.5)**

**Bono cloud authority:** campaigns table CREATE/APPROVE/TRANSITION/CANCEL; campaign_attributions WRITE on billing event receipt; rate_windows table (Phase 2-A); WhatsApp dispatch (Wave 5 trigger).

**James venue authority:** billing calculator (Phase 2-B) computes session_billing_paise from Phase 2-A REST; pod-side surfaces read-only consumers via GET /v2/campaigns/active; at session-close james emits billing event to bono cloud.

**Attribution match (bono cloud on billing event):**
```
billing_event arrives from james {session_id, customer_id, rate_window_id, total_paise, campaign_id?}
  → if campaign_id provided: SELECT campaigns WHERE campaign_id = $1 AND status IN ('LIVE','ENDED')
  → else fallback: SELECT campaigns WHERE rate_window_id = $1 AND status IN ('LIVE','ENDED')
  → lookup arrived_at_utc for customer_id (from check-in event)
  → if (arrived_at_utc - broadcast.send_at_utc) <= attribution_window_min minutes:
      INSERT campaign_attributions(campaign_id, customer_id, arrived_at_utc, revenue_paise, session_id, attribution_confirmed_at=NOW())
```

Q-2F-5b dual-attribution: when 2 LIVE campaigns overlap on same surface, BOTH attributed; campaign_attributions has 2 rows with revenue_paise duplicated. Wave 4 ingestion handles double-counting per its own spec.

**F. Attribution capture mechanism (Q-2F-4b RATIFIED)**

Billing-event match: attribution row written when billing event lands from james (session_id + rate_window_id + customer_id present); revenue_paise populated immediately atomically with attribution row; attribution_confirmed_at = billing-event timestamp. NOT separate check-in event dual-write (rejected Q-2F-4c).

**G. Cancellation hard-void (Q-2F-3b RATIFIED)**

If campaign APPROVED state and broadcast scheduled but not yet fired: DELETE /v2/campaigns/{id} → status=CANCELLED + audit event. Wave 5 pre-send checks read `campaigns.status` at fire-time; if CANCELLED, drop broadcast + log `campaign_broadcast_voided`. Prevents orphaned broadcasts without Captain manual recall.

**H. MI ingestion path (Q-2F-7a RATIFIED — bono recommendation; james AMPLIFIER CAVEAT distinct from Q-2F-7a path)**

Wave 4 MI batch reads campaign_attributions table directly from v2-db (same DB connection; Wave 4 same-host bono cloud). NO REST API layer. NO event-stream. Direct DB read per Wave 4 precedent.

**NOTE on Q-2F-7 disambiguation:** PACT-DRAFT-2-F §9 Q-2F-7 is "MI ingestion path" (3 options); ratified Q-2F-7a is "direct DB read". james AMPLIFIER msg=36386 CAVEAT-1 references Q-2F-7 in the **sibling sense of billing-event payload extension** — both items occupy the Q-2F-7 numeric slot in different framings (PACT-DRAFT Q-asks vs AMPLIFIER caveat numbering). Substrate-PR drafts both: MI direct-DB-read (PACT-2F-7a) ratified clean + billing-event payload extension (AMPLIFIER CAVEAT) requires james-LEAD sub-RCA before 2-F FILE.

**Named gap (R-Phase-2-F):** V2 Campaign Object primitive is the NEW-MECHANISM-CLASS that closes the demand-creation control loop per `v2-skeleton/02-flows-and-roles.md §Demand-Creation-Control-Loop` (MI senses → Bono drafts → Captain approves → Bono publishes → Customer arrives → Registration links to promotion → Pricing applies → Billing reflects → Substrate captures attribution → MI learns). 2 sibling sub-gaps surface: billing-event payload extension (james-LEAD sub-RCA) + Wave 4 ingestion dual-attribution handling (DEFERRED to Wave 4 spec authors).

## 5. V2-framed proposed change

**Phasing (3 sub-phases; ~300-450 LOC; bono cloud-LEAD substrate-PR per PACT-DRAFT-2-F §12 authority basis; james AMPLIFIER on cross-organ billing-event payload extension; FILE-gated on cascade below):**

**Phase 1 — Schema + 2 tables + 6 REST endpoints + 5-state machine** (~250 LOC; depends-on Phase 2-A rate_windows FK target landed)
- v2-db migration: campaigns + campaign_attributions tables + 6 indices + FK-rewriting audit pattern
- `crates/api-gateway/src/v2_campaigns/` module (NEW) — 6 handlers per §4.B table
- 5-state machine encoded as Rust enum + transition fn with explicit constitutional invariants (DRAFT→APPROVED→LIVE→ENDED + CANCELLED side-branch)
- Routes register POST/GET/DELETE per §4.B
- Unit tests cover §S-203 surface CHECK enforcement (cafe REJECTED) + §S-204 Q-2F-1..7 RATIFIED behavior + §S-158 audit events fire on state-changes only + Captain APPROVED non-collapsible invariant + Joint #4 broadcast-only audience_filter shape

**Phase 2 — Attribution capture + Wave 5 cancellation cascade integration** (~120 LOC; depends-on Q-2F-7 billing-event payload sub-RCA james-LEAD landed + Phase 2-B billing-calculator emits extended payload)
- billing-event handler in api-gateway matches incoming `{session_id, customer_id, rate_window_id, total_paise, campaign_id?}` against active campaigns
- attribution row INSERT with revenue_paise + attribution_confirmed_at atomically
- Idempotency `(campaign_id, session_id)` uniqueness constraint
- Wave 5 pre-send check extension: read campaigns.status at fire-time; if CANCELLED, drop + log `campaign_broadcast_voided`
- WS notification `campaign_state_change` emitted to admin surfaces on APPROVED + LIVE transitions
- Integration tests: create → approve → simulate billing event → assert attribution row + revenue_paise + attribution_confirmed_at non-null

**Phase 3 — Wave 4 MI direct-DB read interface + cancellation E2E** (~80 LOC; depends-on Wave 4 MI ingestion FILE landed)
- Wave 4 batch job consumer of campaign_attributions via direct v2-db read (Q-2F-7a path)
- conversion_rate + revenue_lift_paise + attribution_lag_min metrics computed per campaign
- daily_viability_snapshot consumes per Wave 4 §4.3 dual-target schema
- E2E test: cancel APPROVED campaign before broadcast → Wave 5 pre-send drops → assert NO broadcast fired + audit events present + no orphan attribution rows

**Anti-pattern guard:**
- Test asserts surface CHECK enforces `IN ('sim','ps5')`; cafe campaign INSERT FAILS (§S-203 Wallet-Framing-C)
- Test asserts APPROVED → LIVE transition gates on `send_at_utc` (not immediate); Q-2F-2a non-collapsible invariant
- Test asserts §S-158 routine GET /v2/campaigns/active returns 0 new audit_log rows
- Test asserts customer_data_delete on customers with campaign_attributions row → BLOCKED per §S-242 Q-1.14 Option B ON DELETE RESTRICT
- Test asserts broadcast_spec.audience_filter rejects `customer_id` field (Joint #4 broadcast-only)
- Test asserts ±2s tolerance: validity_end boundary tick fires LIVE → ENDED transition within Joint #2 window
- Test asserts dual-attribution Q-2F-5b: 2 LIVE campaigns same surface overlapping → 2 campaign_attributions rows for arriving customer
- Test asserts cancellation hard-void Q-2F-3b: cancel APPROVED + 60min later (post send_at_utc) → no broadcast fired + `campaign_broadcast_voided` logged
- Test asserts `(campaign_id, session_id)` uniqueness blocks billing-event retry storm double-write

**Mechanism-trust check (§S-186 5-Q):**
- (1) atomic primitives? **YES** — campaign INSERT + state-transition + attribution INSERT each single atomic /exec; billing-event handler atomically inserts attribution row with revenue_paise
- (2) TTL-bounded sentinels? **N/A** (no long-lived sentinels; campaigns.status drives everything; ENDED is immutable; CANCELLED is immutable)
- (3) behavioral-verify success? **YES** — Phase 2 + Phase 3 contract tests verify attribution row presence with non-null revenue_paise; admin dashboard surfaces LIVE campaign count
- (4) single-target dry-run? **YES** — staging endpoint + unit/integration/E2E test hierarchy; Class A Captain-manual path is itself the dry-run for Class B auto-fire
- (5) guard contracts? **YES** — bono cloud sole writer; james venue read-only consumer + billing-event emitter; api-gateway staff:captain/admin auth on /approve; staff:manager+ auth on POST/DELETE; service-key on GET /active; §S-203 surface CHECK constraint; §S-242 ON DELETE RESTRICT
- **Verdict: PASS** (V2-aligned; substrate-PR can proceed once cascade dependencies clear)

**Mechanism-trust dependencies blocking ship (cascade per james AMPLIFIER msg=36386):**
1. Wave 1 SessionBillingService + refund routing + idle-timeout + PIN-LOCKOUT LAND (verify-by 2026-05-21)
2. Phase 2-A rate_table service FILE'd (sibling §S-249 RCA `RCA-2026-05-13-phase-2-a-rate-table-substrate.md`; PACT-DRAFT-2-A activation_trigger gates on #1)
3. james-LEAD billing-event payload extension §S-146 5-section RCA (sub-RCA forward-defer; NOT in this §S-249 cascade) — extends V1 payload to include `campaign_id: nullable<ulid>` + `rate_window_id: nullable<ulid>`; classification: single-boundary, ≤200 LOC, protocol-change → full §S-146 likely required (foundational boundary contract)
4. PACT-DRAFT-phase-2-f-campaign-object.md slot-RESERVE + FILE (currently AMPLIFIER-READY; gates on #1-#3)
5. Wave 5 WhatsApp framework FILE'd (BLOCKER for Class B auto-fire; Class A Captain-manual path works pre-Wave-5)
6. Wave 4 MI ingestion FILE'd (Phase 3 of substrate-PR; not blocker for Phase 1+2 ship)

**V2 doctrine alignment statement:**
> V2 doctrine alignment: closes 1 of 19 V1→V2 STRUCTURAL GAPS (G-Phase-2-F-1 NEW-MECHANISM-CLASS Campaign Object primitive); surfaces 1 sibling sub-gap (G-Phase-2-F-2 billing-event payload extension; james-LEAD sub-RCA). Establishes V2 cloud-side authoritative campaign substrate per V2-PROGRESS-MAP §2 W2 Phase 2-F (Layer 10.15 AMPLIFIER-READY) + §6.10 row Q-2F-1..7 Captain G33 RATIFIED §S-204. Encodes §S-203 Wallet-Framing-C surface CHECK (sim/ps5 NEVER cafe) + §S-204 all-bono-recommendations RATIFIED Q-2F-1..7 + §S-158 audit-log discipline (3-event enum + DO-NOT-LOG list) + §S-170 MI mission-journal substrate + Joint #4 broadcast-only audience_filter + Joint #2 ±2s tolerance + Captain APPROVED non-collapsible constitutional invariant + §S-242 Q-1.14 DPDP Option B (ON DELETE RESTRICT for revenue-bearing rows). Closes demand-creation control loop per `v2-skeleton/02-flows-and-roles.md`. Composes-with sibling §S-249 RCA Phase 2-A rate_table substrate (FK target prereq) · §S-248 batch RCA #8 Phase 2-E combo-offer (campaign-line-item candidate via broadcast_spec.offer_ids) · §S-248 batch RCA #9 Phase 2 dyn-pricing engine (campaign-object compose-with engine §4.A campaign_object.rs module). Cancellation hard-void Q-2F-3b prevents orphaned broadcasts. Dual-attribution Q-2F-5b preserves MI learning richness; Wave 4 ingestion handles double-counting per its own spec.

## Captain decision queue

| Decision | Status |
|---|---|
| **D-Phase-2-F-1** PACT-DRAFT-2-F slot-RESERVE + FILE | AMPLIFIER-READY-GATED on Wave 1 ship + Phase 2-A FILE + billing-event payload sub-RCA |
| **D-Phase-2-F-2** Substrate-PR Phase 1 schema + 6 endpoints + 5-state machine | AUTHORED-PENDING (this RCA = §S-146 V1↔V2 gate prereq satisfied) |
| **D-Phase-2-F-3** MMA Step 1 DIAGNOSE (foundational) | bono OpenRouter; AWAITING-Captain-budget-auth (~$1 share of §S-248.D-2 batch) |
| **D-Phase-2-F-4** Q-2F-1..7 Captain G33 disposition | ✅ **RATIFIED §S-204 2026-05-12 ~11:05 IST** verbatim "all bono recommendations" |
| **D-Phase-2-F-5** Q-2F-7 billing-event payload extension sub-RCA (james-LEAD) | DEPENDS-ON Wave 1 ship + Phase 2-A FILE; sub-RCA author = james (LEAD per AMPLIFIER msg=36386 CAVEAT-1) |
| **D-Phase-2-F-6** Per-PR Captain merge auth on substrate-PR (foundational boundary) | AWAITING substrate-PR draft |
| **D-Phase-2-F-7** Wave 4 dual-attribution double-count handling (Q-2F-5b → Wave 4 spec) | DEFERRED-to-Wave-4-ingestion-spec |
| **D-Phase-2-F-8** Wave 5 framework FILE (BLOCKER for Class B auto-fire) | Class A Captain-manual path works pre-Wave-5 |

## Composes-with

- [⭐⭐ V1-dep V2 RCA doctrine](feedback_v1_dependent_v2_root_cause_before_proceeding.md)
- [⭐ Mechanism-trust-check upstream of fix RCA §S-172](feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md)
- [Pre-§S-146 small-fix fast-lane (§S-186)](feedback_pre_s146_small_fix_fastlane_20260511.md) — borderline-eligibility classification of Q-2F-7 sub-RCA
- **Sibling §S-249 RCA `RCA-2026-05-13-phase-2-a-rate-table-substrate.md`** (FK target prereq)
- **Companion PACT-DRAFT-phase-2-f-campaign-object.md** (bono-LEAD engineering spec; Captain RATIFIED Q-2F-1..7; james AMPLIFIER AGREE-WITH-CAVEAT-RCA-GATE msg=36386)
- RCA-2026-05-13-phase-2-e-combo-offer-primitive (§S-248 batch RCA #8) — broadcast_spec.offer_ids[] composer
- RCA-2026-05-13-phase-2-dynamic-pricing-engine (§S-248 batch RCA #9) — campaign_object.rs module §4.A integration
- RCA-2026-05-13-q-1.14-dpdp-v2-clean-separation (§S-242 Q-1.14 Option B DPDP doctrine — ON DELETE RESTRICT for revenue-bearing rows)
- `project_dynamic_pricing_synthesis_20260509.md §10` — synthesis gap closure (campaign = MI mission journal)
- `project_mi_mission_statement_mini_jaeger_20260509.md` §S-170 — MI STABLE* mini-Jaeger frame empirical substrate
- `v2-skeleton/02-flows-and-roles.md §Demand-Creation-Control-Loop` — canonical loop closure
- `v2-skeleton/04-connection-matrix.md connection_dynamic_pricing_substrate` — attribution rows substrate definition
- §S-49 Captain G33-GUIDE-CONFIRM Level B (primary workflow spec; rule 1 window-time-based eligibility)
- §S-91 §RATE-TABLE canonical binding (rate_window_id FK target lives in rate_windows)
- §S-101 GST-INCLUSIVE doctrine (revenue_paise semantics)
- §S-158 V2 Audit-Log Doctrine (3-event enum + DO-NOT-LOG list)
- §S-170 MI STABLE* architectural layer
- §S-203 Wallet-Framing-C LOCKED (surface CHECK doctrine)
- §S-204 Q-2F-1..7 Captain G33 RATIFIED 2026-05-12 ~11:05 IST
- §S-242 Q-1.14 DPDP V2-Clean Option B doctrine
- §S-248 9-RCA batch parent cascade (this RCA = §S-249.2 follow-up)
- PACT-DRAFT-wave-5-whatsapp-workflow-framework-captain-curated.md (Wave 5 broadcast_spec transport; Class A path)
- PACT-DRAFT-wave-4-mi-ingestion-rp-internal.md (Wave 4 consumer of campaign_attributions)
- PHASE-2-B-BONO-CLOUD-SURFACES-REFERENCE.md (commit abbf52a8) — billing event payload extension contract bilateral sync
- Captain doctrines 2026-05-08 ~22:01 IST (sqlx::migrate cache invalidation + SQLite RENAME FK rewriting)
- Captain approves campaigns constitutional invariant (v2-skeleton/02; APPROVED non-collapsible)
- Joint #4 broadcast-only / non-personalized (audience_filter tier-class shape)
- Joint #2 Billing 2-second tolerance (validity_start/end boundary tick)

## Stale-at

2026-08-13 (90 days). Re-read against current code state before substrate-PR derivation — PACT-DRAFT-2-F may have advanced past AMPLIFIER-READY state; Q-2F-7 billing-event payload sub-RCA may have landed; Wave 4 ingestion spec may have changed dual-attribution handling.
