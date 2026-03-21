# RaceControl Logbook

Chronological record of all changes by Bono (cloud) and James (venue).
Both must append here when committing. Format: `| timestamp | author | commit | summary |`

---

## 2026-03-01

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 01 07:37 UTC | Bono | `fa9b88c` | Add foolproof billing system with drive-time detection |
| Mar 01 18:53 UTC | Bono | `4fb0235` | Add remote game launcher with AI crash debugger |
| Mar 01 19:20 UTC | Bono | `d4fd8ee` | Fix Ollama default port from 32769 to 11434 |
| Mar 01 20:07 UTC | Bono | `9e8cfce` | Add AC LAN multiplayer with hotlap-optimized defaults |
| Mar 01 20:10 UTC | Bono | `9a7b1b9` | Fix audio cutout on AC session start by enforcing CSP 2144+ |

## 2026-03-02

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 02 12:13 UTC | Bono | `82f6ce9` | Add customer authentication system + PWA scaffold |
| Mar 02 13:03 UTC | Bono | `f3e648e` | Rebrand venue dashboard + customer PWA to Racing Point brand identity |
| Mar 02 13:08 IST | James | `a488ea3` | Add kiosk security + lock screen for customer authentication |
| Mar 02 13:09 IST | James | `6a2adcd` | Apply Racing Point brand identity to lock screen |
| Mar 02 14:06 IST | James | `654cca4` | Add explorer.exe to kiosk allowed process whitelist |
| Mar 02 14:51 IST | James | `5d508a7` | Fix SQL injection, mutex poisoning, CSS import bugs, clean warnings |
| Mar 02 17:48 IST | James | `bc2b89e` | Add full AI integration + Docker containerization |
| Mar 02 18:19 IST | James | `601dcd0` | Fix Rust Docker image version + add env var overrides for AI config |

## 2026-03-03

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 03 07:35 UTC | Bono | `902ef7f` | Add signature pad to waiver form |
| Mar 03 08:57 UTC | Bono | `a2da3e6` | Fix Tailwind v4 CSS layers breaking all spacing in PWA |
| Mar 03 17:34 IST | James | `6f925fe` | Add RaceControl Kiosk Terminal — staff + spectator web interface |

## 2026-03-04

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 04 04:04 IST | James | `a8669b7` | Add AC launcher, pod lock screen timer, billing fixes, and auto-start scripts |
| Mar 04 22:09 IST | James | `e8c8fec` | Add wallet, lap tracker, pod reservations, camera control, spectator rewrite, kiosk improvements |
| Mar 04 22:09 IST | James | `1a4e2f3` | Add cloud-to-local sync for customer data (drivers, wallets, pricing, experiences) |
| Mar 04 22:48 IST | James | `305a105` | Add utility scripts, PWA booking/register pages, kiosk designs |
| Mar 04 23:18 IST | James | `d15294c` | Add wallet balance display to PWA profile page |
| Mar 04 23:20 IST | James | `cd952ec` | Replace rupee symbols with credits across PWA |
| Mar 04 23:40 IST | James | `785d2f7` | Add customer display IDs (RP001) and cloud terminal for remote commands |

## 2026-03-05

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 05 00:26 IST | James | `7647778` | Add pod-agent watchdog to rc-agent for mutual auto-recovery |
| Mar 05 00:49 IST | James | `b92a58f` | Add Tier 2 pod monitor: auto-detect stale pods and restart rc-agent |
| Mar 05 01:11 IST | James | `860848a` | Add Wake-on-LAN and remote shutdown for pod power management |
| Mar 05 01:21 IST | James | `3a6dfe5` | Add PodStatus::Disabled to prevent auto-recovery on intentionally shut down pods |
| Mar 05 04:39 IST | James | `a950a2a` | Add custom experience booking APIs and employee debug PIN system |
| Mar 05 05:09 IST | James | `a3a6bd8` | Add PIN-protected terminal auth for james.racingpoint.cloud |
| Mar 05 18:29 UTC | Bono | `87ab732` | Add F1 telemetry, friends system, multiplayer groups, smart scheduler, AI WebSocket |
| Mar 05 18:33 UTC | Bono | `5912b12` | Wire up Friends, Multiplayer, and Telemetry API routes |
| Mar 05 19:34 UTC | Bono | `d7d3911` | Marketing features — shareable reports, public leaderboard, referrals, coupons, packages, memberships |
| Mar 05 19:38 UTC | Bono | `637cda7` | Dynamic pricing, referral rewards, coupons admin, review nudges |
| Mar 05 19:49 UTC | Bono | `dc9f758` | Tournaments, coaching comparison, time trial admin |
| Mar 05 19:51 UTC | Bono | `eecad51` | Include wallet_debit_paise and refund_paise in session detail response |

## 2026-03-06

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 06 20:25 IST | James | `bf57e2b` | Security hardening and reliability fixes across RaceControl |
| Mar 06 15:32 UTC | Bono | `168b6f5` | Nickname system, WhatsApp OTP, unified login flow |
| Mar 06 16:58 UTC | Bono | `c08aa7b` | Broadcast kiosk settings to agents on update |
| Mar 06 17:14 UTC | Bono | `d7cd82b` | Implement screen blanking in rc-agent |
| Mar 06 17:27 UTC | Bono | `fdce6fd` | Fix rc-agent build errors — deps placement + missing impls |
| Mar 06 18:03 UTC | Bono | `e5301ea` | Fix 8 PWA bugs — leaderboard parsing, auth guards, error handling |
| Mar 06 23:36 IST | James | `c1bbcd5` | PWA generates PIN, kiosk becomes PIN entry terminal |
| Mar 06 23:44 IST | James | `04efd82` | Add PIN entry numpad to pod blanking screen |
| Mar 06 18:19 UTC | Bono | `088c8c9` | Add kiosk_settings to cloud sync so pods receive settings |
| Mar 06 18:56 UTC | Bono | `16efa4f` | Cloud sync sends x-terminal-secret header for authentication |
| Mar 06 19:03 UTC | Bono | `ee06143` | Send kiosk settings to agents on WebSocket connect |
| Mar 07 00:42 IST | James | `89ed7b0` | Kiosk basePath, dynamic WS URL, AC catalog types |
| Mar 06 19:12 UTC | Bono | `16f0e7d` | Implement Linux browser launch for lock screen |
| Mar 06 19:52 UTC | Bono | `018aa59` | Add debug server to rc-agent for remote pod diagnostics |

## 2026-03-07

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 07 01:46 IST | James | `6f4c436` | Staff-authenticated kiosk with game configurator |
| Mar 07 02:44 IST | James | `1958538` | Use full path to msedge.exe for lock screen browser |
| Mar 07 03:40 IST | James | `2d7923d` | Direct launch flow — skip customer PIN waiting step |
| Mar 07 04:10 IST | James | `75a9fee` | Add 'consuming' to auth_tokens CHECK constraint |
| Mar 06 23:06 UTC | Bono | `2cfe82e` | Per-pod screen blanking control + fix is_blanked() guard bug |
| Mar 07 02:08 UTC | Bono | `4135b8a` | Venue → cloud sync for laps, billing, pods, leaderboard |
| Mar 07 08:37 IST | James | `b6983ee` | Claude CLI as primary AI debugger + expanded error aggregator |
| Mar 07 08:57 IST | James | `2b584d3` | WebSocket reconnect with exponential backoff for rc-agent |
| Mar 07 09:14 IST | James | `0cf9687` | Self-healing pod daemon with AI diagnostics (pod_healer) |
| Mar 07 10:02 IST | James | `664e999` | Staff kiosk billing flow + error handling fix |
| Mar 07 10:28 IST | James | `185834d` | Ollama learning system — train from Claude CLI responses |
| Mar 07 10:54 IST | James | `9b17274` | Surface billing error reasons to kiosk instead of generic message |
| Mar 07 11:20 IST | James | `e7c2d23` | Auto-blank/unblank screen tied to billing lifecycle |
| Mar 07 11:39 IST | James | `ba4113a` | Prevent blanking override during active billing sessions |
| Mar 07 17:25 UTC | Bono | `2f45e81` | Wallet debit endpoint, credits system, cafe wallet payments + debit atomicity fix |
| Mar 07 19:30 IST | James | `689eee2` | WebSocket reconnect stability + billing auto-cleanup |
| Mar 07 21:01 IST | James | `5cc6c86` | Architecture stabilization — action queue, RAG training, audit & tests |
| Mar 07 21:24 IST | James | `367562e` | Add bot booking endpoints for WhatsApp integration |
| Mar 07 23:18 IST | James | `f9f3a6a` | Wallet topup bonus credits (10% >= 2000, 20% >= 4000) |
| Mar 07 23:35 IST | James | `4954dd5` | Blanking screen during active sessions fix, expand car catalog to 325 |
| Mar 07 23:58 IST | James | `c0238e3` | Staff attribution for billing sessions |
| Mar 07 18:35 UTC | Bono | `6bd73fc` | Enforce wallet balance check on staff billing start |
| Mar 07 18:52 UTC | Bono | `289a8c8` | AC telemetry — sector times, lap completion, validity tracking |
| Mar 07 19:29 UTC | Bono | `495c68f` | UDP heartbeat protocol for instant pod liveness detection |
| Mar 07 20:55 UTC | Bono | `f98aafd` | Wallet sync — cloud balance is authoritative |
| Mar 07 21:47 UTC | Bono | `22c7f8d` | Sync P1/P2 — protect venue-owned fields + add pricing_rules sync |

## 2026-03-08

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 08 00:22 IST | James | `79b1bbd` | Stabilize car name display on kiosk pod card |
| Mar 08 01:38 IST | James | `305638b` | Prevent rc-agent zombie processes with Windows named mutex |
| Mar 08 01:42 IST | James | `417dd06` | Persist kiosk staff login in sessionStorage |
| Mar 08 01:46 IST | James | `05ef1d6` | Clear blank screen immediately when billing session starts |
| Mar 08 01:59 IST | James | `273db1c` | Resync active billing session when rc-agent reconnects |
| Mar 08 02:19 IST | James | `3ae1475` | Add debugging playbook for RAG knowledge sharing |
| Mar 08 02:24 IST | James | `ed946db` | Use SO_REUSEADDR for lock screen server to survive TIME_WAIT |
| Mar 08 02:59 IST | James | `71d8a2b` | Wallet balance sync push + kiosk search shows balance & phone |
| Mar 08 03:06 IST | James | `d99687c` | Wallet sync resolves driver by phone when IDs differ |
| Mar 08 04:22 IST | James | `dc9709a` | Disable lock screen overlay during active sessions, auto-blank after summary |
| Mar 08 04:41 IST | James | `2100ec5` | Racing HUD overlay with session timer, lap times, sector splits |
| Mar 08 05:03 IST | James | `ce19bff` | Wallet sync pull resolves driver by phone/email across ID mismatch |
| Mar 07 23:51 UTC | Bono | `9fadb70` | Normalize ISO timestamps in sync queries — wallets never synced after first cycle |
| Mar 08 05:29 IST | James | `1bbc40a` | Add GET /wallet/transactions endpoint |
| Mar 08 05:55 IST | James | `e8862ee` | Defer billing start until game is selected in staff kiosk flow |
| Mar 08 06:00 IST | James | `38bc3e8` | Kill orphaned kiosk Edge processes on close_browser |
| Mar 08 06:28 IST | James | `3d76f80` | Remove track/car combo and wallet balance from kiosk pod cards |
| Mar 08 06:33 IST | James | `fce55dd` | Retry overlay server bind on port conflict |
| Mar 08 06:45 IST | James | `408fab7` | Default transmission to manual, retry overlay server bind |
| Mar 08 07:01 IST | James | `17d492c` | Transmission toggle during active session + revert default to auto |
| Mar 08 07:20 IST | James | `deaea96` | Damage enabled + transmission ignored: write assists.ini alongside race.ini |
| Mar 08 07:41 IST | James | `03b0b37` | Automated post-session cleanup: kill AC/Conspit, dismiss errors, foreground lock screen |
| Mar 08 07:58 IST | James | `9faa6a6` | Redesign racing HUD overlay: speed/gear/current lap time |
| Mar 08 08:01 IST | James | `39b775f` | Keep Conspit Link running between sessions, minimize instead of killing |
| Mar 08 08:19 IST | James | `61c01b7` | FFB strength presets (Light/Medium/Strong) for kiosk |
| Mar 08 20:20 IST | James | `5ddaf98` | Pod Debug System — staff kiosk terminal |
| Mar 08 20:48 IST | James | `1d4c5b1` | VMS-style kiosk control system — 2x4 grid + per-pod customer kiosk |
| Mar 08 21:06 UTC | Bono | `14acb0d` | Billing refunds, admin driver full-profile, dynamic leaderboard |
| Mar 08 21:17 IST | James | `8d065fa` | Pod-launch-experience uses in-memory pods + JOIN for driver name |
| Mar 08 21:36 IST | James | `82d422c` | Minimize all non-game windows during active sessions |
| Mar 08 22:28 IST | James | `2e34af2` | Keep kiosk clean: minimize all windows + enforce kiosk foreground every 10s |

## 2026-03-09

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 09 00:47 IST | James | `8064559` | Content Manager integration + multiplayer support for AC launch |
| Mar 09 00:57 UTC | Bono | `54ba2c3` | Friends dashboard, multiplayer booking, ops stats overview |
| Mar 09 01:34 IST | James | `8fc1b22` | Replace modal popups with inline side panel + setup wizard |
| Mar 09 01:49 UTC | Bono | `13f15b2` | Fix 15 critical bugs found via TDD debugger audit |
| Mar 09 02:28 IST | James | `4578586` | Customer self-service booking wizard at /kiosk/book |
| Mar 09 03:01 IST | James | `856497c` | Race Engineer — unified activity log with real-time WebSocket feed |
| Mar 09 03:35 IST | James | `6453cab` | Center overlay HUD, strip Edge title bar, smooth countdown timers |
| Mar 09 03:42 IST | James | `6c4b9bd` | Pod healer false positive — health check via pod-agent exec + AI cooldown |
| Mar 09 03:54 IST | James | `15beb4f` | API error handling + visible error banner on debug page |
| Mar 09 04:25 IST | James | `032f957` | Critical bug fixes from system audit + debug UI layout |
| Mar 09 04:35 IST | James | `b75af98` | Debug UI diagnostics bar now always visible |
| Mar 09 04:44 IST | James | `a572eed` | Overlay at bottom of screen, block blank during active sessions |
| Mar 09 04:53 IST | James | `f74a5f9` | Orphaned billing sessions + pod state recovery on restart |
| Mar 09 05:23 IST | James | `a0edc5a` | Pod state sync — heartbeat merge, billing lifecycle, reconnect grace period |
| Mar 09 05:52 IST | James | `6796a15` | Staff messages and Race Engineer diagnosis now appear in activity feed |
| Mar 09 05:57 IST | James | `d444015` | Overlay moved to top of screen, hidden from taskbar |
| Mar 09 06:05 IST | James | `d7677b9` | Hydration mismatch fix on kiosk staff terminal |
| Mar 09 06:23 IST | James | `beb0cd1` | Hide free trial after use + unlimited trial test driver |
| Mar 09 06:42 IST | James | `5519dfd` | Overlay waits for HTTP server before launching Edge |
| Mar 09 07:46 IST | James | `1782f48` | Smooth 1s lock screen timer countdown (was 3s page reload jumps) |
| Mar 09 08:00 IST | James | `9f2267a` | Fix Test Driver (Unlimited) trial: UPSERT instead of INSERT OR IGNORE |
| Mar 09 08:22 IST | James | `5f9d307` | Fix 3 pod UX issues: taskbar, Conspit minimize, CM settings error |
| Mar 09 08:49 IST | James | `59387c1` | Add CM error detection for multiplayer launches |
| Mar 09 08:59 IST | James | `49c36a7` | Fix taskbar visible and game hidden behind kiosk browser |
| Mar 09 09:14 IST | James | `615a01d` | Fix stale WebSocket disconnect marking pod offline after agent restart |
| Mar 09 09:27 IST | James | `58d6106` | Fix car missing error: set CAR_0 MODEL to actual car ID in race.ini |
| Mar 09 10:01 IST | James | `150a3d5` | Fix car missing (ks_ prefix) + replace browser overlay with native Win32 window |
| Mar 09 10:17 IST | James | `d9f6713` | Stop killing Conspit Link on session start — minimize only |
| Mar 09 10:21 IST | James | `ba6505a` | Add Conspit Link crash watchdog — auto-restart and minimize |
| Mar 09 10:41 IST | James | `f6579fd` | Add F1 2020 speedometer gauge to kiosk in-session view |
| Mar 09 10:48 IST | James | `65b8716` | Add RPM color bar and enhance sector times in Racing HUD overlay |
| Mar 09 10:53 IST | James | `f656679` | Wire F1 25 telemetry adapter into Racing HUD overlay |
| Mar 09 11:03 IST | James | `0d83b62` | Add Blank Screen toggle per pod below Start Session in kiosk terminal |
| Mar 09 11:51 IST | James | `793b52a` | Fix AC shared memory offsets for sector times and improve Conspit watchdog |

## 2026-03-10

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 10 00:52 IST | James | `b2c5871` | Whitelist claude.exe and ollama.exe in pod healer |
| Mar 10 02:09 IST | James | `f0089e3` | Add enforce_safe_state() — unified pod reset for crashes, disconnects, session end |
| Mar 10 04:02 UTC | Bono | `179aa02` | Add multi-game kiosk support (AC Evo, iRacing, F1 25, LMU) + accounting & sync fixes |
| Mar 10 04:23 IST | James | `4662b1c` | AI debugger: PodStateSnapshot, auto-fix, QLoRA training pipeline |
| Mar 10 07:43 IST | James | `deaa380` | Fix AI debugger pipeline: auto-fix blocking + WebSocket delivery |
| Mar 10 09:43 IST | James | `2f2c3a4` | Add power management controls: restart endpoint + kiosk UI buttons |
| Mar 10 19:47 UTC | Bono | `868f3f6` | Unified discount system: discount fields in billing APIs, daily reports, PWA |
| Mar 10 19:49 IST | James | `e99e26c` | Simplify ConspitLink: remove from AI auto-fix, rely on watchdog |
| Mar 10 19:54 IST | James | `3803395` | Cascade update: remove stale ConspitLink AI-restart references |
| Mar 10 21:14 UTC | Bono | `de68812` | AC Session Splitting: fix timer, reservation flow, kiosk between-sessions UI |
| Mar 10 21:52 IST | James | `0dae1ec` | Fix kiosk crash-loop: add pod-agent, cmd, powershell, ConspitLink to whitelist |
| Mar 11 00:59 IST | James | `80ec001` | Fix Edge process stacking bug + static CRT build + watchdog improvements |
| Mar 10 23:44 UTC | Bono | *(pending)* | PWA game launch: agent reports installed games, F1 25 auto-launch, pod number on blanking screen |
| Mar 10 23:44 UTC | Bono | *(pending)* | Guard: prevent same driver from being on multiple pods simultaneously |

## 2026-03-11

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 11 00:38 UTC | Bono | `005f16c` | GSD: map existing codebase |
| Mar 11 00:48 IST | James | `af0cb63` | GSD: map existing codebase — 7 documents |
| Mar 11 01:04 UTC | Bono | `582c06b` | GSD: map codebase for stability/sync project |
| Mar 11 01:36 UTC | Bono | `7f2b09a` | GSD: initialize stability/sync project |
| Mar 11 01:47 UTC | Bono | `76f18cd` | GSD: complete project research |
| Mar 11 02:03 UTC | Bono | `2b7892b` | GSD: define v1 requirements |
| Mar 11 02:09 UTC | Bono | `c031a03` | GSD: create roadmap (6 phases) |
| Mar 11 02:55 UTC | Bono | `2255344` | GSD: capture phase 1 context |
| Mar 11 03:03 UTC | Bono | `023ac3e` | GSD: add phase 1 research and validation strategy |
| Mar 11 03:09 UTC | Bono | `d2b9b32` | GSD: create phase 1 plans (5 plans, 3 waves) |
| Mar 11 03:14 UTC | Bono | `d2a4c87` | feat: add wallet_transactions to push_to_cloud sync |
| Mar 11 03:15 UTC | Bono | `350d778` | feat: cloud-side upsert handler for wallet_transactions |
| Mar 11 03:16 UTC | Bono | `085f217` | feat: balance shadow verification logging |
| Mar 11 03:18 UTC | Bono | `fca8912` | feat: PWA wallet transaction history page |
| Mar 11 03:21 UTC | Bono | `07c729f` | docs: update codebase map with wallet_transactions sync |
| Mar 11 03:26 UTC | Bono | `755fa72` | feat: orphaned acServer process cleanup on racecontrol startup |
| Mar 11 03:27 UTC | Bono | `f7bc1b9` | feat: PID-based stop fallback for acServer |
| Mar 11 03:30 UTC | Bono | `f55a209` | feat: PID persistence and orphan cleanup for rc-agent games |
| Mar 11 03:37 UTC | Bono | `1b3bc94` | feat: PausedDisconnect billing status variant |
| Mar 11 03:39 UTC | Bono | `72b9e46` | feat: pause tracking columns and BillingTimer fields |
| Mar 11 03:44 UTC | Bono | `fb18b4b` | feat: disconnect-pause state machine in billing tick |
| Mar 11 03:45 UTC | Bono | `b0996e4` | test: partial refund calculation unit test |
| Mar 11 03:46 UTC | Bono | `e0faff7` | test: serde roundtrip tests for pause/resume protocol messages |
| Mar 11 03:56 UTC | Bono | `5c9234c` | fix: reject startup with default JWT secret |
| Mar 11 03:57 UTC | Bono | `31ef835` | fix: replace panic-risk unwrap() in routes.rs |
| Mar 11 03:57 UTC | Bono | `463966b` | fix: replace unwrap() in scheduler, pod_healer, websocket |
| Mar 11 03:59 UTC | Bono | `dbd37cd` | fix: add error logging to silenced .ok() calls |
| Mar 11 04:02 UTC | Bono | `a085ec9` | revert: undo error-handling commits — re-execute as port allocation |
| Mar 11 04:06 UTC | Bono | `4638aaf` | feat: PortAllocator for dynamic AC server port assignment |
| Mar 11 04:09 UTC | Bono | `b620c6b` | feat: integrate PortAllocator into AC server lifecycle |
| Mar 11 06:29 IST | James | `0559388` | GSD: initialize Racing HUD & Wheelbase Safety project |
| Mar 11 06:45 IST | James | `ce80bf3` | GSD: complete domain research (HUD + FFB safety) |
| Mar 11 06:55 IST | James | `5ea6820` | GSD: define v1 requirements (19 reqs) |
| Mar 11 06:59 IST | James | `b030f1a` | GSD: create roadmap (4 phases, 19 requirements) |
| Mar 11 07:08 IST | James | `93b9b59` | feat: FFB safety — zero wheelbase torque on session end and startup |
| Mar 11 08:21 IST | James | `f1becab` | docs: HUD infrastructure research and validation strategy |
| Mar 11 08:30 IST | James | `9fff055` | docs: plan 02-01 with checker fixes |
| Mar 11 08:40 IST | James | `146585f` | feat(overlay): GDI resource cache + compute_layout + characterization tests |
| Mar 11 08:56 IST | James | `e1acc69` | feat(overlay): HudComponent trait + dispatcher + GDI leak detector |
| Mar 11 09:26 IST | James | `fe6c42a` | fix(hud): sync timer with game, first lap detection, sector format, dynamic RPM |
| Mar 11 09:31 IST | James | `f5d1191` | fix(ac): snapshot initial completed_laps to prevent stale lap detection |

## 2026-03-12

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 12 00:50 UTC | Bono | `557b20c` | test: integration test infrastructure |
| Mar 12 00:53 UTC | Bono | `1c2a9d9` | test: E2E smoke test script |
| Mar 12 00:55 UTC | Bono | `7435b01` | docs: complete E2E verification test suite plan |
| Mar 12 01:01 UTC | Bono | `598af52` | docs: complete phase 01 execution |
| Mar 12 01:23 UTC | Bono | `322fbad` | docs: phase 1.1 research — cloud-venue WebSocket sync |
| Mar 12 01:24 UTC | Bono | `ae4a579` | docs: phase 1.1 validation strategy |
| Mar 12 01:33 UTC | Bono | `3ac10d4` | docs: phase 1.1 plans — Cloud-Venue WebSocket Sync |
| Mar 12 01:47 UTC | Bono | `fbd22fe` | fix: revise phase 1.1 plans based on checker feedback |
| Mar 12 07:16 IST | James | `7996122` | fix: revise plan 02-01 based on checker feedback |
| 2026-03-20 11:30 IST | James | `4af3f5b` | feat(51-01): create CLAUDE.md — 179-line Racing Point context file (all 8 pod IPs/MACs, crate names, deploy rules, 4-tier debug, standing rules, brand identity) |
| 2026-03-20 11:30 IST | James | `d19b916` | docs(51-01): SUMMARY + STATE/ROADMAP/REQUIREMENTS updated — SKILL-01 complete |
| 2026-03-20 11:53 IST | James | `d1fd50b` | docs(52): research Phase 52 MCP Servers — Gmail fix, Sheets/Calendar scaffold, rc-ops-mcp API surface |
| 2026-03-20 12:15 IST | James | `e90532b` | feat(52-01): add OAuth token refresh utility (refresh-token.js) in racingpoint-google |
| 2026-03-20 12:15 IST | James | `7f0ce1e` | feat(52-01): create racingpoint-mcp-sheets — read_sheet and write_sheet MCP tools |
| 2026-03-20 12:15 IST | James | `7e2c8ce` | feat(52-01): create racingpoint-mcp-calendar — list_events, create_event, delete_event MCP tools |
| 2026-03-20 12:15 IST | James | `6e9eca9` | docs(52-01): SUMMARY + STATE/ROADMAP/REQUIREMENTS updated — Tasks 1+2 complete, OAuth checkpoint reached |
| 2026-03-20 12:18 IST | James | `40214df` | feat(52-02): create rc-ops-mcp MCP server with 10 racecontrol API tools (in rc-ops-mcp repo) |
| 2026-03-20 12:18 IST | James | `0a241c8` | docs(52-02): complete rc-ops-mcp plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated |
| 2026-03-20 13:36 IST | James | `9f7addd` | feat(53-02): add /rp:deploy-fleet canary-first fleet deploy skill — Pod 8 + verify.sh gate + approval prompt before pods 1-7 |
| 2026-03-20 13:36 IST | James | `acee734` | docs(53-02): complete deploy-fleet plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated (DEPLOY-02, DEPLOY-03) |
| 2026-03-20 13:36 IST | James | `290e0a6` | chore(53-01): register RacingPoint-StagingHTTP and RacingPoint-WebTerm Task Scheduler tasks (ONLOGON, user bono) |
| 2026-03-20 13:37 IST | James | `4f260bd` | feat(53-01): add tests/e2e/deploy/auto-start.sh — liveness check for :9998 and :9999 |
| 2026-03-20 13:38 IST | James | `563e9c4` | docs(53-01): complete autostart plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated (DEPLOY-01) |
| 2026-03-20 16:14 IST | James | `451b4c6` | feat(54-01): structured JSON logging — racecontrol file layer uses .json(), daily rotation to racecontrol-YYYY-MM-DD.jsonl, 30-day cleanup on startup |
| 2026-03-20 16:14 IST | James | `b667d6a` | docs(54-01): complete structured-json-logging plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated (MON-01) |
| 2026-03-20 14:17 IST | James | `0c42b1a` | feat(54-02): rc-agent structured JSON logging — daily-rotating rc-agent-YYYY-MM-DD.jsonl, pod_id span injected after config load, 30-day cleanup, stdout stays plain text |
| 2026-03-20 14:20 IST | James | `0392281` | docs(54-02): complete rc-agent JSON logging plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated (MON-02) |
| 2026-03-20 14:21 IST | James | `73a1bbb` | feat(54-03): ErrorCountLayer with sliding window VecDeque, mpsc bridge to async alerter task, 4 unit tests |
| 2026-03-20 14:27 IST | James | `47293b2` | feat(54-03): MonitoringConfig in config.rs + ErrorCountLayer wired into tracing registry in main.rs |
| 2026-03-20 14:28 IST | James | `123ffd5` | docs(54-03): complete error rate alerting plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS (MON-03) updated |
| 2026-03-20 15:00 IST | James | `17e7b95` | feat(55-01): add Netdata fleet E2E verification script (netdata-fleet.sh checks all 9 hosts :19999/api/v1/info) |
| 2026-03-20 15:13 IST | James | `6388f93` | docs(55-01): complete Netdata deploy scripts plan — SUMMARY, STATE, ROADMAP, LOGBOOK updated |
| 2026-03-20 16:09 IST | James | `pending` | chore(66-01): server .23 NIC confirmed static IP 192.168.31.23 — PrefixOrigin Manual, DHCP disabled, DNS set to 192.168.31.1 |
| 2026-03-20 10:39 IST | James | 2833425 | feat(66-03): wire Bono ExecHandler + 4 failover COMMAND_REGISTRY entries + James exec_result handler |
| 2026-03-20 16:17 IST | James | dc9fbf5 | feat(56-01): add WhatsApp P0 alerter module with AlertingConfig and SQLite tables |
| 2026-03-20 16:17 IST | James | 309e218 | feat(56-01): convert error_rate mpsc to broadcast + wire whatsapp_alerter in main.rs |
| 2026-03-20 16:17 IST | James | b642e98 | docs(56-01): complete WhatsApp P0 alerter plan — SUMMARY, STATE, ROADMAP updated |
| 2026-03-20 17:01 IST | James | 41528ff | chore(66-02): verify rc-agent :8090 exec via Tailscale 100.71.226.83 + LAN 192.168.31.23 — both paths confirmed, server hostname returned |
| 2026-03-20 17:28 IST | James | cb177a1 | feat(66-04): add POST /relay/exec/send endpoint for James->Bono exec_request |
| 2026-03-20 17:28 IST | James | 35cea4f | comms: notify Bono of 66-04 exec/send endpoint |
| 2026-03-20 17:28 IST | James | fe2c042 | docs(66-04): complete exec/send relay endpoint plan — SUMMARY, STATE, ROADMAP updated |
| 2026-03-20 17:30 IST | James | 075414e | feat(71-01): add rc-common exec module with ExecResult, run_cmd_sync, run_cmd_async, 5 unit tests |
| 2026-03-20 17:31 IST | James | dc99840 | feat(71-01): wire rc-common into rc-sentry, zero tokio in cargo tree (SHARED-01..03) |
| 2026-03-20 17:31 IST | James | c197515 | docs(71-01): complete rc-common exec module plan — SUMMARY, STATE, ROADMAP updated |
| 2026-03-20 17:44 IST | James | b8a43b9 | feat(71-02): harden rc-sentry — timeout via run_cmd_sync, SlotGuard/429, Content-Length TCP read loop, tracing (SHARD-01..05) |
| 2026-03-20 17:45 IST | James | c45aac7 | docs(71-02): complete rc-sentry core hardening plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated |
| 2026-03-20 18:28 IST | James | 0d0baf6 | chore(72-01): add build.rs and sysinfo/winapi dependencies to rc-sentry |
| 2026-03-20 18:28 IST | James | 185eb7d | feat(72-01): add 4 endpoints + graceful shutdown to rc-sentry |
| 2026-03-20 18:28 IST | James | 971856d | docs(72-01): complete rc-sentry endpoint expansion plan |
| 2026-03-20 18:34 IST | James | 2a7e72b | feat(72-02): add 7 inline integration tests to rc-sentry — all endpoints covered, ephemeral ports, no tokio (TEST-04) |
| 2026-03-20 18:35 IST | James | c952a9d | docs(72-02): complete rc-sentry integration tests plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated |
| 2026-03-20 18:37 IST | James | 956efde | feat(67-01): config sanitizer with allowlist -- venue/pods/branding only (SYNC-01, SYNC-02) |
| 2026-03-20 18:38 IST | James | a3b2cdc | feat(67-01): config watcher -- poll + SHA-256 change detection + sanitized snapshot |
| 2026-03-20 18:38 IST | James | 406628b | feat(67-01): wire ConfigWatcher into james/index.js for config sync |
| 2026-03-20 18:39 IST | James | 5b9aa74 | docs(67-01): complete config-sync plan 1 -- watcher + sanitizer SUMMARY, STATE, ROADMAP, REQUIREMENTS |
| 2026-03-20 18:50 IST | James | e7366cb | feat(67-02): add VenueConfigSnapshot to AppState and config_snapshot to sync_push |
| 2026-03-20 18:50 IST | James | f5a9a71 | test(67-02): unit tests for config_snapshot parsing -- 3 tests pass |
| 2026-03-20 18:51 IST | James | b8a738b | docs(67-02): complete config-sync cloud-receive plan -- SUMMARY, STATE, ROADMAP, REQUIREMENTS |
| 2026-03-20 18:56 IST | James | 1887334 | feat(73-01): add FfbBackend trait seam + mockall mock tests (TEST-03) |
| 2026-03-20 18:57 IST | James | f0cb55f | docs(73-01): complete FfbBackend trait seam plan -- SUMMARY, STATE, ROADMAP, REQUIREMENTS |
| 2026-03-20 19:03 IST | James | bc2cfc9 | test(73-02): add timer+channel tests for BILL-02/BILL-03 (TEST-01) -- 6 async tests, tokio::time::Instant |
| 2026-03-20 19:04 IST | James | 076f905 | test(73-02): add requirement-named tests for CRASH-01/CRASH-02 (TEST-02) -- 6 condition tests |
| 2026-03-20 19:20 IST | James | 6e51331 | docs(73-02): complete critical business tests plan -- SUMMARY, STATE, ROADMAP |
| 2026-03-20 19:30 IST | James | cccd7c9 | feat(68-01): add SwitchController variant to CoreToAgentMessage + serde round-trip test |
| 2026-03-20 19:30 IST | James | c26f939 | feat(68-01): add failover_url to CoreConfig, last_switch_ms to HeartbeatStatus, 4 unit tests |
| 2026-03-20 19:37 IST | James | b4dde24 | feat(68-02): wire Arc<RwLock<String>> active_url into reconnect loop + SwitchController handler |
| 2026-03-20 19:38 IST | James | 766b1da | feat(68-02): add last_switch_ms guard to self_monitor WS-dead relaunch check + 3 unit tests |
| 2026-03-20 19:50 IST | James | cde40a7 | feat(77-02): wire HTTPS listener, security headers, CORS update in main.rs |
| 2026-03-20 19:50 IST | James | e165e7b | feat(77-02): protocol-aware kiosk API_BASE -- no mixed content on HTTPS |
| 2026-03-21 06:27 IST | James | 0849580 | feat(69-01): create HealthMonitor FSM — 12-tick/60s hysteresis, server_down event (comms-link) |
| 2026-03-21 06:27 IST | James | 4545729 | feat(69-01): create FailoverOrchestrator + wire into james/index.js (comms-link) |
| 2026-03-21 06:28 IST | James | df19db4 | docs(69-01): 69-01-SUMMARY.md + STATE.md + ROADMAP.md |
| 2026-03-21 06:24 IST | James | 92bd65b | feat(69-02): add POST /api/v1/failover/broadcast endpoint to racecontrol |
| 2026-03-21 06:25 IST | James | 02030f4 | feat(69-02): add split-brain guard to rc-agent SwitchController handler |
| 2026-03-21 06:25 IST | James | 67ffcab | docs(69-02): complete failover-broadcast + split-brain guard plan |
| 2026-03-21 06:28 IST | James | d1e2048 | feat(78-01): harden Edge kiosk flags + keyboard hook (KIOSK-01, KIOSK-02) |
| 2026-03-21 06:29 IST | James | 1a5bbe3 | feat(78-01): extend pod-lockdown with USB, accessibility, TaskMgr lockdown (KIOSK-03, KIOSK-04) |
| 2026-03-21 06:30 IST | James | cdc3755 | docs(78-01): complete kiosk session hardening plan |
| 2026-03-21 06:28 IST | James | 02019c6 | feat(74-01): extract config.rs from main.rs — AgentConfig, 7 structs, load/validate/detect, 20 tests (DECOMP-01) |
| 2026-03-21 06:30 IST | James | 067fcbe | docs(74-01): 74-01-SUMMARY.md, STATE.md phase 74 progress, REQUIREMENTS DECOMP-01 complete |
| 2026-03-21 06:31 IST | James | 7d1fe0a | feat(69-03): secondary watchdog in bono/index.js for venue power outage failover (HLTH-04) |
| 2026-03-21 06:31 IST | James | 3e9cc7b | docs(69-03): complete secondary watchdog plan summary and state updates |
| 2026-03-21 06:28 IST | James | e6d2ff4 | feat(78-02): network source classification middleware with IP tagging (KIOSK-07) |
| 2026-03-21 06:37 IST | James | 62b603a | feat(78-02): wire source middleware and protect staff routes from pod access (KIOSK-05) |
| 2026-03-21 06:37 IST | James | 69b9508 | docs(78-02): complete network source tagging plan summary and state updates |

## 2026-03-21

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| 2026-03-21 07:08 IST | James | 7ce7abf (comms-link) | feat(69-04): notify_failover COMMAND_REGISTRY + EXEC_REASON + send-email.js + watchdog fix + email on both failover paths (ORCH-04) |
| 2026-03-21 07:10 IST | James | e5d5f78 (racecontrol) | docs(69-04): SUMMARY.md + STATE.md + ROADMAP.md update |
| 2026-03-21 13:28 IST | James | 4c7a591 | feat(74-02): extract AppState struct from main.rs — 34 pub(crate) fields, all reconnect loop refs updated to state.field pattern (DECOMP-02) |
| 2026-03-21 13:35 IST | James | bde40b3 | docs(74-02): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md updates |
| 2026-03-21 07:00 IST | James | 73244a9 | feat(81-02): GamePickerPanel + game logo display on pod card (LAUNCH-01, LAUNCH-06) |
| 2026-03-21 07:02 IST | James | 5270be2 | feat(81-02): GameLaunchRequestBanner + PWA request WebSocket handling (LAUNCH-02) |
| 2026-03-21 07:03 IST | James | 066ef8f | docs(81-02): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md updates |
| 2026-03-21 07:11 IST | James | ce15e55 | feat(81-01): non-AC crash recovery calling GameProcess::launch() + DashboardEvent::GameLaunchRequested variant |
| 2026-03-21 07:12 IST | James | e04805c | feat(81-01): POST /api/v1/customer/game-request -- validates pod/game, broadcasts GameLaunchRequested |
| 2026-03-21 07:13 IST | James | 59f9b95 | docs(81-01): 81-01-SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md |
| 2026-03-21 07:18 IST | James | 985b3db | feat(74-03): extract ws_handler.rs with handle_ws_message() dispatching 22 CoreToAgentMessage variants; select! ws_rx arm to 27-line delegation; main.rs 3009->2037 lines (DECOMP-03) |
| 2026-03-21 07:18 IST | James | d7c42ac | docs(74-03): 74-03-SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md updates |
| 2026-03-21 07:21 IST | James | fa8a0be | feat(70-02): add COMMAND_REGISTRY entries (export_failover_sessions, notify_failback) + HealthMonitor server_recovery event (down->healthy guard) |
| 2026-03-21 07:21 IST | James | e1826d5 | feat(70-02): FailoverOrchestrator.initiateFailback() 9-step sequence + #httpGet helper + failoverStartedAt tracking + james/index.js server_recovery wiring |
| 2026-03-21 07:21 IST | James | 6effd53 | docs(70-02): 70-02-SUMMARY.md + STATE.md + ROADMAP.md updates |
