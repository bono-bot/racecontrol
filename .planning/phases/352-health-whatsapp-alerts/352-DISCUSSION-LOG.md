# Phase 352: Health + WhatsApp Alerts - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 352-health-whatsapp-alerts
**Areas discussed:** Per-Subsystem Health Probes, Alert Dispatch Path, Dedup Strategy, 343-02 Wiring, Structured Logging, Log Rotation, Dashboard Page
**Mode:** --auto (all decisions auto-selected as recommended defaults)

---

## D1: Per-Subsystem Health Probe Architecture

| Option | Description | Selected |
|--------|-------------|----------|
| Extend existing `/api/v1/health` | Add `subsystems` object alongside current flat fields (backward-compatible) | ✓ |
| New dedicated `/api/v1/health/subsystems` endpoint | Separate endpoint for detailed probe data | |
| Replace current health response entirely | Break backward compat, new schema | |

**User's choice:** [auto] Extend existing endpoint — backward-compatible, 7 subsystems probed
**Notes:** Implemented in 352-01 commit `1a92e749`. subsystem_health.rs created with LazyLock cached state.

---

## D2: Alert Dispatch Path — Direct vs Relay

| Option | Description | Selected |
|--------|-------------|----------|
| Keep direct Evolution API + relay fallback | Primary: direct dispatch (~200ms). Fallback: comms-link relay when Evolution unreachable | ✓ |
| All alerts through comms-link relay | Single dispatch path, higher latency (~500ms) | |
| Direct only, no fallback | Simplest, no relay dependency | |

**User's choice:** [auto] Direct + relay fallback — preserves existing 10-module pattern, adds resilience
**Notes:** Implemented in 352-02. Fallback chain: direct Evolution API -> POST /relay/alert on James :8766.

---

## D3: Alert Dedup Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Central dedup layer in subsystem_health | HashMap<(subsystem, error_code), Instant>, 10-min window | ✓ |
| Extend existing per-module dedup | Modify all 8 existing modules | |
| No additional dedup | Rely on existing per-module dedup only | |

**User's choice:** [auto] Central dedup in new module — avoids touching 8 existing modules
**Notes:** Implemented in 352-01 with transition detection (ok->degraded, degraded->ok).

---

## D4: Phase 343-02 alert_incidents Wiring

| Option | Description | Selected |
|--------|-------------|----------|
| Wire alert_incidents table | INSERT on every WhatsApp alert, tracks severity/correlation_id/resolved_at | ✓ |
| Skip wiring, leave as TODO | Defer to later phase | |

**User's choice:** [auto] Wire the table — closes 343-02 TODO
**Notes:** Implemented in 352-01. ALTER TABLE ADD COLUMN with duplicate-column error suppression.

---

## D5: Structured JSON Logging

| Option | Description | Selected |
|--------|-------------|----------|
| tower-http TraceLayer middleware | Per-request: method, route, status, latency_ms, staff_id, correlation_id | ✓ |
| Custom middleware | Roll own request logging | |
| Log only errors | Minimal approach | |

**User's choice:** [auto] TraceLayer — uses existing tracing infrastructure
**Notes:** Implemented in 352-03. Custom make_span_with + on_request + on_response.

---

## D6: Log Rotation + Rsync

| Option | Description | Selected |
|--------|-------------|----------|
| Daily rotation + SCP to Bono VPS | Existing rolling appender + new log_sync.rs background task | ✓ |
| rsync via relay exec | Use comms-link relay to trigger rsync | |
| No off-venue backup | Logs stay on venue server only | |

**User's choice:** [auto] Daily SCP sync — hourly check with IST 02:00-04:00 window
**Notes:** Implemented in 352-03. log_sync.rs created with daily SCP pattern.

---

## D7: Admin Dashboard Health Page

| Option | Description | Selected |
|--------|-------------|----------|
| Defer to Phase 354 | Backend API delivered in 352; UI in 354 (UI Hardening) | ✓ |
| Include health page in this phase | Scope creep — adds frontend work | |

**User's choice:** [auto] Defer — Phase 354 already has /settings/health in scope
**Notes:** Phase 354-01 and 354-02 already shipped (nav cleanup + skeleton loading). Health page UI pending.

---

## Claude's Discretion

- Probe interval timing (30s chosen to match app_health_monitor)
- LazyLock vs Arc<RwLock> for cached probe state (LazyLock chosen)
- Correlation ID format (UUID v4)
- SCP sync window (IST 02:00-04:00 chosen — low traffic)

## Deferred Ideas

- Alert analytics dashboard (trending, SLA, MTTR)
- Centralized cross-module dedup
- SMS fallback for WhatsApp
- Per-pod alert routing
- Alert severity standardization across modules
