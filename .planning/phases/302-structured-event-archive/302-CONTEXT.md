# Phase 302: Structured Event Archive - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Every significant system event is captured, queryable, and permanently archived off-server. SQLite events table with structured schema (type, source, pod, timestamp, payload), daily JSONL export, 90-day SQLite retention with purge, nightly JSONL shipped to Bono VPS via SCP, and REST query API with filters.

Requirements: EVENT-01 through EVENT-05

Success Criteria:
1. After any significant system action (session start/end, deploy, alert fire, pod recovery), a row appears in the events table with type, source, pod, timestamp, and JSON payload populated
2. A JSONL file for the previous day's events exists in the archive directory by 01:00 IST each morning
3. Events in SQLite older than 90 days are purged by the daily maintenance task; the corresponding JSONL files remain untouched
4. The nightly JSONL file for the previous day appears on Bono VPS after the archive task runs
5. GET /api/v1/events returns a filtered list of events when given type, pod, or date range query parameters

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- Extend existing activity_log.rs or create new event_archive.rs module
- Events table in SQLite (same WAL-mode DB)
- Daily JSONL export runs as a tokio task
- SCP to Bono VPS reuses backup_pipeline.rs SCP pattern from Phase 300
- REST API follows existing Axum route patterns
- 90-day purge runs as part of daily maintenance

</decisions>

<code_context>
## Existing Code Insights

Codebase context will be gathered during plan-phase research.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
