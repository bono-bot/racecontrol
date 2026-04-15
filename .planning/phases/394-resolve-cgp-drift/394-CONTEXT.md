# Phase 394: Resolve CGP Drift — Context

**Gathered:** 2026-04-15
**Status:** Ready for planning
**Milestone:** v52.0 Claude Workspace Restructure
**Predecessor:** Phase 393 Foundation Decisions (committed `55755c28`, awaiting Bono ratification — NOT a blocker for 394)

<domain>
## Phase Boundary

Diff `cgp-enforce.js` and `cgp-session-inject.js` across James (`~/.claude/hooks/`) and Bono VPS (`~/.claude/hooks/`), pick a canonical winner per file with per-hunk rationale, and commit the decision to memory. **Scope is these two files only.** No filesystem writes to either machine's live hooks — that's Phase 401/402. No migration into a canonical repo — that repo doesn't exist yet (Phase 396). The deliverable is a decision document + canonical text blob, nothing on-disk outside of memory/.

Phase 394 is the prerequisite gate for Phase 401 (hooks migration on James) per `project_workspace_restructure.md` Risk #2: "before overwriting Bono's hooks, diff each drifted file and decide canonical direction per file."

</domain>

<decisions>
## Implementation Decisions

### Canonical-Picking Rule
- **D-01:** Superset-wins with per-hunk merge. When James and Bono differ on a hunk, the winning text is whichever side has the strictly-more-capable logic; if both sides added distinct features, merge both into the canonical. "Newer timestamp" is NOT the rule — mtime lies after `git pull`, `touch`, and editor saves.
- **D-02:** Every hunk decision is recorded with a one-line rationale ("Bono added MCP tool allowlist — kept", "James added Smart Pipes classifier — kept", etc.). No silent picks.
- **D-03:** Tiebreaker when both sides are functionally equivalent: prefer James's version (James is the session currently running; avoids a needless edit when eventually deployed).

### Scope
- **D-04:** Only `cgp-enforce.js` and `cgp-session-inject.js`. Do NOT open the scope to other hooks even if the probe reveals drift elsewhere. Log additional drift as a deferred finding for Phase 400 (probe-driven sweep).
- **D-05:** No filesystem writes to `~/.claude/hooks/` on either machine during 394. The canonical text is a string in the decision document, not a committed file. Disk reconciliation happens in Phase 401/402 via `install-hooks.sh`.

### Platform Classification
- **D-06:** During 394, each canonical file is classified as cross-platform, windows-only, or linux-only, but that classification is metadata in the decision doc — no `cross-platform/` / `windows-only/` / `linux-only/` subdirs are created yet. Subdir layout is Phase 401's responsibility.
- **D-07:** If a hunk contains OS-conditional logic (e.g., Git Bash `TZ` workaround, `fs.readFileSync(0)` stdin handling), the canonical retains the conditional — don't strip it, don't split the file. Keep a single cross-platform file where feasible.

### Decision Artifact Location
- **D-08:** New memory file: `~/.claude/projects/C--Users-bono/memory/decision_cgp_drift_resolution.md`. Rationale: memory/ auto-pushes to Bono VPS backup + GitHub; Bono's next session reads it via partner-memory-read hook — no extra sync step required. Phase 393 used the same mechanism successfully.
- **D-09:** The decision doc contains: (a) raw probe output showing the drift, (b) per-file winner + rationale, (c) full canonical text as a fenced code block, (d) SHA256 of canonical text, (e) platform classification per file.
- **D-10:** An index entry is added to `MEMORY.md` pointing to the decision doc.

### Bono Coordination Model
- **D-11:** James diffs, decides, writes the canonical, commits the decision doc. Bono ratifies asynchronously via comms-link reply (same pattern as Phase 393). Phase 394 does NOT block on Bono's reply — ratification happens before Phase 401 begins.
- **D-12:** Ratification message sent via comms-link in the same session the decision doc is committed. Message references the doc path and the canonical SHA256 so Bono can verify on its side without re-running the diff.
- **D-13:** If Bono counters a decision (via comms-link reply), the decision doc is amended in a new commit — no force-push, no silent rewrites. Phase 394 closes only after Bono has explicitly ACKed or no reply within 48 hours (deferred-default to James's picks).

### Drift Measurement
- **D-14:** Re-run `cgp-distribution-probe.js` at the start of Phase 394 execution to produce a fresh diff. Probe output saved to `394-resolve-cgp-drift/PROBE-OUTPUT-<timestamp>.txt`. Do NOT trust prior-session probe output — hook files may have been touched between sessions.
- **D-15:** If the fresh probe reveals drift on files OTHER than these two, log the paths in the decision doc's "Deferred" section and leave them for Phase 400. Do NOT expand 394's scope.

### Verification Gate for "Resolved"
- **D-16:** Phase 394 is resolved when: (a) decision doc exists at the memory path, (b) both canonical text blocks are syntactically valid JavaScript (`node -c` or equivalent parse check), (c) canonical SHA256 is recorded, (d) comms-link ratification message sent to Bono, (e) MEMORY.md index updated. On-disk reconciliation is explicitly NOT part of the resolved gate.
- **D-17:** Running `cgp-distribution-probe.js` AFTER Phase 394 will still show drift — that's expected and correct, because no disk writes happened. Probe-green is Phase 403's gate, not 394's.

### Claude's Discretion
- Exact formatting of per-hunk rationale within the decision doc (prose vs table)
- Whether to include a unified diff or full side-by-side in the decision doc (pick whichever is more readable for the amount of drift found)
- SHA256 vs SHA1 vs content-addressable hash choice (any cryptographic hash is fine)
- Whether to use `diff -u` or a JavaScript-aware diff tool

</decisions>

<specifics>
## Specific Ideas

- Decision doc should read like a post-merge commit message on steroids — future sessions should be able to reconstruct *why* each hunk was picked without re-running the diff.
- Phase 393 committed Foundation Decisions as 6 separate artifacts in racecontrol (`55755c28`). Phase 394's artifact is intentionally smaller: one memory file, because the workspace repo that would host it doesn't exist yet.
- The canonical text for each file is the source-of-truth that Phase 401's `install-hooks.sh` will eventually write to `~/.claude/hooks/`. Treat it accordingly — any bug in 394's canonical becomes a fleet-wide bug in 401.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner) MUST read these before acting.**

### Project context
- `~/.claude/projects/C--Users-bono/memory/project_workspace_restructure.md` — Full scope of v52.0, the "do not migrate drift into canonical" rule, Risk #2 (per-file decision), phased migration plan
- `~/.claude/projects/C--Users-bono/memory/session_handoff_20260415_v52_phase393.md` — Phase 393 status, Bono ratification pattern, 6 Foundation Decisions that 394 inherits
- `.planning/ROADMAP.md` §v52.0 (lines 1946-2014) — Phase 394 definition, predecessor/successor context, core gate

### Drift source material
- `~/.claude/hooks/cgp-enforce.js` (James, 99 lines, 3440 bytes, Apr 4 03:30) — one side of the drift
- `~/.claude/hooks/cgp-session-inject.js` (James, 156 lines, 7027 bytes, Apr 11 12:00) — other side of the drift
- Bono VPS `~/.claude/hooks/cgp-enforce.js` and `~/.claude/hooks/cgp-session-inject.js` — fetched via `curl -s -X POST http://localhost:8766/relay/exec/run` at the start of Phase 394 execution. NOT cached from prior sessions.
- `~/.claude/projects/C--Users-bono/memory/scripts/cgp-distribution-probe.js` — the parity oracle. Run fresh at start of 394.

### Protocol definitions (what the hooks enforce)
- `C:/Users/bono/racingpoint/racecontrol/COGNITIVE-GATE-PROTOCOL.md` — CGP v4.3 spec. The canonical hook behavior must match this spec; where hooks and spec disagree, the spec wins.
- `~/.claude/projects/C--Users-bono/memory/feedback_cgp43_backlog_gate.md` — v4.3 Backlog Gate reference (affects cgp-session-inject.js injection text)
- `~/.claude/projects/C--Users-bono/memory/feedback_cgp41_smart_pipes.md` — v4.1 Smart Pipes classifier reference (affects cgp-session-inject.js risk classifier)

### Sync contract
- `~/.claude/projects/C--Users-bono/memory/reference_partner_memory_sync.md` — how the memory/ auto-push reaches Bono; Phase 394's decision doc rides this channel
- `~/.claude/comms-link.env` (NEVER commit) — comms-link PSK/URL for ratification message

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable assets
- `cgp-distribution-probe.js` already exists at `memory/scripts/` — it produced the original drift finding. Phase 394 reuses it as-is, no changes. Do NOT move it to `workspace/scripts/` yet (that's Phase 397).
- `partner-memory-read` hook already propagates memory/ changes to Bono's next session — no new plumbing needed for the decision doc.
- Phase 393 already established the Bono ratification pattern via comms-link. 394 reuses the same message shape.

### Established patterns
- Memory files with `---` frontmatter + `name:`, `description:`, `type:` — 394's decision doc follows this (type: project).
- Per-hunk rationale is novel for this repo — no existing memory file uses it. Establish format in 394, reuse in 402 if Bono-side drift resolution needs the same treatment.

### Integration points
- MEMORY.md index: one new line pointing to the decision doc
- comms-link outbound: one ratification message referencing the doc path + SHA256
- racecontrol repo: untouched during 394 (this phase is memory-only)

### Constraints from the environment
- James is Windows (Git Bash). Bono is Linux (bash). Canonical text must be portable — any Windows-specific path handling (backslashes, `\r\n`) is an automatic "needs conditional" flag.
- Hook files are loaded by Claude Code from the fixed path `~/.claude/hooks/`. This constrains what Phase 401 can do, but is not a 394 concern — 394 only writes text into memory/.
- memory/ auto-push runs on every write; verify the decision doc's rationale is sanitized before commit (no PSK leaks, no OpenRouter keys — the cgp-session-inject.js classifier mentions patterns like `OPENROUTER_KEY` but the canonical should not contain actual values).

</code_context>

<deferred>
## Deferred Ideas

- **Drift on other hooks** — if the fresh probe run in 394 reveals drift on `backlog-enforce.js`, `g9-auto-detect.js`, `pre-flight-file-read.js`, or any other file, log the paths and defer to Phase 400 (sync tooling phase) or a new dedicated phase. Do NOT expand 394's scope.
- **Workspace repo creation** — Phase 396.
- **`install-hooks.sh` that actually writes to disk** — Phase 400.
- **Subdir split into `cross-platform/` / `windows-only/` / `linux-only/`** — Phase 401.
- **Parity verification via probe-green output** — Phase 403.
- **Automated drift guard (pre-commit hook that runs probe)** — Phase 403 or later.
- **Rebuilding Bono's hook set from canonical** — Phase 402.
- **CGP lean restructure (project_cgp_v5_restructure.md)** — separate track; not v52.0 scope.

</deferred>

---

*Phase: 394-resolve-cgp-drift*
*Context gathered: 2026-04-15*
*Next step: `/gsd:plan-phase 394` (no research phase needed — drift resolution is a single-session, deterministic task; planner can read this CONTEXT.md directly)*
