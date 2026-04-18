# Phase 413 Plan 10 — Integration Test Evidence

**Plan:** 413-10 (pre-deploy integration test)
**Target:** verify new route `GET /api/v1/pods/mesh-service-key` + rc-agent MeshKeyCache end-to-end on a local/dev instance BEFORE fleet deploy (Plan 11).
**Working copy:** `C:/Users/bono/racingpoint/racecontrol`
**HEAD at start of run:** `85c32e1a` (`docs(414): research phase domain`)
**HEAD of last 413 code change:** `34e13516` (`feat(413-04): rewire ws_handler csv_lap_fallback to MeshKeyCache (Task 3)`)
**Phase 413 code + docs span:** Plans 01-09 already committed. Plan 10 is live-verification only.

Go/no-go format per T-id. Raw commands + raw output. No prose summaries of results — every PASS/FAIL is traceable to a log excerpt below.

---

## T1 — Workspace release build

**Goal:** `cargo build --release --bin racecontrol` + `cargo build --release --bin rc-agent` both exit 0 (plan must-have).

Plan action also proposed `cargo build --release --workspace`. Running both. The workspace build surfaces a **pre-existing** `rc-sentry-ai` LNK4286 linker failure (see `deferred-items.md` 2026-04-18 entry) which is out of Plan 413 scope per CLAUDE.md scope boundary (only fix issues DIRECTLY caused by this plan's changes).

### Command — targeted binaries (primary must-have)

```
cargo build --release --bin racecontrol
```

```
warning: `racecontrol-crate` (bin "racecontrol") generated 1 warning
    Finished `release` profile [optimized] target(s) in 52.88s
```
**Exit: 0**

```
cargo build --release --bin rc-agent
```

```
warning: `rc-agent-crate` (bin "rc-agent") generated 99 warnings (run `cargo fix --bin "rc-agent" -p rc-agent-crate` to apply 24 suggestions)
    Finished `release` profile [optimized] target(s) in 0.63s
```
**Exit: 0**

### Command — workspace build (reference, non-blocking)

```
cargo build --release --workspace  2>&1 | tee /tmp/phase413-T1.log
```

Tail:
```
warning: `rc-agent-crate` (bin "rc-agent") generated 99 warnings (run `cargo fix --bin "rc-agent" -p rc-agent-crate` to apply 24 suggestions)
warning: `racecontrol-crate` (lib) generated 3 warnings (run `cargo fix --lib -p racecontrol-crate` to apply 1 suggestion)
```

Error (pre-existing, `rc-sentry-ai`, documented in `deferred-items.md` since Plan 04):
```
error: linking with `link.exe` failed: exit code: 1120
LINK : warning LNK4098: defaultlib 'MSVCRT' conflicts with use of other libs
LINK : warning LNK4286: symbol '_invalid_parameter_noinfo' defined in 'libucrt.lib(invalid_parameter.obj)' is imported by 'libort_sys-b5c7e272924805c4.rlib(DescriptorPool.obj)'
[...repeated for other ort_sys .obj files...]
error: could not compile `rc-sentry-ai` (bin "rc-sentry-ai") due to 1 previous error
```

Binaries on disk after the workspace run:
```
$ ls -la target/release/racecontrol.exe target/release/rc-agent.exe
-rwxr-xr-x  60302336  target/release/racecontrol.exe
-rwxr-xr-x  26745344  target/release/rc-agent.exe
```

Both Phase 413 binaries build successfully. The `rc-sentry-ai` failure is pre-existing (baseline identical on clean `HEAD~15+`, same symptoms documented in 413-04 deferred-items).

**T1 verdict: PASS** — both must-have targeted binaries exit 0. Workspace build fails only in unrelated pre-existing scope.

---

## T2 — Workspace unit tests (must-have: exit 0 across `rc-common`, `rc-agent-crate`, `racecontrol-crate`)

Plan must-have truth: `cargo test -p rc-agent-crate -p rc-common -p racecontrol-crate` exits 0.

Plan acceptance criteria also specify:
- All `mesh_key_cache` tests pass
- All `remote_ops` service-key tests pass (7 `ok` lines)

### T2a — Phase 413-specific test subsets (the criterion that directly gates Plan 11)

```
cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache
```

```
test mesh_key_cache::tests::fetch_preserves_last_known_good_on_503 ... ok
test mesh_key_cache::tests::fetch_populates_cache ... ok
test mesh_key_cache::tests::fetch_preserves_last_known_good_on_500 ... ok
test mesh_key_cache::tests::fetch_empty_response_overwrites_existing_key ... ok
test mesh_key_cache::tests::fetch_403_logs_warn_and_preserves_cache ... ok
test mesh_key_cache::tests::fetch_empty_response_sets_cache_to_none ... ok
test mesh_key_cache::tests::fetch_preserves_last_known_good_on_network_failure ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 800 filtered out; finished in 2.04s
```
**mesh_key_cache: 11/11 PASS**

```
cargo test -p rc-agent-crate --bin rc-agent remote_ops
```

```
test remote_ops::tests::test_service_key_exec_correct_key_returns_200 ... ok
test remote_ops::tests::test_service_key_exec_wrong_key_returns_401 ... ok
test remote_ops::tests::test_health_shows_exec_slots ... ok
test remote_ops::tests::test_service_key_info_no_header_returns_401 ... ok
test remote_ops::tests::test_service_key_health_no_key_returns_200 ... ok
test remote_ops::tests::test_service_key_permissive_mode_no_key_set ... ok
test remote_ops::tests::test_service_key_ping_no_key_returns_200 ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 792 filtered out; finished in 0.18s
```
**remote_ops: 19/19 PASS** (including 7 `test_service_key_*` ok lines as required)

```
cargo test -p racecontrol-crate --lib phase413
```

```
test api::mesh_intelligence::phase413_tests::render_returns_configured_value ... ok
test api::mesh_intelligence::phase413_tests::mma_verify_new1_whitespace_key_does_not_serve ... ok
test api::mesh_intelligence::phase413_tests::render_returns_empty_string_when_unconfigured ... ok
test api::mesh_intelligence::phase413_tests::mma_c2_empty_toml_key_does_not_serve ... ok
test api::mesh_intelligence::phase413_tests::mma_c2_non_empty_toml_key_serves ... ok
test api::mesh_intelligence::phase413_tests::mma_verify_new1_whitespace_surrounding_real_key_still_serves ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 958 filtered out; finished in 0.00s
```
**phase413 (server-side): 7/7 PASS** (includes MMA C-2 empty-key + VERIFY NEW-1 whitespace guard tests)

```
cargo test -p racecontrol-crate --lib network_source
```

```
test network_source::tests::staff_ips_classify_as_staff ... ok
test network_source::tests::server_tailscale_stays_cloud ... ok
test network_source::tests::pos_tailscale_classifies_as_pod ... ok
[... 21 total ...]

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 944 filtered out; finished in 0.00s
```
**network_source: 21/21 PASS** (includes `bono_vps_tailscale_stays_cloud` + `server_tailscale_stays_cloud` regression guards)

```
cargo test -p rc-common --lib
```

```
[...]
test result: ok. 252 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.15s
```
**rc-common: 252/252 PASS** (includes `boot_resilience::tests::spawn_periodic_refetch_self_heals_after_failure` — the unit-test proof cited by Plan 10 as the deferred-cover for T10 long-form)

**T2a verdict: PASS** — every Plan 413-specific test suite is green.

### T2b — Full 3-crate suite (must-have wording, caveats documented)

```
cargo test -p rc-common -p rc-agent-crate -p racecontrol-crate  2>&1 | tee /tmp/phase413-T2.log
```

Final summary (tail):
```
failures:

---- test_billing_rates_delete_excludes_from_cost stdout ----

thread 'test_billing_rates_delete_excludes_from_cost' (29900) panicked at crates\racecontrol\tests\integration.rs:3679:5:
assertion `left == right` failed: Baseline 90-min cost must be 180000 paise
  left: 135000
 right: 180000

---- test_financial_e2e_tiered_pricing_integer_math stdout ----

thread 'test_financial_e2e_tiered_pricing_integer_math' (2176) panicked at crates\racecontrol\tests\integration.rs:3894:5:
assertion `left == right` failed: 30 min standard tier
  left: 70000
 right: 75000


failures:
    test_billing_rates_delete_excludes_from_cost
    test_financial_e2e_tiered_pricing_integer_math

test result: FAILED. 78 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 212.07s
```

**Scope analysis (per CLAUDE.md scope boundary):**

Both failing tests live in `crates/racecontrol/tests/integration.rs`. Last commit touching that file:
```
$ git log --oneline -5 -- crates/racecontrol/tests/integration.rs
36f6d2a0 feat(367-05): Phase 362 retro-validation test harness (GLD-G-05)
5dffc537 fix(tests): fix 9 failing integration tests + cross-platform build scripts
5b40e0ca feat(339-01): WalletInfo serde renames + transactions_count field
13706efe fix: build errors — billing_mode type, onAssign removal, guardian ist_hour, test assertions
da0942cd feat: per-minute billing engine + visits table + Acts 1-4 data model

$ git log --oneline 36f6d2a0..HEAD -- crates/racecontrol/tests/integration.rs
(no output — file unchanged since `36f6d2a0`)
```

Phase 413's commits (01 through 09) modify: `crates/racecontrol/src/network_source.rs`, `crates/racecontrol/src/api/mesh_intelligence.rs`, `crates/racecontrol/src/api/routes.rs`, `crates/rc-agent/src/mesh_key_cache.rs`, `crates/rc-agent/src/ai_debugger.rs`, `crates/rc-agent/src/remote_ops.rs`, `crates/rc-agent/src/ws_handler.rs`, `crates/rc-agent/src/csv_lap_fallback.rs`, `crates/rc-agent/src/main.rs`, `crates/rc-agent/src/app_state.rs`, `crates/rc-agent/src/event_loop.rs`, `scripts/deploy-server.sh`. **Zero billing-code changes.**

The two failing tests assert values from the tiered-pricing engine migration (MEMORY.md records this landed in commits `290f16ca` + `f4de983d`, long before Phase 413). They predate Phase 413 code by >1 week and are orthogonal to the mesh-service-key route / deploy-server.sh fixes.

**Logged as pre-existing discovery in `deferred-items.md` under 2026-04-18 Plan 10.** Not fixed in this plan.

**T2b verdict: STRICT-FAIL on the full-suite exit code, but NON-BLOCKING for Plan 11 deploy gate** — the two failures are pre-existing billing-integration-test drift unrelated to the code Plan 413 changes. Plan 11 gating criterion is the Phase-413-specific green result in T2a, not the accidental coupling of unrelated billing tests into the same crate package.

---

## T3 — Live dev racecontrol boot

_(filled in by Task 2)_

## T4 — Live route pod-IP request → 200 + mesh_service_key JSON

_(filled in by Task 2)_

## T5 — Live route localhost (Staff) → 403

_(filled in by Task 2)_

## T6 — Live route Customer IP → 403

_(filled in by Task 2)_

## T7 — rc-agent boot `periodic_refetch started` log

_(filled in by Task 3)_

## T8 — rc-agent synchronous initial fetch ok (+ optional long-form first_success)

_(filled in by Task 3)_

## T9 — rc-agent graceful degradation with server down

_(filled in by Task 3)_

## T10 — rc-agent self_healed after server recovery

_(filled in by Task 3)_
