# Racing Point eSports — Project Context

## Project Identity

- **Repo:** racecontrol — Rust/Axum + Next.js monorepo (`C:\Users\bono\racingpoint\racecontrol`)
- **James Vowles** — on-site operations AI, james@racingpoint.in, GitHub: james-racingpoint
- **Bono** — partner AI on VPS (srv1422716.hstgr.cloud), bono@racingpoint.in
- **Uday Singh** — boss, usingh@racingpoint.in. Goal: automate so he can be with his daughter.
- **Timezone:** Always IST (UTC+5:30) for all timestamps. **WARNING:** Rust `tracing` logs are in UTC. When reading racecontrol JSONL logs, always convert: `UTC + 5:30 = IST`. Misreading UTC as IST caused "5 unexplained restarts" to be reported when only 1 was real (post-reboot) and 4 were our own deploys.

---

## Network Map

| Device | IP | MAC | Notes |
|--------|----|-----|-------|
| Pod 1 | 192.168.31.89 | 30-56-0F-05-45-88 | Tailscale: sim1-1 / 100.92.122.89 |
| Pod 2 | 192.168.31.33 | 30-56-0F-05-46-53 | Tailscale: sim2 / 100.105.93.108 |
| Pod 3 | 192.168.31.28 | 30-56-0F-05-44-B3 | Tailscale: sim3 / 100.69.231.26 |
| Pod 4 | 192.168.31.88 | 30-56-0F-05-45-25 | Tailscale: sim4 / 100.75.45.10 |
| Pod 5 | 192.168.31.86 | 30-56-0F-05-44-B7 | Tailscale: sim5 / 100.110.133.87 |
| Pod 6 | 192.168.31.87 | 30-56-0F-05-45-6E | Tailscale: sim6 / 100.127.149.17 |
| Pod 7 | 192.168.31.38 | 30-56-0F-05-44-B4 | Tailscale: sim7 / 100.82.196.28 |
| Pod 8 | 192.168.31.91 | 30-56-0F-05-46-C5 | Tailscale: sim8 / 100.98.67.67 |
| Server | 192.168.31.23 | 10-FF-E0-80-B1-A7 | Racing-Point-Server, 64GB RAM, Tailscale: 100.125.108.37 (james@ node), Node v24.14.0 |
| James | 192.168.31.27 | D8-BB-C1-CD-B3-CF | RTX 4070, static IP, Ollama :11434, Node v22.22.0, go2rtc :1984 |
| POS PC | 192.168.31.20 | 10-4A-7D-5B-C4-DA | WiFi, Tailscale: pos1/100.95.211.1 |
| Spectator | 192.168.31.200 | 00-E0-4C-77-77-DF | WiFi, DeskIn: 712 906 402 |
| Router | 192.168.31.1 | | |
| NVR | 192.168.31.18 | | Dahua 13x cameras |

---

## Crate Names and Binary Naming

| Crate dir | Cargo name | Binary | Role |
|-----------|-----------|--------|------|
| `crates/racecontrol/` | `racecontrol` | `racecontrol.exe` | Server, port 8080 |
| `crates/rc-agent/` | `rc-agent` | `rc-agent.exe` | Pod agent, port 8090 |
| `crates/rc-common/` | `rc-common` | (lib only) | Shared types |

- NEVER call the server "rc-core" in conversation. Crate dir name only.
- Server config: `C:\RacingPoint\racecontrol.toml` (NOT `C:\RaceControl\`)
- Server starts via `start-racecontrol.bat` → HKLM Run key on server
- Pods start via `start-rcagent.bat` → HKLM `Run\RCAgent` key on each pod
- Cargo PATH: `export PATH="$PATH:/c/Users/bono/.cargo/bin"`
- Build commands:
  - `cargo build --release --bin rc-agent`
  - `cargo build --release --bin racecontrol`
  - Tests: `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`

---

## Server Services

| Service | Port | Location | Start |
|---------|------|----------|-------|
| racecontrol | 8080 | Server .23 | `start-racecontrol.bat` (HKLM Run). Build: `0c0c8134` |
| server_ops | 8090 | Server .23 | Part of racecontrol binary |
| kiosk | 3300 | Server .23 | Scheduled task |
| web dashboard | 3200 | Server .23 | Scheduled task |
| rc-agent | 8090 | All pods | `start-rcagent.bat` (HKLM Run). Build: `0c0c8134` |
| rc-sentry | 8091 | All pods | `start-rcsentry.bat` (HKLM Run). Build: `0c0c8134` |
| go2rtc | 1984 | James .27 | `go2rtc.exe` — 29 RTSP streams, API on :1984 (NOT 8096) |
| comms-link relay | 8766 | James .27 | `start-comms-link.bat`, Task Scheduler every 2min watchdog |
| AI healer | — | James .27 | `rc-watchdog.exe` via `CommsLink-DaemonWatchdog` task, 10 services, Ollama diagnosis |
| webterm | 9999 | James .27 | `python C:/Users/bono/racingpoint/deploy-staging/webterm.py` |
| Ollama | 11434 | James .27 | qwen2.5:3b + llama3.1:8b — venue-only |
| rc-sentry-ai | — | James .27 | Face detection on 3 cameras (cam2, cam9, entrance) |
| Cloud racecontrol | 8080 | Bono VPS | pm2 `racecontrol`. Build: `129a24f2` |
| Cloud comms-link | 8765 | Bono VPS | pm2 `comms-link` — WS server |

---

## Fleet Endpoints

- `GET http://192.168.31.23:8080/api/v1/fleet/health` — array of PodFleetStatus
  - Fields: `pod_number`, `ws_connected`, `http_reachable`, `version`, `build_id`, `uptime_secs`, `last_seen`
  - Filter by `pod_number` field (NOT array index)
- `POST http://192.168.31.23:8080/api/v1/fleet/exec` — remote exec via rc-agent :8090
- Cloud sync: pull/push every 30s. Cloud authoritative: drivers, pricing. Local authoritative: billing, laps, game state.

---

## Standing Rules

### Ultimate Rule

**Before marking ANY milestone or phase as shipped, run all three verification layers:**

```bash
# 1. Quality Gate — automated tests (contract + integration + syntax)
cd C:/Users/bono/racingpoint/comms-link && COMMS_PSK="..." bash test/run-all.sh

# 2. E2E — live round-trip verification
curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"node_version"}'   # single exec
curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[{"command":"node_version"}]}'  # chain
curl -s http://localhost:8766/relay/health   # health + connection mode

# 3. Standing Rules — check compliance (auto-push, Bono synced, watchdog running, rules categorized)
```

**All three must pass. No exceptions. No "I'll verify later."**
_Why: v18.0 shipped with 8 integration bugs that 135 unit tests missed. Every bug was caught only by manual E2E after deploy. This rule ensures automated + live + compliance verification happens BEFORE shipped._

**4. Visual verification for display-affecting deploys:**
Any change that touches lock screen, Edge kiosk, overlay, blanking, or browser launch MUST include a visual check — ask the user "are the screens showing correctly?" BEFORE marking shipped. Build IDs, fleet health, and cargo tests cannot catch flicker, misalignment, or rendering issues. Do NOT declare "PASS" from terminal output alone when the change affects what customers see.
_Why: v17.0 browser watchdog caused screen flicker on all pods (kill+relaunch cycle every 30s, plus location.reload() every 5s). Four deploy rounds declared "fixed" without anyone looking at the screens. The flicker was obvious to anyone in the venue._

### Deploy

- **Remote deploy sequence (rc-agent):** (1) `cargo build --release`, (2) copy to deploy-staging, (3) start HTTP server on :9998, (4) exec download on pod: `curl.exe -s -o C:\RacingPoint\rc-agent-new.exe http://192.168.31.27:9998/rc-agent.exe`, (5) exec `RCAGENT_SELF_RESTART` sentinel — rc-agent calls `relaunch_self()` → `start-rcagent.bat` swaps `rc-agent-new.exe` → `rc-agent.exe` and starts the new binary. (6) verify build_id on `/health`.
  _Why: Previous deploy-update.bat used `taskkill /F` which killed the exec handler mid-command, preventing the restart from executing. `RCAGENT_SELF_RESTART` spawns the new process before exiting — reliable across all pods._
- **NEVER use `taskkill /F /IM rc-agent.exe` followed by `start` in the same exec chain.** The taskkill kills the process serving the exec endpoint — subsequent commands in the chain may never execute. Use `RCAGENT_SELF_RESTART` sentinel instead.
  _Why: Pod 5 went offline for 2+ minutes during v17.0 deploy because taskkill killed rc-agent before the restart command ran. rc-sentry eventually recovered it, but the gap is unacceptable._
- **Server deploy (racecontrol) — 7 steps, no shortcuts:**
  1. **Record expected build_id:** `git rev-parse --short HEAD` — save BEFORE staging
  2. **Download first (while old process still runs :8090):** Write JSON to file, then `curl -s -X POST http://192.168.31.23:8090/exec -d @file.json` with `curl.exe -o C:\RacingPoint\racecontrol-new.exe http://192.168.31.27:9998/racecontrol.exe`
  3. **SSH kill+swap:** `ssh ADMIN@100.125.108.37` (Tailscale IP) then: `taskkill /F /IM racecontrol.exe & ping -n 4 127.0.0.1 >nul & cd /d C:\RacingPoint & del racecontrol-old.exe & ren racecontrol.exe racecontrol-old.exe & ren racecontrol-new.exe racecontrol.exe`
  4. **Start via schtasks:** `schtasks /Run /TN StartRCTemp` — this calls `start-racecontrol.bat` which kills orphan watchdogs, then runs `schtasks /Run /TN StartRCDirect` (direct racecontrol.exe launch, persists in non-interactive context).
  5. **Verify build_id:** `curl -s http://192.168.31.23:8080/api/v1/health` — `build_id` must match step 1. If size mismatch between local and deployed, the swap failed — repeat step 3.
  6. **Verify the EXACT fix, not just health:** Test the specific endpoint/behavior that was changed. `build_id` match proves the binary deployed, NOT that the bug is fixed.
  7. **If any step fails, stop and recover** — SCP the binary directly: `scp racecontrol.exe ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.exe` then `schtasks /Run /TN StartRCDirect`.
  **NEVER combine taskkill + download in one exec chain** — racecontrol hosts :8090, killing it kills the exec handler mid-download.
  **Server uses a PowerShell watchdog** (`start-racecontrol-watchdog.ps1`) that auto-restarts racecontrol on crash. Each `schtasks /Run /TN StartRCTemp` call starts the bat which kills existing watchdogs via WMIC before starting a new one. The watchdog has a singleton mutex (`Global\RaceControlWatchdog`) to prevent multiplication. If watchdog multiplication occurs (multiple PowerShell instances fighting over port 8080), kill ALL powershell first: `taskkill /F /IM powershell.exe`, then restart.
  _Why: 2026-03-24 — 16 orphan watchdog PowerShell instances accumulated (~960MB RAM) from repeated schtasks calls. Each watchdog respawned racecontrol after taskkill, preventing binary swap. Fixed by adding WMIC watchdog cleanup to bat + singleton mutex to watchdog.ps1. SSH `start` command doesn't persist — use schtasks. `timeout` command fails in non-interactive SSH — use `ping -n N 127.0.0.1` for delays._
- **NEVER run pod binaries on James's PC** (rc-agent.exe, pod-agent.exe, ConspitLink.exe) — crashes workstation.
  _Why: Pod binaries assume hardware/ports that don't exist on James's machine; crash is instant._
- **Test before upload** = `cargo test` + size check + deploy to Pod 8 first, verify, then other pods.
  _Why: Pod 8 canary catches runtime failures (DLL missing, wrong CWD, config mismatch) before fleet-wide damage._
- **`touch build.rs` before release builds after new commits.** Cargo caches the binary if no source files changed — `build.rs` embeds `GIT_HASH` at compile time, but cargo doesn't detect new commits as a source change. After `git commit`, always `touch crates/<crate>/build.rs` before `cargo build --release` to force a fresh `GIT_HASH`. Verify with step 4 of the deploy sequence.
  _Why: Deployed racecontrol showed build_id `daaa9298` (old) instead of `129a24f2` (current HEAD). Binary was served from a stale cargo cache. Required force-rebuild + redeploy._
- **Smallest Reversible Fix First** — when fixing a production issue, prefer the smallest change that can be tested and rolled back. Don't rewrite Rust code when a bat file one-liner works. Don't touch self-restart logic when a boot-time cleanup suffices. Save elegant fixes for when you have a test environment.
  _Why: PowerShell memory leak fix attempt changed self_monitor.rs relaunch logic. Four iterations (cmd/c, CREATE_NO_WINDOW, exit, Environment::Exit) all broke self-restart, each time taking Pod 6 down with manual recovery. The working fix was always `taskkill /F /IM powershell.exe` in start-rcagent.bat — one line, zero risk._
- **Have a rollback plan before deploying** — before changing critical paths (self-restart, deploy chain, process guard), prepare a one-command recovery: Tailscale SSH + schtasks to restart, or SCP the old binary back. Never deploy without knowing how to undo.
  _Why: Pod 6 went down 4 times during self-restart fix attempts with no prepared recovery path. Had to discover Tailscale SSH mid-incident._
- **Tailscale SSH fallback for pod recovery** — when rc-agent is dead and LAN exec is unavailable, SSH via Tailscale: `ssh -o StrictHostKeyChecking=no User@<tailscale_ip>`. Use `schtasks /Run /TN StartRCAgent` to restart. Pod Tailscale IPs: sim1-sim8 (run `tailscale status` to find).
  _Why: Discovered during Pod 6 incident — only way to recover a pod when rc-agent is dead, rc-sentry doesn't restart it, and no one is physically at the venue._
- **Deploy staging path:** `C:\Users\bono\racingpoint\deploy-staging\`
  _Why: Consistent staging root prevents "which binary is current" confusion across sessions._
- **Pendrive install:** `D:\pod-deploy\install.bat <pod_number>` (v5) — run as admin on the pod. For pods with RCAGENT_SERVICE_KEY blocking exec.
  _Why: Pendrive path is fixed; using ad-hoc paths leaves install.bat version drift._
- **Rebuild + redeploy after functional code commits.** When `git log <deployed_build_id>..HEAD -- crates/<crate>/` shows functional changes (not docs-only), the deployed binary is stale. Rebuild and redeploy before starting new work. At session start, always compare `/api/v1/health` `build_id` against `git rev-parse --short HEAD` — if they differ, check the diff. Docs-only = OK. Any `.rs` change = rebuild required.
  _Why: 2026-03-24 audit found server running `0bebb9aa` while HEAD was `848b127b` — 6 functional commits behind including new endpoints (`/app-health`, `/games/catalog`). Server was serving 404 for endpoints that existed in the codebase. Nobody noticed because health endpoint was green._
- **Server binary swap: rename, don't overwrite.** Windows locks running executables — `move /Y` and `del` fail while the process holds a handle. Instead: (1) `ren racecontrol.exe racecontrol-old.exe` (Windows allows renaming running .exe), (2) `ren racecontrol-new.exe racecontrol.exe`, (3) `taskkill /F /IM racecontrol.exe` (kills old process by original image name), (4) `schtasks /Run /TN StartRCTemp`. Clean up `racecontrol-old.exe` later.
  _Why: 2026-03-24 deploy stuck in a loop — `move /Y` returned "Access is denied" because the running process held a file lock. Watchdog kept restarting the old binary between kill attempts. Rename trick bypassed the lock on first try._
- **single-binary-tier policy (v22.0):** All pods run the SAME binary compiled with default features (full build). Feature selection is done at RUNTIME via feature flags (FF-01+), NOT at compile time per pod. The `--no-default-features` build exists for CI verification and future testing scenarios only — it is NEVER deployed to production pods. Do not create per-pod Cargo feature profiles, per-pod binaries, or pod-specific compile-time feature sets.
  _Why: Per-pod compile-time variants create a combinatorial explosion of untested binaries. 8 pods x N feature combinations = build/test/deploy nightmare. Runtime feature flags (v22.0 Phase 177+) provide the same capability with one tested binary._

- **rc-agent MUST run in Session 1 (interactive desktop).** Session 0 (services) prevents ALL GUI operations: Edge browser, game launching, ConspitLink, overlay HUD, window management, SendInput, taskbar control, and freeze detection. The `RCWatchdog` Windows service handles restarts using `WTSQueryUserToken` + `CreateProcessAsUser` to spawn `start-rcagent.bat` in Session 1. **NEVER create schtasks or services that start rc-agent directly** — they run as SYSTEM in Session 0. The HKLM Run key (`start-rcagent.bat`) handles first boot in Session 1; `RCWatchdog` handles crash recovery in Session 1.
  _Why: 2026-03-26 — ALL 8 pods had blanking screen broken for unknown duration. The bat-based `RCAgentWatchdog` schtask ran as SYSTEM (Session 0), restarted rc-agent there after crashes. Edge couldn't create windows. `lock_screen_state: screen_blanked` with `edge_process_count: 0` — an impossible state that went undetected because the audit checked health/build_id (proxies) instead of actual behavior. No customer-facing screen was working on any pod._
- **Audit must verify Session context.** At session start AND in every audit, run `tasklist /V /FO CSV | findstr rc-agent` and confirm the session column shows `Console` (not `Services`). Also check `:18924/debug` endpoint: `edge_process_count` must be >0 when `lock_screen_state` is `screen_blanked`. If edge=0 + state=blanked, the blanking screen is broken regardless of what health says.
  _Why: The previous audit checked build_id, WS connectivity, HTTP reachability, and health endpoints — all passed while blanking was broken on ALL pods. The debug endpoint had the answer the whole time but was never queried._
- **Behavioral verification for blanking.** After deploying rc-agent or rc-watchdog, trigger `RCAGENT_BLANK_SCREEN` via exec and verify `edge_process_count > 0` at `:18924/debug` within 12 seconds. This is the ONLY reliable test — health endpoints and build IDs are necessary but not sufficient.
  _Why: `show_blank_screen()` sets state to `screen_blanked` even when `launch_browser()` silently fails. The state change succeeds but the browser never launches. Only checking the actual Edge process count catches this._

### Comms

- **Bono INBOX.md:** Append to `C:\Users\bono\racingpoint\comms-link\INBOX.md` → `git add INBOX.md && git commit && git push`. Entry format: `## YYYY-MM-DD HH:MM IST — from james`. Then also send via WS (send-message.js). Git push alone is insufficient — Bono does not auto-pull.
  _Why: Git-only comms left Bono's context stale on three occasions; WS+git is the required dual channel._
- **Auto-push + notify (atomic sequence):** `git push` → comms-link WS message → INBOX.md entry. Do all three before marking tasks complete, starting new work, or responding to Uday. Every push, every commit — even cleanup/docs/logbook. No ranking of "important" vs "minor" commits.
  _Why: Commits without push leave Bono's context stale and break deploy chains; treating minor commits as optional caused missed notifications._
- **Bono VPS exec (v18.0 — DEFAULT):** Use comms-link relay, not SSH. Single: `curl -s -X POST http://localhost:8766/relay/exec/run -H "Content-Type: application/json" -d '{"command":"git_pull"}'`. Chain: `curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[...]}'`. SSH (`ssh root@100.70.177.44`) only when relay is down.
  _Why: SSH requires Tailscale up and leaves no audit trail; relay is always-on and returns structured results._
- **Standing Rules Sync:** After modifying CLAUDE.md standing rules, always sync to Bono via comms-link so both AIs operate under the same rules.
  _Why: Rules drift between AIs causes inconsistent behavior and contradictory decisions in multi-agent tasks._

### Code Quality

- **No `.unwrap()` in production Rust** — use `?`, `.ok()`, or match.
  _Why: Unwrap panics crash the entire service; production code must degrade gracefully._
- **No `any` in TypeScript** — type everything explicitly.
  _Why: `any` hides real type errors that surface at runtime, not compile time._
- **`.bat` files: clean ASCII + CRLF.** Use bash heredoc + `sed 's/$/\r/'`. Never Write tool directly (adds UTF-8 BOM = breaks cmd.exe). Never use parentheses in if/else — use `goto` labels. Test with `cmd /c` before deploying.
  _Why: BOM and parentheses in .bat files cause silent command failures on Windows; caught after multiple broken deploys._
- **Static CRT:** `.cargo/config.toml` `+crt-static` — no vcruntime140.dll dependency on pods.
  _Why: Pod images don't ship VS redistributables; dynamic CRT causes instant crash-on-launch._
- **Cascade updates (RECURSIVE):** When changing a process, update ALL linked references (training data, playbooks, prompts, docs, memory). Never change one place and leave stale references. This includes **data formats** — if you change how a file is written (name, format, location), grep for every reader of that file and update them too. **The cascade is recursive**: if updating process A requires changing file B, then check what process B affects and update those too. Continue until no further downstream impacts exist. Document the full cascade chain in the commit message.
  _Cascade checklist for ANY change:_ (1) grep for all consumers of the changed interface/file/endpoint, (2) update each consumer, (3) for each consumer updated, repeat step 1 on THAT consumer, (4) update OpenAPI specs, contract tests, shared types, (5) document deploy impacts (cloud rebuild, pod redeploy).
  _Why: Stale references in playbooks or prompts cause both AIs to apply the old behavior after a fix. v23 example: rolling appender changed from `racecontrol.log.*` to `racecontrol-*.jsonl` but the `/api/v1/logs` reader still searched for the old name — API returned 3-day-old data silently for days. Kiosk audit (2026-03-24): adding `/games/catalog` endpoint missed 5 downstream consumers — web dashboard had 3 missing games, leaderboards had only 2/8 games, OpenAPI spec was stale, contract tests had no coverage, shared types lacked the response type._
- **Next.js hydration:** Never read `sessionStorage`/`localStorage` in `useState` initializer — use `useEffect` + hydrated flag.
  _Why: SSR reads fail server-side; hydration mismatch breaks the entire page silently._
- **Git Bash JSON:** Write JSON payloads to a file with Write tool, then `curl -d @file`. Bash string escaping mangles backslashes.
  _Why: Inline JSON in Git Bash strips backslashes from Windows paths, corrupting the payload._
- **Never pipe SSH output into config files.** Use `scp` to copy files from remote hosts, not `ssh ... "cat file" > local`. SSH banners (post-quantum warning, MOTD) go to stderr but some wrappers merge streams, silently prepending garbage to the file. If SSH piping is unavoidable, use `ssh ... 2>/dev/null "cat file"`. After any remote file copy, validate the first line: `head -1 file | grep -q '^\[' || echo "CORRUPTED"`.
  _Why: 2026-03-24 — racecontrol.toml had 3 SSH banner lines prepended. TOML parser rejected from line 1. load_or_default() fell back to empty defaults. process_guard ran with 0 allowed entries for 2+ hours. No operator saw anything because the error was logged via tracing (not yet initialized at config-load time)._
- **UI must reflect config truth** — no hardcoded camera lists, names, or layouts. All UI must read from API/config dynamically. If the backend config changes, the UI must update without code changes.
  _Why: v16.1 cameras dashboard was initially built with hardcoded 13-camera arrays. When cameras were added/removed from NVR config, the UI showed stale/phantom tiles. Dynamic fetch from /api/v1/cameras fixed it — this rule prevents regression._

### Process

- **Refactor Second** — characterization tests first, verify green, then refactor. No exceptions.
  _Why: Refactoring without a green test baseline turns every compile error into an unknown regression._
- **Cross-Process Updates** — changing a feature? Update ALL: rc-agent, racecontrol, PWA, Admin, Gateway, Dashboard. This means ALL ENVIRONMENTS too — venue (.23), cloud (Bono VPS), and James (.27). Deploy to one and forget the other = schema divergence.
  _Why: Single-crate updates leave other components speaking a different protocol version. Cloud sync broke for 3+ hours because venue had new migrations but cloud DB was on an old binary with missing columns._
- **DB migrations must cover ALL consumers.** `CREATE TABLE IF NOT EXISTS` won't alter existing tables. If a column is used in sync/query code, the migration must `ALTER TABLE ADD COLUMN` for it — even if the CREATE TABLE already includes it. Old databases won't have columns added after initial creation.
  _Why: `updated_at` was in 10 CREATE TABLE statements but only 2 had ALTER migrations. Cloud and venue DBs created by different binary versions had different schemas. Required manual ALTER on 8 tables to fix._
- **Review parallel session commits against standing rules before deploying.** Code from other sessions may not follow current rules (bat parentheses, missing verification, stale references). Always `git show <hash>` and check against standing rules before accepting.
  _Why: Parallel session commit `a948569` used parentheses in bat if/else blocks — caught during standing rule review, fixed before deploy._
- **Convert timestamps before counting events.** Racecontrol logs are UTC; all operations are IST. Before reporting "N events happened," convert every timestamp and exclude your own actions (deploys, restarts, test kills). An audit that reports its own deploys as "unexplained restarts" wastes investigation time and erodes trust in findings.
  _Why: "5 unexplained restarts" turned out to be 1 post-reboot startup + 4 of our own deploys. UTC 03:28 was misread as IST instead of IST 08:58. The Event Viewer check that would have caught this in 30 seconds was deferred for hours._
- **First-run verification after enabling any guard/filter/blocklist.** When flipping `enabled = false` to `true` on any filtering system (process guard, firewall, allowlist, rate limiter), check the FIRST scan result immediately: how many items flagged? If "everything" or "nothing" — the config is wrong. An empty allowlist + enabled guard = block everything. This is structurally incomplete — don't mark shipped.
  _Why: Process guard was enabled with an empty allowlist. Every process was flagged — 28,749 false violations/day for 2 days. Nobody noticed because (a) the log API was broken (F12), (b) no automated monitoring existed, (c) no first-run check was done after enabling._
- **No Fake Data** — use `TEST_ONLY`, `0000000000`, or leave empty. Never real-looking identifiers.
  _Why: Realistic-looking fake data (names, IDs, emails) has leaked into production databases twice._
- **Prompt Quality Check** — missing clarity/specificity/actionability/scope → ask one focused question before acting.
  _Why: Acting on ambiguous prompts produces work that must be redone; one question costs less than one wrong implementation._
- **Links and References = "Apply Now"** — when the user shares a link, article, or methodology alongside a problem, apply it to the current problem FIRST, document it SECOND. A reference shared during active work is a tool to use, not information to file.
  _Why: User shared 4 debugging methodologies during an active crash investigation. James wrote a comparison table and updated rules instead of applying them to the open bug. Three prompts wasted before actual debugging happened._
- **Learn From Past Fixes** — check LOGBOOK + commit history before re-investigating.
  _Why: Re-investigating solved problems wastes session time; LOGBOOK has resolved the same issue in under 2 minutes._
- **LOGBOOK:** After every commit, append `| timestamp IST | James | hash | summary |` to `LOGBOOK.md`.
  _Why: LOGBOOK is Tier 2 debugging — without consistent entries, memory-based debugging fails._

### Testing & Verification

- **Verify the EXACT behavior path, not proxies.** After deploying a fix, test the EXACT data flow that was broken: input string → transform → parse → decision → action. Health endpoints and build IDs prove the binary is running, NOT that the bug is fixed. A 2-character difference (`"` quotes on curl output) kept all 8 pods flickering through two deploy cycles because the proxy checks (health OK, build_id correct) all passed while the actual parse path failed silently.
  _Why: Pod healer curl fix deployed twice — both times declared "fixed" based on health endpoint. The actual stdout was `"200"` (with quotes), which failed `u32::parse()` → `unwrap_or(0)` → healer still thought lock screen was down → ForceRelaunchBrowser spam continued._
- **"Removed" means removed from EVERY machine.** When removing a process, registry key, scheduled task, or config from infrastructure, verify on EVERY target: server (.23), all 8 pods, James (.27). "Removed from server" ≠ "removed from pods."
  _Why: CCBootClient was "removed" from server HKCU Run but was still in Pod 1's HKLM Run and still running. The removal was declared complete without checking pods._
- **Never move on from a failed operation.** When a command fails (quoting error, permission denied, timeout), either fix it NOW with a different approach or explicitly tell the user it's unresolved. "I'll deal with it later" = "I forgot about it."
  _Why: GoPro Webcam registry removal failed due to cmd.exe quote nesting. Moved on without resolving — it stayed in Pod 1's startup for the rest of the session._
- **Audit what the CUSTOMER sees, not what the API returns.** Check visible window titles (`tasklist /V /FO CSV`), check what's on the physical screen, check what processes have foreground windows. API health checks and process lists are internal diagnostics — the customer experience is the screen.
  _Why: 5 instances of M365 Copilot, NVIDIA Overlay, AMD DVR, Steam login dialog, visible cmd.exe windows — all overlaying the blanking screen on every pod. None detectable via health endpoints or fleet status._
- **Investigate anomalies, don't dismiss them.** `violation_count_24h: 100` on all 8 pods should have been alarming. "Expected behavior" is a hypothesis, not a conclusion — verify WHY before dismissing.
  _Why: Process guard had empty whitelist on all pods (fetched when server was down). Every process was flagged. Dismissed as "expected, report_only mode" without checking why whitelisted processes (svchost.exe) were being flagged._
- **NEVER restart explorer.exe on pods with NVIDIA Surround.** Explorer restart disrupts GPU display configuration — NVIDIA Surround drops to 1024x768 single-monitor fallback. Requires full reboot to restore. This broke 3 pods during a taskbar-hide attempt.
  _Why: `Stop-Process -Name explorer` in hide-taskbar script collapsed all triple-monitor setups from 7680x1440 to 1024x768. Required rebooting Pods 5, 6, 7 to restore._
- **Test display changes on ONE pod before fleet-wide.** Any change affecting screen resolution, blanking, kiosk mode, or explorer should be tested on Pod 8 canary first. Display issues are visually obvious but invisible to API health checks.
  _Why: Applied explorer restart to 3 pods simultaneously — all 3 broke. One pod test would have caught it._
- **Screenshot verification triggers taskbar auto-hide.** PowerShell `CopyFromScreen` causes a focus change that reveals auto-hidden taskbar. Don't use screenshot artifacts to diagnose taskbar issues — ask the user to verify physically instead.
  _Why: Taskbar was auto-hiding correctly but every screenshot showed it visible, leading to unnecessary fix attempts that broke NVIDIA Surround._
- **Fix during audit, don't just catalog.** Finding issues without fixing them creates a growing backlog. Apply the smallest reversible fix during the audit pass, then move on. Separate "investigate" from "defer" — deferred items must be explicitly communicated to the user.
  _Why: 9+ items cataloged as "investigate later" during audit — Antamedia, Salt ports, CCBoot, OneDrive, unknown ports, scheduled tasks. Most never got investigated until the user pushed._
- **Context switches kill open investigations.** When the user asks for something new, finish or explicitly park the current investigation with a clear status. Don't silently drop it.
  _Why: "Preflight checks not initiated properly, pods still blinking" was reported, then "push commit" was requested. Context-switched to committing, never came back to investigate the blinking._
- **`git log` before calling builds "old".** Different hash ≠ outdated. Always run `git log <old_hash>..<new_hash> -- <crate_path>` to check actual code changes before claiming a redeploy is needed. Docs-only commits don't change binaries.
  _Why: All 8 pods on `82bea1eb` were called "old build" — but git log showed zero functional rc-agent code changes since that commit. Pods were on the correct build._
- **cmd.exe is hostile to quoting.** Any command routed through rc-agent's `/exec` endpoint goes through `cmd /C`. Strings with spaces, `$`, `"`, or `\` WILL be mangled. Use PID-based targeting (taskkill /F /PID), write batch files to the pod, or use sysinfo/Win32 APIs in Rust — avoid cmd.exe string interpretation entirely.
  _Why: `taskkill /F /IM "GoPro Webcam.exe"` fails because CreateProcessW wraps the /C arg in outer quotes → cmd.exe sees nested quotes → parse breaks. PowerShell `$r` variable stripped by cmd.exe caused the original pod healer flicker bug._

- **Verify monitoring targets against the running system, not docs.** When adding health checks, monitoring, or watchdog targets, check `netstat`, `tasklist`, and the service's own config to confirm host:port. Never copy endpoints from CLAUDE.md or documentation without verifying — they drift. A stale monitoring URL creates false alarms that erode trust in the entire monitoring system.
  _Why: AI healer checked go2rtc at .23:8096 (from stale docs). go2rtc actually runs on .27:1984. 36 consecutive false-DOWN alerts over 72 minutes. The full audit reported "go2rtc DOWN — HIGH severity" for a service that was perfectly healthy._
- **`.spawn().is_ok()` does NOT mean the child started.** On Windows, `spawn()` returning Ok only means CreateProcess was accepted, NOT that the target is running. Always verify the child process is alive after spawn — poll `/health`, check `tasklist`, or read a sentinel file written by the child.
  _Why: rc-sentry's `restart_service()` returned Ok for cmd/C start, PowerShell Start-Process, AND schtasks — all three silently failed to start rc-agent. Pods stayed dead for days because "restarted=true" was logged but never verified._
- **Non-interactive Windows context cannot launch interactive processes.** `cmd /C start`, `PowerShell Start-Process`, and `schtasks /Run` all fail when called from `std::process::Command` with `CREATE_NO_WINDOW` in a non-interactive session. The ONLY proven working path is: call through an HTTP `/exec` endpoint that uses `cmd /C` (different process creation context), or register a Windows Service with SCM.
  _Why: rc-sentry tested 3 different launch methods — all returned success, all silently failed. The same schtasks command worked via the `/exec` HTTP endpoint but not from Rust's Command::new(). Four E2E test cycles were needed to confirm this._
- **MAINTENANCE_MODE sentinel is a silent pod killer.** Once `C:\RacingPoint\MAINTENANCE_MODE` is written (after 3 restarts in 10 min), ALL restarts stop permanently with no timeout, no auto-clear, no alert to staff. Before any restart debugging, ALWAYS clear: `del MAINTENANCE_MODE GRACEFUL_RELAUNCH rcagent-restart-sentinel.txt`.
  _Why: E2E test declared "restart fix doesn't work" twice before discovering MAINTENANCE_MODE was blocking all restarts from a previous crash storm._
- **At session start, check for MAINTENANCE_MODE on all pods.** If any pod shows `ws_connected: false` + `http_reachable: false` in fleet health but responds to `ping`, check for MAINTENANCE_MODE via rc-sentry: `curl -X POST http://<pod_ip>:8091/exec -d @check-maintenance.json`. Three pods went dark for 1.5+ hours because MAINTENANCE_MODE blocked rc-agent with no alert. Recovery: clear sentinels + `schtasks /Run /TN StartRCAgent` via rc-sentry exec.
  _Why: 2026-03-24 audit — Pods 5, 6, 7 all had MAINTENANCE_MODE from a previous crash storm. Pods were powered on, rc-sentry alive, but rc-agent permanently blocked. Same timestamp on all 3 last_seen values was the clue (simultaneous disconnect = external event, not individual crashes)._
- **Audit changes must be cascade-audited before closing.** After any audit, maintenance, or bulk-fix session that modifies infrastructure (configs, firewall rules, services, scheduled tasks, registry keys, TOML files, env vars), run a cascade verification: for EVERY change made, identify all downstream consumers and verify they still work. Changes that look local often have cross-system impact.
  _Cascade checklist:_ (1) List every change made during the session, (2) For each change, identify what reads/depends on the modified file/service/port, (3) Test each downstream consumer — not just "is it running" but "is it producing correct output", (4) If a change requires a service restart to take effect (e.g. go2rtc YAML, racecontrol.toml), document that the restart is pending and what will break if skipped.
  _Why: 2026-03-25 audit — disabling process_guard on Bono required TOML edit + rebuild + restart, but sed left conflicting `enabled = false` and `enabled = true` lines (TOML uses last value = still enabled). UFW enable could have blocked comms-link WS port if not pre-allowed. go2rtc YAML moved creds to env vars but go2rtc was still running with old in-memory config — restart needed at next maintenance window. Each would have been a silent downstream failure without cascade verification._

### Debugging

- **Cross-Process Recovery Awareness** — independent recovery systems (self_monitor, rc-sentry watchdog, server pod_monitor/WoL, scheduler wake) can fight each other. When adding or modifying any auto-recovery, auto-restart, or auto-wake logic, verify it won't cascade with the others.
  - A graceful self-restart must be distinguishable from a real crash (use sentinel files or IPC).
  - Escalation (e.g. MAINTENANCE_MODE) must know *why* restarts are happening, not just count them. Server-down restarts ≠ pod crashes.
  - WoL auto-wake will revive pods that entered MAINTENANCE_MODE, creating infinite loops. Any "pod offline" recovery must check whether the pod was deliberately taken offline.
  - Always test recovery paths against **server downtime**, not just pod failures.

  _Why: Self-restart + watchdog + WoL created an infinite restart loop that took 45 minutes to diagnose; the systems had no coordination._
- **Allowlist Auth — RESOLVED.** GET endpoints (`/config/kiosk-allowlist`, `/guard/whitelist/pod-{N}`) moved to `public_routes` — rc-agent fetches without auth. POST/DELETE still require staff JWT. See Security section.
  _Why: 401 on GET caused rc-agent to fall back to empty default allowlist._
- **Process guard allowlist: fetch-at-boot + 5-min periodic re-fetch (DONE in `821c3031`).** rc-agent fetches from `/api/v1/guard/whitelist/pod-{N}` at startup AND every 300s via a background tokio task. If the server is down at boot, pods get `MachineWhitelist::default()` (empty) but self-heal within 5 minutes once the server is back. Manual restart is no longer required but can be used for immediate effect: `curl -X POST http://<pod_ip>:8091/exec -d '{"cmd":"taskkill /F /IM rc-agent.exe & schtasks /Run /TN StartRCAgent"}'` via rc-sentry. Verify: `violation_count_24h` should stop increasing after the next re-fetch cycle.
  _Why: 2026-03-24 — all 8 pods showed violation_count_24h: 100 (false positives). Server had restarted, pods booted while server was briefly down, fetched empty default, and never re-fetched. Periodic re-fetch implemented same day to prevent recurrence._
- **Boot Resilience: No single-fetch-at-boot without retry.** Any data fetched from a remote source at startup MUST have a periodic re-fetch background task using `rc_common::boot_resilience::spawn_periodic_refetch()`. Single-fetch-at-boot without retry is a banned pattern — if the server is down at boot, the resource stays at its cached/default value forever. Current startup-fetched resources and their re-fetch status:
  - Allowlist (process guard): DONE — 5-min periodic re-fetch (commit `821c3031`)
  - Feature flags: DONE — 5-min periodic re-fetch via HTTP GET /api/v1/flags (BOOT-02)
  - Billing rates: CHECK — verify if billing rates have periodic re-fetch or only load at boot
  - Camera config: CHECK — verify if camera config has periodic re-fetch or only load at boot
  _Why: Feature flags were fetched once at boot via WS FlagSync and never re-fetched if WS connection failed. Server transience at boot left pods running with stale cached flags indefinitely. spawn_periodic_refetch() provides self-healing within one interval (5 min)._
- **"Shipped" Means "Works For The User"** — A milestone is NOT shipped until every user-facing endpoint is verified working at runtime:
  - Binary built, deployed, and **running** (not just compiled). All runtime dependencies present (DLLs, models, config files).
  - Every API endpoint returns correct data (not just HTTP 200 — check response content).
  - Every UI page renders and is interactive (open in browser, verify visually with screenshot).
  - Hardware integrations tested with live data (cameras, GPU inference, network devices).
  - **Frontend: verify from the user's browser, not from the server.** `NEXT_PUBLIC_` env vars are baked at build time — rebuild with correct LAN IP.
  - **Frontend: standalone deploy requires `.next/static` copied into `.next/standalone/`.** AND all `next.config.ts` files MUST set `outputFileTracingRoot: path.join(__dirname)`. Without this, Next.js embeds build-machine absolute paths in `required-server-files.json` (`appDir` field) and `server.js` (`outputFileTracingRoot`, `turbopack.root`). Pages render via SSR but ALL static files (CSS, JS, fonts) return 404 on the deployed server — the UI loads as unstyled HTML with no interactivity. After EVERY deploy, verify by curling one `_next/static/` URL — a 200 proves static serving works, a 404 means the `appDir` path is stale.
  _Why: 2026-03-25 — kiosk and web dashboard had all static files returning 404 for an unknown duration. Health endpoint showed "healthy" (it only checks page availability, not static file serving). The fix was changing `appDir` in `required-server-files.json` from `C:\Users\bono\...` (build machine) to `C:\RacingPoint\...` (deploy target). Permanent fix: set `outputFileTracingRoot` in all `next.config.ts` files._
  - **Frontend: grep ALL `NEXT_PUBLIC_` references after any env var change.** One missing var (e.g. `NEXT_PUBLIC_WS_URL`) silently falls back to `localhost` — works on the server, fails on every remote browser (POS, spectator, staff phones). After adding or modifying any `NEXT_PUBLIC_` var, run `grep -rn NEXT_PUBLIC_ src/` and verify EVERY one has a value in `.env.production.local`.
  - **Frontend: after every dashboard rebuild/deploy, verify from a machine that is NOT the server.** SSH to POS or open from James's browser pointing at `.23:3200`. `curl` to the dashboard URL proves HTML loads, not that JavaScript/WebSocket works.
  - `cargo check` and unit tests are necessary but NOT sufficient. They prove structure, not function.

  _Why: "Phase Complete" was reported 9 times based on compilation alone — runtime failures were hidden each time. `NEXT_PUBLIC_WS_URL` was never set — `NEXT_PUBLIC_API_URL` was correct so REST worked, but WebSocket defaulted to `ws://localhost:8080` causing "page loads but no data" on the POS machine for every session until caught._
- **Long-Lived Tasks Must Log Lifecycle** — Any `tokio::spawn` or `std::thread::spawn` loop must log: (a) when it starts, (b) when it processes its first item, (c) when it exits. Errors in new pipelines use `warn`/`error`, not `debug`.
  _Why: Silent task death (panic in spawned thread, channel close) went undetected for hours because no lifecycle logs existed._
- **Cause Elimination Before Fix** — Never jump from symptom to fix. Follow the 5-step Cause Elimination Process (see Debugging Methodology section): Document symptom → List ALL hypotheses → Test & eliminate one by one → Fix confirmed cause → Verify fix works. "Found a crash dump" ≠ "found the cause."
  _Why: Pod 6 game crash was attributed to Variable_dump.exe based on crash dumps alone without testing other hypotheses (RAM pressure, FFB driver, USB hardware). The fix was never verified because pods went offline. Correlation-based fixes leave the real cause unfixed 40% of the time._

### Security

- **Allowlist endpoints: GET is public, POST/DELETE require staff auth.** `GET /api/v1/config/kiosk-allowlist` and `GET /api/v1/guard/whitelist/pod-{N}` are in `public_routes` so rc-agent can fetch without auth. Write operations (POST to add, DELETE to remove entries) still require staff JWT.
  _Why: rc-agent fetches the allowlist at boot and every 5 min (periodic re-fetch added in `821c3031`). Requiring auth on GET caused 401 → empty allowlist → false violations fleet-wide._
- **Process guard safe mode:** Do not disable rc-process-guard during testing sessions — use the allowlist override instead.
  _Why: Disabling the guard entirely during a test left the machine unprotected when the session ended without re-enabling it._
- **Security gate (SEC-GATE-01) must pass before any deploy.** `node comms-link/test/security-check.js` runs 31 static assertions covering auth middleware, route auth coverage, credential leaks, protocol immutability, and deploy pipeline integrity. Integrated into: (1) `run-all.sh` Suite 4, (2) `stage-release.sh` pre-build, (3) `deploy-pod.sh` + `deploy-server.sh` pre-deploy, (4) `gate-check.sh` via Suite 0.
  _Why: Security fixes were point-in-time patches that regressed across milestones. No automated check existed to prevent new phases from adding unprotected routes, leaking credentials, or removing auth middleware. 22 milestones shipped without security regression tests._
- **Pre-commit hooks block credential leaks.** Both repos (comms-link + racecontrol) have `.git/hooks/pre-commit` that blocks: private keys, AWS keys, hardcoded passwords, sensitive files (.env.local, racecontrol.toml). Install with `bash comms-link/scripts/install-hooks.sh`. Warns on `.unwrap()` in Rust and `any` in TypeScript.
  _Why: Credentials and sensitive config files were committed to git multiple times. Pre-commit hooks prevent the leak before it enters version control._
- **Deploy pipeline enforces security + manifest.** The correct workflow is: `stage-release.sh` (security pre-flight → build → SHA256 → manifest) → `gate-check.sh --pre-deploy` → `deploy-pod.sh` (security gate + manifest check) → `deploy-server.sh` (security gate + manifest check) → `gate-check.sh --post-wave`. Each step is self-verifying. Skipping any step = potential regression.
  _Why: Deploy scripts previously had no security enforcement. Stale binaries, corrupted downloads, and wrong build_id were caught only by human discipline._

### Regression Prevention

- **Every manual fix MUST have code-enforced startup verification.** If you fix a problem by changing an OS setting (power plan, USB suspend, registry key), app config ("Forced update"), or process state (killing duplicates), the fix MUST be encoded in a startup script (start-rcagent.bat, pre-flight check, or rc-agent boot sequence) that runs on every boot. Settings that aren't enforced at boot WILL regress through Windows updates, app auto-updates, deploy cycles, or pod restarts.
  _Why: ConspitLink flickering was fixed three times in the same day: (1) USB suspend + power plan + forced update set manually, (2) same settings reverted after deploy cycle, (3) process multiplication after restart with stale bat. Only the fourth fix — adding enforcement to start-rcagent.bat — stuck permanently. MAINTENANCE_MODE had the same pattern: cleared manually, came back because no code prevented re-entry. 2026-03-25._
- **Deploy cycle MUST include bat file sync.** When deploying new rc-agent/rc-sentry binaries, also deploy the current `start-rcagent.bat` and `start-rcsentry.bat` from the repo. Stale bat files on pods cause settings regression, missing process kills, and wrong startup procedures. Add bat download step to the deploy JSON chain.
  _Why: Pod 1 had a bat file missing 8 bloatware kill lines, the ConspitLink singleton guard, and the power settings enforcement. The stale bat allowed ConspitLink to multiply to 11 instances._
- **Process multiplication: always kill-all before start-one.** Any process that can be started multiple times (ConspitLink, watchdogs, PowerShell helpers) must have `taskkill /F /IM <name>` BEFORE the `start` command in the bat file. Check `tasklist | findstr <name>` count after deploy to verify singleton.
  _Why: ConspitLink accumulated 4-11 instances per pod from accumulated restarts. Each instance grabbed the HID device, causing `Bind failed` errors and visible steering wheel flickering._

### OTA Pipeline

- **Always preserve previous binary before swap.** Rename the current binary to `*-prev.exe` before placing the new one. Never delete the previous binary during the swap step. Manual rollback = rename prev back.
  _Why: Without a preserved previous binary, a failed deploy requires rebuilding from source — 5+ minutes of downtime vs 10 seconds for a rename._
- **Never deploy without a signed manifest.** Every OTA release requires a `release-manifest.toml` locking binary SHA256, config schema version, frontend build_id, git commit, and timestamp. gate-check.sh verifies the manifest exists and all fields are populated before any binary leaves staging.
  _Why: Deploying without a manifest means no SHA256 to verify against post-deploy — health checks can't confirm the right binary is running._
- **Billing sessions must drain before binary swap on any pod.** The OTA pipeline checks `has_active_billing_session()` before swapping. Pods with active sessions defer swap until session ends or checkpoint to DB. Never kill a billing session mid-transaction.
  _Why: Killing a billing session mid-race loses the customer's time and money tracking — requires manual reconciliation and erodes trust._
- **OTA sentinel file protocol.** Write `C:\RacingPoint\OTA_DEPLOYING` sentinel at OTA start, clear on complete or rollback. All recovery systems (rc-sentry, pod_monitor, WoL) MUST check this file before triggering restarts during OTA. A restart during OTA corrupts the binary swap.
  _Why: rc-sentry restarted rc-agent mid-binary-copy during an early deploy test — the binary was truncated, pod went into MAINTENANCE_MODE._
- **Config push NEVER goes through fleet exec endpoint.** Config changes use the dedicated ConfigPush WebSocket channel (CP-01). Fleet exec is for operational commands only. Mixing config into exec creates an unaudited config change path that bypasses schema validation.
  _Why: An early prototype pushed billing rate changes via fleet exec — no validation, no audit log, no ack tracking. Two pods ran different rates for 4 hours._
- **Rollback window: previous binary preserved for 72 hours minimum.** Do not clean up `*-prev.exe` files within 72 hours of deploy. Late-emerging issues (weekend traffic patterns, edge-case billing scenarios) may require rollback days after deploy.
  _Why: A billing edge case (session spanning midnight) only surfaced 36 hours after deploy. The previous binary had already been cleaned up — required a full rebuild instead of a 10-second rollback._

---

## Debugging Methodology

### Cause Elimination Process (MANDATORY for all non-trivial bugs)

Before fixing any bug, follow this structured process. Do NOT jump from symptom to fix.

**Step 1 — Reproduce & Document Symptom**
- What exactly happened? (user's words, screenshot, error message)
- When? What action triggered it? What was the system state?

**Step 2 — Hypothesize (list ALL possible causes)**
- Write down every plausible cause, not just the first one found
- Include: software, hardware, config, network, user error, interaction between systems
- Example (Pod 6 crash): (a) Variable_dump.exe USB disruption, (b) AC FFB driver crash, (c) RAM pressure from 15 orphan PowerShell processes, (d) VSD Craft itself, (e) USB hub/cable fault

**Step 3 — Test & Eliminate (one by one)**
- For each hypothesis, define a test that would confirm or rule it out
- Run tests in order of likelihood and ease
- Cross off eliminated causes with evidence, not assumptions
- "Found a crash dump" ≠ "found the cause" — correlation is not causation

**Step 4 — Fix & Verify**
- Apply the fix for the confirmed cause
- **Reproduce the original trigger** — verify the bug is actually gone
- Visual verification for UI/display issues (standing rule)
- If you can't reproduce (e.g. pods offline), mark as UNVERIFIED and schedule retest

**Step 5 — Log**
- Record in LOGBOOK.md: symptom, hypotheses tested, confirmed cause, fix applied, verification result

### 4-Tier Debug Order (WHERE to look)

| Tier | Method | When | Action |
|------|--------|------|--------|
| 1 | **Deterministic** | Always first | Stale sockets, game cleanup, temp files, WerFault — apply without LLM |
| 2 | **Memory** | After Tier 1 fails | Check LOGBOOK.md + commit history for identical past incident |
| 3 | **Local Ollama** | After Tier 2 fails | Query qwen2.5:3b at James .27:11434 |
| 4 | **Cloud Claude** | Last resort | Escalate — NOT auto-triggered |

The 4-Tier order tells you WHERE to look. The Cause Elimination Process tells you HOW to reason. Use both together.

---

## Billing and Rates

- 30min / ₹700 | 60min / ₹900 | 5min free trial | 10s idle threshold
- PWA shows "credits" (not rupees)
- Wheelbases: Conspit Ares 8Nm — OpenFFBoard VID:0x1209 PID:0xFFB0
- UDP telemetry ports: 9996 (AC) | 20777 (F1) | 5300 (Forza) | 6789 (iRacing) | 5555 (LMU)

---

## Brand Identity

- Racing Red: `#E10600` | Asphalt Black: `#1A1A1A` | Gunmetal Grey: `#5A5A5A`
- Card: `#222222` | Border: `#333333`
- Fonts: Montserrat (body), Enthocentric (headers)
- OLD orange `#FF4400` is DEPRECATED — do not use

---

## Security Cameras

- 13x Dahua 4MP. Auth: `admin` / `Admin@123`, RTSP `subtype=1`
- NVR: .18 | Entrance: .8 | Reception: .15, .154
- People tracker: port 8095, FastAPI + YOLOv8, entry/exit counting

---

## Key File Paths

| Path | Purpose |
|------|---------|
| `C:\RacingPoint\racecontrol.toml` | Server config (on server .23, user: ADMIN) |
| `C:\RacingPoint\start-racecontrol.bat` | Server start script (HKLM Run) |
| `C:\RacingPoint\start-rcagent.bat` | Pod agent start script (HKLM Run on each pod) |
| `C:\Users\bono\racingpoint\deploy-staging\` | Build staging area + HTTP server (James) |
| `C:\Users\bono\racingpoint\deploy-staging\webterm.py` | Web terminal (Uday's phone → :9999) |
| `C:\Users\bono\racingpoint\comms-link\INBOX.md` | James→Bono comms channel |
| `D:\pod-deploy\` | Pendrive deploy kit (install.bat v5) |
| `LOGBOOK.md` | Incident + commit log at repo root |
| `.cargo\config.toml` | Static CRT build config |

---

## Development Rules

- No `.unwrap()` in production Rust. No `any` in TypeScript. Idempotent SQL migrations.
- Static CRT: `.cargo/config.toml` `+crt-static` — eliminates vcruntime140.dll on pods
- Git config (per-repo): `user.name="James Vowles"`, `user.email="james@racingpoint.in"`
- Cascade updates (RECURSIVE): when changing a process, update ALL linked references AND their downstream consumers recursively. See Standing Rules > Code Quality for full checklist
- LSP: rust-analyzer enabled in settings.json
- Next.js hydration: never read `sessionStorage`/`localStorage` in useState initializer — use `useEffect` + hydrated flag
- `.bat` files: NEVER use parentheses in if/else blocks — use `goto` labels. Test with `cmd /c` before deploying.
- Git Bash JSON: write JSON payloads to file with Write tool, then `curl -d @file` (bash string escaping mangles `\\`)
- `start` command: always use `/D C:\RacingPoint` to set CWD (rc-agent uses relative `rc-agent.toml`)

---

## Current Blockers

- v6.0 blocked on BIOS AMD-V (SVM Mode disabled on server Ryzen 7 5800X) — does not affect v9.0
- ~~Gmail OAuth tokens expired~~ — RESOLVED 2026-03-22
- Pod 6 UAC prompt (2026-03-16) — unknown install request, under investigation
- USB mass storage lockdown pending (Group Policy)
- Server DHCP reservation needed: MAC 10-FF-E0-80-B1-A7 → 192.168.31.23
- Server .23 Node v24.14.0 should be downgraded to v22 LTS at next maintenance window (no runtime impact — build-only)
- Process guard in `report_only` mode — monitor 24-48h then switch to `kill_and_report`
- Server .23 Tailscale re-authenticated under `james@` (node `racing-point-server-1`, IP 100.125.108.37). Old `bono@` node (`racing-point-server`, 100.71.226.83) is stale — remove from Tailscale admin console
