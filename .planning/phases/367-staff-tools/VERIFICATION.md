# Phase 367 — Staff Tools — VERIFICATION

## Status: RETROACTIVE CLOSE

## What Was Built
- Suspect sessions list + telemetry heatmap: `GET /admin/suspect-sessions` and `GET /admin/sessions/{id}/telemetry-heatmap` with admin portal page at `/sessions/suspect` (recharts BarChart drill-down)
- On-demand pod verify: `POST /admin/pods/{pod_id}/verify` with 15s timeout + 8-pod React grid at `/fleet/verify`
- Session replay player: `GET /admin/sessions/{id}/replay` with 10k event cap + admin page with scrubber, speed controls, live gauges
- Batch export: `GET /admin/export` (CSV/JSONL) + `GET /admin/export/estimate` with 30-day cap + admin page with date picker and row estimates
- Phase 362 retro-validation: superadmin test endpoint + 5 per-adapter mismatch unit tests + 8-pod concurrent load test + API/TS docs

## Evidence
- Commits (367-01): `8c2e7047` (racecontrol routes), `c87f630` (admin suspect page)
- Commits (367-02): `b36e8a7b` (racecontrol handler), `b011f35` (admin verify page)
- Commits (367-03): `77fcb43b` (racecontrol replay route), `2ad880a` (admin replay page)
- Commits (367-04): `9b6e94f3` (racecontrol export handlers), `8e434de` (admin export page)
- Commits (367-05): `36f6d2a0` (retro-validation: test endpoint + 5 adapter tests + concurrent test + docs)
- Tests: 959 racecontrol-crate lib tests pass; 7 launch_verifier mismatch tests pass; 1 concurrent load test pass; TypeScript clean (tsc --noEmit exit 0)
- Requirements closed: GLD-G-01, GLD-G-02, GLD-G-03, GLD-G-04, GLD-G-05

## Verification Method
Retroactive artifact closure — code shipped and summarized, VERIFICATION.md was missing.
Closed: 2026-04-16 by James (autonomous session).

## Outstanding Items
- Deploy required: racecontrol binary rebuild + server .23 + cloud (Bono VPS); admin :3201 rebuild on both
- Phase 363 migration must run first (billing_sessions.suspect column prerequisite for 367-01)
- WhatsApp E2E alert verification pending post-deploy (367-05 superadmin test endpoint)
