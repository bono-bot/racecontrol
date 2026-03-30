# Phase 261: Design System Foundation - Context

**Gathered:** 2026-03-30
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Establish unified design token system, initialize shadcn/ui in both web and kiosk apps, integrate JetBrains Mono for numerics, add motion@12 for animations, and tw-animate-css for shadcn component animations. Replace Lucide icons. Zero component work — pure foundation.

Requirements: DS-01, DS-02, DS-03, DS-04, DS-05, DS-06, DS-07

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Key constraints from research:
- Token divergence: web has `rp-red-light`, kiosk has `rp-red-hover` — unify to `rp-red-hover`
- Use `motion@12` (NOT `framer-motion`), import from `motion/react`
- Use `tw-animate-css` (NOT `tailwindcss-animate` — incompatible with Tailwind v4)
- shadcn/ui CLI v4 for both apps, `new-york` style
- JetBrains Mono via `next/font/google` in web layout.tsx
- Lucide React for icons (replacing emoji in Sidebar.tsx)
- Do NOT enable React Compiler (TanStack Table bug #5567)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- web/src/app/globals.css — existing @theme inline with rp-* vars
- kiosk/src/app/globals.css — existing @theme inline with rp-* vars (has rp-surface, rp-purple, rp-green, rp-yellow that web lacks)
- web/src/app/layout.tsx — Montserrat via next/font/google
- kiosk/src/app/layout.tsx — already has JetBrains Mono

### Established Patterns
- Tailwind v4 CSS-first config via @theme inline blocks
- next/font/google for font loading
- Both apps use standalone output mode

### Integration Points
- web/src/components/Sidebar.tsx — emoji icons to replace with Lucide
- web/src/app/globals.css — token vars to extend
- kiosk/src/app/globals.css — token vars to align with web

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Follow research findings from .planning/research/STACK.md and ARCHITECTURE.md.

</specifics>

<deferred>
## Deferred Ideas

None — infrastructure phase.

</deferred>
