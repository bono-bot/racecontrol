#!/usr/bin/env node
/**
 * check-schema-compat.js
 * Verify that all reader services can still query the tables they depend on.
 *
 * Reads DEPENDENCIES.json and for each database:
 * 1. Opens the DB file in readonly mode via better-sqlite3
 * 2. Gets actual table list via sqlite_master
 * 3. Checks every reader's expected tables exist
 *
 * Exit 0 = all OK, Exit 1 = errors found
 */

const path = require('path');
const Database = require('better-sqlite3');

const DEPS_PATH = path.resolve(__dirname, '..', 'DEPENDENCIES.json');

let deps;
try {
  deps = require(DEPS_PATH);
} catch (err) {
  console.error('ERROR: Cannot load DEPENDENCIES.json:', err.message);
  process.exit(1);
}

const errors = [];
const warnings = [];
let dbsChecked = 0;
let tablesChecked = 0;

for (const [dbName, db] of Object.entries(deps.databases)) {
  // Skip databases with no readers (nothing to validate)
  if (!db.readers || db.readers.length === 0) {
    continue;
  }

  let dbConn;
  try {
    dbConn = new Database(db.path, { readonly: true, fileMustExist: true });
  } catch (e) {
    // If DB file does not exist (e.g., venue-only), skip with a warning
    if (e.code === 'SQLITE_CANTOPEN' || e.message.includes('not exist') || e.message.includes('CANTOPEN') || e.message.includes('no such file')) {
      warnings.push(`SKIP: ${dbName} — file not found at ${db.path} (may be venue-only)`);
      continue;
    }
    errors.push(`Cannot open ${dbName} at ${db.path}: ${e.message}`);
    continue;
  }

  dbsChecked++;

  const actualTables = dbConn.prepare(
    "SELECT name FROM sqlite_master WHERE type='table'"
  ).all().map(r => r.name);

  for (const reader of db.readers) {
    for (const table of reader.tables) {
      tablesChecked++;
      if (!actualTables.includes(table)) {
        errors.push(
          `${reader.service} expects table "${table}" in ${dbName} but it does not exist. ` +
          `Available tables: ${actualTables.join(', ')}`
        );
      }
    }
  }

  dbConn.close();
}

// Output results
const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const RESET = '\x1b[0m';

if (warnings.length > 0) {
  for (const w of warnings) {
    console.log(`${YELLOW}WARN${RESET}: ${w}`);
  }
}

console.log(`Checked ${dbsChecked} databases, ${tablesChecked} table references`);

if (errors.length > 0) {
  console.error(`\n${RED}SCHEMA COMPATIBILITY ERRORS:${RESET}`);
  for (const e of errors) {
    console.error(`  ${RED}FAIL${RESET}: ${e}`);
  }
  process.exit(1);
} else {
  console.log(`${GREEN}PASS${RESET}: Schema compatibility check passed`);
  process.exit(0);
}
