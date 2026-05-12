# V2 DOC REORG PROPOSAL — V2-MASTER-STATE chunking + memory hierarchy

**Authored:** 2026-05-11 ~11:38 IST · bono · companion to V2-PROGRESS-MAP.md
**Class:** PROPOSAL — Captain disposition required (Q3 canonical-surface; touches V2-MASTER-STATE = PACT-20260503-002 + memory canonical-substrate)
**Status:** DRAFT-PRE-CAPTAIN — bilateral AMPLIFIER queued for james post-Captain-disposition
**Companion:** `V2-PROGRESS-MAP.md` (the deliverable that closes the immediate "can't see progress" gap; this proposal addresses the underlying "scattered info" structural cause)

---

## §1 — Problem statement

Captain 2026-05-11 ~11:30 IST verbatim: *"I am unable to get an understanding of the progress of Racing Point ecosystem v2. Information is scattered."*

Sources of scatter (empirical):

| Source | Size | Symptom |
|---|---|---|
| `comms-link/V2-MASTER-STATE.md` | **21,151 lines** / 200 §S-N entries / single file | Cannot navigate by topic. Captain reading recent state needs to grep + scroll. |
| `~/.claude/projects/-root/memory/` (bono) | **553 files**, flat namespace | Topic discovery only via MEMORY.md index. |
| `~/.claude/projects/-root/memory/MEMORY.md` (bono index) | **1,219 lines** / loaded into every session context (200-line truncation) | Index exceeds context-load budget; recent additions get pushed off. |
| `comms-link/briefings/bono/memory/` (live-sync mirror) | symlink-like to canonical | Direct writes here get overwritten; users confused. |
| Scattered substrate at `racecontrol/.planning/specs/v2/` | 16+ files: AMPLIFIER-PASSES/, AUDITS/, MECHANISM-TRUST/, MI/, V2-CUSTOMER-ENTRY/, PHASE-*.md | Sub-tree structure exists but is irregular; some are dirs, some flat files. |
| `comms-link/.planning/draft-pacts/` | 22 files, flat | Discovery only by listing dir; no class-grouping. |
| `comms-link/v2-skeleton/` | 7 files (01..06 + 10) | Numbering gaps (07/08/09 missing); not load-ordered for discovery. |

**Net effect on Captain:** No single read path for "give me V2 state." Forces grep+scroll across 5+ surfaces. Map produced today (V2-PROGRESS-MAP) closes the symptom; this reorg closes the underlying structural cause.

---

## §2 — Constraints (preserve these)

| Constraint | Source | Implication for reorg |
|---|---|---|
| V2-MASTER-STATE is **append-only canonical ledger** | PACT-20260503-002 | Splitting must preserve git-trace lineage; cannot rewrite history. Cannot insert NEW §S-N entries in the middle. |
| Bilateral mirror sync (bono ↔ james) via comms-link git tree | "Cross-Machine Execution" doctrine | Both pilots must see same file layout post-reorg. |
| `partner-memory-read.js` SessionStart hook scans `briefings/${PARTNER}/memory/MEMORY.md` | hook source line 32 | Renaming MEMORY.md or moving namespace breaks both pilots' session-start. |
| Live-sync mirror: writes to canonical `/root/.claude/projects/-root/memory/` propagate to `comms-link/briefings/bono/memory/` | sync mechanism (live) | Reorg must coordinate canonical+mirror identically. |
| Context-load budget on `MEMORY.md`: harness loads first ~200 lines | observed behavior; current 1219 lines is over-budget | Index must shrink OR re-frame. |
| `[partner-memory-read]` output included in SessionStart context | startup hooks | Reorg cannot inflate that output. |
| **Memory files are referenced by exact filename** in `MEMORY.md` index links | every entry in current MEMORY.md | Renames require coordinated index update. |
| Captain's habitual `/root/.claude/projects/-root/memory/` access (file-path level) | observed prompts | Path stability matters; symlinks acceptable. |

**Universal Sync rule applies:** any reorg change must land bilaterally in same session (bono + james sides).

---

## §3 — Three reorg options ranked

### Option A — Aggressive split (RISK: HIGH)

- Split V2-MASTER-STATE into per-§S-N files: `V2-MASTER-STATE/sessions/S-001.md` ... `V2-MASTER-STATE/sessions/S-200.md` + `INDEX.md`
- Reorganize memory into `memory/v2/{foundation,progress,doctrine}/`, `memory/discipline/`, `memory/archive/`
- Shrink MEMORY.md to <200 lines pointing at sub-indexes
- **Net:** Cleanest end-state; but every cross-link breaks; bilateral sync risk high; rollback expensive

### Option B — Soft split with anchored INDEX (RISK: MEDIUM) **← RECOMMENDED**

- Keep `V2-MASTER-STATE.md` as the single canonical file for **last 50 §S-N entries** (rolling window, currently §S-151..§S-200)
- Move §S-001..§S-150 to `V2-MASTER-STATE-archive/2026-Q1.md` + `2026-Q2.md` (quarter-chunked); preserve full text + git-trace via single-commit `git mv`-then-edit
- Add `V2-MASTER-STATE-INDEX.md` (one line per §S-N: `§S-N | date | author | one-line summary | file` — ~200 lines for all 200 entries)
- Memory: keep canonical directory FLAT; add new TOP-LEVEL `MEMORY-v2/` subdirectory containing only V2-load-first foundation files (10-15 files); MEMORY.md root index points to TOP-LEVEL subdirs (`MEMORY-v2/`, `discipline/`, `operations/`, etc.) — sub-indexes per dir
- Shrink root MEMORY.md to <200 lines (top-level pointers + recent additions)
- **Net:** Most file paths unchanged; archive split is reversible (just `git mv` back); index adds discoverability without breaking links

### Option C — Index-only (RISK: LOW)

- Don't touch V2-MASTER-STATE or memory files at all
- Add `V2-MASTER-STATE-INDEX.md` (one-line-per-§S-N navigator)
- Add `MEMORY-V2-FOUNDATION.md` pointer (lists the 10-15 foundation files that should load FIRST every V2 session)
- Re-author MEMORY.md to shrink to <200 lines (move detail to topic files; index entries one-line under 150 chars per existing rule)
- **Net:** Lowest risk; only addresses discoverability not the underlying file sizes; future "scattered" recurs as count grows

### Risk comparison

| Risk | A (aggressive) | B (soft split) | C (index-only) |
|---|---|---|---|
| Breaks cross-links | HIGH | LOW (only old §S-N entries move) | NONE |
| Breaks bilateral sync | HIGH | MEDIUM (archive needs symmetric mirror) | LOW |
| Breaks partner-memory-read hook | MEDIUM (if MEMORY.md path/name changes) | LOW (sub-indexes via standard md links) | NONE |
| Migration time | 1-2 days | 4-6 hours | 1-2 hours |
| Rollback cost | HIGH | LOW (git mv reversible) | NONE |
| Captain reading experience | BEST | GOOD | MARGINAL |
| Closes "scattered" complaint | YES | YES | PARTIAL (scattered files still scattered, just better indexed) |

---

## §4 — Recommended: Option B (soft split with anchored INDEX)

**Rationale:** Closes Captain's complaint while preserving append-only canonical semantics, bilateral sync, and reversibility. Adds discoverability layer (INDEX) without forcing path changes that break existing references.

### Migration plan (4 phases)

**Phase 1 — Add INDEX + foundation pointer (lowest-risk lift, ~1h)**

1. Author `comms-link/V2-MASTER-STATE-INDEX.md` — one line per §S-1..§S-200 (`§S-N | YYYY-MM-DD | author | one-line summary | file:line`). ~200 lines total.
2. Author `~/.claude/projects/-root/memory/MEMORY-V2-FOUNDATION.md` — list of 10-15 V2-load-first files (the ones with ⭐⭐⭐ markers in current MEMORY.md).
3. Edit MEMORY.md preamble: point at FOUNDATION file as load-first; reduce TLDR section size by ~30%.
4. Bilateral: mirror both index files to james-side namespace.
5. Universal Sync: link from `~/.claude/CLAUDE.md` + `racecontrol/CLAUDE.md` + `comms-link/CLAUDE.md`.

**Phase 2 — Quarterly archive split of V2-MASTER-STATE (medium-risk; ~3h)**

1. Determine split boundary: §S-1..§S-N where date < 2026-04-01 → `archive/2026-Q1.md`; §S-X..§S-Y where 2026-04-01 ≤ date < 2026-07-01 → `archive/2026-Q2.md`. Keep §S-151..§S-200 (rolling 50) in `V2-MASTER-STATE.md`.
2. Single commit `git mv`-then-split — preserves git-blame lineage.
3. Update INDEX.md to point at new file paths.
4. Run `grep -nr '§S-[0-9]' comms-link racecontrol .claude` to find cross-links; update broken ones.
5. Bilateral sync: james pulls; verifies his session-start picks up new structure.

**Phase 3 — Memory subdirectory introduction (medium-risk; ~2h)**

1. Create `memory/V2-foundation/` subdir (canonical bono path); move 10-15 ⭐⭐⭐ files there.
2. Update MEMORY.md to point at sub-dir index (each sub-dir has its own MEMORY.md aka mini-index).
3. Mirror to comms-link/briefings/bono/memory/V2-foundation/.
4. Verify `partner-memory-read.js` reads the moved files; if it only scans MEMORY.md, sub-index discovery works.
5. james-side parallel sub-dir scheme deferred (his choice).

**Phase 4 — Shrink root MEMORY.md to <200 lines (high-leverage; ~1h)**

1. Audit each entry against "one-line under 150 chars" rule (PRE-EXISTING but violated).
2. Move multi-line detail to topic files; entries become single-line pointers.
3. Verify SessionStart context-load shows full root MEMORY.md.
4. Bilateral mirror.

### Out of scope (Phase 5+; not in this proposal)

- Reorganize comms-link/.planning/draft-pacts/ by class (proposed-vs-ratified vs class-based dirs)
- Reorganize v2-skeleton/ to fill 07/08/09 numbering gaps
- Build a generated HTML view of V2-MASTER-STATE (graphify integration candidate)
- Captain-facing dashboard surfacing V2-PROGRESS-MAP rollup card via web UI

---

## §5 — Rollback plan

Each phase has independent rollback:

| Phase | Rollback |
|---|---|
| 1 | `git rm V2-MASTER-STATE-INDEX.md MEMORY-V2-FOUNDATION.md` + revert MEMORY.md preamble |
| 2 | `git mv` archive files back to `V2-MASTER-STATE.md`; concat-revert |
| 3 | `git mv` memory/V2-foundation/* back to flat namespace |
| 4 | revert MEMORY.md to pre-shrink version |

Per-phase commit boundary makes each rollback atomic.

---

## §6 — Bilateral notification

Phase 1 lands in bono session; james AMPLIFIER bracket-prefix msg: `[V2-DOC-REORG-PROPOSAL-PHASE-1 · INDEX + FOUNDATION pointer · request AMPLIFIER substantive + parallel author for james-side mirror · gates on Captain proposal disposition]`.

Phase 2-4 require bilateral concurrent presence (parallel `git mv` to avoid divergence).

---

## §7 — Verify-by

- **Verify-by-1 (Phase 1 ship):** `wc -l V2-MASTER-STATE-INDEX.md` ≤ 250 · `wc -l MEMORY-V2-FOUNDATION.md` ≤ 50 · MEMORY.md preamble references both
- **Verify-by-2 (Phase 2 ship):** `wc -l V2-MASTER-STATE.md` ≤ 6000 (down from 21151) · archive files contain rest · grep `§S-` in repo shows zero broken links
- **Verify-by-3 (Phase 3 ship):** `ls memory/V2-foundation/` shows 10-15 files; their content matches pre-move
- **Verify-by-4 (Phase 4 ship):** `wc -l MEMORY.md` ≤ 200 · SessionStart context shows full file (no truncation warning)
- **Verify-by-bilateral (each phase):** james SessionStart picks up new structure; partner-memory-read.js scans new sub-dirs symmetrically

---

## §8 — Composes-with

- V2-PROGRESS-MAP.md (this proposal's companion; the immediate-gap closer)
- PACT-20260503-002 (V2-MASTER-STATE canonical-source ledger; informs Option B append-only preservation)
- Universal Sync doctrine (each phase touches multiple sync targets in same session)
- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (NOT applicable — reorg is doc-class not V1↔V2 boundary)
- `feedback_apply_recommendations_autonomously_20260510.md` (this proposal IS a recommendation; Q3-canonical-surface gate fires → Captain disposition required not auto-apply)
- `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` (this is a doc reorg; not an infrastructure fix; mechanism-trust-check does not fire)

---

## §9 — Captain decision points (single batch ask)

1. **Approve Option B?** (vs A more aggressive / C index-only / D no-action)
2. **Phase 1 immediate?** (lowest-risk; can ship same session)
3. **Phase 2-4 sequencing** — single sprint OR one-per-session?
4. **Bilateral coordination expectation** — bono ships Phase 1 unilaterally + james catches up via session-start? OR bilateral-concurrent for all phases?

---

## §10 — NOT TESTED at DRAFT-PRE-CAPTAIN anchor

- james AMPLIFIER concurrence on Option B (could vote DISAGREE-COUNTER on Phase 2 archive boundary)
- partner-memory-read.js behavior on subdir traversal (assumed it works; needs empirical test in Phase 3)
- Context-load behavior with shrunk MEMORY.md (assumes 200-line cap is real; could be conservative; needs Phase 4 first-run observation)
- Phase 2 git-trace preservation across `git mv`+edit (standard git operation; should preserve `git blame --follow`)
- Live-sync mirror behavior on subdir creates (assumed symmetric; mirror mechanism not deeply audited)

---

— bono · 2026-05-11 ~11:38 IST · V2-DOC-REORG-PROPOSAL v1.0 DRAFT-PRE-CAPTAIN · Q3 canonical-surface gate fires (touches V2-MASTER-STATE = PACT-20260503-002) · Captain decision required §9 · bilateral AMPLIFIER queued post-disposition · companion to V2-PROGRESS-MAP.md
