# Phase 447: Manifest Schema & Scope Lock - Context

**Gathered:** 2026-04-24
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped — infrastructure/schema-definition phase per autonomous workflow infrastructure-detect heuristic)

<domain>
## Phase Boundary

Publish `schemas/fleet-manifest.schema.json` covering every field that downstream phases (448 probes, 450 build graph, 451 deploy graph, 452 diff tool) must read or write. Establish the on-disk layout `state/fleet-manifest/<iso-ts>/<target_id>.json` + `_meta.json` summary index. Lock `schema_version` forward-compat rules so future schema evolution does not break prior manifests.

**What this phase delivers:**
- `schemas/fleet-manifest.schema.json` — JSON Schema draft 2020-12 defining per-target manifest shape
- `schemas/fleet-manifest.example.json` — one valid example per target class (server/pod/pos/james/vps/cloud-admin/cloud-rc/relay)
- `docs/fleet-drift/schema-versioning.md` — forward-compat rules (unknown-field tolerance, version bump semantics)
- `state/fleet-manifest/.gitkeep` + `.gitignore` entries for runtime manifest output (ephemeral state, not committed)
- Unit tests validating example manifests against the schema (Node ajv or equivalent JSON-schema validator)

**What this phase does NOT deliver:**
- Any probe scripts (Phase 448)
- Any graph or diff tooling (Phase 450-452)
- Runtime writers — Phase 448 probes will be the first consumers of the schema

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion

All implementation choices are at Claude's discretion — this is a pure infrastructure/schema phase with no design ambiguity. Constraints are fully captured in SCHEMA-01/02/03 requirement IDs in `.planning/REQUIREMENTS-v53.md`. Reasonable defaults:

- **Schema format:** JSON Schema draft 2020-12 (current standard, good tooling support)
- **Location in repo:** `schemas/` top-level dir (consistent with existing `schemas/` pattern in repo; check `grep -r 'schemas/' crates/ scripts/' for precedent)
- **Validator:** Node `ajv` (already a transitive dep via existing tooling) or Python `jsonschema` (if Python preferred for probe scripts)
- **Versioning:** semver-like — `schema_version: "1.0"` initial. Unknown fields tolerated via `additionalProperties: true` on root and nested objects per SCHEMA-02.
- **Timestamp format:** ISO-8601 UTC with millisecond precision (to match existing `tracing` log convention); separately carry `probed_at_ist` for human readability per project timezone rule
- **Target ID format:** `{role}_{index}` (e.g. `server_23`, `pod_1`, `pos_130`, `james_27`, `bono_vps`, `cloud_admin`, `cloud_racecontrol`, `relay_james`, `relay_vps`) — flat namespace, lowercase, underscore-separated

### Schema Fields (locked by SCHEMA-01)

From REQUIREMENTS-v53.md SCHEMA-01, required fields:
- `target_id` (string)
- `host` (string — hostname or IP)
- `ip` (string — primary IP)
- `role` (enum: server, pod, pos, james, vps, cloud_admin, cloud_racecontrol, relay)
- `probed_at_ist` (string — ISO-8601 with `+05:30` offset)
- `probe_status` (enum: ok, probe_failed, partial)
- `binary_sha256` (object: `{filename: sha256_hex}` — multiple binaries per target)
- `build_id` (string — Rust `env!("GIT_HASH")` for binaries that embed it; null otherwise)
- `config_hash` (object: `{config_file: sha256_hex}`)
- `running_procs` (array of `{name, pid, cmdline_hash}`)
- `scheduled_tasks` (array of `{name, state, last_run, next_run}`)
- `autostart_entries` (array of `{source: "HKLM_Run"|"HKCU_Run"|"startup_folder"|"schtask", key, value}`)
- `env_vars_hash` (sha256 of sorted env var names only — no values, security boundary)
- `last_deploy_ts` (string — ISO-8601 IST)

Plus `schema_version` (SCHEMA-02) at the root.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- Existing `schemas/` convention in repo (if present — planner should grep). Memory references `schemas/drift-report.schema.json` (Phase 452) and `schemas/fleet-targets.json` (future) — so `schemas/` is the established location.
- Rust `env!("GIT_HASH")` pattern (standing rule: "Release builds always produce fresh GIT_HASH") gives us `build_id` for rc-agent, rc-sentry, racecontrol binaries for free.
- Existing `tasklist /V /FO CSV` pattern (standing rule: "Audit must verify Session context") is the Windows convention for `running_procs` on pod/POS/james/server targets.
- Existing `schtasks /Query /V /FO LIST` pattern for `scheduled_tasks`.
- `reg query HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` pattern for `autostart_entries`.
- Existing SWAPLOG.md at repo root — will feed DIFF-05 sha→commit lookup in Phase 452.

### Established Patterns
- JSON output for machine-readable data (drift.json, manifest JSON, audit_known_issues seeds) — consistent across existing tooling.
- IST timezone convention — `probed_at_ist` uses `+05:30` offset, NOT UTC. Memory: "Racecontrol logs are UTC; all operations are IST."
- ASCII-only for scripts on Windows (memory: `feedback_ascii_only_script_constraint.md`) — schema itself is platform-agnostic but supporting tooling must respect this.

### Integration Points
- Phase 448 probe scripts will write manifests matching this schema to `state/fleet-manifest/<iso-ts>/<target_id>.json`
- Phase 451 deploy graph reads these files as input
- Phase 452 diff tool reads both build and deploy graphs; manifest schema is the data format for deploy graph input
- Phase 453 ground-truth validation depends on schema stability
- `.gitignore` must be updated to exclude `state/fleet-manifest/*/` runtime output (keep `.gitkeep` only)

</code_context>

<specifics>
## Specific Ideas

- **Security boundary:** `env_vars_hash` must be a hash of sorted env VAR NAMES only, never values. Standing rule: "Secrets (`.env`, API keys) MUST NOT be in any auto-push repo." Even hashed values could leak entropy.
- **probe_status = partial:** A target that reachable but where some sub-probes failed (e.g. SSH ok but `tasklist` errored) should be `partial` with a `probe_errors[]` array listing which sub-probes failed. Full schema coverage so the diff tool can make sensible decisions on partial data.
- **Forward-compat test:** Include a test that loads a manifest with a field NOT in the schema, and verifies the validator accepts it (SCHEMA-02's core guarantee). This catches future regressions where someone accidentally sets `additionalProperties: false`.
- **Example manifests as documentation:** Each target-class example should be a realistic snapshot — not `"TODO"` placeholders. Use values from actual fleet state (Server .23 IP, Pod 1 Tailscale IP from CLAUDE.md Network Map, etc.) so examples double as target-class reference.

</specifics>

<deferred>
## Deferred Ideas

- **Manifest signing / tamper-evidence** — out of scope per PROJECT.md. Would add `signature` field; deferred to v53.1+.
- **Time-series retention** — deferred to v53.1 per REQUIREMENTS-v53.md Future Requirements.
- **Contract drift between typed-API (Phase 445) and probe output** — deferred to v53.1 per REQUIREMENTS-v53.md Future Requirements.
- **Target registry at `schemas/fleet-targets.json`** — the manifest schema defines shape, but which TARGETS exist is a separate registry. Minimal v1 treats target list as hardcoded in probe scripts (Phase 448); formal registry deferred unless Phase 448 finds it blocking.

</deferred>

---

*Phase: 447-manifest-schema-scope-lock*
*Context gathered: 2026-04-24 via autonomous workflow infrastructure-detect path. REQ-IDs: SCHEMA-01, SCHEMA-02, SCHEMA-03 (tracked in `.planning/REQUIREMENTS-v53.md`).*
