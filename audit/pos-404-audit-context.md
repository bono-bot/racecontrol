# POS Machine 404 Error — MMA Audit Context

## Problem
User reports seeing a "404: This page could not be found" error on the POS machine's Edge browser (kiosk mode).

## POS Configuration
- **Edge command:** `msedge.exe --kiosk http://192.168.31.23:3200/billing --edge-kiosk-type=fullscreen --no-first-run`
- **POS IP:** 192.168.31.20 (LAN) / 100.95.211.1 (Tailscale)
- **Server IP:** 192.168.31.23 (LAN)
- **rc-pos-agent:** running on :8090, build `668e7f48`, healthy
- **Edge:** running with multiple processes (no URL bar visible — kiosk mode)

## What Works (verified from POS → server)
- `http://192.168.31.23:3200/billing` → **200**
- `http://192.168.31.23:3200/billing/pricing` → **200**
- `http://192.168.31.23:3200/billing/history` → **200**
- `http://192.168.31.23:3200/login` → **200**
- `http://192.168.31.23:3200/pods` → **200**
- `http://192.168.31.23:3200/games` → **200**
- `http://192.168.31.23:3200/book` → **200**
- `http://192.168.31.23:3200/_next/static/chunks/a20264063ef2e8e6.css` → **200** (static files OK)

## What 404s (verified from POS → server)
- `http://192.168.31.23:3200/billing/pos` → **404** (route doesn't exist)
- `http://192.168.31.23:3200/billing/session` → **404** (route doesn't exist)
- `http://192.168.31.23:3200/api/v1/pods` → **404** (API is on :8080, not :3200)
- `http://192.168.31.23:3200/kiosk/session` → **404** (wrong app — kiosk is :3300)

## Architecture
- **Web dashboard** (Next.js, :3200): Full management UI with sidebar nav. Billing page uses WebSocket for real-time data, NO REST API calls from billing page itself.
- **Kiosk app** (Next.js, :3300, basePath `/kiosk`): Customer-facing pod selection, booking.
- **Racecontrol API** (Rust/Axum, :8080): REST API + WebSocket server.
- **WebSocket URL:** `NEXT_PUBLIC_WS_URL` env var, defaults to `ws://localhost:8080/ws/dashboard`

## Web App Routes (deployed on server)
All these exist as `.html` files in `C:\RacingPoint\web\.next\server\app\`:
- `/` (index), `/ac-lan`, `/ac-sessions`, `/ai`, `/billing`, `/book`, `/bookings`
- `/cafe`, `/cameras`, `/cameras/playback`, `/drivers`, `/events`, `/flags`
- `/games`, `/games/reliability`, `/kiosk`, `/leaderboards`, `/login`
- `/ota`, `/pods`, `/presenter`, `/sessions`, `/settings`, `/telemetry`
- `/billing/history`, `/billing/pricing`

## Sidebar Navigation (web app)
The sidebar has links to all routes above. Clicking any sidebar link triggers Next.js client-side navigation.

## Key Files
- `web/src/app/billing/page.tsx` — Uses `useWebSocket()` hook, no REST fetches
- `web/src/hooks/useWebSocket.ts` — WS_BASE defaults to `ws://localhost:8080/ws/dashboard`
- `web/src/components/Sidebar.tsx` — 21 nav links, all to valid routes
- `web/src/components/DashboardLayout.tsx` — BackButton with parentMap for billing sub-pages
- `web/next.config.ts` — `output: "standalone"`, `outputFileTracingRoot: path.join(__dirname)`

## Standing Rule Context
- CLAUDE.md warns: `NEXT_PUBLIC_WS_URL` not set → defaults to `ws://localhost:8080` → works on server, fails on remote browsers (POS)
- CLAUDE.md warns: standalone deploy requires `.next/static` copied → verified OK (CSS returns 200)
- CLAUDE.md warns: `appDir` in `required-server-files.json` can have stale build-machine path

## Task
Find ALL possible reasons the POS Edge browser could show a 404 error while loading or navigating from `http://192.168.31.23:3200/billing`. Consider:
1. Next.js routing edge cases (client-side navigation, RSC data fetches, _rsc queries)
2. WebSocket disconnection leading to error pages
3. Edge kiosk mode specific behaviors
4. AuthGate component blocking/redirecting
5. Stale builds or build hash mismatches
6. _next/data/ prefetch requests for non-existent routes
7. Browser session restore in kiosk mode
8. Service worker or cache issues
9. Middleware redirects
10. Any other possible cause

For each cause found, rate severity (P0-P3) and provide: root cause, how to reproduce, how to fix, how to verify.
