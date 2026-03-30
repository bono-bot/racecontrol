---
phase: 263-web-primitive-components
plan: 04
subsystem: web-login
tags: [login, pinpad, motorsport-aesthetic, lockout, error-states]
dependency_graph:
  requires: [263-02]
  provides: [login-page-pinpad-integration, lockout-countdown, racing-red-login]
  affects: [264-web-dashboard-pages]
tech_stack:
  added: []
  patterns: [component-composition, SSR-safe-auth-check, lockout-countdown-timer]
key_files:
  created: []
  modified:
    - web/src/app/login/page.tsx
decisions:
  - Used PinPad component from 263-02 instead of inline numpad code
  - Lockout countdown uses setInterval with cleanup in useEffect
  - Speed-line background via inline style repeating-linear-gradient (CSS-only, no external dep)
metrics:
  duration: ~5min
  completed: "2026-03-30"
  tasks_completed: 1
  tasks_total: 1
  files_modified: 1
requirements:
  - LP-01
  - LP-02
---

# Phase 263 Plan 04: Login Page Redesign Summary

Login page rewritten to use PinPad component (from 263-02) with motorsport aesthetic, Racing Red accents, and lockout error handling.

## What Changed

### Task 1: Login page redesign -- PinPad integration + motorsport aesthetic + error states

**Rewrote `web/src/app/login/page.tsx`** (175 lines reduced to 125 lines):

- **Removed all inline numpad code**: `handleDigit`, `handleBackspace`, `handleClear` functions, the 3x4 button grid, PIN display boxes, and the duplicate keyboard `useEffect` -- PinPad now owns all of this
- **Integrated PinPad component**: `import PinPad from "@/components/PinPad"` with props `onComplete`, `disabled`, `error`, `loading`
- **Motorsport aesthetic**: `bg-rp-black` full-screen with CSS speed-line background (repeating-linear-gradient at -45deg with `rgba(225,6,0,0.03)` lines), `bg-rp-card` center card with `border-rp-border`, rounded-2xl, shadow-2xl
- **Racing Red accent bar**: `h-1 bg-rp-red` strip at the top of the login card
- **RaceControl wordmark**: Inline SVG racing flag icon + "RaceControl" h1 + "Racing Point Bandlaguda" subtitle
- **Lockout countdown (LP-02)**: `lockoutSeconds` state with `setInterval` timer that counts down from server-provided seconds to 0; PinPad disabled during lockout with error message "Locked out -- Ns remaining"
- **Error states (LP-02)**: Three distinct error messages -- "Invalid staff PIN" (invalid), "Too many attempts. Try again in Ns" (429), "Cannot reach server. Check your connection." (network error)
- **SSR safe**: No `sessionStorage`/`localStorage` in `useState` initializers; `isAuthenticated()` check in `useEffect` only
- **All Tailwind classes use `rp-*` tokens**: Zero raw hex values (`#E10600`, `#1A1A1A`, etc.) in the file

## Verification Results

| Check | Result |
|-------|--------|
| `npx tsc --noEmit` | 0 errors |
| `import PinPad` present | line 6 |
| `handleDigit\|handleBackspace\|handleClear` | 0 hits (removed) |
| `lockoutSeconds` references | 6 hits (countdown logic present) |
| `rp-red\|rp-black\|rp-card\|rp-border` | All present |
| `#E10600\|#1A1A1A\|#222222\|#333333` | 0 hits (no raw hex) |
| `sessionStorage\|localStorage` | 0 hits (SSR safe) |

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None -- all data flows are wired (PinPad -> handleComplete -> validate-pin API -> setToken/error).
