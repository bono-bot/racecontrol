# Phase 172: Standing Rules Sync - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Propagate relevant standing rules from racecontrol's CLAUDE.md to all active repos, sync to Bono's VPS repos, and create an automated compliance check script.

</domain>

<decisions>
## Implementation Decisions

### Rule Propagation Strategy
- racecontrol CLAUDE.md is the canonical source of standing rules
- Each repo gets a RELEVANT SUBSET — not a full copy (kiosk doesn't need Rust rules, comms-link doesn't need deploy rules)
- Rule categories: Deploy, Comms, Code Quality, Process, Debugging, Security
- Each repo's CLAUDE.md should reference racecontrol as canonical source

### Repo Categories and Applicable Rules
- **Node.js repos** (comms-link, kiosk, admin, api-gateway, WhatsApp/Discord bots, MCP servers): Code Quality (TS rules), Process, Comms
- **Rust repos** (pod-agent): Code Quality (Rust rules), Deploy, Debugging
- **Ops repos** (deploy-staging): Deploy, Process
- **Bono VPS repos**: Comms, Code Quality, Process (synced via comms-link relay)

### Compliance Script
- Bash script that checks each repo for required rule sections
- Exits 0 with "All repos compliant" or non-zero listing gaps
- Located in deploy-staging or racecontrol repo

### Claude's Discretion
- Exact rule subset per repo (based on repo type)
- Compliance script implementation details
- How to handle repos without existing CLAUDE.md (create vs skip)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- racecontrol CLAUDE.md — full standing rules (canonical source)
- comms-link relay — for syncing to Bono VPS

### Established Patterns
- Standing rule: "After modifying CLAUDE.md, sync to Bono via comms-link"
- Git config normalized in Phase 170

### Integration Points
- All repos under C:/Users/bono/racingpoint/
- Bono VPS repos accessed via comms-link relay exec
- Each repo has its own .git directory

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
