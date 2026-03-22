# Phase 147: cameras.html Dashboard Rewrite - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Full rewrite of cameras.html (embedded via include_str! in rc-sentry-ai) to a professional NVR dashboard with layout modes, drag-to-rearrange, WebRTC fullscreen, zone grouping, and status indicators. Vanilla JS only (no React/npm). Fetches camera metadata from /api/v1/cameras and layout from /api/v1/cameras/layout.

</domain>

<decisions>
## Implementation Decisions

### Layout & Grid
- Layout mode buttons: icon buttons (grid icons like ⊞) in toolbar — compact, DMSS-style
- Cameras that don't fit the selected grid: scroll within the grid area — all cameras always visible
- Zone headers: collapsible section headers between camera groups — "ENTRANCE (2)"
- Grid gap: 2px between tiles — maximize camera visibility

### WebRTC Fullscreen
- Fullscreen transition: fade-in overlay (200ms) with dark backdrop — smooth, non-jarring
- Fullscreen controls: camera name + close (X) + connection status dot — minimal overlay, auto-hide after 3s
- WebRTC connection failure: show snapshot fallback with "Live unavailable" badge — never show blank
- Pre-warm indicator: subtle pulsing border (green) on hovered tile — visual feedback without distraction

### Drag & Status
- Drag visual feedback: semi-transparent ghost tile at 60% opacity + drop zone highlight (dashed border)
- Status indicator: top-right corner dot (6px) overlaid on tile — unobtrusive
- Offline camera: 40% opacity + red dot + "OFFLINE" text centered — clearly distinguished
- Drag reorder: auto-save on drop immediately (PUT to layout API) — no save button needed

### Claude's Discretion
- HTML/CSS architecture (single file, vanilla JS, no build step)
- WebRTC signaling implementation (go2rtc WebSocket protocol)
- DOM element reuse strategy for layout switching (CSS class swap, no DOM rebuild)
- Snapshot refresh mechanism (Image() preload pattern already established)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- cameras.html at crates/rc-sentry-ai/cameras.html — current 150-line version with snapshot grid
- go2rtc WebRTC: ws://192.168.31.27:1984/api/ws?src={stream_name} for signaling
- /api/v1/cameras — returns 8-field metadata (name, display_name, display_order, role, zone, nvr_channel, stream_url, status)
- /api/v1/cameras/layout — GET/PUT for grid_mode, camera_order, zone_filter
- /api/v1/cameras/nvr/:channel/snapshot — cached JPEG proxy (~1ms response)

### Established Patterns
- Brand: Racing Red #E10600, Asphalt Black #1A1A1A, Card #222222, Border #333333
- Font: Montserrat (body) — already loaded in current cameras.html
- DOM construction: createElement, not innerHTML (security hook requirement)
- Snapshot refresh: Image() preload with cache-busting ?t=Date.now()

### Integration Points
- include_str!("../cameras.html") in mjpeg.rs — file must stay in crate root
- go2rtc at :1984 — WebRTC signaling via WebSocket
- rc-sentry-ai at :8096 — snapshot proxy + camera API + layout API
- Stream names: ch1-ch13 for WebRTC, NVR channels 1-13 for snapshots

</code_context>

<specifics>
## Specific Ideas

- Inspired by DMSS HD: smooth camera switching, clean grid, professional feel
- Layout modes: 1×1, 2×2, 3×3, 4×4 with toolbar icon buttons
- Single WebRTC connection at a time — teardown on camera switch via singleton pattern
- Hover pre-warm: start WebRTC negotiation after 500ms hover to reduce cold-start
- All cameras always visible (scroll if needed) — never hide cameras from staff
- Zone grouping: entrance, reception, pods, other — collapsible headers

</specifics>

<deferred>
## Deferred Ideas

- PTZ controls — not supported by current Dahua cameras in this venue
- Recording playback timeline — separate feature, not part of live dashboard
- Multi-monitor pop-out — future enhancement

</deferred>
