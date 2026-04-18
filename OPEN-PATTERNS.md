# Open Patterns — Single Ledger

> **Single source of truth for in-flight debug patterns.** Replaces the 20+ handoff files scattered in `~/.claude/projects/C--Users-bono/memory/session_handoff_*`. Every new session starts here.
>
> Updated: 2026-04-18 IST | HEAD: `cfa73772` | Fleet: `68f4d61e` | Delta: `8a52cc36` + `11664dce` only

## How to use

- **Every session:** read this file first. Update the state column after every action.
- **Lifecycle (strict, no skipping):**
  ```
  REPORTED → TRIAGED → EVIDENCE-FLOOR → HYPOTHESIS → FIX → DEPLOYED → VERIFIED → CLOSED
  ```
  - REPORTED: symptom observed, no classification yet
  - TRIAGED: known pattern (dupe of BUG-TRACKER entry) OR novel (new INV-N assigned)
  - EVIDENCE-FLOOR: pod-local log / DB row / raw output captured from the FAILING machine — no RCA without this
  - HYPOTHESIS: enumerated hypotheses + PoE eliminations (James-reachable data only)
  - FIX: commit hash + regression test that would have caught the bug
  - DEPLOYED: commit live on all affected targets (server, pods, POS, cloud) — per-target evidence
  - VERIFIED: named behavior tested + raw output + where-tested + not-tested list
  - CLOSED: BUG-TRACKER.md updated, memory handoff retired, entry moved to the Closed section

- **Before claiming "evidence floor":** run the pod-local capture command. If SSH/exec is broken, say so in the Notes column — do NOT substitute "reasoning from DB summary" as evidence. This is the CGP H3 WHERE rule enforced at investigation-entry, not just at completion.

- **Anti-drift rule (Rule 0):** memory files claim fleet state from the time they were written. Before acting on any handoff recommendation, `git merge-base --is-ancestor <commit> <fleet_build_id>` to verify the commit is still undeployed. If the handoff claim is already live, retire the claim — don't redo the work.

## Open patterns

### Pattern A — Pod 4 F1 25 orphan/crash storms
| Field | Value |
|---|---|
| State | EVIDENCE-FLOOR (pending pod-local log) |
| First seen | 2026-04-17 (14 crashes, 2 storms 9h apart) |
| Last seen | 2026-04-17 21:13 IST |
| Classification | Known class (INV-3 exit-code-1 variant on Pod 4), but not confirmed as orphan. V-2 fix `bf8a30e4` deployed in `68f4d61e`. |
| Evidence floor | **Missing.** Need Pod 4 `tasklist /V` during storm + `%USERPROFILE%\Documents\My Games\FORMULA ONE 25\` state + Windows Event Viewer `nvlddmkm` entries. Prior SSH attempts returned empty. |
| Hypotheses remaining | (1) EA Anti-Cheat kill; (2) GPU TDR; (3) save corruption; (4) Pod-4 specific hardware |
| Blocker | Pod-local log access (SSH aliases reported broken in prior sessions — check rc-agent `:8090/exec` or rc-sentry `:8091/exec` instead) |
| Notes | INV-1 exit-code capture is LIVE in `68f4d61e` — next F1 25 storm WILL log an exit code automatically. Wait for next storm rather than chase old logs. |

### Pattern E — Pod 6 AC rapid-crash cluster
| Field | Value |
|---|---|
| State | EVIDENCE-FLOOR (pending pod-local log) |
| First seen | 2026-04-17 18:06-18:08 IST (3 AC crashes in 51s) |
| Last seen | 2026-04-17 18:08 IST |
| Classification | INV-1 class (no exit code captured at time — `68f4d61e` now captures exit codes, next crash will be informative) |
| Evidence floor | **Missing.** Need Pod 6 `%USERPROFILE%\Documents\Assetto Corsa\logs\log.txt` for the 18:06-18:08 window + `python.ini` contents to confirm ZL-1/ZL-2 actually applied. |
| Hypotheses remaining | (1) corrupt AC preset; (2) ZL-1 plugin still broken on this pod specifically; (3) FFB/Conspit HID mid-session; (4) customer selected content that instant-crashes |
| Blocker | Pod-local log access (same as Pattern A) |
| Notes | Lower priority than Pattern A because the old logs lack exit codes. Wait for next crash with INV-1 capture live. |

### Pattern INV-10 — AC Rally 3-min deterministic crash (Pod 1)
| Field | Value |
|---|---|
| State | CLOSED |
| First seen | 2026-04-17 13:04 IST |
| Resolution | Fixed 2026-04-18 via `5fcabd38` (BILL-14). Server-side `billing_timer_expiry_timeout::handle_launch_timeouts` was retrying at 180s when non-AC games reached `Running` but never emitted `AcStatus::Live`, defaulting to AssettoCorsa adapter on `sim_type=None` and swapping adapter on the running game. Fix adds two guards: suppress retry when Running, never default to AC. |
| Deployed | `5fcabd38` IN `68f4d61e` (fleet verified via `git merge-base --is-ancestor`) |
| Verified | 3 regression tests pass. Runtime verification deferred — needs AC Rally session on Pod 1. **NOT TESTED: runtime reproduction on Pod 1.** |
| Notes | Pattern was previously tracked as "Pattern B" in session handoffs. Closed here, should also be struck through in BUG-TRACKER if present. |

## Resolved / deferred patterns (kept for cross-session continuity)

### Pattern C — Pod 3 iRacing Steam dialog (INV-9)
- **CLOSED.** INV-9 fix `49bcd69b` is in fleet `68f4d61e`. Memory handoffs claiming "not deployed" are stale — verified via `git merge-base`. No further action.

### Pattern D — Pod 6 "Launch timed out (30s)"
- **CLOSED (by-design).** Not a launch timeout — it's the `game_launcher_support.rs:131` Stopping-state cleanup firing in normal operation. BUG-TRACKER INV-4 wording should drop the "when the server restarts" phrasing (action: edit BUG-TRACKER on next session touching it).

### Pattern F — Pods 2 / 5 / 6 / 7 silent days
- **CLOSED (not a bug).** Billing pipeline symmetric with launch events: zero billing events + zero launch events = truly idle pods, no customers. Operational question, not telemetry bug.

### F1 25 launch fix chain (6 layers)
- **CLOSED** via Phase 413.1 (`68f4d61e`). Chain documented in [project_f1_25_launch_fix.md](../../.claude/projects/C--Users-bono/memory/project_f1_25_launch_fix.md). No further re-regression since `bf8a30e4` V-2 deployed.

## Process rules this ledger enforces

1. **One-ledger principle.** No pattern lives in a handoff file alone. If it's active, it's here. Retire handoffs after migrating.
2. **Evidence floor before RCA.** No pattern moves to HYPOTHESIS without pod-local data. "We reasoned from DB summaries" is not evidence.
3. **ROI-first next step.** Each entry's "Blocker" column names the single observation that eliminates the most hypotheses. That's the next action — not whatever feels urgent.
4. **Regression test as landing gate.** No FIX → DEPLOYED transition without a test that would have caught the bug. F1 25's 6-chain regression is the existence proof this matters.
5. **One-pattern-per-deploy** for novel fixes. Bundle known-pattern deploys only. Attribution requires it.
6. **Anti-drift check.** Every handoff recommendation is verified against current fleet build_id before acting. Memory goes stale in days, not months.

## Cross-references

- [BUG-TRACKER.md](BUG-TRACKER.md) — authoritative bug catalog (to be synced with this ledger)
- [LOGBOOK.md](LOGBOOK.md) — commit audit trail
- `~/.claude/projects/C--Users-bono/memory/session_handoff_20260417_*.md` — historical PoE analysis (to be retired as patterns close here)
- [docs/CLOSED-LOOP-DEBUG.md](docs/CLOSED-LOOP-DEBUG.md) — the 5-step method feeding the lifecycle above
