# Stack Research

**Domain:** Camera dashboard hybrid streaming — WebRTC fullscreen + snapshot grid (v16.1)
**Researched:** 2026-03-22
**Confidence:** HIGH (go2rtc WebRTC API verified via source code + DeepWiki; drag-drop pattern verified against HTML5 spec; localStorage per MDN)

---

## Context: What Already Exists (Do Not Re-research)

| Component | Version | Status |
|-----------|---------|--------|
| Axum | 0.7 | In Cargo.toml |
| reqwest | 0.12 | In Cargo.toml |
| tokio | workspace | In Cargo.toml |
| tower-http (CORS) | 0.6 | In Cargo.toml |
| serde_json | workspace | In Cargo.toml |
| Embedded HTML | cameras.html via `include_str!` | Existing pattern |
| Snapshot proxy | `/api/v1/cameras/nvr/{ch}/snapshot` | Already serving background-cached snapshots |
| go2rtc | port 1984 | Running on James .27 (192.168.31.27:1984) |
| SnapshotCache | `mjpeg.rs` | Background-refreshed per-channel cache, already working |

**Constraint confirmed:** Stay as a single embedded HTML page. No npm, no bundler, no new Rust crates.

---

## Recommended Stack (New Additions Only)

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| go2rtc `video-stream.js` | served from go2rtc :1984 | WebRTC player custom element `<video-stream>` | Built into go2rtc; zero extra deps; handles WebRTC/MSE/MJPEG fallback automatically; already running on the LAN |
| HTML5 Drag and Drop API | browser native | Camera grid reorder via tile drag | No library needed; `draggable` attribute + `dragstart`/`dragover`/`drop` events; all modern browsers support it since 2012 |
| `localStorage` | browser native | Layout persistence (mode, order, names) | Survives page reload without a server round-trip; JSON-serializable; 5MB quota (layout JSON is under 1KB); synchronous reads safe in vanilla JS |
| CSS Grid with column-count class swap | browser native | Layout mode switching (1/4/9/16 split) | Single class change on `.grid` element switches all layouts; no JS layout calculation; browser handles cell sizing |

### Backend Additions (Rust/Axum)

**No new Rust crates required.** The existing snapshot proxy endpoint continues unchanged. The only potential new endpoint is optional:

| Endpoint | Method | Purpose | Notes |
|----------|--------|---------|-------|
| `/api/v1/cameras/dashboard-config` | GET + POST | Persist layout/names server-side for multi-browser sync | Optional — localStorage covers single-browser use; only add if Uday needs to see the same layout from different devices |

If added, this endpoint reads/writes a flat JSON file at `C:\RacingPoint\camera-dashboard.json` using existing `serde_json` + `tokio::fs`. No new crate.

### Browser-Side Library (Load from go2rtc)

| Library | Version | Source | When to Use |
|---------|---------|--------|-------------|
| `video-stream.js` | bundled with go2rtc | `http://192.168.31.27:1984/video-stream.js` | Load as ES module via `<script type="module">` in cameras.html; use `<video-stream>` custom element for WebRTC fullscreen |

**Critical:** Load from the live go2rtc instance, never copy/vendor. The JS and server must stay in sync — protocol changes in go2rtc updates would break a vendored copy.

---

## go2rtc WebRTC Integration

### How go2rtc WebRTC Works (Verified from Source)

go2rtc exposes WebSocket-based WebRTC signaling at `/api/ws?src=<stream_name>`. The bundled `video-stream.js` (class `VideoStream extends VideoRTC`) registers a `<video-stream>` custom element that handles the full offer/answer/ICE exchange internally. Protocol fallback order (automatic): WebRTC → MSE → HLS → MJPEG.

**Stream naming:** Each camera must have a named stream in `go2rtc.yaml`. For Dahua NVR channel N, the stream name convention is `cam_chNN`. The sub-stream (`subtype=1`) is correct for dashboard use — lower bitrate, browser-decodable H.264, ~512Kbps vs ~4Mbps for main stream.

**go2rtc.yaml pattern for all 13 Dahua NVR channels:**

```yaml
streams:
  cam_ch01:
    - rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=1&subtype=1
  cam_ch02:
    - rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=2&subtype=1
  cam_ch03:
    - rtsp://admin:Admin%40123@192.168.31.18/cam/realmonitor?channel=3&subtype=1
  # ... repeat through cam_ch13
```

Note: `Admin@123` must be percent-encoded as `Admin%40123` in RTSP URLs. Currently only 3 cameras are in go2rtc — all 13 must be added before v16.1 can work.

**Embedding a WebRTC player in the HTML page:**

```html
<!-- Load once, at top of <body> or in <head> -->
<script type="module" src="http://192.168.31.27:1984/video-stream.js"></script>

<!-- Per-camera element — src auto-converts http/relative to ws:// -->
<video-stream
  src="ws://192.168.31.27:1984/api/ws?src=cam_ch01"
  mode="webrtc,mse,mjpeg"
  style="width:100%;height:100%;background:#000;">
</video-stream>
```

**Key properties (source-verified — HIGH confidence):**

| Property | Value | Notes |
|----------|-------|-------|
| `src` | WebSocket URL or path | Setter auto-converts `http://` → `ws://`; relative paths resolve against current origin |
| `mode` | `"webrtc,mse,mjpeg"` | Protocol priority order; WebRTC tried first, MJPEG as last resort |
| `background` | `false` (default) | Pauses stream when element is not visible — use to stop non-fullscreen streams |
| `visibilityCheck` | `true` (default) | Pauses stream when browser tab loses focus |

### Fullscreen Mode — Hybrid Switch Pattern

The grid shows snapshots (cheap, 0.5fps). Fullscreen upgrades one camera to WebRTC. Pattern:

1. User clicks a camera tile
2. Snapshot polling loop pauses (`clearInterval(timer)`)
3. Fullscreen overlay becomes `display: flex`
4. `<video-stream>` element is created and inserted into overlay with the clicked camera's `src`
5. WebRTC connection negotiates (typically sub-second on LAN)
6. User presses Escape or clicks outside
7. `<video-stream>` is removed from DOM — this closes the WebRTC peer connection automatically
8. Snapshot polling resumes (`startLoop()`)

**Why this pattern:** Only one WebRTC connection is active at any time. Attempting 13 simultaneous WebRTC connections would overwhelm the Dahua NVR (it has a connection limit) and waste bandwidth. One connection at a time matches the DMSS HD pattern.

**No TURN server needed:** All devices are on the 192.168.31.x LAN. go2rtc uses direct host ICE candidates. WebRTC completes without STUN negotiation in most cases.

---

## Layout Mode Implementation

### CSS Grid Class Swap

Four layout modes controlled by a single class on `.grid`. No JavaScript layout math.

```css
.grid { display: grid; gap: 4px; padding: 4px; height: 100%; }
.grid.mode-1  { grid-template-columns: 1fr; }
.grid.mode-4  { grid-template-columns: 1fr 1fr; }
.grid.mode-9  { grid-template-columns: 1fr 1fr 1fr; }
.grid.mode-16 { grid-template-columns: 1fr 1fr 1fr 1fr; }
```

Switching in JS: `grid.className = 'grid mode-' + count;`

The layout selector in the toolbar becomes: `<select onchange="setMode(this.value)">` with options 1, 4, 9, 16.

---

## Drag-to-Reorder Implementation

### HTML5 Native Drag and Drop API

No library. The built-in API is sufficient for a fixed-count grid where cells swap positions.

```javascript
var dragSrc = null;

function makeDraggable(card) {
  card.draggable = true;
  card.addEventListener('dragstart', function(e) {
    dragSrc = card;
    e.dataTransfer.effectAllowed = 'move';
  });
  card.addEventListener('dragover', function(e) {
    e.preventDefault();               // REQUIRED — without this, drop never fires
    e.dataTransfer.dropEffect = 'move';
  });
  card.addEventListener('drop', function(e) {
    e.preventDefault();
    if (dragSrc && dragSrc !== card) {
      swapCards(dragSrc, card);
      persistLayout();
    }
  });
}

function swapCards(a, b) {
  // Swap DOM positions
  var parentA = a.parentNode;
  var siblingA = a.nextSibling === b ? a : a.nextSibling;
  b.parentNode.insertBefore(a, b);
  parentA.insertBefore(b, siblingA);
  // Update order array
  updateOrderFromDOM();
}
```

**Critical:** `dragover` must call `e.preventDefault()`. This is the most common drag-drop bug — without it, the browser treats the target as non-droppable and `drop` never fires.

---

## Layout Persistence

### localStorage Schema

```javascript
var LAYOUT_KEY = 'rp_camera_layout_v1';

var defaultLayout = {
  mode: 9,                   // default: 3x3 grid
  order: [1,2,3,4,5,6,7,8,9,10,11,12,13],  // ch numbers in display order
  names: {                   // friendly names, keyed by ch number as string
    '1':  'Entrance',
    '2':  'Camera 2',
    '3':  'Reception Wide',
    '4':  'Reception',
    // ... etc
  }
};

function persistLayout() {
  localStorage.setItem(LAYOUT_KEY, JSON.stringify({
    mode: currentMode,
    order: cameraOrder,
    names: cameraNames
  }));
}

function loadLayout() {
  try {
    var saved = localStorage.getItem(LAYOUT_KEY);
    return saved ? JSON.parse(saved) : defaultLayout;
  } catch (e) {
    return defaultLayout;
  }
}
```

The `try/catch` around `JSON.parse` is required — localStorage can contain stale/invalid JSON if the schema changes between versions. The `_v1` suffix in the key allows a future schema bump to use `_v2` without migration logic.

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| go2rtc `video-stream.js` custom element | Writing raw WebRTC signaling from scratch | go2rtc's JS handles offer/answer/ICE/codec negotiation + MSE/MJPEG fallback; writing it from scratch is ~400 lines, fragile, and unmaintained |
| HTML5 native drag-drop | SortableJS (6KB gzip) | Works fine, but adds an external dependency to an embedded HTML page with no bundler; native API is sufficient for fixed-count grid swap |
| `localStorage` | Server-side SQLite or JSON file layout endpoint | Adds Axum endpoint + state for a single-browser dashboard; localStorage is simpler and equally durable for the single-operator use case |
| CSS Grid class swap | JavaScript `flex-wrap` or inline style calculation | CSS handles the math; JS class swap is one line; no layout thrashing |
| go2rtc sub-stream `subtype=1` | Main stream `subtype=0` | Main stream is 4MP ~4Mbps per camera; browser WebRTC decode of even one simultaneous 4MP stream is heavy; sub-stream is ~512Kbps H.264, designed for monitoring |
| One WebRTC connection (fullscreen only) | 13 simultaneous WebRTC connections | NVR has connection limits; browser peer connection overhead for 13 simultaneous streams is significant; snapshot grid is adequate for overview |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `video.js`, `plyr`, or similar player libraries | Wrong abstraction layer — designed for HTTP video, not WebRTC signaling | go2rtc's bundled `video-stream.js` |
| React / Vue / any component framework | No bundler, no build step, single HTML page constraint | Vanilla JS DOM manipulation |
| SortableJS or similar drag library | External dep with no bundler; native HTML5 DnD is sufficient | HTML5 DnD API (`draggable`, `dragstart`, `dragover`, `drop`) |
| TURN server | Not needed on LAN — all devices on 192.168.31.x subnet | go2rtc uses direct host ICE candidates; no relay needed |
| Copying/vendoring `video-stream.js` | Version drift with go2rtc server causes signaling failures on go2rtc update | Always load from live go2rtc instance at `:1984` |
| 13 simultaneous WebRTC connections in the grid | NVR connection limit exceeded; browser overhead | One WebRTC connection for fullscreen only; snapshot polling for grid |
| MSE/HLS as primary streaming mode | Higher latency than WebRTC; no advantage on LAN | WebRTC as primary with MSE fallback (handled automatically by `video-stream.js`) |
| New Rust crates in Cargo.toml | Unnecessary; browser APIs + go2rtc cover all requirements | Existing axum, serde_json, tokio::fs |

---

## go2rtc Configuration Checklist

Before v16.1 implementation begins:

- [ ] All 13 cameras added to `go2rtc.yaml` (currently only 3)
- [ ] Stream names follow `cam_ch01` through `cam_ch13` convention
- [ ] RTSP URLs use `subtype=1` (sub-stream) not `subtype=0`
- [ ] `Admin@123` percent-encoded as `Admin%40123` in RTSP URLs
- [ ] go2rtc accessible from browser at `http://192.168.31.27:1984`
- [ ] go2rtc CORS headers allow origin from rc-sentry-ai's port (8096) — or use relative URL if proxied

**CORS option:** If the browser loads cameras.html from rc-sentry-ai at `:8096` but the WebRTC WS connects to go2rtc at `:1984`, this is cross-origin. go2rtc allows this by default (it serves `Access-Control-Allow-Origin: *` on its API). Verify before implementation; if CORS is restricted, add a proxy route in Axum that forwards `/go2rtc/*` to `http://127.0.0.1:1984/*`.

---

## Version Compatibility

| Component | Compatible With | Notes |
|-----------|-----------------|-------|
| go2rtc `video-stream.js` | go2rtc running instance | Must match the server version — load from live instance |
| HTML5 DnD API | Chrome 4+, Firefox 3.5+, Safari 5+ | All modern browsers; not for touch screens |
| CSS Grid | Chrome 57+, Firefox 52+, Safari 10.1+ | All modern browsers |
| `localStorage` | All modern browsers | 5MB quota; layout JSON is under 1KB |
| Axum 0.7 | tower-http 0.6, tokio | Already in Cargo.toml — no change |
| `include_str!` macro | All Rust versions | Existing pattern in cameras.html serve |

---

## Sources

- go2rtc `www/stream.html` (GitHub source) — `<video-stream>` element, `api/ws?src=` URL format: HIGH confidence
- go2rtc `www/video-rtc.js` (GitHub source) — `src` setter implementation, `mode` attribute, `background` property: HIGH confidence
- DeepWiki AlexxIT/go2rtc web interface documentation — VideoStream/VideoRTC class, protocol fallback order, `/api/webrtc` WHIP endpoint: HIGH confidence
- MDN Web Docs — HTML Drag and Drop API (`draggable`, `dragstart`, `dragover`, `drop`, `e.preventDefault()` requirement): HIGH confidence
- MDN Web Docs — Window.localStorage, JSON.parse/stringify: HIGH confidence
- Dahua NVR RTSP URL format (`channel=N&subtype=1`, percent-encoding) — community + Frigate docs + GitHub Discussion #14956: MEDIUM confidence (widely documented pattern)
- Existing rc-sentry-ai codebase — `Cargo.toml`, `cameras.html`, `mjpeg.rs`, `config.rs`, `main.rs`: HIGH confidence (direct read 2026-03-22)

---

*Stack research for: v16.1 Camera Dashboard Pro — hybrid streaming, layout modes, drag-to-rearrange*
*Researched: 2026-03-22 IST*
