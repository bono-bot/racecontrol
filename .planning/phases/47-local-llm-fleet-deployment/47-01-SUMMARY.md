---
phase: 47-local-llm-fleet-deployment
plan: 01
subsystem: infra
tags: [ollama, ai-debugger, modelfile, rp-debug, qwen3, debug-memory, fleet-deploy]

# Dependency graph
requires:
  - phase: 46-crash-safety-panic-hook
    provides: ai_debugger.rs with PodErrorContext + try_auto_fix() patterns 1-7
provides:
  - deploy-staging/Modelfile for rp-debug model with all 14 diagnostic keyword patterns
  - deploy-staging/seed-debug-memory.sh to pre-seed 7 fix patterns on all 8 pods
  - training/Modelfile corrected server IP (.51 -> .23)
affects:
  - 47-02 (ollama-health E2E references rp-debug model built from this Modelfile)
  - Phase 50 (future: wire auto-fix code for patterns 8-14 added informally here)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Modelfile DIAGNOSTIC KEYWORDS section directly mirrors try_auto_fix() pattern matching — keywords must stay in sync as new patterns are added
    - seed-debug-memory.sh uses /write endpoint (not /exec) as primary path — cleaner than shell-escaping JSON through PowerShell
    - Each INCIDENT_N on its own bash variable line for grep-countability and maintainability

key-files:
  created:
    - deploy-staging/Modelfile
    - deploy-staging/seed-debug-memory.sh
  modified:
    - training/Modelfile

key-decisions:
  - "deploy-staging/Modelfile uses FROM qwen3:0.6b (Ollama model), not GGUF path — deploy fleet uses Ollama pull, not QLoRA-trained custom model"
  - "Modelfile num_predict set to 512 (same as training/Modelfile) — gives model space for multi-step diagnostic reasoning"
  - "seed-debug-memory.sh uses python3 for JSON assembly and escaping — avoids bash heredoc quoting pitfalls with nested JSON"
  - "Patterns 8-14 in Modelfile are informational only — try_auto_fix() code will be wired in Phase 50"
  - "DebugIncident pattern_key format verified against ai_debugger.rs pattern_key() fn: '{SimType:?}:{exit_code}'"

patterns-established:
  - "Pattern: DIAGNOSTIC KEYWORDS in Modelfile must 1:1 match try_auto_fix() string checks — add to both simultaneously"
  - "Pattern: seed scripts source tests/e2e/lib/common.sh + pod-map.sh for consistent pod iteration and pass/fail/skip reporting"

requirements-completed: [LLM-03, LLM-04]

# Metrics
duration: 4min
completed: 2026-03-19
---

# Phase 47 Plan 01: Local LLM Fleet Deployment Summary

**rp-debug Modelfile with all 14 diagnostic keywords + debug-memory.json seed script covering 7 deterministic AC/F1/fleet crash patterns**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-19T01:39:19Z
- **Completed:** 2026-03-19T01:44:00Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Created deploy-staging/Modelfile for `rp-debug` model (FROM qwen3:0.6b) with full 14-pattern DIAGNOSTIC KEYWORDS section aligned with ai_debugger.rs try_auto_fix()
- Created deploy-staging/seed-debug-memory.sh: writes DebugMemory JSON to all 8 pods via :8090/write with /exec+PowerShell fallback — enables instant fix replay from first boot
- Fixed training/Modelfile server IP: .51 -> .23 (was incorrect, now matches actual server address)

## Task Commits

Each task was committed atomically:

1. **Task 1: Expand deploy-staging/Modelfile + fix training/Modelfile IP** - `53a3eab` (feat)
2. **Task 2: Create seed-debug-memory.sh** - `6e8d07e` (feat)

## Files Created/Modified

- `deploy-staging/Modelfile` - rp-debug Ollama model definition with 14 DIAGNOSTIC KEYWORDS matching try_auto_fix() patterns 1-7 (auto-fix) and 8-14 (informational for Phase 50)
- `deploy-staging/seed-debug-memory.sh` - Fleet seeder: 7 DebugIncident entries (AC crash -1, AC frozen, F125 DX12 crash, WerFault, CLOSE_WAIT, disk space, USB wheelbase), writes via /write endpoint with /exec fallback
- `training/Modelfile` - Corrected server IP from .51 to .23

## Decisions Made

- Modelfile uses `FROM qwen3:0.6b` (standard Ollama pull) — deploy fleet gets the base model, not the QLoRA-fine-tuned GGUF (that's for training/ only)
- num_predict 512 in deploy Modelfile matches training Modelfile — gives model room for multi-step reasoning without wasting tokens
- seed-debug-memory.sh uses python3 for JSON payload construction — eliminates nested quoting problems when embedding JSON in bash curl -d
- Patterns 8-14 added to Modelfile as INFORMATIONAL — try_auto_fix() code additions deferred to Phase 50 per plan spec

## Deviations from Plan

None - plan executed exactly as written. The deploy-staging/Modelfile did not exist (plan said "current file only has 4 patterns" — the file didn't exist at all), so it was created fresh with all 14 patterns.

## Issues Encountered

None - straightforward file creation and edit. Verification confirmed all 14 keyword patterns present, 7 incidents in seed script, and both Modelfiles now reference server .23.

## User Setup Required

None - no external service configuration required. Run `bash deploy-staging/seed-debug-memory.sh` when pods are running to apply the seed.

## Next Phase Readiness

- Phase 47-02 (ollama-health.sh E2E) can verify the rp-debug model is deployed correctly — uses the Modelfile created here
- Phase 50 can wire auto-fix code for patterns 8-14 (DirectX, shader cache, OOM, DLL, Steam, FPS, network)
- seed-debug-memory.sh is ready to run against the fleet immediately

---
*Phase: 47-local-llm-fleet-deployment*
*Completed: 2026-03-19*
