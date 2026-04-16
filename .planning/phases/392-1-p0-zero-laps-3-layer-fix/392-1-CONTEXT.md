---
phase: 392-1-p0-zero-laps-3-layer-fix
status: ready-for-execution
source: session_handoff_20260416_zero_laps_real_root_cause.md + ROADMAP.md:1926
gathered: 2026-04-16
---

# Phase 392.1 — Context

> **Source of truth:** `.planning/ROADMAP.md` lines 1926-1974 (INSERTED 2026-04-16).
> Plan-phase workflow was manually run — `gsd-tools roadmap get-phase` parser cannot
> read racecontrol's `### Phase N:` nested format (`phase_found: false` on all phases
> 1-393). All standard agents (researcher, planner, checker) skipped; scope is fully
> locked upstream in the handoff + roadmap entry — nothing new to research.

<domain>
## Phase Boundary

Restore lap recording end-to-end on per-minute billing sessions, bundled with the
C1 FK-PRAGMA source-code fix (racecontrol `d24b17f7`) because both require the same
racecontrol binary rebuild + server .23 + Bono VPS swap window.

Out-of-scope: Strategy-B launch_state_is_live tracking, server↔agent launch_id
protocol mismatch, old-build exit-grace fleet rollout (all filed separately
post-ship).
</domain>

<decisions>
## Locked decisions

**Root cause (DB-confirmed 2026-04-16 02:45 UTC, both environments):**
- `laps = 0`, `lap_rejections = 0` → events never reach the DB layer. This is NOT
  a rejection path. The billing session tears down before AC can emit a lap event.
- Per-minute tier `min_duration_secs ≈ 60s`. Fastest F2004 Spa lap ≈ 105s. Server
  sends normal StopGame at T+66s (Pod 8 jsonl 2026-04-16).
- Therefore: product-config, not rc-agent code. Not a billing-timer code bug.

**Plan split:**
- **Plan 01 (this phase, ship-now):** Layer 1 (`min_duration_secs` raise) +
  folded C1 FK-PRAGMA deploy. One rebuild, one swap window, one verify pass.
- **Plan 02 (deferred, follow-up):** Layer 2 kiosk UX warning — blocked on
  per-track × per-car reference-lap data source (unknown; see NOT TESTED).
- **Plan 03 (deferred, follow-up):** Layer 3 server grace window in
  `billing_timer_expiry.rs` — requires reference-lap data model + MMA audit
  (cross-system: billing ↔ wallet ↔ timer).

**Deploy parity — MANDATORY:**
- Targets: server .23 + Bono VPS (cloud). Pods are NOT affected (rc-agent
  unchanged). POS unaffected. Kiosk/web/admin frontends unaffected by Layer 1.
- Order: venue cleanup → venue swap → cloud cleanup → cloud swap (cloud second
  so venue's FK-enforced writes cannot push fresh orphans cloud-ward mid-window).

**Verification — authoritative target for the "fixed" claim:**
- A row in the `laps` table after a real 2-min Pod 8 per-minute session with
  Brands Hatch Indy + any road car, visible via authenticated
  `GET /api/v1/laps` from James .27 (NOT SSH curl on the server — that
  substitutes the target).
- `PRAGMA foreign_keys = 1` on BOTH server .23 and Bono VPS read from a SECOND
  pool connection (first may be cached).
- `billing_events` and `billing_sessions` orphan counts = 0 on both environments.
</decisions>

<canonical_refs>
## Canonical references (MUST read before execution)

- `.planning/ROADMAP.md` lines 1926-1974 — phase body (authoritative scope)
- `~/.claude/projects/C--Users-bono/memory/session_handoff_20260416_zero_laps_real_root_cause.md` — full diagnosis, DB probe outputs, Pod 8 jsonl evidence
- `~/.claude/projects/C--Users-bono/memory/project_zero_laps_investigation.md` — full incident history
- `~/.claude/projects/C--Users-bono/memory/decision_c1_fk_policies_locked.md` — C1 ON DELETE policies (RESTRICT on pricing_tiers, CASCADE on billing_session_id)
- `~/.claude/projects/C--Users-bono/memory/session_handoff_20260416_c1_fk_phase1_sweep.md` — 29-orphan sweep evidence (20 billing_sessions + 9 billing_events, identical on server + cloud)
- `docs/ARCHITECTURE.md` §22 — Deploy Manifest Protocol
- Rollback snapshots (already captured 2026-04-16 ~04:18 UTC):
  - Venue: `C:/RacingPoint/backups/racecontrol-pre-c1-20260416.db` (176,910,336 B)
  - Cloud: `/root/racecontrol/backups/racecontrol-pre-c1-20260416.db` (172,019,712 B)
</canonical_refs>

<deferred>
## Deferred / NOT TESTED (carry into execution)

- Other FK-declaring tables (`billing_rates`, `wallet_transactions`, `drivers`,
  `laps`, `sessions`, `pricing_tiers`) — not swept for orphans. Post-swap
  `PRAGMA foreign_key_check` will surface any we missed.
- Live UPDATE code paths touching rows in the deleted-orphan set — sqlx will now
  return FK errors where it previously swallowed them silently. No audit yet.
- Per-track × per-car fastest-lap reference data source — unknown. Blocks Plans
  02 and 03.
- Layer 1 store location (`pricing_rules` table vs `billing_config.toml`) — plan
  01 resolves this as its first step.
</deferred>
