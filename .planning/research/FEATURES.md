# Features Research: Racing HUD & Wheelbase FFB Safety

**Date:** 2026-03-11
**Milestone:** Subsequent (pre-implementation research)
**Scope:** AC Essentials HUD analysis, racing HUD best practices, FFB safety standards, venue-specific prioritization

---

## 1. AC Essentials HUD — What It Shows

### Built-in Assetto Corsa "Essentials" App (Stock)

The stock AC Essentials app is intentionally minimal. It displays:

- **Speed** (km/h or mph)
- **Current gear**
- **Lap time** (current lap)
- **Best lap time**
- **Sector times** (S1, S2, S3 — current and best)
- **Session timer / laps remaining**
- **Position** (in race mode)
- **Pit limiter indicator**

Layout: Compact single-panel widget, top-left or top-right corner. No customization. Font is AC's default sans-serif, medium size. No color theming — uses AC's grey/white palette.

### CMRT Essential HUD (Most Popular Third-Party)

A modular replacement that ships as 9 independent draggable windows:

| Module | Data Shown |
|--------|-----------|
| Lap Times | Current, best, last lap + delta |
| Sector Splits | S1/S2/S3 with color flash on PB/worse |
| Speed / Gear | Large digit speed, gear number, RPM bar |
| Tire Temps | Per-corner temperature with gradient color |
| Fuel | Liters remaining, laps of fuel left |
| Position | Race position + gap to car ahead/behind |
| Delta | Live delta bar vs. personal best |
| Flag Status | Green/Yellow/Red/Chequered virtual flags |
| Mini Map | Optional — track map with car dot |

**Key design insight:** CMRT keeps each module as a separate draggable window so drivers can arrange to taste. However for a venue (kiosk mode), customization must be locked.

### Race Essentials App (30+ Parameters)

Covers everything CMRT does plus: DRS availability, KERS/ERS, traction control intervention indicator, ABS activation indicator, brake bias display, differential settings read-out, engine temp, water temp, fuel per lap average.

**Venue relevance:** Most of these 30+ params are irrelevant for customers on AC with fixed setups — they create noise, not signal.

---

## 2. Racing HUD Best Practices

### F1 Broadcast Color Coding (Industry Standard)

This is the globally understood convention. Any venue HUD should match it exactly — customers already know it from watching F1.

| Color | Meaning |
|-------|---------|
| **Purple** | Fastest time of anyone in the session (overall best) |
| **Green** | Personal best for this driver in this session |
| **Yellow** | Slower than personal best (not necessarily bad — relative to self) |
| **White/Grey** | No reference time yet / neutral |
| **Red** | Invalid lap (track limits cut) |

**Applies to:** Sector splits (S1/S2/S3), lap time banner, delta indicator bar.

**Source:** F1 official timing, replicated in every major sim (iRacing, ACC, F1 game, AC).

### Information Hierarchy by Driver State

Different moments in a session demand different data priority:

**During lap (high-speed, eyes on track):**
- Speed (glanceable, large digit)
- Gear (large, center-bottom or dash)
- RPM bar (peripheral vision warning for shift point)
- Current sector split (flashes briefly)
- Delta bar vs. PB (thin horizontal bar — how far ahead/behind best)

**Between sectors (brief attention window):**
- S1/S2/S3 split with color flash
- Gap to car ahead/behind (in race)

**On straight / free attention:**
- Lap number / laps remaining
- Position
- Tire heat indicator (for longer sessions)
- Fuel warning (if running low)

**Practice/qualifying (most attention available):**
- All of above plus delta in seconds
- Theoretical best (sum of best sectors)

### SimHub / RaceLab Overlay Conventions

Industry-standard overlay tools (used by 80%+ of serious sim racers) converge on these widget patterns:

- **Relative bar:** Horizontal strip showing cars ±5 seconds around you, with gap deltas — essential for race awareness
- **Fuel calculator:** Average per lap + laps remaining — critical for endurance
- **Proximity radar:** Top-down circular radar showing adjacent cars — reduces blind-side collisions
- **Input trace:** Throttle/brake bars (green/red) — useful for coaching but can be distracting for casual drivers
- **Tire temp heatmap:** Four tire squares, gradient blue→green→yellow→red — immediate visual of grip status

### Viewing Distance & Readability

At typical sim rig distance (50–80 cm from screen):
- Minimum legible font: 18pt / 24px equivalent at 1080p
- Optimal primary data size: 36–48pt (speed, gear)
- Color contrast minimum: 4.5:1 (WCAG AA) for safety-critical indicators
- Brightness matters more than resolution at distance

---

## 3. FFB Safety — What the Industry Does

### The Core Problem

Direct drive wheelbases (including the Conspit Ares 8Nm) run a high-torque servo motor controlled by the game via DirectInput HID protocol. When the game crashes or exits abnormally, three failure modes occur:

1. **FFB freeze at last commanded torque:** Wheel stays locked at whatever force was last sent. At 8Nm this is enough to break a wrist against a fixed rim stop.
2. **FFB runaway (rare):** Game crashes mid-command, driver stack sends max torque command. Motor spins to rotation limit violently.
3. **USB disconnect detection lag:** Windows HID stack can take 2–5 seconds to recognize a device disconnect, during which the wheelbase may hold last state.

### What Hardware Vendors Do

**Fanatec DD Pro / DD2:**
- Hardware E-Stop button (sold separately for DD2, mandatory for pro use)
- Software "wheelbase torque limit" cap (separate from in-game FFB %)
- "Power off mode" — USB disconnect triggers immediate torque-to-zero within 200ms via firmware watchdog
- Torque key: physical key limits max torque for junior/casual users

**OpenFFBoard (Conspit Ares firmware base):**
- Hardware E-Stop pin on STM32 board — pulling low immediately disables TMC motor driver enable pin
- Firmware watchdog: if no valid HID FFB update received for configurable timeout (default ~500ms), motor torques to zero
- Soft limit cap: configurable max torque % independent of game command
- USB disconnection → immediate zero (firmware handles this before OS layer)
- The Ares specifically has a "safety mode" that caps force for less experienced users — this is exposed in ConspitLink software

**Moza:**
- Auto-centering on game exit (software detects game process termination)
- Torque limit via Pit House software
- Natural damping force when no game is connected (prevents free-spin)

### What Software Layers Should Do (rc-agent responsibility)

Industry best practice from commercial sim venues and pro setups:

1. **Monitor game process:** On game process exit (clean or crash), immediately send zero-force command via DirectInput or ConspitLink API before OS cleans up the HID session.
2. **USB watchdog handshake:** OpenFFBoard already has this, but rc-agent should verify via health ping — if wheelbase reports non-zero torque and game is not running, trigger emergency zero.
3. **Max torque cap at session start:** Set wheelbase max torque to 60–70% for trial/new customers, full for regulars. Via ConspitLink command line or API.
4. **Force-to-zero on:** billing end, lock screen engagement, game crash detection, network dropout (if cloud auth fails).
5. **Graceful vs. emergency shutdown distinction:**
   - Graceful: game exits normally → game itself sends zero FFB → rc-agent confirms within 1s
   - Emergency: process killed/crash → rc-agent detects within 500ms → sends zero FFB directly → logs event

### Current Gap at Racing Point

rc-agent already monitors game process via the AI debugger and ai auto-fix. What is NOT yet implemented:
- Active FFB zero command on game exit (we rely on OpenFFBoard firmware watchdog only)
- Torque cap differentiated by customer tier (trial vs. regular)
- FFB status logging in PodStateSnapshot

---

## 4. Table Stakes vs. Differentiators vs. Anti-Features

### Context: Commercial Sim Racing Venue (Not Home Use)

Key differences from home use:
- Customer may have never used a sim rig before
- Average session is 30–60 minutes, not 4+ hours
- Driver cannot configure anything — venue controls it
- Staff need to quickly diagnose what went wrong
- The experience must feel premium from first glance

---

### TABLE STAKES (Must Have — Absence = Bad Experience)

| Feature | Why It's Non-Negotiable | Complexity |
|---------|------------------------|-----------|
| **Speed** (large, km/h) | First thing every customer looks for | Low |
| **Current gear** | Essential — customers stall, crash from wrong gear | Low |
| **Lap time** (current + best) | Core engagement metric — why customers return | Low |
| **Sector S1/S2/S3 splits** with purple/green/yellow | F1 color coding is universally understood | Medium |
| **Session timer** (time remaining in booking) | Customer needs to know how much time is left | Medium |
| **Position** (in multiplayer/race) | "Am I winning?" — basic race awareness | Low |
| **RPM bar** (shift indicator) | Prevents over-rev stalls on unfamiliar cars | Low |
| **Lap number / total laps** (in race) | Race awareness — when does it end? | Low |
| **Flag indicators** (yellow/red/chequered) | Safety for on-track incidents, race end | Medium |
| **DRS zone indicator** (AC, where applicable) | Prevents confusion on straights | Low |

---

### DIFFERENTIATORS (Premium — What Makes Racing Point Stand Out)

| Feature | Why It's Premium | Complexity |
|---------|----------------|-----------|
| **Live delta bar** vs. personal best | Instant skill feedback — drives improvement | Medium |
| **Theoretical best lap** (sum of best S1+S2+S3) | Shows driver their potential — creates excitement | Medium |
| **Personal best banner flash** | Celebration moment when PB is set — emotional hit | Medium |
| **Tire temperature heatmap** (4-corner squares) | Shows car balance — conversation starter with staff | Medium |
| **Proximity radar** (top-down car positions) | Reduces multi-car collisions, feels F1-level | High |
| **Relative bar** (gap to cars ±5s) | Race immersion, strategic awareness | High |
| **Fuel per lap + laps remaining** | Adds strategy dimension for longer bookings | Medium |
| **Session leaderboard** (best laps, all drivers) | Social competition — drives return visits | High |
| **Speed trap** (max speed on straight) | Bragging metric — "I hit 267km/h!" | Low |
| **Cornering G indicator** | Visceral feedback, ties to motion sensation | Medium |
| **Custom Racing Point branding** (logo, colors) | Venue identity, premium feel | Low |
| **Booking time remaining** (prominent countdown) | Reduces anxiety, upsell opportunity at 5min left | Medium |

---

### ANTI-FEATURES (Do NOT Show — Clutter or Confusion)

| What NOT to Show | Why |
|-----------------|-----|
| **Engine temp / water temp** | Meaningless to 95% of customers, creates noise |
| **Brake bias** | Setup metric — customers on fixed setups can't change it |
| **Differential settings** | Same reason as above |
| **Traction control / ABS intervention counter** | Shames beginners; adds complexity for no gain |
| **Input trace** (throttle/brake bars) | Coaching tool — distracts during session, useful only in replay |
| **Full telemetry graphs** | Way too much during active driving |
| **Lap invalidation reason codes** | "INVALID: CUT T3" — customers don't know what T3 is |
| **Debug/diagnostic info** | Never leak rc-agent state, pod IP, crash logs to customer screen |
| **Other drivers' real names** (in multi) | Privacy; use callsigns or seat numbers instead |
| **Billing/payment details** (on HUD) | Keep payment in lock screen flow, not driving HUD |
| **PC system metrics** (CPU/RAM) | Internal only; never show on customer-facing screen |
| **Overlay borders/handles** | Lock the HUD — customers must not be able to drag/move widgets |
| **Mini-map** (track dot) | Useful for home racing; for venues it's distracting clutter on a short unfamiliar track |

---

## 5. Complexity Estimates

Using T-shirt sizing: S (< 1 day), M (1–3 days), L (3–7 days), XL (> 1 week)

### HUD Features

| Feature | Size | Notes |
|---------|------|-------|
| Speed + gear display | S | Direct from AC UDP port 9996 |
| RPM bar | S | Single float value |
| Lap time (current + best) | S | AC UDP already parsed in rc-agent |
| Sector splits + color coding | M | Need split deltas + color state machine |
| Session timer from billing | M | Wire billing state to HUD renderer |
| Personal best tracking | M | Persist across session in memory |
| Delta bar vs PB | M | Running delta calculation per frame |
| Theoretical best | M | Sum of best sectors — needs sector PB store |
| PB flash animation | S | Trigger on lap complete if new PB |
| Tire temp heatmap | M | 4 floats from AC; color gradient mapping |
| Speed trap (max speed) | S | Running max of speed telemetry |
| Proximity radar | L | Requires multi-car position data; opponent tracking |
| Relative bar | L | Opponent timing gap calculation |
| Fuel calculator | M | Fuel per lap running average |
| Session leaderboard | L | Server-side ranking + push to pod screen |
| Booking countdown | M | Wire to billing timer |
| HUD lock (no drag/resize) | S | CSS pointer-events: none on widget layer |
| Racing Point branding | S | CSS + fonts already defined |

### FFB Safety Features

| Feature | Size | Notes |
|---------|------|-------|
| Zero FFB on game crash/exit | M | rc-agent process watcher + HID command |
| Torque cap by customer tier | M | ConspitLink CLI or USB command at session start |
| FFB status in PodStateSnapshot | S | Add wheelbase health field to existing struct |
| USB disconnect detection | M | Poll HID device presence from rc-agent |
| Emergency zero on billing end | M | Hook into existing billing termination flow |
| Verify zero via OpenFFBoard API | L | Need ConspitLink API or USB CDC query |

---

## 6. Key Technical Decisions Required

1. **HUD renderer:** Is this an Electron overlay, a Next.js page in Edge, or an AC Python app? (Impacts how we inject and lock the UI)
2. **AC data source:** Shared memory (same machine, best latency) vs. UDP relay (current rc-agent approach). HUD needs <50ms latency.
3. **FFB zero mechanism:** ConspitLink CLI (current approach in rc-agent) vs. direct USB CDC command to OpenFFBoard vs. DirectInput PID command. CLI is slowest (~500ms). DirectInput is fastest (<50ms).
4. **Session leaderboard scope:** Local (this pod's sessions only) vs. venue-wide (all pods, requires core API endpoint).

---

## Sources

- [CMRT Essential HUD — OverTake.gg](https://www.overtake.gg/downloads/cmrt-essential-hud.69475/)
- [What Does Purple Sector Mean In F1? — F1 Chronicle](https://f1chronicle.com/what-does-purple-sector-mean-in-f1/)
- [What Does Yellow Sector Mean In F1? — Flow Racers](https://flowracers.com/blog/yellow-sector-in-f1/)
- [Sim Racing HUD Preferences — OverTake.gg](https://www.overtake.gg/news/hud-preferences-of-the-overtake-community-immersion-vs-information.4114/)
- [OpenFFBoard GitHub — Safety & E-Stop](https://github.com/Ultrawipf/OpenFFBoard)
- [OpenFFBoard Games Setup Wiki](https://github.com/Ultrawipf/OpenFFBoard/wiki/Games-setup)
- [Risk of Injury with DD Motors — OverTake.gg](https://www.overtake.gg/threads/risk-of-injury-with-dd-motors.187609/)
- [Conspit Ares Series Function Guide PDF](https://oss.conspit.com/video/2025/6/6/1749194528494.pdf)
- [Multitap — Sim Racing Venue Platform](https://multitap.space/features.html)
- [Sim Racing Control VMS V5.0 Features](https://www.simracing.co.uk/features.html)
- [RaceLab Overlay Features](https://racelab.app/)
- [Assetto Corsa Telemetry — UDP Port 9996](https://github.com/rickwest/ac-remote-telemetry-client)
- [Sim Racing Telemetry for Beginners — MySimRig](https://mysimrig.nl/en/blog/simracing/sim-racing-telemetry-for-beginners/)
- [See What You Need: Dash Displays — OverTake.gg](https://www.overtake.gg/news/see-what-you-need-sim-racing-dash-displays.1053/)
