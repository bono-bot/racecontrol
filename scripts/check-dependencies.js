#!/usr/bin/env node
'use strict';

/**
 * check-dependencies.js — Pre-commit impact detection script
 *
 * Reads DEPENDENCIES.json and checks staged files against known cross-process
 * dependency boundaries. Prints warnings when changes affect downstream services.
 *
 * Usage:
 *   node scripts/check-dependencies.js          # Normal mode (reads git staged files)
 *   node scripts/check-dependencies.js --test    # Test mode (uses mock staged files)
 *
 * Exit code is always 0 — this is advisory only, never blocks commits.
 */

const path = require('path');
const { execFileSync } = require('child_process');

// ANSI color codes
const YELLOW = '\x1b[33m';
const RED = '\x1b[31m';
const CYAN = '\x1b[36m';
const BOLD = '\x1b[1m';
const RESET = '\x1b[0m';

const isTestMode = process.argv.includes('--test');

// Load DEPENDENCIES.json
let deps;
try {
  deps = require(path.resolve(__dirname, '..', 'DEPENDENCIES.json'));
} catch (err) {
  console.error(`${RED}ERROR: Cannot load DEPENDENCIES.json: ${err.message}${RESET}`);
  process.exit(0);
}

// Get staged files
let stagedFiles;
if (isTestMode) {
  stagedFiles = [
    'crates/racecontrol/src/db/mod.rs',
    'crates/racecontrol/src/api/routes.rs',
    'crates/racecontrol/src/cloud_sync.rs'
  ];
  console.log(`${CYAN}[TEST MODE] Simulating staged files:${RESET}`);
  stagedFiles.forEach(f => console.log(`  ${f}`));
  console.log('');
} else {
  try {
    const output = execFileSync('git', ['diff', '--cached', '--name-only'], {
      encoding: 'utf-8',
      cwd: path.resolve(__dirname, '..')
    });
    stagedFiles = output.trim().split('\n').filter(Boolean);
  } catch (err) {
    // Not in a git repo or no staged files — exit silently
    process.exit(0);
  }
}

if (stagedFiles.length === 0) {
  process.exit(0);
}

const impacts = [];

// Normalize file paths for comparison (strip leading ./ or /)
function normalizePath(p) {
  return p.replace(/^\.\//, '').replace(/^\/root\/racecontrol\//, '');
}

// Check 1: Schema source changes (database schema modifications)
for (const [dbName, dbConfig] of Object.entries(deps.databases || {})) {
  const schemaSource = normalizePath(dbConfig.schema_source || '');
  if (!schemaSource) continue;

  for (const staged of stagedFiles) {
    const normalizedStaged = normalizePath(staged);
    if (normalizedStaged === schemaSource) {
      const readers = (dbConfig.readers || []).map(r => ({
        service: r.service,
        tables: r.tables || []
      }));
      if (readers.length > 0) {
        impacts.push({
          type: 'schema',
          file: staged,
          dbName,
          readers
        });
      }
    }
  }
}

// Check 2: API route source changes
for (const [apiName, apiConfig] of Object.entries(deps.api_contracts || {})) {
  const routeSource = normalizePath(apiConfig.route_source || '');
  if (!routeSource) continue;

  for (const staged of stagedFiles) {
    const normalizedStaged = normalizePath(staged);
    if (normalizedStaged === routeSource) {
      const consumers = (apiConfig.consumers || []).map(c => ({
        service: c.service,
        endpoints: c.endpoints || []
      }));
      if (consumers.length > 0) {
        impacts.push({
          type: 'api',
          file: staged,
          apiName,
          consumers
        });
      }
    }
  }
}

// Check 3: Sync boundary changes
for (const [direction, syncConfig] of Object.entries(deps.sync_boundary || {})) {
  const syncSource = normalizePath(syncConfig.sync_source || '');
  if (!syncSource) continue;

  for (const staged of stagedFiles) {
    const normalizedStaged = normalizePath(staged);
    if (normalizedStaged === syncSource) {
      impacts.push({
        type: 'sync',
        file: staged,
        direction,
        tables: syncConfig.tables_pulled || syncConfig.tables_pushed || []
      });
    }
  }
}

// Print results
if (impacts.length === 0) {
  process.exit(0);
}

// Collect all affected services for the restart command
const affectedServices = new Set();

console.log('');
console.log(`${BOLD}${YELLOW}${'='.repeat(50)}${RESET}`);
console.log(`${BOLD}${YELLOW}  CROSS-PROCESS IMPACT DETECTED${RESET}`);
console.log(`${BOLD}${YELLOW}${'='.repeat(50)}${RESET}`);
console.log('');

for (const impact of impacts) {
  if (impact.type === 'schema') {
    console.log(`${BOLD}Changed:${RESET} ${impact.file} (schema for ${impact.dbName})`);
    console.log(`${BOLD}Affected services:${RESET}`);
    for (const reader of impact.readers) {
      console.log(`  ${RED}-${RESET} ${BOLD}${reader.service}${RESET} (reads: ${reader.tables.join(', ')})`);
      affectedServices.add(reader.service);
    }
    console.log(`${BOLD}Action:${RESET} Verify these services still work after this schema change.`);
    console.log('');
  }

  if (impact.type === 'api') {
    console.log(`${BOLD}Changed:${RESET} ${impact.file} (API routes for ${impact.apiName})`);
    console.log(`${BOLD}Affected consumers:${RESET}`);
    for (const consumer of impact.consumers) {
      console.log(`  ${RED}-${RESET} ${BOLD}${consumer.service}${RESET} (uses: ${consumer.endpoints.join(', ')})`);
      affectedServices.add(consumer.service);
    }
    console.log(`${BOLD}Action:${RESET} Verify these consumers still work after this API change.`);
    console.log('');
  }

  if (impact.type === 'sync') {
    const dirLabel = impact.direction === 'cloud_to_venue' ? 'Cloud -> Venue' : 'Venue -> Cloud';
    console.log(`${BOLD}Changed:${RESET} ${impact.file} (sync boundary: ${dirLabel})`);
    console.log(`${BOLD}Tables synced:${RESET} ${impact.tables.join(', ')}`);
    console.log(`${RED}${BOLD}WARNING:${RESET} Cloud-venue sync boundary modified!`);
    console.log(`${BOLD}Action:${RESET} James must be notified — venue racecontrol must be rebuilt.`);
    console.log('');
  }
}

if (affectedServices.size > 0) {
  const serviceList = [...affectedServices].join(' ');
  console.log(`${CYAN}Run: bash scripts/restart-dependents.sh ${serviceList}${RESET}`);
}

console.log(`${BOLD}${YELLOW}${'='.repeat(50)}${RESET}`);
console.log('');

// Always exit 0 — impact detection is advisory only
process.exit(0);
