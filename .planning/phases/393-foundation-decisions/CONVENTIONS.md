# workspace — CONVENTIONS

**Draft from Phase 393.** Every rule names its enforcer. If a rule cannot be enforced mechanically, it is not in this file.

---

## The 8 Rules

| # | Rule | Enforcer |
|---|---|---|
| 1 | Hooks live under `hooks/{cross-platform,windows-only,linux-only}/` | `sync/install.sh` refuses to install files outside these paths; CI fails on orphans |
| 2 | Every `memory/*.md` file has an entry in `memory/INDEX.md` | CI orphan check (`ls memory/*.md` vs grep of INDEX.md) |
| 3 | No secrets in tracked files (`sk-`, `Bearer `, `password=`, `secret=`, `.env*`) | `sync/pre-commit` hook + CI secret scan |
| 4 | Cross-platform JS (`hooks/cross-platform/*.js`) parses on both OSes | CI `node --check` on Linux runner |
| 5 | Files ≤500 lines (warn) / ≤1000 lines (hard fail) | CI size check |
| 6 | All merges to `main` are squash-merge, linear history | GitHub branch protection rule |
| 7 | `sync/install.sh` runs automatically on every `git pull main` | Post-merge git hook installed by `sync/install-git-hooks.sh` |
| 8 | Both machines pull `main` at every SessionStart | SessionStart Claude Code hook (`hooks/cross-platform/workspace-pull.js`) |

---

## What Is NOT In This File (On Purpose)

Rules from CLAUDE.md that rely on memory or discipline were **not** ported here. They stay in CLAUDE.md because they're cultural/behavioral, not structural. Examples:

- "Always update LOGBOOK after commit" — discipline, not structural
- "Cascade updates recursively" — requires judgment
- "Audit all PCs regardless of venue hours" — operational, not repo-related

These are governed by CGP hooks (cgp-enforce, backlog-enforce) and sit in a different layer.

**The test for inclusion here:** can I write a script that blocks a commit/push/install when the rule is violated? If yes, it belongs. If no, it doesn't.

---

## Adding a New Rule

Before adding a rule to this file:

1. Write the enforcer first (the hook, CI check, or script)
2. Test the enforcer catches the violation
3. Test the enforcer does NOT catch valid cases (no false positives)
4. Then add the row to the table above with the enforcer named

Rules without enforcers are rejected. The guiding principle from Phase 393 CONTEXT applies: **if a rule is not enforced mechanically, we will not follow it, so we will not write it down.**

---

## Removing a Rule

If a rule's enforcer has false positive rate >5% OR has been bypassed (manually overridden) >3 times in a month, remove the rule. False enforcement is worse than no enforcement — it trains us to ignore the signal.

---

*Draft — subject to revision in Phase 395 (Architecture Docs). Will move to `workspace/CONVENTIONS.md` when the repo exists.*
