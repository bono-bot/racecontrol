# Phase 190: Phase Scripts Tiers 1-9 (Sequential Baseline) - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Port all v3.0 AUDIT-PROTOCOL phases 1-34 (Tiers 1-9) as non-interactive bash functions in `audit/phases/tierN/phaseNN.sh`. Each function uses lib/core.sh primitives (emit_result, http_get, safe_remote_exec, safe_ssh_capture). Add mode-based tier selection (--mode quick/standard/full/pre-ship/post-incident) and per-tier/per-phase selectors (--tier N, --phase N) to audit.sh. Phase 01 already exists from Phase 189.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase (mechanical port of existing bash commands).

Key constraints:
- Source of truth: AUDIT-PROTOCOL.md at repo root (1928 lines, all 60 phases with bash commands)
- Phase 01 (Fleet Inventory) already implemented in Phase 189 — skip it, port phases 02-34
- Each phaseNN.sh must: source lib/core.sh, define `run_phaseNN()`, use `emit_result` for output, always return 0
- QUIET status for venue-closed checks (display, hardware tiers)
- Timeout via curl -m and --connect-timeout flags (10s default)
- Tier mapping from v3.0:
  - Tier 1 (1-10): Infrastructure Foundation
  - Tier 2 (11-16): Core Services
  - Tier 3 (17-20): Display & UX
  - Tier 4 (21-25): Billing & Commerce
  - Tier 5 (26-29): Games & Hardware
  - Tier 6 (30-34): Notifications & Marketing
  - Tier 7 (35-38): Data & Sync
  - Tier 8 (39-42): Advanced Systems
  - Tier 9 (43-44): Cameras & AI
- Mode mapping:
  - quick: Tiers 1-2 (phases 1-16)
  - standard: Tiers 1-11 (phases 1-50)
  - full: All 18 tiers (phases 1-60)
  - pre-ship: Phases 1, 51, 53, 57, 46, 48-50, 58
  - post-incident: Phases 1, 8, relevant tier, 48-50, 60

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `audit/audit.sh` — entry point with mode parsing, prereq checks, auth token (Phase 189)
- `audit/lib/core.sh` — all 8 shared primitives (Phase 189)
- `audit/phases/tier1/phase01.sh` — reference implementation pattern (Phase 189)
- `AUDIT-PROTOCOL.md` — all 60 phase bash commands (source of truth for porting)

### Established Patterns
- Phase function signature: `run_phaseNN() { ... }` sourcing lib/core.sh
- All results via `emit_result "$PHASE" "$TIER" "$HOST" "$STATUS" "$SEVERITY" "$MESSAGE"`
- Pod loop: `for IP in $PODS; do ... done`
- Venue-closed check: `[[ "$VENUE_STATE" == "closed" ]] && STATUS="QUIET"`
- Always `return 0` — errors encoded in JSON, not exit codes

### Integration Points
- audit.sh `load_phases()` sources each phaseNN.sh
- audit.sh dispatch calls `run_phaseNN` based on mode/tier selection
- Phase 01 already wired in audit.sh — new phases follow same pattern

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond mechanical porting. Each phase script should be a faithful conversion of the corresponding AUDIT-PROTOCOL.md section, using lib/core.sh functions instead of raw curl/ssh.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
