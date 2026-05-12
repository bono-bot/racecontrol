# V2 Customer Entry — UI Review v0.1

**Audited:** 2026-05-12 ~13:13 IST
**Baseline:** UI-SPEC v0.2 (CAPTAIN-RATIFIED §S-204 2026-05-12 ~11:05 IST) + v0.1 §1-§8 inherited foundation + canonical brand sources (`packages/shared-tokens/tokens.css`, `kiosk/src/app/globals.css`)
**Commit audited:** `110cad4d` (2026-05-12 11:43 IST) — Row 1.4 V2-PROGRESS-MAP IN-FLIGHT
**Auditor:** `gsd-ui-auditor` agent (via james)
**Screenshots:** captured against live production `https://v2.racingpoint.cloud/v2/` (HTTP 200 followed from 308) — desktop 1440x900 / tablet 768x1024 / mobile 375x812 / privacy desktop. Stored at `.planning/ui-reviews/v2-customer-entry-20260512-131307/` (binary-gitignored).
**Customer-day class:** V2-LIVE-BLOCKING (first customer touch on V2 substrate; 14:05 customer-day beat)

---

## Verdict

**PASS-WITH-FIXES** — no BLOCK-class issues. Row 1.4 may proceed to closure pending the **3 FLAG-class fixes** below; FLAGs are non-blocking but tracked as launch-gate Verify-by.

**Reasoning:** Implementation honors all 7 Q-CUST dispositions (verified in §Detailed Findings). Cookie-aware Hero/Header is functional and SSR-clean. WhatsApp opt-in form covers all 4 states with inline DPDP consent and submit-gated-until-checked. Privacy stub is DPDP-baseline-sufficient. No customer-facing copy ships with "V2" nomenclature (Captain 2026-05-11 09:28 IST verbatim). No hardcoded prices outside the §S-204-ratified `₹700/30min` + `₹900/60min` static values. No `any`, no `.unwrap()`-class antipatterns. No customer flow blocked.

**Why no BLOCK:** All BLOCK candidates surface as customer-flow breakers (form non-submittable, cookie write failure that 500s the page, missing consent checkbox, broken POST endpoint). None observed. The FLAGs below are brand-fidelity gaps (Orbitron silently absent, hover-red shade off by 2 hex digits, Privacy Policy is unstyled `<main>` with no header/footer) — they degrade brand expression but do not block the customer journey.

---

## Pillar Scores

| Pillar | Score | Key Finding |
|---|---|---|
| 1. Copywriting | 4/4 | All 7 Q-CUST disposition copies match UI-SPEC verbatim; zero generic labels; differentiated 4-state error/success/submit text |
| 2. Visuals | 3/4 | Hero focal point strong; trust band + sectioning clear; but Privacy Policy is an orphan (no header/footer) and cafe placeholder reads as "intentional gradient" not "real photo coming" |
| 3. Color | 3/4 | Accent (Racing Red) usage proportional and clean; `--rp-racing-red-hover: #ff3b2d` diverges from canonical `#FF1A1A`; one inline hex in privacy/page.tsx |
| 4. Typography | 2/4 | **Orbitron entirely absent** despite UI-SPEC §1-§8 display-font requirement; production CSS references undefined `--rp-font-display` var and falls back to Montserrat; `font-weight: 800` used in 4 places (outside declared 400/500/600/700 Montserrat ladder) |
| 5. Spacing | 4/4 | Consistent rem-based scale; no arbitrary px values in `page.module.css`; mobile-first responsive flow scales cleanly through 768/1024/1280 breakpoints |
| 6. Experience Design | 4/4 | 4 explicit form states (idle/submitting/success/error); cookie-aware Hero/Header swap server-side; skip-link present; ARIA labels on every section; submit-gated-until-consent |

**Overall: 20/24** (passes 4.0/6 threshold for V2-LIVE-BLOCKING customer touch).

---

## Top 3 Priority Fixes

1. **FLAG-A — Display font Orbitron silently missing** — Headings (hero h1, section h2, h3) render in Montserrat at weight 800 because `--rp-font-display` is referenced in compiled CSS but never declared in `layout.tsx` or `globals.css`. Brand premise of "premium-but-not-aloof" relies on Orbitron's geometric authority for the hero. **Fix:** add a second `next/font/google` Orbitron import in `web-v2/src/app/layout.tsx` exposing `--rp-font-display` (weights 500/700/900), apply via `html className={`${montserrat.variable} ${orbitron.variable}`}`, and add `font-family: var(--rp-font-display)` to `.heroHeading` + `.sectionHeader h2` + `.cafeText h2` selectors. Verify by curling the compiled `e26ce26bea99ce9f.css` after rebuild and grepping for `Orbitron`.

2. **FLAG-B — Privacy Policy page is unstyled-orphan** — `web-v2/src/app/privacy/page.tsx` ships only the inherited globals.css `<main>` (max-width 720px, no header, no footer, no skip-link, no breadcrumb). DPDP baseline disclosure is legally adequate but the page reads as "unfinished side-room" rather than continuous brand experience — degrades trust at exactly the moment customers verify the brand handles their data professionally. **Fix:** wrap `PrivacyPolicyPage` body with the same `<Header />` + `<Footer />` components extracted from `page.tsx`; add `id="main"` and a skip-link; consider lifting Header/Footer into a `(marketing)` route group layout to share across landing + privacy. Non-blocking but visible to every consent-curious customer.

3. **FLAG-C — Racing-red-hover token diverges from canonical** — `globals.css:23` declares `--rp-racing-red-hover: #ff3b2d` while the canonical source `packages/shared-tokens/tokens.css:8` is `--rp-red-hover: #FF1A1A`. UI-SPEC v0.2 §0 also cites `#FF1A1A`. Affects every CTA / nav / card hover state on the page. The page.tsx `globals.css` comment notes "no shared-tokens import per quarantine-discipline §2 of PACT-20260503-001" — quarantine is justified for Turbopack reasons, but the hand-copied value drifted. **Fix:** change `--rp-racing-red-hover: #ff3b2d` → `--rp-racing-red-hover: #FF1A1A` in `web-v2/src/app/globals.css:23` and `--rp-racing-red: #e10600` → `#E10600` (case-canonical) on line 22. One-line fix; survives quarantine boundary.

---

## Detailed Findings

### Pillar 1: Copywriting (4/4)

UI-SPEC v0.2 Q-CUST dispositions vs implementation (all paths absolute):

- **Q-CUST-1 (hero photography fallback):** `page.tsx:122` comment explicitly references CAPTAIN-RATIFIED fallback (b) per §S-204 → `heroBackdrop` div with `radial-gradient` + speed-streaks. No broken image, no blank box. Matches v0.2 §9 verbatim. ✓
- **Q-CUST-2 (returning-customer cookie):** `page.tsx:55-58` reads cookie via `next/headers`; `middleware.ts:7-29` sets `rp_returning=1` with TTL 60×60×24×90 = 7776000s (90 days) · path=/ · sameSite=Lax · httpOnly · secure. All five cookie spec fields match Q-CUST-2 disposition verbatim. CTA swap verified visually in desktop screenshot (renders "Continue to your dashboard" + "Dashboard" nav CTA after first visit set the cookie). ✓
- **Q-CUST-3 (pricing):** `page.tsx:196,204` static `"₹700 / 30 min · GST inclusive"` and `"₹900 / 60 min · GST inclusive"`. `page.tsx:244-248` reinforces "Prices shown above are **final** — 18% GST is already included." No `fetch("/api/v1/pricing")`. No "starting from" qualifier. Wallet Framing C terms ("1 credit = ₹1", "wallet redeems for sim time only", "Cafe is always separate") all present. ✓
- **Q-CUST-4 (WhatsApp opt-in submission):** `WhatsAppOptInForm.tsx:40` POSTs to `/v2/api/v2/marketing/whatsapp-optin` with payload `{ phone, consent_text, consent_ts, source: "v2-landing" }` — exact §S-158 audit-log schema. Stub at `route.ts:39-77` accepts 202 with note "v0 stub — api-gateway proxy pending". ✓
- **Q-CUST-5 (English-only v0):** No i18n routing, no locale switch UI, no `messages/*.json`. All copy is English. ✓
- **Q-CUST-6 (no A/B infra):** No GrowthBook SDK, no feature-flag client, no experiment assignment. Single deterministic implementation. ✓
- **Q-CUST-7 (inline DPDP):** `WhatsAppOptInForm.tsx:20-21` consent text matches v0.2 §9 Q-CUST-7 draft verbatim. `WhatsAppOptInForm.tsx:31-32` `canSubmit = consent && phone.trim().length >= 8 && state !== "submitting"` — submit blocked until checkbox ticked. Standalone footer Privacy Policy link at `page.tsx:405`. No at-arrival modal. ✓

Generic-label grep returned **zero hits** on the customer-entry surface (`page.tsx` + `WhatsAppOptInForm.tsx` + `privacy/page.tsx`): no "Submit", no "Click Here", no "OK", no "Cancel". Submit button label is the brand-aware `"Get racing updates"` / `"Sending…"`. Error states differentiate `submission_failed` ("Something went wrong. Please try again or message us on WhatsApp directly.") from network error ("We could not reach our server. Please try again in a moment.") — `WhatsAppOptInForm.tsx:115-119`.

No "V2" / "V2.0" anywhere in customer-visible copy. `page.tsx:15-16` comment explicitly enforces this per Captain 2026-05-11 09:28 IST.

### Pillar 2: Visuals (3/4)

**Strengths (visible in screenshots):**
- Hero focal point: 78vh min-height (`page.module.css:115`) + clamp-scaled headline (`2.25rem → 4.75rem`) + radial-gradient red glow at 75%/30% creates an obvious primary read.
- Trust band immediately below hero (8 sims / Cafe / Hyderabad) gives quick venue-credentials read without scroll.
- Cards differentiate via 1px border + Racing-Red hover lift (translateY -2px); CTAs arrows scan as actionable.
- ARIA scaffolding strong: every `<section>` has `aria-labelledby` or `aria-label`; brand mark has `aria-label="RacingPoint"`; nav has `aria-label="Primary"`; backdrop has `aria-hidden="true"`. Skip-link at `page.tsx:64`.

**Gaps (FLAG-class):**
- **Cafe placeholder reads as deliberate-design, not "real photo coming":** `page.module.css:412-433` `cafePhoto` is a 16:10 dark gradient with caption overlay `"Coffee · Light bites · Cafe-as-amphitheater"`. The Hero placeholder is honestly red-glow-as-texture; the cafe placeholder pretends to be a photo. A customer can't tell from the cafe block that "real cafe photo arriving at next venue reopen" — degrades authenticity. Recommend either (a) a small `data-placeholder` annotation in dev/staging until real photo lands, or (b) embrace the gradient as final and remove the caption.
- **Privacy Policy is layout-orphan:** see FLAG-B in Top 3. Globals.css `main` rule (max-width 720px, 2rem padding) is the only structure. Reads as "documentation page", not "RacingPoint page".
- **Mobile screenshot:** the bottom of the page (footer + DPDP line) reads as competent but unmemorable. Eyebrow uppercase letter-spacing 0.18em on `"RACINGPOINT · HYDERABAD"` is the strongest design moment in the hero — would benefit from echoing once in the footer to bracket the page.

### Pillar 3: Color (3/4)

**Token usage (grep across `web-v2/src/`):**
- Hex-literal occurrences: 38 matches. Of those, **35 are inside CSS-var declarations** (canonical brand tokens) or `#fff` for text-on-red (safe and intentional).
- Three non-canonical hex literals:
  - `globals.css:23` — `#ff3b2d` (red-hover) — should be `#FF1A1A` per canonical → see FLAG-C
  - `globals.css:22` + `:60` — `#e10600` lowercase — non-blocking style drift; canonical capitalization is `#E10600`
  - `privacy/page.tsx:113` — inline `color: "#a0a0a0"` for "← Back to RacingPoint". Should reference `var(--rp-text-muted)`.

**Accent distribution (page.module.css grep `--rp-racing-red`):** 17 occurrences. Distribution by category: hero accent (1), eyebrow + headings highlight (2), nav CTA + hover (4), primary CTAs (2), card hover + price-snapshot border-left (2), credits-list left-rail + numerals (2), location-block headers (1), opt-in form submit + checkbox accent (2), footer link hover (1). All proportional — accent is used to ladder attention through CTAs and section anchors, not decoratively.

**No accent overuse:** Racing-Red appears on the elements a customer should look at (book CTA, prices, consent checkbox, hover affordances). Not on borders, not on body text, not on icons that aren't actionable.

**Background scale:** uses `#0e0e0e` (footer/whatsapp gradient bottom) → `#1a1a1a` (page base) → `#222222` (cards) → `#2a1a14` (cafe gradient highlight). Five-step elevation is correct for the dark theme; no surface ambiguity.

### Pillar 4: Typography (2/4)

**BLOCKING-FLAG class evidence (FLAG-A):**

- `web-v2/src/app/layout.tsx:10-15` — only Montserrat is loaded via `next/font/google`, exposing `--rp-font-body`. **No Orbitron import exists.**
- `web-v2/src/app/globals.css:13-33` — declares `--rp-font-body` via `var(--rp-font-body, system-ui)` but **declares no `--rp-font-display`** anywhere.
- Production CSS chunk `e26ce26bea99ce9f.css` (line-by-line grep): every font-family declaration resolves to `Montserrat`. Second chunk `b3c2c3d61646c64b.css` references `font-family:var(--rp-font-display,var(--rp-font-body,sans-serif))` — referencing an undefined CSS variable that **silently falls through to Montserrat**. Verified by `curl https://v2.racingpoint.cloud/v2/_next/static/chunks/b3c2c3d61646c64b.css | grep -oE "font-family[^;}]+"`.
- UI-SPEC v0.2 §1-§8 (inherited v0.1 + reaffirmed in §0 of v0.2): "Orbitron (display 500/700/900)". `comms-link/v2-skeleton/10-ui-design-system.md` is the canonical V2 substrate.
- racecontrol/CLAUDE.md "Brand Identity": "Fonts: Montserrat (body, 400/500/600/700), Orbitron (display, 500/700/900)".

**Weight ladder drift:**
- `page.module.css` uses weights: 500, 600, 700, **800**. The 800 weight (`page.module.css:61, 169, 249, 387`) is outside both the Montserrat ladder (400/500/600/700) AND the Orbitron ladder (500/700/900). It works visually because next/font auto-loads adjacent weights, but it's a contract drift. Either move hero/numerals to Orbitron 900 (canonical) or normalize to Montserrat 700.

**Size scale:** 9 distinct rem sizes in use (0.75 / 0.8125 / 0.875 / 0.9375 / 1 / 1.0625 / 1.125 / 1.25). Plus 4 clamp() pairs for responsive heading scaling. This is on the high end (UI-SPEC didn't lock a numeric scale — v0.2 §1-§8 inherits from v0.1 without specifying a type-scale ratio), but readable. Pillar 4 takes the hit primarily from the Orbitron absence, not the size count.

**Score 2/4 reasoning:** The Orbitron absence is the single largest contract divergence in the implementation. The page is *legible* and *brand-toned*, but the canonical display voice is missing. With Orbitron loaded, this pillar goes to 4/4.

### Pillar 5: Spacing (4/4)

**Distribution:** 69 padding/margin/gap declarations in `page.module.css`. All use rem units. No `px` arbitrary values. No `[42px]` Tailwind escape-hatches (the page is CSS-modules, not Tailwind-utility — Tailwind prefix `rp-*` isn't applicable here, which is itself consistent with v0.2 §13 noting that web-v2 ships its own minimal CSS per `quarantine-discipline §2`).

**Common spacing values observed (top by frequency):**
- `1.5rem` (24px) — section padding, card gaps, locationBlock interior padding
- `1rem` (16px) — paragraph margins, primary gap
- `0.875rem` (14px) — header padding, smaller surfaces
- `0.75rem` (12px) — CTA row gap, secondary buttons
- `0.5rem` (8px) — tight inline gaps (brand mark, nav links)
- `4rem` / `4.5rem` (64/72px) — vertical section breathing room

**Breakpoint ladder:** mobile-first → 768px (tablet — cards go 1-col → 2-col; cafe two-column splits; location grid 3-col) → 1024px (cards 2-col → 3-col) → 1280px max-width inner containers. Three breakpoints is the right count for a marketing page; behavior verified in three screenshot widths (375 / 768 / 1440).

**Responsive verify:** tablet screenshot shows the Group & Corporate card orphaned on the third row (cards: 2-col grid, 3 items). On 1024px+ they fall to 3-col same-row; mobile they stack vertically. Acceptable — no broken layouts. Could be improved by either making the 3rd row card span 2 columns or using `auto-fit minmax(280px, 1fr)`. Not a FLAG because functional.

### Pillar 6: Experience Design (4/4)

**State coverage (`WhatsAppOptInForm.tsx`):**
- 4 explicit submit states: `"idle" | "submitting" | "success" | "error"` (line 23)
- Idle: form visible with consent unchecked → submit disabled (`canSubmit` predicate line 31-32)
- Submitting: button label swaps to `"Sending…"` (line 111); button stays disabled
- Success: returns alternate JSX (lines 65-75) with confirmation copy + STOP reply instruction
- Error: differentiates `submission_failed` (server-side) vs `network_error` (transport) at lines 115-119

**Accessibility (audit-grep against `page.tsx`):**
- 11 aria-* / role / `aria-hidden` attributes
- `aria-labelledby` on every section header that has a heading
- `aria-label` on brand mark + nav landmark + venue trust-band (no heading)
- `aria-hidden="true"` on decorative backdrops (heroBackdrop, cafePhoto)
- `<a href="#main" className={styles.skipLink}>` at `page.tsx:64`; `main` id at `page.tsx:69`
- Form field uses `<label>` wrappers (semantic, not just `htmlFor`); checkbox + phone input both have visible labels
- Error message has `role="alert"`; success has `role="status"`
- `aria-disabled={!canSubmit}` complements the native `disabled` attribute

**Server-side cookie write (Q-CUST-2):**
- `middleware.ts:10-22` — first-visit sets cookie; second-visit skips (idempotent)
- `page.tsx:55-58` reads via `await cookies()` (Next.js 16 Promise-returning API correctly awaited)
- Header + Hero both receive `returning` prop; CTA labels and URLs swap accordingly (lines 102, 105, 138-156)
- Verified visually: desktop screenshot taken on second visit shows the returning-customer variant; the cookie write/read round-trip works in production

**Destructive-action confirmation:** No destructive actions on this surface (opt-in is additive). The opt-in itself is gated by explicit checkbox + button click — correct for marketing scope DPDP.

**Empty/error states for the page itself:** N/A — this is a static marketing page with one conditional CTA swap. No data-fetched lists that need empty/error states. The one fetched surface (opt-in submission) has explicit error handling.

**No `.unwrap()` / no `any`:** TypeScript on `WhatsAppOptInForm.tsx` types `SubmitState` discriminated union and uses `instanceof Error` narrowing on catch (line 60). API route uses a type-guard `isValidPayload` (line 23-37) rather than casting.

---

## V2 Doctrine Alignment

**Composes-with verify:**
- **Wallet Framing C** — `page.tsx:244-248` + `CreditsExplainer` lines 264-285 implement Single-Purpose Voucher framing verbatim ("1 credit = ₹1", "wallet redeems for sim time only", "Cafe is always separate", "balance does not expire"). 4-step explainer matches `project_v2_wallet_framing_c_locked_20260503` constraints. ✓
- **§S-158 Audit-Log Doctrine** — `WhatsAppOptInForm.tsx:43-48` payload schema matches `customer_consent_change` action_type fields (phone hashed downstream at api-gateway per Q-CUST-4 (b)); stub at `route.ts:60-68` correctly does NOT log raw phone (avoids stub-time PII leak). ✓
- **DPDP compliance v0** — explicit opt-in (unchecked default), clear scope (racing-only), 3 revocation routes (STOP, email, WhatsApp message), 1-business-day removal commitment. `privacy/page.tsx:79-87`. ✓
- **§S-146 V1↔V2 RCA gate** — this is a net-new V2 surface (per UI-SPEC v0.2 §13: "v2.racingpoint.cloud customer entry is a net-new V2 surface; no V1 ancestor code sharing"). §S-146 5-section RCA not required. ✓
- **Captain "V2 nomenclature internal-only" 2026-05-11 09:28 IST** — `page.tsx:15-16` comment enforces; zero customer-visible "V2" strings. ✓
- **Apply-Recommendations-Autonomously (§S-186)** — implementation proceeded under §S-204 Captain G33 ratify without re-asking on autonomous-locked items (Q-CUST-2 + Q-CUST-6). Q3-cleared. ✓

---

## Files Audited

- `C:\Users\bono\racingpoint\racecontrol\web-v2\src\app\page.tsx` (418 lines)
- `C:\Users\bono\racingpoint\racecontrol\web-v2\src\app\page.module.css` (687 lines)
- `C:\Users\bono\racingpoint\racecontrol\web-v2\src\app\globals.css` (70 lines)
- `C:\Users\bono\racingpoint\racecontrol\web-v2\src\app\layout.tsx` (37 lines)
- `C:\Users\bono\racingpoint\racecontrol\web-v2\src\components\WhatsAppOptInForm.tsx` (129 lines)
- `C:\Users\bono\racingpoint\racecontrol\web-v2\src\app\privacy\page.tsx` (118 lines)
- `C:\Users\bono\racingpoint\racecontrol\web-v2\src\middleware.ts` (29 lines)
- `C:\Users\bono\racingpoint\racecontrol\web-v2\src\app\api\v2\marketing\whatsapp-optin\route.ts` (77 lines)
- `C:\Users\bono\racingpoint\racecontrol\.planning\specs\v2\V2-CUSTOMER-ENTRY\UI-SPEC-v0.2.md` (282 lines — design contract)
- `C:\Users\bono\racingpoint\racecontrol\packages\shared-tokens\tokens.css` (37 lines — canonical color tokens)
- Production CSS chunks: `e26ce26bea99ce9f.css` + `b3c2c3d61646c64b.css` (curl-fetched from live)

## Screenshots Captured

Stored at `C:\Users\bono\racingpoint\racecontrol\.planning\ui-reviews\v2-customer-entry-20260512-131307\`:

| File | Viewport | Bytes | Captured against |
|---|---|---|---|
| `desktop.png` | 1440x900 | 604,753 | `https://v2.racingpoint.cloud/v2/` (returning-customer variant — middleware set cookie pre-render) |
| `tablet.png` | 768x1024 | 599,562 | `https://v2.racingpoint.cloud/v2/` |
| `mobile.png` | 375x812 | 399,271 | `https://v2.racingpoint.cloud/v2/` |
| `privacy-desktop.png` | 1440x900 | 129,095 | `https://v2.racingpoint.cloud/v2/privacy` |

`.gitignore` for `.planning/ui-reviews/` ensures these PNGs never reach the commit (per `gsd-ui-auditor` gitignore-gate).

## NOT TESTED

- Cookie-write round-trip on a clean-state visitor (screenshots were taken on second visit — playwright session 1 set the cookie; session 2 captured the returning-variant). First-time-visitor CTA copy ("Book your first sim time" + "Book") was verified by reading the code path but not visually captured. Recommend a follow-up `npx playwright screenshot --no-http-cache` or `--ignore-https-errors --user-data-dir=/tmp/fresh` capture.
- Actual WhatsApp opt-in POST round-trip (stub returns 202; api-gateway proxy is a separate child phase per `route.ts:7-13`). Form submission state transitions tested in code review only, not behaviorally.
- 4G / 3G network performance budget (UI-SPEC §7 inherited from v0.1). Page weight: HTML 27KB + 2 CSS chunks. Reasonable but not measured against UI-SPEC budget.
- Cross-browser rendering (only Chromium tested via playwright). Safari iOS / Firefox Android not captured.
- Hero photography swap-in path (Q-CUST-1 (a) — brand-assets/photos/ check returned "directory does not exist" at audit time; CAPTAIN-RATIFIED fallback (b) is what shipped).
- Legal-AMPLIFIER queued for Q-CUST-4 + Q-CUST-7 wording finalization (in-flight ledger `legal-amplifier-queue-qcust4-qcust7-wording-20260512-1110-IST`). Non-blocking on this audit per §S-204.
- Conversion baseline +30d (Verify-by-1 from UI-SPEC v0.2 §10 row 14) — gates on time, not on audit.
- Hindi v1 (Q-CUST-5 (b)) — explicitly v1, not v0.
- GrowthBook / feature-flag infra (Q-CUST-6 v1) — explicitly v1, gates on 30d baseline.

---

## Composes-With

- UI-SPEC v0.2 — `.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md` (design contract)
- UI-SPEC v0.1 §1-§8 — `.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.1.md` (inherited foundation)
- V2-MASTER-STATE §S-204 close-anchor — `comms-link/V2-MASTER-STATE.md` (Captain G33 ratify 2026-05-12 ~11:05 IST verbatim "all bono recommendations")
- V2-PROGRESS-MAP Row 1.4 — `.planning/specs/v2/V2-PROGRESS-MAP.md` (IN-FLIGHT; gates on this UI-REVIEW per racecontrol/CLAUDE.md Subagent Gates)
- racecontrol/CLAUDE.md "Subagent Gates" — "No frontend phase ships without UI-SPEC.md AND UI-REVIEW.md"
- racecontrol/CLAUDE.md "Brand Identity" — `(canonical: packages/shared-tokens/tokens.css for colors; kiosk/src/app/globals.css for fonts; comms-link/v2-skeleton/10-ui-design-system.md for full V2 substrate)`
- §S-186 pre-§S-146 small-fix fast-lane — FLAG-A + FLAG-B + FLAG-C all eligible (≤200 LOC each, single-boundary, no schema, no protocol, bug-fix class)
- V2-LBAC v0.1 — these FLAGs follow the OPEN → DESCEND → FIX → CLOSE → SWEEP cycle; CLOSE evidence will be a re-run of this audit's relevant grep + a fresh screenshot

---

— james / 2026-05-12 ~13:13 IST · UI-REVIEW v0.1 PASS-WITH-FIXES · row 1.4 green-lighted pending FLAG-A/B/C closure · §S-204 close-anchor downstream gate
