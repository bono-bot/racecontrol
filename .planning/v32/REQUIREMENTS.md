# Requirements: v32.0 Cloud Dashboard

**Defined:** 2026-03-31
**Designed via MMA:** 5 models (R1, Gemini Flash, GPT-5.4 Nano, Grok 4.1, MiMo Pro), consensus-driven
**Core Value:** Uday monitors everything from phone/laptop — fleet health, revenue, incidents, cameras — without being at the venue

## Phase 1: Dashboard Core (Must-Have MVP)

- [ ] **CD-01**: Real-time pod status grid — 8 pods showing online/offline/in-use/error, current session, driver name, time remaining
- [ ] **CD-02**: Daily revenue summary — today's total, credits consumed, active sessions count
- [ ] **CD-03**: Critical alert push notifications — pod down, billing error, security alert (WhatsApp + browser push via FCM)
- [ ] **CD-04**: Fleet health overview — leverage existing /api/v1/fleet/health, incident banner with last incident time
- [ ] **CD-05**: Secure authentication — OAuth2 or magic link for Uday's devices, JWT-based session
- [ ] **CD-06**: Mobile-responsive Next.js app at cloud.racingpoint.cloud (port 3600)
- [ ] **CD-07**: API aggregation endpoints — /api/v1/dashboard/overview, /api/v1/dashboard/revenue, /api/v1/dashboard/incidents

## Phase 2: Analytics + Cameras

- [ ] **CD-08**: Revenue analytics — daily/weekly/monthly charts, peak hour detection, per-pod revenue breakdown
- [ ] **CD-09**: Live camera feeds — 13 RTSP streams via HLS/WebRTC proxy, selectable grid, low-latency
- [ ] **CD-10**: MMA diagnostic history — timeline of autonomous fixes, searchable by pod + date, 4-step protocol details
- [ ] **CD-11**: Incident timeline — deep-dive logs with filters (severity, pod, date range)
- [ ] **CD-12**: Revenue range selector — yesterday, week-to-date, month-to-date comparisons

## Phase 3: Operations + Intelligence

- [ ] **CD-13**: Staff attendance log — clock-in/out from POS, daily summary
- [ ] **CD-14**: Customer flow — active drivers count, session queue, daily footfall from billing data
- [ ] **CD-15**: Automated daily summary — WhatsApp/email at 10 PM IST with day's metrics
- [ ] **CD-16**: Predictive alerts — "Pod 3 showing early failure signs" from MMA KB patterns
- [ ] **CD-17**: KB dashboard — view fleet knowledge base, permanent fixes found, workaround→permanent upgrade pipeline

## Phase 4: Polish + Multi-User

- [ ] **CD-18**: Custom alert thresholds — Uday configures what triggers notifications
- [ ] **CD-19**: Multi-user roles — manager view (limited), owner view (full), staff view (basic)
- [ ] **CD-20**: Dashboard widgets — drag-and-drop layout customization
- [ ] **CD-21**: Offline mode — PWA with cached last-known state when network drops
- [ ] **CD-22**: Historical reports — exportable PDF/CSV for accounting

## Architecture (MMA consensus 5/5)

```
Phone/Laptop (Uday)
    |
    HTTPS
    |
cloud.racingpoint.cloud (port 3600)
    |
    Next.js Cloud Dashboard (NEW — on Bono VPS)
    |
    ├── API Gateway (:3100) — aggregation endpoints
    |       ├── Cloud racecontrol (:8080) — synced venue data
    |       ├── rc-guardian — fleet health + incident stream
    |       └── comms-link (:8765) — real-time events via WS
    |
    ├── Camera Proxy — HLS transcoding from venue NVR
    |       └── Venue NVR (192.168.31.18) via Tailscale
    |
    └── Push Notifications — FCM for browser, WhatsApp for critical
```

## Technology Stack (MMA consensus 4/5)

- **Frontend:** Next.js 16 (consistent with existing admin/dashboard apps)
- **Hosting:** Bono VPS (cloud.racingpoint.cloud, port 3600)
- **Data source:** Cloud racecontrol DB replica (synced every 30s)
- **Real-time:** WebSocket via comms-link for live updates
- **Camera:** go2rtc or FFmpeg HLS proxy on James machine, served via Tailscale
- **Push:** Firebase Cloud Messaging (browser) + Evolution API (WhatsApp)
- **Auth:** JWT with magic link (email to usingh@racingpoint.in)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Native mobile app | PWA with responsive design covers phone use case |
| AI chatbot in dashboard | Existing WhatsApp bot serves this purpose |
| Venue WiFi monitoring | Not in current infrastructure |
| Payment gateway integration | Billing uses credits, not direct payments |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CD-01..CD-07 | Phase 1 | Pending |
| CD-08..CD-12 | Phase 2 | Pending |
| CD-13..CD-17 | Phase 3 | Pending |
| CD-18..CD-22 | Phase 4 | Pending |

**Coverage:** 22 requirements across 4 phases

---
*Requirements defined: 2026-03-31 via MMA (5 models, consensus-driven)*
