# Phase 115: Face Enrollment System - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Build the face enrollment system: CRUD API for person profiles, photo upload and processing through the quality gates + ArcFace pipeline, multi-angle enrollment with 3-5 quality frames, SQLite persistence, in-memory gallery synchronization, and duplicate detection via embedding similarity.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase building on established patterns from Phase 114.

Key constraints:
- SQLite database already exists (recognition/db.rs from Phase 114 has persons + embeddings tables)
- Gallery already exists (recognition/gallery.rs with in-memory Vec + load_from_db)
- Quality gates already exist (recognition/quality.rs)
- ArcFace already exists (recognition/arcface.rs)
- Axum HTTP server already at :8096
- Privacy audit log must be used for all enrollment operations (DPDP compliance)
- API endpoints should follow existing pattern (e.g., /api/v1/privacy/*)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- recognition/db.rs: SQLite CRUD for persons/embeddings (Phase 114)
- recognition/gallery.rs: In-memory gallery with load_from_db, add_entry, remove_person
- recognition/quality.rs: QualityGates with check() method
- recognition/arcface.rs: ArcfaceRecognizer with extract_embedding()
- recognition/alignment.rs: align_face() for 112x112 crops
- recognition/clahe.rs: apply_clahe() for lighting normalization
- privacy/audit.rs: AuditWriter for logging biometric operations

### Established Patterns
- Axum Router with state sharing via Arc
- TOML config for service settings
- tokio::broadcast for event distribution

### Integration Points
- New enrollment routes merge into existing :8096 Router
- Gallery.add_entry() / remove_person() for sync
- AuditWriter.log() for every enrollment/deletion operation

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
