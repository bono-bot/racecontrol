# §S-146 short-RCA — heart_v2.rs clippy lint-debt cleanup (2026-05-31)

**Surface:** `crates/racecontrol/src/api/heart_v2.rs` (foundational — heart V2 session/launch surface)
**Class:** zero-behavior lint hygiene (NOT a V1↔V2 boundary change). Rides in PR #112 with the Arc::get_mut panic-fix.
**Captain per-PR auth:** AskUserQuestion selection "Fix clippy + merge green" 2026-05-31 (named heart_v2.rs surface explicitly).
**Why short-RCA, not full 5-section + MMA Step-1:** the changes touch no V1↔V2 seam, no schema, no protocol, no runtime behavior. heart_v2.rs is V2-native code. MMA Step-1 DIAGNOSE is for foundational *semantic* changes; running 5-model consensus on "derive Default vs manual impl" is not warranted. Below even the §S-186 small-fix bar (which is itself for behavioral fixes).

## 1. What
Three clippy `-D warnings` errors on current main (`b7067829`), pre-existing since the heart-V2 merge — main CI has been red since. Not introduced by the panic-fix (which only touches `main.rs`). Fixes:
- **L168 `AckReq`** — manual `impl Default` is byte-identical to `#[derive(Default)]` output (sole field `Option<String>` → `None`). Replace manual impl with derive. `clippy::derivable_impls`.
- **L181 `LaunchOutcome`** — `Ok` variant 832 bytes vs others ≤24 → `clippy::large_enum_variant`. Apply clippy's own suggested override `#[allow(clippy::large_enum_variant)]` on the enum. NO Box (would ripple to callers/construction sites); the lint is a perf-nit, not correctness.
- **L735 `match { … }`** — `clippy::blocks_in_conditions`. Remove the redundant braces around the match scrutinee.

## 2. Why still needed
`cargo clippy --workspace -- -D warnings …` (CI `build` job, windows-latest) fails with `could not compile racecontrol-crate (lib) due to 3 previous errors`. Blocks PR #112 from green + keeps main CI red. gate-clean (CI green) is part of the V2 definition-of-done.

## 3. V2-compat check
Zero behavior/schema/protocol change. `AckReq::default()` semantics unchanged (None). `LaunchOutcome` shape unchanged (allow-attr is a no-op at runtime). The match scrutinee is unchanged (braces were syntactic). heart_v2 module tests (33/33) must stay green post-edit — re-run is the verification. No conflict with any V2 anchor; this moves the surface toward gate-clean.
