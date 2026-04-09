# Phase 367: Staff Tools - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-04-09
**Phase:** 367-staff-tools
**Mode:** --auto (all choices selected automatically)
**Areas discussed:** Repo split, API design, auth roles, heatmap visualization, on-demand verify, session replay, batch export, GLD-G-05 retro-validation scope, deploy order

---

## Repo Split (D-01)

| Option | Description | Selected |
|--------|-------------|----------|
| All in racecontrol | Backend + frontend templates in Rust | |
| Cross-repo split | Frontend in racingpoint-admin, backend in racecontrol | Yes |

**Auto choice:** Cross-repo split -- matches CLAUDE.md Admin Portal Source of Truth rule. Admin portal is `racingpoint-admin/` (port 3201), plans document backend changes but execution stays in-repo.

---

## Heatmap Library (D-06)

| Option | Description | Selected |
|--------|-------------|----------|
| Recharts | Matches existing admin portal usage in analytics pages | Yes |
| D3.js | More powerful but adds heavy dependency | |
| SVG-only | Lightweight but complex to maintain | |

**Auto choice:** Recharts -- existing dependency, matches established pattern.

---

## On-Demand Verify Mechanism (D-07)

| Option | Description | Selected |
|--------|-------------|----------|
| Synthetic internal test | No real game process; inject deliberate mismatch, verify detection fires, reset | Yes |
| Real game launch | Would require a spare pod; risky for production | |

**Auto choice:** Synthetic internal test -- safe, no real session affected, matches Phase B's deferred test spec.

---

## Session Replay Pattern (D-08)

| Option | Description | Selected |
|--------|-------------|----------|
| Fetch-then-play | Fetch full event array, play back client-side | Yes |
| Real-time WS stream | Server streams events at selected speed | |

**Auto choice:** Fetch-then-play -- simpler, no server-side timer management, reliable for QA review sessions.

---

## Batch Export Format (D-09)

| Option | Description | Selected |
|--------|-------------|----------|
| CSV only | Standard for staff, opens in Excel | |
| JSONL only | Better for tooling | |
| Both (CSV default) | Maximum flexibility | Yes |

**Auto choice:** Both formats, CSV default -- staff can open CSV directly, JSONL available for offline tooling.

---

## GLD-G-05 Test Endpoint Auth (D-10)

| Option | Description | Selected |
|--------|-------------|----------|
| Manager role | Accessible to any manager | |
| Superadmin only | Restricted synthetic test endpoint | Yes |

**Auto choice:** Superadmin only -- synthetic mismatch test could generate WhatsApp noise; restrict to superadmin.

---

## Claude's Discretion

- Color scale for heatmap: red-to-green with grey for missing data
- Max date range for export: 30 days (prevent OOM)
- 8-pod "Verify All" button disabled during in-flight verifies
- Recharts cell sizing for heatmap (planner decides)

---

## Deferred Ideas

- Real-time push notification for new suspect sessions -- future phase
- AI-tier-aware suspect thresholds -- Phase 365 GLD-E-01/E-02
- Historical backfill of suspect flags -- future migration job phase
