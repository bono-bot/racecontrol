# V2-Theme Kiosk Migration — Producer Handoff (2026-05-15 IST)

> **Producer handoff, not a session-handoff.** Out of `session_handoff_*.md` PreToolUse schema; reviewer should treat as forward-work spec under V2-LBAC v0.1.
>
> **Snapshot timestamp:** 2026-05-15T12:21Z (Fri 2026-05-15 17:51 IST) — live wall-clock fetched, not memory-projected. Per-page grep counts taken at this timestamp against racecontrol HEAD = `1295184d`. Verify against current HEAD before executing; counts drift on every kiosk-touching commit.
>
> **Author:** james@racingpoint.in (Opus 4.7 1M-context session) under Captain "Producer handoff for the V-theme gap" 2026-05-15 ~17:56 IST.
> **Ledger anchor (pending):** §S-N close-anchor in `comms-link/V2-MASTER-STATE.md` to be appended on doc commit; reference back to this file once allocated.

---

## §0 — Hot-cache TL;DR

14 kiosk pages live at [racecontrol/kiosk/src/app/](../../kiosk/src/app/). 5 are clean V2 (`page.tsx`, `settings`, `demo`, `pod/[number]`, `register`-mostly). 9 need work. Customer-impact ordered hit list:

1. **[fleet/page.tsx](../../kiosk/src/app/fleet/page.tsx)** — staff-facing, 403 LOC, V2=2 / legacy=1 / generic-tw=26. §S-186 fast-lane eligible (presentation-only).
2. **[spectator/page.tsx](../../kiosk/src/app/spectator/page.tsx)** — customer-visible, **1076 LOC**, V2=3 / legacy=3 / generic-tw=48. §S-146 full RCA required (>200 LOC).
3. **[staff/page.tsx](../../kiosk/src/app/staff/page.tsx)** — 925 LOC, V2=5 / legacy=3 / generic-tw=20. **§S-146** (>200 LOC). Outer chrome already V2 via child components; gap is page-body.
4. **[blanking/page.tsx](../../kiosk/src/app/blanking/page.tsx)** — idle screen, 311 LOC, V2=0 / legacy=0 / generic-tw=0. Zero theming — needs full V2 design (not a token swap; design work).
5. **[shutdown/page.tsx](../../kiosk/src/app/shutdown/page.tsx)** — customer-visible end-of-session, 414 LOC, V2=25 / legacy=5 / generic-tw=20. §S-146 required (>200 LOC); mostly V2, 5 legacy markers to sweep.
6. **[control/page.tsx](../../kiosk/src/app/control/page.tsx)** — 444 LOC, V2=9 / legacy=3 / generic-tw=9. §S-146 required.
7. **[register/page.tsx](../../kiosk/src/app/register/page.tsx)** — customer onboarding, 176 LOC, V2=12 / legacy=1 / generic-tw=11. §S-186 fast-lane eligible.
8. **[preview-idle/page.tsx](../../kiosk/src/app/preview-idle/page.tsx)** — 209 LOC, V2=8 / legacy=0 / generic-tw=12. §S-186 fast-lane eligible.
9. **[debug/page.tsx](../../kiosk/src/app/debug/page.tsx)** — staff-only diag, **1867 LOC**, V2=39 / legacy=25 / generic-tw=134. §S-146 + MMA Step 1 (foundational due to size + legacy depth). DEFER unless Captain re-prioritizes — lowest customer impact.

**Pre-flight verify:** [preview-hud/page.tsx](../../kiosk/src/app/preview-hud/page.tsx) is a 52-line stub (V2=0 / legacy=0 / generic=2). Confirm live-use before investing — may be vestigial.

---

## §1 — Current-state evidence (live, this session)

### 1.1 Deployed kiosk

| Field | Value | Source |
|-------|-------|--------|
| URL | `http://192.168.31.23:3300/kiosk/staff` | venue Server .23 :3300 |
| `git_commit` | `fd59cf4b-dirty` | `curl http://192.168.31.23:3300/kiosk/build-info.json` 2026-05-15T12:21Z |
| `build_time_utc` | `2026-05-14T09:25:15.255Z` (14:55 IST) | same |
| `dirty: true` | working-tree had uncommitted changes at build | same |
| `fd59cf4b` resolves to | `fix(web-v2): row 1.4 AMPLIFIER caveat disposition C1+I1 — hover-red token + privacy chrome wrap` 2026-05-14 04:25 IST | `git log -1 fd59cf4b` |
| racecontrol HEAD | `1295184d` | this session |
| Commits between deploy and HEAD that touch `kiosk/` | **0** | `git log fd59cf4b..HEAD -- kiosk/` |
| Visual ground truth | screenshot `C:\Users\bono\kiosk-staff-deployed-20260515.png` | chrome-devtools MCP via this session |
| What the screenshot shows | RACINGPOINT / STAFF TERMINAL / Tap to Sign In / Staff PIN required / ← Customer Login | Orbitron font, Racing Red icon, Asphalt Black background — V2 brand correct |

**Implication:** deployed kiosk source matches racecontrol HEAD on the kiosk subtree. "Stale" reports against `/kiosk/staff` are NOT a deploy gap — they are a **migration gap** (page still has legacy/generic Tailwind classes not yet converted to V2 `rp-*` tokens). Migration work is the producer scope of this handoff.

### 1.2 V2-marker survey (timestamp 2026-05-15T12:21Z, HEAD 1295184d)

| Page | LOC | V2 markers | Legacy markers | Generic TW | Eligibility |
|------|-----|-----------|---------------|------------|-------------|
| `page.tsx` (root /kiosk) | 490 | 17 | 0 | 5 | CLEAN — no migration needed |
| `staff/page.tsx` | 925 | 5 | 3 | 20 | §S-146 |
| `blanking/page.tsx` | 311 | 0 | 0 | 0 | DESIGN-WORK (not token swap) |
| `control/page.tsx` | 444 | 9 | 3 | 9 | §S-146 |
| `debug/page.tsx` | 1867 | 39 | 25 | 134 | §S-146 + MMA Step 1 |
| `demo/page.tsx` | 311 | 15 | 0 | 4 | CLEAN |
| `fleet/page.tsx` | 403 | 2 | 1 | 26 | §S-186 fast-lane |
| `preview-hud/page.tsx` | 52 | 0 | 0 | 2 | VERIFY-USE first |
| `preview-idle/page.tsx` | 209 | 8 | 0 | 12 | §S-186 fast-lane |
| `pod/[number]/page.tsx` | 159 | 6 | 0 | 0 | CLEAN |
| `register/page.tsx` | 176 | 12 | 1 | 11 | §S-186 fast-lane |
| `settings/page.tsx` | 293 | 33 | 0 | 2 | CLEAN |
| `shutdown/page.tsx` | 414 | 25 | 5 | 20 | §S-146 |
| `spectator/page.tsx` | 1076 | 3 | 3 | 48 | §S-146 |

**Survey regex (reproducible):**

```bash
cd C:/Users/bono/racingpoint/racecontrol/kiosk/src/app && for page in <list>; do
  lines=$(wc -l < "$page")
  v2=$(grep -cE 'rp-(red|black|grey|border|card|surface|red-hover)|font-display|Orbitron' "$page")
  legacy=$(grep -cE 'Enthocentric|#FF4400|bg-(orange|amber|slate)-[0-9]|text-(orange|amber|slate)-[0-9]' "$page")
  generic=$(grep -cE 'bg-(gray|zinc|neutral|stone|red|blue|green|yellow)-[0-9]|text-(gray|zinc|neutral|stone|red|blue|green|yellow)-[0-9]' "$page")
done
```

---

## §2 — Canonical V2 design substrate (read before editing)

**Substrate-pointer convention applies (racecontrol/CLAUDE.md §Doctrine Conventions).** Do NOT trust derived summaries; read these sources directly:

| Surface | Canonical path | Verified 2026-05-15 |
|---------|---------------|---------------------|
| Color tokens (single source of truth) | [racecontrol/packages/shared-tokens/tokens.css](../../packages/shared-tokens/tokens.css) | 1254 bytes ✓ |
| Kiosk app globals (re-imports tokens + fonts + body chrome) | [kiosk/src/app/globals.css](../../kiosk/src/app/globals.css) | 5863 bytes ✓ |
| V2 design system substrate (full UI doctrine) | [comms-link/v2-skeleton/10-ui-design-system.md](../../../../comms-link/v2-skeleton/10-ui-design-system.md) | 9503 bytes ✓ (read before any page touch) |
| Brand assets (logos) | [racecontrol/brand-assets/README.md](../../brand-assets/README.md) | exists ✓ |

### 2.1 V2 color tokens (canonical names — verbatim from tokens.css)

```
--rp-red:        #E10600   // Tailwind: rp-red
--rp-red-hover:  #FF1A1A   // Tailwind: rp-red-hover  (web previously had rp-red-light; rename complete)
--rp-black:      #1A1A1A   // Tailwind: rp-black
--rp-grey:       #5A5A5A   // Tailwind: rp-grey
--rp-surface:    #2A2A2A   // Tailwind: rp-surface   (modals, elevated panels)
--rp-card:       #222222   // Tailwind: rp-card      (card backgrounds)
--rp-border:     #333333   // Tailwind: rp-border    (dividers / outlines)
--rp-green:      #16a34a   // Tailwind: rp-green     (success)
--rp-yellow:     #ca8a04   // Tailwind: rp-yellow    (warning)
--rp-purple:     #a855f7   // Tailwind: rp-purple    (special)
```

### 2.2 V2 fonts (verbatim from globals.css `@theme inline`)

```
--font-sans:     'Montserrat', sans-serif     // body — use Tailwind default `font-sans`
--font-display:  'Orbitron', sans-serif       // headings/branding — Tailwind `font-display`
--font-mono:     'JetBrains Mono', monospace  // lap times, hashes — Tailwind `font-mono`
--font-heading:  'Montserrat', sans-serif     // section headers — Tailwind `font-heading`
```

### 2.3 Deprecated tokens — BANNED in this migration

| Banned | Replacement | Why |
|--------|-------------|-----|
| `Enthocentric` (any reference) | `Orbitron` / `font-display` | racecontrol/CLAUDE.md §Brand Identity: "Enthocentric (display) DEPRECATED 2026-05-08 — never shipped" |
| `#FF4400` (orange) | `#E10600` (`rp-red`) | CLAUDE.md §Brand Identity: "OLD orange `#FF4400` is DEPRECATED" |
| `rp-red-light` | `rp-red-hover` | tokens.css comment: "canonical name — web previously used rp-red-light" |
| `bg-orange-[0-9]+` / `text-orange-[0-9]+` | `bg-rp-red` / `text-rp-red` (and re-evaluate semantic intent) | non-V2 palette |
| `bg-amber-[0-9]+` | `bg-rp-yellow` (if warning) or `rp-surface` | non-V2 palette |
| `bg-slate-[0-9]+` / `text-slate-[0-9]+` | `bg-rp-card` / `text-rp-grey` (per context) | non-V2 palette |

---

## §3 — Mechanical migration recipe

For pages that just need token-swaps (no design re-thinking), apply this find/replace sweep. Manual review every match — context matters:

| Find (regex) | Replace | Notes |
|--------------|---------|-------|
| `bg-gray-9(00\|50)\|bg-zinc-9(00\|50)\|bg-neutral-9(00\|50)\|bg-stone-9(00\|50)` | `bg-rp-black` or `bg-rp-card` | `rp-black` = page bg; `rp-card` = card bg |
| `bg-gray-8(00\|50)\|bg-zinc-8(00\|50)` | `bg-rp-surface` | elevated panels / modals |
| `border-gray-[7-9]00\|border-zinc-[7-9]00` | `border-rp-border` | dividers |
| `text-gray-[4-6]00\|text-slate-[4-6]00\|text-zinc-[4-6]00` | `text-rp-grey` | secondary text |
| `text-(gray\|slate\|zinc)-(100\|200\|300\|50)` | `text-white` | primary text on dark bg |
| `bg-red-[5-7]00\|text-red-[5-7]00` | `bg-rp-red` / `text-rp-red` | brand red |
| `hover:bg-red-[6-8]00` | `hover:bg-rp-red-hover` | brand red hover |
| `bg-orange-[0-9]+\|text-orange-[0-9]+` | `bg-rp-red` / `text-rp-red` (re-evaluate) | deprecated orange — likely red intent |
| `bg-amber-[5-7]00\|text-amber-[5-7]00` | `bg-rp-yellow` / `text-rp-yellow` | warnings |
| `bg-green-[5-7]00\|text-green-[5-7]00` | `bg-rp-green` / `text-rp-green` | success |
| `font-mono` | (keep — V2-aligned) | already canonical |
| `font-sans` (default) | (keep — V2-aligned via globals) | already canonical |
| Inline style `fontFamily: '...'` | drop in favor of `className="font-display"` etc. | layer-of-truth = Tailwind utility |

**Anti-pattern:** blanket sed/replace_all without reading context. A `bg-red-500` on a destructive-action button is correctly `bg-rp-red`; on a status-error chip it might want `bg-rp-red` OR (per design intent) a semantic destructive variant. Apply manually.

---

## §4 — Per-page migration entries

Each entry: scope · LOC · counts · eligibility · pre-RCA pointers · acceptance · NOT in scope.

### 4.1 `fleet/page.tsx` — STAFF-FACING — §S-186 FAST-LANE

- **Path:** [racecontrol/kiosk/src/app/fleet/page.tsx](../../kiosk/src/app/fleet/page.tsx)
- **LOC:** 403 · V2=2 · legacy=1 · generic-tw=26
- **Eligibility:** §S-186 fast-lane — ≤200 LOC borderline (403 LOC, ABOVE threshold) → **actually §S-146 required**. Re-classifying: §S-146.
- **Correction to §0 TL;DR:** fleet is §S-146, not fast-lane. Apologies for the §0 misclassification — left unedited above for audit, corrected here.
- **Customer impact:** medium — staff-only operations dashboard
- **Pre-RCA pointers (5-section):** (1) boundary map — fleet page consumes `useKioskSocket` + fleet health API; presentation-layer-only migration unless component refactors are introduced. (2) Inherited issues — none known specific to this page; check LOGBOOK for `kiosk/src/app/fleet/` grep. (3) Past-bug disposition — N/A for token migration. (4) V2-alignment delta — page currently uses generic Tailwind grays/reds; should adopt `rp-*` tokens. (5) Proposal — straight token swap per §3 recipe.
- **Acceptance:** V2 count > 20 · legacy count = 0 · generic-tw count ≤ 5 · screenshot before/after at `/kiosk/fleet` shows brand-correct rendering · `npm run build` green
- **NOT in scope:** WS contract changes · layout/IA · adding fields · component extraction

### 4.2 `spectator/page.tsx` — CUSTOMER-VISIBLE — §S-146 + LARGE

- **Path:** [racecontrol/kiosk/src/app/spectator/page.tsx](../../kiosk/src/app/spectator/page.tsx)
- **LOC:** 1076 · V2=3 · legacy=3 · generic-tw=48
- **Eligibility:** §S-146 full 5-section RCA. >200 LOC and multi-section page. Captain per-PR auth needed.
- **Customer impact:** high — spectator overlay is customer/visitor-facing
- **Risk note:** 48 generic-tw markers + 1076 LOC suggests this page may use Tailwind primitives for race-engineer visualizations (timing pylons, leaderboards) where the generic palette is intentional. Reviewer must distinguish brand-chrome (migrate) from data-visualization color encoding (preserve).
- **Migration sub-scope:**
  - Page chrome (background, panels, header) → `rp-black`/`rp-card`/`rp-surface`
  - Body text → `text-white` / `text-rp-grey`
  - Brand accents → `rp-red` / `rp-red-hover`
  - **Preserve:** semantic data colors (lap-time deltas, position-change arrows, sector colors) — these are visualization, not brand
- **Acceptance:** spectator surface chrome 100% V2; data-viz colors intentionally preserved with comment justifying each; screenshot review on actual spectator URL
- **NOT in scope:** data feed changes · layout · adding spectator features

### 4.3 `staff/page.tsx` — STAFF-FACING — §S-146

- **Path:** [racecontrol/kiosk/src/app/staff/page.tsx](../../kiosk/src/app/staff/page.tsx)
- **LOC:** 925 · V2=5 · legacy=3 · generic-tw=20
- **Eligibility:** §S-146 full RCA. >200 LOC; **but** outer chrome already V2 via `StaffLoginScreen` / `KioskHeader` / `SidePanel` components. Migration scope is page-body management UI.
- **Customer impact:** medium — staff operations terminal
- **Pre-RCA pointer:** check imported components in [staff/page.tsx](../../kiosk/src/app/staff/page.tsx) lines 1-19 — many child components already V2. Body usage of generic-tw is likely in inline JSX (status chips, list rows, modals).
- **Risk note:** staff page has `auth/JWT/sessionStorage` patterns — token migration must NOT touch auth state code paths. Presentation-only.
- **Acceptance:** V2 count > 25 · legacy = 0 · generic-tw ≤ 5 · Tap-to-Sign-In flow still renders identically post-migration · screenshot at `/kiosk/staff` matches `kiosk-staff-deployed-20260515.png` for outer chrome
- **NOT in scope:** auth flow changes · session timeout logic · WS subscription changes · component extraction

### 4.4 `blanking/page.tsx` — IDLE SCREEN — DESIGN-WORK

- **Path:** [racecontrol/kiosk/src/app/blanking/page.tsx](../../kiosk/src/app/blanking/page.tsx)
- **LOC:** 311 · V2=0 · legacy=0 · generic-tw=0
- **Eligibility:** This is NOT a token-migration. Page has zero theming markers — needs **design work** from scratch following V2 substrate.
- **Customer impact:** high — visible on every pod during idle state
- **Pre-RCA pointer:** check what blanking/page.tsx currently renders — likely a pure overlay/blank with no Tailwind. May need brand-correct idle screen (RP logo, breathing animation, time, "Tap to start" prompt) per [comms-link/v2-skeleton/10-ui-design-system.md](../../../../comms-link/v2-skeleton/10-ui-design-system.md).
- **Acceptance:** RP-branded idle screen with `rp-black` bg, `Orbitron`/`font-display` headline, breathe/pulse animation per globals.css (`.breathe` class already defined line 127-129) · screenshot on a pod (not James-local) · customer-eye review (Captain or staff)
- **NOT in scope:** kiosk routing changes · idle timeout logic

### 4.5 `shutdown/page.tsx` — CUSTOMER-VISIBLE — §S-146

- **Path:** [racecontrol/kiosk/src/app/shutdown/page.tsx](../../kiosk/src/app/shutdown/page.tsx)
- **LOC:** 414 · V2=25 · legacy=5 · generic-tw=20
- **Eligibility:** §S-146 full RCA.
- **Customer impact:** medium — end-of-session screen
- **Scope:** mostly V2 already; sweep 5 legacy markers + reduce generic-tw count
- **Acceptance:** legacy = 0 · generic-tw ≤ 5 · session-end flow renders correctly
- **NOT in scope:** end-of-session logic · billing reconciliation

### 4.6 `control/page.tsx` — §S-146

- **Path:** [racecontrol/kiosk/src/app/control/page.tsx](../../kiosk/src/app/control/page.tsx)
- **LOC:** 444 · V2=9 · legacy=3 · generic-tw=9
- **Eligibility:** §S-146 full RCA.
- **Customer impact:** medium — pod-side control surface
- **Scope:** 3 legacy + 9 generic-tw markers to sweep
- **Acceptance:** legacy = 0 · generic-tw ≤ 3
- **NOT in scope:** control logic · command dispatch · WS contracts

### 4.7 `register/page.tsx` — CUSTOMER ONBOARDING — §S-186 FAST-LANE

- **Path:** [racecontrol/kiosk/src/app/register/page.tsx](../../kiosk/src/app/register/page.tsx)
- **LOC:** 176 · V2=12 · legacy=1 · generic-tw=11
- **Eligibility:** §S-186 fast-lane — **all 6 criteria pass**: ≤200 LOC ✓ · single-boundary (presentation) ✓ · no schema ✓ · no protocol ✓ · bug/migration only ✓ · pre-§S-146-stale-equivalent (token-migration class) ✓. Use 3-section short-RCA template.
- **Customer impact:** high — first-touch customer surface
- **Scope:** 1 legacy + 11 generic-tw markers
- **Acceptance:** legacy = 0 · generic-tw ≤ 2 · registration flow E2E unchanged
- **NOT in scope:** form validation · customer record schema · WhatsApp identity binding

### 4.8 `preview-idle/page.tsx` — §S-186 FAST-LANE

- **Path:** [racecontrol/kiosk/src/app/preview-idle/page.tsx](../../kiosk/src/app/preview-idle/page.tsx)
- **LOC:** 209 · V2=8 · legacy=0 · generic-tw=12
- **Eligibility:** §S-186 fast-lane — 209 LOC borderline (≤200 cap); 9 LOC over → re-classify as §S-146. Use full RCA.
- **Correction:** §0 TL;DR listed as fast-lane; corrected here.
- **Customer impact:** low — preview/dev surface (verify live-use)
- **Pre-RCA pointer:** verify if `preview-idle` is actively routed or vestigial preview. If vestigial → DELETE candidate rather than migrate; check git log for last touch.
- **Acceptance:** if active — generic-tw ≤ 3 · if vestigial — deleted with §S-N audit-trail

### 4.9 `debug/page.tsx` — STAFF-ONLY — §S-146 + MMA-1 — DEFER

- **Path:** [racecontrol/kiosk/src/app/debug/page.tsx](../../kiosk/src/app/debug/page.tsx)
- **LOC:** **1867** · V2=39 · legacy=25 · generic-tw=134
- **Eligibility:** §S-146 + MMA Step 1 DIAGNOSE — foundational due to size + legacy depth. Per-PR Captain merge auth required.
- **Customer impact:** **lowest** — staff-only diagnostic tool, no customer surface
- **Recommendation:** **DEFER** unless Captain explicitly prioritizes. Migrating 1867 LOC for staff-diagnostic is high-effort/low-customer-impact. Document in V2-PROGRESS-MAP as "DEFERRED — staff-only diag tool" with stale-at trigger.
- **If executed anyway:** scope must batch by section (header / pod list / event stream / WS controls / etc.); each section gets its own RCA + commit.

### 4.10 `preview-hud/page.tsx` — VERIFY-USE FIRST

- **Path:** [racecontrol/kiosk/src/app/preview-hud/page.tsx](../../kiosk/src/app/preview-hud/page.tsx)
- **LOC:** 52 · V2=0 · legacy=0 · generic-tw=2
- **Eligibility:** §S-186 fast-lane (52 LOC) — BUT verify live-use first.
- **Pre-action:** `git log --oneline -- kiosk/src/app/preview-hud/` to check last-touch; grep racecontrol for `/preview-hud` route reference; if no live consumer → DELETE candidate
- **Customer impact:** unknown (likely none)

### 4.11–4.14 — Clean pages (NO migration needed)

- [`page.tsx`](../../kiosk/src/app/page.tsx) (root /kiosk) — V2=17 / legacy=0 / generic-tw=5 — CLEAN
- [`settings/page.tsx`](../../kiosk/src/app/settings/page.tsx) — V2=33 / legacy=0 / generic-tw=2 — CLEANEST
- [`demo/page.tsx`](../../kiosk/src/app/demo/page.tsx) — V2=15 / legacy=0 / generic-tw=4 — CLEAN
- [`pod/[number]/page.tsx`](../../kiosk/src/app/pod/[number]/page.tsx) — V2=6 / legacy=0 / generic-tw=0 — CLEAN

No producer work needed on these; mark as reference exemplars for migration pattern.

---

## §5 — Dependencies, blockers, sequence

### 5.1 Build + deploy

- **Build:** `cd C:/Users/bono/racingpoint/racecontrol/kiosk && npm run build` (output to `.next/`)
- **Deploy venue Server .23:** existing kiosk scheduled task on Server .23 :3300 (see racecontrol/CLAUDE.md §Server Services). Confirm with current `bash scripts/deploy/deploy-nextjs.sh kiosk` or equivalent — script presence not verified in this session.
- **Deploy parity to Bono VPS cloud:** **MANDATORY** per racecontrol/CLAUDE.md §Key Operational Rules "DEPLOY PARITY (NO EXCEPTIONS)". git push → relay `git_pull` → cloud rebuild → verify both. Incomplete cloud = NOT deployed.
- **`-dirty` build avoidance:** ensure working tree is clean at build time; otherwise `build-info.json` reports `dirty: true` and breaks provenance audit (this session's anchor).

### 5.2 Doctrine gates

| Gate | Trigger | Applied |
|------|---------|---------|
| §S-186 small-fix fast-lane | per-page ≤200 LOC + single-boundary + no schema/protocol + presentation-only | 3-section short-RCA per PR |
| §S-146 V1↔V2 RCA | per-page >200 LOC OR multi-boundary OR foundational | 5-section RCA per PR |
| MMA Step 1 DIAGNOSE | foundational + >1000 LOC (debug only) | 5-model consensus before plan |
| Per-PR Captain merge auth | foundational PRs | retained — apply-recommendations-autonomously does NOT relax |
| V2-LBAC v0.1 step 4.5 MAOR | every closure | mandatory mechanism-quality review |
| F1 SCOPE GATE | substrate-existence pre-check | apply if any test scaffolding added |
| F3 ACCOUNTING REFORM | row status definitions | mark TEST-SCAFFOLDED appropriately |
| Permanence Gate | every fix | source code only — no manual server edits |
| Universal Sync | every doctrine touch | N/A for token-migration; applies only if globals.css or tokens.css touched |
| DEPLOY PARITY | every deploy | venue + cloud both |

### 5.3 Suggested execution sequence (highest value first)

1. **register** (§S-186, 176 LOC, customer-onboarding) — smallest reversible high-impact change · 1 PR
2. **fleet** (§S-146, 403 LOC, staff-ops) — 1 PR with 5-section RCA
3. **blanking** (DESIGN-WORK, 311 LOC) — needs design-pass; consider UI subagent (`gsd-ui-researcher` → `UI-SPEC.md` → impl → `gsd-ui-auditor` → `UI-REVIEW.md`)
4. **shutdown** (§S-146, 414 LOC, customer end-of-session)
5. **staff** body (§S-146, 925 LOC, staff terminal) — careful: auth code paths untouched
6. **control** (§S-146, 444 LOC)
7. **spectator** (§S-146, 1076 LOC) — careful: data-viz colors preserved
8. **preview-idle** (§S-146 if active, DELETE if not) — verify-use first
9. **preview-hud** (§S-186 if active, DELETE if not) — verify-use first
10. **debug** — DEFER unless Captain re-prioritizes

### 5.4 Concurrent-pilot considerations

- **Branch hygiene:** each page = its own branch + PR. Per [[branch-state-mutation-by-parallel-pilot]] N=5 BILATERAL ACTIVE — use `git pull --rebase` not `--ff-only`, and `git fetch && git status` before commit-prep, after partner-sync hooks.
- **WIP cap:** V2-LBAC v0.1 limits 3 concurrent in-flight items. Two parallel pages OK; ≥3 blocks new pickup.
- **MAOR step 4.5:** every closure runs MAOR Tier-1 batch ($0.20-0.30 / ~110s) — budget ~$3 if all 7 active pages executed.
- **§S-N close-anchor push:** standing rule allows direct push to main for ledger entries.

---

## §6 — Acceptance criteria summary (per-page)

Common (every page):

1. **Token-count delta:** post-migration count of `rp-*` tokens > pre-migration; legacy count = 0; generic-tw count reduced ≥ 50% (target — actual depends on data-viz exemptions)
2. **Visual no-regression:** chrome-devtools screenshot at deployed URL post-migration shows brand-correct rendering. Compare against the §1.1 baseline screenshot where applicable.
3. **Behavior no-regression:** E2E user flow on the page unchanged (Tap-to-Sign-In for staff; lap timing for spectator; etc.)
4. **Build:** `npm run build` green; no new lint or TypeScript errors
5. **Build provenance:** `build-info.json` shows `dirty: false`
6. **Deploy parity:** venue Server .23 AND Bono VPS cloud both deployed and probed (HTTP 200 + build_time_utc fresh + screenshot ground-truth)
7. **MAOR pass:** every PR passes MAOR Tier-1 with no CRITICAL findings (per V2-LBAC §14.1)

Per-page-specific in §4 entries.

---

## §7 — NOT in scope of this handoff

The following are explicitly OUT of scope; do NOT bundle into V2-theme migration PRs:

- Functional behavior changes (auth, billing, WS protocol, session lifecycle, idle timeouts)
- Component refactors / extraction / consolidation
- Adding new pages
- Information architecture / routing changes
- Logo / image asset updates (brand-assets/ is a separate scope)
- API contract changes
- Schema / migration changes
- Customer copy / i18n strings
- Performance optimization
- A11y improvements (separate sweep)
- Test coverage additions (unless mandated by F1 SCOPE GATE for newly-touched code)

**If an executor finds a functional bug while migrating:** open a separate issue/PR; do NOT bundle.

---

## §8 — Pickup-cold context for next executor

If you're picking this up fresh (different session, different pilot, after compact):

1. **Read this doc fully** — §0 → §7
2. **Read [comms-link/v2-skeleton/10-ui-design-system.md](../../../../comms-link/v2-skeleton/10-ui-design-system.md) fully** — this is the V2 UI substrate canonical
3. **Read [racecontrol/CLAUDE.md](../../../CLAUDE.md) §Brand Identity** — verify substrate-pointer claims still resolve
4. **Verify the survey** — re-run §1.2 grep against current racecontrol HEAD. Counts drift on every kiosk commit.
5. **Pick the smallest highest-impact page** — `register/page.tsx` is the recommended start (§4.7) — fast-lane eligible, customer-onboarding, 176 LOC
6. **Author 3-section short-RCA OR 5-section RCA** per eligibility (§5.2 table)
7. **Apply migration recipe (§3)** — manually, not sed/replace_all
8. **Build, deploy parity (venue + cloud)**, MAOR pass, §S-N close-anchor
9. **Loop**

### Where to ask Captain if stuck

- **Migration recipe ambiguity** (e.g., "is `bg-orange-500` meant to be `rp-red` or a semantic warning state?") — ask Captain in chat or via comms-link
- **§S-146 RCA scope expansion** (e.g., spectator data-viz color preservation list) — Captain auth required before PR
- **Foundational-boundary touch detected** (e.g., migration would touch auth or billing) — HALT and ask
- **debug page Captain re-prioritization** — only Captain can lift the DEFER

### Blockers possible at pickup

- `npm run build` may fail on Windows with EBUSY/file-locks if a prior dev server is running — kill `node` processes first
- `kiosk/.git/` orphan repo can confuse git tooling — operate from racecontrol root, never `cd kiosk` for git ops
- Bono VPS rebuild may be relay-degraded — fall back to SSH via `~/.claude/comms-link.env` per CLAUDE.md
- Server .23 racecontrol cold-start (30-120s) — per [[capability-claim-without-probe]] N=2 spaced probes before claiming "deployed"

---

## §9 — Live evidence appendix (this session)

**Captured 2026-05-15T12:21Z:**

```
GET http://192.168.31.23:3300/kiosk/build-info.json →
  {
    "git_commit": "fd59cf4b-dirty",
    "git_sha_full": "fd59cf4baba8aa5881e26b02904c890703a0e453",
    "dirty": true,
    "build_time_utc": "2026-05-14T09:25:15.255Z",
    "build_time_ist": "2026-05-14T14:55:15.255+05:30"
  }

GET http://192.168.31.23:3300/kiosk/staff →
  HTTP/1.1 200 OK
  x-nextjs-prerender: 1
  x-nextjs-cache: HIT
  Content-Length: 9297

chrome-devtools screenshot: C:\Users\bono\kiosk-staff-deployed-20260515.png
  Rendered content: "RACINGPOINT / STAFF TERMINAL / Tap to Sign In /
                     Staff PIN required / ← Customer Login"
  Brand: Orbitron font ✓ · Racing Red icon ✓ · Asphalt Black bg ✓

cd racecontrol && git log fd59cf4b..1295184d -- kiosk/ → (empty)
  Conclusion: deployed kiosk source = HEAD kiosk source
```

**NOT tested in this session:**
- Tap-to-Sign-In end-to-end flow (PIN → JWT → staff panel render)
- Any kiosk page beyond `/kiosk/staff` (no screenshot baseline for fleet/spectator/blanking/shutdown/control/register/preview-*/debug)
- Bono VPS cloud kiosk parity (deployed there? same commit? — not probed)
- Pod-side kiosk (pods host their own kiosk surface? — not probed)
- `npm run build` from clean HEAD (chunk-SHA comparison vs deployed) — proposed in chat but not executed
- SWAPLOG kiosk deploy provenance (only racecontrol entries scanned — kiosk deploy provenance not located in this session)

These NOT-tested items are pickup-eligible for the next executor or explicit Captain ask.

---

## §10 — Revision log

| Date | Author | Change |
|------|--------|--------|
| 2026-05-15T12:26Z | james@racingpoint.in (Opus 4.7 1M) | INITIAL draft authored on Captain "Producer handoff for the V-theme gap" |

**Stale-at:** 2026-06-15 (30d). After this date, re-run §1.2 survey before relying on counts; verify §2 canonical paths haven't moved.

---

**End of handoff.**
