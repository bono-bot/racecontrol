# Phase 191: Parallel Engine and Phase Scripts Tiers 10-18 - Context

**Gathered:** 2026-03-25
**Status:** Ready for planning

<domain>
## Phase Boundary

Build lib/parallel.sh with file-based semaphore (max 4 concurrent connections), pod_loop helper, 200ms launch stagger. Port remaining 16 v3.0 phases (45-60, Tiers 10-18) as non-interactive bash functions. Integrate parallel engine into audit.sh for full-mode runs. Target: full audit under 8 minutes.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase (parallel engine + mechanical port of remaining bash commands from AUDIT-PROTOCOL.md).

Key constraints:
- Source of truth: AUDIT-PROTOCOL.md (phases 45-60 sections)
- Semaphore: file-based lock in /tmp or $RESULT_DIR, max 4 concurrent
- `wait -n` for background job coordination (bash 4.3+)
- 200ms stagger between pod query launches to avoid ARP flood
- Each phaseNN.sh follows established pattern: `run_phaseNN()`, `emit_result`, always return 0
- Tier mapping (Tiers 10-18):
  - Tier 10 (45-47): Ops & Compliance (Log Health, Comms-Link E2E, Standing Rules)
  - Tier 11 (48-50): E2E Journeys (Customer, Staff/POS, Security/Auth)
  - Tier 12 (51-53): Code Quality (Static Analysis, Frontend Deploy, Binary Consistency)
  - Tier 13 (54): Registry & Relay Integrity
  - Tier 14 (55-56): Data Integrity Deep (DB Migration, LOGBOOK/OpenAPI)
  - Tier 15 (57): Full Test Suites
  - Tier 16 (58): Cloud & Cross-Boundary E2E
  - Tier 17 (59): Customer & Staff Flow E2E
  - Tier 18 (60): Cross-System Chain E2E
- Parallel engine used in phases that loop over pods; server-only phases stay sequential

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `audit/audit.sh` — entry point with mode parsing, load_phases(), tier dispatch
- `audit/lib/core.sh` — 8 shared primitives (emit_result, http_get, safe_remote_exec, etc.)
- `audit/phases/tier1-9/` — 44 phase scripts (established pattern to follow)
- `AUDIT-PROTOCOL.md` — all 60 phase bash commands (source of truth for porting)

### Established Patterns
- Phase function: `run_phaseNN() { local phase="NN" tier="T"; ... emit_result ...; return 0 }`
- Pod loop: `for ip in $PODS; do host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"; ... done`
- Venue-closed: `[[ "$venue_state" = "closed" ]] && emit_result ... "QUIET" ... && return 0`
- Token usage: `local token; token=$(get_session_token)` with `-H "x-terminal-session: ${token:-}"`
- No `set -e`, uses `set -u` + `set -o pipefail`

### Integration Points
- audit.sh `load_phases()` needs tier10-18 directories added
- audit.sh mode dispatch needs full mode to call tiers 10-18
- Parallel engine sourced alongside other libs, called from phases with pod loops

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond mechanical porting + parallel engine. Phase scripts are faithful conversions of AUDIT-PROTOCOL.md sections. Parallel engine wraps existing pod loop pattern.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
