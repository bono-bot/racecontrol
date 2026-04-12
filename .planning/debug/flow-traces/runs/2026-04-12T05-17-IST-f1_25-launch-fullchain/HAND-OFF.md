# F1 25 Launch Full-Chain — HAND-OFF

**Session:** 2026-04-12 ~02:00-12:30 IST (James, Opus 4.6)
**Status:** 5 of 6 layers fixed. 1 remaining: UDP packet delivery.
**Pod 4 current state:** build `131ac6f9`, game NOT running, lock screen blanked.
**Fleet (pods 1-3, 5-8):** still on baseline `b1fc9484`, unaffected.
**Local git commits (ALL UNPUSHED):**
```
131ac6f9 fix(rc-agent): ADAPTER-SWAP-04 — gate AC 60s launch timeout to AC-only
106df13d fix(rc-agent): ADAPTER-SWAP-03 — SO_REUSEADDR for F125Adapter port sharing
5766afcc fix(rc-agent): ADAPTER-SWAP-02b — throttle adapter connect WARN by error change
8300c19f fix(rc-agent): ADAPTER-SWAP-02 — unblock F125Adapter UDP bind + log connect errors
96940ad0 fix(rc-agent): ADAPTER-SWAP-01 — per-launch sim adapter rebuild (fixes 5 sims)
ee9d9f0b fix(rc-agent): protect fleet games from minimize_background_windows (Bug B)
2a80641a docs(debug): F1 25 staff-launch flow trace + Bug 2 root cause + Bug B finding
29a4d8f1 fix(rc-agent): split UdpReachable from UdpActive for F1 25 launch verification
```

---

## The problem (one sentence)

Staff fires F1 25 on Pod 4 from kiosk staff page → game launches visibly → game is killed at exactly T+180s by server's `check_launch_timeouts` retry because rc-agent never sends `AcStatus::Live` to the server.

## Root cause chain (6 layers, 5 fixed, 1 remaining)

### Layer 1 — No F125Adapter constructed (FIXED: 96940ad0)
- `main.rs` constructed a single adapter at startup from `config.pod.sim`
- All 8 pods have `sim = "assetto_corsa"` → only AC adapter ever existed
- **Fix:** `sims::build_sim_adapter()` factory + per-launch adapter rebuild in `ws_handler.rs` LaunchGame handler
- **Evidence:** Log shows `ADAPTER-SWAP: built fresh F125 adapter for launch` on every attempt after this commit

### Layer 2 — run_udp_monitor held port 20777 (FIXED: 8300c19f)
- `main.rs` run_udp_monitor bound ALL telemetry ports at startup including 20777
- F125Adapter::connect() plain bind failed with "address in use"
- Error was silently swallowed by `if adapter.connect().is_ok()` (no else branch)
- **Fix:** Filter port 20777 out of run_udp_monitor port list. Log WARN on connect errors (with change-detection throttle added in 5766afcc to avoid AC SHM spam).

### Layer 3 — ConspitLink2.0 holds port 20777 (FIXED: 106df13d)
- ConspitLink2.0 (wheelbase FFB driver, PID 21392) binds `127.0.0.1:20777` to receive F1 25 UDP telemetry for force feedback
- Even after run_udp_monitor filter, F125Adapter's plain bind failed
- **Fix:** F125Adapter::connect() now uses `socket2::Socket` with `set_reuse_address(true)` (SO_REUSEADDR)
- **Evidence:** Netstat shows dual bind after fix:
  ```
  UDP    0.0.0.0:20777     PID 25528  (rc-agent F125Adapter)
  UDP    127.0.0.1:20777   PID 21392  (ConspitLink2.0)
  ```

### Layer 4 — AC 60s launch timeout fires for all sims (FIXED: 131ac6f9)
- `event_loop.rs` line 780: `AC_LAUNCH_TIMEOUT_SECS = 60` applied to ALL sims, not just AC
- F1 25 was killed at T+61s before F125-specific PlayableSignal path could fire
- **Fix:** Wrapped lines 729-846 with `if matches!(conn.current_sim_type, Some(SimType::AssettoCorsa) | None)`

### Layer 5 — UdpReachable signal split (FIXED: 29a4d8f1, was dead code until layers 1-4 landed)
- F125Adapter emits `DetectorSignal::UdpReachable` on any valid F1 25 packet
- event_loop signal handler sets `conn.f1_udp_playable_received = true`
- PlayableSignal dispatch checks this flag → emits `AcStatus::Live` → satisfies server launch verifier
- **Status:** Code is correct but UNTESTED at runtime because layer 6 blocks packets

### Layer 6 — Windows UDP socket specificity routing (NOT FIXED)
- F1 25 sends UDP to `127.0.0.1:20777` (confirmed via hardware_settings_config.xml)
- ConspitLink binds `127.0.0.1:20777` (specific address match)
- F125Adapter binds `0.0.0.0:20777` (wildcard)
- Windows SO_REUSEADDR with different bind specificity: the **more specific** socket (`127.0.0.1`) gets exclusive delivery; the wildcard (`0.0.0.0`) receives nothing
- F125Adapter receives zero packets → UdpReachable never fires → f1_udp_playable_received stays false → AcStatus::Live never sent → server's 180s check_launch_timeouts fires → retry LaunchGame → pre_launch_checks kills F1_25.exe

**Proposed fix for layer 6:** Change F125Adapter bind address from `0.0.0.0:20777` to `127.0.0.1:20777`. With identical bind address + SO_REUSEADDR, Windows should deliver packets to both sockets. One-line change in `sims/f1_25.rs` line ~488 (the `"0.0.0.0:20777"` string).

**Confidence in layer 6 fix: ~80%.** Windows SO_REUSEADDR with identical addresses may still route to only one socket (round-robin or newest-wins). If that happens, alternative approach: don't fight ConspitLink for the port at all — instead, have F1 25 send to TWO ports (one for ConspitLink, one for rc-agent) by changing `hardware_settings_config.xml` port, or by using a local UDP proxy that receives on one port and forwards to both consumers.

---

## Bug B — minimize_background_windows (FIXED in code, UNVERIFIED at runtime)

**Commit:** `ee9d9f0b`
- Added F1_25, iRacingSim64DX11, LMU, AssettoCorsaEVO, acr, ForzaMotorsport, ForzaHorizon5 to the PowerShell `$allowList`
- Log evidence from attempt with this fix: zero `Minimized: F1_25` entries
- Cannot CLD-close until F1 25 survives long enough to encounter the periodic minimize tick

## Bug C — F1 25 actual crash (UNRESOLVED, may not exist)

- Earlier sessions reported "F1 25 crashed at ~3 min with 7GB RAM, exit code 1"
- All 3-minute crashes in this session were the 180s server timeout kill, not a real game crash
- Bug C is likely the same bug, not a separate issue
- Will know for sure once layer 6 is fixed and a launch survives past 180s

## Problem B — Kiosk staff page shows no launch state (NOT STARTED)

- Kiosk staff page shows "ready" permanently — no Launching/Loading/Running state
- No timer, no game name, no way to stop a running game
- This is a kiosk frontend code change (WS event subscription + UI state machine)
- Independent of the rc-agent launch chain fixes
- Tracked in parent GSD session as Problem B

---

## What was tested per attempt

| Attempt | Build | Launch→Death | Kill cause | Layer verified |
|---------|-------|-------------|------------|----------------|
| 01 | b1fc9484 | ~180s | Server 180s timeout (no adapter) | Baseline symptom captured |
| 02 | 29a4d8f1 | ~180s | Same (UdpReachable was dead code) | Layer 5 code correct but inactive |
| 03 | ee9d9f0b | ~180s | Same (allowlist worked but irrelevant) | Bug B allowlist works |
| 04 | 96940ad0 | ~180s | Same (adapter built but port 20777 blocked by run_udp_monitor) | Layer 1 ADAPTER-SWAP fires |
| 05 | 8300c19f | ~180s | Same (port filter correct but ConspitLink holds port) | Layer 2 port filter works |
| 06 | 5766afcc | n/a | (throttle-only fix, not retested) | Layer 2b throttle works (1 WARN not 200) |
| 07 | 106df13d | ~65s | AC 60s timeout (SO_REUSEADDR bound but AC timeout fires first) | Layer 3 SO_REUSEADDR dual-bind confirmed via netstat |
| 08 | 131ac6f9 | ~180s | Server 180s timeout (0.0.0.0 vs 127.0.0.1 specificity) | Layer 4 AC timeout gated. Layer 6 identified. |

## G9s this session: 2

1. Reactive patching instead of structured PoE (user corrected)
2. False "Bug 2 verified" on attempt 02 — used ready_delay_ms as proxy for launch verifier satisfaction

## Files touched (source code, all committed locally)

- `crates/rc-agent/src/sims/mod.rs` — `build_sim_adapter()` factory
- `crates/rc-agent/src/sims/f1_25.rs` — SO_REUSEADDR bind, UdpReachable emission
- `crates/rc-agent/src/app_state.rs` — `signal_tx` field for adapter rebuild
- `crates/rc-agent/src/main.rs` — factory call, port filter, signal_tx in AppState literal
- `crates/rc-agent/src/event_loop.rs` — AC timeout gate, connect error logging with throttle, UdpReachable handler
- `crates/rc-agent/src/ws_handler.rs` — ADAPTER-SWAP block in LaunchGame handler
- `crates/rc-agent/src/ac_launcher.rs` — Bug B allowlist extension
- `crates/rc-agent/src/driving_detector.rs` — UdpReachable variant + match arm

## P0 secret leak (tracked, not fixed)

All 8 pod rc-agent.toml files contain hardcoded `openrouter_api_key = "sk-or-v1-b762be6e..."`. Delegated to Bono for rotation. Pod-tomls observation files in this trace folder have been redacted.

## Next session: pick up from here

1. Apply layer 6 fix: change `"0.0.0.0:20777"` to `"127.0.0.1:20777"` in f1_25.rs
2. Rebuild + deploy to Pod 4 + retrace
3. If packets still don't arrive: alternative approach — change F1 25 to send to a different port (e.g., 20778) via hardware_settings_config.xml, bind F125Adapter to that port exclusively, let ConspitLink keep 20777
4. Once F1 25 survives past 180s with AcStatus::Live confirmed: CLD-close Bug 2
5. Then: fleet sweep pods 1-8, push commits, LOGBOOK, Bono notification
6. Then: Problem B (kiosk UI)
