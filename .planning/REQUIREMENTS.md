# Requirements: v21.0 Cross-Project Sync & Stabilization

**Defined:** 2026-03-23
**Core Value:** All Racing Point repos work in sync — shared contracts, no dead code, no known bugs, unified deploy, every service verified running.

## v21.0 Requirements

### Repo Hygiene

- [x] **REPO-01**: Dead repos (game-launcher, ac-launcher, conspit-link) are archived with README noting "Archived — code merged into racecontrol"
- [x] **REPO-02**: Non-git folders (bat-sandbox, computer-use, glitch-frames, marketing, serve, voice-assistant) are catalogued and archived or deleted
- [x] **REPO-03**: All active repos have consistent git config (user.name, user.email) and .gitignore
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

- [x] **CONT-01**: All API boundaries documented (racecontrol <-> kiosk, racecontrol <-> admin, racecontrol <-> comms-link, racecontrol <-> rc-agent)
- [x] **CONT-02**: Shared TypeScript types/interfaces extracted for racecontrol <-> kiosk API communication
- [x] **CONT-03**: Shared TypeScript types/interfaces extracted for racecontrol <-> admin API communication
- [x] **CONT-04**: OpenAPI specs generated for racecontrol REST API endpoints
- [x] **CONT-05**: Contract tests validate request/response shapes between services
- [x] **CONT-06**: CI check prevents API drift (contract test runs on PR)

### Deployment

- [x] **DEPL-01**: deploy-staging cleaned up (714 dirty files triaged — keep, delete, or .gitignore)
- [x] **DEPL-02**: Unified deploy script covers all services (racecontrol, rc-agent, kiosk, web dashboard, comms-link)
- [x] **DEPL-03**: Deployment runbook documents step-by-step for each service with rollback procedures
- [x] **DEPL-04**: deploy-staging committed and pushed with clean git status

### Standing Rules

- [x] **RULE-01**: CLAUDE.md standing rules synced to all active repos (relevant subset per repo)
- [x] **RULE-02**: Bono's VPS repos updated with matching standing rules
- [x] **RULE-03**: Standing rules compliance check script (automated, runnable before any ship)

### Dependency Audit

- [x] **DEPS-01**: npm audit run on all Node.js repos, security vulnerabilities patched
- [x] **DEPS-02**: cargo audit run on all Rust crates, vulnerabilities patched
- [x] **DEPS-03**: Outdated packages flagged with upgrade-or-defer decision documented

### Health Monitoring

- [x] **HLTH-01**: Every running service exposes a /health endpoint
- [x] **HLTH-02**: Central health check script polls all services and reports status
- [x] **HLTH-03**: Health check integrated into deploy verification (post-deploy auto-check)

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
| REPO-01 | Phase 170 | Complete |
| REPO-02 | Phase 170 | Complete |
| REPO-03 | Phase 170 | Complete |
| REPO-04 | Phase 174 | Pending |
| REPO-05 | Phase 174 | Pending |
| BUG-01 | Phase 171 | Pending |
| BUG-02 | Phase 171 | Pending |
| BUG-03 | Phase 171 | Pending |
| BUG-04 | Phase 171 | Pending |
| RULE-01 | Phase 172 | Complete |
| RULE-02 | Phase 172 | Complete |
| RULE-03 | Phase 172 | Complete |
| CONT-01 | Phase 173 | Complete |
| CONT-02 | Phase 173 | Complete |
| CONT-03 | Phase 173 | Complete |
| CONT-04 | Phase 173 | Complete |
| CONT-05 | Phase 173 | Complete |
| CONT-06 | Phase 173 | Complete |
| HLTH-01 | Phase 174 | Complete |
| HLTH-02 | Phase 174 | Complete |
| HLTH-03 | Phase 174 | Complete |
| DEPL-01 | Phase 174 | Complete |
| DEPL-02 | Phase 174 | Complete |
| DEPL-03 | Phase 174 | Complete |
| DEPL-04 | Phase 174 | Complete |
| DEPS-01 | Phase 170 | Complete |
| DEPS-02 | Phase 170 | Complete |
| DEPS-03 | Phase 170 | Complete |
| E2E-01 | Phase 175 | Pending |
| E2E-02 | Phase 175 | Pending |
| E2E-03 | Phase 175 | Pending |
| E2E-04 | Phase 175 | Pending |

**Coverage:**
- v21.0 requirements: 32 total
- Mapped to phases: 32
- Unmapped: 0

| Phase | Requirements |
|-------|-------------|
| 170. Repo Hygiene & Dependency Audit | REPO-01, REPO-02, REPO-03, DEPS-01, DEPS-02, DEPS-03 |
| 171. Bug Fixes | BUG-01, BUG-02, BUG-03, BUG-04 |
| 172. Standing Rules Sync | RULE-01, RULE-02, RULE-03 |
| 173. API Contracts | CONT-01, CONT-02, CONT-03, CONT-04, CONT-05, CONT-06 |
| 174. Health Monitoring & Unified Deploy | HLTH-01, HLTH-02, HLTH-03, REPO-04, REPO-05, DEPL-01, DEPL-02, DEPL-03, DEPL-04 |
| 175. E2E Validation | E2E-01, E2E-02, E2E-03, E2E-04 |

---
*Requirements defined: 2026-03-23*
*Phase mapping added: 2026-03-23*
*Last updated: 2026-03-23 after initial definition*
