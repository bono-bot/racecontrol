---
phase: 255-legal-compliance
plan: 03
subsystem: database, api
tags: [dpdp-act, data-retention, pii-anonymization, consent-revocation, sqlite, rust, axum]

# Dependency graph
requires:
  - phase: 255-01
    provides: invoices table + GST journal entries (LEGAL-01, LEGAL-02)
  - phase: 255-02
    provides: waiver gate + minor consent (LEGAL-03/04/05) — db/mod.rs and routes.rs were modified in parallel

provides:
  - data_retention_config table (financial_records_years=8, pii_inactive_months=24)
  - drivers retention columns: last_activity_at, pii_anonymized, pii_anonymized_at, consent_revoked, consent_revoked_at
  - POST /customer/revoke-consent — customer-initiated PII anonymization (DPDP Act right of erasure)
  - POST /drivers/{id}/revoke-consent — staff-initiated for guardian requests
  - Daily background job anonymizing inactive drivers (>24 months, 1-hour initial delay, 86400s interval)
  - last_activity_at updated on billing start and wallet topup

affects:
  - 256-game-specific-hardening (uses billing infrastructure)
  - 257-billing-edge-cases (uses billing infrastructure)
  - future-audit (data retention compliance reports)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Daily retention background job with initial delay (same pattern as orphan detector + reconciliation job)
    - Shared anonymization helper (anonymize_driver_pii) called by both customer + staff endpoints
    - Idempotent consent revocation (already-revoked returns ok:true, no error)
    - activity tracking as non-critical post-commit update (never blocks billing start or topup)

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/main.rs

key-decisions:
  - "Data retention config seeded with INSERT OR IGNORE — safe to run on existing DBs with existing config rows"
  - "Driver row retained on anonymization (not deleted) — billing_sessions.driver_id FK must remain valid for 8-year financial retention"
  - "last_activity_at update is non-critical post-commit — failure does not affect billing start or topup result"
  - "Background job uses LIMIT 500 per cycle to bound write pressure on a daily run"
  - "consent_revoked drivers are excluded from background job (already handled at revocation time)"

patterns-established:
  - "Idempotent consent revocation: check consent_revoked flag first, return ok:true if already revoked"
  - "PII anonymization: name='ANONYMIZED-'+substr(id,1,8), all contact fields NULL — driver row preserved"
  - "Financial records never touched in any DPDP compliance operation — only drivers table PII fields"

requirements-completed: [LEGAL-08, LEGAL-09]

# Metrics
duration: 25min
completed: 2026-03-29
---

# Phase 255 Plan 03: Legal Compliance — Data Retention Summary

**DPDP Act 2023 compliance: 8-year financial record retention config, daily PII anonymization job for inactive drivers, and immediate consent revocation endpoints for customers and guardian-proxy requests.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-03-29T00:00:00Z
- **Completed:** 2026-03-29T00:25:00Z
- **Tasks:** 1 (combined LEGAL-08 + LEGAL-09)
- **Files modified:** 3

## Accomplishments

- data_retention_config table created with defaults: financial_records_years=8, pii_inactive_months=24
- Five new driver columns for retention tracking: last_activity_at, pii_anonymized, pii_anonymized_at, consent_revoked, consent_revoked_at — all via idempotent ALTER TABLE
- Index on drivers(last_activity_at) for efficient daily background job queries
- POST /customer/revoke-consent anonymizes driver PII immediately via extract_driver_id (customer JWT)
- POST /drivers/{id}/revoke-consent for staff-initiated guardian requests (cashier+ access)
- spawn_data_retention_job (pub async fn) exported from api::routes, wired in main.rs at startup
- last_activity_at updated post-commit on every billing start and wallet topup — prevents active customers from being anonymized
- Financial records (journal_entries, billing_sessions, invoices, wallet_transactions) never touched

## Task Commits

1. **Task 1: Data retention schema + background job + consent revocation** - `12c1b62f` / `1db260dc` (feat)

The 255-02 agent running in parallel committed the bulk of db/mod.rs + routes.rs changes as part of its commit. The 255-03 main.rs spawn was committed separately.

- `12c1b62f` — 255-02 commit that also included db/mod.rs retention schema and routes.rs handlers (parallel execution)
- `1db260dc` — feat(255-03): wire data retention background job at startup (LEGAL-08)

## Files Created/Modified

- `crates/racecontrol/src/db/mod.rs` — data_retention_config table + INSERT OR IGNORE seed + 5 driver ALTER TABLE columns + idx_drivers_last_activity index
- `crates/racecontrol/src/api/routes.rs` — revoke_consent_handler, staff_revoke_consent_handler, anonymize_driver_pii helper, spawn_data_retention_job, run_pii_anonymization_cycle, last_activity_at update in start_billing and topup_wallet, route registrations
- `crates/racecontrol/src/main.rs` — spawn data retention background task alongside orphan detector + reconciliation job

## Decisions Made

- Driver row is retained on anonymization (not deleted) — billing_sessions.driver_id FK must remain valid to satisfy the 8-year financial record retention requirement. The row exists, just with anonymized PII.
- last_activity_at update is non-critical post-commit. If the UPDATE fails (e.g. DB blip), the billing session already committed successfully — activity tracking is supplementary.
- Background job uses LIMIT 500 per cycle to bound write pressure on a single daily pass.
- consent_revoked drivers are excluded from the background job. They were already anonymized at revocation time — no double work.
- INSERT OR IGNORE on data_retention_config seed — safe for existing deployed DBs.
- anonymize_driver_pii uses COALESCE(pii_anonymized, 0) = 0 guard in UPDATE — idempotent even if called twice.

## Deviations from Plan

### Parallel Execution Overlap

The 255-02 agent ran in parallel and committed all db/mod.rs + routes.rs + main.rs content for this plan (255-03) as part of commit `12c1b62f`. This was correct behavior — 255-02 could see the plan's must_haves artifacts and fulfilled them. The only 255-03-exclusive commit was `1db260dc` (main.rs spawn). No work was duplicated or lost.

**Total deviations from plan specification:** 0 — all planned functionality delivered, cargo check passes.

## Issues Encountered

- Parallel plan execution (255-02) committed the bulk of 255-03 content. This was handled by committing only the remaining piece (main.rs spawn) under 255-03's commit.
- Pre-existing test failure: `config::tests::config_fallback_preserved_when_no_env_vars` fails when run after test-suite-wide env var pollution (pre-existing, not caused by this plan — passes in isolation).

## Known Stubs

None — data retention config is seeded with correct production defaults (8 years financial, 24 months PII). Background job will begin anonymizing on day 1 after any driver's last_activity_at exceeds 24 months.

## Next Phase Readiness

- LEGAL-08 and LEGAL-09 complete. Phase 255 (legal compliance) has all 9 requirements covered.
- Phase 256 (game-specific hardening) can proceed independently.
- Deployment: server rebuild required to activate background job and new endpoints.

---
*Phase: 255-legal-compliance*
*Completed: 2026-03-29*
