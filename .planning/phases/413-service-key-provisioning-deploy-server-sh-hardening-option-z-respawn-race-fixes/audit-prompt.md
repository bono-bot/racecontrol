# Phase 413 MMA Audit — DIAGNOSE Prompt

You are auditing Phase 413 of Racing Point racecontrol (Rust/Axum server + rc-agent pod agent + Bash deploy script). This phase introduces a CROSS-SYSTEM BRIDGE and per CLAUDE.md is MANDATORY MMA before deploy. Your job is to find real bugs and architecture-level risks. Be concrete — point at file:line. Do NOT rewrite the code or propose new features.

---

## Phase goal (1 paragraph)

Replace the per-pod HKLM `RCAGENT_SERVICE_KEY` env-var provisioning (which drifted out of sync with server's `racecontrol.toml` `pods.sentry_service_key`, causing silent 401s fleet-wide since MMA-v29 — "Gap 4") with a server-fetched, periodically-refreshed cache. The server exposes a pod-IP-gated endpoint; rc-agent fetches at boot + every 300s via the existing `boot_resilience::spawn_periodic_refetch` pattern; three consumers (ai_debugger Tier 0, remote_ops middleware, ws_handler csv_lap_fallback) read from cache instead of env. Also: deploy-server.sh hardened with 3 fixes that caused the 2026-04-18 03:13 IST deploy abort (schtasks coverage expanded to 8 tasks, sentinel renamed `DEPLOY_IN_PROGRESS` → `OTA_DEPLOYING` to match the runtime-shipped PS watchdog, taskkill WINDOWTITLE wildcard replaced with WMIC commandline match).

---

## What shipped (bulleted, with file refs)

### Server (Plan 01)
- `crates/racecontrol/src/network_source.rs`:
  - New `require_pod_source` middleware (fail-CLOSED — rejects missing RequestSource extension with 403).
  - `classify_ip` gains two entries: `192.168.31.130` (POS LAN) → Pod; `100.95.211.1` (POS Tailscale) → Pod (single-IP exception; **not** a 100.x.x.x widen — Bono VPS 100.70.177.44 stays Cloud, server 100.125.108.37 stays Cloud).
- `crates/racecontrol/src/api/mesh_intelligence.rs`: new `pods_mesh_service_key` handler returns `{"mesh_service_key": "<key>"}` from `state.config.pods.sentry_service_key.as_deref().unwrap_or("")`.
- `crates/racecontrol/src/api/routes.rs`: registers `GET /api/v1/pods/mesh-service-key` inside `public_routes` with `.route_layer(axum::middleware::from_fn(require_pod_source))` (per-route scope, not `.layer`).

### rc-agent (Plan 02 + 03 + 04)
- `crates/rc-agent/src/mesh_key_cache.rs` (NEW, 329 lines):
  - `pub type MeshKeyCache = Arc<RwLock<Option<String>>>` (tokio RwLock).
  - `fetch_from_server(client, http_base, cache) -> Result<(), reqwest::Error>`:
    - Calls `GET {http_base}/pods/mesh-service-key`.
    - 403 → tracing::warn! + `error_for_status()?` returns Err; cache unchanged.
    - Other non-2xx → tracing::debug! + Err; cache unchanged.
    - Network error → Err; cache unchanged.
    - 200 + non-empty `mesh_service_key` → cache = `Some(key)`.
    - 200 + empty `mesh_service_key` OR missing field → cache = `None` (explicit OVERWRITE of existing key).
  - `get_key_or_env(&cache) -> Option<String>`:
    - Cache read first; if `Some(non_empty)` return it.
    - Else `std::env::var("RCAGENT_SERVICE_KEY")`; return it if non-empty.
    - Else `None`.
  - 10 wiremock unit tests (including 403-preserves-last-known-good + empty-overwrites-existing).
- `crates/rc-agent/src/main.rs` (Plan 03 + Plan 04 scaffolding):
  - `let mesh_key_cache = crate::mesh_key_cache::new_cache();` — instantiated (feature-gated `http-client`) above `remote_ops::start_checked` so the cache is passed as param into the middleware sub-router.
  - Boot: synchronous best-effort `fetch_from_server(..)` — Ok→info log, Err→warn log; never blocks startup.
  - Periodic: `rc_common::boot_resilience::spawn_periodic_refetch("mesh_service_key", Duration::from_secs(300), || async { fetch_from_server(..).await })`.
  - `AppState { .., #[cfg(feature = "http-client")] mesh_key_cache: MeshKeyCache }` so ai_debugger + ws_handler read via `state.mesh_key_cache`.
- `crates/rc-agent/src/ai_debugger.rs` — `analyze_crash` takes `mesh_key_cache: MeshKeyCache` param, `check_audit_known_issues` uses `get_key_or_env(&cache).await`. On HTTP 403 emits a distinct tracing::warn! (W5 fix). No-default-features variant returns None (Tier 0 unavailable).
- `crates/rc-agent/src/remote_ops.rs` — `require_service_key(State<MeshKeyCache>, req, next)` reads `get_key_or_env(&cache).await`. Wired via axum `from_fn_with_state` on a protected sub-router with `.with_state(cache)`. A preserved env-only variant for `#[cfg(not(feature = "http-client"))]`.
- `crates/rc-agent/src/ws_handler.rs` — csv_lap_fallback resolves the key INSIDE the tokio::spawn (clones Arc, awaits get_key_or_env inside the async move); 2 analyze_crash call sites take the cache clone.
- `crates/rc-agent/src/event_loop.rs` — 2 analyze_crash call sites pass `state.mesh_key_cache.clone()`.

### deploy-server.sh (Plans 05 + 06 + 07)
- Step 3a disable block (line ~215): `schtasks /Change /TN <name> /Disable` expanded from 2 tasks (StartRCOnBoot, StartRCTemp) to 8 (adds RCWatchdog, RaceControlStartup, StartRCDirect, StartRaceControl, StartRCWatchdog, StartFrontendWatchdog). Also **replaces** `taskkill /F /IM powershell.exe /FI "WINDOWTITLE eq *watchdog*"` with `wmic process where "name='powershell.exe' and commandline like '%%start-racecontrol-watchdog.ps1%%'" delete`. Also writes `OTA_DEPLOYING` sentinel (renamed from `DEPLOY_IN_PROGRESS` — the PS watchdog at `start-racecontrol-watchdog.ps1:61` already read `OTA_DEPLOYING`; the writer had been using a name the checker never read).
- Step 5b (success path, line ~268): `schtasks /Change /TN <name> /Enable` expanded to 8 + `del /Q C:\RacingPoint\OTA_DEPLOYING`.
- Rollback path (line ~336): identical to Step 5b (symmetric re-enable + sentinel delete).

---

## Key semantic facts (for audit)

1. `/pods/mesh-service-key` handler does NO tokio RwLock access — it reads `state.config.pods.sentry_service_key.as_deref().unwrap_or("")` synchronously (config is a plain Arc<Config>, no runtime mutation).
2. `fetch_from_server` holds a `cache.write().await` guard for ONE assignment (`*guard = new_value`) then `drop(guard)`. No await between acquire and release.
3. `get_key_or_env` holds a `cache.read().await` guard for a `.clone()` — returns the cloned Option<String> after drop implicit. No await under the read lock.
4. Periodic refetch closure moves 3 clones (cache, base, client) and awaits `fetch_from_server`. The closure is Send + 'static and reruns every 300s.
5. `require_pod_source` is FAIL-CLOSED: missing `RequestSource` extension → 403. `require_non_pod_source` (existing) is FAIL-OPEN. Audit concern: is the asymmetry documented/sound?
6. rc-agent's consumers now have a 3-tier fallback: (a) cache, (b) env var, (c) None. Production env is intended to be unset after Plan 05 pendrive regen.
7. POS (192.168.31.130) is now classified as Pod (not Customer/Staff). It runs rc-agent and is a mesh-key fetcher. Staff routes (gated by `require_non_pod_source`) now EXCLUDE POS — verify whether any existing staff flow relied on POS being non-Pod (regression risk).
8. The `pods.sentry_service_key` TOML value is the ONLY source of truth after Phase 413. If an operator accidentally wipes it (empty string in TOML), the server returns 200 + empty → ALL pods overwrite their cache to None → ALL Tier 0 checks go silent until operator notices. No "refuse to serve empty" guard was added.
9. WMIC is deprecated in Windows 11 24H2+. Server .23 currently has WMIC. If the server is reimaged to 24H2+, Factor 3 (watchdog kill) breaks silently.
10. `%%` escape in the WMIC invocation: bash single-quotes → JSON → `cmd /C` on rc-sentry /exec → `%` in WMIC. If rc-sentry's exec handler changes how it invokes cmd (e.g., direct CreateProcessW without cmd /C), the `%%` arrives literal and WMIC matches nothing.

---

## 8 questions you must answer concretely

1. **Auth boundary:** Does pod-IP gating on `/pods/mesh-service-key` plus `require_pod_source` fail-closed adequately bound the secret? Specifically address: (a) customer WiFi on LAN 192.168.31.*, (b) Tailscale traffic from any node that shares the Tailnet, (c) cloud (Bono VPS at 100.70.177.44) requests, (d) spoofed X-Forwarded-For (is ConnectInfo trustworthy?).

2. **Cache semantics — explicit-empty-overwrites:** Is "server 200 + empty → cache None (overwrite)" vs "network fail → preserve last-known-good" a correct split? Concretely: if operator accidentally empties `pods.sentry_service_key` in TOML and reloads, pods overwrite cache to None within 300s and all Tier 0 go silent. Is that the right failure mode? Should there be a guard?

3. **Race / lock / deadlock:** Any lock-held-across-await in `mesh_key_cache.rs`? Any deadlock possibility between `periodic_refetch` writer and middleware reader under load (remote_ops receives 100s of req/s from server pushes)? Is `tokio::sync::RwLock` write-starving under heavy reader load? Is the RwLock the right primitive?

4. **Test coverage:** The 10 mesh_key_cache tests + 7 preserved remote_ops service-key tests + the new S10 `test_service_key_cache_wins_over_env` — sufficient? What's MISSING? (e.g., integration test that the periodic refetch closure actually executes on the 300s timer; behavior when cache is `Some("")` — treated as None in `get_key_or_env`, but what does remote_ops middleware do if it gets `None` back?)

5. **Deploy script — 3-factor fix:** Do Plans 05+06+07 close all 3 abort factors without introducing new ones? Specifically: (a) Is the 8-task list complete or could `schtasks /Query` on .23 reveal unlisted tasks?, (b) Can the `OTA_DEPLOYING` sentinel be left stale (both success and rollback paths `del /Q` it — but what if the curl itself fails mid-delete?), (c) Is WMIC the right cross-version choice given Win11 24H2 deprecation?

6. **Cross-process / ecosystem:** What ELSE on a pod reads `RCAGENT_SERVICE_KEY` after this phase? Enumerate: rc-sentry (separate env var RCSENTRY_SERVICE_KEY — audit kept), install.bat (pendrive, to be regen'd in Plan 11), any PowerShell/batch scripts, any other Rust crates in the workspace. Is the scope complete? Is cloud's rc-agent (if any) affected?

7. **Bootstrap chicken-and-egg:** At FIRST BOOT after install, the cache is empty, env may be unset (per Plan 11's pendrive regen goal). The initial sync fetch is best-effort; if the server is unreachable at that moment, the cache stays None and consumers have NO auth for 300s. What's the customer-visible impact during that window? Does remote_ops serve requests with no auth (permissive mode) or reject? Does ai_debugger Tier 0 just return None (degrade-open)?

8. **Observability — W5 gap coverage:** The 403 warn is emitted by `fetch_from_server` and by `ai_debugger::check_audit_known_issues`. Is there an equivalent warn on the csv_lap_fallback push path? On `push_csv_fallback`? If the key is silently wrong and csv pushes silently 401 for days, what log line surfaces? What alert fires?

---

## Scoring

Output JSON of this shape (ONLY JSON, no preamble):

```json
{
  "model": "<your id>",
  "score_deploy_ready": <1..5 float, 5 = very high confidence>,
  "consensus_concerns": [
    {
      "severity": "HIGH" | "MEDIUM" | "LOW",
      "question": <1..8>,
      "issue": "<concrete, with file:line>",
      "mitigation_suggested": "<optional, brief>"
    }
  ],
  "one_liner_summary": "<one sentence>"
}
```

Score rubric:
- 5.0 = architecture is sound + tests cover failure modes + no HIGH concerns
- 4.0 = one or two MEDIUM concerns with clear mitigations, no HIGH
- 3.0 = one HIGH concern OR multiple MEDIUM, mitigations unclear
- 2.0 = multiple HIGH concerns
- 1.0 = fundamental flaw (don't ship)

Return ONLY the JSON. No prose before or after.
