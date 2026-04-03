# Phase 317: Server Inventory & Fleet Intelligence - Context

**Gathered:** 2026-04-03
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped via autonomous mode)

<domain>
## Phase Boundary

Server persists per-pod game inventory and combo validation results, aggregates fleet availability, auto-disables universally broken combos, and alerts staff on crash loops and chain launch failures. Handles GameInventoryUpdate + ComboValidationReport WS messages from agents. Adds crash loop detection to fleet_health.rs and chain failure alerting via EscalationRequest → WhatsApp.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — autonomous mode. Key constraints:
- GameInventoryUpdate handler: upsert to pod_game_inventory table, update in-memory PodFleetStatus
- ComboValidationReport handler: store in combo_validation_flags table, recompute fleet_validity
- Fleet validity: valid (all pods), partial (some pods), invalid (no pods) — based on combo_validation_flags
- Auto-disable: only when invalid on ALL pods — set enabled=false + WhatsApp alert
- Crash loop detection: 3+ StartupReport in 5min with uptime<30s → crash_loop:true in fleet health + WhatsApp
- Chain failure: 3+ consecutive game launch failures same pod/SimType in 10min → EscalationRequest → WhatsApp
- WhatsApp alerts route through EscalationRequest WS path (standing rule), NOT direct Evolution API
- New tables: pod_game_inventory, combo_validation_flags (or extend combo_reliability)
- Endpoint changes: GET /api/v1/presets returns fleet_validity field
- GET /api/v1/fleet/health returns crash_loop field per pod

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/ws/mod.rs` — WS message handlers for AgentMessage variants
- `crates/racecontrol/src/fleet_health.rs` — fleet health tracking, StartupReport handling
- `crates/racecontrol/src/preset_library.rs` — preset CRUD + reliability scoring
- `crates/racecontrol/src/game_launcher.rs` — GameTracker, launch state management
- `crates/racecontrol/src/whatsapp_alerter.rs` — WhatsApp notification dispatch
- `crates/racecontrol/src/db/mod.rs` — SQLite DB schema and migrations

### Integration Points
- WS handler: add match arms for GameInventoryUpdate and ComboValidationReport
- Fleet health: extend with crash_loop flag
- Preset library: extend list_presets_with_reliability to include fleet_validity
- WhatsApp alerter: new alert types for crash loop and chain failure

</code_context>

<specifics>
## Specific Ideas

No specific requirements — refer to ROADMAP success criteria.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
