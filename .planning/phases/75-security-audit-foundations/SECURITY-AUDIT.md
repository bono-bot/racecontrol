# Security Audit: Racing Point eSports — racecontrol

**Date:** 2026-03-20
**Scope:** All exposed services — racecontrol (:8080), rc-agent (:8090), rc-sentry (:8091)
**Purpose:** Baseline security posture before Phase 76+ hardening

---

## Endpoint Inventory

### 1a. racecontrol :8080

All API routes are nested under `/api/v1` via `Router::new().nest("/api/v1", api_routes())` in `main.rs:535`. Additional root-level routes exist for WebSocket, registration, and health.

**Important:** The `jwt_error_to_401` middleware (line 554) is applied globally but does NOT enforce authentication. It only reformats JWT-related errors into 401 responses. No route has auth middleware — all auth is opt-in per handler via `extract_driver_id()`.

#### Tier: Public (no auth needed)

| # | Method | Path | Auth | Gap |
|---|--------|------|------|-----|
| 1 | GET | `/api/v1/health` | None | OK |
| 2 | GET | `/api/v1/venue` | None | OK |
| 3 | GET | `/api/v1/fleet/health` | None | OK |
| 4 | POST | `/api/v1/customer/login` | None | OK |
| 5 | POST | `/api/v1/customer/verify-otp` | None | OK |
| 6 | POST | `/api/v1/customer/register` | None | OK |
| 7 | GET | `/api/v1/wallet/bonus-tiers` | None | OK |
| 8 | GET | `/api/v1/public/leaderboard` | None | OK |
| 9 | GET | `/api/v1/public/leaderboard/{track}` | None | OK |
| 10 | GET | `/api/v1/public/circuit-records` | None | OK |
| 11 | GET | `/api/v1/public/vehicle-records/{car}` | None | OK |
| 12 | GET | `/api/v1/public/drivers` | None | OK |
| 13 | GET | `/api/v1/public/drivers/{id}` | None | OK |
| 14 | GET | `/api/v1/public/time-trial` | None | OK |
| 15 | GET | `/api/v1/public/laps/{lap_id}/telemetry` | None | OK |
| 16 | GET | `/api/v1/public/sessions/{id}` | None | OK |
| 17 | GET | `/api/v1/public/championships/{id}/standings` | None | OK |
| 18 | GET | `/api/v1/public/events` | None | OK |
| 19 | GET | `/api/v1/public/events/{id}` | None | OK |
| 20 | GET | `/api/v1/public/events/{id}/sessions` | None | OK |
| 21 | GET | `/api/v1/public/championships` | None | OK |
| 22 | GET | `/api/v1/public/championships/{id}` | None | OK |
| 23 | GET | `/` (root health) | None | OK |
| 24 | GET | `/register` (HTML page) | None | OK |

**Public route count: 24**

#### Tier: Customer (JWT required via extract_driver_id)

These routes should require a valid customer JWT. The `extract_driver_id()` function is called inside individual handlers (not middleware). Routes marked "YES" below actually call `extract_driver_id()` in their handler; "MISSING" means the handler should validate JWT but does not.

| # | Method | Path | extract_driver_id | Gap |
|---|--------|------|-------------------|-----|
| 1 | GET | `/api/v1/customer/profile` | YES | OK |
| 2 | PUT | `/api/v1/customer/profile` | YES | OK |
| 3 | GET | `/api/v1/customer/sessions` | YES | OK |
| 4 | GET | `/api/v1/customer/sessions/{id}` | YES | OK |
| 5 | GET | `/api/v1/customer/laps` | YES | OK |
| 6 | GET | `/api/v1/customer/stats` | YES | OK |
| 7 | GET | `/api/v1/customer/wallet` | YES | OK |
| 8 | GET | `/api/v1/customer/wallet/transactions` | YES | OK |
| 9 | GET | `/api/v1/customer/experiences` | YES | OK |
| 10 | GET | `/api/v1/customer/ac/catalog` | YES | OK |
| 11 | POST | `/api/v1/customer/book` | YES | OK |
| 12 | GET | `/api/v1/customer/active-reservation` | YES | OK |
| 13 | POST | `/api/v1/customer/end-reservation` | YES | OK |
| 14 | POST | `/api/v1/customer/continue-session` | YES | OK |
| 15 | GET | `/api/v1/customer/friends` | YES | OK |
| 16 | GET | `/api/v1/customer/friends/requests` | YES | OK |
| 17 | POST | `/api/v1/customer/friends/request` | YES | OK |
| 18 | POST | `/api/v1/customer/friends/request/{id}/accept` | YES | OK |
| 19 | POST | `/api/v1/customer/friends/request/{id}/reject` | YES | OK |
| 20 | DELETE | `/api/v1/customer/friends/{id}` | YES | OK |
| 21 | PUT | `/api/v1/customer/presence` | YES | OK |
| 22 | POST | `/api/v1/customer/book-multiplayer` | YES | OK |
| 23 | GET | `/api/v1/customer/group-session` | YES | OK |
| 24 | POST | `/api/v1/customer/group-session/{id}/accept` | YES | OK |
| 25 | POST | `/api/v1/customer/group-session/{id}/decline` | YES | OK |
| 26 | GET | `/api/v1/customer/multiplayer-results/{group_session_id}` | YES | OK |
| 27 | GET | `/api/v1/customer/telemetry` | YES | OK |
| 28 | GET | `/api/v1/customer/waiver-status` | YES | OK |
| 29 | GET | `/api/v1/customer/tournaments` | YES | OK |
| 30 | POST | `/api/v1/customer/tournaments/{id}/register` | YES | OK |
| 31 | GET | `/api/v1/customer/compare-laps` | YES | OK |
| 32 | GET | `/api/v1/customer/sessions/{id}/share` | YES | OK |
| 33 | GET | `/api/v1/customer/referral-code` | YES | OK |
| 34 | POST | `/api/v1/customer/referral-code/generate` | YES | OK |
| 35 | POST | `/api/v1/customer/redeem-referral` | YES | OK |
| 36 | POST | `/api/v1/customer/apply-coupon` | YES | OK |
| 37 | GET | `/api/v1/customer/packages` | YES | OK |
| 38 | GET | `/api/v1/customer/membership` | YES | OK |
| 39 | POST | `/api/v1/customer/membership/subscribe` | YES | OK |
| 40 | POST | `/api/v1/customer/ai/chat` | YES | OK |

**Customer route count: 40 (28+ confirmed calling extract_driver_id in handler)**

#### Tier: Staff/Admin (requires staff credential — CURRENTLY ZERO AUTH)

Every route in this tier is callable by anyone on the network with zero authentication. This is the largest security gap.

| # | Method | Path | Current Auth | Gap |
|---|--------|------|-------------|-----|
| 1 | GET | `/api/v1/pods` | None | NEEDS AUTH |
| 2 | POST | `/api/v1/pods` | None | NEEDS AUTH |
| 3 | GET | `/api/v1/pod-status-summary` | None | NEEDS AUTH |
| 4 | POST | `/api/v1/pods/seed` | None | NEEDS AUTH |
| 5 | GET | `/api/v1/pods/{id}` | None | NEEDS AUTH |
| 6 | POST | `/api/v1/pods/{id}/wake` | None | NEEDS AUTH |
| 7 | POST | `/api/v1/pods/{id}/shutdown` | None | NEEDS AUTH |
| 8 | POST | `/api/v1/pods/{id}/lockdown` | None | NEEDS AUTH |
| 9 | POST | `/api/v1/pods/{id}/enable` | None | NEEDS AUTH |
| 10 | POST | `/api/v1/pods/{id}/disable` | None | NEEDS AUTH |
| 11 | POST | `/api/v1/pods/{id}/screen` | None | NEEDS AUTH |
| 12 | POST | `/api/v1/pods/{id}/unrestrict` | None | NEEDS AUTH |
| 13 | POST | `/api/v1/pods/{id}/restart` | None | NEEDS AUTH |
| 14 | POST | `/api/v1/pods/wake-all` | None | NEEDS AUTH |
| 15 | POST | `/api/v1/pods/shutdown-all` | None | NEEDS AUTH |
| 16 | POST | `/api/v1/pods/restart-all` | None | NEEDS AUTH |
| 17 | POST | `/api/v1/pods/lockdown-all` | None | NEEDS AUTH |
| 18 | POST | `/api/v1/pods/{id}/exec` | None | NEEDS AUTH |
| 19 | GET | `/api/v1/pods/{id}/self-test` | None | NEEDS AUTH |
| 20 | GET | `/api/v1/drivers` | None | NEEDS AUTH |
| 21 | POST | `/api/v1/drivers` | None | NEEDS AUTH |
| 22 | GET | `/api/v1/drivers/{id}` | None | NEEDS AUTH |
| 23 | GET | `/api/v1/drivers/{id}/full-profile` | None | NEEDS AUTH |
| 24 | GET | `/api/v1/sessions` | None | NEEDS AUTH |
| 25 | POST | `/api/v1/sessions` | None | NEEDS AUTH |
| 26 | GET | `/api/v1/sessions/{id}` | None | NEEDS AUTH |
| 27 | GET | `/api/v1/laps` | None | NEEDS AUTH |
| 28 | GET | `/api/v1/sessions/{id}/laps` | None | NEEDS AUTH |
| 29 | GET | `/api/v1/leaderboard/{track}` | None | NEEDS AUTH |
| 30 | GET | `/api/v1/events` | None | NEEDS AUTH |
| 31 | POST | `/api/v1/events` | None | NEEDS AUTH |
| 32 | GET | `/api/v1/bookings` | None | NEEDS AUTH |
| 33 | POST | `/api/v1/bookings` | None | NEEDS AUTH |
| 34 | GET | `/api/v1/pricing` | None | NEEDS AUTH |
| 35 | POST | `/api/v1/pricing` | None | NEEDS AUTH |
| 36 | PUT | `/api/v1/pricing/{id}` | None | NEEDS AUTH |
| 37 | DELETE | `/api/v1/pricing/{id}` | None | NEEDS AUTH |
| 38 | GET | `/api/v1/billing/rates` | None | NEEDS AUTH |
| 39 | POST | `/api/v1/billing/rates` | None | NEEDS AUTH |
| 40 | PUT | `/api/v1/billing/rates/{id}` | None | NEEDS AUTH |
| 41 | DELETE | `/api/v1/billing/rates/{id}` | None | NEEDS AUTH |
| 42 | POST | `/api/v1/billing/start` | None | NEEDS AUTH |
| 43 | GET | `/api/v1/billing/active` | None | NEEDS AUTH |
| 44 | GET | `/api/v1/billing/sessions` | None | NEEDS AUTH |
| 45 | GET | `/api/v1/billing/sessions/{id}` | None | NEEDS AUTH |
| 46 | GET | `/api/v1/billing/sessions/{id}/events` | None | NEEDS AUTH |
| 47 | GET | `/api/v1/billing/sessions/{id}/summary` | None | NEEDS AUTH |
| 48 | POST | `/api/v1/billing/{id}/stop` | None | NEEDS AUTH |
| 49 | POST | `/api/v1/billing/{id}/pause` | None | NEEDS AUTH |
| 50 | POST | `/api/v1/billing/{id}/resume` | None | NEEDS AUTH |
| 51 | POST | `/api/v1/billing/{id}/extend` | None | NEEDS AUTH |
| 52 | POST | `/api/v1/billing/{id}/refund` | None | NEEDS AUTH |
| 53 | GET | `/api/v1/billing/{id}/refunds` | None | NEEDS AUTH |
| 54 | GET | `/api/v1/billing/report/daily` | None | NEEDS AUTH |
| 55 | GET | `/api/v1/billing/split-options/{duration_minutes}` | None | NEEDS AUTH |
| 56 | POST | `/api/v1/billing/continue-split` | None | NEEDS AUTH |
| 57 | POST | `/api/v1/games/launch` | None | NEEDS AUTH |
| 58 | POST | `/api/v1/games/relaunch/{pod_id}` | None | NEEDS AUTH |
| 59 | POST | `/api/v1/games/stop` | None | NEEDS AUTH |
| 60 | GET | `/api/v1/games/active` | None | NEEDS AUTH |
| 61 | GET | `/api/v1/games/history` | None | NEEDS AUTH |
| 62 | GET | `/api/v1/games/pod/{pod_id}` | None | NEEDS AUTH |
| 63 | POST | `/api/v1/pods/{pod_id}/transmission` | None | NEEDS AUTH |
| 64 | POST | `/api/v1/pods/{pod_id}/ffb` | None | NEEDS AUTH |
| 65 | POST | `/api/v1/pods/{pod_id}/assists` | None | NEEDS AUTH |
| 66 | GET | `/api/v1/pods/{pod_id}/assist-state` | None | NEEDS AUTH |
| 67 | GET | `/api/v1/ac/presets` | None | NEEDS AUTH |
| 68 | POST | `/api/v1/ac/presets` | None | NEEDS AUTH |
| 69 | GET | `/api/v1/ac/presets/{id}` | None | NEEDS AUTH |
| 70 | PUT | `/api/v1/ac/presets/{id}` | None | NEEDS AUTH |
| 71 | DELETE | `/api/v1/ac/presets/{id}` | None | NEEDS AUTH |
| 72 | POST | `/api/v1/ac/session/start` | None | NEEDS AUTH |
| 73 | POST | `/api/v1/ac/session/stop` | None | NEEDS AUTH |
| 74 | GET | `/api/v1/ac/session/active` | None | NEEDS AUTH |
| 75 | GET | `/api/v1/ac/sessions` | None | NEEDS AUTH |
| 76 | GET | `/api/v1/ac/sessions/{id}/leaderboard` | None | NEEDS AUTH |
| 77 | POST | `/api/v1/ac/session/{session_id}/continuous` | None | NEEDS AUTH |
| 78 | POST | `/api/v1/ac/session/retry-pod` | None | NEEDS AUTH |
| 79 | POST | `/api/v1/ac/session/update-config` | None | NEEDS AUTH |
| 80 | GET | `/api/v1/ac/content/tracks` | None | NEEDS AUTH |
| 81 | GET | `/api/v1/ac/content/cars` | None | NEEDS AUTH |
| 82 | POST | `/api/v1/auth/assign` | None | NEEDS AUTH |
| 83 | POST | `/api/v1/auth/cancel/{id}` | None | NEEDS AUTH |
| 84 | GET | `/api/v1/auth/pending` | None | NEEDS AUTH |
| 85 | GET | `/api/v1/auth/pending/{pod_id}` | None | NEEDS AUTH |
| 86 | POST | `/api/v1/auth/start-now` | None | NEEDS AUTH |
| 87 | POST | `/api/v1/auth/validate-pin` | None | NEEDS AUTH |
| 88 | POST | `/api/v1/auth/kiosk/validate-pin` | None | NEEDS AUTH |
| 89 | POST | `/api/v1/auth/validate-qr` | None | NEEDS AUTH |
| 90 | GET | `/api/v1/wallet/{driver_id}` | None | NEEDS AUTH |
| 91 | POST | `/api/v1/wallet/{driver_id}/topup` | None | NEEDS AUTH |
| 92 | GET | `/api/v1/wallet/{driver_id}/transactions` | None | NEEDS AUTH |
| 93 | POST | `/api/v1/wallet/{driver_id}/debit` | None | NEEDS AUTH |
| 94 | POST | `/api/v1/wallet/{driver_id}/refund` | None | NEEDS AUTH |
| 95 | GET | `/api/v1/wallet/transactions` | None | NEEDS AUTH |
| 96 | GET | `/api/v1/waivers` | None | NEEDS AUTH |
| 97 | GET | `/api/v1/waivers/check` | None | NEEDS AUTH |
| 98 | GET | `/api/v1/waivers/{driver_id}/signature` | None | NEEDS AUTH |
| 99 | GET | `/api/v1/kiosk/experiences` | None | NEEDS AUTH |
| 100 | POST | `/api/v1/kiosk/experiences` | None | NEEDS AUTH |
| 101 | GET | `/api/v1/kiosk/experiences/{id}` | None | NEEDS AUTH |
| 102 | PUT | `/api/v1/kiosk/experiences/{id}` | None | NEEDS AUTH |
| 103 | DELETE | `/api/v1/kiosk/experiences/{id}` | None | NEEDS AUTH |
| 104 | GET | `/api/v1/kiosk/settings` | None | NEEDS AUTH |
| 105 | PUT | `/api/v1/kiosk/settings` | None | NEEDS AUTH |
| 106 | POST | `/api/v1/kiosk/pod-launch-experience` | None | NEEDS AUTH |
| 107 | POST | `/api/v1/kiosk/book-multiplayer` | None | NEEDS AUTH |
| 108 | GET | `/api/v1/config/kiosk-allowlist` | None | NEEDS AUTH |
| 109 | POST | `/api/v1/config/kiosk-allowlist` | None | NEEDS AUTH |
| 110 | DELETE | `/api/v1/config/kiosk-allowlist/{name}` | None | NEEDS AUTH |
| 111 | GET | `/api/v1/pos/lockdown` | None | NEEDS AUTH |
| 112 | POST | `/api/v1/pos/lockdown` | None | NEEDS AUTH |
| 113 | POST | `/api/v1/ai/chat` | None | NEEDS AUTH |
| 114 | GET | `/api/v1/ops/stats` | None | NEEDS AUTH |
| 115 | POST | `/api/v1/ai/diagnose` | None | NEEDS AUTH |
| 116 | GET | `/api/v1/ai/suggestions` | None | NEEDS AUTH |
| 117 | POST | `/api/v1/ai/suggestions/{id}/dismiss` | None | NEEDS AUTH |
| 118 | GET | `/api/v1/ai/training/stats` | None | NEEDS AUTH |
| 119 | GET | `/api/v1/ai/training/pairs` | None | NEEDS AUTH |
| 120 | POST | `/api/v1/ai/training/import` | None | NEEDS AUTH |
| 121 | GET | `/api/v1/activity` | None | NEEDS AUTH |
| 122 | GET | `/api/v1/pods/{pod_id}/activity` | None | NEEDS AUTH |
| 123 | POST | `/api/v1/pods/{pod_id}/watchdog-crash` | None | NEEDS AUTH |
| 124 | POST | `/api/v1/staff/events` | None | NEEDS AUTH |
| 125 | GET | `/api/v1/staff/events` | None | NEEDS AUTH |
| 126 | GET | `/api/v1/staff/events/{id}` | None | NEEDS AUTH |
| 127 | PUT | `/api/v1/staff/events/{id}` | None | NEEDS AUTH |
| 128 | POST | `/api/v1/staff/championships` | None | NEEDS AUTH |
| 129 | GET | `/api/v1/staff/championships` | None | NEEDS AUTH |
| 130 | GET | `/api/v1/staff/championships/{id}` | None | NEEDS AUTH |
| 131 | POST | `/api/v1/staff/championships/{id}/rounds` | None | NEEDS AUTH |
| 132 | POST | `/api/v1/staff/events/{id}/link-session` | None | NEEDS AUTH |
| 133 | POST | `/api/v1/staff/group-sessions/{id}/complete` | None | NEEDS AUTH |
| 134 | GET | `/api/v1/deploy/status` | None | NEEDS AUTH |
| 135 | POST | `/api/v1/deploy/rolling` | None | NEEDS AUTH |
| 136 | POST | `/api/v1/deploy/{pod_id}` | None | NEEDS AUTH |
| 137 | POST | `/api/v1/staff/validate-pin` | None | NEEDS AUTH |
| 138 | GET | `/api/v1/staff` | None | NEEDS AUTH |
| 139 | POST | `/api/v1/staff` | None | NEEDS AUTH |
| 140 | GET | `/api/v1/employee/daily-pin` | None | NEEDS AUTH |
| 141 | POST | `/api/v1/employee/debug-unlock` | None | NEEDS AUTH |
| 142 | GET | `/api/v1/pricing/rules` | None | NEEDS AUTH |
| 143 | POST | `/api/v1/pricing/rules` | None | NEEDS AUTH |
| 144 | PUT | `/api/v1/pricing/rules/{id}` | None | NEEDS AUTH |
| 145 | DELETE | `/api/v1/pricing/rules/{id}` | None | NEEDS AUTH |
| 146 | GET | `/api/v1/coupons` | None | NEEDS AUTH |
| 147 | POST | `/api/v1/coupons` | None | NEEDS AUTH |
| 148 | PUT | `/api/v1/coupons/{id}` | None | NEEDS AUTH |
| 149 | DELETE | `/api/v1/coupons/{id}` | None | NEEDS AUTH |
| 150 | GET | `/api/v1/review-nudges/pending` | None | NEEDS AUTH |
| 151 | POST | `/api/v1/review-nudges/{id}/sent` | None | NEEDS AUTH |
| 152 | GET | `/api/v1/time-trials` | None | NEEDS AUTH |
| 153 | POST | `/api/v1/time-trials` | None | NEEDS AUTH |
| 154 | PUT | `/api/v1/time-trials/{id}` | None | NEEDS AUTH |
| 155 | DELETE | `/api/v1/time-trials/{id}` | None | NEEDS AUTH |
| 156 | GET | `/api/v1/tournaments` | None | NEEDS AUTH |
| 157 | POST | `/api/v1/tournaments` | None | NEEDS AUTH |
| 158 | GET | `/api/v1/tournaments/{id}` | None | NEEDS AUTH |
| 159 | PUT | `/api/v1/tournaments/{id}` | None | NEEDS AUTH |
| 160 | GET | `/api/v1/tournaments/{id}/registrations` | None | NEEDS AUTH |
| 161 | GET | `/api/v1/tournaments/{id}/matches` | None | NEEDS AUTH |
| 162 | POST | `/api/v1/tournaments/{id}/generate-bracket` | None | NEEDS AUTH |
| 163 | POST | `/api/v1/tournaments/{id}/matches/{match_id}/result` | None | NEEDS AUTH |
| 164 | PUT | `/api/v1/scheduler/settings` | None | NEEDS AUTH |
| 165 | GET | `/api/v1/scheduler/status` | None | NEEDS AUTH |
| 166 | GET | `/api/v1/scheduler/analytics` | None | NEEDS AUTH |
| 167 | GET | `/api/v1/accounting/accounts` | None | NEEDS AUTH |
| 168 | GET | `/api/v1/accounting/trial-balance` | None | NEEDS AUTH |
| 169 | GET | `/api/v1/accounting/profit-loss` | None | NEEDS AUTH |
| 170 | GET | `/api/v1/accounting/balance-sheet` | None | NEEDS AUTH |
| 171 | GET | `/api/v1/accounting/journal` | None | NEEDS AUTH |
| 172 | GET | `/api/v1/audit-log` | None | NEEDS AUTH |

**Staff/Admin route count: 172. Current auth enforcement: ZERO.**

#### Tier: Service (machine-to-machine)

| # | Method | Path | Current Auth | Gap |
|---|--------|------|-------------|-----|
| 1 | GET | `/api/v1/sync/changes` | terminal_secret in handler | PARTIAL |
| 2 | POST | `/api/v1/sync/push` | terminal_secret in handler | PARTIAL |
| 3 | GET | `/api/v1/sync/health` | terminal_secret in handler | PARTIAL |
| 4 | POST | `/api/v1/actions` | None | NEEDS AUTH |
| 5 | GET | `/api/v1/actions/pending` | None | NEEDS AUTH |
| 6 | POST | `/api/v1/actions/process` | None | NEEDS AUTH |
| 7 | POST | `/api/v1/actions/{id}/ack` | None | NEEDS AUTH |
| 8 | GET | `/api/v1/actions/history` | None | NEEDS AUTH |
| 9 | POST | `/api/v1/terminal/auth` | None | NEEDS AUTH |
| 10 | GET | `/api/v1/terminal/commands` | None | NEEDS AUTH |
| 11 | POST | `/api/v1/terminal/commands` | None | NEEDS AUTH |
| 12 | GET | `/api/v1/terminal/commands/pending` | None | NEEDS AUTH |
| 13 | POST | `/api/v1/terminal/commands/{id}/result` | None | NEEDS AUTH |
| 14 | POST | `/api/v1/terminal/book-multiplayer` | None | NEEDS AUTH |
| 15 | GET | `/api/v1/terminal/group-sessions` | None | NEEDS AUTH |
| 16 | GET | `/api/v1/bot/lookup` | terminal_secret in handler | PARTIAL |
| 17 | GET | `/api/v1/bot/pricing` | terminal_secret in handler | PARTIAL |
| 18 | POST | `/api/v1/bot/book` | terminal_secret in handler | PARTIAL |
| 19 | GET | `/api/v1/bot/pods-status` | terminal_secret in handler | PARTIAL |
| 20 | GET | `/api/v1/bot/events` | terminal_secret in handler | PARTIAL |
| 21 | GET | `/api/v1/bot/leaderboard` | terminal_secret in handler | PARTIAL |
| 22 | GET | `/api/v1/bot/customer-stats` | terminal_secret in handler | PARTIAL |
| 23 | POST | `/api/v1/bot/register-lead` | terminal_secret in handler | PARTIAL |
| 24 | GET | `/api/v1/logs` | None | NEEDS AUTH |
| 25 | GET | `/ws/agent` | None | NEEDS AUTH |
| 26 | GET | `/ws/dashboard` | None | NEEDS AUTH |
| 27 | GET | `/ws/ai` | None | NEEDS AUTH |

**Service route count: 27. Only sync/* and bot/* have partial terminal_secret checks.**

#### Tier: Debug (should be admin-only or disabled)

| # | Method | Path | Current Auth | Gap |
|---|--------|------|-------------|-----|
| 1 | GET | `/api/v1/debug/activity` | None | CRITICAL |
| 2 | GET | `/api/v1/debug/playbooks` | None | CRITICAL |
| 3 | GET | `/api/v1/debug/incidents` | None | CRITICAL |
| 4 | POST | `/api/v1/debug/incidents` | None | CRITICAL |
| 5 | PUT | `/api/v1/debug/incidents/{id}` | None | CRITICAL |
| 6 | POST | `/api/v1/debug/diagnose` | None | CRITICAL |

**Debug route count: 6. Current auth enforcement: ZERO.**

#### racecontrol :8080 Summary

| Tier | Route Count | With Auth | Without Auth | Status |
|------|-------------|-----------|-------------|--------|
| Public | 24 | N/A (none needed) | 24 | OK |
| Customer | 40 | ~28 (extract_driver_id) | ~12 | PARTIAL |
| Staff/Admin | 172 | 0 | 172 | CRITICAL |
| Service | 27 | 11 (terminal_secret) | 16 | PARTIAL |
| Debug | 6 | 0 | 6 | CRITICAL |
| **Total** | **269** | **~39** | **~230** | |

**39 of 269 routes have appropriate auth enforcement (14.5%).**

### 1b. rc-agent :8090

All 11 routes are unauthenticated. The HTTP server binds to `0.0.0.0:8090` (LAN-accessible from any device on 192.168.31.x subnet).

Source: `crates/rc-agent/src/remote_ops.rs`

| # | Method | Path | Auth | Gap |
|---|--------|------|------|-----|
| 1 | GET | `/ping` | None | OK (health) |
| 2 | GET | `/health` | None | OK (health) |
| 3 | GET | `/info` | None | NEEDS AUTH |
| 4 | GET | `/files` | None | NEEDS AUTH |
| 5 | GET | `/file` | None | NEEDS AUTH |
| 6 | POST | `/exec` | None | **CRITICAL** |
| 7 | POST | `/mkdir` | None | NEEDS AUTH |
| 8 | POST | `/write` | None | **CRITICAL** |
| 9 | GET | `/screenshot` | None | NEEDS AUTH |
| 10 | GET | `/cursor` | None | NEEDS AUTH |
| 11 | POST | `/input` | None | NEEDS AUTH |

**CRITICAL:** `/exec` allows arbitrary command execution on pod machines. Any device on the LAN can run `curl -X POST http://192.168.31.89:8090/exec -d '{"command":"cmd /c whoami"}'` and get shell access. `/write` allows arbitrary file writes to the pod filesystem.

**rc-agent route count: 11. Auth enforcement: ZERO.**

### 1c. rc-sentry :8091

Single TCP handler. No HTTP — raw TCP protocol. Binds to `0.0.0.0:8091`.

Source: `crates/rc-sentry/src/main.rs`

| # | Protocol | Port | Auth | Gap |
|---|----------|------|------|-----|
| 1 | Raw TCP | 8091 | None | **CRITICAL** |

The handler accepts HTTP-formatted POST requests with a JSON body containing a `command` field, executes the command via `cmd.exe /C`, and returns stdout/stderr. Zero authentication. The source code explicitly notes: "binds to 0.0.0.0 on a private subnet with no auth."

**CRITICAL:** Any device on the LAN can execute arbitrary shell commands on any machine running rc-sentry.

**rc-sentry endpoint count: 1. Auth enforcement: ZERO.**

---

## PII Location Audit

### 2a. SQLite `drivers` table

**Source:** `crates/racecontrol/src/db/mod.rs`

| Column | PII Type | Added By | Sensitivity |
|--------|----------|----------|-------------|
| `name` | Full name | CREATE TABLE (base schema) | MEDIUM |
| `email` | Email address | CREATE TABLE (base schema) | MEDIUM |
| `phone` | Phone number | CREATE TABLE (base schema) | HIGH |
| `dob` | Date of birth | ALTER TABLE migration | HIGH |
| `guardian_name` | Guardian full name | ALTER TABLE migration | MEDIUM |
| `guardian_phone` | Guardian phone number | ALTER TABLE migration | HIGH |
| `signature_data` | Waiver signature (binary/base64) | ALTER TABLE migration | MEDIUM |
| `nickname` | User-chosen display name | ALTER TABLE migration | LOW |
| `otp_code` | One-time password (temporary) | ALTER TABLE migration | CRITICAL |
| `pin_hash` | Hashed PIN | ALTER TABLE migration | HIGH |

**Storage:** SQLite file on server disk. No encryption at rest. Accessible to anyone with filesystem access to the server.

### 2b. SQLite `staff_members` table

**Source:** `crates/racecontrol/src/db/mod.rs`

| Column | PII Type | Sensitivity |
|--------|----------|-------------|
| `name` | Full name | MEDIUM |
| `phone` | Phone number | HIGH |

### 2c. Application logs (phone numbers and OTP codes)

**Source:** `crates/racecontrol/src/auth/mod.rs`

| Line | Level | What is logged | Severity |
|------|-------|----------------|----------|
| ~1060 | INFO | `"OTP sent via WhatsApp to {wa_phone}"` — phone number | HIGH |
| ~1063 | WARN | `"Evolution API returned {status}: OTP for {phone} is {otp_str}"` — phone AND OTP code | **CRITICAL** |
| ~1066 | WARN | `"Failed to send OTP via WhatsApp: {e}. OTP for {phone} is {otp_str}"` — phone AND OTP code | **CRITICAL** |
| ~1070 | INFO | `"OTP for phone {phone}: {otp_str} (Evolution API not configured)"` — phone AND OTP code | **CRITICAL** |

**Source:** `crates/racecontrol/src/billing.rs`

| Line | Level | What is logged | Severity |
|------|-------|----------------|----------|
| ~2263 | INFO | `"WhatsApp receipt sent to {wa_phone} for session {session_id}"` — phone number | HIGH |
| ~2266 | WARN | `"Evolution API returned {status} for receipt to {wa_phone}"` — phone number | HIGH |

**Impact:** OTP codes in logs allow account takeover. Anyone with log access can see the current OTP for any phone number. Logs are written to stdout and potentially to file depending on tracing subscriber configuration.

### 2d. WhatsApp message payloads

Phone numbers are embedded in Evolution API request bodies for:
- OTP delivery (`auth/mod.rs`) — customer phone number sent to Evolution API endpoint
- Billing receipt delivery (`billing.rs`) — customer phone number sent to Evolution API endpoint

These payloads transit the local network to the Evolution API server. Phone numbers are included as required API parameters (recipient addressing).

### 2e. Cloud sync payloads

**Source:** `crates/racecontrol/src/api/routes.rs` (handler at `/api/v1/sync/changes`)

The `/sync/changes` endpoint sends full driver records as JSON to Bono's VPS (app.racingpoint.cloud). Synced fields include:
- `name` (full name)
- `email` (email address)
- `phone` (phone number)
- `pin_hash` (hashed PIN)

This data transits over HTTPS (cloud API on port 443 with Let's Encrypt TLS), but the full PII payload is exposed to the cloud server. The codebase contains explicit comments acknowledging this PII exposure.

### 2f. PII Summary Table

| Location | PII Types | Storage | Encryption | Severity |
|----------|-----------|---------|------------|----------|
| SQLite `drivers` table | name, email, phone, dob, guardian_name, guardian_phone, signature_data, otp_code, pin_hash | Disk (server) | None at rest | MEDIUM |
| SQLite `staff_members` table | name, phone | Disk (server) | None at rest | MEDIUM |
| Application logs (auth) | phone + OTP code | stdout / log files | None | **CRITICAL** |
| Application logs (billing) | phone | stdout / log files | None | HIGH |
| WhatsApp API payloads | phone | Network transit | HTTP (local) | MEDIUM |
| Cloud sync `/sync/changes` | name, email, phone, pin_hash | Network transit + VPS storage | HTTPS (TLS) | HIGH |

---

## CORS, HTTPS, and Auth State

### 3a. CORS Configuration

**Source:** `crates/racecontrol/src/main.rs`, lines 556-567

```rust
CorsLayer::new()
    .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
        let origin = origin.to_str().unwrap_or("");
        origin.starts_with("http://localhost:")
            || origin.starts_with("http://127.0.0.1:")
            || origin.starts_with("http://192.168.31.")
            || origin.starts_with("http://kiosk.rp")
            || origin.contains("racingpoint.cloud")
    }))
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::PATCH, Method::DELETE, Method::OPTIONS])
    .allow_headers(tower_http::cors::Any)
    .allow_credentials(false)
```

**Configuration details:**

| Setting | Value | Assessment |
|---------|-------|------------|
| `allow_origin` | Predicate-based: `http://localhost:*`, `http://127.0.0.1:*`, `http://192.168.31.*`, `http://kiosk.rp*`, `*racingpoint.cloud*` | Entire LAN subnet allowed |
| `allow_methods` | GET, POST, PUT, PATCH, DELETE, OPTIONS | Standard set, acceptable |
| `allow_headers` | `Any` — accepts any request header | Should be restricted to Content-Type, Authorization, X-Terminal-Secret |
| `allow_credentials` | `false` | Cookies not sent cross-origin (blocks future admin cookie auth) |

**Issues:**
1. `allow_headers(Any)` accepts arbitrary headers — should be restricted to known headers
2. `http://192.168.31.*` allows any device on the LAN subnet to make cross-origin requests (intentional for venue WiFi, but any customer device or rogue device on WiFi can call staff APIs)
3. No HTTPS origins in predicate except via `racingpoint.cloud` (which uses `.contains()` substring match — could be spoofed with domain like `evil-racingpoint.cloud.attacker.com`)
4. `allow_credentials(false)` prevents cookie-based auth for future admin sessions

### 3b. HTTPS State Per Service

| Service | Port | Protocol | TLS | Certificate | Notes |
|---------|------|----------|-----|-------------|-------|
| racecontrol | 8080 | HTTP | NO | None | All API traffic unencrypted on LAN |
| rc-agent | 8090 | HTTP | NO | None | Pod management traffic unencrypted |
| rc-sentry | 8091 | Raw TCP | NO | None | Shell commands in plaintext |
| Kiosk PWA | 3300 | HTTP | NO | None | Customer-facing UI unencrypted |
| Web Dashboard | 3200 | HTTP | NO | None | Staff dashboard unencrypted |
| Cloud API | 443 | HTTPS | YES | Let's Encrypt | app.racingpoint.cloud on Bono's VPS |

**Impact:** All on-premise traffic is unencrypted. Customer JWTs, staff PINs, OTP codes, and all PII transit in plaintext over WiFi. Any device on the venue network can sniff this traffic with a packet capture tool.

### 3c. Auth Infrastructure State

#### jwt_error_to_401 middleware

**Source:** `crates/racecontrol/src/main.rs`, line 554

Applied globally via `.layer(axum_mw::from_fn(jwt_error_to_401))`. This middleware:
- Intercepts responses that contain JWT-related error messages
- Converts them to HTTP 401 Unauthorized responses
- Does **NOT** require authentication — it is an error formatter, not an access control layer
- If no JWT is provided, the request passes through without any 401

**Misconception risk:** The presence of this middleware may give the false impression that routes are protected. They are not.

#### extract_driver_id() pattern

**Source:** `crates/racecontrol/src/api/routes.rs`, line ~4127

```rust
fn extract_driver_id(state: &AppState, headers: &HeaderMap) -> Result<String, String>
```

- Called inside ~28 handler functions (all in the Customer tier)
- Reads `Authorization: Bearer {token}` header
- Calls `auth::verify_jwt(token, &state.config.auth.jwt_secret)`
- Returns the `driver_id` (sub claim) on success, error string on failure
- **Not middleware** — each handler must explicitly call this function
- Missing from all Staff/Admin routes — these routes have no identity check whatsoever

#### Claims struct

**Source:** `crates/racecontrol/src/auth/mod.rs`, lines 38-43

```rust
pub struct Claims {
    pub sub: String,  // driver_id
    pub exp: usize,
    pub iat: usize,
}
```

- Contains only `sub` (driver_id), `exp` (expiry), `iat` (issued at)
- **No `role` field** — cannot distinguish customer from staff from admin
- All JWTs are customer tokens; there is no concept of a staff JWT
- Implication: even if auth were enforced on staff routes, there is no way to verify the caller is staff vs customer

#### terminal_secret

- A shared secret string configured in `racecontrol.toml` under `[cloud]`
- Checked via `X-Terminal-Secret` header comparison in some sync and bot handlers
- Not middleware — checked manually in each handler
- Present in: `/sync/*` handlers, `/bot/*` handlers
- Missing from: `/actions/*`, `/terminal/*`, `/logs`

#### Staff PIN validation

- `POST /api/v1/staff/validate-pin` exists as an endpoint
- Validates a 4-digit daily PIN against the staff_members table
- **Not enforced as middleware** on any route
- Staff PIN is purely used by the admin dashboard's login screen (client-side gate only)
- A direct API call to any staff route bypasses this entirely

---

## Risk Summary

Prioritized security gaps for Phase 76+ remediation:

| # | Severity | Risk | Routes Affected | Remediation Phase |
|---|----------|------|----------------|-------------------|
| 1 | **CRITICAL** | 172 staff/admin routes with zero auth — any LAN device can start/stop billing, modify pricing, access driver PII, control pods | 172 routes | Phase 76 |
| 2 | **CRITICAL** | rc-agent `/exec` and `/write` allow arbitrary command execution and file write on every pod — zero auth, LAN accessible | 2 routes (all pods) | Phase 77 |
| 3 | **CRITICAL** | rc-sentry TCP handler allows arbitrary shell commands on any machine running it — zero auth, binds 0.0.0.0 | 1 endpoint (all machines) | Phase 77 |
| 4 | **CRITICAL** | OTP codes logged in plaintext at INFO/WARN level — anyone with log access can hijack customer accounts | 3 log statements | Phase 78 |
| 5 | **HIGH** | JWT secret defaults to known string `"racingpoint-jwt-change-me-in-production"` — source code readers can forge any customer token | All JWT-protected routes | Phase 75 (Plan 02) |
| 6 | **HIGH** | Cloud sync `/sync/changes` transmits full PII (name, email, phone, pin_hash) to VPS | 1 route | Phase 79 |
| 7 | **HIGH** | No HTTPS for any on-premise service — JWTs, PINs, OTPs, and PII transit in plaintext over WiFi | All services | Phase 80 |
| 8 | **MEDIUM** | CORS allows any header (`allow_headers(Any)`) and entire LAN subnet (`192.168.31.*`) | All routes | Phase 76 |
| 9 | **MEDIUM** | No role field in JWT Claims — cannot differentiate customer/staff/admin tokens even after adding auth | All routes | Phase 76 |
| 10 | **MEDIUM** | Debug routes (`/debug/*`) expose incident data and diagnostic capabilities with zero auth | 6 routes | Phase 76 |
| 11 | **LOW** | `racingpoint.cloud` origin check uses `.contains()` — could match spoofed domains | CORS | Phase 76 |
| 12 | **LOW** | WebSocket endpoints (`/ws/agent`, `/ws/dashboard`, `/ws/ai`) have zero auth — dashboard data accessible to any LAN device | 3 routes | Phase 76 |
