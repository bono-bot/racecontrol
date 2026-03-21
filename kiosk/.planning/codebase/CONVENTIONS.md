# Coding Conventions

**Analysis Date:** 2026-03-21

## Language & Runtime

- **Language:** TypeScript 5.9.3
- **Framework:** Next.js 16.1.6 (React 19.2.3)
- **Strict mode:** Enabled (`"strict": true`)
- **Type checking:** No `any` — all types explicitly defined

## Naming Patterns

**Files:**
- Page components: `src/app/[route]/page.tsx` (kebab-case routes, Next.js convention)
- Components: `src/components/PascalCase.tsx` (e.g., `KioskPodCard.tsx`, `LiveSessionPanel.tsx`)
- Hooks: `src/hooks/useKioskSocket.ts` (lowercase `use` prefix)
- Libraries: `src/lib/api.ts`, `src/lib/types.ts`, `src/lib/constants.ts`
- CSS: `globals.css` (Tailwind + custom theme variables)

**Functions:**
- Exported functions: `camelCase` (e.g., `formatLapTime()`, `gameLabel()`)
- Component functions: `PascalCase` (e.g., `ActivePodCard()`, `PinModal()`)
- Helper functions: lowercase first letter, grouped at top of file with `// ─── Helpers ─────` divider

**Variables:**
- Constants: `UPPERCASE` (e.g., `INACTIVITY_MS`, `SUCCESS_RETURN_MS`, `GAMES`)
- State variables: `camelCase` (e.g., `selectedPodId`, `billingTimers`)
- Props objects: descriptive interface (e.g., `KioskPodCardProps`)

**Types:**
- Interfaces: `PascalCase` (e.g., `Pod`, `BillingSession`, `TelemetryFrame`)
- Type unions: `PascalCase` (e.g., `PodStatus = "offline" | "idle" | "in_session"`)
- Utility types: lowercase (e.g., `Record<string, Pod>`, `Map<string, BillingSession>`)

## Code Organization

**File Structure:**
All files follow consistent header pattern:

```typescript
"use client";  // For interactive components

import { hooks, state, types, components };
import type { Interfaces, Types };

// ─── Types/Interfaces ─────────────────────────────────────────
interface MyComponentProps { ... }

// ─── Constants ─────────────────────────────────────────────────
const TIMEOUT_MS = 5000;

// ─── Helper Functions ─────────────────────────────────────────
function formatValue(v: T): string { ... }

// ─── Main Component ───────────────────────────────────────────
export default function MyComponent() { ... }

// ─── Subcomponents ────────────────────────────────────────────
function SubComponent() { ... }
```

**Section Dividers:**
Use visual separators for logical sections:
```typescript
// ─── Section Name ─────────────────────────────────────────────
```

**Import Organization:**
1. React hooks (`import { useState, useEffect }`)
2. Next.js utilities (`import Link from "next/link"`)
3. Custom hooks (`import { useKioskSocket }`)
4. Library functions (`import { api }`)
5. Types (`import type { Pod }`)
6. Components (`import { ErrorBoundary }`)
7. CSS (`import "./globals.css"`)

## TypeScript Patterns

**Strict Types:**
- All props require `Props` interface: `MyComponentProps`
- Props always typed as `Readonly<{ children: ReactNode }>`
- Return types explicit on functions (not inferred)

**Type Safety:**
- No implicit `any` — all unknowns typed explicitly
- Union types for state machines: `type PinStep = "numpad" | "validating" | "success" | "error"`
- Discriminated unions: `type DeployState = { state: 'idle' } | { state: 'downloading'; detail: {...} }`
- `type` over `interface` for unions and primitives

**Null/Undefined Handling:**
- Optional properties: `prop?: Type`
- Nullable state: `value: Type | null`
- Checks: `if (value) { ... }` or `value?.property` (optional chaining)
- Maps for state: `Map<string, Pod>` instead of array for O(1) lookups

## Error Handling

**Pattern:**
```typescript
try {
  const res = await api.validateKioskPin(pin, selectedPodId);
  if (res.error) {
    setErrorMsg(res.error);
    return;
  }
  // success path
} catch {
  setErrorMsg("Network error — please try again");
}
```

**Rules:**
- API errors checked via response object (`if (res.error)`)
- Network errors caught with generic `catch` (e.message not needed)
- Error messages shown via state setter (`setErrorMsg()`)
- No `throw` in UI code — convert to state updates

**Console Logging:**
- Production logs: prefixed `[Context]` (e.g., `[Kiosk]`, `[ErrorBoundary]`)
- Log levels: `console.log()` for info, `console.warn()` for warnings, `console.error()` for errors
- Examples:
  ```typescript
  console.log("[Kiosk] Connected to RaceControl");
  console.warn("[Kiosk] Parse error:", e);
  console.error("[ErrorBoundary] Caught:", error);
  ```

## React Patterns

**Hooks:**
- `useState` for local UI state (modals, forms, timers)
- `useRef` for stable object references (WebSocket, stateRef across renders)
- `useCallback` for memoized event handlers (prevents dependency issues)
- `useEffect` for side effects (connections, intervals, timers)
- Dependencies always explicit and verified

**State Management:**
- Local component state when isolated (no global context)
- WebSocket hook (`useKioskSocket()`) for shared data (pods, billing, telemetry)
- Maps for indexed collections: `Map<podId, Pod>`
- Immutable updates: `new Map(prev); next.set(key, value); return next;`

**Event Handlers:**
- Named functions: `handleDigit()`, `handleSubmit()`, `handleClick()`
- Async handlers wrapped: `async function handler() { try { ... } catch { ... } }`
- Touch tracking for inactivity: `touch()` callback sets `lastActivity`

## Component Patterns

**Functional Components:**
- Always `"use client"` for interactive kiosk components
- Props destructured inline: `function Component({ prop1, prop2 }: Props)`
- Return early for conditional renders:
  ```typescript
  if (condition) return <div>Empty</div>;
  return <div>Content</div>;
  ```

**Modal/Overlay Patterns:**
- Render conditionally: `{selectedPodId && <Modal />}`
- Backdrop click dismissal: check `e.target === e.currentTarget`
- Multiple step handling: switch on state enum (e.g., `pinStep`)

**Data Binding:**
- Direct from hook: `const { pods, billing } = useKioskSocket()`
- Map lookups: `const pod = pods.get(podId)`
- Fallback values: `telemetry?.speed_kmh ?? 0`

## Styling Conventions

**CSS Framework:** Tailwind 4 with custom Racing Point theme

**Color System:**
- Racing Red: `rp-red` (use for CTAs, alerts, active states)
- Black: `rp-black` (backgrounds)
- Grey: `rp-grey` (muted text, secondary labels)
- Card: `rp-card` (container backgrounds)
- Border: `rp-border` (dividers, input borders)
- Surface: `rp-surface` (elevated surfaces)

**Font System:**
- Body: `font-sans` (Montserrat)
- Display: `font-[family-name:var(--font-display)]` (Space Grotesk for headers)
- Mono: `font-[family-name:var(--font-mono-jb)]` (JetBrains Mono for times, speeds)

**Animations:**
- Pulse: `.pulse-dot` (connection indicator)
- Glow: `.glow-active` (active pod card)
- Transitions: `transition-all`, `transition-colors` (hover states)

**Responsive:**
- Grid: `grid-cols-4 grid-rows-2` (4x2 pod grid for 8 rigs)
- Flex: `flex flex-col`, `flex items-center justify-between`
- Spacing: consistent 4px units via `gap-3`, `px-4`, `py-2`

## Comments

**When to Comment:**
- Section dividers (visual organization): `// ─── Section Name ─────`
- Non-obvious logic (e.g., debounce timers): `// Debounce UI update — prevent false "Disconnected" flashes`
- Hacks or workarounds: Mark with context (rarely needed)

**Not Commented:**
- Self-evident function names (`formatLapTime()` is clear)
- React hook usage (patterns well-known)
- Type definitions (interface names are descriptive)

## Module Design

**Exports:**
- Default exports for pages: `export default function Page()`
- Named exports for components: `export function MyComponent()`
- Re-export types from `lib/types.ts` at module entry point

**Barrel Files:**
- No barrel files (index.ts) — import directly from source files
- Import paths: `import { api } from "@/lib/api"` (not `from "@/lib"`)

**API Module Pattern:**
```typescript
// lib/api.ts
export async function fetchApi<T>(path: string, options?: RequestInit): Promise<T>
export const api = {
  health: () => fetchApi<HealthResponse>("/health"),
  listPods: () => fetchApi<{ pods: Pod[] }>("/pods"),
}
```

## Path Aliases

**Configured in tsconfig.json:**
- `@/*` → `./src/*` (allows `import from "@/components"`)

---

*Convention analysis: 2026-03-21*
