# Feature Research: v16.1 Camera Dashboard Pro — Professional NVR Dashboard

**Domain:** Professional NVR camera dashboard (web, venue operations, 13x Dahua cameras)
**Researched:** 2026-03-22
**Confidence:** HIGH (codebase audit of existing cameras/page.tsx, nvr.rs, config.rs, mjpeg.rs; competitor analysis of DMSS HD, Hik-Connect, Frigate, camera.ui, Blue Iris; go2rtc documentation)

---

## Context: What Already Exists (Do Not Re-Build)

| Already Shipped in v16.0 | Net New for v16.1 |
|--------------------------|-------------------|
| 13-camera snapshot grid at `/cameras/live` | Layout mode switcher (1x1, 2x2, 3x3, 4x4) |
| Auto-refreshing snapshot cache in rc-sentry-ai | WebRTC fullscreen via go2rtc (single camera, sub-second latency) |
| NVR digest auth proxy (`NvrClient` in nvr.rs) | Camera friendly names (display labels, not stream names) |
| MJPEG streaming for 3 cameras (entrance, reception, reception_wide) | Drag-to-rearrange grid order with persistence |
| Click-to-fullscreen (basic, shows cached snapshot) | Preference persistence (localStorage JSON for layout + order + names) |
| `CameraConfig` in TOML (name, stream_name, role, fps, nvr_channel) | All 13 cameras added to go2rtc |
| `/api/v1/cameras` endpoint returning CameraInfo[] | Fullscreen WebRTC upgrade on click |
| Dual deployment: rc-sentry-ai (:8096) + server web dashboard | Standalone dashboard page accessible from server |

The existing snapshot grid is the foundation. v16.1 layers professional NVR UX on top of it without replacing the snapshot cache approach (which correctly handles 13 cameras efficiently).

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features that any professional NVR dashboard has. Missing these makes the dashboard feel like a prototype.

| # | Feature | Why Expected | Complexity | Dependency on Existing |
|---|---------|--------------|------------|------------------------|
| TS-1 | **Layout mode switcher: 1x1, 2x2, 3x3, 4x4** | DMSS HD, Hik-Connect, Blue Iris all offer selectable grid layouts as the primary navigation paradigm. Staff expect to choose between overview (4x4, all cameras) and focused view (2x2, specific area). This is the first thing a user looks for on any NVR dashboard. | LOW | Grid is already CSS. Add a layout selector UI component that changes the `grid-cols-N` Tailwind class. State managed in React. |
| TS-2 | **Camera friendly names (display labels)** | "cam_entrance_01" is unreadable on an operational dashboard. Every professional NVR (Dahua NVR OSD, Hikvision, Blue Iris) shows human-readable location names. Staff need "Entrance", "Cashier", "Pod Area 1" — not stream names. | LOW | `CameraConfig` already has `name` field. Add optional `display_name` to TOML. Fall back to `name` if not set. Frontend reads display_name from API response. No backend database change needed. |
| TS-3 | **Click-to-fullscreen with WebRTC live video** | The existing click-to-fullscreen shows a cached snapshot — this is not fullscreen in any professional sense. DMSS HD, Hik-Connect, and Frigate all switch to a live stream when you open a camera fullscreen. Staff opening a camera in fullscreen expect real-time video, not a photo. | MEDIUM | go2rtc already running with 3 cameras. Embed `<iframe src="http://192.168.31.27:1984/stream.html?src={stream_name}&mode=webrtc">` in fullscreen modal. Must add all 13 cameras to go2rtc config. |
| TS-4 | **Camera online/offline status indicators** | Status dots already exist in the UI but are based on snapshot fetch success — not a dedicated health check. Every professional NVR shows per-camera connectivity status. If a camera is offline, staff need to know immediately without trying to view it. | LOW | Status already in `CameraInfo.status` from backend. UI already shows colored dots. This is table stakes because it must be reliable — the dot must reflect actual camera state, not just "did the last snapshot fetch work". |
| TS-5 | **Persistent layout preference (survives page reload)** | Hik-Connect and Blue Iris remember the last layout mode the user selected. A dashboard that resets to default every refresh forces staff to reconfigure on every load. Layout choice must persist in localStorage. | LOW | Pure frontend. `useEffect` to read/write `localStorage.setItem('rp_camera_layout', mode)`. Hydration-safe (read in useEffect, not useState initializer — per CLAUDE.md rules). |
| TS-6 | **Loading state and error fallback per camera** | Professional NVR dashboards show a placeholder (dark tile with camera name) when a camera is loading or fails. The current UI shows nothing or blank while snapshots load. Staff need to distinguish "loading" from "offline" from "working". | LOW | Already partially implemented with `isOffline()` check. Extend to show skeleton placeholder during initial snapshot fetch and distinguish timeout/error states. |
| TS-7 | **Camera count and status summary in header** | DMSS HD and Frigate both show "12/13 cameras online" or equivalent in the dashboard header. Staff need at-a-glance venue health without counting dots. | LOW | Compute from `cameras.filter(c => c.status === 'connected').length` on the frontend. No backend change. |

### Differentiators (Competitive Advantage)

Features that make this dashboard notably better than generic NVR UIs for the specific venue context.

| # | Feature | Value Proposition | Complexity | Notes |
|---|---------|-------------------|------------|-------|
| D-1 | **Drag-to-rearrange camera grid order** | camera.ui offers this; DMSS HD and Hik-Connect require settings menus instead. Drag-to-rearrange lets staff organize cameras logically (entrance cameras together, pod area cameras together) without editing config files. Persisted to localStorage so order survives reload. | MEDIUM | Use `@dnd-kit/core` (lightweight, no jQuery dependency). Each camera tile is a draggable item. On drag end, update `cameraOrder` array in state and persist to `localStorage.setItem('rp_camera_order', JSON.stringify(order))`. The order array maps display positions to camera names. |
| D-2 | **Hybrid streaming: snapshot grid + WebRTC on demand** | Frigate pioneered this pattern: show snapshots for all cameras (low bandwidth) and switch to live stream only for the selected camera (high quality). This is the right approach for 13 cameras on a LAN — streaming 13 WebRTC streams simultaneously would saturate the NVR RTSP output. Snapshot grid for overview, WebRTC only when a camera is clicked. | LOW | Architecture already correct. Grid uses cached snapshots (existing). Fullscreen modal loads go2rtc iframe (TS-3). The pattern is the differentiator — document it explicitly so it is not "fixed" to stream all cameras. |
| D-3 | **Camera grouping by location zone** | DMSS HD organizes cameras "by room" (their terminology). For Racing Point, natural zones are: Entrance/Reception, Pod Area, Cashier/Admin, Exterior. Grouping lets staff switch between "show all" and "show pod area only" without manually navigating a layout. | MEDIUM | Add `zone` field to `CameraConfig` in TOML. Frontend groups cameras by zone in the grid header. Layout switcher can optionally filter by zone. No backend API change — zone comes through existing `/api/v1/cameras` endpoint by adding `zone` to `CameraInfo`. |
| D-4 | **Auto-refresh rate control** | The current dashboard has a single Refresh button. DMSS HD supports configurable snapshot refresh intervals. For an operational venue dashboard, staff want fast refresh (5s) during incidents and slower refresh (30s) during normal ops. Reduce NVR load and browser tab CPU with a rate slider or preset buttons. | LOW | Frontend-only. Replace fixed interval with configurable one. Persist choice in localStorage. Presets: "5s (incident)", "15s (active)", "30s (normal)". |
| D-5 | **Snapshot timestamp overlay on each tile** | Professional NVR dashboards show when the snapshot was last captured on each tile. This lets staff confirm the feed is live and current. Especially important when a camera appears online but the snapshot is stale. | LOW | `CameraInfo` currently lacks `last_snapshot_at`. Add this field to the backend response (track timestamp when snapshot was cached). Frontend renders "3s ago" overlay on each tile. |
| D-6 | **Single-camera focused view (1x1 large)** | Hik-Connect supports tapping a camera to expand it to full page without leaving the dashboard context. Different from fullscreen (which opens a modal/overlay). This "featured camera" mode shows one camera large with the others in a sidebar column. | MEDIUM | Add a "feature" click action (distinct from fullscreen click). Selected camera renders in main area (3/4 width), remaining cameras in compact sidebar (1/4 width). No streaming change needed — still uses snapshots. |

### Anti-Features (Do Not Build)

| Anti-Feature | Why Requested | Why Problematic | Alternative |
|--------------|---------------|-----------------|-------------|
| **Simultaneous WebRTC streams for all 13 cameras** | "Shouldn't the live view show live video for all cameras?" | The Dahua NVR supports limited simultaneous RTSP connections. 13 concurrent WebRTC streams via go2rtc saturates the NVR RTSP output, causes frame drops across all streams, and spikes browser CPU to unusable levels. Frigate explicitly avoids this with smart streaming. | Hybrid approach: cached snapshots for grid (low bandwidth, low NVR load), WebRTC only for the single camera being viewed in fullscreen. |
| **RTSP direct browser streaming** | "Skip go2rtc, just serve RTSP in the browser" | Browsers cannot play RTSP natively. Embedding RTSP requires a VLC plugin or browser extension that is not installable on venue machines. | go2rtc already handles RTSP-to-WebRTC transcoding. Use go2rtc's iframe embed approach (TS-3). |
| **Recording playback from NVR in the dashboard** | "Staff should be able to review footage from the dashboard" | NVR playback from browser requires either raw DAV format support (browser cannot play .dav files without codec), or re-encoding on the fly (high CPU). The NVR API for file search (`nvr.rs` `search_files()`) is already implemented but playback is a separate milestone concern. `playback/page.tsx` already exists as a placeholder. | Keep playback in the existing `/cameras/playback` page. Do not mix it into the live dashboard. |
| **Motion detection alerts in the dashboard** | "Show a red border when motion is detected" | Motion detection from the Dahua NVR requires a separate event stream subscription (TCP push or AMQP), not available via the existing HTTP CGI interface. Implementing this would require a new persistent NVR connection and significant backend work. | The existing attendance/detection engine in rc-sentry-ai handles motion-adjacent events. Alerts are delivered via the existing alert WebSocket. Do not re-implement in the camera dashboard. |
| **Camera PTZ controls in the dashboard** | "Add pan/tilt/zoom buttons" | None of the 13 Dahua cameras at Racing Point are PTZ cameras. Building PTZ UI for fixed cameras is waste. | No alternative needed — not applicable. |
| **User authentication per-camera (role-based access)** | "Cashier camera should only be visible to managers" | The dashboard is an internal-LAN staff tool on the server web dashboard (port 3200). All staff who access the dashboard have full access. Adding per-camera RBAC requires auth middleware, user roles, and session management — a separate milestone. | Operate under the existing server dashboard auth model (LAN-only access = implicit trust). |
| **Cloud-accessible camera dashboard** | "Uday wants to view cameras from his phone remotely" | Serving live camera streams through the cloud VPS introduces: (a) bandwidth costs for video, (b) latency from India to VPS and back, (c) NVR credential exposure risk if the proxy is misconfigured. | DMSS HD app on Uday's phone connects directly to the Dahua NVR via P2P cloud relay — this is the correct solution for remote access and requires zero custom code. |

---

## Feature Dependencies

```
TS-1 (layout mode switcher)
    └──requires──> TS-5 (layout persistence: persist the selected mode)
    └──enables──> D-3 (zone grouping: zone filter changes the camera set shown in grid)

TS-3 (WebRTC fullscreen via go2rtc)
    └──requires──> go2rtc configured with all 13 cameras (infra task, not code)
    └──requires──> go2rtc accessible from browser at :1984
    └──enables──> D-2 (hybrid streaming: TS-3 is the fullscreen half of the hybrid pair)

TS-5 (layout persistence)
    └──required by──> TS-1 (layout mode must be persisted)
    └──required by──> D-1 (camera order must be persisted after drag)
    └──pattern shared with──> D-4 (refresh rate persistence uses same localStorage pattern)

D-1 (drag-to-rearrange)
    └──requires──> TS-5 (persist reordered camera positions)
    └──requires──> @dnd-kit/core installed (new npm dependency)
    └──independent from──> TS-3 (WebRTC fullscreen — drag affects grid, not fullscreen)

TS-2 (camera friendly names)
    └──requires──> TOML config change: add display_name field to CameraConfig
    └──requires──> API change: include display_name in CameraInfo response
    └──enables──> D-3 (zone grouping: zone is also a new TOML field alongside display_name)

D-5 (snapshot timestamp overlay)
    └──requires──> backend change: track and expose last_snapshot_at in CameraInfo
    └──independent from──> layout features
```

### Dependency Notes

- **go2rtc camera registration is the infrastructure gate for TS-3**: All 13 cameras must be added to go2rtc's YAML/TOML config before WebRTC fullscreen can work. This is config work, not code. The 3 existing cameras (entrance, reception, reception_wide) are already registered.
- **TS-1, TS-5, D-1 form a cohesive "layout management" cluster**: Build them in the same phase. TS-5 (persistence) is shared infrastructure for all three.
- **TS-2 (friendly names) and D-3 (zones) share the same TOML + API change**: If building D-3, do TS-2 in the same phase since the backend change is identical (add fields to CameraConfig, expose in CameraInfo).
- **D-1 (drag-to-rearrange) is the only new npm dependency**: `@dnd-kit/core` is the correct library (lightweight, accessible, no jQuery). Avoid `react-beautiful-dnd` (deprecated by Atlassian) and `react-sortable-hoc` (unmaintained).
- **WebRTC works on LAN without TURN**: go2rtc's built-in WebRTC relay handles NAT traversal within the LAN. No TURN server needed. This is confirmed by Frigate's go2rtc integration docs.

---

## MVP Definition

### Launch With (v16.1 core)

The minimum that transforms the basic grid into a dashboard Uday would show off to customers.

- [ ] **TS-1** — Layout mode switcher: 1x1, 2x2, 3x3, 4x4 toggle buttons in the header bar
- [ ] **TS-2** — Camera friendly names: `display_name` in TOML, shown in tile header
- [ ] **TS-3** — WebRTC fullscreen: click camera tile, modal opens with go2rtc iframe
- [ ] **TS-5** — Persist layout choice to localStorage (survives page reload)
- [ ] **TS-6** — Per-tile loading skeleton and offline/error state distinction
- [ ] **TS-7** — Camera count summary in header ("12/13 online")
- [ ] Go2rtc camera registration: all 13 cameras added to go2rtc config (infra prerequisite)

### Add After Validation (v16.1.x)

Once the core is working and staff are using it:

- [ ] **D-1** — Drag-to-rearrange with localStorage persistence — trigger: staff ask why cameras are in wrong order
- [ ] **D-4** — Refresh rate control (5s/15s/30s) — trigger: staff complain about CPU or want faster refresh during incidents
- [ ] **D-5** — Snapshot timestamp overlay — trigger: staff unsure if snapshot is current
- [ ] **TS-4** reliability improvement — health check separate from snapshot fetch

### Future Consideration (v16.2+)

- [ ] **D-3** — Zone grouping — defer: requires TOML config design decision + more cameras
- [ ] **D-6** — Single-camera featured view — defer: nice UX but grid + fullscreen covers the core need
- [ ] NVR playback integration — separate milestone (`/cameras/playback` page already stubbed)

---

## Feature Prioritization Matrix

| Feature | Staff Value | Implementation Cost | Priority |
|---------|-------------|---------------------|----------|
| TS-1 Layout switcher | HIGH — first thing staff look for | LOW — CSS grid change + React state | P1 |
| TS-2 Friendly names | HIGH — names are unreadable now | LOW — TOML + API field addition | P1 |
| TS-3 WebRTC fullscreen | HIGH — existing fullscreen shows a photo | MEDIUM — go2rtc config + iframe modal | P1 |
| TS-5 Layout persistence | HIGH — resets on every reload | LOW — localStorage in useEffect | P1 |
| TS-6 Loading/error states | MEDIUM — UX polish | LOW — React state distinction | P1 |
| TS-7 Status summary | MEDIUM — at-a-glance health | LOW — frontend count from array | P1 |
| Go2rtc: all 13 cameras | HIGH — prerequisite for TS-3 | LOW — config only, no code | P1 |
| D-1 Drag-to-rearrange | MEDIUM — organization | MEDIUM — @dnd-kit dependency | P2 |
| D-4 Refresh rate control | MEDIUM — bandwidth + CPU | LOW — frontend interval | P2 |
| D-5 Timestamp overlay | MEDIUM — freshness confidence | LOW-MEDIUM — backend field + frontend | P2 |
| D-2 Hybrid streaming | HIGH — architecture decision | LOW — the approach, not a feature | P1 (documented pattern) |
| D-3 Zone grouping | LOW-MEDIUM — venue-specific | MEDIUM — TOML design + frontend | P3 |
| D-6 Featured camera view | LOW — fullscreen covers it | MEDIUM — layout restructure | P3 |

**Priority key:**
- P1: Must have for v16.1 launch
- P2: Should have, add when P1 is stable
- P3: Nice to have, future consideration

---

## Competitor Feature Analysis

| Feature | DMSS HD (Dahua) | Hik-Connect (Hikvision) | Frigate NVR | Blue Iris | Our Approach |
|---------|-----------------|------------------------|-------------|-----------|--------------|
| Layout modes | 1/4/9/16-split | 1/4/9/12-split, tap icon to switch | Single camera + "All Cameras" groups | 1:6, 1:9, 4-up, drag+drop | 1x1, 2x2, 3x3, 4x4 toggle buttons |
| Camera naming | OSD from NVR config | OSD from NVR config | YAML config name field | Per-camera label in settings | `display_name` in rc-sentry-ai TOML |
| Drag to rearrange | Yes (press+hold) | Yes (favorites group) | No (config-defined order) | Yes (drag+drop in layout) | @dnd-kit/core, localStorage |
| Live streaming | RTSP → H264 P2P | RTSP → H264 P2P | WebRTC via go2rtc, fallback jsmpeg | H264 direct to browser | WebRTC via go2rtc on fullscreen only |
| Fullscreen click | Opens full stream | Opens full stream | Opens full stream + detection overlay | Opens full stream | Opens go2rtc WebRTC iframe modal |
| Snapshot grid | Not applicable (always live) | Not applicable | Smart streaming: 1fps when idle | Motion-triggered | Cached snapshots from NVR, background refresh |
| Layout persistence | Yes (app settings) | Yes (app settings) | Yes (camera groups config) | Yes (profile save) | localStorage JSON |
| Remote access | Dahua P2P cloud | Hikvision cloud | Tailscale/VPN required | Blue Iris cloud | Not in scope — use DMSS HD app directly |

**Key insight from competitor analysis:** DMSS HD and Hik-Connect target remote mobile access; their architecture is fundamentally cloud-relayed. Frigate and Blue Iris are the closer reference points — they are LAN-first dashboards like ours. Frigate's hybrid streaming (snapshot at idle, WebRTC on click) is exactly the right model for 13 cameras on a single NVR. Blue Iris's drag-to-rearrange approach is the right UX for camera order management.

---

## Technical Integration Notes

### go2rtc Embed Pattern (for TS-3)

go2rtc provides a built-in stream page that works as an iframe src:

```
http://192.168.31.27:1984/stream.html?src={stream_name}&mode=webrtc
```

The go2rtc JavaScript player library (`webrtc-player.js`) is also available for custom integration if the iframe approach is too restrictive. The iframe approach is simpler and correct for fullscreen modal use.

go2rtc confirmed: ~0.5s latency on LAN over WebRTC. No TURN server needed for same-LAN clients.

### Preference Persistence Pattern (for TS-5, D-1, D-4)

Per CLAUDE.md rules: never read `localStorage` in `useState` initializer (SSR fails). Use:

```typescript
const [layout, setLayout] = useState<Layout>('3x3'); // default
useEffect(() => {
  const saved = localStorage.getItem('rp_camera_layout');
  if (saved) setLayout(saved as Layout);
}, []);
```

Single JSON blob for all preferences: `rp_camera_prefs: { layout, order, refreshRate }` to avoid fragmentation.

### Snapshot Freshness (for D-5)

The rc-sentry-ai snapshot cache currently does not expose `last_snapshot_at` in the `/api/v1/cameras` API response. The `CameraInfo` struct needs a new field. Backend change is small: track `Instant::now()` when a snapshot is successfully cached; serialize as ISO 8601 in the response.

---

## Sources

- **Codebase audit (HIGH confidence):**
  - `web/src/app/cameras/page.tsx` — existing grid UI, snapshot fetch from rc-sentry-ai :8096
  - `crates/rc-sentry-ai/src/nvr.rs` — `NvrClient`, digest auth proxy, snapshot endpoint
  - `crates/rc-sentry-ai/src/config.rs` — `CameraConfig` (name, stream_name, role, fps, nvr_channel), `NvrConfig`
  - `.planning/PROJECT.md` — v16.1 target features, constraints (go2rtc relay, no TURN server, TOML config)

- **Competitor analysis (MEDIUM confidence — WebSearch + WebFetch):**
  - DMSS HD: multi-window live view, press+hold drag, 1/4/9/16 layouts — [Google Play listing](https://play.google.com/store/apps/details?id=com.mm.android.DMSSHD&hl=en), [Dahua Wiki](https://dahuawiki.com/DMSS)
  - Hik-Connect: drag-to-rearrange in favorites, 1/4/9/12 layouts, favorites group for cross-NVR organization — [Hikvision support](https://supportusa.hikvision.com/support/solutions/articles/17000129177)
  - Frigate NVR: smart streaming (1fps idle → WebRTC on click), camera groups, go2rtc integration — [Frigate live docs](https://docs.frigate.video/configuration/live/)
  - Blue Iris: drag+drop layouts, 64-camera support, dark mode UI — [Blue Iris software](https://blueirissoftware.com/)
  - camera.ui: Camview drag+drop, tile-based minimalist layout, fullscreen per tile — [GitHub](https://github.com/seydx/camera.ui)

- **go2rtc integration (HIGH confidence — official repo + live docs):**
  - WebRTC via go2rtc: ~0.5s latency LAN, no TURN needed, iframe embed via `stream.html?src=NAME&mode=webrtc` — [go2rtc GitHub](https://github.com/AlexxIT/go2rtc)
  - Frigate's go2rtc guide confirms iframe embed pattern — [Frigate go2rtc guide](https://docs.frigate.video/guides/configuring_go2rtc/)

- **Frontend patterns (HIGH confidence — CLAUDE.md rules):**
  - localStorage hydration pattern from CLAUDE.md: useEffect + hydrated flag, never in useState initializer
  - @dnd-kit/core recommended over react-beautiful-dnd (deprecated) and react-sortable-hoc (unmaintained)

---

*Feature research for: v16.1 Camera Dashboard Pro — Professional NVR Dashboard*
*Researched: 2026-03-22*
