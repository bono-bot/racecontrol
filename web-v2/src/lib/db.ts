/*
 * db.ts — singleton pg Pool for V2 audit substrate (racingpoint_v2).
 *
 * Graceful degradation (B-3 wire-in):
 *   - PG_URL set   → real pg Pool against racingpoint_v2 as web_v2_audit role
 *   - PG_URL unset → getPool() returns null; callers fall back to dev-stub
 *     (f5.ts → console.log; queries.ts → mock data). Build + dev work without
 *     a DB credential; production flips to real Postgres when Captain sets PG_URL.
 *
 * Connection: postgresql://web_v2_audit:<pw>@127.0.0.1:5432/racingpoint_v2
 *   - web_v2_audit role has INSERT/SELECT on audit_log ONLY (append-only;
 *     UPDATE/DELETE/TRUNCATE revoked at grant level per §S-146 RCA W1)
 *   - Credential lives in .env.production.local (gitignored) — NEVER source
 *
 * Pool bounds (NEW-1 mitigation per RCA §6 threat model):
 *   - max=10 connections
 *   - statement_timeout=5s (no runaway queries)
 *   - connectionTimeoutMillis=5s (fail fast on unreachable DB)
 *   - idleTimeoutMillis=30s
 */

import { Pool } from "pg";

let pool: Pool | null = null;
let initialized = false;

export function getPool(): Pool | null {
  if (initialized) return pool;
  initialized = true;

  const url = process.env.PG_URL;
  if (!url) {
    pool = null;
    return null;
  }

  pool = new Pool({
    connectionString: url,
    max: 10,
    idleTimeoutMillis: 30_000,
    connectionTimeoutMillis: 5_000,
    statement_timeout: 5_000,
  });

  // Surface pool-level errors (idle client error) without crashing the process.
  pool.on("error", (err) => {
    console.error("[db] pg pool error:", err.message);
  });

  return pool;
}

export function isDbAvailable(): boolean {
  return Boolean(process.env.PG_URL);
}
