# Phase 300: SQLite Backup Pipeline - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Operational databases are continuously backed up and staff can see backup health at a glance. Hourly WAL-safe .backup of all SQLite databases, local rotation (7 daily + 4 weekly), nightly SCP to Bono VPS with SHA256 verification, staleness WhatsApp alert (> 2 hours), and admin dashboard backup status panel.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase. Use ROADMAP phase goal, success criteria, and codebase conventions to guide decisions.

Key constraints from standing rules:
- Backup uses SQLite .backup API (WAL-safe, not file copy)
- Nightly SCP to Bono VPS (100.70.177.44) — use existing SSH config
- WhatsApp alerts via existing Bono VPS Evolution API alerter
- Admin dashboard panel in racingpoint-admin Next.js app
- TOML config for backup paths and schedule

</decisions>

<code_context>
## Existing Code Insights

Codebase context will be gathered during plan-phase research.

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase. Refer to ROADMAP phase description and success criteria.

</specifics>

<deferred>
## Deferred Ideas

None — discuss phase skipped.

</deferred>
