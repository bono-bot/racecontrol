# Phase 395: Resolve Remaining Hook Drift + Classify Single-Machine Hooks — Context

**Gathered:** 2026-04-16
**Status:** Ready for planning
**Milestone:** v52.0 Claude Workspace Restructure
**Predecessors:** Phase 393 Foundation Decisions (locked, awaiting Bono ratification), Phase 394 CGP Drift Resolution (✓ complete, superset-wins pattern established)
**Mode:** Autonomous discussion (recommended defaults locked without interactive Q&A at user request 2026-04-16)

<domain>
## Phase Boundary

Two deliverables, scoped as the "mop up the rest of the hook layer" phase before canonical layout (397) and install tooling (404):

1. **Canonicalize 6 deferred drifted hook files** from 394's D-15 deferred list:
   `gsd-check-update.js`, `gsd-context-monitor.js`, `gsd-prompt-guard.js`,
   `gsd-statusline.js`, `gsd-workflow-guard.js`, `memory-staleness-check.js`.
   Same superset-wins rule as 394, but with lighter rigor because these are advisory
   hooks, not gate enforcers (see D-05 below).

2. **Classify every hook** in the union of `~/.claude/hooks/` across James and Bono
   into one of three buckets: `cross-platform`, `windows-only`, `linux-only`.
   Output: machine-readable JSON manifest that Phase 404 `install.sh` will consume
   verbatim. Every file in the union must have a bucket — including the ~40 already
   byte-identical files that need no drift resolution.

**In scope:**
- Per-file drift resolution for the 6 deferred files (canonical text + SHA256 + bucket)
- Classification for ALL files in the union, not just drifted ones
- JSON manifest at `~/.claude/projects/C--Users-bono/memory/hook-classification.json`
- New decision doc `decision_hook_drift_classification.md` + MEMORY.md index entry
- Comms-link ratification message to Bono referencing the new doc + manifest SHA
- Handling of any additional drift discovered at the start of 395 execution (expand scope, don't defer)

**Out of scope:**
- No disk writes to `~/.claude/hooks/` on either machine — identical to 394's posture
- No creation of `cross-platform/` / `windows-only/` / `linux-only/` subdirectories on disk (that's Phase 397's job)
- No workspace repo operations (repo doesn't exist yet; Phase 398)
- No `install.sh` authoring (Phase 404)
- Hook fixture tests (Phase 403)
- CGP-enforce.js / cgp-session-inject.js — already resolved in 394, do not re-touch

</domain>

<decisions>
## Implementation Decisions

### Classification Method (Gray Area 1)

- **D-01:** Hybrid classification. **Content scan is authoritative**, filename is sanity check, manual judgment for ambiguous files.
- **D-02:** Content-scan marker grammar:
  - **Windows markers:** `powershell`, `tasklist`, `schtasks`, `\\.\\`, `C:\\`, `.bat`, `wmic`, `winmgmt`, `process.platform === "win32"`, `HKLM`, `Start-Process`, `cmd /C`, `taskkill`
  - **Linux markers:** `/proc/`, `/etc/`, `systemctl`, `process.platform === "linux"`, `apt-get`, `/root/`, `/var/log/`
  - **OS-conditional:** if BOTH Windows and Linux markers appear in branching logic (`if platform === "win32" ... else ...`), classify as `cross-platform` (the conditional IS the portability)
  - **Neither:** pure Node with no OS-specific paths/commands → `cross-platform`
- **D-03:** Filename sanity check rules:
  - `.ps1` / `.cmd` / `.bat` → default `windows-only` (override requires explicit cross-platform marker)
  - `rp-james-*` → default `windows-only`
  - `rp-bono-*` → default `linux-only`
  - `.js` / `.mjs` / `.sh` → rely on content scan
- **D-04:** When content scan and filename disagree, content scan wins. Log the disagreement in the decision doc so Bono's review can catch misclassification.

### Canonical-Pick Rigor for the 6 Drifted Files (Gray Area 5)

- **D-05:** **Lighter than 394.** Default pick rule: if James's version parses clean (`node --check`), is a functional superset of Bono's (no dropped behaviors), and has no Windows-only assumption, accept James wholesale with a one-line rationale ("James is functional superset, parses clean, no OS-specific code"). Per-hunk merge is required ONLY when both sides added distinct non-overlapping features — rare for these advisory hooks.
- **D-06:** Same D-03 tiebreaker from 394: James wins on functional equivalence.
- **D-07:** Each canonical file still gets SHA256 + size + `node --check` result recorded in the decision doc. Rigor is lighter on the *rationale paragraph*, not the *verification checklist*.
- **D-08:** If a canonical file ends up being OS-conditional after merge (rare — only if one side adds platform branching), retain the conditional per 394 D-07. Don't split into two files.

### Classification Scope (Gray Area 4)

- **D-09:** Classify **every file** in the union of `~/.claude/hooks/` on James and Bono. The union is approximately 70 files: 40+ byte-identical cross-platform, 6 drifted (this phase canonicalizes), 16 James-only, 4 Bono-only. Every file gets a bucket entry in the manifest.
- **D-10:** Byte-identical files get a **trivial classification entry** (filename + bucket + SHA256) with NO rationale prose. They don't need a canonical decision — they ARE canonical by virtue of being identical on both sides.
- **D-11:** The 6 drifted files and the 20 single-machine files get a **full entry** (filename + bucket + rationale + SHA256 of canonical version + which-side-is-canonical). The drift-resolution and classification decisions for these are the meat of the decision doc.
- **D-12:** Single-machine default classification:
  - **James-only (16):** default `windows-only` if filename ends `.ps1`/`.cmd`/`.bat`; scan content if `.js`/`.mjs`; promote to `cross-platform` only if content is genuinely OS-agnostic AND the file would be useful on Bono (Claude's judgment during execution).
  - **Bono-only (4):** default `linux-only`; promote to `cross-platform` only if content is genuinely OS-agnostic AND the file would be useful on James.

### Manifest Format + Location (Gray Area 3)

- **D-13:** Manifest is **JSON**, not TOML or Markdown. Install.sh will parse it with `jq` — works identically under Git Bash and Linux bash, no format-conversion step.
- **D-14:** Manifest location (for Phase 395): `~/.claude/projects/C--Users-bono/memory/hook-classification.json`. Rationale: memory auto-push reaches Bono; same transport as 394's decision doc.
- **D-15:** Manifest schema:
  ```json
  {
    "version": "1.0",
    "generated": "2026-04-16T__:__:__+05:30",
    "generator": "phase-395",
    "sources": {
      "james": "C:/Users/bono/.claude/hooks/",
      "bono": "/root/.claude/hooks/"
    },
    "hooks": [
      {
        "filename": "cgp-enforce.js",
        "bucket": "cross-platform",
        "canonical_source": "james",
        "canonical_sha256": "8765f29b...",
        "size_bytes": 3440,
        "drifted": false,
        "rationale_ref": "decision_cgp_drift_resolution.md#cgp-enforce"
      },
      {
        "filename": "gsd-check-update.js",
        "bucket": "cross-platform",
        "canonical_source": "james",
        "canonical_sha256": "...",
        "size_bytes": 5368,
        "drifted": true,
        "rationale_ref": "decision_hook_drift_classification.md#gsd-check-update"
      },
      {
        "filename": "rp-james-exec.ps1",
        "bucket": "windows-only",
        "canonical_source": "james",
        "canonical_sha256": "...",
        "size_bytes": 4200,
        "drifted": false,
        "rationale_ref": "decision_hook_drift_classification.md#rp-james-exec"
      }
    ]
  }
  ```
- **D-16:** Manifest includes **CGP-enforce.js and cgp-session-inject.js** entries (already canonicalized in 394) by referencing the 394 decision doc via `rationale_ref`. The manifest is the single source of truth for Phase 404; it must be complete, not partial.
- **D-17:** Manifest gets re-homed to `workspace/sync/hook-classification.json` in Phase 398 (workspace skeleton) or Phase 403 (hook test fixtures). Until then, memory is the home. Install.sh authored in Phase 404 will read from the workspace location, not memory — the re-home is part of the 398/403 work, not 395's concern.
- **D-18:** The decision doc (`decision_hook_drift_classification.md`) holds human-readable rationale and raw diffs; the JSON manifest holds machine-readable structure. Both live in memory during 395; both migrate together.

### Decision Doc Location (Gray Area 2)

- **D-19:** **New file:** `~/.claude/projects/C--Users-bono/memory/decision_hook_drift_classification.md`. Do NOT append to 394's `decision_cgp_drift_resolution.md` — that file is scoped to the two CGP gate hooks and should stay grep-clean.
- **D-20:** New MEMORY.md index entry under "Active Work — Handoff" pointing to the classification doc (mirrors 394's entry format).
- **D-21:** Cross-link: `decision_hook_drift_classification.md` links back to `decision_cgp_drift_resolution.md` in its header for reader context; `decision_cgp_drift_resolution.md` gets a one-line footer referencing 395's doc so forward readers find it.

### Bono-Only and James-Only Hook Disposition (Gray Areas 6 + 7)

- **D-22:** James-only 16 files: default `windows-only`; only `.js`/`.mjs` files get content-scanned for promotion to cross-platform. Any file promoted to cross-platform becomes a candidate for install.sh to push to Bono in Phase 405-406 — so promotion is a non-trivial decision. **Default bias is toward windows-only** — promote only when the evidence is unambiguous.
- **D-23:** Bono-only 4 files: default `linux-only`; same content-scan promotion rule. **Default bias is toward linux-only.**
- **D-24:** For each promotion or demotion decision, the decision doc records: filename, original default, actual pick, marker evidence from content scan, one-line rationale. This creates an audit trail so Bono can review.
- **D-25:** Files that are CLEARLY single-purpose-OS-specific (PowerShell admin helpers, systemd scripts) get a one-line entry: "filename: windows-only (PowerShell script)" — no deep rationale needed, obvious from filename + shebang.

### Newly-Discovered Drift Handling (Gray Area 8)

- **D-26:** **Expand scope, do NOT defer again.** Unlike 394 (which was explicitly scoped to 2 files and deferred other drift), Phase 395's charter from ROADMAP is "Resolve Remaining Hook Drift" — the word "remaining" means all of it. If the fresh 395 probe reveals drift beyond the known 6 files, those files get folded into the canonicalization work in this same phase.
- **D-27:** Exception: if the additional drift is in files that would obviously be single-machine-only (e.g., a `.ps1` file only on James with a Linux counterpart that's clearly unrelated), classify them as single-machine and skip drift resolution — they're not actually "drifted", they're two different files with coincidentally similar names.
- **D-28:** If newly-discovered drift is catastrophic (>5 additional drifted files, or drift in CGP hooks we thought 394 resolved), STOP and escalate to Bono before proceeding. This is a red flag that something touched hook files between 394 and 395 and we need to understand why.

### Fresh Probe Requirement

- **D-29:** Inherit 394 D-14: re-run `cgp-distribution-probe.js` at the start of 395 execution. Save probe output to `.planning/phases/395-resolve-remaining-hook-drift/PROBE-OUTPUT-<timestamp>.txt`. Do NOT trust 394's probe output — hooks may have been touched between sessions.
- **D-30:** Probe output is the ground truth for "what exists on each machine right now". The classification manifest MUST be internally consistent with the probe.

### Bono Coordination Model

- **D-31:** Same dual-channel pattern as 394: INBOX.md append (via `inbox-append.js`, NEVER manual Edit) + WS `send-message.js`. Ratification message references the new decision doc path + manifest path + manifest SHA256 so Bono can verify without re-running the diff.
- **D-32:** Phase 395 does NOT block on Bono's ratification reply. Same deferred-default as 394 D-13: if Bono doesn't reply within 48 hours before Phase 404 begins, default to James's picks. Phase 405 (James hook migration) and Phase 406 (Bono hook migration) are the gates where ratification actually matters.
- **D-33:** If Bono counters a classification decision via comms-link reply, the decision doc + JSON manifest are amended in a new commit. No force-push, no silent rewrites (same as 394 D-13).

### Verification Gate for "Resolved"

- **D-34:** Phase 395 is resolved when:
  - (a) Fresh probe output saved
  - (b) All 6 previously-deferred drifted files have canonical text in the decision doc with SHA256
  - (c) All canonical files parse clean (`node --check` or equivalent per language)
  - (d) JSON manifest exists, is valid JSON (`jq empty`), and contains an entry for every file in the union of James + Bono `~/.claude/hooks/`
  - (e) Decision doc exists, MEMORY.md index updated, comms-link ratification delivered (INBOX + WS)
  - (f) No disk writes to `~/.claude/hooks/` on either machine — mtime snapshot before/after is identical
- **D-35:** Running `cgp-distribution-probe.js` AFTER 395 will STILL show drift — same expected state as 394 D-17. Probe-green is Phase 407's gate, not 395's.
- **D-36:** The manifest passing `jq empty` is necessary but not sufficient. Also verify: union-of-James-plus-Bono file count equals `jq '.hooks | length'`. If counts don't match, the manifest has a gap.

### Claude's Discretion

- Exact formatting of per-file rationale blocks in the decision doc (prose vs table vs per-file section)
- Whether the probe output is embedded in the decision doc or linked from it
- Granularity of content-scan markers — Claude can extend the marker grammar in D-02 if a real file exposes a gap, but must document the extension
- Order of operations within the phase (probe first, then drift resolution, then classification, vs interleaved) — planner's call
- Choice of JSON pretty-print vs minified for the manifest (recommend pretty-printed so `git diff` is readable)

### Folded Todos

None. No pending todos matched Phase 395's scope.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents (researcher, planner) MUST read these before acting.**

### Predecessor phase artifacts (inheriting rules)
- `.planning/phases/394-resolve-cgp-drift/394-CONTEXT.md` — D-01..D-17 (superset-wins rule, memory-only posture, tiebreaker, coordination model). Phase 395 explicitly inherits D-03, D-07, D-11, D-13, D-14, D-17.
- `.planning/phases/394-resolve-cgp-drift/394-01-SUMMARY.md` — list of 6 deferred drifted files with byte sizes, plus 16 James-only + 4 Bono-only counts
- `.planning/phases/394-resolve-cgp-drift/PROBE-OUTPUT-20260415-2330.txt` — the original probe that produced the deferred list. Use as a reference point ONLY; do NOT use as current ground truth (D-29 requires fresh probe).
- `~/.claude/projects/C--Users-bono/memory/decision_cgp_drift_resolution.md` — 394's decision doc. The cgp-enforce / cgp-session-inject manifest entries in 395 link back to this.
- `.planning/phases/393-foundation-decisions/393-CONTEXT.md` — Foundation Decisions FND-01..FND-08. Relevant: FND-02 (drift resolution path), FND-04 (install model = copy not symlink), FND-05 (CI gate, including hook parity check).

### Project context
- `~/.claude/projects/C--Users-bono/memory/project_workspace_restructure.md` — full v52.0 scope, Risk #2 (per-file decision), sync contract
- `~/.claude/projects/C--Users-bono/memory/project_v52_restructure_20260416.md` — Option A 20-phase map that introduced split of FND-02 across 394 (cgp) and 395 (remaining)
- `.planning/ROADMAP.md` §v52.0 (lines ~1966-2014) — Phase 395 definition, downstream dependency chain into 403/404

### Classification inputs (live machines)
- `~/.claude/hooks/` on **James** (Windows, Git Bash) — one side of the union
- `/root/.claude/hooks/` on **Bono VPS** — other side of the union; fetch via comms-link relay `git_pull`/`exec` or SSH fallback at start of execution
- `~/.claude/projects/C--Users-bono/memory/scripts/cgp-distribution-probe.js` — the parity oracle. Run fresh per D-29.

### Sync + coordination contract
- `~/.claude/projects/C--Users-bono/memory/reference_partner_memory_sync.md` — how memory auto-push reaches Bono (395's decision doc and manifest both ride this channel)
- `~/.claude/comms-link.env` (NEVER commit) — PSK/URL for ratification message
- `C:/Users/bono/racingpoint/comms-link/INBOX.md` — append target for ratification; use `inbox-append.js`, not manual Edit (G9 from session_handoff_20260415_v52_phase393.md)
- `~/.claude/projects/C--Users-bono/memory/feedback_logbook_parallel_session_race.md` — parallel-session clobber risk; verify post-push commit hash matches

### Feedback references (things to NOT redo wrong)
- `~/.claude/projects/C--Users-bono/memory/feedback_verify_before_generate.md` — enumerate union of hook dirs from filesystem, not from 394's stale list
- `~/.claude/projects/C--Users-bono/memory/feedback_poe_primary_method.md` — PoE before any "we classified all hooks" claim (H4 gate)
- `~/.claude/projects/C--Users-bono/memory/reference_local_capabilities.md` — check before claiming any tool is unavailable

### Downstream consumers (what depends on 395's output)
- **Phase 403 (Hook Tests Fixtures):** Consumes canonical text blocks from 394 + 395 to build test fixtures
- **Phase 404 (install.sh + verify-parity.sh):** Consumes the JSON manifest directly. Schema stability between 395 and 404 matters — any schema change after 395 breaks 404.
- **Phase 405/406 (Hook Migration James + Bono):** Consumes the install.sh that consumes the manifest. Ratification of 395 classifications is a gate for 406.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable assets
- `cgp-distribution-probe.js` already exists in `memory/scripts/` and produced the original drift output. 395 re-runs it unchanged. Do NOT modify the probe.
- 394 established the per-hunk rationale + SHA256 + parse-check pattern. 395 reuses the format, applying lighter rigor per D-05.
- `inbox-append.js` already handles timestamp-correct INBOX append (the G9-learned way). Ratification message flow is the same as 394.
- `partner-memory-read` hook already propagates memory/ changes to Bono's next session. No new plumbing for the decision doc or JSON manifest.
- Existing `MEMORY.md` index pattern for "Active Work — Handoff" section — 395 adds one line in the same style as 394.

### Established patterns
- Memory files with `---` frontmatter + `name:`, `description:`, `type:` — 395's decision doc uses `type: project`.
- JSON manifests in memory/ are a **new pattern for v52.0** — neither 393 nor 394 produced one. Phase 395 establishes the format; Phases 398/403/404 re-use it. Format stability matters, so D-15 schema is the contract.
- Dual-channel Bono ratification (INBOX + WS) is already battle-tested in 394.
- "Deferred Drift" logging per D-26 is a new pattern (394 deferred, 395 expands) — the expansion rule is explicit in the decision doc so a future similar phase doesn't re-ask.

### Integration points
- `~/.claude/projects/C--Users-bono/memory/MEMORY.md` — one new index line
- `~/.claude/projects/C--Users-bono/memory/decision_hook_drift_classification.md` — new file
- `~/.claude/projects/C--Users-bono/memory/hook-classification.json` — new file
- `.planning/phases/395-resolve-remaining-hook-drift/PROBE-OUTPUT-<timestamp>.txt` — new artifact
- `C:/Users/bono/racingpoint/comms-link/INBOX.md` — new append line
- racecontrol repo: only `.planning/phases/395-*` receives commits. Hooks dir untouched.

### Constraints from environment
- James = Windows + Git Bash, Bono = Linux + bash. JSON + `jq` is portable; hook classification must work under both shells.
- Hook files are loaded from fixed path `~/.claude/hooks/` on each machine. Classification dictates what install.sh copies to each target in Phase 404 — a misclassification in 395 becomes a fleet-wide install bug in 405/406.
- Memory auto-push runs on every write. Verify decision doc and manifest have no embedded secrets (PSK, OpenRouter keys, etc.) before committing. Hooks that reference secrets should describe them by name, never paste them.
- Probe + classification must complete in a single session per v52.0 session discipline ("ONE phase per session, maximum").

### Gotchas inherited from prior sessions
- Parallel-session LOGBOOK clobber risk — verify `git log -1` after push to confirm YOUR commit hash is HEAD before claiming success.
- `TZ=Asia/Kolkata date` silently fails under Git Bash — use `bash scripts/ist-now.sh` or the python fallback for any IST timestamps in the decision doc.
- `inbox-append.js` NOT manual `Edit` for INBOX — enforced G9 from 393 session.

</code_context>

<specifics>
## Specific Ideas

- The decision doc should read like a sortable per-file reference. Future sessions should be able to ctrl-F a filename and find its bucket + rationale in one scroll.
- JSON manifest formatting: pretty-printed with 2-space indent so `git diff` highlights per-file changes clearly. Machine readers don't care; human reviewers do.
- Per-file rationale length: one paragraph max. These 70-odd files don't need essay-length justifications — a sentence is usually enough. 394 treated 2 files with deep rigor; 395 treats 70 files with surface rigor. Budget your prose.
- Consider a "classification summary" table at the top of the decision doc: counts per bucket, counts of drifted vs clean, counts of promoted vs defaulted. Bono should be able to read the top of the doc and understand the shape of the manifest without reading every entry.
- If a hook's purpose is ambiguous (Claude can't tell what it does from filename + content), flag it in the doc with `NEEDS REVIEW — purpose unclear` and default its bucket to its origin machine (James-only → windows-only, Bono-only → linux-only). Don't guess.

</specifics>

<deferred>
## Deferred Ideas

- **Subdirectory layout on disk** (`cross-platform/`, `windows-only/`, `linux-only/`) — Phase 397
- **install.sh that actually writes to disk** — Phase 404
- **Hook test fixtures built from canonical text** — Phase 403
- **Parity verification via probe-green** — Phase 407
- **Workspace repo creation** (where manifest will eventually live permanently) — Phase 397-398
- **Automated drift guard (pre-commit hook running probe)** — Phase 407 or later
- **Re-hoisting the manifest from memory/ to workspace/sync/** — Phase 398/403 (when workspace exists)
- **Classification of agents + slash commands** — Phase 402 (MIG-05) handles `agents/` and `commands/` separately
- **Settings.json classification** — Phase 408 (CLN-01) handles shared-vs-local settings

### Reviewed Todos (not folded)
None — no todos matched Phase 395's scope during cross-reference step.

</deferred>

---

*Phase: 395-resolve-remaining-hook-drift*
*Context gathered: 2026-04-16 (autonomous mode, recommended defaults locked)*
*Next step: `/gsd:plan-phase 395 --auto` (no research phase needed — deterministic drift resolution + classification, planner can read this CONTEXT.md directly)*
