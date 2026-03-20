# Phase 51: CLAUDE.md + Custom Skills - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Claude Code sessions always start with full Racing Point context — pod IPs, crate names, naming conventions, constraints — and James can trigger structured deploy and incident workflows with single slash commands. No manual copy-paste of context.

Deliverables: 1 CLAUDE.md file + 4 custom skills (/rp:deploy, /rp:deploy-server, /rp:pod-status, /rp:incident). All files live in the racecontrol repo.

</domain>

<decisions>
## Implementation Decisions

### CLAUDE.md Scope
- CLAUDE.md **replaces MEMORY.md** as the primary context source for Racing Point operations
- Include **everything** a new Claude session needs: full network map (IPs + MACs), crate names, binary naming, deploy rules, billing rates, 4-tier debug order, brand identity, constraints, security cameras, server services
- Include **full tables** — pod IPs with MACs, camera IPs, server ports — Claude can answer any network question without memory lookups
- Include **standing process rules** — Refactor Second, No Fake Data, Cross-Process Updates, Prompt Quality Check, Learn From Past Fixes, Bono comms protocol, deploy rules
- MEMORY.md shrinks to: James Vowles identity, Bono relationship, Uday info, timezone preference, feedback memories, current milestones, open issues, recent commits (~60 lines)
- CLAUDE.md lives at repo root: `racecontrol/CLAUDE.md` (auto-loaded by Claude Code on session start)

### /rp:deploy (rc-agent build + stage)
- `disable-model-invocation: true` — user-only, never auto-triggered
- Full sequence: `cargo build --release --bin rc-agent` → size check → copy to `C:\Users\bono\racingpoint\deploy-staging\rc-agent.exe` → run verify script (binary size check)
- Outputs the pendrive deploy command for James to run manually
- Does NOT push to any pod — canary gate is in Phase 53 (DEPLOY-03)

### /rp:deploy-server (racecontrol build + swap + full pipeline)
- `disable-model-invocation: true` — user-only
- **Full pipeline**: `cargo build --release --bin racecontrol` → kill old racecontrol process automatically (no confirmation prompt — James invoked deliberately) → swap binary → start new process → verify :8080 returns 200 → git commit → notify Bono via comms-link INBOX.md
- Kill is automatic because James explicitly invoked the skill
- Git commit + Bono notification included in the skill (not deferred to HOOK-02)

### /rp:pod-status (pod state query)
- Model-invocable (read-only, safe to auto-trigger during conversations)
- Queries `/api/v1/fleet/health` and extracts the specific pod's data
- Dynamic IP injection from pod number → IP mapping in the skill

### /rp:incident (structured incident response)
- Model-invocable (can be auto-triggered when James describes a pod problem)
- **Auto-query + auto-fix**: queries affected pod status from /fleet/health, identifies likely issue from 4-tier debug order, suggests specific fix command
- **Confirm for destructive only**: read-only queries run automatically; process kills, restarts, and billing actions require James to confirm
- **Auto-logs to LOGBOOK**: after fix is confirmed working, appends timestamped IST entry with incident description + resolution. Combines incident response + logging in one workflow.
- Falls back to guide-only mode if racecontrol server is unreachable

### Claude's Discretion
- Exact CLAUDE.md section ordering and formatting
- How to handle MEMORY.md migration (which memories to keep vs move)
- Skill file naming conventions within `.claude/skills/`
- Error handling patterns within skills (what to show on failure)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Claude Code Skills
- `.planning/research/FEATURES.md` — Feature landscape with table stakes, anti-features, and skill design patterns (disable-model-invocation, skill budget, dynamic context injection)
- `.planning/research/STACK.md` — SKILL.md format, frontmatter fields, Claude Code official docs patterns

### Current Context Sources (to migrate into CLAUDE.md)
- `C:\Users\bono\.claude\projects\C--Users-bono\memory\MEMORY.md` — Current 280+ line context file (source material for CLAUDE.md)
- `.planning/codebase/STRUCTURE.md` — Repo structure, crate layout, file locations
- `.planning/codebase/CONVENTIONS.md` — Rust naming conventions, error handling patterns, workspace deps

### Existing Configuration
- `C:\Users\bono\.claude\settings.json` — Current hooks (SessionStart, PostToolUse) and MCP server config
- `.planning/research/ARCHITECTURE.md` — Where skills/CLAUDE.md fit in the system architecture

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- No existing `.claude/skills/` directory — creating from scratch
- No existing `CLAUDE.md` — creating from scratch
- MEMORY.md has comprehensive Racing Point context that can be migrated directly

### Established Patterns
- Existing hooks in `~/.claude/settings.json` use `node` commands — skills are separate (markdown files in repo)
- GSD skills already installed globally (`~/.claude/get-shit-done/`) — project skills go in `racecontrol/.claude/skills/`
- Existing MCP servers show the pattern for Claude Code integration (stdio transport, env vars for auth)

### Integration Points
- `CLAUDE.md` at repo root — auto-loaded by Claude Code when working directory is racecontrol
- `.claude/skills/*.md` — discovered by Claude Code skill system
- Deploy scripts reference `C:\Users\bono\racingpoint\deploy-staging\` — skills must use same paths
- `/api/v1/fleet/health` on server :8080 — queried by /rp:pod-status and /rp:incident
- comms-link `INBOX.md` at `C:\Users\bono\racingpoint\comms-link\INBOX.md` — notified by /rp:deploy-server

</code_context>

<specifics>
## Specific Ideas

- CLAUDE.md should be comprehensive enough that a fresh Claude session with zero MEMORY.md can still operate the venue
- /rp:deploy-server is a "one-button" server update — build, swap, verify, commit, notify Bono. James should never need to remember the sequence.
- /rp:incident should feel like a senior engineer debugging — auto-queries, identifies the issue, proposes the fix, asks confirmation only for destructive actions, then logs what happened

</specifics>

<deferred>
## Deferred Ideas

- HOOK-01 (SessionStart context re-injection after compaction) — v9.x future
- HOOK-02 (PostToolUse auto-notify Bono on git commits) — v9.x future, partially addressed by /rp:deploy-server including Bono notification
- /rp:logbook as a standalone skill — partially absorbed into /rp:incident auto-logging
- /rp:fleet-health (summarize all pod states) — v9.x future
- /rp:new-pod-config (generate pod TOML) — v9.x future

</deferred>

---

*Phase: 51-claude-md-custom-skills*
*Context gathered: 2026-03-20*
