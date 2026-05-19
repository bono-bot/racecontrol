#!/usr/bin/env node
// substrate-status.mjs — V2 substrate health probe (read-only)
//
// One-command snapshot of the V2 substrate state across:
//   - Postgres: connectivity, racingpoint_v2 DB, pgcrypto extension,
//     migration ledger, per-table row + column + constraint count
//   - Redis: ping + SETEX/GET roundtrip on a transient key
//
// Composes with `node scripts/migrate.mjs --status` (lower-level migration
// ledger detail) by widening the surface to substrate-as-a-whole.
//
// Usage:
//   node scripts/substrate-status.mjs              human-readable
//   node scripts/substrate-status.mjs --json       machine-readable single-line JSON
//
// Env:
//   PG_USER          default 'postgres'
//   PG_DB            default 'racingpoint_v2'
//   REDIS_HOST       default '127.0.0.1'
//   REDIS_PORT       default 6379
//
// Captain auth scope: V2 substrate operational tooling under bono-owned
// web-v2/. Same class as migrate.mjs. Per Apply-Recommendations-Autonomously
// standing rule + "Proceed with Replit recommendations aligned with V2
// development. Proceed autonomously" Captain verb (2026-05-19 IST).

import { execFileSync } from "node:child_process";

const PG_USER = process.env.PG_USER ?? "postgres";
const PG_DB = process.env.PG_DB ?? "racingpoint_v2";
const REDIS_HOST = process.env.REDIS_HOST ?? "127.0.0.1";
const REDIS_PORT = process.env.REDIS_PORT ?? "6379";

const args = new Set(process.argv.slice(2));
const jsonMode = args.has("--json");

// Expected schema topography (post-0001 + 0002).
// New migrations add rows here; assertion: every expected table exists.
const EXPECTED_TABLES = [
  "audit_log",
  "households",
  "idempotency_keys",
  "schema_migrations",
  "staff",
  "wallet_balances",
  "wallet_transactions",
];

function psql(sql) {
  return execFileSync(
    "sudo",
    ["-u", PG_USER, "psql", "-d", PG_DB, "-tAq", "-v", "ON_ERROR_STOP=1", "-c", sql],
    { encoding: "utf-8", stdio: ["ignore", "pipe", "pipe"] },
  ).trim();
}

function redisCli(...cliArgs) {
  return execFileSync(
    "redis-cli",
    ["-h", REDIS_HOST, "-p", REDIS_PORT, ...cliArgs],
    { encoding: "utf-8", stdio: ["ignore", "pipe", "pipe"] },
  ).trim();
}

function safeCall(label, fn) {
  try {
    return { ok: true, value: fn() };
  } catch (e) {
    return { ok: false, error: e instanceof Error ? e.message.split("\n")[0] : String(e), label };
  }
}

function probePostgres() {
  const dbExists = safeCall("db_exists", () => psql("SELECT current_database();"));
  const pgcrypto = safeCall("pgcrypto", () =>
    psql("SELECT extname FROM pg_extension WHERE extname = 'pgcrypto';"),
  );
  const migrations = safeCall("migrations", () =>
    psql("SELECT version FROM schema_migrations ORDER BY version;").split("\n").filter(Boolean),
  );

  const tablesRaw = safeCall("tables", () =>
    psql("SELECT tablename FROM pg_tables WHERE schemaname='public' ORDER BY tablename;")
      .split("\n")
      .filter(Boolean),
  );

  const tables = {};
  const missing = [];
  if (tablesRaw.ok) {
    for (const t of EXPECTED_TABLES) {
      if (!tablesRaw.value.includes(t)) {
        missing.push(t);
        tables[t] = { present: false };
        continue;
      }
      const rows = safeCall(`rows.${t}`, () => Number(psql(`SELECT COUNT(*) FROM ${t};`)));
      const cols = safeCall(`cols.${t}`, () =>
        Number(
          psql(
            `SELECT COUNT(*) FROM information_schema.columns WHERE table_schema='public' AND table_name='${t}';`,
          ),
        ),
      );
      const constraints = safeCall(`constraints.${t}`, () =>
        Number(
          psql(
            `SELECT COUNT(*) FROM information_schema.table_constraints WHERE table_schema='public' AND table_name='${t}';`,
          ),
        ),
      );
      tables[t] = {
        present: true,
        rows: rows.ok ? rows.value : null,
        columns: cols.ok ? cols.value : null,
        constraints: constraints.ok ? constraints.value : null,
      };
    }
  }

  // Unexpected tables (present but not in EXPECTED_TABLES) — useful drift signal
  const unexpected = tablesRaw.ok
    ? tablesRaw.value.filter((t) => !EXPECTED_TABLES.includes(t))
    : [];

  return {
    reachable: dbExists.ok,
    database: dbExists.ok ? dbExists.value : null,
    pgcrypto_active: pgcrypto.ok && pgcrypto.value === "pgcrypto",
    migrations_applied: migrations.ok ? migrations.value : [],
    tables,
    missing_tables: missing,
    unexpected_tables: unexpected,
    error: dbExists.ok ? null : dbExists.error,
  };
}

function probeRedis() {
  const ping = safeCall("ping", () => redisCli("PING"));
  if (!ping.ok) {
    return { reachable: false, ping: null, roundtrip_ok: false, error: ping.error };
  }

  const probeKey = `substrate_status_probe_${Date.now()}_${Math.floor(Math.random() * 1e6)}`;
  const probeValue = `pid${process.pid}_t${Date.now()}`;

  const setexResult = safeCall("setex", () => redisCli("SETEX", probeKey, "10", probeValue));
  const getResult = safeCall("get", () => redisCli("GET", probeKey));
  const cleanup = safeCall("del", () => redisCli("DEL", probeKey));

  const roundtrip_ok = setexResult.ok && getResult.ok && getResult.value === probeValue;

  return {
    reachable: ping.value === "PONG",
    ping: ping.value,
    roundtrip_ok,
    probe_key: probeKey,
    cleanup_ok: cleanup.ok,
    error: roundtrip_ok ? null : `roundtrip-fail: setex=${setexResult.ok} get=${getResult.ok}`,
  };
}

function main() {
  const ts = new Date().toISOString();
  const pg = probePostgres();
  const redis = probeRedis();

  const ok =
    pg.reachable &&
    pg.pgcrypto_active &&
    pg.missing_tables.length === 0 &&
    redis.reachable &&
    redis.roundtrip_ok;

  const report = {
    ts,
    ok,
    postgres: pg,
    redis,
  };

  if (jsonMode) {
    process.stdout.write(JSON.stringify(report) + "\n");
    process.exit(ok ? 0 : 1);
  }

  // Human-readable
  const banner = ok ? "OK" : "DEGRADED";
  console.log(`=== V2 substrate status [${banner}] ${ts} ===`);
  console.log("");
  console.log("Postgres:");
  console.log(`  reachable        ${pg.reachable}`);
  console.log(`  database         ${pg.database ?? "n/a"}`);
  console.log(`  pgcrypto         ${pg.pgcrypto_active ? "active" : "MISSING"}`);
  console.log(`  migrations       ${pg.migrations_applied.length} applied`);
  for (const v of pg.migrations_applied) console.log(`    - ${v}`);
  if (pg.missing_tables.length > 0) {
    console.log(`  MISSING TABLES   ${pg.missing_tables.join(", ")}`);
  }
  if (pg.unexpected_tables.length > 0) {
    console.log(`  unexpected tables (informational): ${pg.unexpected_tables.join(", ")}`);
  }
  console.log("  tables:");
  for (const [name, info] of Object.entries(pg.tables)) {
    if (!info.present) {
      console.log(`    ${name.padEnd(22)} MISSING`);
      continue;
    }
    const rows = info.rows ?? "?";
    const cols = info.columns ?? "?";
    const cons = info.constraints ?? "?";
    console.log(
      `    ${name.padEnd(22)} rows=${String(rows).padStart(4)}  cols=${cols}  constraints=${cons}`,
    );
  }
  console.log("");
  console.log("Redis:");
  console.log(`  reachable        ${redis.reachable}`);
  console.log(`  ping             ${redis.ping ?? "n/a"}`);
  console.log(`  roundtrip_ok     ${redis.roundtrip_ok}`);
  if (redis.cleanup_ok === false) console.log(`  WARN: probe key cleanup failed`);
  if (redis.error) console.log(`  error            ${redis.error}`);
  console.log("");
  console.log(`Overall: ${banner}`);

  process.exit(ok ? 0 : 1);
}

main();
