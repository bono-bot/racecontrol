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

---
