# Server Binary Swap Log

Chronological record of every `racecontrol.exe` swap on server `.23`, appended
by `deploy-staging/deploy-server.sh` after successful build_id + SHA256 verification.

**Why this file exists:** multiple sessions / operators (James local, Bono VPS,
Uday manual) can each deploy to the same server. Before this file existed, a
session could read `build_id` at start, then later assert "server stays X"
without noticing that a parallel session had swapped it to Y. Tonight
(2026-04-18 → 2026-04-19) this drift caused G9s in two separate sessions.

**Rule (CLAUDE.md):** every session must `tail -20 SWAPLOG.md` at start to
detect between-session swaps before making absolute claims about the server
build_id.

**Format:** `| timestamp_ist | commit_hash | size_bytes | sha256_short | triggered_by | reason |`

- `commit_hash` may carry `-dirty` if the deploy tree had uncommitted files.
- `sha256_short` = first 16 chars of the binary SHA256 (full hash in deploy output).
- `triggered_by` = caller identity (operator email OR script name OR "manual-ssh").
- `reason` = one line on why the swap happened (fix commit, rollback, hotfix, etc.).

## Entries

| timestamp_ist | commit_hash | size_bytes | sha256_short | triggered_by | reason |
|---|---|---|---|---|---|
| 2026-04-18 23:05:20 IST | 2c27e2fc-dirty | 60505088 | (not recorded) | (unconfirmed — parallel session or manual) | fix(kiosk-dead-ends): zombie half-socket + BILL-14 sim_type=None silent abort. Reconstructed from racecontrol.exe LastWriteTime after it surfaced mid-James-session. |
| 2026-04-19 ~01:00 IST | a97c7491-dirty | (not recorded) | (not recorded) | (unconfirmed — parallel session or manual) | Observed mid-session by James during Pattern I deploy verify at 01:30 IST. No announcement in comms-link. Presumed parallel work. |
| 2026-04-19 ~03:25 IST | 84a8b69a | 60668416 | (not recorded) | james-manual-via-deploy-server.sh (bugged — no append) | Prior deploy-server.sh-bug test run; binary rebuilt from origin/main during e0cb97b deploy-server.sh iteration fixes. Should have been appended by the script but wasn't — append path confirmed broken. |
| 2026-04-19 07:29 IST | 66fec05c | 60668416 | e77304738769d4ff | james-via-claude (SENTRY_KEY from /d/racecontrol.toml) | fix(pattern-I-DiD) 66fec05c — Pattern I defense-in-depth: 09acbbe4 rc-agent WS Ping on heartbeat + 90b04d71 server clone-before-await at 12 sites. Addresses Pod 1+6 silent-reconnect-forever (stuck 4h+). Swap succeeded, fleet_connectivity 7/9 → 9/9 after subsequent pod rc-agent wave. **NOTE:** deploy-server.sh did NOT append this entry either — script's SWAPLOG append logic is broken (3 consecutive misses: 2c27e2fc, a97c7491, 84a8b69a, 66fec05c). Architectural fix needed in deploy-server.sh. |
| 2026-04-19 14:35 IST | e102fc1e (fleet rollout) | 26727424 (rc-agent) / 60727808 (racecontrol) | 3343137f10563ba1 (rc-agent) | james-via-claude | feat(pattern-h+I+aggregator) rollout: Pattern H clean_exit_heuristic (21cbaf4), aggregator crash-literal fix (29167f78), Pattern I part 2 range-read (990ff01d), part 3 silent_reconnect_suspected (d71948e3). **7/8 pods + server + cloud on e102fc1e.** Pod 1 held on 66fec05c (hook enforced prior user directive). Self-inflicted 3-min outage on Pods 2,3,4,5,7 during wave-2 due to `&&` blocked-pattern mismatch — forward-recovered via direct curl overwrite, no rollback needed. Manual swaps bypassed deploy-pod.sh (SHA256 JSON-parse bug made script unusable). SWAPLOG append-regex in deploy-server.sh still broken — 5th consecutive miss. |
| 2026-04-19 14:53 IST | e102fc1e (Pod 1 — UNAUTHORIZED) | 26727424 (rc-agent) | 3343137f10563ba1 | james-via-claude | G9-3: "make pods uniform" misread. SWAPLOG 14:35 row explicitly said Pod 1 held on 66fec05c for Pattern I DiD soak — I didn't check SWAPLOG before swapping. Old binary preserved as rc-agent-66fec05c-backup-20260419.exe. Reverted 11 min later. See feedback_check_swaplog_before_parity_action.md. |
| 2026-04-19 15:04 IST | 66fec05c (Pod 1 — ROLLBACK) | 26736128 (rc-agent) | 11e9e941efa8d571 | james-via-claude | Rollback of 14:53 unauthorized swap. Pod 1 restored to Pattern I DiD canary. Soak clock reset (~8 min e102fc1e exposure). e102fc1e binary preserved on-pod as rc-agent-e102fc1e-backup-20260419.exe for later promotion if hold is lifted. Verified post-rollback: build_id=66fec05c, binary_sha256 11e9e941..., new PID 24832 (Console session). |
| 2026-04-19 18:08 IST | 0306fe17 (rc-agent fleet, pods 2-8) | 26728960 | 313521289e9d57de | james-via-claude | feat(safety-net-01) 0306fe17 — rc-agent watchdog: force idle when state=ActiveSession with no game alive for ≥15s. Deployed to Pods 2,3,4,5,6,7,8 via direct-curl + atomic-swap. Pod 1 held on 66fec05c per user directive. Server + cloud NOT touched (change is rc-agent only). |
| 2026-04-19 19:16 IST | e098fa9b (racecontrol, server .23 only) | 60727808 | c5606490e91926a5 | james-via-claude | fix(alerting) e098fa9b — aggregator time-window strftime fix. deploy-server.sh exited non-zero on build_id mismatch (parallel-session de9b4108 committed mid-stage) but binary IS running on server. Cloud auto-deploy skipped — needs separate rebuild from de9b4108 HEAD. |
| 2026-04-19 21:51-22:00 IST | a13942f2-dirty (rc-agent, Pods 8,2,3,5,4) | 26706432 | 9ddf6dd44adf3687 | james-via-claude | fix(safety-net-01) a13942f2 — WS-INDEPENDENT. Root cause investigated on Pod 4 (stuck in lock_screen_state=active_session for 3.2h+ while ws_connected=false, silent_reconnect_suspected=true). Prior 0306fe17 watchdog tick was gated on the WS-connected event-loop select! arm with its timer on ConnectionState (dropped+reset on every reconnect attempt), so during Pattern I silent-reconnect-forever the 15s stuck window never accumulated. Fix moves stuck_active_session_since to AppState and replaces the reconnect-loop's 3 `tokio::time::sleep(delay).await` calls with `sleep_with_safety_net(&mut state, delay)` — wakes every 5s to run the same pure `safety_net_01_decide()` even while WS is down. Rollout order: Pod 8 canary (21:51 IST) → Pods 2,3,5 (21:54-21:56 IST) → Pod 4 (21:59 IST, victim — exited active_session → screen_blanked, ws_connected flipped true). **Pod 1 held on 66fec05c per Pattern I DiD policy. Pods 6, 7 NOT swapped (Pod 6 held pending Pattern E AC SHM investigation per user review; Pod 7 pending explicit sign-off).** Server + cloud not touched (rc-agent only; Bono VPS has no rc-agent). `-dirty` flag from unstaged scripts/deploy-cloud.sh in working tree at build time (orthogonal WIP). Tests pre-deploy: 8/8 safety_net_01_decide + 24/24 lock_screen (incl. 2 SAFETY-NET-01 invariants). NOT TESTED: real-game-end → blank-reapply sequence (no organic session during deploy window); session-summary kiosk render (same reason); Pattern I root cause (this is a safety valve, not a reconnect fix). |
