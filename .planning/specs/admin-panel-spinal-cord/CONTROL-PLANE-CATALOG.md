# Admin control-plane catalog (Track A′)

**Date:** 2026-04-23
**Status:** v0 INVENTORY (audit-derived; not exhaustive)
**Companion:** `GATEWAY-CONTRACT.md`, `project_admin_panel_operator_model.md` doctrine §1, `plan_admin_panel_spinal_cord_gap_20260422.md`

## Doctrine recap (Uday §1)

> There are a lot of features in the admin panel that need to be mapped out. Control both the pods, RC agent, Kiosk and Billing.

Gateway is the transport layer. Above it sits the **control plane** — admin OWNS the controls for these subsystems. Each control = (settings store) + (propagation channel) + (reflex contract).

This catalog enumerates what admin must control. v0 derived from existing audits — not exhaustive. Each entry will get its own design once Track A (gateway) lands.

## Schema

| Field | Meaning |
|---|---|
| Subsystem | pod / rc-agent / kiosk / billing / cafe / pricing / etc. |
| Control | The specific knob the operator turns |
| Current state | UI exists / stub / missing / island |
| Settings store | Where the canonical value lives (admin DB / RC DB) |
| Reflex targets | Surfaces that must hear about a change (downstream) |
| Brain target | RC table/route that owns canonical state (upstream) |
| Failure mode | What happens if push partially fails |

## Pods (subsystem)

| Control | Current | Settings store | Reflex targets | Brain target | Failure mode |
|---|---|---|---|---|---|
| Pod power state (wake/shutdown/restart) | UI wired (`/fleet`) | RC | rc-agent (target pod), kiosk on that pod | RC `/pods/{id}/(wake\|shutdown\|restart)` | Operator retries; idempotent |
| Pod lockdown | UI wired (`/fleet`) | RC | rc-agent (process_guard rules), kiosk | RC `/pods/{id}/lockdown` | Lockdown sticky; clear-maintenance to recover |
| Pod enable/disable | UI wired (`/fleet`) | RC | rc-agent on that pod | RC `/pods/{id}/(enable\|disable)` | Disabled pod refuses billing.start |
| Process guard whitelist | NO UI | RC | rc-agent fetches every 5min | RC `/guard/whitelist/pod-{N}` | rc-agent re-fetch self-heals |
| Pod content drift / inventory | stub UI (`/fleet/content-drift`) | — | — | RC content_drift_events table | Stub today |

## rc-agent (subsystem)

| Control | Current | Settings store | Reflex targets | Brain target | Failure mode |
|---|---|---|---|---|---|
| Feature flags (per-pod or fleet) | NO UI | RC | rc-agent WS push + 5min poll | RC `/flags` | rc-agent uses last-known on WS drop |
| rc-agent config push | NO UI | RC | rc-agent on target pod | RC `/config/pod/{pod_id}` | rc-agent restart picks up |
| Mesh service-key rotation | NO UI | RC | rc-agent fetches every 5min | RC `/pods/mesh-service-key` | Stale key → 401 from MI service routes |
| Pod debug actions (incidents, launches) | partial UI (`/fleet` activity) | RC | rc-agent observes | RC `/debug/launches/*`, `/debug/incidents` | Operator sees stale incident |

## Kiosk (subsystem)

| Control | Current | Settings store | Reflex targets | Brain target | Failure mode |
|---|---|---|---|---|---|
| Kiosk experience presets | stub UI (`/kiosk`) | RC | kiosk reads on launch | RC `/kiosk/experiences` | Old preset until kiosk reload |
| Kiosk allowlist (apps, processes) | NO UI | RC | rc-agent fetches every 5min | RC `/config/kiosk-allowlist` | Empty allowlist blocks all (recovery via re-fetch) |
| Kiosk settings (hours, branding) | NO UI | RC | kiosk reads on launch | RC `/kiosk/settings` | UI stale until reload |
| POS lockdown toggle | NO UI | RC | POS rc-agent | RC `/pods/{pos_id}/lockdown` | POS frozen / unfrozen |

## Billing (subsystem)

| Control | Current | Settings store | Reflex targets | Brain target | Failure mode |
|---|---|---|---|---|---|
| Pricing tiers (rate CRUD) | UI wired (`/billing/rates`) | RC `pricing_rules` | billing app, kiosk, PWA, WhatsApp bot, RC billing engine | RC `/billing/rates` | **Split-state risk**: customer sees old price on PWA, charged new price by RC. Reconciliation needed. |
| Pricing rules (dynamic multipliers) | UI wired (`/pricing`) | RC `pricing_rules` (with day/hour multipliers) | same as above | RC `/pricing/rules` | Same |
| Coupons | UI wired (`/coupons`) | RC | PWA, WhatsApp, kiosk | RC `/coupons` | Old coupon usable until cache expires |
| Memberships | UI wired (`/memberships`) — but in wrong body per memory | RC | PWA primarily | RC `/memberships` | Membership benefit applied based on stale snapshot |
| Wallet bonus tiers | UI wired (`/wallet/bonus-tiers`) | RC | PWA, WhatsApp | RC `/wallet/bonus-tiers` | Customer top-up gets old bonus |
| Wallet top-up presets | UI wired (`/wallet/topup-presets`) | RC | PWA, WhatsApp | RC `/wallet/topup-presets` | Same |
| Promo offers | partial via `/cafe/promos` | RC | billing, cafe POS, WhatsApp | RC `/cafe/promos` | Customer sees old / new promo asymmetry |
| Refund policy / threshold | NO UI | RC | RC billing engine, admin disputes | RC | Operator-blind |
| Disputes | NO UI | RC | — | RC `/admin/disputes` | Backlog grows |

## Cafe / Inventory (subsystem)

| Control | Current | Settings store | Reflex targets | Brain target | Failure mode |
|---|---|---|---|---|---|
| Menu items | local-island (`/cafe`) — admin.db | admin SQLite (Phase 346 dual-path) | cafe POS, PWA, WhatsApp | RC `/cafe/items` (target) | Cafe POS shows wrong item / availability |
| Inventory levels | local-island (`/cafe/inventory`) | admin SQLite | cafe POS | RC `/cafe/items/low-stock` | Stock-out goes unreported |
| Cafe promos | UI wired (`/cafe/promos` via RC proxy) | RC | cafe POS, WhatsApp | RC `/cafe/promos` | Same as billing promos |
| Cafe marketing broadcast | NO UI | RC | WhatsApp | RC `/cafe/marketing/broadcast` | Operator-blind |

## Staff / HR (subsystem)

| Control | Current | Settings store | Reflex targets | Brain target | Failure mode |
|---|---|---|---|---|---|
| Staff CRUD / PIN reset | UI wired (`/staff/manage`) | RC | rc-agent (PIN cache), kiosk, POS | RC `/staff` | Old PIN stops working / new PIN doesn't yet |
| Staff checklists (Phase 391) | NO UI | RC | staff dashboard at start of shift | RC `/staff/checklists*` | Operator-blind |
| Shift handoff | NO UI | RC | next-shift staff dashboard | RC `/staff/shift-handoff` | Handoff context lost |
| Daily PIN distribution | NO UI | RC | staff dashboard, WhatsApp | RC `/employee/daily-pin` | Manual PIN distribution required |
| HR (employees, attendance, leaves, hiring) | local-islands (4 pages) — admin.db | admin SQLite | none (HR isolated today) | (not in RC) | HR data trapped in admin |

## Tournaments / Events (subsystem)

| Control | Current | Settings store | Reflex targets | Brain target | Failure mode |
|---|---|---|---|---|---|
| Tournaments (CRUD, brackets, results) | UI wired (`/tournaments`) | RC | PWA, kiosk, WhatsApp, Discord | RC `/tournaments` | Bracket out of sync across surfaces |
| Championships | NO UI | RC | PWA, kiosk | RC `/staff/championships` | Operator-blind |
| Hotlap events | NO UI | RC | PWA, kiosk | RC `/staff/events` | Operator-blind |
| Time trials | NO UI | RC | PWA, kiosk | RC `/time-trials` | Operator-blind |

## Operations / Diagnostics (subsystem)

| Control | Current | Settings store | Reflex targets | Brain target | Failure mode |
|---|---|---|---|---|---|
| Mesh Intelligence (incidents, solutions, promote) | stub UI (`/mesh-intelligence` calls wrong path) | RC | rc-agent, admin error renderer | RC `/mesh/*` | Operator can't promote/retire solutions |
| Debug incidents | NO UI | RC | rc-agent | RC `/debug/incidents` | Backlog grows |
| Telemetry heatmaps (Phase 367) | stub UI (`/sessions/[id]/replay`) | RC | — | RC `/admin/sessions/{id}/telemetry-heatmap` | Fraud detection invisible |
| Pod verification (Phase 367) | NO UI | RC | — | RC `/admin/pods/{pod_id}/verify` | Operator-blind |
| Suspect sessions (Phase 367) | stub UI (`/sessions/suspect`) | RC | — | RC `/admin/suspect-sessions` | Fraud invisible |

## Patterns observed

1. **Most controls already have an RC backend** — the gap is admin UI + propagation chemistry, not backend logic
2. **Local-API islands are doctrine violations** — HR (4 pages) + cafe menu/inventory + finance + sales + purchases + analytics + calendar own data that never reaches the brain
3. **Reflex contracts are mostly missing today** — when operator changes pricing in admin, the change reaches RC but doesn't actively push to PWA/kiosk/WhatsApp; surfaces re-fetch on their own cadence (or not at all). Doctrine §3 demands proactive push
4. **Split-state risk is highest in billing controls** — pricing change applied at RC but not yet visible on PWA = customer pays new rate while seeing old. This is the failure mode the body must detect

## Track A′ scope

Track A (gateway) covers transport. Track A′ (control plane) covers what flows through. Per doctrine §1+2+3+4, A′ delivers:

- **Settings stores** for admin-owned controls (most live in RC; a few in admin DB for cross-cutting config)
- **Propagation channels** — push notification mechanism from admin to surface subscribers (WebSocket reuse? new SSE? per-control config-fetch endpoint?)
- **Reflex contracts** — per-control declaration of (target surfaces, expected push timing, fallback if push fails)
- **Reconciliation** — periodic check that surfaces hold the values RC says they should; alert on divergence
- **Catalog UI** in admin — operator-facing inventory of what's controllable, what's stub, what's broken

## Out of scope for v0

- Designing the propagation mechanism (WebSocket vs SSE vs poll) — separate spec when one control gets implemented
- HR migration (data-model question, not just routing)
- Local-API island disposition (Uday verdict pending per existing plan)
- Reflex E2E test framework — design when first control ships
