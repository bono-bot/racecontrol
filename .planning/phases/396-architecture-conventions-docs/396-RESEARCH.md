# Phase 396 — Research

**Date:** 2026-04-16
**Phase:** 396 / FND-03 / Architecture + Conventions Docs
**Researcher:** gsd-phase-researcher
**Confidence:** HIGH (all claims grounded in on-disk file probes + 394/395 verification docs)

---

## Summary

Phase 396 formalizes two docs (`ARCHITECTURE.md`, `CONVENTIONS.md`) drafted in Phase 393 into final form, under the rule that every convention must name a *real* mechanical enforcer or be deleted. The catch: **none of the 8 enforcers named in the 393 CONVENTIONS draft exist on disk today** — they are all forward references to phases 397/398/400/404. Per CONTEXT D-03, the correct move is to promote all 8 rules into a `## Deferred Rules` section with explicit phase pointers, not ship them as live rules. The live rules table will be empty or near-empty until phases 397-404 land. Phase 394 did NOT ship a `@canonical-source:` marker, so Rule #9 is NOT added (D-05). Phase 395 shipped a JSON manifest but it is an input-for-Phase-404, not a rule generator. ARCHITECTURE.md is greenfield (no 393 draft exists for it) and its table will be entirely `[DEFERRED — Phase N]` rows. Both docs are expected to come in well under the 500-line cap. Planner must write docs that are honest about this asymmetry rather than papering over it.

**Primary recommendation:** Ship CONVENTIONS.md with an empty (or 1-row) live table and a full 8-row Deferred Rules section. Ship ARCHITECTURE.md with a decision table where every row is `[DEFERRED — Phase N]`. Both docs restate the 393 guiding principle verbatim. Do not invent enforcers.

---

## 1. Enforcer Reconciliation Matrix

**Method:** Enumerate the 8 rules in `.planning/phases/393-foundation-decisions/CONVENTIONS.md` lines 9-18. For each named enforcer, probe (a) `~/.claude/hooks/` for live JS, (b) `.git/hooks/` for git hooks, (c) ROADMAP §v52.0 phase table for a creating phase.

**Live probes run 2026-04-16:**
- `ls C:/Users/bono/.claude/hooks/` — 28 files, NO `workspace-pull.js`
- `ls C:/Users/bono/racingpoint/workspace` — **does not exist** (expected; Phase 398 creates it)
- `ls C:/Users/bono/racingpoint/racecontrol/sync` — **does not exist**
- `ls C:/Users/bono/.claude/projects/C--Users-bono/memory/INDEX.md` — **does not exist**
- `racecontrol/.github/workflows/ci.yml` — exists but scopes racecontrol build, NOT workspace repo (different repo entirely)
- `racecontrol/.git/hooks/pre-commit` — exists but is racecontrol's secret-scan, not `workspace/sync/pre-commit`

| # | Rule (short) | Claimed Enforcer | Claimed Path | Status | Creating Phase / Evidence |
|---|---|---|---|---|---|
| 1 | Hooks live under `hooks/{cross,win,linux}/` | `sync/install.sh` refuses out-of-path; CI orphan fail | `workspace/sync/install.sh` + `workspace/.github/workflows/ci.yml` | **deferred** | install.sh = Phase 404 (HOOK-01); CI = Phase 397 (FND-04a). Neither exists. |
| 2 | Every `memory/*.md` has entry in `memory/INDEX.md` | CI orphan check | `workspace/.github/workflows/ci.yml` (check #6) | **deferred** | CI yaml = Phase 397. `memory/INDEX.md` itself created in Phase 400 (MIG-02). |
| 3 | No secrets in tracked files | `sync/pre-commit` + CI secret scan | `workspace/sync/pre-commit` + CI | **deferred** | Both = Phase 397 (FND-04a). Note: racecontrol has its own `.git/hooks/pre-commit` secret-scan but that is a *different* repo's enforcer — does not satisfy this rule. |
| 4 | Cross-platform JS parses on both OSes | CI `node --check` on Linux runner | `workspace/.github/workflows/ci.yml` (check #1) | **deferred** | Phase 397. |
| 5 | Files ≤500 warn / ≤1000 fail | CI size check | `workspace/.github/workflows/ci.yml` (check #5) | **deferred** | Phase 397. |
| 6 | Squash-merge linear history on main | GitHub branch protection | GitHub repo settings (not a file) | **deferred** | Configured by Uday/James during Phase 397 human gate. Not file-based — enforcement is GitHub API config. Rule is *verifiable* by `gh api repos/.../branches/main/protection`, not a file grep. |
| 7 | `install.sh` auto-runs on `git pull main` | Post-merge git hook installed by `sync/install-git-hooks.sh` | `workspace/sync/install-git-hooks.sh` | **deferred** | Phase 404 (HOOK-01). |
| 8 | Both machines pull `main` at SessionStart | `hooks/cross-platform/workspace-pull.js` | `workspace/hooks/cross-platform/workspace-pull.js` | **deferred** | No such file in `~/.claude/hooks/` today (confirmed by grep — 28 files, none named workspace-pull). Must be authored during Phase 404 or 405. |

**Summary:**
- **live:** 0 rules
- **deferred:** 8 rules (all have a named future phase)
- **vapor (delete per D-10):** 0 rules

This is the single most important finding: **the 393 draft is entirely forward-referencing.** Every rule is a promise about code that will exist in a future phase. The planner must not shy from this — per D-03 the correct encoding is one empty live table + one 8-row Deferred Rules section with phase pointers.

---

## 2. New Rule Candidates (394/395 learnings)

**Rule #9 decision (D-05):** DO NOT ADD.

- **Phase 394 output audit:** `394-01-SUMMARY.md` says James files were picked "unmodified." No `@canonical-source:` header was inserted. Grep across `.claude/projects/C--Users-bono/memory/` for `@canonical-source` returns zero hits in any committed file (only in session transcript jsonl). The decision doc records SHA256 + per-hunk rationale in memory, NOT as an in-file marker. There is no machine-readable per-file marker to grep for.
- **Conclusion:** Rule #9 (canonical-source marker) has NO enforcer and NO future phase creating one. Per D-05's conditional-on-evidence clause: **skip**. Document the absence in `396-DISCUSSION-LOG.md` as a one-line "evaluated, not added — 394 chose SHA256-in-memory over in-file marker."

**Phase 395 rule candidates:** 395 shipped `hook-classification.json` (SHA256 `c0e72b29...`). This is a data manifest consumed by install.sh at Phase 404, not a pattern that generates a new convention. The thing that would *become* a rule ("every hook has a classification entry") is enforced when Phase 404's `install.sh` reads the manifest and refuses to copy unclassified files — that enforcer doesn't exist yet, so per D-04 no rule is added in 396. Note in Deferred Rules if the planner wants, with creating phase = 404.

**Net new rules added in 396:** 0.

---

## 3. ARCHITECTURE.md Draft Table Rows

Enumerate from 393-CONTEXT §2 folder layout (lines 44-67). Every enforcer below is deferred — no artifact type currently has a live enforcer, because the `workspace` repo doesn't exist.

| Artifact type | Destination folder | Naming rule | Enforcer | Status |
|---|---|---|---|---|
| Cross-platform hook (JS/sh) | `hooks/cross-platform/` | kebab-case `.js` or `.sh` | `sync/install.sh` refuses out-of-folder; CI `node --check` | `[DEFERRED — Phases 397+404]` |
| Windows-only hook | `hooks/windows-only/` | `.ps1` / `.bat` / `.js` | `install.sh` routes by classification manifest | `[DEFERRED — Phase 404]` |
| Linux-only hook | `hooks/linux-only/` | `.sh` / `.js` | `install.sh` routes by classification manifest | `[DEFERRED — Phase 404]` |
| Memory doc | `memory/*.md` | lowercase snake, topic-first | CI orphan check vs `memory/INDEX.md` | `[DEFERRED — Phases 397+400]` |
| Subagent | `agents/*.md` | role-first name | `install.sh` manifest | `[DEFERRED — Phases 402+404]` |
| Slash command | `commands/*.md` | verb-first | `install.sh` manifest | `[DEFERRED — Phases 402+404]` |
| Skill | `skills/<name>/SKILL.md` + `rules/*.md` | skill dir per topic | (no structural enforcer named in 393) | `[DEFERRED — Phase 404 candidate; may have no enforcer]` |
| Settings | `settings/base.json` + `README.md` | `base.json` shared, `.local.json` per-machine | `install-settings.sh` merge logic | `[DEFERRED — Phase 408 (CLN-01)]` |
| Script/probe | `scripts/*.js` | kebab-case | CI `node --check`; reader-grep before rename | `[DEFERRED — Phase 399 (MIG-01)]` |
| Bootstrap | `bootstrap/{vps,windows}/` | OS sub-dir | None (docs only) | `[DEFERRED — Phase 409 (CLN-02)]` |
| Test fixture | `tests/<hook-name>/*.{json,txt}` | fixture-per-hook | `cgp-distribution-probe.js` consumers | `[DEFERRED — Phase 403 (MIG-03)]` |
| Sync tooling | `sync/{install.sh,verify-parity.sh,pre-commit}` | fixed names | Self (install.sh is its own enforcer) | `[DEFERRED — Phases 397+404]` |
| Docs | `docs/*.md` | topic-first | CI size check (Rule #5) | `[DEFERRED — Phase 397]` |

**11 rows. Every row deferred.** Planner should not try to synthesize "live" rows — there are none until Phase 404.

---

## 4. Deferred Rules List

All 8 rules from the 393 draft demote. Format planner should use:

```markdown
## Deferred Rules

These rules will become live when their enforcers are written. Until then they are NOT in force.

| # | Rule | Will be enforced by | Creating phase |
|---|---|---|---|
| 1 | Hooks live under `hooks/{cross-platform,windows-only,linux-only}/` | `workspace/sync/install.sh` + `workspace/.github/workflows/ci.yml` | 397 + 404 |
| 2 | Every `memory/*.md` has entry in `memory/INDEX.md` | `workspace/.github/workflows/ci.yml` (orphan check #6) | 397 (writes ci) + 400 (creates INDEX.md) |
| 3 | No secrets in tracked files | `workspace/sync/pre-commit` + CI secret scan | 397 |
| 4 | Cross-platform JS parses on both OSes | CI `node --check` | 397 |
| 5 | Files ≤500 lines warn / ≤1000 fail | CI size check | 397 |
| 6 | Squash-merge, linear history on `main` | GitHub branch protection (API config, not a file) | 397 (human gate — Uday sets in GitHub UI) |
| 7 | `sync/install.sh` runs on every `git pull main` | Post-merge git hook via `sync/install-git-hooks.sh` | 404 |
| 8 | Both machines pull `main` at SessionStart | `hooks/cross-platform/workspace-pull.js` | 404 (author file) + 405 (install on James) + 406 (install on Bono) |
```

---

## 5. Delete / Demote / Promote Decisions

**Promote to live table:** **0 rules.** Nothing can be live-cited because nothing exists.

**Demote to Deferred Rules section:** **8 rules** (all of them — see section 4).

**Delete outright (vapor per D-10):** **0 rules.** Every rule has a named creating phase in ROADMAP §v52.0, so deletion is not warranted. None are orphaned.

**Concrete acceptance criteria the planner can use:**
- `grep -c '^| [1-8] |' CONVENTIONS.md` in the **live rules** table section → expected **0** (no live rules yet) OR 1 if the planner chooses to add one live "Every Deferred Rule names its creating phase" meta-rule whose enforcer is `grep -c "Creating phase: 3[0-9][0-9]" CONVENTIONS.md >= 8`. Planner discretion.
- `grep -c '^| [1-8] |' CONVENTIONS.md` in **Deferred Rules** section → expected **8**.
- Every Deferred Rules row cites a phase number between 397 and 412 inclusive.

---

## 6. Doc Length Projection

- **CONVENTIONS.md:** header + guiding principle (~15 lines) + empty live table + 8-row Deferred section (~30 lines) + "Adding a New Rule" procedure (~15 lines) + "Removing a Rule" meta-rule (~10 lines) + footer (~5 lines) = **~75-100 lines.** Well under 500.
- **ARCHITECTURE.md:** header + guiding principle + Google Drive framing (~25 lines) + 11-row decision table (~20 lines) + narrative folder tree from 393 (~25 lines) + "Adding a New Artifact Type" procedure (~15 lines) + "Why this structure?" rationale (~20 lines) + footer (~5 lines) = **~110-140 lines.** Well under 500.

**Neither doc needs splitting.** Convention Rule #5 (the 500-line cap) is safe.

---

## 7. Back-Reference Citations

Every decision/row that comes from an upstream artifact needs a `(Phase N)` cite. Planner should include these:

**CONVENTIONS.md:**
- Guiding principle paragraph → cite `(Phase 393)`
- "Removing a Rule" meta-rule (5% false-positive / 3 bypass threshold) → cite `(Phase 393)`
- Every Deferred Rules row → cite the creating phase in the "Creating phase" column (already structural)
- Rule #6 (squash-merge rationale) → footnote or prose cite `(Phase 393 §Rationale Notes)`

**ARCHITECTURE.md:**
- Google Drive / dual-consciousness framing → attribution `(Uday Singh, 2026-04-15; captured in Phase 393)`
- Folder tree → cite `(Phase 393 §Folder Layout)`
- Neutral-origin rationale → cite `(Phase 393 §Rationale Notes)`
- Every decision table row → cite creating phase in the Enforcer column
- Secrets boundary exclusion (`~/.claude-secrets/`) → cite `(Phase 393 D-6; migration Phase 401)`
- Session state exclusion → cite `(Phase 393 Decision 7)`
- Agents+commands folder rationale → cite `(Phase 393 Decision 8; migration Phase 402)`

**Both docs:**
- Footer with `*Drafted: Phase 396 / 2026-04-16. Canonical home: workspace/<filename>.md once Phase 398 initializes the repo.*` — mandated by D-12.

---

## Validation Architecture

Grep-able checks the planner should encode as acceptance criteria for the 396 executor. All runnable against the final `CONVENTIONS.md` and `ARCHITECTURE.md` in `.planning/phases/396-architecture-conventions-docs/`.

1. **Both files exist:**
   `test -f .planning/phases/396-architecture-conventions-docs/CONVENTIONS.md && test -f .planning/phases/396-architecture-conventions-docs/ARCHITECTURE.md`
2. **500-line cap:**
   `[ $(wc -l < CONVENTIONS.md) -le 500 ] && [ $(wc -l < ARCHITECTURE.md) -le 500 ]`
3. **Guiding principle verbatim in both:**
   `grep -qF "If a rule is not enforced mechanically" CONVENTIONS.md && grep -qF "If a rule is not enforced mechanically" ARCHITECTURE.md`
4. **Deferred Rules section present in CONVENTIONS.md:**
   `grep -q '^## Deferred Rules' CONVENTIONS.md`
5. **All 8 rules demoted to Deferred:**
   `awk '/^## Deferred Rules/,/^## /' CONVENTIONS.md | grep -c '^| [1-8] ' == 8`
6. **Every Deferred row cites a v52 phase number (397-412):**
   `awk '/^## Deferred Rules/,/^## /' CONVENTIONS.md | grep -c -E '(Phase|phase) (39[7-9]|4[0-1][0-9])' >= 8`
7. **ARCHITECTURE.md has the decision table with ≥10 rows:**
   `grep -c '^| \[DEFERRED' ARCHITECTURE.md >= 10`
8. **Every ARCHITECTURE.md table row either cites a file path OR is `[DEFERRED — Phase N]`:**
   `awk '/^\| /' ARCHITECTURE.md | grep -cvE '(DEFERRED|Enforcer|Destination|---)' == 0` (no orphan rows)
9. **No live "Enforcer: `<path>`" that points to a non-existent file** — for every `Enforcer:` citation, `test -f <path>` or row is prefixed `[DEFERRED`. Since we expect 0 live citations, this check reduces to `grep -c '^Enforcer:' CONVENTIONS.md ARCHITECTURE.md == 0` or all such lines are under Deferred Rules.
10. **D-12 footer present in both:**
    `grep -qF 'Canonical home: workspace/' CONVENTIONS.md && grep -qF 'Canonical home: workspace/' ARCHITECTURE.md`
11. **Rule #9 (canonical-source marker) NOT added** — per D-05 conditional:
    `! grep -q 'canonical-source' CONVENTIONS.md` OR the string only appears in a "not added — see 396-DISCUSSION-LOG" note.
12. **393 back-references present:**
    `grep -c '(Phase 393' ARCHITECTURE.md >= 3 && grep -c '(Phase 393' CONVENTIONS.md >= 2`

These are deterministic, run in <1s, and leave zero ambiguity about whether 396 shipped the right thing.

---

## Open Questions (for planner)

1. **Should the planner add ONE live meta-rule?** Candidate: *"Every Deferred Rule in this file must cite a creating phase number between 397-412; violated when an orphan rule is merged."* Enforcer: `grep -c "Creating phase:" CONVENTIONS.md` must equal the number of deferred rule rows. This is the *only* rule whose enforcer exists today (a one-line grep). Adds self-consistency without violating D-04. Planner's call.
2. **Glossary section?** Per Claude's Discretion in CONTEXT.md: threshold is >6 acronyms. Counted acronyms in both drafts: CGP, MMA, CI, PSK, GSD, WoL, SHA256, WS — that's 8. → **Include a Glossary** in ARCHITECTURE.md. (Or inline on first use in CONVENTIONS.md since it's shorter.)
3. **ASCII tree diagram in ARCHITECTURE.md?** The 393 folder tree is 23 lines. Keeping it verbatim fits the 500-line budget easily. → **Include it.**
4. **Changelog/revision history?** 393 didn't have one; planner discretion. Recommendation: **skip** for 396 to avoid pre-committing format before Phases 397+ start editing. Revisit in v53.

---

## Sources

### Primary (HIGH)
- `.planning/phases/393-foundation-decisions/CONVENTIONS.md` lines 9-18 — 8-rule table (source of truth)
- `.planning/phases/393-foundation-decisions/393-CONTEXT.md` lines 44-67, 133-144 — folder layout + deferred phase map
- `.planning/phases/394-resolve-cgp-drift/394-01-SUMMARY.md` — confirms no `@canonical-source:` marker shipped
- `.planning/phases/394-resolve-cgp-drift/394-VERIFICATION.md` — 9/9 truths verified, SHA256s in memory not files
- `.planning/phases/395-resolve-remaining-hook-drift/395-01-SUMMARY.md` — hook-classification.json is a Phase 404 input
- `.planning/phases/395-resolve-remaining-hook-drift/395-VERIFICATION.md` — 10/10 passed
- `.planning/ROADMAP.md` lines 1946-2094 — v52.0 phase table, hard blockers, progress
- Live filesystem probes 2026-04-16:
  - `C:/Users/bono/.claude/hooks/` (28 files, no workspace-pull.js)
  - `C:/Users/bono/racingpoint/workspace` (does not exist)
  - `C:/Users/bono/racingpoint/racecontrol/sync` (does not exist)
  - `C:/Users/bono/.claude/projects/C--Users-bono/memory/INDEX.md` (does not exist)

### Secondary
- 396-CONTEXT.md (14 decisions D-01..D-14)
- 396-DISCUSSION-LOG.md (7 gray areas, default picks)

---

## Metadata

**Confidence breakdown:**
- Enforcer matrix: **HIGH** — every "deferred" classification is grounded in a live `ls` that returned "no such file" + a ROADMAP phase cite.
- Rule #9 skip: **HIGH** — 394-01-SUMMARY.md explicitly says canonical was "unmodified" (no marker inserted).
- ARCHITECTURE table: **MEDIUM** — the 11 artifact types come cleanly from 393, but folder ownership of "skills" is underspecified in 393 and may need planner interpretation.
- Doc length: **HIGH** — back-of-envelope well under 500.

**Research date:** 2026-04-16
**Valid until:** 2026-04-30 (will be invalidated by Phase 397/398/404 landing — re-run reconciliation matrix then).

## RESEARCH COMPLETE
