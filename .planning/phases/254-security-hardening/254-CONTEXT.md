# Phase 254: Security Hardening - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase, discuss skipped)

<domain>
## Phase Boundary

The system rejects injection attacks, enforces role boundaries, and stores credentials safely. This phase adds INI injection prevention, FFB safety cap, PIN CAS, RBAC, audit log immutability, OTP hashing, WSS, PII masking, and agent mutex.

Requirements: SEC-01 (INI injection whitelist), SEC-02 (FFB GAIN cap), SEC-03 (PIN CAS), SEC-04 (RBAC), SEC-05 (self-top-up block), SEC-06 (audit log append-only), SEC-07 (WSS), SEC-08 (OTP hash), SEC-09 (PII masking), SEC-10 (agent mutex)

Depends on: Phase 252 (RBAC interacts with money-moving endpoints)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase. Key guidance:

- SEC-01: Validate car/track/skin names in launch_args against regex `^[a-zA-Z0-9_-]+$`. Reject anything with newlines, =, [, or other INI-special chars. Do this server-side in the /games/launch handler BEFORE forwarding to agent.
- SEC-02: In /games/launch handler, parse FFB from launch_args JSON. If ffb value maps to GAIN > 100, cap to 100. Log WARN when capping.
- SEC-03: PIN redemption already has CAS from Phase 252 patterns. Apply same: UPDATE auth_tokens SET redeemed_at=NOW() WHERE pin=? AND redeemed_at IS NULL, check rows_affected.
- SEC-04: Add `role TEXT DEFAULT 'cashier'` to staff table. Middleware checks role against endpoint requirements. Roles: cashier, manager, superadmin.
- SEC-05: In /wallet/{id}/topup, compare requesting staff JWT user_id with target driver_id. Block if same unless role=superadmin.
- SEC-06: Create separate audit_log_immutable table with no DELETE trigger. Or use SQLite trigger to prevent DELETE on audit_log.
- SEC-07: WSS requires TLS certs. For LAN (server→pod), self-signed is fine. Configure in racecontrol.toml.
- SEC-08: Hash OTP with bcrypt before storing. Compare with bcrypt_verify on verification.
- SEC-09: Add response transformer that masks phone/email in JSON responses unless role >= manager.
- SEC-10: Add tokio::sync::Mutex in agent's ws_handler around clean_state_reset(). LaunchGame waits for mutex.

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/api/routes.rs` — all API endpoints, middleware
- `crates/racecontrol/src/auth/mod.rs` — JWT auth, PIN validation, OTP handling
- `crates/racecontrol/src/api/middleware.rs` — auth middleware
- `crates/rc-agent/src/ws_handler.rs` — agent WS handler, LaunchGame, clean_state_reset
- `crates/rc-agent/src/ac_launcher.rs` — content ID validation (already has basic regex)
- `crates/racecontrol/src/db/mod.rs` — audit_log table schema

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
