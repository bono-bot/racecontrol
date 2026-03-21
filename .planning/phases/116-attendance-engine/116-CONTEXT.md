# Phase 116: Attendance Engine - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the attendance engine: auto-log attendance on face recognition, cross-camera deduplication within 5-minute window, staff clock-in/clock-out state machine with 4-hour minimum shift, and API endpoints for presence/history queries.

</domain>

<decisions>
## Implementation Decisions

### Attendance Logic
- Cross-camera deduplication window: 5 minutes (configurable)
- "Present" determined by last seen within 30 minutes (simple recency)
- Attendance log stored in SQLite table alongside persons DB
- Auto-log: person_id, camera_id, timestamp, confidence — zero manual action

### Staff Shift Tracking
- Clock-in: first recognition of the day (midnight reset)
- Clock-out: last recognition + configurable minimum hours (default 4h)
- Minimum shift duration: 4 hours (standard half-shift)
- Shift history queryable via API

### Claude's Discretion
- SQLite schema for attendance and shifts tables
- State machine implementation details
- API response formats
- How recognition results feed into attendance (broadcast subscriber)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- recognition/tracker.rs: FaceTracker with 60s cooldown (per-person dedup within single camera)
- recognition/gallery.rs: Gallery with find_match() returning person_id
- detection/pipeline.rs: broadcasts RecognitionEvent via tokio::broadcast
- recognition/db.rs: SQLite with persons/embeddings tables
- privacy/audit.rs: AuditWriter for logging

### Established Patterns
- tokio::broadcast for event distribution
- rusqlite with spawn_blocking for DB ops
- Axum Router with Arc state

### Integration Points
- Subscribe to recognition broadcast channel for attendance events
- New attendance module with SQLite tables
- API endpoints merged into :8096 Router
- Staff role flag on persons table (or separate field)

</code_context>

<specifics>
## Specific Ideas

No specific requirements — standard attendance tracking patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
