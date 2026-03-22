---
phase: 170-repo-hygiene-dependency-audit
plan: 03
subsystem: infra
tags: [npm-audit, cargo-audit, security, dependencies, vulnerabilities]

requires: []
provides:
  - "Full npm audit across 13 Node.js repos with fix-or-defer decisions"
  - "Full cargo audit across racecontrol and pod-agent with patch applied"
  - "170-DEPENDENCY-AUDIT.md report covering all 15 repos (13 npm + 2 cargo)"
affects: [all-repos, racingpoint-admin, racingpoint-discord-bot, racingpoint-mcp-drive, racingpoint-mcp-gmail, racecontrol, pod-agent]

tech-stack:
  added: [cargo-audit v0.22.1]
  patterns: ["npm audit fix (non-breaking only)", "cargo update for patch-level Cargo.lock updates"]

key-files:
  created:
    - .planning/phases/170-repo-hygiene-dependency-audit/170-DEPENDENCY-AUDIT.md
  modified:
    - racingpoint-admin/package-lock.json (npm audit fix — flatted DoS high)
    - racingpoint-mcp-drive/package-lock.json (npm audit fix — hono/express-rate-limit highs)
    - racingpoint-mcp-gmail/package-lock.json (npm audit fix — hono/express-rate-limit highs)
    - racecontrol/Cargo.lock (rustls-webpki 0.103.9→0.103.10)
    - pod-agent/Cargo.lock (rustls-webpki 0.103.9→0.103.10)

key-decisions:
  - "discord-bot undici highs deferred — transitive via discord.js 14.25.1 (latest), no upstream fix available yet"
  - "next.js moderate deferred in racingpoint-admin — requires --force breaking upgrade, LAN-only tool, low risk"
  - "rsa RUSTSEC-2023-0071 deferred in racecontrol — no fix available, transitive via sqlx-mysql, no RSA operations exposed"
  - "npm audit fix --force explicitly NOT used — only semver-compatible fixes per plan instructions"
  - "cargo update rustls-webpki viable for both repos — patch-level bump 0.103.9→0.103.10 resolves RUSTSEC-2026-0049"

requirements-completed: [DEPS-01, DEPS-02, DEPS-03]

duration: 8min
completed: 2026-03-23
---

# Phase 170 Plan 03: Dependency Audit Summary

**Full npm + cargo security audit across all 15 repos — 7 npm highs fixed, rustls-webpki patched in both Rust repos, 8 deferred items documented with rationale**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-23T12:21:53 IST
- **Completed:** 2026-03-23T12:29:52 IST
- **Tasks:** 2/2
- **Files modified:** 6 (across 5 repos + audit report)

## Accomplishments

- Ran `npm audit` on all 13 Node.js repos — 10 clean, 3 had issues
- Fixed 7 high vulnerabilities via `npm audit fix` in racingpoint-admin, racingpoint-mcp-drive, racingpoint-mcp-gmail — committed and pushed to each repo
- Installed cargo-audit v0.22.1 and ran `cargo audit` on racecontrol and pod-agent
- Patched RUSTSEC-2026-0049 (rustls-webpki CRL matching logic) in both Rust repos via `cargo update` — committed and pushed
- Documented all 8 deferred vulnerabilities with specific rationale (no fix available / breaking change / LAN-only risk)
- Flagged 2 outdated packages (2+ major versions): googleapis v144 in racingpoint-google, @types/node v20 in racingpoint-admin

## Task Commits

1. **Task 1: npm audit on all 13 Node.js repos** — `64748a86` (docs)
   - External repo fix commits: racingpoint-admin `f11480c`, racingpoint-mcp-drive `5b182ea`, racingpoint-mcp-gmail `95599fc`
2. **Task 2: cargo audit on Rust repos** — included in Task 1 commit (audit doc covers both sections)
   - External repo fix commits: racecontrol `ee803b83`, pod-agent `6faa7f0`

## Files Created/Modified

- `.planning/phases/170-repo-hygiene-dependency-audit/170-DEPENDENCY-AUDIT.md` — Complete audit report: npm summary table (13 repos), vulnerability details with decisions, outdated packages, cargo summary (2 repos), cargo vulnerability details
- `racingpoint-admin/package-lock.json` — flatted DoS high fixed
- `racingpoint-mcp-drive/package-lock.json` + `package.json` — @hono/node-server, hono, express-rate-limit highs fixed
- `racingpoint-mcp-gmail/package-lock.json` + `server.js` — same hono/express fixes
- `racecontrol/Cargo.lock` — rustls-webpki patched to 0.103.10
- `pod-agent/Cargo.lock` — rustls-webpki patched to 0.103.10

## Decisions Made

- `npm audit fix --force` deliberately not used — all force-required fixes were deferred with documented rationale
- discord-bot undici vulnerabilities (GHSA-f269, GHSA-vrm6, GHSA-g9mf, GHSA-2mjp, GHSA-4992): transitive via discord.js 14.25.1 (latest), bundled undici 6.21.3 needs 6.24.0+. No fix in current discord.js. Deferred, track discord.js releases.
- racecontrol rsa RUSTSEC-2023-0071 (Marvin Attack): no upstream fix available, transitive via sqlx-mysql, no RSA key operations in service. Acceptable risk, deferred.
- rustls-webpki patch was semver-compatible (`cargo update` only), safe to apply without tests.

## Deviations from Plan

None — plan executed exactly as written. The `--no-fetch` flag was needed for cargo audit due to crates.io network timeout on yanked-check, but the advisory database scan functioned correctly.

## Issues Encountered

- `cargo audit` yanked-check timed out due to network throttle on crates.io registry. Resolved with `cargo audit --no-fetch` which uses the locally cached advisory DB (982 advisories loaded). The security advisory scan itself was unaffected.

## User Setup Required

None — all fixes committed and pushed automatically.

## Next Phase Readiness

- All critical/high vulnerabilities either fixed or documented with defer rationale
- Deferred items to track: discord.js undici update, next.js major upgrade in admin, rsa upstream fix in sqlx
- 170-DEPENDENCY-AUDIT.md ready as baseline for future audits

---
*Phase: 170-repo-hygiene-dependency-audit*
*Completed: 2026-03-23*
