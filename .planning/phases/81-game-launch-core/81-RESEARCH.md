# Phase 81: Game Launch Core - Research

**Researched:** 2026-03-21
**Domain:** Multi-game launch, crash recovery, kiosk UX, PWA game request flow
**Confidence:** HIGH — all findings sourced directly from the existing codebase

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- All 5 games installed via Steam — use `steam://rungameid/{id}` launch method
- `detect_installed_games()` already handles per-pod game filtering via Steam appmanifest
- Start with Steam launch only, add args if specific games require them
- Full crash recovery for non-AC games — same as AC: detect, cleanup, auto-relaunch with backoff, alert staff after N failures
- Non-AC crash branch at main.rs line ~1559 logs warning only — needs `GameProcess::launch()` call with cached config
- Direct launch for non-AC games — no wizard. Click game icon on pod card, it launches immediately
- AC keeps its existing wizard flow (custom experience booking)
- Game logos + names in kiosk — including Assetto Corsa
- Customer PWA game menu — customer sees available games on their phone, taps one, staff gets a notification to confirm and launch
- Existing `GameState` enum (Idle, Launching, Running, Crashed) is sufficient for non-AC games
- Fleet health API already reports `current_game` and `game_state` — no schema changes needed
- Spectator view shows game name as text only — e.g., "F1 25", "iRacing"

### Claude's Discretion
- Steam PID discovery strategy for process monitoring
- Game logo asset sourcing and bundling approach
- Exact crash recovery backoff parameters for non-AC games (can mirror AC's `EscalatingBackoff`)
- Whether to add EA WRC-specific config (telemetry JSON config file deployment) — may be Phase 87 scope

### Deferred Ideas (OUT OF SCOPE)
- None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| LAUNCH-01 | Staff can select F1 25, iRacing, AC EVO, EA WRC, or LMU from kiosk and launch on any pod with safe defaults | Direct launch path in KioskPodCard/SetupWizard already wired; needs non-AC game icon UI + bypassing wizard for non-AC |
| LAUNCH-02 | Customer can request a game launch from PWA/QR, staff confirms via kiosk | New: PWA game request endpoint + `DashboardEvent` variant + kiosk confirm UI |
| LAUNCH-03 | Game launch profiles define exe path, launch args, and safe defaults per game (TOML config) | `GameExeConfig` fully supports this; Steam app IDs documented in rc-agent.example.toml |
| LAUNCH-04 | Game process monitored — detect crash/hang, auto-cleanup stale processes | `failure_monitor.rs` + `game_process.rs` already monitor; coverage gap: non-AC process name EA WRC |
| LAUNCH-05 | Crash recovery auto-restarts game or alerts staff with option to relaunch | Non-AC crash branch in `CrashRecoveryState::PausedWaitingRelaunch` arm at main.rs:1559 is a stub — needs `GameProcess::launch()` call |
| LAUNCH-06 | Which game is running on which pod visible in kiosk and fleet health dashboard | `PodInfo.current_game + game_state` already flow to kiosk via WS; fleet page needs game badge display |
</phase_requirements>

---

## Summary

Phase 81 is almost entirely a wire-up phase. The infrastructure already exists — `GameProcess::launch()`, `GamesConfig`, `detect_installed_games()`, `GameManager`, `handle_game_state_update()`, `CrashRecoveryState` — and most of the kiosk game launch flow is already functional for the AC wizard path. The primary gaps are:

1. **Non-AC crash recovery in `main.rs`** — the `CrashRecoveryState::PausedWaitingRelaunch` arm at line 1559 has an `else` branch that logs a warning and does nothing. It needs the same `GameProcess::launch()` call the LaunchGame handler uses.
2. **Direct launch UI in kiosk** — `onLaunchGame` on the pod card currently opens the AC wizard. Non-AC games need a direct path: click game icon → immediate launch without wizard steps.
3. **Customer PWA game request** — there is no game request endpoint or `DashboardEvent` variant yet. Needs a new REST endpoint, a new `DashboardEvent::GameLaunchRequested` variant, and a kiosk confirm banner.
4. **Steam app IDs in racecontrol.toml** — the rc-agent.example.toml already documents the correct app IDs for all 5 target games. The deployed TOML files (pod1.toml etc.) currently only have `assetto_corsa` configured; each pod needs its installed games added.
5. **Game logos in kiosk** — no logo assets exist yet; approach must be decided (static PNGs bundled in kiosk/public/).

**Primary recommendation:** Wire non-AC crash recovery in `main.rs` first (one function call), then fix the kiosk direct launch UI, then add PWA game request as an additive new endpoint + event, then populate TOML and bundle logos.

---

## Standard Stack

### Core (no new dependencies required)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `sysinfo` | existing | Process name scan via `find_game_pid()` | Already in rc-agent; used for PID discovery on Steam launches |
| `winapi` 0.3 | existing | `OpenProcess` / `GetExitCodeProcess` for `is_process_alive()` | Already in rc-agent; no additional crates needed |
| `tokio` | existing | Async crash recovery timer | Already in rc-agent event loop |
| Next.js + React | existing | Kiosk UI | No new frontend packages |

**No new dependencies are required for any part of this phase.** The existing stack covers all needs.

**Installation:** None — zero new deps.

---

## Architecture Patterns

### Recommended Project Structure (what changes)

```
crates/rc-agent/src/
├── main.rs                  # Fix non-AC crash recovery branch (~line 1559)
├── game_process.rs          # No changes — launch() already works
├── config.rs                # No changes — GamesConfig already has all fields
└── failure_monitor.rs       # No changes — already detects crashes within 30s

crates/rc-common/src/
└── protocol.rs              # Add DashboardEvent::GameLaunchRequested variant

crates/racecontrol/src/
├── api/routes.rs            # Add POST /pwa/game-request endpoint
└── game_launcher.rs         # No changes — handle_game_state_update already works

kiosk/src/
├── components/KioskPodCard.tsx        # Add non-AC direct launch game picker
├── components/SetupWizard.tsx         # No AC path changes; non-AC bypasses wizard
├── hooks/useKioskSocket.ts            # Handle new GameLaunchRequested event
├── app/staff/page.tsx                 # Add game launch request confirmation banner
└── public/game-logos/                 # New: PNG logos for 6 games

deploy/
└── rc-agent.template.toml   # Add game stanzas for all 5 non-AC games
target/release/
└── rc-agent-pod*.toml       # Update each pod's TOML with installed games
```

### Pattern 1: Non-AC Crash Recovery (the key fix)

**What:** The `CrashRecoveryState::PausedWaitingRelaunch` arm in `main.rs` already handles AC perfectly (AC-specific launcher call). The non-AC branch (`else` at line 1559) is a stub. It should call `GameProcess::launch()` with the cached `last_sim_type` and `last_launch_args_stored`.

**When to use:** Triggered by `CrashRecoveryState::PausedWaitingRelaunch { attempt < 2, last_sim_type != AssettoCorsa }`

**Example (the fix):**
```rust
// Source: crates/rc-agent/src/main.rs ~line 1559
// BEFORE (stub):
} else {
    tracing::warn!("[crash-recovery] Non-AC relaunch for {:?} — check LaunchGame handler else branch", last_sim_type);
}

// AFTER (fix — mirrors the LaunchGame else branch at ~line 2040):
} else {
    // Non-AC: use GameProcess::launch() with cached config
    let base_config = match last_sim_type {
        SimType::AssettoCorsaEvo => &config.games.assetto_corsa_evo,
        SimType::AssettoCorsaRally => &config.games.assetto_corsa_rally,
        SimType::IRacing => &config.games.iracing,
        SimType::F125 => &config.games.f1_25,
        SimType::LeMansUltimate => &config.games.le_mans_ultimate,
        SimType::Forza => &config.games.forza,
        SimType::ForzaHorizon5 => &config.games.forza_horizon_5,
        _ => &config.games.assetto_corsa, // unreachable — AC handled above
    };
    let mut game_cfg = base_config.clone();
    if let Some(ref args) = last_launch_args { game_cfg.args = Some(args.clone()); }
    match game_process::GameProcess::launch(&game_cfg, last_sim_type) {
        Ok(gp) => {
            let pid = gp.pid;
            game_process = Some(gp);
            let _ = failure_monitor_tx.send_modify(|s| { s.game_pid = pid; });
            tracing::info!("[crash-recovery] Attempt 2: {:?} relaunched (pid: {:?})", last_sim_type, pid);
        }
        Err(e) => {
            tracing::error!("[crash-recovery] Attempt 2: {:?} launch failed: {}", last_sim_type, e);
        }
    }
}
```

### Pattern 2: Direct Launch UI (kiosk)

**What:** When staff clicks on a pod card and a billing session is active, non-AC games bypass the SetupWizard. The `onLaunchGame` callback currently opens the wizard at `select_game`. The fix adds a game picker panel that lists installed games; selecting a non-AC game calls `api.launchGame(podId, simType, null)` directly.

**Existing hook point:** `KioskPodCard.tsx` line 389 — `{onLaunchGame && (!gameInfo || gameInfo.game_state === "idle") && ...}` already renders a launch button. The button calls `onLaunchGame(pod.id)`. In `staff/page.tsx` line 416-420, `onLaunchGame` currently opens the wizard at `select_game`.

**Fix:** Change the `onLaunchGame` handler so that for non-AC game selection, it calls `api.launchGame(podId, simType, null)` directly (no wizard), then switches to `live_session` panel. AC still goes through the wizard as before.

### Pattern 3: PWA Game Request (new endpoint)

**What:** Customer taps a game in PWA → POST to `/api/v1/pwa/game-request` → server stores pending request + broadcasts `DashboardEvent::GameLaunchRequested` → kiosk shows confirmation banner → staff taps confirm → calls existing `api.launchGame()`.

**New DashboardEvent variant to add in `protocol.rs`:**
```rust
// Source: crates/rc-common/src/protocol.rs — add to DashboardEvent enum
/// Customer requested a game launch from PWA — staff must confirm
GameLaunchRequested {
    pod_id: String,
    sim_type: SimType,
    driver_name: String,
    request_id: String,
},
```

**New racecontrol endpoint:**
```rust
// POST /api/v1/pwa/game-request
// Body: { pod_id, sim_type, driver_name }
// Auth: customer JWT (same as other PWA endpoints)
// Action: store pending request in AppState, broadcast DashboardEvent::GameLaunchRequested
// Staff confirm: existing POST /api/v1/games/pod/{pod_id}/launch
```

### Pattern 4: TOML Game Profiles

**What:** Each pod's `rc-agent.toml` needs `[games.X]` stanzas for installed games. `GameExeConfig` is already fully deserialized from TOML.

**Steam App IDs (from rc-agent.example.toml — HIGH confidence):**
| Game | Steam App ID | TOML key |
|------|-------------|----------|
| F1 25 | 3059520 | `[games.f1_25]` |
| iRacing | 266410 | `[games.iracing]` |
| AC EVO | 3058630 | `[games.assetto_corsa_evo]` |
| EA WRC (AC Rally) | 3917090 | `[games.assetto_corsa_rally]` |
| LMU | 1564310 | `[games.le_mans_ultimate]` |

**Standard stanza (same for all Steam games):**
```toml
[games.f1_25]
steam_app_id = 3059520
use_steam = true
```

### Pattern 5: Steam PID Discovery (Claude's Discretion — resolved)

**Recommendation:** Use the existing `find_game_pid(sim_type)` function in `game_process.rs`. The 2-second `game_check_interval` in `main.rs` already calls this for Steam-launched games and transitions to `GameState::Running` when a PID appears. No new code needed. The existing pattern is the correct strategy.

**How it works:** `find_game_pid()` uses `sysinfo::System::refresh_processes()` to scan by process name. The `process_names()` function in `game_process.rs` is already exhaustive across all 8 `SimType` variants. The test `test_process_names_exhaustive()` enforces this invariant at compile time.

### Pattern 6: Game Logo Assets (Claude's Discretion — resolved)

**Recommendation:** Static bundled PNGs in `kiosk/public/game-logos/`. No CDN, no dynamic loading. Place files as:
```
kiosk/public/game-logos/
├── assetto-corsa.png
├── assetto-corsa-evo.png
├── assetto-corsa-rally.png
├── iracing.png
├── f1-25.png
├── le-mans-ultimate.png
└── forza.png
```
Reference with `<img src="/game-logos/f1-25.png" />`. Next.js serves `public/` statically — no config needed. This is the simplest approach: no new deps, no runtime fetch, works offline on LAN.

### Anti-Patterns to Avoid

- **Modifying `GameExeConfig` or `GamesConfig`** — these structs are complete for this phase. Do not add fields.
- **Adding a new `CrashRecoveryState` variant** — the existing `PausedWaitingRelaunch` handles both AC and non-AC. Only the `else` branch needs to call `GameProcess::launch()`.
- **Blocking the main.rs event loop with `launch_ac()`** — AC uses `tokio::task::spawn_blocking`. Non-AC uses the non-blocking `GameProcess::launch()` (it spawns `cmd /C start` and returns immediately). Do not add `spawn_blocking` to non-AC.
- **Changing the `GameState` enum** — no new states needed for this phase. `Idle/Launching/Running/Error` cover all cases.
- **Requiring billing for PWA game request** — the game request is a request, not a launch. Staff confirms and the billing check happens at `launch_game()` server-side as usual.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process alive check | Custom WinAPI wrapper | `is_process_alive()` in `game_process.rs` | Already battle-tested, handles STILL_ACTIVE (exit code 259) |
| Steam game PID discovery | New sysinfo integration | `find_game_pid(sim_type)` in `game_process.rs` | Already exhaustive across all SimType variants |
| Crash detection timing | Custom timer | `failure_monitor.rs` with 5s poll + 30s UDP silence threshold | Existing — covers CRASH-01 and CRASH-02 already |
| Game process cleanup | Custom kill logic | `cleanup_orphaned_games()` in `game_process.rs` | Handles PID file + name scan; called on agent startup |
| Server-side auto-relaunch | New logic | `handle_game_state_update()` in `game_launcher.rs` (already has Race Engineer with 5s delay, 2 attempts) | Already complete including billing pause |
| Backoff | Custom struct | `EscalatingBackoff` in `rc-common/src/watchdog.rs` (30s→2m→10m→30m) | Existing — for Phase 81 the 60s fixed timer in `CrashRecoveryState` matches AC behavior, no change needed |

---

## Common Pitfalls

### Pitfall 1: Dual crash recovery paths (agent vs server)

**What goes wrong:** Both `main.rs` (agent-side, `CrashRecoveryState`) and `game_launcher.rs` (server-side, Race Engineer) attempt auto-relaunch. If both fire simultaneously, the pod gets double-launched.

**Why it happens:** The agent-side recovery fires when `billing_active=true` (line 1198). The server-side Race Engineer also fires when it receives `GameState::Error` with an active billing session (line 393 of `game_launcher.rs`).

**How to avoid:** The agent-side recovery sends `GameCrashed` to the server, which sets `recovery_in_progress=true` in `failure_monitor_tx`. The server-side Race Engineer delay (5s tokio::spawn) should be checked — if both try to relaunch at the same time, only one will succeed because `GameProcess::launch()` is idempotent (Steam URL) but could produce two processes. Verify: does the agent-side crash recovery suppress the server Race Engineer? Currently the server does NOT check `recovery_in_progress` before auto-relaunching. **Mitigation:** Add a comment noting this — for Phase 81, both paths running is acceptable since Steam will only launch one instance. If double-launch becomes observable, suppressing server Race Engineer for non-AC can be done in Phase 82.

**Warning signs:** Two game processes running simultaneously (detectable via `find_game_pid()` returning a PID for a game that was supposedly launching).

### Pitfall 2: Steam PID gap — agent sees no PID after launch

**What goes wrong:** `GameProcess::launch()` for Steam games returns `pid: None` (Steam URL via `cmd /C start` does not give a child PID). The `game_check_interval` (2s) must call `find_game_pid()` to discover the PID. If Steam takes more than 90s to start (update, patch, etc.), `failure_monitor.rs` CRASH-02 fires and kills the launch attempt.

**Why it happens:** Steam may run updates or patches before launching the game. The 90s `LAUNCH_TIMEOUT_SECS` in `failure_monitor.rs` is tight for games that auto-update.

**How to avoid:** Do not change the 90s timeout for Phase 81. Document that staff should pre-launch Steam and ensure games are up to date. This is a known operational constraint.

**Warning signs:** `failure_monitor` logging "Launch timeout: 90s elapsed, no game PID" immediately after a Steam update.

### Pitfall 3: TOML file not updated on pods — `detect_installed_games()` reports only AC

**What goes wrong:** `GamesConfig` fields default to `GameExeConfig::default()` (all None). `detect_installed_games()` skips any game with `steam_app_id.is_none()`. If the pod TOML doesn't have `[games.f1_25]` etc., no non-AC games are reported in `installed_games`.

**Why it happens:** The deployed pod TOML files (e.g., `rc-agent-pod1.toml`) currently only contain `[games.assetto_corsa]`. New stanzas must be deployed to each pod.

**How to avoid:** Update `deploy/rc-agent.template.toml` and redeploy to all pods as part of this phase. The kiosk filters the game picker to `pod.installed_games` — if a game isn't installed, it won't appear.

**Warning signs:** Kiosk showing no non-AC games on a pod card despite the game being installed.

### Pitfall 4: EA WRC process name not in `all_game_process_names()`

**What goes wrong:** EA WRC uses `SimType::AssettoCorsaRally` (per existing enum). The process name must be `acr.exe` (confirmed in `process_names()` at `game_process.rs:291`). If the actual EA WRC executable on pods has a different name, orphan cleanup and PID discovery will fail.

**Why it happens:** EA WRC is a Codemasters game on Steam and may use a different binary name than `acr.exe`.

**How to avoid:** Verify the actual executable name on a pod before assuming `acr.exe`. This requires physical verification on Pod 8 (test pod). If different, add to `process_names(SimType::AssettoCorsaRally)` and `all_game_process_names()`.

**Warning signs:** `find_game_pid(SimType::AssettoCorsaRally)` returning None even when the game is running.

### Pitfall 5: Kiosk game logo path case sensitivity

**What goes wrong:** Windows filesystem is case-insensitive but Next.js production builds on Linux-based CI may fail to serve `/game-logos/F1-25.png` vs `/game-logos/f1-25.png`.

**Why it happens:** The kiosk runs on Windows Server .23 but may be built/tested on Linux.

**How to avoid:** Use all-lowercase kebab-case filenames for logo PNGs. Reference them with the exact same casing in code.

---

## Code Examples

### Non-AC Crash Recovery Fix (main.rs)

The fix location is the `else` branch at approximately line 1559 in `main.rs`:

```rust
// Source: crates/rc-agent/src/main.rs — CrashRecoveryState::PausedWaitingRelaunch handler
// In the `attempt < 2` arm, after the AC block:
} else {
    // Non-AC game — use GameProcess::launch() with cached config
    let base_config = match last_sim_type {
        SimType::AssettoCorsaEvo   => &config.games.assetto_corsa_evo,
        SimType::AssettoCorsaRally => &config.games.assetto_corsa_rally,
        SimType::IRacing           => &config.games.iracing,
        SimType::F125              => &config.games.f1_25,
        SimType::LeMansUltimate    => &config.games.le_mans_ultimate,
        SimType::Forza             => &config.games.forza,
        SimType::ForzaHorizon5     => &config.games.forza_horizon_5,
        SimType::AssettoCorsa      => &config.games.assetto_corsa, // unreachable
    };
    let mut game_cfg = base_config.clone();
    if let Some(ref a) = last_launch_args { game_cfg.args = Some(a.clone()); }
    heartbeat_status.game_running.store(true, std::sync::atomic::Ordering::Relaxed);
    let launching_info = GameLaunchInfo {
        pod_id: pod_id.clone(),
        sim_type: last_sim_type,
        game_state: GameState::Launching,
        pid: None,
        launched_at: Some(Utc::now()),
        error_message: None,
        diagnostics: None,
    };
    let _ = ws_tx.send(Message::Text(
        serde_json::to_string(&AgentMessage::GameStateUpdate(launching_info))
            .unwrap_or_default()
            .into()
    )).await;
    let _ = failure_monitor_tx.send_modify(|s| {
        s.launch_started_at = Some(std::time::Instant::now());
    });
    match game_process::GameProcess::launch(&game_cfg, last_sim_type) {
        Ok(gp) => {
            let pid = gp.pid;
            game_process = Some(gp);
            let _ = failure_monitor_tx.send_modify(|s| { s.game_pid = pid; });
            tracing::info!("[crash-recovery] attempt {} {:?} relaunched (pid: {:?})", attempt + 1, last_sim_type, pid);
        }
        Err(e) => {
            tracing::error!("[crash-recovery] attempt {} {:?} failed: {}", attempt + 1, last_sim_type, e);
        }
    }
}
```

### TOML Profile Example

```toml
# rc-agent-pod8.toml — Pod 8 has F1 25 and iRacing installed
[pod]
number = 8
name = "Pod 8"
sim = "assetto_corsa"
sim_ip = "127.0.0.1"
sim_port = 9996

[core]
url = "ws://192.168.31.23:8080/ws/agent"

[kiosk]
enabled = false

[games.assetto_corsa]
steam_app_id = 244210
use_steam = false

[games.f1_25]
steam_app_id = 3059520
use_steam = true

[games.iracing]
steam_app_id = 266410
use_steam = true
```

### DashboardEvent::GameLaunchRequested (new variant in protocol.rs)

```rust
// Source: crates/rc-common/src/protocol.rs — add to DashboardEvent enum
/// Customer requested a game launch from PWA — staff must confirm before launch
GameLaunchRequested {
    pod_id: String,
    sim_type: SimType,
    driver_name: String,
    request_id: String,
},
```

### PWA Game Request Endpoint (new in routes.rs)

```rust
// POST /api/v1/pwa/game-request
// Same auth pattern as other PWA endpoints — customer JWT required
#[derive(Deserialize)]
struct GameRequestBody {
    pod_id: String,
    sim_type: SimType,
}

async fn pwa_game_request(
    State(state): State<Arc<AppState>>,
    claims: CustomerClaims,
    Json(body): Json<GameRequestBody>,
) -> impl IntoResponse {
    let request_id = uuid::Uuid::new_v4().to_string();
    // Validate: pod exists + game installed on pod
    // Store pending request in AppState.pending_game_requests (new RwLock<HashMap>)
    // Broadcast:
    let _ = state.dashboard_tx.send(DashboardEvent::GameLaunchRequested {
        pod_id: body.pod_id,
        sim_type: body.sim_type,
        driver_name: claims.name,
        request_id,
    });
    Json(json!({ "ok": true, "request_id": request_id }))
}
```

### Kiosk Direct Launch Handler (staff/page.tsx)

```typescript
// In staff/page.tsx — onLaunchGame handler
// Non-AC: show game picker → user selects → call api.launchGame directly
// AC: open wizard at select_game step (no change)
const handleDirectLaunch = async (podId: string, simType: string) => {
  if (simType === "assetto_corsa") {
    setSelectedPodId(podId);
    setPanelMode("setup");
    wizard.reset();
    wizard.goToStep("select_game");
  } else {
    await api.launchGame(podId, simType, null);
    setPanelMode("live_session");
  }
};
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| AC-only crash recovery | Parameterized by `last_sim_type` in `CrashRecoveryState` | Already in place (Phase 24+) | Non-AC branch is wired but stubbed — just needs the actual launch call |
| AC wizard for all games | Direct launch for non-AC (to be added) | Phase 81 | UX simplification: click-and-go for non-AC |
| No customer game choice | PWA game request (new) | Phase 81 | Customer empowerment without staff burden |

**Deprecated/outdated:**
- The comment at `main.rs:1560` "check LaunchGame handler else branch" — Phase 81 resolves this stub.
- `rc-agent-pod*.toml` files missing non-AC game stanzas — Phase 81 populates them.

---

## Open Questions

1. **EA WRC actual executable name**
   - What we know: `SimType::AssettoCorsaRally` → `process_names()` returns `["acr.exe"]`
   - What's unclear: Does the actual EA WRC Steam binary match `acr.exe` or use something else (e.g., `WRC.exe`, `WRCTheGame.exe`)?
   - Recommendation: Verify on Pod 8 before deploying. Check `C:\Program Files (x86)\Steam\steamapps\common\` for the EA WRC install directory. Update `process_names()` if needed.

2. **Which pods have which games installed**
   - What we know: All pods have AC. Pod-specific installs are unknown without on-site verification.
   - What's unclear: Do all 8 pods have F1 25? iRacing? LMU?
   - Recommendation: Uday or James physical verification before updating TOML files. Start with Pod 8 (test pod first rule).

3. **PWA game request confirmation UX**
   - What we know: `DashboardEvent::GameLaunchRequested` will broadcast to kiosk; kiosk must show a confirm banner.
   - What's unclear: How long does the request stay pending? Auto-expire after 60s?
   - Recommendation: Add 60s auto-expiry via `tokio::time::sleep` in the route handler. If staff doesn't confirm in 60s, broadcast a cancellation event and remove the request.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (nextest configured at `.config/nextest.toml`) |
| Config file | `.config/nextest.toml` |
| Quick run command | `cargo test -p rc-agent -- crash_recovery 2>&1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LAUNCH-01 | Staff launches non-AC game via kiosk | manual smoke | deploy to pod8, launch F1 25 | ❌ Wave 0 |
| LAUNCH-02 | PWA game request → kiosk confirm | manual smoke | PWA on phone → staff kiosk | ❌ Wave 0 |
| LAUNCH-03 | TOML launch profile loads correctly | unit | `cargo test -p rc-agent -- test_game_exe_config` | ✅ existing |
| LAUNCH-03 | detect_installed_games uses steam_app_id | unit | `cargo test -p rc-agent -- test_installed_games` | ✅ existing |
| LAUNCH-04 | Crash detection within 30s | unit | `cargo test -p rc-agent -- crash01` | ✅ existing |
| LAUNCH-04 | Launch timeout after 90s | unit | `cargo test -p rc-agent -- crash02` | ✅ existing |
| LAUNCH-05 | Non-AC crash recovery calls GameProcess::launch | unit | `cargo test -p rc-agent -- non_ac_crash_recovery` | ❌ Wave 0 |
| LAUNCH-06 | current_game/game_state in PodInfo | unit | `cargo test -p racecontrol -- game_state_update` | ✅ existing |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent 2>&1 | tail -5`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/main.rs` — unit test for non-AC crash recovery path (`non_ac_crash_recovery`) — covers LAUNCH-05. Add to `main.rs` `#[cfg(test)]` block or extract testable function.
- [ ] Manual smoke checklist for LAUNCH-01 and LAUNCH-02 (cannot be automated without real pods)

*(Existing tests in `failure_monitor.rs`, `game_process.rs`, `config.rs`, and `game_launcher.rs` cover LAUNCH-03, LAUNCH-04, LAUNCH-06.)*

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/game_process.rs` — `GameProcess::launch()`, `find_game_pid()`, `process_names()`, `all_game_process_names()`, `cleanup_orphaned_games()`
- `crates/rc-agent/src/config.rs` — `GamesConfig`, `GameExeConfig`, `detect_installed_games()`, `is_steam_app_installed()`
- `crates/rc-agent/src/main.rs` (lines 60-77, 912-916, 1196-1240, 1430-1580, 2040-2151) — `CrashRecoveryState`, LaunchGame handler, crash recovery state machine
- `crates/rc-agent/src/failure_monitor.rs` — `FailureMonitorState`, CRASH-01/CRASH-02 detection, 90s launch timeout, 30s UDP silence threshold
- `crates/racecontrol/src/game_launcher.rs` — `GameManager`, `launch_game()`, `handle_game_state_update()`, Race Engineer auto-relaunch
- `crates/rc-common/src/types.rs` — `SimType`, `GameState`, `PodInfo.current_game`, `PodInfo.installed_games`
- `crates/rc-common/src/protocol.rs` — `DashboardEvent`, `CoreToAgentMessage::LaunchGame`, `AgentMessage::GameCrashed`
- `crates/rc-common/src/watchdog.rs` — `EscalatingBackoff`
- `kiosk/src/components/KioskPodCard.tsx` — `onLaunchGame`, `onRelaunchGame` props
- `kiosk/src/components/SetupWizard.tsx` — game selection step, non-AC path (`isAc` check)
- `kiosk/src/app/staff/page.tsx` — `handleGameLaunch`, wizard flow
- `rc-agent.example.toml` — Steam app IDs for all 6 games (HIGH confidence — authored as definitive reference)
- `target/release/rc-agent-pod1.toml` — current pod TOML format (confirms only AC configured today)

### Secondary (MEDIUM confidence)
- None needed — all findings sourced directly from repo

### Tertiary (LOW confidence)
- EA WRC executable name `acr.exe` — assumed from existing `process_names()` enum, not verified on physical pod

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — zero new deps, all existing libraries verified in source
- Architecture: HIGH — all patterns sourced from existing working code
- Pitfalls: HIGH — non-AC crash stub confirmed at main.rs:1559; Steam PID gap is documented in existing comments
- Steam App IDs: HIGH — sourced from `rc-agent.example.toml` which is the authored reference file
- EA WRC process name: LOW — assumed `acr.exe`, needs physical pod verification

**Research date:** 2026-03-21 IST
**Valid until:** Stable (Rust crates don't change; only risk is EA WRC exe name on actual pods)
