# Phase 142: Rules Hygiene - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Reorganize CLAUDE.md standing rules from a flat numbered list into named categories. Prune obsolete/duplicate rules. Add one-line justification to each. Sync standing-rules.md memory file.

Files to modify:
- C:/Users/bono/racingpoint/racecontrol/CLAUDE.md (Standing Process Rules section)
- C:/Users/bono/.claude/projects/C--Users-bono/memory/standing-rules.md

</domain>

<decisions>
## Implementation Decisions

### Rule Categories
Organize into these sections:
- **Deploy** — binary builds, staging, pod-8 canary, verification sequence
- **Comms** — Bono messaging, INBOX.md, auto-push, deploy notifications, v18.0 exec default
- **Code Quality** — Rust no .unwrap(), TypeScript no any, .bat CRLF, static CRT, cascade updates
- **Process** — refactor second, cross-process updates, no fake data, prompt quality, learn from past
- **Debugging** — 4-tier debug order, cross-process recovery awareness, E2E before shipped, lifecycle logging
- **Security** — allowlist auth, anti-cheat safe mode notes

### Pruning Candidates
- Deploy rules 4+5 (clean old binaries, latest builds priority) — redundant with verification sequence
- Standing rule 8 (Bono deploy updates as atomic sequence) — merge into rule 7 (auto-push)
- Separate "Deployment Rules" and "Standing Process Rules" sections should be merged into one categorized block

### Claude's Discretion
- Exact wording of justification comments
- Whether to use subsections or a flat categorized list
- How to handle the numbered vs unnumbered inconsistency

</decisions>

<code_context>
## Existing Code Insights

### Current State
- CLAUDE.md has 2 rule sections: "Deployment Rules" (9 rules) + "Standing Process Rules" (15 rules) = 24 total
- standing-rules.md memory has sections A-H with different organization
- Some rules appear in both files with different wording
- Rules 10-15 in Standing Process Rules were all added in the last week from debugging incidents

### Integration Points
- CLAUDE.md — auto-loaded by Claude Code when CWD = racecontrol
- standing-rules.md — loaded into conversation context via MEMORY.md
- comms-link/CLAUDE.md — shared rules (already has its own standing rules section)

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond the category structure above.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
