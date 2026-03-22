# Racing Point API Boundaries

**Last updated:** 2026-03-23
**Source of truth:** Rust structs in `crates/rc-common/src/types.rs` and `crates/racecontrol/src/`
**Base path prefix:** All racecontrol endpoints are under `/api/v1/` (e.g. `GET /api/v1/health`)

---

## Overview

| Boundary | Direction | Consumer | Port | Auth |
|----------|-----------|----------|------|------|
| racecontrol | Server → Kiosk PWA | kiosk Next.js | :8080 | Staff JWT / Customer JWT / None |
| racecontrol | Server → Admin Dashboard | racingpoint-admin Next.js | :8080 | Staff JWT (admin-login) |
| racecontrol | Server → comms-link/bot | comms-link relay, WhatsApp bot | :8080 | terminal_secret header |
| rc-agent | Pod → racecontrol fleet exec | racecontrol fleet/exec, deploy | :8090 | None (LAN-only) |

Admin calls go through a Next.js proxy: `/api/rc/[...path]` → `http://192.168.31.23:8080/api/v1/*`

---

## 1. racecontrol ↔ kiosk / admin (Auth Endpoints)

Rate-limited: 5 req/min per IP via tower_governor.

| Method | Path | Auth | Request Body | Response | Notes |
|--------|------|------|--------------|----------|-------|
| POST | /customer/login | None | `{ phone: string }` | `{ message: string }` | Sends OTP via WhatsApp |
| POST | /customer/verify-otp | None | `{ phone: string, otp: string }` | `{ token: string, driver: DriverProfile }` | Returns customer JWT |
| POST | /auth/validate-pin | None | `{ pin: string, pod_id: string }` | `{ token: string, driver_name: string, tier: PricingTier }` | Kiosk PIN auth |
| POST | /auth/kiosk/validate-pin | None | `{ pin: string }` | `{ token: string }` | Staff JWT for kiosk PWA |
| POST | /kiosk/redeem-pin | None | `{ pin: string }` | `{ token: string, driver_id: string }` | Customer PIN redeem on kiosk |
| POST | /staff/validate-pin | None | `{ pin: string }` | `{ token: string, role: string }` | Staff PIN → staff JWT |
| POST | /auth/admin-login | None | `{ username: string, password: string }` | `{ token: string }` | Admin password → staff JWT |

---

## 2. racecontrol — Public Endpoints (No Auth)

| Method | Path | Auth | Request | Response | Consumer |
|--------|------|------|---------|----------|----------|
| GET | /health | None | - | `{ status: string, version: string, build_id: string }` | All |
| GET | /fleet/health | None | - | `PodFleetStatus[]` | Admin dashboard, kiosk |
| POST | /sentry/crash | None | `{ pod_id: string, reason: string, details?: string }` | `{ ok: boolean }` | rc-sentry on pods |
| GET | /guard/whitelist/{machine_id} | None | - | `{ entries: ProcessEntry[] }` | rc-agent process guard |
| GET | /venue | None | - | `VenueInfo` | Kiosk, PWA |
| POST | /customer/register | None | `{ name: string, phone: string, email?: string }` | `{ driver_id: string, token: string }` | Kiosk registration |
| GET | /wallet/bonus-tiers | None | - | `BonusTier[]` | Kiosk |
| GET | /public/leaderboard | None | - | `LeaderboardEntry[]` | PWA, kiosk |
| GET | /public/leaderboard/{track} | None | - | `LeaderboardEntry[]` | PWA |
| GET | /public/circuit-records | None | - | `CircuitRecord[]` | PWA |
| GET | /public/vehicle-records/{car} | None | - | `VehicleRecord[]` | PWA |
| GET | /public/drivers | None | `?q=string` (query) | `Driver[]` | PWA search |
| GET | /public/drivers/{id} | None | - | `Driver` | PWA |
| GET | /public/time-trial | None | - | `TimeTrialEntry[]` | PWA |
| GET | /public/laps/{lap_id}/telemetry | None | - | `TelemetryFrame[]` | PWA |
| GET | /public/sessions/{id} | None | - | `SessionSummary` | PWA |
| GET | /public/championships | None | - | `Championship[]` | PWA |
| GET | /public/championships/{id} | None | - | `ChampionshipStandings` | PWA |
| GET | /public/championships/{id}/standings | None | - | `ChampionshipStandings` | PWA |
| GET | /public/events | None | - | `Event[]` | PWA, kiosk |
| GET | /public/events/{id} | None | - | `EventLeaderboard` | PWA |
| GET | /public/events/{id}/sessions | None | - | `SessionInfo[]` | PWA |
| GET | /cafe/menu | None | - | `{ categories: CafeCategory[], items: CafeItem[] }` | PWA, kiosk |
| GET | /cafe/promos/active | None | - | `CafePromo[]` | PWA, kiosk |

---

## 3. racecontrol — Kiosk Endpoints (Staff JWT required, pod-accessible)

Staff JWT from `/auth/kiosk/validate-pin`. These routes bypass the pod-source block.

| Method | Path | Auth | Request Body | Response | Notes |
|--------|------|------|--------------|----------|-------|
| GET | /kiosk/experiences | Staff JWT | - | `KioskExperience[]` | List available experiences |
| GET | /kiosk/settings | Staff JWT | - | `KioskSettings` | Kiosk config (idle timeout, etc.) |
| POST | /kiosk/pod-launch-experience | Staff JWT | `{ pod_id: string, experience_id: string, driver_id: string }` | `{ ok: boolean }` | Launch game from kiosk |
| POST | /kiosk/book-multiplayer | Staff JWT | `{ pods: string[], experience_id: string }` | `{ group_session_id: string }` | Staff-confirmed multiplayer start |

---

## 4. racecontrol — Customer Endpoints (Customer JWT in-handler)

Customer JWT from `/customer/login` + `/customer/verify-otp`. JWT is validated per-call inside each handler via `extract_driver_id()`.

| Method | Path | Auth | Request | Response |
|--------|------|------|---------|----------|
| GET | /customer/waiver-status | Customer JWT | - | `{ signed: boolean, signed_at?: string }` |
| GET | /customer/profile | Customer JWT | - | `CustomerProfile` |
| PUT | /customer/profile | Customer JWT | `CustomerProfileUpdate` | `CustomerProfile` |
| GET | /customer/sessions | Customer JWT | - | `SessionInfo[]` |
| GET | /customer/sessions/{id} | Customer JWT | - | `SessionDetail` |
| GET | /customer/laps | Customer JWT | - | `LapData[]` |
| GET | /customer/stats | Customer JWT | - | `DriverStats` |
| GET | /customer/wallet | Customer JWT | - | `WalletInfo` |
| GET | /customer/wallet/transactions | Customer JWT | - | `WalletTransaction[]` |
| GET | /customer/experiences | Customer JWT | - | `KioskExperience[]` |
| GET | /customer/ac/catalog | Customer JWT | - | `AcCatalogEntry[]` |
| POST | /customer/book | Customer JWT | `{ pricing_tier_id: string, pod_id?: string }` | `{ auth_token: string }` |
| GET | /customer/active-reservation | Customer JWT | - | `ReservationInfo?` |
| POST | /customer/end-reservation | Customer JWT | `{ reservation_id: string }` | `{ ok: boolean }` |
| POST | /customer/continue-session | Customer JWT | `{ billing_session_id: string }` | `{ ok: boolean }` |
| GET | /customer/friends | Customer JWT | - | `FriendInfo[]` |
| GET | /customer/friends/requests | Customer JWT | - | `FriendRequest[]` |
| POST | /customer/friends/request | Customer JWT | `{ target_driver_id: string }` | `{ ok: boolean }` |
| POST | /customer/friends/request/{id}/accept | Customer JWT | - | `{ ok: boolean }` |
| POST | /customer/friends/request/{id}/reject | Customer JWT | - | `{ ok: boolean }` |
| DELETE | /customer/friends/{id} | Customer JWT | - | `{ ok: boolean }` |
| PUT | /customer/presence | Customer JWT | `{ status: string }` | `{ ok: boolean }` |
| POST | /customer/book-multiplayer | Customer JWT | `{ group_session_id: string }` | `{ ok: boolean }` |
| GET | /customer/group-session | Customer JWT | - | `GroupSession?` |
| POST | /customer/group-session/{id}/accept | Customer JWT | - | `{ ok: boolean }` |
| POST | /customer/group-session/{id}/decline | Customer JWT | - | `{ ok: boolean }` |
| GET | /customer/multiplayer-results/{group_session_id} | Customer JWT | - | `MultiplayerResults` |
| GET | /customer/telemetry | Customer JWT | - | `TelemetryFrame[]` |
| GET | /customer/tournaments | Customer JWT | - | `Tournament[]` |
| POST | /customer/tournaments/{id}/register | Customer JWT | - | `{ ok: boolean }` |
| GET | /customer/compare-laps | Customer JWT | `?lap_ids=...` | `LapComparison` |
| GET | /customer/sessions/{id}/share | Customer JWT | - | `SessionShareReport` |
| GET | /customer/referral-code | Customer JWT | - | `{ code: string }` |
| POST | /customer/referral-code/generate | Customer JWT | - | `{ code: string }` |
| POST | /customer/redeem-referral | Customer JWT | `{ code: string }` | `{ ok: boolean, bonus_paise: number }` |
| POST | /customer/apply-coupon | Customer JWT | `{ code: string }` | `{ ok: boolean, discount: number }` |
| GET | /customer/packages | Customer JWT | - | `Package[]` |
| GET | /customer/membership | Customer JWT | - | `MembershipInfo?` |
| POST | /customer/membership/subscribe | Customer JWT | `{ plan_id: string }` | `{ ok: boolean }` |
| POST | /customer/ai/chat | Customer JWT | `{ message: string }` | `{ reply: string }` |
| POST | /customer/game-request | Customer JWT | `{ sim_type: string }` | `{ ok: boolean }` |
| GET | /customer/data-export | Customer JWT | - | JSON blob of all driver data |
| DELETE | /customer/data-delete | Customer JWT | - | `{ ok: boolean }` |
| GET | /customer/passport | Customer JWT | - | `DrivingPassport` |
| GET | /customer/badges | Customer JWT | - | `Badge[]` |
| GET | /customer/active-session/events | Customer JWT | - | `SessionPbEvent[]` |
| GET | /customer/reservation | Customer JWT | - | `Reservation?` |
| DELETE | /customer/reservation | Customer JWT | - | `{ ok: boolean }` |
| POST | /customer/reservation/create | Customer JWT | `{ slot: string, pod_id?: string }` | `Reservation` |
| PUT | /customer/reservation/modify | Customer JWT | `{ reservation_id: string, slot: string }` | `Reservation` |
| POST | /customer/cafe/orders | Customer JWT | `{ items: OrderItem[] }` | `{ order_id: string }` |
| GET | /customer/cafe/orders/history | Customer JWT | - | `CafeOrder[]` |

---

## 5. racecontrol — Staff / Admin Endpoints (Staff JWT + pod-source block)

All require `Authorization: Bearer <staff_jwt>` and reject requests from pod IPs (403).

### Pods

| Method | Path | Request Body | Response |
|--------|------|--------------|----------|
| GET | /pods | - | `PodInfo[]` |
| POST | /pods | `{ number, name, ip_address, sim_type }` | `PodInfo` |
| GET | /pod-status-summary | - | `PodStatusSummary` |
| POST | /pods/seed | - | `{ seeded: number }` |
| GET | /pods/{id} | - | `PodInfo` |
| POST | /pods/{id}/wake | - | `{ ok: boolean }` |
| POST | /pods/{id}/shutdown | - | `{ ok: boolean }` |
| POST | /pods/{id}/lockdown | - | `{ ok: boolean }` |
| POST | /pods/{id}/enable | - | `{ ok: boolean }` |
| POST | /pods/{id}/disable | - | `{ ok: boolean }` |
| POST | /pods/{id}/screen | `{ blank: boolean }` | `{ ok: boolean }` |
| POST | /pods/{id}/unrestrict | - | `{ ok: boolean }` |
| POST | /pods/{id}/freedom | `{ enabled: boolean }` | `{ ok: boolean }` |
| POST | /pods/{id}/restart | - | `{ ok: boolean }` |
| POST | /pods/wake-all | - | `{ ok: boolean }` |
| POST | /pods/shutdown-all | - | `{ ok: boolean }` |
| POST | /pods/restart-all | - | `{ ok: boolean }` |
| POST | /pods/lockdown-all | - | `{ ok: boolean }` |
| POST | /pods/{id}/exec | `{ cmd: string }` | `ExecResponse` |
| GET | /pods/{id}/self-test | - | `SelfTestResult` |
| POST | /pods/{id}/clear-maintenance | - | `{ ok: boolean }` |
| POST | /pods/{pod_id}/transmission | `{ mode: string }` | `{ ok: boolean }` |
| POST | /pods/{pod_id}/ffb | `{ preset: string }` | `{ ok: boolean }` |
| POST | /pods/{pod_id}/assists | `{ ... }` | `{ ok: boolean }` |
| GET | /pods/{pod_id}/assist-state | - | `AssistState` |
| GET | /pods/{pod_id}/activity | - | `ActivityEvent[]` |
| POST | /pods/{pod_id}/watchdog-crash | `{ reason: string }` | `{ ok: boolean }` |

### Drivers

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /drivers | - | `Driver[]` |
| POST | /drivers | `{ name, phone?, email? }` | `Driver` |
| GET | /drivers/{id} | - | `Driver` |
| GET | /drivers/{id}/full-profile | - | `DriverFullProfile` |

### Sessions & Laps

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /sessions | - | `SessionInfo[]` |
| POST | /sessions | `SessionCreateRequest` | `SessionInfo` |
| GET | /sessions/{id} | - | `SessionInfo` |
| GET | /sessions/{id}/laps | - | `LapData[]` |
| GET | /laps | - | `LapData[]` |
| GET | /leaderboard/{track} | - | `LeaderboardEntry[]` |

### Events & Bookings

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /events | - | `Event[]` |
| POST | /events | `EventCreateRequest` | `Event` |
| GET | /bookings | - | `Booking[]` |
| POST | /bookings | `BookingRequest` | `Booking` |

### Pricing

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /pricing | - | `PricingTier[]` |
| POST | /pricing | `PricingTierCreate` | `PricingTier` |
| PUT | /pricing/{id} | `PricingTierUpdate` | `PricingTier` |
| DELETE | /pricing/{id} | - | `{ ok: boolean }` |
| GET | /pricing/rules | - | `PricingRule[]` |
| POST | /pricing/rules | `PricingRuleCreate` | `PricingRule` |
| PUT | /pricing/rules/{id} | `PricingRuleUpdate` | `PricingRule` |
| DELETE | /pricing/rules/{id} | - | `{ ok: boolean }` |

### Billing

| Method | Path | Request | Response |
|--------|------|---------|----------|
| POST | /billing/start | `{ driver_id, pod_id, pricing_tier_id }` | `BillingSessionInfo` |
| GET | /billing/active | - | `BillingSessionInfo[]` |
| GET | /billing/sessions | - | `BillingSessionInfo[]` |
| GET | /billing/sessions/{id} | - | `BillingSessionInfo` |
| GET | /billing/sessions/{id}/events | - | `BillingEvent[]` |
| GET | /billing/sessions/{id}/summary | - | `BillingSessionSummary` |
| POST | /billing/{id}/stop | - | `{ ok: boolean }` |
| POST | /billing/{id}/pause | - | `{ ok: boolean }` |
| POST | /billing/{id}/resume | - | `{ ok: boolean }` |
| POST | /billing/{id}/extend | `{ extra_seconds: number }` | `{ ok: boolean }` |
| POST | /billing/{id}/refund | `{ reason: string }` | `{ ok: boolean }` |
| GET | /billing/{id}/refunds | - | `BillingRefund[]` |
| GET | /billing/report/daily | `?date=YYYY-MM-DD` | `DailyBillingReport` |
| GET | /billing/rates | - | `BillingRate[]` |
| POST | /billing/rates | `BillingRateCreate` | `BillingRate` |
| PUT | /billing/rates/{id} | `BillingRateUpdate` | `BillingRate` |
| DELETE | /billing/rates/{id} | - | `{ ok: boolean }` |
| GET | /billing/split-options/{duration_minutes} | - | `SplitOption[]` |
| POST | /billing/continue-split | `{ billing_session_id: string }` | `{ ok: boolean }` |

### Game Launcher

| Method | Path | Request | Response |
|--------|------|---------|----------|
| POST | /games/launch | `{ pod_id, sim_type, driver_id, session_id? }` | `GameLaunchInfo` |
| POST | /games/relaunch/{pod_id} | - | `GameLaunchInfo` |
| POST | /games/stop | `{ pod_id: string }` | `{ ok: boolean }` |
| GET | /games/active | - | `GameLaunchInfo[]` |
| GET | /games/history | - | `GameLaunchInfo[]` |
| GET | /games/pod/{pod_id} | - | `GameLaunchInfo?` |

### AC LAN Server

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /ac/presets | - | `AcPreset[]` |
| POST | /ac/presets | `AcPresetCreate` | `AcPreset` |
| GET | /ac/presets/{id} | - | `AcPreset` |
| PUT | /ac/presets/{id} | `AcPresetUpdate` | `AcPreset` |
| DELETE | /ac/presets/{id} | - | `{ ok: boolean }` |
| POST | /ac/session/start | `{ preset_id: string, pods: string[] }` | `{ session_id: string }` |
| POST | /ac/session/stop | - | `{ ok: boolean }` |
| GET | /ac/session/active | - | `AcSession?` |
| GET | /ac/sessions | - | `AcSession[]` |
| GET | /ac/sessions/{id}/leaderboard | - | `LeaderboardEntry[]` |
| POST | /ac/session/{session_id}/continuous | `{ enabled: boolean }` | `{ ok: boolean }` |
| POST | /ac/session/retry-pod | `{ pod_id: string }` | `{ ok: boolean }` |
| POST | /ac/session/update-config | `AcConfigUpdate` | `{ ok: boolean }` |
| GET | /ac/content/tracks | - | `string[]` |
| GET | /ac/content/cars | - | `string[]` |

### Auth (staff-facing)

| Method | Path | Request | Response |
|--------|------|---------|----------|
| POST | /auth/assign | `{ pod_id, driver_id, pricing_tier_id }` | `{ token: string }` |
| POST | /auth/cancel/{id} | - | `{ ok: boolean }` |
| GET | /auth/pending | - | `AuthTokenInfo[]` |
| GET | /auth/pending/{pod_id} | - | `AuthTokenInfo?` |
| POST | /auth/start-now | `{ pod_id, driver_id }` | `{ ok: boolean }` |
| POST | /auth/validate-qr | `{ token: string }` | `{ ok: boolean, driver_id: string }` |

### Wallet (staff-facing)

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /wallet/transactions | - | `WalletTransaction[]` |
| GET | /wallet/{driver_id} | - | `WalletInfo` |
| POST | /wallet/{driver_id}/topup | `{ amount_paise: number, note?: string }` | `WalletInfo` |
| GET | /wallet/{driver_id}/transactions | - | `WalletTransaction[]` |
| POST | /wallet/{driver_id}/debit | `{ amount_paise: number, reason: string }` | `{ ok: boolean }` |
| POST | /wallet/{driver_id}/refund | `{ amount_paise: number, reason: string }` | `{ ok: boolean }` |

### Staff Management

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /staff | - | `StaffMember[]` |
| POST | /staff | `StaffCreate` | `StaffMember` |
| GET | /employee/daily-pin | - | `{ pin: string }` |
| POST | /employee/debug-unlock | `{ pin: string }` | `{ ok: boolean }` |

### Coupons & Waivers

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /coupons | - | `Coupon[]` |
| POST | /coupons | `CouponCreate` | `Coupon` |
| PUT | /coupons/{id} | `CouponUpdate` | `Coupon` |
| DELETE | /coupons/{id} | - | `{ ok: boolean }` |
| GET | /waivers | - | `WaiverRecord[]` |
| GET | /waivers/check | `?driver_id=...` | `{ signed: boolean }` |
| GET | /waivers/{driver_id}/signature | - | binary (PNG) |

### Config & POS

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /config/kiosk-allowlist | Staff JWT | `ProcessEntry[]` | Note: rc-agent calls without auth (known bug — 401) |
| POST | /config/kiosk-allowlist | `{ name: string, hash?: string }` | `{ ok: boolean }` |
| DELETE | /config/kiosk-allowlist/{name} | - | `{ ok: boolean }` |
| GET | /pos/lockdown | - | `{ locked: boolean }` |
| POST | /pos/lockdown | `{ locked: boolean }` | `{ ok: boolean }` |

### Kiosk Management (admin-only create/update)

| Method | Path | Request | Response |
|--------|------|---------|----------|
| POST | /kiosk/experiences | `KioskExperienceCreate` | `KioskExperience` |
| GET | /kiosk/experiences/{id} | - | `KioskExperience` |
| PUT | /kiosk/experiences/{id} | `KioskExperienceUpdate` | `KioskExperience` |
| DELETE | /kiosk/experiences/{id} | - | `{ ok: boolean }` |
| PUT | /kiosk/settings | `KioskSettingsUpdate` | `KioskSettings` |

### Accounting & Audit

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /accounting/accounts | - | `Account[]` |
| GET | /accounting/trial-balance | - | `TrialBalance` |
| GET | /accounting/profit-loss | `?from=&to=` | `ProfitLoss` |
| GET | /accounting/balance-sheet | - | `BalanceSheet` |
| GET | /accounting/journal | - | `JournalEntry[]` |
| GET | /audit-log | `?from=&to=&actor=` | `AuditEntry[]` |

### Deploy

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /deploy/status | - | `DeployPodStatus[]` |
| POST | /deploy/rolling | `{ version: string }` | `{ job_id: string }` |
| POST | /deploy/{pod_id} | `{ version: string }` | `DeployPodStatus` |

### Tournaments & Events (staff)

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /tournaments | - | `Tournament[]` |
| POST | /tournaments | `TournamentCreate` | `Tournament` |
| GET | /tournaments/{id} | - | `Tournament` |
| PUT | /tournaments/{id} | `TournamentUpdate` | `Tournament` |
| GET | /tournaments/{id}/registrations | - | `TournamentRegistration[]` |
| GET | /tournaments/{id}/matches | - | `TournamentMatch[]` |
| POST | /tournaments/{id}/generate-bracket | - | `{ ok: boolean }` |
| POST | /tournaments/{id}/matches/{match_id}/result | `{ winner_id: string }` | `{ ok: boolean }` |
| GET | /time-trials | - | `TimeTrial[]` |
| POST | /time-trials | `TimeTrialCreate` | `TimeTrial` |
| PUT | /time-trials/{id} | `TimeTrialUpdate` | `TimeTrial` |
| DELETE | /time-trials/{id} | - | `{ ok: boolean }` |
| POST | /staff/events | `HotlapEventCreate` | `HotlapEvent` |
| GET | /staff/events | - | `HotlapEvent[]` |
| GET | /staff/events/{id} | - | `HotlapEvent` |
| PUT | /staff/events/{id} | `HotlapEventUpdate` | `HotlapEvent` |
| POST | /staff/championships | `ChampionshipCreate` | `Championship` |
| GET | /staff/championships | - | `Championship[]` |
| GET | /staff/championships/{id} | - | `Championship` |
| POST | /staff/championships/{id}/rounds | `RoundCreate` | `{ ok: boolean }` |
| POST | /staff/events/{id}/link-session | `{ group_session_id: string }` | `{ ok: boolean }` |
| POST | /staff/group-sessions/{id}/complete | - | `{ ok: boolean }` |

### AI & Debug

| Method | Path | Request | Response |
|--------|------|---------|----------|
| POST | /ai/chat | `{ message: string }` | `{ reply: string }` |
| POST | /ai/diagnose | `{ pod_id: string, symptoms: string }` | `DiagnosticReport` |
| GET | /ai/suggestions | - | `AiSuggestion[]` |
| POST | /ai/suggestions/{id}/dismiss | - | `{ ok: boolean }` |
| GET | /ai/training/stats | - | `TrainingStats` |
| GET | /ai/training/pairs | - | `TrainingPair[]` |
| POST | /ai/training/import | `TrainingPairImport` | `{ ok: boolean }` |
| GET | /ops/stats | - | `OpsStats` |
| GET | /activity | - | `ActivityEvent[]` |
| GET | /debug/activity | - | `ActivityEvent[]` |
| GET | /debug/playbooks | - | `Playbook[]` |
| GET | /debug/incidents | - | `DebugIncident[]` |
| POST | /debug/incidents | `IncidentCreate` | `DebugIncident` |
| PUT | /debug/incidents/{id} | `IncidentUpdate` | `DebugIncident` |
| POST | /debug/diagnose | `DiagnoseRequest` | `DiagnosticReport` |

### Scheduler

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /scheduler/status | - | `SchedulerStatus` |
| PUT | /scheduler/settings | `SchedulerSettings` | `{ ok: boolean }` |
| GET | /scheduler/analytics | - | `SchedulerAnalytics` |

### Psychology & Review

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /psychology/badges | - | `Badge[]` |
| GET | /psychology/badges/{driver_id} | - | `Badge[]` |
| GET | /psychology/streaks/{driver_id} | - | `StreakInfo` |
| GET | /psychology/nudge-queue | - | `NudgeEntry[]` |
| POST | /psychology/test-nudge | `{ driver_id: string }` | `{ ok: boolean }` |
| GET | /review-nudges/pending | - | `ReviewNudge[]` |
| POST | /review-nudges/{id}/sent | - | `{ ok: boolean }` |

### Cafe (staff management)

| Method | Path | Request | Response |
|--------|------|---------|----------|
| GET | /cafe/items | - | `CafeItem[]` |
| POST | /cafe/items | `CafeItemCreate` | `CafeItem` |
| PUT | /cafe/items/{id} | `CafeItemUpdate` | `CafeItem` |
| DELETE | /cafe/items/{id} | - | `{ ok: boolean }` |
| POST | /cafe/items/{id}/toggle | - | `{ ok: boolean }` |
| POST | /cafe/items/{id}/image | multipart | `{ url: string }` |
| POST | /cafe/items/{id}/restock | `{ quantity: number }` | `{ ok: boolean }` |
| GET | /cafe/items/low-stock | - | `CafeItem[]` |
| GET | /cafe/categories | - | `CafeCategory[]` |
| POST | /cafe/categories | `CategoryCreate` | `CafeCategory` |
| POST | /cafe/import/preview | multipart CSV | `ImportPreview` |
| POST | /cafe/import/confirm | `{ import_id: string }` | `{ ok: boolean }` |
| POST | /cafe/orders | `{ pod_id: string, items: OrderItem[] }` | `{ order_id: string }` |
| GET | /cafe/promos | - | `CafePromo[]` |
| POST | /cafe/promos | `CafePromoCreate` | `CafePromo` |
| PUT | /cafe/promos/{id} | `CafePromoUpdate` | `CafePromo` |
| DELETE | /cafe/promos/{id} | - | `{ ok: boolean }` |
| POST | /cafe/promos/{id}/toggle | - | `{ ok: boolean }` |
| POST | /cafe/marketing/broadcast | `{ promo_id: string }` | `{ sent: number }` |

---

## 6. racecontrol — Service Endpoints (terminal_secret auth in handler)

These are called by cloud backend (Bono VPS), comms-link relay, and the WhatsApp bot. Auth: `X-Terminal-Secret` header.

### Cloud Sync

| Method | Path | Auth | Request | Response |
|--------|------|------|---------|----------|
| GET | /sync/changes | terminal_secret | `?since=timestamp` | `SyncChangeset` |
| POST | /sync/push | terminal_secret | `SyncChangeset` | `{ ok: boolean }` |
| GET | /sync/health | terminal_secret | - | `{ status: string, last_sync: string }` |
| POST | /sync/import-sessions | terminal_secret | `SessionImport[]` | `{ imported: number }` |

### Cloud Action Queue

| Method | Path | Auth | Request | Response |
|--------|------|------|---------|----------|
| POST | /actions | terminal_secret | `ActionCreate` | `Action` |
| GET | /actions/pending | terminal_secret | - | `Action[]` |
| POST | /actions/process | terminal_secret | - | `{ processed: number }` |
| POST | /actions/{id}/ack | terminal_secret | - | `{ ok: boolean }` |
| GET | /actions/history | terminal_secret | - | `Action[]` |

### Terminal (remote command execution)

| Method | Path | Auth | Request | Response |
|--------|------|------|---------|----------|
| POST | /terminal/auth | terminal_secret | `{ secret: string }` | `{ token: string }` |
| GET | /terminal/commands | terminal_secret | - | `TerminalCommand[]` |
| POST | /terminal/commands | terminal_secret | `{ cmd: string }` | `TerminalCommand` |
| GET | /terminal/commands/pending | terminal_secret | - | `TerminalCommand[]` |
| POST | /terminal/commands/{id}/result | terminal_secret | `{ output: string, exit_code: number }` | `{ ok: boolean }` |
| POST | /terminal/book-multiplayer | terminal_secret | `{ pods: string[], experience_id: string }` | `{ group_session_id: string }` |
| GET | /terminal/group-sessions | terminal_secret | - | `GroupSession[]` |

### Bot (WhatsApp)

| Method | Path | Auth | Request | Response |
|--------|------|------|---------|----------|
| GET | /bot/lookup | terminal_secret | `?phone=...` | `DriverLookup?` |
| GET | /bot/pricing | terminal_secret | - | `PricingTier[]` |
| POST | /bot/book | terminal_secret | `{ phone: string, tier_id: string }` | `{ ok: boolean, auth_token: string }` |
| GET | /bot/pods-status | terminal_secret | - | `{ available: number, total: number }` |
| GET | /bot/events | terminal_secret | - | `Event[]` |
| GET | /bot/leaderboard | terminal_secret | - | `LeaderboardEntry[]` |
| GET | /bot/customer-stats | terminal_secret | `?phone=...` | `DriverStats` |
| POST | /bot/register-lead | terminal_secret | `{ phone: string, name: string }` | `{ ok: boolean }` |

### Other Service

| Method | Path | Auth | Request | Response |
|--------|------|------|---------|----------|
| GET | /logs | terminal_secret | - | `{ lines: string[] }` |
| POST | /failover/broadcast | terminal_secret | `{ message: string }` | `{ sent: number }` |
| POST | /guard/report | X-Guard-Token | `GuardReport` | `{ ok: boolean }` |

---

## 7. rc-agent :8090 Endpoints

Runs on each pod at `http://192.168.31.{pod_ip}:8090`. No auth (LAN-only).

| Method | Path | Auth | Request | Response | Notes |
|--------|------|------|---------|----------|-------|
| GET | /ping | None | - | `"pong"` | Quick liveness check |
| GET | /health | None | - | `AgentHealth` | Fleet probe calls this every ~30s |
| GET | /info | None | - | `SystemInfo` | System info including memory, CPU |
| POST | /exec | None | `ExecRequest` | `ExecResponse` | Remote command execution |
| GET | /files | None | `?path=string` | `FileEntry[]` | List directory |
| GET | /file | None | `?path=string` | binary | Read file (max 50MB) |
| POST | /write | None | `{ path: string, content: string }` | `{ ok: boolean }` | Write file |
| POST | /mkdir | None | `{ path: string }` | `{ ok: boolean }` | Create directory |
| GET | /screenshot | None | - | JPEG binary | Screen capture |
| GET | /cursor | None | - | `{ x: number, y: number }` | Cursor position |
| POST | /input | None | `InputEvent` | `{ ok: boolean }` | Mouse/keyboard input |

**Note:** `/info`, `/files`, `/file`, `/exec`, `/write`, `/mkdir`, `/screenshot`, `/cursor`, `/input` were previously protected by `RCAGENT_SERVICE_KEY` middleware. Middleware was removed — pods are LAN-only behind firewall.

---

## 8. comms-link Relay :8766 Endpoints

Runs on James's machine (`http://localhost:8766` or `http://192.168.31.27:8766`).

| Method | Path | Auth | Request | Response | Notes |
|--------|------|------|---------|----------|-------|
| POST | /relay/exec/run | None (LAN) | `{ command: string, reason: string }` | `{ ok: boolean, result: string }` | Single command relay to Bono VPS |
| POST | /relay/chain/run | None (LAN) | `{ steps: Step[] }` or `{ template: string }` | `{ ok: boolean, results: StepResult[] }` | Multi-step chain |
| GET | /relay/health | None (LAN) | - | `{ connected: boolean, mode: string }` | Connection status |

---

## 9. Key Shared Shapes

### PodFleetStatus (GET /api/v1/fleet/health returns `PodFleetStatus[]`)

| Field | Type | Nullable | Notes |
|-------|------|----------|-------|
| pod_number | number | No | 1–8 |
| pod_id | string? | Yes | null if pod not registered |
| ws_connected | boolean | No | WebSocket connection to server |
| http_reachable | boolean | No | HTTP probe to :8090/health succeeded |
| version | string? | Yes | rc-agent semver, null if no StartupReport yet |
| build_id | string? | Yes | Git commit hash from rc-agent /health |
| uptime_secs | number? | Yes | Computed from agent_started_at |
| crash_recovery | boolean? | Yes | True if rc-agent restarted after crash |
| ip_address | string? | Yes | Pod LAN IP |
| last_seen | string? | Yes | ISO-8601 timestamp |
| last_http_check | string? | Yes | ISO-8601 timestamp of last probe attempt |
| in_maintenance | boolean | No | True if PreFlightFailed and not cleared |
| maintenance_failures | string[] | No | Check names from last PreFlightFailed |
| violation_count_24h | number | No | Process violations in last 24h |
| last_violation_at | string? | Yes | ISO-8601 timestamp of most recent violation |
| idle_health_fail_count | number | No | Consecutive idle health failures (0 = healthy) |
| idle_health_failures | string[] | No | Check names from most recent IdleHealthFailed |

### PodInfo (GET /api/v1/pods returns `PodInfo[]`)

| Field | Type | Nullable | Notes |
|-------|------|----------|-------|
| id | string | No | UUID |
| number | number | No | 1–8 |
| name | string | No | Display name |
| ip_address | string | No | LAN IP |
| mac_address | string? | Yes | WoL target |
| sim_type | SimType | No | Primary sim installed |
| status | PodStatus | No | `offline\|idle\|in_session\|error\|disabled` |
| current_driver | string? | Yes | Driver name |
| current_session_id | string? | Yes | Active session UUID |
| last_seen | string? | Yes | ISO-8601 |
| driving_state | DrivingState? | Yes | `active\|idle\|no_device` |
| billing_session_id | string? | Yes | Active billing session UUID |
| game_state | GameState? | Yes | `idle\|launching\|loading\|running\|stopping\|error` |
| current_game | SimType? | Yes | Currently running sim |
| installed_games | SimType[] | No | Defaults to [] |
| screen_blanked | boolean? | Yes | True = black screen active |
| ffb_preset | string? | Yes | `"light"\|"medium"\|"strong"` |
| freedom_mode | boolean? | Yes | All restrictions lifted |

### BillingSessionInfo (GET /api/v1/billing/active returns `BillingSessionInfo[]`)

| Field | Type | Nullable | Notes |
|-------|------|----------|-------|
| id | string | No | UUID |
| driver_id | string | No | UUID |
| driver_name | string | No | Display name |
| pod_id | string | No | UUID |
| pricing_tier_name | string | No | e.g. "30 min" |
| allocated_seconds | number | No | Total purchased time |
| driving_seconds | number | No | Seconds spent driving |
| remaining_seconds | number | No | Time left |
| status | BillingSessionStatus | No | See enum below |
| driving_state | DrivingState | No | `active\|idle\|no_device` |
| started_at | string? | Yes | ISO-8601 |
| split_count | number | No | Number of sub-sessions (1 = no split) |
| split_duration_minutes | number? | Yes | Per-split duration |
| current_split_number | number | No | 1-indexed |
| elapsed_seconds | number? | Yes | Count-up model; null for legacy countdown |
| cost_paise | number? | Yes | Running cost in paise |
| rate_per_min_paise | number? | Yes | 2330 standard, 1500 value |

**BillingSessionStatus enum:** `pending | waiting_for_game | active | paused_manual | paused_disconnect | paused_game_pause | completed | ended_early | cancelled`

### Driver (GET /api/v1/public/drivers/{id})

| Field | Type | Nullable | Notes |
|-------|------|----------|-------|
| id | string | No | UUID |
| name | string | No | Display name |
| email | string? | Yes | |
| phone | string? | Yes | |
| steam_guid | string? | Yes | |
| iracing_id | string? | Yes | |
| total_laps | number | No | All-time lap count |
| total_time_ms | number | No | All-time driving time |
| created_at | string | No | ISO-8601 |

### AgentHealth (GET :8090/health)

| Field | Type | Nullable | Notes |
|-------|------|----------|-------|
| status | string | No | Always `"ok"` |
| version | string | No | rc-agent semver |
| build_id | string | No | Git commit hash |
| uptime_secs | number | No | Seconds since start |
| exec_slots_available | number | No | Free exec slots (max 8) |
| exec_slots_total | number | No | Always 8 |

### ExecRequest / ExecResponse (POST :8090/exec)

**Request:**
```
{ cmd: string, timeout_ms?: number, detached?: boolean }
```

**Response:**
```
{ success: boolean, exit_code: number?, stdout: string, stderr: string }
```
Special sentinel: `cmd = "RCAGENT_SELF_RESTART"` — triggers rc-agent self-restart via `relaunch_self()`.

### AuthTokenInfo (GET /api/v1/auth/pending)

| Field | Type | Nullable | Notes |
|-------|------|----------|-------|
| id | string | No | UUID |
| pod_id | string | No | UUID |
| driver_id | string | No | UUID |
| driver_name | string | No | |
| pricing_tier_id | string | No | UUID |
| pricing_tier_name | string | No | |
| auth_type | string | No | `"pin"\|"qr"` |
| token | string | No | PIN or QR payload |
| status | string | No | `"pending"\|"consumed"\|"expired"\|"cancelled"` |
| allocated_seconds | number | No | |
| custom_price_paise | number? | Yes | Override price |
| custom_duration_minutes | number? | Yes | Override duration |
| created_at | string | No | ISO-8601 |
| expires_at | string | No | ISO-8601 |

---

## 10. Auth Architecture Summary

| JWT Type | Issued by | Used for | Expiry |
|----------|-----------|---------|--------|
| Customer JWT | /customer/verify-otp | Customer endpoints, kiosk redeem | Session-lived |
| Staff JWT | /staff/validate-pin, /auth/kiosk/validate-pin, /auth/admin-login | Kiosk, staff, admin endpoints | Shift-lived |
| terminal_secret | Config (racecontrol.toml) | Service/sync/bot/terminal routes | Never expires |
| X-Guard-Token | Config (process_guard.report_secret) | /guard/report only | Never expires |

**Pod source block:** Staff routes reject requests from pod IPs (192.168.31.28–192.168.31.91 range) with 403. Kiosk routes bypass this block — pods can call them with a staff JWT.
