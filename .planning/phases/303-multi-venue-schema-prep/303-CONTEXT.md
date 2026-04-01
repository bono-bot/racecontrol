# Phase 303: Multi-Venue Schema Prep - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

The database schema supports a second venue without data model changes — only a config value changes. Add venue_id column to all major tables with default 'racingpoint-hyd-001'. Migration is backward compatible. All INSERT/UPDATE queries include venue_id. Design doc for venue 2 trigger conditions.

Requirements: VENUE-01 through VENUE-04

Success Criteria:
1. Every major table has venue_id; existing rows all have 'racingpoint-hyd-001'; behavior unchanged
2. Migration runs on production database without data loss or manual intervention
3. All INSERT and UPDATE queries pass venue_id explicitly — no row written without venue_id
4. MULTI-VENUE-ARCHITECTURE.md exists with trigger conditions, schema strategy, sync model

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints:
- venue_id default: 'racingpoint-hyd-001'
- ALTERs must be idempotent (let _ = pattern for existing DBs)
- Must NOT break existing billing, sessions, game launch flows
- routes.rs is 16K lines — venue_id additions must be systematic, not ad-hoc
- Design doc: docs/MULTI-VENUE-ARCHITECTURE.md

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
