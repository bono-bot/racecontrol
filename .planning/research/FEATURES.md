# Feature Landscape: Racing Point UI Redesign

**Domain:** Premium motorsport simulator venue management system
**Researched:** 2026-03-30
**Scope:** Design features only — what to ADD or UPGRADE visually and UX-wise. Existing functional logic is NOT in scope.

---

## Existing System Inventory

The system has 25+ pages across two Next.js apps. These already exist and work. The redesign adds premium design features ON TOP, not instead of.

**Web Dashboard (staff-facing, :3200)**
Pages: Live Overview, Pods, Games, Telemetry, AC LAN Race, AC Results, Sessions, Drivers, Leaderboards, Events, Billing, Pricing, History, Bookings, AI Insights, Cameras, Playback, Cafe Menu, Settings, Feature Flags, OTA Releases, Presenter View, HR, Analytics, Maintenance

**Kiosk App (customer-facing, :3300)**
Pages: Landing/Pod Grid, Booking, PIN Redeem, Staff Login, Spectator, Pod Detail, Fleet Display, Settings, Control

**Current design baseline:**
- Dark theme using `rp-black`, `rp-card`, `rp-border` palette with `#E10600` red accent
- Sidebar navigation with emoji icons and plain text labels
- PodCard: border-color state changes (emerald idle, red active, yellow pending)
- StatusBadge: colored dot + label pill (pulsing on active states)
- No page-level visual hierarchy beyond `text-2xl font-bold text-white` headings
- Kiosk ActivePodCard: 2-column telemetry grid with monospace numbers

---

## Table Stakes

Features that staff or customers will immediately notice are missing. Absence makes the product feel unfinished relative to comparable premium venues.

### Staff Dashboard

| Feature | Why Expected | Complexity | Upgrade Path |
|---------|--------------|------------|--------------|
| SVG icon set in sidebar | Emoji icons read as low-effort at premium price points; Lucide or Heroicons are standard in 2025 dark dashboards | Low | Replace emoji strings in `Sidebar.tsx` nav array with `<svg>` or Lucide icon component imports |
| Left-side accent bar on active nav item | Currently only `border-r-2` right border — standard pattern is `border-l-4` left bar with subtle full-width background fill | Low | One-line Tailwind change in `Sidebar.tsx` active class |
| Page-level stat summary row | Every ops dashboard has a 3-5 stat header bar (e.g. "6 active / 2 idle / Rs.4,200 revenue today") before main content | Medium | Shared `StatsBar` component; data from `/api/v1/fleet/health` + billing summary endpoint |
| Skeleton loading states | "Loading pods..." plain text is jarring; skeleton shimmer cards are expected | Medium | `PodCard` skeleton variant; reusable `<Skeleton>` component used across all data-fetching pages |
| Empty state components | Currently `<p>No pods registered</p>` — should be icon + headline + action hint | Low | Consistent `<EmptyState icon label hint />` component applied across all pages |
| Toast / notification system | No feedback on actions (cancel token, stop session, launch game) — just silent state updates | Medium | Global `useToast` hook + top-right toast stack; renders in `DashboardLayout` |
| Breadcrumb navigation | Deep pages (billing/history, billing/pricing) have only a back arrow | Low | `<Breadcrumb>` component in `DashboardLayout.tsx` for sub-pages |
| Persistent WS connection indicator | Hidden in individual pages; should be a persistent status dot in the sidebar footer visible at all times | Low | Move WS status from per-page components to `Sidebar.tsx` footer |

### Kiosk

| Feature | Why Expected | Complexity | Upgrade Path |
|---------|--------------|------------|--------------|
| Pricing displayed on idle pod cards | Customers need to see cost before tapping; currently only shows "Tap to Enter PIN" with no context | Low | Add tier pricing (minutes + price) from `/api/v1/billing/pricing` to idle card footer |
| Session remaining time visible on active pod | Active pod shows elapsed time but not remaining time; customers glancing at the floor screen want to see how long the driver has left | Medium | Use `billingTimers.remaining_seconds` already in `BillingSession` type in `ActivePodCard` |
| "Almost done" visual warning | No alert when session has < 5 min remaining — industry standard is red countdown pulse | Low | Threshold check in `ActivePodCard` timer logic; add `animate-pulse text-rp-red` class when `remaining_seconds < 300` |
| Touch press feedback animation | Pod card buttons use `transition-colors` only; physical kiosk touch on large displays needs explicit press state so customers know they tapped | Low | Add `active:scale-[0.97]` transform to idle pod card button |
| Offline pod count in header | Header shows Available and Racing counts but not how many pods are offline — staff need this | Low | Add third count pill using `pod.status === 'offline'` filter in kiosk header |

---

## Differentiators

Features that set Racing Point apart from generic management software. Not expected, but create "premium motorsport venue" perception when present.

### Staff Dashboard

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| F1 Timing Tower layout on Live Overview | The defining motorsport UI pattern — one horizontal row per pod with position number, driver name, current lap time, best lap time, and sector-colored cells; customers and staff recognize it immediately from F1 broadcasts | High | New `TimingTower` component; data already flows from billing WS + telemetry WS; replaces or augments current pod card grid on the overview page |
| F1 color coding on sector time cells | F1-native system: purple = session best lap, green = personal best, yellow = slower than personal best; apply to lap time display in telemetry and leaderboard pages | Medium | Color logic is straightforward; apply to existing `TelemetryPage` sector fields and `Leaderboards` page cells; no backend change needed |
| Animated countdown arc on active PodCard | Visual countdown as a circular SVG progress arc around the pod number, depleting in red as time runs out — makes remaining time readable at a glance from across the room | Medium | SVG `<circle>` with `stroke-dashoffset` animation; data from `billing.remaining_seconds` already available |
| Fleet health heatmap strip in sidebar | 8 colored cells at sidebar footer showing pod status (green/red/grey) — gives staff instant fleet awareness without leaving any page | Low | 8 colored dots in `Sidebar.tsx` footer; data from fleet health WebSocket already connected on the dashboard |
| Driver rating with trend indicator | Show driver skill tier (Beginner / Intermediate / Pro / Elite) with +/- delta arrow since last session — gamification hook that encourages return visits | Medium | Data exists in `driver_ratings` table shipped in v28.0; needs a `<DriverTierBadge>` component in Drivers and Sessions pages |
| Achievement overlay on track record broken | When a driver sets a track record, a celebratory banner overlay fires — current WS already emits `RecordBrokenEvent` but there is no UI response | Medium | Use existing `RecordBrokenEvent` type already imported in leaderboard page; add a `<RecordBanner>` overlay with a glow burst or pulsing border animation |
| Revenue sparkline in billing header | Small inline SVG chart of today's revenue by hour — instant ops awareness without opening the analytics page | Medium | Needs chart rendering (inline SVG path or Recharts); billing history endpoint already exists |
| Command palette for rapid ops | Staff can assign pods, end sessions, and restart games from a keyboard shortcut without navigating away — reduces multi-step workflows to seconds | High | Global `Cmd+K` command palette overlay wrapping existing API calls; high complexity but very high staff productivity payoff |

### Kiosk

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Leaderboard ticker on idle screen | When all pods are idle or between customer interactions, cycle through top 5 lap times per sim — creates competitive atmosphere and motivates bookings | Medium | Idle-state overlay on kiosk landing page; data from existing leaderboard API; show when `activeCount === 0` or after 30s inactivity |
| Driver personal best shown on PIN success | After PIN validated, show the driver's personal best for the sim type they are about to race — sets competitive context before sitting down | Low | Add to existing `PinModal` success step; requires one extra call to `/api/v1/leaderboards?driver_id=...` |
| Sim game logo / track art badge on active pod | Replace the text abbreviation ("AC", "F1") with a game logo image badge — visual recognition for spectators who can read it from a distance | Medium | Static image map keyed by `sim_type`; game logo assets (AC, F1, iRacing, LMU, Forza) must be sourced and placed in `public/` |
| Ambient race-mode background when venue is full | When the majority of pods are active, a subtle CSS animation (slow horizontal speed lines, or pulsing red glow on the header border) gives the room an "in-race" atmosphere on the floor screen | Medium | Pure CSS animation triggered by `activeCount > idleCount`; no performance cost; add/remove a CSS class on the root container |

---

## Anti-Features

Features to explicitly NOT build in this redesign.

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Light mode toggle | Racing venues operate in controlled dim lighting; a light mode would wash out screens and break the motorsport aesthetic at Rs.700/session price points | Lock to dark theme; brand palette only |
| Drag-and-drop pod reordering | Pods have fixed physical positions (Pod 1 = physical rig 1); visual reordering creates dangerous staff confusion when assigning customers | Keep pod `number` field as the primary and only sort key |
| Animated route transitions (page slide or fade) | Full-page transitions add 200-400ms perceived latency; in ops context staff need instant response; they also cause layout shift on the sidebar area | Animate WITHIN page components only (skeleton loading to content); instant route switching |
| Notification inbox / bell icon | Too much UI complexity for an ops tool; Uday's goal is automation so he doesn't need to watch a dashboard | Toast for immediate in-session feedback; WhatsApp alerts for async staff notification (already built) |
| User preference panels or theme customization | One venue, one brand, one theme; customization adds maintenance surface with zero operational benefit | Hard-code brand tokens; expose only operationally necessary settings via existing Settings page |
| Customer-visible billing amounts in rupees on kiosk pod cards | Displaying Rs.700 on a public touchscreen is uncomfortable in an Indian venue context and can lead to sticker-shock abandonment | Show session duration (30 min / 60 min) not price; price appears on the booking flow page only where context is appropriate |
| Social sharing or photo upload features | Out of scope for venue management; adds auth, storage, and moderation complexity | Let customers use their phones; the personal-best-on-success-screen serves the same social sharing motivation indirectly |
| 3D animations or WebGL effects | Adds bundle size and reduces performance on the server machine (:3200) which also runs 8 WS connections + billing engine | Use CSS transforms and SVG animations only; stays fast on constrained hardware |

---

## Feature Dependencies

```
SVG icon set               --> no dependencies, independent swap in Sidebar.tsx
StatsBar component         --> fleet health WS + billing summary endpoint
Skeleton loading states    --> no dependencies, pure UI layer
Toast system               --> consumed by all action-based components (cancel token, end session, launch game)
Breadcrumb component       --> DashboardLayout.tsx; parentMap already defined there
TimingTower component      --> billing WS + telemetry WS (both already subscribed in overview page)
F1 color coding            --> existing sector_ms fields in telemetry type (no backend change)
Countdown arc SVG          --> billing.remaining_seconds in BillingSession type (already flows)
Fleet heatmap sidebar      --> fleet health WS (already polled on dashboard; needs sidebar access)
Driver rating badge        --> driver_ratings table (v28.0, data exists)
Achievement overlay        --> RecordBrokenEvent (WS message type already imported in leaderboard page)
Leaderboard kiosk ticker   --> existing /api/v1/leaderboards endpoint
Driver PB on PIN success   --> existing leaderboard API + driver_id from PIN validation response
Sim logo badges            --> static image assets needed; no API dependency
Ambient race animation     --> activeCount derived from pods WS (already computed in kiosk page)
```

---

## Component Upgrade Map

Existing components that need design upgrades. These are additive changes to existing files, not rewrites.

| Component | File | Current State | Target Upgrade |
|-----------|------|---------------|----------------|
| `PodCard` | `web/src/components/PodCard.tsx` | Border-color state + key-value rows | Add countdown arc SVG (active state); add sim logo badge; add driver rating pill |
| `StatusBadge` | `web/src/components/StatusBadge.tsx` | Colored dot + text pill | Already clean — no changes needed |
| `Sidebar` | `web/src/components/Sidebar.tsx` | Emoji + text links | Replace emoji with Lucide icons; left-border active item style; add fleet heatmap strip at footer; add persistent WS status dot |
| `DashboardLayout` | `web/src/components/DashboardLayout.tsx` | Sidebar + main content + AI panel | Add `<Breadcrumb>` for sub-pages; render `<ToastStack>` here |
| Login page | `web/src/app/login/page.tsx` | PIN numpad, functional | Add Racing Point wordmark above numpad; subtle CSS speed-line background animation |
| Pods page | `web/src/app/pods/page.tsx` | Static grid, one-time fetch | Add 5s auto-refresh or convert to WS subscription; add `StatsBar` at top |
| Telemetry page | `web/src/app/telemetry/page.tsx` | Raw numbers, existing chart | Apply F1 color coding (purple/green/yellow) to sector time cells |
| Leaderboards page | `web/src/app/leaderboards/page.tsx` | Table with lap times | Add mini-sector columns with F1 colors; add `<RecordBanner>` overlay for RecordBrokenEvent |
| Kiosk `ActivePodCard` | `kiosk/src/app/page.tsx` (inline component) | 2-col telemetry grid | Add remaining timer row with red pulse when < 5 min; add game logo badge |
| Kiosk idle pod card | `kiosk/src/app/page.tsx` (inline) | Number + "Available" pill | Add pricing from API; add `active:scale-[0.97]` press feedback |
| Kiosk `PinModal` success step | `kiosk/src/app/page.tsx` (inline) | Rig number + driver name | Add driver's personal best for the current sim below rig number |

---

## MVP Build Order

Highest staff-visible and customer-visible impact first. Each phase is independently shippable.

**Phase 1 — Foundation (zero logic changes, pure visual polish):**
1. Lucide SVG icons in sidebar + left-border active item style
2. Empty state component applied across all no-data views
3. Skeleton loading component for pod cards and table rows
4. Toast notification system in DashboardLayout
5. Persistent WS status dot in sidebar footer

**Phase 2 — Kiosk premium feel (small data additions):**
6. Pricing on idle pod cards
7. Remaining time display on active pod cards
8. "Almost done" red pulse at < 5 min remaining
9. Driver personal best shown on PIN success screen
10. Touch press feedback animation on pod card buttons

**Phase 3 — Operator differentiation (higher complexity, highest wow factor):**
11. Fleet heatmap strip in sidebar footer (8 dots, data already flows)
12. F1 color coding on sector times in Telemetry + Leaderboards pages
13. Achievement overlay on track record broken (WS event already exists)
14. F1 Timing Tower layout on Live Overview page

**Defer to later milestone:**
- Command palette (high complexity, low urgency)
- Ambient race-mode background on kiosk
- Sim game logo assets (requires asset sourcing)
- Revenue sparkline (requires chart library decision)
- Countdown arc SVG on PodCard (medium complexity, lower urgency than timing tower)

---

## Confidence Assessment

| Area | Confidence | Reason |
|------|------------|--------|
| Table stakes list | HIGH | Derived directly from reading existing source code; gaps are unambiguous |
| F1 timing tower pattern | HIGH | Verified from f1-dash help docs + F1 sector color system well-documented |
| Kiosk UX patterns | MEDIUM | SRL VMS features list + general kiosk UX research; no direct inspection of competitor kiosk code |
| Component upgrade paths | HIGH | Based on actual file reads; all named files confirmed to exist at stated paths |
| Anti-features | HIGH | Based on domain knowledge + specific operational constraints from CLAUDE.md (Windows, constrained server hardware, fixed pod numbering) |

---

## Sources

- [f1-dash help documentation](https://f1-dash.com/help) — F1 timing tower color system (purple/green/yellow), mini-sector design rationale; MEDIUM confidence (help page content, not code)
- [SRL Venue Management System V5.0](https://www.simracing.co.uk/features.html) — kiosk mode patterns, leaderboard display, phone telemetry feature validation; MEDIUM confidence
- [AVIXA Kiosk UX Checklist](https://xchange.avixa.org/posts/kiosk-ux-ui-design-checklist) — touch target sizing, session duration research
- [Flow Racers — F1 sector color explanation](https://flowracers.com/blog/yellow-sector-in-f1/) — purple/green/yellow semantics; HIGH confidence (authoritative F1 explanation)
- [F1 Chronicle — purple sector meaning](https://f1chronicle.com/what-does-purple-sector-mean-in-f1/) — color coding confirmation; HIGH confidence
- Codebase: `PodCard.tsx`, `StatusBadge.tsx`, `Sidebar.tsx`, `DashboardLayout.tsx`, `login/page.tsx`, `pods/page.tsx`, `telemetry/page.tsx`, `leaderboards/page.tsx`, `kiosk/src/app/page.tsx` — all read directly; HIGH confidence
