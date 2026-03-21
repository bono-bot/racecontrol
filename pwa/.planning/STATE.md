---
gsd_state_version: 1.0
milestone: v1.0
milestone_name: milestone
status: completed
stopped_at: Completed 02-02-PLAN.md (Phase 02 complete)
last_updated: "2026-03-21T22:05:09.435Z"
last_activity: 2026-03-22 — Plan 02-02 complete (PWA deployed, API unreachable pending racecontrol start)
progress:
  total_phases: 10
  completed_phases: 5
  total_plans: 12
  completed_plans: 12
  percent: 100
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-21)

**Core value:** Customers book and pay from anywhere, walk in with a PIN, and race — while Uday sees everything live from his phone without being on-site.
**Current focus:** Phase 5: Kiosk PIN Launch (next unstarted phase)

## Current Position

Phase: 2 of 10 (API + PWA Cloud Deploy) — COMPLETE
Plan: 2 of 2 in current phase (02-02 complete)
Status: Phase 02 complete (PWA live, API pending racecontrol binary on VPS)
Last activity: 2026-03-22 — Plan 02-02 complete (PWA deployed, API unreachable pending racecontrol start)

Progress: [██████████] 100%

## Performance Metrics

**Velocity:**
- Total plans completed: 10
- Average duration: 7 min
- Total execution time: 0.75 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 03-sync-hardening | 3 | 10 min | 3 min |
| 04-remote-booking-pin-generation | 3 | 8 min | 3 min |
| 05-kiosk-pin-launch | 2 | 7 min | 4 min |

**Recent Trend:**
- Last 5 plans: 3, 3, 4, 5, 2 min
- Trend: stable

*Updated after each plan completion*
| Phase 05 P01 | 5 min | 2 tasks | 2 files |
| Phase 05 P02 | 2 | 2 tasks | 3 files |
| Phase 01 P01 | 2 min | 2 tasks | 4 files |
| Phase 01 P02 | 5 min | 3 tasks | 2 files |
| Phase 02 P01 | 3 min | 2 tasks | 3 files |
| Phase 02 P02 | 45 min | 2 tasks | 1 files |

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: 10 phases derived from 47 requirements across 8 categories
- [Roadmap]: Phases 6+7 can run in parallel (both depend on Phase 3, independent of each other)
- [Roadmap]: Phases 8+9 can run anytime after Phase 1 (infrastructure-only dependencies)
- [03-01]: Placed new table migrations at end of run_migrations() before final Ok(())
- [03-01]: origin_id defaults to "local" via serde default function
- [03-02]: Origin filter placed before all upsert blocks in sync_push for early rejection
- [03-02]: Debit intents processed after sync pull, before push, so results push back same cycle
- [03-02]: Wallet debit uses debit_session txn_type with reservation_id as reference
- [03-03]: Status field changed from static "ok" to computed health_status (healthy/degraded/critical/unknown)
- [03-03]: Lag thresholds: healthy <= 60s, degraded <= 300s, critical > 300s, unknown when no sync data
- [04-01]: Used separate route paths for body-accepting handlers due to Axum MethodRouter chaining limitations
- [04-01]: Table is kiosk_experiences not experiences - corrected queries
- [04-01]: ThreadRng scoped to non-async block to avoid Send trait issues
- [04-02]: Refund debit_intents use origin='local' so cloud sync picks them up
- [04-02]: Both pending_debit and confirmed statuses can expire; pending intents cancelled, completed get refund
- [04-02]: Negative amount_paise signals refund in debit_intents table
- [Phase 04]: Cloud mode detected via NEXT_PUBLIC_IS_CLOUD env var rather than URL sniffing
- [Phase 04]: Modify reservation uses inline form on /reservations page rather than navigating to /book
- [05-01]: Redeem-pin route in auth_rate_limited_routes (5/min tower-governor) since customers use directly
- [05-01]: Pod availability checked BEFORE consuming PIN to avoid losing reservation on full venue
- [05-01]: Lockout uses LazyLock static HashMap rather than AppState field
- [05-01]: Pricing tier resolved via kiosk_experiences table
- [Phase 05]: Character grid 7x5 layout for 31-char PIN charset, auto-close success after 15s
- [01-01]: Staging ACME CA used initially to avoid Let's Encrypt rate limits
- [01-01]: Alpine containers use wget healthcheck; bookworm-slim uses curl
- [01-01]: Dashboard port changed from 3000 to 3200 to match port convention
- [01-02]: Staging ACME CA used initially then switched to production after verification
- [01-02]: Repo Caddyfile synced to match VPS after production cert confirmation
- [02-01]: NEXT_PUBLIC_IS_CLOUD defaults to false in Dockerfile so local builds unaffected
- [02-01]: Admin/dashboard service blocks kept in compose.yml, only Caddy depends_on trimmed
- [02-02]: Approved deploy with known issue: api.racingpoint.cloud unreachable (racecontrol binary not running on VPS host)
- [02-02]: PWA deployment proceeds independently of API availability — PWA container + Caddy working

### Pending Todos

None yet.

### Blockers/Concerns

- racingpoint.cloud domain: verify registered and DNS A records point to 72.60.101.58 before Phase 1
- WhatsApp Business API templates: submit booking confirmation + PIN delivery templates early (Phase 3) for approval before Phase 4
- Admin repo (racingpoint-admin): separate repo needs to be cloned to VPS or built and pushed to registry for Phase 6
- racecontrol binary not running on VPS host: api.racingpoint.cloud routes to host.docker.internal:8080 but nothing listens there. Needs racecontrol binary started on VPS for API to work.

## Session Continuity

Last session: 2026-03-22T04:10:00+05:30
Stopped at: Completed 02-02-PLAN.md (Phase 02 complete)
Resume file: None
