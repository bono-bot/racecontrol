# Feature Research

**Domain:** Sim racing venue competitive platform — leaderboards, events, telemetry, championships
**Researched:** 2026-03-14
**Confidence:** HIGH (competitor analysis of racecentres.com ecosystem, Multitap, simracing.co.uk VMS, SRT, case studies)

## Context

This is the v3.0 milestone for an existing system. v1.0 and v2.0 shipped venue operations
(pod management, billing, lock screens, kiosk reliability). v3.0 adds the competitive platform
that converts first-time visitors into returning regulars.

**Primary reference:** The racecentres.com ecosystem (r2r.racecentres.com, blueprint.racecentres.com)
is the direct analogue — a multi-venue platform built on the same VMS (simracing.co.uk) that
serves as the public competitive layer. Its navigation is the baseline: Hotlapping Events,
Group Events, Championships, Circuit Records, Vehicle Records, Driver Data.

**Key insight from case studies (Multitap NXT LVL Gaming):** The single most effective driver
of repeat visits is an automated "you've been beaten" notification. The leaderboard is the
product; everything else is infrastructure to make laps worth submitting.

**Existing foundations in RaceControl:**
- laps table: sector1/2/3_ms, valid flag, driver_id, car, track
- personal_bests, track_records tables
- telemetry_samples: speed, throttle, brake, steering, gear, rpm, xyz
- group_sessions with shared PIN and pod allocation
- drivers table: total_laps, total_time_ms
- cloud_sync: pushes to app.racingpoint.cloud every 30s
- Existing endpoints: /leaderboard/{track}, /public/leaderboard, /public/laps/{id}/telemetry

---

## Feature Landscape

### Table Stakes (Users Expect These)

Features that any competitive sim racing venue leaderboard must have. The racecentres.com
reference platform has all of these. Missing any of them makes Racing Point look incomplete
compared to established venues.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Public leaderboard by track** | Customers type a URL and see who's fastest at each circuit. This is the minimum viable competitive feature. Without it, there's nothing to share or return for. | LOW | Foundation already exists: /leaderboard/{track} endpoint and /public/leaderboard. Needs UI polish and filtering (by car, by date range, by event). |
| **Hotlap events** | Staff can create a "Hotlap Challenge at Spa — McLaren F1 — March 2026" and only laps in that window and car count toward that event's board. This is how racecentres.com venues create recurring reasons to visit. | MEDIUM | Requires events table with: track, car (or class), start_time, end_time, description. Laps are associated with events via timestamp + car match. Staff create/edit events in rc-core admin. |
| **Car class rankings within hotlap events** | Real motorsport uses classes (GT3, GT4, LMP, etc.). At a venue, it's A/B/C/D by performance tier. A customer in Class D competing on the same board as a Class A driver is demotivating — they need their own board to win. | MEDIUM | Classes are staff-defined per event. A car belongs to exactly one class. A lap's class is derived from the car at time of entry. Within an event, one board per class. 107% rule applies within class (laps > 107% of class leader time are excluded as unrepresentative). |
| **107% rule enforcement** | Industry standard in motorsport for qualifying validity. In venue context: if a lap is more than 7% slower than the current class leader, it's valid but marked "out of representative pace" — shown on the board with lower visual weight. The rule prevents complete beginners' times from cluttering a competitive event board. | LOW | Pure calculation: if lap_time_ms > (class_leader_time_ms * 1.07) then flag as `outside_107_pct`. Filter is UI-side toggle; times are still stored. |
| **Circuit records (all-time bests per car per track)** | "Who holds the outright record at Monza in the Ferrari 488?" is the question customers ask. This is separate from event leaderboards — it's the permanent hall of fame. racecentres.com has a dedicated "Circuit Records" section. | LOW | Aggregate query over valid laps: MIN(lap_time_ms) GROUP BY track, car. Needs a materialized view or nightly refresh for performance. Car-level granularity is the baseline; class-level records are a secondary view. |
| **Vehicle records** | Complement to circuit records: "What's the fastest any driver has gone in a Porsche 911 GT3, at any track?" | LOW | Aggregate query: for each car, show fastest lap across all tracks. One row per car with track context. |
| **Driver profile page** | Customers want to see "my stats": best lap per track, total laps driven, lifetime time at venue, history over time. This is the feature that makes the experience personal. LapLegends shows: laps recorded, fastest laps count, PB laps count, tracks covered, cars used. | MEDIUM | Existing drivers table has total_laps and total_time_ms. Profile needs: lap history table (most recent N laps), PB per track/car combination, stats cards (total laps, total time, best result in events), class badge. No login required to view — driver profiles are public, accessed by driver name or ID. |
| **Lap validity display** | Customers know when a time doesn't count. Showing invalid laps on the board (even greyed out) causes confusion. Only valid laps appear by default; a toggle shows all. The valid flag already exists in the laps table. | LOW | Simple UI filter. The valid flag already comes from AC/F1 game data. Invalid laps are stored but hidden by default. Show a count: "2 invalid laps not shown" with a reveal toggle. |
| **Group event results** | When a group of friends books a race session, they want to see the race results: positions, gap to leader, points scored. This is what the "Group Events" section of racecentres.com serves. | MEDIUM | Requires group_event_results table: session_id, driver_id, finishing_position, gap_to_leader_ms, points_scored. Points calculated automatically using F1 system (25/18/15/12/10/8/6/4/2/1 for P1-P10, 0 thereafter). Fastest lap bonus point for P1-P10 holder. |
| **No login required to browse** | Venue competitive platforms are public by nature. Requiring login adds friction that kills organic sharing. racecentres.com is entirely public. Leaderboards, records, and driver profiles must be readable without an account. | LOW | All /public/* routes stay unauthenticated. Driver lookup by name or PIN-linked ID. Staff management routes stay behind auth. |
| **Mobile-first display** | Customers check leaderboards on their phones immediately after a session. If the PWA is not readable on a phone screen, the engagement loop breaks. | LOW | Next.js PWA already exists. Leaderboard tables must use horizontal scroll or card layout on mobile. Font size minimums: 14px for times, 16px for positions. |

### Differentiators (Competitive Advantage)

Features the racecentres.com ecosystem does not have, or does poorly. These are where
Racing Point can pull ahead. Pick 2-3 to execute well rather than shipping all of them shallowly.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Telemetry comparison: speed trace** | "See exactly where you lost time vs the fastest lap." This is professional motorsport analysis available to a casual customer. Sim Racing Telemetry, Track Titan, and VRS all do this — but they require software installs. Showing it in the cloud PWA (mobile-accessible) with zero setup is the differentiator. | HIGH | telemetry_samples already stored (speed, throttle, brake, steering, gear, rpm, xyz). Need: (1) distance normalization (GPS xyz → track distance %, triggered once per lap), (2) time delta channel (delta_ms at each distance point vs reference lap), (3) Recharts/Chart.js area chart with linked cursor. Two lap comparison: player's PB vs track record holder. This is the highest engineering effort item in v3.0. |
| **Telemetry: 2D track map overlay** | A minimap showing the racing line colored by speed (green=fast, red=braking zone) is immediately understandable by non-engineers. The xyz data is already captured. Converting xyz to a 2D path with speed coloring is a strong visual hook. | HIGH | Requires: (1) normalize xyz to 2D projection (drop one axis or use PCA on first 2 principal components), (2) SVG polyline colored by speed quantile, (3) render in PWA with canvas or SVG. Complexity is in the projection and normalization, not the render. |
| **Telemetry: inputs trace** | Throttle, brake, and steering angle plotted alongside speed trace. Shows where a customer was still braking while the reference was at full throttle. This is the "aha" moment that converts a casual visitor into someone who wants to improve. | MEDIUM | Same infrastructure as speed trace. Additional Recharts series on same x-axis (distance %). Linked cursor shows all channels simultaneously. Moderate complexity given the speed trace infrastructure is shared. |
| **Automated "you've been beaten" notification** | The NXT LVL Gaming case study is unambiguous: "People come in specifically saying 'I got the email that someone beat my time.'" This is the single highest-ROI feature for repeat visits. | MEDIUM | Requires: (1) driver email capture (currently via booking flow), (2) trigger in cloud_sync or rc-core: when a new track record is set, compare to previous holder, send email to previous holder via existing send_email.js. Reuses existing Gmail auth. Template: "Your lap record at [track] in [car] was beaten by [time] — come back and take it." |
| **Gold/Silver/Bronze badges on hotlap events** | Instead of just a position number, award badges based on time vs a staff-set reference lap. Gold: within 2% of reference. Silver: within 5%. Bronze: within 8%. This gives every customer a win even if they're P47 on the board. | LOW | Staff sets a reference time when creating the event. Badge calculation is: if within 2% → gold, within 5% → silver, within 8% → bronze. Display as colored pill/badge on the driver row. Low implementation cost, high perceived value for casual customers. |
| **Driver skill class and rating** | A simple performance-based class system (A/B/C/D or Novice/Bronze/Silver/Gold) that updates automatically as a driver improves. When a driver improves their lap times, they see their class badge change. This is a progression mechanic that creates long-term engagement. The SGP ranking system (ML-based) is overkill for a venue; simple Elo or a percentile-based class is sufficient. | MEDIUM | Approach: percentile-based class within a track/car combination. Top 10% of valid laps → Class A. 10-30% → Class B. 30-60% → Class C. Bottom 40% → Class D. Recalculate nightly or on each new lap. Avoids Elo's cold-start problem (no data needed for first classification). Store as driver_class in drivers table or derived from laps. |
| **Multi-round championships** | A season of 4-6 group events, each awarding F1 points, cumulative standings updated after each round. This is what keeps a regular customer group engaged over months. simracing.gp, SimGrid, and Radical Sim Racing all have this. | HIGH | Requires: championships table (name, description, status), championship_rounds (championship_id, event_id, round_number), championship_standings (championship_id, driver_id, total_points, round_positions). Tiebreaker: most wins, then most P2s, etc. Standings page with round-by-round breakdown. Staff create championships and assign group events as rounds. |
| **WhatsApp/email share card** | After a session, a driver can generate a shareable image card: "I set a 1:47.3 at Spa today — Racing Point eSports #4 on the leaderboard." Auto-generated OG image with position, time, track, car, venue branding. Strong organic marketing mechanic. | HIGH | Requires: server-side image generation (satori/vercel OG or canvas). Next.js /api/og?lap=xxx route. The image is a PNG that WhatsApp and iMessage preview natively. High complexity but high viral potential. Defer to v3.x if engineering time is tight. |

### Anti-Features (Commonly Requested, Often Problematic)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Real-time live leaderboard** (WebSocket push on every lap) | "Live updates feel exciting." | At 8 pods with laps every 90-120s each, the update rate is ~1 event per 15 seconds. Polling every 30s is visually identical. WebSocket infrastructure for a public page introduces complexity (connection management at scale, reconnect loops, load) for zero visible benefit at venue scale. The cloud_sync already pushes every 30s. | Poll every 30s from the cloud PWA. Show "Updated X seconds ago." |
| **Login/accounts for customers** | "Accounts let drivers track their progress over time." | Drivers already have a PIN at the venue. Adding a separate web login creates a password reset workflow, account linking (PIN vs web account), and GDPR surface. The racecentres.com platform has no logins — driver lookup is by name. Race venues are not apps; customers don't want another account. | Public driver profiles accessible by driver name or shareable URL. PIN-linked driver IDs. No login required. |
| **Social feed / activity stream** | "Let drivers comment on laps, like each other's times." | Social moderation is a full-time job. Venue scale (hundreds of customers) does not generate enough content for a feed to feel alive, but does generate enough problematic content to require moderation. This is table stakes for a social network, not a sim racing venue. | Discord bot integration (Multitap does this well) — post leaderboard updates to a Discord server that the venue already manages. The moderation burden stays with Discord's existing tools. |
| **Video replay / lap recording** | "Record the customer's session as a video for sharing." | Screen capture of a sim running at 60fps, across 8 pods simultaneously, creates storage and processing requirements that are incompatible with the venue's on-site hardware (64GB RAM server already used for other things). The existing telemetry data is far more useful for improvement than a video. | Telemetry visualization (speed trace, inputs, track map) serves the performance improvement use case without video storage overhead. |
| **Comparison across different cars** | "Let me see my Ferrari time vs someone's Porsche time adjusted for car performance." | Car performance normalization requires a calibrated lap time model per car, which doesn't exist and cannot be crowd-sourced from a single venue's data. The result would be meaningless pseudo-scientific numbers. | Keep leaderboards within a single car or car class. Circuit records are car-specific. Cross-car comparison is a future feature only if a reference lap time model is available from the sim data. |
| **Global ranking vs other sim racing venues** | "Connect to racecentres.com global database." | Racing Point's data is unique (custom UDP telemetry pipeline, Assetto Corsa AC server). Interoperability with the racecentres.com ecosystem requires a proprietary API integration with simracing.co.uk VMS, which is closed. Building toward an open standard (like simresults.net) takes significant engineering investment. | Focus on being the best leaderboard for Racing Point's own customers. Global ranking can be a future v4.0 milestone if customer demand materializes. |
| **Elo/Glicko driver rating** | "A real skill rating like iRacing uses." | Elo/Glicko require head-to-head matchups where the same drivers race each other repeatedly. In a walk-in venue context, the matchup pool is small and uneven. SGP's analysis explicitly states that Elo is wrong for sim racing because race results depend on car, track, and field size — not just relative skill. iRacing has millions of drivers to make Elo converge; Racing Point has hundreds per month. | Percentile-based class system (A/B/C/D) within track+car combination. A driver's class is the percentile of their best lap vs all valid laps at that track in that car. Recalculates automatically. No cold-start problem, no convergence requirement. |
| **Pace car / marshal mode (replays in game)** | "Show the leader's lap as a ghost in the game." | Requires modifying AC server config per session, coordinating ghost lap file export and import, and changes to the ac_server_manager flow. This is a game-level integration with significant complexity. The existing UDP pipeline doesn't capture ghost data. | Telemetry visualization in the PWA is the "ghost" — show the reference lap's inputs alongside yours as a chart. Achieves the coaching goal without game integration complexity. |

---

## Feature Dependencies

```
[Hotlap Events]
    └──requires──> [Events table (track, car/class, start, end)]
    └──requires──> [Car class definition (staff-managed)]
    └──enables──> [Gold/Silver/Bronze badges]
    └──enables──> [107% rule enforcement]
    └──enables──> [Championship rounds]

[Circuit Records]
    └──requires──> [Lap validity filter (valid=true only)]
    └──independent of──> [Hotlap Events]

[Vehicle Records]
    └──requires──> [Same query layer as Circuit Records]
    └──independent of──> [Hotlap Events]

[Driver Profile]
    └──requires──> [Circuit Records (PB per track)]
    └──enhanced by──> [Driver skill class and rating]
    └──enhanced by──> [Championship standings]

[Telemetry Comparison]
    └──requires──> [telemetry_samples already stored]
    └──requires──> [Distance normalization (xyz → track%)]
    └──requires──> [Reference lap selection (track record holder)]
    └──enhanced by──> [2D Track Map]
    └──enhanced by──> [Inputs Trace]

[2D Track Map]
    └──requires──> [Distance normalization (shared with Telemetry Comparison)]
    └──requires──> [xyz → 2D projection]

[Group Event Results]
    └──requires──> [group_sessions (already exists)]
    └──requires──> [F1 points calculation]
    └──enables──> [Championship rounds]

[Championships]
    └──requires──> [Group Event Results]
    └──requires──> [Championship tables (championships, rounds, standings)]

[Driver Skill Class]
    └──requires──> [Sufficient lap history (cold start: first classification after N laps)]
    └──enhanced by──> [Hotlap Events (more structured data)]

[Automated "beaten" notification]
    └──requires──> [Driver email in profile]
    └──requires──> [track_records table (already exists)]
    └──requires──> [send_email.js (already exists)]
    └──independent of──> [Hotlap Events (applies to all-time records)]

[Gold/Silver/Bronze Badges]
    └──requires──> [Hotlap Events (reference time is set per event)]
    └──independent of──> [Championships]
```

### Dependency Notes

- **Hotlap Events unlock a chain of features:** The events table is the dependency for badges, 107% rule, and championship rounds. Build events first.
- **Telemetry visualization is self-contained but expensive:** It shares only the stored telemetry_samples with the rest of the system. Its primary dependency is internal (distance normalization). It does not block or unblock other features — it can be built in parallel or deferred.
- **Circuit/Vehicle Records are independent and fast:** These are aggregate SQL queries over existing data. No new tables required. They should be built early because they populate the competitive platform even before any events run.
- **Championships depend on Group Event Results:** The points system requires at least one group event to be scored. Championship standings are a downstream view of event results.
- **Automated notifications are low-dependency:** They reuse existing email infrastructure and the track_records table. The only new requirement is driver email storage, which may already exist in the billing/driver profile flow.

---

## MVP Definition

### Launch With (v3.0 — this milestone)

The minimum set that makes the competitive platform usable and drives return visits. Focus
on data exposure (records, profiles) and the notification hook. Telemetry is the "wow"
feature but can ship after the leaderboard core is working.

- [ ] **Public leaderboard by track with car filter** — the core product; must be polished not just functional
- [ ] **Circuit records (best per car per track)** — immediately populated from existing data, zero events needed
- [ ] **Vehicle records (best per track per car)** — same query infrastructure as circuit records
- [ ] **Driver profile page** — lap history, PBs, stats cards, accessible by name search
- [ ] **Hotlap events** — staff creation UI in rc-core, public leaderboard per event, car class support
- [ ] **107% rule** — display filter within hotlap event board, toggle to show all
- [ ] **Gold/Silver/Bronze badges** — staff sets reference time on event creation, auto-calculated
- [ ] **Group event results with F1 scoring** — scoring engine (25/18/15/12/10/8/6/4/2/1), results display
- [ ] **Automated "beaten" notification** — email when track record is broken; reuses send_email.js

### Add After Validation (v3.x)

Add once the core leaderboard is live and customer feedback confirms priorities:

- [ ] **Driver skill class (A/B/C/D)** — add after sufficient lap data accumulates; percentile-based
- [ ] **Telemetry: speed trace + time delta** — add after leaderboard is stable; highest-value "wow" feature
- [ ] **Telemetry: inputs trace** — add immediately after speed trace (same infrastructure)
- [ ] **Championships** — add after 2+ group events have been scored; validates the data model first

### Future Consideration (v4+)

Defer until product-market fit on the competitive platform is confirmed:

- [ ] **2D track map overlay** — high complexity, requires good xyz projection; powerful but not MVP
- [ ] **WhatsApp/email share card** — high viral potential but high engineering effort; confirm demand first
- [ ] **Discord bot integration** — effective community mechanic; needs a Discord server to post to
- [ ] **Global multi-venue leaderboards** — requires external API integration; Racing Point data is sufficient for v3.0

---

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Public leaderboard by track | HIGH | LOW | P1 |
| Circuit records | HIGH | LOW | P1 |
| Vehicle records | MEDIUM | LOW | P1 |
| Driver profile page | HIGH | MEDIUM | P1 |
| Hotlap events | HIGH | MEDIUM | P1 |
| 107% rule | MEDIUM | LOW | P1 |
| Gold/Silver/Bronze badges | HIGH | LOW | P1 |
| Group event results (F1 scoring) | HIGH | MEDIUM | P1 |
| Automated "beaten" notification | HIGH | LOW | P1 |
| Driver skill class (A/B/C/D) | MEDIUM | MEDIUM | P2 |
| Telemetry speed trace + delta | HIGH | HIGH | P2 |
| Telemetry inputs trace | HIGH | MEDIUM | P2 |
| Championships (multi-round) | MEDIUM | HIGH | P2 |
| 2D track map overlay | MEDIUM | HIGH | P3 |
| Share card (OG image) | MEDIUM | HIGH | P3 |
| Discord bot integration | LOW | MEDIUM | P3 |

**Priority key:**
- P1: Must ship in v3.0 — drives the core engagement loop
- P2: Ship in v3.x once P1 is validated — deepens engagement
- P3: Future milestone — high cost or unconfirmed demand

---

## Competitor Feature Analysis

| Feature | racecentres.com (VMS) | Multitap | Racing Point v3.0 |
|---------|----------------------|----------|-------------------|
| Public hotlap leaderboard | Yes — Hot Lapping Events section | Yes — "TAPPED IN" global board | Yes — event boards + circuit records |
| Circuit records | Yes — Circuit Records section | Not mentioned | Yes — per car per track |
| Vehicle records | Not evident in r2r | Not mentioned | Yes — per car across tracks |
| Group event results | Not evident in r2r/blueprint | Yes — Traffic Dodging mode | Yes — F1 points scoring |
| Championships | Mentioned in code (Championships page) but not prominent | Not mentioned | Yes — multi-round with cumulative points |
| Driver profile | Basic (search by name, see lap history) | Yes — public driver stats | Yes — richer (PBs, class badge, stats cards) |
| Telemetry visualization | Not public-facing in racecentres.com | iRacing only | Yes — speed trace, delta, inputs (v3.x) |
| "Beaten" notification | Not evident | Yes — automated email | Yes — reuses existing email infra |
| Car class system | Not evident in public interface | Not mentioned | Yes — A/B/C/D within events |
| Gold/Silver/Bronze badges | Not evident | Not mentioned | Yes — per hotlap event |
| Mobile PWA | Responsive but not PWA | Yes | Yes — existing cloud PWA |
| No login required | Yes — fully public | Partially (stats public) | Yes — all public |

---

## Sources

- **racecentres.com ecosystem (MEDIUM confidence):** r2r.racecentres.com and blueprint.racecentres.com — direct web fetch. Confirmed: Hot Lapping Events, Circuit Records, Driver Data sections. Group Events and Championships referenced in page code but not detailed in public UI.
- **simracing.co.uk VMS (HIGH confidence):** simracing.co.uk/features.html and /options.html — direct web fetch. Confirmed: hotlapping leaderboards, group events, championships, telemetry graphs, driver progression, "data to phone" delivery.
- **Multitap case study (HIGH confidence):** multitap.space + NXT LVL Gaming case study — direct web fetch. Confirmed: automated "beaten" email is #1 driver of repeat visits, Discord integration, public driver stats.
- **simracing.gp SGP Ranking (HIGH confidence):** simracing.gp/tutorials/how-does-sgp-ranking-work — direct web fetch. Confirmed: Elo is wrong for sim racing; ML or percentile-based approach is more appropriate for venue context.
- **LapLegends driver profiles (HIGH confidence):** laplegends.net/drivers — direct web fetch. Confirmed: laps recorded, fastest laps, PB laps, tracks, cars, sort/filter on driver list.
- **Sim Racing Telemetry (HIGH confidence):** simracingtelemetry.com — direct web fetch. Confirmed: speed trace, lap comparison, time delta (TDiff), 2D track map overlay, interactive slider, multi-lap session view.
- **Radical Sim Racing (MEDIUM confidence):** radicalsimracing.com — direct web fetch. Confirmed: championship standings, regional divisions, amateur/professional tiers, race schedules, formal rulebooks.
- **107% rule in sim racing (HIGH confidence):** toolcr.com/sim-racing-107-rule-lap-time-calculator + f1.fandom.com/wiki/107%25_Rule — confirmed calculation and community adoption for hotlap event qualification.
- **F1 championship tiebreaker (HIGH confidence):** Wikipedia 2023-24 F1 Sim Racing World Championship — confirmed tiebreaker sequence: most wins → most P2s → most P3s → earliest occurrence.
- **WebSearch: sim racing leaderboard features 2025** — MEDIUM confidence general findings confirmed against direct site inspections above.

---
*Feature research for: RaceControl v3.0 Leaderboards, Telemetry & Competitive*
*Researched: 2026-03-14*
