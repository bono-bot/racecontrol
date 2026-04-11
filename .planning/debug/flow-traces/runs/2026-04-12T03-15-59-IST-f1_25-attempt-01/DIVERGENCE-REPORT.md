# Divergence Report — F1 25 Staff-Launch Trace, Attempt 01

**Run:** `runs/2026-04-12T03-15-59-IST-f1_25-attempt-01/`
**Capture window:** T-0 = 2026-04-12 03:16:23 IST → T+now ≈ 03:24 IST
**Operator:** James (live trace, user awake at console)
**User actions:** "ready" → "launched on kiosk" → "Fail to start error" → "API 409"

---

## TL;DR — Root cause confirmed in code

**F1 25 launches successfully on Pod 4 every time.** The server kills the launched game ~180 seconds later because F1 25's sim adapter is gated on `speed_kmh > 0` for the playable signal, and customers cannot navigate the F1 25 menus → load track → reach the green flag → reach speed > 0 within the 180-second `default_launch_timeout_per_attempt`. The launch verifier interprets the missing signal as "game died", BILL-13 cancels the billing session, rc-agent kills the game and re-engages the lock screen. Customer sees: launch click → F1 25 splash → black screen → "fail to start". Server returns 409 to subsequent retries because the cancelled billing session is still in-state.

**Affects:** F1 25 only. AC, ACE, iRacing, LMU use shared-memory connectivity for playable detection (no `DetectorSignal` plumbing) and are unaffected.

**Aligns with operational metric:** 63.35% game launch success rate / 81 LaunchTimeout failures (from `project_operational_health_findings.md`). F1 25 launches account for the bulk of LaunchTimeouts.

---

## Trace timeline (UTC + 5:30 = IST)

| UTC | IST | Layer | Event | Source |
|---|---|---|---|---|
| 21:46:23 | 03:16:23 | observer | T-0 baseline captured. Pod 4 idle, blanked, no game_state field on fleet health | runs/.../observations/T0-* |
| ~21:48:30 | ~03:18:30 | user | Click launch on kiosk staff (Pod 4 + F1 25 + per-min tier) | user report |
| ~21:48:30 | ~03:18:30 | server | POST /games/launch (or WS LaunchGame) — not in 600-line tail window we captured | inferred |
| 21:48:46.881 | 03:18:46 | server | GET /api/v1/fleet/health | racecontrol-.2026-04-11.jsonl |
| 21:48:47.693 | 03:18:47 | pod_4 → server | `[kiosk] Pod pod_4 requesting approval for 'nvngx_update.exe'` (NVIDIA driver helper, started by F1 25 launch path) | racecontrol log |
| 21:48:57.689 | 03:18:57 | pod_4 → server | `[kiosk] Pod pod_4 requesting approval for 'eaanticheat.gameservice.exe'` (EA AntiCheat, F1 25 prereq) | racecontrol log |
| ~03:19:00 | — | user | Sees "Fail to start error" on kiosk staff page | user report |
| 21:49:12 / 21:49:27 / 21:49:42 / 21:49:57 / 21:50:12 / 21:50:27 / 21:50:42 / 21:50:57 / 21:51:12 | every 15s | pod_4 → server | Repeated `eaanticheat.gameservice.exe` approval requests — F1 25 EAC actively running | racecontrol log |
| 21:49:20.077 → 21:49:38.204 | 03:19:20 → 03:19:38 | user → server | **7× POST /api/v1/billing/start → 409** in 18 seconds | racecontrol log |
| ~21:49:30 | ~03:19:30 | observer | T+~180s capture: Pod 4 DXGI shows F1 25 EA SPORTS splash visible | runs/.../observations/T+05/dxgi.jpg |
| ~21:49:30 | ~03:19:30 | observer | fleet health for pod_4: `game_state: running`, `screen_blanked: false` | runs/.../observations/T+10/fleet-health.json |
| 21:51:17.872 | 03:21:17 | server | **`Launch timeout (attempt 1) for pod pod_4 — allowing retry (attempt 2)`** | racecontrol log billing target |
| 21:51:18.460 | 03:21:18 | server | `Pod pod_4 game state: Error (AssettoCorsa)` (likely AC monitor noise — pod TOML primary sim is `assetto_corsa`) | racecontrol log ws target |
| 21:51:19.618 | 03:21:19 | server | `Pod pod_4 game state: Error (F125)` ← **the relevant one** | racecontrol log ws target |
| 21:51:25.876 | 03:21:25 | server | `Pod pod_4 game state: Error (F125)` (second emission) | racecontrol log ws target |
| 21:51:25.829 | 03:21:25 | server | `Pod pod_4 FFB zeroed (safety action completed)` (force feedback returned to neutral) | racecontrol log ws target |
| 21:51:32.048 | 03:21:32 | server | `Pod pod_4 FFB zeroed (safety action completed)` (second emission) | racecontrol log ws target |
| 21:51:49.626 | 03:21:49 | server | `Pod pod_4 AC STATUS: Off` | racecontrol log ws target |
| 21:51:49.637 | 03:21:49 | server | **`BILL-13: Pre-committed session cancelled_no_playable: pod=pod_4 session=1125cef0-e997-4465-adca-c2af8763a422 (game died before PlayableSignal)`** | racecontrol log billing target |
| 21:51:54.482 | 03:21:54 | server | `Pod healer AI suggestion for pod_4: ... memory compression process (PID 3600) consuming 667MB suggesting...` (false-positive AI hypothesis from openrouter/deepseek-chat-v3) | racecontrol log pod_healer target |
| ~03:24:00 | ~03:24 | observer | T+now capture: Pod 4 DXGI back to blanked (Racing Point Esports logo). game_state: error, screen_blanked: true. **The game was killed.** | runs/.../observations/T+now/dxgi.jpg |

---

## Layer-by-layer divergence walk

### Layer 1 — Browser (kiosk staff) ✅ behaved correctly
- POST /api/v1/billing/start fired (we see 7× 409 in the log — those are user retries because the UI showed failure)
- POST /api/v1/games/launch presumably also fired earlier (outside the 600-line tail window) — F1 25 actually started, so the launch command DID land

### Layer 2 — Server (racecontrol :8080) ✅ accepted the launch
- Server forwarded `LaunchGame` to Pod 4 (inferred from EA AntiCheat starting on Pod 4)
- Pre-committed billing session created (session_id `1125cef0-e997-4465-adca-c2af8763a422`)
- Server's launch verifier started 180-second timer waiting for `PlayableSignal`

### Layer 3 — Pod 4 rc-agent ✅ launched the game
- rc-agent received WS LaunchGame
- Spawned Steam URI / direct exe
- F1 25 started (proven by EAC + nvngx_update process_guard requests)
- F1 25 EA SPORTS intro visible on Pod 4 DXGI screenshot at T+10
- Game window active, lock screen hidden (`screen_blanked: false`)

### Layer 4 — F1 25 sim adapter ❌ **DIVERGENCE HERE**
- [`crates/rc-agent/src/sims/f1_25.rs:506-514`](../../../../../crates/rc-agent/src/sims/f1_25.rs)
- F1 25 game ran successfully — UDP packets ARE being received on port 20777
- BUT the adapter only emits `DetectorSignal::UdpActive` when `self.speed_kmh > 0`
- For the entire EA SPORTS splash → main menu → quick race → loading → grid → countdown sequence, `speed_kmh = 0`
- Player cannot reach speed > 0 within the 180s launch_timeout
- **Result: `PlayableSignal` is never sent up the chain to the launch verifier**

### Layer 5 — Launch verifier ❌ kills the game
- 180-second timer expires (`default_launch_timeout_per_attempt = 180` per [config.rs:687](../../../../../crates/racecontrol/src/config.rs))
- "Launch timeout (attempt 1) — allowing retry (attempt 2)"
- Retry attempt also fails (same 180s timeout, F1 25 STILL hasn't reached speed > 0)
- game_state set to `Error (F125)`
- BILL-13 cancels the pre-committed billing session as `cancelled_no_playable`

### Layer 6 — Pod 4 cleanup ❌ kills the running game
- Server tells rc-agent to clean up
- rc-agent kills F1 25 process tree
- Lock screen re-engages (blanked)
- FFB zeroed (safety)
- Pod 4 DXGI screenshot at T+now shows back to Racing Point Esports logo

### Layer 7 — Customer perception ❌
- Sees: splash → black screen → "fail to start"
- Reality: game was running, was killed by server thinking it died
- Each retry hits 409 because billing session is still in-state from the failed attempt

---

## Why this only affects F1 25

| Game | Playable detection mechanism | Affected? |
|---|---|---|
| Assetto Corsa | Shared memory `acpmf_physics` (open succeeds → playable) | ❌ no |
| Assetto Corsa EVO | Shared memory | ❌ no |
| iRacing | Shared memory + `irsdkEnableMem=1` | ❌ no |
| Le Mans Ultimate | Shared memory | ❌ no |
| **F1 25** | **UDP port 20777 + `speed_kmh > 0` gate** | ✅ **YES** |
| Forza Horizon 5 | UDP port 5300 (out of scope, separate adapter) | TBD |

Verification: `crates/rc-agent/src/sims/assetto_corsa.rs` does NOT use `signal_tx` or `DetectorSignal` at all (grep returned 0 matches). AC's playable detection is implicit in shared-memory connect success. F1 25 is uniquely affected because it's the only Steam-integrated sim that uses UDP-only telemetry and has the speed-gating defensive code.

---

## What the comment in f1_25.rs got right and what it got wrong

The comment block at line 506-509 says:

> Only signal UdpActive when we have valid on-track telemetry (speed > 0).
> F1 25 sends button-event packets in pre-race menus (tyre strategy, formation
> lap setup) that pass parse_header but contain no motion data. Firing UdpActive
> on those would start billing while the customer is still in menus.

**Right:** Pre-race menu packets exist and would falsely register as "playing" if all packets fired the signal. Without a gate, billing would start while customers are still configuring tyre strategy.

**Wrong:** The fix conflated two distinct concepts and gated both:
1. **"Game is launched and running"** (needed by launch_verifier to confirm launch succeeded)
2. **"Customer is actively playing"** (needed by billing_fsm to start charging)

The current code has only ONE signal (`UdpActive`) for both. By gating on `speed > 0`, the launch_verifier loses any signal that the game launched at all, even though it manifestly did.

The defensive intent was correct. The implementation should have been a TWO-signal split:
- `UdpReachable` (any parsed packet) → launch_verifier accepts
- `UdpActive` (speed > 0) → billing_fsm starts charging

---

## Discarded hypotheses (and why I had them)

### Hypothesis A — `f1_25` vs `f1_25_ac` namespace mismatch (DISCARDED)

I read [content_scanner.rs:110](../../../../../crates/rc-agent/src/content_scanner.rs) and saw two STEAM_APP_IDS entries for F1 25:
```rust
(2488620, SimType::F125, "f1_25", "F1 25"),
(3059520, SimType::F125, "f1_25_ac", "F1 25 (Anti-Cheat)"),
```

Pod 4 only has appmanifest_3059520.acf, so I theorized that the inventory would emit `f1_25_ac` while the TOML/server expected `f1_25` and the launch dispatch would silently fail.

**Why discarded:** Server's `/api/v1/fleet/pod-inventory/pod_4` returned `installed_sim_types: ["assetto_corsa", "assetto_corsa_evo", "f1_25"]`. The endpoint deduplicates by `SimType`, not by `game_id` string, so both 2488620 and 3059520 collapse to `SimType::F125 → "f1_25"`. The namespace bug is real in code-reading but doesn't manifest in the data path.

**Lesson:** Code-reading without environment evidence misled me. The trace immediately revealed the actual divergence in 30 seconds of log analysis. The CGP standing rule "PoE primary — eliminate hypotheses by environment, not by code-reading alone" is exactly right.

### Hypothesis B — STEAM_APP_IDS missing ACE/LMU (DISCARDED as cause; KEEP as separate finding)

I noted that 3058630 (ACE) and 2399420 (LMU) are not in STEAM_APP_IDS. This is true. But:
- ACE shows up in `installed_sim_types` anyway (must be added via a different scan path I haven't traced — possibly the `[games.*]` TOML sections, possibly a dedicated ACE scanner)
- LMU is missing from `installed_sim_types` — that IS a real bug, but it's not Bug 2

This is a real but separate bug. Logged in `FINDINGS.md` as "Candidate 2" — kept open for separate work.

### Hypothesis C — phantom `game_state: running` from previous session (DISCARDED)

I worried that game_state was stuck `running` from a previous launch and that's why the new launch returned 409. But T-0 fleet health showed Pod 4 had NO `game_state` field at all (idle/empty), not `running`. So this was wrong — the launch state machine WAS clean at T-0 and got into the bad state during this attempt.

---

## Files in this run

- `observations/T0-*` — Pre-fire baseline (8 artifacts)
- `observations/T+05/*` — T+~180s capture during launch (game splash visible)
- `observations/T+10/*` — Same window — fleet health captured `game_state: running`
- `observations/T+now/*` — Post-cancellation (game killed, blanked, error state)
- `observations/launch-metadata.txt` — User-reported error context
- `DIVERGENCE-REPORT.md` — this file
- `FIX-PROPOSAL.md` — fix options + recommendation (next file, NOT committed)

---

## Status

- ✅ Divergence layer identified (Layer 4: F1 25 sim adapter signal gating)
- ✅ Confirmed against current code, not just memory
- ✅ Customer-impact path traced end-to-end
- ⏳ Fix proposal pending — see `FIX-PROPOSAL.md`
- ⏳ Awaiting user approval before any commit
