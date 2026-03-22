# Pitfalls Research

**Domain:** Adding WebRTC streaming + professional NVR dashboard UI to existing Rust/Axum service — v16.1 Camera Dashboard Pro
**Researched:** 2026-03-22 IST
**Confidence:** HIGH for go2rtc integration pitfalls (confirmed via go2rtc issue tracker + official docs), HIGH for credential proxy (direct Dahua NVR knowledge), MEDIUM for include_str! binary size (compiler issue tracker + benchmark data), MEDIUM for WebRTC connection management (community post-mortems + WebRTC spec), LOW for layout persistence tradeoffs (pattern-based inference, not benchmark-backed)

---

## Context: What Is Being Added to What

This is an extension to an existing, working rc-sentry-ai service (:8096) that already:
- Proxies Dahua NVR snapshots and caches them in memory
- Serves a single HTML file via `include_str!`
- Has ONNX Runtime linked (requires dynamic CRT: `-C target-feature=-crt-static`)
- go2rtc is already running on the server with 3 cameras configured; 10 more need adding

The pitfalls below are **integration pitfalls** — things that break specifically because of how these new features interact with the existing system, not generic WebRTC or dashboard problems.

---

## Critical Pitfalls

### Pitfall 1: go2rtc CORS Blocks WebRTC Signaling from a Different Origin

**What goes wrong:**
The HTML page is served from rc-sentry-ai on port 8096. go2rtc's WHEP endpoint is on port 1984. The browser makes a cross-origin POST to `http://192.168.31.23:1984/api/webrtc` — this triggers a CORS preflight. go2rtc's default config does NOT include `Access-Control-Allow-Origin` headers, so the browser blocks the request before it reaches go2rtc. The WebRTC button appears to do nothing; the console shows CORS errors that look like a network failure.

**Why it happens:**
go2rtc's CORS config parameter is `origin` under `[api]`, not `cors`. Using `cors: "*"` (wrong key) silently does nothing — go2rtc ignores unknown config keys. The CORS failure is invisible server-side because go2rtc never logs rejected preflight OPTIONS requests.

**How to avoid:**
In `go2rtc.yaml` (or wherever the server config lives):
```yaml
api:
  listen: ":1984"
  origin: "*"
```
Use `origin`, not `cors`. Verify by manually sending an OPTIONS request to `http://192.168.31.23:1984/api/webrtc` and checking for the `Access-Control-Allow-Origin: *` response header before writing any frontend WebRTC code.

**Warning signs:**
- Browser console shows "Response to preflight request doesn't pass access control check"
- go2rtc server logs show no record of the request at all
- WebRTC works from go2rtc's own built-in web UI (`http://server:1984`) but fails from rc-sentry-ai's page

**Phase to address:**
go2rtc setup phase (adding 13 cameras + verifying CORS) — before writing a single line of frontend WebRTC code.

---

### Pitfall 2: go2rtc WHEP SDP Negotiation Has a 5-10 Second Cold Start Delay

**What goes wrong:**
The first WebRTC connection to a camera after go2rtc starts takes 5-10 seconds before the video appears. If go2rtc is configured to use lazy/on-demand stream loading (the default), it must connect to the camera's RTSP stream, perform two-way codec negotiation, and gather ICE candidates before responding to the browser's SDP offer. During this time the video element stays black. Users click the camera again, triggering a second connection, which compounds the delay.

The delay is not a bug — it is documented go2rtc behavior confirmed in issue #1392. The root cause: go2rtc cannot skip RTSP probing to determine what codecs the camera provides, even if you've seen that camera before.

**Why it happens:**
go2rtc's "lazy" mode does not keep RTSP connections open between viewers. Each new first-viewer for a camera triggers a fresh RTSP connect + codec negotiation. For Dahua cameras on the LAN this is typically 3-8 seconds depending on camera model.

**How to avoid:**
For the fullscreen click-to-stream use case, pre-warm the stream: the moment the user hovers over a camera tile (or on page load for the most-viewed cameras), send a lightweight probe to go2rtc (`GET /api/streams?src=camera_name`) to establish the connection before the user clicks. The actual WebRTC negotiation then finds an already-connected stream and responds in under 1 second.

Alternatively, enable persistent RTSP connections in go2rtc config by adding all cameras under `streams:` — this keeps the RTSP connection alive regardless of viewer count, eliminating cold-start at the cost of sustained NVR bandwidth.

**Warning signs:**
- Video appears after 5-10 second delay but then streams fine — this is the cold-start symptom, not a connection failure
- Clicking a different camera after the first succeeds is fast — confirms lazy loading for first viewer

**Phase to address:**
Frontend WebRTC phase — add hover-prewarming or connection pooling before shipping click-to-fullscreen.

---

### Pitfall 3: 13 Simultaneous WebRTC Streams Will Overload the NVR and the Browser

**What goes wrong:**
go2rtc proxies RTSP from the Dahua NVR and re-streams to WebRTC clients. If the dashboard opens 13 WebRTC connections simultaneously (one per camera tile), it creates 13 peer connections in the browser — each requires ICE negotiation, DTLS handshake, and a sustained media receiver. Simultaneously: the NVR is serving 13 RTSP substreams to go2rtc. Dahua NVRs typically allow 64-128 simultaneous remote connections total, but real bottlenecks appear earlier: at 13 concurrent substreams, CPU on the NVR can spike, and go2rtc's CPU on the server will spike due to H.264 packet repackaging for WebRTC.

Additionally, Chrome allows up to 500 RTCPeerConnections per tab but starts showing degraded performance in practice well below that. At 13 simultaneous 720p streams, browser GPU decode demand becomes the bottleneck on lower-spec machines.

**Why it happens:**
The "show all cameras as WebRTC" instinct comes from wanting sub-second latency everywhere. But the hybrid design (snapshots for grid, WebRTC only for fullscreen) exists precisely to avoid this. If the frontend adds a video element with WebRTC for each grid tile "for future-proofing," the resource issue is immediate.

**How to avoid:**
Never open more than 1 WebRTC connection at a time. The grid uses snapshots (HTTP polling every 1-2 seconds — already implemented). WebRTC is only for the fullscreen single-camera view. When the user clicks a camera to fullscreen, open 1 WebRTC connection. When they exit fullscreen or switch cameras, call `peerConnection.close()` before opening the next one.

Enforce this at the JavaScript layer with a module-level singleton: `let activePeerConnection = null`. Any new WebRTC connection attempt first closes the active one.

**Warning signs:**
- More than one WebRTC video element exists in the DOM at the same time
- go2rtc logs show 13+ simultaneous stream sessions
- Browser tab memory usage exceeds 500MB with all cameras visible

**Phase to address:**
Frontend architecture phase — define the singleton WebRTC connection pattern before writing any camera tile code.

---

### Pitfall 4: NVR Credentials Exposed to the Browser via Direct RTSP URL in HTML

**What goes wrong:**
The Dahua NVR uses `admin/Admin@123` for all access. If the frontend JavaScript constructs a WebRTC URL that includes credentials (e.g., `go2rtc?src=rtsp://admin:Admin@123@192.168.31.18/...`), the credentials appear in the browser source, network tab, and any log that records the URL. Any person on the LAN who opens DevTools sees the admin password. The existing snapshot proxy (`/api/cameras/:id/snapshot`) already solves this by proxying through rc-sentry-ai — the same pattern must apply to WebRTC.

**Why it happens:**
go2rtc's WHEP endpoint identifies streams by stream name, not by RTSP URL. If streams are pre-configured in go2rtc.yaml with names like `pod_area`, `cashier`, etc., the browser only needs to pass `?src=pod_area` — no credentials. But if the developer adds cameras to go2rtc by constructing the URL client-side (to avoid editing yaml), the credentials end up in the browser.

**How to avoid:**
All 13 cameras must be pre-configured in go2rtc.yaml with canonical names before frontend development starts. The frontend JavaScript uses only the stream name (e.g., `pod_area`), never the RTSP URL. The go2rtc.yaml lives on the server and is not served to clients. The rc-sentry-ai snapshot proxy already hides credentials — the WebRTC flow must follow the same principle.

**Warning signs:**
- `192.168.31.18` or `Admin@123` appearing in any JavaScript string in the served HTML
- go2rtc stream URL construction happening in the frontend rather than in go2rtc.yaml

**Phase to address:**
go2rtc configuration phase — name all 13 streams before writing frontend code. Lock this as a prerequisite.

---

### Pitfall 5: `include_str!` Binary Bloat Slows Rebuilds When JavaScript Grows Large

**What goes wrong:**
The existing dashboard is served via `include_str!("../static/dashboard.html")` — the HTML + CSS + JS baked into the Rust binary at compile time. For a simple dashboard this is elegant. When complex JavaScript is added (drag-and-drop library, WebRTC signaling, layout state machine, smooth transitions), the single HTML file grows. At ~100KB of inline JavaScript, `include_str!` begins increasing compile times noticeably because the Rust compiler must process the string literal. At ~500KB+ (which is reachable if a drag-and-drop library is inlined), compile time increases by seconds and binary size grows proportionally.

The documented issue in rust-lang/rust#65818: using `include_bytes!` on multi-megabyte blobs causes significantly slower compile times because pretty-printing byte string literals takes disproportionate time.

**Why it happens:**
The developer adds a drag-and-drop library by copy-pasting its minified source into the HTML. The minified source is 80KB. Then WebRTC signaling code adds another 30KB. Soon the HTML is 200KB, and `cargo build` takes noticeably longer on every iteration because the whole file is re-parsed.

**How to avoid:**
Use CDN-hosted libraries for development (link to `sortable.min.js` from a CDN) and only inline the application-specific code. For production, if CDN is unacceptable (LAN-only deployment), use `rust-embed` with compression:
```toml
[dependencies]
rust-embed = { version = "8", features = ["compression"] }
```
`rust-embed` with compression reduces binary size and compile time versus raw `include_str!`. Keep application JS under 30KB — if drag-and-drop requires more, that is a sign a library was chosen without considering the embedding constraint.

**Warning signs:**
- `cargo build --release` time increases by more than 3 seconds after adding JavaScript
- The HTML file in `static/` exceeds 100KB
- A minified third-party library was inlined into the HTML file

**Phase to address:**
Frontend architecture phase — decide on the embedding strategy before choosing drag-and-drop libraries. Size budget: keep the HTML under 80KB including all JavaScript.

---

### Pitfall 6: Drag-and-Drop Camera Order Not Synced Between Both Deployment Origins

**What goes wrong:**
The dashboard is served from two origins: rc-sentry-ai (:8096) and the server web dashboard (:3200). If camera order is persisted in `localStorage`, it is scoped to the origin — a layout change at `:8096` is invisible at `:3200`. Staff who rearrange cameras on the sentry dashboard see a different order on the server dashboard. If camera names are stored in `racecontrol.toml` (server-side) but layout order is in `localStorage` (client-side), they diverge as soon as anyone uses the second origin.

**Why it happens:**
`localStorage` is the path of least resistance for persisting UI state. It works immediately, requires no server changes, and is fast. The developer tests only from one origin and does not notice the synchronization gap.

**How to avoid:**
Layout order must be stored server-side, not in `localStorage`. The rc-sentry-ai service has a TOML config (or will gain a JSON config) for camera names — extend this to include layout order. Expose a `PUT /api/cameras/layout` endpoint that persists the order. Both serving origins talk to the same backend (same rc-sentry-ai process) so both see the same layout.

`localStorage` is acceptable as a **cache** for the last-known layout to avoid a loading flash, but the source of truth is the server. On page load: load from localStorage immediately (no flash), then fetch from server, update if different, and re-save to localStorage.

**Warning signs:**
- Camera order only saved in `localStorage`
- No `PUT` or `POST` endpoint for layout in rc-sentry-ai
- Developer only tests from one port

**Phase to address:**
Backend API phase — add `PUT /api/cameras/layout` before building the drag-and-drop frontend interaction.

---

### Pitfall 7: go2rtc RTSP Snapshot and WebRTC Competing for the Same Camera Connection

**What goes wrong:**
The existing snapshot cache hits the Dahua NVR HTTP snapshot endpoint directly (`/cgi-bin/snapshot.cgi`). go2rtc connects via RTSP to the same cameras for WebRTC. These are separate connections to the same NVR. Dahua NVRs have a documented soft limit on simultaneous stream connections per channel (typically 2-4 depending on model and stream type). When the snapshot cache + go2rtc WebRTC + possibly Dahua's own web client all connect simultaneously, the NVR may refuse new connections or drop existing ones with "maximum stream" errors.

The existing snapshot proxy already makes 13 background HTTP connections. Adding 1 RTSP connection via go2rtc for the active WebRTC stream means 14 simultaneous NVR connections. During connection setup (when go2rtc is negotiating), there is a brief moment with both the snapshot HTTP connection and the RTSP connection active for the same channel.

**Why it happens:**
HTTP snapshot and RTSP stream are assumed to be independent. They are at the protocol level, but Dahua tracks them both against the per-channel connection limit.

**How to avoid:**
Route all camera access through go2rtc — use go2rtc's snapshot API (`GET /api/frame.jpeg?src=camera_name`) instead of the existing direct NVR snapshot proxy. go2rtc maintains a single RTSP connection per camera and serves both snapshots and WebRTC from it. This eliminates the competing-connections problem.

If routing through go2rtc is not viable immediately (it changes the existing working snapshot cache), reduce the snapshot poll rate to 5s during WebRTC sessions. Add a flag to the snapshot cache: when a WebRTC connection is active for camera N, skip polling camera N's HTTP snapshot to avoid competing with the RTSP stream.

**Warning signs:**
- Dahua NVR logs or web UI showing "Maximum number of connections exceeded" for a channel
- Snapshot for one camera becoming unavailable exactly when WebRTC connects to the same camera
- go2rtc logs showing "RTSP connection refused" on first connect

**Phase to address:**
go2rtc camera onboarding phase — decide whether snapshots flow through go2rtc or directly to the NVR. Make this decision before implementing the snapshot pause-during-WebRTC logic.

---

### Pitfall 8: WebRTC Peer Connection Not Closed When User Navigates Away or Changes Layout Mode

**What goes wrong:**
When the user switches from fullscreen (WebRTC active) to grid mode by pressing 4x4, the video element is removed from the DOM. But the `RTCPeerConnection` is not explicitly closed — it lives in JavaScript memory. go2rtc continues streaming to this ghost connection. The browser's garbage collector eventually closes it, but Chrome does not aggressively GC open network connections. Meanwhile go2rtc is consuming RTSP bandwidth and encoding for a peer that is no longer rendering anything.

In the fullscreen mode, if the user clicks a second camera before the first's connection is fully torn down, two simultaneous go2rtc sessions exist briefly. If the NVR is at its connection limit (see Pitfall 7), the second connection fails and the user sees a black screen.

**Why it happens:**
WebRTC connections are not tied to DOM element lifecycles. Removing a video element does not close the peer connection. This is not obvious to developers who assume DOM cleanup equals connection cleanup.

**How to avoid:**
Maintain a module-level reference and a dedicated teardown function:
```javascript
let activeRtcPc = null;

function teardownRtc() {
  if (!activeRtcPc) return;
  activeRtcPc.getSenders().forEach(s => { if (s.track) s.track.stop(); });
  activeRtcPc.getReceivers().forEach(r => { if (r.track) r.track.stop(); });
  activeRtcPc.close();
  activeRtcPc = null;
}
```
Call `teardownRtc()` on: layout mode change, camera deselect, page unload (`beforeunload`), and visibility change (`visibilitychange` to hidden). Verify closure by checking go2rtc's `/api/streams` endpoint — the viewer count for the camera should drop to 0 within 2 seconds of closing.

**Warning signs:**
- go2rtc `/api/streams` shows viewer count > 0 after the fullscreen is dismissed
- Browser Memory tab shows `RTCPeerConnection` objects accumulating over time in the heap
- go2rtc CPU does not drop after exiting fullscreen

**Phase to address:**
Frontend WebRTC implementation phase — write `teardownRtc()` before any connection-opening code.

---

### Pitfall 9: Layout Mode Switching Triggers Expensive DOM Thrash with 13 Camera Elements

**What goes wrong:**
Switching from 4x4 (16 tiles) to 3x3 (9 tiles) to 2x2 (4 tiles) naively implemented removes and re-creates camera tile elements. Each removal destroys the snapshot `<img>` element, which then must re-fetch the snapshot on creation. If the transition also triggers a CSS animation while images are loading, the user sees 13 images blink simultaneously and the dashboard looks broken for 2-3 seconds.

Additionally, if layout mode switching creates and destroys video elements for any WebRTC stream, it may trigger the connection leak described in Pitfall 8.

**Why it happens:**
The simplest implementation of "change layout" is to clear the container and rebuild from scratch — destroy everything, rebuild. This is fine at low element counts but visible at 13+ elements with network-loaded images.

**How to avoid:**
Use CSS grid mode switching, not DOM replacement. All 13 camera tiles exist in the DOM at all times. Switching layout mode changes the CSS grid template and shows/hides tiles, not creates/destroys them. Snapshot images persist across mode switches — no re-fetch.

```css
.grid-1x1 { grid-template-columns: 1fr; }
.grid-2x2 { grid-template-columns: 1fr 1fr; }
.grid-3x3 { grid-template-columns: 1fr 1fr 1fr; }
.grid-4x4 { grid-template-columns: 1fr 1fr 1fr 1fr; }
```

Tiles not shown in the current layout get `display: none` — their snapshot cache and any existing connection state is preserved.

**Warning signs:**
- Network tab shows 13 simultaneous image requests on every layout mode switch
- Layout switch takes more than 200ms to visually complete (check with Performance tab)
- The DOM is fully rebuilt on each mode switch

**Phase to address:**
Frontend architecture phase — decide on DOM strategy before any tile rendering code is written.

---

### Pitfall 10: TOML Round-Trip Corruption When Writing User Preferences Back to Config

**What goes wrong:**
`rc-sentry-ai.toml` is read at startup via config. If camera names and layout order are stored in the same TOML file as the ONNX model path, NVR credentials, and Dahua endpoint config, then a `PUT /api/cameras/layout` endpoint that writes back to the file must serialize the entire config as TOML. This risks:
1. Losing comments in the TOML file (the `toml` crate's serialization drops all comments)
2. Reordering config sections (alphabetical vs. human-organized)
3. A partial write during a crash corrupting the file and preventing startup

**Why it happens:**
"Just serialize the AppState back to TOML" is natural. The `toml` crate's serializer produces valid TOML but does not preserve the original file's formatting or comments. After the first `PUT`, every subsequent file diff shows the entire file rewritten, making git history unreadable.

**How to avoid:**
Store user preferences (camera names, layout order, camera-to-grid-position mapping) in a separate file: `camera-layout.json`. This file is:
- Small, simple JSON — no structure worth commenting
- Written atomically (write to `.tmp` then rename)
- Never merged with the main TOML config at read or write time
- Loaded alongside the main config at startup

Keep `rc-sentry-ai.toml` read-only at runtime. Only `camera-layout.json` is written by the running service.

**Warning signs:**
- A `write_config()` function that serializes the full AppState to TOML
- The TOML config file contains camera names or grid positions
- git diff of `rc-sentry-ai.toml` after a layout change shows the entire file rewritten

**Phase to address:**
Backend API phase — define the config split before implementing any persistence endpoint.

---

## Technical Debt Patterns

Shortcuts that seem reasonable but create long-term problems.

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Open WebRTC per grid tile (13 connections) | No click required for live video | NVR overloaded, browser degraded, go2rtc at capacity | Never — hybrid design exists precisely to avoid this |
| `localStorage` for layout persistence | No backend changes needed | Layout diverges between the two serving origins | Only as a cache, never as source of truth |
| Inline minified drag-and-drop library in HTML | No CDN dependency | Binary bloat, slower rebuilds | Never — use rust-embed with compression or load from local path |
| Write layout back to main TOML config | Single config file | Comment loss, partial-write corruption, unreadable git history | Never — use a separate JSON file |
| Open new WebRTC connection without closing old | Simpler code path | Ghost connections accumulate on go2rtc side | Never |
| DOM rebuild on layout switch | Simple implementation | 13-image reload flash on every mode change | Only on initial page load, never on mode switch |
| Hardcode RTSP URLs with credentials in frontend | Faster prototyping | Admin password visible in browser source | Never, even for development |

---

## Integration Gotchas

Common mistakes when connecting new features to the existing rc-sentry-ai service.

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| go2rtc CORS | Use `cors:` config key | Use `origin:` under `[api]` section in go2rtc.yaml |
| go2rtc cold start | Connect WebRTC on click (5-10s delay visible) | Pre-warm stream on hover; send probe request before user clicks |
| Snapshot proxy + go2rtc RTSP | Both connect independently to NVR per camera | Route snapshots through go2rtc's frame API OR pause snapshot polling during WebRTC session |
| WebRTC peer cleanup | Remove video element to "close" connection | Explicitly call `peerConnection.close()` + stop all tracks |
| Layout persistence dual-origin | Store in `localStorage` | Backend endpoint + `localStorage` as cache |
| Camera naming | Add to main `racecontrol.toml` or `rc-sentry-ai.toml` | Separate `camera-layout.json`, loaded alongside main config |
| go2rtc stream names | Use RTSP URLs with credentials in JS | Pre-configure named streams in `go2rtc.yaml`, use names only in JS |
| include_str! growth | Inline third-party libraries | Keep HTML under 80KB; use rust-embed with compression for larger assets |

---

## Performance Traps

Patterns that work in testing but degrade in the live 13-camera scenario.

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Snapshot polling all 13 cameras at 1s intervals simultaneously | NVR CPU spikes; occasional 503s from NVR HTTP endpoint | Stagger polling: poll camera N at offset N*76ms, not all simultaneously | At 13 cameras, 1s intervals — already at scale |
| Eager WebRTC for all visible cameras | go2rtc CPU at 100% on server; browser tab at 100% GPU | WebRTC for fullscreen only (1 connection maximum) | At 2+ simultaneous connections |
| go2rtc lazy stream loading with no pre-warm | 5-10s black screen on fullscreen click | Hover pre-warm or eager RTSP connect at go2rtc level | Every first-viewer event |
| CSS transitions on 13 elements simultaneously | Janky layout switch animation on lower-spec machines | Use CSS grid reflow (no element creation) + `will-change: transform` on tiles | On any machine; visible on server's integrated GPU |
| Snapshot image re-fetch on layout mode switch | 13 network requests on every mode change | CSS show/hide tiles; never destroy/recreate tile DOM | At 13 cameras, every mode switch |

---

## Security Mistakes

Domain-specific security issues for this NVR dashboard addition.

| Mistake | Risk | Prevention |
|---------|------|------------|
| NVR credentials in JavaScript strings | Admin password visible in browser DevTools to any LAN user | Pre-configure named streams in go2rtc.yaml; never put credentials in frontend code |
| go2rtc API exposed on `0.0.0.0:1984` without auth | Any LAN device can list cameras, change streams, access RTSP | go2rtc listens on `127.0.0.1:1984` or `.23` loopback only; rc-sentry-ai proxies WHEP if needed |
| Snapshot endpoint logging full NVR URL | Credentials appear in Rust tracing logs | Log only `camera_id`, not the full URL with password |
| `camera-layout.json` world-writable | Any local process can overwrite camera names | File permissions: owner-write only; validate on read (expected schema, not arbitrary JSON) |
| WebRTC ICE candidate leaking internal IP | Internal IP addresses appear in browser JS | Filter ICE candidates to LAN-only types; add `filters.candidates: ["host"]` in go2rtc webrtc config |

---

## UX Pitfalls

Common user experience mistakes specific to NVR dashboards.

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Black screen for 5-10s on WebRTC connect with no indicator | User thinks the feature is broken; clicks again, causing double-connection | Show a spinner/loading state during ICE negotiation; disable click until connection resolves |
| Snapshot images have stale timestamps (no cache-busting) | User cannot tell if snapshots are live or frozen; real security incident goes unnoticed | Include `?t=${Date.now()}` in snapshot URLs; show "last updated" timestamp per tile |
| Drag-and-drop with no visual drop target indicator | User cannot tell where the camera will land during drag | Use placeholder highlight during drag; animate tile swap |
| Camera names reset to defaults after service restart | Staff rename cameras for the session, reboot, names are gone | Persist names to `camera-layout.json` immediately on save, not on service stop |
| Layout modes with fewer tiles than cameras hide cameras silently | Staff in 2x2 mode forget 9 cameras exist | Always show a "X cameras hidden" badge when layout hides cameras; clicking badge cycles to fuller layout |
| Fullscreen exits but WebRTC keeps streaming (no visible indicator) | Bandwidth wasted; staff confused by go2rtc stream count in admin | Always show "LIVE" badge while WebRTC is active; badge disappears on close |

---

## "Looks Done But Isn't" Checklist

Things that appear complete but are missing critical pieces in NVR dashboard context.

- [ ] **go2rtc CORS:** WebRTC connects from go2rtc's own UI — verify it also connects from rc-sentry-ai's page at port 8096 (different origin)
- [ ] **Snapshot + WebRTC coexistence:** WebRTC fullscreen works — verify snapshot polling does NOT cause NVR to drop the RTSP stream when both are active simultaneously for the same camera
- [ ] **Peer connection cleanup:** Fullscreen dismissed — verify go2rtc `/api/streams` shows 0 viewers for that camera within 5 seconds
- [ ] **Layout persistence cross-origin:** Layout changed at port 8096 — verify the same layout appears at port 3200 after page refresh (not just in the same tab)
- [ ] **Camera names persistence:** Camera renamed and service restarted — verify names survive restart (not just in-memory)
- [ ] **Cold-start delay handled:** Click a camera for first stream — verify a loading indicator appears within 100ms and video appears within 10s
- [ ] **13-camera load:** All 13 cameras added to go2rtc — verify go2rtc does NOT open 13 RTSP connections at startup (lazy mode) and does NOT cause NVR overload
- [ ] **TOML config integrity:** Layout saved — verify `rc-sentry-ai.toml` is NOT modified; only `camera-layout.json` changes
- [ ] **Dual deployment HTML:** Same HTML file served from both ports — verify no hardcoded localhost:8096 URLs in the JavaScript (use relative paths or dynamic origin detection)
- [ ] **Drag-and-drop persistence:** Cameras reordered — verify order persists after page reload, not just within the session

---

## Recovery Strategies

When pitfalls occur despite prevention, how to recover.

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| go2rtc CORS blocking WebRTC | LOW | Add `origin: "*"` to go2rtc.yaml under `[api]`; restart go2rtc (no code rebuild needed) |
| NVR connection limit hit (RTSP refused) | LOW | Pause snapshot polling; restart go2rtc to clear stale sessions; increase NVR connection limit in Dahua config if supported |
| Ghost WebRTC connections accumulating on go2rtc | LOW | go2rtc `/api/streams` shows stuck viewers; restart go2rtc to force-close (brief camera outage); deploy fix for explicit `peerConnection.close()` |
| `rc-sentry-ai.toml` corrupted by layout write | MEDIUM | Restore from git last good commit; deploy camera-layout.json separation; verify startup with config validation |
| Binary size too large due to include_str! growth | MEDIUM | Split HTML into static file served by tower-http `ServeDir`; loses single-binary advantage but recovers compile times |
| Layout diverges between two origins (localStorage mismatch) | LOW | Clear localStorage on both; both origins re-fetch from server; deploy backend persistence endpoint |
| Camera names lost after restart (in-memory only) | LOW | Re-enter names; deploy `camera-layout.json` persistence before next restart |

---

## Pitfall-to-Phase Mapping

How roadmap phases should address these pitfalls.

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| go2rtc CORS blocking WHEP | Phase: go2rtc setup + camera onboarding | `curl -X OPTIONS http://server:1984/api/webrtc` returns `Access-Control-Allow-Origin: *` |
| WHEP cold-start 5-10s delay | Phase: Frontend WebRTC implementation | Click-to-fullscreen shows spinner within 100ms; video within 10s |
| 13 simultaneous WebRTC connections | Phase: Frontend architecture | `activePeerConnection` singleton enforced; go2rtc `/api/streams` never shows more than 1 viewer simultaneously |
| NVR credentials in frontend JS | Phase: go2rtc setup | Grep the served HTML for `192.168.31.18` and `Admin@123` — must return empty |
| include_str! binary bloat | Phase: Frontend architecture | `cargo build --release` time delta less than 3s after adding drag-and-drop; HTML less than 80KB |
| Layout persistence cross-origin | Phase: Backend API | `PUT /api/cameras/layout` endpoint exists; localStorage is cache only |
| Snapshot + WebRTC NVR contention | Phase: go2rtc setup | Activate WebRTC; confirm snapshot polling still returns 200 for same camera |
| Peer connection not closed | Phase: Frontend WebRTC implementation | Playwright test: open fullscreen, close, check go2rtc API shows 0 viewers |
| Layout DOM thrash | Phase: Frontend architecture | Performance tab: layout mode switch less than 200ms, 0 network requests during switch |
| TOML write corruption | Phase: Backend API | `camera-layout.json` is written; `rc-sentry-ai.toml` is unchanged after layout save |

---

## Sources

- [go2rtc WHIP CORS preflight issue #1311](https://github.com/AlexxIT/go2rtc/issues/1311) — `origin:` vs `cors:` config key confusion, confirmed fix
- [go2rtc WHEP SDP delay issue #1392](https://github.com/AlexxIT/go2rtc/issues/1392) — 5-10s cold start is documented go2rtc behavior; codec negotiation cannot be skipped
- [go2rtc WebSocket origin issue #1314](https://github.com/AlexxIT/go2rtc/issues/1314) — WebSocket cross-origin failures separate from WHEP CORS
- [go2rtc WebRTC official docs](https://go2rtc.org/internal/webrtc/) — Symmetric NAT impact, port config, filter candidates
- [go2rtc multiple sessions issue #835](https://github.com/AlexxIT/go2rtc/issues/835) — single RTSP connection reused for multiple output clients; camera session limit still applies
- [go2rtc CPU usage Frigate discussion #14012](https://github.com/blakeblackshear/frigate/discussions/14012) — 700% CPU with multiple streams; transcoding is the trigger
- [go2rtc snapshot latency issue #1736](https://github.com/AlexxIT/go2rtc/issues/1736) — snapshot latency 1-2s from go2rtc due to keyframe wait
- [rust-lang/rust #65818](https://github.com/rust-lang/rust/issues/65818) — include_bytes!/include_str! compile time regression with large files
- [rust-embed crate docs](https://docs.rs/rust-embed/latest/rust_embed/) — compression feature; alternative to raw include_str! for large assets
- [Dahua NVR connection limits — IP Cam Talk forum](https://ipcamtalk.com/threads/dahua-nvr-limits-number-of-cameras-how-to-overcome.51753/) — NVR connection count limits, user-permission stream counts
- [WebRTC peer connection resource limits — TensorWorks](https://tensorworks.com.au/blog/webrtc-stream-limits-investigation/) — practical browser limits before spec limits; ghost connection CPU cost
- [WebRTC one PC per stream — BlogGeek](https://bloggeek.me/webrtc-rtcpeerconnection-one-per-stream/) — resource overhead per RTCPeerConnection
- [RTCPeerConnection CPU leak after close — webtorrent issue #551](https://github.com/webtorrent/webtorrent/issues/551) — connections not GC'd immediately after close
- [Home Assistant 2024.11 go2rtc integration](https://www.home-assistant.io/blog/2024/11/06/release-202411/) — real-world WebRTC + snapshot coexistence patterns with go2rtc
- [Frigate go2rtc configuration guide](https://docs.frigate.video/guides/configuring_go2rtc/) — production go2rtc config patterns for multi-camera NVR setups
- [go2rtc 10+ second initial stream delay issue #1110](https://github.com/AlexxIT/go2rtc/issues/1110) — lazy vs eager RTSP connect tradeoffs
- Direct system knowledge: Dahua 13x 4MP cameras at .18, NVR auth admin/Admin@123, rc-sentry-ai ONNX Runtime constraint, existing snapshot proxy implementation

---

*Pitfalls research for: v16.1 Camera Dashboard Pro — WebRTC streaming + drag-and-drop + layout modes added to existing Rust/Axum NVR dashboard*
*Researched: 2026-03-22 IST*
