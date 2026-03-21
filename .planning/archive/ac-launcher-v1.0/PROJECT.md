# AC Launcher — Assetto Corsa Launch System for Racing Point Pods

## Current Milestone: v1.0 Full AC Launch Experience

**Goal:** Give customers a complete, polished Assetto Corsa experience — from session selection to in-game driving — with billing synced to actual gameplay, valid-only options, and both single-player and multiplayer modes working across all 8 pods.

## What This Is

A purpose-built Assetto Corsa launch and session management system for Racing Point eSports' 8 sim racing pods. Extends the existing rc-agent custom experience booking (36 tracks, 325 cars) with smart option filtering, billing synchronization, difficulty tiers, multiplayer orchestration, and enforced safety presets (100% grip, 0% damage). Customers interact via PWA, staff via kiosk.

## Core Value

When a customer selects a session and hits go, the game launches with exactly the settings they chose, billing starts only when they're actually driving, and they never see an option that doesn't work.

## Requirements

### Validated

- ✓ Custom experience booking with AC catalog (36 tracks, 325 cars) — existing in rc-agent
- ✓ Game launch from staff kiosk — existing
- ✓ PIN auth for customer sessions — existing
- ✓ AC server running on Racing-Point-Server (.51) with RP_OPTIMAL preset — existing
- ✓ Difficulty presets in rc-agent — existing foundation

### Active

- [ ] Billing timer syncs with in-game session start (not game launch)
- [ ] DirectX initialization delay handled — billing doesn't start during loading
- [ ] Only valid AC session/mode combinations presented in kiosk and PWA
- [ ] Invalid options dynamically filtered (e.g. no "Race with AI" under Practice if not supported)
- [ ] Single-player race vs AI with configurable grid
- [ ] Multi-pod multiplayer races (customers across pods in same race)
- [ ] AI fills remaining grid spots in multiplayer
- [ ] Racing-themed difficulty tiers (Rookie / Amateur / Semi-Pro / Pro / Alien)
- [ ] Difficulty maps to AI strength, aggression, and behavior settings
- [ ] Fixed safety presets: Tyre Grip 100%, Damage 0% — always enforced
- [ ] Customer picks car, track, session type, difficulty via PWA
- [ ] Staff configures sessions via kiosk
- [ ] Popular preset combos (curated car/track/session packages)
- [ ] In-game assist changes: transmission auto/manual switchable mid-session
- [ ] In-game force feedback adjustable mid-session
- [ ] ABS, traction control, stability control changeable mid-session
- [ ] PWA launch via QR/PIN triggers correct AC session
- [ ] AC server preset management for multiplayer sessions
- [ ] Architecture extensible for other sims (F1, Forza, iRacing) later

### Out of Scope

- F1/Forza/iRacing/LMU launch integration — AC first, others later
- Leaderboard/ranking system — future feature
- Replay recording/sharing — not in v1
- Custom livery selection — complexity, defer
- Voice chat between pods — hardware dependent, defer

## Context

- **Venue:** Racing Point eSports, 8 pods (Conspit Ares 8Nm wheelbases, OpenFFBoard)
- **Existing stack:** rc-agent (Rust/Axum) on each pod, rc-core on server, pod-agent for remote ops
- **AC launch method:** Content Manager CLI integration already exists in rc-agent
- **Known issue:** DirectX initialization can fail/delay, causing "took too long to initialize" errors — billing starts too early
- **AC Server:** Running on .51 (Racing-Point-Server), preset RP_OPTIMAL (100% grip). Multiplayer capability needs investigation.
- **Customer flow:** QR scan on rig → PWA PIN auth → select experience → game launches
- **Staff flow:** Kiosk → configure session → assign to pod → launch
- **CSP gui.ini:** FORCE_START=1 + HIDE_MAIN_MENU=1 already configured

## Constraints

- **Tech stack**: Must integrate with existing rc-agent (Rust/Axum) and rc-core architecture
- **Hardware**: Conspit Ares 8Nm wheelbases with OpenFFBoard — FFB settings must be compatible
- **AC limitations**: Options must respect what Assetto Corsa actually supports — no invalid combos
- **Network**: Multiplayer depends on AC dedicated server on .51 — latency across LAN should be fine
- **Safety**: Grip 100% and Damage 0% are non-negotiable — customer safety and hardware protection
- **Billing accuracy**: Cannot charge for time spent in loading screens or DirectX init

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| AC-first, extensible architecture | Most-used game at venue, but others coming | — Pending |
| Build on existing custom experience booking | 36 tracks, 325 cars already cataloged | — Pending |
| Customer picks via PWA, staff via kiosk | Different UX needs, same backend | — Pending |
| Racing-themed difficulty names | More engaging than generic Easy/Medium/Hard | — Pending |
| Billing syncs to in-game timer, not launch | Eliminates DirectX delay billing issue | — Pending |
| Separate GSD project from racecontrol reliability | Different scope and concerns | — Pending |

---
*Last updated: 2026-03-13 after initialization*
