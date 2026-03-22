# Phase 150: Menu Import - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Bulk menu import from Excel/CSV files with preview-and-confirm flow, plus per-item image upload with local filesystem storage. Extends the existing /cafe admin page from Phase 149.

</domain>

<decisions>
## Implementation Decisions

### Import File Handling
- Support Excel (.xlsx) and CSV formats — no PDF (unreliable table extraction)
- Upload → parse → show preview table → admin reviews/edits invalid rows → confirm to import
- Invalid rows flagged inline (highlighted in red) — admin can fix before confirming
- Auto-detect column mapping by header names with fuzzy matching, allow manual column override

### Image Upload
- Store images in local filesystem: `data/cafe-images/` directory on server
- Serve as static files via Axum static file handler
- Accept JPEG, PNG, WebP — resize to max 800px width on upload
- Per-item upload button (camera icon) in admin table — click to upload/replace image

### Admin UX
- Import button on existing /cafe page — opens a modal with file upload area + preview table
- No separate import page needed
- Categories auto-created from spreadsheet's category column if they don't already exist

### Claude's Discretion
- Exact fuzzy matching algorithm for column header detection
- Preview table pagination/scrolling for large imports
- Image resize library choice (sharp vs jimp vs browser-side)
- Error message wording for parse failures

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/cafe.rs` — existing create_cafe_item and create_cafe_category handlers (Phase 149)
- `crates/racecontrol/src/db/mod.rs` — cafe_items and cafe_categories tables
- `web/src/app/cafe/page.tsx` — existing admin page (407 lines) to extend with import button + image column
- `web/src/lib/api.ts` — existing cafe API methods to extend

### Established Patterns
- Axum multipart upload for file handling
- Server-side JSON data directory: `data/` (existing pattern for runtime data)
- Next.js modal pattern used in other pages

### Integration Points
- `cafe.rs` — add bulk_import_items handler and image_upload handler
- `api/routes.rs` — register new import and image upload routes
- `page.tsx` — add Import button + modal, add image column to table

</code_context>

<specifics>
## Specific Ideas

No specific requirements — follow established patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
