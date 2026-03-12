#!/usr/bin/env node
/**
 * xsync-validate.js — Central validation runner for cross-process sync safety.
 *
 * Flags:
 *   --quick          Run only schema compatibility check (fast, < 5s)
 *   --full           Run all checks (schema compat + dep-map + pre-deploy + cross-process)
 *   --dep-map        Validate DEPENDENCIES.json structure
 *   --pre-deploy     Verify all referenced paths exist on disk
 *   --restart-order  Validate PM2 restart order is topologically sorted
 *   --cross-process  Schema compat + health endpoint checks
 *
 * Exit code = number of failures (0 = all pass)
 */

const path = require('path');
const fs = require('fs');
const { execFileSync } = require('child_process');

// ANSI colors
const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const YELLOW = '\x1b[33m';
const BOLD = '\x1b[1m';
const RESET = '\x1b[0m';

const DEPS_PATH = path.resolve(__dirname, '..', 'DEPENDENCIES.json');
const SCHEMA_SCRIPT = path.resolve(__dirname, 'check-schema-compat.js');

// Parse flags
const args = process.argv.slice(2);
const flags = {
  quick: args.includes('--quick'),
  full: args.includes('--full'),
  depMap: args.includes('--dep-map'),
  preDeploy: args.includes('--pre-deploy'),
  restartOrder: args.includes('--restart-order'),
  crossProcess: args.includes('--cross-process'),
};

// If no flags, show usage
if (!Object.values(flags).some(Boolean)) {
  console.log(`${BOLD}xsync-validate.js${RESET} — Cross-process sync validation runner\n`);
  console.log('Usage: node scripts/xsync-validate.js [FLAGS]\n');
  console.log('Flags:');
  console.log('  --quick          Schema compatibility check only (fast)');
  console.log('  --full           All checks combined');
  console.log('  --dep-map        Validate DEPENDENCIES.json structure');
  console.log('  --pre-deploy     Verify referenced files exist on disk');
  console.log('  --restart-order  Validate PM2 restart order topology');
  console.log('  --cross-process  Schema compat + service health checks');
  process.exit(0);
}

let failures = 0;
let passes = 0;

function pass(label) {
  passes++;
  console.log(`  ${GREEN}PASS${RESET}: ${label}`);
}

function fail(label, detail) {
  failures++;
  console.error(`  ${RED}FAIL${RESET}: ${label}${detail ? ' — ' + detail : ''}`);
}

function warn(label) {
  console.log(`  ${YELLOW}WARN${RESET}: ${label}`);
}

function section(title) {
  console.log(`\n${BOLD}[${title}]${RESET}`);
}

// ─── Load DEPENDENCIES.json ──────────────────────────────────────────────
let deps;
try {
  deps = JSON.parse(fs.readFileSync(DEPS_PATH, 'utf-8'));
} catch (err) {
  console.error(`${RED}FATAL${RESET}: Cannot load DEPENDENCIES.json: ${err.message}`);
  process.exit(1);
}

// ─── --dep-map: Validate structure ───────────────────────────────────────
if (flags.depMap || flags.full) {
  section('Dependency Map Validation');

  // Required top-level keys
  const requiredKeys = ['databases', 'api_contracts', 'sync_boundary', 'pm2_services', 'pm2_restart_order'];
  for (const key of requiredKeys) {
    if (deps[key]) {
      pass(`Top-level key "${key}" present`);
    } else {
      fail(`Missing top-level key "${key}"`);
    }
  }

  // Validate databases section
  if (deps.databases) {
    const dbCount = Object.keys(deps.databases).length;
    if (dbCount >= 5) {
      pass(`${dbCount} databases defined (expected >= 5)`);
    } else {
      fail(`Only ${dbCount} databases defined (expected >= 5)`);
    }

    for (const [dbName, db] of Object.entries(deps.databases)) {
      if (!db.path) fail(`${dbName}: missing "path"`);
      if (!db.schema_source) fail(`${dbName}: missing "schema_source"`);
      if (!db.owner) fail(`${dbName}: missing "owner"`);
      if (!Array.isArray(db.readers)) fail(`${dbName}: missing or invalid "readers" array`);
      else {
        for (const reader of db.readers) {
          if (!reader.service) fail(`${dbName}: reader missing "service"`);
          if (!reader.repo_path) fail(`${dbName}: reader "${reader.service}" missing "repo_path"`);
          if (!Array.isArray(reader.tables) || reader.tables.length === 0) {
            fail(`${dbName}: reader "${reader.service}" has empty tables array`);
          }
          if (!reader.mode) fail(`${dbName}: reader "${reader.service}" missing "mode"`);
        }
        if (db.readers.length >= 0) {
          pass(`${dbName}: ${db.readers.length} reader(s) with valid structure`);
        }
      }
    }
  }

  // Validate api_contracts
  if (deps.api_contracts) {
    for (const [apiName, api] of Object.entries(deps.api_contracts)) {
      if (!api.base_url) fail(`api_contracts.${apiName}: missing "base_url"`);
      if (!Array.isArray(api.consumers) || api.consumers.length === 0) {
        fail(`api_contracts.${apiName}: empty consumers array`);
      } else {
        pass(`api_contracts.${apiName}: ${api.consumers.length} consumers defined`);
      }
    }
  }

  // Validate sync_boundary
  if (deps.sync_boundary) {
    const pull = deps.sync_boundary.cloud_to_venue;
    const push = deps.sync_boundary.venue_to_cloud;
    if (pull && Array.isArray(pull.tables_pulled) && pull.tables_pulled.length > 0) {
      pass(`sync_boundary: ${pull.tables_pulled.length} tables pulled (cloud -> venue)`);
    } else {
      fail('sync_boundary: missing or empty cloud_to_venue.tables_pulled');
    }
    if (push && Array.isArray(push.tables_pushed) && push.tables_pushed.length > 0) {
      pass(`sync_boundary: ${push.tables_pushed.length} tables pushed (venue -> cloud)`);
    } else {
      fail('sync_boundary: missing or empty venue_to_cloud.tables_pushed');
    }
  }

  // Validate pm2_services
  if (deps.pm2_services) {
    const svcCount = Object.keys(deps.pm2_services).length;
    if (svcCount >= 5) {
      pass(`${svcCount} PM2 services defined`);
    } else {
      fail(`Only ${svcCount} PM2 services (expected >= 5)`);
    }
    for (const [svcName, svc] of Object.entries(deps.pm2_services)) {
      if (typeof svc.tier !== 'number') fail(`pm2_services.${svcName}: missing "tier"`);
    }
  }

  // Validate pm2_restart_order
  if (deps.pm2_restart_order) {
    if (Array.isArray(deps.pm2_restart_order) && deps.pm2_restart_order.length > 0) {
      pass(`pm2_restart_order: ${deps.pm2_restart_order.length} tiers defined`);
    } else {
      fail('pm2_restart_order: empty or not an array');
    }
  }
}

// ─── --pre-deploy: Verify referenced paths exist ─────────────────────────
if (flags.preDeploy || flags.full) {
  section('Pre-Deploy Path Verification');

  // Check database paths
  for (const [dbName, db] of Object.entries(deps.databases)) {
    if (fs.existsSync(db.path)) {
      pass(`DB file exists: ${db.path}`);
    } else {
      warn(`DB file not found: ${db.path} (${dbName} — may be venue-only)`);
    }

    // Check access_files for each reader
    for (const reader of (db.readers || [])) {
      for (const accessFile of (reader.access_files || [])) {
        const fullPath = path.join(reader.repo_path, accessFile);
        if (fs.existsSync(fullPath)) {
          pass(`Access file exists: ${accessFile} (${reader.service})`);
        } else {
          fail(`Access file missing: ${fullPath} (${reader.service} reads ${dbName})`);
        }
      }
    }
  }

  // Check api_contract source files
  for (const [apiName, api] of Object.entries(deps.api_contracts)) {
    for (const consumer of (api.consumers || [])) {
      if (consumer.proxy_file) {
        // Find the repo_path for this service from databases
        let repoPath = null;
        for (const db of Object.values(deps.databases)) {
          for (const r of (db.readers || [])) {
            if (r.service === consumer.service) {
              repoPath = r.repo_path;
              break;
            }
          }
          if (repoPath) break;
        }
        if (repoPath) {
          const fullPath = path.join(repoPath, consumer.proxy_file);
          if (fs.existsSync(fullPath)) {
            pass(`Proxy file exists: ${consumer.proxy_file} (${consumer.service})`);
          } else {
            fail(`Proxy file missing: ${fullPath} (${consumer.service} -> ${apiName})`);
          }
        }
      }
      if (consumer.client_file) {
        // Check client files relative to known repo paths
        const knownPaths = {
          'racingpoint-bot': '/root/racingpoint-whatsapp-bot',
          'racecontrol-pwa': '/root/racecontrol',
        };
        const repoPath = knownPaths[consumer.service];
        if (repoPath) {
          const fullPath = path.join(repoPath, consumer.client_file);
          if (fs.existsSync(fullPath)) {
            pass(`Client file exists: ${consumer.client_file} (${consumer.service})`);
          } else {
            fail(`Client file missing: ${fullPath} (${consumer.service} -> ${apiName})`);
          }
        }
      }
    }
  }
}

// ─── --restart-order: Validate topology ──────────────────────────────────
if (flags.restartOrder || flags.full) {
  section('PM2 Restart Order Validation');

  if (deps.pm2_restart_order && deps.pm2_services) {
    const order = deps.pm2_restart_order;
    const services = deps.pm2_services;

    // Check ascending tier order
    let prevMaxTier = -1;
    let orderValid = true;

    for (let i = 0; i < order.length; i++) {
      const tierServices = order[i];
      if (!Array.isArray(tierServices) || tierServices.length === 0) {
        fail(`Tier ${i}: empty or not an array`);
        orderValid = false;
        continue;
      }

      // All services in this tier should have the same tier number
      const tiers = tierServices.map(s => services[s]?.tier).filter(t => t !== undefined);
      const uniqueTiers = [...new Set(tiers)];

      if (uniqueTiers.length === 0) {
        fail(`Tier ${i}: no matching services found in pm2_services`);
        orderValid = false;
        continue;
      }

      if (uniqueTiers.length > 1) {
        fail(`Tier ${i}: mixed tier numbers ${uniqueTiers.join(',')} — services should be same tier`);
        orderValid = false;
        continue;
      }

      const tierNum = uniqueTiers[0];
      if (tierNum <= prevMaxTier) {
        fail(`Tier ${i} (tier=${tierNum}): not ascending from previous (tier=${prevMaxTier})`);
        orderValid = false;
      }

      prevMaxTier = tierNum;
    }

    if (orderValid) {
      pass(`Restart order is topologically sorted (${order.length} tiers)`);
    }

    // Check that all services in pm2_services appear in restart_order
    const orderedServices = new Set(order.flat());
    for (const svcName of Object.keys(services)) {
      if (orderedServices.has(svcName)) {
        pass(`Service "${svcName}" included in restart order`);
      } else {
        fail(`Service "${svcName}" missing from pm2_restart_order`);
      }
    }
  } else {
    fail('Missing pm2_restart_order or pm2_services');
  }
}

// ─── --quick: Schema compat only ─────────────────────────────────────────
if (flags.quick || flags.full || flags.crossProcess) {
  section('Schema Compatibility Check');

  try {
    execFileSync('node', [SCHEMA_SCRIPT], {
      stdio: 'inherit',
      timeout: 30000,
    });
    pass('check-schema-compat.js passed');
  } catch (err) {
    fail('check-schema-compat.js failed', `exit code ${err.status}`);
  }
}

// ─── --cross-process: Schema compat + health checks ──────────────────────
if (flags.crossProcess || flags.full) {
  section('Service Health Checks');

  for (const [svcName, svc] of Object.entries(deps.pm2_services)) {
    if (!svc.health || !svc.port) continue;

    const url = `http://localhost:${svc.port}${svc.health}`;
    try {
      execFileSync('curl', ['-sf', '--max-time', '5', url], {
        stdio: 'pipe',
        timeout: 10000,
      });
      pass(`${svcName} health OK (${url})`);
    } catch (err) {
      fail(`${svcName} health FAILED`, url);
    }
  }
}

// ─── Summary ─────────────────────────────────────────────────────────────
console.log(`\n${BOLD}Summary:${RESET} ${GREEN}${passes} passed${RESET}, ${failures > 0 ? RED : ''}${failures} failed${RESET}`);
process.exit(failures);
