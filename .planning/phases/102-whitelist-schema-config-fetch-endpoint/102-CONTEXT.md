# Phase 102: Whitelist Schema + Config + Fetch Endpoint - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Populate racecontrol.toml with a deny-by-default process whitelist covering all Racing Point machines. Add per-machine override sections. Implement the ProcessGuardConfig deserialization in racecontrol and expose a GET endpoint for pods/James to fetch their merged whitelist. No enforcement logic — just config and serving.

</domain>

<decisions>
## Implementation Decisions

### Whitelist Content
- Baseline whitelist from kiosk.rs ALLOWED_PROCESSES (60+ entries) plus known James/server processes
- James-specific: Ollama, node, python, VS Code, comms-link processes, cargo, deploy-staging HTTP server, webterm
- Pod-specific: rc-agent, ConspitLink, game processes (acs.exe, AssettoCorsa.exe, etc.), Edge kiosk
- Server-specific: racecontrol, kiosk (Next.js), web dashboard, node
- System processes: Windows core (svchost, csrss, lsass, etc.), NVIDIA drivers (nv*.exe), Logitech drivers
- Steam explicitly NOT whitelisted on any machine

### TOML Schema Design
- Array of inline tables: `[[process_guard.allowed]]` with fields: `name`, `category`, `machines` (array of "pod", "james", "server", or "all")
- Wildcard syntax: simple glob with `*` only (e.g., `nv*.exe`) — basic string matching, no regex
- violation_action: `"report_only"` (default), `"kill_and_report"`, `"monitor"` — three modes
- `poll_interval_secs = 60` in `[process_guard]` section
- Per-machine override sections: `[process_guard.overrides.james]`, `[process_guard.overrides.pod]`, `[process_guard.overrides.server]`
- Categories: system, racecontrol, game, peripheral, ollama, development, monitoring

### HTTP Endpoint Design
- `GET /api/v1/guard/whitelist/{machine_id}` where machine_id = `pod-1`..`pod-8`, `james`, `server`
- Returns merged `MachineWhitelist` JSON (global entries filtered by machine + machine-specific overrides combined)
- No auth — internal network only, consistent with fleet/health endpoint

### Claude's Discretion
- ProcessGuardConfig struct field names and deserialization approach
- How to store the parsed config in AppState (Arc<RwLock> or similar)
- Wildcard matching implementation (simple contains vs glob crate)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- kiosk.rs ALLOWED_PROCESSES — 60+ entries, categorized in comments
- Config deserialization patterns in racecontrol/src/config.rs
- AppState pattern with Arc fields for shared state
- Existing Axum route handlers in racecontrol/src/routes/

### Established Patterns
- TOML config deserialized via serde into typed structs
- Config loaded at startup, stored in AppState
- API routes use Axum extractors (State, Path)
- JSON responses via axum::Json

### Integration Points
- racecontrol.toml — add [process_guard] section
- racecontrol config.rs — add ProcessGuardConfig to main Config struct
- racecontrol routes — add guard whitelist endpoint
- AppState — store parsed whitelist config

</code_context>

<specifics>
## Specific Ideas

- Use the existing ALLOWED_PROCESSES list from kiosk.rs as the starting point for the pod whitelist
- Default to report_only mode so first deploy doesn't kill anything
- The whitelist must be comprehensive enough that a clean pod produces zero violations on first scan

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
