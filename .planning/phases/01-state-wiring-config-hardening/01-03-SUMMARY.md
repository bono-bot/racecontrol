---
phase: 01-state-wiring-config-hardening
plan: 03
subsystem: infra
tags: [toml, deploy, rc-agent, config, deploy_pod]

# Dependency graph
requires:
  - phase: 01-state-wiring-config-hardening
    provides: rc-agent config validation (ConfigError lock screen, AgentConfig struct)
provides:
  - Fixed deploy template that generates AgentConfig-compatible TOML for all 8 pods
  - Closes DEPLOY-04: deployed configs now deserialize successfully via toml::from_str
affects:
  - Phase 4 (deployment hardening — all pod deploys use this template)
  - Any future pod config generation via deploy_pod.py

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "TOML section names in template must exactly match serde Deserialize field names in Rust structs"
    - "Template variables ({pod_number}, {pod_name}) are substituted by deploy_pod.py .replace() — template structure must match target struct, not just contain the variables"

key-files:
  created: []
  modified:
    - deploy/rc-agent.template.toml
    - ../deploy-staging/rc-agent.template.toml

key-decisions:
  - "Template uses [pod] with number/name/sim and [core] with url — matching AgentConfig/PodConfig/CoreConfig serde Deserialize layout exactly"
  - "sim defaults to assetto_corsa in template (primary game at venue) — not a per-pod variable"
  - "deploy_pod.py script left unchanged — template fix sufficient, script logic correct"

patterns-established:
  - "Template field names must match Rust struct field names exactly — no aliases without serde rename"

requirements-completed: [DEPLOY-04]

# Metrics
duration: 8min
completed: 2026-03-13
---

# Phase 1 Plan 3: Deploy Config Template Fix Summary

**Fixed rc-agent.template.toml to use [pod]/[core] sections matching AgentConfig serde layout, closing DEPLOY-04 gap where deployed configs would fail TOML deserialization**

## Performance

- **Duration:** ~10 min (including human checkpoint verification)
- **Started:** 2026-03-13T~00:50:00Z
- **Completed:** 2026-03-13T~01:00:00Z
- **Tasks:** 2 of 2 (both complete, including Pod 8 human verification)
- **Files modified:** 2

## Accomplishments
- Replaced `[agent]` section (wrong) with `[pod]` section using correct field names `number`, `name`, `sim`
- Added `[core]` section with `url` field (was `server_url` under `[agent]` — wrong section and field name)
- Added missing required `sim = "assetto_corsa"` field (was entirely absent from old template)
- Fixed field name mismatches: `pod_number` -> `number`, `pod_name` -> `name`
- Copied fix to deploy-staging/ operational copy (both copies now identical)
- Verified with Python assertion tests for pod 1 and pod 8 boundary cases

## Task Commits

Each task was committed atomically:

1. **Task 1: Fix rc-agent.template.toml to match AgentConfig struct** - `a796132` (fix)
2. **Task 2: Verify deployed config on Pod 8** - checkpoint:human-verify approved (config written 1337 bytes, rc-agent started without errors)

**Plan metadata:** `c3c2567` (docs: SUMMARY and STATE for deploy config template fix)

## Files Created/Modified
- `deploy/rc-agent.template.toml` - Fixed TOML structure: [pod] + [core] sections matching AgentConfig struct
- `../deploy-staging/rc-agent.template.toml` - Operational copy (identical to deploy/ version)

## Decisions Made
- Template uses `sim = "assetto_corsa"` as a fixed value since all pods run AC as primary game — not a per-pod variable
- deploy_pod.py script left unchanged — the substitution logic (`.replace()`) is correct; only the template structure needed fixing

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Unicode box-drawing characters (`─`) in template comment header caused `UnicodeEncodeError` when printing to Windows console (cp1252 codec). Worked around by removing the print from the verification script — assertions still passed cleanly.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- DEPLOY-04 fully closed: Pod 8 confirmed rc-agent started with correct [pod]/[core] config (1337 bytes, no errors)
- Phase 1 (State Wiring & Config Hardening) is complete — all 3 plans done (01-01, 01-02, 01-03)
- Phase 2 ready: Watchdog Hardening — pod_monitor/pod_healer use shared backoff, post-restart verification, email alerts

---
*Phase: 01-state-wiring-config-hardening*
*Completed: 2026-03-13*
