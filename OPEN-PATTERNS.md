# Open Patterns — Single Ledger

> **Single source of truth for in-flight debug patterns.** Replaces the 20+ handoff files scattered in `~/.claude/projects/C--Users-bono/memory/session_handoff_*`. Every new session starts here.
>
> Updated: 2026-04-18 22:30 IST | HEAD: `cfa73772` | Server: `d4b60fb5` (verified `/api/v1/health`, process StartTime 18/04 19:44 IST) | Pods rc-agent: TBD per-target | Delta: `8a52cc36` + `11664dce` + later
>
> **Anti-drift note:** earlier header claimed fleet `68f4d61e`. Server is actually `d4b60fb5` (newer). Per-pod build_ids not re-verified this session — call out before any "fleet-wide" claim.

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
| Last seen | 2026-04-18 16:42:21 IST (multi-minute exit-0 variant; older 51s rapid cluster did not recur today) |
| Classification | INV-1 class. INV-1 fix LIVE on `d4b60fb5` — exit codes ARE captured today (e.g., `Process exited unexpectedly (exit code: 0)` on Pod 6 at 12:11/12:18/12:24/16:36/16:42 IST). Sub-class: clean game exit, not crash dump. |
| Evidence floor | **Missing pod-local.** `game_launch_events` confirms the exit code = 0 (clean exit) for today's instances. Need Pod 6 `%USERPROFILE%\Documents\Assetto Corsa\logs\log.txt` for one of those crash windows + `python.ini` contents. |
| Hypotheses remaining | (1) Customer ALT-F4'd cleanly each time; (2) AC content (mod / car / track) crashed AC cleanly; (3) ZL-1 plugin still broken on this pod specifically; (4) Single-instance check kicking AC out when something tries a re-launch. Exit-code-0 rules out hard segfault. |
| Blocker | Pod-local log access. Try rc-agent `:8090/exec` with `type` command. |
| Notes | Today's data is on the fixed binary (16:42 IST is on `d4b60fb5`'s prev — actually mtime 13:58 IST so 16:42 is on prev binary). The 12:18-12:24 cluster was on the morning binary. Mixed-binary data — be careful with attribution. |

### Pattern G — Dynamic-timeout under-estimation triggers BILL-14 retry cascade

| Field | Value |
|---|---|
| State | OPEN — root caused, fix not designed |
| First seen | 2026-04-18 17:19:43 IST — Pod 4 F1 25 fired `Launch timed out (39s)` ×3 in 33 seconds, then 4th attempt finally reached Running |
| Source | `crates/racecontrol/src/metrics.rs:327` — `query_dynamic_timeout` returns `ceil((median + 2σ)/1000)` of the last ≤10 SUCCESSFUL launches matching `(sim_type, car, track)`, floored at 30s. Falls back to per-sim default (90s/120s) if < 3 samples. |
| Failure mode | When historical successful F1 25 launches for a specific car/track were ~25-35s, dynamic timeout = 39s. Today's launch was slower (cold cache / EA AC update / unrelated delay), exceeded 39s, server killed it. Next retry then triggered BILL-14 retry cascade — and if `sim_type` was None on the retry, the AC adapter swap kills any running game. |
| Algorithm fragility | (a) 3 samples is too few for σ to be meaningful; (b) launch times have long-tail distribution, normal-2σ underestimates; (c) no cold-cache awareness; (d) no per-game patch-day adjustment. |
| Proposed fix (not yet designed) | Floor at `max(per_sim_default, computed)` instead of `max(30, computed)`. Or use 95th percentile + headroom instead of median+2σ. Or require ≥5 samples before trusting the dynamic value. |
| Evidence | Today's `game_launch_events` for Pod 4 17:19:40-17:20:13 — three "Launch timed out (39s)" events 4-5 seconds apart, then `running` PID 6128 at 17:20:23. |

### Pattern INV-10 — AC Rally 3-min deterministic crash (Pod 1)
| Field | Value |
|---|---|
| State | UNVERIFIED — was CLOSED, now reset pending fresh runtime test |
| First seen | 2026-04-17 13:04 IST |
| Code fix | `5fcabd38`-bundle (`already_running` guard + `sim_type=None` advance). Source verified at `crates/racecontrol/src/billing_timer_expiry_timeout.rs:240-310`. |
| Deployed | Code is in `d4b60fb5` (verified via `git show d4b60fb5:` inspection — both guards present at lines 247, 286). Binary built from `d4b60fb5`, process started 2026-04-18 19:44 IST. |
| **Recurrence today (pre-fix-binary)** | Pod 1 AC Rally launched 2026-04-18T07:32:18Z, ran for 3 min, server log at 07:35:15Z shows `WARN Launch timeout (attempt 1) for pod pod_1 — allowing retry (attempt 2)` from `racecontrol_crate::billing_timer_expiry_timeout`, retry sent `Launching (AssettoCorsa)` swap, Rally crashed `Error (AssettoCorsaRally)` at 07:35:20Z. **All on the OLD binary** — server didn't restart with `d4b60fb5` until 14:14Z (19:44 IST). So this recurrence does NOT invalidate the fix; it just shows the fix wasn't running yet. |
| Runtime verification | **NOT YET TESTED on `d4b60fb5`.** Need a fresh non-AC launch (AC Rally / iRacing / F1 25 / LMU / AC Evo) on Pod 1 that reaches Running and stays > 3 min. If it survives the 180s retry-window without an `AssettoCorsa` adapter swap, the fix is verified. If retry still fires (or sim defaults to AC), the guards are wired wrong. |
| Notes | Pattern was previously tracked as "Pattern B" in session handoffs. Reset because the "CLOSED" claim relied on regression-test pass alone, not on production reproduction. Closing this requires Step 4 of CLOSED-LOOP-DEBUG (re-run the EXACT failing test from Step 1). |

## Resolved / deferred patterns (kept for cross-session continuity)

### Pattern C — Pod 3 iRacing Steam dialog (INV-9) — REOPENED 2026-04-18

| Field | Value |
|---|---|
| State | OPEN — recurrence on the fixed binary |
| Code fix | `49bcd69b` removes the dismiss-once flag in `steam_checks.rs`; retries every 2s poll cycle. |
| Deployed | `49bcd69b` IS in `d4b60fb5` (verified). |
| **Recurrence on fixed binary** | Pod 3 iRacing 2026-04-18 20:17:31 IST (UTC 14:47:31) — `Game window not detected: Game failed to launch - only Steam dialog visible after 60s timeout`. Server was on `d4b60fb5` since 19:44 IST (2:14 hours before). |
| Implication | The dismiss-once removal is INSUFFICIENT. Either (a) Steam introduced a new dialog form not matched by the dismiss code, (b) Pod 3 has stuck Steam state (login expired, update pending), (c) the 60s window-detect deadline elapses before the dismiss loop catches the dialog. |
| Next action | (1) Snapshot Pod 3's Steam state (`tasklist /V` for `steamwebhelper.exe`, `steamerrorreporter64.exe`, login dialog window class) via rc-agent `:8090/exec`. (2) Read `steam_checks.rs` poll cycle timing — does it actually retry every 2s or is there a race with the 60s window timeout? (3) Check whether 49bcd69b adds the new SteamUI dialog class to its dismiss list. |

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
