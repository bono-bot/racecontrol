// Build script for the v2-db crate.
//
// `sqlx::migrate!("./migrations")` (src/lib.rs) embeds the migration set into the
// binary at COMPILE time. Cargo only recompiles a crate when one of its tracked
// inputs changes — and the `migrations/` *directory* is NOT a tracked input unless
// a build script declares it. Without this, adding a new migration file does NOT
// invalidate the cached compilation of this crate, so an incremental build keeps a
// STALE embedded migration set. A binary built that way then fails at startup with
// `migration <ver> was previously applied but is missing in the resolved migrations`
// against any database that already has the newer migration applied.
//
// Root cause of the 2026-06-06/07 `.23` deploy failure: the staged binary embedded
// 8 of 9 migrations (missing 20260531030000_heart_v2_sessions) for exactly this
// reason. Declaring the directory here makes every migration add/edit bust the
// cache and re-run the macro. (sqlx's documented recommendation.)
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
