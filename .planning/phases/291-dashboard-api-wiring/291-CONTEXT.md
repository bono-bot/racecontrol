# Phase 291: Dashboard API Wiring - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (gap closure phase)

<domain>
## Phase Boundary

Replace stub/fake-data functions in the admin dashboard metrics page with real API calls to Phase 286 endpoints. Fix API contract mismatches between TypeScript interfaces and Rust response structs.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion — gap closure phase. Key constraints:
- Replace 3 TODO-marked stub functions in `racingpoint-admin/src/lib/api/metrics.ts`
- Use `rcFetch` pattern already in the admin app for API calls
- Fix field name mismatches: `metric_name` → `name`, `pod_id` (string) → `pod` (Option<u32>), `recorded_at` (ISO) → `updated_at` (i64 unix epoch)
- Fix NamesResponse: stub returns bare array, API returns `{ names: string[] }` wrapper
- Keep SWR hooks and auto-refresh behavior (already working)

</decisions>

<canonical_refs>
## Canonical References

- `racingpoint-admin/src/lib/api/metrics.ts` — Stub functions with TODO markers
- `racingpoint-admin/src/app/(dashboard)/metrics/page.tsx` — Dashboard page consuming the API functions
- `crates/racecontrol/src/api/metrics_query.rs` — Rust response structs (QueryResponse, NamesResponse, SnapshotResponse)
- `.planning/v34.0-MILESTONE-AUDIT.md` — Gap 2 details + API contract mismatch table

</canonical_refs>

<code_context>
## Existing Code Insights

### API Contract Mismatches (from audit)
| Field | TypeScript | Rust | Fix |
|-------|-----------|------|-----|
| Metric name in snapshot | metric_name | name | Change TS to `name` |
| Pod ID in snapshot | pod_id: string | pod: Option<u32> | Change TS to `pod: number \| null` |
| Timestamp in snapshot | recorded_at: string (ISO) | updated_at: i64 (unix) | Change TS to `updated_at: number` |
| Names response | string[] (bare) | { names: string[] } (wrapped) | Unwrap in fetch |

</code_context>

<specifics>
## Specific Ideas

No specific requirements beyond replacing stubs and fixing contracts.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
