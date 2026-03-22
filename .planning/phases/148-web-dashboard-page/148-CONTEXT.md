# Phase 148: Web Dashboard Page - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Rewrite web/src/app/cameras/page.tsx to a feature-identical camera dashboard matching cameras.html from Phase 147. Uses React/Next.js with Tailwind CSS. Must share layout state with cameras.html via the server-side camera-layout.json API.

</domain>

<decisions>
## Implementation Decisions

### Visual Decisions (locked from Phase 147)
- Layout mode buttons: icon buttons in toolbar — 1×1/2×2/3×3/4×4
- Zone headers: collapsible section headers between camera groups
- Fullscreen: fade-in overlay (200ms) with dark backdrop, auto-hide controls after 3s
- WebRTC failure: snapshot fallback with "Live unavailable" badge
- Pre-warm: green pulsing border on hover (>500ms)
- Status indicators: top-right corner dot (6px)
- Offline cameras: 40% opacity + red dot + "OFFLINE" text
- Drag: semi-transparent ghost at 60% opacity + dashed border drop zones
- Drag reorder: auto-save on drop via PUT API

### React-Specific Decisions
- Use "use client" directive — this page is entirely client-side
- localStorage only in useEffect (standing rule: Next.js hydration)
- Use @dnd-kit/core for drag-to-rearrange (or HTML5 DnD directly like cameras.html)
- WebRTC connection managed via useRef (singleton RTCPeerConnection)
- Fetch cameras from SENTRY_BASE (http://192.168.31.27:8096)
- Layout state from /api/v1/cameras/layout (server-side, shared with cameras.html)

### Claude's Discretion
- Whether to use @dnd-kit/core or native HTML5 DnD (native preferred to avoid new dependency)
- Component structure (single page component vs extracted sub-components)
- Tailwind class patterns for layout modes
- WebRTC hook design (inline vs custom useWebRTC hook)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- web/src/app/cameras/page.tsx — existing 135-line basic cameras page (to be rewritten)
- web/src/components/DashboardLayout.tsx — shared layout wrapper
- cameras.html (Phase 147) — reference implementation with all features
- Tailwind CSS config — rp-card, rp-border, rp-grey, rp-black custom colors
- SENTRY_BASE = "http://192.168.31.27:8096"

### Established Patterns
- "use client" for interactive pages
- DashboardLayout wrapper for all pages
- Fetch from rc-sentry-ai at SENTRY_BASE
- Tailwind utility classes for all styling
- No `any` in TypeScript (standing rule)

### Integration Points
- /api/v1/cameras — camera metadata (display_name, display_order, zone, nvr_channel, status)
- /api/v1/cameras/layout — GET/PUT for grid_mode, camera_order
- /api/v1/cameras/nvr/:channel/snapshot — cached JPEG proxy
- ws://192.168.31.27:1984/api/ws?src=ch{N} — go2rtc WebRTC signaling
- DashboardLayout wraps the page content

</code_context>

<specifics>
## Specific Ideas

- Feature parity with cameras.html — same layout modes, drag, zones, WebRTC, status
- Shared layout state via server-side API — changes at :8096 visible at :3200
- This is the LAST phase — completing it ships the full v16.1 milestone

</specifics>

<deferred>
## Deferred Ideas

None — all features defined by Phase 147 reference implementation

</deferred>
