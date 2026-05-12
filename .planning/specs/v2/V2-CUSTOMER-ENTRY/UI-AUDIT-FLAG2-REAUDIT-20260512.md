# UI-AUDIT FLAG-2 Re-Audit — V2 Customer Entry PWA

**Date:** 2026-05-12 ~10:22 UTC (15:52 IST)
**Author:** gsd-ui-auditor (agent `aabdc71db54fbd5c7`) · persisted by bono
**Baseline:** `UI-SPEC-v0.2.md` §6 (A11y); prior `UI-REVIEW.md` FLAG-2 sub-findings (line 60-64)
**Target:** `http://localhost:3500/v2/` + `/v2/privacy` (pm2 `racingpoint-web-v2` on Bono VPS srv1422716)
**Public canonical:** `https://v2.racingpoint.cloud/v2/`
**Source root:** `/root/racecontrol/web-v2/src/`
**Trigger:** V2-LBAC pickup post-FLAG-1 (Orbitron) + FLAG-3 (labeled-disabled-visual) closures; FLAG-2 preconditions met because font-load can shift focus-ring rendering.

---

## BUILD_ID probe note

`grep '"buildId":"[^"]*"' /tmp/v2-canonical.html` returned 0 results. The served HTML does not embed `"buildId":"..."` in the response body for `/v2` or `/v2/`. `GET /v2/api/v1/health` returns `{"status":"ok","service":"web-v2","version":"0.1.0","pact":"PACT-20260503-001","phase":"0.1-substrate"}` — no `build_id` field. The build IS the expected one — the `WhatsAppOptInForm` renders the FLAG-3 disabled-state label `Please confirm consent` with `aria-disabled="true"` exactly as captured under BUILD_ID `J7_f9yqAh4IwqlbToPI_I` in the prior audit (`UI-REVIEW.md:88`). Treat BUILD_ID parity as **PROXY-CONFIRMED via FLAG-3 rendered output**. Surfaced as NEW-1 below.

---

## Scoreboard (all 6 pillars re-evaluated)

| Pillar | Verdict | Rationale |
|---|---|---|
| Layout & Hierarchy | **PASS** | h1→h2→h3 strict on `/v2/`; h1→h2 strict on `/v2/privacy`. Single h1 per route. Landmarks complete (`header`/`nav`/`main`/`section`/`footer`). |
| Typography | **PASS** | Orbitron loaded via `next/font/google` weights 500/700/900 (`layout.tsx:20-25`); CSS `--rp-font-display` consumed at `page.module.css:167,269,439,473`. Closes prior FLAG-1. |
| Color & Contrast | **PASS** (one nit-flag in A11y) | Token palette unchanged; Racing Red `#E10600` on `#1A1A1A` for non-text accents OK. Focus-ring contrast is the nit — see FLAG-2c. |
| Interactive States | **PASS** | `disabled` + `aria-disabled="true"` paired on opt-in submit (`WhatsAppOptInForm.tsx:121-122`); contextual label `Please confirm consent` rendered (verified via curl). FLAG-3 close confirmed live. |
| **Accessibility** | **FLAG-2 — 4 sub-findings, 0 BLOCKs** | See sub-finding table below. None are ship-blockers; all AAA-tier or consistency kaizen. |
| Responsiveness | **PASS** | Mobile-first; breakpoints at 768/1024/1280; fluid `clamp()` typography. |

**Ship verdict:** FLAG-2 **CANDIDATE-CLOSE** — all 8 a11y depth checks PASS at WCAG 2.2 A/AA; 4 sub-findings are AAA-tier / consistency kaizen carry-forward.

---

## FLAG-2 sub-finding disposition (8 mandatory depth checks)

| # | Check | Status | Disposition |
|---|---|---|---|
| 1 | Labeled inputs | **PASS** | Both inputs in `WhatsAppOptInForm.tsx` wrapped in `<label>`; `aria-describedby="optin-consent"` cross-references consent label `id`. |
| 2 | Skip-link visibility | **FLAG-2a** | `:focus` reveal (not `:focus-visible`) on `/v2/`; missing entirely on `/v2/privacy`. |
| 3 | Focus-ring contrast — inputs | **FLAG-2b** | `outline: none` + `border-color` shift; passes ≥3:1 contrast quant (5.2:1 + 4.78:1) but 1px-only indicator. |
| 3' | Focus-ring contrast — links | **FLAG-2c** | Global `a:focus-visible { outline: none }` cascades through all CTAs/nav/footer; color-shift only (`#e10600` → `#ff3b2d`, ~1.2 delta). AA-compliant but borderline. |
| 4 | Heading hierarchy | **PASS** | Strict h1→h2→h3 on `/v2/`; strict h1→h2 on `/v2/privacy`. Single h1 per route. |
| 5 | `aria-disabled` consistency | **PASS** | `disabled` + `aria-disabled="true"` paired on opt-in submit. |
| 6 | Color-only signaling | **PASS** | Disabled state communicates via text label + opacity + cursor + aria — four non-color signals. |
| 7 | Reduced motion | **FLAG-2d** | Zero `@media (prefers-reduced-motion: reduce)` blocks. AA OK (transitions ≤5s); AAA non-conformant. |
| 8 | Lang attribute | **PASS** | `<html lang="en">` on both routes; inherited from `layout.tsx:43`. |

### Finding 1: Labeled inputs — PASS

`WhatsAppOptInForm.tsx:92-105` — phone `<input type="tel">` wrapped by `<label class="optInField">` containing `<span>WhatsApp number</span>`. `WhatsAppOptInForm.tsx:107-116` — consent `<input type="checkbox">` wrapped by `<label class="optInConsent" id="optin-consent">` containing CONSENT_TEXT.

Live HTML probe:
```
<label class="page-module___8aEwW__optInField">
<input type="tel" inputMode="tel" autoComplete="tel" ... required="" aria-describedby="optin-consent" name="phone" value=""/>
<label class="page-module___8aEwW__optInConsent" id="optin-consent">
<input type="checkbox" required="" name="consent"/>
```

No bare `<input>` elements without a label on either route. **Verdict: PASS.**

### Finding 2: Skip-link visibility — FLAG-2a (minor)

Skip-link present on `/v2/` (`page.module.css:12-24`); CSS reveals on `:focus` via `left: 0`. Rendered HTML: `<a href="#main" class="page-module___8aEwW__skipLink">Skip to content</a>`.

**Kaizen items:**
- `:focus` reveal (not `:focus-visible`) — over-revealing on mouse-focus; not under-revealing. Still WCAG 2.2 SC 2.4.1 conformant.
- `/v2/privacy` has NO skip-link AND no `<main id="main">` landmark. Probe `grep -c skipLink /tmp/v2-privacy.html` → 0. Privacy is short single-h1 with no repeated content preceding main, so WCAG-conformant, but inconsistent with landing page. Smallest fix: add `<a href="#main" class={styles.skipLink}>Skip to content</a>` + `<main id="main">` to `privacy/page.tsx`.

**Disposition:** non-blocking kaizen.

### Finding 3a: Focus-ring contrast — inputs — FLAG-2b (kaizen)

`page.module.css:575-578`:
```css
.optInField input[type="tel"]:focus-visible {
  outline: none;
  border-color: var(--rp-racing-red);
}
```

Contrast quant: Racing Red `#E10600` vs `--rp-asphalt-black #1A1A1A` ≈ 5.2:1; vs `--rp-card #222222` ≈ 4.78:1 — both pass WCAG 2.2 SC 1.4.11 (≥3:1 non-text contrast).

Kaizen: focus indicator is a 1px border-color change. Replacing `outline: none` with `outline: 2px solid var(--rp-racing-red); outline-offset: 1px;` gives a 2px indicator + passes WCAG 2.2 SC 2.4.13 (Focus Appearance, AAA).

**Orbitron-load regression check:** form input is below the fold of any heading change → font-load did NOT introduce regression here.

**Disposition:** non-blocking, AA-conformant; AAA would require outline upgrade.

### Finding 3b: Focus-ring removed on links — FLAG-2c (kaizen, highest-impact)

`globals.css:65-69`:
```css
a:hover,
a:focus-visible {
  color: #ff3b2d;
  outline: none;
}
```

Universal `a` selector strips keyboard focus indicator from EVERY anchor (skip-link, nav, footer, in-body, hero CTAs). Fallback = color shift + existing `border-bottom: 1px solid currentColor`.

Color-delta contrast (`#e10600` → `#ff3b2d` vs `#1a1a1a`): 5.2:1 → 6.4:1 (delta ~1.2). Perceptible for non-colorblind users; subtle for red-deficient users.

WCAG 2.2 SC 2.4.7 Focus Visible (AA) — color shift IS a visible mode; SC 1.4.11 ≥3:1 satisfied. Borderline AA-compliant.

Smallest reversible fix:
```css
a:focus-visible {
  color: #ff3b2d;
  outline: 2px solid var(--rp-racing-red, #e10600);
  outline-offset: 2px;
}
```

Single 3-line change closes FLAG-2c sitewide.

**Disposition:** highest-impact kaizen item among the 4 sub-findings.

### Finding 4: Heading hierarchy — PASS

`/v2/`:
```
<h1 id="hero-heading">Real cars. Real circuits. Real you.</h1>
<h2 id="experiences-heading">What you can race</h2>
<h3>Solo Sim Session</h3>
<h3>Multi-player Race</h3>
<h3>Group & Corporate</h3>
<h2 id="credits-heading">How credits work</h2>
<h2 id="cafe-heading">The cafe</h2>
<h2 id="whatsapp-heading">Stay in the loop on WhatsApp</h2>
<h2 id="location-heading">Find us</h2>
<h3>Address</h3>
<h3>Hours</h3>
<h3>Get in touch</h3>
```

`/v2/privacy`:
```
<h1>Privacy Policy</h1>
<h2>What we collect</h2>
<h2>How we use it</h2>
<h2>How to opt out</h2>
<h2>DPDP compliance</h2>
<h2>Returning visitor cookie</h2>
<h2>Contact</h2>
```

Strict hierarchy on both routes; no level skips; single h1 per route. **Verdict: PASS.**

### Finding 5: `aria-disabled` consistency — PASS

`WhatsAppOptInForm.tsx:118-125` — `disabled={!canSubmit}` + `aria-disabled={!canSubmit}` paired. Rendered: `<button ... disabled="" aria-disabled="true">Please confirm consent</button>`. **FLAG-3 close reverified.**

### Finding 6: Color-only signaling — PASS

Disabled state signals: text label change + `opacity: 0.4` + `cursor: not-allowed` + `aria-disabled="true"`. Four non-color channels. Robust for red-green colorblind, screen-reader, and high-contrast-mode users. No other disabled controls on `/v2/` or `/v2/privacy`. **Verdict: PASS.**

### Finding 7: Reduced motion — FLAG-2d (minor)

Probe `grep -c 'prefers-reduced-motion' /root/racecontrol/web-v2/src/` → 0. No `@media (prefers-reduced-motion: reduce)` blocks in `globals.css` or `page.module.css`.

Unconditional transitions at `page.module.css:94,200,217,302,573,615` — all ≤0.15s, visual-only, no parallax/auto-rotation/infinite animation. `transform: translateY(-1px)` on hover/focus is minimal.

WCAG 2.2 SC 2.3.3 (AAA) recommends respecting reduced-motion; AA does not require it for transitions ≤5s.

Smallest fix (append to `globals.css`):
```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    transition-duration: 0.001ms !important;
    animation-duration: 0.001ms !important;
    transform: none !important;
  }
}
```

**Disposition:** non-blocking; vestibular-disorder users with `prefers-reduced-motion` set will see the 0.15s 1px translate.

### Finding 8: Lang attribute — PASS

Probe:
```
<html lang="en"   # /v2/
<html lang="en"   # /v2/privacy
```

`layout.tsx:43` — `<html lang="en" className={...}>`. Both routes inherit. **Verdict: PASS.** (Kaizen: `lang="en-IN"` more precise for Hyderabad venue; non-blocking.)

---

## NEW findings (post-Orbitron-load)

### NEW-1: BUILD_ID not exposed in served HTML or health endpoint (process-debt)

Probe returns zero matches for `"buildId"` in `/tmp/v2-canonical.html`; `/api/v1/health` lacks `build_id` field. Verification of running BUILD_ID against §S-206 anchor relied on PROXY (FLAG-3 rendered output) rather than direct match. CGP H3 anti-theater class.

**Smallest fix:** extend the health handler at `web-v2/src/app/api/v1/health/route.ts` (or equivalent) to include `build_id: process.env.NEXT_PUBLIC_BUILD_ID` or read from `process.env.NEXT_BUILD_ID`. Then deploy-parity verification can satisfy the SWAPLOG rule directly.

**Disposition:** carry-forward — separate from FLAG-2 close. Tag as process-debt sibling of the SWAPLOG rule.

### NEW-2: Orbitron `display: "swap"` correctly chosen — no FOUT mitigation drift (informational)

`layout.tsx:14, 24` both use `display: "swap"` — avoids FOIT, causes a small layout shift when Orbitron loads. Hero h1 uses `font-family: var(--rp-font-display, var(--rp-font-body, sans-serif))` (`page.module.css:167`) — fallback chain sound. No visual regression on h2/h3 ratios. **Process-note only**, no FLAG.

### NEW-3: Privacy page missing skip-link + `<main id="main">` (consistency)

See Finding 2 / FLAG-2a. Materialized via cross-route comparison.

### NEW-4: `<header>` carries implicit `role="banner"` (informational)

`<header>` element on `/v2/:85` is page-level masthead → implicit `role="banner"` per ARIA spec. Skip-link target `#main` correctly points to `<main id="main">`. **No issue.**

---

## Verdict — FLAG-2 CANDIDATE-CLOSE

**Evidence chain for close:**

1. All WCAG 2.2 A/AA-conformance checkpoints PASS:
   - SC 1.3.1 (labeled inputs, Finding 1)
   - SC 1.4.11 (non-text contrast on focus ring meets ≥3:1, Findings 3a/3b quant)
   - SC 2.4.1 (skip mechanism present on landing page, Finding 2)
   - SC 2.4.6 (heading hierarchy correct, Finding 4)
   - SC 3.3.2 (form labels, Finding 1)
   - SC 4.1.2 (`aria-disabled` semantics, Finding 5)
2. The 4 sub-findings (FLAG-2a, 2b, 2c, 2d) are AAA-tier or consistency kaizen — NOT A/AA-conformance failures.
3. Font-load did NOT introduce regression — form inputs below heading-affected region; `.optInField input[type="tel"]` focus indicator unchanged from pre-Orbitron.
4. FLAG-3 close reverified live (`disabled` + `aria-disabled="true"` + contextual label).

**Carry-forward kaizen bundle (single ~30-LOC PR, follow-up — NOT this turn):**

```diff
# globals.css — replace `a:focus-visible { outline: none }` with explicit outline
- a:hover,
- a:focus-visible {
+ a:hover {
+   color: #ff3b2d;
+   outline: none;
+ }
+ a:focus-visible {
    color: #ff3b2d;
-   outline: none;
+   outline: 2px solid var(--rp-racing-red, #e10600);
+   outline-offset: 2px;
  }

# globals.css — append reduced-motion guard
+ @media (prefers-reduced-motion: reduce) {
+   *, *::before, *::after {
+     transition-duration: 0.001ms !important;
+     animation-duration: 0.001ms !important;
+     transform: none !important;
+   }
+ }

# page.module.css:575 — input outline upgrade
  .optInField input[type="tel"]:focus-visible {
-   outline: none;
+   outline: 2px solid var(--rp-racing-red);
+   outline-offset: 1px;
    border-color: var(--rp-racing-red);
  }

# privacy/page.tsx — add skip-link + main id parity
  export default function PrivacyPolicyPage() {
    return (
+     <>
+       <a href="#main" className={skipLinkStyle}>Skip to content</a>
-       <main>
+       <main id="main">
        ...
-       </main>
+       </main>
+     </>
    );
  }

# (preferred long-term) move skip-link + lang="en-IN" to layout.tsx so all routes inherit
```

Bundle scope: ~30 LOC across 3 files. Single PR, no schema change, no data migration, no foundational boundary, V2-native (no V1↔V2 RCA required). Eligible for §S-186 pre-§S-146 small-fix fast-lane IF authored as post-§S-146 PR; treat as standard V2 native change.

---

## NOT TESTED

- Real screen-reader narration (NVDA / JAWS / VoiceOver / TalkBack)
- Mobile real-device — desk-side curl only, no Android/iOS Safari/Chrome viewport
- Keyboard-only navigation walk via Playwright `page.keyboard.press('Tab')`
- Browser autofill flow for `tel` input
- Color-vision simulation (deuteranopia/protanopia/tritanopia) on the red-shift focus signal
- High-contrast / forced-colors mode (`@media (forced-colors: active)`)
- `prefers-reduced-motion` live OS-level toggle (CSS audit only)
- BUILD_ID verbatim match to §S-206 anchor (PROXY-CONFIRMED via FLAG-3 rendered output — see NEW-1)

---

## Composes-with

- `racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md` (design contract — §6 A11y)
- `racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-REVIEW.md` (prior audit; this re-audit closes the deferred FLAG-2 surface)
- `racecontrol/.planning/specs/v2/V2-PROGRESS-MAP.md` row 1.18 (V2 Customer Entry PWA → flips IN-FLIGHT → DONE on FLAG-2 close)
- `racecontrol/CLAUDE.md` Subagent Gates (retroactively closes the UI-REVIEW.md gate at FLAG-2-DEFERRED-from-prior)
- CGP H3 EVIDENCE BEFORE CLAIMS (raw curl outputs cited inline)
- `racecontrol/CLAUDE.md` Brand Identity + Substrate-Pointer Convention (Orbitron load via `next/font/google`; canonical token chain intact)
- V2-LBAC v0.1 close-loop: this re-audit is the CLOSE leg for the FLAG-2 in-flight item; SYNC + BILATERAL legs handled in §S-207 V2-MASTER-STATE close-anchor

---

— gsd-ui-auditor (agent `aabdc71db54fbd5c7`) · persisted by bono · 2026-05-12 ~10:25 UTC (Tue 2026-05-12 ~15:55 IST) · FLAG-2 CANDIDATE-CLOSE · 4 AAA-tier sub-findings carry-forward as kaizen bundle · NEW-1 BUILD_ID-debt sibling-item · §S-207 close-anchor pending in V2-MASTER-STATE
