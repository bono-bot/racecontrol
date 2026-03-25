# Phase 95: Staff Gamification - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase adds opt-in gamification for staff: performance leaderboard, skill badges based on observable actions, team challenges with collective goals, and peer kudos. All features live in the admin dashboard and backend API. Participation is strictly opt-in per employee.

</domain>

<decisions>
## Implementation Decisions

### Data Model & Opt-in
- New `gamification_opt_in` boolean column on `staff_members` (ALTER TABLE, default 0) — staff must explicitly opt in
- Observable metrics for leaderboard: sessions hosted (billing_sessions where staff started), events created, total hours worked this week — all derivable from existing data, no new tracking table needed
- Staff gamification UI lives in the admin dashboard at `/staff/gamification` — staff already log in there with PIN + JWT
- Seed 5 initial staff badges in db/mod.rs using existing `staff_badges` table: First Shift, Event Host, Streak 4 Weeks, Pod Master (100 sessions hosted), Team Player (10 kudos received)

### Kudos & Recognition
- Any opted-in staff member can give kudos to any other staff — peer-to-peer, no manager gating
- Kudos visible to all opted-in staff on the gamification dashboard — public recognition is the psychological motivator
- Kudos structure: new `staff_kudos` table with sender_id, receiver_id, message (text), category (teamwork/service/initiative), created_at

### Team Challenges
- Admin/manager creates challenges via admin dashboard — reuse existing `staff_challenges` table schema
- Progress tracked automatically from billing_sessions data (e.g., "Team: host 50 sessions this week") — `current_progress` column already exists
- Challenge rewards are text description only (`reward_description` field) — symbolic, boss fulfills physically

### Claude's Discretion
- API endpoint naming and response structure
- Admin dashboard component file organization
- Badge icon names and visual design

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `staff_badges` table (db/mod.rs line 2282) — already created in Phase 89, has id/name/description/criteria_json/badge_icon/is_active
- `staff_challenges` table (db/mod.rs line 2297) — already created, has goal_type/goal_target/current_progress/status/start_date/end_date
- `psychology.rs` — `evaluate_badges()`, `parse_criteria_json()`, `queue_notification()` patterns reusable for staff
- `staff_members` table — id, name, phone, pin, is_active, role, last_login_at
- Staff JWT auth already works via `/auth/staff-login`

### Established Patterns
- Staff routes in `staff_routes()` under `/api/v1/staff/*` requiring JWT auth
- Admin dashboard: Next.js App Router at racingpoint-admin, PIN login, Edge middleware
- Badge criteria stored as JSON: `{"type":"<metric>","operator":">=","value":100}`

### Integration Points
- Backend: new endpoints in routes.rs under staff_routes
- Admin dashboard: new `/staff/gamification` page with leaderboard, badges, kudos, challenges
- DB: ALTER TABLE staff_members + new staff_kudos table + badge seed data
- Psychology engine: evaluate_staff_badges() function parallel to evaluate_badges()

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches based on existing codebase patterns.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
