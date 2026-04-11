# Pre-flight Findings — 2026-04-12 02:49 IST

**Operator:** James (autonomous, user sleeping)
**Scope:** Pod 4 instrumentation pre-flight for F1 25 staff-launch flow trace
**Target flow:** `.planning/debug/flow-traces/f1_25-staff-launch.md`
**Parent run:** `runs/2026-04-12T02-49-07-IST-preflight/`

---

## Observation Channel Validation

| Channel | Target | Status | Evidence |
|---|---|---|---|
| P1 — rc-agent log tail | Pod 4 `C:\RacingPoint\rc-agent-.2026-04-11.jsonl` | ✅ WORKING | `powershell -NoProfile Get-Content <path> -Tail N` via `/exec` |
| P2 — DXGI screenshot | Pod 4 `GET :8090/screenshot?method=dxgi` | ✅ WORKING | HTTP 200, 186750 byte JPEG, 7680x1440 resolution — see `observations/pod4-hop0-idle-dxgi.jpg` |
| P3 — tasklist | Pod 4 `/exec tasklist /V /FO CSV` | ✅ WORKING | Full process list including window titles returned |
| P4 — server log tail | Server .23 racecontrol jsonl | ⏳ NOT YET TESTED | Deferred — server host, needs separate validation |
| P5 — go2rtc camera | NVR camera on Pod 4 display | ⏳ NOT TESTED | Demoted to tertiary per user's subtle-diff concern |

**Primary observation stack is validated.** P4 can be stood up when needed; P5 is backup only.

---

## Hop 0 — Pod 4 idle baseline (2026-04-12 02:49 IST)

**Pod 4 rc-agent `/health` (direct from .88:8090):**
```json
{"bat_sha256":"d59ea5c4dbcf8753dd58befa3a7b043212edfcf44dc89381bc454220291789f9",
 "binary_sha256":"17a4f5c43f1e6f3c6501f39229d424a3667dcb07d06d35e3d792e9f1dd852cb5",
 "build_id":"b1fc9484",
 "exec_slots_available":8,"exec_slots_total":8,
 "status":"ok","uptime_secs":5128,"version":"0.1.0"}
```

**Pod 4 from server `/api/v1/fleet/health`:**
- `screen_blanked: true` — IDLE-01 lock-screen fix is **visibly working right now**
- `http_reachable: true`, `ws_connected: true`
- `windows_session_id: 1` (Console, not Services)
- `in_maintenance: false`, `crash_loop: false`, `crashes_last_hour: 0`
- `experience_score: 100`, `violation_count_24h: 0`
- `avg_ready_delay_ms: 120000.0` — anomalous vs. neighbours at 30000; parking
- `clock_drift_secs: 4` — parking

**Pod 4 screenshot via DXGI:** `observations/pod4-hop0-idle-dxgi.jpg`
- 7680x1440, triple-monitor NVIDIA Surround intact
- Black background with centred Racing Point eSports logo (red speedometer + car silhouette)
- Faint red particle effect in lower half — the animated blank-state painter
- **NOT the PIN grid** — confirms IDLE-01 fix is live
- (Visual) No game window, no launcher window, no Edge kiosk overlay

**Pod 4 Console-session processes (filtered highlights from `tasklist /V /FO CSV`):**
- `rc-agent.exe` PID 19676, Session 1, window title **"Racing Point Lock Screen"** ✅ native Win32 painter active
- `steam.exe` PID 9068 + 6× `steamwebhelper.exe` — Steam client up and idle, no game
- No `F1_25.exe`, no `acs.exe`, no `iRacingSim64DX11.exe`, no `LMU.exe`, no `AssettoCorsaEVO.exe`
- `explorer.exe` Session 1 ✅ (NVIDIA Surround preserved)
- `ConspitLink2.0.exe` × **4 instances** (PIDs 21392, 9820, 3428, 7564) — **anomaly, singleton rule violated**, parking
- `VSD Craft.exe` 228 MB — Gigabyte monitoring, historical Pod 6 crash suspect, parking
- 5× `node20.exe` — diagnostic engine workers
- `rc-sentry.exe` Session 0 ✅

---

## Divergence candidates discovered during pre-flight

### Candidate 1 — F1 25 game_id namespace mismatch (LIVE LEAD for Bug 2)

**Severity:** HIGH — possible root cause for "F1 25 not spawning" half of Problem A + Bug 2.

**Evidence chain:**

1. Pod 4 `C:\RacingPoint\rc-agent.toml` defines F1 25 as:
   ```toml
   [games.f1_25]
   exe_path = "C:\\Program Files (x86)\\Steam\\steamapps\\common\\F1 25\\F1_25.exe"
   steam_app_id = 3059520
   use_steam = true
   ```
   → TOML `game_id = "f1_25"`, Steam app 3059520.

2. `crates/rc-agent/src/content_scanner.rs:110` hardcoded map:
   ```rust
   const STEAM_APP_IDS: &[(u64, SimType, &str, &str)] = &[
       (244210, SimType::AssettoCorsa, "assetto_corsa", "Assetto Corsa"),
       (2488620, SimType::F125, "f1_25", "F1 25"),
       (3059520, SimType::F125, "f1_25_ac", "F1 25 (Anti-Cheat)"),
       (266410, SimType::IRacing, "iracing", "iRacing"),
   ];
   ```
   → Inventory scanner maps Steam app 3059520 to `game_id = "f1_25_ac"` (NOT `f1_25`).

3. Pod 4 has `appmanifest_3059520.acf` but **NO** `appmanifest_2488620.acf`.
   → Pod 4 inventory will emit exactly one F1 25 entry with `game_id = "f1_25_ac"`.

4. Rc-agent log: `"Inventory rescan: 4 games found"` every 5 min.

**Hypothesis:** When staff fires F1 25 launch via kiosk staff page, the kiosk/server sends `LaunchGame { game_id: "f1_25", ... }` or similar. Rc-agent looks up `f1_25` in:
- TOML `[games.f1_25]` → finds exe_path + app_id (→ dispatches Steam URI `steam://rungameid/3059520`)
- Inventory (for validation / state tracking) → `f1_25` not found, only `f1_25_ac`

Depending on which path is authoritative:
- If TOML wins for dispatch: Steam URI fires correctly BUT server's `game_state` tracker uses inventory's `f1_25_ac`, server and rc-agent disagree about what's running → phantom `game_state=running`, stuck Launching, etc.
- If inventory wins for dispatch: launch is silently rejected or dispatched to wrong adapter
- If both must match: launch rejected with "game not in inventory"

**Class of bug:** Cross-boundary serialization / field-name mismatch — exact pattern of Pod 8 session type incident (`feedback_pod8_session_type_incident.md`) and AI difficulty wizard bug. Log says "launch OK" while the wrong thing (or nothing) actually runs.

**Verification needed in Phase 4 TRACE:** grep launch dispatch path for where `game_id` is compared — ws_handler.rs `LaunchGame` handler, game_process.rs dispatch, server-side launch state machine. Find which source is authoritative and trace a real launch to see where divergence happens.

**Fix sketch (do NOT commit without approval):** Either (a) change STEAM_APP_IDS so `3059520 → ("f1_25", "F1 25")` and delete the separate `f1_25_ac` entry (F1 25 only ships as the anti-cheat version — 2488620 is legacy), or (b) add a TOML field `inventory_game_id = "f1_25_ac"` that explicitly aliases TOML key to inventory key. Option (a) is simpler; option (b) preserves the anti-cheat distinction if it matters elsewhere.

---

### Candidate 2 — Assetto Corsa EVO and Le Mans Ultimate NOT in STEAM_APP_IDS

**Severity:** MEDIUM — affects two configured games, may or may not manifest as launch failures depending on same dispatch-path question as Candidate 1.

**Evidence:**

Pod 4 `rc-agent.toml` configures:
- `[games.assetto_corsa_evo]` steam_app_id = 3058630
- `[games.le_mans_ultimate]` steam_app_id = 2399420

Pod 4 Steam appmanifests confirm both are installed:
- `appmanifest_3058630.acf` ✅
- `appmanifest_2399420.acf` ✅

STEAM_APP_IDS does NOT contain 3058630 or 2399420. Inventory scanner will not emit `assetto_corsa_evo` or `le_mans_ultimate` game_ids even though they're configured and installed.

**Knock-on:** same bug class as Candidate 1. A "Launch ACE" or "Launch LMU" click from staff kiosk may have the same dispatch / inventory disagreement.

**Fix sketch:** Add both to STEAM_APP_IDS:
```rust
(3058630, SimType::AssettoCorsaEvo, "assetto_corsa_evo", "Assetto Corsa EVO"),
(2399420, SimType::LMU, "le_mans_ultimate", "Le Mans Ultimate"),
```
(Requires SimType enum variants for ACE + LMU — which already exist per session memory, verified by sims/ module tree.)

---

## Non-trace findings (parking for user awareness)

### P0 — OpenRouter API key leaked in Pod 4 rc-agent.toml

`C:\RacingPoint\rc-agent.toml` on Pod 4 contains:
```toml
[ai_debugger]
enabled = true
ollama_url = "http://192.168.31.27:11434"
ollama_model = "qwen2.5:3b"
openrouter_api_key = "sk-or-v1-<REDACTED — see standing rule, rotate IMMEDIATELY>"
openrouter_model = "openrouter/auto"
```

**This violates:**
- "NEVER hardcode key here — OpenRouter auto-revokes keys found in LLM prompts" (CLAUDE.md)
- "MMA bootstrap is env-only" standing rule
- Pre-commit hook credential-leak protection (pod deploy bypassed the hook for raw toml sync)

**The key is now in this conversation's tool output.** OpenRouter's key-scanner may revoke it automatically within hours of this conversation reaching their training ingest. User should:

1. Rotate the key immediately (`openrouter.ai/settings/keys`)
2. Move the new key to an env var (`RCAGENT_OPENROUTER_KEY`) or to the `[mma]` section of `racecontrol.toml` (per MMA-21 standing rule)
3. Strip `openrouter_api_key` from ALL pod `rc-agent.toml` files in `deploy/configs/`
4. Re-deploy config to all 8 pods
5. Add grep check to deploy-pod.sh / stage-release.sh to block future commits with the `sk-or-` prefix

**I did NOT write the key to any file under `runs/` — it is only present in this conversation's in-memory tool output buffer.** Rotating the key now invalidates the leak even if this conversation is later logged.

### P2 — `[preflight] enabled = false` and `[process_guard] enabled = false` on Pod 4

Comment in the TOML explains: *"Pre-release testing toggles (2026-04-11). Disabled to allow all software during pre-release testing. Re-enable (set enabled = true, remove this block) before public launch."*

**Implication for trace:** Pre-flight checks are OFF on Pod 4. Hypothesis H9 (pre-flight blocks launch) from the flow-trace doc is **NOT applicable right now** — there's no pre-flight to block anything. Hypothesis can be dropped from the F1 25 trace plan.

**Action for user:** Schedule re-enable of both guards before venue opens. Not a trace blocker.

### P3 — ConspitLink multiplication on Pod 4

4 instances of `ConspitLink2.0.exe` running (PIDs 21392, 9820, 3428, 7564). Memory standing rule says ConspitLink must be singleton (taskkill-all-before-start-one in start-rcagent.bat). Multiple instances cause steering-wheel FFB flicker.

**Not a trace blocker** but real infra drift. Parking — user sees when they wake up.

### P4 — Parallel F-01/F-02 session is LOW contamination

rc-agent log shows `EXEC BLOCKED: shell metacharacters in 'netstat -ano | findstr ...'` every ~2 min from another Claude Code session probing Pod 4's `/exec`. The commands are being **rejected** by the metachar filter, so pod state does not change — only log WARN lines are added.

**Trace impact:** Log pattern matching must filter out `EXEC BLOCKED` entries so they don't pollute signal. The parallel session is not actively fixing anything (idempotency guardrail in tier-engine also blocks any "Duplicate fix detected"). Contamination classification: **noise, not state change**.

---

## Channels + tooling proven in pre-flight

### Working /exec invocation pattern (via file to avoid shell escaping)

```bash
# Write JSON payload to file
cat > C:/Users/bono/tmp/exec_cmd.json << 'EOF'
{"cmd":"powershell -NoProfile Get-Content C:\\RacingPoint\\rc-agent-.2026-04-11.jsonl -Tail 30"}
EOF

# POST via curl
curl -s --connect-timeout 10 \
  -X POST http://192.168.31.88:8090/exec \
  -H "Content-Type: application/json" \
  -d @C:/Users/bono/tmp/exec_cmd.json
```

**Constraints:**
- `/exec` blocks shell metacharacters: `&`, `|`, `>`, `<`, `` ` ``, `;`, `^`
- No piping — use PowerShell `Select-Object`, `Where-Object`, etc. OR do multiple calls and filter client-side
- PowerShell commands must not use `-Command "..."` wrapper — outer quoting is mangled by cmd.exe layer
- Paths without spaces work without quotes; paths with spaces need `'single quotes'` inside the PowerShell command
- Bash JSON strings WILL mangle backslashes — always write to file, never `-d '{"cmd":"..."}'`

### DXGI screenshot

```bash
curl -s -o screenshot.jpg \
  "http://192.168.31.88:8090/screenshot?method=dxgi&quality=40&scale=30"
```
- Returns JPEG of 7680x1440 (triple monitor)
- `method=dxgi` captures GPU framebuffer — works through D3D exclusive fullscreen (unlike GDI)
- `scale=30` gives ~186 KB files (good for rapid hop-capture without storage blowup)
- `quality=40` is acceptable for pattern matching; use 80 for visual inspection

---

## Exit state for pre-flight

- All findings saved to this FINDINGS.md file
- Screenshot P2 validation saved to `observations/pod4-hop0-idle-dxgi.jpg`
- Hand-off doc for user at `HAND-OFF.md` (next artifact)
- No code changes made, no commits, no state mutations on server / pods / cloud
- Pod 4 still idle, still in blanked state, still ready for a real launch trace when user fires it
- Parallel F-01/F-02 session: not contacted (no channel to do so quietly); noise-only contamination documented
