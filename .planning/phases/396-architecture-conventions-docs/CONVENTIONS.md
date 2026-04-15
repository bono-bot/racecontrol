# workspace — CONVENTIONS

**Guiding principle:** If a rule is not enforced mechanically by a hook or CI check, we will not follow it — so we will not write it down. *(Phase 393)*

Corollary: every rule in this file names its enforcer by file path. No enforcer, no rule.

---

## Live Rules

*None yet.* The `workspace` repo does not exist as of Phase 396; every enforcer named in the Phase 393 draft is a forward reference to a phase in the 397–412 range. Rules move from the "Deferred Rules" table below into this section when, and only when, their enforcer file exists on disk in `main`. See "Adding a New Rule" below.

---

## Deferred Rules

These rules will become live when their enforcers are written. Until then they are NOT in force. Each row names the creating phase; see `.planning/ROADMAP.md` §v52.0 for the phase map.

| # | Rule | Will be enforced by | Creating phase |
|---|---|---|---|
| 1 | Hooks live under `hooks/{cross-platform,windows-only,linux-only}/` | `workspace/sync/install.sh` + `workspace/.github/workflows/ci.yml` | Phase 397 + Phase 404 |
| 2 | Every `memory/*.md` has an entry in `memory/INDEX.md` | `workspace/.github/workflows/ci.yml` orphan check | Phase 397 (writes ci) + Phase 400 (creates INDEX.md) |
| 3 | No secrets in tracked files (`sk-`, `Bearer `, `password=`, `secret=`, `.env*`) | `workspace/sync/pre-commit` + CI secret scan | Phase 397 |
| 4 | Cross-platform JS (`hooks/cross-platform/*.js`) parses on both OSes | CI `node --check` on Linux runner | Phase 397 |
| 5 | Files <=500 lines (warn) / <=1000 lines (hard fail) | CI size check | Phase 397 |
| 6 | All merges to `main` are squash-merge, linear history | GitHub branch protection rule *(Phase 393 §Rationale Notes: main is a list of verified states, not history)* | Phase 397 (human gate — Uday configures in GitHub UI) |
| 7 | `sync/install.sh` runs automatically on every `git pull main` | Post-merge git hook installed by `sync/install-git-hooks.sh` | Phase 404 |
| 8 | Both machines pull `main` at every SessionStart | SessionStart hook `hooks/cross-platform/workspace-pull.js` | Phase 404 (author file) + Phase 405 (install on James) + Phase 406 (install on Bono) |

---

## What Is NOT In This File (On Purpose)

Rules from `CLAUDE.md` that rely on memory or discipline were **not** ported here — they stay in `CLAUDE.md` because they're cultural/behavioral, not structural (e.g. "always update LOGBOOK after commit", "cascade updates recursively", "audit all PCs regardless of venue hours"). These are governed by CGP hooks (`cgp-enforce`, `backlog-enforce`) and sit in a different layer.

**Test for inclusion here:** can a script block a commit/push/install when the rule is violated? If yes, it belongs. If no, it doesn't. *(Phase 393)*

---

## Adding a New Rule

Before adding a rule to the Live Rules table:

1. Write the enforcer first (the hook, CI check, or script).
2. Test the enforcer catches the violation.
3. Test the enforcer does NOT catch valid cases (no false positives).
4. Add the row to the Live Rules table above with the enforcer file path named.

Rules without enforcers are rejected. If the enforcer is planned for a future phase, the rule lives in the Deferred Rules table with its creating phase named — it does NOT enter the Live Rules table until the enforcer file exists in `main`.

---

## Removing a Rule

If a rule's enforcer has false positive rate >5% OR has been bypassed (manually overridden) >3 times in a calendar month, remove the rule from the Live Rules table. False enforcement is worse than no enforcement — it trains us to ignore the signal. *(Phase 393)*

---

*Drafted: Phase 396 / 2026-04-16. Canonical home: workspace/CONVENTIONS.md once Phase 398 initializes the repo.*
