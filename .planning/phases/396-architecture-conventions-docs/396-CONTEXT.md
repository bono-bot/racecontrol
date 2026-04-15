# Phase 396: Architecture + Conventions Docs — Context

**Gathered:** 2026-04-16
**Mode:** `/gsd:discuss-phase 396 --auto` (non-interactive, Claude-selected defaults)
**Status:** Ready for planning
**Milestone:** v52.0 Claude Workspace Restructure
**Requirement:** FND-03

<domain>
## Phase Boundary

Formalize two docs that frame the `workspace` repo before any migration phase (399+) touches files:

1. **`CONVENTIONS.md`** — promote the Phase 393 draft (`.planning/phases/393-foundation-decisions/CONVENTIONS.md`) to final. Every rule MUST name its mechanical enforcer. Rules without enforcers are deleted, not ported.
2. **`ARCHITECTURE.md`** — NEW doc. Answers "where does a new X go?" with a decision table mapping artifact type → folder → enforcer. Does not yet exist in 393 drafts.

**In scope:** draft text of both files, enforcer audit of every rule, incorporation of learnings from Phase 394 (CGP superset-wins) and Phase 395 (hook drift classification).

**Out of scope:** creating the `workspace` repo (Phase 397/398), writing the CI workflows themselves (Phase 397), running `install.sh` (Phase 404), migrating any files (Phase 399+).

**Deliverable location:** drafts live in `.planning/phases/396-architecture-conventions-docs/` until Phase 398 initializes the `workspace` skeleton, at which point Phase 398 copies them to `workspace/ARCHITECTURE.md` and `workspace/CONVENTIONS.md`. 396 does not touch any file outside its own phase directory.

</domain>

<decisions>
## Implementation Decisions

### Draft Location + Promotion Path

- **D-01:** Both docs are drafted inside `.planning/phases/396-architecture-conventions-docs/` — filenames `ARCHITECTURE.md` and `CONVENTIONS.md` (no numeric prefix; they are repo-relative names, not GSD plan artifacts). *Why:* the `workspace` repo does not exist until Phase 398; staging under the phase dir keeps them in git (racecontrol) without polluting `.claude/` or pretending the destination exists. *Auto-selected:* first option (phase-dir staging) over alternatives (stage at repo root / stage in `.planning/specs/`).
- **D-02:** Phase 398 is responsible for copying both drafts into `workspace/` at skeleton init. 396 does NOT write to `workspace/` — the repo doesn't exist yet. Plan handoff note in `396-SUMMARY.md` must call this out explicitly.

### CONVENTIONS.md — Promotion Rules

- **D-03:** Start from the 393 draft's 8 rules table verbatim. Each of the 8 rules is re-audited against "does its named enforcer exist as real code in 394/395 output, or is it still vapor?" Rules backed by real code stay; rules backed by unwritten enforcers are **demoted** (not deleted yet) to a new `## Deferred Rules` section with the enforcer-writing phase explicitly named (Phase 397 for CI gate, Phase 404 for install.sh, etc.). *Auto-selected:* demote-with-phase-pointer over hard-delete — keeps traceability without weakening the guiding principle.
- **D-04:** **NO new rules added in 396 unless the enforcer already exists on disk.** If Phase 394/395 produced something that needs a rule but the enforcer is a Phase 404+ artifact, the rule is noted in `## Deferred Rules` with the creating phase named, not added to the live table. *Why:* keeps 396 honest to the 393 guiding principle ("if it's not enforced mechanically, we don't write it down").
- **D-05:** Canonical-source marker convention (from 394 superset-wins resolution) — IF Phase 394's output includes a machine-readable marker (e.g., `// @canonical-source:` header) in each resolved file, THAT becomes Rule #9 with enforcer = "grep check in install.sh + CI lint". Research phase must confirm whether 394 actually shipped this marker before adding the row. *Auto-selected:* conditional-on-evidence over unconditional-add.

### ARCHITECTURE.md — Structure

- **D-06:** Structure = **decision table first, narrative tree second**. Table columns: `Artifact type | Destination folder | Naming rule | Enforcer | Example`. Narrative section below the table repeats the folder tree from 393-CONTEXT §2 verbatim, annotated with one-sentence "what belongs here / what doesn't" per folder. *Why:* the doc is a routing table, not a philosophy essay. *Auto-selected:* table+tree over pure-narrative, pure-diagram, or YAML.
- **D-07:** Every row in the decision table MUST cite its enforcer by file path — e.g., "Enforcer: `sync/install.sh` lines 40-60" — or the row doesn't ship. Rows whose enforcer is a Phase 404+ artifact are prefixed `[DEFERRED — Phase N]` and live in the same table section for visibility. Same rule as CONVENTIONS D-03.
- **D-08:** Include a `## Adding a New Artifact Type` section (parallel to 393 draft CONVENTIONS §"Adding a New Rule") — numbered procedure: (1) write the enforcer first, (2) test it, (3) add the table row. No "drive-by additions" to ARCHITECTURE.md without enforcer code. *Auto-selected:* include section (matches 393 convention model).

### Enforcer Audit — How

- **D-09:** Research phase (Phase 396-RESEARCH) runs a three-source enforcer reconciliation: (a) grep Phase 393 draft for every `Enforcer:` named, (b) grep actual files in `.planning/phases/394-*` and `.planning/phases/395-*/` for matching code, (c) produce a reconciliation matrix `{rule_id, claimed_enforcer, actual_code_path, status: live|deferred|vapor}`. Planner consumes the matrix; vapor rows get deleted, deferred rows get their destination phase named, live rows get their code path cited in the final doc. *Auto-selected:* matrix-driven audit over ad-hoc audit.
- **D-10:** If the reconciliation matrix finds a rule whose named enforcer does NOT match any Phase 394/395 artifact AND has no future phase to create it, the rule is **deleted outright** — not deferred. The 393 guiding principle takes priority over sunk-cost preservation. Log deletions in `396-DISCUSSION-LOG.md` with rule text + reason.

### Format Conventions for Both Docs

- **D-11:** Markdown only. No YAML, no JSON. Tables for decision matrices, prose for explanations, ≤500 lines per doc (per Convention Rule #5). *Auto-selected:* markdown-only over mixed-format.
- **D-12:** Both docs end with a `*Drafted: Phase 396 / 2026-04-16. Canonical home: workspace/<filename>.md once Phase 398 initializes the repo.*` footer. Prevents confusion if someone finds the staged draft in `.planning/phases/` after the workspace repo exists.

### Upstream Dependencies — Hard

- **D-13:** Phase 396 **blocks on Phase 394 + 395 being readable** — both are ✓ Complete per ROADMAP.md (394: 2026-04-15 / 395: 2026-04-16 per progress table at ROADMAP §v52.0 Progress). Research phase must verify their output files exist before starting the reconciliation matrix. If either phase's output is missing or empty, 396-RESEARCH halts with a failure report rather than drafting on assumed content.
- **D-14:** Phase 396 **does NOT block on Phase 393 ratification**. Per ROADMAP §Hard Blockers, 393 ratification only gates Phases 405-406 (hook migration). 393 drafts are readable; ratification is a separate Uday/Bono sign-off workflow that does not affect documentation-only phases.

### Claude's Discretion

- Exact wording and row ordering within both docs — left to the planner/executor, constrained by the structure decisions above.
- Whether to include a "Glossary" or "Acronyms" section — planner decides based on how many jargon terms survive the reconciliation matrix (thresholds: >6 distinct acronyms → include glossary; ≤6 → inline definitions on first use).
- Whether `ARCHITECTURE.md` gets a visual ASCII tree diagram in addition to the narrative — executor decides based on whether the tree from 393-CONTEXT §2 still fits after the enforcer annotations.

### Folded Todos

None — `gsd-tools todo match-phase 396` returned 0 matches.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (gsd-phase-researcher, gsd-planner) MUST read these before acting.**

### Phase 393 Foundation (the source of truth 396 is formalizing)

- `.planning/phases/393-foundation-decisions/393-CONTEXT.md` — 8 locked decisions, guiding principle, folder layout tree, conflict handling. THE upstream input for 396.
- `.planning/phases/393-foundation-decisions/CONVENTIONS.md` — 8-rule draft table with named enforcers. Direct input for CONVENTIONS.md promotion.
- `.planning/phases/393-foundation-decisions/TOPOLOGY-FOR-UDAY.md` — external-audience framing; 396 must not contradict it.
- `.planning/phases/393-foundation-decisions/BONO-RATIFICATION.md` — ratification draft; 396 does not block on this but should not pre-empt its language.

### Phase 394 — CGP Drift Resolution (one of two input dependencies)

- `.planning/phases/394-resolve-cgp-drift/394-CONTEXT.md`
- `.planning/phases/394-resolve-cgp-drift/394-01-PLAN.md`
- `.planning/phases/394-resolve-cgp-drift/394-01-SUMMARY.md`
- `.planning/phases/394-resolve-cgp-drift/394-VERIFICATION.md`

### Phase 395 — Hook Drift Classification (the second input dependency)

- `.planning/phases/395-resolve-remaining-hook-drift/395-CONTEXT.md`
- `.planning/phases/395-resolve-remaining-hook-drift/395-01-PLAN.md`
- `.planning/phases/395-resolve-remaining-hook-drift/395-01-SUMMARY.md`
- `.planning/phases/395-resolve-remaining-hook-drift/395-VERIFICATION.md`
- `.planning/phases/395-resolve-remaining-hook-drift/395-DISCUSSION-LOG.md` — hook classification manifest reference

### Milestone-Level Anchors

- `.planning/ROADMAP.md` §v52.0 Claude Workspace Restructure (lines 1946–2094) — 20-phase map and "Hard Blockers" section
- `.planning/REQUIREMENTS-v52.md` — FND-03 requirement text (entry conditions + exit criteria for 396)
- `~/.claude/projects/C--Users-bono/memory/project_v52_restructure_20260416.md` — reason log for the 393→412 restructure; explains why FND-03 was renumbered from the original 16-phase plan
- `~/.claude/projects/C--Users-bono/memory/decision_cgp_drift_resolution.md` — James's canonical picks for 394, referenced by D-05 above
- `~/.claude/projects/C--Users-bono/memory/decision_hook_drift_classification.md` — James's classification manifest for 395, referenced by D-09

### External Memory Index

- `~/.claude/projects/C--Users-bono/memory/MEMORY.md` — index; do not derive architecture from this file, only use it to navigate memory

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets (from 393 drafts)

- **8-rule table template** (CONVENTIONS.md draft lines 9-18) — row format `| # | Rule | Enforcer |` — keep verbatim, extend with status column during reconciliation.
- **Folder tree** (393-CONTEXT §2, lines 44-67) — already canonical; ARCHITECTURE.md narrative section reuses it 1:1.
- **"Adding a New Rule" procedure** (CONVENTIONS.md draft lines 36-45) — template for `ARCHITECTURE.md`'s parallel "Adding a New Artifact Type" section.

### Established Patterns

- **Every rule names its enforcer by file path** — pattern already live in the 393 CONVENTIONS.md draft. 396 extends this to ARCHITECTURE.md rows.
- **Deferred sections explicitly named the future phase** — pattern from 393-CONTEXT §"What This Phase Does NOT Decide" (lines 133-144). 396 reuses this for Deferred Rules.

### Integration Points

- **Phase 398 (Init Skeleton)** — consumes 396 output. Must know the drafts are at `.planning/phases/396-architecture-conventions-docs/{ARCHITECTURE,CONVENTIONS}.md` so it can `cp` them into the fresh `workspace/` clone.
- **Phase 404 (Sync Tooling)** — `install.sh` and `verify-parity.sh` will grep for `Enforcer:` file-path citations in CONVENTIONS.md to verify the named files exist. 396 is what gives 404 something to grep. Keep citations machine-readable (consistent prefix, no "see also").

### What Does NOT Exist Yet (so 396 can't cite it)

- No `workspace` repo, no `workspace/ARCHITECTURE.md`, no `workspace/CONVENTIONS.md`, no `.github/workflows/ci.yml`, no `sync/install.sh`, no `sync/pre-commit`, no `memory/INDEX.md`. All of these are the *enforcers* 396 NAMES — but the naming is a forward reference. The "Deferred Rules" pattern (D-03) exists precisely because of this asymmetry.

</code_context>

<specifics>
## Specific Ideas from 393 That Must Survive 396

- **"If a rule is not enforced mechanically, we will not write it down."** — guiding principle from 393-CONTEXT §"Guiding Principle". Both 396 docs must restate this verbatim at the top.
- **"Dual consciousness" / Google Drive analogy** — one place per artifact type, neither AI owns canonical. ARCHITECTURE.md opens with this framing (attribution: Uday, 2026-04-15).
- **Neutral origin rationale** — 393 §"Rationale Notes" para "Why a neutral origin instead of Bono VPS as canonical". ARCHITECTURE.md "Why this structure?" section quotes it.
- **Squash-merge linear history rationale** — 393 §"Rationale Notes" para "Why squash-merge not fast-forward". CONVENTIONS.md Rule #6 references it.
- **False-positive removal threshold (>5% false positives OR >3 manual bypasses/month)** — 393 CONVENTIONS draft §"Removing a Rule". Promote verbatim — this is itself a meta-rule about the convention set and needs no additional enforcer beyond log review.

</specifics>

<deferred>
## Deferred Ideas

- **Graphify as query layer for CONVENTIONS + ARCHITECTURE** — noted in 393-CONTEXT line 171. Still deferred to v53 or later. 396 drafts plain markdown only.
- **Multi-language code style guides (.rs, .ts, .sh, .ps1)** — NOT scoped to 396. If they exist, they belong under `workspace/docs/style-guides/` and are enforced by linters, not by ARCHITECTURE.md. Mention as a deferred Phase (v53 candidate) in ARCHITECTURE.md Deferred Artifact Types section only if it doesn't bloat the doc past 500 lines.
- **Decision log for future CONVENTIONS changes** — 393 draft does not include a changelog section. Whether to add one to 396's final CONVENTIONS.md is deferred to the planner — either a standalone `workspace/CONVENTIONS-CHANGELOG.md` (new file) or inline `## Revision History` at the bottom. Planner picks; no auto-default needed.
- **Rule on commit message format** — tempting to add "all commits follow conventional-commits spec" but no enforcer exists (no pre-commit or CI check for message format in 394/395 artifacts). Per D-04 this CANNOT be added in 396. Note as a Phase 397 candidate if Uday wants conventional-commits enforced.

### Reviewed Todos (not folded)

None — 0 todos matched phase 396 via `gsd-tools todo match-phase 396`.

</deferred>

<auto_mode_audit>
## --auto Mode Audit Trail

This phase was discussed in `--auto` mode with no interactive user input. Every gray area was auto-selected using the recommended default per the workflow rules in `discuss-phase.md §discuss_areas`.

Decisions the user (James/Uday) should review if any of the following default-picks are wrong:

| ID | Decision | Default picked | Alternative rejected |
|----|----------|----------------|----------------------|
| D-01 | Where drafts live | `.planning/phases/396-*/` (in-tree staging) | repo root / `.planning/specs/` |
| D-03 | Rules without live enforcers | Demote to "Deferred Rules" section with phase pointer | Delete outright |
| D-05 | Canonical-source marker → Rule #9 | Conditional: only IF 394 shipped a machine-readable marker | Add unconditionally |
| D-06 | ARCHITECTURE.md structure | Decision table first + narrative tree | Pure narrative / pure diagram / YAML |
| D-09 | Enforcer audit method | Three-source reconciliation matrix | Ad-hoc grep |
| D-10 | Rules with no enforcer AND no future phase | Delete outright | Keep as cultural rules |
| D-11 | Doc format | Markdown only | Mixed markdown + YAML frontmatter |

**How to override:** reopen this context via `/gsd:discuss-phase 396` (without `--auto`) and pick different options when prompted, OR edit this CONTEXT.md manually before `/gsd:plan-phase 396` runs.

</auto_mode_audit>

---

*Phase: 396-architecture-conventions-docs*
*Milestone: v52.0 Claude Workspace Restructure*
*Context gathered: 2026-04-16 via `/gsd:discuss-phase 396 --auto`*
