# Codebase Structure

**Analysis Date:** 2026-03-21

## Directory Layout

```
pwa/
├── src/
│   ├── app/                          # Next.js App Router pages and layouts
│   │   ├── layout.tsx                # Root layout with metadata, fonts, toaster
│   │   ├── globals.css               # Tailwind and Racing Point theme
│   │   ├── page.tsx                  # / (redirect to /dashboard or /login)
│   │   ├── login/                    # Authentication
│   │   │   └── page.tsx              # Phone, OTP, registration form
│   │   ├── register/                 # Registration alternative route
│   │   ├── dashboard/                # Main authenticated area
│   │   │   ├── layout.tsx            # Auth guard, BottomNav persistent nav
│   │   │   └── page.tsx              # Home dashboard with profile, stats, sessions
│   │   ├── book/                     # Booking wizard
│   │   │   ├── page.tsx              # Tier selection, game/track/car picker
│   │   │   ├── active/               # Active booking flow
│   │   │   ├── group/                # Multiplayer booking
│   │   │   └── multiplayer/          # Multiplayer session details
│   │   ├── sessions/                 # Session history and details
│   │   │   ├── page.tsx              # Session list
│   │   │   ├── [id]/                 # Dynamic: single session details
│   │   │   │   ├── page.tsx
│   │   │   │   └── public/           # Public session summary share
│   │   ├── wallet/                   # Credits and topup
│   │   │   ├── page.tsx              # Wallet balance overview
│   │   │   ├── history/              # Transaction history
│   │   │   └── topup/                # Credit topup with Razorpay
│   │   ├── leaderboard/              # Global and track-specific leaderboards
│   │   │   ├── page.tsx              # Track selector
│   │   │   └── public/               # Public leaderboard
│   │   ├── profile/                  # User profile management
│   │   ├── drivers/                  # Driver search and profiles
│   │   │   ├── page.tsx
│   │   │   └── [id]/                 # Single driver profile
│   │   ├── stats/                    # User statistics and trends
│   │   ├── telemetry/                # Live telemetry viewer
│   │   ├── terminal/                 # Terminal commands (admin)
│   │   ├── coaching/                 # Coaching comparison tool
│   │   ├── records/                  # Track records
│   │   ├── passport/                 # Passport/achievement badges
│   │   ├── friends/                  # Friends list and requests
│   │   ├── tournaments/              # Tournament listing and registration
│   │   ├── scan/                     # QR code scanner for check-in
│   │   ├── ai/                       # AI chatbot interface
│   │   └── [More route segments...]
│   ├── components/
│   │   ├── BottomNav.tsx             # Persistent 7-tab navigation
│   │   ├── SessionCard.tsx           # Reusable session summary card
│   │   ├── TelemetryChart.tsx        # Recharts multi-panel telemetry modal
│   │   ├── Confetti.tsx              # Canvas-confetti celebration animation
│   │   └── Toaster.tsx               # Sonner toast notifications
│   └── lib/
│       └── api.ts                    # API client, types, auth helpers (1085 lines)
├── public/
│   ├── manifest.json                 # PWA manifest
│   └── clear.html                    # Service worker clear utility
├── package.json
├── tsconfig.json
├── next.config.ts                    # `output: standalone` for Docker
├── postcss.config.mjs
├── Dockerfile
└── .dockerignore
```

## Directory Purposes

**`src/app/`:**
- Purpose: Next.js App Router — maps URL segments to React components
- Contains: Page components (`page.tsx`), layouts (`layout.tsx`), dynamic routes (`[id]`), route groups
- Key files: `layout.tsx` (root), `page.tsx` (entry), `dashboard/layout.tsx` (auth guard)

**`src/components/`:**
- Purpose: Reusable React components for UI elements
- Contains: Presentational components, hooks, component-specific logic
- Current components: `BottomNav`, `SessionCard`, `TelemetryChart`, `Confetti`, `Toaster`

**`src/lib/`:**
- Purpose: Shared utilities, API client, type definitions
- Contains: `api.ts` — centralized API client with 60+ typed endpoint methods, auth helpers, domain models
- Used by: All page components

**`public/`:**
- Purpose: Static assets served directly by Next.js
- Contains: PWA manifest, service worker utilities, favicon, etc.

## Key File Locations

**Entry Points:**
- `src/app/layout.tsx` — Root layout with metadata, fonts, Toaster provider
- `src/app/page.tsx` — Redirect router (/ → /dashboard or /login based on auth)
- `src/app/login/page.tsx` — Authentication entry point
- `src/app/dashboard/page.tsx` — Main app dashboard

**Configuration:**
- `tsconfig.json` — TypeScript compiler options with `@/*` path alias
- `next.config.ts` — Next.js config with `output: standalone`
- `postcss.config.mjs` — Tailwind CSS PostCSS plugin
- `package.json` — Dependencies, scripts, build config

**Core Logic:**
- `src/lib/api.ts` — All API communication, types, auth token management
- `src/app/dashboard/layout.tsx` — Protected route guard for authenticated users

**Testing:**
- No test files detected (testing not yet implemented)

**Styling:**
- `src/app/globals.css` — Tailwind imports, Racing Point color variables

## Naming Conventions

**Files:**

| Pattern | Example | Use Case |
|---------|---------|----------|
| PascalCase `.tsx` | `BottomNav.tsx`, `SessionCard.tsx` | React components |
| camelCase `.ts` | `api.ts` | TypeScript utilities |
| lowercase `page.tsx` | `page.tsx` | Route components (Next.js convention) |
| `layout.tsx` | `layout.tsx` | Route layouts (Next.js convention) |
| `[param].tsx` | `[id]/page.tsx` | Dynamic route segments |

**Directories:**

| Pattern | Example | Use Case |
|---------|---------|----------|
| lowercase | `app/`, `components/`, `lib/` | Feature/utility directories |
| lowercase | `book/`, `dashboard/`, `wallet/` | Route segments |
| `[param]` | `sessions/[id]/`, `drivers/[id]/` | Dynamic routes |

**TypeScript Interfaces:**

- PascalCase: `DriverProfile`, `BillingSession`, `LapRecord`
- Suffix patterns: `Info` (read-only), `Payload` (request body), `Response` (API response)
- Enum-like constants: `STEP_LABELS_SINGLE`, `DIFFICULTY_PRESETS`

**Functions & Variables:**

- camelCase for all functions: `isLoggedIn()`, `fetchApi()`, `handleSendOtp()`
- camelCase for variables: `phone`, `otp`, `setStep`
- Constant prefixes: `const API_BASE =`, `const GATEWAY_URL =`

## Where to Add New Code

**New Feature Page:**
1. Create directory: `src/app/feature-name/`
2. Add page component: `src/app/feature-name/page.tsx`
3. Add types to `src/lib/api.ts` if needed
4. Add API methods to `api` object in `src/lib/api.ts`
5. Update `BottomNav.tsx` if feature should appear in bottom navigation
6. Example: `/streaming` → `src/app/streaming/page.tsx`

**New Component:**
1. Create component: `src/components/FeatureName.tsx`
2. Define props interface at top of file
3. Use `"use client"` if it needs state or browser APIs
4. Export as default
5. Import and use in page components
6. Example: new card variant → `src/components/PodCard.tsx`

**New API Endpoint:**
1. Add TypeScript interface for response in `src/lib/api.ts`
2. Add method to `api` object: `api.newEndpoint: () => fetchApi<ResponseType>(...)`
3. Page component imports and calls: `await api.newEndpoint()`
4. Always use `fetchApi()` wrapper for consistent auth/error handling

**Utilities:**
1. Create file: `src/lib/utils.ts` or feature-specific: `src/lib/booking.ts`
2. Export reusable functions
3. Import in components/pages that need them
4. Example: `formatDuration()` in `SessionCard.tsx` — candidate for `src/lib/formatting.ts`

**Styling:**
- Use Tailwind CSS class names directly in JSX
- Dark mode defined via `className="dark"` on `<html>` tag
- Racing Point brand colors available as Tailwind CSS variables: `bg-rp-red`, `text-rp-grey`, `border-rp-border`
- Custom theme in `src/app/globals.css` with CSS variables

## Special Directories

**`src/app/` (App Router):**
- Purpose: File-based routing — every directory with `page.tsx` or `layout.tsx` creates routes
- Generated: No (manually created)
- Committed: Yes
- Convention: `page.tsx` for pages, `layout.tsx` for shared structure, `[id]` for dynamic segments

**`public/`:**
- Purpose: Static assets served at `/` path
- Generated: No
- Committed: Yes
- Convention: Add favicons, PWA manifest, robots.txt, etc.

**`.next/` (Build Output):**
- Purpose: Next.js build artifacts (JavaScript, CSS, etc.)
- Generated: Yes (by `next build`)
- Committed: No (in `.gitignore`)
- Ephemeral: Re-created on each build

**`node_modules/`:**
- Purpose: Installed npm packages
- Generated: Yes (by `npm install`)
- Committed: No (in `.gitignore`)
- Recreate: `npm install`

---

*Structure analysis: 2026-03-21*
