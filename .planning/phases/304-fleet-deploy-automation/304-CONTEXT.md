# Phase 304: Fleet Deploy Automation - Context

**Gathered:** 2026-04-02
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Staff can deploy a new binary to the entire fleet in one API call with automatic safety gates. POST /api/v1/fleet/deploy endpoint with canary-first (Pod 8), health verify, auto-rollout in waves, auto-rollback on failure, billing session drain, and deploy status endpoint.

Requirements: DEPLOY-01 through DEPLOY-06

Success Criteria:
1. POST /api/v1/fleet/deploy with binary hash + scope initiates deployment and returns deploy_id immediately
2. Deploy goes to Pod 8 first; next wave does not start until Pod 8 passes health check
3. After canary passes, remaining pods receive binary in waves with configurable delay; full fleet updated without manual action
4. If Pod 8 or any wave pod fails health check, all affected pods automatically revert to previous binary
5. GET /api/v1/fleet/deploy/status shows current wave, per-pod status, rollback event log
6. No pod swaps binary while it has active billing session; swap deferred until session ends

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Extends existing OTA pipeline from v22.0 (ota_pipeline.rs)
- Uses existing fleet exec infrastructure for binary download + swap
- Canary = Pod 8 (standing rule)
- Billing drain uses existing has_active_billing_session() check
- OTA_DEPLOYING sentinel file protocol (standing rule)
- Previous binary preserved for 72h rollback window (standing rule)
- Deploy status stored in AppState (in-memory, not SQLite)

</decisions>

<code_context>
## Existing Code Insights

Codebase context will be gathered during plan-phase research.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
