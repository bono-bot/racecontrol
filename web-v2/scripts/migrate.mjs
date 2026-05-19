#!/usr/bin/env node
// migrate.mjs — racingpoint_v2 migration runner
//
// Reads schema_migrations ledger, lists ./migrations/NNNN_*.sql files,
// applies unapplied migrations in lex order via `sudo -u postgres psql`.
// Each migration file is responsible for its own schema_migrations INSERT
// (forward-only ledger pattern). Runner verifies the ledger advanced
// after each apply.
//
// Usage:
//   node scripts/migrate.mjs                  apply all unapplied
//   node scripts/migrate.mjs --dry-run        list what would apply, exit
//   node scripts/migrate.mjs --status         show ledger state, exit
//
// Env:
//   PG_USER  default 'postgres'
//   PG_DB    default 'racingpoint_v2'
//
// Captain auth context: V2 substrate work, Replit recommendations
// aligned with V2 development (Captain 2026-05-19 ~15:19 IST scope).

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const MIGRATIONS_DIR = join(__dirname, "..", "migrations");
const PG_USER = process.env.PG_USER ?? "postgres";
const PG_DB = process.env.PG_DB ?? "racingpoint_v2";

const args = new Set(process.argv.slice(2));
const dryRun = args.has("--dry-run");
const statusOnly = args.has("--status");

function psql(sql, opts = {}) {
  const cmdArgs = ["-u", PG_USER, "psql", "-d", PG_DB, "-tAq"];
  if (opts.stopOnError !== false) cmdArgs.push("-v", "ON_ERROR_STOP=1");
  if (opts.input) cmdArgs.push("-c", sql);
  return execFileSync("sudo", opts.input ? cmdArgs : cmdArgs.concat(["-c", sql]), {
    encoding: "utf-8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

function psqlFile(absPath) {
  // /root is mode 700 (root-only). Reading via shell redirect `< file` from
  // sudo'd postgres user fails. Read the file in the parent process and pipe
  // the SQL via stdin instead.
  const sql = readFileSync(absPath, "utf-8");
  return execFileSync(
    "sudo",
    ["-u", PG_USER, "psql", "-d", PG_DB, "-tAq", "-v", "ON_ERROR_STOP=1"],
    { input: sql, encoding: "utf-8", stdio: ["pipe", "pipe", "pipe"] }
  );
}

function listAppliedVersions() {
  const out = psql("SELECT version FROM schema_migrations ORDER BY version;");
  return out.split("\n").map((s) => s.trim()).filter(Boolean);
}

function listMigrationFiles() {
  try {
    statSync(MIGRATIONS_DIR);
  } catch {
    return [];
  }
  return readdirSync(MIGRATIONS_DIR)
    .filter((f) => /^\d{4}_.*\.sql$/.test(f))
    .sort();
}

function versionFromFilename(filename) {
  return filename.replace(/\.sql$/, "");
}

function main() {
  let applied;
  try {
    applied = listAppliedVersions();
  } catch (e) {
    console.error("ERROR: cannot read schema_migrations table.");
    console.error("Hint: ensure database exists + 0001 migration has been applied at least once.");
    console.error(`Underlying error: ${e.message}`);
    process.exit(2);
  }

  const files = listMigrationFiles();
  const appliedSet = new Set(applied);
  const pending = files.filter((f) => !appliedSet.has(versionFromFilename(f)));

  if (statusOnly) {
    console.log(`Database: ${PG_DB} (user: ${PG_USER})`);
    console.log(`Migrations directory: ${MIGRATIONS_DIR}`);
    console.log(`Applied versions (${applied.length}):`);
    for (const v of applied) console.log(`  ✅ ${v}`);
    console.log(`Pending files (${pending.length}):`);
    for (const f of pending) console.log(`  ⏳ ${f}`);
    process.exit(0);
  }

  if (pending.length === 0) {
    console.log(`schema is up to date (${applied.length} migrations applied).`);
    process.exit(0);
  }

  console.log(`Pending migrations: ${pending.length}`);
  for (const f of pending) console.log(`  ⏳ ${f}`);

  if (dryRun) {
    console.log("(dry-run — exiting without applying)");
    process.exit(0);
  }

  for (const filename of pending) {
    const version = versionFromFilename(filename);
    const path = join(MIGRATIONS_DIR, filename);
    console.log(`\n--- applying ${filename} ---`);
    try {
      const out = psqlFile(path);
      process.stdout.write(out);
    } catch (e) {
      console.error(`FAILED on ${filename}:`);
      console.error(e.stderr?.toString() ?? e.message);
      process.exit(3);
    }
    const reapplied = listAppliedVersions();
    if (!reapplied.includes(version)) {
      console.error(`ERROR: ${filename} ran but schema_migrations does not show '${version}'.`);
      console.error("       Each migration file must INSERT INTO schema_migrations(version) VALUES('<version>');");
      process.exit(4);
    }
    console.log(`✅ ${version} applied`);
  }

  console.log(`\nAll ${pending.length} migration(s) applied successfully.`);
}

main();
