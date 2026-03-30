---
phase: 261-design-system-foundation
plan: 01
subsystem: design-tokens
tags: [css, design-system, tokens, tailwind-v4]
dependency_graph:
  requires: []
  provides: [shared-color-tokens, rp-red-hover-canonical]
  affects: [web-globals, kiosk-globals, all-downstream-components]
tech_stack:
  added: [packages/shared-tokens]
  patterns: [css-custom-properties, tailwind-v4-theme-inline, relative-import]
key_files:
  created:
    - packages/shared-tokens/tokens.css
  modified:
    - web/src/app/globals.css
    - kiosk/src/app/globals.css
decisions:
  - "Unified rp-red-light to rp-red-hover as canonical name across both apps"
  - "Font tokens kept app-specific (not in shared tokens) since web and kiosk use different font stacks"
  - "PWA rp-red-light references left out of scope (PWA not in plan scope)"
metrics:
  duration: 153s
  completed: "2026-03-30T10:03:00Z"
  tasks: 3
  files: 3
requirements: [DS-01]
---

# Phase 261 Plan 01: Shared Design Tokens Summary

Unified 10 color tokens into packages/shared-tokens/tokens.css, wired both web and kiosk globals.css to import it, eliminating rp-red-light/rp-red-hover divergence.

## What Was Done

### Task 1: Create packages/shared-tokens/tokens.css
- Created `packages/shared-tokens/tokens.css` with 10 color tokens
- `:root` block defines raw CSS custom properties (--rp-red, --rp-red-hover, --rp-black, --rp-grey, --rp-surface, --rp-card, --rp-border, --rp-green, --rp-yellow, --rp-purple)
- `@theme inline` block maps to `--color-rp-*` names for Tailwind v4 utility class generation
- No font tokens (those are app-specific)
- **Commit:** `a5a759e9`

### Task 2: Update web/src/app/globals.css
- Added `@import "../../../packages/shared-tokens/tokens.css"` as second import
- Removed `:root` block (all color vars now from shared tokens)
- Removed all `--color-rp-*` entries from `@theme inline` (now from shared tokens)
- Removed `rp-red-light` entirely (replaced by `rp-red-hover` in shared tokens)
- Kept web-specific: `--color-background`, `--color-foreground`, `--font-sans`, `--font-mono`
- Preserved body styles, scrollbar CSS, and record-flash animation
- **Commit:** `18bcafab`

### Task 3: Update kiosk/src/app/globals.css
- Added `@import "../../../packages/shared-tokens/tokens.css"` as second import
- Removed `:root` block (all color vars now from shared tokens)
- Removed all `--color-rp-*` entries from `@theme inline` (now from shared tokens)
- Kept kiosk-specific font tokens: `--font-sans`, `--font-display`, `--font-mono`
- Preserved `overflow: hidden` and `user-select: none` on body
- Preserved all kiosk animations: pulse-dot, red-glow, slideUp
- **Commit:** `a4c6bc06`

## Deviations from Plan

None - plan executed exactly as written.

## Discovered Items (Out of Scope)

PWA app (`pwa/src/`) has 7 references to `rp-red-light` (1 in globals.css, 6 in TSX files). These are outside this plan's scope (web/src/ and kiosk/src/ only). Should be addressed when PWA is updated to use shared tokens.

## Known Stubs

None - all tokens are fully defined with production color values.

## Verification Results

| Check | Result |
|-------|--------|
| tokens.css has rp-red-hover (>=2 hits) | PASS (2) |
| web imports shared-tokens | PASS |
| kiosk imports shared-tokens | PASS |
| rp-red-light in web/src/ + kiosk/src/ = 0 | PASS (0) |
| kiosk body has overflow:hidden | PASS |
| kiosk body has user-select:none | PASS |
| No --color-rp-* in web @theme inline | PASS (0) |
| No --color-rp-* in kiosk @theme inline | PASS (0) |

## Self-Check: PASSED

All 3 created/modified files exist. All 3 task commits verified in git log.
