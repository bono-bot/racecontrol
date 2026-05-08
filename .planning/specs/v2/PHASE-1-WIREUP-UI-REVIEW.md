# Phase 1 Wire-up — UI Review (Wave 0 POS .130 surfaces)

**Audited:** 2026-05-08 (IST)
**Branch / HEAD:** `feat/pact-001-phase-1-wireup` @ `ba17088f`
**Baseline:** PACT-20260506-001 §AMEND-1 + Phase 1 wire-up PLAN §AMEND-1.B (NF-james-B WARN gate) + §AMEND-1.D (Arabic-Indic) + §AMEND-1.E (PrivilegedAction enum) + Q5-A (session cookie sufficient for non-privileged) + DoD §1.2 (90s registration target) + race-engineer voice ("show, don't gate") + brand identity (Racing Red `#E10600` / Asphalt Black `#1A1A1A` / Gunmetal Grey `#5A5A5A` / Montserrat / OLD orange `#FF4400` deprecated).
**Method:** Code-only retroactive audit. **No screenshots captured** (no dev server running on .27 in this audit pass; this is a pre-PR-open gate, not a deploy gate). Evidence anchored on file:line.
**Surface in scope:** 7 files (`pos/lookup/page.tsx` + 5 POS components + 1 auth pill) — *not* the rest of web-v2.

---

## Pillar Scores (1-5; mean ≥3.5 = PASS, any <3.0 = FLAG, any <2.0 = BLOCK)

| # | Pillar | Score | Verdict | Key finding |
|---|---|---|---|---|
| 1 | Information architecture | **4.0** | PASS | State machine clean: idle/looking_up/found/not_found/error/walkin all readable in `page.tsx:37-43`. Single primary action (phone field) auto-focused. |
| 2 | Visual hierarchy | **3.5** | PASS | Cards have consistent padding-rhythm + uppercase metadata labels. Two minor scan-issues: `<h1>` vs `<h2>` size near-identical (1.5rem vs 1.5rem, only weight differs); error banner has no leading icon glyph (string `!`). |
| 3 | Interaction patterns | **2.5** | **FLAG** | §AMEND-1.D Arabic-Indic / Devanagari digit support is silently broken: `phone.replace(/\D/g, "")` strips `१२३` to empty. WARN gate fires on `+91…` paste (correct intent, but no E.164 path). |
| 4 | Brand consistency | **3.0** | **FLAG** (borderline) | Brand color tokens used correctly throughout. **Montserrat is NOT loaded** — every component falls back to system-ui. `--rp-font-body` referenced in `ManagerPill.module.css:6` but never declared in `globals.css`. No deprecated `#FF4400` anywhere (clean). |
| 5 | Accessibility | **3.5** | PASS | aria-label on badges + slots, `role="alert"` on error banner, `role="status" aria-live="polite"` on NotFound, autoFocus on phone, focus-visible outline on pill. Two gaps: looking_up state has no aria-live, "Continue anyway" inline button is a styled link with no underline-on-focus. |
| 6 | Implementation quality | **3.5** | PASS | CSS modules used correctly. No `display: none` (CAVEAT-2 clean — verified via grep). Two inline-style anti-patterns (page.tsx:135-143 reset button; PhoneLookupInput.tsx:80-89 ack link) — fix is mechanical. ManagerPill.test.tsx exists; 0 tests for the other 6 components is an explicit Wave 0 deferral, not a blocker. |

**Mean: 3.33** — falls below the 3.5 PASS threshold by 0.17 because Pillars 3 and 4 are FLAGs.
**No BLOCK pillar.** Both FLAGs have small reversible fixes that close them without scope expansion.

---

## Top FLAG fixes (smallest reversible change each)

### FLAG-1 — Arabic-Indic digit support broken (Pillar 3, §AMEND-1.D contract violation)

`web-v2/src/components/v2/pos/PhoneLookupInput.tsx:28,34`

```ts
// CURRENT — strips Devanagari/Arabic-Indic to empty
const digitsOnly = phone.replace(/\D/g, "");
const next = e.target.value.replace(/\D/g, "").slice(0, 15);
```

Empirical confirmation (Node): `'१२३'.replace(/\D/g, '')` → `""`. `\D` is `[^0-9]` ASCII-only.

**Fix sketch:** Add a normalize step that maps Devanagari `[०-९]` and Arabic-Indic `[٠-٩]` to ASCII `0-9` *before* the strip. Helper belongs in `lib/types/cirs-lookup.ts` so the Playwright codepoint test (PLAN.md Session 6 §5.4) targets a stable export.

```ts
// lib/types/cirs-lookup.ts
export function normalizeDigits(s: string): string {
  return s.replace(/[०-९]/g, d => String(d.charCodeAt(0) - 0x0966))
          .replace(/[٠-٩]/g, d => String(d.charCodeAt(0) - 0x0660));
}

// PhoneLookupInput.tsx (2 sites)
const digitsOnly = normalizeDigits(phone).replace(/\D/g, "");
const next = normalizeDigits(e.target.value).replace(/\D/g, "").slice(0, 15);
```

This is the minimum surface that satisfies §AMEND-1.D. Eastern Arabic-Indic (`[۰-۹]`) is NOT in §AMEND-1.D scope; defer.

### FLAG-2 — Montserrat not loaded; brand-voice typography is system-ui (Pillar 4)

`web-v2/src/app/layout.tsx:1-19` + `web-v2/src/app/globals.css:14`

```css
/* globals.css line 14 — current */
font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
```

```ts
/* layout.tsx — no next/font import */
import type { Metadata } from "next";
import "./globals.css";
```

`ManagerPill.module.css:6` already references `var(--rp-font-body, …)` with a system-ui fallback — token plumbing is half-built but the variable is never declared.

**Fix sketch (4 lines, layout.tsx + globals.css only — no component changes):**

```tsx
// layout.tsx
import { Montserrat } from "next/font/google";
const montserrat = Montserrat({ subsets: ["latin"], weight: ["400","700"], variable: "--rp-font-body", display: "swap" });
// in <html> wrapper
<html lang="en" className={montserrat.variable}>
```

```css
/* globals.css :root — replace line 14 */
font-family: var(--rp-font-body), system-ui, -apple-system, "Segoe UI", Roboto, sans-serif;
```

This light-touch fix lets every existing component CSS module (which inherits body font) immediately render Montserrat, including the already-declared `var(--rp-font-body)` reference in ManagerPill. Zero component edits.

---

## Detailed findings

### Pillar 1 — Information architecture (4.0/5, PASS)

- State machine in `page.tsx:37-43` is a 6-variant discriminated union; every `view.kind` is exhaustively rendered (lines 118-172). Readable to a new staff-AI in <30s.
- Primary action: phone input is `autoFocus` (`PhoneLookupInput.tsx:59`). Single visible CTA at the start of the flow — no decision paralysis.
- "Lookup another customer" reset is alignSelf:flex-start (page.tsx:142) — implicit visual demotion below the profile card. Good.
- WalkInGuestDropdown also renders alongside `not_found` (line 170-172) — small redundancy with NotFoundCTA's `Use Walk-In Guest` button, but the visible-second-CTA reduces clicks-to-walkin to 1 in the not_found path. Acceptable.
- *Minor:* the `looking_up` `searchedPhone` field (line 81) is captured but never displayed back to staff — could ease "did the keypress register?" anxiety. Defer.

### Pillar 2 — Visual hierarchy (3.5/5, PASS)

- Typographic scale in use: 1.5rem (h1, h2.name, metricValue), 1.125rem (heading, tierValue), 1rem (slotName, error-heading), 0.9375rem (NotFound buttons), 0.875rem (body, label, error body), 0.75rem (metric labels, pill category). 7 sizes — close to the typical 6-step scale. Not excessive but not enforced via tokens either; one drift could cascade.
- Spatial rhythm: every card uses 1rem-1.5rem padding + 0.75rem gap. Consistent.
- Uppercase + letter-spacing on metadata labels (metric, tier, slotStatus) is correctly secondary-weight visual.
- *Minor flag:* `<h1>` (page.tsx:111, 1.5rem/700) and `<h2>` ProfilePreview name (1.5rem/700) are typographically identical. The name should out-rank the page title or the title should out-rank the name; current pair invites cognitive scan-confusion at the moment a profile renders.
- `LookupErrorBanner.tsx:30-32` uses a string `!` glyph for the error icon — works but inconsistent with the otherwise-iconless system. Either remove (rely on red border/background, already strong enough) or commit to inline-SVG icons (the locked tech-stack rejects Lucide per Layer 1 design contract per session-handoff).

### Pillar 3 — Interaction patterns (2.5/5, **FLAG**)

- See FLAG-1 above (Arabic-Indic strip).
- WARN gate at `PhoneLookupInput.tsx:29` correctly implements §AMEND-1.B: WARN-shown but submission allowed after `Continue anyway` ack. Race-engineer voice "show, don't gate" satisfied.
- The WARN copy (`PhoneLookupInput.tsx:71-74`): "This doesn't look like an Indian mobile number (Indian mobiles start with 6, 7, 8, or 9)." — informative without being scolding. Good.
- Submit button label dynamically swaps between "Lookup" and "Confirm & lookup" (line 67) — good signal that the WARN ack is required.
- `+91XXXXXXXXXX` paste case: `+` strips, `91` becomes prefix-9 → `isIndianMobilePrefix` returns true → submits as 12-digit `91…`. Works *accidentally* (canonicalize_phone substrate handles country code), but UX undocumented. Defer to copy-clarity sub-PACT.
- `slice(0, 15)` digit cap (line 34) is correct for E.164 max length.
- WalkInGuestDropdown `slotsOccupied` defaulting to `{guest1:false, guest2:false}` (line 37): Wave 0 surface ships visually-correct but data-incorrect — staff might double-book. The component comment explicitly flags this as a known Wave 0 limitation. Acceptable as long as Session 5 wires the live source before customer-facing Wave 0 ship; track in DEPLOY-MANIFEST.
- Error banner has `Retry` and `Use Walk-In Guest` — good two-path recovery, no dead-end.

### Pillar 4 — Brand consistency (3.0/5, **FLAG**)

- **Color usage:** Racing Red `--rp-racing-red` (#E10600) used as accent on submit button + focus-state input border + needsPin pill border + new-customer badge + register CTA. Asphalt Black on input bg + tier slot bg + monospace customer-id chip. Gunmetal Grey explicitly named in `ManagerPill.module.css:32`. Token discipline is consistent — every CSS-module references CSS custom properties from `globals.css:18-29`.
- **No hardcoded `#FF4400`** anywhere in the audited surface (grep: 0 hits across `web-v2/src/components/v2`). Clean against the deprecated-orange rule.
- **Montserrat:** SEE FLAG-2. Not loaded. Every text node currently renders system-ui. The brand voice is the typography in this stack (no logo on screen) — system-ui ≠ Racing Point. This is the single biggest brand-consistency gap.
- **Race-engineer voice ("show, don't gate"):**
  - Phone WARN gate is an exemplar — shows + warns + does not block. Score: clean.
  - NotFound copy "We didn't find a registered customer for X. Either register them now (PWA), or use a Walk-In Guest slot for this session." — paths-not-walls, factual, no apology theater. Clean.
  - Error banner "Lookup failed" + raw code — engineer-honest. Clean. ("Something went wrong" pattern correctly avoided.)
  - ProfilePreview tier slot: "Tier display pending Wave 4 MI substrate (experience-score)" — explicit, not hidden. Clean.
- *Minor:* "Customer tier & offer" label uses `&amp;` HTML entity (ProfilePreviewCard.tsx:83) — correct in JSX but worth an explicit doc since other labels use plain text.

### Pillar 5 — Accessibility (3.5/5, PASS)

- aria-label on NEW/REPEAT badge (`ProfilePreviewCard.tsx:61`) and Walk-In slot (`WalkInGuestDropdown.tsx:47-50`).
- `role="alert"` on LookupErrorBanner (interrupts SR), `role="status" aria-live="polite"` on NotFoundCTA (announces non-disruptively). Correct two-tier urgency split.
- `aria-describedby="phone-warn"` on input when WARN visible (`PhoneLookupInput.tsx:63`). SR users get the WARN context.
- Manager pill `aria-label` distinguishes verified vs needs-pin states (`ManagerPill.tsx:79-83`).
- `:focus-visible` outline on pill (`ManagerPill.module.css:18-21`) — visible keyboard focus.
- *Gap 1:* `looking_up` status line `<p>` (page.tsx:126) has no `aria-live="polite"` — SR users won't know the lookup is in flight. 1-line fix: add `role="status" aria-live="polite"` to the status paragraph.
- *Gap 2:* "Continue anyway" inline button (`PhoneLookupInput.tsx:77-91`) is a styled-as-link button with `text-decoration: underline` — fine *visually*, but on focus it inherits no outline. The inline-style block doesn't include `:focus-visible`. Also stylistic — see Implementation FLAG below.
- *Gap 3:* Color-contrast spot-check on `--rp-text-muted: #a0a0a0` over `--rp-card: #222222` — calculated contrast ≈ 6.5:1 (WCAG AA passes for body text, AAA for large). Acceptable.
- *Gap 4:* Walk-In `disabled` state at 0.6 opacity (line 53 module.css) — at 0.6 against the dark card the muted text contrast drops below 4.5:1. Borderline. Increase to 0.7 or pair with `cursor: not-allowed` only. Defer.

### Pillar 6 — Implementation quality (3.5/5, PASS)

- **CSS modules:** every component pairs with a `.module.css` and uses `styles.foo`. No global class leakage. ✓
- **CAVEAT-2 (no `display: none` for fallback components):** verified via `grep -rn 'display:\s*none'` — 0 hits in `src/`. ✓ Compliance achieved by conditional render at parent (page.tsx:118-172) — components are mounted/unmounted via the discriminated-union view, not hidden.
- **Inline-style anti-patterns (2 sites):**
  - `pos/lookup/page.tsx:135-143` — reset button uses object-literal style. Should be a `.module.css` class on `page.module.css`. 8 lines → 1 class.
  - `PhoneLookupInput.tsx:80-89` — "Continue anyway" link-button uses object-literal style. Should be `.warnAck` class in `PhoneLookupInput.module.css` so :focus-visible can be properly styled (closes accessibility Gap 2 same patch).
- **Component composition:** page.tsx is the sole orchestrator; the 5 POS components are render-only (no internal API calls). Clean separation. Manager pill is render-only by explicit comment (line 13-15) — kaizen-discipline OK.
- **Dead code:** none observed. The `tier`/`discount_pct` optional fields on `ProfilePreviewWithTier` are explicitly Wave 4 placeholders (commented). Not dead.
- **Test coverage:** ManagerPill has `.test.tsx`; the 5 POS components do not. PLAN §5.3 schedules these for Session 6. Wave 0 ship can pre-date that ONLY if DEPLOY-MANIFEST flags it; otherwise Session 6 must precede PR-open.
- **TypeScript types:** all imports use the `@/lib/types/cirs-lookup` module + `@/lib/types/privileged-action` (Rust enum mirror). No `any` observed in the audited surface. ✓ (compliance with CLAUDE.md "No `any` in TypeScript" standing rule.)
- **`view.kind === "looking_up"` AND `view.kind === "idle"`** show PhoneLookupInput. Submit button correctly disabled while `looking_up` (page.tsx:122). No double-submit window.

---

## Off-scope notes (informational, not scored)

- The page lives at `app/pos/lookup/page.tsx` (URL `/v2/pos/lookup`) — page comment lines 12-16 documents the basePath de-prefix correction (PLAN.md said `app/v2/pos/lookup`). Correct call; basePath drift caught early.
- API endpoint `/api/v1/cirs/lookup` is unwired in Wave 0 (page.tsx:18-22 acknowledges Session 5 proxy scope). Not an audit issue at this gate.
- `globals.css:43-48` sets `<main>` max-width:720px — `page.module.css:.shell` overrides to 640px. Slight token drift; pick one and document. Defer.

---

## Verdict

**Mean 3.33/5 — FLAG-only (no BLOCK).** Both FLAGs (FLAG-1 Arabic-Indic, FLAG-2 Montserrat) have surgical, no-component-edit fixes that close them and lift the mean to ~3.83/5 on a re-audit. Recommend:

1. Apply FLAG-1 + FLAG-2 + the 2 inline-style → CSS-class refactors as a single follow-up commit on `feat/pact-001-phase-1-wireup` BEFORE PR-open.
2. Add a Playwright codepoint smoke test (per PLAN §5.4) that pastes `९८७६५४३२१०` and asserts the input shows `9876543210` — locks FLAG-1 closure.
3. Add `aria-live="polite"` to the looking_up status line (1-line accessibility close-out).

After those four edits + a PR-open screenshot pass on POS .130 (per CLAUDE.md "Visual verification for display-affecting deploys" + the screenshot-enforcement hook on this branch), the surface clears the UI gate.

Per-PR Captain merge auth gate STANDS regardless of UI-REVIEW outcome.

---

## Files audited

| File | LoC | Notes |
|---|---|---|
| `web-v2/src/app/pos/lookup/page.tsx` | 175 | orchestrator + state machine |
| `web-v2/src/app/pos/lookup/page.module.css` | 36 | shell/header/title |
| `web-v2/src/components/v2/pos/PhoneLookupInput.tsx` | 98 | WARN gate + Arabic-Indic FLAG |
| `web-v2/src/components/v2/pos/PhoneLookupInput.module.css` | 75 | tokens-clean |
| `web-v2/src/components/v2/pos/ProfilePreviewCard.tsx` | 122 | NEW/REPEAT badge + tier placeholder |
| `web-v2/src/components/v2/pos/ProfilePreviewCard.module.css` | 166 | tokens-clean |
| `web-v2/src/components/v2/pos/WalkInGuestDropdown.tsx` | 85 | slot occupancy surface |
| `web-v2/src/components/v2/pos/WalkInGuestDropdown.module.css` | 97 | tokens-clean |
| `web-v2/src/components/v2/pos/NotFoundCTA.tsx` | 52 | dual-CTA recovery |
| `web-v2/src/components/v2/pos/NotFoundCTA.module.css` | 71 | tokens-clean |
| `web-v2/src/components/v2/pos/LookupErrorBanner.tsx` | 48 | role=alert + retry |
| `web-v2/src/components/v2/pos/LookupErrorBanner.module.css` | 59 | tokens-clean |
| `web-v2/src/components/v2/auth/ManagerPill.tsx` | 95 | render-only by design |
| `web-v2/src/components/v2/auth/ManagerPill.module.css` | 78 | references undeclared --rp-font-body (FLAG-2) |
| `web-v2/src/lib/types/cirs-lookup.ts` (lines 60-76 only) | 17 | isIndianMobilePrefix helper |
| `web-v2/src/app/globals.css` (referenced) | 67 | brand tokens declared; Montserrat NOT loaded |
| `web-v2/src/app/layout.tsx` (referenced) | 19 | no next/font import (FLAG-2) |

— UI auditor / 2026-05-08 IST · retroactive code-only audit (no screenshots; pre-PR-open gate) · branch `feat/pact-001-phase-1-wireup` HEAD `ba17088f` · mean 3.33/5 → 2 FLAGs + 0 BLOCKs · ship-eligible after FLAG-1 + FLAG-2 patches
