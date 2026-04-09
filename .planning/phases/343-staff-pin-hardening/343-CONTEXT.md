# Phase 343: Staff PIN Change Hardening - Context

**Gathered:** 2026-04-09
**Status:** Ready for planning (scope trimmed 2026-04-09 after v47.0 declaration)
**Trigger incident:** Vishal Chavan PIN `0009` invalid on kiosk (2026-04-09)
**Milestone:** Standalone (precursor to v47.0 Admin Dashboard Venue-Ready Hardening)

## Relationship to v47.0

v47.0 (Admin Dashboard Venue-Ready Hardening) owns all admin dashboard UI work and
starts at Phase 344. Phase 343 was originally scoped with 4 plans including an admin
dashboard UI plan (Plan 03). After v47.0 was declared, Plan 03 was **superseded** —
the admin dashboard UI work belongs in v47.0 Phase 344+, not here.

**Phase 343 active plans (this phase):**
- Plan 01 — Cloud-authority guard: 409 Conflict on venue staff mutations (racecontrol backend)
- Plan 02 — Post-write verify: immediate row re-read + delayed sync verify + alert_incidents (racecontrol backend)
- Plan 04 — e2e-regression staff-pin-lifecycle spec (racecontrol/e2e-regression)

**Phase 343 superseded plan (moved to v47.0):**
- ~~Plan 03 — Admin dashboard /admin/staff page + change-pin orchestration~~ → v47.0 Phase 344+

**Why this split:** Phase 343 is the racecontrol-side backend hardening that any admin
dashboard fix depends on. v47.0 is the admin dashboard hardening layer. Phase 343 MUST
ship before v47.0's PIN-change UI, because that UI depends on the 409 guard, post-write
verify, and the sync/pull-now endpoint that Plan 02 adds.

Phase 343 should ship in the racecontrol monorepo. v47.0 ships in racingpoint-admin
(separate repo).

<domain>
## Phase Boundary

Prevent silent failure of staff PIN changes across the venue↔cloud boundary. Close the three failure modes exposed by the Vishal incident:

1. **Hidden sync authority** — `staff_members` is cloud-authoritative but venue API accepts mutations that get overwritten within 30s by cloud sync. Returns HTTP 200 then silently reverts.
2. **No post-write verification** — HTTP 200 is trusted as "PIN changed," but nothing validates the new PIN round-trips through `validate-pin` after the cloud→venue sync tick.
3. **No admin UI path** — PIN changes happen via curl/sqlite3/deploy-staging scripts, bypassing every safety rail. Staff cannot self-service.

**Incident evidence:**
- Session 2026-04-09 — venue `PUT /staff/staff_463cf400 {pin:0009}` returned 200, validate-pin confirmed working at t+5s, then cloud sync (30s interval) overwrote venue back to old PIN `2003`. Same PUT against cloud API succeeded and propagated to venue within one sync cycle.
- Cloud had separately accumulated 2 legacy bootstrap rows (`staff-uday` PIN 4149, `staff-admin` PIN 2198) that were not in venue — divergence with no detection.
- Venue had an orphan duplicate `staff_e1690f8a` (inactive Chavan Vishal PIN 8772) invisible to create_staff uniqueness check (which only blocks `is_active=1` dups).

**Depends on:** None (new cross-cutting hardening phase)
**Does NOT cover:** Plaintext→hashed PIN migration (separate phase — finding C1), racingpoint-admin auth lockout hardening (finding C4).

</domain>

<decisions>
## Implementation Decisions

### Layer 1a — Cloud authority enforcement (Plan 01)
- **D-01:** Add a registry of cloud-authoritative tables in config: `cloud.authoritative_tables = ["staff_members", "drivers", "wallets", "billing_rates"]` (start conservative, add as needed).
- **D-02:** In `update_staff`, `create_staff`, `delete_staff`, `reset_staff_pin` — check if running as venue client (`config.cloud.enabled && config.cloud.api_url is remote`) AND target table is in authoritative set. If both true, return `409 Conflict` with body `{"error":"staff_members is cloud-authoritative","cloud_url":"<api_url>/staff/{id}","hint":"submit your change to the cloud endpoint; it will sync to venue within 30s"}`.
- **D-03:** Cloud instance (Bono VPS) does NOT reject — it's the authoritative writer. Detection: `config.cloud.api_url == config.server.own_url` OR env var `RC_IS_CLOUD=1`.
- **D-04:** Emergency override env var `RC_ALLOW_VENUE_STAFF_WRITE=1` for break-glass scenarios (cloud down + urgent staff change needed).

### Layer 2 — Post-write verification (Plan 02)
- **D-05:** In `update_staff` and `reset_staff_pin`, after successful UPDATE, immediately re-query the row to confirm PIN matches. If mismatch, return 500 with clear error.
- **D-06:** Spawn a `tokio::spawn` delayed verify: after `sync_interval_secs + 5` seconds, re-query the staff row on THIS server. If the PIN has changed (indicating sync overwrote the write), write an `alert_incidents` row and send WhatsApp alert to staff channel.
- **D-07:** The delayed verify runs on both venue AND cloud; on cloud it catches venue→cloud sync regressions; on venue it catches cloud→venue regressions (though with Layer 1a this should not happen).
- **D-08:** Verify row includes a `correlation_id` (UUID) to trace in logs and match against the original mutation.

### Layer 3 — Admin dashboard PIN-change page (Plan 03)
- **D-09:** New page at `racingpoint-admin/src/app/admin/staff/page.tsx` — list all active staff with id, name, role, last_login_at, "Change PIN" button per row.
- **D-10:** Change PIN modal: new PIN input, confirm PIN input, "Change PIN" button. Client-side validation: 4+ digits numeric, both inputs match.
- **D-11:** New backend endpoint `POST /api/v1/admin/staff/{id}/change-pin` (admin-JWT protected) that:
  - Determines cloud vs venue based on config
  - If venue + staff_members in authoritative set: forwards request to cloud API with preserved JWT
  - If cloud: writes directly to DB
  - Calls new endpoint `POST /api/v1/sync/pull-now {tables:["staff_members"]}` to force immediate cloud→venue sync
  - Runs `validate-pin(new_pin)` on BOTH cloud AND venue
  - Returns `{status:"ok", venue_verified:bool, cloud_verified:bool, latency_ms:N}`
- **D-12:** UI shows staged progress: "Writing cloud... ✓ / Syncing venue... ✓ / Verifying cloud... ✓ / Verifying venue... ✓ / PIN is now active." If ANY step fails, block with specific error.
- **D-13:** Page is admin-role only (NOT staff/cashier). Reuse existing admin auth middleware.
- **D-14:** Do NOT display existing PINs (even redacted) — only the fact that a PIN exists and when it was last changed.

### Layer 7 — E2E regression test (Plan 04)
- **D-15:** New spec `e2e-regression/tests/03-drivers/staff-pin-lifecycle.spec.ts` (or a new category if drivers/ doesn't fit).
- **D-16:** Test flow:
  1. Create test staff `TEST_PIN_LIFECYCLE` with PIN `9999`
  2. Validate PIN `9999` works on venue API → assert 200 + correct staff_id
  3. Validate PIN `9999` works on cloud API → assert 200
  4. Change PIN to `8888` via admin endpoint (`POST /admin/staff/{id}/change-pin`)
  5. Immediately (t+1s) validate PIN `8888` works on venue + cloud → assert both 200
  6. Wait `sync_interval_secs * 2 + 10` seconds (= 70s)
  7. Re-validate PIN `8888` works on venue + cloud → assert both still 200 (catches silent-revert regression)
  8. Validate old PIN `9999` NO LONGER works on venue + cloud → assert both 401
  9. Delete test staff via admin endpoint
  10. Validate PIN `8888` no longer works on venue + cloud → assert both 401
- **D-17:** Test uses dedicated test staff ID with prefix `TEST_PIN_LIFECYCLE_` to avoid colliding with real staff.
- **D-18:** Test is gated behind a flag for CI (takes 70s due to sync wait). Runs on every release branch push + nightly.

### Claude's Discretion
- Exact Rust module layout for the cloud-authority check (helper in `auth/` vs `config.rs` vs new `cloud_authority.rs`)
- Whether to add tracing `ERROR` + WhatsApp alert on every 409 or just log at INFO
- UI framework-level choices in racingpoint-admin (modal library, form state management)
- Whether to backfill the staff_members schema with a `last_pin_change_at TIMESTAMP` column for the audit trail (nice-to-have)

</decisions>

<specifics>
## Specific Ideas

- Vishal incident demonstrates sync authority is invisible. Any fix MUST make authority explicit at the code layer, not a README note.
- The "trust HTTP 200" failure mode is identical to the CGP H3 "proxy metrics as evidence" anti-pattern. Post-write verification IS the H3 fix in code.
- Admin dashboard already has a deploy path (Phase 340). Adding a new protected page is ~1 day of work.
- The E2E test is the single highest-leverage item: one spec catches the exact bug forever.
- Layer 1a alone would have prevented the Vishal incident — it's the minimum viable fix.
- Phase 342 already touched cloud_sync.rs for wallets — authors have fresh context on the sync loop, useful for Layer 1a reviewers.

</specifics>

<canonical_refs>
## Canonical References

### Staff auth routes (PRIMARY — being modified)
- `crates/racecontrol/src/api/routes.rs:12607-12656` — `staff_validate_pin` (plaintext compare — NOT touched in this phase)
- `crates/racecontrol/src/api/routes.rs:12666-12750` — `create_staff` (add authority check)
- `crates/racecontrol/src/api/routes.rs:12750+` — `update_staff` (add authority check + post-write verify)
- `crates/racecontrol/src/api/routes.rs` — `reset_staff_pin` (add authority check + post-write verify)
- `crates/racecontrol/src/api/routes.rs:496-498` — route registration

### Cloud sync
- `crates/racecontrol/src/cloud_sync.rs` — push/pull staff_members table. Need to locate the sync direction for staff_members specifically.
- `crates/racecontrol/src/config.rs:1152+` — config struct, add `authoritative_tables` field

### Admin dashboard
- `C:/Users/bono/racingpoint/racingpoint-admin/src/app/` — Next.js app router
- `C:/Users/bono/racingpoint/racingpoint-admin/src/app/api/auth/login/route.ts` — existing admin auth pattern

### E2E test harness
- `racecontrol/e2e-regression/fixtures/api-client.ts` — API client with env-configurable base URL
- `racecontrol/e2e-regression/tests/03-drivers/` — existing test category (may or may not fit — consider new `10-auth/` category)
- `racecontrol/e2e-regression/fixtures/auth.ts` — existing auth fixture

### Incident evidence
- Session 2026-04-09 (this file's trigger) — documented in the decisions above

</canonical_refs>

<code_context>
## Existing Code Insights

### Current staff_validate_pin (plaintext, routes.rs:12607)
```rust
let result = sqlx::query_as::<_, (String, String, Option<String>)>(
    "SELECT id, name, role FROM staff_members WHERE pin = ? AND is_active = 1",
).bind(&req.pin).fetch_optional(&state.db).await;
```

### Current update_staff (no authority check, no verify)
- PUT /staff/{id} with optional pin field
- UPDATE staff_members SET pin = ? WHERE id = ?
- Returns {id, status:"ok"} on success — no verification
- PIN conflict check only scans `is_active=1` rows → misses inactive duplicates

### Sync interval
- `sync_interval_secs = 30` in `C:/RacingPoint/racecontrol.toml`

### Observed in incident
- Venue PUT returned 200 at t=0
- Venue validate-pin(0009) returned 200 at t=5s
- Venue validate-pin(0009) returned 401 at t=~40s (after sync pulled from cloud)
- Cloud still had old PIN 2003 until we PUT to cloud directly

</code_context>

<deferred>
## Deferred Ideas

- **Plaintext → Argon2 PIN hashing** for staff_members (finding C1) — separate migration phase, depends on this phase shipping first
- **Per-staff-member lockout tracking** (finding C4) — admin portal hardening, separate phase
- **Thrashing detection** for staff_members mutations (Layer 5b from design doc) — depends on structured audit log infrastructure
- **Structured audit log** with source field (venue/cloud/sync/sql) — cross-cutting, own phase
- **Periodic consistency cron** diffing venue vs cloud staff_members — ops phase after this one
- **Bidirectional row-level timestamp sync** (Layer 1c from design doc) — major sync engine rewrite, milestone-level effort

</deferred>

---

*Phase: 343-staff-pin-hardening*
*Context gathered: 2026-04-09*
*[auto] Incident-driven phase from live Vishal Chavan PIN incident and audit findings.*
