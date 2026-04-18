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

### Pattern E — Pod 6 AC "Process exited unexpectedly" cluster

#### Sub-pattern E.1 — today's exit-0 multi-minute events (2026-04-18) — RESOLVED

| Field | Value |
|---|---|
| State | RESOLVED — misclassification, not a crash |
| Window | 2026-04-18 12:02-16:44 IST, 6 instances of `Process exited unexpectedly (exit code: 0)` on Pod 6 AC |
| Evidence | Pod 6 `C:\Users\User\Documents\Assetto Corsa\logs\py_log.txt` for the 16:42:21 session (mtime 16:44:20) shows the full RaceControl plugin shutdown sequence: `RaceControl: shutting down...` → `RC Plugin: shared memory marked SHUTDOWN` → `RC Plugin: shared memory closed` → `RaceControl: shutdown complete` → `VMS Connect: shutdown`. Exit code 0 = clean OS-level exit. No crash dump file written for any 2026-04-18 session. errors.txt only contains unrelated `INIReader` warnings about missing `[ASSISTS] STABILITY_CONTROL` / `TYRE_WEAR` keys. Source: rc-agent `:8090/exec` (authenticated, service key from server `racecontrol.toml`), command was a `Get-Content` PowerShell call against the Pod 6 logs directory. |
| Bonus finding | ZL-1/ZL-2 plugin fix is functional on Pod 6 — `RC Plugin: shared memory initialized (400 bytes)` + telemetry writer initialized + on-track state-change events all logged. python.ini `[RACECONTROL]` section is working. |
| Why misclassified | rc-agent's event taxonomy reports any non-rc-agent-initiated process exit as `"Process exited unexpectedly"`. Exit code 0 + RaceControl shutdown lines = customer/staff used the AC menu / ALT-F4 / Drive→Exit. Not a crash. |
| Follow-up | See Pattern H — rc-agent should distinguish clean exits from crashes. |

#### Sub-pattern E.2 — 2026-04-17 51-second 3-PID rapid cluster — UNVERIFIABLE NOW, WATCH

| Field | Value |
|---|---|
| State | EVIDENCE-FLOOR (stale — log files overwritten by today's sessions) |
| Window | 2026-04-17 18:06:50, 18:07:17, 18:07:41 IST (PIDs 17584, 8776, 27692 — three distinct AC processes crashed within 51s) |
| Evidence available | `game_launch_events` rows only. Per-event metadata is empty (no exit code captured at the time — pre-INV-1 binary). Pod 6 AC `log.txt`, `py_log.txt`, etc. were overwritten by 2026-04-18 sessions. |
| Hypotheses remaining | (1) Genuine fast-crash from corrupt preset; (2) AC single-instance guard kicked subsequent launches; (3) Customer rapid-clicked launch button; (4) FFB/Conspit HID handshake failure causing immediate exit. |
| Next step | Wait for recurrence on a binary with INV-1 capture (i.e., `b39c2a6f` or later — ALL pods are now on `b39c2a6f` per fleet check). Next AC rapid-crash on Pod 6 will have the exit code logged + py_log.txt should contain whether shutdown sequence ran or got cut off mid-init. |
| Blocker | No recurrence signal — close after N days clean? |

### Pattern H — rc-agent event taxonomy: clean exit misclassified as "Process exited unexpectedly"

| Field | Value |
|---|---|
| State | OPEN — design proposed, not implemented |
| Source of bug | rc-agent emits the same `event_type=crashed` + `error_message="Process exited unexpectedly (exit code: N)"` for ALL non-rc-agent-initiated exits, regardless of whether the game shut down cleanly. |
| Evidence | Pattern E.1 — Pod 6 AC sessions 2026-04-18 12:02-16:44 IST all reported as `crashed exit 0`, but py_log.txt shows the full RaceControl plugin shutdown sequence ran cleanly. Customer simply quit the game. Same misclassification likely affects every pod for every game when customer ends session via game UI rather than kiosk End-Session button. |
| Impact | Inflates apparent crash counts → noise drowns out real crashes → Pattern E.1 was investigated as a possible bug for two days when it was just normal customer behavior. Real crashes (Pattern A storms, AC Rally 3-min, etc.) are harder to spot in the noise. |
| Proposed fix | At the rc-agent crash-event emission site (`event_loop.rs:1065` per BUG-TRACKER INV-1), check whether: (a) `exit_code == 0`, AND (b) RaceControl plugin emitted a shutdown line in py_log.txt within the last N seconds, OR (c) no Windows Error Reporting crash dump was written. If clean → emit `event_type=stopped` + `error_message=null` (or a new event_type=ended). If unclean → keep current `crashed` taxonomy. |
| Risks | (a) py_log.txt scan is filesystem I/O on every exit — may need a cheaper signal (exit code threshold + time-since-launch heuristic). (b) Steam-launched games (F1 25, iRacing) may not write a known shutdown marker — would need per-sim heuristics. (c) Changing the event taxonomy breaks existing `WHERE event_type='crashed'` queries fleet-wide — needs migration plan. |
| Lower-bound fix (no behavior change) | Add a `clean_exit_heuristic: bool` column to `game_launch_events` that's true when `exit_code == 0` AND `seconds_since_launch > 30`. Then DB queries can filter `WHERE clean_exit_heuristic = false` to see real crashes. Doesn't fix the misleading `error_message` text but lets dashboards split the noise out. |
| Cross-ref | Pattern E.1 resolution depends on this taxonomy decision — without it, every customer end-of-session looks like a crash in the DB. |

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
