# Research Summary: AC Launcher -- Assetto Corsa Launch & Session Management

**Domain:** Sim racing venue pod management -- Assetto Corsa launch/session control
**Researched:** 2026-03-13
**Overall confidence:** HIGH

## Executive Summary

This project extends an existing, production-running system (racecontrol) with comprehensive Assetto Corsa session management. The critical finding is that approximately 70% of the required infrastructure already exists in the codebase. The gaps are specific and well-defined: single-player race mode with AI opponents, billing synchronization to actual on-track state (not process launch), valid option filtering, and racing-themed difficulty tiers.

The AC ecosystem for programmatic control is mature but quirky. Assetto Corsa exposes game state through three shared memory files (`acpmf_physics`, `acpmf_graphics`, `acpmf_static`) that are already fully integrated in the rc-agent SimAdapter. The game is configured via INI files (`race.ini`, `assists.ini`, `controls.ini`) that rc-agent already writes. Content Manager provides a URI protocol (`acmanager://`) for multiplayer server joins, which is already integrated. The AC dedicated server (`acServer.exe`) is managed by rc-core with dynamic port allocation, session lifecycle, and config generation -- all already working.

The most impactful technical decision is using the shared memory `AC_STATUS` field (value 2 = LIVE = on track) as the billing trigger instead of the current approach of resetting billing when the acs.exe process is detected. This single change addresses the known DirectX initialization billing gap (5-30 seconds of overbilling per session) and is technically straightforward since the shared memory reader is already connected.

The second key decision is extending `write_race_ini()` from its current single-car practice-only format to support multiple AI car blocks (`[CAR_1]` through `[CAR_N]`) with configurable AI_LEVEL, enabling the core "Race vs AI" feature that customers are asking for.

## Key Findings

**Stack:** No new dependencies needed. Everything builds on existing Rust/Axum rc-agent, rc-core, Windows API (winapi), stock AC + CSP + Content Manager, and stock acServer.exe. The approach is direct `acs.exe` launch for single-player (deterministic, no CM dependency) and `acmanager://` URI for multiplayer (handles server join handshake).

**Architecture:** Existing three-tier architecture (agent/core/dashboard) is sound. New features fit cleanly into existing component boundaries. Session configurator belongs in rc-core (validation, DB access). Launch execution belongs in rc-agent (INI writing, process management). No architectural changes needed -- only feature additions.

**Critical pitfall:** Billing starts during loading screen (5-30 seconds overbilling per session). Fix: use AC shared memory `AC_STATUS == 2 (LIVE)` as billing trigger. Secondary critical pitfall: FFB safety gap between game kill and wheelbase zero -- must send estop BEFORE killing game process.

## Implications for Roadmap

Based on research, suggested phase structure:

1. **Race Mode + Difficulty Tiers** - Enable single-player race with AI opponents
   - Addresses: Core missing feature (only practice exists today), difficulty customization
   - Avoids: Building multiplayer complexity before solo works
   - Dependencies: race.ini extension, DifficultyTier type, AI car block generation

2. **Billing Sync + Safety Enforcement** - Fix billing accuracy and safety gaps
   - Addresses: DirectX loading billing gap, FFB safety on session end, assists.ini desync
   - Avoids: Revenue inaccuracy accumulating over time
   - Dependencies: AC_STATUS shared memory polling, GameplayStarted signal, FFB zero ordering

3. **Valid Option Filtering** - Prevent broken car/track/session combinations
   - Addresses: Tracks without AI lines, pit count limits, missing content per pod
   - Avoids: Customer-facing failures from invalid combinations
   - Dependencies: Filesystem scanning, catalog extension, API filtering

4. **Presets + UX Polish** - Popular combos, mid-session controls, window management
   - Addresses: Quick-launch presets, transmission/FFB toggle, focus enforcement
   - Avoids: Over-engineering before core features are proven
   - Dependencies: DB presets, overlay/PWA integration, event-driven foreground

5. **Multiplayer Enhancement** - AI fillers, session timing, multi-pod billing sync
   - Addresses: AI fills remaining grid spots, billing alignment across pods
   - Avoids: Multiplayer complexity before single-player race is solid
   - Dependencies: Server entry_list AI slots, billing coordination in rc-core

**Phase ordering rationale:**
- Phase 1 first because it's the biggest functional gap (no race mode) and doesn't touch billing
- Phase 2 next because billing accuracy directly affects revenue
- Phase 3 before UX because it prevents failures that degrade the experience enabled by Phase 1
- Phase 4 is polish that makes existing features better
- Phase 5 last because multiplayer already works for pod-to-pod races; AI fillers are a nice-to-have

**Research flags for phases:**
- Phase 1: AI_LEVEL values (70-100) need tuning on actual pods; AI_AGGRESSION support uncertain across CSP versions
- Phase 2: AC_STATUS polling frequency and stale data detection need testing (packetId heartbeat)
- Phase 3: Track pit count extraction method needs investigation (parse track data files vs count pit positions)
- Phase 5: AI on AC dedicated server may need CSP server-side plugins -- needs deeper research when Phase 5 is started

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Existing codebase verified, no new dependencies, proven AC integration patterns |
| Features | HIGH | Feature gaps are clear and specific; existing code covers 70% of needs |
| Architecture | HIGH | Existing architecture is sound; changes are additions, not restructuring |
| Pitfalls | HIGH | Most pitfalls derive from existing codebase analysis and documented issues |
| AI difficulty tuning | MEDIUM | AI_LEVEL values are community consensus; AI_AGGRESSION needs testing |
| Track content validation | MEDIUM | Filesystem scanning approach is sound but pit count extraction needs verification |
| Multiplayer AI fillers | LOW | Server-side AI in multiplayer may require CSP plugins; needs Phase 5 research |

## Gaps to Address

- **AI_LEVEL to difficulty mapping:** The Rookie=70, Amateur=80, Semi-Pro=90, Pro=95, Alien=100 mapping is based on community patterns. Needs real-world testing on the Conspit Ares hardware with venue customers to calibrate.
- **AI_AGGRESSION support:** Per-car aggression in race.ini may not work in all AC/CSP versions. Need to test on Pod 8 before committing to 5-tier aggression model.
- **Track pit count extraction:** Need to determine how to read pit stall count from AC track data files. May be in `data/surfaces.ini` or require counting pit position markers in the 3D data.
- **CSP version verification:** Need a reliable way to detect CSP version on each pod at startup. The CSP version file location varies by installation method.
- **Multiplayer AI fillers:** Adding AI opponents to a dedicated server session is a separate concern from single-player AI. AC dedicated server does not natively support AI drivers -- this may require CSP server plugins (stracker, AC Server Plugins, or CSP Lua scripts). Needs dedicated research in Phase 5.
- **Mid-session assist effectiveness:** CSP may or may not respect assists.ini changes made while AC is running. Needs testing with specific CSP version on pods.
