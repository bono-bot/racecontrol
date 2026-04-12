# Requirements: RacingPoint v48.0 — Codebase Architecture

**Defined:** 2026-04-13
**Core Value:** A customer walks in, launches a game, drives, and their laps appear on the leaderboard. Every time. For every supported game.

## P0 — Core Product Must Work

The absolute minimum. Nothing else matters until these work end-to-end.

### Game Launch

- [ ] **LNCH-01**: Staff launches AC from kiosk — game starts on pod in <5 seconds, no failures
- [ ] **LNCH-02**: Staff launches F1 25 from kiosk — game starts on pod, no pin-grid block, no browser stuck
- [ ] **LNCH-03**: Staff launches iRacing from kiosk — game starts on pod
- [ ] **LNCH-04**: Staff launches LMU from kiosk — game starts on pod
- [ ] **LNCH-05**: AC launch is VMS-parity — write config, spawn process, done (<500 lines, replacing 19,597-line path)
- [ ] **LNCH-06**: Each sim has a SimLauncher trait implementation (<500 lines each) — no copy-paste from AC
- [ ] **LNCH-07**: Staff Launch (Method 1) and PWA Launch (Method 2) are completely separate code paths that converge only at "validate funds -> debit -> launch"

### Lap Recording

- [ ] **LAPS-01**: AC laps are recorded to the database during a session (shared memory -> rc-agent -> server -> SQLite)
- [ ] **LAPS-02**: F1 25 laps are recorded to the database during a session (UDP telemetry -> rc-agent -> server -> SQLite)
- [ ] **LAPS-03**: iRacing laps are recorded to the database
- [ ] **LAPS-04**: LMU laps are recorded to the database
- [ ] **LAPS-05**: Recorded laps appear on the PWA leaderboard within 10 seconds of completion
- [ ] **LAPS-06**: Telemetry (speed, gear, throttle, brake) is captured for all 4 supported games

### Billing — Arcade Model

- [ ] **BILL-01**: Customer wallet must have sufficient funds BEFORE game launch (coin first, game second)
- [ ] **BILL-02**: Funds are deducted at game start, not session creation
- [ ] **BILL-03**: Per-minute billing runs while game is active — game stops when funds run out
- [ ] **BILL-04**: 30-minute and 1-hour tier options work correctly
- [ ] **BILL-05**: Game crash -> billing pauses automatically, resumes on relaunch

### Multiplayer

- [ ] **MULT-01**: AC multiplayer session launches on 2+ pods simultaneously
- [ ] **MULT-02**: All participants' laps are recorded during multiplayer session
- [ ] **MULT-03**: Multiplayer sessions do not disconnect or orphan drivers mid-race
- [ ] **MULT-04**: Multiplayer billing is atomic — all participants debited, or none

## P1 — Business Model Must Work

Revenue, retention, and the cafe. Blocks growth but not basic operation.

### PWA Self-Service Launch

- [ ] **PWAL-01**: Customer selects game + presets in PWA — receives 4-digit numeric PIN
- [ ] **PWAL-02**: Customer enters PIN on pod's 4-digit PIN Grid — game launches without staff
- [ ] **PWAL-03**: PWA launch path is completely independent from staff launch path in code

### Wallet

- [ ] **WLLT-01**: Credits have a type: cash (refundable) vs promotional (non-refundable, spend-only)
- [ ] **WLLT-02**: Refund logic enforces: refund amount <= total cash deposited (promotional credits never refunded)
- [ ] **WLLT-03**: Same wallet debits for games AND cafe (unified path)

### Cafe Integration

- [ ] **CAFE-01**: Cafe orders debit from the customer's wallet (same credits as games)
- [ ] **CAFE-02**: Combo deals exist: game session + cafe item at a discount

### Customer Experience

- [ ] **CUST-01**: PWA shows session stats, personal bests, and telemetry for any game played
- [ ] **CUST-02**: Leaderboard shows fastest laps across AC, F1 25, iRacing, and LMU (not just AC)
- [ ] **CUST-03**: Session wait time from "staff clicks launch" to "customer is driving" is under 15 seconds

### Marketing — Fill Empty Hours

- [ ] **MKTG-01**: System detects low-utilization hours and triggers deal generation
- [ ] **MKTG-02**: Deals push via WhatsApp to registered customers
- [ ] **MKTG-03**: Cafe + racing combo promotions exist as a promotion type

## P2 — Architecture Must Be Sustainable

Prevents the next 1,397 debug commits. Enables future growth.

### Department Event Contracts

- [ ] **EVNT-01**: Typed DomainEvent enum in rc-common defines events for all departments
- [ ] **EVNT-02**: Event bus on comms-link mesh broadcasts events to all subscribed devices
- [ ] **EVNT-03**: Game Launch publishes GameStarted/GameCrashed/GameEnded events
- [ ] **EVNT-04**: Billing subscribes to game events (not polling) for state awareness
- [ ] **EVNT-05**: Correlation ID traces a customer action across all departments

### Decomposition

- [ ] **DCMP-01**: routes.rs (26K lines) split into department-aligned route modules
- [ ] **DCMP-02**: billing.rs (9K lines) split into wallet, session lifecycle, pricing, post-session hooks
- [ ] **DCMP-03**: db/mod.rs (5K lines) split by department table groups
- [ ] **DCMP-04**: All 141 files >500 lines split along department boundaries
- [ ] **DCMP-05**: Lock screen / blanking / browser lifecycle fully separated from game launch logic

### Fix Tooling

- [ ] **FTOL-01**: fix-scope tool maps blast radius for any function (callers, shared state, cross-crate deps)
- [ ] **FTOL-02**: Pre-commit hook warns on fix commits with insertion:deletion ratio > 2:1
- [ ] **FTOL-03**: Band-aid audit: review 36K lines of fix bloat, replace with root fixes

### Foundation

- [ ] **FNDN-01**: Feature Registry classifies every feature as complete/dead/orphaned/incomplete
- [ ] **FNDN-02**: Dead code removed (target: 10-20% codebase reduction)
- [ ] **FNDN-03**: CI gate runs cargo test + cargo clippy before merge to main
- [ ] **FNDN-04**: CODEOWNERS assigns department ownership (Bono vs James)
- [ ] **FNDN-05**: Every source file under 500 lines

## Future (v49+)

- Pods in multiple locations with PIN-based remote launch
- Multi-venue data sync and architecture
- Mobile native app
- Advanced AI coaching across all games
- New game support beyond AC/F1 25/iRacing/LMU

## Out of Scope

| Feature | Reason |
|---------|--------|
| New game support | Stabilize AC/F1 25/iRacing/LMU first |
| New external services | Keep stack lean |
| Multi-venue deployment | Architecture supports it later |
| Mobile native app | PWA sufficient for now |
| AI coaching | Needs working telemetry first (P0) |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| (populated during roadmap creation) | | |

**Coverage:**
- P0 requirements: 17 (game launch, laps, billing, multiplayer)
- P1 requirements: 12 (PWA launch, wallet, cafe, customer, marketing)
- P2 requirements: 15 (events, decomposition, fix tooling, foundation)
- Total: 44 requirements
- Unmapped: 44 (awaiting roadmap)

**Priority rule:** No P1 phase starts until ALL P0 requirements are verified working. No P2 phase starts until ALL P1 requirements are verified working. Exception: P2 decomposition work that directly unblocks a P0 requirement (e.g., splitting ac_launcher.rs to enable LNCH-05) can run in parallel.

---
*Requirements defined: 2026-04-13*
*Last updated: 2026-04-13 after milestone definition*
