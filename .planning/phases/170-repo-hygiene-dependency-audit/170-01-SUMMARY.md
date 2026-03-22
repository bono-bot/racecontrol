---
phase: 170-repo-hygiene-dependency-audit
plan: 01
subsystem: infra
tags: [github, git, archival, repo-hygiene]

requires: []
provides:
  - "Three dead GitHub repos archived (game-launcher, ac-launcher, conspit-link) with README notices"
  - "Non-git folder catalogue with disposition decisions for 7 folders"
affects: [170-02]

tech-stack:
  added: []
  patterns: []

key-files:
  created:
    - .planning/phases/170-repo-hygiene-dependency-audit/170-NON-GIT-CATALOGUE.md
    - game-launcher/README.md (external repo)
    - ac-launcher/README.md (external repo)
    - conspit-link/README.md (external repo)
  modified: []

key-decisions:
  - "game-launcher archived: functionality merged into racecontrol v13.0 Multi-Game Launcher"
  - "ac-launcher archived: AC launcher complete, no further development planned"
  - "conspit-link archived: roadmap-only repo, never implemented — hardware handled in rc-agent"
  - "marketing folder: keep — 5.8GB of venue media, requires Uday approval before any action"
  - "skills folder: keep — 668MB of active Claude agent dev reference material"
  - "glitch-frames + serve: safe to delete immediately — diagnostic artifacts and empty stub"
  - "bat-sandbox, computer-use, voice-assistant: archive to deploy-staging/archive/ — historical reference value"

patterns-established: []

requirements-completed: [REPO-01, REPO-02]

duration: 15min
completed: 2026-03-23
---

# Phase 170 Plan 01: Archive Dead Repos and Catalogue Non-Git Folders Summary

**Three GitHub repos archived (game-launcher, ac-launcher, conspit-link) with README notices; 7 non-git folders catalogued with archive/delete/keep decisions**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-23T~12:30:00+05:30
- **Completed:** 2026-03-23T~12:45:00+05:30
- **Tasks:** 2
- **Files modified:** 4 (3 external READMEs + 1 catalogue)

## Accomplishments

- Archived game-launcher, ac-launcher, and conspit-link on GitHub — all three return `isArchived: true`
- Created README.md in each archived repo explaining where the code lives now (racecontrol) and why it was archived
- Created 170-NON-GIT-CATALOGUE.md documenting all 7 non-git folders with decision + rationale

## Task Commits

1. **Task 1: Archive dead repos on GitHub with README updates**
   - `7cbc983` in game-launcher — `docs: add archive notice to README`
   - `ec29063` in ac-launcher — `docs: add archive notice to README`
   - `16856f0` in conspit-link — `docs: add archive notice to README`
   - Repos archived via `gh repo archive --yes`

2. **Task 2: Catalogue non-git folders** - `e58504cd` (chore)

**Plan metadata:** (docs commit — next)

## Files Created/Modified

- `C:/Users/bono/racingpoint/game-launcher/README.md` — Archive notice, points to racecontrol v13.0
- `C:/Users/bono/racingpoint/ac-launcher/README.md` — Archive notice, AC complete, points to racecontrol
- `C:/Users/bono/racingpoint/conspit-link/README.md` — Archive notice, roadmap-only, points to rc-agent
- `.planning/phases/170-repo-hygiene-dependency-audit/170-NON-GIT-CATALOGUE.md` — Disposition decisions for 7 non-git folders

## Decisions Made

- **marketing folder**: keep — 5.8GB of venue photos/videos/strategy docs that belong to Uday. Do not touch without his explicit approval.
- **skills folder**: keep — Active reference library (668MB) for Claude agent development including Anthropic SDK, cookbooks, MCP servers.
- **glitch-frames**: delete — Diagnostic PNGs from a resolved kiosk investigation. No value.
- **serve**: delete — Empty directory stub. Zero risk.
- **bat-sandbox, computer-use, voice-assistant**: archive — Historical reference value, compress to deploy-staging/archive/ then delete originals.

## Deviations from Plan

None — plan executed exactly as written. The three repos had no existing README.md files (only .git and .planning directories), so new READMEs were created from scratch as the plan specified.

## Issues Encountered

None. All three repos were git-initialized and connected to james-racingpoint org remotes. GitHub archival via `gh repo archive --yes` succeeded without prompts.

## User Setup Required

None — no external service configuration required beyond what was already set up.

## Next Phase Readiness

- Phase 170-01 complete. All repo archival done.
- Non-git catalogue is ready for action: two folders (glitch-frames, serve) can be deleted immediately.
- Three folders (bat-sandbox, computer-use, voice-assistant) await compression to archive before deletion.
- Marketing folder decision deferred to Uday — note in MEMORY.md recommended.

---
*Phase: 170-repo-hygiene-dependency-audit*
*Completed: 2026-03-23*
