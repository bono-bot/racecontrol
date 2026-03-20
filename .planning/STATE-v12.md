# Project State: v12.0 Operations Security

## Project Reference

See: .planning/PROJECT-v12.md (updated 2026-03-20)

**Core value:** No unauthorized actor can manipulate billing, launch sessions without payment, or access customer data.
**Current focus:** Phase 75 - Security Audit & Foundations

## Current Position

Phase: 75 (1 of 6) — Security Audit & Foundations
Plan: 0 of 2 in current phase
Status: Ready to plan
Last activity: 2026-03-20 — Roadmap created

Progress: [░░░░░░░░░░] 0%

## Performance Metrics

**Velocity:**
- Total plans completed: 0
- Average duration: -
- Total execution time: 0 hours

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| - | - | - | - |

**Recent Trend:**
- Last 5 plans: -
- Trend: -

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT-v12.md Key Decisions table.
Recent decisions affecting current work:

- Gradual hardening over big-bang (cafe is live, can't break everything at once)
- Uday-only PIN (no RBAC needed at current scale)
- API auth + admin PIN first (biggest attack surface, direct financial impact)
- LAN pod traffic stays HTTP+HMAC; HTTPS only for WiFi/external browser traffic
- Expand-migrate-contract rollout pattern for auth (server accepts both modes during transition)

### Pending Todos

None yet.

### Blockers/Concerns

- Server DHCP reservation needed (MAC 10-FF-E0-80-B1-A7 to 192.168.31.23) before any network changes
- tower-helmet and tower_governor crate versions need build-time verification
- Route classification (80+ routes into public/customer/staff/service) needed before Phase 76

## Session Continuity

Last session: 2026-03-20
Stopped at: Roadmap created, ready to plan Phase 75
Resume file: None
