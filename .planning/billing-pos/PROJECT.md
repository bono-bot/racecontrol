# Project: RaceControl Billing & Point of Sale

**Milestone:** v3.0 Billing & POS
**Repo(s):** racecontrol (racecontrol, kiosk, PWA), racingpoint-admin (dashboard.racingpoint.cloud)
**Owner:** James Vowles
**Started:** 2026-03-14

## Core Value

Every rupee earned is tracked, every customer gets a receipt, and Uday can see venue revenue in real-time from anywhere — without touching the ops machine.

## Problem Statement

The billing engine (racecontrol) is robust — timers, cost calculation, dynamic pricing, wallet, split sessions, journal entries — but it is invisible to everyone except the pods. Uday cannot see today's revenue from his phone. Customers get no receipt. Staff cannot issue refunds from the kiosk. The cloud dashboard shows cafe sales, not sim sessions.

## What Success Looks Like

1. **dashboard.racingpoint.cloud** shows all 8 pods live, today's revenue, and weekly charts — auto-updating, no refresh needed
2. **PWA** shows a customer's session cost breakdown, lap count, best lap, and timeline — with a WhatsApp receipt sent on session end
3. **Kiosk** lets staff issue refunds and apply discounts without opening a terminal
4. **Every session** has a complete paper trail: payment method recorded, refunds audited, wallet transactions visible on cloud

## Scope

| Surface | In Scope | Out of Scope |
|---------|----------|--------------|
| **Cloud Dashboard** | Live pod grid, revenue ticker, analytics charts, session list, CSV export | POS hardware (card terminals), multi-venue |
| **PWA** | Session detail, WhatsApp receipt, session results | Razorpay additional payment methods |
| **Kiosk** | Refund UI, manual discount, payment method selection | Receipt printing hardware |
| **Backend** | Billing sync to cloud, audit trail API, receipt generation | Multi-currency, external accounting integrations |

## Architecture Context

```
racecontrol (port 8080) ─── billing_sessions (SQLite, local-authoritative)
      │                       │
      │ cloud_sync.rs          │ Phase 1: ADD to sync tables
      ▼                       ▼
Cloud API (VPS)  ◄────── billing_sessions (cloud read-only copy)
      │
      ▼
dashboard.racingpoint.cloud (racingpoint-admin)
      Phase 2: Live pod grid + revenue ticker
      Phase 3: Analytics + charts + export

rc-agent (pods) ──► BillingTick (ws) ──► kiosk overlay, PWA
                                          Phase 4: Session detail + WhatsApp receipt
                                          Phase 5: Refund UI + discount UI
```

## Key Decisions

| Decision | Rationale |
|----------|-----------|
| Cloud is READ-ONLY for billing_sessions | Venue stays authoritative — no cloud dependency for live ops |
| Sync after session completion | No partial-session sync complexity; 60s latency acceptable |
| WhatsApp receipt via Bono (comms-link) | Bono already has Evolution API access and WhatsApp sending capability |
| Refunds in kiosk (not PWA) | Staff-only operation — customer-initiated refunds out of scope |
| racingpoint-admin for cloud dashboard | Existing Next.js app at dashboard.racingpoint.cloud already deployed |

## Requirements

See [REQUIREMENTS.md](REQUIREMENTS.md) — 25 requirements across 5 categories.

## Phases

See [ROADMAP.md](ROADMAP.md) — 6 phases, 13 plans.
