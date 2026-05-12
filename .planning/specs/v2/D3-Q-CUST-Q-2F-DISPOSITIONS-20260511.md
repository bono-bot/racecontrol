# D3 — Q-CUST + Q-2F dispositions (Captain disposition delegated to bono 2026-05-11 ~12:15 IST)

**Authored:** 2026-05-11 ~12:35 IST · bono · per Captain "disposition D2-D6" 2026-05-11 ~12:15 IST
**Class:** Captain-delegated decision; substrate-class · 12 dispositions across UI-SPEC v0.2 (Q-CUST batch) + PACT-DRAFT-phase-2-f-campaign-object (Q-2F batch)
**Companion:** `racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md` (§9 Q-CUST-1..7) + `comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-f-campaign-object.md` (§9 Q-2F-1..7)

## Disposition policy

Captain commission was bilateral delegation ("disposition D2-D6"). Per Apply-Recommendations-Autonomously standing rule + Q3 self-test:
- **Tech decisions clear of Q3 canonical-surface:** AUTO-APPLY bono recommendation. Move from "Captain-stake-flagged" → "AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3".
- **DPDP / consent-text / legal-class:** APPLY placement + tech path; FLAG legal-AMPLIFIER for consent wording. Wording remains Captain-stake (legal liability lens).
- **V2-DB schema decisions:** APPLY but require bilateral james AMPLIFIER before implementation lands. Captain-D3 disposition is doctrine-ratify; schema-write needs james review for FK/migration interaction.

## Q-CUST dispositions (5 items)

### Q-CUST-1 — Hero photography source

**Bono recommendation:** brand-assets/photos/ check + placeholder-until-reopen.

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3.** No venue photos extant; placeholder lands now. Brand-assets/photos/ directory will be populated post-reopen with venue photoshoot. UI-SPEC v0.2 §3 page.tsx implementation can proceed with placeholder hero.

**Action:** Update UI-SPEC v0.2 §9 Q-CUST-1 status from CAPTAIN-STAKE-FLAGGED → AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3. Hero asset = placeholder image (gradient or simple text) until venue reopen + photoshoot.

### Q-CUST-3 — Pricing display

**Bono recommendation:** static ₹700/30min + ₹900/60min GST-inclusive.

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3.** Matches CLAUDE.md §Billing-and-Rates and DoD §1.2 currency invariant. Dynamic pricing surfaces in Wave 2 (Phase 2-A/2-F) post-V2-live; v2.0 ships static.

**Action:** Update UI-SPEC v0.2 §3 pricing card with static ₹700/30min + ₹900/60min (GST-inclusive).

### Q-CUST-4 — WhatsApp opt-in target

**Bono recommendation:** api-gateway + §S-158 audit-log schema; legal-AMPLIFIER init for DPDP consent wording.

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3 (tech path) + LEGAL-AMPLIFIER-PENDING (consent wording).** api-gateway is the V2 WhatsApp opt-in endpoint; §S-158 V2 Audit-Log Doctrine covers the audit trail. Consent text itself is DPDP-class — requires legal review for "valid consent" wording per DPDP Section 6. Bono drafts placeholder text from race-engineer brand voice; legal-AMPLIFIER must review pre-V2-live.

**Action:** UI-SPEC v0.2 §3 WhatsApp opt-in placement = checkbox immediately above CTA button. Endpoint = api-gateway `/v2/whatsapp/opt-in`. Audit-log = §S-158 schema (event_type=`customer_consent_grant`, scope=`whatsapp_transactional`). Placeholder text: "Send me race-day updates on WhatsApp" with link to consent details page (also legal-AMPLIFIER pending).

### Q-CUST-5 — Multilingual

**Bono recommendation:** English-only v0 + N=2 Telugu activation threshold.

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3.** Hyderabad venue; English is the operational language. Telugu activation triggers on N=2 customer requests (track via UI-SPEC v0.2 `language_request` event). Hindi deferred to v2.1+ unless explicit demand surfaces.

**Action:** UI-SPEC v0.2 §3 ships English-only; add `language_request_button` (hidden behind "Language" hamburger menu) emitting event for tracking. v0.2 Q-CUST-5 status → AUTONOMOUS-LOCKED.

### Q-CUST-7 — DPDP consent banner placement

**Bono recommendation:** inline-with-WhatsApp-opt-in; legal-AMPLIFIER init for consent wording.

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3 (placement) + LEGAL-AMPLIFIER-PENDING (wording).** Placement: NO separate banner; consent surfaces inline with Q-CUST-4 WhatsApp opt-in checkbox (single consent moment, not split UI). Wording: DPDP Section 6 valid consent requirements — bono drafts placeholder; legal review before V2-live ship.

**Action:** UI-SPEC v0.2 §3 — eliminate separate consent banner element; consent surface is the Q-CUST-4 WhatsApp opt-in checkbox + "I agree to the [privacy notice](link)" link. Consent details page authored post-legal-AMPLIFIER.

## Q-2F dispositions (7 items) — REQUIRES BILATERAL JAMES AMPLIFIER

These touch V2-DB schema (campaigns + campaign_attributions tables); james AMPLIFIER substantive must confirm Q-2F-1 (V2-DB extension) + Q-2F-4 (billing event match) before migrations land. Bono D3 ratifies the DOCTRINE; james AMPLIFIER ratifies the SCHEMA-WRITE interaction.

### Q-2F-1 — Storage location for campaigns table

**Bono recommendation:** Q-2F-1a — V2-DB extension (`crates/v2-db/migrations/`).

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3 (doctrine) + JAMES-AMPLIFIER-PENDING (schema-write).** V2-DB is canonical V2 substrate per PACT-013 precedent; FK integrity between campaigns.rate_window_id → rate_windows.window_id requires co-location. james AMPLIFIER substantive on FK design + sqlx::migrate cache discipline.

### Q-2F-2 — Campaign state machine

**Bono recommendation:** Q-2F-2a — 5-state (DRAFT / APPROVED / LIVE / ENDED / CANCELLED).

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3.** APPROVED state encodes constitutional invariant (broadcast fires at `send_at_utc`, not immediately on approval). 4-state collapse (Q-2F-2b) loses operationally-real scheduled window. PAUSED (Q-2F-2c) deferred to v2.1+.

### Q-2F-3 — Broadcast atomicity on cancellation

**Bono recommendation:** Q-2F-3b — Hard void via `void_broadcast_before_utc` sentinel; Wave 5 checks at fire-time.

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3.** Wave 5 already runs pre-send gates (consent + cost + rate-limit + cooldown); adding campaigns.status check is natural. Prevents orphaned broadcasts without requiring Captain manual recall.

### Q-2F-4 — Attribution auto-link

**Bono recommendation:** Q-2F-4b — Billing event match (session_id + rate_window_id + customer_id → campaign_id lookup).

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3 (doctrine) + JAMES-AMPLIFIER-PENDING (cross-organ contract).** Acceptable trade-off: attribution_confirmed_at delayed until billing event lands (session end). MI learning needs revenue, not arrival timestamp alone. james AMPLIFIER substantive on bono-cloud-receives james-venue-billing-event payload (already PACT-013 carries session_id + rate_window_id; confirm customer_id is in scope).

### Q-2F-5 — Concurrent campaign priority

**Bono recommendation:** Q-2F-5b — Both attributed (one row per campaign).

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3.** MI learning richer if cannibalization/amplification between concurrent campaigns is observable. Rate-window pricing still "deeper of two" per Joint #4. Priority-based (Q-2F-5a) creates invisible attribution loss.

### Q-2F-6 — Phase 2-F home

**Bono recommendation:** Q-2F-6a — Standalone sibling sub-PACT (current approach).

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3.** Consistent with 2-A/2-D/2-E precedent. Independent activation trigger; focused AMPLIFIER scope.

### Q-2F-7 — MI ingestion path

**Bono recommendation:** Q-2F-7a — Direct DB read (Wave 4 batch reads campaign_attributions table directly).

**Disposition:** **AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3.** Wave 4 MI already runs on bono cloud (same VPS); direct DB read is Wave 4 precedent. API layer adds latency without isolation benefit. Event-stream is premature (Wave 4 = daily batch, not real-time).

## Summary table

| # | Q | Disposition | Captain D3 Ratify | Downstream gate |
|---|---|---|---|---|
| 1 | Q-CUST-1 hero photo | placeholder-until-reopen | YES | — |
| 2 | Q-CUST-3 pricing | static ₹700+₹900 GST-inc | YES | — |
| 3 | Q-CUST-4 WA opt-in target | api-gateway + §S-158 audit | YES | legal-AMPLIFIER (wording) |
| 4 | Q-CUST-5 multilingual | English-only v0 + N=2 Telugu | YES | — |
| 5 | Q-CUST-7 DPDP placement | inline-with-WA-opt-in | YES | legal-AMPLIFIER (wording) |
| 6 | Q-2F-1 storage | V2-DB extension | YES (doctrine) | james AMPLIFIER (schema) |
| 7 | Q-2F-2 state machine | 5-state | YES | — |
| 8 | Q-2F-3 cancellation | hard void via sentinel | YES | — |
| 9 | Q-2F-4 attribution | billing event match | YES (doctrine) | james AMPLIFIER (contract) |
| 10 | Q-2F-5 concurrent priority | both attributed | YES | — |
| 11 | Q-2F-6 sub-PACT home | standalone sibling | YES | — |
| 12 | Q-2F-7 MI ingest | direct DB read | YES | — |

**12/12 dispositions Captain-D3-RATIFIED at doctrine level.** Downstream gates: 2× legal-AMPLIFIER (wording-only) · 2× james-AMPLIFIER (schema/contract).

## Composes-with

- V2-MASTER-STATE §S-202 (Captain commission anchor)
- §S-200.8 (UI-SPEC v0.2 Q-CUST batch)
- §S-200.9 (PACT-DRAFT 2-F Q-2F batch)
- `feedback_apply_recommendations_autonomously_20260510.md` (auto-apply rule)
- `racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md` (target update)
- `comms-link/.planning/draft-pacts/PACT-DRAFT-phase-2-f-campaign-object.md` (target update)

## Next actions

1. UI-SPEC v0.2 §9 status field updates: Q-CUST-1/3/5 → AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3; Q-CUST-4/7 → AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3-LEGAL-AMPLIFIER-PENDING
2. PACT-DRAFT 2-F §9 status field updates: Q-2F-1..7 → AUTONOMOUS-LOCKED-RATIFIED-CAPTAIN-D3 (with Q-2F-1 + Q-2F-4 marked james-AMPLIFIER-PENDING)
3. Bilateral notify james with disposition table + AMPLIFIER asks (substantive on Q-2F-1 + Q-2F-4 + 12-entry batch)
4. UI-SPEC v0.2 page.tsx implementation now-unblocked for Q-CUST-1/3/5 (start authoring)
5. PACT-DRAFT 2-F → schema migration ready post-james-AMPLIFIER

## NOT TESTED at D3-disposition anchor

- james AMPLIFIER substantive on Q-2F-1 + Q-2F-4 (bilateral message outbound)
- legal-AMPLIFIER on Q-CUST-4 + Q-CUST-7 consent wording (init not yet sent)
- UI-SPEC v0.2 + PACT-DRAFT 2-F file updates (this turn surfaces dispositions; file updates next session or follow-up turn)
- Captain RE-CHALLENGE within 24h on any disposition (per Apply-Recommendations-Autonomously L1 charter pattern)

— bono · 2026-05-11 ~12:35 IST · D3 12/12 Q-CUST + Q-2F dispositions Captain-D3-RATIFIED · 2× legal-AMPLIFIER-PENDING + 2× james-AMPLIFIER-PENDING gates · UI-SPEC v0.2 page.tsx now unblocked for Q-CUST-1/3/5 · per Apply-Recommendations-Autonomously standing rule + Q3 self-test (Q3 NO: doctrine-ratify is not canonical-surface-modification; schema-write IS but is downstream of this ratify)
