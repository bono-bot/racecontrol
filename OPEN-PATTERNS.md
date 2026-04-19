# Open Patterns — Single Ledger

> **Single source of truth for in-flight debug patterns.** Replaces the 20+ handoff files scattered in `~/.claude/projects/C--Users-bono/memory/session_handoff_*`. Every new session starts here.
>
> Updated: 2026-04-19 08:02 IST | HEAD: `7375fd9c` | Server: `66fec05c` (status=ok, fleet 9/9) | Pods rc-agent: `66fec05c` on 8/8 via :8090 HTTP; **Pattern I CLOSED** — 30-min soak PASS, Pods 1+6 WS restored. POS still on `e7e01ae3` (not Pattern I-affected). Delta since 2026-04-18: Pattern I defense-in-depth (`09acbbe4` rc-agent WS Ping + `90b04d71` server clone agent_senders) deployed venue + fleet 07:29 IST; snap-to-package test fixes `1de266ef`; L1 health-endpoint git_commit truth in `84a8b69a`.
>
> **CORRECTION (G9 2026-04-18 23:30 IST):** Earlier in this session I wrote "Server stays d4b60fb5 (intentional)" into this header, LOGBOOK.md, INBOX.md, and a comms-link WS to Bono. That claim is FALSE — server was upgraded to `2c27e2fc-dirty` at 23:05:20 IST (same minute as my pod swap; concurrent parallel-session or Uday-approved manual swap). I asserted the build_id from a single read at session start and did not re-verify before absolute claims. **Structural:** any absolute claim about a remote system's build/state must be preceded by a fresh `/api/v1/health` or equivalent probe in the same action block. Not "I checked 20 minutes ago." The BILL-14 runtime-verify target is now 2c27e2fc (not d4b60fb5) — which is a SUPERSET of the d4b60fb5 guards, so the verify is still meaningful, just against a newer fix level.
>
> **Anti-drift note:** earlier header claimed fleet `68f4d61e`, then `d4b60fb5`, then `b39c2a6f`. Fleet now on `a6d29291`; server is on `2c27e2fc-dirty` (was `d4b60fb5` at session start; drifted mid-session).

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

## Session follow-ups — 2026-04-19 00:05 IST

Captured at session end; these are watch/queue items, not active investigations. Do not close any pattern row on the strength of these notes — they exist so the next session re-surfaces them.

- **Server dirty flag (`2c27e2fc-dirty`) — origin unconfirmed.** Header already notes the concurrent 23:05:20 IST swap; still unknown whether the uncommitted delta came from a parallel session, a manual Uday edit, or a build-artifact leak. **Next session:** on server, `cd C:\RacingPoint && git status -- .` from the racecontrol source tree (if one exists there) OR diff the deployed binary's `GIT_HASH` vs `git describe --dirty` on James's checkout at HEAD to localise the delta. Low priority — doesn't block any deploy.
- **Pattern G NOT on server.** metrics.rs floor change (test-verified locally) did not ship in the `2c27e2fc` server binary — bundled with the next server bounce. Keep the single-occurrence cost/benefit (see Pattern G row) — don't bounce server just for G.
- **BILL-14 / Pattern INV-10 runtime-verify target is now `2c27e2fc`**, which is a superset of `d4b60fb5`'s guards + Pattern H's DashboardEvent CommandError on sim_type=None. Verification event still the same: next Pod 1 non-AC customer launch that reaches Running and stays >3 min without an `AssettoCorsa` adapter swap.
- **Pod 3 `game_state=error` is stale.** Leftover from this morning's iRacing failure (Pattern C — INV-9 v2 steam_checks recurrence on the fixed binary). Unaffected by the Pod 1/6 restarts. Clears on next Pod 3 iRacing launch — which is also the window for the INV-9 v2 diag to log the observed top-level window classes. Do not force-clear by synthetic launch; wait for a real customer event.
- **Latent rc-agent WS-reconnect diagnostic gap (→ Pattern I).** Silent WS-reconnect-forever with no `/debug/ws-state` endpoint; 100 MB jsonl > 50 MB `/file` limit makes remote diagnosis painful. Small follow-up commit.

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

### Pattern A-valve — blanking + session-summary stuck when WS is down
| Field | Value |
|---|---|
| State | FIX (`a13942f2` committed, NOT DEPLOYED) |
| Symptom | Pod 4 2026-04-19 ~18:30-21:40 IST: lock_screen_state=ActiveSession with no game alive, `ws_connected=false`, `silent_reconnect_suspected=true` for 3.2h+. Customer sees raw desktop after game end/crash; no session-summary card. |
| Root cause | safety-net-01 tick (added in `0306fe17`) lives inside the WS-connected tokio::select! arm and stores `stuck_active_session_since` on `ConnectionState`. `ConnectionState` is dropped on every WS disconnect, so during silent-reconnect-forever the 15s stuck window can never accumulate. Comment at `event_loop.rs:1511-1512` already documented this class ("Observed on Pod 3 on 2026-04-19: stuck in ActiveSession for 3h+ while WS was down") — same mechanism now hit Pod 4. Session-summary rides the same WS signal pipeline so is also silently suppressed while WS is down. |
| Fix applied | `a13942f2`: move `stuck_active_session_since` from `ConnectionState` to `AppState`; make `safety_net_01_decide` `pub(crate)`; add `run_safety_net_01_reconnect` + `sleep_with_safety_net` helpers in `main.rs`; replace the three `tokio::time::sleep(delay)` calls in the reconnect loop with the new helper (wakes every 5s, runs the same invariant, force-idles lock screen on fire). Pure function untouched → 8/8 existing unit tests still pass + 2 lock_screen SAFETY-NET-01 invariant tests still green. |
| Deploy gate | Pods 2-8 via rc-sentry atomic swap (Pod 8 canary first). Pod 1 held per Pattern I DiD policy. Server unaffected. Bono VPS N/A (rc-agent is pod-only). Binary staged at `deploy-staging/rc-agent-a13942f2.exe` (26706432 bytes, sha256 `9ddf6dd44adf3687…`). |
| Remaining gap | This is a **safety valve** — it keeps the customer-visible screen correct when WS is down. It does NOT fix the underlying WS reconnect failure (Pattern I class). Whatever left Pod 4 silent-reconnecting today is separately tracked as a Pattern I recurrence. |
| Verification target | After deploy to Pod 4: observe `lock_screen_state` leave `active_session` within 20s even if WS remains down (force-idle path); after WS recovers, kiosk "Thank you for racing!" summary card should render on next session-end. |

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
| State | FIX (code complete, NOT DEPLOYED). Surfaces 1+2+3+5 shipped in `aa52b813` (SQL migration) + `021cbaf4` (rc-agent emission, server ingestion, API filter). Surface 4 (cloud sync manifest) is a documented no-op — `game_launch_events` is venue-local, not in SYNC_TABLES. All 47 game_launcher unit tests pass on 2026-04-19. |
| Source of bug | rc-agent emits the same `event_type=crashed` + `error_message="Process exited unexpectedly (exit code: N)"` for ALL non-rc-agent-initiated exits, regardless of whether the game shut down cleanly. |
| Evidence | Pattern E.1 — Pod 6 AC sessions 2026-04-18 12:02-16:44 IST all reported as `crashed exit 0`, but py_log.txt shows the full RaceControl plugin shutdown sequence ran cleanly. Customer simply quit the game. Same misclassification likely affects every pod for every game when customer ends session via game UI rather than kiosk End-Session button. |
| Impact | Inflates apparent crash counts → noise drowns out real crashes → Pattern E.1 was investigated as a possible bug for two days when it was just normal customer behavior. Real crashes (Pattern A storms, AC Rally 3-min, etc.) are harder to spot in the noise. |
| Decision (2026-04-18 23:05 IST) | **Go with the lower-bound `clean_exit_heuristic: bool` column first. Defer the full taxonomy split.** Column definition: `TRUE` when `exit_code == 0` AND `seconds_since_launch >= 30` AND no `WerFault.exe` child for this PID was observed in the last 10s. Written by rc-agent at crash-event emission. `event_type` stays `crashed` for all non-agent-initiated exits (preserves every existing query). Dashboards and real-crash alerting add `WHERE clean_exit_heuristic = 0` to see the actual-crash signal. |
| Rationale | (1) **Blast radius:** column-add migration affects one table; taxonomy split requires auditing every `WHERE event_type='crashed'` across racecontrol, admin, whatsapp-bot, cloud sync, and any saved dashboard filters — unscoped and easy to miss. (2) **Reversibility:** column can be dropped; a changed taxonomy is irreversible without another migration. (3) **Policy separation:** the column stores the *signal* (exit code + time + crash-dump presence); "what is a crash" stays a query-time decision, which we're not yet confident about (exit-0-in-10s-after-launch and exit-0-after-30min-race are different signals and may deserve different downstream handling). (4) **Shippable in one commit**, no cross-service coordination needed. (5) **Enables the bigger fix later:** once the heuristic has ~2-4 weeks of production data, we can backtest whether it reliably separates real crashes from clean exits. If yes, the full taxonomy split becomes a 2-commit change (add `event_type=stopped`, then bulk-rename `crashed`-with-clean-exit-heuristic rows) with strong evidence. If not, we iterate the heuristic without churning the taxonomy. |
| Known tradeoffs accepted | (a) `error_message` text stays misleading ("Process exited unexpectedly (exit code: 0)") — cosmetic; patch in a follow-up commit without schema impact. (b) Dashboards that don't know to filter on the new column see the same noise they see today — no regression. (c) Steam-launched games (F1 25, iRacing) don't write a RaceControl plugin shutdown marker, so the heuristic is time-based only for them — acceptable: if the game ran ≥30s and exit=0, it almost certainly reached the menu at least once and is more likely a clean exit than a crash. |
| Implementation surface | (a) SQL migration: `ALTER TABLE game_launch_events ADD COLUMN clean_exit_heuristic INTEGER NOT NULL DEFAULT 0;` (boolean-as-int per existing convention). (b) rc-agent `event_loop.rs:~1065` crash-event emission: compute heuristic, include in event payload. (c) Server ingestion path: persist the column (additive). (d) Cloud sync schema update: add column to sync manifest. (e) Admin dashboard crashes panel: add `WHERE clean_exit_heuristic = 0` filter to the "real crashes" view; leave the raw count visible as "including clean exits". |
| Deferred | Full taxonomy split (`event_type=stopped` vs `crashed`) — revisit 2-4 weeks after `clean_exit_heuristic` lands in production. Gate: ≥90% agreement between heuristic-true rows and human-reviewed clean-exit samples. |
| Cross-ref | Pattern E.1 resolution hinges on this column landing — once deployed, E.1 clean exits will be automatically filtered out of the "real crashes" view and the Pod 6 AC exit-0 cluster will stop looking like a bug. |

### Pattern G — Dynamic-timeout under-estimation triggers BILL-14 retry cascade

| Field | Value |
|---|---|
| State | FIX (code + tests, NOT DEPLOYED — server stays `d4b60fb5` until BILL-14 verify on Pod 1 completes). Regression test in place. |
| First seen | 2026-04-18 17:19:43 IST — Pod 4 F1 25 fired `Launch timed out (39s)` ×3 in 33 seconds, then 4th attempt finally reached Running |
| Source | `crates/racecontrol/src/metrics.rs:327` — `query_dynamic_timeout` returns `ceil((median + 2σ)/1000)` of the last ≤10 SUCCESSFUL launches matching `(sim_type, car, track)`. Previously floored at 30s. Falls back to per-sim default (90s/120s) if < 3 samples. |
| Failure mode | When historical successful F1 25 launches for a specific car/track were ~25-35s, dynamic timeout = 39s. Today's launch was slower (cold cache / EA AC update / unrelated delay), exceeded 39s, server killed it. Next retry then triggered BILL-14 retry cascade — and if `sim_type` was None on the retry, the AC adapter swap kills any running game. |
| Algorithm fragility | (a) 3 samples is too few for σ to be meaningful; (b) launch times have long-tail distribution, normal-2σ underestimates; (c) no cold-cache awareness; (d) no per-game patch-day adjustment. |
| Fix applied (2026-04-18) | `metrics.rs`: floor changed from `timeout_secs.max(30)` to `computed_secs.max(default_secs)`. Dynamic value can now only *raise* the per-sim safe default (for genuinely slow historical launches), never lower it. Telemetry log now distinguishes "raised to default" vs "above default". Tests updated: `test_dynamic_timeout_with_sufficient_history`, `test_dynamic_timeout_varied_history`, plus new regression `test_dynamic_timeout_pattern_g_floor_is_per_sim_default` (reproduces the Pod 4 scenario with 10×1000ms samples, default=120 → timeout must be 120) and `test_dynamic_timeout_can_exceed_default_when_slow` (10×150000ms, default=90 → timeout ≥150). All 6 pass. The 95th-percentile and ≥5-samples ideas are deferred — not in this patch. |
| Evidence | Today's `game_launch_events` for Pod 4 17:19:40-17:20:13 — three "Launch timed out (39s)" events 4-5 seconds apart, then `running` PID 6128 at 17:20:23. |
| Remaining gap | Fix is server-side; server on `d4b60fb5` does not yet carry it. Deploy gate: after Pattern INV-10 is runtime-verified on `d4b60fb5` (see below), combine Pattern G + any other queued server fixes into one server deploy. Do NOT deploy just for G — the Pod 4 cascade is rare (single occurrence on 2026-04-18) and server-bounce cost > avoided-recurrence cost this week. |

### Pattern INV-10 — AC Rally 3-min deterministic crash (Pod 1)
| Field | Value |
|---|---|
| State | UNVERIFIED — was CLOSED, now reset pending fresh runtime test |
| First seen | 2026-04-17 13:04 IST |
| Code fix | `5fcabd38`-bundle (`already_running` guard + `sim_type=None` advance). Source verified at `crates/racecontrol/src/billing_timer_expiry_timeout.rs:240-310`. |
| Deployed | Code is in `d4b60fb5` (verified via `git show d4b60fb5:` inspection — both guards present at lines 247, 286). Binary built from `d4b60fb5`, process started 2026-04-18 19:44 IST. |
| **Recurrence today (pre-fix-binary)** | Pod 1 AC Rally launched 2026-04-18T07:32:18Z, ran for 3 min, server log at 07:35:15Z shows `WARN Launch timeout (attempt 1) for pod pod_1 — allowing retry (attempt 2)` from `racecontrol_crate::billing_timer_expiry_timeout`, retry sent `Launching (AssettoCorsa)` swap, Rally crashed `Error (AssettoCorsaRally)` at 07:35:20Z. **All on the OLD binary** — server didn't restart with `d4b60fb5` until 14:14Z (19:44 IST). So this recurrence does NOT invalidate the fix; it just shows the fix wasn't running yet. |
| Runtime verification | **NOT YET TESTED on `d4b60fb5`.** Need a fresh non-AC launch (AC Rally / iRacing / F1 25 / LMU / AC Evo) on Pod 1 that reaches Running and stays > 3 min. If it survives the 180s retry-window without an `AssettoCorsa` adapter swap, the fix is verified. If retry still fires (or sim defaults to AC), the guards are wired wrong. **Observation-only for now** (2026-04-18 23:05 IST): Pod 1 is currently in `game_state=error` with no active billing session and no customer on site; cannot force a synthetic launch without staff PIN + billing session + customer waiver. Next natural customer launch on Pod 1 that runs >3 min will be the verification event. Monitoring query: `SELECT event_type, sim_type, created_at FROM billing_events WHERE pod_id='pod_1' AND created_at > datetime('now', '-1 day') ORDER BY created_at DESC` — looking for absence of `Launching (AssettoCorsa)` swap events during a non-AC session >3 min long. |
| Notes | Pattern was previously tracked as "Pattern B" in session handoffs. Reset because the "CLOSED" claim relied on regression-test pass alone, not on production reproduction. Closing this requires Step 4 of CLOSED-LOOP-DEBUG (re-run the EXACT failing test from Step 1). |

### Pattern I — rc-agent silent WS-reconnect-forever + diagnostic gap

| Field | Value |
|---|---|
| State | **CLOSED — 30-min soak PASS 2026-04-19 08:02 IST.** T+33 min probe: venue `/api/v1/health` build=`66fec05c` status=`ok`. `/api/v1/fleet/health`: 9/9 WS connected (Pods 1-8 on `66fec05c`, POS on `e7e01ae3`). Pods 1+6 (original silent-reconnect offenders) still `ws_connected=true` — no drift. Defense-in-depth pair `09acbbe4` (rc-agent WS Ping on heartbeat) + `90b04d71` (server clone agent_senders before .await at 12 dispatch sites) deployed 2026-04-19 07:29 IST; Pods 1+6 transitioned False → True within 25s of rc-agent swap; `fleet_connectivity` 7/9 → 9/9; status `degraded` → `ok` immediately. Part 1 (`92e699f4` ws_state.rs + `/debug/ws-state` diagnostic) not deployed in this wave — orthogonal observability improvement, can land in next rc-agent deploy. |
| Observed | 2026-04-18 23:05 IST → 2026-04-19 00:09 IST — Pod 1 + Pod 6 rc-agent HTTP alive, reconnect loop running, but server-side `ws_reconnect_count: 0` on both (all other pods showed 2) for 53 min. No alert fired. Only signal was `fleet_connectivity: 7/9 pods connected` in server `/health`. Resolved via `taskkill /F /IM rc-agent.exe` + RCWatchdog respawn; the actual connect-failure reason was never read because today's 100 MB `rc-agent-.2026-04-18.jsonl` exceeded the `/file` 50 MB cap. |
| Why it hurts | (a) No `/debug/ws-state` endpoint on rc-agent to report `current_phase`, `attempt_count`, `consecutive_failures`, `last_connect_error`, `last_successful_connect_at`. (b) Pod-local `rc-agent.jsonl` rolls only daily; can exceed the `/file` 50 MB remote-read cap during busy days. (c) The WS reconnect loop (`main.rs:2085-2174`) retries forever with 1s→30s backoff + jitter — there is no give-up signal that RCWatchdog or rc-sentry could react to. A stuck process looks identical to a healthy one. |
| Fix applied (2026-04-19, `92e699f4`) | **Part 1 landed:** new `crates/rc-agent/src/ws_state.rs` module with `Arc<RwLock<WsStateInner>>` behind a `OnceLock` (DIAG_LOG pattern). 4 update points wired into the reconnect loop: `record_attempt` before `connect_async`, `record_success` on `Ok(Ok(..))`, `record_failure(err)` on `Ok(Err(..))`, `record_failure("connect_timeout_10s")` on `Err(_)`. New `GET /debug/ws-state` authenticated handler in `remote_ops.rs`, registered on all 5 production `protected_routes` blocks. URL query-string (`?token=`/`?jwt=`) redacted before storage. Error strings capped at 500 chars. Tests: 2 tokio tests pass (`redact_strips_query_string`, `ws_state_lifecycle_behaves_correctly`). Binary built (26,727,936 bytes, +21 KB over `a6d29291`). |
| Remaining parts | **(2)** Raise `/file` 50 MB cap or add range-read support so today's 100 MB jsonl is readable remotely. **(3)** Surface `ws_state.consecutive_failures > N` in server-side `fleet/health` as a yellow flag so silent-reconnect-forever pages staff rather than requiring a manual probe. Neither is in `92e699f4`. |
| Remaining gap | Binary staged locally (`target/release/rc-agent.exe` at `2026-04-19 00:24`), **NOT deployed**. Deploy requires per-pod rc-sentry exec (taskkill + ren + RCWatchdog respawn) × 8 pods — the exact class of race that triggered the incident this endpoint is designed to observe. Deploy authorization pending. |
| Blocker | None for parts (2) and (3) — can be follow-up commits. For part (1) deploy: awaiting user go-ahead for the 8-pod atomic swap. Binary ready at `target/release/rc-agent.exe` (not yet staged into `deploy-staging/rc-agent-<hash>.exe`). |
| Cross-ref | Pattern DE-1 (dashboard WS half-socket, already fixed in `2c27e2fc`) + Pattern H (rc-agent event taxonomy). Related observability class, same subsystem. The silent-reconnect today was NOT a half-socket (no active WS existed) — it was a pre-register handshake failure loop with no diagnostic surface. |

## Resolved / deferred patterns (kept for cross-session continuity)

### Pattern C — Pod 3 iRacing Steam dialog (INV-9) — REOPENED 2026-04-18

| Field | Value |
|---|---|
| State | OPEN — recurrence on the fixed binary |
| Code fix | `49bcd69b` removes the dismiss-once flag in `steam_checks.rs`; retries every 2s poll cycle. |
| Deployed | `49bcd69b` IS in `d4b60fb5` (verified). |
| **Recurrence on fixed binary** | Pod 3 iRacing 2026-04-18 20:17:31 IST (UTC 14:47:31) — `Game window not detected: Game failed to launch - only Steam dialog visible after 60s timeout`. Server was on `d4b60fb5` since 19:44 IST (2:14 hours before). |
| Implication | The dismiss-once removal is INSUFFICIENT. Either (a) Steam introduced a new dialog form not matched by the dismiss code, (b) Pod 3 has stuck Steam state (login expired, update pending), (c) the 60s window-detect deadline elapses before the dismiss loop catches the dialog. |
| Next action | INV-9 v2 diag binary `a6d29291` deployed fleet-wide 2026-04-18 23:03-23:05 IST (verified all 8 pods via `/health`). Next Pod 3 iRacing launch timeout will now log the observed top-level window classes both to the `rc-agent` tracing WARN and into the returned error string (so it lands in `game_launch_events.error_message`). Passive wait: no action needed until next failure. When it fires, query `SELECT error_message FROM game_launch_events WHERE pod_id='pod_3' AND sim_type='iRacing' ORDER BY created_at DESC LIMIT 1` and look for `Observed window classes: [...]`. Then add the real blocking class to `dismiss_steam_dialogs()` (after confirming WM_CLOSE on it won't close Edge kiosk). |

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
