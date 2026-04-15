# workspace — ARCHITECTURE

**Guiding principle:** If a rule is not enforced mechanically by a hook or CI check, we will not follow it — so we will not write it down. *(Phase 393)*

This document is a routing table. It answers one question: **"where does a new X go, and what enforces that?"** If you are about to add a file to the `workspace` repo and the answer isn't already here, either add the row first (with a named enforcer) or don't add the file.

---

## The Google Drive Analogy

> "Dual consciousness — whether I'm on Bono or on James, I can access both and understand both. One place for each type of thing, not one building on another."
> — Uday Singh, 2026-04-15 *(captured in Phase 393)*

Translated to structure: one shared repository, typed folders, both AIs are peer clients of a neutral origin, neither owns the canonical copy. The repo is the Drive; each folder is a typed mount point; `main` is the current version; `install.sh` is the sync client.

**Why neutral origin (GitHub under Uday) and not Bono VPS?** Bono VPS is one AI's hardware. Uday's GitHub account is the human's authority. If James and Bono disagree on a commit, Uday holds admin on the origin and can force-resolve. This matches real-world authority. *(Phase 393 §Rationale Notes)*

---

## Decision Table — Where Does a New X Go?

Every row below is currently deferred because the `workspace` repo does not yet exist (Phase 398 creates it) and no enforcer has been written (Phases 397 + 404 write them). When an enforcer file exists on disk in `main`, promote the row by replacing `[DEFERRED — Phase N]` with the real file path.

| Artifact type | Destination folder | Naming rule | Enforcer | Status |
|---|---|---|---|---|
| Cross-platform hook (JS/sh) | `hooks/cross-platform/` | kebab-case `.js` or `.sh` | `sync/install.sh` refuses out-of-folder; CI `node --check` | `[DEFERRED — Phase 397 + Phase 404]` |
| Windows-only hook | `hooks/windows-only/` | `.ps1` / `.bat` / `.js` | `install.sh` routes by classification manifest | `[DEFERRED — Phase 404]` |
| Linux-only hook | `hooks/linux-only/` | `.sh` / `.js` | `install.sh` routes by classification manifest | `[DEFERRED — Phase 404]` |
| Memory doc | `memory/*.md` | lowercase snake, topic-first | CI orphan check vs `memory/INDEX.md` | `[DEFERRED — Phase 397 + Phase 400]` |
| Subagent | `agents/*.md` | role-first name | `install.sh` manifest | `[DEFERRED — Phase 402 + Phase 404]` |
| Slash command | `commands/*.md` | verb-first | `install.sh` manifest | `[DEFERRED — Phase 402 + Phase 404]` |
| Skill | `skills/<name>/SKILL.md` + `rules/*.md` | one skill dir per topic | (no structural enforcer named in Phase 393 — candidate for Phase 404) | `[DEFERRED — Phase 404]` |
| Settings | `settings/base.json` + `README.md` | `base.json` shared, `.local.json` per-machine (never tracked) | `install-settings.sh` merge logic | `[DEFERRED — Phase 408]` |
| Script/probe | `scripts/*.js` | kebab-case | CI `node --check`; reader-grep before rename | `[DEFERRED — Phase 399]` |
| Bootstrap | `bootstrap/{vps,windows}/` | OS sub-dir | None (docs-only folder) | `[DEFERRED — Phase 409]` |
| Test fixture | `tests/<hook-name>/*.{json,txt}` | fixture-per-hook | consumers in `scripts/cgp-distribution-probe.js` | `[DEFERRED — Phase 403]` |

*(11 rows. Every row prefixed `[DEFERRED — Phase N]` per D-07. No row carries a live enforcer file path because no enforcer file exists yet.)*

---

## Folder Tree — Narrative

Structure locked in Phase 393 §Folder Layout:

```
workspace/
├── hooks/
│   ├── cross-platform/     (Node.js, POSIX sh — runs on both)
│   ├── windows-only/       (James: .ps1, .bat, Windows Git Bash)
│   └── linux-only/         (Bono: bash specifics, systemd)
├── memory/                 (shared memory files + INDEX.md)
├── agents/                 (subagents, was ~/.claude/agents/)
├── commands/               (slash commands, was ~/.claude/commands/)
├── skills/                 (was ~/.claude/skills/)
├── settings/
│   ├── base.json           (shared)
│   └── README.md           (documents settings.local.json overrides)
├── scripts/                (cgp-distribution-probe.js, key recovery, etc.)
├── bootstrap/              (ex claude-code-bootstrap/{vps,windows}/)
├── tests/                  (per-hook fixtures, parity test cases)
├── sync/
│   ├── install.sh          (copies workspace files → ~/.claude/)
│   ├── verify-parity.sh    (runs cgp-distribution-probe.js)
│   └── pre-commit          (secret scan + syntax check)
├── docs/                   (reference docs, runbooks)
├── ARCHITECTURE.md         ("where does a new X go?")
└── CONVENTIONS.md          (the enforced rules)
```

**What belongs / what doesn't, per folder:**

- `hooks/cross-platform/` — BELONGS: `.js`/`.sh` hooks that run on both Windows and Linux. DOES NOT BELONG: anything that imports a Windows-only or Linux-only dependency.
- `hooks/windows-only/` — BELONGS: `.ps1`, `.bat`, or `.js` hooks that only make sense on James. DOES NOT BELONG: cross-platform code (goes up one level).
- `hooks/linux-only/` — BELONGS: bash/systemd scripts specific to Bono VPS. DOES NOT BELONG: anything Windows can run.
- `memory/` — BELONGS: shared `.md` memory files both AIs read + `INDEX.md`. DOES NOT BELONG: session transcripts, temporary scratch, per-machine history.
- `agents/` — BELONGS: Claude Code subagent definitions. DOES NOT BELONG: running state, per-session context.
- `commands/` — BELONGS: slash-command definitions. DOES NOT BELONG: command output or logs.
- `skills/` — BELONGS: skill directories with `SKILL.md` + `rules/*.md`. DOES NOT BELONG: downloaded third-party skills (vendor in place or submodule).
- `settings/` — BELONGS: `base.json` (shared) + `README.md`. DOES NOT BELONG: `settings.local.json` (per-machine, `.gitignore`d).
- `scripts/` — BELONGS: shared probes, key recovery, utilities. DOES NOT BELONG: one-off experiments (those live outside the repo).
- `bootstrap/{vps,windows}/` — BELONGS: fresh-machine setup scripts + docs. DOES NOT BELONG: day-to-day operational scripts (use `scripts/`).
- `tests/` — BELONGS: per-hook fixtures and parity test cases. DOES NOT BELONG: runtime results.
- `sync/` — BELONGS: `install.sh`, `verify-parity.sh`, `pre-commit`. DOES NOT BELONG: anything else — this folder is the repo's self-sync contract.
- `docs/` — BELONGS: reference docs and runbooks. DOES NOT BELONG: anything `ARCHITECTURE.md` or `CONVENTIONS.md` should answer.

---

## What Is NOT In The Repo (Hard Boundaries)

- **Secrets:** live in `~/.claude-secrets/` on each machine. Excluded via `.gitignore` + pre-commit blocklist. Migration from scattered locations happens in Phase 401. *(Phase 393 Decision 6)*
- **Session state:** `~/.claude/projects/*/tool-results/`, checkpoints, per-machine caches. Not synced, not in repo. Per-machine working memory is not shared knowledge. *(Phase 393 Decision 7)*

---

## Adding a New Artifact Type

Before adding a new row to the decision table:

1. Write the enforcer first (hook, CI check, install.sh route, or manifest entry).
2. Test the enforcer on both OSes where applicable.
3. Add the decision table row with the enforcer file path named — or, if the enforcer is scheduled for a future phase, add the row prefixed `[DEFERRED — Phase N]` with the creating phase named.
4. Update the folder tree above if a new folder is introduced.

No drive-by additions. If you can't name an enforcer and can't name a creating phase, the artifact type doesn't belong in the repo.

---

*Drafted: Phase 396 / 2026-04-16. Canonical home: workspace/ARCHITECTURE.md once Phase 398 initializes the repo.*
