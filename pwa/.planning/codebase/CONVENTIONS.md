# Coding Conventions

**Analysis Date:** 2026-03-21

## Naming Patterns

**Files:**
- Page files: `[name]/page.tsx` (e.g., `src/app/dashboard/page.tsx`, `src/app/login/page.tsx`)
- Components: PascalCase with `.tsx` extension (e.g., `src/components/BottomNav.tsx`, `TelemetryChart.tsx`)
- Utility modules: camelCase (e.g., `src/lib/api.ts`)

**Functions:**
- Exported React components: PascalCase (e.g., `DashboardPage`, `BottomNav`, `TelemetryChart`)
- Internal helper functions: camelCase (e.g., `formatDuration`, `formatLapTime`, `handleSendOtp`)
- Type predicates and utility functions: camelCase (e.g., `getToken`, `isLoggedIn`, `fetchApi`)

**Variables:**
- State variables: camelCase (e.g., `loading`, `phone`, `profile`, `recentSessions`)
- Constants: camelCase for values, UPPER_SNAKE_CASE for configuration constants
  - Example: `const API_BASE = ...` (config constant)
  - Example: `const tabs = [...]` (data constant, camelCase)
- Props and callback handlers: camelCase (e.g., `onClose`, `lapId`, `sessionId`, `enabled`)

**Types:**
- Interfaces: PascalCase with descriptive names (e.g., `DriverProfile`, `BillingSession`, `TelemetryChartProps`)
- Type aliases: PascalCase (e.g., `Step = "phone" | "otp" | "register"`)
- Generic parameters: Standard single letters (e.g., `<T>`) or descriptive (e.g., `<Props>`)

## Code Style

**Formatting:**
- No explicit linter/formatter configured in package.json (no ESLint or Prettier config detected)
- Consistent indentation: 2 spaces (observed throughout codebase)
- Line length: no explicit limit observed, functional lines vary (50-120 characters observed)
- Quote style: double quotes for strings and JSX attributes (consistent throughout)
- Trailing commas: used in multiline structures

**Linting:**
- ESLint is invoked via `npm run lint` but no configuration file present
- Single ESLint disable comment used in codebase: `// eslint-disable-next-line @typescript-eslint/no-explicit-any` in `src/components/TelemetryChart.tsx` line 32

**TypeScript:**
- Strict mode enabled: `"strict": true` in `tsconfig.json`
- JSX pragma: `"jsx": "react-jsx"` (Next.js default with React 19)
- Path aliases: `@/*` resolves to `./src/*`
- Type imports: use `import type { ... }` for type-only imports (consistently done)
  - Example: `import type { Metadata, Viewport } from "next";`
  - Example: `import type { DriverProfile, CustomerStats } from "@/lib/api";`

## Import Organization

**Order:**
1. External framework imports: `next`, `react` modules
2. Next.js components and utilities: `next/navigation`, `next/font/google`
3. Relative or aliased internal imports: `@/components`, `@/lib`, `@/`
4. CSS/stylesheet imports: `./globals.css`

**Example from `src/app/layout.tsx`:**
```typescript
import type { Metadata, Viewport } from "next";
import { Montserrat } from "next/font/google";
import "./globals.css";
import RpToaster from "@/components/Toaster";
```

**Example from `src/app/dashboard/page.tsx`:**
```typescript
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { DriverProfile, CustomerStats, BillingSession } from "@/lib/api";
import SessionCard from "@/components/SessionCard";
```

**Path Aliases:**
- Use `@/` prefix for all internal imports from `src/` directory
- Never use relative paths like `../../../components`

## Client vs Server Boundaries

**Client Components:**
- All page files in `src/app/*/page.tsx` marked with `"use client"` at top of file
- All interactive components marked with `"use client"` (e.g., `BottomNav.tsx`, `TelemetryChart.tsx`)
- Root layout `src/app/layout.tsx` is a Server Component (no `"use client"`)
- Dashboard layout `src/app/dashboard/layout.tsx` appears to be Server Component

**Pattern observed in `src/app/page.tsx`:**
```typescript
"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { isLoggedIn } from "@/lib/api";

export default function RootPage() {
  const router = useRouter();

  useEffect(() => {
    if (isLoggedIn()) {
      router.replace("/dashboard");
    } else {
      router.replace("/login");
    }
  }, [router]);
  ...
}
```

## Error Handling

**Patterns:**
- Try-catch with empty catch handlers for non-critical failures:
  ```typescript
  try {
    // operation
  } catch {
    // Silent fail or set error state
  } finally {
    setLoading(false);
  }
  ```
- Error state in components: `const [error, setError] = useState<string | null>(null)`
- Error messages shown inline in UI: `{error && <p className="text-red-400 text-sm mb-4">{error}</p>}`
- API error responses have shape: `{ error?: string; [other fields] }`
  - Example: `if (res.error) { setError(res.error); } else { /* success */ }`

**Logging:**
- No dedicated logging framework observed (console logging could be used but not evident in sample files)
- Error handling relies on returned error objects from API calls, not thrown exceptions

## Async Operations

**Pattern - useEffect with async IIFE:**
```typescript
useEffect(() => {
  if (!isLoggedIn()) return;

  async function load() {
    try {
      const [pRes, sRes] = await Promise.all([
        api.profile(),
        api.stats(),
      ]);
      if (pRes.driver) setProfile(pRes.driver);
    } catch {
      // handle error
    } finally {
      setLoading(false);
    }
  }

  load();
}, [dependencies]);
```

**Fetch wrapper at module level:**
- All API calls go through `fetchApi<T>()` generic function in `src/lib/api.ts`
- Generic type parameter ensures type safety
- Automatic JWT token injection via `Authorization` header
- Auto-logout on JWT decode errors

## Component Structure

**Functional components with hooks:**
- Always use arrow function `export default function ComponentName() { ... }` or `const ComponentName = () => { ... }`
- Prefer `export default` for page and exported components
- Named exports for shared utility functions/constants

**Props pattern:**
- Destructured in function signature
- TypeScript interfaces for prop types, named with `Props` suffix or inlined with object destructuring
  - Example: `export default function TelemetryChart({ lapId, onClose }: TelemetryChartProps)`
  - Example: `export default function SessionCard({ session }: { session: BillingSession })`

**State initialization:**
- Use `useState` with explicit types: `const [loading, setLoading] = useState(true)`
- Type annotations required for non-obvious types: `const [profile, setProfile] = useState<DriverProfile | null>(null)`

## Styling with Tailwind CSS

**Classes:**
- Dark theme with custom CSS variables: `dark` mode enabled in `html` tag
- Custom color classes use RacingPoint brand colors:
  - `bg-rp-dark`: Dark background (#1A1A1A)
  - `bg-rp-card`: Card background (#222222)
  - `bg-rp-border`: Border color (#333333)
  - `text-rp-red`: Racing Red (#E10600)
  - `text-rp-grey`: Gunmetal Grey (#5A5A5A)

**Safe area insets (PWA):**
- Used for bottom-positioned navigation: `safe-area-bottom`
- Used for full-screen layouts: `safe-area-inset-*` variants

**Responsive design:**
- Max-width container pattern: `max-w-lg mx-auto` (observed in multiple pages)
- Mobile-first approach with fixed bottom navigation on all pages

## Comments

**JSDoc for exported functions:**
- Used for utility/component documentation
  - Example in `src/components/Toaster.tsx`:
    ```typescript
    /**
     * RacingPoint-themed sonner Toaster.
     * Dark theme, top-center position, card colors matching rp-card/rp-border.
     */
    export default function RpToaster() { ... }
    ```

**Inline comments for complex logic:**
- Used sparingly, mainly to explain business logic
- Observed in `src/components/TelemetryChart.tsx`: `// Transform samples for recharts: offset_ms -> time_s`
- Section comments with dashes: `// ─── Auth helpers ──────────────────────────────────────────────────────────` in api.ts

**Browser hydration warnings:**
- No explicit comments but pattern shows awareness of hydration:
  ```typescript
  if (typeof window !== "undefined" && sessionStorage.getItem(key)) return;
  ```

## Module Exports

**API module pattern (`src/lib/api.ts`):**
- 49 total exports: functions, types, and interfaces
- Export structure: auth helpers, fetch wrapper, type definitions, API functions
- Grouped logically with section comments
- Mixed export types: `export function`, `export interface`, `export const`

**Component exports:**
- Single default export per file (typical React convention)
- Example: `export default function BottomNav() { ... }`

## Special Patterns

**localStorage usage:**
- Token management: `getToken()`, `setToken(token)`, `clearToken()` functions
- Session tracking: `sessionStorage` for confetti one-per-session gate
- Environment check before access: `if (typeof window === "undefined") return null`

**Form handling:**
- Controlled inputs with onChange handlers
- Manual validation before submission (length checks, format validation)
- State lifted to page level, passed to handlers
- Error messages displayed below inputs

**Data transformation:**
- Format functions are internal helpers: `formatDuration()`, `formatDate()`, `formatLapTime()`
- Transform arrays before rendering: `const chartData = (data?.samples || []).map(s => ({ ... }))`
- Status mapping via switch statements: `statusColor()`, `statusLabel()`

---

*Convention analysis: 2026-03-21*
