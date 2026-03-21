# Codebase Structure

**Analysis Date:** 2026-03-21

## Directory Layout

```
racecontrol/kiosk/
├── .next/                  # Next.js build output (generated, not committed)
├── .planning/
│   └── codebase/          # Codebase analysis documents (this directory)
├── designs/                # UI design assets (Figma exports, mockups)
├── node_modules/          # Dependencies (not committed)
├── public/                # Static assets
│   ├── fonts/            # Custom fonts (Montserrat, Space Grotesk, JetBrains Mono)
│   ├── f1hud/            # F1 telemetry HUD graphics (speed steps, gears, RPM digits, etc.)
│   │   ├── background/
│   │   ├── brake/
│   │   ├── drs/
│   │   ├── gears/
│   │   ├── rpm_digits/
│   │   ├── speed_steps/
│   │   └── throttle/
│   └── game-logos/        # Game abbreviation/brand logos (AC, F1, iRacing, etc.)
├── src/
│   ├── app/              # Next.js App Router routes (pages and layouts)
│   ├── components/       # Reusable React components
│   ├── hooks/           # Custom React hooks
│   ├── lib/             # Utilities, types, API client
│   └── globals.css      # Tailwind + custom CSS
├── .gitignore           # Git ignore rules
├── next.config.ts       # Next.js configuration
├── next-env.d.ts        # Next.js type definitions
├── package.json         # Dependencies and scripts
├── package-lock.json    # Lock file (Node)
├── postcss.config.mjs   # PostCSS config for Tailwind
├── redirect80.py        # Python redirect script (HTTP 80 → HTTPS 8443)
├── tsconfig.json        # TypeScript compiler config
└── tsconfig.tsbuildinfo # TypeScript build info (incremental compilation)
```

## Directory Purposes

**src/app:**
- Purpose: Next.js App Router pages and layouts
- Contains: Route-level TSX components (one per route)
- Key files:
  - `layout.tsx` — root layout with fonts, metadata, error boundary
  - `page.tsx` — `/` landing page (4x2 pod grid, PIN modal)
  - `book/page.tsx` — `/book` booking flow (1308 lines, OTP auth, experience picker)
  - `staff/page.tsx` — `/staff` staff login screen
  - `control/page.tsx` — `/control` staff control panel (pod grid, bulk actions)
  - `pod/[number]/page.tsx` — `/pod/1`, `/pod/2`, etc. (individual pod detail)
  - `fleet/page.tsx` — `/fleet` fleet health dashboard
  - `spectator/page.tsx` — `/spectator` external spectator display
  - `debug/page.tsx` — `/debug` operations debug + incident system
  - `settings/page.tsx` — `/settings` kiosk configuration (staff only)

**src/components:**
- Purpose: Reusable UI components across pages
- Contains: 29 TSX files (interactive elements, panels, modals)
- Key files:
  - **Core Display:**
    - `KioskPodCard.tsx` — individual pod card (idle/active/offline states, telemetry display)
    - `PodKioskView.tsx` — full-featured pod view with controls
    - `KioskHeader.tsx` — top bar (logo, connection status, staff logout)
  - **Authentication & Setup:**
    - `StaffLoginScreen.tsx` — staff 4-digit PIN entry
    - `DriverRegistration.tsx` — new driver form (name, email, phone)
    - `SetupWizard.tsx` — multi-step booking flow component
  - **Session Management:**
    - `LiveSessionPanel.tsx` — active billing session controls (pause, resume, extend)
    - `LiveTelemetry.tsx` — real-time vehicle data display (speed, RPM, brake, gear)
    - `SessionTimer.tsx` — countdown timer for allocated time
    - `LiveLapTicker.tsx` — best/last lap display
    - `F1Speedometer.tsx` — F1 HUD-style speedometer (uses public/f1hud assets)
  - **Game & Experience:**
    - `GamePickerPanel.tsx` — game selection UI (Assetto Corsa, F1, iRacing, LMU)
    - `GameLaunchRequestBanner.tsx` — notification banner for game launch requests
  - **Wallet & Billing:**
    - `WalletTopup.tsx` — single topup transaction form
    - `WalletTopupPanel.tsx` — wallet credit management panel
  - **Staff Tools:**
    - `SidePanel.tsx` — collapsible side panel for staff (driver reg, game picker, wallet)
    - `DeployPanel.tsx` — pod binary deployment UI (download progress, verification)
  - **Utilities:**
    - `ErrorBoundary.tsx` — React error fallback component
    - `AssistanceAlert.tsx` — driver assistance request popup

**src/hooks:**
- Purpose: Custom React hooks for state and side effects
- Contains: 2 TS files
- Key files:
  - `useKioskSocket.ts` — central WebSocket connection + state management
    - Manages: pods, telemetry, billing, game states, auth tokens, assistance requests, deploy states
    - Broadcasts: 20+ event types (pod_list, billing_tick, game_state_changed, etc.)
    - Exports: `connected`, pods Map, telemetry Map, sendCommand() function
  - `useSetupWizard.ts` — booking flow multi-step state machine
    - Manages: current step, driver data, pricing selection, experience, game config
    - Exports: step progress, data accessors, next/prev step functions

**src/lib:**
- Purpose: Utilities, types, and API client
- Contains: 4 TS files
- Key files:
  - `api.ts` — HTTP API client for RaceControl server (395 lines)
    - Pattern: `api.functionName()` → typed Promise
    - Base URL: `NEXT_PUBLIC_API_URL` env var or `${window.location.host}`
    - Endpoints: health, pods, drivers, billing, auth, games, experiences, debug, wallet, kiosk
    - Auth: Bearer token for customer booking, no token for staff/kiosk
    - Error handling: throws on non-200, caller must catch
  - `types.ts` — TypeScript type definitions (462 lines)
    - Pod, BillingSession, TelemetryFrame, GameLaunchInfo, AuthTokenInfo
    - Pricing, Experience, Wallet, Debug types
    - State enums: PodStatus, BillingStatus, GameState, DeployState, etc.
  - `constants.ts` — shared constants (GAMES list, CLASS_COLORS, DIFFICULTY_PRESETS)
  - `gameDisplayInfo.ts` — game display metadata (logos, abbreviations)

**public:**
- Purpose: Static assets served by Next.js
- Contains: Fonts, F1 HUD graphics, game logos
- Usage:
  - Fonts loaded in `layout.tsx` via Google Fonts API
  - F1 HUD images referenced by `F1Speedometer.tsx`
  - Game logos referenced by `GameLogo()` component in `KioskPodCard.tsx`

**Design & Config:**
- `.gitignore` — excludes `node_modules`, `.next`, `.env*`, `*.log`
- `tsconfig.json` — strict mode enabled, path alias `@/*` → `src/*`
- `next.config.ts` — minimal config (Tailwind PostCSS)
- `package.json` — Next.js 16.1.6, React 19.2.3, TypeScript 5.9.3, Tailwind 4

## Key File Locations

**Entry Points:**

| Route | File | Purpose |
|-------|------|---------|
| `/` | `src/app/page.tsx` | Customer landing — pod grid, PIN modal |
| `/book` | `src/app/book/page.tsx` | Booking flow — OTP, driver reg, experience picker |
| `/staff` | (no file; uses `StaffLoginScreen` component) | Staff login — PIN entry |
| `/control` | `src/app/control/page.tsx` | Staff control — pod grid, bulk actions |
| `/pod/[number]` | `src/app/pod/[number]/page.tsx` | Pod detail — telemetry, session controls |
| `/fleet` | `src/app/fleet/page.tsx` | Fleet health — pod status dashboard |
| `/spectator` | `src/app/spectator/page.tsx` | External spectator display |
| `/debug` | `src/app/debug/page.tsx` | Operations debug — incident system |
| `/settings` | `src/app/settings/page.tsx` | Kiosk settings (staff only) |

**Configuration:**

| File | Purpose |
|------|---------|
| `tsconfig.json` | TypeScript strict mode, path aliases |
| `next.config.ts` | Next.js 16 app, Tailwind PostCSS |
| `package.json` | Dependencies, scripts (dev, build, start, lint) |
| `postcss.config.mjs` | Tailwind CSS pipeline |
| `.env.local` | (not in repo) `NEXT_PUBLIC_API_URL`, `NEXT_PUBLIC_WS_URL` |

**Core Logic:**

| File | Purpose |
|------|---------|
| `src/lib/api.ts` | HTTP client — all RaceControl API calls |
| `src/lib/types.ts` | Domain types — Pod, Billing, Game, etc. |
| `src/hooks/useKioskSocket.ts` | WebSocket connection + state sync |
| `src/components/SetupWizard.tsx` | Booking multi-step flow |

**Testing:**

| Pattern | File |
|---------|------|
| (Not found) | No dedicated test files in current structure |
| Test IDs | `data-testid` attributes on landing page: `ws-status`, `pod-grid`, `pod-card-{number}`, `pin-modal`, `book-session-btn` |

## Naming Conventions

**Files:**

- **Pages:** `page.tsx` in route directory (e.g., `src/app/staff/page.tsx`)
- **Components:** PascalCase, descriptive (e.g., `KioskPodCard.tsx`, `SetupWizard.tsx`)
- **Hooks:** `use` prefix, descriptive (e.g., `useKioskSocket.ts`)
- **Utilities:** camelCase (e.g., `api.ts`, `types.ts`, `constants.ts`)

**Directories:**

- **Route dirs:** lowercase, plural or specific (e.g., `book`, `staff`, `control`)
- **Feature dirs:** lowercase, descriptive (e.g., `components`, `hooks`, `lib`)
- **No index files:** Each component in its own file (not index.tsx barrel files)

**Components:**

- **Container (Page) Components:** End in `Page` or live in `app/*/page.tsx`
- **UI Components:** Descriptive names (e.g., `KioskHeader`, `PodKioskView`, `SessionTimer`)
- **State-holding:** Use hook names (e.g., `useSetupWizard`)
- **Props:** Named export or interface next to component

**Functions & Variables:**

- **camelCase:** All functions, variables, hooks
- **UPPERCASE:** Constants (GAMES, CLASS_COLORS, DIFFICULTY_PRESETS)
- **Type names:** PascalCase (Pod, BillingSession, TelemetryFrame)

**Exports:**

- **Named exports:** Utilities, types, constants (e.g., `export const api = {...}`)
- **Default exports:** Page components (e.g., `export default function CustomerLanding() {}`)
- **Component props:** Interface `{ComponentName}Props` next to component

## Where to Add New Code

**New Feature (e.g., Lap Replay):**

1. **Type definitions:** Add to `src/lib/types.ts` (LapReplaySession, LapFrame, etc.)
2. **API client:** Add methods to `src/lib/api.ts` (e.g., `api.getLapReplay(lapId)`)
3. **Hook (if stateful):** Create `src/hooks/useLapReplay.ts` (manage replay state)
4. **Components:**
   - Viewer: `src/components/LapReplayViewer.tsx`
   - Thumbnail: `src/components/LapReplayThumbnail.tsx`
5. **Route (if new page):** Create `src/app/replay/page.tsx`, import components
6. **Tests:** Would go in `src/components/__tests__/LapReplayViewer.test.tsx` (when testing added)

**New Component (e.g., LapComparison):**

1. Create file: `src/components/LapComparison.tsx`
2. Export as default: `export default function LapComparison({ lap1, lap2 }: Props) {}`
3. Import where needed: `import LapComparison from "@/components/LapComparison"`
4. Pass props: include TypeScript interface at top of file

**New Utility (e.g., Lap Time Formatter):**

1. Create file: `src/lib/lapTimeFormatter.ts` (or add to `constants.ts` if simple)
2. Export function: `export function formatLapTime(ms: number): string {}`
3. Import in components: `import { formatLapTime } from "@/lib/lapTimeFormatter"`
4. Use: `formatLapTime(120500)` → "2:00.500"

**New API Endpoint:**

1. Add to `src/lib/api.ts`:
   ```typescript
   myNewEndpoint: (params: Type) =>
     fetchApi<ResponseType>("/path", {
       method: "POST",
       body: JSON.stringify(params),
     }),
   ```
2. Add type to `src/lib/types.ts` (request, response interfaces)
3. Use in component: `const res = await api.myNewEndpoint(data)`

**Global Style Change:**

1. Edit `src/app/globals.css` (Tailwind imports, custom colors)
2. Tailwind config: extend in `tailwind.config.js` (if exists) or `postcss.config.mjs`
3. Racing Point colors: `--rp-red` (#E10600), `--rp-black` (#1A1A1A), `--rp-grey` (#5A5A5A)
4. Fonts: `--font-montserrat`, `--font-display` (Space Grotesk), `--font-mono-jb` (JetBrains)

## Special Directories

**public/f1hud:**
- Purpose: F1 telemetry display graphics
- Generated: No (hand-crafted or exported from design tool)
- Committed: Yes
- Usage: `F1Speedometer.tsx` references images for HUD overlay

**public/game-logos:**
- Purpose: Game brand logos and abbreviations
- Generated: No (curated asset library)
- Committed: Yes
- Usage: `gameDisplayInfo.ts` maps game IDs to logo paths

**designs/:**
- Purpose: UI design mockups and design system documentation
- Generated: Possibly (Figma exports)
- Committed: Yes
- Usage: Reference for UI consistency, component specs

**.next/:**
- Purpose: Next.js build output (pages, static files, webpack bundles)
- Generated: Yes (by `npm run build`)
- Committed: No (excluded in .gitignore)
- Cleaned: Before each build

**node_modules/:**
- Purpose: npm dependencies
- Generated: Yes (by `npm install`)
- Committed: No (excluded in .gitignore)
- Management: package-lock.json tracks versions

## Routing Map

**Public Routes (no auth):**
- `/` — landing page (pod grid, PIN entry)
- `/book` — booking flow (OTP auth, experience selection)
- `/staff` — staff login

**Authenticated Routes (staff):**
- `/control` — control panel (requires sessionStorage auth)
- `/settings` — configuration (staff only)

**Semi-public:**
- `/spectator` — external display (often on TV, no auth)
- `/debug` — operations debug (staff visible, no lock)
- `/fleet` — fleet health (read-only, no lock)

**Dynamic:**
- `/pod/[number]` — individual pod detail (1-8)

## Module Resolution

**Path Alias:** `@/*` → `src/*`

Examples:
- `import { useKioskSocket } from "@/hooks/useKioskSocket"` — resolves to `src/hooks/useKioskSocket.ts`
- `import type { Pod } from "@/lib/types"` — resolves to `src/lib/types.ts`
- `import { api } from "@/lib/api"` — resolves to `src/lib/api.ts`

**No Relative Imports:** Use `@/` alias consistently for clarity and refactoring safety.

---

*Structure analysis: 2026-03-21*
