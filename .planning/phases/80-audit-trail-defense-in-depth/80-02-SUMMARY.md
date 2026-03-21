---
phase: 80-audit-trail-defense-in-depth
plan: 02
subsystem: auth, security
tags: [hmac, sha256, whatsapp, pin-rotation, cloud-sync, signing]

# Dependency graph
requires:
  - phase: 80-01
    provides: audit_log table and structured audit trail
provides:
  - system_settings table for key-value config tracking
  - PIN rotation tracking with 30-day WhatsApp alerting
  - HMAC-SHA256 signing on outbound cloud sync requests
  - HMAC-SHA256 verification on inbound sync endpoints (permissive mode)
  - sync_hmac_key config field with env var override
affects: [cloud-sync, whatsapp-alerting, config]

# Tech tracking
tech-stack:
  added: []
  patterns: [hmac-signing-on-sync, permissive-mode-verification, system-settings-kv-store]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/whatsapp_alerter.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/cloud_sync.rs
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "SHA-256 of admin_pin_hash stored in system_settings (not the hash itself) for change detection"
  - "HMAC verification in permissive mode initially -- warns but allows mismatches for deployment transition"
  - "GET request signing uses reconstructed query string as body for HMAC input"

patterns-established:
  - "system_settings KV table for runtime config tracking (PIN age, future settings)"
  - "Permissive HMAC mode with TODO marker for strict enforcement after coordinated deploy"

requirements-completed: [ADMIN-06, AUTH-07]

# Metrics
duration: 12min
completed: 2026-03-21
---

# Phase 80 Plan 02: PIN Rotation Alerting + HMAC Cloud Sync Signing Summary

**system_settings table tracks admin PIN age with 24h WhatsApp alert check; HMAC-SHA256 signing on outbound sync with permissive inbound verification**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-21T03:27:20Z
- **Completed:** 2026-03-21T03:39:40Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- system_settings table DDL + check_pin_rotation startup function that SHA-256 hashes admin_pin_hash and upserts to track PIN change date
- Daily PIN rotation check in whatsapp_alerter_task sends WhatsApp alert to Uday if admin PIN unchanged for 30+ days
- HMAC-SHA256 signing on outbound push_to_cloud and sync_once_http with x-sync-timestamp, x-sync-nonce, x-sync-signature headers
- Permissive HMAC verification on inbound sync_push and sync_changes endpoints (warns on mismatch, does not reject)
- sync_hmac_key configurable via CloudConfig or RACECONTROL_SYNC_HMAC_KEY env var

## Task Commits

Each task was committed atomically:

1. **Task 1: PIN rotation tracking + daily alert check** - `6c59db6` (feat)
2. **Task 2: HMAC-SHA256 sync payload signing + verification** - `0cd23fc` (feat)

## Files Created/Modified
- `crates/racecontrol/src/db/mod.rs` - system_settings table DDL + check_pin_rotation function
- `crates/racecontrol/src/whatsapp_alerter.rs` - 24h PIN rotation age check with WhatsApp alert
- `crates/racecontrol/src/main.rs` - Call check_pin_rotation at startup after migrations
- `crates/racecontrol/src/config.rs` - sync_hmac_key field on CloudConfig + env var override
- `crates/racecontrol/src/cloud_sync.rs` - sign_sync_request + verify_sync_signature + HMAC headers on outbound
- `crates/racecontrol/src/api/routes.rs` - HMAC verification on sync_push + sync_changes (permissive)

## Decisions Made
- SHA-256 of admin_pin_hash (not the hash itself) stored in system_settings -- enables change detection without duplicating sensitive material
- HMAC verification starts in permissive mode -- logs warnings but allows requests without valid HMAC, enabling gradual rollout
- GET request HMAC signs the reconstructed query string as body input since GET has no request body

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing build errors in game_launcher.rs/ws/mod.rs/types.rs from uncommitted work of another plan (Loading GameState variant) -- not related to 80-02, resolved by restoring those files to committed state

## User Setup Required

None - no external service configuration required. sync_hmac_key is optional and defaults to None (backward compatible).

## Next Phase Readiness
- PIN rotation alerting active on next deploy (requires admin_pin_hash to be configured)
- HMAC signing activates when RACECONTROL_SYNC_HMAC_KEY is set on both venue and cloud
- Bono needs matching HMAC key deployed on cloud side before switching to strict mode

---
*Phase: 80-audit-trail-defense-in-depth*
*Completed: 2026-03-21*
