// Cleanup TEST_ONLY drivers from racecontrol DB
// Usage: node cleanup-test-drivers.mjs [db_path]
// Requires Node 22.5+ (node:sqlite)

import { DatabaseSync } from 'node:sqlite';

const DB_PATH = process.argv[2] || 'C:\\RacingPoint\\data\\racecontrol.db';
const db = new DatabaseSync(DB_PATH);

// Show what we're about to delete
const victims = db.prepare("SELECT id, name FROM drivers WHERE name LIKE 'TEST_ONLY%'").all();
console.log(`Found ${victims.length} TEST_ONLY drivers to clean up:`);
for (const v of victims) console.log(`  ${v.id} — ${v.name}`);

if (victims.length === 0) {
  console.log('Nothing to clean up.');
  process.exit(0);
}

const ids = victims.map(v => v.id);
const placeholders = ids.map(() => '?').join(',');

// All FK-referencing tables (from DPDP customer_data_delete handler in routes.rs)
const tables = [
  'wallet_transactions', 'wallets', 'billing_sessions', 'laps',
  'customer_sessions', 'auth_tokens', 'personal_bests', 'memberships',
  'friend_requests', 'friendships', 'group_session_members',
  'tournament_registrations', 'pod_reservations', 'event_entries',
  'session_feedback', 'coupon_redemptions', 'referrals',
  'session_highlights', 'review_nudges', 'multiplayer_results',
  'driver_ratings',
];

// Tables with non-standard FK column names
const dualFkTables = [
  { table: 'friend_requests', cols: ['sender_id', 'receiver_id'] },
  { table: 'friendships', cols: ['driver_a_id', 'driver_b_id'] },
  { table: 'referrals', cols: ['referrer_id', 'referee_id'] },
];

// Disable FK checks for cleanup (re-enabled after COMMIT)
db.exec('PRAGMA foreign_keys = OFF');
db.exec('BEGIN');
let totalDeleted = 0;

// Handle dual-FK tables first (sender_id/receiver_id etc.)
for (const { table, cols } of dualFkTables) {
  try {
    const conds = cols.map(c => `${c} IN (${placeholders})`).join(' OR ');
    const allIds = [];
    for (let i = 0; i < cols.length; i++) allIds.push(...ids);
    const r = db.prepare(`DELETE FROM ${table} WHERE ${conds}`).run(...allIds);
    if (r.changes > 0) {
      console.log(`  ${table}: ${r.changes} rows deleted`);
      totalDeleted += r.changes;
    }
  } catch {
    // Table may not exist — skip
  }
}

// Handle standard driver_id FK tables
for (const table of tables) {
  // Skip dual-FK tables already handled
  if (dualFkTables.some(d => d.table === table)) continue;
  try {
    const r = db.prepare(`DELETE FROM ${table} WHERE driver_id IN (${placeholders})`).run(...ids);
    if (r.changes > 0) {
      console.log(`  ${table}: ${r.changes} rows deleted`);
      totalDeleted += r.changes;
    }
  } catch {
    // Table may not exist — skip
  }
}

// Finally delete the driver records
const driverResult = db.prepare("DELETE FROM drivers WHERE name LIKE 'TEST_ONLY%'").run();
console.log(`  drivers: ${driverResult.changes} rows deleted`);
totalDeleted += driverResult.changes;
db.exec('COMMIT');
db.exec('PRAGMA foreign_keys = ON');

console.log(`\nCleanup complete: ${totalDeleted} total rows deleted across ${tables.length + 1} tables.`);

// Verify
const remaining = db.prepare("SELECT count(*) as c FROM drivers WHERE name LIKE 'TEST_ONLY%'").get();
console.log(`Remaining TEST_ONLY drivers: ${remaining.c}`);
