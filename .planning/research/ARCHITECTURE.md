# Architecture Research

**Domain:** Hybrid streaming NVR dashboard — v16.1 Camera Dashboard Pro
**Researched:** 2026-03-22
**Confidence:** HIGH (all integration points verified against live code and go2rtc source)

---

## System Overview

```
Browser (cameras.html @ :8096  OR  web dashboard @ :3200)
  │
  │  Snapshot poll (HTTP GET, ~1-2 fps, instant from cache)
  ├──────────────────────────────────────────────────────────────────►
  │                                              rc-sentry-ai :8096
  │  ◄── JPEG bytes (SnapshotCache, no NVR wait per request)
  │
  │  WebRTC fullscreen (on camera click)
  │  ws://192.168.31.27:1984/api/ws?src=<stream_name>
  ├──────────────────────────────────────────────────────────────────►
  │                                              go2rtc :1984
  │  ◄── webrtc/offer → webrtc/answer → webrtc/candidate exchange
  │      RTCPeerConnection established, H.265 video streams directly
  │
  │  Camera names + stream map
  ├──────────────────────────────────────────────────────────────────►
  │                              GET /api/v1/cameras (rc-sentry-ai)
  │                              Returns name, nvr_channel, stream_name, display_order
  │
  │  Layout state (grid size + camera order)
  └──────────────────────────────────────────────────────────────────►
                                 localStorage (browser-side, no server round-trip)

NVR 192.168.31.18 (Dahua, 13 cameras, H.265)
  │
  ├── RTSP ch1-ch13 ──────────────────────────────────► go2rtc :1984
  │                                                      (needs 13 entries in go2rtc.yaml)
  │
  └── HTTP CGI snapshot.cgi ──────────────────────────► NvrClient (rc-sentry-ai)
                                                         → SnapshotCache (background fetch)
```

### Component Responsibilities

| Component | Responsibility | Status |
|-----------|----------------|--------|
| `SnapshotCache` | RwLock<HashMap> — fetches all 13 NVR channels sequentially ~200ms/cycle, serves cached JPEGs instantly | Exists — no changes needed |
| `NvrClient` | reqwest + digest auth with nonce caching — `snapshot.cgi` and playback CGI | Exists — no changes needed |
| `mjpeg_router` | HTTP routes: `/cameras/live`, `/api/v1/cameras`, `/api/v1/cameras/nvr/:channel/snapshot` | Exists — extend response shape only |
| `cameras.html` | Embedded via `include_str!`, current snapshot-only grid | Exists — full rewrite in-place |
| `go2rtc` | RTSP relay + WebRTC signaling — currently 3 cameras configured | Exists — needs ch1-ch13 added to yaml |
| `web/src/app/cameras/page.tsx` | Next.js staff dashboard cameras page (server :3200) | Exists — full rewrite |
| Camera display order | `display_order` field in rc-sentry-ai.toml per camera | New — add to `CameraConfig` |
| Layout state | Grid size + drag order in user's browser | New — localStorage only |

---

## WebRTC Integration: How go2rtc Works with the Browser

This is the critical integration point. The go2rtc WebRTC protocol is verified against go2rtc source code (video-rtc.js, deepwiki WebRTC protocol documentation).

### go2rtc WebRTC Signaling Protocol

go2rtc exposes a WebSocket at `/api/ws?src=<stream_name>` on port 1984. The browser does a standard WebRTC offer/answer exchange over this WebSocket.

**Message types exchanged over the WebSocket:**

| Direction | Message | Shape |
|-----------|---------|-------|
| Browser → go2rtc | Offer | `{ type: 'webrtc/offer', value: sdp_string }` |
| go2rtc → Browser | Answer | `{ type: 'webrtc/answer', value: sdp_string }` |
| Browser → go2rtc | ICE candidate | `{ type: 'webrtc/candidate', value: candidate_string }` |
| go2rtc → Browser | ICE candidate | `{ type: 'webrtc/candidate', value: candidate_string }` |

**Browser implementation pattern:**

```javascript
async function openWebRTC(streamName, videoEl) {
  const pc = new RTCPeerConnection();
  pc.addTransceiver('video', { direction: 'recvonly' });

  const ws = new WebSocket(
    `ws://192.168.31.27:1984/api/ws?src=${streamName}`
  );

  pc.onicecandidate = (e) => {
    if (e.candidate && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({
        type: 'webrtc/candidate',
        value: e.candidate.candidate
      }));
    }
  };

  pc.ontrack = (e) => {
    videoEl.srcObject = e.streams[0];
  };

  ws.onopen = async () => {
    const offer = await pc.createOffer();
    await pc.setLocalDescription(offer);
    ws.send(JSON.stringify({ type: 'webrtc/offer', value: offer.sdp }));
  };

  ws.onmessage = async (e) => {
    const msg = JSON.parse(e.data);
    if (msg.type === 'webrtc/answer') {
      await pc.setRemoteDescription({ type: 'answer', sdp: msg.value });
    } else if (msg.type === 'webrtc/candidate') {
      await pc.addIceCandidate({ candidate: msg.value, sdpMid: '0' });
    }
  };

  // Return cleanup function
  return () => { pc.close(); ws.close(); };
}
```

**Teardown on fullscreen close:** call `pc.close()` and `ws.close()`. go2rtc releases the consumer slot. The RTSP session stays open (go2rtc keeps it open for future viewers).

### Stream Name Convention

go2rtc.yaml defines stream names as keys. The browser passes this key as `src=<name>` in the WebSocket URL. Convention for v16.1: use `ch1` through `ch13` for NVR channels. The mapping from NVR channel number to go2rtc stream name must be returned by the `/api/v1/cameras` API so the browser never hardcodes it.

### Codec: H.265 Direct Relay

The Dahua NVR streams H.265 (HEVC). go2rtc can relay H.265 directly to WebRTC without transcoding. This works on Chrome 136+, Safari 18+. Since the staff dashboard runs on a controlled LAN machine, H.265 direct relay is the correct approach — no ffmpeg transcoding needed.

The existing go2rtc.yaml already uses ffmpeg transcoding to H.264 for the detection pipeline (`entrance_h264` etc.) because openh264 only decodes H.264. Those entries stay as-is. The new ch1-ch13 entries for the dashboard do NOT transcode — they relay H.265 directly.

### CORS and LAN Access

go2rtc's WebSocket endpoint on port 1984 is accessible directly from the browser on LAN. No proxy through Axum is needed or desirable. go2rtc is permissive on LAN CORS by default. The `mjpeg_router` in rc-sentry-ai already has `CorsLayer` for the snapshot API — that is independent of go2rtc.

---

## Layout State Persistence

### Grid Size and Camera Order: localStorage

Layout is browser-side UI state. It does not need server persistence.

**Storage shape:**

```javascript
const LAYOUT_KEY = 'rp_camera_layout_v1';

// Stored in localStorage
{
  "gridSize": "2x2",            // "1x1" | "2x2" | "3x3" | "4x4"
  "cameraOrder": [1, 3, 2, 4]  // NVR channel numbers in display order
}
```

**Why not server-side:** cameras.html and the web dashboard at :3200 are separate browser sessions, potentially used by different staff. Shared server-side layout would create coupling with no operational benefit. If shared layout is ever needed, add `POST /api/v1/cameras/layout` then — do not over-engineer now.

**Next.js hydration rule (mandatory — existing codebase rule):** Never initialize state from localStorage in `useState` — use `useEffect` + hydrated flag:

```typescript
const [layout, setLayout] = useState<LayoutState>(DEFAULT_LAYOUT);
const [hydrated, setHydrated] = useState(false);
useEffect(() => {
  const saved = localStorage.getItem(LAYOUT_KEY);
  if (saved) setLayout(JSON.parse(saved));
  setHydrated(true);
}, []);
if (!hydrated) return <CameraGridSkeleton />;
```

### Camera Names and Display Order: rc-sentry-ai.toml

Camera names ("Entrance", "Pod Area", "Cashier") are venue constants, not user preferences. They belong in the TOML config alongside `nvr_channel` and `stream_name`. They survive browser cache clears and are consistent across all viewers.

**Add `display_order` to existing `CameraConfig`:**

```toml
[[cameras]]
name = "Entrance"
stream_name = "ch1"
nvr_channel = 1
role = "entrance"
fps = 5
display_order = 1     # New field — default grid position
```

The `cameras_list_handler` JSON response needs to include `nvr_channel`, `stream_name`, and `display_order`. The browser reads this on page load to build its initial grid order before checking localStorage.

---

## Dual Deployment Architecture

### Deployment 1: cameras.html (Embedded in rc-sentry-ai)

**URL:** `http://192.168.31.27:8096/cameras/live`

The `cameras_page_handler` in `mjpeg.rs` serves `cameras.html` via `include_str!`. Recompile rc-sentry-ai when the HTML changes. No new Axum routes needed.

**Constraints:**
- Single HTML file, no build step. Vanilla JavaScript only — no bundler, no npm.
- Snapshot requests go to `/api/v1/cameras/nvr/:channel/snapshot` — same origin, no CORS.
- WebRTC signaling goes to `ws://192.168.31.27:1984/api/ws?src=ch{N}` — cross-origin from the browser's perspective, but go2rtc is permissive.
- Camera config from `GET /api/v1/cameras` at page load — hardcoded base URL is fine (`http://192.168.31.27:8096`).

### Deployment 2: Next.js Web Dashboard (Server at :3200)

**URL:** `http://192.168.31.23:3200/cameras`

`web/src/app/cameras/page.tsx` is a full rewrite. The existing page fetches camera list via MJPEG stream URLs — the new page uses snapshot polling + WebRTC.

**Constraints:**
- `SENTRY_BASE` (`http://192.168.31.27:8096`) must be a `NEXT_PUBLIC_` env var baked at build time, not hardcoded in source.
- `GO2RTC_WS_BASE` (`ws://192.168.31.27:1984`) same rule.
- WebRTC goes browser → go2rtc directly (no Next.js proxy). This is a direct WebSocket from the browser to James's machine — works on LAN.
- Rebuild Next.js after changing env vars. Copy `.next/static` into `.next/standalone/` before deploying (mandatory standing rule).

### Feature Parity

| Feature | cameras.html | web/cameras/page.tsx |
|---------|-------------|----------------------|
| Snapshot grid (13 cameras) | Yes — vanilla JS `setInterval` | Yes — React `useEffect` + `setInterval` |
| Layout selector (1x1/2x2/3x3/4x4) | Yes — CSS grid-template-columns | Yes — Tailwind grid-cols-N |
| Camera names from API | Yes | Yes |
| Drag-to-reorder | Yes — HTML5 drag events | Yes — React drag state |
| WebRTC fullscreen | Yes — vanilla RTCPeerConnection | Yes — `useWebRTC` hook + `<video>` ref |
| Layout persistence | localStorage | localStorage |
| DashboardLayout chrome | No | Yes — existing wrapper |

Both deployments are functionally equivalent for camera operations.

---

## New vs Modified Components

### New Components

| Component | Location | What It Does |
|-----------|----------|-------------|
| go2rtc yaml entries ch1-ch13 | `C:\RacingPoint\go2rtc\go2rtc.yaml` | RTSP relay for all 13 NVR channels, enabling WebRTC access |
| WebRTC connection helper | `cameras.html` (inline JS) | Creates RTCPeerConnection, handles go2rtc WebSocket signaling, attaches stream to `<video>` |
| Layout selector UI | `cameras.html` + `web/cameras/page.tsx` | Buttons for 1x1/2x2/3x3/4x4 — changes CSS grid layout |
| Fullscreen WebRTC modal | Both | `<video>` overlay on camera click, RTCPeerConnection lifecycle |
| Drag reorder | Both | Reorders NVR channel display array, persists to localStorage |
| `useWebRTC` hook | `web/src/hooks/useWebRTC.ts` | React hook — RTCPeerConnection + go2rtc WebSocket, returns `videoRef` |
| `CameraGridSkeleton` | `web/src/components/` | Loading skeleton for hydration gap |

### Modified Components

| Component | Location | What Changes |
|-----------|----------|-------------|
| `cameras.html` | `crates/rc-sentry-ai/cameras.html` | Full rewrite — layout controls, drag, WebRTC fullscreen replaces snapshot-only grid |
| `web/cameras/page.tsx` | `web/src/app/cameras/page.tsx` | Full rewrite — snapshot polling, layout controls, drag, WebRTC fullscreen |
| `CameraConfig` | `crates/rc-sentry-ai/src/config.rs` | Add `display_order: Option<u32>` field |
| `cameras_list_handler` | `crates/rc-sentry-ai/src/mjpeg.rs` | Extend JSON response: add `nvr_channel`, `stream_name`, `display_order` |
| `rc-sentry-ai.toml` | `C:\RacingPoint\rc-sentry-ai.toml` | Add all 13 cameras with `stream_name = "ch1"` through `"ch13"`, `display_order` |
| `go2rtc.yaml` | `C:\RacingPoint\go2rtc\go2rtc.yaml` | Add ch1-ch13 pointing to NVR RTSP URLs |

**What does NOT change:**
- `SnapshotCache` — no modifications, works correctly as-is
- `NvrClient` — no modifications, snapshot.cgi fetch works for all 13 channels already
- `spawn_snapshot_fetcher` — already iterates `1..=nvr_channels`, no changes
- `nvr_snapshot_handler` — works as-is, already serves cached JPEG by channel number
- All Axum routing in `main.rs` — no new routes needed

---

## Data Flow

### Snapshot Grid Flow (polling, all 13 cameras)

```
Browser page load
  └── GET /api/v1/cameras
        └── cameras_list_handler → CameraConfig vec → JSON
              Returns: [{ name, role, nvr_channel, stream_name, display_order, status }]
              Browser builds grid in display_order, checks localStorage for drag overrides

Browser setInterval (~1000ms per camera, staggered)
  └── GET /api/v1/cameras/nvr/1/snapshot?t=<timestamp>
        └── nvr_snapshot_handler → SnapshotCache.get(1) → JPEG bytes → img.src

Background (always running in rc-sentry-ai)
  └── spawn_snapshot_fetcher: loop ch 1..13
        └── NvrClient.snapshot(ch) → Dahua CGI → JPEG → SnapshotCache.set(ch, bytes)
              ~200ms between channels, all 13 updated every ~2-3 seconds
```

### WebRTC Fullscreen Flow (single camera, on click)

```
User clicks camera tile (ch = NVR channel, name = go2rtc stream_name)
  │
  ├── Show fullscreen overlay (<video> element)
  ├── new RTCPeerConnection()
  ├── pc.addTransceiver('video', { direction: 'recvonly' })
  ├── offer = await pc.createOffer()
  ├── await pc.setLocalDescription(offer)
  │
  └── new WebSocket(`ws://192.168.31.27:1984/api/ws?src=${name}`)
        │
        ├── ws.onopen → ws.send({ type: 'webrtc/offer', value: offer.sdp })
        ├── pc.onicecandidate → ws.send({ type: 'webrtc/candidate', value: c.candidate })
        │
        ├── ws.onmessage (answer) → pc.setRemoteDescription({ type:'answer', sdp: msg.value })
        ├── ws.onmessage (candidate) → pc.addIceCandidate({ candidate: msg.value, sdpMid:'0' })
        │
        └── pc.ontrack → videoEl.srcObject = event.streams[0]
              Video plays — sub-second latency on LAN, H.265 hardware decoded

User closes fullscreen (ESC or click outside)
  ├── pc.close()
  └── ws.close()
```

### Layout Persistence Flow

```
User changes grid size or drags camera
  └── localStorage.setItem('rp_camera_layout_v1', JSON.stringify({
        gridSize: '3x3',
        cameraOrder: [3,1,4,1,5,9,2,6,5]  // ordered NVR channel numbers
      }))

Page load (subsequent visit)
  └── localStorage.getItem('rp_camera_layout_v1')
        └── Restore gridSize → apply CSS grid-template-columns
            Restore cameraOrder → reorder camera tiles
```

---

## Architectural Patterns

### Pattern 1: Hybrid Streaming — Snapshot Grid + On-Demand WebRTC

**What:** Show all 13 cameras as JPEG snapshots for ambient monitoring. Establish WebRTC only for the actively viewed fullscreen camera.

**Why:** 13 simultaneous WebRTC connections would mean 13 concurrent peer connections, 13 DTLS handshakes, 13 ICE negotiations, and 13 hardware video decoders in the browser. On LAN this is feasible in theory but wasteful. The grid view is for ambient monitoring — 1-2 fps JPEG snapshots are sufficient. WebRTC is reserved for the moment a camera needs real-time attention.

**Trade-offs:**
- Grid is not real-time (1-2 fps snapshot delay). Acceptable for security monitoring.
- WebRTC is per-demand — fast startup (~500ms on LAN), no idle resource usage.
- If real-time grid view is ever needed (future milestone), move to WebRTC multistream per cell.

### Pattern 2: go2rtc Stream Name Indirection

**What:** The browser never knows NVR IPs or credentials. It asks `/api/v1/cameras` for the go2rtc stream name, then connects to go2rtc using only that name.

**Why:** NVR credentials (`admin`/`Admin@123`) exist in two places: `rc-sentry-ai.toml` (for snapshot.cgi) and `go2rtc.yaml` (for RTSP relay). The browser sees neither. If NVR IP or password changes, update the config files — not the JavaScript.

### Pattern 3: Single Active WebRTC Connection

**What:** At most one WebRTC connection is active at a time. Opening a new fullscreen camera closes the previous connection.

**Why:** Simple resource management. The cleanup path is one function. No need to track multiple concurrent ICE states. Staff will not need simultaneous multi-camera WebRTC views for v16.1.

---

## Anti-Patterns

### Anti-Pattern 1: WebRTC for All 13 Cameras Simultaneously

**What people do:** Open 13 WebRTC peer connections for a real-time grid view.

**Why it's wrong:** 13 concurrent DTLS handshakes, 13 video decoders in the browser, 13 go2rtc consumer threads. Even if technically feasible, the startup time (all 13 negotiating simultaneously) would be poor UX. Browser hardware decoder limits apply (typically 8-16 H.265 streams).

**Do this instead:** Snapshot polling for grid (JPEG from SnapshotCache — instant, server-cached). WebRTC only for fullscreen single camera.

### Anti-Pattern 2: Proxying WebRTC Signaling Through Axum

**What people do:** Route WebRTC WebSocket messages through the Axum server at :8096 to avoid CORS.

**Why it's wrong:** go2rtc is the WebRTC signaling endpoint. Proxying adds latency and complexity. go2rtc is permissive on LAN. The browser can connect to `ws://192.168.31.27:1984` directly without any proxy.

**Do this instead:** Browser connects directly to go2rtc. Snapshot API uses rc-sentry-ai. Each server handles its own protocol.

### Anti-Pattern 3: Storing Layout in rc-sentry-ai.toml

**What people do:** Add a writable layout endpoint that updates the TOML file at runtime.

**Why it's wrong:** Layout is a UI preference (per-viewer). Writing user preferences into a service config file creates runtime file mutations that conflict with the config-loaded-at-startup model. TOML is a deploy-time artifact.

**Do this instead:** localStorage for grid size and drag order. TOML only for venue constants (names, NVR channels, roles).

### Anti-Pattern 4: go2rtc H.264 Transcoding for Dashboard

**What people do:** Add ffmpeg transcoding entries in go2rtc.yaml for the dashboard cameras (like the existing `entrance_h264` entries for detection).

**Why it's wrong:** The detection pipeline needs H.264 because openh264 only decodes H.264. The browser dashboard has native H.265 hardware decoding (Chrome 136+, Safari 18+). Transcoding wastes CPU, adds latency, and reduces quality.

**Do this instead:** Add ch1-ch13 entries as direct RTSP relay (`rtsp://...@192.168.31.18/...`), not `ffmpeg:...#video=h264`. The existing `entrance_h264` entries stay for the detection pipeline — they are separate streams.

---

## Integration Points Summary

| Integration | From | To | Protocol | Notes |
|-------------|------|-----|----------|-------|
| Camera config | Browser | rc-sentry-ai :8096 | HTTP GET `/api/v1/cameras` | Extend JSON to include `nvr_channel`, `stream_name`, `display_order` |
| Snapshot | Browser | rc-sentry-ai :8096 | HTTP GET `/api/v1/cameras/nvr/:ch/snapshot` | Already exists, cache-backed, instant |
| Dashboard page | Browser | rc-sentry-ai :8096 | HTTP GET `/cameras/live` | `include_str!` cameras.html — full rewrite |
| WebRTC signaling | Browser | go2rtc :1984 | WebSocket `/api/ws?src=ch{N}` | go2rtc built-in — no code changes in go2rtc |
| RTSP relay | go2rtc | NVR :18 | RTSP | go2rtc.yaml — add ch1-ch13 |
| NVR snapshots (server) | rc-sentry-ai | NVR :18 | HTTP + Digest | NvrClient.snapshot(ch) — already works |
| Layout state | Browser | localStorage | — | No server involvement |

---

## Build Order (Phase Dependencies)

```
Phase 1: go2rtc.yaml — add ch1-ch13 (no code, just config)
  Prerequisite for: all WebRTC UI work
  Action: Add 13 RTSP entries to go2rtc.yaml, restart go2rtc
  Verify: open ws://192.168.31.27:1984 in browser, confirm each stream listed

Phase 2: rc-sentry-ai config + API response (prerequisite for both UIs)
  Action:
    1. Add `display_order: Option<u32>` to CameraConfig in config.rs
    2. Extend cameras_list_handler JSON response: add nvr_channel, stream_name, display_order
    3. Update rc-sentry-ai.toml with all 13 cameras, stream_name = "ch1".."ch13"
  Output: Rebuild + restart rc-sentry-ai
  Verify: GET /api/v1/cameras returns all 13 with correct fields

Phase 3a: cameras.html rewrite (after Phase 1 + 2)
  Action: Full HTML/JS rewrite of cameras.html
    - Layout selector buttons → CSS grid-template-columns change
    - Drag-to-reorder → HTML5 dragstart/dragover/drop
    - localStorage save/restore for gridSize + cameraOrder
    - WebRTC fullscreen on camera click
  Output: Recompile rc-sentry-ai (include_str! picks up new HTML)
  Verify: Open /cameras/live, confirm grid, layout change, WebRTC fullscreen

Phase 3b: web dashboard cameras page rewrite (after Phase 1 + 2, parallel with 3a)
  Action: Full rewrite of web/src/app/cameras/page.tsx
    - Add useWebRTC hook in web/src/hooks/useWebRTC.ts
    - Replace MJPEG img src with snapshot polling + WebRTC fullscreen
    - Add NEXT_PUBLIC_SENTRY_URL and NEXT_PUBLIC_GO2RTC_WS_BASE env vars
  Output: Next.js rebuild + copy .next/static to .next/standalone/
  Verify: Open /cameras on web dashboard, confirm identical features to cameras.html
```

Phase 3a and 3b are independent and can proceed in parallel after Phase 2 completes.

---

## Sources

- Live code inspection: `crates/rc-sentry-ai/src/mjpeg.rs` — SnapshotCache, mjpeg_router, snapshot handler
- Live code inspection: `crates/rc-sentry-ai/src/nvr.rs` — NvrClient, digest auth
- Live code inspection: `crates/rc-sentry-ai/src/config.rs` — CameraConfig struct
- Live code inspection: `crates/rc-sentry-ai/cameras.html` — existing snapshot-only dashboard
- Live code inspection: `web/src/app/cameras/page.tsx` — existing Next.js cameras page
- Live config: `C:\RacingPoint\go2rtc\go2rtc.yaml` — 3-stream configuration (ch1-ch13 pending)
- go2rtc WebRTC protocol: [deepwiki.com — WebRTC Protocol](https://deepwiki.com/AlexxIT/go2rtc/3.2-webrtc-protocol) — WebSocket signaling, message types `webrtc/offer`, `webrtc/answer`, `webrtc/candidate` verified against go2rtc video-rtc.js source
- Codec support: H.265 WebRTC relay confirmed for Chrome 136+ / Safari 18+ (MEDIUM confidence — from go2rtc documentation; browser versions on staff machines not independently verified)

---

*Architecture research for: v16.1 Camera Dashboard Pro — hybrid streaming NVR dashboard*
*Researched: 2026-03-22 IST*
