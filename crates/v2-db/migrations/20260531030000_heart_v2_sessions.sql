-- L3-1 (2026-05-31): heart-V2 session restart-persistence.
-- A durable store so an in-flight pod session survives a heart (process) restart.
-- Previously HeartStore.sessions was in-memory only => a restart orphaned every
-- live session (pod display + billing proxy lose their session reference).
--
-- V2-ISOLATED: lives in the v2db pool, NO foreign key into any V1 table
-- (heart-V2 RCA §1 V1/V2 isolation). Stores the full PodSession as a JSON blob
-- (exact serde round-trip on rehydrate) + indexed columns: `state` for
-- query/observability, `updated_at` (monotonic ms epoch) for last-writer-wins
-- conflict resolution on the write-through UPSERT (MMA Step-1 consensus,
-- 2026-05-31: deepseek/qwen/nvidia).
--
-- Ended sessions are RETAINED (not pruned by age) so a re-delivered end/pause
-- after a restart stays idempotent (200, not 404) — MMA nvidia/deepseek catch.

CREATE TABLE IF NOT EXISTS heart_v2_sessions (
    id          TEXT PRIMARY KEY NOT NULL,
    pod_id      TEXT NOT NULL,
    state       TEXT NOT NULL,
    updated_at  INTEGER NOT NULL DEFAULT 0,
    data        TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_heart_v2_sessions_pod ON heart_v2_sessions(pod_id);
