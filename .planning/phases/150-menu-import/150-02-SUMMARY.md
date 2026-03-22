---
phase: 150-menu-import
plan: "02"
subsystem: cafe-admin-frontend
tags: [nextjs, typescript, react, menu-import, xlsx, csv, image-upload, modal]
dependency_graph:
  requires:
    - phase: 150-menu-import/150-01
      provides: "cafe-import-api, cafe-image-upload-api, /static/cafe-images ServeDir"
  provides:
    - cafe-import-modal-ui
    - cafe-image-column-ui
    - import-api-ts-types
  affects: [cafe-admin-page, web-dashboard]
tech_stack:
  added: []
  patterns: [two-step-import-modal, raw-fetch-for-multipart, per-row-validation-display, inline-image-upload]
key_files:
  created: []
  modified:
    - web/src/lib/api.ts
    - web/src/app/cafe/page.tsx
key_decisions:
  - "importCafePreview and uploadCafeItemImage use raw fetch (not fetchApi) because fetchApi sets Content-Type: application/json which breaks multipart boundary"
  - "Image column inserted after Name column so thumbnail is visible alongside item name"
  - "Column mapping bar is read-only in v1 — fuzzy matching handles 95% of cases, dropdown override deferred"
  - "displayRows limited to first 100 via importPreview.rows.slice(0, 100)"
requirements-completed: [MENU-01, MENU-06]
duration: 18min
completed: "2026-03-22"
---

# Phase 150 Plan 02: Menu Import Frontend Summary

**Import modal with XLSX/CSV preview-and-confirm flow, per-item image upload column, and 4 new TypeScript API types on the /cafe admin page**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-22T12:40:00+05:30
- **Completed:** 2026-03-22T12:58:00+05:30
- **Tasks:** 2 (Task 3 is human-verify checkpoint)
- **Files modified:** 2

## Accomplishments

- Extended CafeItem TypeScript interface with `image_path: string | null`
- Added 4 new types: `ImportColumnMapping`, `ImportRowResult`, `ImportPreview`, `ConfirmedImportRow`
- Added 3 new API methods: `importCafePreview` (multipart raw fetch), `confirmCafeImport`, `uploadCafeItemImage`
- Import button in /cafe header opens a two-step modal (file upload → preview table with column mapping pills)
- Invalid import rows highlighted red with error text; valid rows count shown in summary
- Confirm button calls confirmCafeImport and reloads items table
- Image column added to items table: 40x40 thumbnail or placeholder + camera icon label that triggers file input
- No TypeScript errors, no `any` types, all brand colors maintained

## Task Commits

Each task was committed atomically:

1. **Task 1: TypeScript types and API methods for import and image upload** - `0b330f34` (feat)
2. **Task 2: Import modal and image column on /cafe admin page** - `be01510e` (feat)

## Files Created/Modified

- `web/src/lib/api.ts` - Added image_path to CafeItem, 4 new import/image types, 3 new API methods
- `web/src/app/cafe/page.tsx` - Import button + modal, image column with thumbnail + camera upload, handleImageUpload + handleImportPreview + handleImportConfirm functions

## Decisions Made

- Used raw `fetch` (not `fetchApi`) for `importCafePreview` and `uploadCafeItemImage` — the `fetchApi` helper always sets `Content-Type: application/json`, which overrides the multipart boundary that the browser sets automatically for FormData. Mixing those headers breaks multipart uploads.
- Column mapping bar rendered as read-only pills in v1 — full dropdown override deferred as stretch goal since fuzzy matching in backend handles the common case.
- Preview table limited to first 100 rows via `.slice(0, 100)` with a note if total_rows > 100.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - TypeScript compiled cleanly on first attempt for both tasks.

## Next Phase Readiness

- Human visual verification needed (Task 3 checkpoint): navigate to /cafe, test Import modal with a CSV, verify image upload
- Backend from 150-01 must be running with `/static/cafe-images` serving enabled
- After verification, phase 150 is complete (MENU-01 + MENU-06 both satisfied)

---
*Phase: 150-menu-import*
*Completed: 2026-03-22*
