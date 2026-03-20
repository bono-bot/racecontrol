# Racing Point Operations Security

## What This Is

Security hardening for the Racing Point eSports cafe operations stack — locking down open APIs, protecting customer data (PII + payment details), hardening the kiosk PWA against escape and unauthorized access, and adding authentication to the admin panel. This is a gradual hardening effort, starting with the biggest holes and layering defenses over time.

## Core Value

No unauthorized actor — customer, staff, or external — can manipulate billing, launch sessions without payment, or access customer data.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] API authentication for all billing and session endpoints
- [ ] Admin panel PIN/password protection (Uday-only access)
- [ ] Session launch integrity — prevent bypass via direct API, kiosk manipulation, or bot commands
- [ ] PWA route protection — no access to admin routes or other users' data
- [ ] Kiosk escape prevention — lock down browser/OS breakout vectors
- [ ] HTTPS for data in transit (PWA ↔ server communication)
- [ ] Customer PII storage audit and protection (phone, name, email, payment details, session history)
- [ ] Data-at-rest security for customer records
- [ ] Bot command authorization — verify payment before session launch via Discord/WhatsApp
- [ ] Security audit — discover current auth state, data storage locations, HTTPS coverage

### Out of Scope

- Full compliance certification (PCI-DSS, GDPR) — overkill for current scale, but follow best practices
- Multi-user admin with role-based access — Uday-only PIN is sufficient for now
- Network-level security (firewall rules, VLANs) — separate infrastructure concern
- Penetration testing by external firm — internal hardening first

## Context

- **Current state**: Some basic auth exists but incomplete. Billing API endpoints are open — anyone on the network can add credits or start sessions via curl/Postman. Admin panel has no authentication.
- **Customer data**: Phone numbers, names, emails, payment details, and session history are collected. Storage locations need auditing (likely mix of local SQLite and cloud services).
- **HTTPS status**: Unknown — needs audit. Local network likely plain HTTP.
- **PWA**: Customer-facing PWA has multiple exposure vectors — route access, kiosk escape, unencrypted transit.
- **Threat actors**: Curious tech-savvy customers, staff misuse, external attackers on the network, accidental triggers.
- **Approach**: Gradual hardening — plug the biggest holes first (API auth, admin PIN), then layer on PWA hardening, data protection, and audit trails.

## Constraints

- **Stack**: Rust (racecontrol core), React/TypeScript (kiosk PWA) — auth must work across both
- **Deployment**: James workstation + Bono VPS + pod fleet — auth tokens/keys must work across the distributed system
- **Downtime**: Cafe is operational — changes must be deployable without extended downtime
- **Backward compat**: Kiosk pods must continue working during rollout — phased deployment required

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Gradual hardening over full lockdown | Cafe is live, can't break everything at once | — Pending |
| Uday-only PIN (no role-based access) | Single owner, no need for complex RBAC yet | — Pending |
| Start with API auth + admin PIN | Biggest attack surface — direct financial impact | — Pending |

---
*Last updated: 2026-03-20 after initialization*
