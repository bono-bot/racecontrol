# Feature Research

**Domain:** Sim racing venue wheelbase management (Conspit Ares 8Nm fleet, 8 pods)
**Researched:** 2026-03-20
**Confidence:** HIGH (based on actual hardware docs, venue PROJECT.md, and competitor landscape)

## Feature Landscape

### Table Stakes (Users Expect These)

Features that are non-negotiable for a commercial sim racing venue. Missing these means staff intervention every session, safety risk, or customer complaints.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Wheelbase safe centering on session end** | Wheel snapping to random position is a safety hazard and the single worst UX issue. Every venue competitor solves this. | HIGH | Root cause is unreleased DirectInput FFB effects + ConspitLink holding last force vector. Must apply spring centering force BEFORE game process dies, then zero torque. rc-agent's existing ESTOP (HID 0x0A) fires too late or gets overwritten (P-20). Needs a sequenced shutdown: apply centering spring via ConspitLink preset or HID command, wait for wheel to center, then zero torque. |
| **Per-game FFB presets tuned for venue hardware** | Customers expect the wheel to feel right for each game without fiddling. Default presets are mediocre on 8Nm bases. | MEDIUM | Yifei Ye pro presets exist for AC/ACC/AC EVO as starting points. F1 25 and AC Rally need custom tuning. Each .Base preset has ~15 parameters (angle, max force, damper, spring, friction, inertia, filtering). Tune once, deploy to fleet. |
| **Auto game-profile switching** | Staff should not manually switch profiles between sessions. Conspit Link has `AresAutoChangeConfig: "open"` in Global.json but it is broken. | MEDIUM | ConspitLink claims to detect game launch and load correct preset via GameToBaseConfig.json mappings. Currently non-functional. Debug path: verify GameToBaseConfig.json paths are correct, ensure ConspitLink is running before game launches, check if process name detection matches actual game executables. |
| **Fleet-wide config consistency** | 8 pods must feel identical. Manual per-pod config causes drift, inconsistent customer experience, and staff confusion. | MEDIUM | Push Settings.json, Global.json, GameToBaseConfig.json, and all .Base preset files to all 8 pods via rc-agent. File-level sync -- no API needed. Hash comparison for drift detection. |
| **ConspitLink process health monitoring** | ConspitLink crashes break everything -- no FFB, no dashboards, no profile switching. Existing rc-agent watchdog restarts it, but needs to be robust. | LOW | Already partially implemented: `ensure_conspit_link_running()` with 10s watchdog. Enhance with crash-count tracking, graceful restart (never taskkill /F), and post-restart config verification. |
| **Kiosk-mode window management** | ConspitLink window must stay minimized so the kiosk lock screen is always visible to customers. | LOW | Already implemented: `minimize_conspit_window()`. Keep as-is, ensure it survives ConspitLink restarts. |

### Differentiators (Competitive Advantage)

Features that elevate the venue above typical sim racing centers. Not expected, but create "wow" moments and operational efficiency competitors lack.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Telemetry dashboard on wheel display** | Customers see real-time RPM, speed, gear, temps, and flags on the 290GP's 4.3" LCD -- makes Racing Point feel professional/authentic. Most venues leave this unconfigured. | MEDIUM | ConspitLink has built-in telemetry displays toggled via center rotary encoder. Supports AC, ACC, F1 via shared memory and UDP. Need to verify which dashboards work with which games. Configuration is per-wheel in ConspitLink, pushed as part of fleet config. |
| **Shift light LED configuration** | RPM-triggered shift lights surrounding the display create immersion. LEDs can be auto-configured (max RPM detection) or manual per-game. | LOW | ConspitLink supports Auto RPM mode for AC, ACC, iRacing, LMU. Encrypted .conspit config files already exist. For F1 25, may need manual RPM thresholds. Configuration: LED color, trigger RPMs, auto vs manual mode. |
| **RGB button lighting tied to telemetry** | Buttons light up for DRS available, ABS active, TC active, flags, pit entry -- visual feedback without looking at screen. | LOW | ConspitLink supports per-button RGB color + telemetry function assignment. Available functions: DRS Available, DRS Activated, ABS, TC, Wheel Slip, Wheel Lock, Flag, Pit. Configure once per game, push to fleet. |
| **One-click FFB for non-native games** | Games like Forza, EA WRC, and DiRT Rally need special FFB configuration. ConspitLink's "One-Click Game Configuration" handles this. Saves staff from manual setup. | LOW | Documented in ConspitLink function guide. Required for rFactor 2, LMU, DiRT Rally 2.0. Not needed for current venue games (AC, F1 25, ACC use native FFB) but future-proofs for game additions. |
| **Custom Racing Point preset library** | Branded "Racing Point Tuned" presets in #My Presets, curated by staff who know the hardware. Pro driver presets are good starting points but not tuned for 8Nm specifically. | MEDIUM | Create .Base files per game, per driving style (casual vs competitive). Export/import via ConspitLink. Store in version control for reproducibility. Currently only `default.Base` exists in #My Presets. |
| **rc-agent config state monitoring** | Central dashboard showing ConspitLink health, active preset, firmware version, and config hash per pod. Staff see fleet status at a glance. | MEDIUM | rc-agent already monitors USB connect/disconnect and driving input. Add: periodic config file hash check, active preset detection (read ConspitLink state), dashboard status reporting to racecontrol. |
| **UDP telemetry forwarding chain** | ConspitLink receives telemetry on port 20778. Forward to SimHub, external dashboards, or data collection. Enables future analytics, spectator displays, motion platforms. | LOW | ConspitLink Settings.json already has UDP port 20778 configured. UDP forwarding is "send and forget" -- ConspitLink receives, can be configured to forward. SimHub can also forward to additional ports. Chain: Game -> ConspitLink (20778) -> SimHub (optional) -> external consumers. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem good but create problems at a commercial venue.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Per-customer saved profiles** | "Let regulars save their FFB preferences" | Config proliferation across 8 pods. Sync nightmare. Customers tweak things they do not understand, creating support load. ConspitLink is not designed for user-facing profile selection. | Curate 2-3 venue presets per game (Casual, Competitive, Drift). Staff can switch on request. Keep it simple. |
| **SimHub integration for dashboards** | "SimHub has better dashboards and more customization" | SimHub is another process to manage, crashes independently, requires per-game plugin config. ConspitLink's built-in dashboards work without SimHub. Adds complexity to an already fragile stack. | Use ConspitLink's native telemetry dashboards first. Revisit SimHub only if ConspitLink dashboards prove insufficient for customer needs. Explicitly out of scope per PROJECT.md. |
| **Customer-accessible FFB adjustment** | "Let customers adjust force from the wheel" | Customers crank torque to max, complain about clipping, or reduce to zero and think the wheel is broken. Resets needed between every session. | Lock FFB at venue-tuned presets. If a customer wants less force, staff can switch to "Casual" preset. |
| **Firmware auto-update across fleet** | "Always run latest firmware automatically" | Firmware updates can break FFB feel, introduce bugs, or require ConspitLink version changes. Conspit firmware v1.0.4 added CAN support but also changed steering angle limit logic. Untested updates on all 8 pods simultaneously = potential fleet-wide outage. | Manual firmware updates: test on one pod, validate for a week, then roll out to fleet. Track firmware version per pod in rc-agent monitoring. |
| **ConspitLink software modification/patching** | "Fix the auto-switch bug by modifying the executable" | Violates software license. Binary patching breaks on updates. Creates an unmaintainable fork. | Work around bugs via JSON config, HID commands from rc-agent, and process orchestration. Configure, do not modify. |
| **Motion platform / haptic integration** | "Add bass shakers or motion to enhance immersion" | Entirely different hardware stack, separate software (SimHub / SRS), additional failure modes, maintenance burden. Scope explosion. | Defer to a separate project. UDP telemetry forwarding chain enables this later without touching conspit-link scope. |

## Feature Dependencies

```
[Safe centering on session end]
    |--requires--> [ConspitLink process health monitoring]
    |--requires--> [Understanding of HID estop timing vs ConspitLink override (P-20)]

[Auto game-profile switching]
    |--requires--> [Per-game FFB presets tuned]
    |--requires--> [ConspitLink process health monitoring]
    |--requires--> [GameToBaseConfig.json correctly mapped]

[Fleet-wide config consistency]
    |--requires--> [Per-game FFB presets tuned]
    |--requires--> [Telemetry dashboard configured]
    |--requires--> [Shift light + RGB configured]
    (all config must exist before you can push it)

[Telemetry dashboard on wheel display]
    |--requires--> [ConspitLink process health monitoring]
    |--requires--> [UDP telemetry forwarding configured per game]

[Shift light LEDs]
    |--requires--> [Per-game FFB presets tuned] (same config session)

[RGB button lighting]
    |--requires--> [Per-game FFB presets tuned] (same config session)

[rc-agent config state monitoring]
    |--requires--> [Fleet-wide config consistency] (needs to know what "correct" looks like)

[Custom Racing Point preset library]
    |--requires--> [Per-game FFB presets tuned] (presets ARE the library)

[One-click FFB for non-native games]
    |--independent--| (only needed when adding new games)
```

### Dependency Notes

- **Safe centering requires ConspitLink health:** Cannot send centering commands if ConspitLink is crashed. The watchdog must guarantee ConspitLink is alive during session transitions.
- **Fleet push requires all config to exist first:** You cannot push partial config. Tune presets, configure dashboards, set up LEDs, THEN push the complete package.
- **Auto-switch requires correct presets:** If GameToBaseConfig.json points to bad presets, auto-switch makes things worse by loading wrong FFB settings silently.
- **Telemetry dashboard requires UDP config:** Games like F1 25 use UDP telemetry (port 20778). ConspitLink must be listening on the right port. AC/ACC use shared memory (no UDP config needed).

## MVP Definition

### Launch With (v1)

Minimum viable: the wheel does not hurt anyone and works correctly for every session.

- [ ] **Fix stuck-rotation / safe centering** -- Safety-critical. Cannot open venue without this.
- [ ] **Per-game FFB presets for 4 active titles** -- AC, F1 25, ACC/AC EVO, AC Rally. Use Yifei Ye presets as base, tune for 8Nm.
- [ ] **Auto game-profile switching working** -- Debug AresAutoChangeConfig, fix GameToBaseConfig.json mappings. Zero staff intervention between sessions.
- [ ] **Fleet-wide config push via rc-agent** -- Push validated config to all 8 pods. File sync with hash verification.
- [ ] **ConspitLink health monitoring hardened** -- Crash detection, graceful restart, post-restart config verification.

### Add After Validation (v1.x)

Features to add once the core safety and FFB loop is solid.

- [ ] **Telemetry dashboards configured per game** -- Once presets are stable, configure wheel LCD displays.
- [ ] **Shift light + RGB LED configuration** -- Auto RPM for supported games, manual thresholds for others, telemetry-linked button colors.
- [ ] **rc-agent fleet monitoring dashboard** -- Config hash comparison, active preset detection, firmware version tracking across pods.
- [ ] **Custom Racing Point preset library** -- Expand from "one good preset per game" to Casual/Competitive/Drift variants.

### Future Consideration (v2+)

Features to defer until v1 is battle-tested.

- [ ] **One-click FFB for non-native games** -- Only needed when adding Forza, EA WRC, DiRT to game lineup.
- [ ] **UDP telemetry forwarding to external consumers** -- Spectator displays, analytics, motion platform prep.
- [ ] **Per-car FFB sub-presets** -- Different FFB for open-wheel vs GT vs drift cars within the same game. Only if customers request it.
- [ ] **Firmware fleet management** -- Tracked rollout with one-pod-first validation. Only when Conspit releases significant updates.

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Safe centering on session end | HIGH | HIGH | P1 |
| Per-game FFB presets (4 titles) | HIGH | MEDIUM | P1 |
| Auto game-profile switching | HIGH | MEDIUM | P1 |
| Fleet-wide config push | HIGH | MEDIUM | P1 |
| ConspitLink health monitoring | HIGH | LOW | P1 |
| Kiosk window management | MEDIUM | LOW | P1 (already done) |
| Telemetry dashboard | MEDIUM | LOW | P2 |
| Shift light LEDs | MEDIUM | LOW | P2 |
| RGB button telemetry | LOW | LOW | P2 |
| rc-agent fleet monitoring | MEDIUM | MEDIUM | P2 |
| Custom preset library | MEDIUM | MEDIUM | P2 |
| One-click FFB (non-native games) | LOW | LOW | P3 |
| UDP telemetry forwarding | LOW | LOW | P3 |
| Per-car sub-presets | LOW | MEDIUM | P3 |
| Firmware fleet management | LOW | MEDIUM | P3 |

**Priority key:**
- P1: Must have for launch -- safety, core FFB, automation
- P2: Should have, add once P1 is validated and stable
- P3: Nice to have, future consideration when game lineup expands

## Competitor Feature Analysis

| Feature | SRL VMS V5.0 | SimLuxx | Multitap | Racing Point (Our Approach) |
|---------|-------------|---------|----------|---------------------------|
| Session management | Full booking + events | Self-service kiosk | Leaderboards + marketing | rc-agent handles session lifecycle |
| Wheelbase FFB control | Not mentioned (game-level only) | Not mentioned | Not mentioned | Deep ConspitLink integration -- per-game presets, auto-switch, fleet push |
| Fleet config management | Cloud-based sim control | Real-time status monitoring | N/A | rc-agent file sync with hash verification |
| Safety / centering | Not mentioned | Not mentioned | Not mentioned | HID-level estop + ConspitLink spring centering sequence |
| Telemetry displays | Telemetry graphs to phone | N/A | N/A | Wheel-mounted LCD dashboard via ConspitLink native |
| Kiosk mode | Self-serve to staff-operated toggle | Self-service check-in | N/A | Existing kiosk lock screen + ConspitLink minimized |

**Key insight:** No commercial venue management system (SRL, SimLuxx, Multitap, Sim-Department) handles wheelbase-level FFB configuration or fleet hardware management. They all operate at the session/booking/leaderboard layer. Racing Point's conspit-link project fills a gap that nobody else addresses -- the hardware configuration and safety layer beneath the venue management software.

## Sources

- [Conspit Official Store - Ares product page](https://conspit.com/product?id=204) -- hardware specs and ConspitLink features
- [Conspit Link 2.0 PW1 Tutorial](https://oss.conspit.com/file/4/b/1c/PW1%E9%A9%B1%E5%8A%A8%E6%95%99%E7%A8%8BEN%20V1.1.pdf) -- function guide for ConspitLink software
- [Conspit H.AO Function Guide for ConspitLink 2.0](https://conspit.cn/uploads/20250314/CONSPIT_H.AO_FunctionGuideforConspitLink2.0_EN%20v1.1.pdf) -- LED, telemetry, one-click config features
- [Conspit 300GT Function Guide](https://oss.conspit.com/file/1/7/96/CONSPIT_300%20GT_FunctionGuide_EN%20v1.0.pdf) -- wheel-specific LED and dashboard features
- [Conspit Ares Series Torque Boost](https://boxthislap.org/conspit-ares-series-torque-boost-2nm-extra-by-software/) -- firmware update capabilities
- [SRL Venue Management System V5.0](https://www.simracing.co.uk/features.html) -- competitor feature set
- [SimLuxx Venue Management](https://simluxx.com/pages/venue-creation-and-management) -- competitor feature set
- [Multitap Sim Racing Center Software](https://multitap.space/) -- competitor feature set
- [Sim-Department Software](https://www.sim-department.eu/en/software/) -- competitor feature set
- [SimHub UDP Forwarding Wiki](https://github.com/SHWotever/SimHub/wiki/Sharing-UDP-data-with-other-applications) -- UDP telemetry chain patterns
- [OpenFFBoard GitHub](https://github.com/Ultrawipf/OpenFFBoard) -- underlying firmware architecture (VID 0x1209, PID 0xFFB0)
- [Direct Drive Safety Guide](https://simxpro.com/blogs/guides/direct-drive-safety-torque-limits-wrist-injuries-and-the-emergency-stop) -- safety best practices for commercial use
- [Conspit Ares & 290GP Review - OC Racing](https://www.ocsimracing.com/reviews/conspit-ares-review) -- hardware review with ConspitLink software details
- [CONSPIT Ares 10Nm Review - simracing-pc.de](https://simracing-pc.de/en/2025/01/09/conspit-ares-10-nm-wheel-base-review/) -- FFB configuration details
- [290GP Function Guide](https://oss.conspit.com/video/2025/6/6/1749204119783.pdf) -- telemetry dashboard and shift light features

---
*Feature research for: Conspit Link wheelbase management at a sim racing venue*
*Researched: 2026-03-20*
