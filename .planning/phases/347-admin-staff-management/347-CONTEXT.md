# Phase 347: Admin Staff Management - Context

**Gathered:** 2026-04-10
**Status:** Ready for planning

<domain>
## Phase Boundary

`/admin/staff` page + `change_staff_pin_safe` endpoint + `sync/pull-now` endpoint. Make safe PIN changes the easy path. Replaces curl/sqlite3/deploy-staging scripts with a proper admin UI.

**Two repos involved:**
- `racecontrol` — new Rust endpoints: `POST /api/v1/admin/staff/{id}/change-pin` and `POST /api/v1/sync/pull-now`
- `racingpoint-admin` — enhance existing `/staff/manage/page.tsx` with safe PIN change modal + staged progress

**Hard dependency:** Phase 343 Plans 01+02 (cloud-authority 409 guard + post-write verify) MUST be deployed before this phase ships. Phase 343 is code-complete but NOT yet live-deployed.

</domain>

<decisions>
## Implementation Decisions

### PIN Change UX Flow
- **D-01:** Replace the existing inline PIN edit in `/staff/manage/page.tsx` with a dedicated "Change PIN" button per staff row that opens a modal
- **D-02:** Modal shows: new PIN input, confirm PIN input, Change PIN button. Client-side validation: 4+ digits numeric, both inputs match (STAFF-04)
- **D-03:** Staged progress stepper inside the modal during operation: "Writing cloud... / Syncing venue... / Verifying cloud... / Verifying venue..." with checkmarks as each step completes (STAFF-08)
- **D-04:** Existing PINs are NEVER displayed anywhere — not even redacted. Only metadata (name, role, last_login_at) shown in the list (STAFF-03)
- **D-05:** The existing `staffApi.update()` path for name/phone/role edits remains unchanged. Only PIN changes route through the new safe endpoint

### Endpoint Architecture
- **D-06:** New handler `change_staff_pin_safe` in `crates/racecontrol/src/api/routes.rs` — follows existing flat module pattern
- **D-07:** Route: `POST /api/v1/admin/staff/{id}/change-pin` — admin-JWT protected (superadmin + manager only, per STAFF-02)
- **D-08:** Orchestration sequence:
  1. Determine cloud vs venue from config
  2. If venue: forward PIN change to cloud API with preserved JWT
  3. If cloud: write directly to DB
  4. Call `POST /api/v1/sync/pull-now {tables:["staff_members"]}` to trigger immediate cloud->venue sync
  5. Run `validate-pin(new_pin)` on BOTH cloud AND venue
  6. Return `{status:"ok", cloud_verified:bool, venue_verified:bool, latency_ms:u64, correlation_id:String}`
- **D-09:** New endpoint `POST /api/v1/sync/pull-now` — triggers immediate cloud->venue pull for specified tables, bypassing the 30s interval. Admin-JWT protected.
- **D-10:** Admin Next.js proxy route `src/app/api/rc/admin/staff/[id]/change-pin/route.ts` forwards to racecontrol with JWT

### Error Recovery
- **D-11:** On partial success (cloud OK but venue sync failed), show error banner with specific failure message: "PIN changed on cloud but venue sync failed - contact James" (STAFF-09)
- **D-12:** No auto-retry — explicit error with contact info is safer for staff operations
- **D-13:** Change PIN button disabled during operation to prevent double-submission
- **D-14:** `correlation_id` included in all responses for debugging (ties to Phase 343 D-08)

### Feature Flag
- **D-15:** `FEATURE_STAFF_PIN_UI` env var defaults to `off`. The existing manage page works as-is without the flag.
- **D-16:** When flag is `off`: hide the safe "Change PIN" button, fall back to existing `staffApi.update({pin})` path
- **D-17:** Pre-deploy gate script checks Phase 343 Plans 01+02 are shipped in racecontrol git log before enabling (DEP-04)
- **D-18:** Feature flag checked client-side via `NEXT_PUBLIC_FEATURE_STAFF_PIN_UI` env var

### Claude's Discretion
- Exact Rust module placement for `change_staff_pin_safe` (inline in routes.rs vs extracted helper)
- HTTP client approach for venue->cloud forwarding (reqwest vs hyper)
- Admin proxy route structure in Next.js (catch-all vs dedicated route)
- Whether to add `last_pin_change_at` column to `staff_members` table for audit trail
- Loading skeleton pattern for the staff list (reuse SkeletonTable from Phase 354)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase 343 (prerequisite)
- `.planning/phases/343-staff-pin-hardening/343-CONTEXT.md` -- Cloud-authority guard decisions D-01..D-08, admin UI decisions D-09..D-14
- `crates/racecontrol/src/api/routes.rs` -- Existing staff CRUD handlers with `cloud_authority_guard`, `staff_validate_pin`
- `crates/racecontrol/src/cloud_sync.rs` -- Cloud sync logic, sync_interval_secs, staff_members upsert

### Admin app (existing code)
- `racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx` -- Existing staff manage page with CRUD
- `racingpoint-admin/src/lib/api/staff.ts` -- staffApi: list, create, update, deactivate
- `racingpoint-admin/src/lib/api/base.ts` -- rcFetch helper with circuit breaker
- `racingpoint-admin/src/components/AdminLayout.tsx:84` -- Sidebar nav entry "Staff & PINs"
- `racingpoint-admin/src/components/ConfirmDialog.tsx` -- Reusable confirm dialog component

### Requirements
- `.planning/REQUIREMENTS.md` -- STAFF-01..10 (Theme 11), DEP-01..04 (Theme 12)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **`/staff/manage/page.tsx`**: Full staff CRUD page already exists — list, create, edit inline, deactivate with confirm dialog. Uses `staffApi`, `toast` (sonner), `ConfirmDialog`. This page needs enhancement, NOT replacement.
- **`staffApi` (staff.ts)**: List, create, update, deactivate wrappers around `rcFetch`. Needs new `changePin(id, newPin)` method calling the safe endpoint.
- **`ConfirmDialog`**: Reusable modal component — can be adapted for PIN change modal or a new modal built alongside it.
- **`SkeletonTable`**: Loading skeleton component added in Phase 354 — reuse for staff list loading state.
- **`toast` (sonner)**: Already wired for success/error notifications throughout the admin app.
- **`rcFetch`**: Base API helper with circuit breaker + retry logic.

### Established Patterns
- **Admin API proxy**: All racecontrol calls go through Next.js API routes at `src/app/api/rc/[...path]/route.ts` — catch-all proxy pattern
- **Role colors**: `ROLE_COLORS` map in manage page — consistent role badge styling
- **Form state**: Local `useState` forms with validation — no form library (react-hook-form, formik)
- **Cloud authority**: `cloud_authority_guard()` in routes.rs blocks venue writes to cloud-authoritative tables with 409

### Integration Points
- **Route registration**: New endpoints registered in `routes.rs` router builder, admin-JWT protected section
- **Sidebar nav**: `AdminLayout.tsx` — already has `/staff/manage` entry, no new nav needed
- **Sync trigger**: `cloud_sync.rs` — need to expose a way to trigger immediate pull (currently on 30s interval)

</code_context>

<specifics>
## Specific Ideas

- The existing `/staff/manage` page is functional but uses raw `staffApi.update({pin})` which hits the venue API directly — this is the unsafe path that Phase 343 now blocks with 409. The new `change_staff_pin_safe` endpoint provides the safe orchestrated path.
- Uday is the primary user of PIN changes. The UI must be simpler than curl commands — one modal, two PIN fields, one button, staged progress.
- PIN change should feel instant but show each verification step for transparency. The 5-second success criterion (STAFF-01) includes the full cloud write + sync + double verify cycle.

</specifics>

<deferred>
## Deferred Ideas

None -- discussion stayed within phase scope

</deferred>

---

*Phase: 347-admin-staff-management*
*Context gathered: 2026-04-10*
