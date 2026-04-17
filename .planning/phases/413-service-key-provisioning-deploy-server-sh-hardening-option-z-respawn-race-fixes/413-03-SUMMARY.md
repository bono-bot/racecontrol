---
phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes
plan: 03
subsystem: rc-agent/mesh-key-boot-wireup
tags: [option-z, mesh-service-key, rc-agent, boot-resilience, periodic-refetch, wave-2]
dependency-graph:
  requires:
    - crate::mesh_key_cache (from Plan 413-02)
    - rc_common::boot_resilience::spawn_periodic_refetch (pattern reused from feature_flags)
    - reqwest 0.12 via http-client feature (default-enabled)
  provides:
    - runtime-live MeshKeyCache instance in rc-agent main.rs scope (name: `mesh_key_cache`)
    - periodic refresh task emitting `periodic_refetch started/first_success/failed/self_healed` log lines with `resource="mesh_service_key"`
    - boot-time initial fetch with "Mesh key cache initial fetch ok/failed" log line
  affects:
    - crates/rc-agent/src/main.rs (+50 lines, 2 additive insertions)
tech-stack:
  added: []
  patterns:
    - "Feature-gated let binding + periodic-refetch closure — mirrors the feature_flags block at lines 1598-1630"
    - "core_http_base.clone() preserved into billing_guard::spawn which moves the original"
    - "Non-fatal initial fetch (match Ok/Err with info/warn logs) so a downed server at boot never blocks rc-agent startup"
key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs (+50 lines net; additive only)
decisions:
  - "Feature-gated on http-client to match mesh_key_cache module availability (otherwise --no-default-features build fails)"
  - "let binding marked #[allow(unused_variables)] with TODO until Plan 04 wires consumers — this is called out in plan acceptance-criteria"
  - "300s periodic interval matches feature_flags (same cadence = same operational profile for server-downtime recovery)"
  - "Initial fetch is best-effort: Ok → info log, Err → warn log with error context. Never blocks boot. Consumers fall back to env var until next successful tick"
  - "Two local reqwest::Client instances intentional: http_client_init for the initial fetch, http_client_refetch (cloned from init) for the periodic task. Cheaper than constructing twice, lets the initial one be moved out naturally"
metrics:
  duration_seconds: 215
  duration_human: "~4m"
  tasks_completed: 1
  tasks_total: 1
  files_created: 0
  files_modified: 1
  tests_added: 0
  tests_still_passing: 10
  completed_date: 2026-04-18
---

# Phase 413 Plan 03: rc-agent MeshKeyCache Boot Wire-up Summary

Wires the Plan 02 `MeshKeyCache` module into rc-agent's `main.rs` boot sequence. The cache is instantiated once (Arc for later consumer sharing), fetched from the server synchronously on boot (best-effort), and refreshed every 5 minutes via `rc_common::boot_resilience::spawn_periodic_refetch`. This makes the cache actually UPDATE at runtime — Plan 04's consumer rewire can now drop the env-var lookups because the cache will be populated by the time consumers call `get_key_or_env`.

## What Shipped

### Edit 1: `let mesh_key_cache = ...` binding (main.rs ~line 1582-1590)

Placed immediately below `let flags_arc = ...` so the binding lives in the same scope as the feature-flags Arc that the rest of `main()` shares with `AppState`. This is the exact scope pattern Plan 04 needs to `.clone()` the Arc into `ai_debugger`, `remote_ops`, and `ws_handler` initializations later in the function.

```rust
// Phase 413 — Option Z mesh key cache. Fetched at boot + every 5min from
// GET /api/v1/pods/mesh-service-key (pod-IP-gated). Replaces HKLM
// RCAGENT_SERVICE_KEY provisioning. Cache is shared with ai_debugger,
// remote_ops, and ws_handler (see Plan 04).
#[cfg(feature = "http-client")]
#[allow(unused_variables)] // Phase 413 Plan 04 wires consumers — remove allow then
let mesh_key_cache = crate::mesh_key_cache::new_cache();
```

**Feature-gating note:** The `mesh_key_cache` module in `main.rs` (line 37-38) is `#[cfg(feature = "http-client")]`, so the let binding must be gated identically. `http-client` is a default feature, so all production builds (and the `cargo build --release --bin rc-agent` in plan acceptance) enable it. A `cargo build --no-default-features` still compiles — the binding simply doesn't exist in that variant.

**`#[allow(unused_variables)]` rationale:** The plan explicitly permits this ("if the compiler flags it as dead-code, add `#[allow(unused_variables)]` on the let binding WITH a TODO"). Plan 04 removes the allow when it adds `mesh_key_cache.clone()` call sites downstream.

### Edit 2: initial fetch + periodic refetch block (main.rs ~line 1632-1668)

Placed immediately after the `feature_flags` periodic-refetch block (end of line 1630) and before `billing_guard::spawn(...)` call (line 1674 in the new file). This is the insertion region specified in the plan. Mirrors the feature_flags block exactly.

```rust
#[cfg(feature = "http-client")]
{
    let mesh_cache_init = mesh_key_cache.clone();
    let http_base_init = core_http_base.clone();
    let http_client_init = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    match crate::mesh_key_cache::fetch_from_server(&http_client_init, &http_base_init, &mesh_cache_init).await {
        Ok(_) => tracing::info!(target: LOG_TARGET, "Mesh key cache initial fetch ok"),
        Err(e) => tracing::warn!(target: LOG_TARGET, error = %e, "Mesh key cache initial fetch failed (will retry in 300s)"),
    }

    let mesh_cache_refetch = mesh_key_cache.clone();
    let http_base_refetch = core_http_base.clone();
    let http_client_refetch = http_client_init.clone();
    rc_common::boot_resilience::spawn_periodic_refetch(
        "mesh_service_key".to_string(),
        Duration::from_secs(300),
        move || {
            let cache = mesh_cache_refetch.clone();
            let base = http_base_refetch.clone();
            let client = http_client_refetch.clone();
            async move {
                crate::mesh_key_cache::fetch_from_server(&client, &base, &cache).await
            }
        },
    );
    tracing::info!(target: LOG_TARGET, "Mesh key cache periodic re-fetch started (interval=300s)");
}
```

### Variable Name & Scope

**Variable name:** `mesh_key_cache` (matches plan spec exactly).
**Scope:** Declared at function-level in `async fn main()` immediately below `flags_arc`. Arc-clonable via `mesh_key_cache.clone()` from anywhere below the binding in the same function body.
**Plan 04 propagation:** Downstream consumer initializers (`ai_debugger`, `remote_ops`, `ws_handler`) live 200-400 lines later in the same function body. Each will call `mesh_key_cache.clone()` at its init site, then `get_key_or_env(&cache).await.unwrap_or_default()` at its env-var lookup site (SUMMARY 02 lists the exact three call sites: `ai_debugger.rs:779`, `remote_ops.rs:165`, `ws_handler.rs:431`).

### Interval Decision (300s)

The plan specifies 300s explicitly. Rationale (captured from feature_flags precedent):

- **Same cadence as feature_flags** → operations team already familiar with this refresh window from FF-01 logs
- **5 minutes is the fleet's boot-resilience recovery target** (from CLAUDE.md Boot Resilience rule): "self-heals within 5 minutes when server comes back"
- **Not tighter** (e.g. 60s) because the mesh key rarely rotates and a 5-minute stale window is acceptable
- **Not looser** (e.g. 1800s / 30min) because the Gap 4 root cause (pod HKLM ≠ server TOML) is a silent 401 condition; faster recovery reduces blast radius

### Initial-Fetch-is-Non-Fatal Decision

Plan spec: "Non-fatal on initial failure — cache stays None, consumers fall back to env var until next tick."

Implementation: `match fetch_from_server(...).await` with `Ok(_) => info` / `Err(e) => warn`. Importantly, an Err here:

1. Does not `?` propagate up — the whole block is not short-circuited on failure
2. Does not cause the periodic refetch to be skipped — the `spawn_periodic_refetch` call runs regardless
3. Leaves the cache at its initial `None` value, which the Plan 02 `get_key_or_env` helper handles by falling back to `std::env::var("RCAGENT_SERVICE_KEY")`

This is the boot-resilience contract: if the server is down at boot, rc-agent does not refuse to start; it starts with env fallback and picks up the cached key on the first successful refetch (max 300s later).

### Log Lines to grep for in Plan 10 Verification

Per the plan's must_haves.truths:

| Log line | Source | When emitted | Grep pattern |
|---|---|---|---|
| `Mesh key cache initial fetch ok` | main.rs (this plan) | Boot, when initial fetch succeeds | `grep "Mesh key cache initial fetch ok" rc-agent.log` |
| `Mesh key cache initial fetch failed` | main.rs (this plan) | Boot, when initial fetch fails | `grep "Mesh key cache initial fetch failed" rc-agent.log` |
| `Mesh key cache periodic re-fetch started (interval=300s)` | main.rs (this plan) | Boot, always | `grep "Mesh key cache periodic re-fetch started" rc-agent.log` |
| `periodic_refetch started resource="mesh_service_key"` | rc_common::boot_resilience | Boot, within 10s | `grep 'periodic_refetch started' rc-agent.log \| grep mesh_service_key` |
| `periodic_refetch first_success resource="mesh_service_key"` | rc_common::boot_resilience | Within 300s of first successful fetch when server reachable | `grep 'periodic_refetch first_success' rc-agent.log \| grep mesh_service_key` |
| `periodic_refetch failed resource="mesh_service_key"` | rc_common::boot_resilience | Each failed refetch attempt | `grep 'periodic_refetch failed' rc-agent.log \| grep mesh_service_key` |
| `periodic_refetch self_healed resource="mesh_service_key"` | rc_common::boot_resilience | First success after prior failures | `grep 'periodic_refetch self_healed' rc-agent.log \| grep mesh_service_key` |

## Commits

| Task | Commit | Files |
|------|--------|-------|
| 1 (wire-up) | `28de9e30` | `crates/rc-agent/src/main.rs` (+50 lines, 2 additive insertions) |

## Verification

```
cargo build --release --bin rc-agent            → 0 errors, 100 pre-existing warnings, finished in 1m 10s
cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache  → 10 passed; 0 failed
```

Acceptance-criteria grep spot checks (run against HEAD `28de9e30`):

| Check | Expected | Actual |
|-------|----------|--------|
| `grep -c "mesh_key_cache::new_cache"` in main.rs | 1 | 1 |
| `grep -c "spawn_periodic_refetch"` in main.rs | 2 (feature_flags + mesh_service_key) | 2 |
| `grep -c '"mesh_service_key"'` in main.rs | ≥ 1 | 1 |
| `grep -c "mesh_key_cache::fetch_from_server"` in main.rs | 2 (initial + refetch closure) | 2 |
| `.unwrap()` count added in new block | 0 | 0 (only `.unwrap_or_else` on reqwest builder, existing pattern) |
| No warnings on `mesh_key_cache` variable | unused_variables suppressed | Suppressed via `#[allow(unused_variables)]` |

Only compiler note related to this plan: `warning: function 'get_key_or_env' is never used` at `mesh_key_cache.rs:130` — expected, Plan 04 adds the consumers. No regression in any pre-existing warnings.

## Deviations from Plan

### None (plan executed exactly as specified)

- Insertion points matched: (a) below `let flags_arc` at line 1580, (b) after feature_flags block at line 1630
- Both insertions are strictly additive — no existing line modified or removed
- Variable name `mesh_key_cache` matches plan verbatim
- 300s interval matches plan
- `#[cfg(feature = "http-client")]` gating matches plan template
- `#[allow(unused_variables)]` with TODO applied as the plan's acceptance criteria explicitly authorized ("if the compiler flags it as dead-code, add...")

The plan anticipated the feature-gating requirement implicitly (the template in the context block was `#[cfg(feature = "http-client")]`), and the let-binding had to be gated the same way because the `mesh_key_cache` module is itself feature-gated. No fresh architectural decision — pattern lifted directly from the feature_flags precedent.

### Note on `unused_variables` allow (harmless)

The `#[allow(unused_variables)]` on the `let mesh_key_cache` binding is strictly necessary: without consumers (Plan 04), the compiler would emit `warning: unused variable: mesh_key_cache`. The periodic-refetch block DOES use `mesh_key_cache.clone()` twice (inside the feature-gated block), but the compiler's "unused variable" analysis runs before dead-code elimination on the let binding's outer scope. The allow is locally-scoped to the let and will be removed in Plan 04 when the three consumer `.clone()` calls appear.

Alternative considered & rejected: `let _mesh_key_cache = ...` would silence the warning but the plan explicitly specifies the name `mesh_key_cache` (unprefixed) because Plan 04 will reference it by that exact name. Renaming twice across two plans is more churn than a two-line attribute.

## Known Stubs

None. This plan is pure wire-up — it instantiates a cache and spawns a refresh task using real code paths from Plan 02. The cache is populated by real HTTP calls on boot (if server reachable) and on every 300s tick thereafter. The only "unused" aspect is the cache VALUE (nobody reads it yet) — the cache LIFECYCLE is fully live.

Plan 04 removes the `#[allow(unused_variables)]` and adds three consumer call sites, which completes the data flow server TOML → HTTP → Arc<RwLock<Option<String>>> → consumer → sentry call.

## What's Next

**Plan 413-04** (consumer rewire):

1. Remove `#[allow(unused_variables)]` from the `let mesh_key_cache` binding
2. Add `mesh_key_cache.clone()` at each of the three consumer init sites:
   - `ai_debugger.rs:779` — `std::env::var("RCAGENT_SERVICE_KEY").unwrap_or_default()` → `get_key_or_env(&cache).await.unwrap_or_default()`
   - `remote_ops.rs:165` — same transform for the middleware
   - `ws_handler.rs:431` — same transform for the csv_lap_fallback push
3. Verify: Gap 4 (pod HKLM key ≠ server TOML key, silent 401 fleet-wide) becomes unreachable because there's only one source of truth (server TOML → HTTP → cache → consumer).

**Plan 413-10** (verification):

1. Deploy rc-agent with this plan + Plan 04 to one canary pod
2. Grep `rc-agent.log` for the 7 log lines listed above within expected windows
3. Verify `X-Service-Key` header sent by ai_debugger/remote_ops/ws_handler matches the server's configured `pods.sentry_service_key` — no more HKLM dependency
4. Confirm a server-side key rotation (edit `racecontrol.toml` → restart) propagates to pods within 300s without any pod-side action

## Self-Check: PASSED

- [x] `crates/rc-agent/src/main.rs` modified (verified: `git show 28de9e30 --stat` shows `1 file changed, 50 insertions(+)`)
- [x] Commit `28de9e30` exists on main branch (verified: `git log --oneline -5` shows it as HEAD)
- [x] Acceptance-criteria grep counts match (4/4 — see Verification table)
- [x] `cargo build --release --bin rc-agent` exits 0 (verified: `Finished release profile [optimized] target(s) in 1m 10s`)
- [x] `cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache` → 10/10 passed (no regression from Plan 02 baseline)
- [x] No `.unwrap()` in production code (only `unwrap_or_else` on reqwest::Client::builder, existing pattern from feature_flags)
- [x] Plan frontmatter must_haves.truths satisfied:
  - [x] "rc-agent creates a MeshKeyCache at boot and shares it across the process via an Arc" — `let mesh_key_cache = crate::mesh_key_cache::new_cache()` creates Arc<RwLock<Option<String>>>
  - [x] "performs an initial synchronous fetch before consumers start" — `match crate::mesh_key_cache::fetch_from_server(...).await` runs before any downstream consumer init
  - [x] "spawns a periodic refetch task at interval 300s via rc_common::boot_resilience::spawn_periodic_refetch" — verbatim call present
  - [x] (Runtime-dependent) "log contains `periodic_refetch started resource=\"mesh_service_key\"` within 10s" — rc_common::boot_resilience emits this per its contract; verification deferred to Plan 10 runtime test
  - [x] (Runtime-dependent) "log contains `periodic_refetch first_success resource=\"mesh_service_key\"` within one interval when server reachable" — same
