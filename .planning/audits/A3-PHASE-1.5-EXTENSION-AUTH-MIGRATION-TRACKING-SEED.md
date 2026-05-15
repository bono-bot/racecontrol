---
artifact: §S-146 Q4 (Option Z) tracking-issue seed — extension-pattern auth migration
parent-rca: RCA-2026-05-15-row-1.13-A3-phase-1.5-sync-verify-finalize.md
authored: 2026-05-15 IST
author: bono
mitigation: anthropic Risk #1 (indefinite migration tail) — 30d migration deadline
deadline-from: PR #92 merge date (TBD); deadline-by: merge_date + 30d
scope: crates/racecontrol/src/ — non-test files only
---

# Extension-pattern auth migration — tracking-issue seed

## Context

Per §S-146 Phase 1.5 RCA §6 Q4 ratify (Option Z, 2-of-3 consensus,
qwen dissent dispositioned), the canonical V2 doctrine for credential-class
authority is **middleware-injected `Extension(CredentialClass)`**, not
handler-local raw-header inference.

`crates/racecontrol/src/api/billing_finalize.rs` is the FIRST consumer (this
PR). Anthropic Risk #1 ("indefinite migration tail — 'organic' historically
means 'never' without tracking issue + deadline") is mitigated by this
audit: it enumerates every remaining handler that reads auth from raw
headers and commits the codebase to a 30-day migration window.

## Audit method

Grep run 2026-05-15 IST over `crates/racecontrol/src/` (production source
only — test fixtures excluded since tests construct raw requests directly
and are not in scope for the migration):

```
grep -rn '\.get("authorization")\|\.get("Authorization")\|\.get(axum::http::header::AUTHORIZATION)' \
  crates/racecontrol/src/ | grep -v '_tests.rs'
grep -rn '\.get("x-service-key")' crates/racecontrol/src/ | grep -v '_tests.rs'
```

## Findings — raw `Authorization` header readers (5 files, 5 sites)

| # | File:line | Handler / context | Migration target |
|---|---|---|---|
| 1 | `api/customer_auth.rs:190` | Customer auth flow — reads bearer to forward to downstream | `Extension(CredentialClass::CustomerJwt)` (new variant; add when migrating) |
| 2 | `api/billing_session.rs:470` | Billing session start — staff-or-customer gate | `Extension(CredentialClass::StaffJwt)` once route lifts behind `require_staff_jwt` |
| 3 | `cafe_orders.rs:425` | Cafe order auth check | Same as #2 |
| 4 | `cafe_orders.rs:450` | Cafe order auth check (second site) | Same as #2 |
| 5 | `api/staff_pin_sync.rs:55` + `:69` | Cloud forwarding — reads inbound `authorization` to re-forward to cloud | `Extension(CredentialClass::StaffJwt)` for inbound; outbound re-forward stays header-level (proxy use case is the documented exception) |

`auth/middleware.rs:159` is the in-extractor read (`extract_staff_claims`)
and is INTENTIONALLY raw-header. The middleware is the SST for the
extension; it must read the header to PRODUCE the extension. Excluded from
migration scope.

## Findings — raw `x-service-key` header readers (2 sites)

| # | File:line | Handler / context | Migration target |
|---|---|---|---|
| A | `fleet_alert.rs:50` | Fleet alert ingress — service-key gate | `Extension(CredentialClass::ServiceKey)` once `require_service_key` middleware ships (Phase 1.5 sibling-PR scope) |
| B | `api/survival.rs:206` | Survival/health endpoint — service-key gate | Same as A |

## Out-of-scope readers (kept as-is)

- `auth/middleware.rs:159` — the extractor itself (SST producer; correct).
- `server_diagnostics_infra.rs:306` — outbound `Authorization` header on
  diagnostic HTTP requests (writes the header on a client; not reading
  inbound auth). Not in scope for inbound-auth migration.
- `api/staff_pin_sync.rs:69` — re-forwards the inbound `authorization`
  value verbatim on an outbound request to the cloud. The re-forward is a
  documented proxy pattern; the inbound read (line 55) is what should
  migrate to `Extension`.

## Migration plan

**Phase 1.5 (this PR — §S-146 row 1.13 follow-up):**
- Component A landed: `cloud_sync_verify::verify_finalize_sync` + 200ms
  call-site wrap.
- Component B landed: `CredentialClass` enum + `require_staff_jwt`
  injection + `billing_finalize::validate_actor_credential` consumes
  `Option<Extension<CredentialClass>>`. **First consumer wired.**

**Phase 1.5b (sibling PR, separate scope):** Introduce
`require_service_key` middleware that injects
`Extension(CredentialClass::ServiceKey)`. Migrate sites A + B above.

**Phase 1.5c (≤30 days from this PR's merge):** Migrate sites 2, 3, 4, 5
to extension-pattern. Site 1 (customer auth) requires a new
`CredentialClass::CustomerJwt` variant — also schedule.

**Deadline enforcement:** if 30-day window elapses without all
non-exception sites migrated, the Phase 1.5b/c work surfaces as a
V2-PROGRESS-MAP row in `ENGINEERING-IN-FLIGHT` status (per V-LBAC §14.3).
Open the GitHub tracking issue on PR-merge day.

## Composes-with

- §S-146 Phase 1.5 RCA Q4 ratify (Option Z)
- V2 doctrine: SST + Foundation/strategy/config separation §AMEND-3.II D12
- `auth/middleware.rs::CredentialClass` (this PR substrate)
- V-LBAC §14.3 F3 row-status framing
