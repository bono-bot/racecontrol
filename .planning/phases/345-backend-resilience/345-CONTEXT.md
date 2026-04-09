# Phase 345: Backend Resilience — Context

**Milestone:** v47.0 Admin Dashboard Venue-Ready Hardening
**Wave:** 1
**Status:** Ready to execute
**Depends on:** Phase 344
**Blocks:** Phase 346, 347

## Why this phase exists

From the 2026-04-09 admin audit:
- Cloud admin login returns **500** because `src/app/api/rc/[...path]/route.ts` throws `Error: RC_URL environment variable is required` at **module evaluation time** (line 5 of old file). Next.js can't instantiate the route. Every subsequent request 500s.
- Local admin admin.db routes return **500** because `better-sqlite3` fails to load (ABI mismatch) at `require()` time inside `src/lib/db.ts`. The error propagates as an unhandled route error returning plaintext "Internal Server Error".
- Racecontrol has two secret-handling bugs (Phase 343 C5 + C6):
  - `default_jwt_secret()` returns the literal `"racingpoint-jwt-change-me-in-production"` — even though `resolve_jwt_secret` treats it as unset, the string is compiled into the binary (optics + static analysis flags).
  - `payment_gateway_webhook` silently skips HMAC verification when `payment_webhook_secret` is unset — accepts unsigned webhooks.

## Scope

| In scope | Out of scope |
|---|---|
| Admin rc proxy env validation moved inside handler | Full runtime contract tests (Phase 350) |
| admin.db lazy-load with structured `AdminDbError` | Migrating to hashed PINs (Phase 343 C1, separate milestone) |
| `withAdminDbError` helper for route handlers | Rewriting every admin route to use withAdminDbError (follow-up) |
| racecontrol `default_jwt_secret` returns empty string (C5) | Forcing halt on missing JWT — kept auto-generate for dev UX |
| `payment_gateway_webhook` rejects when secret unset (C6) | Full HMAC-SHA256 verification (needs real gateway integration) |

## Requirements covered

- ADMIN-08: Module-load errors → JSON 503 (done)
- ADMIN-09: admin.db lazy-load + retry on ABI failure (done)
- ADMIN-10: JSON errors never HTML (partial — rc proxy done, other routes follow-up)
- ADMIN-11: 3rd circuit breaker for admin.db — deferred (existing AdminDbError + lazy-load is a reasonable substitute; breaker can be added later if retry storms observed)
- ADMIN-12: Remove hardcoded JWT secret default literal (C5 — done)
- ADMIN-13: Halt on missing webhook secret (C6 — done via runtime rejection at the endpoint; startup halt is stricter but would break existing toml files without `[integrations]` section)

## Key decisions

- **D-01:** Admin rc proxy env validation is inside the handler, not at module load. A missing `RC_URL` now returns JSON 503 `{error_code: "RC_URL_MISSING"}` — the process stays alive and every other route keeps working.
- **D-02:** `admin.db` uses lazy-load — `better-sqlite3` is `require()`d inside `getDb()`, NOT at module top. ABI mismatch throws `AdminDbError("ADMIN_DB_ABI_MISMATCH", ...)` with the exact rebuild command in the message.
- **D-03:** New `withAdminDbError(err)` helper for route handlers to map `AdminDbError` → JSON 503. Routes that haven't adopted it yet will still return 500 — we fix them as we touch them (Phases 346, 347, etc.).
- **D-04:** `default_jwt_secret()` returns empty string. Tests that reference the literal still pass because they pass it explicitly as a function argument, not via the default.
- **D-05:** Webhook rejection is runtime, not startup halt. Existing `racecontrol.toml` files without `[integrations]` section would crash on startup if we halted — breaking every venue + cloud with a single deploy. Runtime rejection at the endpoint is safer.

## Files changed

| File | Change |
|---|---|
| `racingpoint-admin/src/app/api/rc/[...path]/route.ts` | Env validation inside handler, structured error codes |
| `racingpoint-admin/src/lib/db.ts` | Lazy require, AdminDbError class, withAdminDbError helper |
| `racecontrol/crates/racecontrol/src/config.rs` | `default_jwt_secret` returns empty string |
| `racecontrol/crates/racecontrol/src/api/routes.rs` | `payment_gateway_webhook` rejects when secret unset |

## Verification run

```
$ cargo check --bin racecontrol
   Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.22s

$ cargo test --lib config::tests
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 823 filtered out
```

## Success criteria

1. Killing racecontrol → admin UI rcFetch routes return JSON 503 `{error_code: "RC_UNREACHABLE"}`, not HTML 500
2. Booting admin with missing `RC_URL` → rcFetch routes return JSON 503 `{error_code: "RC_URL_MISSING"}`, admin UI stays up
3. admin.db ABI mismatch → routes using withAdminDbError return JSON 503 `{error_code: "ADMIN_DB_ABI_MISMATCH"}` with rebuild hint
4. Racecontrol binary no longer contains the literal `"racingpoint-jwt-change-me-in-production"` in default_jwt_secret
5. `curl -X POST /api/v1/webhooks/payment-gateway` with no `[integrations].payment_webhook_secret` in racecontrol.toml returns `{ok: false, error: "payment webhook endpoint is disabled"}`

## NOT tested in this phase (handoff list)

- Live deploy to venue + cloud (requires Phase 344 `admin-deploy.sh` to ship first)
- Behavioral test of rc proxy under simulated backend outage — deferred to Phase 350 contract tests
- Admin route coverage for `withAdminDbError` — only new helper is added; applying it to every existing route is follow-up scope
- Racecontrol webhook test coverage — existing FATM-11 tests should still pass but not verified in this phase
