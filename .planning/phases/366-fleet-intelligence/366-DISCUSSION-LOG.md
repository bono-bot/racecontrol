# Phase 366: Fleet Intelligence - Discussion Log

**Session:** 2026-04-10 (autonomous --auto mode)
**Operator:** James (user asleep -- all decisions made by Claude per --auto protocol)

## Gray Areas Identified and Auto-Resolved

### 1. Health Score Storage Strategy

**Q:** Should the composite health score be pre-materialized (background job + new table)
or computed on-query from existing `billing_sessions` data?

**[auto]** Selected: **Compute-on-query** (recommended default)

Rationale: 8-pod fleet means 8 indexed queries per `/fleet/intelligence` call --
sub-100ms expected. Avoids a staleness window and background job complexity. TSDB
`METRIC_POD_HEALTH_SCORE` already exists for historical trending -- upgrade it to
emit the composite value every 30s instead of binary 0/1.

Alternatives rejected:
- Pre-materialized in new table: staleness window, requires background job with watchdog
- Redis/in-memory cache only: no historical trending, loses on restart

---

### 2. MI audit_known_issues Reuse vs New Table

**Q:** Should Phase 366 write fleet intelligence data into the existing
`audit_known_issues` table (MI Tier 0) or create new tables?

**[auto]** Selected: **New `content_drift_events` table; no write to `audit_known_issues`**

Rationale: `audit_known_issues` is for symptom-to-fix knowledge base (MI operational
patterns, gossip protocol consumers). Content drift events are append-only audit log
records with different lifecycle (staff marks resolved, not MI auto-resolved). Writing
drift events to `audit_known_issues` would pollute the MI KB with operational noise.
Health scores are not stored at all (compute-on-query) -- no table needed.

---

### 3. Real-Time vs Batch Processing

**Q:** Which components should be real-time and which should be batch?

**[auto]** Selected: **Mixed approach** (recommended for each component based on usage)

- Health score: compute-on-query (real-time per API call) + TSDB emit every 30s (trending)
- Time-of-day analysis: compute-on-query within `/fleet/intelligence` (infrequent, on-demand)
- Content drift: batch poll every 60 minutes (drift is slow-changing; real-time is wasteful)
- Concurrent session guard: synchronous in-memory on every session start (must be hot-path)

---

### 4. Concurrent Session Guard Implementation

**Q:** Where to put the guard -- new middleware, billing.rs check, or DB constraint?

**[auto]** Selected: **In-memory check at `start_billing_session()` in billing.rs**

Rationale: `active_timers` map is already the canonical source of truth for active
billing (used by lockdown guard). O(1) lookup. No DB TOCTOU race. Consistent with
existing lockdown guard pattern at line 1237. No new infrastructure.

Alternatives rejected:
- DB UNIQUE constraint: requires a DB write-read-write cycle; race window exists
- New middleware layer: adds abstraction without value for an 8-pod fleet

---

### 5. Content Drift Ground Truth

**Q:** TOML vs TOML (mutation detection) or TOML vs live disk (physical drift)?

**[auto]** Selected: **Researcher must verify rc-agent capability**

Context: Phase 361 built TOML-reader inventory at `/api/v1/pods/{id}/inventory`.
Phase 362 built SharedMemory verification for launch-time config. If rc-agent exposes
a live disk content-list endpoint, prefer TOML vs live disk. If not, fall back to
TOML change detection (TOML vs previous snapshot). Researcher to check
`rc-agent/src/` for any content-enumeration endpoints.

---

## Auto-Selected Component Weights

| Component | Weight | Rationale |
|-----------|--------|-----------|
| Session success rate | 40 pts | Primary quality signal; directly maps to customer experience |
| Telemetry completeness | 30 pts | Second-most impactful; determines billing accuracy |
| Config mismatch rate | 20 pts | Important but less frequent; Phase 362 data source |
| Crash rate | 10 pts | Already in FleetHealthStore; complements the session-based view |

---

## Deferred Ideas

None surfaced during auto-discussion.

## Next Step

Run `/gsd:plan-phase 366 --auto`
