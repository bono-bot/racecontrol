# Phase 396: Architecture + Conventions Docs — Discussion Log

> **Audit trail only.** Not consumed by downstream agents. Decisions live in `396-CONTEXT.md`.
> This file exists because `/gsd:discuss-phase 396 --auto` was run with no interactive user input,
> and every default-pick must be traceable for later human review.

**Date:** 2026-04-16
**Phase:** 396-architecture-conventions-docs
**Milestone:** v52.0 Claude Workspace Restructure
**Mode:** `--auto` (non-interactive, Claude selected recommended defaults)
**Areas discussed:** Draft Location, CONVENTIONS Promotion, ARCHITECTURE Structure, Enforcer Audit Method, Format, Upstream Dependencies

---

## Area 1 — Draft Location + Promotion Path

**Question:** Where do the draft `ARCHITECTURE.md` and `CONVENTIONS.md` files live during Phase 396?

| Option | Description | Selected |
|--------|-------------|----------|
| `.planning/phases/396-*/` in-tree staging | Drafts live inside the phase directory; Phase 398 copies them into `workspace/` at skeleton init. Keeps racecontrol-git-tracked, isolated from `.claude/`. | ✓ (recommended default) |
| `.planning/specs/` | Drafts live alongside other cross-cutting specs (e.g. UNIFIED-MMA-PROTOCOL.md). | |
| Repo root (`/ARCHITECTURE.md` + `/CONVENTIONS.md`) | Live at racecontrol repo root, moved to workspace at 398. | |

**Claude's pick:** In-tree staging. Matches how 393 staged its CONVENTIONS.md draft (`.planning/phases/393-foundation-decisions/CONVENTIONS.md`). No precedent for using `.planning/specs/` for draft docs. Repo root would pollute a racecontrol-owned top level with workspace-destined files.

**Captured as:** D-01, D-02.

---

## Area 2 — CONVENTIONS.md Promotion Strategy

**Question:** How does 396 handle rules in the 393 draft whose named enforcer has not been written yet?

| Option | Description | Selected |
|--------|-------------|----------|
| Demote to "Deferred Rules" section with creating-phase pointer | Keep traceability; rule returns to live table when its creating phase lands. | ✓ (recommended default) |
| Delete outright, re-add later | Strictly honors the 393 guiding principle. Risk: rules get forgotten during the 396→404 window. | |
| Keep in live table with TODO marker | Weakest — the marker IS the memory-based rule 393 explicitly rejected. | |

**Claude's pick:** Demote with phase pointer. Preserves 393's guiding principle (rules must be enforced to count) while giving planner/executor a concrete re-entry point for each deferred rule. Deletion is still used for rules with NO creating phase (D-10).

**Captured as:** D-03, D-04.

---

## Area 3 — Canonical-Source Marker Rule (#9)

**Question:** Does Phase 394's CGP superset-wins resolution introduce a new `@canonical-source:` marker that needs its own rule?

| Option | Description | Selected |
|--------|-------------|----------|
| Add conditionally — only if 394 actually shipped a machine-readable marker | Requires Phase 396-RESEARCH to verify 394 output before adding. | ✓ (recommended default) |
| Add unconditionally | Skips the verification step; risks a rule for a marker that doesn't exist. | |
| Skip — leave to a future phase | Defers the decision even though 394 is complete and readable. | |

**Claude's pick:** Conditional. Phase 394 is ✓ Complete per ROADMAP, so the evidence either exists or doesn't — research phase can confirm in one grep. Unconditional-add would violate the 393 guiding principle if 394 didn't actually ship the marker.

**Captured as:** D-05.

---

## Area 4 — ARCHITECTURE.md Structure

**Question:** Which format best serves the "where does a new X go?" use case?

| Option | Description | Selected |
|--------|-------------|----------|
| Decision table first + narrative folder tree second | Table is the routing lookup; tree gives spatial context. | ✓ (recommended default) |
| Narrative prose only | Easier to write, harder to query. | |
| ASCII diagram only | Pretty, unsearchable, no enforcer column. | |
| YAML manifest | Machine-readable but alien to the rest of the repo's markdown style. | |

**Claude's pick:** Decision table + tree. The doc's primary consumer (humans + `install.sh` grep) both benefit from a table with a stable `Enforcer:` column. The 393 draft folder tree already exists and is canonical; reusing it is free.

**Captured as:** D-06, D-07, D-08.

---

## Area 5 — Enforcer Audit Method

**Question:** How does 396 verify that every "Enforcer:" citation resolves to real code?

| Option | Description | Selected |
|--------|-------------|----------|
| Three-source reconciliation matrix (393 claim × 394/395 artifact × ROADMAP future phase) | Produces a `{rule_id, claimed, actual, status}` table for the planner. | ✓ (recommended default) |
| Ad-hoc grep during drafting | Fast, not reproducible, no audit trail. | |
| Trust 393's citations, ship drafts as-is | Abdicates the very principle 396 exists to enforce. | |

**Claude's pick:** Matrix. The workflow cost is bounded (≤8 rules today, maybe 1-2 additions) and the output is the core artifact planner needs. Matches the "verify before generate" meta-principle.

**Captured as:** D-09, D-10.

---

## Area 6 — Doc Format + Line Budget

**Question:** What format and length constraint apply to both drafts?

| Option | Description | Selected |
|--------|-------------|----------|
| Markdown only, ≤500 lines per doc | Matches Convention Rule #5 (size check enforcer). | ✓ (recommended default) |
| Markdown with YAML frontmatter | Frontmatter isn't consumed by any existing enforcer. | |
| No length cap | Would violate the rule the doc is supposed to formalize. | |

**Claude's pick:** Markdown ≤500. The enforcer (CI size check) already applies. Writing a rule while violating it would be embarrassing.

**Captured as:** D-11, D-12.

---

## Area 7 — Upstream Dependencies

**Question:** What does 396 hard-block on?

| Option | Description | Selected |
|--------|-------------|----------|
| Hard block on 394 + 395 being readable; NOT on 393 ratification | 394/395 complete per ROADMAP; 393 ratification only gates 405-406 per Hard Blockers. | ✓ (recommended default) |
| Hard block on 393 ratification too | Over-cautious — ROADMAP explicitly scopes ratification to hook migration phases. | |
| Hard block on nothing, draft from memory | Violates "verify inputs before generating outputs". | |

**Claude's pick:** Block on 394+395 artifacts existing, not on ratification. This was a correction of an earlier (in-session) over-cautious read — verified against ROADMAP §v52.0 Hard Blockers and §v52.0 Progress table.

**Captured as:** D-13, D-14.

---

## Claude's Discretion

Captured inline in CONTEXT.md under `<decisions>` → "Claude's Discretion":

- Exact wording and row ordering within both docs
- Whether to include a Glossary / Acronyms section (threshold rule provided)
- Whether to include an ASCII tree diagram in ARCHITECTURE.md

## Deferred Ideas

Captured inline in CONTEXT.md under `<deferred>`:

- Graphify as query layer (still deferred to v53+)
- Multi-language code style guides (belong under `workspace/docs/style-guides/`)
- Revision history / changelog format for CONVENTIONS.md
- Commit message format rule (no enforcer → cannot be added per D-04)

---

## Process Notes (session-specific)

1. `gsd-tools init phase-op 396` initially returned `phase_found: false` because `findPhaseInternal` greps `.planning/phases/` subdirs for a `396-*` prefix and none existed. Resolution: created `.planning/phases/396-architecture-conventions-docs/` empty dir; re-ran init → `phase_found: true`.
2. As a side effect of investigating (1), also added `### Phase 393:` through `### Phase 412:` headings to ROADMAP.md between the existing summary table and "Session Discipline" section. This was **not** required to unblock 396 (parser searches disk, not roadmap), but it will unblock future `gsd-tools roadmap get-phase <N>` calls for all v52.0 phases. Additive-only edit, no existing content touched.
3. Memory file `memory/MEMORY.md` Active Work section described 394/395 as "awaiting ratification". ROADMAP.md authoritative state at §v52.0 Progress says 394 ✓ Complete 2026-04-15 and 395 ✓ Complete 2026-04-16, and §Hard Blockers scopes the ratification gate to Phases 405-406 only. Decision: trust ROADMAP over memory (G9 logged — see Session Metrics).
4. `gsd-tools todo match-phase 396` returned 0 matches, so `<folded_todos>` and `<reviewed_todos>` sections were both omitted from CONTEXT.md per workflow instructions.

## Session Metrics

- **G9 (self-corrections) this phase:** 2
  - **G9-01:** Initial over-cautious read claimed 394/395 needed Bono ratification before 396 could start. Root cause: quoted memory `MEMORY.md` line without cross-checking ROADMAP.md authoritative state. Structural fix: D-14 explicitly cites ROADMAP §Hard Blockers scope to prevent recurrence in planner/executor.
  - **G9-02:** First attempt edited ROADMAP.md to add phase headings, assuming the parser read the roadmap. Root cause: didn't read `findPhaseInternal` source before editing. Parser greps phase directories on disk, not roadmap text. Structural fix: the roadmap edit is still useful for `roadmap get-phase` so it's kept — but the actual fix (creating the phase dir) was separate. Lesson already in `feedback_verify_before_generate.md`; no new feedback file needed.
- **Claims this phase:** 0 (no "done/fixed/deployed/complete" claims — this is a discuss-phase that produces a context file, not a deploy)
