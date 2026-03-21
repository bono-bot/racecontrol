# External Integrations

**Analysis Date:** 2026-03-21

## APIs & External Services

**RaceControl Server (Primary Backend):**
- Purpose: All business logic, pod control, billing, authentication, fleet management
- API Client: Fetch API with wrapper at `src/lib/api.ts`
- Base URL: `NEXT_PUBLIC_API_URL` or `window.location` origin (default: http://192.168.31.23:8080)
- Endpoints: All prefixed with `/api/v1/`

**WebSocket Connection (Real-Time Updates):**
- Service: RaceControl Server
- Purpose: Live pod states, billing tickers, telemetry, game launches, activity logs
- Client: Native WebSocket at `src/hooks/useKioskSocket.ts`
- URL: `NEXT_PUBLIC_WS_URL` or `ws://{hostname}:8080/ws/dashboard`
- Event types: pod_list, pod_update, telemetry, lap_completed, billing_session_list, billing_tick, game_state_changed, auth_token_created, assistance_needed, deploy_progress, ac_server_update, group_session_all_validated, GameLaunchRequested

## API Endpoints Consumed

**Health & Status:**
- `GET /api/v1/health` - Server health check (status, version)
- `GET /api/v1/fleet/health` - Pod fleet health (ws_connected, http_reachable, maintenance status per pod)

**Pod Management:**
- `GET /api/v1/pods` - List all pods with current state
- `POST /api/v1/pods/{pod_id}/screen` - Blank/unblank pod display
- `POST /api/v1/pods/{pod_id}/transmission` - Set transmission mode
- `POST /api/v1/pods/{pod_id}/ffb` - Set force feedback preset
- `POST /api/v1/pods/{pod_id}/unrestrict` - Lift kiosk mode enforcement
- `POST /api/v1/pods/{pod_id}/freedom` - Enable unrestricted mode
- `POST /api/v1/pods/{pod_id}/wake|shutdown|restart` - Power management
- `POST /api/v1/pods/{pod_id}/lockdown` - Lock/unlock pod
- `POST /api/v1/pods/{pod_id}/enable|disable` - Enable/disable pod
- `POST /api/v1/pods/{pod_id}/clear-maintenance` - Clear maintenance flag
- `POST /api/v1/pods/wake-all|shutdown-all|restart-all|lockdown-all` - Bulk pod ops

**Driver Management:**
- `GET /api/v1/drivers` - List all drivers
- `POST /api/v1/drivers` - Create new driver (phone, name)

**Billing & Pricing:**
- `GET /api/v1/pricing` - List pricing tiers
- `GET /api/v1/billing/active` - Active billing sessions
- `POST /api/v1/billing/start` - Start billing session (pod_id, driver_id, pricing_tier_id, optional split_count)
- `GET /api/v1/billing/split-options/{minutes}` - Get split duration options for given session length
- `POST /api/v1/billing/continue-split` - Continue next split after current split completion

**Authentication:**
- `POST /api/v1/auth/assign` - Assign driver to pod with pricing/auth type (returns token)
- `POST /api/v1/auth/cancel/{token_id}` - Cancel pending auth token
- `POST /api/v1/auth/start-now` - Start session immediately (consumes token)
- `POST /api/v1/auth/kiosk/validate-pin` - Validate kiosk PIN (returns pod info, allocated time)
- `POST /api/v1/staff/validate-pin` - Validate staff PIN (returns staff_id, staff_name)
- `POST /api/v1/customer/login` - Phone-based customer login (OTP flow)
- `POST /api/v1/customer/verify-otp` - Verify OTP and get auth token
- `POST /api/v1/customer/book` - Book session with token (Bearer auth)
- `POST /api/v1/kiosk/book-multiplayer` - Multiplayer booking with pod count (Bearer auth)

**Game Launcher:**
- `POST /api/v1/games/launch` - Launch game on pod (pod_id, sim_type, launch_args)
- `POST /api/v1/games/relaunch` - Relaunch game
- `POST /api/v1/games/stop` - Stop game
- `POST /api/v1/ac/session/update-config` - Update AC session config (track, track_config, cars)
- `POST /api/v1/ac/session/{session_id}/continuous` - Toggle continuous mode
- `POST /api/v1/ac/session/retry-pod` - Retry pod join to AC session
- `GET /api/v1/customer/ac/catalog` - Get AC tracks, cars, presets, categories

**Kiosk Experiences:**
- `GET /api/v1/kiosk/experiences` - List all kiosk experiences
- `POST /api/v1/kiosk/experiences` - Create new experience
- `GET /api/v1/kiosk/experiences/{id}` - Get single experience
- `PUT /api/v1/kiosk/experiences/{id}` - Update experience
- `DELETE /api/v1/kiosk/experiences/{id}` - Delete experience
- `POST /api/v1/kiosk/pod-launch-experience` - Launch pre-configured experience on pod

**Kiosk Settings:**
- `GET /api/v1/kiosk/settings` - Get venue settings (name, tagline, business hours, spectator config)
- `PUT /api/v1/kiosk/settings` - Update settings

**Wallet (Staff/Premium):**
- `GET /api/v1/wallet/{driver_id}` - Get wallet balance (balance_paise, total_credited, total_debited)
- `POST /api/v1/wallet/{driver_id}/topup` - Add credit (amount_paise, method, notes)
- `POST /api/v1/wallet/{driver_id}/refund` - Refund balance (amount_paise, notes, reference_id)
- `GET /api/v1/wallet/{driver_id}/transactions` - Get transaction history (limit param)

**Activity & Logging:**
- `GET /api/v1/activity` - Global activity log (limit param)
- `GET /api/v1/pods/{pod_id}/activity` - Pod-specific activity log
- `GET /api/v1/logs` - Server logs (lines, level params)

**Debug System:**
- `GET /api/v1/debug/activity` - Debug activity data (pod health, billing events, game events)
- `GET /api/v1/debug/playbooks` - Get debug playbooks (incident remediation guides)
- `POST /api/v1/debug/incidents` - Create debug incident (description, pod_id)
- `GET /api/v1/debug/incidents` - List incidents (status filter)
- `POST /api/v1/debug/diagnose` - Get AI diagnosis for incident
- `PUT /api/v1/debug/incidents/{id}` - Resolve incident (status, resolution_text, effectiveness)

**Kiosk Allowlist:**
- `GET /api/v1/config/kiosk-allowlist` - List allowed processes
- `POST /api/v1/config/kiosk-allowlist` - Add process to allowlist (process_name, notes)
- `DELETE /api/v1/config/kiosk-allowlist/{process_name}` - Remove process from allowlist

## Data Storage

**Databases:**
- Not directly accessed by kiosk
- All data managed by racecontrol server backend
- Driver, billing, and session data stored in server's database

**File Storage:**
- Not used by kiosk
- Pod binaries and configs stored on server, deployed to pods via API

**Caching:**
- Browser-based: React state and hooks manage runtime data
- No persistent cache; state refreshed via WebSocket broadcasts

## Authentication & Identity

**Auth Provider:**
- Custom (Racing Point proprietary)
- PIN-based authentication for staff and customers
- Bearer token (JWT-style) for customer self-service and multiplayer booking
- OTP flow for phone-based customer login

**Implementation:**
- Staff: PIN validation at `POST /api/v1/staff/validate-pin`
- Customer: Phone + OTP at `/api/v1/customer/login` and `/api/v1/customer/verify-otp`
- Kiosk: PIN validation at `POST /api/v1/auth/kiosk/validate-pin`
- Token types: "pin" or "qr" (from api.ts AuthType)

## Monitoring & Observability

**Error Tracking:**
- Not integrated (errors logged to browser console)
- Server-side incident tracking via `/api/v1/debug/incidents`

**Logs:**
- Server logs fetched via `GET /api/v1/logs` in debug page
- Activity logs via `GET /api/v1/activity` and `GET /api/v1/pods/{pod_id}/activity`
- Client-side: browser console.log/warn (no external aggregation)

## CI/CD & Deployment

**Hosting:**
- Racing Point Server (192.168.31.23, port 3300)
- Deployed via Windows scheduled task
- Standalone Next.js output (no separate Node.js runtime required)

**CI Pipeline:**
- Not detected (manual deployment via npm run build + copy standalone to server)

## Environment Configuration

**Required env vars:**
- `NEXT_PUBLIC_API_URL` - Backend API base URL (default: current origin, e.g., http://192.168.31.23:8080)
- `NEXT_PUBLIC_WS_URL` - WebSocket URL for real-time updates (default: ws://{hostname}:8080/ws/dashboard)

**Secrets location:**
- None stored in kiosk (authentication via server backend)
- Bearer tokens generated server-side and passed to client
- PIN validation happens server-side only

## Webhooks & Callbacks

**Incoming:**
- WebSocket events from racecontrol server (one-way server → kiosk push)

**Outgoing:**
- Fetch API calls to racecontrol `/api/v1/*` endpoints
- Commands via WebSocket (e.g., deploy_rolling)

## Real-Time Features

**WebSocket Event Subscriptions:**
- Pod state changes (pod_list, pod_update)
- Telemetry streaming (telemetry frames, lap completion)
- Billing updates (session list, per-session ticks, warnings, session changes)
- Game state changes (launch, stop, error)
- Auth token lifecycle (created, consumed, cleared)
- Assistance requests and camera focus
- Pod activity log entries
- Deployment progress
- AC multiplayer server updates
- Game launch requests

## Integration Points Summary

| Component | Purpose | Protocol | Auth |
|-----------|---------|----------|------|
| racecontrol :8080/api/v1 | All business logic, pod control, billing | HTTP/REST | None (runs on internal network) |
| racecontrol :8080/ws/dashboard | Real-time pod, billing, game, activity updates | WebSocket | None (internal) |
| Browser storage | Session tokens (temporary, not persisted) | N/A | N/A |
| Electron Edge | Display/input (Windows 11 kiosk mode) | IPC (via window.postMessage if needed) | Window context |

---

*Integration audit: 2026-03-21*
