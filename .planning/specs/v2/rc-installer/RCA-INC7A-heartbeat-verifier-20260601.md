# RCA / design gate — INC-7a license-heartbeat verifier (foundational-auth)

**Date:** 2026-06-01 · **Author:** bono · **Surface:** `crates/rc-installer/src/heartbeat.rs` (NEW) · **Branch:** `feat/rc-installer-trust-core`
**Auth:** Captain per-PR auth for INC-7a (2026-06-01, AskUserQuestion "Authorize 7a now"). Foundational-auth boundary → MMA Step 1 DIAGNOSE run BEFORE code.

## MMA Step 1 DIAGNOSE
Surface `MMA-INC7A-heartbeat-verifier-bono-2026-06-01` · OpenRouter · **4/5 OK across 4 vendor families** (anthropic, openai, google, mistralai; deepseek-r1 timed out at 90s abort) — meets ≥3-vendor-family threshold. Spend ~$0.078 (logged `openrouter-spend-bono.jsonl`).

### Consensus root-cause findings → enforced checks
| # | Sev | Check in `verify_heartbeat` | Disposition |
|---|---|---|---|
| 1 | CRITICAL | **machine_fingerprint == local fingerprint** (exact) | 7a (local fp passed in via ctx; compute is 7b/platform) |
| 2 | CRITICAL | temporal: gate on `valid_until` only; reject `valid_until<=issued_at`, TTL>max, future issued; bounded skew; anti-rollback floor | 7a (now/max/skew/floor via ctx) |
| 3 | CRITICAL | kid-pinned key lookup; verify only with that key; reject unknown/revoked; no try-all | 7a (reuses `TrustedKeySet`) |
| 4 | CRITICAL | canonicalization == pinned recipe | 7a (shared `canonical_signed_bytes`; golden vectors lock it) |
| 5 | CRITICAL | ed25519 only; reject malformed/wrong-length sig | 7a |
| 6 | CRITICAL | tenant_id == expected | 7a (expected via ctx) |
| 7 | CRITICAL | fail-closed `Result`; strict deserialize; no unwrap_or(true) | 7a |
| 8 | HIGH | anti-rollback persistence (`min_valid_until`) | 7a decision (`ctx.min_valid_until_ms`); persistence is 7b |
| 9 | HIGH | revocation: trust-store key-status + TTL content-rev | 7a (status gate) + 7b (trust-store updates) |
| 10 | HIGH | unique immutable kid; reject duplicate | 7a (`TrustedKeySet::find` first-match; duplicate-kid lint is a trust-store build concern) |
| 11 | HIGH | offline/grace bounded, fail-closed otherwise | **7b** (caller policy; out of pure core) |
| 12 | HIGH | `license_class` closed enum, no default-to-production | 7a (serde enum, unknown → deserialize error = fail-closed) |
| 13 | HIGH | feature_opt_in: signed→authentic; enforce expected keys | 7b (entitlement policy; core just authenticates) |
| 14 | HIGH | refresh-loop rate-limit; payload size cap | **7b** (transport boundary) |

**Single most important (unanimous):** machine_fingerprint binding — a valid signature without it is a transferable token.

## 7a scope (this PR) vs 7b (blocked)
- **7a (pure):** `LicenseHeartbeat` + `SignedLicenseHeartbeat` + `LicenseClass` + `canonical_signed_bytes` + `verify_heartbeat(&SignedLicenseHeartbeat, &VerifyContext) -> Result<(), HeartbeatError>`. No network/DB/state; all runtime inputs via `VerifyContext`. Fully unit-testable.
- **7b (blocked on Replit endpoints + F-series + per-PR auth):** compute local fingerprint (Windows), HTTPS fetch claim/refresh (behind `redeem_client` + `tls_pin` + `host_allowlist`), persist last-accepted (anti-rollback), bounded offline grace, refresh cadence, payload-size cap, entitlement enforcement.

## Dependency note
Adds `base64` (wire signature is base64 per Replit's `SignedLicenseHeartbeat` envelope; ReleaseManifest uses hex — heartbeat differs). Pure-Rust, cross-compiles to `x86_64-pc-windows-gnu`. Confined to the verifier.

## V1-dependence
Net-new V2 surface (no V1 license-heartbeat) → no V1-dependent 5-section RCA required; the auth-boundary escalation (MMA Step 1 + per-PR auth) is satisfied above.

## NOT covered by 7a (explicit)
Live fetch/refresh, local fingerprint computation, anti-rollback persistence, offline grace, rate-limiting, entitlement policy, real production-key live verification — all 7b, gated.
