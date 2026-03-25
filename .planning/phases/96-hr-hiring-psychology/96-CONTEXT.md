# Phase 96: HR & Hiring Psychology - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase adds hiring psychology (SJT scenarios, realistic job previews), Cialdini-based WhatsApp campaign templates, loss-framed review nudge optimization, and an employee recognition page to the admin dashboard.

</domain>

<decisions>
## Implementation Decisions

### SJT & Hiring Bot
- New `hiring_sjts` table: id, scenario_text, options_json (array of choices), scoring_json (scores per option), is_active — data-driven, editable from admin
- 3 hospitality-specific SJT scenarios seeded: (1) Angry customer demands refund during peak hours (2) Child unsupervised at sim pod (3) Equipment malfunction mid-session
- `job_preview` table: id, title, content, media_url, sort_order — realistic job preview content for hiring bot
- API endpoints to serve SJTs and job previews for the WhatsApp hiring bot

### Campaigns & Templates
- `campaign_templates` table: id, name, cialdini_principle, message_template, target_segment, is_active
- 3 seeded templates using Cialdini principles: Social Proof, Scarcity, Commitment/Consistency
- `nudge_templates` table: id, template_type (review/winback/milestone), copy_text, timing_rules_json — loss-framed review nudges with peak-end timing

### Employee Recognition
- Admin dashboard page at `/hr/recognition` — alongside existing /hr/attendance, /hr/hiring, /hr/leaves
- Shows staff kudos feed (from Phase 95), badges, and a simple recognition wall

### Claude's Discretion
- Exact SJT scenario wording and scoring weights
- Campaign template message phrasing
- Admin page component structure

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `review_nudges` table already exists (Phase 89 post_session_hooks)
- `staff_kudos` table from Phase 95
- `staff_badges` + `staff_earned_badges` from Phase 95
- WhatsApp bot at C:/Users/bono/racingpoint/whatsapp-bot/

### Integration Points
- Backend: new tables + API endpoints in racecontrol routes.rs
- Admin dashboard: new /hr/recognition page in racingpoint-admin
- WhatsApp bot: reads SJTs and campaign templates via API

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
