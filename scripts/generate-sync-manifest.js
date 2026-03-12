#!/usr/bin/env node
/**
 * generate-sync-manifest.js — Bono-James coordination manifest generator.
 *
 * Generates a structured JSON manifest describing what changed in a commit and
 * which services need to be rebuilt/restarted. Optionally sends the manifest
 * to James via the comms API.
 *
 * Usage:
 *   node scripts/generate-sync-manifest.js --summary "Fixed billing pause logic"
 *   node scripts/generate-sync-manifest.js --commit abc123 --send --summary "Schema migration"
 *   node scripts/generate-sync-manifest.js --validate
 *
 * Flags:
 *   --commit <hash>    Git commit to analyze (default: HEAD)
 *   --branch <name>    Branch name (default: current branch)
 *   --summary <text>   Human-readable summary (required for --send)
 *   --send             Send manifest to James via comms API
 *   --validate         Validate manifest structure without sending
 */

const path = require('path');
const fs = require('fs');
const http = require('http');
const { execFileSync } = require('child_process');

// ANSI colors
const GREEN = '\x1b[32m';
const RED = '\x1b[31m';
const RESET = '\x1b[0m';

const ROOT_DIR = path.resolve(__dirname, '..');
const DEPS_PATH = path.join(ROOT_DIR, 'DEPENDENCIES.json');

// ─── Parse arguments ─────────────────────────────────────────────────────
const args = process.argv.slice(2);

function getArg(name) {
  const idx = args.indexOf(name);
  if (idx === -1 || idx + 1 >= args.length) return null;
  return args[idx + 1];
}

const commitArg = getArg('--commit');
const branchArg = getArg('--branch');
const summaryArg = getArg('--summary');
const doSend = args.includes('--send');
const doValidate = args.includes('--validate');

// ─── Load DEPENDENCIES.json ──────────────────────────────────────────────
let deps;
try {
  deps = JSON.parse(fs.readFileSync(DEPS_PATH, 'utf-8'));
} catch (err) {
  console.error(`${RED}ERROR${RESET}: Cannot load DEPENDENCIES.json: ${err.message}`);
  process.exit(1);
}

// ─── Resolve git info (using execFileSync for safety — no shell injection) ─
function git(...gitArgs) {
  try {
    return execFileSync('git', gitArgs, { cwd: ROOT_DIR, encoding: 'utf-8' }).trim();
  } catch (err) {
    return '';
  }
}

const commitHash = commitArg || git('rev-parse', 'HEAD');
const branchName = branchArg || git('rev-parse', '--abbrev-ref', 'HEAD');

// ─── Get changed files ──────────────────────────────────────────────────
const changedFiles = git('diff-tree', '--no-commit-id', '--name-only', '-r', commitHash)
  .split('\n')
  .filter(Boolean);

// ─── Determine impact ───────────────────────────────────────────────────

// Build lookup maps from DEPENDENCIES.json
// Map schema_source -> { dbName, readers[] }
const schemaMap = {};
for (const [dbName, db] of Object.entries(deps.databases)) {
  if (db.schema_source) {
    schemaMap[db.schema_source] = {
      dbName,
      readers: (db.readers || []).map(r => r.service)
    };
  }
}

// Map route_source -> apiName
const routeMap = {};
for (const [apiName, api] of Object.entries(deps.api_contracts)) {
  if (api.route_source) {
    routeMap[api.route_source] = apiName;
  }
}

// Sync sources
const syncSources = [];
if (deps.sync_boundary) {
  if (deps.sync_boundary.cloud_to_venue && deps.sync_boundary.cloud_to_venue.sync_source) {
    syncSources.push(deps.sync_boundary.cloud_to_venue.sync_source);
  }
  if (deps.sync_boundary.venue_to_cloud && deps.sync_boundary.venue_to_cloud.sync_source) {
    syncSources.push(deps.sync_boundary.venue_to_cloud.sync_source);
  }
}

const schemaChanges = [];
const apiChanges = [];
const syncChanges = [];
const affectedServiceSet = new Set();

for (const file of changedFiles) {
  // Check schema sources
  if (schemaMap[file]) {
    const entry = schemaMap[file];
    schemaChanges.push({
      file,
      database: entry.dbName,
      affected_readers: entry.readers
    });
    entry.readers.forEach(s => affectedServiceSet.add(s));
  }

  // Check route sources
  if (routeMap[file]) {
    const apiName = routeMap[file];
    const api = deps.api_contracts[apiName];
    const consumers = (api.consumers || []).map(c => c.service);
    apiChanges.push({
      file,
      api: apiName,
      affected_consumers: consumers
    });
    consumers.forEach(s => affectedServiceSet.add(s));
  }

  // Check sync sources
  for (const syncSrc of syncSources) {
    if (file === syncSrc) {
      syncChanges.push({
        file,
        tables_pulled: deps.sync_boundary.cloud_to_venue.tables_pulled || [],
        tables_pushed: deps.sync_boundary.venue_to_cloud.tables_pushed || []
      });
      // Sync changes affect racecontrol itself (venue side)
      affectedServiceSet.add('racecontrol');
    }
  }

  // Any racecontrol crate change affects racecontrol service
  if (file.startsWith('crates/') || file === 'Cargo.toml' || file === 'Cargo.lock') {
    affectedServiceSet.add('racecontrol');
  }
}

// ─── Build affected services array ───────────────────────────────────────

// Build command map
const buildCommands = {
  'racecontrol': 'cargo build --release -p rc-core',
  'racingpoint-admin': 'npm run build',
  'racingpoint-dashboard': 'npm run build',
  'racecontrol-pwa': 'npm run build',
  'racingpoint-api-gateway': 'pm2 restart racingpoint-api-gateway',
  'racingpoint-bot': 'pm2 restart racingpoint-bot',
  'racingpoint-discord-bot': 'pm2 restart racingpoint-discord-bot',
  'racingpoint-hiring': 'pm2 restart racingpoint-hiring',
  'racingpoint-website': 'pm2 restart racingpoint-website',
  'racingpoint-website-api': 'pm2 restart racingpoint-website-api',
  'bono-failsafe': 'pm2 restart bono-failsafe',
};

// Repo map
const repoMap = {
  'racecontrol': 'racecontrol',
  'racingpoint-admin': 'racingpoint-admin',
  'racingpoint-dashboard': 'racingpoint-dashboard',
  'racecontrol-pwa': 'racecontrol',
  'racingpoint-api-gateway': 'racingpoint-api-gateway',
  'racingpoint-bot': 'racingpoint-whatsapp-bot',
  'racingpoint-discord-bot': 'racingpoint-discord-bot',
  'racingpoint-hiring': 'racingpoint-hiring-bot',
  'racingpoint-website': 'racingpoint-website',
  'racingpoint-website-api': 'racingpoint-website',
  'bono-failsafe': 'bono-failsafe',
};

const affectedServices = [...affectedServiceSet].map(service => {
  const isRacecontrol = (repoMap[service] === 'racecontrol');
  return {
    service,
    action: isRacecontrol ? 'pull_and_rebuild' : 'restart',
    repo: repoMap[service] || service,
    build_command: buildCommands[service] || `pm2 restart ${service}`,
    restart_command: `pm2 restart ${service}`
  };
});

// ─── Build testing_required ──────────────────────────────────────────────
const testingRequired = [];

if (schemaChanges.length > 0) {
  testingRequired.push('Run schema compatibility check: node scripts/check-schema-compat.js');
  testingRequired.push('Verify affected readers can query updated tables');
}

if (apiChanges.length > 0) {
  testingRequired.push('Run API smoke test: bash tests/e2e/smoke.sh');
  testingRequired.push('Verify API consumers still receive expected responses');
}

if (syncChanges.length > 0) {
  testingRequired.push('Verify cloud-venue sync completes without errors');
  testingRequired.push('Check sync table coverage: bash tests/e2e/cross-process.sh');
}

if (affectedServices.length > 0) {
  testingRequired.push('Run cross-service health check: bash scripts/cross-service-health.sh');
}

// Always include a basic verification
if (testingRequired.length === 0) {
  testingRequired.push('Verify build succeeds: cargo build --release -p rc-core');
  testingRequired.push('Verify health endpoint: curl http://localhost:8080/api/v1/health');
}

// ─── Build manifest ─────────────────────────────────────────────────────
const manifest = {
  type: 'sync_manifest',
  version: '1.0',
  timestamp: new Date().toISOString(),
  commit: commitHash,
  branch: branchName,
  summary: summaryArg || '',
  affected_services: affectedServices,
  schema_changes: schemaChanges,
  api_changes: apiChanges,
  sync_changes: syncChanges,
  testing_required: testingRequired
};

// ─── --validate mode ────────────────────────────────────────────────────
if (doValidate) {
  const requiredFields = ['type', 'version', 'timestamp', 'commit', 'branch',
                          'affected_services', 'schema_changes', 'api_changes',
                          'sync_changes', 'testing_required'];
  let valid = true;

  for (const field of requiredFields) {
    if (manifest[field] === undefined || manifest[field] === null) {
      console.error(`${RED}FAIL${RESET}: Missing field "${field}"`);
      valid = false;
    }
  }

  if (!manifest.commit || manifest.commit.length === 0) {
    console.error(`${RED}FAIL${RESET}: "commit" is empty`);
    valid = false;
  }

  if (!manifest.branch || manifest.branch.length === 0) {
    console.error(`${RED}FAIL${RESET}: "branch" is empty`);
    valid = false;
  }

  if (!Array.isArray(manifest.affected_services)) {
    console.error(`${RED}FAIL${RESET}: "affected_services" is not an array`);
    valid = false;
  }

  if (!Array.isArray(manifest.testing_required) || manifest.testing_required.length === 0) {
    console.error(`${RED}FAIL${RESET}: "testing_required" is empty`);
    valid = false;
  }

  if (valid) {
    console.log(`${GREEN}Manifest validation passed${RESET}`);
    console.log(`  commit: ${manifest.commit.substring(0, 8)}`);
    console.log(`  branch: ${manifest.branch}`);
    console.log(`  affected_services: ${manifest.affected_services.length}`);
    console.log(`  schema_changes: ${manifest.schema_changes.length}`);
    console.log(`  api_changes: ${manifest.api_changes.length}`);
    console.log(`  sync_changes: ${manifest.sync_changes.length}`);
    console.log(`  testing_required: ${manifest.testing_required.length}`);
    process.exit(0);
  } else {
    process.exit(1);
  }
}

// ─── Print manifest ─────────────────────────────────────────────────────
console.log(JSON.stringify(manifest, null, 2));

// ─── --send mode ────────────────────────────────────────────────────────
if (doSend) {
  if (!summaryArg) {
    console.error(`${RED}ERROR${RESET}: --summary is required when using --send`);
    process.exit(1);
  }

  const payload = JSON.stringify({
    sender: 'bono',
    recipient: 'james',
    type: 'command',
    content: JSON.stringify(manifest)
  });

  const options = {
    hostname: 'localhost',
    port: 3100,
    path: '/api/comms/messages',
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Content-Length': Buffer.byteLength(payload)
    }
  };

  const req = http.request(options, (res) => {
    let data = '';
    res.on('data', chunk => data += chunk);
    res.on('end', () => {
      if (res.statusCode >= 200 && res.statusCode < 300) {
        console.error(`\n${GREEN}Manifest sent to James via comms API${RESET}`);
      } else {
        console.error(`\n${RED}ERROR${RESET}: Comms API returned status ${res.statusCode}: ${data}`);
        process.exit(1);
      }
    });
  });

  req.on('error', (err) => {
    console.error(`\n${RED}ERROR${RESET}: Failed to send manifest: ${err.message}`);
    process.exit(1);
  });

  req.write(payload);
  req.end();
}
