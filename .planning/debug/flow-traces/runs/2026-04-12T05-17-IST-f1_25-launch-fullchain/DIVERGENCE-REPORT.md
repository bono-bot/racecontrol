# F1 25 Launch Full-Chain — DIVERGENCE REPORT

**Trace:** `runs/2026-04-12T05-17-IST-f1_25-launch-fullchain/`
**Method:** Flow-Trace Debugging v2 (layer-by-layer DESCEND, hypotheses enumerated from environment)
**Status:** ROOT CAUSE CONFIRMED, FIX NOT YET APPLIED
**Predecessors (superseded):**
- `runs/2026-04-12T03-15-59-IST-f1_25-attempt-01/` — misdiagnosed root cause as speed>0 gate
- `runs/2026-04-12T04-13-17-IST-f1_25-attempt-02-canary/` — claimed "Bug 2 verified" based on ready_delay_ms proxy (now known to be unrelated to launch verifier)

---

## Symptom (OPEN — user-facing)

Staff fires F1 25 launch on Pod 4 from `http://192.168.31.23:3300/kiosk/staff` (any tier, any game mode).
Observed behavior (attempt 03, canary `ee9d9f0b` with Bug 2 "fix" + Bug B allowlist fix):

- Kiosk staff page Pod 4 tile shows `ready` — never transitions to Launching/Loading/Running (Problem B, separate loop)
- F1 25 visibly launches on Pod 4 display — EA SPORTS splash + audio (user confirmed)
- Game runs ~180 seconds (user estimated "30s"), then disappears
- rc-agent log shows `Non-AC game F125 exited (code: 1) — treating as crash (GAME-08)` at T+178s
- Server-side billing: "Game exited with no active billing"
- Repeats deterministically on every kiosk click

## Loop-closure criteria (written before fix — CLD Step 4 target)

A single kiosk staff launch of F1 25 on any pod must achieve **all** of:

1. Pod display: F1 25 visible continuously through the full session duration
2. rc-agent: `launch_state = Live`, `AcStatus::Live` WS message sent to server
3. Server: `waiting_for_game` entry cleared (no 180s retry fires)
4. Billing: session record created (pre-committed BILL-13 path activated), timer ticks
5. Kiosk staff page: Pod tile reflects launch progress (Problem B, tracked separately — do not gate fix on this)
6. Game exits gracefully when session ends

## Root cause chain (walked layer-by-layer, evidence per step)

```
Staff clicks launch on kiosk/staff
    ↓  WS: server → rc-agent LaunchGame
rc-agent ws_handler processes LaunchGame
    ↓  log: "Pre-launch checks passed for F125" [23:18:38]
    ↓  log: "Launching game: F125 (args...)"    [23:18:38]
    ↓  log: "GAME-07: Steam URL launch for F125 — waiting for game window" [23:18:41]
rc-agent detects F1_25.exe process via find_game_pid()
    ↓  log: "GAME-08: ProcessMonitor created (Steam-launch) for F125 (pid 20484)" [23:18:59]
    ↓  event_loop.rs:901 sets game.state = GameState::Running
    ↓  event_loop.rs:922 emits GameState::Loading to server (first)
    ↓  event_loop.rs:943 emits GameState::Running to server (immediately after)
Server game_launcher.rs:1187-1196 records ready_delay_ms on first Running
    ↓  server log: "Phase 282: pod pod_4 ready_delay_ms=21165"  [attempt 02 only — server-side only]
F1 25 game emits UDP telemetry at 127.0.0.1:20777 (hardware_settings_config.xml confirmed)
    ↓  udp enabled="true" ip="127.0.0.1" port="20777" sendRate="30" format="2025"
    ↓  Packets ARE sent by F1 25 — verified by game config, NOT by packet capture
rc-agent F125Adapter.read_telemetry() would process these packets and emit UdpReachable
    ↓  **XXX RUPTURE XXX** F125Adapter does not exist on any pod in the fleet
    ↓  Evidence: main.rs:963-975 constructs adapter based on config.pod.sim ONCE at startup
    ↓  Evidence: all 8 pod rc-agent.toml files have sim = "assetto_corsa" (fleet-wide read, see observations/pod-tomls/)
    ↓  Evidence: NO state.adapter = ... reassignment anywhere in rc-agent source (grep confirmed)
    ↓  Evidence: git log of main.rs shows NO prior per-launch adapter logic that was removed
    ↓  Consequence: UDP port 20777 has no listener → F1 25 packets discarded at OS level
    ↓  Consequence: UdpReachable never fires → f1_udp_playable_received stays false
    ↓  Consequence: event_loop.rs:1128 condition never met → launch_state stays WaitingForLive
    ↓  Consequence: AcStatus::Live never sent to server
    ↓  Consequence: Server waiting_for_game entry never cleared
Server billing.rs:732 check_launch_timeouts fires at T+180s (config default_launch_timeout_per_attempt=180)
    ↓  billing.rs:2436-2479: attempt 1 timeout → sends LaunchGame WS to rc-agent (retry)
    ↓  rc-agent ws_handler receives second LaunchGame → runs pre_launch_checks
    ↓  pre_launch_checks finds F1_25.exe (PID 20484) still alive
    ↓  log: "ERROR Pre-launch check FAILED: orphan game process F1_25.exe (PID 20484) persists after cleanup" [23:21:39.411]
    ↓  rc-agent cleanup path kills F1 25 → exit code 1
    ↓  log: "Non-AC game F125 exited (code: 1) — treating as crash (GAME-08)" [23:21:45]
User sees F1 25 disappear from Pod display
```

**The rupture is at the "F125Adapter does not exist" step.** Everything downstream is consequence.

## Evidence — per hypothesis

### H-A: ALL pods sim-locked to AC at startup — CONFIRMED FLEET-WIDE

Read all 8 pod `rc-agent.toml` files via `/exec` to port 8090. Results (see `observations/pod-tomls/pod{1-8}.json`):

| Pod | IP | `[pod].sim` | Reachable |
|-----|-----|-------------|-----------|
| 1 | 192.168.31.89 | `"assetto_corsa"` | ✓ |
| 2 | 192.168.31.33 | `"assetto_corsa"` | ✓ |
| 3 | 192.168.31.28 | `"assetto_corsa"` | ✓ |
| 4 | 192.168.31.88 | `"assetto_corsa"` | ✓ |
| 5 | 192.168.31.86 | `"assetto_corsa"` | ✓ |
| 6 | 192.168.31.87 | `"assetto_corsa"` | ✓ |
| 7 | 192.168.31.38 | `"assetto_corsa"` | ✓ |
| 8 | 192.168.31.91 | `"assetto_corsa"` | ✓ |

Every pod in the fleet has only an AC adapter at startup. F1 25, iRacing, LMU, ACE, ACR, FH5 all suffer the same launch failure chain because none of their UDP/SHM adapters are constructed. Only Assetto Corsa works end-to-end.

**Also observed (P0 secret leak — fleet-wide):** All 8 pod TOMLs contain the same hardcoded `openrouter_api_key = "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"`. Tracked separately, delegated to Bono for rotation.

### H-B: Per-launch adapter swap DOES NOT EXIST — CONFIRMED

Grep of `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src` for adapter reassignment patterns:

- `state.adapter: Option<Box<dyn SimAdapter>>` declared in [app_state.rs:41](../../../../../crates/rc-agent/src/app_state.rs#L41)
- Only constructor: [main.rs:963-995](../../../../../crates/rc-agent/src/main.rs#L963)
- Zero occurrences of `state.adapter =` reassignment
- Zero occurrences of `adapter.replace()`
- Zero occurrences of `rebuild_adapter` / `swap_adapter` / `new_sim_adapter` / `build_sim_adapter`

The adapter is constructed once at rc-agent boot based on `config.pod.sim` and never replaced for the lifetime of the process.

### H-C/D: F1 25 UDP telemetry IS enabled and correct — CONFIRMED

File: `C:\Users\User\Documents\My Games\F1 25\hardwaresettings\hardware_settings_config.xml` (last written 2026-04-12 04:50 IST, during attempt 03 runtime — F1 25 was alive and modifying its own config)

```xml
<motion>
    <dbox enabled="true" />
    <udp enabled="true" broadcast="false" ip="127.0.0.1" port="20777" sendRate="30" format="2025" yourTelemetry="restricted" onlineNames="off" />
</motion>
```

All fields correct for F125Adapter:
- `enabled="true"` — UDP telemetry ON
- `ip="127.0.0.1"` — loopback, no firewall issue
- `port="20777"` — matches `telemetry_ports.f1` in pod TOML and hardcoded F125Adapter port
- `format="2025"` — matches `f1_25.rs:138` packet_format check
- `sendRate="30"` — 30Hz continuous emission (menu + session)

F1 25 is blameless. The packets leave the game correctly; nothing on the receiving end is listening.

### H-E: No prior per-launch adapter logic in git history — CONFIRMED

`git log --oneline -- crates/rc-agent/src/main.rs` (last 30 commits): no commit adds or removes dynamic-adapter behavior. Adapter-related commits:

- `c727b709 feat(110-02): gate F1 25 UDP socket to Running state` — added HARD-04 gate but assumes F125Adapter exists
- `79ff2b4a feat(86-01): wire EVO adapter into main.rs sim type matching and creation` — added EVO to the same single-sim-at-startup match
- `e7452743 feat(86): add AC Rally adapter variant + wire EVO/Rally into main.rs` — same pattern

No regression. Single-sim-at-startup has been the architecture since the beginning of the codebase.

### H-F: Server Phase 282 `ready_delay_ms` is unrelated to launch verifier — CONFIRMED

Server `ready_delay_ms` is set at [game_launcher.rs:1192](../../../../../crates/racecontrol/src/game_launcher.rs#L1192) in the `update_game_state` handler. Condition: `info.game_state == GameState::Running && tracker.playable_at.is_none()`.

`GameState::Running` is sent by rc-agent from [event_loop.rs:898-944](../../../../../crates/rc-agent/src/event_loop.rs#L898) when `find_game_pid` discovers the process in the OS task list — this is **pure process detection**, not launch-verifier satisfaction.

So `ready_delay_ms=21165` from attempt 02 means "F1_25.exe appeared in tasklist 21s after launch request." It does NOT mean "the launch verifier saw telemetry." Previous session's claim that attempt 02 was "verified at launch verifier layer" was based on proxy evidence (H3 violation). **G9 recorded.**

Server's actual launch verifier is `check_launch_timeouts` at [billing.rs:732](../../../../../crates/racecontrol/src/billing.rs#L732), which waits for `AcStatus::Live` WS message, not `GameState::Running`. AcStatus::Live is emitted by rc-agent at [event_loop.rs:1131-1144](../../../../../crates/rc-agent/src/event_loop.rs#L1131), gated on `f1_udp_playable_received`. Which requires UdpReachable. Which requires F125Adapter. Which does not exist.

## Why the previous "Bug 2 fix" (commit 29a4d8f1) was dead code

Commit `29a4d8f1 fix(rc-agent): split UdpReachable from UdpActive for F1 25 launch verification` made three edits:

1. Added `DetectorSignal::UdpReachable` variant + match arm + 3 tests in [driving_detector.rs](../../../../../crates/rc-agent/src/driving_detector.rs)
2. Added UdpReachable emission in [f1_25.rs:497-522](../../../../../crates/rc-agent/src/sims/f1_25.rs#L497) — fires after any valid F1 25 packet arrives via `read_telemetry()`
3. Added UdpReachable signal handler in [event_loop.rs:795-801](../../../../../crates/rc-agent/src/event_loop.rs#L795) — sets `conn.f1_udp_playable_received = true`

**All three edits are on a code path that is never executed on any pod in the fleet.**

- F125Adapter::read_telemetry() is only called from the telemetry_interval tick handler
- The telemetry_interval handler reads `state.adapter`, which on all 8 pods is an `AssettoCorsaAdapter`, not an `F125Adapter`
- The F125Adapter struct exists in source but is never instantiated in any running binary
- Therefore f1_25.rs:520 `tx.try_send(UdpReachable)` never fires
- Therefore driving_detector.rs match arm is never reached
- Therefore event_loop.rs:799 `conn.f1_udp_playable_received = true` is never set
- Therefore event_loop.rs:1128 `if conn.f1_udp_playable_received && ...` is always false for F125 on any pod

The canary binary (`ee9d9f0b`) currently on Pod 4 contains this dead code along with the Bug B allowlist fix. It is harmless but ineffective. Bug 2 is still open, its root cause was never correctly identified until now.

## Why the previous "Bug B fix" (commit ee9d9f0b) status is UNDETERMINED

The allowlist fix in `ee9d9f0b` protects F1_25 (and 9 other game processes) from being minimized by `minimize_background_windows()`. In attempt 03, the log showed zero `Minimized: F1_25` entries during the 180s F1 25 lifetime.

But:
- Without a working launch verifier, every F1 25 launch dies at T+180s regardless of minimize behavior
- Attempt 02's original Bug B finding (`Minimized: F1_25 (PID 20308)` at 23s into launch) cannot be re-reproduced until Bug 2 is actually fixed — the game won't survive long enough to retry the close-loop without pre-existing kill path
- The fix is "correct" at the layer it targets, but cannot be CLD-closed in isolation from Bug 2

Bug B fix remains committed but unverified. It does not cause harm if Bug 2 is also fixed.

## Refined root cause (one sentence)

**rc-agent is sim-locked at startup (single `Box<dyn SimAdapter>` constructed once from `config.pod.sim`), and all 8 pods have `sim = "assetto_corsa"`, so no non-AC sim's telemetry adapter ever exists and no non-AC launch can ever be verified by the server's 180s launch_timeout → every non-AC launch is killed via server retry after 180 seconds.**

## What this means for the other non-AC sims

Same bug affects: F1 25 (confirmed), iRacing (same pattern — IsOnTrack SHM signal never read), LMU (same), ACE (same), ACR (same), FH5 (same). Only Assetto Corsa works end-to-end on the current fleet because it is the only sim whose adapter is instantiated.

Pods that have these games installed (per TOMLs): F1 25 on all 8, ACE on 6 pods, ACR on 4 pods, LMU on 4 pods, iRacing on 8 pods. **The fleet has 5 sims effectively broken for customers.** The bug is not F1 25-specific.

## Fix options (for user decision — not yet applied)

### Option A — Multi-adapter at startup, multiplex in event_loop
Build ALL sim adapters at rc-agent boot (AC, F125, iRacing, LMU, ACE, ACR, Forza). Store in a struct like `adapters: HashMap<SimType, Box<dyn SimAdapter>>`. In telemetry_interval tick, read from the adapter whose `sim_type()` matches the currently-launched game's sim_type (tracked via `conn.current_sim_type` or `state.game_process.sim_type`).

- **Pros:** One-time architectural fix. Works for all 5 broken sims at once. Future sims just need an entry in the construction match. Respects HARD-04 (UDP sockets bind lazily via existing `game_running` gate). Fleet stays on one code path.
- **Cons:** Touches main.rs adapter construction + event_loop.rs telemetry_interval read site + app_state.rs struct. ~40-80 lines changed. Memory overhead of unused adapters (small — they're just structs until connect() is called).
- **Reversibility:** Single commit, revertable. Previous binary in `rc-agent-prev.exe` on Pod 4 gives instant rollback.
- **Testability:** Existing ac_launcher tests + rc-common tests should pass unchanged. Need new unit test for "multiplex selects correct adapter by sim_type".
- **Blast radius:** Per-pod binary swap. AC path untouched behaviorally (same adapter, same code path).

### Option B — Per-launch adapter replace in ws_handler LaunchGame
On `LaunchGame { sim_type }` WS message, tear down current `state.adapter` and construct a new one matching the launched sim. On game exit, restore default from config.

- **Pros:** Smaller code footprint than A.
- **Cons:** Fights rc-agent's `state` ownership model — telemetry_interval holds `&mut state.adapter` during reads; reassigning from ws_handler requires extra locking. Risk of race between game launch and first telemetry tick. Creates a window where `state.adapter` is None, which complicates error paths.
- **Reversibility:** Harder to reason about — behaves differently during concurrent launches.
- **Testability:** Requires integration tests for adapter lifecycle transitions.

### Option C — Always construct F125 adapter alongside AC
Modify main.rs:967-975 to always construct BOTH AssettoCorsaAdapter and F125Adapter as a tuple or pair; hand-pick based on active game at read time.

- **Pros:** Minimal change.
- **Cons:** Only fixes F125 — iRacing, LMU, ACE, ACR still broken. Kicks the same can down the road, just one sim wider. Rejected.

### Option D — Config-time fix: change pod TOMLs to sim = "multi" or per-launch sim
Add a new config mode where rc-agent reads a list of sims from TOML and constructs all of them at boot.

- **Pros:** Cleanly separates configuration from code.
- **Cons:** Requires TOML schema change + deploy all 8 pods' TOMLs. Still needs multi-adapter support in rc-agent (same as Option A). Strictly a superset of Option A's work. Deferred.

### Recommendation

**Option A.** It is the minimum architectural change that fully closes the loop for all 5 broken sims simultaneously, without changing the public API of adapter construction or the TOML schema. HARD-04 gating already exists at [event_loop.rs:358-366](../../../../../crates/rc-agent/src/event_loop.rs#L358) and will continue to work because it inspects `adapter.sim_type()` on whichever adapter is currently selected.

## CLD step 4 — what the CLOSE test must be

Same staff click that opened the loop:

1. Open `http://192.168.31.23:3300/kiosk/staff` in a real browser (not curl)
2. Select Pod 4 + F1 25 + any tier, click Launch
3. Watch Pod 4 display via go2rtc snapshot at T+10s, T+60s, T+120s, T+200s, T+full-duration
4. Tail rc-agent log for the exact sequence:
   - `GAME-07: Game window confirmed for F125`
   - `F1 25 PlayableSignal (UdpActive) — emitting AcStatus::Live` (NEW — must appear)
   - `Splash dismissed — F1 25 is Live`
   - NO `Pre-launch check FAILED: orphan game process F1_25.exe`
   - NO `Non-AC game F125 exited (code: 1)` before session timer expires
5. Verify server-side: fleet/health for Pod 4 shows `game_state: Running` throughout, no restart of `waiting_for_game` entry at T+180s
6. Repeat on at least one more pod (Pod 1 or Pod 8 canary) before fleet rollout

## CLD step 5 — sweep targets

Per H4 enumeration, fleet targets needing the Option A binary + verification:

- Pod 1 (.89) — currently `b1fc9484` + sim=ac
- Pod 2 (.33) — currently `b1fc9484` + sim=ac
- Pod 3 (.28) — currently `b1fc9484` + sim=ac
- Pod 4 (.88) — currently `ee9d9f0b` canary + sim=ac (swap to new binary)
- Pod 5 (.86) — currently `b1fc9484` + sim=ac
- Pod 6 (.87) — currently `b1fc9484` + sim=ac
- Pod 7 (.38) — currently `b1fc9484` + sim=ac
- Pod 8 (.91) — currently `b1fc9484` + sim=ac

**Not affected:**
- Server .23: racecontrol server binary does not ship sim adapters
- POS .20: rc-agent runs with `is_pos=true`, adapter is None regardless
- Bono VPS, James .27, comms-link: out of scope

## NOT TESTED (the lie-free list)

- I have NOT captured live UDP packets on Pod 4 port 20777 to confirm F1 25 actually emits them at runtime (relied on hardware_settings_config.xml as proxy evidence)
- I have NOT reproduced the 180s retry kill while observing server-side log for the retry LaunchGame WS message (inferred from billing.rs:2472 source code)
- I have NOT confirmed that Bug B allowlist fix worked, because every attempt gets killed at 180s before the periodic minimize tick has a chance to fire against a long-lived game
- I have NOT tested whether Option A affects iRacing / LMU SHM adapter construction order (the SHM adapters have their own HARD-03 defer logic)
- I have NOT confirmed that attempt 02 was actually killed at 180s vs. some other cause — the memory record said "crashed at ~3 min with 7GB RAM" but the 180s match is suspicious; 3 minutes could be the 180s launch timeout, or a real game crash, or both
- I have NOT read the server's waiting_for_game lifecycle code to confirm the exact WS retry message shape (read billing.rs:2467-2479 but did not trace end-to-end)
- I have NOT checked whether Pod 4's rc-agent.toml has any `[sims]` or similar section that would enable multi-adapter mode if present — only verified `[pod].sim = "assetto_corsa"`

## Decision pending

User asked to return to structured debugging. DIVERGENCE REPORT is complete. Fix is NOT applied.

Next step: user picks Option A / B / C / D or requests further hypothesis testing before fix.
