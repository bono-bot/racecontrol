---
phase: 170-repo-hygiene-dependency-audit
plan: "02"
subsystem: repo-hygiene
tags: [git, gitignore, hygiene, cross-repo]
dependency_graph:
  requires: []
  provides: [consistent-git-identity, secret-protection-gitignore]
  affects: [all-16-active-repos]
tech_stack:
  added: []
  patterns: [per-repo-git-local-config, additive-gitignore-updates]
key_files:
  created:
    - .planning/phases/170-repo-hygiene-dependency-audit/170-GIT-CONFIG-REPORT.md
    - C:/Users/bono/racingpoint/pod-agent/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-mcp-calendar/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-mcp-sheets/.gitignore
  modified:
    - C:/Users/bono/racingpoint/racecontrol/.gitignore
    - C:/Users/bono/racingpoint/comms-link/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-admin/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-api-gateway/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-discord-bot/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-google/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-mcp-drive/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-mcp-gmail/.gitignore
    - C:/Users/bono/racingpoint/racingpoint-whatsapp-bot/.gitignore
    - C:/Users/bono/racingpoint/rc-ops-mcp/.gitignore
    - C:/Users/bono/racingpoint/whatsapp-bot/.gitignore
    - C:/Users/bono/racingpoint/deploy-staging/.gitignore
    - C:/Users/bono/racingpoint/people-tracker/.gitignore
decisions:
  - "Git config fixed in 4 repos only via local .git/config — no commits needed (config is non-tracked)"
  - "people-tracker has no git remote — push skipped; this is expected for a local-only repo"
  - ".gitignore updates are additive only — existing entries preserved, new section appended under Phase 170 label"
metrics:
  duration: "18 minutes"
  completed: "2026-03-23T01:12:00+05:30"
  tasks_completed: 2
  files_changed: 16
---

# Phase 170 Plan 02: Git Config & .gitignore Normalization Summary

Git config and .gitignore normalized across all 16 active Racing Point repos — identity consistent and secrets excluded from all repos.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Normalize git config across all active repos | e4598bda | 170-GIT-CONFIG-REPORT.md |
| 2 | Ensure comprehensive .gitignore in every active repo | 4025e791 (racecontrol) + 15 others | 16 .gitignore files |

## What Was Done

### Task 1: Git Config Normalization

Audited all 16 repos for correct `user.name` / `user.email`. Found 4 repos with unset config:

- `racingpoint-api-gateway` — no local config (was inheriting global or empty)
- `racingpoint-discord-bot` — no local config
- `racingpoint-mcp-drive` — no local config
- `racingpoint-mcp-gmail` — no local config

Set `user.name = "James Vowles"` and `user.email = "james@racingpoint.in"` in all 4.
12 repos were already correct.

This is a local `.git/config` change only — no commits needed.

### Task 2: .gitignore Normalization

Applied comprehensive .gitignore entries across all 16 repos without removing any existing entries:

**Created new .gitignore files (3 repos had none):**
- `pod-agent` — Rust repo: node_modules, secrets, OS, IDE, target/, *.exe
- `racingpoint-mcp-calendar` — Node.js: full standard set
- `racingpoint-mcp-sheets` — Node.js: full standard set

**Updated existing .gitignore files (13 repos):**
- All appended under `# Added by Phase 170 — repo hygiene` section
- Added: `.env.*`, `!.env.example`, `*.pem`, `*.key`, `credentials.json`, `token.json`, `.DS_Store`, `Thumbs.db`, `desktop.ini`, `.vscode/settings.json`, `.idea/`
- Node.js repos also got: `dist/`, `build/`, `.next/`, `out/`, `*.tsbuildinfo`
- Rust repos also got: `target/`, `*.exe`, `!*.exe.manifest`
- `racecontrol` and `racingpoint-admin` were already comprehensive — minimal additions only

**Committed and pushed in each repo** (per standing rule).
Exception: `people-tracker` has no git remote — commit made locally, push skipped (expected).

## Verification Results

All 16 repos pass acceptance criteria:

```
comms-link: node_modules=OK .env=OK credentials=OK
pod-agent: node_modules=OK .env=OK credentials=OK
racecontrol: node_modules=OK .env=OK credentials=OK
racingpoint-admin: node_modules=OK .env=OK credentials=OK
racingpoint-api-gateway: node_modules=OK .env=OK credentials=OK
racingpoint-discord-bot: node_modules=OK .env=OK credentials=OK
racingpoint-google: node_modules=OK .env=OK credentials=OK
racingpoint-mcp-calendar: node_modules=OK .env=OK credentials=OK
racingpoint-mcp-drive: node_modules=OK .env=OK credentials=OK
racingpoint-mcp-gmail: node_modules=OK .env=OK credentials=OK
racingpoint-mcp-sheets: node_modules=OK .env=OK credentials=OK
racingpoint-whatsapp-bot: node_modules=OK .env=OK credentials=OK
rc-ops-mcp: node_modules=OK .env=OK credentials=OK
whatsapp-bot: node_modules=OK .env=OK credentials=OK
deploy-staging: node_modules=OK .env=OK credentials=OK
people-tracker: node_modules=OK .env=OK credentials=OK
```

## Deviations from Plan

None — plan executed exactly as written.

Note: `people-tracker` push failed with "No configured push destination" — this is expected (local-only repo with no remote), not a failure.

## Commits

| Repo | Commit Hash | Message |
|------|-------------|---------|
| racecontrol | e4598bda | chore(170-02): normalize git config across 16 repos, create GIT-CONFIG-REPORT |
| racecontrol | 4025e791 | chore: normalize .gitignore for repo hygiene (phase 170) |
| comms-link | 70d9331 | chore: normalize .gitignore for repo hygiene (phase 170) |
| pod-agent | a7da03b | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-admin | a3e1978 | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-api-gateway | 55eed66 | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-discord-bot | bfc2626 | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-google | e644b9c | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-mcp-calendar | 8d788c5 | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-mcp-drive | bb3449b | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-mcp-gmail | 55fa944 | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-mcp-sheets | cb92e12 | chore: normalize .gitignore for repo hygiene (phase 170) |
| racingpoint-whatsapp-bot | 5144e73 | chore: normalize .gitignore for repo hygiene (phase 170) |
| rc-ops-mcp | c5d1236 | chore: normalize .gitignore for repo hygiene (phase 170) |
| whatsapp-bot | 3d8dfae | chore: normalize .gitignore for repo hygiene (phase 170) |
| deploy-staging | 506777c | chore: normalize .gitignore for repo hygiene (phase 170) |
| people-tracker | 277ec10 | chore: normalize .gitignore for repo hygiene (phase 170) |
