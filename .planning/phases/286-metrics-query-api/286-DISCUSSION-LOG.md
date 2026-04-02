# Phase 286: Metrics Query API - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-04-01
**Phase:** 286-metrics-query-api
**Areas discussed:** Response Format, Error Handling, Authentication, Auto-Resolution
**Mode:** --auto (all recommendations auto-selected)

---

## Response Format

| Option | Description | Selected |
|--------|-------------|----------|
| Array of {ts, value} with metadata wrapper | Unix timestamps, f64 values, resolution field | ✓ |
| Nested object with separate time/value arrays | Columnar format, smaller payload | |
| ISO 8601 timestamps with string values | Human-readable, larger payload | |

**User's choice:** [auto] Array of {ts, value} objects — matches common time-series API conventions
**Notes:** Consistent with recharts data format expected by Phase 287 dashboard

---

## Error Handling

| Option | Description | Selected |
|--------|-------------|----------|
| 200 with empty array | Dashboard-friendly, never breaks on missing data | ✓ |
| 404 for unknown metric | Strict, requires error handling in frontend | |
| 204 No Content | Semantically correct but breaks JSON parsing | |

**User's choice:** [auto] 200 with empty points array — dashboards should degrade gracefully
**Notes:** 400 for truly invalid params (bad date format)

---

## Authentication

| Option | Description | Selected |
|--------|-------------|----------|
| Staff-only (auth middleware) | Protects business intelligence data | ✓ |
| Public (no auth) | Simpler, but exposes revenue/session data | |
| Mixed (names public, query staff-only) | Partial protection | |

**User's choice:** [auto] Staff-only — metrics data reveals business intelligence
**Notes:** Consistent with existing metrics routes being in the staff-only section

---

## Auto-Resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Server auto-selects with client override | <24h=raw, 24h-7d=hourly, >7d=daily; ?resolution= override | ✓ |
| Client must specify resolution | More control, more complexity for dashboard | |
| Always return raw, client aggregates | Simplest server, heaviest client | |

**User's choice:** [auto] Server auto-selects with client override — best UX for dashboard
**Notes:** Dashboard can use defaults; advanced users can force resolution

---

## Claude's Discretion

- SQL query structure and indexing
- Module organization (new file vs extend existing)
- Pagination strategy
- Response caching headers
