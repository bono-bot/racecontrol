# Architecture

**Analysis Date:** 2026-03-21

## Pattern Overview

**Overall:** Next.js App Router with Client-Side State Management

**Key Characteristics:**
- Server-side rendering (SSR) with `output: standalone` for Docker deployment
- Client-centric architecture using `"use client"` for interactive pages and components
- Token-based JWT authentication stored in `localStorage`
- Centralized API client library (`@/lib/api.ts`) with typed fetch wrapper
- Layout-based route organization with authentication guards
- Component composition for UI elements (reusable cards, navigation, charts)

## Layers

**App Router Layer:**
- Purpose: URL routing, page composition, layout inheritance
- Location: `src/app/`
- Contains: Page components (`.tsx`), layout wrappers, route segments with `[id]` dynamic parameters
- Depends on: React 19, Next.js 16, components, API client
- Used by: Browser navigation, Next.js routing engine

**Page Components:**
- Purpose: Render full-page views for routes, manage page-level state
- Location: `src/app/*/page.tsx` (e.g., `src/app/dashboard/page.tsx`, `src/app/login/page.tsx`)
- Contains: Form handling, data fetching with `useEffect`, state management with `useState`
- Depends on: API client (`api.*` methods), utility components, React hooks
- Used by: App Router, authentication guards in layouts

**Layout Components:**
- Purpose: Provide persistent UI structure (header, navigation, footer)
- Location: `src/app/layout.tsx` (root), `src/app/dashboard/layout.tsx` (feature-scoped)
- Contains: Metadata, viewport config, authentication checks, global providers
- Depends on: Toaster, fonts, global styles
- Used by: All child routes

**Component Layer:**
- Purpose: Reusable UI primitives and feature-specific components
- Location: `src/components/`
- Contains: Presentation logic, interactive elements (cards, charts, navigation)
- Depends on: React hooks, Tailwind CSS, third-party libraries (recharts, canvas-confetti)
- Used by: Page components, other components

**API Client Layer:**
- Purpose: Centralized HTTP communication with backend
- Location: `src/lib/api.ts`
- Contains: `fetchApi()` wrapper, typed interfaces for all domain models, API method collection, auth token management
- Depends on: `fetch` API, localStorage
- Used by: All page components

## Data Flow

**Authentication Flow:**

1. User enters phone on `/login`
2. `api.login(phone)` → POST `/customer/login` → OTP sent via WhatsApp
3. User enters OTP → `api.verifyOtp(phone, otp)` → POST `/customer/verify-otp`
4. Backend returns JWT token → `setToken(token)` → stored in `localStorage`
5. Token attached to all subsequent requests via `Authorization: Bearer` header in `fetchApi()`
6. Invalid JWT → `forceLogout()` → clear token, redirect to `/login`

**Page Load with Auth Guard:**

1. Page component mounts (e.g., `/dashboard/page.tsx`)
2. `useEffect` → `isLoggedIn()` → checks `localStorage.rp_token`
3. If not logged in: `router.replace("/login")`
4. If logged in: fetch data with `api.profile()`, `api.sessions()`, etc.
5. Display data with loading spinner
6. Parent layout (`dashboard/layout.tsx`) also guards route

**Session Booking Flow:**

1. User selects duration tier on `/book`
2. Wizard state updated: `setTier()`, `setGame()`, `setSessionType()`, etc.
3. User confirms → `api.bookSession()` or `api.bookCustom()` → POST `/customer/book`
4. Backend allocates pod and returns `reservation_id`, `pod_number`, `pin`
5. Redirect to `/sessions/[id]` to view active session details
6. Poll `api.activeReservation()` to track remaining time

**Data Fetching Pattern:**

```typescript
// Page component
useEffect(() => {
  async function load() {
    const [pRes, sRes, sessRes] = await Promise.all([
      api.profile(),
      api.stats(),
      api.sessions(),
    ]);
    if (pRes.driver) setProfile(pRes.driver);
    // ... handle errors
  }
  load();
}, []);
```

**State Management:**

- Local component state with `useState` (no Redux/Context)
- Server state fetched on demand via API client
- Persistent auth token in localStorage
- URL params for pagination/filtering (e.g., `/sessions/[id]`)
- Search params for UI mode switching (e.g., `/book?trial=true`)

## Key Abstractions

**API Client (Typed Fetch Wrapper):**
- Purpose: Encapsulate HTTP requests with auth, error handling, response typing
- Location: `src/lib/api.ts`
- Pattern: Typed async functions returning domain models
- Example: `api.profile()` → `Promise<{ driver?: DriverProfile; error?: string }>`

**Domain Models:**
- Purpose: TypeScript interfaces for backend data structures
- Location: `src/lib/api.ts` (lines 94-570)
- Examples: `DriverProfile`, `BillingSession`, `LapRecord`, `GroupSessionInfo`, `TournamentInfo`
- Used by: Page components for type safety, IDE autocompletion

**UI Component Primitives:**
- Purpose: Reusable presentation elements
- Location: `src/components/`
- Examples:
  - `SessionCard` — displays billing session summary with progress bar
  - `TelemetryChart` — renders multi-panel telemetry visualization with recharts
  - `BottomNav` — persistent navigation with 7 tabs and badge notification
  - `Confetti` — celebration animation on session completion

**Layout Guards:**
- Purpose: Protect routes requiring authentication
- Pattern: `useEffect` in layout/page → `isLoggedIn()` → conditionally `router.replace("/login")`
- Location: `src/app/layout.tsx`, `src/app/dashboard/layout.tsx`, all protected pages

## Entry Points

**Root Entry Point:**
- Location: `src/app/page.tsx`
- Triggers: Page load at `/`
- Responsibilities: Check auth status, redirect to `/dashboard` (logged in) or `/login` (not logged in)

**Authentication Flow:**
- Location: `src/app/login/page.tsx`
- Triggers: User accesses `/login` or redirected from protected route
- Responsibilities: Phone/OTP verification, registration form, JWT token storage

**Main Application:**
- Location: `src/app/dashboard/layout.tsx` + `src/app/dashboard/page.tsx`
- Triggers: Logged-in user accesses `/dashboard` or subroutes
- Responsibilities: Auth guard, render BottomNav, display user profile and recent sessions

**API Configuration:**
- Location: `src/lib/api.ts` (lines 1-5)
- Env vars:
  - `NEXT_PUBLIC_API_URL` — backend API base (default: `http://localhost:8080/api/v1`)
  - `NEXT_PUBLIC_GATEWAY_URL` — payment gateway (default: `/api/payments`)

## Error Handling

**Strategy:** Graceful degradation with inline error messages and auto-logout on auth failure

**Patterns:**

1. **Network Error:**
   - `fetchApi()` catches `fetch()` failures
   - Returns empty object `{}` as `T` if response is non-JSON
   - Page components show error state or empty UI

2. **Auth Error:**
   - Detect JWT errors in response: `error.includes("JWT decode error")` or `error.includes("Missing Authorization")`
   - Trigger `forceLogout()` → clear token, redirect to `/login`
   - Guard against multiple redirect attempts with `_redirecting` flag

3. **API Error Response:**
   - API returns `{ error: "message" }` in response
   - Page component checks `res.error` field
   - Display error message with `setError()` or `sonner` toast

4. **Validation Error (Client-side):**
   - Form inputs validated before submission (e.g., phone length, OTP format)
   - Show inline error message
   - Disable submit button

## Cross-Cutting Concerns

**Logging:** Console logging for debugging; `sonner` toast for user-facing errors

**Validation:**
- Client-side: Input length checks, date parsing, format validation
- Server-side: Full validation deferred to backend

**Authentication:**
- Token-based JWT in localStorage
- Token attached to all requests via `Authorization: Bearer` header
- Auto-logout on 401/JWT errors
- Guard routes with `useEffect` + `isLoggedIn()` check

**Styling:** Tailwind CSS v4 with Racing Point brand colors defined as CSS variables
- `--rp-red`: #E10600
- `--rp-dark`: #1A1A1A
- `--rp-card`: #222222
- `--rp-border`: #333333
- `--rp-grey`: #5A5A5A

**Fonts:** Montserrat (body, 300-700 weights) from Google Fonts

---

*Architecture analysis: 2026-03-21*
