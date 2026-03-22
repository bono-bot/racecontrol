# Requirements: v21.0 Cross-Project Sync & Stabilization

**Defined:** 2026-03-23
**Core Value:** All Racing Point repos work in sync — shared contracts, no dead code, no known bugs, unified deploy, every service verified running.

## v21.0 Requirements

### Repo Hygiene

- [ ] **REPO-01**: Dead repos (game-launcher, ac-launcher, conspit-link) are archived with README noting "Archived — code merged into racecontrol"
- [ ] **REPO-02**: Non-git folders (bat-sandbox, computer-use, glitch-frames, marketing, serve, voice-assistant) are catalogued and archived or deleted
- [ ] **REPO-03**: All active repos have consistent git config (user.name, user.email) and .gitignore
- [ ] **REPO-04**: Every active repo has latest code built and deployed to its target environment
- [ ] **REPO-05**: Every deployed service verified running at runtime (API responds, UI renders) — not just compiled

### Bug Fixes

- [ ] **BUG-01**: racecontrol auto-seeds pods table on startup when empty (pods DB desync fix)
- [ ] **BUG-02**: start-rcagent.bat kills orphan powershell.exe on boot (deployed to all 8 pods)
- [ ] **BUG-03**: Process guard allowlist built from live pod scan, enabled in report_only mode
- [ ] **BUG-04**: Variable_dump.exe killed on pod boot via start-rcagent.bat (deployed to all 8 pods)

### E2E Testing

- [ ] **E2E-01**: All 231 E2E tests from E2E-TEST-SCRIPT.md executed on POS (:3200)
- [ ] **E2E-02**: All 231 E2E tests from E2E-TEST-SCRIPT.md executed on Kiosk (:8000)
- [ ] **E2E-03**: Cross-cutting real-time sync tests pass (POS action reflected on Kiosk and vice versa)
- [ ] **E2E-04**: All test failures triaged, critical failures fixed, remaining documented as known issues

### API Contracts

- [ ] **CONT-01**: All API boundaries documented (racecontrol <-> kiosk, racecontrol <-> admin, racecontrol <-> comms-link, racecontrol <-> rc-agent)
- [ ] **CONT-02**: Shared TypeScript types/interfaces extracted for racecontrol <-> kiosk API communication
- [ ] **CONT-03**: Shared TypeScript types/interfaces extracted for racecontrol <-> admin API communication
- [ ] **CONT-04**: OpenAPI specs generated for racecontrol REST API endpoints
- [ ] **CONT-05**: Contract tests validate request/response shapes between services
- [ ] **CONT-06**: CI check prevents API drift (contract test runs on PR)

### Deployment

- [ ] **DEPL-01**: deploy-staging cleaned up (714 dirty files triaged — keep, delete, or .gitignore)
- [ ] **DEPL-02**: Unified deploy script covers all services (racecontrol, rc-agent, kiosk, web dashboard, comms-link)
- [ ] **DEPL-03**: Deployment runbook documents step-by-step for each service with rollback procedures
- [ ] **DEPL-04**: deploy-staging committed and pushed with clean git status

### Standing Rules

- [ ] **RULE-01**: CLAUDE.md standing rules synced to all active repos (relevant subset per repo)
- [ ] **RULE-02**: Bono's VPS repos updated with matching standing rules
- [ ] **RULE-03**: Standing rules compliance check script (automated, runnable before any ship)

### Dependency Audit

- [ ] **DEPS-01**: npm audit run on all Node.js repos, security vulnerabilities patched
- [ ] **DEPS-02**: cargo audit run on all Rust crates, vulnerabilities patched
- [ ] **DEPS-03**: Outdated packages flagged with upgrade-or-defer decision documented

### Health Monitoring

- [ ] **HLTH-01**: Every running service exposes a /health endpoint
- [ ] **HLTH-02**: Central health check script polls all services and reports status
- [ ] **HLTH-03**: Health check integrated into deploy verification (post-deploy auto-check)

## v2 Requirements

None — this milestone is stabilization, not feature work.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Monorepo migration | Too disruptive — would break all existing CI, deploy, and dev workflows |
| Duplicate WhatsApp bot consolidation | Separate concern, needs Bono coordination, not blocking stability |
| New feature development | This milestone is purely stabilization and sync |
| Performance optimization | Focus on correctness first, optimize later |
| people-tracker status | Single-commit repo, needs Uday input on direction |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| REPO-01 | TBD | Pending |
| REPO-02 | TBD | Pending |
| REPO-03 | TBD | Pending |
| REPO-04 | TBD | Pending |
| REPO-05 | TBD | Pending |
| BUG-01 | TBD | Pending |
| BUG-02 | TBD | Pending |
| BUG-03 | TBD | Pending |
| BUG-04 | TBD | Pending |
| E2E-01 | TBD | Pending |
| E2E-02 | TBD | Pending |
| E2E-03 | TBD | Pending |
| E2E-04 | TBD | Pending |
| CONT-01 | TBD | Pending |
| CONT-02 | TBD | Pending |
| CONT-03 | TBD | Pending |
| CONT-04 | TBD | Pending |
| CONT-05 | TBD | Pending |
| CONT-06 | TBD | Pending |
| DEPL-01 | TBD | Pending |
| DEPL-02 | TBD | Pending |
| DEPL-03 | TBD | Pending |
| DEPL-04 | TBD | Pending |
| RULE-01 | TBD | Pending |
| RULE-02 | TBD | Pending |
| RULE-03 | TBD | Pending |
| DEPS-01 | TBD | Pending |
| DEPS-02 | TBD | Pending |
| DEPS-03 | TBD | Pending |
| HLTH-01 | TBD | Pending |
| HLTH-02 | TBD | Pending |
| HLTH-03 | TBD | Pending |

**Coverage:**
- v21.0 requirements: 28 total (updated to 32 with RULE/DEPS/HLTH)
- Mapped to phases: 0
- Unmapped: 32

---
*Requirements defined: 2026-03-23*
*Last updated: 2026-03-23 after initial definition*
