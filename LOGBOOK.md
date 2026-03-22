# RaceControl Logbook

Chronological record of all changes by Bono (cloud) and James (venue).
Both must append here when committing. Format: `| timestamp | author | commit | summary |`

---

## 2026-03-22

| Timestamp | Author | Commit | Summary |
|-----------|--------|--------|---------|
| Mar 22 12:18 IST | James | `0b5ee831` | 154-03: Add CafeOrderItem/Response types and placeCafeOrder API method |
| Mar 22 12:18 IST | James | `13156b26` | 154-03: Transform CafeMenuPanel into POS order builder for staff |
| Mar 22 12:18 IST | James | `6f31118f` | 154-03: docs — SUMMARY, STATE, ROADMAP updated |
| Mar 22 22:09 IST | James | `08779e48` | 154-02: Add stock fields to CafeMenuItem and placeCafeOrder API method |
| Mar 22 22:09 IST | James | `363c70fc` | 154-02: Add cart, checkout flow, and order submission to cafe PWA page |
| Mar 22 22:09 IST | James | `e06327b3` | 154-02: docs — SUMMARY, STATE, ROADMAP updated (phase 154 Complete) |
| Mar 22 22:30 IST | James | `10078e53` | 155-01: send_order_receipt_whatsapp + Step L spawn + CafeConfig in config |
| Mar 22 22:30 IST | James | `6864c454` | 155-01: print_thermal_receipt + list_customer_orders + GET /customer/cafe/orders/history |
| Mar 22 22:30 IST | James | `52a9f8f9` | 155-01: docs — SUMMARY, STATE, ROADMAP updated |
| Mar 22 22:45 IST | James | `24ff9223` | 155-02: Add CafeOrderHistoryItem type and getCafeOrderHistory to api.ts |
| Mar 22 22:45 IST | James | `00b50b93` | 155-02: Build /cafe/orders page with expand/collapse order history |
| Mar 22 22:45 IST | James | `729787c7` | 155-02: docs — SUMMARY, STATE, ROADMAP updated |

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
| 2026-03-21 07:38 IST | Bono | 9d95a18 | feat(89-01): 7 psychology tables (achievements, driver_achievements, streaks, driving_passport, nudge_queue, staff_badges, staff_challenges) + 8 indexes added to db/mod.rs migration |
| 2026-03-21 13:27 IST | James | 8ab3775 | feat(88-01): track name normalization + sim_type-scoped PB/TR schema — TRACK_NAME_MAP 28 cross-game mappings, personal_bests/track_records PK extended to include sim_type, migrate_leaderboard_sim_type() idempotent migration |
| 2026-03-21 13:38 IST | James | c754a9c | feat(88-01): wire track normalization and sim_type scoping into persist_lap — normalized_track through all DB writes, PB/TR queries sim_type-scoped, get_previous_record_holder gains sim_type param |
| 2026-03-21 13:38 IST | James | 91bc7b9 | docs(88-01): 88-01-SUMMARY.md + STATE.md + ROADMAP.md updates |
| 2026-03-21 07:38 IST | Bono | a620a52 | feat(89-01): psychology.rs module skeleton — NotificationChannel, NudgeStatus, MetricType, Operator, BadgeCriteria, parse_criteria_json, evaluate_criteria, 5 async stubs, 13 unit tests passing |
| 2026-03-21 07:38 IST | Bono | 865e1fb | docs(89-01): 89-01-SUMMARY.md + STATE.md update |
| 2026-03-21 07:38 IST | Bono | f26032a | docs(89-01): ROADMAP.md phase 89 progress update |
| 2026-03-21 08:26 IST | James | c4411f7 | feat(79-03): DPDP data export + cascade delete endpoints with 8 tests |
| 2026-03-21 08:26 IST | James | dfa7742 | docs(79-03): complete DPDP data rights plan -- export + cascade delete |
| 2026-03-21 09:34 IST | Bono | 4486468 | feat(90-01): update_driving_passport() + backfill_driving_passport() in psychology.rs; get_featured_tracks/cars_for_passport() in catalog.rs; persist_lap wired to upsert passport on every valid lap |
| 2026-03-21 09:34 IST | Bono | 104a22c | feat(90-01): GET /customer/passport (lazy backfill + Starter/Explorer/Legend tiers) + GET /customer/badges (earned + available with progress) endpoints in routes.rs; fix pre-existing AppState::new arity in psychology tests |
| 2026-03-21 09:34 IST | Bono | 0c9d28c | docs(90-01): 90-01-SUMMARY.md + STATE.md + ROADMAP.md |
| 2026-03-21 11:00 IST | James | 60f7d9e | feat(82-01): shared types -- GameState::Loading, PlayableSignal, protocol sim_type |
| 2026-03-21 11:00 IST | James | 80f32d1 | feat(82-01): DB migration + per-game billing engine + API sim_type |
| 2026-03-21 11:00 IST | James | 2080395 | docs(82-01): complete server-side billing foundation plan |
| 2026-03-21 09:24 IST | James | be61b1f | feat(97-01): add PreFlightPassed, PreFlightFailed, ClearMaintenance protocol variants |
| 2026-03-21 09:34 IST | James | 70612c2 | feat(97-01): add PreflightConfig struct and wire into AgentConfig |
| 2026-03-21 09:34 IST | James | 411b779 | docs(97-01): complete protocol variants + PreflightConfig plan |
| 2026-03-21 09:44 IST | James | 1064f1f | feat(97-02): create pre_flight.rs with concurrent check runner (HID, ConspitLink, orphan game) and auto-fix |
| 2026-03-21 09:44 IST | James | 40467d8 | feat(97-02): wire pre-flight gate into BillingStarted — billing_active.store(true) inside Pass branch only |
| 2026-03-21 09:44 IST | James | 89325ed | docs(97-02): complete pre_flight.rs + ws_handler gate plan |
| 2026-03-21 10:15 IST | James | 9bb4f92 | docs(98): research phase — MaintenanceRequired lock screen + display checks (PF-04, PF-05, PF-06, DISP-01, DISP-02) |
| 2026-03-21 10:05 IST | James | 0dedde2 | test(98-01): add failing tests for MaintenanceRequired lock screen variant (TDD RED) |
| 2026-03-21 10:20 IST | James | 41c952a | feat(98-02): DISP-01 HTTP probe (127.0.0.1:18923) + DISP-02 GetWindowRect (Chrome_WidgetWin_1) in pre_flight.rs — 5 concurrent checks, 4 new tests (DISP-01, DISP-02) |
| 2026-03-21 10:22 IST | James | 5ac39ee | feat(98-02): 30-second maintenance retry loop in event_loop.rs — auto-clears in_maintenance on Pass, sends PreFlightPassed, refreshes screen on failure (PF-06) |
| 2026-03-21 10:25 IST | James | 5393770 | docs(98-02): complete display checks + maintenance retry plan — 98-02-SUMMARY.md, STATE.md, ROADMAP.md, REQUIREMENTS.md |
| 2026-03-21 10:10 IST | James | 6ba5372 | feat(98-01): MaintenanceRequired LockScreenState variant + show/is methods + render fn + health/idle updates (PF-04, PF-05) |
| 2026-03-21 10:15 IST | James | cb79088 | feat(98-01): in_maintenance AtomicBool on AppState + ClearMaintenance handler in ws_handler |
| 2026-03-21 10:17 IST | James | 11390d5 | docs(98-01): complete plan — 98-01-SUMMARY.md, STATE.md, ROADMAP.md, REQUIREMENTS.md |
| 2026-03-21 09:59 IST | James | 483b4dc | test(83-01): add 6 F1 25 unit tests — lap completion, sector splits, invalid lap, session type mapping, first-packet safety, take semantics (TEL-F1-01, TEL-F1-02, TEL-F1-03) |
| 2026-03-21 09:59 IST | James | 4f14435 | docs(83-01): complete F1 25 telemetry test coverage plan — 83-01-SUMMARY.md, STATE.md, ROADMAP.md |
| 2026-03-21 10:48 IST | James | 9a4234b | feat(99-01): 4 new pre-flight checks (billing_stuck, disk_space, memory, ws_stability) + 9-way tokio::join! runner (SYS-02, SYS-03, SYS-04, NET-01) |
| 2026-03-21 10:49 IST | James | dad850c | docs(99-01): complete system/network billing checks plan — 99-01-SUMMARY.md, STATE.md, ROADMAP.md, REQUIREMENTS.md |
| 2026-03-21 11:30 IST | James | 651249d | feat(84-01): IracingAdapter — shared memory, lap detection, session transitions, pre-flight, 8 unit tests (TEL-IR-01, TEL-IR-02, TEL-IR-03, TEL-IR-04) |
| 2026-03-21 10:55 IST | James | 71e75b7 | feat(99-02): add last_preflight_alert field to AppState — Option<Instant> for STAFF-04 rate-limiting |
| 2026-03-21 10:55 IST | James | afed1c2 | feat(99-02): wire PreFlightFailed alert rate-limiting (STAFF-04) — 60s cooldown in BillingStarted, retry loop confirmed no-alert by design, reset on Pass |
| 2026-03-21 10:56 IST | James | 21c5ee5 | docs(99-02): complete alert rate-limiting plan — 99-02-SUMMARY.md, STATE.md, ROADMAP.md, REQUIREMENTS.md |
| 2026-03-21 11:15 IST | James | 5f27c06 | feat(84-02): wire IracingAdapter and PlayableSignal — main.rs IRacing arm, event_loop.rs IsOnTrack billing trigger, 90s fallback retained for other sims |
| 2026-03-21 11:15 IST | James | a4337d7 | docs(84-02): complete iRacing wiring plan — 84-02-SUMMARY.md, STATE.md, ROADMAP.md, phase 84 complete |
| 2026-03-21 11:42 IST | James | 65f6c4e | feat(100-01): add in_maintenance + maintenance_failures to FleetHealthStore + PodFleetStatus + 2 unit tests (STAFF-03) |
| 2026-03-21 11:50 IST | James | f9688ec | feat(100-01): wire PreFlightFailed/Passed WS handlers + POST /pods/{id}/clear-maintenance endpoint (STAFF-02, STAFF-03) |
| 2026-03-21 11:55 IST | James | 0156d19 | docs(100-01): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md updates |
| 2026-03-21 12:30 IST | James | 1161d80 | feat(85-01): LmuAdapter — rF2 shared memory reader for Le Mans Ultimate (Scoring + Telemetry buffers, torn-read guard, sector splits, first-packet safety, session transition, 6 unit tests) |
| 2026-03-21 12:32 IST | James | 487c6a7 | docs(85-01): 85-01-SUMMARY.md + STATE.md + ROADMAP.md updates |
| 2026-03-21 11:41 IST | James | 4b9e6a8 | feat(100-02): add in_maintenance+maintenance_failures to PodFleetStatus TypeScript type; api.clearMaintenance method (STAFF-01, STAFF-02) |
| 2026-03-21 11:55 IST | James | af45623 | feat(100-02): fleet page Racing Red Maintenance badge + PIN-gated modal + Clear Maintenance button (STAFF-01, STAFF-02) |
| 2026-03-21 11:57 IST | James | 58c3a36 | docs(100-02): 100-02-SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md — phase 100 complete |
| 2026-03-21 12:51 IST | Bono | e643174 | feat(91-01): PbAchieved broadcast + compute_percentile + enhanced session detail + active-session polling endpoint |
| 2026-03-21 12:52 IST | Bono | de363ba | feat(91-01): install canvas-confetti + sonner + update TypeScript types for session experience (Phase 91) |
| 2026-03-21 12:52 IST | Bono | 1c5410f | docs(91-01): 91-01-SUMMARY.md + STATE.md + ROADMAP.md updates |
| 2026-03-21 13:31 IST | Bono | fc329f2 | feat(92-01): variable_reward_log DB table + four retention functions in psychology.rs (notify_pb_beaten_holders, maybe_grant_variable_reward, check_streak_at_risk, check_membership_expiry_warnings) |
| 2026-03-21 13:33 IST | Bono | 1dca228 | feat(92-01): wire retention triggers in lap_tracker, billing, scheduler, passport API (grace_expires_date + longest_streak + last_visit_date) |
| 2026-03-21 13:35 IST | Bono | 8c2af57 | docs(92-01): retention loops SUMMARY.md + STATE.md + ROADMAP.md updates |
| 2026-03-21 13:50 IST | Bono | fe16023 | feat(92-02): enhance passport streak card with grace urgency and longest streak (red border, days countdown, best streak) |
| 2026-03-21 13:51 IST | Bono | 11f37b4 | fix(92-02): scope ThreadRng before .await to satisfy tokio Send bound in psychology.rs |
| 2026-03-21 13:52 IST | Bono | 9f109d4 | docs(92-02): 92-02-SUMMARY.md + STATE.md + ROADMAP.md — phase 92 complete |
| 2026-03-21 14:30 IST | James | b76787b | docs(102): create phase plan — whitelist schema + config + fetch endpoint (GUARD-01,02,03,06) |
| 2026-03-21 15:10 IST | James | 17750da | feat(102-01): ProcessGuardConfig + AllowedProcess + ProcessGuardOverride structs; 6 TDD tests pass; C:/RacingPoint/racecontrol.toml with 185 global allowed entries + 3 per-machine overrides |
| 2026-03-21 13:48 IST | James | d88f422 | feat(88-02): add sim_type filtering to all leaderboard endpoints — public, public_track, staff track, bot |
| 2026-03-21 13:49 IST | James | 42775d9 | docs(88-02): complete leaderboard sim_type filtering plan — SUMMARY.md + STATE.md + ROADMAP.md |
| 2026-03-21 13:57 IST | James | ad364f3 | fix(102-02): remove duplicate EnterFreedomMode/ExitFreedomMode variants in CoreToAgentMessage — blocked all racecontrol tests |
| 2026-03-21 13:57 IST | James | 8b4aebb | docs(102-02): 102-02-SUMMARY.md + STATE-v12.1.md — phase 102 complete, GET /api/v1/guard/whitelist/{machine_id} endpoint verified |
| 2026-03-21 14:18 IST | James | c6ef4c2 | feat(103-01): add ProcessGuardConfig to config.rs and walkdir dep |
| 2026-03-21 14:18 IST | James | 93060ed | feat(103-01): add guard_whitelist and guard_violation channel to AppState |
| 2026-03-21 14:18 IST | James | 368ac82 | docs(103-01): complete ProcessGuardConfig foundations plan |
| 2026-03-21 14:43 IST | James | 482079b | feat(103-02): implement process_guard.rs — spawn_blocking sysinfo scan, two-cycle grace, PID-verified taskkill, CRITICAL racecontrol.exe, 512KB log rotation, 9 tests |
| 2026-03-21 14:43 IST | James | b6e2344 | docs(103-02): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md — PROC-01-05, ALERT-01, ALERT-04 complete |
| 2026-03-21 14:57 IST | James | b5035a1 | feat(103-03): add run_autostart_audit() + autostart tests to process_guard.rs (TDD, 17 tests green) |
| 2026-03-21 14:57 IST | James | 3416f9e | feat(103-03): wire process guard into rc-agent — whitelist fetch, spawn, WS drain, UpdateProcessWhitelist handler |
| 2026-03-21 14:57 IST | James | a79f042 | docs(103-03): Phase 103 complete — AUTO-01/02/04, DEPLOY-01 requirements satisfied |
| 2026-03-21 15:45 IST | James | 6ca77f0 | feat(106-01): add build_id to root span and migrate all 65 tracing calls in main.rs to LOG_TARGET |
| 2026-03-21 15:45 IST | James | bc2e18f | docs(106-01): SUMMARY.md + STATE.md + ROADMAP.md — LOG-01, LOG-02, LOG-03 complete |
| 2026-03-21 16:15 IST | James | 00e0bd3 | feat(106-02): migrate ws_handler.rs (60) and event_loop.rs (53) to structured target: labels |
| 2026-03-21 16:20 IST | James | 578d053 | feat(106-02): migrate ac_launcher.rs (51 calls) to structured target: labels — 164 total migrated |
| 2026-03-21 17:01 IST | James | 99412bc | feat(106-04): migrate remote_ops.rs, self_monitor.rs, self_heal.rs to structured log labels |
| 2026-03-21 17:01 IST | James | b4d0588 | feat(106-04): migrate game_process.rs, overlay.rs, billing_guard.rs, pre_flight.rs to structured log labels |
| 2026-03-21 17:01 IST | James | a47847f | docs(106-04): complete structured log labels plan 04 — 79 tracing calls in 7 files migrated |
| 2026-03-21 17:15 IST | James | 8dbd3ba | feat(106-05): migrate utility/monitor files to structured target: labels — 8 files, 36 call sites |
| 2026-03-21 17:20 IST | James | 046f6a7 | feat(106-05): migrate sim module files to structured target: labels — 5 files, 32 call sites |
| 2026-03-21 17:25 IST | James | 942f5ed | docs(106-05): complete plan — all remaining rc-agent files structured log labels (13 files, 68 calls) |
| 2026-03-21 15:42 IST | James | 1aa9bf1 | feat(106-03): migrate ffb_controller.rs and ai_debugger.rs to structured target labels (44+28 calls) |
| 2026-03-21 15:44 IST | James | 570f775 | feat(106-03): migrate kiosk.rs and lock_screen.rs to structured target labels (21+20 calls) |
| 2026-03-21 15:45 IST | James | af60071 | docs(106-03): complete plan 03 — 114 tracing calls migrated, [rc-bot] eliminated, kiosk-llm preserved |
| 2026-03-21 18:10 IST | James | 47bc75d | feat(106-06): final audit — strip 43 bracket prefixes, fix pre-existing test failure (test_auto_fix_no_match) |
| 2026-03-21 18:12 IST | James | d4bc612 | docs(106-06): complete final audit — Phase 106 structured log labels 100% migrated, 418 tests pass |
| 2026-03-21 16:17 IST | James | d37f083 | feat(104-01): ViolationStore + pod_violations AppState field + fleet_health_handler violation stats |
| 2026-03-21 16:19 IST | James | 42ebcb6 | feat(104-01): ProcessViolation WS handler + email escalation for repeat offenders |
| 2026-03-21 17:00 IST | James | c8f8324 | feat(104-02): spawn_server_guard() server scan loop — rc-agent.exe CRITICAL, 512KB log rotation, pod_violations["server"] |
| 2026-03-21 17:02 IST | James | 151bf37 | docs(104-02): complete server guard module plan — STATE, ROADMAP, REQUIREMENTS, SUMMARY |
| 2026-03-21 16:41 IST | James | 9506d1d | feat(104-03): violation badge on kiosk fleet grid — PodFleetStatus violation_count_24h + last_violation_at, Racing Red badge |
| 2026-03-21 IST | James | 512166f | feat(105-02): post_guard_report_handler + report_secret config field — X-Guard-Token auth, ViolationStore reuse |
| 2026-03-21 IST | James | bd2f78e | feat(105-02): register POST /guard/report in service_routes() — rc-process-guard James HTTP intake |
| 2026-03-21 IST | James | 961276b | docs(105-02): complete guard/report intake endpoint plan — SUMMARY, STATE, ROADMAP |
| 2026-03-21 IST | James | 8663a45 | feat(105-01): add parse_netstat_listening + run_port_audit with TDD — IPv4/IPv6, kill+fallback, 6 tests |
| 2026-03-21 IST | James | 53f4551 | feat(105-01): add parse_schtasks_csv + run_schtasks_audit with TDD — Microsoft skip, disable action, 5 tests |
| 2026-03-21 IST | James | f486cd0 | docs(105-01): complete port audit + schtasks audit plan — PORT-01, PORT-02, AUTO-03 closed |
| 2026-03-21 17:11 IST | James | 50d144e | chore(105-03): scaffold rc-process-guard crate + add to workspace — sysinfo 0.33, reqwest 0.12, walkdir 2 |
| 2026-03-21 17:11 IST | James | 01f44eb | test(105-03): TDD RED — failing tests for is_james_self_excluded, is_james_critical, parse helpers |
| 2026-03-21 17:46 IST | James | e83b33e | feat(105-03): rc-process-guard standalone binary — 10 tests pass, 4.0MB static CRT, HTTP POST violations |
| 2026-03-21 17:46 IST | James | 93c6f1c | docs(105-03): complete rc-process-guard plan — DEPLOY-03 closed, STATE/ROADMAP updated |
| 2026-03-21 18:10 IST | James | 205ce52 | docs: complete v15.0 AntiCheat Compatibility research — STACK, FEATURES, ARCHITECTURE, PITFALLS, SUMMARY-v15 |
| 2026-03-21 19:05 IST | James | 8ec4a4e | feat(107-02): create per-game anti-cheat compatibility matrix (17 subsystems x 6 games, SAFE/UNSAFE/SUSPEND/GATE) |
| 2026-03-21 19:06 IST | James | f329291 | feat(107-02): create ConspitLink audit template with ProcMon capture procedure (verdict DEFERRED) |
| 2026-03-21 19:08 IST | James | a1f0517 | docs(107-02): complete plan — 107-02-SUMMARY, STATE, ROADMAP, REQUIREMENTS (AUDIT-02, AUDIT-04 closed) |
| 2026-03-21 19:14 IST | James | 9b432de | docs(107-01): create rc-agent anti-cheat risk inventory (28 behaviors classified, CRITICAL/HIGH/MEDIUM/LOW per system) |
| 2026-03-21 19:15 IST | James | 3e1d12f | docs(107-01): populate pod OS edition table — Windows 11 Pro decision, Phase 108 MUST use GPO registry keys |
| 2026-03-21 19:16 IST | James | 029554a | docs(107-01): complete behavior audit plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated (AUDIT-01, AUDIT-03 closed) |
| 2026-03-21 21:35 IST | James | 2d20fac | feat(108-01): replace SetWindowsHookEx with GPO registry lockdown — NoWinKeys=1 + DisableTaskMgr=1 via reg.exe, hook gated behind keyboard-hook feature flag |
| 2026-03-21 20:36 IST | James | f2b0067 | docs(112-01): go2rtc v1.9.13 RTSP relay installed — 3 cameras, firewall rule, HKLM Run key, API verified at :1984 |
| 2026-03-21 20:52 IST | James | 0921d5c | feat(109-01): create safe_mode.rs module — SafeMode struct, WMI watcher, startup scan, 21 tests passing |
| 2026-03-21 20:53 IST | James | ebe1020 | feat(109-01): integrate safe_mode into AppState and main.rs — 5 new fields, startup detection, WMI spawn, process_guard stub |
| 2026-03-21 20:54 IST | James | a2e2f22 | docs(109-01): complete safe mode state machine foundation — SUMMARY, STATE, ROADMAP, REQUIREMENTS (SAFE-01/02/03 closed) |
| 2026-03-21 20:41 IST | James | d79a9b0 | feat(112-02): create rc-sentry-ai crate scaffold — config, frame buffer, TOML parsing for 3 cameras |
| 2026-03-21 20:41 IST | James | a614213 | feat(112-02): per-camera retina RTSP stream extraction — reconnect loops, H.264 NAL frames, rate limiting |
| 2026-03-21 20:41 IST | James | 79e87de | docs(112-02): complete rc-sentry-ai crate plan — SUMMARY, STATE, ROADMAP updated |
| 2026-03-21 20:50 IST | James | 87d4ced | feat(112-04): people tracker migrated to go2rtc relay — rtsp_url override in config.yaml + main.py |
| 2026-03-21 20:55 IST | James | f342cb2 | docs(112-04): complete people tracker RTSP relay migration plan — SUMMARY, STATE, ROADMAP updated |
| 2026-03-21 20:47 IST | James | b995dae | feat(112-03): add stream health monitoring endpoint at :8096 — relay.rs, health.rs, Axum server wiring |
| 2026-03-21 20:48 IST | James | a74b984 | docs(112-03): complete stream health monitoring plan — SUMMARY, STATE, ROADMAP updated |
| 2026-03-21 20:48 IST | James | 0913e82 | feat(109-02): wire safe mode into event_loop and ws_handler — LaunchGame entry, WMI polling, cooldown timer, Ollama suppression |
| 2026-03-21 20:48 IST | James | 705b07d | feat(109-02): gate process_guard scan loop, kiosk GPO writes, lock_screen Focus Assist during safe mode |

| 2026-03-21 21:50 IST | James | c727b70 | feat(110-02): gate F1 25 UDP socket to GameState::Running, enhance disconnect log with port 20777 |
| 2026-03-21 21:51 IST | James | 2afab9d | docs(110-02): complete UDP socket lifecycle plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated |
| 2026-03-21 22:25 IST | James | 185b4c3 | feat(110-01): add AC EVO telemetry feature flag (HARD-05) — defaults false, gates adapter creation |
| 2026-03-21 22:30 IST | James | 1d2507d | feat(110-01): 5-second deferred SHM connect (HARD-03) — game_running_since + shm_connect_allowed() |
| 2026-03-21 22:32 IST | James | 16a390c | docs(110-01): complete telemetry-gating plan 01 — SUMMARY, STATE, ROADMAP, REQUIREMENTS updated |
| 2026-03-21 22:47 IST | James | 3c0d39a | feat(111-01): build rc-agent from HEAD (243f03d) and deploy to Pod 8 canary — 11MB, ws_connected, uptime>30s |
| 2026-03-22 00:15 IST | James | 518b00c | feat(118-02): add live cameras page with MJPEG feeds from rc-sentry-ai |
| 2026-03-22 00:20 IST | James | 0b1550d | docs(118-02): complete live camera feeds dashboard page plan |
| 2026-03-22 00:37 IST | James | 3cb363b | feat(119-03): NVR playback page with search, video player, event timeline |
| 2026-03-22 00:37 IST | James | 861a717 | docs(119-03): complete NVR playback page plan (phase 119 complete 3/3) |
| 2026-03-21 21:23 IST | James | edc340c | chore(01-02): switch Caddyfile to production Let's Encrypt (remove staging ACME) |
| 2026-03-21 21:29 IST | James | 24bcc7a | docs(01-02): complete VPS deployment plan |
| 2026-03-22 02:30 IST | James | 25e34cb | feat(130-01): add v18.0 MessageType constants to protocol.js (chain_request, chain_step_ack, chain_result, registry_register, registry_ack) |
| 2026-03-22 02:35 IST | James | 6d4a9ca | test(130-01): add failing TDD tests for DynamicCommandRegistry (18 test cases) |
| 2026-03-22 02:45 IST | James | d8eb9c1 | feat(130-01): implement DynamicCommandRegistry — Map storage, binary allowlist, env key isolation, TDD GREEN |
| 2026-03-22 02:50 IST | James | df436fc | docs(130-01): complete protocol-foundation-dynamic-registry plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS (DREG-01, DREG-02, DREG-05) |
| 2026-03-22 03:43 IST | James | 2ee10f2 | feat(130-02): ExecHandler dynamic-first lookup, completedExecs LRU cap, integration tests |
| 2026-03-22 03:43 IST | James | 43f9faf | feat(130-02): James HTTP registry endpoints, Bono WS handler, both-side JSON persistence |
| 2026-03-22 03:43 IST | James | b9dd5df | docs(130-02): SUMMARY + STATE + ROADMAP + REQUIREMENTS updates |
| 2026-03-22 04:10 IST | James | 4102750 | docs(02-02): complete PWA cloud deployment plan (PWA live, API pending racecontrol on VPS) |
| 2026-03-21 22:04 IST | James | 5dacfa5 | fix: prevent false MAINTENANCE_MODE from self-monitor graceful restarts |
| 2026-03-21 22:06 IST | James | 4d940e7 | docs: add cross-process recovery standing rules from pod 6 incident |
| 2026-03-22 03:46 IST | James | 8abcf0d | feat(131-01): ShellRelayHandler class -- binary allowlist, hardcoded APPROVE tier, 14 tests passing |
| 2026-03-22 03:46 IST | James | 513f7c6 | feat(131-01): wire ShellRelayHandler into James/Bono WS routing + HTTP /relay/shell endpoint |
| 2026-03-22 03:46 IST | James | a5e42e0 | docs(131-01): SUMMARY, STATE, ROADMAP, REQUIREMENTS updated |
| 2026-03-21 22:16 IST | James | 734b6d7 | docs: strengthen rule 8 — push/notify/inbox is atomic sequence |
| 2026-03-22 03:46 IST | James | 815674b | feat(06-01): fix admin service build args, env vars, and port in compose.yml |
| 2026-03-22 09:40 IST | James | 56887e3 | test(132-01): add failing tests for ExecResultBroker (RED) |
| 2026-03-22 09:40 IST | James | 87fbe78 | feat(132-01): implement ExecResultBroker shared class (GREEN) |
| 2026-03-22 09:40 IST | James | 05411ce | test(132-01): add failing tests for ChainOrchestrator (RED) |
| 2026-03-22 09:40 IST | James | 696aaaa | feat(132-01): implement ChainOrchestrator class (GREEN) |
| 2026-03-22 09:40 IST | James | 8d5264c | docs(132-01): complete chain-orchestration plan 01 SUMMARY, STATE, ROADMAP, REQUIREMENTS |
| 2026-03-22 10:30 IST | James | 00afb6d | feat(133-01): AuditLogger class with TDD -- append-only JSONL exec audit log |
| 2026-03-22 10:35 IST | James | 680fc74 | feat(133-01): add delegation protocol types + envelope tests |
| 2026-03-22 10:40 IST | James | 464d175 | docs(133-01): complete AuditLogger + delegation protocol foundation plan SUMMARY, STATE, ROADMAP, REQUIREMENTS |
| 2026-03-22 01:55 IST | James | 02f9961 | feat(07-01): add dashboard to Caddy depends_on in compose.yml |
| 2026-03-22 01:57 IST | James | 680226f | docs(07-01): complete dashboard Caddy dependency plan SUMMARY, STATE, ROADMAP |
| 2026-03-22 07:29 IST | James | 0f8e2e5 | docs(07-02): complete dashboard cloud deploy plan — SUMMARY, STATE, ROADMAP, REQUIREMENTS (DASH-01-05) |
| 2026-03-22 02:10 IST | James | 2cceafc | feat(08-01): add GitHub Actions deploy workflow |
| 2026-03-22 12:00 IST | James | c93ea43 | fix(08-01): update deploy workflow for PM2+nginx VPS setup |
| 2026-03-22 12:00 IST | James | 0cf64bf | docs(08-01): complete CI/CD pipeline plan SUMMARY, STATE, ROADMAP, REQUIREMENTS (INFRA-04) |
| 2026-03-22 02:59 IST | James | 68b4c81 | fix: resolve driver upsert UNIQUE constraint (clear stale customer_id) + configure pod healer AI (Ollama → James .27, qwen2.5:3b) |
| 2026-03-22 08:53 IST | James | 6420871 | feat(135-01): add CommsLink daemon watchdog + scheduler registration |
| 2026-03-22 08:53 IST | James | 26c2218 | docs(135-01): complete daemon watchdog plan SUMMARY STATE ROADMAP REQUIREMENTS |
| 2026-03-22 08:58 IST | James | 9aeb6e9 | chore(135-02): register CommsLink-DaemonWatchdog Task Scheduler task, verify HKCU Run key |
| 2026-03-22 09:06 IST | James | f4dba16 | docs(09-02): complete health monitoring deploy plan SUMMARY STATE ROADMAP — Phase 9 complete, INFRA-05 done |
| 2026-03-22 09:26 IST | James | 291c2a9 | feat(137-01): gate close_browser() taskkill behind safe_mode_active, add count_edge_processes() |
| 2026-03-22 09:27 IST | James | 371bf37 | test(137-01): add unit tests for close_browser safe mode gate and count_edge_processes — 3/3 pass |
| 2026-03-22 09:28 IST | James | 76f3bff | docs(137-01): complete browser-watchdog close_browser safe mode gate plan SUMMARY STATE ROADMAP REQUIREMENTS |
| 2026-03-22 09:40 IST | James | 12624c1 | feat(137-02): add is_browser_alive(), is_browser_expected(), pub launch/close_browser on LockScreenManager |
| 2026-03-22 09:41 IST | James | e9c42f1 | feat(137-02): wire browser_watchdog_interval into event loop (BWDOG-01, BWDOG-02, BWDOG-04) |
| 2026-03-22 09:42 IST | James | 6ea1165 | docs(137-02): complete browser watchdog plan SUMMARY STATE ROADMAP REQUIREMENTS |
| 2026-03-22 04:14 IST | James | 883c67c | docs(138): create phase 138 idle health monitor plans (3 plans, 2 waves) |
| 2026-03-22 09:50 IST | James | 4448aa7 | feat(138-01): add AgentMessage::IdleHealthFailed variant to rc-common protocol |
| 2026-03-22 04:23 IST | James | 302953c | feat(138-02): expose check_lock_screen_http and check_window_rect as pub(crate) |
| 2026-03-22 04:23 IST | James | ecc832d | feat(138-02): add idle health monitoring loop to event_loop.rs |
| 2026-03-22 10:15 IST | James | e811db7 | feat(138): add idle_health_fail_count + idle_health_failures to FleetHealthStore and PodFleetStatus |
| 2026-03-22 10:20 IST | James | 825a0a3 | feat(138-03): handle AgentMessage::IdleHealthFailed in server WS handler |
| 2026-03-22 10:13 IST | James | 8d545f5 | docs(142-01): reorganize standing rules into 6 categories with justifications — Deploy, Comms, Code Quality, Process, Debugging, Security |
| 2026-03-22 10:32 IST | James | 4e446a2 | docs(142-01): complete rules hygiene plan SUMMARY STATE ROADMAP REQUIREMENTS |
| 2026-03-22 10:05 IST | James | 0e704e1 | feat(139-01): add ForceRelaunchBrowser to CoreToAgentMessage |
| 2026-03-22 10:10 IST | James | ad3aaf0 | feat(139-01): add relaunch_lock_screen HealAction and Rule 2 WS dispatch |
| 2026-03-22 10:15 IST | James | 85a8d76 | docs(139-01): complete healer edge recovery plan 01 |
| 2026-03-22 10:38 IST | James | 6b9cce5 | feat(139-02): add ForceRelaunchBrowser handler in ws_handler.rs — billing-gated close_browser+launch_browser |
| 2026-03-22 10:39 IST | James | d2bffda | docs(139-02): complete ForceRelaunchBrowser agent handler plan SUMMARY STATE ROADMAP |
| 2026-03-22 10:46 IST | James | d434295 | feat(140-01): add AiSafeAction whitelist enum + parse_ai_action() + prompt injection (8/8 tests pass) |
| 2026-03-22 10:47 IST | James | d28801f | docs(140-01): complete AiSafeAction whitelist plan SUMMARY STATE ROADMAP |
| 2026-03-22 11:10 IST | James | 0a4855b | feat(140-02): execute_ai_action() with safe mode gate in event_loop.rs — 6 tests pass |
| 2026-03-22 11:12 IST | James | e441394 | feat(140-02): server-side AI action logging in pod_healer.rs — parse_ai_action_server + log_pod_activity |
| 2026-03-22 11:35 IST | James | 9f9cf94 | feat(141-01): extend AppState with warn_scanner_last_escalated cooldown field |
| 2026-03-22 11:36 IST | James | 1859c84 | feat(141-01): implement scan_warn_logs() and wire into heal_all_pods() |
| 2026-03-22 12:00 IST | James | 62c7443 | feat(141-02): escalate_warn_surge() — deduplication + AI escalation for WARN surge path |
| 2026-03-22 06:49 IST | James | 2b6e388 | v17.0 deployed: rc-agent to all 8 pods + racecontrol to server .23 + racecontrol.toml (Ollama fix). 0 WARNs post-deploy. |
| 2026-03-22 18:05 IST | James | f500c26 | feat(145-01): register 13 NVR channels (ch1-ch13) + ch1_h264 + CORS in go2rtc.yaml |
| 2026-03-22 18:10 IST | James | 0cc56fa7 | docs(145-01): complete go2rtc infrastructure plan — WebRTC + snapshot coexistence verified live |
| 2026-03-22 18:15 IST | James | 71bd2d1f | feat(146-01): add display_name, display_order, zone to CameraConfig with serde defaults |
| 2026-03-22 18:16 IST | James | 2f0d0572 | feat(146-01): extend /api/v1/cameras to return full camera metadata (8 fields, no nulls) |
| 2026-03-22 18:17 IST | James | e220d147 | docs(146-01): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md (INFRA-03, INFRA-04 complete) |
| 2026-03-22 18:49 IST | James | 8267f3f8 | feat(146-02): add CameraLayout, LayoutState, GET/PUT /api/v1/cameras/layout with atomic file write |
| 2026-03-22 18:50 IST | James | 66cb98c8 | feat(146-02): wire LayoutState into MjpegState in main.rs, load from camera-layout.json at startup |
| 2026-03-22 18:52 IST | James | 0d3db4b6 | docs(146-02): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md (LYOT-04 complete) |
| 2026-03-22 13:18 IST | James | f44335ce | feat(147-01): rewrite cameras.html as professional NVR dashboard (CSS grid 1x1/2x2/3x3/4x4, dynamic API, status dots) |
| 2026-03-22 13:19 IST | James | fa78d9ea | docs(147-01): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md (LYOT-01, UIUX-01/02/04/05, DPLY-01 complete) |
| 2026-03-22 19:15 IST | James | 3db9adfc | docs(147-01): complete plan — checkpoint approved, user verified dashboard at /cameras/live (layout modes, tiles, status indicators) |
| 2026-03-22 19:22 IST | James | a026a190 | feat(147-02): add drag-to-rearrange, zone grouping, collapsible headers, layout persistence to cameras.html |
| 2026-03-22 19:23 IST | James | a22b6595 | docs(147-02): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md (LYOT-02, LYOT-03, LYOT-05 complete) |
| 2026-03-22 13:28 IST | James | 58c624c5 | feat(147-03): implement WebRTC fullscreen with singleton pattern and go2rtc signaling (RTCPeerConnection, pre-warm, auto-hide controls) |
| 2026-03-22 13:29 IST | James | 952febaa | docs(147-03): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md (STRM-01/02/03/04, UIUX-03 complete) |
| 2026-03-22 20:00 IST | James | a7fb74c6 | docs(147-03): checkpoint approved — Task 2 human-verify complete, full dashboard verified at /cameras/live |
| 2026-03-22 14:25 IST | James | a74c35c8 | feat(148-01): rewrite cameras/page.tsx — 849 lines, all 12 features (grid modes, zones, drag-drop, WebRTC fullscreen, pre-warm, snapshot polling, layout persistence) |
| 2026-03-22 14:26 IST | James | 9b8df9de | docs(148-01): SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md (DPLY-02, DPLY-03 complete) |
| 2026-03-22 19:55 IST | James | c192afd8 | docs(148-01): checkpoint approved — Task 2 human-verify complete, :3200/cameras feature parity with cameras.html confirmed, phase 148 complete |
| 2026-03-22 08:59 IST | James | c183cfd4 | fix: deploy standing rules updated — RCAGENT_SELF_RESTART replaces taskkill chain. Pod 5 offline incident root-caused. Service key removed from pods 1/8. OpenSSH installed on all 8 pods. |
| 2026-03-22 16:50 IST | James | 16ec9e6b | feat(149-01): cafe schema (cafe_categories, cafe_items tables + indexes + seeded categories), cafe.rs CRUD module (8 handlers + 5 unit tests all passing), pub mod cafe in lib.rs |
| 2026-03-22 16:51 IST | James | aa78dc67 | feat(149-01): cafe routes registered — admin CRUD in staff_routes (JWT-protected), public menu in public_routes (no auth), release build succeeds |
| 2026-03-22 16:51 IST | James | eeda7207 | docs(149-01): SUMMARY.md + STATE.md + ROADMAP.md (phase 149 plan 01 complete) |
| 2026-03-22 17:15 IST | James | 791380eb | feat(149-02): CafeCategory, CafeItem, CreateCafeItemRequest interfaces + 7 api methods + Cafe Menu sidebar nav entry |
| 2026-03-22 17:20 IST | James | a1edd180 | feat(149-02): /cafe admin page (407 lines) — item table + side panel CRUD + inline category creation + availability toggle |
| 2026-03-22 17:57 IST | James | a10d2470 | feat(150-01): menu import parsing + DB migration — calamine XLSX, csv BOM-strip, validate_import_row, confirm_import_rows (transaction), image_path column, 15 tests all passing |
| 2026-03-22 17:57 IST | James | 08322fa2 | feat(150-01): import/image Axum handlers + routes + ServeDir static mount for /static/cafe-images |
| 2026-03-22 17:57 IST | James | b5d63ea2 | docs(150-01): SUMMARY.md + STATE.md + ROADMAP.md (phase 150 plan 01 complete) |
| 2026-03-22 18:15 IST | James | 0b330f34 | feat(150-02): add image_path to CafeItem, 4 import types (ImportColumnMapping/ImportRowResult/ImportPreview/ConfirmedImportRow), 3 API methods (importCafePreview/confirmCafeImport/uploadCafeItemImage) |
| 2026-03-22 18:18 IST | James | be01510e | feat(150-02): /cafe import modal (2-step: file upload -> preview table with column mapping + red invalid rows) + image column with thumbnail + camera icon upload |
| 2026-03-22 18:20 IST | James | aab31406 | docs(150-02): 150-02-SUMMARY.md + STATE.md progress 99% + ROADMAP.md phase 150 Complete |
| 22/3/2026 18:54:14 IST | James | f40a7372 | docs(150-02): mark plan fully complete after checkpoint approval — STATE/ROADMAP/REQUIREMENTS updated |
| 2026-03-22 20:45 IST | James | adc9204b | feat(151-01): add CafeMenuItem types and publicCafeMenu API method |
| 2026-03-22 20:47 IST | James | a4dec792 | feat(151-01): build CafeMenuPanel and integrate into POS control page |
| 2026-03-22 20:49 IST | James | 2d60fb7c | docs(151-01): SUMMARY.md + STATE.md + ROADMAP.md (phase 151 plan 01 complete) |
| 2026-03-22 21:00 IST | James | 06278ab5 | feat(151-02): CafeMenuItem types, publicApi.cafeMenu(), getImageBaseUrl helper, Cafe tab in BottomNav (replaced Stats) |
| 2026-03-22 21:02 IST | James | 501612e1 | feat(151-02): /cafe page — 2-col item cards grouped by category, filter pills, image fallback, Rs. price formatting |
| 2026-03-22 21:05 IST | James | 9b35e3f3 | docs(151-02): 151-02-SUMMARY.md + STATE.md progress 200/205 + ROADMAP.md phase 151 Complete |
| 2026-03-22 19:15 IST | James | 287591b7 | feat(159-01): add recovery authority contracts (RecoveryAuthority, ProcessOwnership, RecoveryDecision, RecoveryLogger) to rc-common |
| 2026-03-22 19:30 IST | James | 73a8c901 | feat(152-01): add inventory columns migration + CafeItem struct update (is_countable, stock_quantity, low_stock_threshold) — all SELECT/INSERT queries updated |
| 2026-03-22 19:52 IST | James | f980d39f | feat(152-01): add restock endpoint POST /cafe/items/{id}/restock + fix test schema for inventory columns — all 15 cafe tests pass |
| 2026-03-22 19:59 IST | James | 4bc02f36 | feat(159-02): CascadeGuard with 60s window, 3-authority threshold, server-startup exemption, 9 tests — cascade_guard.rs + lib.rs |
| 2026-03-22 19:59 IST | James | 55c3ee97 | feat(159-02): wire CascadeGuard into AppState (cascade_guard field) and pod_healer (is_paused check + record per action) |
| 2026-03-22 19:59 IST | James | 2a0f0c99 | docs(159-02): 159-02-SUMMARY.md + STATE.md + ROADMAP.md phase 159 Complete + CONS-03 done |
| 2026-03-22 19:57 IST | James | 3dba469a | feat(152-02): add inventory fields to CafeItem type and restockCafeItem API method in api.ts |
| 2026-03-22 19:58 IST | James | a48dadc8 | feat(152-02): add inventory UI to cafe admin page — tabs, type/stock columns, restock, threshold badges |
| 2026-03-22 20:45 IST | James | 9daf7dd4 | docs(152-02): 152-02-SUMMARY.md + STATE.md (99%) + ROADMAP.md phase 152 Complete (INV-01,02,04,05,09 done) |
| 2026-03-22 20:13 IST | James | 1b273485 | feat(160-01): add RCAGENT_SELF_RESTART sentinel to tier1_fixes — deploy restarts never miscounted as crashes |
| 2026-03-22 20:14 IST | James | 9a3eb2e9 | feat(160-01): wire RecoveryLogger into rc-sentry crash handler — every recovery decision logged to JSONL |
| 2026-03-22 20:12 IST | James | f8b6333c | feat(153-01): cafe_alerts module — check_low_stock_alerts + reset_alert_cooldown + 8 unit tests + last_stock_alert_at migration |
| 2026-03-22 20:13 IST | James | 8695adff | feat(153-01): wire GET /cafe/items/low-stock route + restock integration calling alert check/reset |
| 2026-03-22 20:14 IST | James | 9b8e2a5a | docs(153-01): 153-01-SUMMARY.md + STATE.md + ROADMAP.md phase 153 1/2 |
| 2026-03-22 20:25 IST | James | 330aaf7b | feat(160-02): add pattern-hit escalation to rc-sentry crash handler |
| 2026-03-22 20:25 IST | James | d07c686d | feat(160-02): pre-restart Ollama query with 8s timeout for unknown patterns |
| 2026-03-22 20:25 IST | James | ea288a92 | docs(160-02): complete pattern escalation + pre-restart Ollama plan |
| 2026-03-22 21:05 IST | James | 0670970e | feat(161-01): add PodRecoveryTracker with graduated offline recovery (4-step: wait/restart/AI/alert) |
| 2026-03-22 21:05 IST | James | 1d26dd6e | docs(161-01): 161-01-SUMMARY.md + STATE.md + ROADMAP.md + REQUIREMENTS.md updated |
| 2026-03-22 21:15 IST | James | 21c8f6f5 | feat(161-02): strip restart/WoL execution from pod_monitor — pure detector only |
| 2026-03-22 21:15 IST | James | a97015a8 | chore(161-02): verify single-authority, update ROADMAP Phase 161 complete |
| 2026-03-22 21:15 IST | James | 94f4732a | docs(161-02): SUMMARY + STATE + ROADMAP for 161-02 plan completion |
| 2026-03-22 21:30 IST | James | 375f6e9d | docs(162): phase 162 plan — james watchdog migration (2 plans) |
| 2026-03-22 21:11 IST | James | ca187c3c | feat(162-01): add failure_state.rs + bono_alert.rs — persistent JSON state, atomic write, degraded Bono alert |
| 2026-03-22 21:11 IST | James | 3d67f8b5 | feat(162-01): implement james_monitor.rs graduated response + main.rs --service branching, 29 tests green |
| 2026-03-22 21:15 IST | James | b7cad99c | feat(162-02): register-james-watchdog.bat + rc-watchdog.exe deployed to staging |
| 2026-03-22 22:05 IST | James | 9795d330 | docs(162-02): SUMMARY + STATE + ROADMAP — phase 162 james watchdog migration complete |
| 2026-03-22 22:30 IST | James | fa858364 | feat(154-01): cafe_orders table + place_cafe_order atomic handler — BEGIN IMMEDIATE, stock race check, compensating rollback |
| 2026-03-22 22:31 IST | James | 494f7ebf | feat(154-01): register cafe/orders routes + public menu stock info (is_countable, stock_quantity, out_of_stock) |
| 2026-03-22 17:05 IST | James | 24ff9223 | feat(155-02): CafeOrderHistoryItem type + getCafeOrderHistory in api.ts |
| 2026-03-22 17:10 IST | James | 00b50b93 | feat(155-02): /cafe/orders PWA page — skeleton, empty state, expand/collapse order list |
| 2026-03-22 17:30 IST | James | 40cd39ce | docs(155-02): SUMMARY + STATE + ROADMAP — plan 155-02 complete, ORD-05/06/09 marked done |
