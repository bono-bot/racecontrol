# Project Research Summary

**Project:** v16.1 Camera Dashboard Pro
**Domain:** Hybrid streaming NVR dashboard — WebRTC on-demand + snapshot grid, 13x Dahua cameras
**Researched:** 2026-03-22
**Confidence:** HIGH

## Executive Summary

v16.1 Camera Dashboard Pro layers professional NVR UX on top of the existing v16.0 snapshot infrastructure in rc-sentry-ai. The research consistently converges on a single core pattern: a **hybrid streaming model** where all 13 cameras are displayed as JPEG snapshots in the grid (low bandwidth, low NVR load, no connection overhead) and WebRTC is established only for the single camera the user clicks into fullscreen. This pattern is validated by Frigate NVR's production architecture, proven against the Dahua NVR's connection constraints, and fits the existing SnapshotCache/NvrClient infrastructure without any new Rust crates. The key technical enabler is go2rtc's bundled `video-stream.js` custom element (`<video-stream>`), which handles the full WebRTC offer/answer/ICE exchange and protocol fallback automatically — no raw WebRTC signaling code needs to be written.

The recommended implementation requires no new Rust crates, no npm libraries beyond the optional `@dnd-kit/core` for drag-to-reorder in the Next.js target, and no build step changes. The single embedded HTML page constraint is maintained. The go2rtc instance already running on James (.27:1984) handles RTSP-to-WebRTC relay; the main infrastructure prerequisite is adding all 13 cameras to `go2rtc.yaml` (currently only 3 are configured). Layout persistence uses localStorage for the single-browser case, but a server-side `PUT /api/cameras/layout` endpoint writing to a separate `camera-layout.json` file is required to synchronize state across the dual deployment origins (:8096 and :3200). Camera names and default display order belong in rc-sentry-ai.toml, not in localStorage.

The critical risks are all go2rtc integration issues that must be resolved before frontend code is written: CORS configuration uses the `origin:` key (not `cors:` — the wrong key is silently ignored), cold-start delays of 5-10 seconds require hover pre-warming, and the snapshot proxy plus WebRTC RTSP connections compete for the same Dahua NVR connection slots. The 13-simultaneous-WebRTC anti-pattern is the failure mode research flags most strongly — it overloads the NVR and browser regardless of how it is implemented, and the singleton `activePeerConnection` pattern must be enforced as an architectural constraint before any tile rendering code is written.

---

## Key Findings

### Recommended Stack

The existing stack (Axum 0.7, reqwest 0.12, tokio, tower-http CORS, serde_json, `include_str!` embedded HTML, SnapshotCache) requires zero new Rust crates. The only new runtime dependency is the go2rtc-hosted JavaScript library loaded at runtime. HTML5 native drag-and-drop, CSS Grid, and localStorage are all browser-native — no bundler, no build step. The embedded HTML size budget must stay under 80KB to avoid `include_str!` compile-time regression (rust-lang/rust#65818).

**Core technologies:**
- `go2rtc video-stream.js` (loaded from :1984 at runtime): `<video-stream>` custom element handles WebRTC offer/answer/ICE + MSE/MJPEG fallback — never vendor this file; must load from the live go2rtc instance to stay in sync with server protocol
- CSS Grid class swap: layout mode switching (1x1/2x2/3x3/4x4) via single class change on `.grid` — zero JS layout math
- HTML5 Drag and Drop API: camera grid reorder via `draggable` + `dragstart`/`dragover`/`drop` events — `dragover` MUST call `e.preventDefault()` or the `drop` event never fires
- `localStorage` (key: `rp_camera_layout_v1`): layout persistence for single-browser use; server-side `camera-layout.json` is the cross-origin source of truth
- go2rtc WebSocket signaling (`/api/ws?src=<stream_name>`): WebRTC uses `webrtc/offer`, `webrtc/answer`, `webrtc/candidate` message types over WebSocket — not WHEP HTTP

**Critical config requirements:** go2rtc sub-stream (`subtype=1`) must be used for all 13 NVR channels (~512Kbps vs ~4Mbps for main stream). `Admin@123` must be percent-encoded as `Admin%40123` in RTSP URLs. CORS requires `origin: "*"` under `[api]` in go2rtc.yaml — not `cors:`, which is silently ignored.

### Expected Features

**Must have — table stakes (v16.1 core):**
- TS-1: Layout mode switcher (1x1, 2x2, 3x3, 4x4) — the first thing any NVR user looks for
- TS-2: Camera friendly names (`display_name` in TOML, shown in tile header — existing `name` field is a stream ID, not a human label)
- TS-3: WebRTC fullscreen on camera click via go2rtc — existing click-to-fullscreen shows a cached snapshot, not live video
- TS-5: Layout persistence to localStorage (survives page reload)
- TS-6: Per-tile loading skeleton and offline/loading/error state distinction
- TS-7: Camera count summary in header ("12/13 online")
- Infra prerequisite: All 13 cameras registered in go2rtc.yaml before any frontend WebRTC work begins

**Should have — differentiators (v16.1.x after validation):**
- D-1: Drag-to-rearrange grid order with localStorage persistence + server sync
- D-4: Configurable snapshot refresh rate presets (5s incident / 15s active / 30s normal)
- D-5: Snapshot timestamp overlay per tile ("last updated 3s ago")

**Defer to v16.2+:**
- D-3: Camera zone grouping (requires TOML design decision, more operational data needed)
- D-6: Single-camera featured view (fullscreen modal covers the core need)
- NVR playback integration (`/cameras/playback` page already stubbed)

**Never build (anti-features):**
- 13 simultaneous WebRTC streams — NVR connection limit exceeded, browser GPU decoder bottlenecked
- RTSP direct browser streaming — browsers cannot play RTSP natively
- PTZ controls — no PTZ cameras at Racing Point
- Cloud-accessible camera dashboard — use DMSS HD app; proxying live video through VPS adds bandwidth cost and credential risk

### Architecture Approach

Two parallel deployment targets share the same rc-sentry-ai backend but have independent frontend implementations: `cameras.html` (embedded via `include_str!`, served at :8096, vanilla JS) and `web/src/app/cameras/page.tsx` (Next.js staff dashboard at :3200, React). Both connect to rc-sentry-ai for snapshots and camera config, and directly to go2rtc at :1984 for WebRTC signaling. Layout state (grid size + drag order) lives in localStorage as cache; `camera-layout.json` is the server-side source of truth. The SnapshotCache and NvrClient require no changes.

**Major components:**
1. `go2rtc :1984` — RTSP relay + WebRTC signaling; needs ch1-ch13 added to go2rtc.yaml; no go2rtc source changes
2. `SnapshotCache / NvrClient` (rc-sentry-ai) — unchanged; background-refreshes all 13 channels from NVR HTTP snapshot endpoint
3. `cameras_list_handler` (mjpeg.rs) — extend JSON response to include `nvr_channel`, `stream_name`, `display_order`
4. `CameraConfig` (config.rs) — add `display_order: Option<u32>`; TOML gets all 13 cameras with stream names
5. `cameras.html` — full rewrite in-place: layout selector, drag-to-reorder, WebRTC fullscreen modal, localStorage persistence
6. `web/cameras/page.tsx` — full rewrite: snapshot polling + `useWebRTC` hook + layout controls; requires `NEXT_PUBLIC_SENTRY_URL` and `NEXT_PUBLIC_GO2RTC_WS_BASE` env vars baked at build time
7. `camera-layout.json` (new, server-side) — user preferences (camera order, display names) persisted separately from rc-sentry-ai.toml; written atomically with temp-file rename; TOML is read-only at runtime
8. `PUT /api/cameras/layout` endpoint — cross-origin layout synchronization; localStorage is the cache, this is the source of truth

**Key architectural patterns:**
- Hybrid streaming: snapshot grid for ambient monitoring (all 13 cameras, 1-2fps JPEG); one WebRTC connection for actively viewed fullscreen camera only
- go2rtc stream name indirection: browser uses stream names only (`ch1`–`ch13`) from API response; NVR IP and credentials never appear in JS
- Singleton WebRTC: module-level `let activePeerConnection = null` — any new connection closes the active one first; `teardownRtc()` written before any connection-opening code
- CSS grid class swap for layout switching: all 13 tiles persist in DOM across mode changes; show/hide with `display:none`, never destroy/recreate

### Critical Pitfalls

1. **go2rtc CORS blocks WebRTC signaling** — Use `origin: "*"` under `[api]` in go2rtc.yaml (not `cors:` — wrong key, silently ignored by go2rtc). Verify with `curl -X OPTIONS http://192.168.31.27:1984/api/webrtc` returning `Access-Control-Allow-Origin: *` before writing any frontend WebRTC code. go2rtc never logs rejected CORS preflight — the failure is invisible server-side.

2. **go2rtc cold-start delay (5-10 seconds on first viewer)** — Lazy RTSP connect triggers a fresh RTSP probe + codec negotiation per camera. Prevention: pre-warm streams on camera tile hover (send `GET /api/streams?src=ch{N}` probe before user clicks); show a loading spinner within 100ms of click; do not disable lazy loading (prevents 13 RTSP connections at go2rtc startup).

3. **Snapshot proxy + WebRTC RTSP competing for NVR connection slots** — SnapshotCache hits NVR HTTP snapshot.cgi; go2rtc hits NVR RTSP; Dahua NVRs count both against per-channel connection limits. Two options: (a) route snapshots through go2rtc's frame API (`GET /api/frame.jpeg?src=ch{N}`) to share one RTSP connection per camera, or (b) pause snapshot polling for the camera currently open in WebRTC fullscreen. Decision must be made in Phase 1 before snapshot cache is assumed stable during WebRTC.

4. **WebRTC peer connection not closed on fullscreen dismiss** — Removing the video element from the DOM does NOT close the `RTCPeerConnection`. Ghost connections accumulate on go2rtc and consume NVR RTSP slots. Write `teardownRtc()` (explicit `pc.close()` + stop all tracks) and call it on: fullscreen close, layout mode change, camera switch, page `beforeunload`, and `visibilitychange`. Verify via go2rtc `/api/streams` — viewer count must drop to 0 within 5 seconds of close.

5. **Layout persistence diverges across dual origins** — localStorage is scoped to origin; layout changes at :8096 are invisible at :3200. Server-side `camera-layout.json` + `PUT /api/cameras/layout` is required as source of truth. localStorage is the fast-load cache only. This must be locked in the backend API phase before drag-to-reorder is built.

6. **NVR credentials in frontend JS** — If the browser constructs go2rtc WebSocket URLs with RTSP credentials embedded, admin password is visible in DevTools to any LAN user. All 13 streams must be pre-configured in go2rtc.yaml with canonical names; frontend JavaScript uses only stream names. Grep the served HTML for `192.168.31.18` and `Admin%40123` — must return empty.

7. **TOML round-trip corruption from layout write** — The `toml` crate serializer drops all comments and reorders sections. If a runtime endpoint writes user preferences back to rc-sentry-ai.toml, git history becomes unreadable and a partial write during crash corrupts startup. User preferences go in `camera-layout.json`; TOML is read-only at runtime.

---

## Implications for Roadmap

The architecture has a clear dependency chain. Phases 1-3 are sequential (each gates the next). Phases 4a and 4b are parallel — they share the same backend but have independent frontend implementations.

### Phase 1: go2rtc Infrastructure + CORS Verification

**Rationale:** All frontend WebRTC work is gated on go2rtc having all 13 cameras configured, CORS working from the rc-sentry-ai origin, and the NVR coexistence strategy decided. Pitfalls 1, 3, and 6 (credential exposure) must be resolved here before a single line of frontend WebRTC code is written. This is config work only — no Rust changes.
**Delivers:** go2rtc.yaml with ch1-ch13 RTSP entries (sub-stream, percent-encoded credentials, no H.264 transcoding); `origin: "*"` under `[api]`; verified CORS response via curl; NVR coexistence decision (snapshot-through-go2rtc vs pause-during-WebRTC) tested against live hardware; cold-start behavior observed (lazy vs eager RTSP connect tradeoff)
**Addresses:** Pitfall 1 (CORS), Pitfall 3 (NVR connection contention), Pitfall 6 (credential exposure in JS)
**Avoids:** Starting frontend before infrastructure is proven — CORS failure is completely invisible server-side
**Research flag:** Standard — go2rtc yaml format is documented; CORS key confusion has a known fix; verification is manual on live hardware

### Phase 2: Backend Config + API Extension

**Rationale:** Both frontend targets fetch `/api/v1/cameras` on page load. The extended response shape (nvr_channel, stream_name, display_order) and the `camera-layout.json` persistence endpoint are shared prerequisites for both cameras.html and page.tsx rewrites. The config/API split decision (camera-layout.json vs TOML mutation) must be locked here to prevent Pitfall 7.
**Delivers:** `display_order: Option<u32>` in CameraConfig; rc-sentry-ai.toml updated with all 13 cameras + stream names matching go2rtc.yaml; cameras_list_handler returning full camera info; `PUT /api/cameras/layout` endpoint writing atomically to camera-layout.json; `GET /api/cameras/layout` for initial load on page open
**Addresses:** TS-2 (friendly names in API), TS-4 (status consistency), cross-origin layout sync (Pitfall 5), TOML integrity (Pitfall 7)
**Avoids:** Building either frontend against a partial API response that silently returns null stream_names
**Research flag:** Standard — Axum handler, serde_json, tokio::fs atomic write (write-to-tmp then rename) are established patterns in this codebase

### Phase 3: Frontend Architecture Foundation

**Rationale:** Before writing any camera tile or WebRTC code in either frontend, the DOM strategy (persistent tiles, CSS class swap), singleton WebRTC pattern, and `teardownRtc()` must be defined. This phase writes the architectural contracts that Pitfalls 4 and 9 (DOM thrash) require. It applies to both frontend targets simultaneously.
**Delivers:** DOM architecture decision locked in code comments; `teardownRtc()` written before any connection-opening code; CSS grid classes for 1x1/2x2/3x3/4x4; `activePeerConnection` singleton; HTML size budget confirmed under 80KB; localStorage schema `rp_camera_layout_v1` defined; Next.js hydration pattern confirmed (`useEffect` + hydrated flag, never `useState` initializer per CLAUDE.md)
**Addresses:** Pitfall 4 (ghost connections), Pitfall 9 (DOM thrash on layout switch), Pitfall 5 (`include_str!` binary bloat)
**Avoids:** The dominant failure mode — writing tile rendering before connection lifecycle management, then retrofitting teardown into complete code

### Phase 4a: cameras.html Rewrite (Embedded Dashboard)

**Rationale:** Parallel with 4b after Phases 1-3 complete. Vanilla JS implementation with no bundler, targeting :8096. Snapshot polling already works — this adds layout controls, drag-to-reorder, and WebRTC fullscreen.
**Delivers:** Full cameras.html rewrite with layout selector (CSS grid class swap), drag-to-reorder (HTML5 DnD with `e.preventDefault()` on dragover), WebRTC fullscreen modal with loading spinner and teardownRtc, localStorage read/write, hover pre-warming for cold-start mitigation, server layout sync via PUT endpoint
**Addresses:** TS-1, TS-2, TS-3, TS-5, TS-6, TS-7 from FEATURES.md; D-1 if scoped in
**Avoids:** Pitfall 2 (cold-start) via hover pre-warm; Pitfall 4 (singleton enforcement before tile code)
**Research flag:** Standard — HTML5 DnD and vanilla JS WebRTC pattern fully specified in ARCHITECTURE.md with working code samples

### Phase 4b: Next.js Web Dashboard Rewrite (Server Dashboard)

**Rationale:** Parallel with 4a. React implementation targeting :3200. Adds `useWebRTC` hook, `CameraGridSkeleton`, and Tailwind grid layout classes. Requires Next.js rebuild with `NEXT_PUBLIC_` env vars baked at build time. `.next/static` must be copied into `.next/standalone/` per standing rule before deploy.
**Delivers:** Full web/cameras/page.tsx rewrite with `useWebRTC` hook in `web/src/hooks/useWebRTC.ts`, snapshot polling via `useEffect`, layout controls with Tailwind `grid-cols-N`, drag-to-reorder via React drag state or `@dnd-kit/core`, hydration-safe localStorage (useEffect + hydrated flag pattern), server layout sync via PUT endpoint; `CameraGridSkeleton` for hydration gap
**Addresses:** Same feature set as Phase 4a; Next.js hydration safety (CLAUDE.md rule: never read localStorage in useState initializer)
**Avoids:** SSR hydration mismatch; standalone deploy without static copy; hardcoded LAN IPs in source (must use NEXT_PUBLIC_ env vars)
**Research flag:** Standard — Next.js patterns match existing codebase conventions; @dnd-kit/core is the correct library (react-beautiful-dnd deprecated by Atlassian)

### Phase Ordering Rationale

- Phase 1 before all others: CORS and NVR connection strategy must be verified on real hardware before any frontend code assumes they work. The CORS failure mode is invisible server-side — go2rtc logs nothing on rejected preflight. This is the single most common reason WebRTC dashboards ship broken.
- Phase 2 before Phases 4a/4b: Both frontends fetch `/api/v1/cameras` on page load. If stream_name is missing from the response, WebRTC fullscreen silently fails with a malformed WebSocket URL.
- Phase 3 before Phases 4a/4b: `teardownRtc()` must be written before any connection-opening code. This is the single most important sequencing constraint from pitfalls research — retrofitting explicit connection cleanup into already-written tile rendering code is error-prone and incomplete.
- Phase 4a and 4b are parallel: they share the Phase 1-3 backend but have independent frontend implementations. Both can proceed simultaneously once Phases 1-3 are complete.

### Research Flags

Phases with well-documented patterns (research-phase not needed):
- Phase 1: go2rtc yaml config format is in official docs; CORS key confusion is a known documented issue
- Phase 2: Axum route + serde_json + tokio::fs atomic write is established in this codebase; no new patterns
- Phase 4a: HTML5 DnD and vanilla JS WebRTC patterns are fully specified in ARCHITECTURE.md with working code samples
- Phase 4b: Next.js patterns match existing CLAUDE.md conventions; @dnd-kit/core usage is standard

Phases that may benefit from targeted live verification before implementation:
- Phase 1 (NVR coexistence): The choice between routing snapshots through go2rtc's frame API vs pausing snapshot polling during WebRTC has performance implications. go2rtc's frame API has 1-2s snapshot latency (issue #1736 — keyframe wait). Test both options against the live NVR before committing. The snapshot cache currently returns cached JPEGs in milliseconds; switching to go2rtc frames adds latency unless pre-warmed.
- Phase 4a/4b (H.265 browser decode): Chrome 136+/Safari 18+ is required for H.265 WebRTC relay without transcoding. Browser version on the staff dashboard machine (.23 or .27) has not been independently verified. If browser is older, ch1-ch13 go2rtc.yaml entries need ffmpeg H.264 transcoding — adding CPU overhead and a new entry pattern alongside the existing detection pipeline entries.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | go2rtc source verified via GitHub + DeepWiki; HTML5 APIs per MDN; existing Cargo.toml confirmed; no new Rust crates required |
| Features | HIGH | Codebase audit of cameras/page.tsx, nvr.rs, config.rs, mjpeg.rs; competitor analysis of DMSS HD, Hik-Connect, Frigate, Blue Iris, camera.ui |
| Architecture | HIGH | All integration points verified against live code + go2rtc source; WebRTC protocol message types confirmed against video-rtc.js |
| Pitfalls | HIGH (go2rtc integration), MEDIUM (WebRTC connection management), LOW (layout persistence tradeoffs) | go2rtc CORS/cold-start from go2rtc issue tracker with confirmed fixes; connection management from WebRTC post-mortems; localStorage tradeoffs from pattern inference |

**Overall confidence:** HIGH

### Gaps to Address

- **Browser H.265 support on staff machines:** Chrome 136+/Safari 18+ required for H.265 WebRTC relay. Browser version on staff dashboard machine (.23) not independently verified. Check before Phase 4a/4b — if unsupported, add H.264 transcoding entries to go2rtc.yaml for ch1-ch13 (separate from existing `entrance_h264` detection pipeline entries).
- **NVR per-channel connection limit (exact number):** Dahua NVR model-specific limits documented as "typically 2-4" from community sources. Test in Phase 1: connect snapshot cache + one go2rtc WebRTC stream to the same channel and confirm NVR accepts both without dropping either.
- **go2rtc frame API snapshot latency:** Routing all snapshots through go2rtc (`/api/frame.jpeg`) eliminates NVR connection contention but adds 1-2s keyframe wait latency. If this is unacceptable, the "pause snapshot polling during WebRTC" strategy is preferred. Must be tested in Phase 1 before the snapshot pipeline is assumed stable during WebRTC sessions.

---

## Sources

### Primary (HIGH confidence)
- go2rtc `www/video-rtc.js` (GitHub source) — `src` setter, `mode` attribute, `background` property, WebRTC protocol message types (`webrtc/offer`, `webrtc/answer`, `webrtc/candidate`)
- go2rtc `www/stream.html` (GitHub source) — `<video-stream>` custom element, `api/ws?src=` URL format
- DeepWiki AlexxIT/go2rtc — WebRTC protocol documentation, VideoStream/VideoRTC class, protocol fallback order
- MDN Web Docs — HTML Drag and Drop API (`draggable`, `dragstart`, `dragover`, `drop`, `e.preventDefault()` requirement); Window.localStorage; JSON.parse/stringify
- Live codebase audit (2026-03-22): `mjpeg.rs`, `nvr.rs`, `config.rs`, `cameras.html`, `Cargo.toml`, `web/src/app/cameras/page.tsx`, `C:\RacingPoint\go2rtc\go2rtc.yaml`

### Secondary (MEDIUM confidence)
- go2rtc issue #1311 — `origin:` vs `cors:` CORS key confusion, confirmed fix
- go2rtc issue #1392 — 5-10s cold start documented behavior; codec negotiation cannot be skipped
- go2rtc issue #1736 — snapshot latency 1-2s from go2rtc due to keyframe wait
- go2rtc issue #835 — single RTSP connection reused for multiple output clients; camera session limit still applies
- Frigate go2rtc configuration guide — production multi-camera config patterns, iframe embed pattern
- Home Assistant 2024.11 go2rtc integration — real-world snapshot + WebRTC coexistence patterns
- DMSS HD (Google Play + Dahua Wiki), Hik-Connect, Blue Iris, camera.ui — competitor feature analysis
- Dahua NVR RTSP URL format (`channel=N&subtype=1`, percent-encoding) — community + Frigate docs + GitHub Discussion #14956
- rust-lang/rust#65818 — `include_str!` compile time regression with large files

### Tertiary (LOW confidence)
- H.265 WebRTC support in Chrome 136+/Safari 18+ — from go2rtc documentation; browser versions on staff machines not independently verified
- Dahua NVR connection limits "typically 2-4 per channel" — from IP Cam Talk forum; exact limit for Racing Point NVR model unverified
- localhost vs. LAN CSS transitions performance ("lower-spec machines" threshold) — pattern inference, not benchmarked

---
*Research completed: 2026-03-22 IST*
*Ready for roadmap: yes*
