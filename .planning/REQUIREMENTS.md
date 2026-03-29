# Requirements: v27.0 Workflow Integrity & Compliance Hardening

**Defined:** 2026-03-29
**Core Value:** Every customer interaction — from registration to refund — is atomic, auditable, safe, and legally compliant

## v27.0 Requirements

### Financial Atomicity (FATM)

- [x] **FATM-01**: Billing start wraps wallet debit + session creation + journal entry in a single DB transaction (rollback on any failure)
- [x] **FATM-02**: All money-moving POSTs (/topup, /billing/start, /billing/stop, /refund) require idempotency keys; duplicate requests return original result
- [x] **FATM-03**: Wallet debit uses SELECT FOR UPDATE row locking to prevent parallel overspend
- [x] **FATM-04**: Session finalization uses compare-and-swap (UPDATE WHERE status='active') to prevent double-end/double-refund
- [x] **FATM-05**: Tier price and rate calculation aligned — tier_30min price matches compute_session_cost(1800s) output
- [x] **FATM-06**: Refund formula uses a single authoritative calculation path (not duplicated in multiple code paths)
- [ ] **FATM-07**: Extension purchase is atomic with session time addition (debit + add time in one transaction)
- [ ] **FATM-08**: Coupon lifecycle is stateful: available → reserved → redeemed → released (with TTL on reserved)
- [ ] **FATM-09**: Coupon restored on session cancellation/failure before billing commit
- [ ] **FATM-10**: Discount stacking rules enforced server-side with hard floor on payable amount
- [ ] **FATM-11**: Payment gateway webhook: wallet credited only after verified gateway confirmation with idempotent application
- [x] **FATM-12**: Scheduled reconciliation job detects wallet vs journal vs session balance drift

### State Machine Integrity (FSM)

- [x] **FSM-01**: Billing session state transitions validated via server-side allowed-transitions table (current_state + event → new_state or REJECT)
- [x] **FSM-02**: Cross-FSM invariant enforced: billing=active requires game≠Idle (phantom billing guard)
- [x] **FSM-03**: Cross-FSM invariant enforced: game=Running requires billing≠cancelled (free gaming guard)
- [x] **FSM-04**: Crash recovery atomically pauses billing before any relaunch attempt
- [x] **FSM-05**: StopGame handled in every recovery FSM state (not silently dropped)
- [x] **FSM-06**: Billing pause timeout and crash recovery auto-end share a single authoritative end-session trigger
- [x] **FSM-07**: Split session modeled as parent order with child entitlements, each with immutable duration and state
- [x] **FSM-08**: Split transition persisted to DB before any new launch command issued
- [x] **FSM-09**: Billing timer state persisted to DB every 60 seconds (survives server restart)
- [x] **FSM-10**: On server startup, orphaned "active" sessions with no heartbeat for 5+ minutes auto-flagged and alerted

### Security (SEC)

- [x] **SEC-01**: Car/track/skin names in launch_args validated against whitelist (alphanumeric + hyphen + underscore only; reject newlines, =, [)
- [x] **SEC-02**: FFB GAIN capped at 100 server-side before sending to agent (physical safety)
- [x] **SEC-03**: PIN redemption uses atomic CAS (UPDATE WHERE redeemed_at IS NULL; check affected rows)
- [x] **SEC-04**: Role-based access control: cashier (topup, session view), manager (reports, pricing), superadmin (system config)
- [x] **SEC-05**: Staff self-top-up blocked at API layer (requesting_user_id != target_user_id for non-superadmin)
- [x] **SEC-06**: Audit log table is append-only (no DELETE permission for staff-level DB users)
- [x] **SEC-07**: WebSocket connections use WSS (TLS) between server and agents
- [x] **SEC-08**: OTP codes stored as bcrypt hashes, not plaintext
- [x] **SEC-09**: PII masked in staff dashboard by default (phone, email); reveal requires manager role
- [x] **SEC-10**: Agent mutex during clean_state_reset — LaunchGame queued until reset completes

### Legal Compliance (LEGAL)

- [x] **LEGAL-01**: 18% GST separated in double-entry journal (Revenue vs GST Payable ledger lines)
- [x] **LEGAL-02**: GST-compliant invoice generated per session with GSTIN, HSN code, tax breakup
- [x] **LEGAL-03**: Waiver signing required before billing start (block if waiver_signed=0 on POS path)
- [x] **LEGAL-04**: Minors (under 18): mandatory guardian name + phone + verifiable consent (OTP to guardian phone)
- [x] **LEGAL-05**: Minor sessions: guardian physical presence acknowledgment recorded (staff confirms via UI toggle)
- [x] **LEGAL-06**: Enhanced liability coverage for minors documented — waiver limitation disclosure shown to guardian
- [x] **LEGAL-07**: Pricing and refund policy displayed on kiosk before wallet top-up (Consumer Protection Act 2019)
- [x] **LEGAL-08**: Data retention policy: financial records 8 years, PII purged/anonymized after 2 years of inactivity (DPDP Act)
- [x] **LEGAL-09**: Parental consent revocation mechanism in PWA (guardian can request data deletion)

### Game-Specific Hardening (GAME)

- [x] **GAME-01**: Steam pre-launch check: verify Steam process running + no pending updates before any Steam-URL game launch
- [x] **GAME-02**: Process name corrections in monitoring: F1, iRacing, LMU, Forza matched to actual exe names on disk
- [x] **GAME-03**: Forza Horizon 5 session enforcer: agent force-terminates after duration_minutes with graceful save warning
- [ ] **GAME-04**: AC EVO config adapter: translate launch_args to Unreal GameUserSettings.ini format (not AC race.ini)
- [ ] **GAME-05**: iRacing subscription check: verify account active before billing start (prevent charge-but-can't-play)
- [x] **GAME-06**: DLC availability check: verify car/track content installed on pod before launch
- [x] **GAME-07**: Steam "Preparing to launch" dialog detection: agent waits for actual game window, not just Steam response
- [x] **GAME-08**: Game-specific telemetry crash detection for non-AC games (process exit monitoring as fallback)

### Deployment Safety (DEPLOY)

- [ ] **DEPLOY-01**: OTA session drain: pods with active billing sessions defer binary swap until session ends
- [ ] **DEPLOY-02**: Graceful agent shutdown on SIGTERM: write shutdown_at to session row, trigger partial refund calculation
- [ ] **DEPLOY-03**: Deployment window lock: require manual override for OTA during 6-11 PM on weekends
- [ ] **DEPLOY-04**: Post-restart session resume: new agent checks DB for interrupted sessions and either resumes or refunds
- [ ] **DEPLOY-05**: WebSocket message-level idempotency keys (command_id UUID) to prevent stale replay on reconnect

### Staff Controls (STAFF)

- [ ] **STAFF-01**: Staff discount requires reason code and manager approval above configurable threshold
- [ ] **STAFF-02**: Self-service wallet operations blocked for staff's own account (API-level enforcement)
- [ ] **STAFF-03**: Daily staff override report: all discounts, manual refunds, tier changes with actor ID
- [ ] **STAFF-04**: Cash drawer reconciliation: end-of-day report comparing system cash total vs physical count input
- [ ] **STAFF-05**: Shift handoff: outgoing staff must acknowledge active sessions before logout; incoming staff sees briefing

### Billing Edge Cases (BILL)

- [ ] **BILL-01**: Inactivity detection: flag sessions with no input/lap progress for N minutes; alert staff
- [ ] **BILL-02**: Session countdown: persistent on-screen timer with yellow (5 min) and red (1 min) warnings
- [ ] **BILL-03**: PWA game-request timeout: 10-minute server-side TTL; auto-expire with customer notification
- [ ] **BILL-04**: Extension pricing rule documented and enforced: extensions use current tier effective rate
- [ ] **BILL-05**: Billing start-time defined: timer starts when game reaches Running state, not on staff click
- [ ] **BILL-06**: Recovery time excluded from billing: crash recovery pause window not charged to customer
- [ ] **BILL-07**: Multiplayer session billing: canonical session object with participant roster and synchronized billing
- [ ] **BILL-08**: Customer charge dispute portal: flag session in PWA; staff reviews logs and approves/denies refund

### Resilience (RESIL)

- [x] **RESIL-01**: SQLite WAL mode enabled with busy_timeout=5000ms
- [x] **RESIL-02**: Billing timer writes staggered by pod index (Pod 1 at :00, Pod 2 at :07, etc.)
- [x] **RESIL-03**: Orphaned session detection job: every 5 minutes, flag active sessions with no agent heartbeat for 5+ min
- [ ] **RESIL-04**: Hardware health heartbeat: agent polls wheel/pedal USB device presence every 5s; pause + alert on disconnect
- [ ] **RESIL-05**: Negative wallet balance alert: immediate staff notification + session block
- [ ] **RESIL-06**: Agent crash rate anomaly detection: >3 crashes in 1 hour on same pod triggers maintenance flag
- [ ] **RESIL-07**: Controls.ini reset between sessions: each new session writes fresh FFB/control config (no leakage)
- [ ] **RESIL-08**: Clock sync check: server vs pod timestamp drift >5s triggers warning in fleet health

### Notifications & UX (UX)

- [ ] **UX-01**: Notification outbox: durable table with states (pending, sent, delivered, failed); background retry with backoff
- [ ] **UX-02**: OTP fallback: if WhatsApp delivery fails, offer on-screen display or SMS fallback
- [ ] **UX-03**: Customer receipt: auto-generated after session end with before/after balance, duration, charges, refund
- [ ] **UX-04**: Leaderboard integrity: entries only from automatic verified session records (no manual entry)
- [ ] **UX-05**: Leaderboard segmented by game + track + car class + assist tier
- [ ] **UX-06**: Lap evidence: per-session lap-event records persisted (session_id, lap_time, validity, assist config)
- [ ] **UX-07**: Telemetry adapter crash marks affected laps as "unverifiable" (never silently lost)
- [ ] **UX-08**: Queue management: virtual queue with position and ETA shown in PWA/kiosk for walk-ins

## Out of Scope

| Feature | Reason |
|---------|--------|
| SQLite → PostgreSQL migration | Architectural change deferred to multi-venue milestone |
| Multi-venue wallet sharing | Requires shared database infrastructure |
| Real-time voice chat | Not core to billing/safety hardening |
| Mobile native app | PWA sufficient for current operations |
| Full i18n/l10n | English sufficient for current venue; defer to expansion |
| Automated NPS/CSAT surveys | Nice-to-have, not a P0/P1 finding |
| Tiered loyalty program | Business feature, not integrity/compliance |
| First-time user tutorial | UX improvement deferred to post-hardening |
| Social sharing features | Not related to integrity/compliance |

## Traceability

Updated during roadmap creation (2026-03-29).

| Requirement | Phase | Status |
|-------------|-------|--------|
| RESIL-01 | Phase 251 | Complete |
| RESIL-02 | Phase 251 | Complete |
| FSM-09 | Phase 251 | Complete |
| FSM-10 | Phase 251 | Complete |
| RESIL-03 | Phase 251 | Complete |
| FATM-01 | Phase 252 | Complete |
| FATM-02 | Phase 252 | Complete |
| FATM-03 | Phase 252 | Complete |
| FATM-04 | Phase 252 | Complete |
| FATM-05 | Phase 252 | Complete |
| FATM-06 | Phase 252 | Complete |
| FATM-12 | Phase 252 | Complete |
| FSM-01 | Phase 253 | Complete |
| FSM-02 | Phase 253 | Complete |
| FSM-03 | Phase 253 | Complete |
| FSM-04 | Phase 253 | Complete |
| FSM-05 | Phase 253 | Complete |
| FSM-06 | Phase 253 | Complete |
| FSM-07 | Phase 253 | Complete |
| FSM-08 | Phase 253 | Complete |
| SEC-01 | Phase 254 | Complete |
| SEC-02 | Phase 254 | Complete |
| SEC-03 | Phase 254 | Complete |
| SEC-04 | Phase 254 | Complete |
| SEC-05 | Phase 254 | Complete |
| SEC-06 | Phase 254 | Complete |
| SEC-07 | Phase 254 | Complete |
| SEC-08 | Phase 254 | Complete |
| SEC-09 | Phase 254 | Complete |
| SEC-10 | Phase 254 | Complete |
| LEGAL-01 | Phase 255 | Complete |
| LEGAL-02 | Phase 255 | Complete |
| LEGAL-03 | Phase 255 | Complete |
| LEGAL-04 | Phase 255 | Complete |
| LEGAL-05 | Phase 255 | Complete |
| LEGAL-06 | Phase 255 | Complete |
| LEGAL-07 | Phase 255 | Complete |
| LEGAL-08 | Phase 255 | Complete |
| LEGAL-09 | Phase 255 | Complete |
| GAME-01 | Phase 256 | Complete |
| GAME-02 | Phase 256 | Complete |
| GAME-03 | Phase 256 | Complete |
| GAME-04 | Phase 256 | Pending |
| GAME-05 | Phase 256 | Pending |
| GAME-06 | Phase 256 | Complete |
| GAME-07 | Phase 256 | Complete |
| GAME-08 | Phase 256 | Complete |
| BILL-01 | Phase 257 | Pending |
| BILL-02 | Phase 257 | Pending |
| BILL-03 | Phase 257 | Pending |
| BILL-04 | Phase 257 | Pending |
| BILL-05 | Phase 257 | Pending |
| BILL-06 | Phase 257 | Pending |
| BILL-07 | Phase 257 | Pending |
| BILL-08 | Phase 257 | Pending |
| STAFF-01 | Phase 258 | Pending |
| STAFF-02 | Phase 258 | Pending |
| STAFF-03 | Phase 258 | Pending |
| STAFF-04 | Phase 258 | Pending |
| STAFF-05 | Phase 258 | Pending |
| DEPLOY-01 | Phase 258 | Pending |
| DEPLOY-02 | Phase 258 | Pending |
| DEPLOY-03 | Phase 258 | Pending |
| DEPLOY-04 | Phase 258 | Pending |
| DEPLOY-05 | Phase 258 | Pending |
| FATM-07 | Phase 259 | Pending |
| FATM-08 | Phase 259 | Pending |
| FATM-09 | Phase 259 | Pending |
| FATM-10 | Phase 259 | Pending |
| FATM-11 | Phase 259 | Pending |
| UX-01 | Phase 260 | Pending |
| UX-02 | Phase 260 | Pending |
| UX-03 | Phase 260 | Pending |
| UX-04 | Phase 260 | Pending |
| UX-05 | Phase 260 | Pending |
| UX-06 | Phase 260 | Pending |
| UX-07 | Phase 260 | Pending |
| UX-08 | Phase 260 | Pending |
| RESIL-04 | Phase 260 | Pending |
| RESIL-05 | Phase 260 | Pending |
| RESIL-06 | Phase 260 | Pending |
| RESIL-07 | Phase 260 | Pending |
| RESIL-08 | Phase 260 | Pending |

**Coverage:**
- v27.0 requirements: 83 total (note: pre-MMA count was 72; final count after 3 audit iterations = 83)
- Mapped to phases: 83/83
- Unmapped: 0

---
*Requirements defined: 2026-03-29*
*Last updated: 2026-03-29 after MMA audit (3 iterations, 12 model runs)*
*Traceability populated: 2026-03-29 by roadmapper*
