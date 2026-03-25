# Phase 181: Standing Rules Gate - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Codify all 41+ standing rules as machine-enforceable checks and wire them as pipeline gates. Every standing rule classified as AUTO, HUMAN-CONFIRM, or INFORMATIONAL. The pre-deploy gate blocks OTA wave 1 if any AUTO check fails. Post-deploy gate blocks subsequent waves. HUMAN-CONFIRM rules pause the pipeline with named checklists. No force-continue or skip-gate exists.

</domain>

<decisions>
## Implementation Decisions

### Standing rules classification document
- Create `standing-rules-registry.json` in repo root — machine-readable registry of all standing rules
- Each entry: `{ "id": "SR-DEPLOY-001", "category": "deploy", "summary": "...", "type": "AUTO|HUMAN-CONFIRM|INFORMATIONAL", "check_command": "..." }`
- AUTO rules have a `check_command` that returns exit code 0 (pass) or non-zero (fail)
- HUMAN-CONFIRM rules have a `checklist` array of items the operator must confirm
- INFORMATIONAL rules have no check — they're documentation-only guidance
- This file is the source of truth — gate-check.sh reads it

### Classification of the ~56 standing rules
Based on analysis of CLAUDE.md:

**AUTO (scriptable, ~18 rules):**
- No `.unwrap()` in diff — `grep -rn '\.unwrap()' <changed_files>`
- No `any` in TypeScript — `grep -rn ': any' web/src/`
- Static CRT config present — `grep -q 'crt-static' .cargo/config.toml`
- `.bat` files clean ASCII — `file *.bat | grep -v ASCII`
- cargo test green — `cargo test`
- LOGBOOK updated — `git diff HEAD~1 -- LOGBOOK.md | grep -q '+'`
- build.rs touched after code commits — timestamp check
- No force push to main
- Git config correct (james@racingpoint.in)
- No hardcoded IPs in new code (except network map)
- No fake data (real-looking identifiers)
- Standing rules synced to Bono — comms-link INBOX.md entry exists

**HUMAN-CONFIRM (need operator eyes, ~12 rules):**
- Visual verification for display-affecting deploys
- Audit what CUSTOMER sees (screens, overlays)
- Test display changes on ONE pod before fleet-wide
- Verify from user's browser (not server)
- Cross-process recovery awareness check
- First-run verification after enabling any guard/filter
- Investigate anomalies, don't dismiss them
- Context switches kill open investigations — park explicitly

**INFORMATIONAL (guidance, ~26 rules):**
- Smallest reversible fix first
- Have rollback plan before deploying
- Cause elimination before fix
- Learn from past fixes
- Prompt quality check
- Links and references = apply now
- Cross-process updates
- Refactor second
- And remaining process/debugging guidance

### gate-check.sh location and structure
- Located at `racecontrol/test/gate-check.sh`
- Extends (calls) `comms-link/test/run-all.sh` as one suite
- Additional suites: cargo tests, standing rule AUTO checks, diff analysis
- Exit code 0 = all gates pass, non-zero = blocked
- Reads `standing-rules-registry.json` for AUTO checks
- Outputs structured results (pass/fail per check with rule ID)
- Two modes:
  - `--pre-deploy` — runs before OTA wave 1 (full suite)
  - `--post-wave N` — runs after wave N completes (build_id verification, health checks, billing roundtrip)

### Pipeline integration with OTA state machine (Phase 179)
- gate-check.sh is called by the OTA pipeline at two integration points:
  1. Pre-deploy: before transitioning from `staging` to `canary` state
  2. Post-wave: after each wave completes, before advancing to next wave
- If gate-check.sh returns non-zero, pipeline transitions to `rolling_back` state
- No `--force` or `--skip-gate` flag exists — the ONLY exit is rollback
- HUMAN-CONFIRM rules cause pipeline to enter a `paused` state (new PipelineState variant)
  - Operator confirms via admin dashboard or REST endpoint
  - Pipeline resumes only after all checklist items confirmed

### New OTA standing rules for CLAUDE.md
Add under a new "### OTA Pipeline" subsection:
1. Always preserve previous binary before swap (rename, don't delete)
2. Never deploy without a signed manifest
3. Billing sessions must drain before binary swap on any pod
4. OTA sentinel file protocol: `OTA_DEPLOYING` sentinel during active deploy, cleared on complete/rollback
5. Config push NEVER goes through fleet exec endpoint — always through the dedicated config channel
6. Rollback window: previous binary preserved for 72 hours minimum

### Standing rules sync to Bono
- After modifying CLAUDE.md with new OTA rules, sync via comms-link
- gate-check.sh includes a check that standing rules were synced (INBOX.md entry within last 24h after CLAUDE.md modification)

### Claude's Discretion
- Exact shell scripting patterns in gate-check.sh
- Color coding and formatting of gate output
- Whether to add `paused` as a PipelineState variant or use existing `health_checking`
- Exact JSON schema for standing-rules-registry.json beyond the specified fields

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing test infrastructure
- `C:/Users/bono/racingpoint/comms-link/test/run-all.sh` — 3-suite E2E framework (contract + integration + syntax)
- `C:/Users/bono/racingpoint/comms-link/test/contract.test.js` — contract tests
- `C:/Users/bono/racingpoint/comms-link/test/integration.test.js` — integration tests

### Standing rules source
- `racecontrol/CLAUDE.md` lines 85-285 — all standing rules in 9 categories
- `C:/Users/bono/.claude/projects/C--Users-bono/memory/standing-rules.md` — memory file with standing rules

### OTA pipeline (Phase 179)
- `crates/racecontrol/src/ota_pipeline.rs` — OTA state machine, PipelineState enum, wave constants
- gate-check.sh must integrate with this state machine

### Sentinel files
- `C:\RacingPoint\MAINTENANCE_MODE` — existing sentinel (blocks restarts)
- `C:\RacingPoint\GRACEFUL_RELAUNCH` — existing sentinel (graceful vs crash)
- New: `C:\RacingPoint\OTA_DEPLOYING` — proposed OTA sentinel

### Requirements
- `.planning/REQUIREMENTS.md` — SR-01 through SR-07, SYNC-06

</canonical_refs>

<code_context>
## Existing Code Insights

### Test framework pattern (run-all.sh)
- 3 suites: contract (always), integration (when PSK set), syntax (always)
- Exit code 0 = pass, non-zero = fail
- Color-coded output
- gate-check.sh should follow same pattern but add cargo + standing rule suites

### OTA state machine states
- PipelineState: idle, building, staging, canary, staged_rollout, health_checking, completed, rolling_back
- May need new `paused` state for HUMAN-CONFIRM rules
- State transitions in ota_pipeline.rs

### Deploy staging
- `C:\Users\bono\racingpoint\deploy-staging\` — build staging area
- gate-check.sh runs here before binaries leave staging

</code_context>

<deferred>
## Deferred Ideas

- Auto-classification of new standing rules added in future (ML-based or keyword-based) — future enhancement
- Dashboard page showing gate check history and results — could be Phase 180.1
- Integration with CI/CD (GitHub Actions) — when cloud deploy pipeline is set up

</deferred>

---

*Phase: 181-standing-rules-gate*
*Context gathered: 2026-03-25*
