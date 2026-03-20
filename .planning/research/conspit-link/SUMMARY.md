# Project Research Summary

**Project:** Conspit Link -- Full Capability Unlock
**Domain:** Fleet wheelbase management for a commercial sim racing venue (8 pods, Conspit Ares 8Nm, OpenFFBoard firmware)
**Researched:** 2026-03-20
**Confidence:** HIGH

## Executive Summary

This project is NOT a new application build. It is a configuration, integration, and safety-hardening project layered onto the existing rc-agent (Rust) codebase and the closed-source Conspit Link 2.0 wheelbase driver. The core challenge is that Conspit Link cannot be modified, only configured via JSON files and controlled indirectly through process management. The wheelbase itself (running OpenFFBoard firmware) can be commanded directly via HID vendor reports (usage page 0xFF00), giving rc-agent a safety backdoor that operates independently of both Conspit Link and the game's DirectInput stack. Experts build this kind of system by treating the closed-source middleware as an opaque config consumer and layering fleet automation around it.

The recommended approach is phased: solve the safety-critical stuck-rotation bug first (Phase 1), then fix the broken auto-switch mechanism and tune presets (Phase 2), then build fleet config distribution (Phase 3), and finally enable telemetry features like dashboards and LEDs (Phase 4). The stuck-rotation fix is the single highest-priority item because it is a physical safety hazard (8Nm torque can injure wrists/fingers) and blocks all other work -- you cannot safely test game switching or preset tuning while the wheel can snap to arbitrary positions. The key discovery from architecture research is that Conspit Link reads `Global.json` from `C:\RacingPoint\` at runtime, NOT from its install directory, which is almost certainly why auto-switch is broken on pods.

The primary risk is the HID write contention between rc-agent and Conspit Link (documented as P-20). Both write to the same USB device; last writer wins. The mitigation is to stop fighting Conspit Link for HID access during routine operations. For session-end safety: close Conspit Link first, then send HID commands, then restart it. For preset changes: write config files, restart Conspit Link, let it apply via its own HID path. Reserve raw HID estop for genuine emergencies only. Secondary risks include config corruption from force-killing Conspit Link (always use WM_CLOSE), fleet config drift (atomic writes + checksum verification), and torque snap-back injury (ramp forces gradually, never instant-apply).

## Key Findings

### Recommended Stack

This project requires no new applications. All work is configuration changes to Conspit Link's JSON files, new `.Base` FFB preset files, and Rust code changes in rc-agent. The stack is the set of protocols rc-agent must understand: OpenFFBoard HID vendor reports (26-byte format, report ID 0xA1), Conspit Link JSON config files (`.Base` presets, `Global.json`, `GameToBaseConfig.json`, `Settings.json`, `GameSettingCenter.json`), and game telemetry protocols (UDP for F1 25, shared memory for AC/ACC/AC EVO).

**Core technologies:**
- **OpenFFBoard HID protocol (0xFF00):** Direct wheelbase safety commands (estop, idlespring, power/gain, position readback) -- the only path that works independently of Conspit Link and game state
- **Conspit Link JSON config files:** `.Base` presets (plain JSON, fully editable), `Global.json` (auto-switch toggle, UDP ports), `GameToBaseConfig.json` (game-to-preset mapping) -- the only way to configure Conspit Link behavior
- **hidapi 2.x (already in rc-agent):** HID device communication for safety commands -- already implemented for estop and gain control
- **serde_json (already in rc-agent):** JSON parsing for all Conspit Link config files -- no new dependencies needed
- **notify 7.x (new, optional):** Filesystem watcher for detecting config changes -- useful for drift detection but not required for Phase 1

**Critical version note:** Conspit Link 2.0 v1.1.2 is installed. Do not update without testing -- firmware/software version mismatches can break FFB behavior.

### Expected Features

**Must have (table stakes):**
- **Wheelbase safe centering on session end** -- Safety-critical. The stuck-rotation bug (P-20) must be fixed before anything else. Sequence: close Conspit Link, clear effects via HID, apply gentle centering spring, restart Conspit Link.
- **Per-game FFB presets for 4 active titles** -- AC, F1 25, ACC/AC EVO, AC Rally. Yifei Ye pro presets as starting base, tuned for 8Nm hardware. All presets in `.Base` JSON format (not encrypted `.conspit`).
- **Auto game-profile switching working** -- Fix `AresAutoChangeConfig` by placing `Global.json` at `C:\RacingPoint\` (the actual runtime read path). Fix `GameToBaseConfig.json` mappings to point to Racing Point presets.
- **Fleet-wide config push via rc-agent** -- Push validated configs to all 8 pods. Atomic file writes, checksum verification, graceful Conspit Link restart.
- **Conspit Link health monitoring hardened** -- Crash detection, graceful restart (never taskkill /F), post-restart config verification.

**Should have (competitive):**
- **Telemetry dashboards on wheel LCD** -- ConspitLink has built-in support; just needs correct per-game config
- **Shift light LED + RGB button configuration** -- Auto RPM configs already exist for AC/ACC/iRacing; F1 25 may need manual thresholds
- **rc-agent fleet monitoring dashboard** -- Config hash comparison, active preset detection, firmware version tracking
- **Custom Racing Point preset library** -- Casual/Competitive/Drift variants per game

**Defer (v2+):**
- One-click FFB for non-native games (only when adding Forza, EA WRC, DiRT)
- UDP telemetry forwarding to external consumers (spectator displays, analytics)
- Per-car FFB sub-presets (only if customers request)
- Firmware fleet management (manual updates only, monitor versions)
- SimHub integration (explicitly out of scope per PROJECT.md)
- Per-customer saved profiles (anti-feature: config proliferation, support burden)

### Architecture Approach

Three-tier architecture: racecontrol (server, config authority) pushes configs via WebSocket to rc-agent (per-pod automation agent), which writes JSON config files to disk and manages the Conspit Link process. Conspit Link reads those configs and controls the Ares wheelbase via HID. rc-agent also has a direct HID path to the wheelbase for safety commands that bypass Conspit Link entirely. The critical boundary is the HID write contention between rc-agent and Conspit Link on each pod -- this is a per-pod problem, not a fleet scaling concern.

**Major components:**
1. **racecontrol (server)** -- Fleet config authority, session lifecycle orchestration, config push via WebSocket, pod health aggregation
2. **rc-agent (per-pod)** -- FFB safety commands via HID, config file distribution, Conspit Link process watchdog, game detection, driving state monitoring
3. **Conspit Link 2.0 (per-pod, closed-source)** -- Wheelbase configuration GUI, game telemetry reader, dashboard/LED driver, auto game-profile switching
4. **OpenFFBoard firmware (per-wheelbase)** -- Motor control, DirectInput FFB processing, vendor HID command interface

**Key architectural discovery:** Conspit Link reads `Global.json` from `C:\RacingPoint\Global.json` at runtime, not from its install directory. This path does not exist on pods. This is almost certainly why `AresAutoChangeConfig` is broken -- the fix is trivial (copy/symlink the file) but the discovery was critical.

### Critical Pitfalls

1. **Orphaned DirectInput FFB effects on game death** -- Game crash leaves last force vector active in firmware indefinitely. Fix: after game process death, send `fxm.reset` (clear effects) then `axis.idlespring` (centering spring). Do NOT use ESTOP for routine session end (it kills centering spring too).
2. **Conspit Link overwrites HID commands (P-20)** -- Conspit Link's polling loop reasserts its FFB state, negating rc-agent's safety commands within milliseconds. Fix: close Conspit Link before sending safety HID commands, then restart it. Do not fight for HID access during normal operation.
3. **Torque snap-back injury risk** -- Applying centering spring or preset at full force can injure customers. Fix: ramp forces gradually (10% to target over 500ms-1s), cap venue power at 70% of max, implement hands-off interlock before enabling FFB.
4. **Force-killing Conspit Link corrupts JSON state** -- `taskkill /F` bypasses config flush, leaving half-written files. Fix: always use WM_CLOSE with 10s timeout, keep backup configs, validate JSON integrity after restart.
5. **Auto-detection race condition** -- Game initializes DirectInput FFB before Conspit Link loads the matching preset. Fix: rc-agent loads preset BEFORE launching game, not after. Do not rely solely on Conspit Link's built-in auto-switch.

## Implications for Roadmap

Based on research, suggested phase structure:

### Phase 1: Safety and Session Lifecycle
**Rationale:** The stuck-rotation bug is a physical safety hazard and blocks all other work. Cannot test preset tuning or game switching while the wheel can snap to arbitrary positions. P-20 HID contention must be solved as part of this.
**Delivers:** Safe session end sequence, hardened Conspit Link watchdog, graceful process management
**Addresses:** Safe centering on session end (P1), Conspit Link health monitoring (P1), kiosk window management (P1)
**Avoids:** Pitfalls 1 (orphaned effects), 2 (P-20 contention), 3 (snap-back injury), 4 (force-kill corruption)
**Key work:**
- Implement close-CL-then-command-then-restart sequence for session end
- Replace ESTOP-based session end with `fxm.reset` + `axis.idlespring` gradual ramp
- Harden `ensure_conspit_link_running()` with graceful shutdown, config backup, JSON validation
- Cap venue power at safe maximum via `axis.power`

### Phase 2: Game Profile Configuration
**Rationale:** Depends on Phase 1 (safe to test game switching only when session-end is safe). Fixes the broken auto-switch that prevents hands-free operation. Preset tuning requires a working auto-switch to validate.
**Delivers:** Working auto game-profile switching, venue-tuned FFB presets for all 4 titles, correct steering angles and force limits per game
**Addresses:** Auto game-profile switching (P1), per-game FFB presets (P1)
**Uses:** `.Base` JSON preset format, `Global.json` at `C:\RacingPoint\`, `GameToBaseConfig.json` mappings
**Avoids:** Pitfall 5 (auto-detection race -- load preset before game launch), Pitfall 10 (encrypted files -- use `.Base` format only)
**Key work:**
- Copy/symlink `Global.json` to `C:\RacingPoint\` on each pod (the critical fix)
- Update `GameToBaseConfig.json` to point to Racing Point presets
- Create venue-tuned `.Base` presets for AC, F1 25, ACC/AC EVO, AC Rally (starting from Yifei Ye pro presets)
- Implement rc-agent pre-launch preset loading (preset first, then game)

### Phase 3: Fleet Config Distribution
**Rationale:** Depends on Phase 2 (must have correct configs to push). Once presets and auto-switch work on one pod, replicate across all 8 identically.
**Delivers:** Fleet-wide config push via rc-agent, config drift detection, atomic write protocol, golden config in version control
**Addresses:** Fleet-wide config consistency (P1), rc-agent config state monitoring (P2)
**Implements:** `conspit_config.rs` module in rc-agent, racecontrol `PushConfig` WebSocket command
**Avoids:** Pitfall 7 (config drift -- checksum verification), Pitfall 11 (concurrent writes -- atomic write pattern)
**Key work:**
- New `conspit_config.rs` module: receive config push, write atomically (temp+rename), verify checksums
- Write `Global.json` to BOTH install dir and `C:\RacingPoint\`
- Stop Conspit Link before push, write files, restart, verify
- Config hash in heartbeat for drift detection
- Golden config directory in repo under `.planning/presets/`

### Phase 4: Telemetry and Display Features
**Rationale:** Depends on Phase 2 (Conspit Link must be properly configured). These are customer-facing polish features that enhance the experience but are not safety-critical.
**Delivers:** Wheel LCD dashboards per game, shift light LEDs, RGB button telemetry lighting, UDP port chain documentation
**Addresses:** Telemetry dashboards (P2), shift light LEDs (P2), RGB button lighting (P2)
**Avoids:** Pitfall 6 (UDP port contention -- clear ownership chain, single listener)
**Key work:**
- Verify `GameSettingCenter.json` has all telemetry fields enabled for venue games
- Configure Auto RPM for shift lights (existing `.conspit` files for AC/ACC/iRacing)
- Set up RGB button assignments per game (DRS, ABS, TC, flags)
- Document UDP port chain: game (20777) -> Conspit Link (20778) -> future consumers

### Phase Ordering Rationale

- **Phase 1 before everything:** Safety is non-negotiable. The stuck-rotation bug is the single worst failure mode and solving it requires understanding the HID contention model that informs all subsequent phases.
- **Phase 2 before Phase 3:** You need working configs on one pod before pushing to eight. Tuning presets on a broken auto-switch wastes time. The `C:\RacingPoint\Global.json` fix is trivial but game-changing.
- **Phase 3 before Phase 4:** Fleet consistency matters more than polish features. A pod with wrong FFB is worse than a pod without shift lights.
- **Phase 4 last:** Telemetry features are customer delight, not operational necessity. They can be added incrementally without affecting safety or core FFB.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1:** Needs research into exact `fxm.reset` and `axis.idlespring` HID command parameters (class IDs verified, but command sequences and timing need empirical testing on hardware)
- **Phase 2:** Needs research into Conspit Link's actual auto-switch polling interval and game detection behavior (log analysis on a live pod)

Phases with standard patterns (skip research-phase):
- **Phase 3:** Fleet config push is a well-understood file sync problem. Atomic writes on NTFS, SHA256 checksums, WebSocket commands -- all standard patterns already used in rc-agent.
- **Phase 4:** Telemetry configuration is purely JSON config editing in known file formats. No code changes, just correct values in `GameSettingCenter.json`.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Verified against local filesystem, OpenFFBoard wiki, and existing rc-agent code. No new applications needed. |
| Features | HIGH | Based on actual hardware docs, venue PROJECT.md, competitor analysis. Clear P1/P2/P3 prioritization. |
| Architecture | HIGH | Three-tier model verified from existing code. Critical discovery: `C:\RacingPoint\Global.json` runtime path from log evidence. |
| Pitfalls | MEDIUM-HIGH | P-20 contention and stuck-rotation well-documented. Conspit Link internal timing (polling interval, config cache behavior) inferred but not directly measured. |

**Overall confidence:** HIGH

### Gaps to Address

- **Conspit Link polling interval:** How often does Conspit Link reassert its FFB state to the wheelbase? This determines the timing window for Phase 1's session-end sequence. Measure empirically by watching HID traffic during a session end.
- **`fxm.reset` command availability:** The OpenFFBoard wiki documents this command but the Conspit fork may not implement it identically. Test on actual hardware before relying on it in the session-end sequence.
- **`C:\RacingPoint\` path on pods:** Log evidence is from one machine. Verify this path is used on all 8 pods, not just the server or a specific install variant.
- **Conspit Link config cache behavior:** Does Conspit Link re-read config files at any point after startup, or only on restart? If it caches everything at startup, the restart-after-config-write pattern is mandatory (not optional).
- **F1 25 and AC Rally preset tuning:** No pro driver presets exist for these titles. Will require hands-on tuning sessions with staff feedback. Budget time for iterative tuning.

## Sources

### Primary (HIGH confidence)
- [OpenFFBoard Wiki: Commands](https://github.com/Ultrawipf/OpenFFBoard/wiki/Commands) -- HID report format, class IDs, command IDs
- [OpenFFBoard Wiki: Configurator Guide](https://github.com/Ultrawipf/OpenFFBoard/wiki/Configurator-guide) -- Power scaling, endstop protection
- [EA Forums: F1 25 UDP Specification](https://forums.ea.com/discussions/f1-25-general-discussion-en/discussion-f1%C2%AE-25-udp-specification/12187351) -- Telemetry packet format
- Local filesystem: `C:\Program Files (x86)\Conspit Link 2.0\` -- All config files verified by direct inspection
- Existing rc-agent source: `ffb_controller.rs`, `driving_detector.rs`, `failure_monitor.rs`, `ac_launcher.rs`
- ConspitLink 2.0 log: `C:\Program Files (x86)\Conspit Link 2.0\log\2026-03-17_log.ConspitLog` -- Runtime Global.json path discovery
- PROJECT.md -- P-20 bug, stuck-rotation, fleet constraints

### Secondary (MEDIUM confidence)
- [Conspit Link 2.0 PW1 Tutorial](https://oss.conspit.com/file/4/b/1c/PW1%E9%A9%B1%E5%8A%A8%E6%95%99%E7%A8%8BEN%20V1.1.pdf) -- ConspitLink function guide
- [Conspit H.AO Function Guide](https://conspit.cn/uploads/20250314/CONSPIT_H.AO_FunctionGuideforConspitLink2.0_EN%20v1.1.pdf) -- LED, telemetry, one-click config
- [SimXPro Direct Drive Safety Guide](https://simxpro.com/blogs/guides/direct-drive-safety-torque-limits-wrist-injuries-and-the-emergency-stop) -- Torque injury risks
- [DCS Force Feedback Fix](https://github.com/walmis/dcs-force-feedback-fix) -- DirectInput orphaned effect patterns
- [SRL VMS V5.0](https://www.simracing.co.uk/features.html), [SimLuxx](https://simluxx.com/pages/venue-creation-and-management), [Multitap](https://multitap.space/) -- Competitor feature sets

### Tertiary (LOW confidence)
- [OverTake.gg DD Injury Thread](https://www.overtake.gg/threads/risk-of-injury-with-dd-motors.187609/) -- Community injury reports (anecdotal but consistent)
- [acc_shared_memory_rs crate](https://crates.io/crates/acc_shared_memory_rs) -- ACC shared memory for Rust (not yet tested)

---
*Research completed: 2026-03-20*
*Ready for roadmap: yes*
