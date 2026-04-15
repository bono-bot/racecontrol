# Phase 393 — Foundation Decisions (CONTEXT)

**Milestone:** v52.0 Claude Workspace Restructure
**Phase:** 393 / FND-01
**Owners:** James (on-site) + Bono (VPS) — joint
**Sponsor:** Uday Singh (GitHub repo owner)
**Status:** DRAFT — awaiting Uday repo-creation approval + Bono ratification
**Date:** 2026-04-15

---

## Guiding Principle

**If a rule is not enforced mechanically by a hook or CI check, we will not follow it — so we will not write it down.**

This is the observation from our own history: rules we've kept are the ones scripts enforce (cgp-enforce, backlog-enforce, auto-push). Rules we've drifted on are the ones that rely on memory (cascade updates, MEMORY.md line limit, rule-sync). The structure in this phase removes all "remember to" obligations.

Corollary: every convention in `CONVENTIONS.md` must name its enforcer. If we cannot name an enforcer, we delete the rule instead of adding it.

## The Google Drive Analogy (why this phase exists)

Uday's framing, 2026-04-15: "Dual consciousness — whether I'm on Bono or on James, I can access both and understand both. One place for each type of thing, not one building on another."

Translated to structure: one shared repository, typed folders (hooks, memory, agents, commands, skills, scripts, etc.), both AIs are peer clients of a neutral origin, neither owns the canonical copy.

---

## Locked Decisions

### 1. Repository

| Item | Decision |
|---|---|
| Name | `workspace` |
| Host | GitHub, private repo |
| Owner | Uday's GitHub account (`usingh@racingpoint.in`) or a `racingpoint` org if he prefers |
| Collaborators | `james-racingpoint`, `bono-racingpoint` — both with push access |
| Offline fallback | Bare mirror at `bono-vps:/root/workspace-mirror.git`, pushed to by either GitHub post-receive hook or Bono cron |

**Why neutral ownership:** survives personnel/hardware changes; tiebreaker authority sits with the human who has real authority; audit trail reads as collaboration not hierarchy; GitHub web UI gives Uday visibility without SSH.

### 2. Folder Layout

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
└── CONVENTIONS.md          (the 8 enforced rules)
```

### 3. Branch Model — "Only Fresh on Repo"

| Item | Decision |
|---|---|
| Persistent branches | `main` only |
| Working branches | `wip/<machine>-<date>-<slug>` — created locally, pushed for CI |
| Merge policy | Squash-merge wip → main, wip branch deletes immediately after |
| Failed wip | Auto-deletes after 24h if not fixed (GitHub Action cleanup) |
| History on main | Linear, one commit per verified change, no merge commits |
| Install source | `install.sh` tracks `main` HEAD only |

**Why:** `main` is always the latest verified working state. Nothing else persists. Matches the Drive analogy — you see the current file, not every intermediate save.

### 4. Install Model

| Item | Decision |
|---|---|
| Mechanism | **Copy**, not symlink |
| Reason | Windows Git Bash symlinks are fragile; copy is atomic and portable |
| Trigger | Post-merge git hook — runs `sync/install.sh` automatically on every `git pull main` |
| SessionStart | Hook runs `git pull main` + triggers install if anything changed |
| Target | `~/.claude/` on each machine (unchanged — Claude Code hardcoded path) |

### 5. CI Gate (runs on every `wip/*` branch push)

1. `node --check` on every `hooks/**/*.js` (Linux runner — catches Windows-only deps)
2. `bash -n` on every `hooks/**/*.sh`
3. `scripts/cgp-distribution-probe.js` — cross-platform parity probe
4. Secret scan — blocklist: `sk-`, `Bearer `, `password=`, `secret=`, `.env*`
5. File size check — warn ≥500 lines, fail ≥1000 lines
6. Orphan check — every `memory/*.md` has an entry in `memory/INDEX.md`

Green → fast-forward/squash to `main`, wip deletes. Red → wip stays, never installs.

### 6. Secret Boundary

| Item | Decision |
|---|---|
| Location | `~/.claude-secrets/` on each machine (NEW — explicit) |
| Contents | API keys, PSK, comms-link.env, OpenRouter keys, etc. |
| In-repo status | Hard exclusion via `.gitignore` + pre-commit blocklist |
| Sync | Never — each machine manages its own secrets |
| Migration | Phase 401 (MIG-04) moves existing scattered secrets from `~/.claude/` into `~/.claude-secrets/` |

### 7. Session State (Ephemeral)

`~/.claude/projects/*/tool-results/`, checkpoints, session-local caches stay **outside** the repo on each machine. Not synced. Not in repo. These are per-machine working memory, not shared knowledge.

### 8. Subagents + Slash Commands

**Join the workspace.** `~/.claude/agents/` → `workspace/agents/`. `~/.claude/commands/` → `workspace/commands/`. Same reasoning as hooks: one place per type. Migration in Phase 402 (MIG-05) — split out as its own phase during 2026-04-16 restructure because agents/commands are first-class workspace tenants, not a bootstrap sub-task.

---

## Conflict Handling

**Race A (mechanical push collision):** second pusher gets `rejected: fetch first` → hook does `git pull --rebase && git push` → resolved in seconds. Standard git. Non-problem at our write volume.

**Race B (semantic drift):** James commits a hook that works on Windows but breaks on Linux. CI gate catches it on the wip branch BEFORE squash-merge. Broken commit never reaches `main`, never reaches either machine's `~/.claude/`. This is the real protection — the gate is the quarantine.

**Failed wip branches are NOT preserved as history.** They auto-delete after 24h. The repo only holds verified state. If a broken change matters, re-submit it fixed on a new wip branch.

---

## What This Phase Does NOT Decide

Deferred to later phases because each benefits from local context:

- **Phase 394 (Resolve CGP Drift — superset files):** three-way diff of `cgp-enforce.js` and `cgp-session-inject.js` across James, Bono, bootstrap. Winner per file. **✓ Complete 2026-04-15.**
- **Phase 395 (Resolve Remaining Drift + Classify):** 6 deferred drifted files + classify 16 James-only + 4 Bono-only hooks into install.sh manifest buckets.
- **Phase 396 (Architecture Docs):** `ARCHITECTURE.md` + `CONVENTIONS.md` final text — this phase drafts them, 396 formalizes.
- **Phase 397 (Uday Repo Gate + CI + Pre-commit):** human gate + write `.github/workflows/ci.yml` + `sync/pre-commit`.
- **Phase 398 (Init Skeleton):** clone fresh repo, `.gitignore`, first green probe.
- **Phase 399-403 (Migration):** scripts/probes (399), memory+INDEX (400), secrets (401), agents+commands (402), test fixtures (403).
- **Phase 404-407 (Hook Migration):** sync tooling (404), James install (405), Bono install + bare mirror (406), parity verification gate (407).
- **Phase 408-412 (Cleanup):** settings (408), bootstrap (409), protocol pointers (410), decommission (411), milestone close (412).

*Restructured 2026-04-16: see ROADMAP.md v52.0 section for full 20-phase map.*

Phase 393 locks the **frame** — repo, layout, branch model, gate contract, install model. Everything else hangs off this frame.

---

## Blockers Before Phase 394 Can Start

1. **Uday approval** to create `workspace` repo under his GitHub account and add `james-racingpoint` + `bono-racingpoint` as collaborators with push access. One-pager at `TOPOLOGY-FOR-UDAY.md`.
2. **Bono ratification** of peer model + gate contract via comms-link. Draft at `BONO-RATIFICATION.md`. Must arrive before Phase 405 touches his hooks (renumbered from 401 during 2026-04-16 restructure).

Both blockers are parallel — neither depends on the other. Phase 394 (CGP drift resolution) can start locally on James before either arrives, since it's analysis + decision work, not migration.

---

## Rationale Notes

**Why not hand-maintain MEMORY.md as the index?** It's already failing. Current `MEMORY.md` is 212 lines (over the 200-line hard limit), getting truncated on load, and the backlog gate has surfaced items I'm not seeing because the manual index drifted. Phase 393 replaces manual indexing with `memory/INDEX.md` maintained by CI orphan-check — mechanical enforcement, not memory.

**Why copy not symlink?** Windows Git Bash symlink creation requires admin or developer mode, and symlinks break when the target file is moved. Copy is dumb but reliable on both OSes. Re-running `install.sh` on every `git pull` makes the copy atomic.

**Why squash-merge not fast-forward?** Fast-forward preserves wip commit history in `main`. Squash collapses the WIP noise into one verified commit with a clean message. Matches the "only fresh on repo" principle — main is a list of verified states, not a list of how we got there.

**Why a neutral origin instead of Bono VPS as canonical?** Bono VPS is "ours" (one AI's hardware). GitHub under Uday's account is "his" (the human's authority). If James and Bono ever disagree on a commit, Uday holds admin on the origin and can force-resolve. This matches real-world authority.

**Why graphify was considered and parked:** graphify (https://github.com/safishamsi/graphify) is a Claude Code skill that generates queryable knowledge graphs from mixed-media folders. Uday surfaced it mid-discussion as a possible replacement for the manual index. Decision: defer to v53 or later, after the workspace structure exists and we can evaluate graphify against real content. Structure first, query layer second.

---

## References

- Source doc: `~/.claude/projects/C--Users-bono/memory/project_workspace_restructure.md`
- Milestone requirements: `.planning/REQUIREMENTS-v52.md`
- Roadmap: `.planning/ROADMAP.md` lines 1946–2014 (v52.0 section)
- Conversation trail: Uday session 2026-04-15 (dual-consciousness framing, staging-branch discussion, "only fresh build stays")

## Next Actions

1. Uday reviews `TOPOLOGY-FOR-UDAY.md` → creates repo → adds collaborators
2. James sends `BONO-RATIFICATION.md` via comms-link → Bono acknowledges
3. Phase 394 starts locally on James (CGP drift diff) regardless of 1 and 2
4. First commit to `main` happens when 1 and 2 both land — contents are Phase 394's output + this CONTEXT.md

---

*Phase 393 status: DRAFT, local only, nothing pushed. Awaiting Uday + Bono sign-off to move to Phase 394.*
