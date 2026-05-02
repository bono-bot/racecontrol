# F7 — Pod Resilience (Step 2.5 Resilience Foundation)

**Status:** SPEC-SKELETON — implementation gated on Captain explicit Step 2.5 implementation-execute verb
**Created:** 2026-05-02 (composite-ratify-event #2 substrate landing)
**Owner:** james-LEAD (per PACT-070 first-mover; bono AMPLIFIER eligible)
**Sub-sequence position:** **3rd** (F9 → F8 → F7 → F12) — F7 ships after F8 closes Kiosk acute failure; F7 closes Pods truth source
**Ratifies:** PACT-20260502-001 quartet F7+F8+F9+F12 + CONSTRAINT-017 ACTIVE
**Substrate-anchor:** comms-link `7d86032` (composite-ratify-event #2 minimal substrate)

---

## Goal

Single source of truth for pod status surface in Admin Pods UI. Eliminate V1 silent-degrade plague where Admin Pods status derives from local rc-agent polling (which can be stale, lock-flapped, or WiFi-flapped).

---

## Contract

**Binding:** CONSTRAINT-017 — *"Pod status surfaces in Admin Pods MUST derive from F7 heartbeat data, not local rc-agent state polling."*

**Heartbeat shape:**
- Each rc-agent (Pods 1-8) emits heartbeat to server every 5s carrying: `pod_number`, `ws_connected_uptime_secs`, `http_reachable`, `build_id`, `version`, `agent_uptime_secs`, `active_session_id?`, `active_game?`, `last_lap_ts?`, `state_flags` (lock_screen / blanked / launching / running / etc.), `health_score` (0-100).
- Server stores in `pod_heartbeats` table (TTL 7d for trend; latest-row indexed for hot-path).
- Admin Pods UI fetches `GET /api/v1/admin/pods` which JOINs `pods` (registry) ⊕ `pod_heartbeats` (latest-per-pod) — never queries rc-agent directly.

**Authoritative-source flip:**
- Pre-F7: Admin Pods polls rc-agent `:8090/health` directly (per-pod HTTP probe; lossy on WiFi-flap).
- Post-F7: Admin Pods reads from server-side `pod_heartbeats` aggregate (lossy on heartbeat-drop only; server signals stale-heartbeat as `last_seen > 30s ago`).

**Stale-detection:** heartbeat-record older than 30s = pod-status flips to `STALE` in Admin UI (distinct from `OFFLINE` which requires ≥3 missed heartbeats = ~15s window).

---

## Composes-with

- **CONSTRAINT-017** (this contract; binding-text in PACT-CHARTER §V2.0)
- **F9 Atomic Deploy** (F7 server module deploys via F9; rc-agent heartbeat-emitter ships in F7-bundled rc-agent build)
- **F8 Kiosk Session Persistence** (F8 ships **before** F7; F7 builds on F8 server-side state pattern)
- **F1 Connection Hub / F2 Feature Flag Service** (V2 substrate; F7 endpoints route through F1; F2 gates rollout)
- **P2 Event ledger** (heartbeat events optionally emit to P2 when ledger lands; pre-P2 dedicated table)
- **Admin Pods UI** (`web/src/app/v2/admin/pods/*`) — F7 client integration replaces direct-rc-agent fetch
- **Existing fleet/health endpoint** (`/api/v1/fleet/health`) — F7 can extend or replace; v1 plan: F7 is sibling endpoint, fleet/health stays for backward-compat through V2.1

---

## Failure modes closed

- **WiFi-flap silent-degrade**: V1 Admin Pods shows `online` when WiFi flapped because rc-agent /health intermittently responds. F7 heartbeat shows `STALE` immediately on first missed beat (5s window).
- **Lock-flap masked**: V1 lock screen broken on pod but Admin Pods shows `online` (rc-agent process running, /health 200 OK, but Edge dead). F7 heartbeat carries `state_flags.lock_screen=broken`.
- **Build-drift undetected**: V1 admin sees pods online but doesn't enumerate build_id parity until manual probe. F7 heartbeat surfaces `build_id` per pod in single Admin view.

---

## Out of scope (F7 v1)

- F7 heartbeat replaces ALL fleet/health usage (V2.1+; v1 keeps /fleet/health as-is for backward compat)
- Auto-restart on F7-detected `STALE` (operational follow-up; F7 v1 is observability-only)
- Predictive failure detection from heartbeat trend (composes with Phase 366 Fleet Intelligence; out of scope here)
- F7 emit from non-pod targets (server, POS, comms-link) — V2.1+

---

## Implementation gating

**Phase 1 (this commit):** spec-shape only. CONSTRAINT-017 binding-text ACTIVE. No code change in `crates/` or `web/v2/admin/pods/`.

**Phase 2 (gated on F8 ship + Captain Step-2.5-implement verb):**
- Server: `racecontrol/crates/racecontrol/src/pod_heartbeat.rs` — POST `/api/v1/pods/heartbeat` endpoint + GET `/api/v1/admin/pods` aggregate
- Schema: `pod_heartbeats` table (idempotent migration)
- rc-agent: `racecontrol/crates/rc-agent/src/heartbeat_emitter.rs` — 5s interval emitter
- Web: `web/src/app/v2/admin/pods/page.tsx` — fetch from `/api/v1/admin/pods` (replace direct fleet/health)
- HALO probe `pod-status-source-coverage` (verifies all Admin-Pods data fetches route through F7)

**Phase 3 (gated on F7 v1 7-day soak PASS across 8/8 pods):**
- CONSTRAINT-017 enforcement flips from honor-system → HALO-driven alert if Admin-Pods code reads from non-F7 source

---

## NOT TESTED (post-spec-shape)

- F7 server endpoints (Phase 2 implementation)
- rc-agent heartbeat emitter under WiFi-flap conditions (Phase 2 implementation)
- 30s STALE detection vs 15s OFFLINE detection thresholds (empirical tuning needed)
- Admin Pods UI integration (Phase 2 implementation)
- Compose with F8 (sequencing post-F8-ship)
- Heartbeat write throughput at 8-pod × 5s = ~96 writes/min sustained

---

## Stale-at

Durable until F7 Phase 2 implementation lands OR scope re-shape via sibling-PACT.
