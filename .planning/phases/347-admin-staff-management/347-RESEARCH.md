# Phase 347: Admin Staff Management - Research

**Researched:** 2026-04-10
**Domain:** Rust/Axum endpoint authoring + Next.js admin frontend modal + cloud sync bypass
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**PIN Change UX Flow**
- D-01: Replace the existing inline PIN edit in `/staff/manage/page.tsx` with a dedicated "Change PIN" button per staff row that opens a modal
- D-02: Modal shows: new PIN input, confirm PIN input, Change PIN button. Client-side validation: 4+ digits numeric, both inputs match (STAFF-04)
- D-03: Staged progress stepper inside the modal during operation: "Writing cloud... / Syncing venue... / Verifying cloud... / Verifying venue..." with checkmarks as each step completes (STAFF-08)
- D-04: Existing PINs are NEVER displayed anywhere — not even redacted. Only metadata (name, role, last_login_at) shown in the list (STAFF-03)
- D-05: The existing `staffApi.update()` path for name/phone/role edits remains unchanged. Only PIN changes route through the new safe endpoint

**Endpoint Architecture**
- D-06: New handler `change_staff_pin_safe` in `crates/racecontrol/src/api/routes.rs` — follows existing flat module pattern
- D-07: Route: `POST /api/v1/admin/staff/{id}/change-pin` — admin-JWT protected (superadmin + manager only, per STAFF-02)
- D-08: Orchestration sequence: (1) Determine cloud vs venue from config (2) If venue: forward PIN change to cloud API with preserved JWT (3) If cloud: write directly to DB (4) Call `POST /api/v1/sync/pull-now {tables:["staff_members"]}` to trigger immediate cloud->venue sync (5) Run `validate-pin(new_pin)` on BOTH cloud AND venue (6) Return `{status:"ok", cloud_verified:bool, venue_verified:bool, latency_ms:u64, correlation_id:String}`
- D-09: New endpoint `POST /api/v1/sync/pull-now` — triggers immediate cloud->venue pull for specified tables, bypassing the 30s interval. Admin-JWT protected.
- D-10: Admin Next.js proxy route `src/app/api/rc/admin/staff/[id]/change-pin/route.ts` forwards to racecontrol with JWT

**Error Recovery**
- D-11: On partial success (cloud OK but venue sync failed), show error banner with specific failure message: "PIN changed on cloud but venue sync failed - contact James" (STAFF-09)
- D-12: No auto-retry — explicit error with contact info is safer for staff operations
- D-13: Change PIN button disabled during operation to prevent double-submission
- D-14: `correlation_id` included in all responses for debugging (ties to Phase 343 D-08)

**Feature Flag**
- D-15: `FEATURE_STAFF_PIN_UI` env var defaults to `off`. The existing manage page works as-is without the flag.
- D-16: When flag is `off`: hide the safe "Change PIN" button, fall back to existing `staffApi.update({pin})` path
- D-17: Pre-deploy gate script checks Phase 343 Plans 01+02 are shipped in racecontrol git log before enabling (DEP-04)
- D-18: Feature flag checked client-side via `NEXT_PUBLIC_FEATURE_STAFF_PIN_UI` env var

### Claude's Discretion
- Exact Rust module placement for `change_staff_pin_safe` (inline in routes.rs vs extracted helper)
- HTTP client approach for venue->cloud forwarding (reqwest vs hyper)
- Admin proxy route structure in Next.js (catch-all vs dedicated route)
- Whether to add `last_pin_change_at` column to `staff_members` table for audit trail
- Loading skeleton pattern for the staff list (reuse SkeletonTable from Phase 354)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| STAFF-01 | `/admin/staff` page renders list of active staff (name, role, last_login_at) with per-row Change PIN button | Existing manage page at `(dashboard)/staff/manage/page.tsx` already lists staff; needs PIN column removed + Change PIN button added |
| STAFF-02 | Page is role-gated superadmin + manager only | `require_role_manager` + `require_role_superadmin` middleware exist in routes.rs; use role-gated sub-router pattern |
| STAFF-03 | Existing PINs are never displayed (not even redacted) — only metadata | Currently `togglePin` shows `s.pin` from API; must remove `pin` field from view-mode row and from `StaffMember` type in Phase 347 display |
| STAFF-04 | Change PIN modal validates: 4+ digit numeric, both inputs match | Client-side React state validation before submit; no form library needed (existing pattern) |
| STAFF-05 | New racecontrol endpoint `POST /api/v1/admin/staff/{id}/change-pin` orchestrates cloud write -> immediate verify -> venue sync -> venue verify | New `change_staff_pin_safe` handler; builds on `cloud_authority_guard` + `post_write_verify_staff_pin` + `sync_once_http` already in routes.rs/cloud_sync.rs |
| STAFF-06 | `change_staff_pin_safe` response includes `cloud_verified: bool`, `venue_verified: bool`, `latency_ms`, `correlation_id` | Straightforward Rust struct; `latency_ms` = `Instant::now().elapsed().as_millis()` |
| STAFF-07 | New racecontrol endpoint `POST /api/v1/sync/pull-now {tables:[...]}` triggers immediate cloud->venue pull, bypassing 30s interval | New `sync_pull_now_handler`; invokes `sync_once_http` or relay-path pull for specified tables; admin-JWT protected |
| STAFF-08 | Admin UI shows staged progress: "Writing cloud... / Syncing venue... / Verifying cloud... / Verifying venue..." | Server-Sent Events OR polling OR single response after all steps; decision below |
| STAFF-09 | Error banner on partial success ("PIN changed on cloud but venue sync failed — contact James") | Frontend `partialSuccess` state flag; render error banner below progress stepper |
| STAFF-10 | Feature-flag `FEATURE_STAFF_PIN_UI=off` by default; deploy gate checks Phase 343 Plans 01+02 shipped | `NEXT_PUBLIC_FEATURE_STAFF_PIN_UI` env var; pre-deploy shell script greps git log |
| DEP-01 | Phase 343 Plans 01+02+04 executed and deployed to venue + cloud racecontrol before Phase 347 ships | Verified: 343 Plans 01+02 are code-complete (commits `b31c38e0`, `6c870f99`) but NOT live-deployed. Phase 347 MUST NOT ship until 343 is deployed. |
| DEP-02 | Venue .23 Node version downgraded to 22 LTS (or deploy script forces explicit Node 22 path) | Existing blocker in STATE.md — separate from Phase 347 code but gating admin deploy |
| DEP-03 | Racecontrol `/api/v1/admin/staff/{id}/change-pin` endpoint returns something other than 404 before Phase 347 deploys | Phase 347-01 adds this endpoint; satisfied by 347 deploy itself |
| DEP-04 | Pre-deploy script greps git log for Phase 343 merge commits and hard-fails Phase 347 deploy if missing | Shell script in `scripts/deploy/` or inline in deploy-admin.sh; grep for `343-01` + `343-02` in git log |
</phase_requirements>

---

## Summary

Phase 347 adds the safe PIN change UI layer on top of the Phase 343 backend hardening. The backend (racecontrol) needs two new handlers: `change_staff_pin_safe` (orchestration: cloud write + sync trigger + dual verify) and `sync_pull_now` (immediate table pull bypass). The frontend (racingpoint-admin) needs the existing `/staff/manage/page.tsx` enhanced with: PIN column stripped, Change PIN modal added, feature-flag guard, and staged progress UI.

Phase 343's `cloud_authority_guard`, `post_write_verify_staff_pin`, and `spawn_delayed_sync_verify` are all already implemented (commits `b31c38e0`, `6c870f99`). The cloud_sync module has `sync_once_http` which can be called directly for the immediate pull. The admin proxy catch-all at `src/app/api/rc/[...path]/route.ts` already forwards all methods with JWT, so the new endpoint is automatically proxied — no new Next.js API route is needed unless a dedicated route with custom logic is preferred.

**Critical dependency:** Phase 343 Plans 01+02 must be live-deployed to both venue server and cloud (Bono VPS) before Phase 347 ships. They are code-complete but not yet deployed (STATE.md confirms this).

**Primary recommendation:** Inline `change_staff_pin_safe` into routes.rs (consistent with the existing flat handler pattern: `reset_staff_pin`, `create_staff`, `update_staff` are all inline). Use reqwest (already used for cloud forwarding in `cameras_health_proxy`). Use the existing catch-all proxy — no dedicated Next.js API route needed. Use single-response pattern (not SSE) for the staged progress: endpoint blocks until all steps complete (~2-3s) and returns the final state; frontend polls a local "step" indicator driven by a simulated timer while awaiting the single response.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | (workspace) | New route handler `change_staff_pin_safe` + `sync_pull_now_handler` | Already used everywhere in routes.rs |
| reqwest | (workspace) | HTTP client for venue->cloud forwarding and cross-validate venue PIN | Already used in `cameras_health_proxy` and cloud_sync.rs `http_client` |
| tokio | (workspace) | Async runtime, `Instant` for `latency_ms` | Already used for all async handlers |
| serde / serde_json | (workspace) | Request/response deserialization | Standard throughout routes.rs |
| uuid | (workspace) | `correlation_id` generation | Already used in `reset_staff_pin` and `spawn_delayed_sync_verify` |
| React (useState, useEffect) | Next.js app | Modal state, progress stepper, feature flag check | Existing pattern in manage page |
| sonner (toast) | (racingpoint-admin) | Success/error notifications | Already imported in manage/page.tsx |
| SkeletonTable | local component | Loading state for staff list | Added in Phase 354-02, at `src/components/Skeleton.tsx` |
| ConfirmDialog | local component | Already used for Deactivate confirm — can be reference for new modal pattern | At `src/components/ConfirmDialog.tsx` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| chrono | (workspace) | `last_pin_change_at` timestamp if audit trail column added | Only if Phase 347 adds the column |
| sqlx | (workspace) | `last_pin_change_at` ALTER TABLE migration | Only if audit trail column added |

**Installation:** No new dependencies — all libraries already in workspace Cargo.toml and package.json.

---

## Architecture Patterns

### Recommended Project Structure

For racecontrol (Rust):
```
crates/racecontrol/src/api/routes.rs
  + fn change_staff_pin_safe()          -- new handler, inline after reset_staff_pin (~line 13325)
  + fn sync_pull_now_handler()          -- new handler, inline in service_routes() OR staff_routes()
  + struct ChangePinRequest             -- {new_pin: String}
  + struct ChangePinResponse            -- {status, cloud_verified, venue_verified, latency_ms, correlation_id}
  + struct SyncPullNowRequest           -- {tables: Vec<String>}
  + struct SyncPullNowResponse          -- {status, tables_synced, latency_ms}
```

For racingpoint-admin (Next.js):
```
src/app/(dashboard)/staff/manage/page.tsx   -- existing file, enhanced with modal
src/lib/api/staff.ts                         -- add changePin(id, newPin) method
```

### Pattern 1: Flat Handler (Rust)

**What:** New handlers are added inline in routes.rs following the exact same pattern as `reset_staff_pin`. No new module, no new file.

**When to use:** When the handler is self-contained and under ~100 lines. `change_staff_pin_safe` fits this: 3 external HTTP calls + 2 DB reads + response struct.

**Example pattern from existing `reset_staff_pin` (routes.rs ~13325):**
```rust
// Source: crates/racecontrol/src/api/routes.rs line 13325
async fn reset_staff_pin(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ResetPinRequest>,
) -> impl IntoResponse {
    if let Some(rejection) = cloud_authority_guard(&state, "staff_members") {
        return rejection.into_response();
    }
    let correlation_id = uuid::Uuid::new_v4().to_string();
    // ... DB update, post_write_verify_staff_pin, spawn_delayed_sync_verify ...
    Json(json!({...})).into_response()
}
```

`change_staff_pin_safe` follows this same signature pattern.

### Pattern 2: Role-Gated Route Registration (Rust)

**What:** `POST /api/v1/admin/staff/{id}/change-pin` must be superadmin + manager only (STAFF-02). The existing route map has a `require_role_manager` sub-router (line ~628 of routes.rs).

**When to use:** Any endpoint that must be restricted to manager+ roles.

**Example (routes.rs line ~654):**
```rust
// Source: crates/racecontrol/src/api/routes.rs line 628-654
Router::new()
    .route("/billing/rates", post(create_billing_rate))
    // ... other manager+ routes ...
    .layer(axum::middleware::from_fn(require_role_manager))
```

Register `change_staff_pin_safe` inside this block so it inherits `require_role_manager`. `sync_pull_now_handler` can go in the superadmin-only block (line ~657) since it's a system operation.

### Pattern 3: Next.js Catch-All Proxy (Already Covers New Endpoints)

**What:** `src/app/api/rc/[...path]/route.ts` already proxies ALL methods to racecontrol with JWT forwarding. New racecontrol endpoint `POST /api/v1/admin/staff/{id}/change-pin` is automatically available as `POST /api/rc/admin/staff/{id}/change-pin` from the Next.js side.

**When to use:** All racecontrol calls from the admin frontend use this pattern via `rcFetch('/admin/staff/${id}/change-pin', { method: 'POST', body: ... })`.

**Decision (Claude's Discretion):** No dedicated Next.js API route needed for this phase. D-10 mentions creating one, but the catch-all already handles it identically. Create a dedicated route ONLY if custom logic (e.g., SSE streaming) is needed, which the current design does not require.

### Pattern 4: Modal with Staged Progress (React)

**What:** The progress stepper (D-03) needs to show step states during the single blocking HTTP call. Best approach: set `step` state to advance through labels while the single `rcFetch` call awaits, using a simulated timer.

**Recommended approach:**
```typescript
// Source: design decision — single response, simulated step progress
const STEPS = ['Writing cloud...', 'Syncing venue...', 'Verifying cloud...', 'Verifying venue...'];
const [step, setStep] = useState(0);
const [submitting, setSubmitting] = useState(false);

const handleChangePin = async () => {
  setSubmitting(true);
  // Advance steps visually while awaiting (approximate timing)
  const timer1 = setTimeout(() => setStep(1), 600);
  const timer2 = setTimeout(() => setStep(2), 1400);
  const timer3 = setTimeout(() => setStep(3), 2200);
  try {
    const res = await rcFetch(`/admin/staff/${staffId}/change-pin`, {
      method: 'POST',
      body: JSON.stringify({ new_pin: newPin }),
    }) as ChangePinResponse;
    clearTimeout(timer1); clearTimeout(timer2); clearTimeout(timer3);
    setStep(4); // all done
    if (!res.venue_verified) {
      // partial success — show error banner
    }
    toast.success('PIN changed successfully');
  } catch (e) {
    // show error
  }
  setSubmitting(false);
};
```

This avoids SSE complexity. The server-side call takes ~2-3 seconds (network round trips to cloud + validate), which is enough time for the visual steps to feel meaningful.

### Pattern 5: Feature Flag Guard

**What:** `NEXT_PUBLIC_FEATURE_STAFF_PIN_UI` is read at component render time. When off, the "Change PIN" button is hidden and the existing inline edit (which includes `pin` field) continues to work.

**When to use:** Per D-15/D-16, feature flag is `off` by default. Enable it only after Phase 343 is deployed.

```typescript
// Source: design decision per D-18
const featureEnabled = process.env.NEXT_PUBLIC_FEATURE_STAFF_PIN_UI === 'on';
// In JSX: {featureEnabled ? <ChangePinButton /> : null}
```

### Anti-Patterns to Avoid

- **Displaying PIN in view mode:** The current manage page has a `togglePin` button that shows `s.pin` from `StaffMember`. Per D-04/STAFF-03, this must be removed entirely in Phase 347. The `StaffMember` type should omit `pin` from the response or the component should simply not render it. The `pin` field is in `UpdateStaffParams` for create (needed) but should NOT be read back for display.
- **Calling `staffApi.update({pin})` for PIN changes when flag is on:** The cloud authority guard (Phase 343) will return 409 on this path if `staff_members` is in the authoritative list. Route all PIN changes through `changePin()` when the feature flag is enabled.
- **Duplicate route registration:** The new `POST /api/v1/admin/staff/{id}/change-pin` must NOT conflict with existing `PUT /api/v1/staff/{id}`. The path differs (admin/ prefix, /change-pin suffix) so no conflict. Verify with the existing uniqueness test: `route_uniqueness_tests::no_duplicate_route_registrations`.
- **Holding lock across `.await` in cloud forwarding:** When forwarding the PIN change to cloud, clone the config values before the await point. Do not hold any `AppState` read lock across the reqwest `.send().await`.
- **Calling `sync_once_http` directly:** `sync_once_http` is `async fn` in cloud_sync.rs but currently `pub(crate)` or private. Check visibility before calling; may need `pub(crate)` annotation added. Alternatively, expose a `pub(crate) async fn trigger_pull_for_tables(state, tables)` wrapper.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cloud-authoritative check | Custom is-cloud logic | `cloud_authority_guard(&state, "staff_members")` (routes.rs:12973) | Already handles env override, config flag, cloud self-detection |
| Post-write PIN verification | Re-read in handler manually | `post_write_verify_staff_pin(&state.db, &id, &pin).await` (routes.rs:12999) | Already handles row-missing case, returns structured Err |
| Delayed sync regression check | Custom tokio::spawn + sleep | `spawn_delayed_sync_verify(db, sync_interval_secs, id, pin, corr_id)` (routes.rs:13026) | Already wired to alert_incidents table |
| JWT forwarding in proxy | Manual fetch with auth header | `rcFetch(path, options)` from admin — catch-all proxy already injects JWT cookie | Proxy at `src/app/api/rc/[...path]/route.ts` handles all methods |
| Unique correlation ID | UUID v4 custom impl | `uuid::Uuid::new_v4().to_string()` (already imported in workspace) | Consistent with rest of codebase |
| Loading skeleton | Custom pulse animation | `SkeletonTable` from `src/components/Skeleton.tsx` | Phase 354-02 added it specifically for this pattern |
| Confirm/cancel modal shell | New modal overlay component | `ConfirmDialog` base structure — or new inline modal following same CSS pattern | Dialog pattern already established; avoid new modal libraries |

---

## Integration Points

### Racecontrol Route Registration

The new endpoint `POST /api/v1/admin/staff/{id}/change-pin` must be registered inside the **manager+** sub-router block (routes.rs ~line 628-654). This ensures superadmin AND manager can access it (the block uses `require_role_manager` which permits manager+ roles, not only manager).

`POST /api/v1/sync/pull-now` is a system operation — register it in the **superadmin-only** sub-router block (~line 657-679) OR in `service_routes()` with admin-JWT check in-handler. The cleaner choice is staff_routes manager+ block since it requires staff login context and should not be reachable without JWT.

**Route uniqueness verification command:**
```bash
grep -n '\.route("/' crates/racecontrol/src/api/routes.rs | sed 's/.*\.route("//' | sed 's/".*//' | sort | uniq -d
```
Must return empty after adding new routes.

### cloud_sync.rs — Exposing a Pull Function

`sync_once_http` at line 965 is `async fn sync_once_http(state: &Arc<AppState>, cloud_url: &str)` and is currently used only inside the `spawn()` background task. To call it from `sync_pull_now_handler`, either:

**Option A (recommended):** Add `pub(crate) async fn pull_tables_now(state: &Arc<AppState>, tables: &[&str]) -> anyhow::Result<()>` that calls a filtered version of the HTTP pull (only requested tables, not SYNC_TABLES).

**Option B:** Make `sync_once_http` `pub(crate)` and call it directly. This pulls ALL tables (not filtered), which is acceptable for the immediate use case (pulling `staff_members` will also pull other authoritative tables but that's not harmful).

Option A is cleaner but Option B is simpler and sufficient for Phase 347.

### staffApi.ts — New Method

Add to `src/lib/api/staff.ts`:
```typescript
export interface ChangePinParams { new_pin: string; }
export interface ChangePinResponse {
  status: string;
  cloud_verified: boolean;
  venue_verified: boolean;
  latency_ms: number;
  correlation_id: string;
}

// In staffApi object:
changePin: (id: string, data: ChangePinParams) =>
  rcFetch(`/admin/staff/${id}/change-pin`, {
    method: 'POST',
    body: JSON.stringify(data),
  }) as Promise<ChangePinResponse>,
```

### manage/page.tsx — View Mode Changes

1. Remove the `visiblePins` state and `togglePin` function entirely
2. Remove the PIN column from the table header and view-mode cells (STAFF-03)
3. Add "Change PIN" button per row in view-mode Actions column (gated behind `featureEnabled`)
4. Add `changePinTarget: StaffMember | null` state to control modal visibility
5. Remove `pin` from `editForm` state fields (PIN edits go through modal, not inline edit)
6. The `startEdit` function currently sets `editForm.pin = s.pin` — remove that field

---

## Common Pitfalls

### Pitfall 1: Phase 343 Not Deployed Before Phase 347 Ships

**What goes wrong:** `change_staff_pin_safe` calls `cloud_authority_guard` and `post_write_verify_staff_pin` which are in Phase 343. If 343 is not deployed, the venue server returns 404 on the new endpoint.

**Why it happens:** 343 is code-complete but NOT live-deployed. It's easy to test locally (where the binary is fresh) and forget the server still runs the old binary.

**How to avoid:** DEP-04 pre-deploy gate script. Do not mark Phase 347 shipped until `curl http://192.168.31.23:8080/api/v1/admin/staff/dummy/change-pin` returns something other than 404 (will return 401 without JWT, which is correct).

**Warning signs:** `404 Not Found` on the new endpoint after deploying admin changes.

### Pitfall 2: `pin` Still Leaking Through StaffMember Type

**What goes wrong:** `StaffMember` in `staff.ts` has `pin: string`. If Phase 347 only hides the UI column but doesn't touch the type, a future developer might add it back. Also, the current `startEdit()` copies `s.pin` into edit state.

**Why it happens:** The existing `staffApi.list()` returns the PIN field from racecontrol. Phase 347 doesn't change the racecontrol list endpoint, so the PIN is still in the network response.

**How to avoid:** Either (a) mark `pin` as `pin?: string` (optional) in the type and never use it in the view-mode JSX, or (b) add a TODO comment. The ideal long-term fix (separate phase) is to exclude PIN from the list endpoint response. For Phase 347: ensure no JSX reads `s.pin` outside of the old inline-edit path, which is being replaced.

**Warning signs:** PIN visible in view mode; `s.pin` used outside of create/update calls.

### Pitfall 3: Double Route Registration

**What goes wrong:** Axum panics at startup with duplicate route. The existing `PUT /staff/{id}` and new `POST /admin/staff/{id}/change-pin` have different paths so no conflict there. But if a copy-paste adds the new route to BOTH `staff_routes()` and the manager sub-router, Axum panics.

**Why it happens:** The manager sub-router is `.merge()`d inside `staff_routes()`. Adding a route outside the merge AND inside it = duplicate.

**How to avoid:** Add ONLY inside the manager `.merge()` block. Run `cargo test -p racecontrol-crate route_uniqueness` before committing.

### Pitfall 4: Venue-Side Forwarding Fails Silently

**What goes wrong:** When running on venue, `change_staff_pin_safe` must forward the PIN change to the cloud API. If the cloud URL is wrong or the JWT doesn't have the right role on cloud, the forwarding returns a non-200 but the handler might ignore it.

**Why it happens:** reqwest `.send().await` returns Ok for non-200 responses unless `.error_for_status()` is called.

**How to avoid:** Always call `.error_for_status()` after `.send()` in the forwarding logic. Return 502 with the cloud's error body if forwarding fails, so the UI shows a clear failure rather than a false success.

### Pitfall 5: Staged Progress Desync

**What goes wrong:** The frontend timer-based step advancement (setTimeout) finishes all 4 steps but the actual HTTP call hasn't returned yet — or the call returns error but the UI already shows all green checkmarks.

**Why it happens:** The simulated timers are independent of the actual response.

**How to avoid:** The response callback must:
1. Clear all pending timers on resolve OR reject
2. Only show "all green" after the response returns with `cloud_verified: true`
3. Show error state if response has `venue_verified: false` regardless of visual steps

### Pitfall 6: `sync_pull_now` Pulling All Tables on Venue

**What goes wrong:** If `sync_pull_now` calls `sync_once_http` without filtering, it pulls ALL SYNC_TABLES (14 tables including drivers, wallets, pricing_tiers, etc.) and may overwrite recent venue-authoritative data.

**Why it happens:** `SYNC_TABLES` const includes all 14 tables, not just `staff_members`.

**How to avoid:** The `sync_pull_now` handler must filter to only the requested tables. If requesting `["staff_members"]`, only pull staff_members from cloud. Implement Option A (filtered pull function) from the Integration Points section above. This is safer and prevents unintended cross-table sync side effects.

---

## Code Examples

### Handler Skeleton — change_staff_pin_safe

```rust
// Source: design based on reset_staff_pin pattern (routes.rs:13325) + cloud_authority_guard pattern
#[derive(Debug, Deserialize)]
struct ChangePinRequest {
    new_pin: String,
}

#[derive(Debug, serde::Serialize)]
struct ChangePinResponse {
    status: String,
    cloud_verified: bool,
    venue_verified: bool,
    latency_ms: u64,
    correlation_id: String,
}

async fn change_staff_pin_safe(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ChangePinRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let correlation_id = uuid::Uuid::new_v4().to_string();

    // Validate: 4+ digits numeric
    if req.new_pin.len() < 4 || !req.new_pin.chars().all(|c| c.is_ascii_digit()) {
        return (StatusCode::BAD_REQUEST, Json(json!({
            "error": "PIN must be 4+ numeric digits",
            "correlation_id": correlation_id
        }))).into_response();
    }

    let is_venue = !crate::config::this_instance_is_cloud(&state.config)
        && state.config.cloud.is_cloud_authoritative_for("staff_members");

    let cloud_verified;
    let venue_verified;

    if is_venue {
        // Forward to cloud
        let cloud_url = match &state.config.cloud.api_url {
            Some(u) => u.clone(),
            None => return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({
                "error": "cloud_url not configured on venue",
                "correlation_id": correlation_id
            }))).into_response(),
        };
        // reqwest POST to cloud change-pin endpoint with forwarded JWT
        // ... (extract Bearer token from request, forward) ...
        cloud_verified = true; // set from cloud response
    } else {
        // Cloud: write directly
        // Duplicate PIN check, UPDATE staff_members SET pin = ?, updated_at = datetime('now') WHERE id = ?
        // post_write_verify_staff_pin(&state.db, &id, &req.new_pin).await
        cloud_verified = true;
    }

    // Trigger immediate sync pull
    if let Err(e) = cloud_sync::pull_tables_now(&state, &["staff_members"]).await {
        tracing::warn!(target: "staff_pin", "sync pull_now failed: {}", e);
    }

    // Verify venue PIN
    venue_verified = post_write_verify_staff_pin(&state.db, &id, &req.new_pin).await.is_ok();

    // Spawn delayed verify
    spawn_delayed_sync_verify(state.db.clone(), state.config.cloud.sync_interval_secs, id, req.new_pin, correlation_id.clone());

    Json(ChangePinResponse {
        status: if cloud_verified && venue_verified { "ok".into() } else { "partial".into() },
        cloud_verified,
        venue_verified,
        latency_ms: start.elapsed().as_millis() as u64,
        correlation_id,
    }).into_response()
}
```

### staffApi.ts — changePin method

```typescript
// Source: design extending existing staffApi pattern in src/lib/api/staff.ts
changePin: (id: string, newPin: string) =>
  rcFetch(`/admin/staff/${id}/change-pin`, {
    method: 'POST',
    body: JSON.stringify({ new_pin: newPin }),
  }) as Promise<ChangePinResponse>,
```

### Pre-Deploy Gate Script (bash)

```bash
#!/usr/bin/env bash
# Source: design decision per DEP-04
set -e
echo "Checking Phase 343 prerequisite..."
if ! git log --oneline | grep -q "343-01"; then
  echo "ERROR: Phase 343 Plan 01 not found in git log. Deploy blocked."
  exit 1
fi
if ! git log --oneline | grep -q "343-02"; then
  echo "ERROR: Phase 343 Plan 02 not found in git log. Deploy blocked."
  exit 1
fi
echo "Phase 343 prerequisite satisfied."
```

Note: Actual commits for 343 are `b31c38e0` (Plan 01) and `6c870f99` (Plan 02). The script can grep for those short hashes or for a marker string in the commit message.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| curl/sqlite3 scripts for PIN changes | `change_staff_pin_safe` endpoint | Phase 347 | Uday can change PINs without terminal access |
| 30s cloud sync interval (only path for PIN propagation) | `sync_pull_now` for immediate propagation | Phase 347 | Kiosk picks up new PIN in <5s instead of ≤30s |
| Inline PIN edit in staff table (shows PIN) | Change PIN modal (never shows existing PIN) | Phase 347 | STAFF-03 compliance |
| No verification that PIN propagated | Dual verify (cloud + venue) in response | Phase 343+347 | Staff sees green before assuming PIN is active |

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| racecontrol binary (venue .23) | Endpoint registration + cloud_authority_guard | Must be redeployed | post-347 | None — must deploy |
| racecontrol binary (cloud Bono VPS) | Cloud-authoritative write path | Must be redeployed | post-343 | None — 343 must deploy first |
| racingpoint-admin rebuild (venue .23) | New UI | Must be rebuilt | post-347 | None |
| racingpoint-admin rebuild (cloud) | Cloud admin parity | Must be rebuilt | post-347 | None (DEPLOY PARITY rule) |
| Phase 343 deployed live | `cloud_authority_guard` + `post_write_verify_staff_pin` in production | NOT YET DEPLOYED | code-complete only | Cannot ship 347 without 343 live |

**Missing dependencies with no fallback:**
- Phase 343 must be live-deployed (venue + cloud) before Phase 347 ships. This is DEP-01.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust `#[cfg(test)]` + `#[tokio::test]` (racecontrol-crate) |
| Config file | Workspace Cargo.toml (`[profile.test]`) |
| Quick run command | `cargo test -p racecontrol-crate change_pin -x` |
| Full suite command | `cargo test -p racecontrol-crate && cargo test -p rc-agent-crate && cargo test -p rc-common` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| STAFF-05 | `change_staff_pin_safe` rejects PIN < 4 digits | unit | `cargo test -p racecontrol-crate change_staff_pin_safe_rejects_short_pin` | Wave 0 |
| STAFF-05 | `change_staff_pin_safe` rejects non-numeric PIN | unit | `cargo test -p racecontrol-crate change_staff_pin_safe_rejects_non_numeric` | Wave 0 |
| STAFF-06 | Response includes `cloud_verified`, `venue_verified`, `latency_ms`, `correlation_id` | unit | `cargo test -p racecontrol-crate change_staff_pin_safe_response_shape` | Wave 0 |
| STAFF-07 | `sync_pull_now_handler` calls pull for specified tables | unit | `cargo test -p racecontrol-crate sync_pull_now_tables_filtered` | Wave 0 |
| DEP-04 | Pre-deploy gate script fails if 343 not in git log | manual smoke | `bash scripts/deploy/phase347-preflight.sh` | Wave 0 |
| STAFF-01+04 | Modal validates 4+ numeric, matching inputs | manual | Playwright screenshot (Phase 350) | Deferred |
| STAFF-08 | Staged progress UI shows 4 steps | manual | Playwright (Phase 350) | Deferred |

### Sampling Rate
- Per task commit: `cargo test -p racecontrol-crate -- --test-thread=1 2>&1 | tail -5`
- Per wave merge: `cargo test -p racecontrol-crate && cargo test -p rc-agent-crate`
- Phase gate: Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/racecontrol/src/api/routes.rs` — unit tests for `change_staff_pin_safe` (validation, response shape)
- [ ] `crates/racecontrol/src/api/routes.rs` — unit test for `sync_pull_now_handler` (table filter)
- [ ] `scripts/deploy/phase347-preflight.sh` — pre-deploy gate script (DEP-04)

---

## Open Questions

1. **JWT extraction in venue->cloud forwarding**
   - What we know: The admin JWT is in the `Cookie` header as `rc_admin_session`. The proxy at `[...path]/route.ts` extracts it from `req.cookies.get(COOKIE_NAME)`. But in the Rust handler, the staff JWT is passed via `Authorization: Bearer <token>`.
   - What's unclear: When `change_staff_pin_safe` runs on venue and forwards to cloud, it needs to forward the same Bearer token. Rust extractors can read the `Authorization` header, but the handler needs the raw token string, not just the validated claims.
   - Recommendation: Extract the token from `TypedHeader<Authorization<Bearer>>` and forward it as-is to the cloud endpoint. Add `use axum::TypedHeader` if not already imported. Alternatively, read from the staff JWT claims that `require_staff_jwt` already validated.

2. **`sync_once_http` visibility for `sync_pull_now`**
   - What we know: `sync_once_http` is defined in cloud_sync.rs and called from the private `spawn()` closure. It pulls ALL 14 SYNC_TABLES.
   - What's unclear: Whether adding a `pub(crate)` filtered-pull function will require significant refactoring.
   - Recommendation: Add a minimal `pub(crate) async fn pull_staff_members_now(state: &Arc<AppState>) -> anyhow::Result<()>` that does only the staff_members upsert path (copy of the relevant section from `sync_once_http`). This avoids touching the existing pull flow and keeps the blast radius small. About 30 lines.

3. **Feature flag: client-side only or also server-side gate?**
   - What we know: D-18 says `NEXT_PUBLIC_FEATURE_STAFF_PIN_UI` is client-side. The new endpoint exists regardless.
   - What's unclear: Should the Rust endpoint also check the flag and return 503 if disabled?
   - Recommendation: No server-side gate on the endpoint itself. The feature flag controls whether the UI button appears. The endpoint is always registered once Phase 347 ships. Adding a server-side gate adds complexity with no security benefit (the endpoint requires manager+ JWT anyway).

---

## Project Constraints (from CLAUDE.md)

The following CLAUDE.md directives apply to this phase:

- **No `.unwrap()` in production Rust** — use `?`, `.ok()`, or match. All new Rust handlers must use `?` for error propagation.
- **No `any` in TypeScript** — `ChangePinResponse` must be a typed interface, not `any`.
- **Route Uniqueness test** — after adding new routes, verify with `cargo test -p racecontrol-crate route_uniqueness`.
- **Two-repo involvement** — both racecontrol (Rust) and racingpoint-admin (Next.js) must be updated. Parity rule applies: deploy both venue AND cloud.
- **Deploy Manifest Protocol (DMP)** — PLAN.md must include `deploy:` section listing rust_binary (racecontrol), frontend_rebuild (admin), targets (server .23, cloud Bono VPS).
- **Subagent gates** — frontend changes require `gsd-ui-researcher` before planning (UI-SPEC.md) and `gsd-ui-auditor` after execution (UI-REVIEW.md). Business logic changes require `gsd-nyquist-auditor`.
- **MMA audit** — Phase 347 creates a cross-system bridge (admin UI -> racecontrol -> cloud -> venue sync -> verify). CLAUDE.md: "Any new feature that creates a data flow across 2+ system boundaries MUST have a multi-model AI audit before deploy."
- **Auto-push rule** — every commit in both repos must be pushed immediately.
- **LOGBOOK.md** — append entry after every commit.
- **Front-end: never read `sessionStorage`/`localStorage` in `useState` initializer** — the feature flag env var is safe (it's `process.env`, not browser storage).
- **DEPLOY PARITY** — admin rebuild on venue `.23:3201` must be followed by cloud admin rebuild on `admin.racingpoint.cloud`.
- **Fix all systems** — after racecontrol deploy, both venue AND cloud binaries must be updated.

---

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/api/routes.rs` — existing staff handlers (`reset_staff_pin`, `cloud_authority_guard`, `post_write_verify_staff_pin`, `spawn_delayed_sync_verify`), route registration patterns (manager+ sub-router), existing route map
- `crates/racecontrol/src/cloud_sync.rs` — `sync_once_http` function, SYNC_TABLES constant, `spawn()` interval logic, pull flow for staff_members
- `racingpoint-admin/src/app/(dashboard)/staff/manage/page.tsx` — existing staff CRUD page, PIN toggle pattern to remove
- `racingpoint-admin/src/lib/api/staff.ts` — staffApi object to extend with `changePin`
- `racingpoint-admin/src/lib/api/base.ts` — `rcFetch` pattern with circuit breaker
- `racingpoint-admin/src/app/api/rc/[...path]/route.ts` — catch-all proxy: handles all methods with JWT forwarding
- `racingpoint-admin/src/components/ConfirmDialog.tsx` — modal overlay pattern
- `racingpoint-admin/src/components/Skeleton.tsx` — `SkeletonTable` component
- `.planning/phases/347-admin-staff-management/347-CONTEXT.md` — locked decisions D-01..D-18
- `.planning/phases/343-staff-pin-hardening/343-CONTEXT.md` — Phase 343 architecture decisions
- `.planning/REQUIREMENTS.md` — STAFF-01..10, DEP-01..04 definitions
- `.planning/STATE.md` — current deploy state (343 code-complete, not live; 347 blocked on 343)

### Secondary (MEDIUM confidence)
- `CLAUDE.md` (both repos) — standing rules for subagent gates, MMA requirement for cross-system bridges, no-unwrap, no-any, deploy parity

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already in workspace, no new dependencies
- Architecture: HIGH — patterns verified directly from source code, not docs
- Pitfalls: HIGH — all verified from actual code (cloud_authority_guard already implemented, route uniqueness test already exists)
- Test gaps: HIGH — exact file paths and function names determined from codebase inspection

**Research date:** 2026-04-10
**Valid until:** 2026-05-10 (stable codebase; only invalidated if cloud_sync.rs refactor changes pull path)
