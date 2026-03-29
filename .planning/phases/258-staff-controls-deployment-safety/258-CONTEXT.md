# Phase 258: Staff Controls & Deployment Safety - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped)

<domain>
## Phase Boundary

Staff cannot abuse discounts or self-service their own accounts; deployments cannot disrupt active billing sessions. Covers staff discount approval, daily override reports, cash reconciliation, shift handoff, OTA session drain, graceful shutdown, deploy window lock, session resume, and WS command idempotency.

Requirements: STAFF-01 (discount approval), STAFF-02 (self-service block — DONE in Phase 254 SEC-05), STAFF-03 (daily report), STAFF-04 (cash reconciliation), STAFF-05 (shift handoff), DEPLOY-01 (OTA drain), DEPLOY-02 (graceful shutdown), DEPLOY-03 (deploy window), DEPLOY-04 (session resume), DEPLOY-05 (WS idempotency)

Depends on: Phase 252 (financial guards), Phase 254 (RBAC)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
Key guidance:
- STAFF-01: Add discount_approval_threshold_paise to config (default 5000 = Rs.50). Discounts above threshold require manager_approval_code in the request body. Validate code against staff table.
- STAFF-02: Already done (SEC-05 in Phase 254). Mark as complete, add test reference.
- STAFF-03: GET /admin/reports/daily-overrides — query billing_sessions + wallet_transactions for staff-initiated discounts, manual refunds, tier overrides. Include actor_id (staff who did it).
- STAFF-04: GET /admin/reports/cash-drawer — sum wallet topups with method='cash' for the day. POST /admin/reports/cash-drawer/close with physical_count input. Compare and log discrepancy.
- STAFF-05: POST /staff/shift-handoff — outgoing staff acknowledges active sessions, logs handoff. Incoming staff sees briefing of active sessions on login.
- DEPLOY-01: In OTA pipeline, check billing.active_timers for the target pod. If non-empty, defer binary swap. Sentinel file: OTA_DEFERRED_<pod_id>.
- DEPLOY-02: On SIGTERM/SIGINT, agent writes shutdown_at to billing_sessions and notifies server. Server calculates partial refund.
- DEPLOY-03: OTA pipeline checks day-of-week + hour. If Saturday/Sunday 18:00-23:00 IST, require --force flag.
- DEPLOY-04: On agent restart, check billing_sessions WHERE status='active' AND pod_id=self. If found, resume session or trigger refund.
- DEPLOY-05: Add command_id UUID to CoreToAgentMessage. Agent maintains seen-set with 5-min TTL. Duplicate command_ids silently acked.

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/api/routes.rs` — billing endpoints, staff routes (RBAC from Phase 254)
- `crates/racecontrol/src/billing.rs` — active_timers, OTA checking
- `crates/rc-agent/src/ws_handler.rs` — CoreToAgentMessage handler
- `crates/rc-common/src/protocol.rs` — WS message types
- `deploy-staging/deploy-server.sh` — existing deploy script
- `deploy-staging/deploy-pod.sh` — existing pod deploy script

</code_context>

<specifics>
## Specific Ideas

STAFF-02 (self-topup block) already implemented in Phase 254 as SEC-05. Just mark complete.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
