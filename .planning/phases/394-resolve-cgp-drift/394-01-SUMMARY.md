---
phase: 394-resolve-cgp-drift
plan: 01
subsystem: cgp-hooks
tags: [cgp, drift, decision-only, memory-only, phase-401-prereq]
dependency-graph:
  requires: [394-CONTEXT.md (D-01..D-17)]
  provides: [canonical cgp-enforce.js, canonical cgp-session-inject.js, Phase 401/402 unblock prerequisite]
  affects: [~/.claude/projects/C--Users-bono/memory/ (decision doc + MEMORY.md index), comms-link INBOX.md]
tech-stack:
  added: []
  patterns: [superset-wins per-hunk merge, memory-as-source-of-truth for canonical text]
key-files:
  created:
    - "~/.claude/projects/C--Users-bono/memory/decision_cgp_drift_resolution.md"
    - ".planning/phases/394-resolve-cgp-drift/PROBE-OUTPUT-20260415-2330.txt"
  modified:
    - "~/.claude/projects/C--Users-bono/memory/MEMORY.md"
    - "C:/Users/bono/racingpoint/comms-link/INBOX.md"
decisions:
  - "James version picked for both files (superset-wins, D-03 tiebreaker)"
  - "cgp-enforce.js drift is header-comment-only; code body byte-identical"
  - "cgp-session-inject.js: James v4.3 is strict superset of Bono v3.2"
  - "6 other drifted files DEFERRED to Phase 400"
  - "NO disk writes to ~/.claude/hooks/ on either machine — mtime snapshots confirm"
metrics:
  duration_minutes: 6
  completed_date: "2026-04-15"
  tasks_completed: 6
  tasks_total: 6
---

# Phase 394 Plan 01: Resolve CGP Drift Summary

**One-liner:** Canonicalized cgp-enforce.js and cgp-session-inject.js per-hunk with James picks (superset-wins), committed to memory as decision doc, ratification request delivered to Bono via INBOX + WS. No disk writes to live hooks.

## Completed Tasks

| # | Name | Commit |
|---|------|--------|
| 1 | Fresh probe run (D-14) | racecontrol `a51c5911` |
| 2 | Three-way diff + hunk enumeration | (analysis, no commit) |
| 3 | Canonical cgp-enforce.js parse-check + SHA256 | (scratch, folded into Task 5) |
| 4 | Canonical cgp-session-inject.js parse-check + SHA256 | (scratch, folded into Task 5) |
| 5 | Write decision doc | memory `40b0a11` |
| 6 | MEMORY.md index + comms-link ratification | memory `da382b1`, comms-link `68d453f` |

## Artifacts

- **Probe evidence:** `.planning/phases/394-resolve-cgp-drift/PROBE-OUTPUT-20260415-2330.txt` (racecontrol `a51c5911`)
- **Decision doc:** `~/.claude/projects/C--Users-bono/memory/decision_cgp_drift_resolution.md` (memory `40b0a11`, amended `da382b1`)
- **MEMORY.md index:** `- [**Phase 394 — CGP drift resolution**](decision_cgp_drift_resolution.md)` under "Active Work — Handoff"
- **Comms-link INBOX:** entry `2026-04-15 23:37 IST — from james` (commit `68d453f`, pushed to origin/main)

## Canonical SHA256s

| File | SHA256 | Size | Source |
|------|--------|------|--------|
| cgp-enforce.js | `8765f29b6c104262c668fb602e32dbf3aa39f6ba4e0fe393116078cc70335e12` | 3440 bytes | James (unmodified) |
| cgp-session-inject.js | `aad8b7c84786a4a453eed94de5e3763c26df50e66ee34e60bceeafbbdffd9fa2` | 7027 bytes | James (unmodified) |

Both parse clean with `node --check`.

## Key Findings

**cgp-enforce.js (42-byte drift):** Only the 11-line header docstring differs. Code body is byte-identical. James has the current "v4.0" label referencing current protocol; Bono still has "v3.0" / "G0 PROBLEM/SYMPTOMS/PLAN" wording. Pure documentation drift, no functional impact.

**cgp-session-inject.js (2511-byte drift):** James v4.3 is a deliberate consolidation of Bono v3.2 per the v4.0 refactor (147 rules → 5 hard gates). James strictly adds:
- Smart Pipes v4.1 risk classifier (73 lines, `classifyRisk()`)
- stdin prompt extraction via `fs.readFileSync(0, "utf8")` (OS-agnostic)
- 5 hard gates H1-H5 + Permanence Gate + Universal Sync + Session Metrics

James drops:
- Bono's `pod-verify.sh` / `verify-fix.sh` / `verify-action.sh` / `validate-plan.mjs` recommendations — folded into CLAUDE.md standing rules instead
- Bono's delete-before-SCP warning — now a CLAUDE.md standing rule
- Bono's "Trial = AC only / domain-rules.json" domain note — not hook-appropriate

All drops documented with rationale rows per D-09.

## Bono Coordination

Dual-channel delivery executed per D-12:
- **INBOX.md:** commit `68d453f` (pushed to origin/main), timestamped `2026-04-15 23:37 IST` via `inbox-append.js` (not manual Edit per G9 from session_handoff_20260415_v52_phase393.md)
- **WebSocket:** `send-message.js` acknowledged by server (COMMS_PSK sourced from `~/.claude/comms-link.env`)

Ratification state: **AWAITING BONO REPLY**. Phase 394 does not block on reply per D-13; Phase 401/402 will block for up to 48h then default to James picks.

## Deferred Drift (out of scope, Phase 400)

Per D-15, 6 other drifted files logged in the decision doc but NOT acted on:

| File | James | Bono |
|------|-------|------|
| gsd-check-update.js | 4310b | 5368b |
| gsd-context-monitor.js | 6051b | 6362b |
| gsd-prompt-guard.js | 3438b | 3490b |
| gsd-statusline.js | 4704b | 5274b |
| gsd-workflow-guard.js | 3357b | 3353b |
| memory-staleness-check.js | 4503b | 4559b |

Plus 16 James-only and 4 Bono-only files (intentional per current roles, no action needed).

## Authentication Gates

None. Comms-link PSK was sourced from `~/.claude/comms-link.env` as designed. SSH to `bono-vps` worked on first try for Bono-side file fetch (relay `shell` subcommand was rejected, fallback per plan).

## Deviations from Plan

**None — plan executed exactly as written.**

Minor notes (not deviations):
- Relay `{"command":"shell"}` was rejected with `Unknown command: shell`; the plan's SSH fallback (`ssh bono-vps "cat ..."`) was used as instructed. This was explicitly planned for and documented in the probe artifact.
- Tasks 3 and 4 produced scratch files (`/tmp/canonical-*.js`) that were consumed by Task 5's decision doc write — no separate commit per task since scratch files are not tracked.

## D-16 Verification Gate

- **(a) decision doc exists:** PASS
- **(b) both canonical blocks parse as valid JavaScript:** PASS (`node --check` clean on both /tmp source files; extracted blocks from decision doc also parse OK)
- **(c) SHA256 recorded for both files:** PASS (`grep -c "Canonical SHA256" = 2`)
- **(d) comms-link ratification delivered:** PASS (INBOX `68d453f`, WS acknowledged)
- **(e) MEMORY.md index entry exists:** PASS
- **(f) no disk writes to ~/.claude/hooks/:** PASS (mtime snapshots before/after are identical — `diff /tmp/394-hook-mtimes-before.txt /tmp/394-hook-mtimes-after.txt` empty)

**Expected remaining drift (D-17, not a failure):** `cgp-distribution-probe.js` still shows drift on both files because no disk writes happened. This is correct per the plan — probe-green is Phase 403's gate, not 394's.

## G9 Count

**0** G9s this session. No user corrections triggered.

## Self-Check: PASSED

- decision_cgp_drift_resolution.md: FOUND (memory repo)
- PROBE-OUTPUT-20260415-2330.txt: FOUND (racecontrol repo)
- MEMORY.md index entry: FOUND
- INBOX.md entry: FOUND (comms-link `68d453f`)
- racecontrol commit `a51c5911`: FOUND
- memory commit `40b0a11`: FOUND
- memory commit `da382b1`: FOUND
- comms-link commit `68d453f`: FOUND

## Link Back

- Decision doc: `~/.claude/projects/C--Users-bono/memory/decision_cgp_drift_resolution.md`
- Plan: `.planning/phases/394-resolve-cgp-drift/394-01-PLAN.md`
- Context: `.planning/phases/394-resolve-cgp-drift/394-CONTEXT.md`
- Probe: `.planning/phases/394-resolve-cgp-drift/PROBE-OUTPUT-20260415-2330.txt`
