# V2 Customer Entry Page — UI-SPEC v0.1

**Status:** DRAFT-PENDING-AMPLIFIER (sibling-of HALO-MI-DESIGN-SKELETON-v0.1.md PRE-AMPLIFIER pattern · bravo-slice item 5 precedent)
**Authored:** 2026-05-11 ~03:38 IST (bono autonomous · Apply-Recommendations rule · Captain pointed question "Are you proceeding autonomously to complete racing point ecosystem v2?" 03:36 IST = standing-rule re-affirmation)
**Target deploy surface:** `racingpoint-web-v2` Next.js app at `/root/racecontrol/web-v2` (pm2 id=18, port 3500, basePath `/v2/`)
**Public URL after deploy:** `https://v2.racingpoint.cloud/v2/` (apex `/` redirects to `/v2/` per nginx vhost edit 2026-05-11 ~03:33 IST)
**Current public state:** Phase 0.1 substrate scaffold (~5.6KB placeholder per PR #57)

**Class:** Frontend phase UI-SPEC.md design contract per racecontrol/CLAUDE.md "Subagent Gates" rule — "Any frontend phase — UI-SPEC.md — Before planning"
**LEAD:** bono · **AMPLIFIER:** james (pending real-time concurrent-session cadence)

---

## §1 — Goal + customer-impact framing

**Operational goal:** Replace Phase 0.1 substrate scaffold at `v2.racingpoint.cloud/v2/` with a customer-ready V2 entry page that converts inbound visitors into the appropriate next-step (book sim time · sign up · WhatsApp contact · cafe info).

**Customer-impact framing:** V2 = single-roof game launch + billing + cafe + WhatsApp marketing (per V2 Core Product Definition 2026-05-02). v2.racingpoint.cloud is the V2 customer-facing domain. Until this page exists, V2 launch is structurally blocked at the customer-entry layer regardless of backend readiness. Composes with: `racingpoint.cloud` apex (RacingPoint brand site, 22KB, already operational this session) · `app.racingpoint.cloud` PWA (V1 customer entry, operational pre-this-session).

**Out-of-scope:** Booking flow itself (PWA handles) · payment processing (PWA / Razorpay) · wallet UI (PWA) · staff dashboards (admin.racingpoint.cloud) · in-venue kiosk UX (kiosk.racingpoint.cloud).

---

## §2 — User segments + intent

| Segment | Inbound signal | Intent | Primary CTA |
|---|---|---|---|
| First-time visitor (organic / Google) | No prior auth, no cookie | Explore offering · understand pricing | "Book your first sim time" → signup/PWA |
| Returning customer (direct nav) | Has session cookie OR phone-recognized via WhatsApp | Re-engage · book again | "Continue to your dashboard" → app.racingpoint.cloud |
| WhatsApp campaign click-through | URL has `?utm_source=whatsapp&campaign=X` | Promotion-driven · conversion intent | Campaign-specific CTA (book / discount / event signup) |
| Corporate / group inquiry | No specific signal | Group bookings · venue rental | "Talk to us" → WhatsApp / phone |
| Cafe-only walk-in inquiry | Cafe-curiosity intent | Hours · menu · ambience | "Visit the cafe" → location + hours |

---

## §3 — Page structure (above-the-fold first, then scroll)

| Section | Content | Notes |
|---|---|---|
| **§3.1 Hero** | Full-bleed photo of sim pod in action · headline ("Real cars. Real circuits. Real you.") · primary CTA button → "Book your first sim time" | Photo source: brand-assets/photos/sim-pod-action-hero.jpg (TBD; capture from venue when reopened) |
| **§3.2 Trust-band** | "8 racing simulators · cafe · venue in Hyderabad" + venue photo strip | Social proof; minimal text |
| **§3.3 Experiences strip** | 3-card grid: Solo Sim Session · Multi-player Race · Group Event/Corporate | Each card has 1-line description + price-snapshot + CTA |
| **§3.4 Wallet-Framing-C explainer** | "How credits work" — Single-Purpose Voucher (per V2 Wallet Framing C LOCKED 2026-05-03) · GST inclusive · sim+PS5 redeemable · cafe always separate | Diagram + 3-bullet explainer; legally-compliant per Wallet Framing C |
| **§3.5 Cafe section** | Photo + "Coffee · light bites · cafe-as-amphitheater" framing | Always-separate from sim wallet per Wallet Framing C |
| **§3.6 WhatsApp opt-in** | "Get session reminders + offers on WhatsApp" + phone input | Required (DPDP-compliant consent) to enroll in marketing per V2 customer workflows |
| **§3.7 Location + hours** | Map · address · hours · contact | Hyderabad venue specifics |
| **§3.8 Footer** | Legal links · social · alt-contact | DPDP privacy policy link required |

---

## §4 — Visual design

**Brand identity canonical sources** (per racecontrol/CLAUDE.md Brand Identity section · packages/shared-tokens/tokens.css · kiosk/src/app/globals.css):

| Token | Value | Use |
|---|---|---|
| Racing Red | `#E10600` | Primary CTAs · brand accents |
| Asphalt Black | `#1A1A1A` | Background · primary text-bg contrast |
| Gunmetal Grey | `#5A5A5A` | Secondary text · borders |
| Card | `#222222` | Card surfaces |
| Border | `#333333` | Dividers · subtle borders |
| Red-hover | `#FF1A1A` | Hover state on CTAs |
| Font display | Orbitron (500/700/900) | Hero headlines · CTAs |
| Font body | Montserrat (400/500/600/700) | Body text · captions |

**Tailwind utility prefix:** `rp-*` (per kiosk canonical: `bg-rp-red`, `text-rp-grey`, `border-rp-border`).

**Visual personality:** Premium-but-not-aloof. Photography-led. Dark base · red accent · generous whitespace · sim-action-photo-driven hierarchy. NOT cluttered banner-grid. Inspiration anchor: brand consistency with `racingpoint.cloud` apex (already-deployed 22KB Next.js, port 3600 origin — design language continuity).

---

## §5 — Responsive breakpoints

| Breakpoint | Min-width | Layout shift |
|---|---|---|
| Mobile | 320px | Single column · hamburger nav · stacked hero |
| Tablet | 768px | 2-column experiences strip · expanded hero |
| Desktop | 1280px | 3-card experiences · full-bleed hero · sidebar-able layouts |
| Wide | 1920px+ | Centered max-w-7xl · no further expansion |

**Mobile-first authoring discipline.** Customer entry is heavily mobile (WhatsApp click-through · phone-based pre-arrival research).

---

## §6 — Accessibility (WCAG AA target)

- Semantic HTML: `<header>`, `<nav>`, `<main>`, `<section>`, `<footer>` with proper landmark roles
- Alt text for ALL images including sim photos
- Color contrast: foreground/background pairings meet AA 4.5:1 ratio minimum (verify: Racing Red on Asphalt Black is high-contrast; gunmetal grey on dark needs verification)
- Keyboard navigation: Tab order respects visual order · skip-to-content link
- Focus indicators: visible 2px outline on focusable elements
- Form labels: WhatsApp opt-in input has proper `<label for>` + ARIA attributes
- DPDP compliance: WhatsApp consent banner explicit + revocable

---

## §7 — Performance budget

| Metric | Target | Notes |
|---|---|---|
| Initial HTML transfer | <30KB gzipped | Currently scaffold at 5.6KB; budget headroom for hero + structure |
| First Contentful Paint | <1.5s on 4G | Photo lazy-load · critical CSS inlined |
| Largest Contentful Paint | <2.5s on 4G | Hero image preloaded |
| Cumulative Layout Shift | <0.1 | All image dimensions explicit |
| Time-to-Interactive | <3.0s on 4G | Minimal JS · no blocking 3rd-party |
| Total page weight | <500KB | Photos optimized via Next.js `<Image>` |

**Build flag:** Next.js standalone output (per existing `racingpoint-web-v2` standalone build pattern; pm2 runs `.next/standalone/server.js`). `outputFileTracingRoot` set per racecontrol/CLAUDE.md "Frontend: standalone deploy" rule.

---

## §8 — Composes-with (V2 doctrine alignment)

- **`comms-link/v2-skeleton/05-definition-of-done.md`** — DoD line 17 keep/mold/discard filter; explicit carry-forwards (currency unit · top-up bonus-credit ladder · kiosk-staff launch first iteration · all V1 organs)
- **`comms-link/v2-skeleton/10-ui-design-system.md`** — V2 UI design system canonical substrate (ratified 2026-05-08)
- **`project_v2_customer_workflows_consolidated_20260503.md`** (bono memory) — 5 base + 6 missed scenarios; PWA/POS/portal/pods/Kiosk surfaces; CR-1..CR-8; 30-item V2.0 list
- **V2 Wallet Framing C LOCKED 2026-05-03** — Single-Purpose Voucher; 18% GST at top-up; no customer expiry; sim+PS5 only redeemable; cafe always separate
- **Brand Identity canonical sources** (racecontrol/CLAUDE.md · packages/shared-tokens/tokens.css · kiosk/src/app/globals.css)
- **V2 Core Product Definition 2026-05-02** — single-roof game launch + billing + cafe + WhatsApp marketing
- **§S-158 V2 Audit-Log Doctrine** — page interactions logged per `customer_consent_change` + `customer_profile_edit` action_types where applicable

---

## §9 — Open questions for Captain G33 batch

| ID | Question | Default proposal |
|---|---|---|
| Q-CUST-1 | Hero photography source: capture-from-venue / stock licensed / commission / brand-assets/photos/ existing | brand-assets/photos/ existing if available; capture-from-venue at next reopen otherwise |
| Q-CUST-2 | Returning-customer detection: cookie-based / WhatsApp-phone-fingerprint / none (treat all as first-time) | cookie-based v0; phone-fingerprint v2 (gates on PACT-020 customer identity primitive) |
| Q-CUST-3 | Pricing-snapshot on landing: live (fetch /api/v1/pricing) / static (hardcoded) / link-to-pricing-page | static v0 (hardcoded 30min/₹700 + 60min/₹900 per CLAUDE.md Billing); live in v2 (gates on Phase 2 Dynamic Pricing) |
| Q-CUST-4 | WhatsApp opt-in submission target: existing whatsapp-bot DB path / new endpoint via api-gateway / direct WhatsApp Business API | api-gateway → whatsapp-bot DB (sibling-of admin path; DPDP-compliant audit-log per §S-158) |
| Q-CUST-5 | Multilingual: English-only / English+Hindi / English+Hindi+Telugu (Hyderabad-localized) | English-only v0; English+Hindi v1; defer Telugu to N=2 customer-demand-signal |
| Q-CUST-6 | A/B testing infra: none / GrowthBook / internal feature-flag | none v0 (premature); v1 if conversion data warrants |
| Q-CUST-7 | DPDP consent banner: at-arrival modal / inline-with-WhatsApp-opt-in / footer-link-only | inline-with-WhatsApp-opt-in (smallest interruption; legally-sufficient for marketing scope) |

---

## §10 — Acceptance criteria

For RATIFY + implementation:
1. ✓ UI-SPEC.md v0.1 substrate filed (this document)
2. PENDING — james AMPLIFIER on §3 structure + §6 accessibility + §9 Q-CUST disposition
3. PENDING — Captain G33 disposition on Q-CUST-1..7 (auth-by-reference Format B candidate sibling-of Q-F05-7 precedent)
4. POST-RATIFY — Actual page authoring in `racecontrol/web-v2/src/app/page.tsx` (or app-router root) replacing Phase 0.1 scaffold
5. POST-AUTHORING — Build + standalone output + pm2 restart racingpoint-web-v2 — Q3 production-deploy class requires explicit Captain per-action auth
6. POST-DEPLOY — UI-REVIEW.md via `gsd-ui-auditor` agent gate (6-pillar visual audit) per racecontrol/CLAUDE.md Subagent Gates
7. POST-UI-REVIEW — N=1 verify-by check on customer-conversion metric (post-launch)

---

## §11 — Verify-by

- **Verify-by-1:** at NOTIFY-AMPLIFIER ship — james AMPLIFIER concurrent-session
- **Verify-by-2:** at FILE-event — post-Captain Q-CUST disposition + auth-by-reference Format B
- **Verify-by-3:** at first deploy — UI-REVIEW.md 6-pillar audit pass
- **Verify-by-4:** at +30d post-deploy — customer conversion metric: visitors-to-booking ratio baseline established
- **Stale-by:** 2026-08-11 (90d) if no implementation lands — re-evaluate; V2 customer-frontpage may need design refresh

---

## §12 — NOT TESTED at this v0.1 anchor

- Real customer cohort feedback (zero current users on V2 entry page; gates on deploy)
- Page-load performance against actual 4G/3G profiles (Indian mobile network tier)
- DPDP consent flow legal-review (need legal AMPLIFIER pass; out-of-scope v0.1 spec)
- WhatsApp opt-in conversion funnel (gates on Q-CUST-4 disposition)
- Visual-design rendering at all 4 breakpoints (gates on actual implementation)
- Photography licensing if Q-CUST-1 resolves to stock-licensed (legal verification needed)
- Component reuse from `racingpoint.cloud` apex site (`racingpoint-website/frontend/`) — bridging assets across pm2 services has dependency-management implications not in scope here

---

## §13 — Outbound coordination

**NOTIFY-AMPLIFIER ship:** post-substrate-write (this turn), one outbound msg to james with bracket-prefix `[V2-CUSTOMER-ENTRY UI-SPEC v0.1 PRE-AMPLIFIER · §3 structure + §6 a11y + §9 Q-CUST-1..7]` + cross-link to msg=NEXT V2-MASTER-STATE §S-N ledger row.

**FILE-event sequence (post-AMPLIFIER + Captain Q-CUST disposition):**
1. Update Status DRAFT-PENDING-AMPLIFIER → RATIFIED-PENDING-AUTHORING
2. Author `racecontrol/web-v2/src/app/page.tsx` (or app-router root)
3. Build + standalone (per `outputFileTracingRoot` rule)
4. Captain auth per-action for pm2 restart on bono VPS (Q3 production-deploy class)
5. UI-REVIEW.md via `gsd-ui-auditor` agent gate
6. comms-link/V2-MASTER-STATE.md §S-N FILE-event ledger row

---

— bono / 2026-05-11 ~03:38 IST · UI-SPEC v0.1 DRAFT-PENDING-AMPLIFIER · sibling-of HALO-MI-DESIGN-SKELETON-v0.1.md PRE-AMPLIFIER pattern · Apply-Recommendations-Autonomously rule application · §S-N substrate-class authoring autonomous-eligible (sibling bravo-slice item 5 precedent) · authored under Captain class-level autonomous auth re-affirmation "proceed to complete racing point ecosystem v2 autonomously" + pointed reaffirmation "Are you proceeding autonomously to complete racing point ecosystem v2?" · NEXT autonomous step: bilateral msg to james + §S-N ledger entry in V2-MASTER-STATE.md
