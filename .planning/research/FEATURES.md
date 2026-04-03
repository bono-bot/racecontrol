# Feature Research

**Domain:** Gaming cafe per-pod game inventory, combo validation, launch telemetry, reliability dashboard
**Milestone:** v41.0 Game Intelligence System
**Researched:** 2026-04-03
**Confidence:** HIGH (based on codebase audit + domain knowledge)

---

## What Already Exists (Do Not Rebuild)

| Component | Location | What It Does |
|-----------|----------|--------------|
| AC content scanner | `crates/rc-agent/src/content_scanner.rs` | Scans cars + tracks dirs, produces `ContentManifest` |
| Game preset library | `crates/racecontrol/src/preset_library.rs` | CRUD API, `GamePresetWithReliability` type |
| `combo_reliability` table | `preset_library.rs` tests | `success_rate`, `total_launches` per `(pod_id, sim_type, car, track)` |
| Game Doctor | `crates/rc-agent/src/game_doctor.rs` | 12-point diagnostic, `GameDiagnosis`, `RetryHint` |
| Launch retry orchestrator | `crates/rc-agent/src/game_launch_retry.rs` | 2 retries + backoff + MMA escalation (60s bound) |
| 5-tier Meshed Intelligence | `crates/rc-agent/src/tier_engine.rs` | `GameLaunchFail`, `PreFlightFailed` handled |
| Crash loop detection | `crates/racecontrol/src/fleet_health.rs` | Flag set at >3 startups within 5 min (uptime <30s) |
| `launch_events` table | `crates/racecontrol/src/api/metrics.rs` | `outcome`, `error_taxonomy`, `duration_to_playable_ms` |
| `installed_games` field | `rc-common/src/types.rs` (PodStatus) | `Vec<SimType>` on each pod, already gated at launch |
| Steam appmanifest check | `crates/rc-agent/src/steam_checks.rs` | Checks C/D/E SteamLibrary paths for `appmanifest_{id}.acf` |
| Game catalog endpoint | `routes.rs:games_catalog` | Returns all games + per-game pod install count |
| WhatsApp crash loop alert | `ws/mod.rs:crash_loop_just_detected` | Fires on transition false to true |

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features the admin and kiosk operators assume exist. Missing = product feels broken or causes customer complaints.

| Feature | Why Expected | Complexity | Depends On (existing) |
|---------|--------------|------------|----------------------|
| **Per-pod kiosk game filter** — kiosk shows ONLY games installed on that pod | Customers click Forza on pods without it, get silent error or bad state | MEDIUM | `installed_games` field (exists), kiosk API + frontend filter (missing) |
| **Boot-time combo validation** — Game Doctor runs proactively at rc-agent startup, not only post-failure | Game launches silently fail 15 seconds in when AC car/track folder does not exist; staff do not know until customer complaint | MEDIUM | `game_doctor.rs` (exists, reactive only), `content_scanner.rs` (AC only) |
| **Non-functional combo flag** — invalid AC combos (missing car dir, missing track dir, no AI lines, no pit stalls) marked `valid: false` in preset library | Staff keep showing combos that never work; reliability score alone does not explain WHY | MEDIUM | `combo_reliability` table (exists), `ContentManifest` (exists), `GamePresetWithReliability` (exists) |
| **Per-pod available combos API** — server exposes which presets are valid on EACH pod (not fleet-wide) | Server currently returns fleet catalog; Pod 3 may have a car that Pod 7 does not; kiosk must know which pod the customer is at | MEDIUM | `content_scanner.rs` (AC), per-pod `ContentManifest` pushed via WS (exists for AC) |
| **Launch timeout watchdog** — kill and escalate if game process does not reach playable state within N seconds | Currently 60s bound in retry orchestrator but no intermediate timeout on the launch itself; pod locks indefinitely if acs.exe hangs mid-load | LOW | `game_launch_retry.rs` (exists), `DiagnosticTrigger` (exists) |

### Differentiators (Competitive Advantage)

Features that go beyond table stakes. Not assumed, but create measurably better operations.

| Feature | Value Proposition | Complexity | Depends On |
|---------|-------------------|------------|------------|
| **Steam library scanner for all games** — extend `content_scanner.rs` to detect F1 25, iRacing, Forza, LMU by appmanifest or known install paths | Replaces the manual `installed_games` TOML config (error-prone, stale); auto-discovers what is actually installed vs. what someone configured | MEDIUM | `steam_checks.rs` (appmanifest logic exists), `content_scanner.rs` (AC only) |
| **Launch timeline tracing** — structured log per launch: `WsCommandSent`, `AgentAckReceived`, `ProcessSpawned`, `TelemetryFirstPacket`, `PlayableConfirmed` with millisecond timestamps | Enables root cause per step (WS drop? Steam launch? Game boot? Telemetry init?) instead of just "launch failed" | HIGH | `launch_events` table (exists, lacks step-level columns), v40.0 WS ACK (in progress) |
| **Chain failure WhatsApp alert** — when 3+ consecutive launches on same pod fail within 10 min, alert staff with pod number, game, error taxonomy | Currently crash loop alerts on agent restart storms; game chain failures (game failing, not agent) are silent | LOW | `error_aggregator.rs` (exists), WhatsApp alerts (exists), `launch_events` table (exists) |
| **Reliability dashboard** — Next.js page showing: combo success rates heatmap, fleet game matrix (8 pods x 8 games grid), flagged combos list, recent chain failures | Gives Uday a one-glance view of what works across the fleet before venue opens | HIGH | `combo_reliability` (exists), `launch_events` (exists), `/api/v1/metrics/launch-matrix` (exists per metrics.rs audit) |
| **Dynamic launch timeout per combo** — timeout scales with combo history (new combo: 90s default; combo with p95 launch time 45s: use 65s) | Prevents 90s wait for combos that consistently launch in 20s; gives more time to combos that legitimately need it | MEDIUM | `launch_events.duration_to_playable_ms` (exists), needs p95 query + config injection |

### Anti-Features (Commonly Requested, Often Problematic)

| Anti-Feature | Why Requested | Why Problematic | Alternative |
|--------------|---------------|-----------------|-------------|
| **Real-time filesystem watcher** — ReadDirectoryChangesW watching the AC content directory for instant manifest updates | "Keep manifest always fresh without restart" | Windows filesystem watchers on Steam dirs are unreliable (Steam file locking + VSS shadow operations fire thousands of spurious events); adds a persistent background thread with complex error recovery | Periodic 5-min rescan + rescan on WS reconnect (already the boot pattern for allowlist and feature flags) |
| **Cross-pod combo sync** — when Pod 1 runs a combo successfully, auto-push that combo to all other pods' preset libraries | "Fleet self-improvement" | Combos depend on per-pod content; a car installed on Pod 1 may not exist on Pod 7. Gossiping "works here" to pods where it does not exist creates false availability. Already caused a class of bug (staff-triggered broadcast to all pods) that needed gating to Tier 2+ confidence | Keep combo availability pod-local; only gossip fix KB entries (not availability) — already gated in `tier_engine.rs` |
| **Full AC config validation** — parse `data/car.ini`, `data/engine.ini`, etc. for every car in manifest | "Catch corrupt game installs" | ~500 cars x 10 files = 5000 file reads at boot; blocks rc-agent startup for 30-90s; 99% of corruption is at folder/AI-lines level, not ini contents | Validate folder existence + `data/` dir + `ai/` dir only (fast, covers 95% of launch failures per Game Doctor history) |
| **Automated combo disable** — when combo fails 3 times, auto-set `enabled: false` in DB | "Stop showing broken combos" | Auto-disabling without human review hides transient failures (network blip, pod reboot mid-session) as permanent. A combo with `enabled: false` from a bad day is invisible until a customer asks for it | Flag `flagged_unreliable: true` (already in `GamePresetWithReliability`) + surface in admin UI + alert staff; human makes the call |
| **Per-combo reliability push notifications** — WhatsApp every time a combo rolling success rate crosses a threshold | "Proactive alerting" | With 8 pods x 100+ AC combos, this generates dozens of alerts per session; alert fatigue kills the channel. WhatsApp is already used for crash loops and system health | Chain failure alerts (3+ consecutive fails on same pod+game, 10-min window) — actionable, not statistical |

---

## Feature Dependencies

```
Steam library scanner (all games)
    └──enables──> Per-pod game inventory (auto, not manual TOML)
                      └──enables──> Per-pod kiosk game filter (accurate)
                      └──enables──> Fleet game matrix dashboard column (accurate)

Non-functional combo flag (filesystem validation)
    └──requires──> ContentManifest per pod (exists for AC, needs validity extension)
    └──enables──> Flagged combos list in dashboard
    └──enhances──> Boot-time combo validation (flag result stored, not re-checked each launch)

Launch timeline tracing
    └──requires──> WS ACK protocol (v40.0 Phase 312 -- adds AgentAckReceived step)
    └──enhances──> Dynamic launch timeout (p95 computed from timeline data)
    └──enables──> Launch timeline viewer in dashboard

Chain failure WhatsApp alert
    └──requires──> launch_events table (exists)
    └──independent of crash_loop detection (different trigger: game fails, not agent crashes)

Reliability dashboard
    └──requires──> combo_reliability (exists)
    └──requires──> fleet game matrix data (per-pod installed_games, already in PodStatus)
    └──requires──> flagged combos API (new endpoint on top of existing flag column)
    └──optional──> launch timeline viewer (high complexity, can ship without)
```

### Dependency Notes

- **Per-pod kiosk filter requires Steam scanner:** Without auto-detection, `installed_games` is what is in TOML config — operators forget to update it. The filter would be inaccurate. Build scanner first, then filter.
- **Timeline viewer requires v40.0 WS ACK:** The `WsCommandSent` to `AgentAckReceived` step only exists after v40.0 ships (Phase 312 adds ACK protocol). Dashboard can ship without timeline viewer; add viewer as a later phase.
- **Combo flag is independent of Game Doctor:** Game Doctor runs at launch time. The filesystem validation for flag runs at boot time and stores result in DB. They share the same validation logic but are separate code paths.
- **Chain failure alert is independent of crash loop:** Crash loop = agent restarts (already exists). Chain failure = game process fails to reach playable state (new trigger). Both use WhatsApp but different detection logic.

---

## MVP Definition

### Launch With (Phase 1 -- Core Inventory)

Minimum to stop showing customers unplayable games.

- [ ] Steam library scanner for all 8 SimTypes — replace manual TOML `installed_games`
- [ ] Per-pod `ContentManifest` includes non-AC game presence (boolean installed, not full file tree)
- [ ] Server API: `GET /api/v1/games/inventory/{pod_id}` — returns installed games for that pod
- [ ] Kiosk game selector filters by pod installed games (not global catalog)

### Launch With (Phase 2 -- Combo Validation)

Minimum to stop showing unlaunchable AC combos.

- [ ] Boot-time combo validation — cross-reference active presets vs ContentManifest at rc-agent startup
- [ ] Filesystem validation checks: car folder exists, track folder exists, `ai/` subdir exists
- [ ] `combo_valid: bool` and `invalid_reason: Option<String>` added to `GamePresetWithReliability`
- [ ] Admin UI: flagged combos list (filter on `combo_valid: false`)

### Launch With (Phase 3 -- Observability)

Minimum to understand and alert on launch failures.

- [ ] Launch timeout watchdog per combo (90s default, configurable via racecontrol.toml)
- [ ] Chain failure detection: 3+ consecutive failures on same pod+game in 10 min
- [ ] Chain failure WhatsApp alert
- [ ] Reliability dashboard: combo success rates + fleet game matrix (8x8 grid) + flagged combos list

### Add After Validation (Phase 4 -- Intelligence)

After core is stable and operators are using the dashboard.

- [ ] Dynamic timeout per combo (computed from p95 of `duration_to_playable_ms`)
- [ ] Launch timeline tracing with step-level milestones (requires v40.0 ACK shipped + fleet deployed)
- [ ] Launch timeline viewer in dashboard

### Future Consideration (v42+)

Defer — requires either more data or adjacent milestones.

- [ ] Non-Steam iRacing inventory via registry-based detection (`HKLM\SOFTWARE\iRacing Sim Racing Simulator\InstallDir`)
- [ ] Predictive combo degradation — flag combos trending toward unreliability before they fail
- [ ] AI-powered fix suggestion from launch timeline data fed to Tier 3/4 MMA

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Per-pod kiosk game filter | HIGH | MEDIUM | P1 |
| Steam library scanner (all games) | HIGH | MEDIUM | P1 |
| Boot-time combo validation | HIGH | MEDIUM | P1 |
| Non-functional combo flag (filesystem) | HIGH | MEDIUM | P1 |
| Chain failure WhatsApp alert | HIGH | LOW | P1 |
| Launch timeout watchdog | MEDIUM | LOW | P2 |
| Reliability dashboard (combo heatmap + fleet matrix) | HIGH | HIGH | P2 |
| Dynamic timeout per combo | MEDIUM | MEDIUM | P3 |
| Launch timeline tracing + viewer | HIGH | HIGH | P3 |

**Priority key:**
- P1: Must have for milestone
- P2: Should have, included in scope
- P3: Add after core validated

---

## Competitor Feature Analysis

This is a custom-built venue management system. Reference points are existing tools in the sim racing ecosystem.

| Feature | SimHub / Garage61 | Commercial cafe POS (e.g. Antamedia) | Our Approach |
|---------|-------------------|--------------------------------------|--------------|
| Game inventory | Per-PC config file, manually maintained | Title-level enable/disable in admin | Auto-detect via Steam appmanifest at boot; rescan every 5 min |
| Combo validation | Not applicable (single-game tools) | Not applicable (no sim-specific combos) | Filesystem check at boot + flag in DB; AC-specific (car/track/ai dirs) |
| Launch observability | Structured telemetry log (SimHub) | None | `launch_events` table (exists); extend with step milestones in Phase 4 |
| Reliability tracking | None | Basic session success/fail | Per-combo `combo_reliability` table with rolling `success_rate` (exists) |
| Fleet visibility | None (single-PC) | Pod grid (connected/disconnected) | 8-pod game matrix: installed + working + reliability per game per pod |

---

## Implementation Notes (Complexity Qualifiers)

**Steam library scanner — MEDIUM, not HIGH:**
- `steam_checks.rs` already has the multi-path appmanifest lookup (C/D/E drives)
- For F1 25, iRacing, Forza, LMU: only need `appmanifest_{app_id}.acf` existence check
- No need to parse game content (no car/track trees for non-AC games)
- Risk: iRacing is not on Steam (standalone installer). Need registry-based fallback: `HKLM\SOFTWARE\iRacing Sim Racing Simulator\InstallDir`

**Boot-time combo validation — MEDIUM, not LOW:**
- Game Doctor has sync filesystem checks; safe to call from boot init path
- Risk: need to call ONLY the filesystem checks, not the process/WMI checks (those need a running game)
- Must not block rc-agent startup — run in `tokio::spawn` after init, write results to DB async
- Validation results written back to server via new WS message type; server updates `combo_valid` column

**Reliability dashboard — HIGH:**
- Multiple query types (combo heatmap, fleet matrix, flagged list, recent failures)
- Fleet game matrix requires joining `combo_reliability` with `PodStatus.installed_games` — different data sources (DB + in-memory state)
- New Next.js page in admin app — requires UI-SPEC.md and UI-REVIEW.md per subagent gate rules

**Launch timeline tracing — HIGH (hard dependency on v40.0):**
- `AgentAckReceived` step does not exist until v40.0 Phase 312 ships WS ACK protocol
- Do not start Phase 4 until v40.0 is confirmed shipped and fleet-deployed
- `launch_events` table needs new columns: `ws_sent_at`, `agent_ack_at`, `process_spawned_at`, `telemetry_first_at`
- DB migration required: `ALTER TABLE launch_events ADD COLUMN ws_sent_at INTEGER` etc.

**Chain failure detection — LOW, done at server level:**
- Query `launch_events` in rolling 10-min window per (pod_id, sim_type)
- Runs as a background task in racecontrol (similar to metrics alert thresholds)
- Reuses existing WhatsApp alert plumbing from `ws/mod.rs`

---

## Sources

- Codebase audit: `crates/rc-agent/src/content_scanner.rs`, `game_doctor.rs`, `game_launch_retry.rs`, `tier_engine.rs`
- Codebase audit: `crates/racecontrol/src/preset_library.rs`, `api/metrics.rs`, `fleet_health.rs`, `ws/mod.rs`
- Codebase audit: `crates/rc-common/src/types.rs` (SimType enum, PodStatus.installed_games, ContentManifest)
- Codebase audit: `crates/rc-agent/src/steam_checks.rs` (appmanifest multi-path logic)
- v41.0 milestone definition in `.planning/PROJECT.md` lines 68-88
- Prior anti-pattern documented in CLAUDE.md: staff-triggered broadcast requiring Tier 2+ gate
- Prior anti-pattern documented in CLAUDE.md: `.spawn().is_ok()` not meaning child started
- Prior anti-pattern documented in CLAUDE.md: single-fetch-at-boot without periodic retry

---
*Feature research for: v41.0 Game Intelligence System — Racing Point eSports*
*Researched: 2026-04-03*
