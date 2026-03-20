# Milestones

## v1.0 RaceControl HUD & Safety (Shipped: 2026-03-13)

**Phases completed:** 5 phases, 15 plans, 16 tasks

**Key accomplishments:**
- Escalating watchdog backoff (30s→2m→10m→30m) with post-restart verification and email alerts — pods self-heal without manual intervention
- WebSocket keepalive (15s WS ping + 30s app-level Ping/Pong) + fast-then-backoff reconnect — no more "Disconnected" flash during game launch
- DeployState FSM with HEAD-before-kill validation, canary-first (Pod 8), and session-aware rolling deploy — deployments work reliably across all 8 pods
- Blanking screen protocol: lock-screen-before-kill ordering, LaunchSplash branded screen, extended dialog suppression — customers never see system internals
- PIN auth unification (validate_pin_inner + PinSource enum) + pod lockdown (taskbar hidden, Win key blocked) — consistent, locked-down customer experience
- Config validation with branded error screen + deploy template fix — rc-agent fails fast on bad config instead of silently running with zero billing rates

## v2.0 Kiosk URL Reliability (Shipped: 2026-03-14)

**Phases completed:** 6 phases, 12 plans

**Key accomplishments:**
- Server IP pinned to .23 via DHCP reservation + racecontrol reverse proxy for kiosk
- Pod lock screens show branded "Connecting..." state — never browser error pages
- Edge auto-update, StartupBoost, BackgroundMode disabled on all 8 pods
- Staff dashboard: one-click lockdown toggle, power management (restart/shutdown/wake) per-pod and bulk
- Customer experience: Racing Point branding on lock/blank screens, session results display, staff-configurable wallpaper

## v3.0 Leaderboards & Competitive (Shipped: 2026-03-15)

**Phases completed:** 3 phases (12, 13, 13.1), 10 plans

**Key accomplishments:**
- SQLite lap times DB with full migration stack — persistent leaderboards across server restarts
- Live leaderboard with top-10 per track/car, personal bests, percentile ranks, gap-to-leader — served via REST API
- Pod fleet reliability: ConspitLink USB watchdog, process cleanup on session end, telemetry UDP failover

## v4.0 Pod Fleet Self-Healing (Shipped: 2026-03-16)

**Phases completed:** 7 phases (16–22), 15 plans

**Key accomplishments:**
- Firewall auto-config: racecontrol opens its own Windows Firewall rule at startup — no manual setup on new server
- WebSocket exec relay: staff can run shell commands on any pod from kiosk (WS proxy through rc-agent)
- Startup self-healing: rc-agent detects and clears stale CLOSE_WAIT sockets, restarts on crash detection
- Watchdog service: server monitors all pods, alerts on missed heartbeats, auto-requeues stuck recovery tasks
- Deploy resilience: deploy_pod.py with canary-first (Pod 8), binary size verification, pod-agent /write endpoint
- Fleet health dashboard: live pod grid with status badges, per-pod metrics, bulk restart/lockdown actions
- Pod 6/7/8 recovery: WinRM-free remote deploy via pod-agent, PowerShell EncodedCommand for firewall/binary fixes

## v4.5 AC Launch Reliability (Shipped: 2026-03-16)

**Phases completed:** 5 phases, 10 plans, 19 requirements

**Key accomplishments:**
- Billing-game lifecycle wired: game killed on billing end, launch gate before billing, anti-double-launch guard, pod reset after session
- Game crash recovery: auto-pause billing on crash, "Game Crashed" badge + "Relaunch" on kiosk dashboard
- Launch resilience: structured `LaunchDiagnostics` (CM exit code, log errors, fallback flag), billing auto-pause on total launch failure
- Multiplayer server lifecycle: `AcServerManager` wired to billing — server auto-starts on booking, auto-stops when all billing ends; kiosk self-serve "Play with Friends" booking with per-pod PINs
- Synchronized group play: coordinated launch (all pods receive `LaunchGame` simultaneously when all PINs validated), continuous race mode with billing-active guard, "Join Failed" + retry on kiosk dashboard, mid-session track/car change for continuous mode

## v5.0 RC Bot Expansion (Shipped: 2026-03-16)

**Phases completed:** 5 phases (23–27), 19 plans, 19 requirements

**Key accomplishments:**
- `PodFailureReason` enum (9 classes) + 5 `AgentMessage` bot variants + `is_pod_in_recovery()` guard — deterministic failure taxonomy shared across all bot tasks (Phase 23)
- Game freeze, launch timeout, USB wheelbase disconnect auto-fix bots with FFB zero-force safety ordering before every process kill (Phase 24)
- `billing_guard.rs` detects stuck sessions (60s) and idle drift (5min); `bot_coordinator.rs` routes to safe `end_session()` with CRDT cloud sync fence (Phase 25)
- Lap validity filter + per-track floor (Monza/Silverstone/Spa) + `review_required` flag; customer/staff PIN counters structurally separated; TELEM-01 email alert + MULTI-01 ordered teardown (Phase 26)
- Tailscale mesh installed on all pods + `bono_relay.rs` bidirectional event/command relay to Bono's VPS on port 8099 (Phase 27)

---

## Comms Link v1.0 — James-Bono Communication (Shipped: 2026-03-12)

**Phases completed:** 8 phases, 14 plans (100%)
**Repo origin:** comms-link (merged into unified .planning/)
**Archive:** archive/comms-link-v1.0/

**Key accomplishments:**
- Persistent WebSocket connection from James → Bono's VPS with PSK auth (Bearer header, not query params)
- Auto-reconnect with queue flush on re-establish — only reconnects on network drops, not auth rejection
- Heartbeat sender with CPU/memory/Claude detection metrics every N seconds
- ClaudeWatchdog: detects Claude Code crash, 2s kill→spawn delay, 3s post-spawn verification, cooldown DI
- Watchdog hardening: cooldown reset policy, ESM isMainModule detection, wireRunner() DI pattern
- Alert system: fixed-window cooldown suppression, email fallback to both usingh and bono, WhatsApp via Evolution API
- LOGBOOK.md bidirectional sync with atomic writes, conflict detection, and ack-based content tracking
- Daily coordination: HealthAccumulator snapshots, IST-windowed daily summary scheduler, pod status HTTP fetch, FAILSAFE retirement

## Cloud Services v1.0 — Bots & API Gateway (Shipped: 2026-03-07)

**Commits:** 9 across 4 repos | **Owner:** Bono (VPS)
**Repos:** racingpoint-api-gateway, racingpoint-whatsapp-bot, racingpoint-discord-bot, racingpoint-google

**Key accomplishments:**
- API Gateway (Express.js): bookings, customers, calendar, proxy routes merging bot data with racecontrol REST API
- WhatsApp bot: automated session booking with Google Calendar, Claude API migration, 2-min direct mode fallback, RaceControl session booking for registered customers
- Discord bot: modal form booking with Google Calendar integration
- Google shared services: Calendar attendees support, Gmail replyEmail, OAuth token refresh utility
- James-Bono instant comms link + brainstorm daemon v2 + comms cron scripts (predecessor to comms-link repo)

## Admin Dashboard v1.0 — Staff Operations Panel (Shipped: 2026-03-08)

**Commits:** 14 | **Owner:** Bono (VPS) + James (remote pushes)
**Repo:** racingpoint-admin (Next.js/TypeScript)

**Key accomplishments:**
- Full admin dashboard: cafe, inventory, sales, purchases, finance modules
- Receipt scanner + bank statement matching + analytics
- Waivers page with signature viewing (Google Sheets integration)
- HR section + hiring nav + marketing admin pages
- Racing Point brand identity rebrand (Racing Red #E10600)
- Docker support + admin chat wired directly to rc-core
- Kiosk control page: per-pod screen blanking toggles, health check
- Sessions page with staff attribution + daily summary
- Wallet log page with Transaction ID column
- Server-side proxy /api/rc/[...path] for rc-core API calls

## Ops Toolkit v1.0 — Fleet Management CLI (In Progress, started 2026-03-17)

**Phases:** 5 planned, 1 complete (Phase 1: Foundation & Safety)
**Repo origin:** deploy-staging (ops.bat + ops.conf.bat)
**Requirements:** 27 (FNDN-01..06, PDOP-01..11, SVOP-01..06, BUILD-01..04)

**Key accomplishments (Phase 1):**
- ops.bat interactive menu + CLI mode dual entry point
- ops.conf.bat centralizes all pod IPs, server IP, paths, ports — zero hardcoded values
- Safety blocklist prevents running pod binaries on James's machine
- CRLF enforcement, delayed expansion, hostname-based safety guard
- Stub routing for all Phase 2-5 commands (incremental build-out)

**Remaining:**
- Phase 2: Health & Status — read-only fleet visibility
- Phase 3: Pod Deploy & Operations — deploy, restart, canary, screenshot
- Phase 4: Server Operations — racecontrol, kiosk, pod-agent service management
- Phase 5: Build & Polish — cargo build automation, colored output

## AC Launcher v1.0 — Full AC Launch Experience (Shipped: 2026-03-14)

**Phases completed:** 9 phases, 20 plans (100%)
**Repo origin:** ac-launcher (code in racecontrol, merged into unified .planning/)
**Archive:** archive/ac-launcher-v1.0/

**Key accomplishments:**
- Session types (Practice, Race, Hotlap) with mode validation — only valid AC combinations presented
- 5 difficulty tiers (Rookie→Alien) mapping to AI strength, aggression, and behavior
- Billing synchronized to in-game session start (not game launch), DirectX init delay handled
- Safety enforcement: 100% grip, 0% damage always applied — non-negotiable
- Content validation: invalid track/car/session combos filtered before customer sees them
- Mid-session controls: transmission, FFB, ABS, TC, stability all adjustable during gameplay
- Curated presets: popular car/track/session packages for quick selection
- Staff/PWA integration: QR/PIN triggers correct AC session, staff configures via kiosk
- Multiplayer enhancement: AC server preset management, lobby enrichment with track/car/AI info
