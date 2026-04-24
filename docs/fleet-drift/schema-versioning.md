# Fleet Manifest -- Schema Versioning Policy

Status: Active (v1.0 initial, effective 2026-04-24)
Applies to: `schemas/fleet-manifest.schema.json`
Companion of: SCHEMA-02 in `.planning/REQUIREMENTS-v53.md`

## Current Version

`schema_version: "1.0"` -- initial version shipped in Phase 447. Every manifest MUST carry this field as a top-level string matching pattern `^\d+\.\d+$`.

## Forward-Compat Guarantee

`additionalProperties: true` is set on the root AND inside every nested object in `fleet-manifest.schema.json`. A manifest written under a future `schema_version` (e.g. `2.0`) with extra fields at root or nested level MUST validate cleanly against the v1 schema. Tools running old schema + new manifest = no error.

This is load-bearing for milestone longevity: probe authors can add fields (e.g. `kernel_version`, `npm_pkg_sha256`) without a coordinated `schema_version` bump, and older readers silently ignore them.

## Version Bump Semantics

### Patch (1.0 -> 1.1)
- Add optional field at root or nested -- no required-array change
- Add new enum value (role, probe_status) -- readers MUST handle unknown enum values defensively (treat as "other")
- Clarify descriptions or regex patterns if strictly looser
- No migration doc required

### Minor (1.x -> 1.(x+1))
- Add required field -- old manifests now fail validation
- Tighten pattern or enum -- old values may be rejected
- Migration doc MANDATORY at `docs/fleet-drift/migrations/v1.x-to-v1.y.md` with before/after example + conversion snippet

### Major (1.x -> 2.0)
- Rename required field -- structural break
- Remove required field -- consumers must adapt
- Change semantics of an enum value (e.g. `role: "pod"` now means something different)
- Migration doc MANDATORY + 30-day dual-emit window where probes write BOTH versions

## Unknown-Field Handling Contract

Every parser consuming manifests under this schema MUST:

1. Use a library or parser mode that tolerates extra properties (jsonschema-python default, ajv with strict:false, serde_json with default-deny explicitly disabled).
2. Preserve unknown fields when rewriting (never drop-and-resave silently).
3. Log unknown fields at debug level with their path -- enables audit of field drift.

Consumers that set `additionalProperties: false` in a local derived schema are OUT OF SPEC and must be corrected.

## Enum Drift Policy

`role` and `probe_status` are closed enums in v1. Adding a value (e.g. `role: "pwa"`) is a 1.x patch bump. Readers MUST treat unknown enum values as `{ role: "other", original_role: "<value>" }` rather than rejecting.

## Version Deprecation Timeline

- v1.x is supported indefinitely while any fleet target emits it
- v2.0 dual-emit window: 30 days from first v2 probe deploy, every probe writes both `v1-compat.json` and `v2.json`
- No silent drops: removing a v1.x field without a migration doc is a P0 DIFF-01 entry fleetwide

## See Also

- `schemas/fleet-manifest.schema.json` -- the schema itself
- `.planning/REQUIREMENTS-v53.md` -- SCHEMA-02 requirement
- Plan `447-02-PLAN.md` -- example manifests exercising v1.0 fields
- Plan `447-03-PLAN.md` -- validator test proving additionalProperties:true works

---

*Versioning policy locked 2026-04-24 as part of Phase 447. Amendments require a planning phase bump (v53.x) not an ad-hoc edit.*
