// crates/racecontrol/src/bin/gen_types.rs
// Phase 445 Wave 2a — real per-type ts-rs emission + OpenAPI spec write-out.
//
// Strategy locked by Plan 00 SPIKE (Verdict A): ts-rs 12.0.1's
// TS::export_all(&Config) works in plain fn main() context (no cargo-test
// fallback needed). API name is `export_all` (not `export_all_to` as
// RESEARCH.md originally documented — see 445-00-SPIKE.md).
//
// Each `::export_all(&cfg)` call writes ONE `.ts` file per apex type PLUS
// every transitive TS-derived dependency. The `Config::new().with_out_dir()`
// builder overrides every type's `#[ts(export_to = "...")]` to point at the
// same `packages/shared-types/generated/` directory so every .ts file lands
// in one place.
//
// Order of calls is stable (alphabetical by type name) so the index.ts
// barrel and the determinism gate (scripts/check-gen-types-determinism.sh)
// produce byte-identical output across runs — Pitfall 6 mitigation.
//
// Decision IDs: D-03 (gen-types binary), D-07 (admin whitelist scope),
// D-09 (output dir packages/shared-types/generated), Pitfall 3 (openapi
// module does NOT wire into live axum Router), Pitfall 6 (determinism
// harness armed).

use std::path::Path;
use ts_rs::TS; // provides ::export_all(&Config)
use utoipa::OpenApi; // provides ApiDoc::openapi() via #[derive(OpenApi)]

fn main() -> anyhow::Result<()> {
    eprintln!("gen-types: starting (Phase 445 Wave 2a)");

    // ────────────────────── OpenAPI emission ──────────────────────
    // ApiDoc is defined in crates/racecontrol/src/api/openapi.rs and
    // feature-gated behind `gen-types`. Plan 02b (parallel) populates its
    // `paths(...)` via utoipa_axum::OpenApiRouter so the generated spec
    // grows route coverage over time. Wave 2a keeps the emission path
    // alive so the determinism/drift scripts never trip on a missing file.
    let openapi = racecontrol_crate::api::openapi::ApiDoc::openapi();
    let yaml = serde_yaml::to_string(&openapi)?;
    let yaml_path = Path::new("docs/openapi.generated.yaml");
    if let Some(dir) = yaml_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(yaml_path, &yaml)?;
    eprintln!(
        "gen-types: wrote {} ({} bytes)",
        yaml_path.display(),
        yaml.len()
    );

    // ────────────────────── ts-rs emission ──────────────────────
    // Apex types from packages/shared-types/generated/.whitelist.txt
    // (Plan 00's 42-name admin list), filtered to rc-common-resident
    // derive-safe entries. Transitive TS-derived deps are pulled in by
    // export_all automatically (e.g. AcLanSessionConfig pulls AcSessionBlock,
    // AcEntrySlot, AcWeatherConfig, AcDynamicTrackConfig; PodInventory pulls
    // GameInventory, AiCountRange).
    //
    // Whitelist entries NOT derived (D-14 safety + missing structs):
    //   CloudAction, CoreToAgentMessage  — #[serde(tag, content)] adjacently tagged
    //   CoreMessage                      — owns a #[serde(flatten)] field
    //   PendingCloudAction               — transitively depends on CloudAction
    //   AgentConfig                      — deferred (complex nested config tree)
    //   FleetHealthResponse              — no struct exists (inline json!() handler)
    //
    // See 445-02a-SUMMARY.md § Deviations for full rationale.
    let gen_dir = Path::new("packages/shared-types/generated");
    std::fs::create_dir_all(gen_dir)?;

    // Config overrides every `#[ts(export_to = "...")]` to point at this
    // one directory so regardless of the relative-path reasoning each
    // derive does, every .ts file lands here.
    let cfg = ts_rs::Config::default().with_out_dir(gen_dir);

    // Order matters only for the index.ts barrel — alphabetical to keep
    // the output deterministic and human-scannable.
    // rc-common::fleet_event
    rc_common::fleet_event::FleetEvent::export_all(&cfg)?;
    rc_common::fleet_event::Incident::export_all(&cfg)?;

    // rc-common::fleet_health_types
    rc_common::fleet_health_types::PodFleetStatus::export_all(&cfg)?;

    // rc-common::inventory_types
    rc_common::inventory_types::AiCountRange::export_all(&cfg)?;
    rc_common::inventory_types::ContentDirsResponse::export_all(&cfg)?;
    rc_common::inventory_types::GameDirs::export_all(&cfg)?;
    rc_common::inventory_types::GameInventory::export_all(&cfg)?;
    rc_common::inventory_types::PodInventory::export_all(&cfg)?;
    rc_common::inventory_types::ValidityError::export_all(&cfg)?;
    rc_common::inventory_types::ValidityErrorCode::export_all(&cfg)?;

    // rc-common::mesh_types
    rc_common::mesh_types::DiagnosisTier::export_all(&cfg)?;
    rc_common::mesh_types::FixType::export_all(&cfg)?;
    rc_common::mesh_types::MeshSolution::export_all(&cfg)?;
    rc_common::mesh_types::SolutionStatus::export_all(&cfg)?;

    // rc-common::protocol
    rc_common::protocol::LaunchNoteEvent::export_all(&cfg)?;
    rc_common::protocol::LaunchState::export_all(&cfg)?;

    // rc-common::survival_types
    rc_common::survival_types::ActionId::export_all(&cfg)?;
    rc_common::survival_types::HealLease::export_all(&cfg)?;
    rc_common::survival_types::HealLeaseRequest::export_all(&cfg)?;
    rc_common::survival_types::HealLeaseResponse::export_all(&cfg)?;
    rc_common::survival_types::SurvivalLayer::export_all(&cfg)?;

    // rc-common::types (core admin surface)
    rc_common::types::AcDynamicTrackConfig::export_all(&cfg)?;
    rc_common::types::AcEntrySlot::export_all(&cfg)?;
    rc_common::types::AcLanSessionConfig::export_all(&cfg)?;
    rc_common::types::AcServerStatus::export_all(&cfg)?;
    rc_common::types::AcSessionBlock::export_all(&cfg)?;
    rc_common::types::AcStatus::export_all(&cfg)?;
    rc_common::types::AcWeatherConfig::export_all(&cfg)?;
    rc_common::types::BillingSessionInfo::export_all(&cfg)?;
    rc_common::types::BillingSessionStatus::export_all(&cfg)?;
    rc_common::types::Booking::export_all(&cfg)?;
    rc_common::types::Driver::export_all(&cfg)?;
    rc_common::types::DrivingState::export_all(&cfg)?;
    rc_common::types::Event::export_all(&cfg)?;
    rc_common::types::EventType::export_all(&cfg)?;
    rc_common::types::GameState::export_all(&cfg)?;
    rc_common::types::LapData::export_all(&cfg)?;
    rc_common::types::Leaderboard::export_all(&cfg)?;
    rc_common::types::LeaderboardEntry::export_all(&cfg)?;
    rc_common::types::PlayableSignal::export_all(&cfg)?;
    rc_common::types::PodInfo::export_all(&cfg)?;
    rc_common::types::PodStatus::export_all(&cfg)?;
    rc_common::types::PricingTier::export_all(&cfg)?;
    rc_common::types::SessionType::export_all(&cfg)?;
    rc_common::types::SimType::export_all(&cfg)?;
    rc_common::types::WatchdogCrashReport::export_all(&cfg)?;

    // ────────────────────── index.ts barrel ──────────────────────
    // Hard-coded alphabetical order (NOT HashMap/DirEntry iteration) so
    // every run produces byte-identical output — Pitfall 6 determinism.
    let index_body = concat!(
        "// Generated by gen-types (Phase 445). DO NOT EDIT.\n",
        "// Re-exported root types from rc-common's admin-consumed whitelist.\n",
        "// Transitive deps are reachable via each file's own `import type` lines.\n",
        "\n",
        "export type { AcDynamicTrackConfig } from './AcDynamicTrackConfig';\n",
        "export type { AcEntrySlot } from './AcEntrySlot';\n",
        "export type { AcLanSessionConfig } from './AcLanSessionConfig';\n",
        "export type { AcServerStatus } from './AcServerStatus';\n",
        "export type { AcSessionBlock } from './AcSessionBlock';\n",
        "export type { AcStatus } from './AcStatus';\n",
        "export type { AcWeatherConfig } from './AcWeatherConfig';\n",
        "export type { ActionId } from './ActionId';\n",
        "export type { AiCountRange } from './AiCountRange';\n",
        "export type { BillingSessionInfo } from './BillingSessionInfo';\n",
        "export type { BillingSessionStatus } from './BillingSessionStatus';\n",
        "export type { Booking } from './Booking';\n",
        "export type { ContentDirsResponse } from './ContentDirsResponse';\n",
        "export type { DiagnosisTier } from './DiagnosisTier';\n",
        "export type { Driver } from './Driver';\n",
        "export type { DrivingState } from './DrivingState';\n",
        "export type { Event } from './Event';\n",
        "export type { EventType } from './EventType';\n",
        "export type { FixType } from './FixType';\n",
        "export type { FleetEvent } from './FleetEvent';\n",
        "export type { GameDirs } from './GameDirs';\n",
        "export type { GameInventory } from './GameInventory';\n",
        "export type { GameState } from './GameState';\n",
        "export type { HealLease } from './HealLease';\n",
        "export type { HealLeaseRequest } from './HealLeaseRequest';\n",
        "export type { HealLeaseResponse } from './HealLeaseResponse';\n",
        "export type { Incident } from './Incident';\n",
        "export type { LapData } from './LapData';\n",
        "export type { LaunchNoteEvent } from './LaunchNoteEvent';\n",
        "export type { LaunchState } from './LaunchState';\n",
        "export type { Leaderboard } from './Leaderboard';\n",
        "export type { LeaderboardEntry } from './LeaderboardEntry';\n",
        "export type { MeshSolution } from './MeshSolution';\n",
        "export type { PlayableSignal } from './PlayableSignal';\n",
        "export type { PodFleetStatus } from './PodFleetStatus';\n",
        "export type { PodInfo } from './PodInfo';\n",
        "export type { PodInventory } from './PodInventory';\n",
        "export type { PodStatus } from './PodStatus';\n",
        "export type { PricingTier } from './PricingTier';\n",
        "export type { SessionType } from './SessionType';\n",
        "export type { SimType } from './SimType';\n",
        "export type { SolutionStatus } from './SolutionStatus';\n",
        "export type { SurvivalLayer } from './SurvivalLayer';\n",
        "export type { ValidityError } from './ValidityError';\n",
        "export type { ValidityErrorCode } from './ValidityErrorCode';\n",
        "export type { WatchdogCrashReport } from './WatchdogCrashReport';\n",
    );
    let index_path = gen_dir.join("index.ts");
    std::fs::write(&index_path, index_body)?;
    eprintln!(
        "gen-types: wrote {} ({} bytes)",
        index_path.display(),
        index_body.len()
    );

    eprintln!("gen-types: done");
    Ok(())
}
