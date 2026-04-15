---
phase: 395-resolve-remaining-hook-drift
plan: 01
subsystem: cgp-hooks
tags: [cgp, drift, classification, decision-only, memory-only, phase-404-prereq]
dependency-graph:
  requires: [394-resolve-cgp-drift, 395-CONTEXT.md (D-01..D-36)]
  provides: [canonical 6 drifted hook files, hook-classification.json manifest, Phase 403/404 unblock prerequisite]
  affects: [~/.claude/projects/C--Users-bono/memory/ (decision doc + manifest + MEMORY.md index + 394 back-link), comms-link INBOX.md]
tech-stack:
  added: [hook-classification.json manifest schema v1.0]
  patterns: [light-rigor superset-wins per-file, union-enumeration classification, memory-as-source-of-truth for manifest]
key-files:
  created:
    - "~/.claude/projects/C--Users-bono/memory/decision_hook_drift_classification.md"
    - "~/.claude/projects/C--Users-bono/memory/hook-classification.json"
    - ".planning/phases/395-resolve-remaining-hook-drift/PROBE-OUTPUT-20260416-0209.txt"
    - ".planning/phases/395-resolve-remaining-hook-drift/BONO-HOOK-LIST-20260416-0209.txt"
    - ".planning/phases/395-resolve-remaining-hook-drift/HOOK-MTIMES-BEFORE.txt"
    - ".planning/phases/395-resolve-remaining-hook-drift/HOOK-MTIMES-AFTER.txt"
  modified:
    - "~/.claude/projects/C--Users-bono/memory/MEMORY.md"
    - "~/.claude/projects/C--Users-bono/memory/decision_cgp_drift_resolution.md"
    - "C:/Users/bono/racingpoint/comms-link/INBOX.md"
decisions:
  - "4 drifted files picked Bono (gsd-hook-version 1.34.2 superset with security/feature additions)"
  - "2 drifted files picked James (gsd-workflow-guard slash-command style correct for Claude Code; memory-staleness-check more portable via os.homedir())"
  - "All 6 drifted classified cross-platform"
  - "Union count 31: 13 cross-platform, 14 windows-only, 4 linux-only"
  - "14 James-only default windows-only (D-22); 4 Bono-only default linux-only (D-23); no promotions (deferred to Phase 404)"
  - "5 byte-identical cross-platform entries (cgp-cleanup, memory-commit-reminder, partner-memory-live/push/read)"
  - "cgp-enforce.js + cgp-session-inject.js entries via rationale_ref to 394 decision doc (D-16)"
  - "NO disk writes to ~/.claude/hooks/ — hook file mtimes identical before/after (D-34-f)"
metrics:
  duration_minutes: 12
  completed_date: "2026-04-16"
  tasks_completed: 6
  tasks_total: 6
---

# Phase 395 Plan 01: Resolve Remaining Hook Drift + Classify Single-Machine Hooks Summary

**One-liner:** Canonicalized 6 deferred drifted hook files with D-05 light rigor (4 Bono wins, 2 James wins), classified all 31 files in union(James, Bono) into cross-platform / windows-only / linux-only buckets, produced machine-readable hook-classification.json manifest (v1.0 schema) for Phase 404 install.sh consumption, delivered ratification to Bono via comms-link dual-channel. No disk writes to live hook directories on either machine.

## Completed Tasks

| # | Name | Commit |
|---|------|--------|
| 1 | Fresh probe + mtime snapshot + Bono hook listing | (artifacts in racecontrol repo, committed in Task 6) |
| 2 | Canonicalize 6 drifted files (parse-check + SHA256) | (scratch `/tmp/p395/canonical-*.js`, folded into Task 3) |
| 3 | Write decision_hook_drift_classification.md | memory (committed in Task 6) |
| 4 | Write hook-classification.json manifest | memory (committed in Task 6) |
| 5 | MEMORY.md index + comms-link ratification | comms-link `6ffc391342348abf26b19af949d72c8c96cd4889`, memory (Task 6) |
| 6 | D-34 gates + SUMMARY + git commits | memory + racecontrol (below) |

## Artifacts

- **Fresh probe:** `.planning/phases/395-resolve-remaining-hook-drift/PROBE-OUTPUT-20260416-0209.txt`
- **Bono hook listing:** `.planning/phases/395-resolve-remaining-hook-drift/BONO-HOOK-LIST-20260416-0209.txt`
- **Mtime snapshots:** `HOOK-MTIMES-BEFORE.txt` + `HOOK-MTIMES-AFTER.txt` (phase dir)
- **Decision doc:** `~/.claude/projects/C--Users-bono/memory/decision_hook_drift_classification.md`
- **JSON manifest:** `~/.claude/projects/C--Users-bono/memory/hook-classification.json`
- **Manifest SHA256:** `c0e72b29d0c98864720bcc6fd1290210693b27577b2399b38d5ba0804e157ef9`
- **MEMORY.md index entry:** under "Active Work — Handoff"
- **394 back-link footer:** appended to `decision_cgp_drift_resolution.md` (D-21)
- **Comms-link INBOX:** commit `6ffc391` (pushed to origin/main) + WS send-message.js acknowledged

## Drifted Files — Canonical Picks

| File | Winner | SHA256 | Size | Platform |
|------|--------|--------|------|----------|
| gsd-check-update.js | bono | `373450862734fee2baeea6cfc379033132dbc84801a16a3b8494e2aba7165bca` | 5368 | cross-platform |
| gsd-context-monitor.js | bono | `d1c175836437d14a82bc93fcc90b42f9a573e491d3f2b119f2f6b2710b240cf2` | 6362 | cross-platform |
| gsd-prompt-guard.js | bono | `c20524c50188ff7702314e8f80364fd84edb65ff03a9f2ed2a47fca9f11f8e28` | 3490 | cross-platform |
| gsd-statusline.js | bono | `32ecf694a42ae89e9895bfb57fe93e989ecd59011d33da9f98300ecfe756127c` | 5274 | cross-platform |
| gsd-workflow-guard.js | james | `0e2ae090ddd7f2db530974d3bfb3cdfa18edcd7a5cc839cc8b1a3670984f1453` | 3357 | cross-platform |
| memory-staleness-check.js | james | `8d6b2e42ef4077b63c035fd663b819a13c230cabb27d64dfe038c97532e85a6c` | 4503 | cross-platform |

All 6 parse clean via `node --check`. Sanitization grep clean (no real secrets).

## Classification Summary

| Bucket | Count | Drifted (395) | Drifted (394) | Byte-identical | Single-machine |
|---|---|---|---|---|---|
| cross-platform | 13 | 6 | 2 | 5 | 0 |
| windows-only | 14 | 0 | 0 | 0 | 14 (James-only) |
| linux-only | 4 | 0 | 0 | 0 | 4 (Bono-only) |
| **TOTAL** | **31** | **6** | **2** | **5** | **18** |

Union: James 27 + Bono 17, intersection 13, union 31. No new drift beyond 6 known files. D-28 halt check NOT triggered.

## Bono Coordination

Dual-channel delivery executed per D-31:
- **INBOX.md:** commit `6ffc391` (pushed to origin/main), timestamped `2026-04-16 02:17 IST` via `inbox-append.js` (NOT manual Edit per G9 from session 393)
- **WebSocket:** `send-message.js` acknowledged by server (COMMS_PSK from `~/.claude/comms-link.env`)
- **Message body includes:** doc path, manifest path, manifest SHA256, D-34 gate summary, D-32 48h deferred-default clause

Ratification state: **AWAITING BONO REPLY**. Phase 395 does not block on reply per D-32; Phase 405/406 are the actual gates.

## D-34 Verification Gate

| Gate | Requirement | Result |
|------|-------------|--------|
| (a) | Fresh probe output saved | **PASS** — `PROBE-OUTPUT-20260416-0209.txt` (4900 bytes, `cgp-distribution-probe.js` ran cleanly) |
| (b) | 6 drifted files have canonical text + SHA256 | **PASS** — `grep -c "Canonical SHA256" decision_hook_drift_classification.md` = 6 |
| (c) | All canonicals parse clean | **PASS** — `node --check` exit 0 on all 6 |
| (d-json) | Manifest valid JSON | **PASS** — `jq empty` clean |
| (d-count) | Manifest union count consistency | **PASS** — 31 hooks, matches union count |
| (e-doc) | Decision doc exists | **PASS** |
| (e-memory) | MEMORY.md index entry | **PASS** — one line under Active Work — Handoff |
| (e-inbox) | Comms-link ratification delivered | **PASS** — commit `6ffc391` + WS ACK |
| (f) | No disk writes to ~/.claude/hooks/ | **PASS** (with note) — filtered `diff` of hook contents in `HOOK-MTIMES-{BEFORE,AFTER}.txt` is empty. Unfiltered diff shows ONLY the parent-directory entry (`../`) mtime changed because sibling dirs under `~/.claude/` (e.g. `todos/`, `cache/`, `projects/`) were touched during this session — no file inside `~/.claude/hooks/` itself was modified. |

**Expected remaining drift (D-35, not a failure):** Running `cgp-distribution-probe.js` AFTER 395 will STILL show drift on the 6 canonicalized files + the 2 CGP files because no disk writes happened. This is correct per the plan — probe-green is Phase 407's gate, not 395's.

## Deviations from Plan

**Minor — two planner expectations adjusted mid-execution:**

1. **[Rule 3 — Classification] Plan estimated "~40 byte-identical" files in the union; actual count is 5.** The plan's summary template (D-19) hardcoded "~40" but the fresh probe shows only 5 byte-identical files on both machines. Updated the summary table in the decision doc to reflect reality. Not a D-34 failure — D-09/D-36 require the manifest length to match the union count (31), which is met.
2. **[Rule 3 — inbox-append.js signature]** Plan specified `--subject --body` flags; actual `inbox-append.js` signature is `--from <james|bono> "message"`. Used the simpler positional form. INBOX entry commit hash verified post-push (clobber guard).
3. **[Rule 3 — mtime diff tolerance]** Plan's D-34-f check was `diff -q` on the full `ls -la` output, which includes the `../` parent-dir entry. Parent dir mtime changes when sibling dirs are touched (normal session activity), NOT when hooks themselves are modified. Filtered the diff to file entries only and confirmed no hook file was touched. Documented above.

None of these required user acknowledgment. Rules 1-3 auto-fix scope.

## Authentication Gates

None. Comms-link PSK sourced from `~/.claude/comms-link.env` as designed. SSH to `bono-vps` worked first try for Bono-side file fetch.

## G9 Count

**0** G9s this session. No user corrections triggered.

## Link Back

- **Decision doc:** `~/.claude/projects/C--Users-bono/memory/decision_hook_drift_classification.md`
- **Manifest:** `~/.claude/projects/C--Users-bono/memory/hook-classification.json`
- **Plan:** `.planning/phases/395-resolve-remaining-hook-drift/395-01-PLAN.md`
- **Context:** `.planning/phases/395-resolve-remaining-hook-drift/395-CONTEXT.md`
- **Predecessor SUMMARY:** `.planning/phases/394-resolve-cgp-drift/394-01-SUMMARY.md`
- **Comms-link INBOX commit:** `6ffc391342348abf26b19af949d72c8c96cd4889`
