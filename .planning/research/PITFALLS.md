# Pitfalls Research

**Domain:** Adding E2E tests to an existing Rust/Next.js venue kiosk + game launch system (RaceControl v7.0)
**Researched:** 2026-03-19
**Confidence:** HIGH — all pitfalls are documented from bugs caught during real test development in this session, not generic advice. See milestone_context in the research prompt for the bug list these were derived from.

---

## Critical Pitfalls

### Pitfall 1: API Tests Pass While the Browser Crashes — JSX Render Errors Are Invisible to curl

**What goes wrong:**
The smoke test and game-launch test call API endpoints with curl and check HTTP status codes. Both return 200. The kiosk Next.js frontend renders a page that calls the same endpoints, encounters an unexpected field shape or null value, and crashes with a React render error or `TypeError: Cannot read properties of undefined`. The curl test passes; the kiosk is broken. This bug was hit during this session: kiosk crashed on a JSX render while all API tests were green.

**Why it happens:**
curl only validates that the HTTP response is 200 and optionally that the body is valid JSON. It cannot execute JavaScript, run React's rendering pipeline, or catch null-dereference in component code. A `response.data.tiers.filter(...)` that crashes when `tiers` is `undefined` is invisible to curl — the API returned JSON successfully, but the component assumed a field shape that wasn't guaranteed.

**How to avoid:**
Add a kiosk frontend smoke layer that fetches the actual HTML body and checks for error strings: `"internal server error"`, `"application error"`, `"unhandled runtime error"`, `"react error boundary"`. game-launch.sh Gate 8 already implements this. For stronger coverage, add Playwright browser tests that actually execute JavaScript and detect React crashes in the browser console. Never treat API HTTP 200 as proof the frontend renders correctly.

**Warning signs:**
- Test runner shows all API gates green but Uday reports blank/error screen in kiosk
- Next.js logs on server show `TypeError` or `Error: ...` in SSR output while HTTP returns 200
- `/kiosk/book` page body contains `"application error"` despite the API returning valid data

**Phase to address:** Phase 1 (Kiosk Frontend Smoke) — any phase writing API tests must add a corresponding frontend body-check test in the same phase, not as a separate later phase.

---

### Pitfall 2: Wrong API Endpoint — /pods Lacks ws_connected, /fleet/health Has It

**What goes wrong:**
A test needs to check whether a pod's agent is connected via WebSocket before attempting a game launch. The developer uses `GET /api/v1/pods` which returns a list of pods. The response does not include `ws_connected`. The test checks for the field, gets `undefined` or `false` for all pods, and either always skips the launch test or always marks the agent as disconnected. A different developer reads the API routes and finds `GET /api/v1/fleet/health` which includes `ws_connected: bool` in the `FleetPodHealth` struct. This exact bug was encountered in this session — the original test used `/pods`, which has no `ws_connected` field.

**Why it happens:**
Two endpoints exist for pod data. `/pods` returns operational pod state (billing, game state, lock screen status). `/fleet/health` returns infrastructure health (agent connectivity, minion reachability, agent version). They are different structs (`PodState` vs `FleetPodHealth`) and neither links to the other in the route comments. A developer finds the first one and assumes it is complete.

**How to avoid:**
Document the endpoint split explicitly in the test suite header. game-launch.sh Gate 4 already uses `/fleet/health` for the ws_connected check — preserve this. When adding new tests that need connectivity state, always use `/fleet/health`. Add a comment to the `/pods` endpoint response shape noting that `ws_connected` lives in `/fleet/health` not here. In any test that checks agent state, write the endpoint choice as a comment: `# ws_connected is in /fleet/health, NOT /pods`.

**Warning signs:**
- Test always skips "Pod agent connected" gate even when the pod is clearly online and serving requests
- `/pods` response body parsed in Python shows no `ws_connected` key
- `jq '.[] | .ws_connected' <(curl /api/v1/pods)` returns `null` for all pods

**Phase to address:** Phase 2 (API Pipeline Tests) — write the endpoint mapping table before writing any agent-connectivity test.

---

### Pitfall 3: Steam "Support Message" Dialog Blocks Game Launch Silently

**What goes wrong:**
A test launches a Steam game (F1 25, AC EVO, LMU, iRacing, Forza) via `steam://rungameid/{id}`. Steam starts, then shows a modal dialog (e.g., "Support Message", "News", "Product Activation Required", "Steam Update Available"). The game never appears. rc-agent's PID scanner does not find the game process within the scan interval. The game state transitions from `Launching` to `Error` with message "game did not start within timeout". The test fails. No error log explains the dialog. This exact bug was found in this session — a Steam "Support Message" dialog silently blocked F1 25 launch.

**Why it happens:**
Steam shows these dialogs before starting the game process. rc-agent launches via `steam://rungameid/{id}` using `cmd /C start`, hands off to Steam, and loses control of the process chain. The game process does not appear until the dialog is dismissed. rc-agent has no mechanism to detect or dismiss Steam dialogs. The timeout is a fixed constant, not adaptive.

**How to avoid:**
Add a per-game Steam dialog auto-dismissal step before the launch test. For F1 25: use `AutoHotkey` or a PowerShell `UIAutomation` script to dismiss known dialog patterns before the test or monitor and dismiss during the launch window. Alternatively, configure the pod's Steam client to suppress known dialogs (Steam startup settings, offline mode, or registry tweaks). In the test framework, log "checking for Steam dialogs" before marking a launch as failed — this surfaces the dialog in the failure output rather than hiding it as a timeout. For CI/automated runs, ensure pods are in Steam offline mode so network-triggered dialogs cannot appear.

**Warning signs:**
- Game launch test always times out on first run but succeeds if you click through Steam manually first
- rc-agent logs show `Launching` → `Error: game did not start within 30s` with no other error
- `tasklist | findstr steam` shows steamwebhelper.exe or GameOverlayUI.exe but no game exe
- The issue is specific to the first launch after pod reboot (dialog appears once, then Steam remembers dismissal)

**Phase to address:** Phase 3 (Per-Game Launch Validation) — dialog dismissal must be part of the pre-test setup, not discovered at test run time.

---

### Pitfall 4: Wrong Steam App ID — EA Anti-Cheat Wrapper Has a Different Store App ID

**What goes wrong:**
A test validates that a game can be launched by checking the `steam_app_id` in the config against the Steam store page. The developer looks up "F1 25" on the Steam store, finds app ID `2805550` (the store listing), and uses that to verify the launch config. The actual launch config in `rc-agent.toml` uses `3059520` (the EA Anti-Cheat bootstrapper that actually starts the game). The test marks the configured app ID as wrong and flags it as a defect. This exact bug was hit in this session — the EA Anti-Cheat wrapper uses a different ID than the store page.

**Why it happens:**
Games that use EA App or EA Anti-Cheat ship as two Steam entries: the store-visible game and an internal bootstrapper/launcher. The bootstrapper is what `steam://rungameid/{id}` must use — it sets up the EA runtime and starts the game. The store ID will open a Steam page but not launch the game correctly. This is not documented on the Steam store page. The correct app IDs must be found empirically (run the game from Steam client with debugging, check what process Steam actually launches) or from EA developer notes.

**How to avoid:**
Document the canonical app IDs in the test suite with a source: for F1 25, the launch ID is `3059520` (EA Anti-Cheat wrapper), not `2805550` (store page). Add a test comment citing this. When validating other games that use third-party launchers (Ubisoft Connect, EA App, Rockstar Launcher), always verify the launch app ID by watching `steam://rungameid/` calls in Steam's game launch log, not by looking up the store page. Keep a `GAME_IDS.md` reference in the test directory mapping game name → store ID → launch ID with notes on why they differ.

**Warning signs:**
- `steam://rungameid/{store_id}` opens the Steam store page instead of launching the game
- The game process never appears after launch despite Steam being open and responsive
- Steam shows "Launching..." for 1-2 seconds then does nothing

**Phase to address:** Phase 3 (Per-Game Launch Validation) — document the correct app ID for every game before writing the launch test; run it manually once to confirm.

---

### Pitfall 5: EADDRINUSE After Kiosk Deploy — Stale Node Process Holds Port 3300

**What goes wrong:**
A deploy test stops and restarts the kiosk. The test sends the restart command, waits 5 seconds, then checks that port 3300 is serving. The new kiosk process fails to start because the old `node` process did not fully exit — it is still holding port 3300 in `CLOSE_WAIT` or `TIME_WAIT`. The kiosk crashes with `Error: listen EADDRINUSE 0.0.0.0:3300`. The kiosk is now down. Uday sees a blank screen. This exact bug was hit in this session.

**Why it happens:**
`next start` forks a Node.js HTTP server process. When the deploy sends a kill signal, the parent process exits but the HTTP server's TCP socket enters TIME_WAIT (for graceful close). On Windows with `pm2 restart` or `taskkill /F`, the socket is forcibly closed but can remain in CLOSE_WAIT if there are active connections. The subsequent `next start` tries to bind the same port and fails. The deploy script's 5-second wait is not enough for TIME_WAIT to expire (default 60-240s on Windows).

**How to avoid:**
Before each kiosk restart in a test, verify the port is free: `netstat -ano | findstr :3300` should show no entries, or all entries should be in TIME_WAIT only. If CLOSE_WAIT entries exist, kill the owning PID first. Add a port-free check to the deploy verification gate: loop for up to 30 seconds checking that port 3300 is bindable before declaring the deploy failed. In the kiosk deployment scripts, add `pm2 delete kiosk && sleep 3 && pm2 start kiosk` rather than `pm2 restart kiosk` to ensure the process fully exits. Alternatively, configure Node to use `SO_REUSEADDR` (Next.js supports `--port` with graceful restart in newer versions).

**Warning signs:**
- Kiosk deploy test reports "port check passed" (old process still responded) then the new process crashes
- Server logs show `Error: listen EADDRINUSE :::3300` immediately after restart
- `netstat -ano | findstr 3300` on the server shows a PID that is no longer in `tasklist`
- Deploy test passes but kiosk is unreachable 30 seconds later (race condition — new process started then crashed)

**Phase to address:** Phase 4 (Deploy Verification) — add port-free verification as step 0 of the deploy test sequence, before starting the new process.

---

### Pitfall 6: Stale Game Tracker Stuck in "Stopping" State Blocks Subsequent Test Runs

**What goes wrong:**
A test launches a game, then sends a stop command. The game process terminates but rc-agent's in-memory game tracker does not clear — it stays in `GameState::Stopping` because the post-stop cleanup message was lost (e.g., the agent restarted between launch and stop, or the WebSocket message was dropped). The next test checks `GET /games/active`, finds the game in `Stopping` state, and either auto-cleanup fails (the process is already dead so `taskkill` returns error) or the double-launch guard blocks the next launch. The test suite hangs waiting for cleanup that will never complete. This exact bug was hit in this session.

**Why it happens:**
The game tracker is in-memory in rc-agent. If rc-agent restarts for any reason (including the test's own cleanup), the in-memory state resets but the server (racecontrol) still holds a stale tracker entry it received over WebSocket before the restart. The server only clears the tracker when the agent sends an explicit `GameStopped` message — which it cannot send if it restarted before cleanup ran. The tracker is now permanently `Stopping` until the next rc-agent restart and reconnect that sends fresh state.

**How to avoid:**
Add a stale-tracker detection and reset step to the test pre-flight. game-launch.sh Gate 5 already has auto-cleanup logic: if a game is found in any state for the test pod, send a stop command and wait. Extend this to also handle the `Stopping` state that never clears: if the game is in `Stopping` for more than 10 seconds, send `POST /games/force-clear` or restart the agent via the remote_ops endpoint to reset in-memory state. Add a maximum wait loop (not an indefinite sleep). Also: add a test teardown hook that always fires regardless of test outcome, to prevent the next test from inheriting stale state.

**Warning signs:**
- Test suite second run always fails on "Game already running" or "double-launch guard" even though no test is active
- `/games/active` returns a game entry with `state: "stopping"` and a timestamp from >10 seconds ago
- Agent logs show no recent `taskkill` output for the game process name
- `tasklist | findstr [game.exe]` on the pod shows no matching process (it's already dead)

**Phase to address:** Phase 2 (API Pipeline Tests) — pre-flight cleanup and post-run teardown hooks must be designed before any stateful test is written.

---

### Pitfall 7: Kiosk Wizard Shows AC-Specific Steps for Non-AC Games

**What goes wrong:**
The wizard step flow is controlled by `getFlow()` in `useSetupWizard.ts`. If `selectedGame` is not `"assetto_corsa"`, AC-only steps (`session_splits`, `player_mode`, `session_type`, `ai_config`, `select_track`, `select_car`, `driving_settings`) are removed from the flow. A test uses Playwright to click through the wizard for F1 25 and verifies the steps shown match the expected non-AC flow (register → plan → game → experience → review). If a code change accidentally makes `isAc` evaluate to `true` for non-AC games (e.g., a typo in the game ID comparison, or a new game added with a name starting with "assetto"), the wizard renders AC steps for that game. Customers see a track/car selector for F1 25 that makes no sense and cannot proceed. This exact bug was found in this session — wizard showed AC-specific options for F1 25, only caught by manual testing.

**Why it happens:**
The `isAc` check is a string equality: `state.selectedGame === "assetto_corsa"`. Any change to game IDs, renaming of existing games, or adding a game whose ID contains "assetto" as a prefix risks breaking this check. There is no test that verifies the step sequence for each supported game. API tests cannot catch this because the wizard logic is pure client-side JavaScript.

**How to avoid:**
Write a Playwright test that clicks through the wizard for each supported game and asserts that the exact steps rendered match the expected flow for that game. The test matrix is: 6 non-AC games (F1 25, AC EVO, AC Rally, iRacing, LMU, Forza) → expected flow `[register_driver, select_plan, select_game, select_experience, review]`; AC → expected flow with AC steps. The test should assert step presence and absence — not just that the wizard completes, but that specific steps do not appear for the wrong game type.

**Warning signs:**
- Manual test reveals "Select Track" or "Select Car" step appears when selecting F1 25 in the wizard
- React renders the `select_track` or `driving_settings` step body during a non-AC game flow
- Playwright screenshot shows a track dropdown on the game configuration screen for Steam games

**Phase to address:** Phase 1 (Playwright Browser Tests) — the per-game wizard flow test is the first priority; it cannot be deferred to a later phase.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| API-only tests (curl smoke) without browser tests | Fast to write, no Playwright dependency | JSX render errors invisible; wizard step bugs undetected | Never for frontend-heavy flows like the booking wizard |
| Shared test state across test runs (no teardown hooks) | Simpler test code, no cleanup boilerplate | Stale game trackers poison subsequent runs; tests become order-dependent | Never — always add teardown even if it no-ops when nothing is dirty |
| Using the first endpoint that returns pod data without verifying field presence | Tests written faster | ws_connected read from wrong endpoint; test always shows agents as disconnected | Never — verify field schema before writing assertions |
| Hardcoding Steam app IDs from store page without running a launch | Avoids manual testing step | Wrong IDs for EA/Ubisoft-wrapped games; launch test never actually launches | Never — always verify by running the actual launch once |
| Single fixed sleep (e.g., `sleep 5`) instead of polling loop for service readiness | Simpler code | Intermittent failures on slow days; false positives on fast days | Only for local-only dev smoke tests, never in the main E2E suite |
| Testing only the happy path (game launches, runs, stops cleanly) | Faster to write | Stale state bugs, Steam dialogs, port conflicts only appear in real-world sequences | Never — always test at least one cleanup failure path per gate |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Steam game launch via `rungameid` | Use store page app ID | Verify launch app ID empirically; EA Anti-Cheat wrapper uses different ID than store listing |
| Steam dialogs during launch | Assume game starts immediately after `rungameid` | Add dialog auto-dismissal step or set Steam to offline mode before launch test |
| `/api/v1/pods` for agent connectivity check | Read `ws_connected` from `/pods` | `ws_connected` is only in `/api/v1/fleet/health` — document endpoint split explicitly |
| Kiosk port 3300 on deploy | Kill old process then immediately start new | Poll for port free (up to 30s) before starting new process; EADDRINUSE possible in TIME_WAIT |
| Game tracker state on agent restart | Assume server state reflects agent state after restart | Agent in-memory state resets on restart but server holds stale tracker; add stale-state cleanup to pre-flight |
| Next.js SSR + React render errors | Trust HTTP 200 from kiosk pages | Check page body for error strings; HTTP 200 does not mean the page rendered without a JavaScript crash |
| Wizard step flow per game | Test only that wizard completes | Assert exact step sequence for each game type; AC steps must not appear for Steam games |
| rc-agent `Stopping` → `Running` transition | Poll until game state is "running" | Always handle `Stopping` stuck state explicitly; add max retry count before force-clear |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Launching all 6 games in a sequential test suite without cleanup between | Games accumulate in memory, port conflicts, UDP ports collide | Stop + verify clean after every game before launching the next; check `games/active` returns empty | After 2nd game in sequence if first stop was not verified |
| Running game-launch tests in parallel across multiple pods | Two tests write to the same server-side game tracker, race condition | Assign one pod per test worker; never share a pod between parallel test runs | Immediately on multi-pod parallel run |
| Waiting for full game startup (game reaches `Running`) for every test | F1 25 takes 45-90s to reach Running on slow pod; test suite wall-clock becomes 10+ minutes | Accept `Launching` state as success for Steam games that have long startup; verify process existence separately | On any test with a 30s timeout |
| Test suite runs on the venue server during business hours | Real customer sessions interfere with test state; billing sessions appear in "active" that the test didn't create | Schedule automated test runs during off-hours (after 23:00); or add pod isolation (reserve pod 8 for testing) | Any time a real customer is on pod 8 during a test run |

---

## "Looks Done But Isn't" Checklist

- [ ] **API smoke passes:** Also verify kiosk frontend body has no error strings — `curl /kiosk/book | grep -i "application error"` returns nothing
- [ ] **Game launch test passes:** Verify the correct game process name appeared in `tasklist` on the pod — `launch accepted` does not mean the game is actually running
- [ ] **Steam app ID documented:** Confirm the configured `steam_app_id` is the launcher ID, not the store page ID — run `steam://rungameid/{id}` manually once and watch what process starts
- [ ] **Wizard flow tested per game:** Each of the 6 supported non-AC games has been clicked through in Playwright — `select_experience` appears, `select_track` does NOT appear
- [ ] **Cleanup verified:** After each test run, `GET /games/active` returns an empty list — no stale trackers left in `Stopping` state
- [ ] **Port 3300 free after deploy test:** `netstat -ano | findstr 3300` shows no CLOSE_WAIT entries after kiosk restart — new process can bind
- [ ] **Steam dialog check:** Game launch test was run on a fresh pod reboot at least once — Steam "Support Message" dialog was dismissed or confirmed not to appear
- [ ] **Cross-service test:** `/fleet/health` was used for ws_connected checks, not `/pods` — confirmed by grepping the test source for the endpoint URL

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| JSX crash missed by curl test | MEDIUM | Add Playwright test for affected page; fix the null-safety bug in the component; add body-check to smoke test |
| Wrong endpoint for ws_connected | LOW | Update test to use `/fleet/health`; add comment citing the endpoint split decision |
| Steam dialog blocks launch | LOW | Dismiss dialog manually on pod; put Steam in offline mode; add pre-test dialog-check step |
| Wrong Steam app ID | LOW | Find correct launcher ID by watching `steam://rungameid/` call in Steam log; update `rc-agent.toml` and test documentation |
| EADDRINUSE after deploy | MEDIUM | `taskkill /F /PID {pid}` of stale node process; wait 10s; restart kiosk; add port-poll loop to deploy test |
| Stale game tracker in Stopping | LOW | `POST /games/stop` with `force: true` if available, or restart rc-agent on the pod; add teardown hook to test |
| AC wizard steps appear for non-AC game | MEDIUM | Fix `isAc` check in `useSetupWizard.ts`; add Playwright step-assertion test per game; run test against staging before shipping |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| API tests pass but frontend crashes | Phase 1: Kiosk Frontend Smoke | `curl /kiosk/book` body contains no error strings; Playwright renders booking page without console errors |
| Wrong endpoint for ws_connected | Phase 2: API Pipeline Tests | `grep fleet/health tests/e2e/game-launch.sh` finds the ws_connected check; no `/pods` references for connectivity |
| Steam dialog blocks launch | Phase 3: Per-Game Launch Validation | Launch test succeeds on first run after pod reboot; Steam dialog handling documented in test header |
| Wrong Steam app ID | Phase 3: Per-Game Launch Validation | Each game's launch ID verified manually and documented in GAME_IDS.md before automated test runs |
| EADDRINUSE after kiosk deploy | Phase 4: Deploy Verification | Deploy test includes port-free poll loop; `netstat` check in test teardown shows no stale 3300 entries |
| Stale game tracker in Stopping | Phase 2: API Pipeline Tests | Pre-flight cleanup loop handles `Stopping` state; post-run teardown asserts `/games/active` empty |
| AC wizard steps appear for non-AC game | Phase 1: Playwright Browser Tests | Per-game step-sequence assertions for all 6 non-AC games; test explicitly asserts `select_track` NOT present |

---

## Sources

- Direct observation: All 7 pitfalls were encountered during real test development in this session (2026-03-19). No speculative bugs — each pitfall has a corresponding real failure.
- game-launch.sh (tests/e2e/) — Gate 4 (fleet/health endpoint), Gate 5 (stale game cleanup), Gate 8 (kiosk frontend smoke) — these gates exist specifically because the underlying pitfall was already hit
- useSetupWizard.ts (kiosk/src/hooks/) — `isAc` check logic, per-game step filtering
- game_process.rs (crates/rc-agent/src/) — Steam `rungameid` launch path, no dialog handling
- deploy.rs (crates/racecontrol/src/) — steam_app_id values for each game (`f1_25 = 3059520`, not the EA store ID `2805550`)
- fleet_health.rs (crates/racecontrol/src/) — `ws_connected` only in `FleetPodHealth`, not in pod list endpoint
- Milestone context: 7 real bugs found during v7.0 test development (see REQUIREMENTS.md comments)

---
*Pitfalls research for: E2E Test Suite for Rust/Axum + Next.js venue kiosk + game launch pipeline (RaceControl v7.0)*
*Researched: 2026-03-19*
