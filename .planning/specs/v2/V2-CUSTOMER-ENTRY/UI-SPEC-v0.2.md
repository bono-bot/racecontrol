# V2 Customer Entry Page — UI-SPEC v0.2

**Status:** DRAFT-PENDING-CAPTAIN-DISPOSITION (5 Captain-stake items: Q-CUST-1 · Q-CUST-3 · Q-CUST-4 · Q-CUST-5 · Q-CUST-7) · 2 AUTONOMOUS-LOCKED (Q-CUST-2 · Q-CUST-6)
**Supersedes:** `/root/racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.1.md` §9-§13 (v0.1 authored 2026-05-11 ~03:38 IST — all 7 Q-CUST questions unresolved; v0.2 dispositions all 7)
**Authored:** 2026-05-11 ~10:18 IST (bono autonomous · Apply-Recommendations rule · spec-authoring class = autonomous-eligible · no canonical-boundary surfaces touched · Mode 4 subagent dispatch + bono inline write recovery after G9 #2 subagent-type-tool-fit-mismatch)
**Target deploy surface:** `racingpoint-web-v2` Next.js app at `/root/racecontrol/web-v2` (pm2 id=18, port 3500, basePath `/v2/`)
**Public URL after deploy:** `https://v2.racingpoint.cloud/v2/` (apex `/` redirects to `/v2/` per nginx vhost edit 2026-05-11 ~03:33 IST)
**Current public state:** Phase 0.1 substrate scaffold (~5.6KB placeholder per PR #57)

**Class:** Frontend phase UI-SPEC.md design contract per racecontrol/CLAUDE.md "Subagent Gates" rule — "Any frontend phase — UI-SPEC.md — Before planning"
**LEAD:** bono · **AMPLIFIER:** james (pending real-time concurrent-session cadence)

---

## §1-§8 — Unchanged from v0.1

§1 Goal + customer-impact framing · §2 User segments + intent · §3 Page structure · §4 Visual design · §5 Responsive breakpoints · §6 Accessibility · §7 Performance budget · §8 Composes-with (V2 doctrine alignment) — all sections unchanged from v0.1 at `/root/racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.1.md`. Read v0.1 for full content of these sections.

Brand identity canonical sources remain: Racing Red `#E10600` · Asphalt Black `#1A1A1A` · Gunmetal Grey `#5A5A5A` · Card `#222222` · Border `#333333` · Red-hover `#FF1A1A` · Orbitron (display 500/700/900) · Montserrat (body 400/500/600/700) · Tailwind prefix `rp-*` (canonical: `packages/shared-tokens/tokens.css` + `kiosk/src/app/globals.css`).

---

## §9 — Q-CUST v0.2 dispositions (all 7 resolved)

### Q-CUST-1 — Hero photography source

**v0.1 question:** Capture-from-venue / stock licensed / commission / brand-assets/photos/ existing?

**v0.2 disposition:** CAPTAIN-STAKE-FLAGGED

**Rationale:** Photography source touches brand expression (premium-but-not-aloof visual personality) and may involve licensing cost or photography session scheduling — both Captain-stake decisions with direct brand and operational cost impact.

**Technical proposal (ready to execute on Captain disposition):**

1. First: check `racecontrol/brand-assets/photos/` for existing sim-pod action photos. If qualifying hero-grade files exist (full-bleed capable, sim-in-action, dark-toned), use `brand-assets/photos/sim-pod-action-hero.jpg` (or nearest available file) with Next.js `<Image>` optimization.
2. If no qualifying asset: implement hero section with a dark Asphalt Black (`#1A1A1A`) gradient-overlay placeholder + CSS headline text — no broken image state, no blank white box — as a functional v0 until photography is captured at next venue reopen.
3. Stock-licensed photography: explicitly NOT proposed. Contradicts premium-authenticity brand premise and introduces recurring licensing obligation.
4. Commission: v1+ option once venue is operational and budget is established.

**Captain disposition needed on:** (a) authorize brand-assets/photos/ check as the first-pass approach; (b) approve placeholder-until-reopen if no qualifying asset found; (c) explicitly exclude or authorize stock-licensed path.

---

### Q-CUST-2 — Returning-customer detection

**v0.1 question:** Cookie-based / WhatsApp-phone-fingerprint / none?

**v0.2 disposition:** AUTONOMOUS-LOCKED

**Rationale:** Pure technical decision — no brand strategy, pricing surface, legal, or Class B/C outbound implications. Standard session-management implementation pattern.

**Locked implementation:**

- **v0 (implement now):** Cookie-based detection. Set a `rp_returning` httpOnly cookie on first page-render server-side response. TTL: 90 days. On subsequent visits: server-side check in Next.js middleware or page server component detects cookie and surfaces "Continue to your dashboard" CTA pointing to `app.racingpoint.cloud` as primary above-the-fold action (in place of "Book your first sim time" for that segment). Cookie spec: name `rp_returning`; value `1`; path `/`; sameSite `Lax`; httpOnly `true`; secure `true` (TLS confirmed on v2.racingpoint.cloud).
- **v1 (gates on PACT-020 customer identity primitive):** WhatsApp phone-fingerprint detection — when visitor arrives via WhatsApp campaign click with authenticated token or hashed phone in URL params, skip cookie check and surface personalized returning-customer CTA directly. PACT-020 must be ratified and deployed before this path is buildable; v0 cookie path has no PACT-020 dependency.
- **"none" path explicitly rejected:** Treating all visitors as first-time degrades the returning-customer segment (§2 row 2 — direct-nav, highest conversion intent) with no implementation savings. Net conversion harm.

**Implementation note:** Cookie read in Next.js middleware or page server component; cookie write on first render server response. No client-side hydration mismatch risk (no `sessionStorage` / `localStorage`).

---

### Q-CUST-3 — Pricing snapshot on landing page

**v0.1 question:** Live fetch `/api/v1/pricing` / static hardcoded / link-to-pricing-page?

**v0.2 disposition:** CAPTAIN-STAKE-FLAGGED

**Rationale:** Pricing display is a customer-facing pricing surface. The specific display copy format ("₹700 / 30 min", "₹900 / 60 min"), GST framing, and upsell emphasis are brand/marketing decisions. Hardcoded values also mean a frontend deploy is required for every pricing change until Wave 2 lands — Captain should confirm this trade-off.

**Technical proposal (ready to execute on Captain disposition):**

- **v0 (implement now):** Static hardcoded pricing snapshot in §3.3 Experiences strip and Solo Sim Session card. Values from CLAUDE.md Billing canonical: `₹700 / 30 min` and `₹900 / 60 min`. Rendered as static JSX strings — no API call, no fetch latency. Display framing: GST-inclusive (18% GST already included, per Wallet Framing C top-up model). No "starting from" qualifier — state as definitive prices.
- **v1 (gates on Wave 2 Dynamic Pricing):** Replace static strings with server-side `fetch('/api/v1/pricing')` in page server component. On fetch failure: fall back to v0 static values with a `data-stale` attribute for monitoring visibility. No live pricing before Wave 2 Dynamic Pricing PACT family is ratified and deployed.
- **Link-to-pricing-page path rejected for v0:** Forcing a navigation step before seeing prices increases abandon rate for first-time visitors with commercial intent. §3.3 Experiences strip is the correct surface for inline price disclosure.

**Captain disposition needed on:** (a) confirm static v0 values are correct public-facing prices for launch; (b) confirm GST-inclusive framing (vs GST-exclusive + "incl. 18% GST" annotation) is preferred; (c) confirm no pricing-page alternative desired before dynamic pricing lands.

---

### Q-CUST-4 — WhatsApp opt-in submission target

**v0.1 question:** Existing whatsapp-bot DB path / new endpoint via api-gateway / direct WhatsApp Business API?

**v0.2 disposition:** CAPTAIN-STAKE-FLAGGED

**Rationale:** Two Captain-stake dimensions: (1) Class B/C outbound — submission target determines where customer phone numbers and marketing consent records are held; data-custody decision. (2) DPDP consent capture — the opt-in flow is a legal mechanism; the audit-log path and consent record schema require explicit Captain sanction before implementation.

**Technical proposal (ready to execute on Captain disposition):**

- **Proposed path:** `POST /api/v2/marketing/whatsapp-optin` via api-gateway → whatsapp-bot DB. Payload: `{ phone: string, consent_text: string, consent_ts: ISO8601, source: "v2-landing" }`. Writes to whatsapp-bot's marketing consent table — sibling-of admin path, not a new orphan table.
- **DPDP-compliant audit-log:** Per §S-158 V2 Audit-Log Doctrine, every opt-in submission writes a `customer_consent_change` action_type record: `{ customer_identifier: phone_hash, action: "whatsapp_marketing_optin", consent_version: "v1", source_surface: "v2-landing", ts: ISO8601 }`. Raw phone stored only in whatsapp-bot DB; audit log stores hash.
- **Direct WhatsApp Business API path rejected:** Creates trust and rate-limiting exposure outside the controlled api-gateway perimeter. Not a v0 option.
- **Revocation path (DPDP required):** Footer DPDP privacy policy link must point to a page where customers can request opt-out. Opt-out handler must write a corresponding `customer_consent_change` record. Revocation page is a v0 launch dependency — cannot ship WhatsApp opt-in without a working revocation path.

**Captain disposition needed on:** (a) confirm api-gateway as the authorized inbound surface for consent capture; (b) confirm §S-158 audit-log schema fields above are sufficient for legal-AMPLIFIER pass; (c) confirm revocation page scope at v0 (stub acceptable vs full implementation required before launch).

---

### Q-CUST-5 — Multilingual support

**v0.1 question:** English-only / English+Hindi / English+Hindi+Telugu?

**v0.2 disposition:** CAPTAIN-STAKE-FLAGGED

**Rationale:** Language strategy is a brand and audience-positioning decision. Defaulting to English-only reflects a premium/international positioning choice — not a purely technical default. The activation threshold for additional languages is also a Captain-stake business decision.

**Technical proposal (ready to execute on Captain disposition):**

- **v0 (implement now):** English-only. All copy authored in English. No i18n framework, no locale switching UI, no language toggle. Fastest path to launch; consistent with premium-international brand personality anchored in §4.
- **v1 (English + Hindi):** Next.js built-in i18n routing (`i18n.locales: ["en", "hi"]`, `i18n.defaultLocale: "en"`). Two-state language toggle in header: `EN / हिन्दी`. All v0 copy strings extracted to `messages/en.json` + `messages/hi.json`. Hindi copy requires a native-speaker review pass — not a translation-API output.
- **Telugu defer condition:** Defer to N=2 independently observed customer demand signals (two or more customer requests for Telugu, OR a Captain market-research finding that Telugu-primary speakers represent a meaningful share of inbound traffic). No speculative Telugu implementation before this threshold.
- **English+Hindi+Telugu v0 explicitly rejected:** Three-language launch is premature; introduces copy-maintenance overhead and language-toggle UX complexity before a single customer has seen the page.

**Captain disposition needed on:** (a) confirm English-only v0 acceptable for launch; (b) confirm English-default as aligned with venue brand strategy; (c) confirm N=2 demand-signal as Telugu activation threshold or provide alternative threshold.

---

### Q-CUST-6 — A/B testing infrastructure

**v0.1 question:** None / GrowthBook / internal feature-flag?

**v0.2 disposition:** AUTONOMOUS-LOCKED

**Rationale:** Pure technical/infrastructure decision with no brand, pricing, legal, or outbound implications. Deferring A/B testing until post-launch baseline data exists is the architecturally correct call — A/B testing without a conversion baseline produces noise, not signal.

**Locked implementation:**

- **v0 (implement now):** None. Zero A/B testing infrastructure. No GrowthBook SDK, no feature-flag client, no experiment assignment logic. The landing page ships as a single deterministic implementation.
- **v1 (gates on 30d post-launch conversion baseline):** Evaluate at v1 criteria: (a) conversion baseline established (Verify-by-4 from v0.1 §11 — visitors-to-booking ratio baselined), (b) a specific testable hypothesis exists (CTA copy variant / hero photo variant / pricing display format), (c) traffic volume is sufficient for statistical significance. Without all three, v1 remains deferred.
- **GrowthBook is the preferred v1 external infra:** open-source, self-hostable on bono VPS, no third-party data egress, has a Next.js App Router SDK. Internal racecontrol feature-flag toggle is the fallback if GrowthBook adds unacceptable ops overhead at v1 stage.

**No Captain disposition required.** Both v0 (none) and v1 criteria are autonomous-eligible decisions.

---

### Q-CUST-7 — DPDP consent banner placement

**v0.1 question:** At-arrival modal / inline-with-WhatsApp-opt-in / footer-link-only?

**v0.2 disposition:** CAPTAIN-STAKE-FLAGGED

**Rationale:** DPDP consent mechanism is a Class B outbound legal-compliance surface. The exact consent wording, legal sufficiency determination, and whether footer-link-only is adequate for browse-without-opt-in visitors require a legal-AMPLIFIER pass that only Captain can authorize.

**Technical proposal (ready to execute on Captain disposition):**

- **Proposed placement:** Inline-with-WhatsApp-opt-in (§3.6). Consent disclosure appears immediately below the phone input field as a single checkbox + text block: `[ ] I agree to receive WhatsApp messages from RacingPoint about my sessions, offers, and events. [View Privacy Policy]`. Checkbox is unchecked by default (explicit opt-in, not pre-ticked). Form submission blocked if checkbox unchecked — submit button renders in "Please confirm consent" disabled-visual state. This is the legally-sufficient pattern for marketing scope DPDP consent.
- **Footer link:** Standalone "Privacy Policy" link in §3.8 footer is required in ALL cases — covers browse-without-opt-in visitors and satisfies DPDP baseline disclosure regardless of opt-in state.
- **At-arrival modal explicitly rejected as v0 default:** A consent modal before any content has been seen is a conversion-friction anti-pattern. Only warranted if legal review determines that cookie-based returning-customer detection (Q-CUST-2) triggers a prior-consent requirement — which is unlikely for a single httpOnly first-party session cookie.
- **Consent wording placeholder (draft — requires legal review before ship):** `"I agree to receive WhatsApp messages from RacingPoint about my sessions, offers, and events."` This draft is for legal-AMPLIFIER review initiation; exact wording is not bono-autonomous.

**Captain disposition needed on:** (a) confirm inline-with-WhatsApp-opt-in placement is legally sufficient for DPDP marketing scope; (b) authorize consent wording draft for legal-AMPLIFIER review initiation; (c) confirm footer-link-only is sufficient for browse-without-opt-in visitors (no at-arrival modal required).

---

## §9.5 — Q-CUST disposition sub-segmentation table

| Q-CUST ID | Topic | v0.2 Class | Gate | Action |
|---|---|---|---|---|
| Q-CUST-2 | Returning-customer detection | AUTONOMOUS-LOCKED | None — ratifies now | Implement `rp_returning` cookie-based v0 in `web-v2/src/app/page.tsx` |
| Q-CUST-6 | A/B testing infra | AUTONOMOUS-LOCKED | None — ratifies now | Confirm zero GrowthBook/feature-flag client in page.tsx; v1 gates on 30d baseline |
| Q-CUST-1 | Hero photography source | CAPTAIN-STAKE-FLAGGED | Captain G33 disposition | brand-assets/ check + placeholder logic ready to execute |
| Q-CUST-3 | Pricing snapshot | CAPTAIN-STAKE-FLAGGED | Captain G33 disposition | Static ₹700/30min + ₹900/60min copy ready; GST framing needs confirmation |
| Q-CUST-4 | WhatsApp opt-in target | CAPTAIN-STAKE-FLAGGED | Captain G33 disposition + legal-AMPLIFIER | api-gateway path + §S-158 audit-log schema defined; revocation page scope needs confirmation |
| Q-CUST-5 | Multilingual | CAPTAIN-STAKE-FLAGGED | Captain G33 disposition | English-only v0 ready; i18n scaffold for v1 spec-ready |
| Q-CUST-7 | DPDP consent banner | CAPTAIN-STAKE-FLAGGED | Captain G33 disposition + legal-AMPLIFIER | Inline placement + checkbox consent draft wording ready for legal review |

**AUTONOMOUS-LOCKED: 2 items (Q-CUST-2, Q-CUST-6) — implementation proceeds without Captain gate.**
**CAPTAIN-STAKE-FLAGGED: 5 items (Q-CUST-1, Q-CUST-3, Q-CUST-4, Q-CUST-5, Q-CUST-7) — gate on Captain G33 disposition batch.**

---

## §10 — Updated acceptance criteria

### Unblocked NOW (zero Captain gating required):

1. ✓ UI-SPEC.md v0.1 substrate filed (v0.1 anchor 2026-05-11 ~03:38 IST)
2. ✓ UI-SPEC.md v0.2 Q-CUST dispositions filed (this document)
3. **PROCEED NOW** — Q-CUST-2: Implement `rp_returning` cookie-based returning-customer detection in `racecontrol/web-v2/src/app/page.tsx` (or middleware)
4. **PROCEED NOW** — Q-CUST-6: Confirm zero A/B infra in page.tsx implementation
5. PENDING — james AMPLIFIER on §3 structure + §6 accessibility + §9 Q-CUST v0.2 dispositions (bilateral concurrent-session cadence; 24h L1 silent-AGREE window)

### Gates on Captain G33 disposition:

6. **PENDING CAPTAIN** — Q-CUST-1: Hero photography source authorization
7. **PENDING CAPTAIN** — Q-CUST-3: Pricing display copy and GST framing confirmation
8. **PENDING CAPTAIN** — Q-CUST-4: WhatsApp opt-in submission target + legal-AMPLIFIER initiation
9. **PENDING CAPTAIN** — Q-CUST-5: Language strategy confirmation
10. **PENDING CAPTAIN** — Q-CUST-7: DPDP consent banner placement + legal-AMPLIFIER for consent wording

### Post-ratify (after all 5 Captain dispositions received):

11. POST-RATIFY — Actual page authoring in `racecontrol/web-v2/src/app/page.tsx` replacing Phase 0.1 scaffold
12. POST-AUTHORING — Build + standalone output + pm2 restart racingpoint-web-v2 (Q3 production-deploy class — requires explicit Captain per-action auth)
13. POST-DEPLOY — UI-REVIEW.md via `gsd-ui-auditor` agent gate (6-pillar visual audit) per racecontrol/CLAUDE.md Subagent Gates
14. POST-UI-REVIEW — N=1 verify-by: visitors-to-booking ratio baseline established at +30d post-deploy

---

## §11 — Bilateral coordination ship plan

**Step 1 — NOTIFY-AMPLIFIER ship (immediate, autonomous):**
Outbound bilateral msg to james with bracket-prefix `[V2-CUSTOMER-ENTRY UI-SPEC v0.2 · 7 Q-CUST dispositions · 2 AUTONOMOUS-LOCKED · 5 CAPTAIN-STAKE-FLAGGED]`. Message includes: v0.2 file path · Q-CUST-2 and Q-CUST-6 autonomous-locked rationale · summary of 5 Captain-stake flags for james AMPLIFIER vote. Cross-link to V2-MASTER-STATE §S-200 ledger row.

**Step 2 — bono outbound msg to Captain (G33 batch):**
Queue 5 Captain-stake dispositions as a single G33 batch ask using auth-by-reference Format B (sibling-of Q-F05-7 precedent). Presented as a table: Q-CUST ID · topic · proposed path · what Captain decides. Format is proposed-default + "confirm or redirect" — NOT options A/B/C/D. All 5 items pass Q1 (V2-aligned) and Q2 (info-complete); they are Captain-stake class, not Q3-boundary stops.

**Step 3 — james substantive AMPLIFIER disposition:**
24h L1 silent-AGREE window opens on james AMPLIFIER notification. james returns substantive or silent-AGREE triggers auto-proceed on AMPLIFIER-only items. Captain-stake items remain gated on Step 2 regardless of AMPLIFIER status.

**Step 4 — Post-Captain disposition execution:**
On Captain G33 batch response: update Q-CUST-1/3/4/5/7 status in this document to CAPTAIN-RATIFIED or CAPTAIN-REDIRECTED (with new direction where redirected). Update §9 entries where redirected. Proceed to page.tsx authoring. V2-MASTER-STATE §S-N FILE-event ledger row at this step.

---

## §12 — NOT TESTED at v0.2 anchor

- Real customer cohort feedback (zero current users on V2 entry page; gates on deploy)
- Page-load performance against actual 4G/3G Indian mobile network profiles
- DPDP consent flow legal-review (legal-AMPLIFIER pass gated on Captain Q-CUST-4 + Q-CUST-7 disposition)
- WhatsApp opt-in API endpoint existence and schema (api-gateway endpoint proposed, not yet built)
- Cookie-based returning-customer detection rendering behavior in production (specified in §9 Q-CUST-2; not yet in code)
- Visual-design rendering at all 4 breakpoints (gates on actual page.tsx implementation)
- Hero section placeholder rendering and brand-assets/photos/ directory contents (not checked at v0.2 spec anchor)
- PACT-020 customer identity primitive availability (Q-CUST-2 v1 WhatsApp fingerprint path gated)
- Wave 2 Dynamic Pricing API availability (Q-CUST-3 v1 live-pricing path gated)
- GrowthBook or feature-flag A/B infra availability and ops overhead (Q-CUST-6 v1 path gated on 30d baseline)
- Revocation page implementation status (Q-CUST-4 DPDP requirement — launch dependency, not v1)
- Component reuse from `racingpoint.cloud` apex site (`racingpoint-website/frontend/`) — cross-pm2-service asset bridging has dependency-management implications not in scope here

---

## §13 — Composes-with

- **UI-SPEC v0.1 SUPERSEDED (partial)** — `/root/racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.1.md`. v0.2 supersedes §9-§13 of v0.1. §1-§8 of v0.1 remain canonical and are referenced by this document, not duplicated. v0.1 retained as historical substrate.
- **§S-158 V2 Audit-Log Doctrine** — Q-CUST-4 WhatsApp opt-in and Q-CUST-7 DPDP revocation both write `customer_consent_change` action_type records per §S-158.
- **Wave 2 Dynamic Pricing** — Q-CUST-3 live-pricing v1 path gates on Wave 2 Dynamic Pricing PACT family. Static v0 values are a deliberate temporary state.
- **PACT-020 Customer Identity Primitive** — Q-CUST-2 v1 WhatsApp phone-fingerprint path gates on PACT-020. v0 cookie path has zero PACT-020 dependency.
- **V2 Wallet Framing C LOCKED 2026-05-03** — §3.4 Wallet-Framing-C explainer and GST-inclusive pricing framing in Q-CUST-3 both implement Wallet Framing C constraints (18% GST at top-up; cafe always separate).
- **`comms-link/v2-skeleton/10-ui-design-system.md`** — V2 UI design system canonical substrate (ratified 2026-05-08). All brand tokens in §4 derive from this.
- **`packages/shared-tokens/tokens.css`** + **`kiosk/src/app/globals.css`** — canonical sources for `rp-*` Tailwind tokens and font declarations (Orbitron / Montserrat).
- **racecontrol/CLAUDE.md "Subagent Gates"** — "Any frontend phase — UI-SPEC.md — Before planning"; "Any frontend — UI-REVIEW.md — After execution, before ship." v0.2 satisfies the UI-SPEC gate for the Q-CUST disposition layer; UI-REVIEW gate fires post-implementation.
- **§S-146 enforcement plan** — no V1-dependent V2 sections touched in this spec. v2.racingpoint.cloud customer entry is a net-new V2 surface; no V1 ancestor code sharing.

---

## v0.2 summary — what this document unblocks

**(a) Q-CUST-2 (returning-customer detection) + Q-CUST-6 (A/B testing infra) are AUTONOMOUS-LOCKED — implementation begins NOW, zero Captain gating.** Q-CUST-2 locks to `rp_returning` httpOnly cookie v0 + PACT-020-gated WhatsApp fingerprint v1. Q-CUST-6 locks to zero infra v0 + 30d-baseline-gated GrowthBook/feature-flag v1. These items unblock direct work in `racecontrol/web-v2/src/app/page.tsx` without any further disposition.

**(b) v0.1 substrate stays referenced as v0.2's foundation.** §1-§8 of v0.1 are unchanged and authoritative. v0.2 supersedes only §9-§13. The v0.1 file is retained as historical substrate at its original path. Future versions (v0.3 post-Captain, v1.0 post-ratify) build on the same §1-§8 foundation.

— bono / 2026-05-11 ~10:18 IST · UI-SPEC v0.2 DRAFT-PENDING-CAPTAIN-DISPOSITION · 2 AUTONOMOUS-LOCKED (Q-CUST-2 · Q-CUST-6) · 5 CAPTAIN-STAKE-FLAGGED (Q-CUST-1 · Q-CUST-3 · Q-CUST-4 · Q-CUST-5 · Q-CUST-7) · authored autonomous — spec-authoring class, no canonical-boundary surface touched · Apply-Recommendations-Autonomously rule · supersedes UI-SPEC-v0.1.md §9-§13 · Mode 4 subagent dispatch with G9 #2 self-catch (code-architect lacks Write tool → bono inline recovery)
