---
phase: 170-repo-hygiene-dependency-audit
verified: 2026-03-23T14:30:00+05:30
status: passed
score: 9/9 must-haves verified
re_verification: false
---

# Phase 170: Repo Hygiene & Dependency Audit Verification Report

**Phase Goal:** Dead repos are archived, non-git folders are catalogued, all active repos have consistent git config and .gitignore, all npm and cargo dependencies are audited for vulnerabilities
**Verified:** 2026-03-23T14:30:00+05:30 IST
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | game-launcher, ac-launcher, conspit-link archived on GitHub | VERIFIED | `gh repo view --json isArchived` returns `true` for all three |
| 2 | Each archived repo has README with archive notice and merge target | VERIFIED | All three README.md files contain "ARCHIVED — 2026-03-23" header and point to racecontrol or rc-agent |
| 3 | Non-git folders (7) each have documented decision (archive/delete/keep) | VERIFIED | 170-NON-GIT-CATALOGUE.md has all 7 sections: bat-sandbox, computer-use, glitch-frames, marketing, serve, voice-assistant, skills |
| 4 | Every active repo has git user.name = "James Vowles" | VERIFIED | `git config user.name` returns "James Vowles" in all 16 repos |
| 5 | Every active repo has git user.email = "james@racingpoint.in" | VERIFIED | `git config user.email` returns "james@racingpoint.in" in all 16 repos |
| 6 | Every active repo .gitignore excludes node_modules, .env, credentials.json | VERIFIED | grep check passed for all 16 repos; Rust repos also have `target/`; Node.js repos also have `dist/`/`.next/` |
| 7 | npm audit run on all 13 Node.js repos, high/critical documented or fixed | VERIFIED | 170-DEPENDENCY-AUDIT.md npm table covers all 13 repos; 7 highs fixed via `npm audit fix`; remaining deferred with explicit rationale |
| 8 | cargo audit run on both Rust repos, vulnerabilities documented or fixed | VERIFIED | RUSTSEC-2026-0049 patched in both racecontrol and pod-agent (cargo update rustls-webpki 0.103.9→0.103.10); RUSTSEC-2023-0071 deferred with rationale |
| 9 | Outdated packages (2+ major versions) flagged with upgrade-or-defer decisions | VERIFIED | @types/node v20→v25 (defer) and googleapis v144→v171 (defer) documented in "Outdated Packages" section |

**Score:** 9/9 truths verified

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `170-NON-GIT-CATALOGUE.md` | Catalogue of non-git folders with disposition decisions | VERIFIED | Exists, 7 sections with Decision and Rationale fields for each folder |
| `170-GIT-CONFIG-REPORT.md` | Report confirming git config consistency across all repos | VERIFIED | Exists, contains before/after table for all 16 repos, "james@racingpoint.in" present |
| `170-DEPENDENCY-AUDIT.md` | Complete dependency audit report with vulnerability findings and decisions | VERIFIED | Exists, contains npm Audit Summary + Cargo Audit Summary sections, all repos covered |
| `game-launcher/README.md` | Archive notice explaining code merged to racecontrol | VERIFIED | Contains "ARCHIVED — 2026-03-23" and racecontrol link |
| `ac-launcher/README.md` | Archive notice explaining AC launcher complete | VERIFIED | Contains "ARCHIVED — 2026-03-23" and racecontrol link |
| `conspit-link/README.md` | Archive notice explaining roadmap-only, handled in rc-agent | VERIFIED | Contains "ARCHIVED — 2026-03-23" and rc-agent note |
| `.gitignore` in all 16 repos | node_modules, .env, credentials.json excluded | VERIFIED | All 16 repos pass grep check; Rust-specific (target/) and Node.js-specific (.next/, dist/) entries also present |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| GitHub repos | archive status | `gh repo archive --yes` | WIRED | All three repos return `isArchived: true` from GitHub API |
| git config --local | user.name / user.email | per-repo .git/config | WIRED | All 16 repos return "James Vowles / james@racingpoint.in" |
| npm audit | vulnerability count | audit output parsing | WIRED | Findings documented with high/critical/moderate columns per repo; fixes committed and pushed |
| cargo audit | vulnerability count | advisory DB (982 advisories) | WIRED | racecontrol and pod-agent both audited; rustls-webpki patch committed and pushed |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| REPO-01 | 170-01 | Dead repos archived with README noting merge target | SATISFIED | GitHub isArchived=true for all three; READMEs exist with archive notices |
| REPO-02 | 170-01 | Non-git folders catalogued and archived/deleted | SATISFIED | 170-NON-GIT-CATALOGUE.md documents all 7 folders with decisions; note: actual deletion/archival deferred to Uday approval for "delete" items, which is by design |
| REPO-03 | 170-02 | All active repos have consistent git config and .gitignore | SATISFIED | All 16 repos pass git config check; all 16 .gitignore files contain required entries |
| DEPS-01 | 170-03 | npm audit run on all Node.js repos, vulnerabilities patched | SATISFIED | All 13 repos in audit table; fixes applied and committed in 3 repos |
| DEPS-02 | 170-03 | cargo audit run on all Rust crates, vulnerabilities patched | SATISFIED | Both Rust repos audited; RUSTSEC-2026-0049 patched via cargo update |
| DEPS-03 | 170-03 | Outdated packages flagged with upgrade-or-defer decision | SATISFIED | 2 outdated packages documented with defer rationale in "Outdated Packages" section |

**REPO-04 and REPO-05** appear in REQUIREMENTS.md mapped to Phase 174 (not Phase 170) — correctly out of scope for this phase.

---

## Anti-Patterns Found

No blockers or warnings detected.

| File | Pattern | Severity | Impact |
|------|---------|----------|--------|
| 170-DEPENDENCY-AUDIT.md | Cargo outdated tool not installed — manual review used instead | INFO | Minor: cargo outdated crates reviewed manually from Cargo.toml rather than via tooling. Coverage adequate for key crates (sqlx, tokio, reqwest). |

---

## Human Verification Required

None. All verification objectives for this phase are programmatically confirmable:
- GitHub archive status via `gh` CLI
- Git config via `git config` commands
- .gitignore contents via grep
- Dependency audit findings via file content

---

## Gaps Summary

No gaps. All 9 observable truths verified. All 6 requirements satisfied. All 3 artifacts are substantive (not stubs) and wired to their purpose.

Notable decisions documented:
- REPO-02 completion: actual deletion/archival of "delete/archive" folders requires Uday's sign-off — the phase correctly treated this as documentation-only per the plan's explicit instruction ("Do NOT actually delete or archive any folders in this task")
- discord-bot undici deferred: transitive via discord.js 14.25.1 (latest), no fix available upstream — correctly documented as a track item
- rsa RUSTSEC-2023-0071 deferred: no fix available in sqlx-mysql, no RSA operations exposed in service — reasonable risk acceptance

---

_Verified: 2026-03-23T14:30:00+05:30 IST_
_Verifier: Claude (gsd-verifier)_
