---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: 04
subsystem: rc-agent/mesh-key-consumer-rewire
tags: [option-z, mesh-service-key, rc-agent, w4-state-shape, w5-observability, s10-cache-wins, wave-2a]
dependency-graph:
  requires:
    - Phase 413-02 (MeshKeyCache module: new_cache, fetch_from_server, get_key_or_env)
    - Phase 413-03 (main.rs boot wire-up: cache instantiation + spawn_periodic_refetch)
    - axum 0.7+ from_fn_with_state + sub-router with_state pattern
  provides:
    - ai_debugger::check_audit_known_issues reads mesh key via get_key_or_env(cache)
    - remote_ops::require_service_key middleware reads via State(MeshKeyCache) sub-router pattern
    - ws_handler csv_lap_fallback push reads via state.mesh_key_cache
    - AppState.mesh_key_cache field (shared Arc accessible to all consumers)
    - S10 regression test (test_service_key_cache_wins_over_env)
    - W5 observability (403 logged at warn! in ai_debugger Tier 0 path)
  affects:
    - crates/rc-agent/src/ai_debugger.rs (signature + W5 logging + stub for no-default-features)
    - crates/rc-agent/src/app_state.rs (mesh_key_cache field)
    - crates/rc-agent/src/event_loop.rs (2 analyze_crash call sites updated)
    - crates/rc-agent/src/main.rs (cache moved earlier, passed to start_checked, added to AppState)
    - crates/rc-agent/src/remote_ops.rs (middleware rewrite + start* fns + tests)
    - crates/rc-agent/src/ws_handler.rs (2 analyze_crash call sites + csv_lap_fallback rewire)
tech-stack:
  added: []
  patterns:
    - "axum sub-router with State<MeshKeyCache> (Option (a) — no FromRef)"
    - "feature-gated dual signature (http-client ON vs --no-default-features) for all 3 consumer sites"
    - "Arc clone at spawn boundary + resolve-inside-spawn (ws_handler csv_lap_fallback)"
key-files:
  created: []
  modified:
    - crates/rc-agent/src/ai_debugger.rs (+70 -12: signature change, W5 warn block, cfg variants)
    - crates/rc-agent/src/app_state.rs (+14 -0: mesh_key_cache field + docstring)
    - crates/rc-agent/src/event_loop.rs (+4 -0: 2x cfg-gated state.mesh_key_cache.clone() args)
    - crates/rc-agent/src/main.rs (+20 -7: cache moved up + passed to start_checked + AppState field)
    - crates/rc-agent/src/remote_ops.rs (+230 -25: middleware rewrite, from_fn_with_state wiring, no-default-features variants, 3 test helpers + 1 new test)
    - crates/rc-agent/src/ws_handler.rs (+14 -3: 2x analyze_crash cfg-gated arg + csv_lap_fallback rewire)
decisions:
  - "W4 state-shape = Option (a) sub-router with State<MeshKeyCache>. No pre-existing OuterState/AppState axum struct existed in remote_ops (every Router was `Router::new()` with unit state), so localizing State to the protected sub-router is cleaner than introducing FromRef plumbing."
  - "`analyze_crash` takes the cache as a function parameter (not a struct field on AiDebuggerConfig). AiDebuggerConfig derives Deserialize/Default; adding a non-Deserialize Arc field there would require manual impls and pollute the config schema. Parameter threading touches 4 call sites which is acceptable churn."
  - "ws_handler csv_lap_fallback resolves the key INSIDE the tokio::spawn, not before. The cache Arc clones cleanly into the async block; the helper push_csv_fallback keeps its String-param signature (minimizes downstream diff)."
  - "Feature-gating applied on BOTH the middleware and the start* functions: http-client ON = cache-based, no-default-features = env-only. Ensures the CI no-default-features check stays compilable (pre-existing unrelated errors in mma_engine/tier_engine/openrouter are out of scope)."
  - "S10 test constructs router via a new test_router_full_with_cache helper that pre-populates the cache with a known value. The other 7 existing tests use the plain (empty-cache) builders so they continue to exercise the env-fallback path unchanged."
  - "W5 fix scope: only the ai_debugger Tier 0 path had a 403 to log distinctly (remote_ops middleware doesn't call out to the server; ws_handler csv_lap_fallback has its own retry logic in push_csv_fallback). Plan 02 already added the W5 log for the fetch_from_server path — plan 04 extends the pattern to the only other consumer that makes an authenticated GET."
metrics:
  duration_seconds: 2100
  duration_human: "~35m"
  tasks_completed: 4
  tasks_total: 4
  files_modified: 6
  tests_added: 1  # S10 test_service_key_cache_wins_over_env
  tests_passing: 103  # 19 remote_ops + 10 mesh_key_cache + 64 ai_debugger + 10 ws_handler
  completed_date: 2026-04-18
---

# Phase 413 Plan 04: rc-agent MeshKeyCache Consumer Rewire Summary

Consumer-rewire of the Option Z mesh service-key cache. Three production call
sites that used to read `std::env::var("RCAGENT_SERVICE_KEY")` now read via
`mesh_key_cache::get_key_or_env(&cache)`. The cache wins; env fallback is
preserved for test compatibility and the pre-first-fetch boot window. Closes
the last gap in the Option Z migration: rc-agent is now cache-first end-to-end.

Gap 4 (pod HKLM key ≠ server TOML key, silent 401 fleet-wide since MMA-v29) is
structurally unreachable in http-client builds — there is only one source of
truth (`racecontrol.toml` → `/pods/mesh-service-key` → `MeshKeyCache` → consumer).

## What Shipped

### Task 1 — ai_debugger::check_audit_known_issues

Before (line 779 of ai_debugger.rs):
```rust
async fn check_audit_known_issues(_config: &AiDebuggerConfig, error_context: &str) -> Option<String> {
    let service_key = std::env::var("RCAGENT_SERVICE_KEY").unwrap_or_default();
    if service_key.is_empty() {
        return None;
    }
    // ... unchanged HTTP call + JSON parse
    match client.get(&search_url).header("X-Service-Key", &service_key).send().await {
        Ok(resp) if resp.status().is_success() => { /* parse body */ }
        _ => None, // silent 403
    }
}
```

After:
```rust
#[cfg(feature = "http-client")]
async fn check_audit_known_issues(
    _config: &AiDebuggerConfig,
    error_context: &str,
    cache: &crate::mesh_key_cache::MeshKeyCache,
) -> Option<String> {
    let Some(service_key) = crate::mesh_key_cache::get_key_or_env(cache).await else {
        return None; // No key in cache AND no env fallback
    };
    // ... unchanged HTTP call + JSON parse

    match client.get(&search_url).header("X-Service-Key", &service_key).send().await {
        Ok(resp) if resp.status().is_success() => { /* parse body */ }
        Ok(resp) if resp.status() == reqwest::StatusCode::FORBIDDEN => {
            // W5: distinct warn line — stale key after rotation observable
            tracing::warn!(
                target: LOG_TARGET,
                status = 403, url = %search_url,
                "Tier 0 mesh oracle rejected service key (403 FORBIDDEN) — \
                 cache key may be stale or server rotated. Last-known-good preserved; \
                 next periodic_refetch will correct."
            );
            None
        }
        Ok(resp) => {
            tracing::debug!(target: LOG_TARGET, status = resp.status().as_u16(),
                "Tier 0 mesh oracle returned non-2xx (not 403) — transient");
            None
        }
        Err(e) => {
            tracing::debug!(target: LOG_TARGET, error = %e,
                "Tier 0 mesh oracle network error — transient");
            None
        }
    }
}

// No-default-features variant (Tier 0 unavailable):
#[cfg(not(feature = "http-client"))]
async fn check_audit_known_issues(_config: &AiDebuggerConfig, _error_context: &str) -> Option<String> {
    None
}
```

`analyze_crash` gains a feature-gated `mesh_key_cache: MeshKeyCache` parameter
and passes a `&cache` reference into `check_audit_known_issues`. All 4 call
sites updated:
- `event_loop.rs:1114` — post-game-exit analysis
- `event_loop.rs:1405` — stopping-state analysis
- `ws_handler.rs:853` — AC launch success path
- `ws_handler.rs:897` — AC launch failure path

Each call site now passes:
```rust
#[cfg(feature = "http-client")]
state.mesh_key_cache.clone(),
```

### Task 2 — remote_ops::require_service_key (W4 state-shape = Option (a))

**W4 decision: sub-router with state = `MeshKeyCache`.** Rationale from codebase
grep:
- `grep -n "struct AppState\|pub struct.*State.*{" crates/rc-agent/src/remote_ops.rs` → zero hits
- Every router was built with `Router::new()` (unit state) — no pre-existing state struct to extend with FromRef
- Localizing State to the protected sub-router keeps the outer Router plain and the cache scoped to exactly the routes that need it

Middleware signature change:
```rust
// Before
async fn require_service_key(
    req: axum::extract::Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let expected = std::env::var("RCAGENT_SERVICE_KEY").unwrap_or_default();
    // ... permissive-mode warn + ct_eq compare
}

// After
#[cfg(feature = "http-client")]
async fn require_service_key(
    State(cache): State<crate::mesh_key_cache::MeshKeyCache>,
    req: axum::extract::Request,
    next: middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let expected_opt = crate::mesh_key_cache::get_key_or_env(&cache).await;
    let Some(expected) = expected_opt else {
        // MMA-P1 SECURITY warn preserved verbatim
        // ... permissive-mode pass-through
    };
    // ... ct_eq compare unchanged
}
```

Sub-router wiring (applied identically in `start`, `start_checked`, `start_checked_tls_inner`):
```rust
let protected_routes = Router::new()
    .route("/info", get(info))
    // ... other protected routes
    .layer(axum::middleware::from_fn_with_state(
        cache.clone(),
        require_service_key,
    ))
    .with_state(cache);  // sub-router state = MeshKeyCache
```

`start`, `start_checked`, and `start_checked_with_tls` now accept a
`cache: MeshKeyCache` parameter. Every code path (plain HTTP, TLS, sub-router
merging) wires the cache into the middleware identically.

**S10 regression test** (new — `test_service_key_cache_wins_over_env`):
```rust
#[cfg(feature = "http-client")]
#[tokio::test]
#[serial]
async fn test_service_key_cache_wins_over_env() {
    unsafe { std::env::set_var("RCAGENT_SERVICE_KEY", "from-env"); }
    let app = test_router_full_with_cache(Some("from-cache")).await;

    // Cache value → 200
    let resp_good = app.clone().oneshot(
        req_with_header("X-Service-Key", "from-cache")
    ).await.unwrap();
    assert!(resp_good.status().is_success(), "cache value should authenticate");

    // Env value → 401 (cache wins; env is ignored)
    let resp_bad = app.oneshot(
        req_with_header("X-Service-Key", "from-env")
    ).await.unwrap();
    assert_eq!(resp_bad.status(), StatusCode::UNAUTHORIZED,
        "env value should be IGNORED when cache has a value");

    unsafe { std::env::remove_var("RCAGENT_SERVICE_KEY"); }
}
```

Legacy tests preserved: `test_router()` and `test_router_full()` construct an
empty cache + attach it via the same `from_fn_with_state + with_state` pattern,
so `get_key_or_env` falls back to env → all 7 existing service-key tests
continue to pass with zero behavioral change.

### Task 3 — ws_handler csv_lap_fallback

Before (line 431):
```rust
// Service key: pods use RCAGENT_SERVICE_KEY env var
let service_key = std::env::var("RCAGENT_SERVICE_KEY").unwrap_or_default();
let sid = billing_session_id.clone();
tokio::spawn(async move {
    crate::csv_lap_fallback::push_csv_fallback(server_http_base, service_key, sid).await
});
```

After:
```rust
// Phase 413 Plan 04: service key from Option Z cache, env fallback for tests.
let cache_clone = state.mesh_key_cache.clone();
let sid = billing_session_id.clone();
tokio::spawn(async move {
    let service_key = crate::mesh_key_cache::get_key_or_env(&cache_clone)
        .await
        .unwrap_or_default();
    crate::csv_lap_fallback::push_csv_fallback(server_http_base, service_key, sid).await
});
```

The key is resolved INSIDE the spawn (not before) — this avoids a pointless
.await on the cache read during session-end processing, which is a latency-
sensitive path. `push_csv_fallback` keeps its `String`-parameter signature
(zero downstream changes to the fallback helper or its tests).

### Task 4 — Fleet-wide verification sweep

Final `grep -rn 'std::env::var("RCAGENT_SERVICE_KEY")' crates/ 2>/dev/null`
after the 3 commits:

```
crates/rc-agent/src/mesh_key_cache.rs:137   ← documented env fallback in get_key_or_env
crates/rc-agent/src/remote_ops.rs:220       ← #[cfg(not(feature = "http-client"))] variant
                                               (env-only by design for no-default-features builds)
```

**Zero production code paths** in http-client builds read the env directly.
Only allowed occurrences are inside `get_key_or_env` (the fallback contract)
and the no-default-features variant of `require_service_key` (intentionally
env-only — no cache module available in that build configuration).

### Scaffolding — AppState + main.rs

`AppState` gains:
```rust
/// Phase 413 Plan 04: shared Option Z mesh service-key cache.
#[cfg(feature = "http-client")]
pub(crate) mesh_key_cache: crate::mesh_key_cache::MeshKeyCache,
```

Populated at construction:
```rust
let mut state = AppState {
    // ...
    #[cfg(feature = "http-client")]
    mesh_key_cache: mesh_key_cache.clone(),
};
```

`main.rs` change: the `mesh_key_cache = new_cache()` binding moved from line 1591
up to ~line 960 (above the `remote_ops::start_checked` call) so the cache can be
passed into the sub-router. The periodic_refetch block lower in main.rs is
unchanged — it still uses the same variable. The `#[allow(unused_variables)]`
breadcrumb that Plan 03 left for Plan 04 is removed (the cache is now read by
consumers, so the compiler won't flag it).

## Commits

| Task           | Commit     | Files                                                                                                                         |
| -------------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------- |
| 1 + 2 + Scaff. | `51356322` | ai_debugger.rs, app_state.rs, event_loop.rs, main.rs, remote_ops.rs, ws_handler.rs (analyze_crash call sites)                 |
| 3              | `34e13516` | ws_handler.rs (csv_lap_fallback push)                                                                                         |

Tasks 1+2 are committed together because main.rs interlocks them: it both
instantiates the cache AND passes it into `remote_ops::start_checked` (Task 2)
AND puts it in AppState for ai_debugger consumption (Task 1). Splitting them
would leave an intermediate broken state. Task 3 is truly independent (only
touches ws_handler csv_lap_fallback) and committed separately.

## Verification

```
cargo build --release --bin rc-agent                             → 0 errors (58s)
cargo build --release --bin racecontrol                          → 0 errors (4m 42s)
cargo test -p rc-agent-crate --bin rc-agent remote_ops           → 19 passed (incl. S10 test_service_key_cache_wins_over_env)
cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache       → 10 passed (no regression from Plans 02+03)
cargo test -p rc-agent-crate --bin rc-agent ai_debugger          → 64 passed
cargo test -p rc-agent-crate --bin rc-agent ws_handler           → 10 passed
```

**Total: 103 tests passing** across the 4 modules touched by this plan.

### Acceptance criteria grep counts (HEAD `34e13516`)

| Check                                                                          | Expected | Actual |
| ------------------------------------------------------------------------------ | -------- | ------ |
| `grep -c 'std::env::var("RCAGENT_SERVICE_KEY")' ai_debugger.rs`                | 0        | 0      |
| `grep -c 'get_key_or_env' ai_debugger.rs`                                      | ≥ 1      | 1      |
| `grep -cE 'warn!.*(403\|FORBIDDEN\|forbidden)' ai_debugger.rs`                 | ≥ 1      | 1      |
| `grep -c 'std::env::var("RCAGENT_SERVICE_KEY")' ws_handler.rs`                 | 0        | 0      |
| `grep -c 'get_key_or_env' ws_handler.rs`                                       | ≥ 1      | 1      |
| `grep -c 'from_fn_with_state' remote_ops.rs`                                   | ≥ 1      | 7      |
| `grep -c 'with_state' remote_ops.rs`                                           | ≥ 1      | 14     |
| `grep -c 'get_key_or_env' remote_ops.rs`                                       | ≥ 1      | 4      |
| `grep -c 'SECURITY: RCAGENT_SERVICE_KEY is not set' remote_ops.rs`             | 1 (preserved) | 2 (http-client + no-default-features variants) |
| `grep -cE 'ct_eq\|ConstantTimeEq' remote_ops.rs`                               | ≥ 1      | 3      |

The `SECURITY:` count went from 1 to 2 because we now have two middleware
variants (cfg-gated). Both preserve the verbatim log text — the warn fires
identically in either compile mode.

## Deviations from Plan

### Rule 3 (Blocking issue) — ws_handler interleaved Task 1 and Task 3

**Found during:** Task 3 commit preparation.
**Issue:** ws_handler.rs has 2 `analyze_crash` call sites (Task 1) at lines 853
+ 897 AND the csv_lap_fallback push (Task 3) at line 431. Committing all 4
tasks atomically at file-level would bundle Tasks 1+2+3 into a single commit.
**Fix:** Temporarily reverted the csv_lap_fallback change, committed Tasks 1+2
with the analyze_crash portions of ws_handler.rs, then re-applied and
committed Task 3 in its own commit.
**Impact:** Clean per-task commit boundaries maintained. Two commits total
(`51356322` for Task 1+2+scaff, `34e13516` for Task 3). Task 4 = verification
sweep is embedded as a grep run, no commit needed.

### Rule 3 (Blocking issue) — analyze_crash also used in event_loop.rs

**Found during:** Task 1 signature change.
**Issue:** Plan's read_first pointed to `check_audit_known_issues` + main.rs,
but `analyze_crash` (the function that calls `check_audit_known_issues`) is
the actual function the call sites invoke. There are 4 call sites total (2
in event_loop.rs, 2 in ws_handler.rs), not just ai_debugger.rs + main.rs as
the plan implied.
**Fix:** Updated all 4 call sites to pass `state.mesh_key_cache.clone()` under
the same `#[cfg(feature = "http-client")]` guard as the function parameter.
**Impact:** Diff is slightly larger than anticipated but structurally clean —
every call site is accounted for. No main.rs call site for analyze_crash exists;
the plan's mention of "call sites in main.rs" was a planning approximation.

### Rule 2 (Auto-add missing functionality) — no-default-features compile parity

**Found during:** Task 2 remote_ops middleware rewrite.
**Issue:** `MeshKeyCache` is feature-gated on `http-client`. A naive rewrite
of `require_service_key` to take `State<MeshKeyCache>` would break the
`--no-default-features` build path (CI verification profile). The baseline
no-default-features build was already broken by 33 pre-existing errors in
`mma_engine.rs`, `tier_engine.rs`, `openrouter.rs` (unrelated out-of-scope),
but we shouldn't worsen the count in our touched files.
**Fix:** Added `#[cfg(not(feature = "http-client"))]` variants for
`require_service_key`, `start`, `start_checked`, and `start_checked_with_tls`.
The no-default-features variants are env-only (original behavior preserved),
delegate TLS to plain HTTP, and compile cleanly against the non-http-client
feature set.
**Impact:** remote_ops.rs contributes 0 new errors to the no-default-features
build. Pre-existing 33 errors in unrelated files remain (Rule: out of scope,
documented here for traceability).

### No other deviations

Plan's W4 (state-shape), W5 (403 logging), and S10 (cache-wins test) fixes
were implemented exactly as specified. The chosen W4 option (a) is explicitly
documented in a comment block at the top of the middleware function. The S10
test is annotated `#[serial]` to avoid env-var race with the other service-
key tests.

## Known Stubs

None. All 3 production consumer sites are now fully cache-first. The env-
var fallback in `get_key_or_env` is documented behavior (test compatibility)
and not a stub. The no-default-features middleware variant is intentional
— `--no-default-features` builds are CI-only and never deployed to pods.

## What's Next

**Plan 413-09 (MMA audit)** will cross-model-review this consumer rewire for:
- Any missed production env-read site
- Sub-router state-shape correctness (Option (a) vs (b) debate)
- Lock-across-await violations in `get_key_or_env` read path
- Race conditions between periodic_refetch (Plan 03) writing the cache and
  consumers reading it (RwLock ordering in Plan 02 should prevent this, but
  MMA may challenge)
- Whether the 403-warn-on-Tier-0 path (Plan 04 W5 extension) deserves a
  matching 403-warn on the CSV push path (push_csv_fallback currently only
  logs generic "failed after retries")

**Plan 413-10 (runtime verification)** will:
1. Deploy rc-agent with Plans 02+03+04 to one canary pod
2. Grep `rc-agent.log` for the boot + periodic_refetch + Tier 0 log lines
3. Verify `X-Service-Key` header matches server's `pods.sentry_service_key`
4. Confirm server-side key rotation (edit `racecontrol.toml` → restart)
   propagates to pods within 300s without any pod-side action (the Option Z
   promise)

**Plan 413-11 (fleet deploy)** will roll this out to all 8 pods + POS.

## Deferred Issues

### `rc-sentry-ai` release linker failure (pre-existing, out of scope)

`cargo build --release --workspace` fails at link time on `rc-sentry-ai`
with LNK4286 (MSVCRT / libucrt conflict via ort + onnx runtime). This is a
pre-existing failure unrelated to Plan 04 — `git stash && cargo build` on a
clean HEAD shows the same error. Out of scope per scope boundary rule
(only fix issues DIRECTLY caused by this plan's changes). Logged to
`.planning/phases/413.../deferred-items.md` for future triage.

### `--no-default-features` pre-existing errors (33, unrelated)

`cargo check --no-default-features -p rc-agent-crate --bin rc-agent` fails
with 33 E0433 errors in mma_engine.rs, tier_engine.rs, openrouter.rs —
these modules use reqwest directly without feature-gating. Pre-existing,
unrelated to Plan 04. Baseline verified by `git stash && cargo check
--no-default-features` showing identical 33 errors. Out of scope.

## Self-Check: PASSED

- [x] `crates/rc-agent/src/ai_debugger.rs` modified (verified: commit `51356322` + `git show --stat`)
- [x] `crates/rc-agent/src/app_state.rs` modified (`51356322`)
- [x] `crates/rc-agent/src/event_loop.rs` modified (`51356322`)
- [x] `crates/rc-agent/src/main.rs` modified (`51356322`)
- [x] `crates/rc-agent/src/remote_ops.rs` modified (`51356322`)
- [x] `crates/rc-agent/src/ws_handler.rs` modified (both `51356322` + `34e13516`)
- [x] `cargo build --release --bin rc-agent` exits 0
- [x] `cargo build --release --bin racecontrol` exits 0
- [x] 19 remote_ops tests pass (7 legacy + S10 new + 11 unrelated preserved)
- [x] 10 mesh_key_cache tests pass (Plan 02 baseline preserved)
- [x] 64 ai_debugger tests pass (no regression)
- [x] 10 ws_handler tests pass (no regression)
- [x] `grep -n 'std::env::var("RCAGENT_SERVICE_KEY")' ai_debugger.rs` → 0 hits
- [x] `grep -n 'std::env::var("RCAGENT_SERVICE_KEY")' ws_handler.rs` → 0 hits
- [x] `grep -n 'std::env::var("RCAGENT_SERVICE_KEY")' remote_ops.rs` → 1 hit in `#[cfg(not(feature = "http-client"))]` variant (acceptable per plan)
- [x] W4 state-shape documented in middleware comment block (Option (a) sub-router)
- [x] W5 warn-on-403 in ai_debugger Tier 0 path (grep -E 'warn!.*403' → 1 hit)
- [x] S10 new test `test_service_key_cache_wins_over_env` present + passing
- [x] No `.unwrap()` added in any production path (middleware uses the Option pattern explicitly)
- [x] Constant-time compare preserved (3 ct_eq occurrences in remote_ops.rs)
- [x] MMA-P1 SECURITY warn log preserved (2 occurrences — one per cfg variant, both verbatim)
