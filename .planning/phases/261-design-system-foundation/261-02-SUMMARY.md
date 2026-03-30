---
phase: 261-design-system-foundation
plan: 02
subsystem: frontend
tags: [shadcn-ui, design-system, icons, tailwind-v4]
dependency_graph:
  requires: [261-01]
  provides: [shadcn-components, lucide-icons, tw-animate-css]
  affects: [web, kiosk]
tech_stack:
  added: [shadcn/ui v4, lucide-react, tw-animate-css, class-variance-authority, clsx, tailwind-merge, @radix-ui/*]
  patterns: [CSS variable theming, component composition via shadcn, Lucide icon components]
key_files:
  created:
    - web/components.json
    - web/src/components/ui/button.tsx
    - web/src/components/ui/badge.tsx
    - web/src/components/ui/table.tsx
    - web/src/components/ui/dialog.tsx
    - web/src/components/ui/select.tsx
    - web/src/components/ui/tooltip.tsx
    - web/src/components/ui/dropdown-menu.tsx
    - web/src/components/ui/separator.tsx
    - web/src/components/ui/tabs.tsx
    - web/src/lib/utils.ts
    - kiosk/components.json
    - kiosk/src/components/ui/button.tsx
    - kiosk/src/components/ui/badge.tsx
    - kiosk/src/components/ui/dialog.tsx
    - kiosk/src/components/ui/progress.tsx
    - kiosk/src/lib/utils.ts
  modified:
    - web/src/app/globals.css
    - web/package.json
    - kiosk/src/app/globals.css
    - kiosk/package.json
    - web/src/components/Sidebar.tsx
decisions:
  - "Used new-york style for shadcn/ui (overrode CLI default base-nova)"
  - "Dark-first theming: single :root block with RP brand vars, removed .dark block (app is always dark)"
  - "Reverted shadcn Geist font injection in both layout.tsx files to preserve Montserrat brand"
  - "ToggleLeft for Feature Flags (distinct from Flag used for AC LAN Race)"
metrics:
  duration: 426s
  completed: 2026-03-30
  tasks: 3
  files: 24
---

# Phase 261 Plan 02: shadcn/ui Init + Lucide Icons Summary

shadcn/ui v4 initialized in both web and kiosk with new-york style, tw-animate-css for Tailwind v4 animations, RP brand mapped to shadcn semantic CSS variables, and Sidebar emoji icons replaced with 21 Lucide React components.

## Task Results

### Task 1: Initialize shadcn/ui in web app
- shadcn/ui v4 CLI init with Tailwind v4 mode
- tw-animate-css imported (line 3 of globals.css), tailwindcss-animate NOT added
- 9 shadcn components added: button, badge, table, dialog, select, tooltip, dropdown-menu, separator, tabs
- RP brand variables mapped to all shadcn semantic tokens (--background, --primary, --card, etc.)
- Preserved shared-tokens import, body styles, scrollbar styles, record-flash animation
- Reverted Geist font injection from layout.tsx
- **Commit:** `ff2aa02e`

### Task 2: Initialize shadcn/ui in kiosk app
- Same shadcn/ui v4 init with identical RP brand mapping
- 4 kiosk-specific components: button, badge, dialog, progress
- Kiosk body `overflow: hidden` and `user-select: none` verified preserved
- Kiosk-specific animations (pulse-dot, red-glow, slideUp) preserved
- Reverted Geist font injection from layout.tsx
- **Commit:** `3b1e948d`

### Task 3: Replace emoji icons with Lucide React in Sidebar
- 21 nav items migrated from emoji strings to Lucide icon components
- LucideIcon type imported and used for typed nav array
- Icons render as `<item.icon size={16} className="shrink-0" />`
- All Sidebar structure preserved (layout, classes, active indicator, footer links)
- **Commit:** `25523034`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] shadcn CLI used base-nova style instead of new-york**
- **Found during:** Task 1 and Task 2
- **Issue:** `npx shadcn@latest init --defaults` used base-nova as the default style, not new-york
- **Fix:** Manually updated components.json `"style"` field to `"new-york"` in both apps
- **Files modified:** web/components.json, kiosk/components.json

**2. [Rule 1 - Bug] shadcn injected Geist font into both layout.tsx files**
- **Found during:** Task 1 and Task 2
- **Issue:** shadcn init added Geist font import and replaced the html className with cn() including geist.variable, breaking the Montserrat brand font
- **Fix:** Reverted layout.tsx in both apps via `git checkout`
- **Files restored:** web/src/app/layout.tsx, kiosk/src/app/layout.tsx

**3. [Rule 1 - Bug] shadcn overwrote font-sans with self-referencing var(--font-sans)**
- **Found during:** Task 1 and Task 2
- **Issue:** shadcn replaced `'Montserrat', sans-serif` with `var(--font-sans)` in the @theme block, creating a circular reference
- **Fix:** Restored explicit font family values in both globals.css files
- **Files modified:** web/src/app/globals.css, kiosk/src/app/globals.css

**4. [Rule 2 - Missing] shadcn generated light-mode :root and .dark blocks**
- **Found during:** Task 1 and Task 2
- **Issue:** Racing Point is a dark-first app with no light mode. shadcn generated both :root (light) and .dark blocks with oklch values unrelated to RP brand
- **Fix:** Replaced with single :root block mapping all shadcn semantic vars to RP brand tokens (--rp-black, --rp-red, --rp-card, etc.)
- **Files modified:** web/src/app/globals.css, kiosk/src/app/globals.css

## Known Stubs

None. All components are fully wired to shadcn's registry. CSS variables reference real RP brand tokens from shared-tokens/tokens.css.

## Self-Check: PASSED

All 6 key files verified on disk. All 3 commit hashes found in git log.
