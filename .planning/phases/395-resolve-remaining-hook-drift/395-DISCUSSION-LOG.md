# Phase 395: Resolve Remaining Hook Drift + Classify — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in `395-CONTEXT.md` — this log preserves the alternatives considered.

**Date:** 2026-04-16
**Phase:** 395-resolve-remaining-hook-drift
**Mode:** Autonomous (user directive: "proceed with your recommendations, continue autonomously")
**Areas discussed:** Classification method, Decision doc location, Manifest format, Classification scope, Drifted-file rigor, Bono-only disposition, James-only disposition, New-drift handling

---

## Classification Method (Gray Area 1)

| Option | Description | Selected |
|--------|-------------|----------|
| Content scan only | Grep each file for Windows/Linux markers; no filename consideration | |
| Purpose-by-filename | Use filename heuristic (e.g., `rp-james-*` = windows-only) without reading content | |
| **Hybrid (content authoritative, filename sanity check)** | Content scan is the decision; filename flags disagreements for manual review | ✓ |
| Manual per-file judgment | Read every file, classify by hand | |

**User's choice:** Autonomous — recommended Hybrid.
**Notes:** Content scan catches genuine OS markers, filename catches obvious misclassifications. Manual fallback for ambiguous files keeps judgment in the loop without slowing the 70-file pass. D-01..D-04 encode the marker grammar and disagreement-handling rule.

---

## Decision Doc Location (Gray Area 2)

| Option | Description | Selected |
|--------|-------------|----------|
| Append to `decision_cgp_drift_resolution.md` | Reuse 394's file to keep all drift resolution in one place | |
| **New file: `decision_hook_drift_classification.md`** | Fresh file scoped to 395's deliverables | ✓ |
| Two files (drift resolution separate from classification manifest prose) | Split drift resolution prose from classification prose | |

**User's choice:** Autonomous — recommended new file.
**Notes:** 394's doc should stay grep-clean for cgp-enforce / cgp-session-inject queries. Cross-linking between the two docs (D-21) preserves reader flow. Both docs + the JSON manifest form the 395 artifact set.

---

## Manifest Format + Location (Gray Area 3)

| Option | Description | Selected |
|--------|-------------|----------|
| Markdown table in decision doc | Human-readable, but install.sh would need to parse Markdown | |
| TOML at `workspace/sync/hooks.toml` | Workspace-native format, but workspace repo doesn't exist until 398 | |
| **JSON at `memory/hook-classification.json` (re-hosted later)** | Portable, jq-parseable on both shells, easy to re-home | ✓ |
| Memory-only decision doc, defer manifest to 403 | Keep 395 prose-only, build manifest later | |

**User's choice:** Autonomous — recommended JSON in memory now, re-home in 398/403.
**Notes:** D-13..D-18 lock the schema. JSON is the install.sh contract; schema stability between 395 and 404 is explicit. 394's cgp-* entries are included by cross-reference so the manifest is complete.

---

## Classification Scope (Gray Area 4)

| Option | Description | Selected |
|--------|-------------|----------|
| Just the 26 non-trivial files (6 drifted + 16 James-only + 4 Bono-only) | Skip the ~40 byte-identical cross-platform files | |
| **Union of all hook files on both machines (~70)** | Every file gets a manifest entry | ✓ |
| Drifted files only (6) | Narrowest scope; classification deferred | |

**User's choice:** Autonomous — recommended union.
**Notes:** Phase 404's install.sh has to know the bucket for EVERY file or it can't operate. Byte-identical files get a trivial one-line entry (D-10) — no rationale burden — so the cost of including them is low while the value for Phase 404 is high.

---

## Canonical-Pick Rigor for 6 Drifted Files (Gray Area 5)

| Option | Description | Selected |
|--------|-------------|----------|
| 394-style per-hunk merge for every file | Full rigor on all 6 | |
| **Lighter: accept James wholesale if parses + superset** | Per-hunk merge only when both sides added distinct features | ✓ |
| Defer to Bono (wait for review) | 395 produces diffs, Bono picks winners | |

**User's choice:** Autonomous — recommended lighter rigor.
**Notes:** 394 treated 2 gate-enforcement hooks with deep per-hunk rigor. The 6 deferred files are advisory hooks (update check, context monitor, prompt guard, statusline, workflow guard, memory staleness check) — lower blast radius. D-05..D-08 preserve the verification checklist (SHA256, parse-check, conditional retention) but allow the rationale paragraph to be brief.

---

## Bono-Only Hook Disposition (Gray Area 6)

| Option | Description | Selected |
|--------|-------------|----------|
| All 4 → `linux-only` automatically | No content scan | |
| **Default linux-only; promote to cross-platform if pure Node with no OS markers** | Opportunistic promotion with audit trail | ✓ |
| Evaluate each for reuse on James | Aggressively look for sharable hooks | |

**User's choice:** Autonomous — recommended default linux-only with promotion exceptions.
**Notes:** D-23..D-25. Bias toward linux-only because promoted files become candidates for install.sh to push to James in Phase 405 — promotion is a non-trivial decision. Clear single-purpose OS scripts get a one-line entry with no deep rationale.

---

## James-Only Hook Disposition (Gray Area 7)

| Option | Description | Selected |
|--------|-------------|----------|
| All 16 → `windows-only` automatically | No content scan | |
| **Default windows-only; content-scan `.js`/`.mjs` for cross-platform promotion** | .ps1/.cmd/.bat auto-classified; Node scripts scanned | ✓ |
| Evaluate each for reuse on Bono | Aggressively look for sharable hooks | |

**User's choice:** Autonomous — recommended default windows-only with selective promotion.
**Notes:** D-22, D-24, D-25. Same bias-toward-single-machine logic as Bono-only. `rp-james-exec.ps1`, `auto-reply-thambu.ps1`, PowerShell scripts → windows-only with no second-guessing. Node scripts (16 files contain several .js entries per prior observations) get scanned against the D-02 marker grammar.

---

## Newly-Discovered Drift Handling (Gray Area 8)

| Option | Description | Selected |
|--------|-------------|----------|
| Defer to a future phase (same as 394 D-15) | Keep scope fixed | |
| **Expand 395 scope to cover any new drift discovered at fresh probe** | Fold new drift in; 395's charter is "remaining drift" | ✓ |
| Stop and escalate to Bono every time | Every drift becomes a ratification blocker | |

**User's choice:** Autonomous — recommended expand scope, with escalation exception for catastrophic discoveries.
**Notes:** D-26..D-28. Phase 394 deferred because its scope was explicitly 2 files. Phase 395's ROADMAP goal uses the word "remaining" — scope is "everything not done yet". Exception (D-28): if >5 additional drifted files or drift in CGP hooks we thought 394 resolved, stop and escalate — that would indicate something edited hook files between 394 and 395 and root cause matters more than forward progress.

---

## Claude's Discretion

Kept flexible (documented in 395-CONTEXT.md Claude's Discretion subsection):
- Exact format of per-file rationale blocks (prose vs table vs per-file section)
- Whether probe output is embedded in the decision doc or linked
- Content-scan marker grammar extension if a real file exposes a gap
- Order of operations within the phase (planner's call)
- JSON pretty-print vs minified (recommended pretty-printed for git diff readability)

## Deferred Ideas

Captured in 395-CONTEXT.md `<deferred>` section:
- Subdirectory layout on disk (Phase 397)
- install.sh authoring (Phase 404)
- Hook fixture tests (Phase 403)
- Probe-green verification (Phase 407)
- Workspace repo creation + manifest re-homing (Phase 397-398/403)
- Automated drift guard pre-commit (Phase 407+)
- Agents + slash commands classification (Phase 402)
- Settings.json classification (Phase 408)

## Session Notes

Discussion was conducted in autonomous mode at user directive ("proceed with your recommendations and continue autonomously"). Recommended defaults were locked for all 8 identified gray areas without interactive Q&A. User's stated goal: "proper restructuring so every time we pull out information, we're all on the same page — you, Bono, and I." The classification manifest + decision doc + MEMORY.md index together form the shared reference surface that future sessions (on either machine) can grep to understand hook state without re-running the probe.

No G9s during discussion. No scope creep. No todos folded (no pending todos matched the phase).
