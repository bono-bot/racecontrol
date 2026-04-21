// Cleanup TEST_ONLY drivers from racecontrol DB
//
// SOURCE OF TRUTH: crates/racecontrol/src/api/customer_legal.rs
// This script shells out to tests/e2e/api/dpdp_erase_list.py to read the
// authoritative ERASE_TABLES + POINTER_TABLES lists. Any new FK-referencing
// table added to customer_legal.rs is automatically handled here without
// editing this file — eliminates the pre-2026-04-21 drift (this script
// hardcoded 21 tables while customer_legal.rs had 42).
//
// Usage: node cleanup-test-drivers.mjs [db_path]
// Requires Node 22.5+ (node:sqlite)

import { DatabaseSync } from 'node:sqlite';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const DB_PATH = process.argv[2] || 'C:\\RacingPoint\\data\\racecontrol.db';
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PARSER = path.join(__dirname, 'api', 'dpdp_erase_list.py');

// Pull authoritative lists from customer_legal.rs via the parser.
// Fails loudly if the parser is missing or returns malformed JSON — we
// prefer a cleanup abort over a silent partial cleanup.
let sot;
try {
  const raw = execFileSync('python3', [PARSER, '--json'], { encoding: 'utf8' });
  sot = JSON.parse(raw);
} catch (err) {
  console.error(`FATAL: could not read SoT via ${PARSER}`);
  console.error(err.message);
  process.exit(2);
}

const eraseTables = sot.erase;      // [{table, fk_columns: [...]}, ...] — 42 rows as of 2026-04-21
const pointerTables = sot.pointer;  // [{table, column}, ...] — 3 rows
console.log(`SoT: ${eraseTables.length} erase / ${pointerTables.length} pointer tables`);

const db = new DatabaseSync(DB_PATH);

// Find TEST_ONLY drivers.
const victims = db.prepare("SELECT id, name FROM drivers WHERE name LIKE 'TEST_ONLY%'").all();
console.log(`Found ${victims.length} TEST_ONLY drivers to clean up:`);
for (const v of victims) console.log(`  ${v.id} — ${v.name}`);

if (victims.length === 0) {
  console.log('Nothing to clean up.');
  process.exit(0);
}

const ids = victims.map(v => v.id);
const placeholders = ids.map(() => '?').join(',');

// Disable FK checks for cleanup (re-enabled after COMMIT).
// This lets us delete in any order without worrying about transient FK
// violations. Transitive-erase tables (billing_events, telemetry_samples,
// split_sessions) don't need explicit handling here — with FK off their
// orphans are harmless; with FK on the parent-first delete sequence in
// ERASE_TABLES wouldn't save them anyway since they FK to billing_sessions
// or laps, not drivers.
db.exec('PRAGMA foreign_keys = OFF');
db.exec('BEGIN');
let totalTouched = 0;

// ERASE_TABLES: hard-delete rows where any fk_column matches.
// Handles single-column (driver_id) and multi-column (friend_requests has
// sender_id + receiver_id; tournament_matches has driver_a + driver_b +
// winner_id) via the same code path.
for (const { table, fk_columns } of eraseTables) {
  try {
    const conds = fk_columns.map(c => `${c} IN (${placeholders})`).join(' OR ');
    const args = [];
    for (let i = 0; i < fk_columns.length; i++) args.push(...ids);
    const r = db.prepare(`DELETE FROM ${table} WHERE ${conds}`).run(...args);
    if (r.changes > 0) {
      console.log(`  ${table}: ${r.changes} rows deleted`);
      totalTouched += r.changes;
    }
  } catch (err) {
    // Table may not exist on older DB snapshots (personal_bests_v2 etc.).
    // Silent skip — it was a no-op in the previous script too.
  }
}

// POINTER_TABLES: null out the column instead of deleting the row
// (preserves the non-PII row per DPDP §12 "point at nothing" policy).
for (const { table, column } of pointerTables) {
  try {
    const r = db.prepare(`UPDATE ${table} SET ${column} = NULL WHERE ${column} IN (${placeholders})`).run(...ids);
    if (r.changes > 0) {
      console.log(`  ${table}.${column}: ${r.changes} rows nulled`);
      totalTouched += r.changes;
    }
  } catch (err) {
    // Table missing — skip silently.
  }
}

// Finally delete the driver records themselves.
const driverResult = db.prepare("DELETE FROM drivers WHERE name LIKE 'TEST_ONLY%'").run();
console.log(`  drivers: ${driverResult.changes} rows deleted`);
totalTouched += driverResult.changes;
db.exec('COMMIT');
db.exec('PRAGMA foreign_keys = ON');

console.log(`\nCleanup complete: ${totalTouched} total rows touched.`);

// Verify.
const remaining = db.prepare("SELECT count(*) as c FROM drivers WHERE name LIKE 'TEST_ONLY%'").get();
console.log(`Remaining TEST_ONLY drivers: ${remaining.c}`);
