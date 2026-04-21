---
phase: 445-typed-api-contract-rust-ts-codegen
plan: 01
subsystem: build-system
tags: [cargo, workspace, feature-flags, utoipa, ts-rs, serde_yaml, bin, scaffolding]

# Dependency graph
requires:
  - 445-00 (enum audit test + admin whitelist + ts-rs spike + skip-branch scripts)
provides:
  - 4 workspace dependencies (utoipa 5.4, utoipa-axum 0.2, ts-rs 12, serde_yaml 0.9)
  - `ts-rs` feature on rc-common (optional, weak-dep pattern)
  - `gen-types` feature on racecontrol-crate
  - `[[bin]] gen-types` entry with required-features gating
  - crates/racecontrol/src/api/openapi.rs (ApiDoc umbrella struct, feature-gated)
  - crates/racecontrol/src/bin/gen_types.rs (binary skeleton, emits placeholder output)
  - First emitted `docs/openapi.generated.yaml` (572 B, 5 tags, 0 paths)
  - First emitted `packages/shared-types/generated/index.ts` (105 B placeholder)
  - Plan 00 determinism script graduated from SKIP to `DETERMINISTIC: 0c927858...`
  - Plan 00 drift script graduated from SKIP to `OK: no drift`
affects: [445-02a, 445-02b, 445-03, 445-04, 445-05]

# Tech tracking
tech-stack:
  added:
    - "utoipa 5.4 (axum_extras, chrono, uuid features) -- workspace dep, racecontrol-crate optional"
    - "utoipa-axum 0.2 -- workspace dep, racecontrol-crate optional"
    - "ts-rs 12 (chrono-impl, uuid-impl, serde-json-impl features) -- workspace dep, rc-common+racecontrol-crate optional"
    - "serde_yaml 0.9 -- workspace dep, racecontrol-crate optional"
  patterns:
    - "Weak-dep feature isolation (Pitfall 1): rc-common ts-rs feature uses `dep:ts-rs` to prevent unification into rc-agent/rc-sentry"
    - "required-features [[bin]] gating: gen-types binary compiled only with --features gen-types; zero cost to default cargo build"
    - "Feature-gated module: #[cfg(feature = \"gen-types\")] pub mod openapi; keeps live-server Router free of utoipa-axum (Pitfall 3)"

key-files:
  created:
    - crates/racecontrol/src/api/openapi.rs
    - crates/racecontrol/src/bin/gen_types.rs
    - docs/openapi.generated.yaml
    - packages/shared-types/generated/index.ts
    - .planning/phases/445-typed-api-contract-rust-ts-codegen/445-01-SUMMARY.md
  modified:
    - Cargo.toml (4 workspace deps added)
    - Cargo.lock (transitive deps resolved)
    - crates/rc-common/Cargo.toml (optional ts-rs dep + ts-rs feature)
    - crates/racecontrol/Cargo.toml (4 optional deps + gen-types feature + [[bin]] gen-types)
    - crates/racecontrol/src/api/mod.rs (feature-gated pub mod openapi)

key-decisions:
  - "Weak-dep pattern (`dep:ts-rs`) chosen on rc-common -- prevents ts-rs leaking into rc-agent/rc-sentry via feature unification when they depend on rc-common without the ts-rs feature"
  - "gen-types binary uses required-features = [\"gen-types\"] rather than a new crate -- stays inside racecontrol-crate, default build skips it entirely"
  - "OpenApi umbrella module is feature-gated (#[cfg(feature = \"gen-types\")] pub mod openapi;) -- Pitfall 3 respected, live axum Router unchanged"
  - "Plan 00 SPIKE Verdict A locked: direct fn main() + TS::export_all(&Config::new().with_out_dir(...)) -- no cargo-test fallback needed"
  - "Plan 02a annotation target: Plan 00's 42-name admin whitelist at packages/shared-types/generated/.whitelist.txt"

patterns-established:
  - "4-tier weak feature propagation: workspace dep (top) -> optional dep (crate) -> gated feature (crate) -> required-features [[bin]] (binary). Each tier gates the next."
  - "Feature-gated mod with feature-gated binary: both the openapi.rs module AND the gen-types binary are gated on the same `gen-types` feature -- they're a compile unit."
  - "Determinism gate armed at binary creation: Plan 00's 3x rerun script was designed to SKIP until the binary existed, then auto-arm. Fired on first run (DETERMINISTIC: 0c927858...)."

requirements-completed: [TYP-01, TYP-02]

# Metrics
duration: 19min
completed: 2026-04-21
---

# Phase 445 Plan 01: Wave 1 Deps + Scaffolding Summary

**Added 4 workspace dependencies (utoipa 5.4, utoipa-axum 0.2, ts-rs 12, serde_yaml 0.9) with 4-tier feature-flag isolation, created gen-types binary skeleton + OpenApi umbrella module, emitted first deterministic stub output at `docs/openapi.generated.yaml` + `packages/shared-types/generated/index.ts`.**

## Performance

- **Duration:** ~19 min
- **Started:** 2026-04-21T05:55 IST (2026-04-21T00:25Z)
- **Completed:** 2026-04-21T06:14 IST (2026-04-21T00:44Z)
- **Tasks:** 2
- **Files created:** 4 (openapi.rs, gen_types.rs, openapi.generated.yaml, index.ts)
- **Files modified:** 5 (Cargo.toml, Cargo.lock, rc-common/Cargo.toml, racecontrol/Cargo.toml, api/mod.rs)

## Accomplishments

- **4 workspace dependencies added** with exact versions per RESEARCH.md § Standard Stack. Pinned against local registry cache (ts-rs 12.0.1 already present; utoipa/utoipa-axum/serde_yaml downloaded during first build). Versions: `utoipa = { version = "5.4", features = ["axum_extras", "chrono", "uuid"] }`, `utoipa-axum = "0.2"`, `ts-rs = { version = "12", features = ["chrono-impl", "uuid-impl", "serde-json-impl"] }`, `serde_yaml = "0.9"`.
- **Feature-flag plumbing isolated ts-rs from all non-gen-types binaries.** rc-common has optional `dep:ts-rs` under new `ts-rs` feature. racecontrol-crate has 4 optional deps under new `gen-types` feature: `gen-types = ["dep:utoipa", "dep:utoipa-axum", "dep:ts-rs", "dep:serde_yaml", "rc-common/ts-rs"]`. Binary registered via `[[bin]] name = "gen-types"` with `required-features = ["gen-types"]` so default `cargo build` skips it entirely.
- **gen-types binary builds, runs, emits deterministic output.** First build took 2m 48s (cold compile pulling utoipa 5.4.0, utoipa-axum 0.2.x, serde_yaml 0.9.x into the dep tree). Subsequent runs were instant. Binary produces `docs/openapi.generated.yaml` (572 B, openapi 3.1.0 spec with 5 tags + empty paths) and `packages/shared-types/generated/index.ts` (105 B placeholder) with byte-identical output across 3 consecutive runs (sha256: `673ee228...` for yaml, `0469501f...` for index.ts).
- **Plan 00's skip-branch scripts graduated from SKIP to active.** `bash scripts/check-gen-types-determinism.sh` now prints `DETERMINISTIC: 0c927858dd1b0806d3e69cc627175f91044dfbe956aefc2ea8149d4cb3b62987`. `bash scripts/check-generated-types-drift.sh` now prints `OK: no drift` (once the emitted files are committed). Both CI gates are armed.
- **Zero regression on default builds.** `cargo check --release -p racecontrol-crate --bin racecontrol`, `-p rc-agent-crate`, `-p rc-sentry` all green. Only pre-existing warnings (4 in racecontrol lib, 98 in rc-agent, 18 in rc-sentry). `cargo test -p rc-common --test enum_tagging_audit` still returns `1 passed` -- Plan 00 D-14 gate preserved.
- **Pitfall 1 feature isolation VERIFIED via cargo tree.** `cargo tree -e features -p rc-agent-crate | grep -c ts-rs` = 0. `cargo tree -e features -p rc-sentry | grep -c ts-rs` = 0. `cargo tree -e features -p racecontrol-crate | grep -c ts-rs` = 0 (default features). `cargo tree -e features -p racecontrol-crate --features gen-types | grep -c ts-rs` = 26 (positive control -- utoipa + ts-rs both pulled in only when the feature is requested).

## Task Commits

Each task committed atomically:

1. **Task 1: Workspace deps + feature-flag plumbing** -- `4141529e` (feat)
   - `Cargo.toml` (+5 lines: 4 workspace deps + section comment)
   - `Cargo.lock` (transitive deps resolved)
   - `crates/rc-common/Cargo.toml` (+6 lines: `[dependencies.ts-rs]` optional + `ts-rs = ["dep:ts-rs"]` feature)
   - `crates/racecontrol/Cargo.toml` (+17 lines: `[[bin]] gen-types`, `[features] gen-types = [...]`, 4 `[dependencies.*]` optional blocks)

2. **Task 2: gen-types binary skeleton + OpenApi umbrella module** -- `ef5cf07e` (feat)
   - `crates/racecontrol/src/api/mod.rs` (+5 lines: feature-gated `pub mod openapi;`)
   - `crates/racecontrol/src/api/openapi.rs` (+38 lines NEW: `#[derive(OpenApi)]` struct ApiDoc + info + 5 tags + placeholder fn)
   - `crates/racecontrol/src/bin/gen_types.rs` (+68 lines NEW: fn main() emitting yaml + index.ts stub, with Plan 02a sketch in comments)
   - `docs/openapi.generated.yaml` (572 B NEW: first emitted output)
   - `packages/shared-types/generated/index.ts` (105 B NEW: placeholder)

Total: 2 task commits, ~141 lines of net additions across 2 new source files + 2 first-emitted artifacts + 2 Cargo.toml blocks + Cargo.lock + api/mod.rs feature-gate.

## Cargo.toml diff (workspace additions)

```toml
# Phase 445 -- Typed API Contract (gen-types binary only; not in default build)
utoipa = { version = "5.4", features = ["axum_extras", "chrono", "uuid"] }
utoipa-axum = "0.2"
ts-rs = { version = "12", features = ["chrono-impl", "uuid-impl", "serde-json-impl"] }
serde_yaml = "0.9"
```

## Feature tree evidence

### rc-agent-crate (ts-rs MUST be absent)

```
$ cargo tree -e features -p rc-agent-crate | grep -c ts-rs
0
```

### rc-sentry (ts-rs MUST be absent)

```
$ cargo tree -e features -p rc-sentry | grep -c ts-rs
0
```

### racecontrol-crate default features (ts-rs MUST be absent)

```
$ cargo tree -e features -p racecontrol-crate | grep -c ts-rs
0
```

### racecontrol-crate with --features gen-types (positive control, ts-rs MUST be present)

```
$ cargo tree -e features -p racecontrol-crate --features gen-types | grep -c ts-rs
26
$ cargo tree -e features -p racecontrol-crate --features gen-types | grep -E "(utoipa |ts-rs |serde_yaml )" | head -3
...
│   ├── utoipa v5.4.0
...
│       ├── ts-rs feature "chrono-impl"
...
```

## First-run gen-types output (stderr)

```
$ cargo run --release --bin gen-types --features gen-types
gen-types: starting (Phase 445 Wave 1 skeleton)
gen-types: wrote docs/openapi.generated.yaml (572 bytes)
gen-types: wrote packages/shared-types/generated\index.ts (105 bytes)
gen-types: done
```

## Determinism check (3 consecutive runs)

```
$ for i in 1 2 3; do ./target/release/gen-types.exe 2>/dev/null; sha256sum docs/openapi.generated.yaml packages/shared-types/generated/index.ts; done
run1: 673ee2285321e5361ebba828b14d417d4448ca370608c68d8f63877aac590b93  docs/openapi.generated.yaml
run1: 0469501f694079fb35ea3299e51ba5ab8a7d0e82f7d0ac0882c302230e17ba1d  packages/shared-types/generated/index.ts
run2: 673ee2285321e5361ebba828b14d417d4448ca370608c68d8f63877aac590b93  docs/openapi.generated.yaml
run2: 0469501f694079fb35ea3299e51ba5ab8a7d0e82f7d0ac0882c302230e17ba1d  packages/shared-types/generated/index.ts
run3: 673ee2285321e5361ebba828b14d417d4448ca370608c68d8f63877aac590b93  docs/openapi.generated.yaml
run3: 0469501f694079fb35ea3299e51ba5ab8a7d0e82f7d0ac0882c302230e17ba1d  packages/shared-types/generated/index.ts
```

All 3 runs byte-identical. Determinism harness (Plan 00) now prints combined-hash `DETERMINISTIC: 0c927858...`.

## emitted docs/openapi.generated.yaml content

```yaml
openapi: 3.1.0
info:
  title: Racing Point racecontrol API
  description: Generated from utoipa annotations. Admin-surface subset (Phase 445 D-06).
  contact:
    name: RacingPoint
    email: bono@racingpoint.in
  license:
    name: MIT
    identifier: MIT
  version: 1.0.0
paths: {}
components: {}
tags:
- name: admin
  description: Admin panel surface
- name: fleet
  description: Fleet health + status
- name: billing
  description: Billing sessions + pricing
- name: games
  description: Game catalog + launch
- name: drivers
  description: Driver/customer management
```

Plan 02b's job: populate `paths:` via `utoipa_axum::OpenApiRouter::new().routes(routes!(...))` and the `#[utoipa::path]` attributes on each admin handler.

## Decisions Made

- **D-A (weak-dep pattern confirmed):** `[dependencies.ts-rs] optional = true` + `ts-rs = ["dep:ts-rs"]` feature on rc-common prevents ts-rs unification when rc-agent/rc-sentry depend on rc-common without the feature. Verified via `cargo tree`.
- **D-B (OpenApi module is feature-gated, NOT wired into Router):** `#[cfg(feature = "gen-types")] pub mod openapi;` in `api/mod.rs` means the module and ApiDoc struct only exist when the feature is active. `src/main.rs` has ZERO references to `api::openapi` (verified `grep`). Pitfall 3 respected.
- **D-C (required-features on [[bin]] skips binary cleanly):** `[[bin]] name = "gen-types" path = "src/bin/gen_types.rs" required-features = ["gen-types"]` means default `cargo build --release` does not even attempt to compile gen_types.rs. This is the zero-cost isolation pattern.
- **D-D (ts-rs API call deferred to Plan 02a):** Per SPIKE Verdict A, Plan 02a replaces the index.ts placeholder in gen_types.rs with `let cfg = Config::new().with_out_dir(gen_dir); BillingSessionInfo::export_all(&cfg)?; ...`. Wave 1 just proves the build chain.
- **D-E (ApiDoc tags locked):** 5 tags (admin, fleet, billing, games, drivers) match the 5 functional domains from CONTEXT.md § D-06. Plan 02b groups handler routes under these tags.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Missing `use utoipa::OpenApi;` in gen_types.rs**
- **Found during:** Task 2, first build attempt (`cargo build --release --bin gen-types --features gen-types`)
- **Issue:** Rust compile error E0599: `no function or associated item named 'openapi' found for struct 'ApiDoc' in the current scope`. The `#[derive(OpenApi)]` macro generates an inherent-looking `ApiDoc::openapi()` but it requires the `utoipa::OpenApi` trait to be in scope at the call site. The openapi.rs module already imports it, but gen_types.rs (in a different compilation unit) did not.
- **Fix:** Added `use utoipa::OpenApi; // provides ApiDoc::openapi() via #[derive(OpenApi)]` at top of gen_types.rs.
- **Files modified:** `crates/racecontrol/src/bin/gen_types.rs`
- **Verification:** Re-ran `cargo build --release --bin gen-types --features gen-types` -- succeeded with only pre-existing warnings.
- **Committed in:** `ef5cf07e` (bundled with rest of Task 2 -- the fix was applied before the commit).

**2. [Rule 2 - Missing] STATE.md requirements tracking for TYP-01/TYP-02**
- **Found during:** final state-update step (attempting `requirements mark-complete TYP-01 TYP-02`)
- **Issue:** `.planning/REQUIREMENTS.md` does not track TYP-* identifiers (consistent with Plan 00 SUMMARY where TYP-07/TYP-09 were named "completed" but are likewise absent from the file). Plan 00 SUMMARY documented this as expected.
- **Fix:** Skipped `requirements mark-complete` for TYP-01/TYP-02. Documented the requirement IDs in the frontmatter of this SUMMARY for future migration (if REQUIREMENTS.md gets a TYP section).
- **Files modified:** None (this SUMMARY frontmatter records the intent).
- **Impact:** Zero -- the requirement coverage is semantic, tracked here and in the plan frontmatter (`requirements: [TYP-01, TYP-02]`). Matches Plan 00 precedent.

### Authentication Gates

None -- no external auth required for this wave.

---

**Total deviations:** 1 Rule 1 auto-fix (compile error) + 1 Rule 2 skip (missing REQUIREMENTS.md tracking).
**Impact on plan:** Rule 1 fix necessary for correctness. Rule 2 matches Plan 00 precedent -- no semantic impact.

## Issues Encountered

- **First gen-types build took 2m 48s.** Expected per CLAUDE.md "fresh full compile (~2-3 min on James)" -- utoipa 5.4.0, utoipa-axum, serde_yaml 0.9.x, ts-rs 12.0.1 and their transitives were downloaded + compiled cold. Subsequent incremental builds were <5s.
- **CRLF warnings on `git add`.** Git's `core.autocrlf` auto-conversion warned on every LF-written file (standard Windows Git Bash behaviour). Non-blocking.
- **`cargo tree` ts-rs count = 26 when --features gen-types** -- much higher than expected "at least 1". Reason: ts-rs appears in the tree once per feature enablement (chrono-impl, uuid-impl, serde-json-impl, default, serde-compat) and per transitive edge. The count is a positive control -- it says "ts-rs is reachable through many feature paths when gen-types is on", which is correct.

## Next Phase Readiness

**Plan 02a (Wave 2a -- rc-common TS derives + gen-types body) can start immediately with:**
1. Working gen-types binary at `crates/racecontrol/src/bin/gen_types.rs` -- replace the index.ts placeholder block with real `TS::export_all(&cfg)` calls per whitelist entry.
2. `rc-common` has `ts-rs` feature wired -- Plan 02a adds `#[cfg_attr(feature = "ts-rs", derive(TS))]` + `#[ts(export_to = "...")]` on each of the 42 admin-consumed types from `.whitelist.txt`.
3. Plan 00's 3 skip-branch gates (determinism + drift + hand-vs-generated) are armed -- Plan 02a's changes will fire them on every run.
4. D-14 safety gate (`cargo test -p rc-common --test enum_tagging_audit`) is live -- if Plan 02a tries to annotate an adjacently-tagged or flatten-containing type, the test will fail at CI time.

**Plan 02b (Wave 2b -- utoipa annotations on 43 admin handlers) can start immediately with:**
1. Working ApiDoc struct at `crates/racecontrol/src/api/openapi.rs` -- Plan 02b adds `paths(...)` and imports the handler functions to annotate with `#[utoipa::path]`.
2. `utoipa-axum` crate is in the tree under the `gen-types` feature -- Plan 02b uses `utoipa_axum::OpenApiRouter::new().routes(routes!(...))` to build the route set.
3. `docs/openapi.generated.yaml` already emits a valid spec with the 5 tags -- Plan 02b's routes populate `paths:` by referencing these tags.

**No blockers.** Plan 01 satisfies every Wave 1 goal from VALIDATION.md.

## Self-Check: PASSED

**Files verified (5/5 exist on disk):**
- crates/racecontrol/src/api/openapi.rs
- crates/racecontrol/src/bin/gen_types.rs
- docs/openapi.generated.yaml
- packages/shared-types/generated/index.ts
- .planning/phases/445-typed-api-contract-rust-ts-codegen/445-01-SUMMARY.md

**Commits verified (2/2 present in `git log --oneline --all`):**
- 4141529e (Task 1: workspace deps + feature-flag plumbing)
- ef5cf07e (Task 2: gen-types binary skeleton + OpenApi umbrella)

**Acceptance criteria (all 6 plan-level + 11 Task 1 greps):**
- `grep -c '^utoipa = ' Cargo.toml` = 1
- `grep -c '^utoipa-axum = ' Cargo.toml` = 1
- `grep -c '^ts-rs = ' Cargo.toml` = 1
- `grep -c '^serde_yaml = ' Cargo.toml` = 1
- `grep -c 'ts-rs = \["dep:ts-rs"\]' crates/rc-common/Cargo.toml` = 1
- `grep -cE '\[dependencies\.ts-rs\]' crates/rc-common/Cargo.toml` = 1
- `grep -c 'gen-types = \[' crates/racecontrol/Cargo.toml` = 1
- `grep -c 'name = "gen-types"' crates/racecontrol/Cargo.toml` = 1
- `grep -c 'required-features = \["gen-types"\]' crates/racecontrol/Cargo.toml` = 1
- `cargo check --release -p racecontrol-crate --bin racecontrol` exits 0 (default build regression-free)
- `cargo build --release --bin gen-types --features gen-types` exits 0 (new binary compiles)
- `target/release/gen-types.exe` exists (3,774,464 bytes)
- `cargo run --release --bin gen-types --features gen-types` exits 0
- `docs/openapi.generated.yaml` contains "title: Racing Point racecontrol API" (grep count = 1)
- `bash scripts/check-gen-types-determinism.sh` prints `DETERMINISTIC: 0c927858...`
- `bash scripts/check-generated-types-drift.sh` prints `OK: no drift`
- `cargo test -p rc-common --test enum_tagging_audit` = 1 passed (Plan 00 preserved)
- `cargo tree -e features -p rc-agent-crate | grep -c ts-rs` = 0 (Pitfall 1 defence)
- `cargo tree -e features -p rc-sentry | grep -c ts-rs` = 0
- `cargo tree -e features -p racecontrol-crate | grep -c ts-rs` = 0 (default)
- `cargo tree -e features -p racecontrol-crate --features gen-types | grep -c ts-rs` = 26 (positive control)

---
*Phase: 445-typed-api-contract-rust-ts-codegen*
*Completed: 2026-04-21*
