---
gsd_state_version: 1.0
milestone: v26.1
milestone_name: Meshed Intelligence
status: executing
stopped_at: Completed 260-notifications-resilience-ux/260-01-PLAN.md
last_updated: "2026-03-29T10:56:15.233Z"
last_activity: 2026-03-29
progress:
  total_phases: 205
  completed_phases: 156
  total_plans: 382
  completed_plans: 374
  percent: 98
---

## Current Position

Phase: 260 (notifications-resilience-ux) — EXECUTING
Plan: 2 of 4
Status: Ready to execute
Last activity: 2026-03-29

Progress: [██████████] 98% (349/355 plans)

## Project Reference

**Milestone:** v27.0 Workflow Integrity & Compliance Hardening
**Core value:** Every customer interaction — from registration to refund — is atomic, auditable, safe, and legally compliant
**Phase range:** 251–260
**Roadmap:** .planning/ROADMAP-v27.md
**Requirements:** .planning/REQUIREMENTS.md (83 requirements, 10 categories)

See: .planning/PROJECT.md (project context)
See: UNIFIED-PROTOCOL.md (operations protocol v3.1)
See: .planning/ROADMAP-v27.md (this milestone's roadmap)

## Performance Metrics

- Requirements defined: 83
- Phases planned: 10
- Plans written: 0
- Plans complete: 0
- Ship gate status: Not started

## Phase Index

| # | Phase | Requirements | Status |
|---|-------|-------------|--------|
| 251 | Database Foundation | RESIL-01, RESIL-02, RESIL-03, FSM-09, FSM-10 | Plan 01 DONE, Plan 02 pending |
| 252 | Financial Atomicity Core | FATM-01–06, FATM-12 | COMPLETE (3/3 plans) |
| 253 | State Machine Hardening | FSM-01–08 | COMPLETE (3/3 plans) |
| 254 | Security Hardening | SEC-01–10 | COMPLETE (3/3 plans: P01 SEC-01/02/04, P02 SEC-03/06/08/09, P03 SEC-05/07/10) |
| 255 | Legal Compliance | LEGAL-01–09 | Not started |
| 256 | Game-Specific Hardening | GAME-01–08 | Not started |
| 257 | Billing Edge Cases | BILL-01–08 | Not started |
| 258 | Staff Controls & Deployment Safety | STAFF-01–05, DEPLOY-01–05 | Not started |
| 259 | Coupon & Discount System | FATM-07–11 | Not started |
| 260 | Notifications, Resilience & UX | UX-01–08, RESIL-04–08 | Not started |
| Phase 251-database-foundation P01 | 15min | 2 tasks | 3 files |
| Phase 251 P02 | 20 | 2 tasks | 2 files |
| Phase 252 P02 | 20 | 1 tasks | 1 files |
| Phase 252 P01 | 45 | 2 tasks | 4 files |
| Phase 252-financial-atomicity-core P03 | 15 | 1 tasks | 3 files |
| Phase 253-state-machine-hardening P01 | 35 | 2 tasks | 3 files |
| Phase 253-state-machine-hardening P02 | 30 | 2 tasks | 5 files |
| Phase 253-state-machine-hardening P03 | 17 | 2 tasks | 4 files |
| Phase 254 P02 | 35 | 2 tasks | 3 files |
| Phase 254-security-hardening P01 | 45 | 2 tasks | 5 files |
| Phase 254-security-hardening P03 | 90 | 2 tasks | 6 files |
| Phase 255-legal-compliance P01 | 25 | 2 tasks | 3 files |
| Phase 255-legal-compliance P03 | 25 | 1 tasks | 3 files |
| Phase 255-legal-compliance P02 | 35 | 2 tasks | 3 files |
| Phase 256 P01 | 35 | 2 tasks | 4 files |
| Phase 256-game-specific-hardening P02 | 23 | 2 tasks | 11 files |
| Phase 256-game-specific-hardening P03 | 30 | 2 tasks | 4 files |
| Phase 257-billing-edge-cases P01 | 45 | 2 tasks | 8 files |
| Phase 257-billing-edge-cases P02 | 45 | 2 tasks | 7 files |
| Phase 257-billing-edge-cases P03 | 37 | 2 tasks | 5 files |
| Phase 258-staff-controls-deployment-safety P01 | 22 | 1 tasks | 2 files |
| Phase 258-staff-controls-deployment-safety P03 | 38 | 2 tasks | 8 files |
| Phase 258-staff-controls-deployment-safety P02 | 525634 | 2 tasks | 3 files |
| Phase 259-coupon-discount-system P01 | 25 | 2 tasks | 2 files |
| Phase 259-coupon-discount-system P02 | 20 | 2 tasks | 4 files |
| Phase 260-notifications-resilience-ux P01 | 35 | 2 tasks | 6 files |

## Accumulated Context

### Key Architectural Decisions (from MMA audit that produced requirements)

- **WAL mode + staggered writes first** (Phase 251): All financial transactions require a stable DB layer. Phase 251 is the unblocking dependency for all other phases.
- **FATM split across two phases** (252 + 259): Core billing atomicity (FATM-01–06, FATM-12) is foundational; coupon/extension system (FATM-07–11) builds on top of it and can ship independently.
- **FSM depends on FATM** (Phase 253 after 252): Cross-FSM invariant guards (billing=active requires game≠Idle) need atomic billing start to be reliable first.
- **Security before Legal** (254 before 255): RBAC gates the legal workflow endpoints (waiver signing, minor consent). Phase 254 must ship first.
- **RwLock across .await is banned** (from standing rules): All lock acquisitions must snapshot then drop before any async call. This affects the WS broadcast path in fleet health.
- **Requirement count note**: REQUIREMENTS.md header says 72 but actual count is 83 (10 categories, counts verified line-by-line). Traceability updated to reflect 83.

### Open Issues Inherited from v26.0

- Server .23 Tailscale stuck in NoState — non-blocking
- Pod 3/6 spontaneous reboots (2026-03-22) — under investigation
- BUG: Server restart with fresh DB leaves pods table empty (auto-seed needed)
- Server schtasks (StartRCTemp, StartRCDirect) silently fail to start racecontrol

### Deferred (Out of Scope for v27.0)

- SQLite → PostgreSQL migration
- Multi-venue wallet sharing
- Real-time voice chat
- Mobile native app
- Full i18n/l10n

## Decisions (Phase 251)

- WAL verification uses fail-fast bail! at init_pool — server refuses to start if WAL mode fails (RESIL-01)
- Two coexisting billing sync loops: 5s for dashboard driving_seconds, 60s staggered for crash-recovery elapsed_seconds (RESIL-02, FSM-09)
- Stagger formula (N*7)%60 spreads 8 pods across 56 distinct seconds with no collisions
- COALESCE(elapsed_seconds, driving_seconds) recovery ensures old sessions recover correctly
- WhatsApp alerts use whatsapp_alerter::send_whatsapp gated on config.alerting.enabled (FSM-10, RESIL-03)
- Background orphan task has 300s initial delay to avoid double-alerting sessions caught by startup scan

## Decisions (Phase 254)

- Router::merge() for RBAC sub-routers avoids rewriting existing route list — manager+ and superadmin routes added as merged sub-routers with .layer() at end of staff_routes() (SEC-04)
- normalized_role() maps legacy 'staff' JWTs to 'cashier' for backward compatibility — no forced re-auth needed fleet-wide (SEC-04)
- validate_launch_args uses same allowlist as agent-side validate_content_id: ^[a-zA-Z0-9._-]{0,128}$ — defense-in-depth, server blocks before WS send to agent (SEC-01)
- admin_login PIN auth issues 'superadmin' role (not 'admin') for consistency with 3-tier RBAC tier names (SEC-04)
- FFB presets (light/medium/strong) pass through unchanged; only numeric values are capped at 100 or defaulted to 'medium' (SEC-02)
- Option<Extension<StaffClaims>> in topup_wallet: doesn't break unauthenticated callers; guard only fires when claims are present (SEC-05)
- game_launch_mutex lives in AppState not ConnectionState: survives WS reconnections without resetting mutex (SEC-10)
- native-tls over rustls: Windows certificate store integration for LAN deployments; tls_skip_verify fallback for self-signed certs (SEC-07)
- native-tls must be a DIRECT dep — transitive via tokio-tungstenite features flag does not expose crate name (SEC-07)

## Decisions (Phase 252)

- std::sync::OnceLock used for reconciliation status instead of once_cell — avoids new dependency (FATM-12)
- Reconciliation status stored in module-level atomics (not AppState) — diagnostic-only, no state management needed (FATM-12)
- HAVING ABS(balance - computed) > 0 LIMIT 100 caps query cost while catching all meaningful drift (FATM-12)
- 60s initial delay for reconciliation job (orphan detection uses 300s; reconciliation is less urgent) (FATM-12)

## Decisions (Phase 255)

- 18% inclusive GST split uses integer arithmetic: net_paise = amount * 100 / 118 — avoids floating-point precision in financial calculations (LEGAL-01)
- Invoice generation is non-critical post-commit: failure logs WARN, billing session continues — invoice is supplementary record, not a gate (LEGAL-02)
- VENUE_GSTIN hardcoded as placeholder constant `29AABCU9603R1ZX` with TODO — avoids config struct dependency (LEGAL-02)
- post_session_debit kept as backward-compatible wrapper calling post_session_debit_gst internally (LEGAL-01)
- Consumer Protection Act pricing disclosure: refund_policy + pricing_policy + gst_note added as static constants to pricing_display_handler (LEGAL-07)
- Driver row retained on anonymization (not deleted) — billing_sessions.driver_id FK must remain valid for 8-year financial retention (LEGAL-08)
- last_activity_at update is non-critical post-commit — failure does not affect billing start or topup result (LEGAL-08)
- Background job LIMIT 500 per cycle bounds write pressure; consent_revoked drivers excluded from job (already anonymized at revocation time) (LEGAL-08)
- Shared anonymize_driver_pii helper called by both customer POST /customer/revoke-consent and staff POST /drivers/{id}/revoke-consent (LEGAL-09)
- Waiver gate is hard block in start_billing — billing returns error if waiver_signed=0; placed before trial check and wallet ops (LEGAL-03)
- Guardian OTP reuses existing hash_otp/verify_otp_hash from auth — SEC-08 compliance by reuse, no new crypto primitives (LEGAL-04)
- Minor disclosure endpoint in public_routes (no auth) — kiosk must show Indian Contract Act text during registration (LEGAL-06)

## Decisions (Phase 256)

- AC skips check_steam_ready — Game Doctor Check 12 handles AC Steam validation, avoiding redundant 10s wait (GAME-01)
- wait_for_game_window uses ws_exec_result_tx not ws_tx clone — SplitSink is not Clone; result channel routes AgentMessage through event loop to WS send (GAME-07)
- check_dlc_installed returns Ok for custom Steam library paths — standard path check avoids false-blocking without full libraryfolders.vdf parsing (GAME-06)
- SteamOverlayUpdate.exe + package_installer.exe-with-Steam-in-path = update detection signals — avoids parsing Steam internal state files (GAME-01)

## Decisions (Phase 257)

- PauseReason enum (None/GamePause/CrashRecovery/Disconnect) on BillingTimer — crash recovery pause time excluded from billable seconds via recovery_pause_seconds.saturating_sub() (BILL-06)
- game_launch_requests INSERT non-fatal (let _ =) — TTL enforcement proceeds even if INSERT fails; customer receives request_id regardless (BILL-03)
- cleanup_expired_game_requests runs every 60s with DashboardEvent::GameRequestExpired broadcast — staff dashboard removes pending PWA request cards automatically (BILL-03)
- billing_timer_started audit event in start_billing_session() proves billing began at GameStateUpdate(Live) not staff launch click (BILL-05)
- extension_rate_policy=current_tier_effective_rate logged on every extension — audit trail for pricing disputes (BILL-04)
- Multiplayer crash uses pause_multiplayer_group() — crash is recoverable; billing ends only when all group members explicitly stop (BILL-07)
- AcStatus::Off + multiplayer group = group-wide pause; AcStatus::Off + single-player = end billing (BILL-07)
- Partial UNIQUE index WHERE status != 'denied' allows re-submission after denial for dispute_requests (BILL-08)
- Approve dispute uses compute_refund()+credit_in_tx() — reuses FATM-06/FATM-03 paths, no new money primitives (BILL-08)
- Dispute admin routes under manager+ RBAC (SEC-04) — financial resolution requires manager authorization (BILL-08)

## Session Continuity

Stopped at: Completed 260-notifications-resilience-ux/260-01-PLAN.md
Next action: Phase 257 Plan 1 complete. Proceed to Plan 2 (257-02).

- RESIL-01: DONE (WAL mode verification — 08acee0c)
- RESIL-02: DONE (Staggered timer writes by pod index — 6babdd40)
- FSM-09: DONE (Billing timer persisted every 60s — 6babdd40)
- FSM-10: DONE (Orphaned session detection on startup — a86f4710)
- RESIL-03: DONE (Background orphan detection job — 9ef6116e)
- FATM-01: DONE (Atomic billing start with single DB transaction — 252-01)
- FATM-02: DONE (Idempotency keys on money-moving endpoints — 252-01)
- FATM-03: DONE (debit_in_tx/credit_in_tx with wallet locking — 252-01)
- FATM-04: DONE (CAS session finalization — 252-02)
- FATM-05: DONE (Tier alignment in compute_session_cost — 252-02)
- FATM-06: DONE (Unified compute_refund() — 252-02)
- FATM-12: DONE (Background reconciliation job — 61c73467)
- SEC-01: DONE (Server-side launch_args INI injection prevention — 76e6e94c)
- SEC-02: DONE (FFB GAIN safety cap at 100 — 76e6e94c)
- SEC-04: DONE (Three-tier RBAC cashier/manager/superadmin — 778c6b46)
- SEC-03: DONE (OTP argon2 hashing — 173175d9)
- SEC-06: DONE (audit_log DELETE trigger — 173175d9)
- SEC-08: DONE (OTP argon2 hashing — 173175d9)
- SEC-09: DONE (PII masking in driver API responses by role — b73f7be0)
- LEGAL-01: DONE (GST-separated 3-line journal entries via post_session_debit_gst — 6791a153)
- LEGAL-02: DONE (invoices table + generate_invoice + GET endpoints — 6791a153, 6e395bca)
- LEGAL-07: DONE (refund_policy + pricing_policy + gst_note in pricing display — 6e395bca)
- LEGAL-03: DONE (waiver gate on start_billing — 12c1b62f)
- LEGAL-04: DONE (guardian OTP for minors — 12c1b62f)
- LEGAL-05: DONE (guardian_present required in billing for minors — 12c1b62f)
- LEGAL-06: DONE (GET /legal/minor-waiver-disclosure — 12c1b62f)
- LEGAL-08: DONE (data_retention_config + driver columns + daily anonymization job — 12c1b62f, 1db260dc)
- LEGAL-09: DONE (POST /customer/revoke-consent + POST /drivers/{id}/revoke-consent — 12c1b62f)
- BILL-03: DONE (game_launch_requests TTL + cleanup_expired_game_requests + GameRequestExpired event — 4efc070f)
- BILL-04: DONE (extension pricing validation + tier rate logging — 4efc070f)
- BILL-05: DONE (billing_timer_started audit event at game-live signal — 4efc070f)
- BILL-06: DONE (PauseReason enum + recovery_pause_seconds excluded from billable time — 4efc070f)
- BILL-07: DONE (pause_multiplayer_group + resume_multiplayer_group + crash detection integration — b44071f7)
- BILL-08: DONE (dispute_requests table + POST /customer/dispute + GET/POST /admin/disputes + approve/deny workflow — f6a3cb76)
- DEPLOY-02: DONE (Agent graceful shutdown with billing session HTTP notify + sentinel fallback — 74b11b47)
- DEPLOY-04: DONE (Post-restart sentinel recovery — recover_interrupted_sessions() — 74b11b47)
- DEPLOY-05: DONE (CoreMessage wrapper + seen_command_ids dedup in ws_handler — c9fa9b2a)

## Decisions (Phase 258 Plan 02)

- is_deploy_window_locked standalone fn with force+actor params — callers get exact error message for 423 response; no AppState needed
- OTA deploy window check via ?force=true query param (not JSON body) — OTA handler takes raw TOML, no JSON body available
- Shift handoff: active sessions require incoming_staff_id (400 validation error, not 403 auth) — it is a data constraint, not a permission problem
- DEPLOY-01 verified: all 3 call sites (billing.rs tick, billing.rs stop path, billing_fsm.rs FSM cancel) call check_and_trigger_pending_deploy after active_timers removal
- Dashboard WS deploy path: force=false, actor='dashboard' — no force override from dashboard UI
- STAFF-05: DONE (POST /staff/shift-handoff + GET /staff/shift-briefing — 1b038a5f)
- DEPLOY-01: DONE (Session drain verified across 3 hook points — a46f7c49)
- DEPLOY-03: DONE (is_deploy_window_locked weekend peak-hour lock with 423 + force override — a46f7c49)

## Decisions (Phase 258 Plan 03)

- CoreMessage wrapper uses serde flatten for backward-compatible command_id addition — old agents parse via bare CoreToAgentMessage fallback
- INTERRUPTED_SESSION sentinel file protocol: written on shutdown failure, consumed on startup — offline resilience for billing recovery
- agent-shutdown in public_routes (no JWT) gated by sentry_service_key Bearer header — agents lack staff JWT
- recover_interrupted_sessions() only runs once per process lifetime after first WS connect — prevents duplicate calls

Ship gate reminder (Unified Protocol v3.1):

1. Quality Gate: `cd comms-link && COMMS_PSK="..." bash test/run-all.sh`
2. E2E: live exec + chain + health round-trip (REALTIME mode)
3. Standing Rules: auto-push, Bono synced, watchdog, rules categorized
4. Multi-Model AI Audit: all consensus P1s fixed, P2s triaged
