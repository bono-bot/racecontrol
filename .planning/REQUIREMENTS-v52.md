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

- [x] **FND-01**: Decide repo name, init strategy (rename memory repo vs fresh), secret boundary, Bono authority model (pull-only vs bidirectional). **✓ Locked Phase 393 (2026-04-15):** `workspace` repo under Uday GitHub, bidirectional peer model, `~/.claude-secrets/` explicit, 6-check CI gate, squash-merge + 24h wip GC.
- [x] **FND-02a**: Resolve existing CGP drift on `cgp-enforce.js` and `cgp-session-inject.js` (superset files). **✓ Complete Phase 394 (2026-04-15):** James superset-wins for both files. Canonical text in `memory/decision_cgp_drift_resolution.md`.
- [ ] **FND-02b**: Resolve 6 remaining drifted hooks (gsd-check-update, gsd-context-monitor, gsd-prompt-guard, gsd-statusline, gsd-workflow-guard, memory-staleness-check) + classify 16 James-only + 4 Bono-only hooks into cross-platform / windows-only / linux-only buckets. Produces install.sh manifest.
- [ ] **FND-03**: Write `ARCHITECTURE.md` + `CONVENTIONS.md` — the rules the whole migration follows. Every convention names its mechanical enforcer or gets deleted.
- [ ] **FND-04a**: Uday repo creation gate (human action) + write `.github/workflows/ci.yml` with 6 checks (node --check, bash -n, parity probe, secret scan, file size, orphan check) + `sync/pre-commit` hook.
- [ ] **FND-04b**: Clone fresh `workspace` repo; `.gitignore` blocks secrets + session state; commit skeleton; first `cgp-distribution-probe.js` run from skeleton must be green on empty state.

### Low-Risk Migration (MIG)

- [ ] **MIG-01**: Migrate `memory/scripts/cgp-distribution-probe.js` + key recovery scripts → `workspace/scripts/` and update references
- [ ] **MIG-02**: Migrate memory files to `workspace/memory/` via dry-run branch; create `memory/INDEX.md` (CI check #6 enforces orphan-free); update auto-memory path in global CLAUDE.md in same commit
- [ ] **MIG-03**: Create `workspace/tests/` with one test per canonical hook (pre-flight-file-read, g9-auto-detect, backlog-enforce, cgp-enforce, cgp-session-inject)
- [ ] **MIG-04**: Secrets boundary migration — move `comms-link.env`, OpenRouter keys, PSK, relay keys from `~/.claude/` into `~/.claude-secrets/` on BOTH machines; grep-update every reader; verify `.gitignore` + pre-commit blocklist prevent re-drift
- [ ] **MIG-05**: Migrate `~/.claude/agents/` → `workspace/agents/` and `~/.claude/commands/` → `workspace/commands/` per D-8; update install.sh manifest

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

Restructured 2026-04-16 (Option A: +4 phases — 395 drift-remainder, 397 repo-gate+CI split, 401 secrets split, 402 agents/commands).

| REQ-ID | Phase | Status |
|---|---|---|
| FND-01 | 393 | Locked (awaiting Bono ratification) |
| FND-02a | 394 | ✓ Complete 2026-04-15 |
| FND-02b | 395 | Not started |
| FND-03 | 396 | Not started |
| FND-04a | 397 | Blocked on Uday repo creation |
| FND-04b | 398 | Blocked on 397 |
| MIG-01 | 399 | Not started |
| MIG-02 | 400 | Not started |
| MIG-04 | 401 | Not started |
| MIG-05 | 402 | Not started |
| MIG-03 | 403 | Not started |
| HOOK-01 | 404 | Not started |
| HOOK-02 | 405 | Blocked on Bono ratification |
| HOOK-03 | 406 | Blocked on 405 + Bono ratification |
| HOOK-04 | 407 | Not started |
| CLN-01 | 408 | Not started |
| CLN-02 | 409 | Not started |
| CLN-03 | 410 | Not started |
| CLN-04 | 411 | Not started |
| CLN-05 | 412 | Not started |
