# Architecture Patterns: Shared Component System — Racing Dashboard UI Redesign

**Domain:** Racing venue management — dual Next.js apps (web dashboard + kiosk terminal)
**Researched:** 2026-03-30
**Confidence:** HIGH — based on direct code inspection of both apps, globals.css, layout files, and all component files

---

## Current State Audit

### Web App (`web/`)

- **Serves at:** `:3200` (dashboard) and `:3201` (admin)
- **Components (13):** AiChatPanel, AiDebugPanel, AuthGate, BillingStartModal, ChunkErrorRecovery,
  CountdownTimer, DashboardLayout, GameLaunchModal, LiveLapFeed, PodCard, Sidebar, StatusBadge, TelemetryBar, TelemetryChart
- **Layout pattern:** Three-column flex — `Sidebar` (w-56) + `main` (flex-1) + `AiChatPanel`
- **Fonts loaded:** Montserrat only (weights 300–700)
- **Token source:** `globals.css` via `@theme inline` block — 6 color tokens, 2 font tokens
- **Auth:** `AuthGate` wrapper at root layout level

### Kiosk App (`kiosk/`)

- **Serves at:** `:3300` with `basePath: /kiosk`
- **Components (24):** AssistanceAlert, CafeMenuPanel, DeployPanel, DriverRegistration, ErrorBoundary,
  F1Speedometer, GameCatalogLoader, GameLaunchRequestBanner, GamePickerPanel, KioskHeader,
  KioskPodCard, LiveLapTicker, LiveSessionPanel, LiveTelemetry, PinRedeemScreen, PodKioskView,
  PricingDisplay, ScarcityBanner, SessionTimer, SetupWizard, SidePanel, StaffLoginScreen, Toast,
  WalletTopup, WalletTopupPanel
- **Layout pattern:** Full-screen, `overflow: hidden`, `user-select: none` — pure kiosk mode
- **Fonts loaded:** Montserrat + Orbitron (`--font-display`) + JetBrains Mono (`--font-mono-jb`)
- **Token source:** `globals.css` via `@theme inline` — 10 color tokens, 3 font tokens
- **Auth:** `StaffLoginScreen` is a component-level gate, NOT at root layout

### Shared Infrastructure (Already Exists)

- **`packages/shared-types/` (`@racingpoint/types`):** Core domain types — Pod, BillingSession, Driver,
  PricingTier, GameState, FleetHealth, WS message shapes. Both apps already import from here.
  Kiosk `lib/types.ts` re-exports and extends it. Web `lib/api.ts` imports directly.
- **NO shared component package exists.** Every UI component is app-local.

---

## Token Divergence — The Root Problem

Both `globals.css` files define the same tokens independently. They have already drifted:

| Token | `web/globals.css` | `kiosk/globals.css` | Status |
|-------|-------------------|---------------------|--------|
| `--color-rp-red` | `#E10600` | `#E10600` | In sync |
| `--color-rp-red-light` | `#FF1A1A` | missing | Web-only name |
| `--color-rp-red-hover` | missing | `#FF1A1A` | Kiosk-only name |
| `--color-rp-black` | `#1A1A1A` | `#1A1A1A` | In sync |
| `--color-rp-grey` | `#5A5A5A` | `#5A5A5A` | In sync |
| `--color-rp-card` | `#222222` | `#222222` | In sync |
| `--color-rp-border` | `#333333` | `#333333` | In sync |
| `--color-rp-surface` | MISSING | `#2A2A2A` | Kiosk-only |
| `--color-rp-purple` | MISSING | `#a855f7` | Kiosk-only |
| `--color-rp-green` | MISSING | `#16a34a` | Kiosk-only |
| `--color-rp-yellow` | MISSING | `#ca8a04` | Kiosk-only |
| `--font-display` | MISSING | `var(--font-display, 'Orbitron')` | Kiosk-only |
| `--font-mono` | `var(--font-geist-mono)` | `var(--font-mono-jb, 'JetBrains Mono')` | Different fonts |

The drift will get worse as the redesign adds new tokens. A shared token file must be created before any component work begins.

---

## Recommended Architecture

### Strategy: Token-Shared, Component-Isolated

Do NOT create a shared component package (`packages/shared-ui/`). The two apps have fundamentally incompatible interaction models:

- **Web:** Mouse-driven, scrollable, data-dense, sidebar navigation, resizable windows
- **Kiosk:** Touch-driven, full-screen, panel-based, no scroll, no text selection, fixed 1920x1080

Attempting to share layout components across these models produces components with so many conditional props that they become harder to maintain than two separate implementations.

The correct sharing boundary:

```
packages/shared-tokens/       NEW — CSS design token source of truth
packages/shared-types/        EXISTING — TypeScript domain types (keep as-is)
web/src/components/           Web-only UI (redesign in place)
kiosk/src/components/         Kiosk-only UI (redesign in place)
```

---

## Component Hierarchy

### Web App — Full Hierarchy

```
RootLayout (layout.tsx)
  ChunkErrorRecovery          KEEP — error boundary for Next.js chunk failures
  AuthGate                    KEEP — session validation wrapper (no visual change)
    DashboardLayout            MODIFY — new chrome, same flex structure
      Sidebar                  REDESIGN — nav hierarchy, brand refresh
      <page slot>              per-page, not in scope here
      AiChatPanel              KEEP — right rail panel, no structural change

  Primitive components (zero dependencies):
    StatusBadge               MODIFY — align token names to unified set
    CountdownTimer            MODIFY — align colors, add variant/size prop

  Data viz (standalone):
    TelemetryBar              KEEP — data viz, token alignment only
    TelemetryChart            KEEP — recharts wrapper, no structural change

  Pod display:
    PodCard                   REDESIGN — new card chrome, same data props
    LiveLapFeed               MODIFY — token alignment

  Modals:
    BillingStartModal         REDESIGN — new modal chrome
    GameLaunchModal           REDESIGN — new modal chrome
    AiDebugPanel              KEEP — debug tool, not customer-facing
```

### Kiosk App — Full Hierarchy

```
RootLayout (layout.tsx)
  ErrorBoundary               KEEP — root error containment
  ToastProvider               KEEP — notification context (wraps children)
    GameCatalogLoader         KEEP — background catalog prefetch

  Per-page:
    KioskHeader               REDESIGN — new brand chrome, same data shape
    <page content>

  Pod display grid:
    KioskPodCard              REDESIGN — new card chrome, preserve 12-state machine
    PodKioskView              REDESIGN — full-pod overlay view

  Session setup pipeline (SetupWizard steps):
    StaffLoginScreen          REDESIGN — new login chrome
    PinRedeemScreen           REDESIGN — new PIN entry chrome
    SetupWizard               REDESIGN — wizard chrome, steps unchanged
      DriverRegistration      REDESIGN — step component
      GamePickerPanel         REDESIGN — step component
      PricingDisplay          REDESIGN — step component
      WalletTopup             REDESIGN — step component

  Live session display:
    SidePanel                 REDESIGN — slide-out panel chrome
    LiveSessionPanel          REDESIGN — active session container
    SessionTimer              MODIFY — align token names
    LiveLapTicker             MODIFY — token alignment
    LiveTelemetry             MODIFY — token alignment
    F1Speedometer             KEEP — specialized gauge widget
    WalletTopupPanel          MODIFY — inline panel variant

  Utility/staff tools:
    CafeMenuPanel             KEEP NOW, redesign later
    DeployPanel               KEEP — staff tool, not customer-facing
    ScarcityBanner            MODIFY — marketing banner chrome
    AssistanceAlert           MODIFY — alert chrome
    GameLaunchRequestBanner   MODIFY — notification banner chrome
    Toast                     REDESIGN — notification chrome
```

---

## Design Token Flow

### New File: `packages/shared-tokens/tokens.css`

This is a plain CSS file — no build step, no package exports. Both apps reference it via relative path in `@import`.

```css
/* Racing Point Design System Tokens — v1.0 */
/* Single source of truth. Both web and kiosk globals.css import this. */

:root {
  /* Brand */
  --rp-red:       #E10600;
  --rp-red-hover: #FF1A1A;    /* canonical name — replaces web's rp-red-light */
  --rp-black:     #1A1A1A;
  --rp-grey:      #5A5A5A;

  /* Surface scale (light → dark) */
  --rp-surface:   #2A2A2A;    /* modals, elevated panels */
  --rp-card:      #222222;    /* card background */
  --rp-border:    #333333;    /* dividers */

  /* Semantic palette */
  --rp-green:     #16a34a;
  --rp-yellow:    #ca8a04;
  --rp-purple:    #a855f7;
}

@theme inline {
  --color-rp-red:       var(--rp-red);
  --color-rp-red-hover: var(--rp-red-hover);
  --color-rp-black:     var(--rp-black);
  --color-rp-grey:      var(--rp-grey);
  --color-rp-surface:   var(--rp-surface);
  --color-rp-card:      var(--rp-card);
  --color-rp-border:    var(--rp-border);
  --color-rp-green:     var(--rp-green);
  --color-rp-yellow:    var(--rp-yellow);
  --color-rp-purple:    var(--rp-purple);
}
```

### Per-App `globals.css` After Import

```css
/* web/src/app/globals.css */
@import "tailwindcss";
@import "../../../packages/shared-tokens/tokens.css";

@theme inline {
  /* Web additions — not in shared tokens */
  --font-sans: 'Montserrat', sans-serif;
  --font-mono: var(--font-geist-mono);
}

body {
  background: var(--rp-black);
  color: #FFFFFF;
  font-family: 'Montserrat', sans-serif;
}
/* web-specific scrollbar, animation classes */
```

```css
/* kiosk/src/app/globals.css */
@import "tailwindcss";
@import "../../../packages/shared-tokens/tokens.css";

@theme inline {
  /* Kiosk additions — three font families */
  --font-sans:    'Montserrat', sans-serif;
  --font-display: var(--font-display, 'Orbitron', sans-serif);
  --font-mono:    var(--font-mono-jb, 'JetBrains Mono', monospace);
}

body {
  background: var(--rp-black);
  color: #FFFFFF;
  overflow: hidden;       /* kiosk-specific: no scroll */
  user-select: none;      /* kiosk-specific: no text selection */
}
/* kiosk-specific animations: pulse-dot, red-glow, slideUp */
```

### Token Flow Diagram

```
packages/shared-tokens/tokens.css
         |
         ├── web/src/app/globals.css
         │         └── Tailwind v4 generates:
         │             bg-rp-red, bg-rp-card, text-rp-grey, border-rp-border ...
         │             bg-rp-surface (newly available in web)
         │
         └── kiosk/src/app/globals.css
                   └── Tailwind v4 generates:
                       bg-rp-red, bg-rp-card, text-rp-grey, border-rp-border ...
                       (identical utility classes for shared tokens)
```

Both apps generate identical Tailwind utilities for all shared tokens. App-specific font tokens (`--font-display` in kiosk only) do not appear in web's generated CSS.

---

## Shared vs App-Specific Boundaries

### Shared — `packages/shared-types/` (existing, keep as-is)

All domain types derived from Rust structs: `Pod`, `BillingSession`, `Driver`, `PricingTier`,
`GameState`, `BillingSessionStatus`, `PodFleetStatus`, `FleetHealthResponse`, WS message shapes.

**Rule:** If the data comes from the Rust backend, it belongs in `@racingpoint/types`.

### Shared — `packages/shared-tokens/` (new)

All brand color tokens. No component logic, no TypeScript.

**Rule:** If it's a CSS custom property that must be identical in both apps, it belongs here.

### App-specific: Web only

| Component | Reason for web-only |
|-----------|---------------------|
| `Sidebar` | Web navigation paradigm — does not exist in kiosk |
| `DashboardLayout` | Web chrome (sidebar + main + panel) |
| `AuthGate` | Web auth wrapper — kiosk uses component-level auth |
| `AiChatPanel`, `AiDebugPanel` | Staff debug tools with mouse hover interactions |
| `BillingStartModal`, `GameLaunchModal` | Mouse-driven modal dialogs |
| `TelemetryChart` | Recharts dependency (not in kiosk package.json) |
| `ChunkErrorRecovery` | Web SPA chunk failure recovery |

### App-specific: Kiosk only

| Component | Reason for kiosk-only |
|-----------|----------------------|
| `KioskHeader` | Touch-optimized top bar with IST clock, pod counts |
| `SetupWizard` + steps | Multi-step kiosk session flow |
| `StaffLoginScreen`, `PinRedeemScreen` | Kiosk auth flows (no browser auth) |
| `SidePanel` | Slide-out panel for touch screens |
| `F1Speedometer` | Specialized real-time telemetry gauge |
| `Toast` / `ToastProvider` | Kiosk notification system |
| `GameCatalogLoader` | Background AC catalog prefetch |
| `ScarcityBanner`, `AssistanceAlert`, `GameLaunchRequestBanner` | Kiosk-specific UX patterns |

### Conceptually Overlapping — Keep Separate

These components exist in both apps solving the same conceptual problem but with different constraints. Keep them separate. Align via shared types and tokens, not shared code.

| Concept | Web component | Kiosk component | Why separate |
|---------|--------------|----------------|--------------|
| Pod status card | `PodCard` | `KioskPodCard` | Web: dense data, hover states. Kiosk: large touch target, 12-state machine, glow animation |
| Session countdown | `CountdownTimer` | `SessionTimer` | Web: compact bar with 3-state color. Kiosk: pause-state aware, local interpolation between WS ticks |
| Status indicator | `StatusBadge` | Inline in `KioskPodCard` | Web: pill badge. Kiosk: colored dot with glow |
| Live lap stream | `LiveLapFeed` | `LiveLapTicker` | Web: scrollable table. Kiosk: animated ticker |

**Alignment rule across both:** Status string values must be identical (they come from `@racingpoint/types`). Color semantics must match: red = active/in_session, green = idle/available, grey = offline. Timer format must match: `MM:SS`. These behavioral alignments are enforced by shared types, not shared components.

---

## Data Flow

```
Rust Backend (:8080)
    |
    ├── REST API: /api/v1/...
    │       ├── web/src/lib/api.ts    ← typed fetch wrappers
    │       └── kiosk/src/lib/api.ts  ← typed fetch wrappers
    │
    └── WebSocket: /ws
            ├── web: socket.io-client (dep in web/package.json)
            │         → page-level hooks → component props
            └── kiosk: native WebSocket (no socket.io dep)
                      → React state → component props

@racingpoint/types (packages/shared-types/)
    └── Both apps import domain types
        (Pod, BillingSession, Driver, GameState, WS message shapes)

packages/shared-tokens/tokens.css  [NEW]
    └── Both globals.css import this
        Tailwind v4 generates identical utility classes in both builds
```

### WebSocket Asymmetry — Keep It

Web uses `socket.io-client`. Kiosk uses native WebSocket. This is intentional — kiosk needs minimal bundle size. Do not unify to one WS library during this redesign.

---

## Build Order for Maximum Reuse

### Phase 0 — Token Foundation (prerequisite for all phases, do first)

No component work. Pure CSS and naming cleanup.

1. Create `packages/shared-tokens/tokens.css` with unified token set (all 10 color tokens)
2. Update `web/src/app/globals.css`: add `@import` of shared tokens, remove inline duplicate token definitions, rename `rp-red-light` → `rp-red-hover` throughout web codebase
3. Update `kiosk/src/app/globals.css`: add `@import` of shared tokens, remove inline duplicate token definitions, keep kiosk-specific font tokens and animations
4. Verify both apps build cleanly: `cd web && npm run build` and `cd kiosk && npm run build`
5. Verify Tailwind generates `bg-rp-surface` in web (was missing), `bg-rp-red-hover` replaces `bg-rp-red-light`

**Output:** Unified token namespace. Zero component changes. Zero runtime risk.

### Phase 1 — Web Primitive Redesign (web-only, no kiosk dependency)

Build primitives before composites. Order within web:

1. `StatusBadge` — pure render, zero deps. Redesign badge chrome. Rename `rp-red-light` uses inside.
2. `CountdownTimer` — pure render, zero deps. Redesign with new token names.
3. `TelemetryBar`, `TelemetryChart` — token alignment only, no structural change.
4. `Sidebar` — redesign chrome. Depends on: Next.js `Link` and `usePathname` only.
5. `DashboardLayout` — redesign shell. Depends on: `Sidebar` (must come after step 4).
6. `PodCard` — redesign card chrome. Depends on: `StatusBadge`, `CountdownTimer` (must come after steps 1 and 2).
7. `LiveLapFeed` — token alignment. Depends on: `StatusBadge`.
8. `BillingStartModal`, `GameLaunchModal` — new modal chrome. Depends on: `StatusBadge`.

### Phase 2 — Kiosk Primitive Redesign (kiosk-only, no web dependency)

Build primitives before composites. Order within kiosk:

1. `Toast` / `ToastProvider` — context provider. Everything else can toast. Zero component deps.
2. `SessionTimer` — pure render, zero deps. Align token names.
3. `KioskHeader` — depends on: `usePathname`, pod data only (no component deps).
4. `KioskPodCard` — redesign card chrome. Depends on: `SessionTimer` (step 2 first).
5. `PodKioskView` — redesign overlay. Depends on: `KioskPodCard`, `SessionTimer`.
6. `SidePanel` — slide-out shell. Depends on: `Toast`.
7. SetupWizard pipeline (in wizard flow order): `StaffLoginScreen` → `PinRedeemScreen` → `SetupWizard` → step components (`DriverRegistration`, `GamePickerPanel`, `PricingDisplay`, `WalletTopup`). Each step is independent of other steps.
8. Live session stack: `LiveSessionPanel` → `LiveLapTicker` → `LiveTelemetry`.
9. Staff tools: `CafeMenuPanel`, `DeployPanel` — defer to last.

### Phase 3 — Cross-App Consistency Audit (after both phases complete)

1. Verify status color semantics match across both apps (same status string → same semantic color meaning)
2. Verify timer format strings match (both use `MM:SS`)
3. Verify driving state display labels match ("Driving", "Paused", "No Device")
4. Verify Tailwind class names for shared tokens resolve identically in both builds
5. Run `grep -r "rp-red-light" web/ kiosk/` — must return zero results (old name fully removed)

---

## Integration Points

### 1. Breaking: Token Rename `rp-red-light` → `rp-red-hover`

Web currently uses `rp-red-light` in component class strings. Kiosk already uses `rp-red-hover` for the same value. The unified token picks `rp-red-hover` (more semantic).

**Action required before Phase 1:** `grep -rn "rp-red-light" web/src/` and update every hit.
Components affected: likely `PodCard.tsx`, `StatusBadge.tsx`, and any page-level class strings.

### 2. `rp-surface` Now Available in Web

Web was missing `--color-rp-surface: #2A2A2A`. After Phase 0, web can use `bg-rp-surface` for modal backgrounds and elevated panels. Redesigned modals (`BillingStartModal`, `GameLaunchModal`) should use `bg-rp-surface` instead of `bg-rp-card` to create visual hierarchy.

### 3. Font Variable Asymmetry — Keep Asymmetric

Web uses `--font-mono: var(--font-geist-mono)`. Kiosk uses `--font-mono: var(--font-mono-jb)`. These are different fonts. The shared token file does NOT define `--font-mono` — each app defines it in its own `@theme inline` block after the import. This is correct: fonts are app-specific concerns.

### 4. Tailwind v4 `@import` Order

The `@import "tailwindcss"` directive must come before `@import "../../../packages/shared-tokens/tokens.css"`. Tailwind v4 processes `@theme inline` blocks in order — the shared tokens file uses `@theme inline` so it must come after the `tailwindcss` import. Verify the PostCSS chain resolves the relative path correctly from each app's directory.

If relative path resolution fails: add a TypeScript `paths` alias or use a symlink. Do NOT copy the token file into both apps.

### 5. Kiosk `overflow: hidden` Constraint

Kiosk body is `overflow: hidden; user-select: none`. Any component designed for or compared to the web app must account for this. Do not use `overflow-auto`, `overflow-y-scroll`, or `overflow-x-scroll` at the panel level in kiosk. Use paginated views, carousels, or expandable sections for content that exceeds the viewport.

### 6. Kiosk `basePath: /kiosk`

All kiosk internal links are prefixed with `/kiosk` automatically by Next.js when `basePath` is set. Component code must never hardcode `/kiosk/...` paths — use root-relative paths and let basePath handle the prefix. This applies especially to `Link` hrefs and any `router.push()` calls in redesigned components.

### 7. Font Loading in Kiosk Layout

Kiosk loads three font families: Montserrat, Orbitron, JetBrains Mono. These are loaded in `layout.tsx` via `next/font/google` and injected as CSS variables (`--font-montserrat`, `--font-display`, `--font-mono-jb`). Any redesigned kiosk component using `font-display` class (Orbitron) or `font-mono` class (JetBrains Mono) depends on these being loaded at root layout. Do not move font loading out of `kiosk/src/app/layout.tsx`.

---

## New vs Modified Summary

| Component | App | Action | Notes |
|-----------|-----|--------|-------|
| `packages/shared-tokens/tokens.css` | shared | NEW | Phase 0 prerequisite |
| `web/globals.css` | web | MODIFY | Import shared tokens, remove duplicates |
| `kiosk/globals.css` | kiosk | MODIFY | Import shared tokens, remove duplicates |
| `StatusBadge` | web | MODIFY | Token name update + design refresh |
| `CountdownTimer` | web | MODIFY | Token name update + design refresh |
| `Sidebar` | web | REDESIGN | New nav chrome |
| `DashboardLayout` | web | MODIFY | New shell chrome |
| `PodCard` | web | REDESIGN | New card chrome, same data props |
| `LiveLapFeed` | web | MODIFY | Token alignment |
| `BillingStartModal` | web | REDESIGN | New modal chrome |
| `GameLaunchModal` | web | REDESIGN | New modal chrome |
| `TelemetryBar`, `TelemetryChart` | web | KEEP | Token alignment only |
| `AuthGate`, `ChunkErrorRecovery` | web | KEEP | No change |
| `AiChatPanel`, `AiDebugPanel` | web | KEEP | No redesign needed |
| `Toast` / `ToastProvider` | kiosk | REDESIGN | New notification chrome |
| `SessionTimer` | kiosk | MODIFY | Token alignment |
| `KioskHeader` | kiosk | REDESIGN | New brand chrome |
| `KioskPodCard` | kiosk | REDESIGN | New card chrome, preserve state machine |
| `PodKioskView` | kiosk | REDESIGN | New overlay chrome |
| `SidePanel` | kiosk | REDESIGN | New slide-out chrome |
| `LiveSessionPanel` | kiosk | REDESIGN | New container chrome |
| `StaffLoginScreen`, `PinRedeemScreen` | kiosk | REDESIGN | New auth chrome |
| `SetupWizard` + step components | kiosk | REDESIGN | Wizard chrome, steps preserved |
| `LiveLapTicker`, `LiveTelemetry` | kiosk | MODIFY | Token alignment |
| `F1Speedometer` | kiosk | KEEP | Specialized widget |
| `CafeMenuPanel`, `DeployPanel` | kiosk | KEEP NOW | Redesign in later milestone |
| `GameCatalogLoader` | kiosk | KEEP | Background loader, no visual |
| `ErrorBoundary` | kiosk | KEEP | Error boundary, no visual |
| `ScarcityBanner`, `AssistanceAlert`, `GameLaunchRequestBanner` | kiosk | MODIFY | Banner/alert chrome |

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Shared Component Package

**What:** Creating `packages/shared-ui/` with shared `PodCard`, layout primitives, etc.
**Why bad:** Web and kiosk have incompatible interaction models. A shared `PodCard` would require conditional props for mouse hover vs touch target sizes, scrollable vs fixed overflow, glow animations vs compact display. The kiosk `KioskPodCard` has a 12-state session machine — sharing it with web would pollute the web component with kiosk-only states.
**Instead:** Share tokens and types. Keep components app-local.

### Anti-Pattern 2: Copying Token Values Instead of Importing

**What:** Keeping duplicate `@theme inline` blocks in both `globals.css` with the same hex values.
**Why bad:** Already caused `rp-red-light` vs `rp-red-hover` naming divergence. Any future brand update requires editing two files and risks drift.
**Instead:** One source of truth in `packages/shared-tokens/tokens.css`, imported by both.

### Anti-Pattern 3: Unifying WebSocket Libraries

**What:** Making kiosk use `socket.io-client` to match web.
**Why bad:** Kiosk runs on fixed venue hardware with a constrained bundle. socket.io adds unnecessary overhead. Both apps talk to the same server endpoints — the client library is an implementation detail.
**Instead:** Accept the asymmetry. Both work correctly with their current WS implementations.

### Anti-Pattern 4: Adding Scroll to Kiosk Components

**What:** Using `overflow-y-auto` inside a kiosk panel to handle content overflow.
**Why bad:** Kiosk body has `overflow: hidden`. Touch screens at the venue do not have scroll wheels. Scroll-locking gestures for inner panels conflict with the kiosk touch model.
**Instead:** Use paginated views, expandable sections, or fixed-height grid layouts.

### Anti-Pattern 5: Hardcoding `/kiosk` in Component Hrefs

**What:** Writing `<Link href="/kiosk/staff">` inside a kiosk component.
**Why bad:** Next.js `basePath: /kiosk` adds the prefix automatically. Double-prefixing causes 404s.
**Instead:** Always write root-relative paths: `<Link href="/staff">`.

---

## Sources

- Direct inspection: `web/src/app/globals.css`, `kiosk/src/app/globals.css` — token divergence confirmed (HIGH confidence)
- Direct inspection: `web/src/components/PodCard.tsx`, `kiosk/src/components/KioskPodCard.tsx` — confirmed incompatible state models (HIGH confidence)
- Direct inspection: `web/src/components/CountdownTimer.tsx`, `kiosk/src/components/SessionTimer.tsx` — same concept, different implementation requirements (HIGH confidence)
- Direct inspection: `web/src/components/StatusBadge.tsx` — 30+ status mappings shared via type system only (HIGH confidence)
- Direct inspection: `kiosk/src/components/KioskHeader.tsx` — IST clock, pod counts, touch nav (HIGH confidence)
- Direct inspection: `packages/shared-types/package.json` and kiosk `lib/types.ts` — existing shared type infrastructure confirmed (HIGH confidence)
- Direct inspection: `web/package.json`, `kiosk/package.json` — socket.io asymmetry confirmed, no shared-ui package (HIGH confidence)
- Direct inspection: `web/next.config.ts`, `kiosk/next.config.ts` — outputFileTracingRoot, basePath settings (HIGH confidence)
- Tailwind v4 `@import "tailwindcss"` + `@theme inline` pattern confirmed in both globals.css (HIGH confidence)
