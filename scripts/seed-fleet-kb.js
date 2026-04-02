#!/usr/bin/env node
// seed-fleet-kb.js — Populate the Fleet Knowledge Base with known solutions
// from Racing Point's operational history (34 shipped milestones).
//
// Usage: node scripts/seed-fleet-kb.js [--db <path>]
// Default DB: ./data/racecontrol.db (relative to CWD)
//
// Run on server: cd C:\RacingPoint && node <repo>/scripts/seed-fleet-kb.js
// Run on pod:    node scripts/seed-fleet-kb.js --db C:\RacingPoint\mesh_kb.db --pod
//
// Solutions are INSERT OR IGNORE — safe to run multiple times.

const Database = require(process.env.BETTER_SQLITE3_PATH || 'better-sqlite3');
const crypto = require('crypto');

const args = process.argv.slice(2);
const isPod = args.includes('--pod');
const dbIdx = args.indexOf('--db');
const dbPath = dbIdx >= 0 ? args[dbIdx + 1] : './data/racecontrol.db';

const now = new Date().toISOString();

function sha256(s) {
  return crypto.createHash('sha256').update(s).digest('hex').slice(0, 16);
}

// ─── Known Solutions from Operational History ──────────────────────────────

const solutions = [
  // --- Process & Startup Issues ---
  {
    problem_key: 'process_crash:conspitlink',
    root_cause: 'ConspitLink process multiplication — multiple instances grab HID device causing Bind failed errors and steering wheel flickering',
    fix_action: 'taskkill /F /IM ConspitLink.exe before start in start-rcagent.bat; enforce singleton via bat',
    fix_type: 'deterministic',
    symptoms: 'Steering wheel flickering, Bind failed errors in ConspitLink logs, 4-11 instances running',
    tags: ['conspitlink', 'hid', 'startup', 'flicker'],
    diagnosis_tier: 'deterministic',
    confidence: 1.0,
    success_count: 40,
    source: 'v27.0_audit',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },
  {
    problem_key: 'sentinel_unexpected:MAINTENANCE_MODE',
    root_cause: 'MAINTENANCE_MODE sentinel left from previous crash storm — blocks ALL restarts permanently with no timeout or auto-clear',
    fix_action: 'del C:\\RacingPoint\\MAINTENANCE_MODE & del C:\\RacingPoint\\GRACEFUL_RELAUNCH & del C:\\RacingPoint\\rcagent-restart-sentinel.txt',
    fix_type: 'deterministic',
    symptoms: 'Pod ws_connected=false, http_reachable=false, but responds to ping. rc-agent not running.',
    tags: ['maintenance', 'sentinel', 'restart', 'critical'],
    diagnosis_tier: 'deterministic',
    confidence: 1.0,
    success_count: 15,
    source: 'v26.0_audit',
    diagnosis_method: 'sre_stuck_state',
    fix_permanence: 'permanent',
  },
  {
    problem_key: 'process_crash:werfault',
    root_cause: 'WerFault.exe (Windows Error Reporting) orphan processes accumulate after crashes, consuming resources',
    fix_action: 'taskkill /F /IM werfault.exe & taskkill /F /IM werreport.exe',
    fix_type: 'deterministic',
    symptoms: 'Multiple werfault.exe/werreport.exe in tasklist, memory usage climbing',
    tags: ['orphan', 'cleanup', 'werfault'],
    diagnosis_tier: 'deterministic',
    confidence: 1.0,
    success_count: 100,
    source: 'tier1_builtin',
    diagnosis_method: 'deterministic',
    fix_permanence: 'permanent',
  },
  {
    problem_key: 'health_check_fail',
    root_cause: 'rc-agent running in Session 0 (SYSTEM) instead of Session 1 (interactive desktop) — cannot launch Edge, games, or GUI',
    fix_action: 'Kill rc-agent, let RCWatchdog service restart it in Session 1 via WTSQueryUserToken+CreateProcessAsUser. Verify: tasklist /V /FO CSV | findstr rc-agent shows Console not Services',
    fix_type: 'restart',
    symptoms: 'Health OK but edge_process_count=0 when lock_screen_state=screen_blanked. Games fail to launch. Blanking screen broken.',
    tags: ['session0', 'gui', 'blanking', 'critical'],
    diagnosis_tier: 'kb',
    confidence: 0.95,
    success_count: 8,
    source: 'v26.0_session0_incident',
    diagnosis_method: 'code_expert_session0',
    fix_permanence: 'permanent',
  },
  {
    problem_key: 'process_crash:powershell',
    root_cause: 'Watchdog PowerShell multiplication — multiple instances fighting over port 8080, accumulating ~60MB each',
    fix_action: 'taskkill /F /IM powershell.exe then schtasks /Run /TN StartRCTemp (bat kills existing watchdogs via WMIC before starting new one)',
    fix_type: 'deterministic',
    symptoms: 'Multiple powershell.exe in tasklist, port 8080 conflict, racecontrol fails to start',
    tags: ['watchdog', 'powershell', 'multiplication', 'server'],
    diagnosis_tier: 'deterministic',
    confidence: 1.0,
    success_count: 6,
    source: 'v17.0_deploy_incident',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },

  // --- Network & Connectivity ---
  {
    problem_key: 'ws_disconnect',
    root_cause: 'WebSocket disconnect due to server restart or network transient — pod auto-reconnects with exponential backoff + jitter',
    fix_action: 'Wait for auto-reconnect (max 60s with jitter). If persists >2min, check server health. If server OK, restart rc-agent on pod.',
    fix_type: 'config',
    symptoms: 'ws_connected=false in fleet health, pod still http_reachable',
    tags: ['websocket', 'connectivity', 'auto-reconnect'],
    diagnosis_tier: 'kb',
    confidence: 0.9,
    success_count: 50,
    source: 'v10.0_connectivity',
    diagnosis_method: 'sre_stuck_state',
    fix_permanence: 'permanent',
  },
  {
    problem_key: 'health_check_fail',
    root_cause: 'Allowlist fetch failed at boot (server was down) — empty allowlist causes all processes flagged as violations',
    fix_action: 'Wait for periodic re-fetch (every 5 min). Or restart rc-agent to trigger immediate fetch. Verify: violation_count_24h stops increasing.',
    fix_type: 'config',
    symptoms: 'violation_count_24h very high (100+), all pods affected simultaneously, started after server restart',
    tags: ['allowlist', 'process-guard', 'boot', 'false-positive'],
    diagnosis_tier: 'kb',
    confidence: 0.95,
    success_count: 8,
    source: 'v12.0_security',
    diagnosis_method: 'reasoner_absence',
    fix_permanence: 'permanent',
  },

  // --- Game Launch Issues ---
  {
    problem_key: 'game_launch_fail',
    root_cause: 'Assetto Corsa server config INI has wrong AI_LEVEL or missing AI cars — kiosk sends field names that dont match Rust AcLaunchParams struct (serde drops unknown fields silently)',
    fix_action: 'Verify kiosk buildLaunchArgs() field names match AcLaunchParams struct. Check generated race.ini on pod after test launch. ai_difficulty->ai_level, ai_count->ai_cars mapping.',
    fix_type: 'config',
    symptoms: 'Game launches but AI difficulty wrong, or zero AI opponents. No error logged anywhere.',
    tags: ['game', 'ac', 'config', 'serde', 'silent-failure'],
    diagnosis_tier: 'kb',
    confidence: 0.9,
    success_count: 5,
    source: 'v27.0_serialization_audit',
    diagnosis_method: 'consensus_5model',
    fix_permanence: 'permanent',
  },
  {
    problem_key: 'game_launch_fail',
    root_cause: 'GameTracker stuck in Launching state — WS message queued but not delivered (WS dropped between queue and delivery). No timeout on Launching state.',
    fix_action: 'Call /games/stop endpoint to clear stuck state. Then retry launch. Long-term: 60s timeout on Launching state auto-transitions to Error.',
    fix_type: 'restart',
    symptoms: 'Game launch returns ok:true but game never starts. Subsequent launches fail with "already has a game active".',
    tags: ['game', 'stuck', 'ws', 'gametracker'],
    diagnosis_tier: 'kb',
    confidence: 0.85,
    success_count: 3,
    source: 'v26.0_pod6_incident',
    diagnosis_method: 'sre_stuck_state',
    fix_permanence: 'workaround',
  },

  // --- Display & UI Issues ---
  {
    problem_key: 'display_mismatch',
    root_cause: 'NVIDIA Surround dropped to 1024x768 after explorer.exe restart — GPU display config disrupted',
    fix_action: 'NEVER restart explorer.exe on pods with NVIDIA Surround. Reboot pod to restore triple-monitor 7680x1440.',
    fix_type: 'manual',
    symptoms: 'Resolution dropped to 1024x768, single monitor instead of triple surround',
    tags: ['display', 'nvidia', 'surround', 'critical'],
    diagnosis_tier: 'kb',
    confidence: 1.0,
    success_count: 3,
    source: 'v16.0_blanking',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },
  {
    problem_key: 'health_check_fail',
    root_cause: 'Pod healer curl output includes quotes ("200" not 200) — u32::parse fails, unwrap_or(0), healer thinks lock screen is down, ForceRelaunchBrowser spam',
    fix_action: 'Strip quotes from curl stdout in PowerShell: $r = (curl.exe -s -o NUL -w "%{http_code}" ...).Trim(\'"\')',
    fix_type: 'deterministic',
    symptoms: 'Blanking screen flickers (kill+relaunch cycle every 30s). edge_process_count fluctuates.',
    tags: ['healer', 'curl', 'quotes', 'flicker', 'blanking'],
    diagnosis_tier: 'kb',
    confidence: 1.0,
    success_count: 8,
    source: 'v26.0_pod_healer_flicker',
    diagnosis_method: 'code_expert_session0',
    fix_permanence: 'permanent',
  },

  // --- Billing & Financial ---
  {
    problem_key: 'error_spike',
    root_cause: 'end_billing_session() overwrites wallet_debit_paise (line 2213) BEFORE reading it for refund calc (line 2255) — customer loses money on early-end',
    fix_action: 'Save original_debit = wallet_debit_paise BEFORE any UPDATE. Use original_debit in refund calculation. Fixed in commit 5d1ea000.',
    fix_type: 'code_change',
    symptoms: 'Customer refund amount incorrect on early session end. Wallet balance lower than expected.',
    tags: ['billing', 'refund', 'financial', 'critical', 'F-05'],
    diagnosis_tier: 'multi_model',
    confidence: 1.0,
    success_count: 1,
    source: 'v33.0_billing_integrity',
    diagnosis_method: 'consensus_5model',
    fix_permanence: 'permanent',
  },

  // --- Deploy & Binary Issues ---
  {
    problem_key: 'health_check_fail',
    root_cause: 'Stale GIT_HASH in binary — cargo caches binary when no source files change, but build.rs embeds GIT_HASH at compile time and cargo doesnt detect new commits',
    fix_action: 'touch crates/<crate>/build.rs before cargo build --release after any git commit. Verify build_id matches git rev-parse --short HEAD.',
    fix_type: 'deterministic',
    symptoms: 'build_id in /health doesnt match HEAD. Binary is functionally old despite new commits.',
    tags: ['deploy', 'build', 'git-hash', 'cargo-cache'],
    diagnosis_tier: 'kb',
    confidence: 1.0,
    success_count: 5,
    source: 'v17.0_deploy',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },
  {
    problem_key: 'process_crash:rc-agent',
    root_cause: 'Crash loop from corrupted WMI/COM state — ntdll.dll access violation (0xC0000005, offset 0xaa83) at ~17s after startup',
    fix_action: 'Reboot pod: shutdown /r /t 5 /f. Do NOT try SSH restarts or binary swaps — OS state is corrupt. After reboot, if persists: wevtutil qe Application, sfc /scannow, winmgmt /verifyrepository',
    fix_type: 'restart',
    symptoms: '>3 startup reports in 5min with uptime<30s. Same binary stable on other pods.',
    tags: ['crash-loop', 'ntdll', 'wmi', 'reboot', 'critical'],
    diagnosis_tier: 'kb',
    confidence: 0.9,
    success_count: 2,
    source: 'v26.0_pod6_crash',
    diagnosis_method: 'scanner_enumeration',
    fix_permanence: 'workaround',
  },

  // --- Config & TOML Issues ---
  {
    problem_key: 'health_check_fail',
    root_cause: 'SSH banner lines prepended to racecontrol.toml — TOML parser fails, load_or_default falls back to empty config, process guard runs with 0 allowed entries',
    fix_action: 'Use scp to copy files from remote hosts, never ssh cat > file. Validate: head -1 file | grep -q "^\\[" || echo CORRUPTED. Re-copy clean config.',
    fix_type: 'config',
    symptoms: 'Process guard flagging everything. Config has SSH banner text on first lines.',
    tags: ['config', 'ssh', 'toml', 'corruption'],
    diagnosis_tier: 'kb',
    confidence: 1.0,
    success_count: 2,
    source: 'v12.0_ssh_incident',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },

  // --- Frontend / Next.js Issues ---
  {
    problem_key: 'health_check_fail',
    root_cause: 'Next.js standalone deploy missing static files — outputFileTracingRoot not set, absolute build-machine paths baked into required-server-files.json',
    fix_action: 'Set outputFileTracingRoot in next.config.ts. Copy .next/static into .next/standalone/. After deploy, curl _next/static/ URL — 200=OK, 404=stale appDir.',
    fix_type: 'config',
    symptoms: 'Pages render but unstyled (no CSS/JS). Health endpoint shows ok. All _next/static/ returns 404.',
    tags: ['nextjs', 'static', 'deploy', 'standalone'],
    diagnosis_tier: 'kb',
    confidence: 1.0,
    success_count: 3,
    source: 'v17.0_cloud',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },

  // --- Tailscale / VPN ---
  {
    problem_key: 'ws_disconnect',
    root_cause: 'Tailscale loses auth on reboot — NeedsLogin state, ForceDaemon not enabled, pre-auth key not set',
    fix_action: 'tailscale up --authkey=<pre-auth-key> --unattended --force-reauth. Set ForceDaemon=true in registry. Verify with tailscale status.',
    fix_type: 'config',
    symptoms: 'Tailscale in NoState or NeedsLogin after reboot. SSH via Tailscale IP fails.',
    tags: ['tailscale', 'auth', 'reboot', 'vpn'],
    diagnosis_tier: 'kb',
    confidence: 1.0,
    success_count: 11,
    source: 'v10.0_connectivity',
    diagnosis_method: 'sre_stuck_state',
    fix_permanence: 'permanent',
  },

  // --- OTA / Deploy Pipeline ---
  {
    problem_key: 'sentinel_unexpected:OTA_DEPLOYING',
    root_cause: 'OTA_DEPLOYING sentinel left from interrupted deploy — blocks all recovery systems',
    fix_action: 'del C:\\RacingPoint\\OTA_DEPLOYING. Then verify rc-agent is running.',
    fix_type: 'deterministic',
    symptoms: 'rc-sentry and pod_monitor refuse to restart rc-agent. OTA_DEPLOYING file present.',
    tags: ['ota', 'sentinel', 'deploy', 'stuck'],
    diagnosis_tier: 'deterministic',
    confidence: 1.0,
    success_count: 3,
    source: 'v22.0_ota',
    diagnosis_method: 'deterministic',
    fix_permanence: 'permanent',
  },

  // --- USB / Hardware ---
  {
    problem_key: 'process_crash:variable_dump',
    root_cause: 'VSD Craft Variable_dump.exe crashes on pedal input via USB — unstable with Conspit Ares 8Nm wheelbases',
    fix_action: 'Kill Variable_dump.exe, disable auto-start. Monitor if game sessions stabilize without it.',
    fix_type: 'deterministic',
    symptoms: 'Variable_dump.exe crash dumps in Event Viewer. Game sessions interrupted.',
    tags: ['usb', 'vsd', 'pedal', 'hardware', 'crash'],
    diagnosis_tier: 'kb',
    confidence: 0.7,
    success_count: 2,
    source: 'v26.0_debug',
    diagnosis_method: 'scanner_enumeration',
    fix_permanence: 'workaround',
  },

  // --- Power & Sleep ---
  {
    problem_key: 'health_check_fail',
    root_cause: 'USB selective suspend or Windows power plan puts USB controllers to sleep — wheelbases and pedals disconnect mid-session',
    fix_action: 'powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c (High Performance). Disable USB selective suspend in device manager. Enforce in start-rcagent.bat.',
    fix_type: 'config',
    symptoms: 'Wheelbase disconnects mid-race. USB devices disappear from Device Manager briefly.',
    tags: ['usb', 'power', 'sleep', 'wheelbase'],
    diagnosis_tier: 'kb',
    confidence: 0.95,
    success_count: 8,
    source: 'v27.0_conspitlink',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },

  // --- Popup / Overlay Interference ---
  {
    problem_key: 'display_mismatch',
    root_cause: 'NVIDIA Overlay, Windows Copilot, Widgets, and Settings popups overlay blanking screen — visible to customers',
    fix_action: 'Disable NVIDIA Overlay (registry), Copilot (policy), Widgets (policy), Settings (process kill). Run disable-popups.ps1 on all pods.',
    fix_type: 'config',
    symptoms: 'Popup windows visible on top of blanking screen or game. Customer sees system UI.',
    tags: ['overlay', 'popup', 'nvidia', 'copilot', 'widgets'],
    diagnosis_tier: 'deterministic',
    confidence: 1.0,
    success_count: 8,
    source: 'v16.0_audit',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },

  // --- Edge / Kiosk Browser ---
  {
    problem_key: 'health_check_fail',
    root_cause: 'Edge --app mode windows persist via SNSS session files, not RestoreOnStartup policy — closing Edge doesnt clear them, next launch shows old tabs',
    fix_action: 'Delete Edge SNSS files before launching: del "%LOCALAPPDATA%\\Microsoft\\Edge\\User Data\\Default\\Sessions\\*" /Q. Add to start-rcagent.bat.',
    fix_type: 'deterministic',
    symptoms: 'Edge kiosk opens with previous session tabs instead of blank/blank screen URL',
    tags: ['edge', 'kiosk', 'session', 'snss', 'startup'],
    diagnosis_tier: 'kb',
    confidence: 0.95,
    success_count: 8,
    source: 'v16.0_kiosk',
    diagnosis_method: 'manual_investigation',
    fix_permanence: 'permanent',
  },
];

// ─── Seeder Logic ──────────────────────────────────────────────────────────

function seedFleetKB(db) {
  // Fleet KB uses the server schema (fleet_solutions table)
  const stmt = db.prepare(`
    INSERT OR IGNORE INTO fleet_solutions
    (id, problem_key, problem_hash, symptoms, environment, root_cause, fix_action,
     fix_type, status, success_count, fail_count, confidence, cost_to_diagnose,
     models_used, diagnosis_tier, source_node, venue_id, created_at, updated_at,
     version, ttl_days, tags)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
  `);

  let inserted = 0;
  for (const sol of solutions) {
    const id = `sol_seed_${sha256(sol.problem_key + sol.root_cause)}`;
    const hash = sha256(sol.problem_key + JSON.stringify(sol.symptoms));
    const status = sol.confidence >= 1.0 ? '"hardened"' :
                   sol.success_count >= 3 ? '"fleet_verified"' : '"candidate"';
    const info = stmt.run(
      id,
      sol.problem_key,
      hash,
      JSON.stringify({ summary: sol.symptoms }),
      JSON.stringify({ tags: [`source:${sol.source}`, 'seeded:true'] }),
      sol.root_cause,
      JSON.stringify({ description: sol.fix_action, steps: [sol.fix_action] }),
      JSON.stringify(sol.fix_type),
      status,
      sol.success_count,
      0, // fail_count
      sol.confidence,
      0, // cost_to_diagnose (manual/seeded)
      JSON.stringify(sol.diagnosis_method ? [sol.diagnosis_method] : []),
      JSON.stringify(sol.diagnosis_tier),
      'james_seed',
      'rp-hyderabad',
      now,
      now,
      1, // version
      365, // ttl_days (1 year for seeded solutions)
      JSON.stringify(sol.tags)
    );
    if (info.changes > 0) inserted++;
  }
  return inserted;
}

function seedPodKB(db) {
  // Pod KB uses the local schema (solutions table, different columns)
  // Check if table exists first
  const tableExists = db.prepare(
    "SELECT name FROM sqlite_master WHERE type='table' AND name='solutions'"
  ).get();

  if (!tableExists) {
    // Create the table (mirrors knowledge_base.rs run_migrations)
    db.exec(`
      CREATE TABLE IF NOT EXISTS solutions (
        id TEXT PRIMARY KEY,
        problem_key TEXT NOT NULL,
        problem_hash TEXT NOT NULL,
        symptoms TEXT NOT NULL,
        environment TEXT NOT NULL,
        root_cause TEXT NOT NULL,
        fix_action TEXT NOT NULL,
        fix_type TEXT NOT NULL,
        success_count INTEGER DEFAULT 1,
        fail_count INTEGER DEFAULT 0,
        confidence REAL DEFAULT 1.0,
        cost_to_diagnose REAL DEFAULT 0,
        models_used TEXT,
        source_node TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        version INTEGER DEFAULT 1,
        ttl_days INTEGER DEFAULT 90,
        tags TEXT,
        diagnosis_method TEXT,
        fix_permanence TEXT DEFAULT 'workaround',
        recurrence_count INTEGER DEFAULT 0,
        permanent_fix_id TEXT,
        last_recurrence TEXT,
        permanent_attempt_at TEXT
      );
      CREATE INDEX IF NOT EXISTS idx_solutions_hash ON solutions(problem_hash);
      CREATE INDEX IF NOT EXISTS idx_solutions_key ON solutions(problem_key);
      CREATE TABLE IF NOT EXISTS experiments (
        id TEXT PRIMARY KEY,
        problem_key TEXT NOT NULL,
        hypothesis TEXT NOT NULL,
        test_plan TEXT NOT NULL,
        result TEXT,
        cost REAL DEFAULT 0,
        node TEXT NOT NULL,
        created_at TEXT NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_experiments_key ON experiments(problem_key);
    `);
  }

  const stmt = db.prepare(`
    INSERT OR IGNORE INTO solutions
    (id, problem_key, problem_hash, symptoms, environment,
     root_cause, fix_action, fix_type, success_count, fail_count,
     confidence, cost_to_diagnose, models_used, source_node,
     created_at, updated_at, version, ttl_days, tags, diagnosis_method,
     fix_permanence, recurrence_count, permanent_fix_id, last_recurrence, permanent_attempt_at)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
  `);

  let inserted = 0;
  for (const sol of solutions) {
    const id = `sol_seed_${sha256(sol.problem_key + sol.root_cause)}`;
    const hash = sha256(sol.problem_key + JSON.stringify(sol.symptoms));
    const info = stmt.run(
      id,
      sol.problem_key,
      hash,
      JSON.stringify({ summary: sol.symptoms }),
      JSON.stringify({ tags: [`source:${sol.source}`, 'seeded:true'] }),
      sol.root_cause,
      sol.fix_action,
      sol.fix_type,
      sol.success_count,
      0,
      sol.confidence,
      0,
      JSON.stringify(sol.diagnosis_method ? [sol.diagnosis_method] : []),
      'james_seed',
      now,
      now,
      1,
      365,
      JSON.stringify(sol.tags),
      sol.diagnosis_method || 'manual_investigation',
      sol.fix_permanence || 'permanent',
      0,
      null,
      null,
      null
    );
    if (info.changes > 0) inserted++;
  }
  return inserted;
}

// ─── Main ──────────────────────────────────────────────────────────────────

try {
  console.log(`Opening DB: ${dbPath}`);
  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');

  const inserted = isPod ? seedPodKB(db) : seedFleetKB(db);

  // Verify
  const table = isPod ? 'solutions' : 'fleet_solutions';
  const total = db.prepare(`SELECT COUNT(*) as c FROM ${table}`).get().c;

  console.log(`\nSeeded ${inserted} new solutions (${total} total in ${table})`);

  // Show summary
  if (!isPod) {
    const byStatus = db.prepare(
      "SELECT status, COUNT(*) as c FROM fleet_solutions GROUP BY status"
    ).all();
    console.log('\nBy status:');
    byStatus.forEach(r => console.log(`  ${r.status}: ${r.c}`));
  } else {
    const byPermanence = db.prepare(
      "SELECT fix_permanence, COUNT(*) as c FROM solutions GROUP BY fix_permanence"
    ).all();
    console.log('\nBy permanence:');
    byPermanence.forEach(r => console.log(`  ${r.fix_permanence}: ${r.c}`));
  }

  db.close();
  console.log('\nDone.');
} catch (err) {
  console.error('Error:', err.message);
  process.exit(1);
}
