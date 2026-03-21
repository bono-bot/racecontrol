# Project State

## Project Reference

See: .planning/PROJECT-v16.md (updated 2026-03-21)

**Core value:** Automatically identify and log every person entering Racing Point HQ -- customers get recognized and their visit is logged without manual check-in, staff attendance is tracked hands-free.
**Current focus:** Phase 1: RTSP Infrastructure & Camera Pipeline

## Current Position

Phase: 1 of 8 (RTSP Infrastructure & Camera Pipeline)
Plan: 0 of 4 in current phase
Status: Ready to plan
Last activity: 2026-03-21 -- Roadmap created

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**
- Last 5 plans: -
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT-v16.md Key Decisions table.
Recent decisions affecting current work:

- [Roadmap]: Local inference (SCRFD + ArcFace via ort on RTX 4070) instead of cloud API -- faster, free, offline-capable, more private
- [Roadmap]: Single Rust crate rc-sentry-ai on James :8096 -- consistent with racecontrol ecosystem
- [Roadmap]: RTSP relay (go2rtc/mediamtx) as mandatory first step -- prevents stream starvation (Dahua drops after 60-90min with multiple consumers)
- [Roadmap]: NVR at .18 handles all recording -- rc-sentry-ai only proxies playback, no separate recording pipeline
- [Roadmap]: DPDP consent framework before collecting any face data

### Pending Todos

None yet.

### Blockers/Concerns

- RTSP relay (go2rtc/mediamtx) Windows compatibility needs verification during Phase 1
- ort CUDA on Windows needs CUDA Toolkit verification (may already be present from Ollama)
- retina crate primarily tested on Linux -- Windows reliability unknown until Phase 1
- Entrance camera backlight may require reception cameras as primary recognition points
- James disk capacity for any local recording TBD

## Session Continuity

Last session: 2026-03-21
Stopped at: Roadmap created, ready to plan Phase 1
Resume file: None
