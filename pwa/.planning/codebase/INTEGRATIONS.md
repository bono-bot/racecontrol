# External Integrations

**Analysis Date:** 2026-03-21

## APIs & External Services

**Racing Point Backend Server:**
- Base URL: `NEXT_PUBLIC_API_URL` (default: `http://localhost:8080/api/v1`)
- API version: v1
- Location: `src/lib/api.ts` (centralized API client)
- Auth: JWT Bearer token in `Authorization` header
- Token storage: `localStorage.rp_token`

**Payment & Wallet:**
- Service: Razorpay (payment processor)
- SDK: Loaded from CDN (`https://checkout.razorpay.com/v1/checkout.js`)
- Window object: `window.Razorpay` (instantiated client)
- Usage: `src/app/wallet/topup/page.tsx`
- Flow:
  1. Create order via `/api/payments/create-order` (Gateway URL)
  2. Open Razorpay checkout modal
  3. Payment webhook handled by server
  4. Client polls wallet balance after 3 seconds
- Supported methods: UPI, Credit Card, Debit Card, Net Banking

**Payment Gateway:**
- Gateway URL: `NEXT_PUBLIC_GATEWAY_URL` (default: `/api/payments`)
- Endpoint: `POST /create-order`
- Request: `{ amount_paise: number }`
- Response: `{ order_id, amount, currency, key_id }`
- Auth: Bearer token (from localStorage)

## Data Storage

**Databases:**
- Not directly accessed from PWA - all data flows through Racing Point server API

**File Storage:**
- Local storage: `localStorage` (browser)
  - `rp_token` - JWT authentication token
  - `rp_auth_v` - Auth version flag (version gating on login)

**Caching:**
- In-memory React state only - no persistent client-side cache
- Server API determines cache strategy

## Authentication & Identity

**Auth Provider:**
- Custom (Racing Point backend)
  - Implementation: OTP via WhatsApp + phone-based registration
  - Flow:
    1. `POST /customer/login` with phone number
    2. `POST /customer/verify-otp` with phone + 6-digit OTP
    3. Server returns JWT token
    4. Token stored in `localStorage.rp_token`
    5. Token sent in all subsequent requests via `Authorization: Bearer <token>`

**Session Management:**
- JWT token stored in localStorage
- Auto-logout triggered on:
  - JWT decode error
  - Missing Authorization header
  - session_expired response
  - Explicit `_clear` flag in error response
- Force logout clears token and redirects to `/login`

## Backend API Endpoints (by category)

**Authentication:**
- `POST /customer/login` - Send OTP
- `POST /customer/verify-otp` - Verify OTP and get token
- `POST /auth/validate-qr` - QR code authentication for pod check-in

**Customer Profile:**
- `GET /customer/profile` - Fetch driver profile
- `PUT /customer/profile` - Update profile (nickname, leaderboard visibility)
- `POST /customer/register` - Register new driver

**Sessions & Billing:**
- `GET /customer/sessions` - List all sessions
- `GET /customer/sessions/{id}` - Session details with laps and events
- `POST /customer/book` - Book experience session
- `POST /customer/book-multiplayer` - Book multiplayer session
- `GET /customer/active-reservation` - Check current booking
- `POST /customer/end-reservation` - End active session
- `POST /customer/continue-session` - Extend session time

**Wallet & Payments:**
- `GET /customer/wallet` - Wallet balance and totals
- `GET /customer/wallet/transactions` - Transaction history
- `GET /wallet/bonus-tiers` - Bonus percentage tiers by amount

**Experiences & Catalog:**
- `GET /customer/experiences` - Available experiences and pricing tiers
- `GET /customer/ac/catalog` - Assetto Corsa car/track catalog

**Laps & Records:**
- `GET /customer/laps` - Driver's lap records
- `GET /customer/stats` - Career statistics
- `GET /customer/compare-laps` - Compare laps by track/car
- `GET /leaderboard/{track}` - Track leaderboard

**Telemetry:**
- `GET /customer/telemetry` - Real-time telemetry frame (live while driving)
- `GET /customer/active-session/events` - Active session events (laps, penalties)

**AI & Coaching:**
- `POST /customer/ai/chat` - Chat with AI coach
  - Supports multi-turn conversation history
  - Returns: reply text and model name

**Social:**
- `GET /customer/friends` - Friend list
- `GET /customer/friends/requests` - Incoming and outgoing requests
- `POST /customer/friends/request` - Send friend request
- `POST /customer/friends/request/{id}/accept` - Accept request
- `POST /customer/friends/request/{id}/reject` - Reject request
- `DELETE /customer/friends/{id}` - Remove friend
- `PUT /customer/presence` - Set online/offline status

**Multiplayer:**
- `GET /customer/group-session` - Current group session info
- `POST /customer/group-session/{id}/accept` - Accept group invite
- `POST /customer/group-session/{id}/decline` - Decline group invite
- `GET /customer/multiplayer-results/{id}` - Multiplayer race results

**Memberships & Packages:**
- `GET /customer/packages` - Package offerings
- `GET /customer/membership` - Membership status and available tiers
- `POST /customer/membership/subscribe` - Subscribe to membership

**Tournaments:**
- `GET /customer/tournaments` - Available tournaments
- `POST /customer/tournaments/{id}/register` - Register for tournament

**Gamification:**
- `GET /customer/passport` - Driving passport (tracks visited, cars driven)
- `GET /customer/badges` - Achievement badges (earned and available)
- `GET /customer/referral-code` - Referral code and successful referrals
- `POST /customer/referral-code/generate` - Generate new referral code
- `POST /customer/redeem-referral` - Redeem referral bonus
- `POST /customer/apply-coupon` - Apply discount coupon

**Session Share:**
- `GET /customer/sessions/{id}/share` - Generate shareable session report

**Public (No Auth):**
- `GET /public/leaderboard` - Global leaderboard
- `GET /public/leaderboard/{track}` - Track-specific leaderboard
- `GET /public/circuit-records` - Best times by circuit
- `GET /public/vehicle-records/{car}` - Best times by vehicle
- `GET /public/drivers` - Search drivers by name
- `GET /public/drivers/{id}` - Public driver profile
- `GET /public/time-trial` - Current time trial info
- `GET /public/laps/{id}/telemetry` - Lap telemetry data (public)
- `GET /public/sessions/{id}` - Public session summary

**Pod Control (In-Session):**
- `POST /pods/{podId}/assists` - Set assists (ABS, TC)
- `POST /pods/{podId}/ffb` - Set force feedback gain
- `GET /pods/{podId}/assist-state` - Read current assist settings

**Terminal (Admin):**
- `POST /terminal/auth` - Authenticate to terminal session
- `POST /terminal/commands` - Submit command
- `GET /terminal/commands` - List command history
- Auth: session token OR `x-terminal-secret: rp-terminal-2026` header

**Venue Info (Public):**
- `GET /venue` - Venue name and location

## Error Handling

**Response Format:**
- All API responses are JSON
- HTTP status codes indicate success/failure
- Error field in response body with descriptive message

**Auth Errors:**
- 401 responses trigger auto-logout
- Error messages checked for: "JWT decode error", "Missing Authorization", "session_expired"
- `_clear` flag in response also triggers logout

**Network Errors:**
- Caught and re-thrown with context (e.g., "HTTP 404: non-JSON response")
- Client handles gracefully with user-friendly messages

## Environment Configuration

**Required env vars:**
- `NEXT_PUBLIC_API_URL` - Backend server endpoint (required for all API calls)
  - Default: `http://localhost:8080/api/v1`
  - Example: `http://192.168.31.23:8080/api/v1` (production server)
- `NEXT_PUBLIC_GATEWAY_URL` - Payment gateway route (required for wallet top-up)
  - Default: `/api/payments` (relative to PWA)
  - Example: `http://payment-gateway.internal/api/payments`

**Secrets location:**
- Razorpay Key ID returned in `/api/payments/create-order` response
- No hardcoded secrets in PWA code
- Terminal secret: `rp-terminal-2026` (admin endpoint protection)

**Build-time configuration:**
- `NEXT_PUBLIC_API_URL` passed as Docker ARG (no .env file)
- Override at runtime via Docker -e flag or environment injection

## Webhooks & Callbacks

**Incoming:**
- Razorpay webhook (received by server, not PWA)
  - Webhook updates wallet balance
  - PWA polls `/customer/wallet` after payment

**Outgoing:**
- None from PWA directly
- All operations go to Racing Point backend API
- Terminal commands can be submitted via `POST /terminal/commands`

## Type Definitions & Data Structures

**Core Domain Types** (in `src/lib/api.ts`):
- `DriverProfile` - User identity and stats
- `WalletInfo` - Balance and transaction totals
- `PodReservation` - Active pod booking
- `BillingSession` - Usage record for pod time
- `LapRecord` - Individual lap performance data
- `SessionDetail` - Full session with laps and events
- `Experience` - Predefined racing scenarios
- `PricingTier` - Session duration/cost options
- `TelemetryFrame` - Real-time telemetry sample
- `GroupSessionInfo` - Multiplayer race state
- `PassportData` - Track/car collection badges
- `Badge` - Achievement unlocks
- `MembershipInfo` - Subscription status
- `TournamentInfo` - Event registration

**Response Types:**
All API methods return typed responses with optional error field for standardized error handling.

---

*Integration audit: 2026-03-21*
