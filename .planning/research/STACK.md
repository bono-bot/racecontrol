# Technology Stack — UI Redesign

**Project:** Racing Point eSports — Motorsport UI Redesign
**Scope:** Frontend only (web + kiosk apps). Rust/Axum backend is unchanged.
**Researched:** 2026-03-30 IST
**Overall confidence:** HIGH for version numbers; MEDIUM for shadcn/ui Tailwind v4 integration specifics

---

## Context: Existing Baseline

Three Next.js apps in the monorepo — all on Next.js 16.1.6, React 19.2.3, Tailwind v4:

| App | Port | Role | Key deps already present |
|-----|------|------|--------------------------|
| `web/` | :3200 | Staff ops dashboard | recharts 3.8, socket.io-client 4.8, satori, resvg-wasm |
| `kiosk/` | :3300 | Customer-facing kiosk | vitest 4.1, playwright (e2e) |
| `pwa/` | :3100 | Customer PWA | sonner 2.0, canvas-confetti, html5-qrcode, recharts 3.8 |

**Brand CSS variables already defined** in both `web/src/app/globals.css` and `kiosk/src/app/globals.css`:
- `--rp-red: #E10600`, `--rp-black: #1A1A1A`, `--rp-grey: #5A5A5A`
- `--rp-card: #222222`, `--rp-border: #333333`
- All three apps use `@import "tailwindcss"` (v4 CSS-first syntax — no `tailwind.config.js`)

**Fonts already loaded in kiosk:**
- `--font-sans: 'Montserrat', sans-serif` (body)
- `--font-display: 'Orbitron', sans-serif` (race displays)
- `--font-mono: 'JetBrains Mono', monospace` (numeric displays)

**What is NOT present in any app:** shadcn/ui components, Radix primitives, motion animations, TanStack Table.

---

## Recommended Stack Additions

### 1. shadcn/ui (Copy-Paste Components on Radix Primitives)

| Attribute | Value |
|-----------|-------|
| Package | `npx shadcn@latest` (CLI v4, March 2026) |
| Underlying primitives | `@radix-ui/*` (installed automatically per component) |
| Animation utility | `tw-animate-css` (Tailwind v4 compatible replacement for `tailwindcss-animate`) |
| Style | `new-york` (recommended for existing dark-first projects) |

**Why shadcn/ui and not a component library with its own styling:** shadcn/ui ships source code directly into `src/components/ui/` — components are owned and editable, not imported from a black-box npm package. This is essential for a dark Racing Red theme: every component can be tweaked to use `rp-red`, `rp-card`, `rp-border` without overrides fighting a vendor's defaults.

**Why shadcn/ui is ready for this stack:** The March 2026 CLI v4 release added full Tailwind v4 + React 19 support. It now uses `@theme inline` for CSS variables, which is exactly how the existing `globals.css` already works. The color variable approach is structurally compatible: shadcn uses `--background`, `--foreground`, `--primary` etc. mapped via `@theme inline` — the existing RP variables slot in as the values.

**Tailwind v4 integration point:** shadcn/ui components reference shadcn CSS variables (`--background`, `--card`, `--primary`, `--border`, `--ring`). Map these to existing RP variables in `globals.css`:

```css
@theme inline {
  /* existing RP vars stay as-is */
  --color-rp-red: #E10600;
  --color-rp-black: #1A1A1A;
  --color-rp-grey: #5A5A5A;
  --color-rp-card: #222222;
  --color-rp-border: #333333;

  /* shadcn semantic vars — map to RP brand */
  --color-background: var(--rp-black);
  --color-foreground: #FFFFFF;
  --color-card: var(--rp-card);
  --color-card-foreground: #FFFFFF;
  --color-primary: var(--rp-red);
  --color-primary-foreground: #FFFFFF;
  --color-border: var(--rp-border);
  --color-ring: var(--rp-red);
  --color-muted: #2A2A2A;
  --color-muted-foreground: var(--rp-grey);
}
```

**Critical:** shadcn v4 deprecated `tailwindcss-animate` in favour of `tw-animate-css`. Install `tw-animate-css` as a devDependency and `@import "tw-animate-css"` at the top of `globals.css`, not `tailwindcss-animate`. Do NOT install `tailwindcss-animate` — it uses the old Tailwind plugin system that does not work with Tailwind v4's CSS-first architecture.

**Installation approach:**
```bash
# In web/ and kiosk/ separately (shadcn scaffolds into the current directory)
cd web && npx shadcn@latest init
# Choose: new-york style, no tailwind config (v4 mode), TypeScript yes
# Then add components as needed:
npx shadcn@latest add button badge table dialog select tooltip
```

Components are added individually — only install what's used. Radix peer deps are pulled automatically per component (e.g. `@radix-ui/react-dialog` only when `dialog` is added).

---

### 2. Motion (Micro-Interactions and Transitions)

| Attribute | Value |
|-----------|-------|
| Package | `motion` |
| Version | `^12.0.0` (latest stable as of March 2026: 12.38.x) |
| Import path | `import { motion, AnimatePresence } from "motion/react"` |
| React 19 | Fully supported |

**Why `motion` not `framer-motion`:** Framer Motion was rebranded to `motion` as an independent package in 2025. `framer-motion@12+` is now `motion` on npm. The API is identical — same `<motion.div>`, same `AnimatePresence`, same `useAnimation` hook. Import from `motion/react` instead of `framer-motion`. The package name change is the only breaking change for new installations.

**Why NOT CSS-only animations for all interactions:** Tailwind's `animate-*` utilities and `tw-animate-css` cover entrance/exit animations and simple state transitions well. Motion is specifically justified for: (a) leaderboard row reordering — `AnimatePresence` with `layout` prop handles list items that physically move between positions, which CSS alone cannot do; (b) pod status card transitions — animated between `Available`, `Active`, `Error` states with color + scale changes that need to be interruptible mid-animation; (c) countdown timer — spring physics for the final second. For static hover effects and simple fades, use Tailwind utilities — do not reach for Motion.

**Integration point with Next.js:** Motion components are client-only. Any component using `<motion.div>` must be in a `"use client"` file or imported via dynamic import. This is already the pattern in the codebase — `PodCard.tsx`, `LiveLapFeed.tsx` etc. are all client components. No Server Component issues expected.

**Installation:**
```bash
npm install motion
```

---

### 3. TanStack Table v8 (Dense Data Grids)

| Attribute | Value |
|-----------|-------|
| Package | `@tanstack/react-table` |
| Version | `^8.21.3` (latest stable) |
| React 19 | Supported with caveat (see below) |

**Why TanStack Table and not plain HTML tables or recharts:** The ops dashboard has dense data: leaderboard tables (8+ columns, 100+ rows, sortable), billing session grids, lap time history per pod. TanStack Table is headless — it provides sort/filter/pagination logic with zero opinion on HTML structure, meaning the Racing Red-themed table rows, sticky headers, and alternating row colors are all controlled by Tailwind classes applied to the TanStack-managed cell renderers. This is the right separation: data logic (TanStack) vs visual (Tailwind + shadcn).

**React 19 caveat:** There is a known issue where TanStack Table does not re-render correctly when combined with React 19's new React Compiler auto-memoization (issue #5567). The React Compiler is opt-in and not enabled in any of the three apps currently. Do NOT enable the React Compiler in `next.config.ts` until TanStack Table resolves this — it will cause stale table data bugs that are hard to diagnose.

**Which app:** Install in `web/` only. The kiosk does not show multi-column sortable tables — it shows single-pod views and the leaderboard display. The leaderboard display is a read-only animated list (use Motion's `AnimatePresence` + `layout`, not a data table).

**Installation:**
```bash
# In web/ only
npm install @tanstack/react-table
```

---

### 4. JetBrains Mono (Numeric Displays)

**Status: Already present in kiosk. Must be added to web.**

The kiosk `globals.css` already declares `--font-mono: 'JetBrains Mono', monospace` and the font is referenced in the `@theme inline` block. The `web/` app uses `--font-mono: var(--font-geist-mono)` — Geist Mono — which is a general-purpose monospace, not tuned for numeric display in a racing context.

**Action for web/:** Add JetBrains Mono via `next/font/google`:

```typescript
// web/src/app/layout.tsx
import { JetBrains_Mono } from "next/font/google"

const jetbrainsMono = JetBrains_Mono({
  subsets: ["latin"],
  variable: "--font-jb-mono",
})
// Apply: <html className={jetbrainsMono.variable}>
```

```css
/* web/src/app/globals.css — update @theme inline */
--font-mono: var(--font-jb-mono);
```

JetBrains Mono is available on Google Fonts and is loaded via `next/font/google` with zero external request at runtime (Next.js self-hosts the font). No npm package needed — `next/font/google` is already in the Next.js 16 runtime.

**Why JetBrains Mono over Geist Mono:** JetBrains Mono has tabular numerals by default (`tnum` OpenType feature), which ensures lap times like `1:23.456` and `1:24.001` align vertically in lists. Geist Mono does not guarantee this. For racing leaderboards where every millisecond is visually significant, tabular alignment is a functional requirement, not an aesthetic preference.

---

## What NOT to Add

| Avoid | Reason | What Exists Instead |
|-------|--------|---------------------|
| `tailwindcss-animate` | Incompatible with Tailwind v4 CSS-first architecture — uses old plugin system | `tw-animate-css` (CSS-based, v4 compatible) |
| `framer-motion` (old package name) | Deprecated; same codebase now published as `motion` | `motion@^12` |
| `@radix-ui/*` directly | shadcn/ui installs these automatically per component; manually adding creates version drift | Install via `npx shadcn@latest add [component]` |
| `react-table` (v7) | Superseded by `@tanstack/react-table` v8; different API | `@tanstack/react-table@^8.21.3` |
| `recharts` (additional charts) | Already in `web/` and `pwa/`; TelemetryChart.tsx uses it | Already present — reuse existing Recharts components |
| `sonner` (toasts) | Already in `pwa/`; for `web/` use shadcn's `Toaster` component which wraps Sonner internally | shadcn `toast` component |
| `@fontsource/jetbrains-mono` | npm-based font package is unnecessary when `next/font/google` self-hosts it free | `next/font/google` |
| React Compiler (`experimental.reactCompiler: true`) | Breaks TanStack Table re-renders (issue #5567) | Leave disabled — default Next.js 16 behavior |
| `class-variance-authority` / `clsx` / `tailwind-merge` | shadcn/ui installs these automatically as part of `init`; don't pre-install | Installed by `npx shadcn@latest init` |

---

## Per-App Installation Plan

### web/ (Staff Operations Dashboard)

```bash
cd web

# 1. shadcn init (creates components/ui/, updates globals.css, installs tw-animate-css + radix deps)
npx shadcn@latest init
# Prompts: TypeScript=yes, style=new-york, base-color=neutral (will be overridden by RP vars), no tailwind config

# 2. Add components used in ops dashboard
npx shadcn@latest add button badge table dialog select tooltip dropdown-menu separator tabs sheet

# 3. Animation library
npm install motion

# 4. Data table
npm install @tanstack/react-table

# 5. Font (add to layout.tsx — no npm install needed)
# Edit layout.tsx to import JetBrains_Mono from "next/font/google"
```

### kiosk/ (Customer-Facing Kiosk)

```bash
cd kiosk

# 1. shadcn init (kiosk uses fewer components — dialog, badge, button for wizard flow)
npx shadcn@latest init

# 2. Components for kiosk flows only
npx shadcn@latest add button badge dialog progress

# 3. Animation (leaderboard reorder, pod status transitions, countdown spring)
npm install motion

# TanStack Table: NOT needed in kiosk — leaderboard is an animated list, not a sortable grid
# JetBrains Mono: ALREADY present in globals.css and @theme inline — no action needed
```

### pwa/ (Customer PWA)

No shadcn/ui or TanStack Table needed. The PWA is customer-facing (booking, wallet, leaderboard view) with simpler UI needs. Sonner is already present. If animated transitions are needed, add `motion` only — but evaluate first whether `tw-animate-css` utilities (already available via Tailwind v4) are sufficient before adding motion as a dep.

---

## Version Compatibility Matrix

| Package | Version | Next.js | React | Tailwind | Notes |
|---------|---------|---------|-------|----------|-------|
| `motion` | ^12.38.x | 16.1.6 | 19.2.3 | v4 | Import from `motion/react`. Server Components safe if no `"use client"` required by component. |
| `@tanstack/react-table` | ^8.21.3 | 16.1.6 | 19.2.3 | n/a | Do NOT enable React Compiler — breaks re-renders. |
| `tw-animate-css` | latest | 16.1.6 | 19.2.3 | v4 | CSS `@import`, not a plugin. |
| `shadcn/ui` (components) | CLI v4 (March 2026) | 16.1.6 | 19.2.3 | v4 | `new-york` style, `@theme inline` for variables. |
| `@radix-ui/*` | auto-managed by shadcn | 16.1.6 | 19.2.3 | n/a | Never install directly — shadcn manages versions. |
| JetBrains Mono | via `next/font/google` | 16.1.6 | 19.2.3 | n/a | Already in kiosk; add to web via layout.tsx. |

---

## Tailwind v4 Integration Notes

**The existing apps are already on the correct Tailwind v4 CSS-first architecture.** No migration needed. All three apps use:
- `@import "tailwindcss"` (not `@tailwind base/components/utilities`)
- `@theme inline { ... }` for custom tokens
- No `tailwind.config.js` file

**shadcn/ui will add to globals.css** on `init` — it appends its semantic variable definitions under `@layer base`. Review and merge with existing RP variable definitions to avoid duplicates. The `--color-background`, `--color-foreground`, `--color-primary` etc. added by shadcn should reference the existing `--rp-*` variables rather than redefining their own values.

**tw-animate-css integration:**
```css
/* globals.css — after @import "tailwindcss" */
@import "tw-animate-css";
```
This enables `animate-accordion-down`, `animate-accordion-up` and the utility set used by shadcn components internally.

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| Component system | shadcn/ui (copy-paste, Radix-based) | Mantine, Chakra UI, Ant Design | All impose their own CSS-in-JS or Tailwind v3 assumptions that conflict with v4 CSS-first. shadcn is the only major system with official Tailwind v4 support (March 2026 CLI). |
| Animation | `motion@^12` | CSS transitions only | CSS transitions cannot animate layout changes (list reordering) — a core requirement for the leaderboard. Motion's `layout` prop is the only practical solution for animating from position A to position B when the DOM order changes. |
| Data table | `@tanstack/react-table` | `react-data-grid`, plain `<table>` | `react-data-grid` is opinionated on CSS and conflicts with dark Tailwind themes. Plain `<table>` has no sort/filter/virtual scroll logic. TanStack Table is headless — full styling control. |
| Numeric font | JetBrains Mono | Geist Mono (current in web/) | JetBrains Mono has tabular numerals by default (`tnum`). Geist Mono does not guarantee tabular alignment — staggered lap time digits in leaderboard columns look wrong to racers. |
| Animation utilities (CSS) | `tw-animate-css` | `tailwindcss-animate` | `tailwindcss-animate` uses the v3 plugin API (`plugin()`). This API is removed in Tailwind v4 CSS-first mode. `tw-animate-css` is a pure CSS `@import` with the same utility names. |

---

## Sources

- [shadcn/ui Tailwind v4 docs](https://ui.shadcn.com/docs/tailwind-v4) — integration approach, `@theme inline` CSS variable system — HIGH confidence
- [shadcn/ui Next.js 15 + React 19 docs](https://ui.shadcn.com/docs/react-19) — React 19 compatibility confirmed — HIGH confidence
- [shadcn/ui CLI v4 changelog (March 2026)](https://ui.shadcn.com/docs/changelog/2026-03-cli-v4) — CLI v4 features, Tailwind v4 + React 19 full support — HIGH confidence
- [Motion upgrade guide](https://motion.dev/docs/react-upgrade-guide) — `framer-motion` → `motion` package rename, `motion/react` import — HIGH confidence
- [Motion npm (framer-motion)](https://www.npmjs.com/package/framer-motion) — version 12.38.x as of March 2026 — HIGH confidence
- [TanStack Table npm — 8.21.3](https://cloudsmith.com/navigator/npm/@tanstack/react-table) — current version — MEDIUM confidence
- [TanStack Table React Compiler issue #5567](https://github.com/TanStack/table/issues/5567) — React 19 + React Compiler rendering bug — MEDIUM confidence (GitHub issue, not official statement)
- [tw-animate-css GitHub](https://github.com/Wombosvideo/tw-animate-css) — Tailwind v4 compatible, CSS `@import` pattern — MEDIUM confidence
- [JetBrains Mono on Google Fonts](https://fonts.google.com/specimen/JetBrains+Mono) — available via `next/font/google` — HIGH confidence
- [Next.js font optimization docs](https://nextjs.org/docs/app/getting-started/fonts) — `next/font/google` self-hosting confirmed — HIGH confidence
- Existing codebase: `web/package.json`, `kiosk/package.json`, `pwa/package.json`, `web/src/app/globals.css`, `kiosk/src/app/globals.css` — read directly — HIGH confidence
