// rc-process-manager — declarative process ownership for the RacingPoint fleet.
//
// S8 Wave 1 scaffold. See plan_process_manager_crate_20260423.md for the full wave plan.
//
// Purpose: eliminate multi-owner process bugs (Steam respawn races, ConspitLink ×2,
// msedge/acmanager overlaps, openconsole leaks) by making this crate the sole
// legal caller of `Command::new` / `spawn_safe` / `taskkill` inside the workspace.
//
// Wave 1 deliverable: compiling skeleton only — no public API, no caller migration,
// no clippy lint. W2 adds `ManagedProcess` enum + TOML registry. W3 adds the typed
// spawn/kill API. W4 migrates 6 existing callers. W6 adds the clippy
// `disallowed-methods` lint banning `Command::new` / `spawn_safe` outside this crate.

pub mod manager;
pub mod process;
pub mod registry;

pub use manager::ProcessManager;
pub use process::ManagedProcess;
pub use registry::{Entry, Registry, RegistryError, RespawnPolicy};

/// Semver discriminator for the registry schema. W2 ships schema_version 1
/// (enum + TOML + loader); W3 will keep the same value unless the typed API
/// forces a shape change. Bump in lockstep with `registry::SCHEMA_VERSION`.
pub const REGISTRY_SCHEMA_VERSION: u32 = registry::SCHEMA_VERSION;
