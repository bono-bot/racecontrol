# Requirements: v16.1 Camera Dashboard Pro

**Defined:** 2026-03-22
**Core Value:** Staff can monitor all 13 venue cameras from a professional, low-latency dashboard — grid overview + instant fullscreen streaming

## v16.1 Requirements

### Infrastructure (INFRA)

- [x] **INFRA-01**: All 13 NVR cameras are registered in go2rtc with RTSP sub-stream URLs
- [x] **INFRA-02**: go2rtc CORS is configured and verified for cross-port WebRTC access from :8096 and :3200
- [ ] **INFRA-03**: Each camera has a configurable display_name and display_order in rc-sentry-ai.toml
- [ ] **INFRA-04**: /api/v1/cameras returns display_name, display_order, nvr_channel, and zone for each camera

### Streaming (STRM)

- [ ] **STRM-01**: User can click any camera tile to open fullscreen with live WebRTC video via go2rtc
- [ ] **STRM-02**: Only one WebRTC connection is active at a time — previous connection is torn down on camera switch
- [ ] **STRM-03**: Hovering a camera tile for >500ms pre-warms the WebRTC connection to reduce cold-start delay
- [ ] **STRM-04**: Fullscreen view shows camera name, connection status indicator, and close button (click or Escape)

### Layout (LYOT)

- [ ] **LYOT-01**: User can switch between layout modes: 1x1, 2x2, 3x3, 4x4 grid presets via toolbar buttons
- [ ] **LYOT-02**: User can drag cameras to reorder their position in the grid
- [ ] **LYOT-03**: Grid layout (mode + camera order) persists across page reloads via server-side camera-layout.json
- [ ] **LYOT-04**: PUT /api/v1/cameras/layout saves layout preferences to camera-layout.json
- [ ] **LYOT-05**: Cameras can be grouped by zone (entrance, pods, reception) with zone headers in the grid

### UI/UX (UIUX)

- [ ] **UIUX-01**: Dashboard fills entire browser viewport with no scrollbars (compact toolbar, edge-to-edge grid)
- [ ] **UIUX-02**: Each camera tile shows status indicator (green=live, red=offline, yellow=stale)
- [ ] **UIUX-03**: Loading state shown during WebRTC connection setup (spinner or placeholder)
- [ ] **UIUX-04**: Smooth CSS transition when switching layout modes (no DOM rebuild flash)
- [ ] **UIUX-05**: Refresh rate selector in toolbar (0.2, 0.5, 1 fps for snapshot grid)

### Deployment (DPLY)

- [ ] **DPLY-01**: cameras.html embedded in rc-sentry-ai serves the full dashboard at /cameras/live
- [ ] **DPLY-02**: Standalone camera dashboard page accessible from web dashboard on server .23 at /cameras
- [ ] **DPLY-03**: Both deployments share identical feature set (layouts, WebRTC, drag-to-rearrange)

## Future Requirements

### Advanced Features

- **ADV-01**: PTZ (pan-tilt-zoom) controls for supported cameras
- **ADV-02**: Motion detection overlay on camera tiles
- **ADV-03**: Recording playback timeline integrated into fullscreen view
- **ADV-04**: Multi-monitor support (pop-out individual cameras)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Simultaneous 13x WebRTC streams | NVR connection limit + GPU decoder overload — hybrid model only |
| RTSP direct in browser | Browsers cannot play RTSP natively |
| Cloud-accessible streaming | DMSS HD app already covers this — no custom cloud streaming |
| Mobile app | Web dashboard is responsive enough for phone/tablet use |
| Audio streaming | Security cameras — visual monitoring only |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| INFRA-01 | Phase 145 | Complete |
| INFRA-02 | Phase 145 | Complete |
| INFRA-03 | Phase 146 | Pending |
| INFRA-04 | Phase 146 | Pending |
| STRM-01 | Phase 147 | Pending |
| STRM-02 | Phase 147 | Pending |
| STRM-03 | Phase 147 | Pending |
| STRM-04 | Phase 147 | Pending |
| LYOT-01 | Phase 147 | Pending |
| LYOT-02 | Phase 147 | Pending |
| LYOT-03 | Phase 147 | Pending |
| LYOT-04 | Phase 146 | Pending |
| LYOT-05 | Phase 147 | Pending |
| UIUX-01 | Phase 147 | Pending |
| UIUX-02 | Phase 147 | Pending |
| UIUX-03 | Phase 147 | Pending |
| UIUX-04 | Phase 147 | Pending |
| UIUX-05 | Phase 147 | Pending |
| DPLY-01 | Phase 147 | Pending |
| DPLY-02 | Phase 148 | Pending |
| DPLY-03 | Phase 148 | Pending |

**Coverage:**
- v16.1 requirements: 21 total
- Mapped to phases: 21
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation (phases 145-148 assigned)*
