# Phase 413 MMA VERIFY Round 2 — post-whitespace-fix

Round 1 VERIFY (Kimi K2.5, Nemotron 3 Super, GPT-5.4 Nano) averaged 2.33/5.0 with recommendations FIX_BLOCKING×2, DEFER×1. Round 1 caught one HIGH that DIAGNOSE missed: whitespace-only TOML key bypass. It also re-raised C-1 (IP-auth boundary) from ACCEPTED to PARTIAL-defensible.

## Fix applied since Round 1 (commit `2c530fc4`)

`crates/racecontrol/src/api/mesh_intelligence.rs`:

```rust
Some(k) if !k.trim().is_empty() => Json(render_mesh_service_key_body(k)).into_response(),
_ => {
    tracing::error!(target: "mesh_intelligence", "...refusing to serve...");
    (axum::http::StatusCode::SERVICE_UNAVAILABLE, "mesh_service_key unconfigured").into_response()
}
```

Changed `!k.is_empty()` → `!k.trim().is_empty()`. Now rejects `" "`, `"   "`, `"\t"`, `"\n"`, `" \t\n "`. 7/7 phase413_tests pass, including new `mma_verify_new1_whitespace_key_does_not_serve` (5 whitespace variants) and `mma_verify_new1_whitespace_surrounding_real_key_still_serves`.

## Previously-fixed concerns (from DIAGNOSE)

- C-2 HIGH (empty TOML key): server 503 + rc-agent cache preserves last-known-good. Now handles whitespace too (post-Round-1 fix).
- C-5 MEDIUM (csv_lap_fallback silent 401): distinct "AUTH REJECTED" warn log added.

## Remaining accepted risks (documented, not fixed)

- C-1 HIGH (IP-auth boundary): same trust model as pre-existing `/config/kiosk-allowlist` and `/guard/whitelist/{N}` routes. Venue has no VLAN isolation between customer WiFi and pod LAN. Upgrading to mTLS is a milestone-level architectural change (Rule 4), not a Phase 413 code fix. Round 1 Kimi argued this is less defensible because HKLM required local admin/pod compromise while a network endpoint requires only LAN access — a fair point that we acknowledge as a NEW blast-radius consideration introduced by Option Z. Mitigations beyond IP gating that DO exist: (a) `require_pod_source` is fail-closed, (b) the key is for rc-agent's /exec endpoint which uses constant-time compare (ct_eq) as defense-in-depth, (c) the key would only grant what rc-agent already grants (pod-level ops on that one pod), not server-level admin.
- C-3 MEDIUM (WMIC deprecation): Server .23 is pre-Win11-24H2. Plan 10 integration test exercises the new WMIC command. Future migration to 24H2+ is a separate ops phase. %% escape behavior will be verified in Plan 10 against the live /exec path.
- C-4 MEDIUM (300s boot-race window): best-effort initial fetch + env fallback during migration window. Pendrive regen (Plan 11) is gated on observed cache-populated logs.

## Your task

Given the whitespace fix is now applied, score Phase 413 deploy-readiness 1-5:

- 5 = ship immediately
- 4 = ship (remaining concerns documented + non-blocking)
- 3 = ship with reservations
- 2 = do not ship
- 1 = fundamental issue

Return ONLY this JSON (no preamble):

```json
{
  "model": "<your id>",
  "score_deploy_ready": <1..5 float>,
  "whitespace_fix_adequate": "YES" | "NO",
  "c1_accepted_risk_defensible_after_full_mitigation_disclosure": "YES" | "NO" | "PARTIAL",
  "any_new_HIGH_concerns": [
    {"issue": "<concrete, file:line>", "why_not_in_prior_rounds": "<why earlier rounds didn't catch this>"}
  ],
  "recommendation": "SHIP" | "DEFER" | "FIX_BLOCKING",
  "one_liner": "<30 words>"
}
```
