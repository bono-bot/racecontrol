---
phase: 442-feature-flag-capability-ui
phase_number: 442
milestone: v50.0 rc-agent-mobile
name: "Feature Flag + Capability UI — admin dashboard toggle page + kill-switch + capability view"
status: ready-to-execute
goal: >
  Admin dashboard UI for toggling per-device + per-driver mobile feature flags with
  audit trail, global kill-switch with confirmation dialog, and read-only capability
  list view per device. Complements Phase 436 which built the server-side flag store
  (REST API + WS push). This phase is the human interface: a Next.js page at
  /mobile/flags under the existing admin dashboard shell from Phase 441, with
  optimistic UI + server-reject rollback, a prominent red global "Halt all drivers"
  kill-switch gated behind a confirmation dialog, a capability list read-only per
  device sourced from Phase 429 registration payloads, and a recent-changes audit
  panel sourced from mobile_flag_audit. Uses the existing staff JWT auth middleware
  and WS channel for real-time updates across concurrent admin sessions.
requirements: [ADMIN-04, FLAG-01, FLAG-02, FLAG-03, FLAG-04]
depends_on: [436, 441]   # 436 = flag server-side API + WS delta + mobile_flag_audit.  441 = admin shell + routing + auth + WS pattern.
wave: 8                  # Wave 7 = 441 (admin shell).  442 runs after 441 + 436 both shipped.  Parallel-safe with 443 (selector-push UI).
plan_count: 8
plans:
  - 442-01-PLAN: UI-SPEC via gsd-ui-researcher (PRE-REQ gate — MANDATORY per CLAUDE.md)
  - 442-02-PLAN: /mobile/flags route scaffold + device picker + flag list render
  - 442-03-PLAN: Toggle switch component with optimistic UI + rollback on server reject
  - 442-04-PLAN: Global kill-switch UI with confirmation dialog
  - 442-05-PLAN: Capability read-only view per device
  - 442-06-PLAN: Audit trail side panel (recent flag changes from mobile_flag_audit)
  - 442-07-PLAN: UI-REVIEW via gsd-ui-auditor (6-pillar MANDATORY post-exec gate)
  - 442-08-PLAN: Playwright E2E — toggle flag + verify agent lifecycle fires within 10s
autonomous: false   # 442-01 (UI-SPEC approval), 442-04 (kill-switch UX decision checkpoint), 442-07 (UI-REVIEW approval), 442-08 (physical-device confirmation at end).
files_modified:
  # ── UI-SPEC + UI-REVIEW artefacts ────────────────────────────────────────
  - .planning/phases/442-feature-flag-capability-ui/UI-SPEC.md          # produced by 442-01 (gsd-ui-researcher)
  - .planning/phases/442-feature-flag-capability-ui/UI-REVIEW.md        # produced by 442-07 (gsd-ui-auditor)
  # ── Next.js admin dashboard pages ────────────────────────────────────────
  - apps/admin/app/mobile/flags/page.tsx                                # NEW — route entry
  - apps/admin/app/mobile/flags/FlagToggleClient.tsx                    # NEW — client component (state + optimistic)
  - apps/admin/app/mobile/flags/DevicePicker.tsx                        # NEW — device selector dropdown
  - apps/admin/app/mobile/flags/FlagList.tsx                            # NEW — per-driver toggle rows
  - apps/admin/app/mobile/flags/KillSwitchPanel.tsx                     # NEW — global pause_all_drivers UI
  - apps/admin/app/mobile/flags/KillSwitchConfirmDialog.tsx             # NEW — confirmation modal
  - apps/admin/app/mobile/flags/CapabilityView.tsx                      # NEW — read-only capability list
  - apps/admin/app/mobile/flags/AuditPanel.tsx                          # NEW — recent changes side panel
  # ── Shared admin UI primitives (REUSE from 441 where possible) ───────────
  - apps/admin/components/ui/ToggleSwitch.tsx                           # MAYBE NEW — may exist from 441; reuse if present
  - apps/admin/components/ui/ConfirmDialog.tsx                          # MAYBE NEW — may exist from 441; reuse if present
  # ── API wrapper + data hooks ─────────────────────────────────────────────
  - apps/admin/lib/api/mobileFlags.ts                                   # NEW — fetch/put/getAudit client wrapper
  - apps/admin/lib/hooks/useMobileFlags.ts                              # NEW — SWR hook around mobileFlags.ts
  - apps/admin/lib/hooks/useFlagDeltaWs.ts                              # NEW — WS subscription hook (reuses 441 WS client)
  - apps/admin/lib/hooks/useCapability.ts                               # NEW — fetches GET :8090/capability via server proxy
  - apps/admin/lib/hooks/useFlagAudit.ts                                # NEW — SWR hook for mobile_flag_audit rows
  # ── Server-side admin dashboard proxy (if 441 did not already do it) ─────
  - crates/racecontrol/src/api/admin_mobile_proxy.rs                    # MAYBE NEW — proxy /capability from admin dashboard to device :8090
  - crates/racecontrol/src/api/routes.rs                                # MODIFY — register proxy route if needed
  # ── WS broadcast addition for admin session fan-out ──────────────────────
  - crates/racecontrol/src/ws/admin_flag_fanout.rs                      # NEW — when mobile_flag_audit row inserted, fan out to all admin WS clients
  - crates/racecontrol/src/state.rs                                     # MODIFY — add admin_flag_senders registry (parallel to mobile_flag_senders)
  - crates/racecontrol/src/flags_mobile.rs                              # MODIFY — put_mobile_flag() also calls broadcast_admin_flag_change()
  # ── Playwright E2E ───────────────────────────────────────────────────────
  - tests/e2e/mobile-flags-ui.spec.ts                                   # NEW — 442-08 integration test
  - .planning/phases/442-feature-flag-capability-ui/SUMMARY.md          # filled at end

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [racecontrol]        # Server-side additions: admin_flag_fanout.rs + optional admin_mobile_proxy.rs
  frontend_rebuild: [admin]         # Next.js admin dashboard — new /mobile/flags route + components
  config_change: none               # No racecontrol.toml changes (flags already live in DB from Phase 436)
  db_migration: none                # No new tables (mobile_flag_audit from Phase 436 is reused for the audit panel)
  infrastructure: none              # Re-uses existing admin WS channel + staff JWT middleware
  data_files: none
  bat_file: none
  cloud_parity:
    - Cloud admin dashboard (Bono VPS) must be rebuilt with new /mobile/flags route (DEPLOY PARITY rule).
    - Cloud racecontrol binary must be redeployed (same build as server .23) so admin_flag_fanout WS channel + optional /capability proxy match.
    - No DB migration on cloud (mobile_flag_audit already present from Phase 436 cloud deploy).
  targets:
    - server              # 192.168.31.23 — racecontrol binary swap (tiny — only admin_flag_fanout.rs + routes.rs)
    - server_admin_ui     # 192.168.31.23:3201 — admin dashboard rebuild + restart
    - bono_vps            # 100.70.177.44 — racecontrol binary + admin dashboard rebuild
  rollback:
    - "Server: revert to previous racecontrol.exe via renaming racecontrol-prev.exe."
    - "Admin UI: stash /mobile/flags directory + rebuild prior commit — standard frontend rollback."
    - "Behaviour on rollback: /mobile/flags page 404s; staff can still toggle flags via curl PUT to /api/v1/mobile/flags (Phase 436 API path is untouched)."
    - "No data-destructive action anywhere — rollback is safe at any point."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: required         # MANDATORY — this is a frontend phase. UI-SPEC.md is a hard gate before 442-02 begins.
  ui_auditor: required            # MANDATORY — post-execution 6-pillar audit before 442-08 drill. Ship gate.
  nyquist_auditor: required       # Business logic: optimistic-UI rollback logic, debounce, version-monotonic check on WS delta, kill-switch priority in UI state.
  mma_audit: required             # Cross-system integration: admin browser <-> racecontrol <-> comms-link <-> Kotlin agent <-> driver lifecycle. Dual reasoning modes REQUIRED (abstract for UI state machine correctness; trace-level for "what does useMobileFlags().data return during the 200ms between optimistic update and server 200 response?").
  integration_checker: required   # Cross-phase: 436 (API + WS) <-> 441 (admin shell) <-> 442 (this) <-> 432 (driver lifecycle that receives the result). Integration check REQUIRED before milestone ship.
  codebase_mapper: skip           # Not a new top-level module — extends apps/admin/ existing Next.js admin dashboard.

risks_summary:
  - "Accidental kill-switch click: catastrophic (halts ALL drivers on ALL devices, ToS incident-response UX). Mitigation: three-layer guard — (1) visually prominent red with warning icon, (2) confirmation dialog requiring typed 'HALT ALL' string to enable the confirm button, (3) 2s delay on confirm button after typed-match to prevent muscle-memory double-click. Mitigation recorded in 442-04."
  - "Optimistic UI drift: toggle appears ON in UI, server rejects (e.g. validator error, race with another admin). Mitigation: rollback to prior state + toast error within 3s; subscribe to mobile_flag_audit WS stream so late-arriving reject still corrects UI."
  - "WS drop causes stale flag state: admin A toggles, admin B's WS is disconnected — B sees stale state. Mitigation: SWR useMobileFlags revalidates on window-focus + every 30s as safety net (Phase 436's 5min agent re-fetch mirrored for admins). Visible 'offline — reconnecting' banner when WS is down (reuse 441 connection-state component)."
  - "Capability list drift: Tab Plus capability changes mid-session (e.g., driver onAppUpdate changes supported_device_types). Mitigation: capability view subscribes to AgentRegistered + CapabilityUpdate WS events (Phase 429-04 defined `capability_update` as a future message type; this phase surfaces it if emitted, otherwise falls back to snapshot-on-device-picker-change)."
  - "Audit panel flood: 100+ rapid toggles by a script (mis-configured automation) saturate the admin feed. Mitigation: virtualize panel (only render 50 latest); 'show more' button; server-side LIMIT 100 on the audit endpoint. No rate-limit in v1 (JWT-gated, trusted staff only)."
  - "Race between two admin sessions: admin A sets enable_zomato=true at v=42; admin B sets enable_zomato=false at v=42 concurrently. Phase 436 endpoint does UPSERT without version CAS — last write wins. Mitigation: UI receives WS delta when either write lands; late admin's UI updates to reflect final state. Document explicitly in UI-SPEC that this is a 'last write wins' surface; future CAS if contention becomes real."
  - "Kill-switch-while-toggling: admin A is mid-toggle of per-driver flag when admin B flips kill-switch. Agent-side (Phase 436) enforces kill-switch priority over per-driver flags — UI must reflect this too: when pause_all_drivers=true, per-driver toggles render read-only + greyed with banner 'kill-switch active — per-driver toggles ignored until released'. Prevents staff confusion."
  - "Admin JWT expiry mid-edit: session token expires while typing in the confirmation dialog. Existing 441 auth middleware returns 401 — PUT fails, rollback happens. Add friendlier toast 'session expired — please re-login' instead of generic 'error 401'."
  - "Next.js basePath middleware: admin dashboard (Phase 441) may use basePath='/admin'. New route must be under basePath — verify before shipping (CLAUDE.md 'Next.js middleware redirects' rule)."

open_questions:
  - id: OQ-1
    question: "Should the global kill-switch be a separate page or inline on /mobile/flags?"
    resolution_recommendation: >
      Inline at the TOP of /mobile/flags, above the device picker, in a visually-distinct red panel.
      Rationale: (a) during ToS incident, staff needs ONE click from dashboard to halt — a separate page adds nav friction;
      (b) inline co-location keeps the semantic "all flag controls live here" model;
      (c) the confirmation dialog + typed-HALT guard makes accidental activation near-impossible even at inline placement.
      FALLBACK: if UI-SPEC researcher disagrees, move to a dedicated /mobile/kill-switch route with a big red button.
      REQUIRES CHECKPOINT — 442-04 is `checkpoint:decision` so Uday can confirm UX pattern before code.
  - id: OQ-2
    question: "Where does the capability list data come from? Real-time GET :8090/capability, or last-known server snapshot?"
    resolution_recommendation: >
      Last-known server snapshot with manual 'refresh' button that proxies to device :8090/capability.
      Rationale: (a) device may be offline — real-time fetch would show a blocking spinner;
      (b) capability rarely changes (Phase 429 registration payload + Phase 432 driver install);
      (c) refresh button gives staff an escape hatch when they know a driver was just added.
      Snapshot source: extend racecontrol to cache last-known capability per device from registration WS message.
      If racecontrol does not yet cache this, add a small new `device_capabilities` table in the SAME plan (442-05).
  - id: OQ-3
    question: "Does /mobile/flags require a dedicated 'admin with mobile permissions' role, or does generic staff JWT suffice?"
    resolution_recommendation: >
      Generic staff JWT for v1 (same gate as Phase 436 REST endpoints).
      Rationale: v50.0 fleet is 2 devices, operated by Uday + 2 on-site staff — role stratification adds complexity without real benefit.
      Document in UI-SPEC that future 'mobile_admin' role can be layered in without UI changes (check a claim on the JWT).
  - id: OQ-4
    question: "What is the WS channel name for admin flag-fanout broadcasts?"
    resolution_recommendation: >
      Reuse existing admin WS connection (from 441) with new message type `AdminFlagChange` payload.
      Rationale: (a) avoids adding another socket per open admin tab; (b) mirrors Phase 436 pattern of extending CoreToAgentMessage additively; (c) admin dashboard already has a WS connection for reception updates (441) — one more message variant is the minimum new surface area.
  - id: OQ-5
    question: "Should the audit panel show actor email, or a generic 'staff' label for privacy?"
    resolution_recommendation: >
      Show actor email (full staff JWT subject).
      Rationale: internal-only tool, 3 named operators, full traceability is a legal/safety win for a system that can halt all drivers.
      If operator pool scales beyond 5, revisit with partial masking.

---

# Phase 442 — Feature Flag + Capability UI

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 442 |
| Name | Feature Flag + Capability UI — admin dashboard toggle page + kill-switch + capability view |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | ADMIN-04, FLAG-01, FLAG-02, FLAG-03, FLAG-04 (UI-side) |
| Dependencies | Phase 436 (server-side flag API + WS delta + mobile_flag_audit), Phase 441 (admin dashboard shell + routing + WS) |
| Wave | 8 |
| Status | Ready to execute |
| Autonomous | No — 442-01 (UI-SPEC approval), 442-04 (kill-switch UX decision), 442-07 (UI-REVIEW approval), 442-08 (physical-device drill confirmation) are human-verify / decision checkpoints |
| Ship test | Admin toggles `enable_zomato_on_rcm_tab_plus=true` in UI → agent lifecycle install() fires on Tab Plus within 10s; admin activates kill-switch → all drivers on both devices halt within 10s; audit panel shows both events with actor + timestamp + before/after |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 14)

1. **Toggle-with-audit:** Admin can toggle `enable_<driver>_on_<device>` flags; every toggle produces an audit-log row visible in the UI within 2s and the device fires the lifecycle hook within 10s.
2. **Capability view:** Admin sees per-device read-only capability list (which drivers the device declared at registration).
3. **10s propagation:** Toggle change visible on device within 10s (verified via agent log entry or persistent-notification body update).

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable)

- **T-1:** Navigating to `/admin/mobile/flags` (under basePath) shows a page with a device picker (Tab Plus, M07), a prominent red kill-switch panel, and a list of per-driver toggles for the selected device. (ADMIN-04)
- **T-2:** Clicking a per-driver toggle updates the UI immediately (optimistic) and, within 1s, a toast confirms "Flag saved". Within the same window, the audit panel shows a new row with actor + timestamp + flag_key + before/after. (ADMIN-04, FLAG-02)
- **T-3:** If the server rejects the toggle (e.g. validator error, 403), the toggle rolls back to its prior state within 3s and an error toast explains why. (FLAG-01 correctness)
- **T-4:** Clicking the global "Halt all drivers" button opens a confirmation dialog requiring the operator to type `HALT ALL` exactly; only after the typed-match does the confirm button enable, and then only after a 2s disable window. (FLAG-04)
- **T-5:** Confirming the kill-switch dispatches `PUT /api/v1/mobile/flags/*/pause_all_drivers` with body `{enabled: true}`; within 10s all drivers on both devices halt (verified via agent logs + capability view reporting active_drivers: []). (FLAG-04)
- **T-6:** When `pause_all_drivers` is active, all per-driver toggle rows render read-only with greyed state + banner "Kill-switch active — per-driver flags ignored until released". Releasing the kill-switch restores interactivity. (FLAG-04 UI safety)
- **T-7:** Capability view shows each device's declared capabilities (from agent registration payload) with a "Refresh" button that proxies to `GET :8090/capability` via the server. (CAPREG-02)
- **T-8:** Audit panel lists the most-recent 50 flag changes with actor (email), target_device, flag_key, before/after, timestamp; "Show more" loads older entries. (FLAG-02)
- **T-9:** If admin A and admin B have the page open simultaneously, admin A's toggle appears in admin B's UI within 2s via WS broadcast. (FLAG-01 real-time)
- **T-10:** WS disconnect shows a "reconnecting..." banner (reused from Phase 441); while disconnected, SWR revalidates on window-focus so stale state is bounded. (FLAG-01 resilience)

### Required artifacts (files that must exist, with minimum behaviour)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `.planning/phases/442-feature-flag-capability-ui/UI-SPEC.md` | Full design from gsd-ui-researcher | 300 | Page layout wireframe, state machine for toggle, kill-switch dialog spec, accessibility notes, error-state handling |
| `apps/admin/app/mobile/flags/page.tsx` | Route entry | 40 | `"use client"` OR server component wrapping client components; renders KillSwitchPanel + DevicePicker + FlagList + CapabilityView + AuditPanel |
| `apps/admin/app/mobile/flags/FlagToggleClient.tsx` | Stateful client wrapper | 120 | useMobileFlags + useFlagDeltaWs + handleToggle(flag_key, newValue) optimistic pattern |
| `apps/admin/app/mobile/flags/DevicePicker.tsx` | Device selector | 50 | Dropdown of [rcm-tab-plus, rcm-m07] with heartbeat-age badge |
| `apps/admin/app/mobile/flags/FlagList.tsx` | Per-driver rows | 100 | Maps over available drivers; renders ToggleSwitch per row; read-only when kill-switch active |
| `apps/admin/app/mobile/flags/KillSwitchPanel.tsx` | Global kill-switch summary panel | 80 | Red-bg, warning icon, current state, "Halt all drivers" button, release button when active |
| `apps/admin/app/mobile/flags/KillSwitchConfirmDialog.tsx` | Confirmation modal | 120 | Typed-HALT gate, 2s disable window, dual-action (cancel + confirm), keyboard ESC cancels |
| `apps/admin/app/mobile/flags/CapabilityView.tsx` | Read-only capability list | 80 | Lists driver_id + supported_device_types per device; Refresh button |
| `apps/admin/app/mobile/flags/AuditPanel.tsx` | Recent changes feed | 100 | Virtualized list; actor + ts + flag_key + before→after; "Show more" pagination |
| `apps/admin/lib/api/mobileFlags.ts` | API wrapper | 80 | `getFlags(deviceId)`, `putFlag(deviceId, flagKey, enabled)`, `getAudit(deviceId?, limit)` typed |
| `apps/admin/lib/hooks/useMobileFlags.ts` | SWR hook | 40 | SWR key `/mobile/flags/${deviceId}`, revalidate on focus + 30s |
| `apps/admin/lib/hooks/useFlagDeltaWs.ts` | WS subscription | 60 | Listens for AdminFlagChange events; invalidates SWR cache on match |
| `apps/admin/lib/hooks/useCapability.ts` | Capability fetcher | 40 | Fetches cached snapshot; `refresh()` triggers live proxy call |
| `apps/admin/lib/hooks/useFlagAudit.ts` | Audit feed hook | 40 | SWR paginated; accepts deviceId filter |
| `crates/racecontrol/src/ws/admin_flag_fanout.rs` | Server-side fan-out | 100 | When mobile_flag_audit INSERT, broadcast AdminFlagChange to admin_flag_senders |
| `tests/e2e/mobile-flags-ui.spec.ts` | Playwright E2E | 200 | Login, navigate, toggle, assert agent log line within 10s, kill-switch drill |
| `.planning/phases/442-feature-flag-capability-ui/UI-REVIEW.md` | 6-pillar audit from gsd-ui-auditor | 200 | Layout, accessibility, errors, states, feedback, resilience |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| FlagToggleClient.handleToggle | mobileFlags.putFlag | Fetch | grep `putFlag` in `FlagToggleClient.tsx` |
| mobileFlags.putFlag | PUT /api/v1/mobile/flags/:deviceId/:flagKey | Next.js fetch → server proxy | grep `/api/v1/mobile/flags` in `mobileFlags.ts` |
| KillSwitchConfirmDialog.confirm | mobileFlags.putFlag(*, pause_all_drivers, true) | Fetch | grep `pause_all_drivers` in `KillSwitchConfirmDialog.tsx` |
| put_mobile_flag (Phase 436) | admin_flag_fanout::broadcast_admin_flag_change | Rust call | grep `broadcast_admin_flag_change` in `flags_mobile.rs` |
| admin_flag_fanout | admin_flag_senders.iter() → mpsc::send | Rust call | grep `admin_flag_senders.read().await` in `admin_flag_fanout.rs` |
| useFlagDeltaWs | SWR.mutate(`/mobile/flags/${deviceId}`) | React hook | grep `mutate` in `useFlagDeltaWs.ts` |
| KillSwitchPanel (when pause_all active) | FlagList props `disabled={true}` | React prop | grep `disabled` in `KillSwitchPanel.tsx` + `FlagList.tsx` |
| CapabilityView.refresh | GET /api/v1/mobile/devices/:id/capability (proxy) | Fetch | grep `/capability` in `CapabilityView.tsx` |
| AuditPanel | GET /api/v1/mobile/flag-audit?device=…&limit=50 | SWR | grep `/flag-audit` in `useFlagAudit.ts` |
| WS AdminFlagChange event | optimistic UI rollback/confirmation | Hook | grep `AdminFlagChange` in `useFlagDeltaWs.ts` |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/PROJECT.md
@./.planning/phases/436-feature-flag-system/PLAN.md               # Server-side API + WS delta this UI consumes
@./.planning/phases/441-admin-dashboard-reception-view/PLAN.md    # Admin shell + routing + WS patterns this phase reuses
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md   # Protocol envelope + capability payload shape
@./.planning/phases/432-driver-framework-capability-registry/PLAN.md  # DriverRegistry — target of lifecycle dispatch
@./crates/racecontrol/src/flags_mobile.rs                         # (after 436) — REST endpoint signatures + broadcast hook
@./crates/racecontrol/src/ws/mobile_flag_sync.rs                  # (after 436) — WS pattern for server->device; mirror it for server->admin
@./crates/rc-common/src/protocol.rs                               # CoreToAgentMessage — AdminFlagChange variant goes here
@./apps/admin/                                                    # (after 441) — existing Next.js admin dashboard conventions

### Interfaces executors will need (extracted from Phase 436 + 441)

The admin dashboard CONSUMES these Phase 436 interfaces:

```
GET  /api/v1/mobile/flags/:device_id            → MobileFlagMapResponse
PUT  /api/v1/mobile/flags/:device_id/:flag_key  → MobileFlagRow (body: {enabled: bool})
GET  /api/v1/mobile/flag-audit                  → [MobileFlagAuditRow]   (NEW — Phase 442 adds this endpoint; derives from mobile_flag_audit table from 436)
WS   AdminFlagChange envelope                   → NEW — Phase 442 adds server-side fanout + client subscription
```

Flag-key validation regex (enforced server-side per Phase 436 OQ-2):
```
^mobile\.(pause_all_drivers|enable_[a-z0-9_]+_on_[a-z0-9_]+)$
```

The UI constructs these keys — it must mirror the server regex so it can show client-side feedback. For the per-driver case, the UI takes `driver_id` + `device_id` and composes `mobile.enable_${driver_id}_on_${device_id}`. For the global case, the UI uses the literal string `mobile.pause_all_drivers`.

Admin WS envelope (new in Phase 442, extends Phase 441's admin WS channel):

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "admin_flag_change",
  "from": "racecontrol",
  "to": "admin:*",
  "ts": 1713440000000,
  "id": "uuid-v4",
  "payload": {
    "flag_key": "mobile.enable_zomato_on_rcm_tab_plus",
    "target_device": "rcm-tab-plus",
    "target_driver": "zomato",
    "old_value": false,
    "new_value": true,
    "version": 42,
    "actor": "uday@racingpoint.in",
    "at": "2026-04-18T14:03:22Z"
  }
}
```

The admin page subscribes to this envelope via the Phase 441 WS client, and on receipt calls `mutate("/mobile/flags/" + payload.target_device)` to refresh SWR cache. This gives real-time updates across open admin tabs without polling.

## 5. Atomic plan breakdown (8 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion. Ordering is strict — 442-01 MUST complete before 442-02 begins (subagent-gate ordering per CLAUDE.md).

---

### 442-01-PLAN — UI-SPEC via gsd-ui-researcher (PRE-REQ gate)

**Goal:** Produce `.planning/phases/442-feature-flag-capability-ui/UI-SPEC.md` via the `gsd-ui-researcher` subagent. This is a MANDATORY gate per CLAUDE.md Subagent Gates table ("Any frontend … gsd-ui-researcher … UI-SPEC.md … Before planning"). The SPEC is the authoritative design; all subsequent 442-NN plans implement against it. Any deviation found in 442-07 (UI-REVIEW) is a bug.

**Covers:** ADMIN-04 (design foundation), FLAG-01..04 (UX shape)

**Dependencies:** Phase 441 shipped (admin shell exists so researcher can reference its patterns), Phase 436 shipped (API exists so researcher knows the data shape).

**Type:** `checkpoint:human-verify` — after the subagent produces UI-SPEC.md, Uday reviews and explicitly approves before 442-02 can begin.

#### Tasks

1. Invoke `gsd-ui-researcher` with this brief (paste into the agent prompt):
   ```
   Phase: 442 (v50.0 rc-agent-mobile) — Feature Flag + Capability UI.
   Produce UI-SPEC.md covering:
     - Route: /admin/mobile/flags (Next.js App Router, under existing basePath)
     - Layout: top-of-page kill-switch panel (red, prominent) → device picker → two-column
       layout (left: per-driver toggles; right: capability view + audit feed)
     - Kill-switch confirmation: typed "HALT ALL" + 2s delay before confirm enables
     - Optimistic UI: toggle flips immediately, rolls back on server reject within 3s
     - Real-time: WS AdminFlagChange event invalidates SWR cache; "offline"
       banner when WS disconnected
     - Accessibility: keyboard nav for all toggles, ARIA live region for audit feed,
       dialog focus-trap for confirm modal
     - Error states: server 400 (validator), 401 (expired JWT), 403, 500,
       network failure — each with a distinct toast/banner UX
     - Kill-switch-active read-only mode for per-driver toggles
     - Audit panel: actor email, timestamp (IST), flag_key, before→after, virtualized;
       "Show more" pagination
     - Capability view: per-device table (driver_id, supported_device_types), "Refresh"
       button (live proxy), last-synced timestamp
     - Loading states + empty states (no devices, no drivers, no audit history)
     - Match racecontrol brand (Racing Red #E10600 for kill-switch,
       Asphalt Black #1A1A1A bg, Montserrat body, Enthocentric headers)
   Reference Phase 441's existing admin-shell components for reuse
   (ToggleSwitch, ConfirmDialog, toast system, WS client, auth wrapper).
   Explicitly list which existing components are reused vs. new.
   ```

2. Capture the resulting UI-SPEC.md at `.planning/phases/442-feature-flag-capability-ui/UI-SPEC.md`.

3. Checkpoint: present UI-SPEC.md to Uday for approval. Uday responds with one of:
   - "approved" → proceed to 442-02.
   - "revise X, Y, Z" → invoke gsd-ui-researcher a second time with explicit deltas.
   - "redesign A as a separate page" (the OQ-1 inline-vs-separate kill-switch decision lives here) → incorporate into UI-SPEC.md before approval.

#### Acceptance

- `.planning/phases/442-feature-flag-capability-ui/UI-SPEC.md` exists, >= 300 lines.
- Contains sections for: layout, state machine, error handling, accessibility, reuse list.
- Uday responds with "approved" (checkpoint gate).

#### Checkpoint (human-verify + decision)

Uday reviews UI-SPEC.md in full. Approval message unlocks 442-02 onward.

#### G4 NOT TESTED list (carry into commit)

- No code yet — this is design-only.
- Integration with Phase 436 API untested (442-02 onward).

#### Commit message

```
docs(442-01): UI-SPEC.md for mobile feature-flag + capability UI

Produced by gsd-ui-researcher per CLAUDE.md Subagent Gates (MANDATORY before frontend work).
Layout: inline kill-switch + device picker + two-column (toggles / capability+audit).
Kill-switch: typed-HALT + 2s delay guard. Optimistic UI with WS-triggered rollback.
Approved by Uday on <date>.

Covers: ADMIN-04 design, FLAG-01..04 UX
```

---

### 442-02-PLAN — `/mobile/flags` route scaffold + device picker + flag list render

**Goal:** Create the page at `apps/admin/app/mobile/flags/page.tsx` with a device picker and a read-only flag list pulling data from `GET /api/v1/mobile/flags/:device_id`. No mutations yet — static render + data fetch only. This lays the skeleton onto which 442-03..06 attach.

**Covers:** ADMIN-04 (route exists), FLAG-01 (GET path consumed)

**Dependencies:** 442-01 (UI-SPEC approved)

**Type:** `auto`

#### Tasks

1. Add route directory `apps/admin/app/mobile/flags/` with:
   - `page.tsx` — server component that renders `<FlagToggleClient />` client wrapper.
   - `FlagToggleClient.tsx` — `"use client"` component; uses `useMobileFlags(deviceId)` + `useFlagDeltaWs(deviceId)` (hooks added here).
   - `DevicePicker.tsx` — dropdown listing `[rcm-tab-plus, rcm-m07]` (hardcoded in this plan; Phase 442-05 will source from server-cached device list).
   - `FlagList.tsx` — renders rows per driver. Derive driver list from flag-key parse (strip `mobile.enable_` prefix, strip `_on_<device>` suffix).

2. Add API wrapper `apps/admin/lib/api/mobileFlags.ts`:
   ```ts
   export interface MobileFlagMapResponse {
     device_id: string;
     flags: { flag_key: string; enabled: boolean; version: number; updated_at: string }[];
     resolved_at: string;
   }

   export async function getFlags(deviceId: string): Promise<MobileFlagMapResponse> {
     const res = await fetch(`/api/v1/mobile/flags/${encodeURIComponent(deviceId)}`, {
       credentials: "include",  // admin dashboard session cookie
     });
     if (!res.ok) throw new Error(`Flags fetch failed: ${res.status}`);
     return res.json();
   }
   ```

3. Add SWR hook `apps/admin/lib/hooks/useMobileFlags.ts`:
   - SWR key: `/mobile/flags/${deviceId}`.
   - Revalidate on window-focus, every 30s interval, and on-network-reconnect.
   - Returns `{ data, error, isLoading, mutate }`.

4. Render read-only toggle rows (styled but disabled) — no mutation wiring yet. Each row shows: driver name, flag_key, current state badge, last-updated IST timestamp. CSS matches brand (Racing Red/Asphalt Black per CLAUDE.md Brand Identity).

5. Route uniqueness sanity: `grep -n "app/mobile/flags" apps/admin/app/ -r` must return exactly this one route.

6. basePath check: if `next.config.ts` has `basePath: "/admin"`, the actual URL is `/admin/mobile/flags`. Verify the middleware does NOT double-prefix (CLAUDE.md "Next.js middleware redirects: use '/' not '/basePath'" rule).

7. Server-side: no changes needed — Phase 436 `GET` endpoint is already live.

8. Unit test: `FlagList.test.tsx` with React Testing Library — render with mock flag data, assert rows exist, toggles are disabled.

#### Acceptance

- Dev server `npm run dev` in `apps/admin/` — browsing `/admin/mobile/flags` renders the page without errors.
- `curl -I http://localhost:3201/admin/mobile/flags` returns 200 (or 307 if auth redirect for unauth'd).
- `/api/v1/mobile/flags/rcm-tab-plus` endpoint is called exactly once on page load (network tab verification).
- With mock data, ToggleSwitches render `disabled={true}` (class has `opacity-50` or equivalent).
- Unit test passes.
- Route uniqueness script returns exactly 1 match for this route.

#### G4 NOT TESTED list

- No mutations yet (442-03).
- No kill-switch (442-04).
- No capability view (442-05).
- No audit feed (442-06).
- Real device toggle roundtrip (442-08 drill).

#### Commit message

```
feat(442-02): /admin/mobile/flags route scaffold + device picker + flag list (read-only)

Next.js App Router page renders device-picker + per-driver flag rows
(disabled toggles) pulled from Phase 436's GET /api/v1/mobile/flags/:device_id.
SWR with 30s revalidation + focus/reconnect hooks. Brand-matched styling.

Covers: ADMIN-04 (route), FLAG-01 (GET consumed)
Not tested: PUT mutations (442-03), kill-switch (442-04), capability (442-05),
audit (442-06), agent E2E (442-08).
```

---

### 442-03-PLAN — Toggle switch component with optimistic UI + rollback on server reject

**Goal:** Wire the `ToggleSwitch` to `PUT /api/v1/mobile/flags/:device_id/:flag_key`. On click, update UI immediately (optimistic), fire the PUT, confirm on 200 via toast + audit panel row, rollback on 4xx/5xx with error toast. Subscribe to `AdminFlagChange` WS events so a concurrent admin's change appears in this admin's UI within 2s.

**Covers:** ADMIN-04 (toggle + audit UX), FLAG-01 (PUT path), FLAG-02 (audit write triggers the WS which surfaces in UI), FLAG-03 (UI fires within 10s — server-side 10s is Phase 436's responsibility)

**Dependencies:** 442-02, 436 (PUT endpoint + WS delta)

**Type:** `auto` with `tdd="true"`

#### Behavior (TDD-first)

- Test 1: Click toggle → optimistic state flips immediately (before fetch resolves).
- Test 2: PUT returns 200 → state stays; toast "Flag saved" shown.
- Test 3: PUT returns 400 with validator error → state rolls back; toast shows error message.
- Test 4: PUT returns 401 → state rolls back; "Session expired, please re-login" toast.
- Test 5: Network failure (fetch rejects) → state rolls back; generic "Network error" toast.
- Test 6: Admin-B's AdminFlagChange WS event arrives → Admin-A's UI flips within 1 tick without a network call.
- Test 7: Rapid double-click protection: clicking toggle during in-flight PUT is a no-op (disabled during request).
- Test 8: Kill-switch active → toggle renders `aria-disabled="true"` with banner; clicks are no-ops with explanatory toast.

#### Tasks

1. Extend `FlagToggleClient.tsx` with `handleToggle(flag_key: string, newValue: boolean)`:
   ```tsx
   const { data, mutate } = useMobileFlags(deviceId);
   const [pendingKeys, setPendingKeys] = useState<Set<string>>(new Set());

   async function handleToggle(flag_key: string, newValue: boolean) {
     if (pendingKeys.has(flag_key)) return;
     if (killSwitchActive) {
       toast.warn("Kill-switch active — per-driver flags ignored until released");
       return;
     }
     setPendingKeys(prev => new Set(prev).add(flag_key));

     // Optimistic update
     const prev = data;
     const next = optimisticUpdate(data, flag_key, newValue);
     mutate(next, { revalidate: false });

     try {
       await putFlag(deviceId, flag_key, newValue);
       toast.success("Flag saved");
       // Do not mutate here — AdminFlagChange WS will invalidate cache
     } catch (err) {
       mutate(prev, { revalidate: false });  // rollback
       if (err.status === 401) toast.error("Session expired — please re-login");
       else if (err.status === 400) toast.error(`Validator rejected: ${err.detail}`);
       else toast.error(`Save failed: ${err.message}`);
     } finally {
       setPendingKeys(prev => {
         const n = new Set(prev); n.delete(flag_key); return n;
       });
     }
   }
   ```

2. Add `apps/admin/lib/api/mobileFlags.ts::putFlag(deviceId, flagKey, enabled)`:
   - Compose full flag_key if caller passed the short form (`enable_zomato`) by prepending `enable_` + appending `_on_${deviceId}`. The PUT endpoint URL-segment accepts the full `mobile.enable_zomato_on_rcm_tab_plus` per Phase 436's route.
   - Throws a typed error with `{status, detail}` so UI can branch.

3. Add `apps/admin/lib/hooks/useFlagDeltaWs.ts`:
   - Use the existing admin WS client from Phase 441 (`useAdminSocket()` or equivalent — read 441 source to confirm API).
   - On receiving an `admin_flag_change` envelope where `target_device === deviceId`, call `mutate("/mobile/flags/" + deviceId)` to refresh SWR.
   - Also update the audit-panel SWR cache (442-06 will add `useFlagAudit`).

4. Server-side additions (Rust) — needed because Phase 436 only fanned out to mobile agents, not to admin browsers:
   - `crates/racecontrol/src/state.rs`: add `admin_flag_senders: Arc<RwLock<HashMap<SessionId, mpsc::UnboundedSender<CoreMessage>>>>` (parallel to 441's existing admin WS senders — extend that registry if already present rather than adding a new one).
   - `crates/racecontrol/src/ws/admin_flag_fanout.rs` (NEW):
     ```rust
     pub async fn broadcast_admin_flag_change(
         state: &AppState,
         flag_key: &str,
         target_device: &str,
         target_driver: Option<&str>,
         old_value: Option<bool>,
         new_value: bool,
         version: u64,
         actor: &str,
     ) {
         // Snapshot-then-drop (CLAUDE.md lock rule)
         let senders = {
             let g = state.admin_flag_senders.read().await;
             g.values().cloned().collect::<Vec<_>>()
         };
         let envelope = build_admin_envelope(/* ... */);
         for s in senders {
             let _ = s.send(envelope.clone());
         }
     }
     ```
   - `crates/racecontrol/src/flags_mobile.rs::put_mobile_flag()` — after 436's `broadcast_mobile_flag_delta()`, add `broadcast_admin_flag_change(...)`. Both run in parallel; failure of either is logged but does not fail the HTTP response.
   - `crates/rc-common/src/protocol.rs`: extend admin message enum (from Phase 441) with `AdminFlagChange(AdminFlagChangePayload)` — additive per Phase 429-04 unknown-field rule.

5. Unit tests (React Testing Library + msw for fetch + mock WebSocket):
   - All 8 behaviors listed above.
   - `FlagToggleClient.test.tsx` covers 1-7.
   - `FlagDeltaWs.test.ts` covers 6 specifically (mock WS event → mutate called).

6. Rust unit tests:
   - `admin_flag_fanout_tests::broadcasts_to_all_admin_senders()` — register 2 mock admin senders, fire broadcast, both receive.
   - `admin_flag_fanout_tests::survives_closed_sender()` — close one sender, fire broadcast, other still receives.

#### Acceptance

- All 8 behaviors pass in unit tests (`apps/admin/ && npm test -- FlagToggleClient`).
- Rust tests pass (`cargo test -p racecontrol-crate admin_flag_fanout_tests`).
- Manual dev-server smoke: toggle flips optimistically, then server log shows PUT, then WS admin event fires, then SWR cache refreshes. Network tab confirms exactly 1 PUT per user click.
- `grep -n "AdminFlagChange" crates/rc-common/src/protocol.rs` returns ≥ 1 match.
- Lock-across-await static check: `grep "senders.read().await" crates/racecontrol/src/ws/admin_flag_fanout.rs | grep -v "let .* = "` returns nothing.

#### G4 NOT TESTED list

- Kill-switch UX (442-04).
- Capability view (442-05).
- Audit panel row render (442-06).
- Real device lifecycle fire (442-08 drill).

#### Commit message

```
feat(442-03): optimistic toggle with rollback + WS-driven real-time sync

FlagToggleClient wires PUT to Phase 436 endpoint with optimistic UI. Rolls back
on 4xx/5xx with typed error toasts (400/401/500/network). Admin-to-admin real-time
via new AdminFlagChange WS envelope — server-side fanout in admin_flag_fanout.rs
registered parallel to Phase 436's mobile fanout. Snapshot-then-drop lock pattern.

Covers: ADMIN-04, FLAG-01, FLAG-02, FLAG-03 (UI half)
Not tested: kill-switch (442-04), capability (442-05), audit panel (442-06),
agent lifecycle fire (442-08).
```

---

### 442-04-PLAN — Global kill-switch UI with confirmation dialog

**Goal:** Implement the `KillSwitchPanel` at the top of `/mobile/flags`, with a prominent red "Halt all drivers" button, a confirmation dialog requiring the operator to type `HALT ALL` exactly, and a 2s disable window on the confirm button. When active, all per-driver toggle rows render read-only with a banner explaining the override. Releasing the kill-switch restores interactivity. OQ-1 decision (inline vs. separate page) is resolved at the checkpoint in this plan.

**Covers:** FLAG-04 (kill-switch UI), ADMIN-04 (safety UX)

**Dependencies:** 442-03 (optimistic PUT infrastructure), Phase 436 (pause_all_drivers endpoint)

**Type:** `checkpoint:decision` at start (OQ-1 final call), then `auto` for implementation.

#### Decision checkpoint (first step)

Present Uday with:
- **Option A (default per UI-SPEC):** Inline red panel at top of `/mobile/flags` page.
- **Option B:** Dedicated `/mobile/kill-switch` page with nothing else on it (full-screen red, one button).

Uday picks. Implementation continues with the chosen option.

#### Tasks

1. `KillSwitchPanel.tsx`:
   - Props: `active: boolean`, `onToggle: (active: boolean) => void`, `lastChangedBy?: string`, `lastChangedAt?: string`.
   - Render:
     - Racing Red (#E10600) background, white text, warning icon (lucide or heroicons `ShieldAlert`).
     - Title: "Halt all drivers" (matches CLAUDE.md Brand font Enthocentric for headers).
     - Subtitle explaining: "Halts every driver on every device within 10s. Use for ToS incident response."
     - Current state: "ACTIVE — halted since {time} by {actor}" (red-outlined badge when active) OR "Inactive" (muted).
     - Button: "Halt all drivers" (when inactive) → opens confirm dialog. "Release kill-switch" (when active) → opens a DIFFERENT confirm dialog (less strict — no typed-HALT, just a confirm click, because releasing is the safer direction).

2. `KillSwitchConfirmDialog.tsx`:
   - Focus-trap + ESC closes (use existing 441 Dialog primitive if available; otherwise `@radix-ui/react-dialog` already likely installed).
   - Body: warning paragraph, affected devices list (Tab Plus + M07), a text input labeled "Type HALT ALL to confirm".
   - State machine:
     1. `typed !== "HALT ALL"` → confirm button disabled + greyed.
     2. `typed === "HALT ALL"` → start 2s timer; confirm button shows countdown ("Confirm in 2s... 1s...").
     3. Timer reaches 0 → confirm button enabled (red, "Halt now").
     4. Click confirm → `mobileFlags.putFlag("*", "pause_all_drivers", true)` via optimistic pattern from 442-03, with extra-loud success/error toasts.
     5. If user edits the input after typing correctly, reset to step 1 (typed check + timer restart).
   - Cancel button always enabled; ESC closes.
   - ARIA: `role="alertdialog"`, `aria-describedby` pointing to the warning paragraph.

3. Kill-switch-active state propagation:
   - `FlagToggleClient` reads `data.flags` and checks for `mobile.pause_all_drivers` === true.
   - When true:
     - `FlagList` receives `disabled={true}`.
     - Banner renders above flag list: "Kill-switch active — per-driver flags ignored until released".
     - Toggle clicks show a warning toast instead of PUT (guard added in 442-03 already — verify the branch is correct).

4. Release flow:
   - "Release kill-switch" button opens a lighter confirm dialog (no typed-HALT; single click confirm with a 1s timer).
   - Calls `putFlag("*", "pause_all_drivers", false)`.
   - On 200, per-driver toggles become interactive; server-side (Phase 436) fires `install()` on previously-running drivers automatically.

5. The `deviceId` portion of the URL for pause_all is `*` — Phase 436-02's PUT handler accepts `*` as the device_id for globals (per OQ-2 resolution). Verify at 442-03 integration time.

6. Server-side: no changes needed beyond 442-03's additions — Phase 436 already handles `pause_all_drivers` at the endpoint.

7. Unit tests (React Testing Library):
   - `KillSwitchConfirmDialog.test.tsx` covers all 5 state-machine steps.
   - Disabled confirm with wrong text.
   - Enabled confirm after 2s delay.
   - Reset on edit after typed-match.
   - ESC closes.
   - `KillSwitchPanel.test.tsx`:
     - Active state renders differently (red badge, release button).
     - onToggle called with `false` when release confirmed.

8. E2E (deferred to 442-08): physical toggle via dashboard → both agents halt within 10s.

#### Acceptance

- All unit tests pass.
- Manual smoke in dev: typing "HALT" enables nothing; typing "HALT ALL" starts 2s timer; timer countdown visible; click-after-timer fires PUT; server responds 200; kill-switch banner appears across all per-driver toggle rows.
- Escape-key + click-outside both close the dialog without firing PUT.
- Lighthouse accessibility audit on `/admin/mobile/flags` with kill-switch active reports no role/ARIA errors (run `npx lighthouse --only-categories=accessibility` or equivalent).
- `aria-disabled="true"` on per-driver toggles when kill-switch active (spot-check with React DevTools).

#### G4 NOT TESTED list

- Agent-side halt within 10s (442-08 drill).
- Concurrent kill-switch from two admins (covered by 442-03's WS mechanism in principle; verified in 442-08 drill).

#### Commit message

```
feat(442-04): global kill-switch UI with typed-HALT + 2s delay dialog

Racing-Red KillSwitchPanel at top of /mobile/flags. Confirm dialog requires
typed "HALT ALL" + 2s disable window before enabling confirm button. Release
flow uses lighter 1s confirm. When active, per-driver toggles render
aria-disabled with explanatory banner. OQ-1 resolved: <inline|separate> per
Uday decision.

Covers: FLAG-04, ADMIN-04 (safety UX)
Not tested: agent-side halt propagation (442-08 drill).
```

---

### 442-05-PLAN — Capability read-only view per device

**Goal:** `CapabilityView` renders each device's declared driver capabilities from the agent's registration payload (Phase 429). Read-only table. "Refresh" button calls a live proxy to `GET :8090/capability` for fresh data. Default data source is a server-cached snapshot per device (resolves OQ-2 recommendation).

**Covers:** CAPREG-02 (capability viewable in admin), ADMIN-04 (capability view)

**Dependencies:** 442-02 (page scaffold), Phase 429 (capability payload shape), Phase 432 (driver registry that populates capabilities)

**Type:** `auto`

#### Tasks

1. Server-side: add capability snapshot storage.
   - Option A: In-memory `state.device_capabilities: Arc<RwLock<HashMap<String, DeviceCapability>>>` populated by the Phase 429 registration WS handler — cheap, loses data on restart but acceptable because agents re-register and re-populate within 30s of racecontrol boot.
   - Option B: Persist to `device_capabilities` DB table. Adds durability for the case where the dashboard loads before any agent has re-registered.
   - **Recommendation: Option A** for v1. If dashboard-boot-before-agent-register is found painful in operation, flip to B in a future phase. Document the tradeoff in UI-SPEC.
   - Add endpoint `GET /api/v1/mobile/devices/:device_id/capability` (staff JWT gated) that reads from the in-memory cache. Returns 404 if device never registered, 200 with `{capabilities: [...], supported_device_types: [...], last_seen: <iso>}` if known.
   - Add endpoint `POST /api/v1/mobile/devices/:device_id/capability/refresh` (staff JWT gated) that proxies to the device's `:8090/capability` endpoint via the rc-agent-mobile HTTP server (Phase 429 exposes this endpoint). Returns the live response AND updates the in-memory cache.

2. Client-side: add `apps/admin/lib/hooks/useCapability.ts`:
   - SWR key `/mobile/devices/${deviceId}/capability`.
   - Revalidate on focus + 60s interval.
   - `refresh()` method calls the POST refresh endpoint then invalidates SWR.

3. `CapabilityView.tsx`:
   - Renders a table of `driver_id | supported_device_types | status (enabled per flag?)`.
   - "Refresh" button in top-right. On click: triggers `refresh()`, button shows spinner for up to 5s, toast on success/failure.
   - "Last synced {IST timestamp}" label below the table.
   - Empty state: "Device has not registered yet. Check agent status."
   - Status column cross-references current flags: if `mobile.enable_<driver>_on_<device>=true`, show green dot; else grey.

4. Unit tests:
   - `CapabilityView.test.tsx` with mock SWR data — table renders all drivers.
   - Refresh button spinner state.
   - Empty state when data === null.
   - Status dot reflects flag state.

5. Integration test (stubbed at unit level until 442-08): mock `GET /api/v1/mobile/devices/rcm-tab-plus/capability` returns `{capabilities: [{driver_id: "zomato"}, {driver_id: "hyperpure"}], supported_device_types: ["tablet"], last_seen: "2026-04-18T14:00:00Z"}` → expect 2 rows.

#### Acceptance

- `GET /api/v1/mobile/devices/:device_id/capability` returns 200 with cached snapshot; 404 when unknown.
- `POST /api/v1/mobile/devices/:device_id/capability/refresh` proxies to device and updates cache. Tested with mocked rc-agent-mobile HTTP server.
- Unit tests pass.
- Manual smoke: dev server + mocked agent returns capability; "Refresh" updates "last synced" timestamp.
- Dashboard renders table with brand-matched styling.

#### G4 NOT TESTED list

- Real-device capability fetch (442-08 drill).
- CapabilityUpdate WS event (Phase 429-04 defined but not implemented in 429-05 scope — would land in a later phase; for now capability only updates via refresh click).

#### Commit message

```
feat(442-05): capability read-only view + server-side capability snapshot cache

In-memory state.device_capabilities populated by Phase 429 registration handler.
New endpoints: GET /mobile/devices/:id/capability + POST .../refresh (proxies
to device :8090/capability). CapabilityView.tsx renders brand-styled table with
manual refresh; status dot reflects active flag state.

Covers: CAPREG-02, ADMIN-04 (capability view)
Not tested: real-device refresh (442-08 drill).
```

---

### 442-06-PLAN — Audit trail side panel

**Goal:** `AuditPanel` on the right of `/mobile/flags` renders the most-recent 50 `mobile_flag_audit` rows with actor email, IST timestamp, flag_key, before/after. "Show more" button loads older entries. Virtualized to handle 1000+ rows without lag. Subscribes to `AdminFlagChange` WS so new rows appear in real-time.

**Covers:** FLAG-02 (audit surfaced), ADMIN-04 (audit in page)

**Dependencies:** 442-03 (WS + optimistic pattern), Phase 436-07 (mobile_flag_audit table + optional activity feed endpoint)

**Type:** `auto`

#### Tasks

1. Server-side: add `GET /api/v1/mobile/flag-audit` endpoint in `crates/racecontrol/src/flags_mobile.rs`.
   - Query params: `device_id` (optional filter), `limit` (default 50, max 200), `before_id` (pagination cursor).
   - Returns `{rows: [{id, actor, target_device, target_driver, flag_key, old_value, new_value, version, created_at}], has_more: bool}`.
   - Staff JWT gated (same as all other mobile endpoints).
   - SQL: `SELECT * FROM mobile_flag_audit WHERE (?1 IS NULL OR target_device = ?1) AND (?2 IS NULL OR id < ?2) ORDER BY id DESC LIMIT ?3`.

2. Client-side: `apps/admin/lib/hooks/useFlagAudit.ts`:
   - SWR-infinite pattern for "show more".
   - Revalidate-on-focus + 30s interval.
   - Accepts `deviceId` filter parameter.

3. `AuditPanel.tsx`:
   - Virtualized list — use `react-window` or equivalent (if 441 already uses one, reuse it; else add `react-window`).
   - Row shape: `[timestamp IST] [actor email] [flag_key] [before → after]`.
   - Styling: monospace timestamps, muted actor, bright flag_key, arrow between before/after.
   - "Show more" button below the last row; fetches next page via SWR infinite.
   - WS integration: on `AdminFlagChange` event, `mutate()` the first page so the new row prepends.
   - Empty state: "No flag changes in the last 30 days."

4. Activity feed integration: Phase 436-07's plan surfaces mobile_flag_audit rows via the existing admin activity feed endpoint. This plan renders a PAGE-LOCAL panel for page-local context; the existing feed stays unchanged. The UI-SPEC (442-01) should confirm this split.

5. Unit tests:
   - `AuditPanel.test.tsx` with mock rows — renders 50 rows; "Show more" triggers fetch.
   - Empty state.
   - WS event prepends a row.
   - IST timestamp formatting (CLAUDE.md rule — ALL timestamps in IST).

6. Server-side tests:
   - `flag_audit_endpoint_tests::returns_most_recent_50_by_default()`.
   - `flag_audit_endpoint_tests::honors_device_filter()`.
   - `flag_audit_endpoint_tests::paginates_before_id()`.
   - `flag_audit_endpoint_tests::rejects_no_jwt()`.

#### Acceptance

- All tests pass.
- Manual smoke: perform 3 flag toggles in quick succession → audit panel shows 3 rows in reverse-chronological order with correct actor + times.
- "Show more" loads older rows (seed 100 rows, verify pagination).
- WS integration: open two tabs; toggle in tab A → audit panel in tab B updates within 2s without reload.
- IST timestamps (not UTC).

#### G4 NOT TESTED list

- Real-device toggles (442-08 drill).

#### Commit message

```
feat(442-06): audit trail side panel with virtualized list + WS real-time

New GET /api/v1/mobile/flag-audit endpoint (staff JWT, paginated by id cursor).
AuditPanel renders last 50 rows, "show more" loads older entries, react-window
virtualization. Timestamps formatted IST per CLAUDE.md. AdminFlagChange WS
event prepends new rows without reload.

Covers: FLAG-02, ADMIN-04 (audit in page)
Not tested: real-device toggle→audit loop (442-08 drill).
```

---

### 442-07-PLAN — UI-REVIEW via gsd-ui-auditor (6-pillar)

**Goal:** Run `gsd-ui-auditor` against the shipped `/mobile/flags` page to produce `.planning/phases/442-feature-flag-capability-ui/UI-REVIEW.md`. This is a MANDATORY post-execution gate per CLAUDE.md Subagent Gates table ("Any frontend … gsd-ui-auditor … UI-REVIEW.md … After execution, before ship"). Any BLOCKER findings must be fixed before 442-08 drill runs.

**Covers:** All of 442's UI surface (audit, not net-new code)

**Dependencies:** 442-02, 442-03, 442-04, 442-05, 442-06 all merged

**Type:** `checkpoint:human-verify` — Uday reviews UI-REVIEW.md and signs off before 442-08 begins.

#### Tasks

1. Invoke `gsd-ui-auditor` with:
   ```
   Phase: 442 — Feature Flag + Capability UI.
   Target: /admin/mobile/flags on Next.js admin dashboard.
   Source UI-SPEC.md: .planning/phases/442-feature-flag-capability-ui/UI-SPEC.md
   Audit against the 6 pillars:
     1. Layout & hierarchy (kill-switch prominence; toggle row density; audit panel placement)
     2. Accessibility (ARIA on kill-switch dialog; keyboard nav; focus trap; contrast ratios in Racing Red panel)
     3. Error handling (4xx/5xx toasts; WS-disconnect banner; empty states; network-failure UX)
     4. State completeness (loading; empty; error; kill-switch-active; concurrent-admin-edit; optimistic-pending)
     5. Feedback loops (toast duration; optimistic rollback visibility; "Show more" pagination; "Refresh" button spinner)
     6. Resilience (WS drop recovery; basePath correctness; JWT expiry mid-edit; rapid double-click protection)

   Compare shipped artefacts against UI-SPEC.md line by line. Flag any deltas.
   Produce UI-REVIEW.md with severity-tagged findings: BLOCKER | HIGH | MEDIUM | LOW.
   BLOCKER + HIGH must be fixed before 442-08 drill.
   MEDIUM + LOW go into the 442 SUMMARY.md backlog for future hardening.
   ```

2. Run any automated audits in parallel:
   - Lighthouse accessibility on `/admin/mobile/flags` + open-dialog state.
   - axe-core via `@axe-core/playwright` for the test spec skeleton (442-08).
   - React DevTools profiler for re-render count during rapid toggle clicks.

3. Checkpoint: present UI-REVIEW.md to Uday. Uday responds with:
   - "approved — proceed" → 442-08 runs.
   - "block: fix A, B, C" → create 442-07A plan with fix tasks, re-run auditor, re-submit.

#### Acceptance

- `.planning/phases/442-feature-flag-capability-ui/UI-REVIEW.md` exists, >= 200 lines.
- Findings table with severity tags for each pillar.
- 0 BLOCKER + 0 HIGH findings (fix cycle closed if any were found).
- Uday approval logged.

#### Checkpoint (human-verify)

Uday approves UI-REVIEW.md. Approval unlocks 442-08.

#### G4 NOT TESTED list

- Agent E2E (442-08 drill — the actual device-side verification).

#### Commit message

```
docs(442-07): UI-REVIEW.md for /admin/mobile/flags — 6-pillar audit

gsd-ui-auditor report: 0 BLOCKER, 0 HIGH findings after fix cycle.
Lighthouse accessibility: 100. axe-core violations: 0. SPEC-delta: none
outstanding.  Approved by Uday.

Covers: ADMIN-04, FLAG-01..04 (UI surface verified)
```

---

### 442-08-PLAN — Playwright E2E: toggle flag + verify agent lifecycle fires within 10s

**Goal:** End-to-end drill with real racecontrol, real admin dashboard, and real (or stubbed) rc-agent-mobile. Toggle a flag from the browser; assert the agent log shows the driver's `install()` hook fired within 10s. Toggle off; assert `uninstall()` fires within 10s. Engage kill-switch; assert BOTH devices halt within 10s. This is the ship gate for Phase 442.

**Covers:** Full phase — verification, not new implementation

**Dependencies:** 442-02 through 442-07 all complete

**Type:** `checkpoint:human-verify` (physical devices) at the end; automated parts run headless.

#### Preconditions

- Tab Plus + M07 both connected to comms-link (Phase 429 verified).
- At least one driver registered (stub is acceptable if Phase 432 not yet in a usable state — see OQ at bottom).
- Playwright installed in admin dashboard: `cd apps/admin && npm install --save-dev @playwright/test` (if not already).
- Admin login credentials available in environment (`ADMIN_TEST_EMAIL`, `ADMIN_TEST_PASSWORD`).

#### Test spec — `tests/e2e/mobile-flags-ui.spec.ts`

```ts
import { test, expect } from "@playwright/test";
import { execSync } from "child_process";

test.describe("Phase 442 — Feature Flag + Capability UI", () => {
  test.beforeEach(async ({ page }) => {
    await page.goto("/admin");
    await page.getByLabel("Email").fill(process.env.ADMIN_TEST_EMAIL!);
    await page.getByLabel("Password").fill(process.env.ADMIN_TEST_PASSWORD!);
    await page.getByRole("button", { name: "Sign in" }).click();
    await page.goto("/admin/mobile/flags");
  });

  test("toggle enable_zomato_on_rcm_tab_plus → agent install() fires within 10s", async ({ page }) => {
    await page.getByRole("combobox", { name: "Device" }).selectOption("rcm-tab-plus");
    const toggle = page.getByRole("switch", { name: /enable zomato/i });
    const t0 = Date.now();
    await toggle.click();

    // Poll adb logcat or agent :8090/logs/tail for the install event
    let installed = false;
    const deadline = t0 + 10_000;
    while (Date.now() < deadline) {
      const logs = execSync("adb -s <tab_plus_serial> logcat -d RcAgentMobile:I *:S", { encoding: "utf-8" });
      if (logs.includes("Driver install: zomato")) { installed = true; break; }
      await new Promise(r => setTimeout(r, 500));
    }
    expect(installed).toBe(true);
    const delta = Date.now() - t0;
    console.log(`install() fired in ${delta}ms`);
    expect(delta).toBeLessThanOrEqual(10_000);
  });

  test("toggle off → uninstall() fires within 10s", async ({ page }) => {
    // symmetric to above, asserting "Driver uninstall: zomato"
  });

  test("kill-switch → all drivers halt on both devices within 10s", async ({ page }) => {
    await page.getByRole("button", { name: "Halt all drivers" }).click();
    await page.getByLabel("Type HALT ALL to confirm").fill("HALT ALL");
    // Wait for 2s delay
    await page.waitForTimeout(2100);
    const t0 = Date.now();
    await page.getByRole("button", { name: /halt now/i }).click();

    // Poll both devices
    const deadline = t0 + 10_000;
    let tabHalted = false;
    let m07Halted = false;
    while (Date.now() < deadline && (!tabHalted || !m07Halted)) {
      const tabLogs = execSync("adb -s <tab_plus_serial> logcat -d RcAgentMobile:I *:S", { encoding: "utf-8" });
      const m07Logs = execSync("adb -s <m07_serial> logcat -d RcAgentMobile:I *:S", { encoding: "utf-8" });
      if (tabLogs.includes("KillSwitchGate: halting")) tabHalted = true;
      if (m07Logs.includes("KillSwitchGate: halting")) m07Halted = true;
      await new Promise(r => setTimeout(r, 500));
    }
    expect(tabHalted).toBe(true);
    expect(m07Halted).toBe(true);
  });

  test("audit panel shows row within 2s of toggle", async ({ page }) => {
    await page.getByRole("combobox", { name: "Device" }).selectOption("rcm-tab-plus");
    const auditPanel = page.getByTestId("audit-panel");
    const initialRowCount = await auditPanel.locator("[data-testid=audit-row]").count();
    await page.getByRole("switch", { name: /enable zomato/i }).click();
    await expect(auditPanel.locator("[data-testid=audit-row]")).toHaveCount(initialRowCount + 1, { timeout: 2000 });
  });

  test("concurrent admin: tab A toggles, tab B reflects within 2s", async ({ browser }) => {
    // open 2 contexts
  });

  test("WS drop → 'reconnecting' banner + SWR revalidation on reconnect", async ({ page }) => {
    // simulate relay restart
  });

  test("optimistic rollback on 400 validator error", async ({ page, context }) => {
    // intercept PUT, return 400
  });
});
```

#### Tasks

1. Write the Playwright spec covering 7 tests (skeleton above + expand).
2. Stand up test environment:
   - Start racecontrol on a test port (e.g. 18080) or use the live dev instance.
   - Ensure rc-agent-mobile on both Tab Plus + M07 is on current build (rebuild + `adb install -r` if needed).
   - Seed a mock driver (if Phase 432 has a sample driver, use it; otherwise add a `hello-world` stub driver for this test).

3. Run: `cd apps/admin && npx playwright test tests/e2e/mobile-flags-ui.spec.ts --headed` (headed for first run; headless on CI).

4. Record: timings for each success criterion (SC-1 toggle-to-install, SC-2 kill-switch, SC-3 audit lag). Commit the outputs to `.planning/phases/442-feature-flag-capability-ui/SUMMARY.md`.

5. Failure protocol: if any SC exceeds 10s or fails, do NOT mark phase complete. Create gap-closure plan (442-08A) per CLAUDE.md Backlog Gate.

6. Automated run must be reproducible with a single command documented in SUMMARY.md.

#### Acceptance (all SC must pass)

- [ ] SC-1: enable_zomato toggle → agent `install()` log within ≤ 10s.
- [ ] SC-2: disable toggle → agent `uninstall()` log within ≤ 10s.
- [ ] SC-3: kill-switch → both devices' KillSwitchGate fires within ≤ 10s.
- [ ] SC-4: audit panel shows row within ≤ 2s of toggle.
- [ ] SC-5: concurrent admin tab reflects change within ≤ 2s.
- [ ] SC-6: WS drop → reconnecting banner + SWR revalidates on reconnect.
- [ ] SC-7: server 400 error → UI rolls back within ≤ 3s with actionable toast.

#### Artifacts in SUMMARY.md

- Playwright HTML report (attached or linked).
- Stopwatch measurements for each SC.
- adb logcat excerpts from both devices (redacted of any PII).
- Screenshots of each UI state (kill-switch active, audit panel with real data, WS-disconnect banner).

#### Checkpoint (human-verify)

Uday confirms "All 7 SCs pass, phase 442 done" OR describes specific failures for gap closure.

#### Commit message

```
test(442-08): Phase 442 E2E drill — toggle + kill-switch + audit + real-time + resilience

All 7 success criteria exercised against real racecontrol + real rc-agent-mobile
on Tab Plus + M07.  Measurements:
  SC-1 toggle→install: <N>ms  SC-2 toggle→uninstall: <N>ms
  SC-3 kill-switch→halt: <N>ms (both devices)  SC-4 audit lag: <N>ms
  SC-5 concurrent-admin: <N>ms  SC-6 ws-reconnect: PASS
  SC-7 4xx rollback: PASS
Evidence in SUMMARY.md.

Covers: Phase 442 acceptance gate.
```

---

## 6. Risks and pitfalls

| # | Risk | Mitigation |
|---|------|------------|
| R-1 | **Accidental kill-switch click** catastrophic — halts all drivers | Three-layer guard (442-04): red prominent + warning copy, typed-HALT-ALL gate, 2s post-type delay. Unit tests cover all 5 state-machine steps. |
| R-2 | **Optimistic UI drift** if server rejects PUT | 442-03 rollback logic with typed error toasts (400/401/500/network). WS AdminFlagChange acts as a second-source-of-truth. |
| R-3 | **WS disconnect leaves UI stale** | SWR revalidates on focus + 30s interval + on-reconnect. Visible "reconnecting..." banner (reuse 441 component). |
| R-4 | **Concurrent admin edits** with last-write-wins at server | 442-03's WS listener ensures whichever write lands last appears in both admins' UIs within 2s. Documented in UI-SPEC as known semantics (OQ-4 of phase 436). |
| R-5 | **Kill-switch-active + per-driver edit attempt** confuses staff | When active, per-driver toggles render `aria-disabled` with banner "Kill-switch active — per-driver flags ignored until released". 442-04 handles this branch. |
| R-6 | **basePath double-prefix** in middleware redirects (CLAUDE.md rule) | 442-02 verifies next.config.ts basePath + middleware uses `"/"` not `/admin` for redirects. |
| R-7 | **JWT expiry mid-edit** surfaces as generic 401 | 442-03 branches on err.status === 401 with "Session expired — please re-login" toast. |
| R-8 | **IST vs UTC timestamp confusion** in audit panel | 442-06 formats all timestamps IST (CLAUDE.md IST rule). Unit test asserts format string. |
| R-9 | **Audit panel flood** from rapid toggles | Virtualization via react-window; server LIMIT 100; no rate-limit in v1 (JWT-gated staff-only). |
| R-10 | **Capability view drift** between dashboard snapshot and live device | Manual "Refresh" button + "Last synced" timestamp; in-memory cache is best-effort. |
| R-11 | **Lighthouse accessibility failures** on red-high-contrast panel | 442-04 design picks #E10600 on white text (already CLAUDE.md brand) — verified contrast ratio ≥ 4.5:1 in UI-SPEC. 442-07 auditor re-verifies. |
| R-12 | **Playwright flakiness** on 10s timing windows | 442-08 uses polling with 500ms intervals and fails fast rather than fixed sleep; CLAUDE.md "verify actual behavior" rule favors polling over arbitrary delays. |
| R-13 | **Cloud parity drift** (server .23 updated but Bono VPS not) | DMP `cloud_parity` list enforces both-environment deploy; 442-08 drill runs against venue but commit message includes cloud rebuild step (CLAUDE.md DEPLOY PARITY rule). |
| R-14 | **next.config.ts basePath mismatch** with tRPC or server proxy causes 404 on PUT | 442-02 + 442-03 verify `fetch("/api/v1/mobile/flags/...")` routes via next-config rewrite; confirm with curl against dev server before merge. |
| R-15 | **React double-render in StrictMode** fires PUT twice | 442-03 `pendingKeys` guard ensures in-flight toggles are no-op on re-click; unit test covers double-click. |

## 7. Test plan

### Unit tests (JVM = React Testing Library + Vitest/Jest + Rust cargo test)
- `FlagList.test.tsx` (442-02)
- `FlagToggleClient.test.tsx` — 8 behaviors (442-03)
- `FlagDeltaWs.test.ts` — WS event integration (442-03)
- `admin_flag_fanout_tests.rs` (442-03)
- `KillSwitchConfirmDialog.test.tsx` — typed gate, 2s delay, reset (442-04)
- `KillSwitchPanel.test.tsx` — active/inactive render (442-04)
- `CapabilityView.test.tsx` (442-05)
- `flag_audit_endpoint_tests.rs` (442-06)
- `AuditPanel.test.tsx` — virtualization, WS prepend, IST format (442-06)

### Integration / E2E (442-08)
- Playwright spec: 7 scenarios (SC-1..SC-7) against real racecontrol + real rc-agent-mobile.

### Accessibility
- Lighthouse on `/admin/mobile/flags` — score ≥ 95.
- axe-core via `@axe-core/playwright` — 0 violations.
- Keyboard-only navigation drill (UI-SPEC + UI-REVIEW gates).

### Cross-phase integration-checker
- `gsd-integration-checker` verifies 436↔441↔442 wiring pre-ship.

## 8. Verification gates (per CLAUDE.md)

- **gsd-ui-researcher (MANDATORY):** UI-SPEC.md via 442-01 before any code. Hard gate.
- **gsd-ui-auditor (MANDATORY):** UI-REVIEW.md via 442-07 after implementation, before 442-08 drill. Hard gate.
- **nyquist-audit (required):** Optimistic-rollback state machine, WS delta-version monotonicity (reuse 436's pattern), kill-switch priority in UI state, typed-HALT gate state machine. Run before 442-07.
- **MMA audit (required — cross-system bridge):** Admin browser ↔ racecontrol ↔ comms-link ↔ Kotlin agent ↔ driver lifecycle is a 5-layer bridge spanning 4 languages (TS, Rust, Node, Kotlin). CLAUDE.md requires dual reasoning modes (abstract for UI state machine correctness; trace-level for "what does useMobileFlags().data return during the 200ms optimistic window before server 200?"). Run before 442-07. Budget: $5.
- **integration-checker (required — multi-phase, cross-language):** 432 (driver lifecycle target) ↔ 436 (API + WS) ↔ 441 (admin shell) ↔ 442 (this). Run before milestone ship gate.
- **SEC gate:** `node comms-link/test/security-check.js` — verify new `/api/v1/mobile/flag-audit` and `/mobile/devices/:id/capability` + `.../refresh` routes are behind staff JWT (no accidental `public_routes` addition).
- **Frontend staleness check:** After 442-07 merge, confirm admin dashboard rebuild fires in CI + next deploy (Suite 5 `frontend-staleness-check.sh`).
- **Deploy Manifest Protocol (DMP):** Frontmatter `deploy:` section ticked by executor; verifier confirms admin rebuild on both .23 and Bono VPS.
- **Backlog gate (CLAUDE.md CGP v4.3):** Phase 442 must reach DEPLOYED-VERIFIED (admin dashboard live on both environments + 442-08 drill passed) before Phase 443 (selector-push UI) can begin.

## 9. Open questions the planner cannot decide

Flagged in frontmatter `open_questions` with recommendations. Repeated here for execution-blocking order:

- **OQ-1 (442-04 checkpoint):** Kill-switch inline vs. separate page. Recommendation: inline. Uday confirms at 442-04's decision checkpoint.
- **OQ-2 (442-05):** Capability source — snapshot cache vs. live fetch. Recommendation: in-memory cache + manual refresh. Confirm during UI-SPEC review (442-01).
- **OQ-3 (cross-phase):** Role stratification for mobile_admin. Recommendation: generic staff JWT for v1. Document future-proofing in UI-SPEC.
- **OQ-4 (442-03):** Admin WS channel reuse vs. new. Recommendation: reuse 441's admin socket with AdminFlagChange message type. Confirm by reading 441's PLAN before 442-03 task 4.
- **OQ-5 (442-06):** Actor display — email vs. masked. Recommendation: full email. Internal-only, small operator pool.

Additional questions surfacing during execution (to revisit):

- **OQ-6 (442-08):** If Phase 432 hasn't delivered a real driver at 442-08 drill time, we either (a) block on 432 completion, (b) use a stub driver that logs `Driver install: hello-world` for the SC-1 assertion. Recommendation: (b) with SUMMARY.md caveat; switch to real driver when 432 ships.
- **OQ-7:** Should releasing the kill-switch auto-restore previously-enabled per-driver flags, or leave them off and require staff to re-toggle? Phase 436 OQ-4 resolved server-side: "install() is re-invoked on drivers that were running before pause". UI surfaces this by just fetching the new flag state — no UI-specific action needed. Confirm in UI-SPEC.

## 10. Cross-references

- **Milestone:** v50.0 rc-agent-mobile (`.planning/ROADMAP-v50.md`)
- **Requirements:** `.planning/REQUIREMENTS-v50.md` (ADMIN-04, FLAG-01..04)
- **Spec source:** `~/.claude/projects/C--Users-bono/memory/project_v50_rc_agent_mobile.md`
- **Phase 436 PLAN:** `.planning/phases/436-feature-flag-system/PLAN.md`
- **Phase 441 PLAN:** `.planning/phases/441-admin-dashboard-reception-view/PLAN.md`
- **Phase 429 PROTOCOL:** `rc-agent-mobile/docs/PROTOCOL.md` (authoritative for envelope + capability payload)
- **Phase 432 PLAN:** `.planning/phases/432-driver-framework-capability-registry/PLAN.md` (DriverRegistry — downstream consumer of flag changes)
- **CLAUDE.md Subagent Gates:** Frontend sections (UI-SPEC mandatory before planning, UI-REVIEW mandatory after execution)
- **CLAUDE.md Brand Identity:** Racing Red `#E10600` for kill-switch; Asphalt Black `#1A1A1A` background; Montserrat body, Enthocentric headers
- **CLAUDE.md Next.js middleware rules:** basePath handling, login-page public status, optimistic-ui rollback pattern

## 11. Output (at phase close)

At the end of Plan 442-08 (E2E drill pass + Uday approval), create `.planning/phases/442-feature-flag-capability-ui/SUMMARY.md` capturing:

- Which commits implemented each plan (442-01 through 442-08)
- Actual stopwatch measurements for SC-1..SC-7
- Playwright HTML report (attached or linked)
- adb logcat excerpts from both devices
- UI-SPEC.md and UI-REVIEW.md approvals referenced
- Any risks encountered and how they were resolved
- Any open questions resolved during execution
- Deploy manifest checklist: every item from `deploy:` frontmatter ticked
- Cloud parity verified (Bono VPS admin dashboard rebuilt + racecontrol binary redeployed + pages served on both environments)
- Handoff to Phase 443 (Selector-map remote push UI) — what's ready, what's deferred

When SUMMARY.md is committed, amend `.planning/ROADMAP-v50.md` Phase 14 entry from `[ ]` to `[x]` in the same commit (per CLAUDE.md ROADMAP plan checkbox sync rule).
