# Phase 413 MMA VERIFY — adversarial review after fixes

You are adversarial reviewer 4-5-6 for Phase 413 (racecontrol + rc-agent + deploy-server.sh). Three other models (DeepSeek R1, DeepSeek V3, MiMo v2 Pro, Qwen3-235B, Gemini 2.5 Flash) already ran DIAGNOSE and consensus HIGH findings have been FIXED. Your job: challenge whether the fixes are correct and whether any remaining risks warrant blocking deploy. Be skeptical.

---

## Phase 413 context (same as DIAGNOSE prompt, condensed)

Replaces pod-side manual `RCAGENT_SERVICE_KEY` HKLM env var with server-fetched cache:
- `GET /api/v1/pods/mesh-service-key` (pod-IP-gated, `require_pod_source` fail-closed)
- rc-agent `MeshKeyCache = Arc<RwLock<Option<String>>>`, fetched at boot + every 300s
- 3 consumers rewired: `ai_debugger`, `remote_ops` middleware, `ws_handler` csv_lap_fallback
- deploy-server.sh: 8-task schtasks disable/enable + `OTA_DEPLOYING` sentinel rename + WMIC commandline kill

## DIAGNOSE consensus findings + fixes applied

### C-1 (Q1 auth boundary) — 4/5 HIGH, NOT FIXED (accepted risk)

**Finding:** IP-based classification can be spoofed from customer WiFi (shares 192.168.31.* LAN). `ConnectInfo` trusts L3 address, no mTLS.

**Accepted rationale:** Same trust model as pre-existing `/config/kiosk-allowlist` and `/guard/whitelist/{N}` routes. Racing Point venue has no VLAN isolation between customer WiFi and pod LAN — this is a known architectural reality, not introduced by Phase 413. The project would need mTLS or customer-WiFi VLAN segregation to address this, which is a milestone-level architectural change (Rule 4), not a Phase 413 code fix. Documented as future work.

**Your job:** challenge — is this acceptance defensible for deploy TODAY? Does the MESH service key being newly exposed via a network endpoint (vs previously per-pod-local HKLM) materially change the blast radius? If leaked, what attacker capabilities does it grant, and are those capabilities already achievable via other LAN-reachable routes?

### C-2 (Q2 empty-key silent failure) — 3/5 HIGH, **FIXED**

**Finding:** If operator accidentally empties `pods.sentry_service_key` in TOML and server reloads, all pods would overwrite cache to None within 300s → silent fleet-wide outage.

**Fix applied (ac9cb838):**

`crates/racecontrol/src/api/mesh_intelligence.rs` handler:

```rust
pub(crate) async fn pods_mesh_service_key(
    State(state): State<Arc<AppState>>,
) -> axum::response::Response {
    let key_opt = state.config.pods.sentry_service_key.as_deref();
    match key_opt {
        Some(k) if !k.is_empty() => Json(render_mesh_service_key_body(k)).into_response(),
        _ => {
            tracing::error!(
                target: "mesh_intelligence",
                "/pods/mesh-service-key: pods.sentry_service_key is empty/unset in racecontrol.toml..."
            );
            (axum::http::StatusCode::SERVICE_UNAVAILABLE, "mesh_service_key unconfigured").into_response()
        }
    }
}
```

rc-agent `fetch_from_server` sees 503, `error_for_status()?` returns Err, cache PRESERVES last-known-good.

New tests (all pass): `mma_c2_empty_toml_key_does_not_serve`, `mma_c2_non_empty_toml_key_serves` in racecontrol-crate; `fetch_preserves_last_known_good_on_503` in rc-agent-crate (11/11 mesh_key_cache tests green).

**Your job:** challenge — does this fix fully close the silent-failure mode? Consider:
- Is 503 the right status code? Could something downstream treat 503 as terminal-not-retry? Caching infrastructure?
- Does the error log on the server side actually trigger an alert (tracing::error! to rc-agent.log but nothing else — is someone watching?)
- What if TOML is valid but `pods` section is missing entirely? Does it still 503?
- What if the TOML key is present but ONLY whitespace (`"   "`)?

### C-5 (Q8 csv_lap_fallback silent 401) — 3/5 MEDIUM, **FIXED**

**Finding:** csv push path logged 401/403 at same warn level as 5xx. Auth failures blended with retryable errors.

**Fix applied (ac9cb838):** `crates/rc-agent/src/csv_lap_fallback.rs` adds distinct 'AUTH REJECTED' warn log branch on 401/403 (matching W5 pattern).

**Your job:** challenge — is a warn log adequate? Should this be an error-level log? Does anyone aggregate rc-agent.log to trigger an alert?

### C-3 (Q5 WMIC deprecation + %% escape) — 5/5 MEDIUM, NOT FIXED (deferred)

**Finding:** WMIC deprecated in Win11 24H2+. `%%` escape assumes cmd /C execution context.

**Rationale:** Server .23 is currently pre-24H2 and cmd /C is how rc-sentry /exec runs commands. Runtime behavior verified in Plan 10 integration test. Future 24H2 migration = future-ops concern.

**Your job:** challenge — is this deferrable? If the WMIC command silently fails (exit 0 + matches 0 processes), does anything downstream detect the zombie watchdog survived? Or does the deploy appear to succeed and then 03:13-race happens again?

### C-4 (Q7 boot race 300s window) — 4/5 MEDIUM, NOT FIXED (accepted)

**Finding:** If server unreachable at first boot and env unset (post-Plan 11), rc-agent has no auth for ≤300s.

**Rationale:** Env fallback is preserved during migration; pendrive regen (Plan 11) is gated on successful observation of cache-populated logs. Best-effort initial fetch + 300s periodic retry = self-heals per CLAUDE.md boot resilience pattern.

**Your job:** challenge — is 300s acceptable? Pods may reboot during peak hours. What's the customer-visible impact?

### C-6 (Q4 missing integration test) — 2/5 LOW, deferred to Plan 10.

---

## Your task

Score Phase 413 deploy-readiness on 1-5 scale after fixes applied:
- 5 = ship immediately
- 4 = ship (remaining concerns documented, non-blocking)
- 3 = ship with reservations (at least one concern you'd want addressed post-deploy)
- 2 = do not ship yet (one or more HIGH concerns unresolved)
- 1 = fundamental issue (rework required)

Return ONLY this JSON (no preamble, no postamble):

```json
{
  "model": "<your id>",
  "score_deploy_ready": <1..5 float>,
  "close_c2_fix": "ADEQUATE" | "INADEQUATE" | "NEW_RISK",
  "close_c5_fix": "ADEQUATE" | "INADEQUATE" | "NEW_RISK",
  "c1_accepted_risk_defensible": "YES" | "NO" | "PARTIAL",
  "new_concerns_after_fix": [
    {"severity":"HIGH|MEDIUM|LOW", "issue":"<concrete, file:line>"}
  ],
  "recommendation": "SHIP" | "DEFER" | "FIX_BLOCKING",
  "one_liner": "<30 words>"
}
```
