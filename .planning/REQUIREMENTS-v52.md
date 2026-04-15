# Milestone v52.0 — Claude Workspace Restructure

**Status:** Defining
**Started:** 2026-04-15
**Owners:** James + Bono (joint — parity is a core requirement)
**Source:** `~/.claude/projects/C--Users-bono/memory/project_workspace_restructure.md`

## Goal

Consolidate scattered Claude-side artifacts (~14 classes, 8+ locations) into a single canonical workspace repo with deterministic James↔Bono sync, automatic drift detection, and a single answer to "where does a new X go?"

## Core Non-Negotiable Success Criterion

**James and Bono VPS must be in sync** — not eventually-consistent, but deterministic parity verified by `cgp-distribution-probe.js` before every phase is closed. The probe's 100%-parity output on cross-platform hooks is the binary gate between "phase done" and "phase not done."

## Fixed Constraints (walls we cannot move)

1. Hooks load from `~/.claude/hooks/` (Claude Code hardcoded)
2. Settings load from `~/.claude/settings.json` (hardcoded)
3. Per-repo CLAUDE.md files are a feature — preserve them
4. Skills load from `~/.claude/skills/` (hardcoded)
5. Secrets (`.env`, API keys) MUST NOT be in any auto-push repo
6. Windows + Linux must both work (James + Bono)

Any requirement that breaks one of these is invalid.

## Requirements

### Foundation & Decisions (FND)

- [ ] **FND-01**: Decide repo name, init strategy (rename memory repo vs fresh), secret boundary, Bono authority model (pull-only vs bidirectional)
- [ ] **FND-02**: Resolve existing CGP drift on `cgp-enforce.js` and `cgp-session-inject.js` BEFORE canonical migration (do not migrate drift)
- [ ] **FND-03**: Write `ARCHITECTURE.md` + `CONVENTIONS.md` — the rules the whole migration follows
- [ ] **FND-04**: Initialize workspace repo skeleton with auto-push to Bono VPS + GitHub mirror, verify `.gitignore` blocks secrets

### Low-Risk Migration (MIG)

- [ ] **MIG-01**: Migrate `memory/scripts/cgp-distribution-probe.js` → `workspace/scripts/` and update references
- [ ] **MIG-02**: Migrate memory files to `workspace/memory/` via dry-run branch; update auto-memory path in global CLAUDE.md in same commit
- [ ] **MIG-03**: Create `workspace/tests/` with one test per canonical hook (pre-flight-file-read, g9-auto-detect, backlog-enforce, cgp-enforce, cgp-session-inject)

### Hook Sync (HOOK)

- [ ] **HOOK-01**: Write `sync/install-hooks.sh` and `sync/verify-parity.sh` tooling; test on Git Bash (Windows) and bash (Linux)
- [ ] **HOOK-02**: Migrate canonical hooks on James side with cross-platform / windows-only / linux-only split; backup `~/.claude/hooks/` before starting
- [ ] **HOOK-03**: Migrate canonical hooks on Bono VPS via git pull + install-hooks.sh; backup Bono hooks before overwriting
- [ ] **HOOK-04**: Probe-verified 100% parity check across James + Bono + bootstrap — this is the gate that proves James+Bono are in sync

### Cleanup & Decommission (CLN)

- [ ] **CLN-01**: Settings migration — extract shared `workspace/settings/base.json`, keep `settings.local.json` for machine-specific overrides, write `install-settings.sh`
- [ ] **CLN-02**: Bootstrap consolidation — move `claude-code-bootstrap/{vps,windows}/` → `workspace/bootstrap/` and delete the old directory
- [ ] **CLN-03**: Protocol doc pointers — decide cache vs pointer for CGP.md and MMA.md; update all CLAUDE.md references
- [ ] **CLN-04**: Decommission old paths — remove `claude-code-bootstrap/`, archive old memory git history as read-only tag, update ARCHITECTURE.md with new canonical paths
- [ ] **CLN-05**: Final parity audit + `MIGRATION-LOG.md` + Bono coordination sign-off + close milestone

## Out of Scope

- New Claude Code features (skills, MCP servers, subagents) — migrate-only, no net-new tooling beyond sync scripts
- Racecontrol/comms-link repos — they keep their own CLAUDE.md files (per-repo context is a feature)
- Secret management tooling — secrets stay in `~/.claude/` flat, outside the workspace repo (hard boundary)
- Per-OS CI runners (defer to post-v52 if needed)

## Future Requirements (deferred)

- GitHub Actions CI running `verify-parity.sh` on every workspace push
- Windows Service / systemd unit for automatic parity probe every N hours
- Bidirectional Bono↔James commit model (if pull-only proves too restrictive)

## Open Questions (answered in Phase 393 / FND-01)

Per source doc §"Open questions for next session's planning phase":
1. Repo name — workspace / claude-state / bono-workspace / other
2. Init from scratch vs evolve existing memory repo (git history tradeoff)
3. Bootstrap mirror fate — subdir vs own repo
4. CI y/n
5. Bono authority — pull-only vs bidirectional
6. Secret path — `~/.claude-secrets/` explicit vs scatter
7. Session state — ephemeral-outside vs ephemeral-inside
8. Subagents + slash commands — audit and join the workspace or not

## Traceability (filled by roadmap)

| REQ-ID | Phase |
|---|---|
| FND-01 | 393 |
| FND-02 | 394 |
| FND-03 | 395 |
| FND-04 | 396 |
| MIG-01 | 397 |
| MIG-02 | 398 |
| MIG-03 | 399 |
| HOOK-01 | 400 |
| HOOK-02 | 401 |
| HOOK-03 | 402 |
| HOOK-04 | 403 |
| CLN-01 | 404 |
| CLN-02 | 405 |
| CLN-03 | 406 |
| CLN-04 | 407 |
| CLN-05 | 408 |
