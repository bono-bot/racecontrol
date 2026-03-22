# Phase 170: Repo Hygiene & Dependency Audit - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Archive dead/merged repos (game-launcher, ac-launcher, conspit-link), catalogue and handle non-git folders, normalize git config across all active repos, and audit all npm + cargo dependencies for vulnerabilities.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.
- Archive method: README update + GitHub archive flag or just README
- Non-git folder handling: delete vs move to archive vs document-only
- Git config normalization approach
- Dependency audit tooling (npm audit, cargo audit, or alternatives)
- Vulnerability remediation priority (fix vs defer with documentation)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- deploy-staging/ contains fleet management scripts
- Each repo has its own package.json or Cargo.toml
- racecontrol CLAUDE.md has the standing rules reference

### Established Patterns
- Git config: user.name="James Vowles", user.email="james@racingpoint.in"
- .gitignore varies per repo — no standard template

### Integration Points
- All repos under C:/Users/bono/racingpoint/
- Some repos deployed to server .23, pods, or Bono VPS
- GitHub org: bono-bot (racecontrol), james-racingpoint

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Follow standing rules (auto-push, cross-process updates).

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
