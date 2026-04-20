// racecontrol/scripts/graphify-post/subsystems.mjs
// Permanent subsystem registry — source-of-truth for where each subsystem lives on disk
// + how to detect its nodes in the graphify graph. Adding a new subsystem = one entry here.

// All paths are normalized with forward slashes; matchers are case-insensitive substring
// or regex predicates applied to node.source_file.

export const SUBSYSTEMS = {
  admin: {
    label: 'Admin Panel',
    purpose: 'Staff admin dashboard at :3201. Championship/event management, HR, tournaments, bulk exports.',
    repo_root: 'C:/Users/bono/racingpoint/racingpoint-admin',
    scan_globs: [
      'racingpoint-admin/src/**/*.{ts,tsx}',
    ],
    backend_paths: [
      // Rust endpoints the admin panel consumes
      /crates[\\/]racecontrol[\\/]src[\\/]api[\\/]admin/i,
      /crates[\\/]racecontrol[\\/]src[\\/]api[\\/]tournament_admin/i,
      /crates[\\/]racecontrol[\\/]src[\\/]api[\\/]staff_crud/i,
      /crates[\\/]racecontrol[\\/]src[\\/]auth[\\/]admin/i,
    ],
    frontend_paths: [
      /racingpoint-admin[\\/]/i,
    ],
    expected_fs_tsx: 117, // filesystem truth (re-measure with find script)
  },

  billing: {
    label: 'Billing',
    purpose: 'USB-HID-driven drive-time billing, pricing tiers, wallet debits, refunds, session FSM.',
    repo_root: 'C:/Users/bono/racingpoint/racecontrol',
    scan_globs: [
      'crates/racecontrol/src/billing*.rs',
      'crates/racecontrol/src/api/billing*.rs',
      'web/src/app/billing/**/*.tsx',
      'kiosk/src/app/billing/**/*.tsx',
    ],
    backend_paths: [
      /[\\/]billing[a-z_]*\.rs$/i,
      /api[\\/]billing/i,
      /api[\\/]pricing/i,
    ],
    frontend_paths: [
      /[\\/](web|kiosk)[\\/].*[\\/]billing[\\/]/i,
    ],
    expected_fs_tsx: 4,    // web/src/app/billing/* pages
    expected_fs_rust: 38,  // crates/racecontrol/src/*billing*.rs
  },

  kiosk: {
    label: 'Kiosk (PWA)',
    purpose: 'Next.js kiosk app at :3300 for pod screens. Staff PIN login, experience-card selection, game launch UI, lock screen, blanking.',
    repo_root: 'C:/Users/bono/racingpoint/racecontrol/kiosk',
    scan_globs: [
      'kiosk/src/**/*.{ts,tsx}',
    ],
    backend_paths: [
      /crates[\\/]racecontrol[\\/]src[\\/]api[\\/]kiosk/i,
      /crates[\\/]racecontrol[\\/]src[\\/]kiosk[^\\/]*\.rs$/i,
      /crates[\\/]rc-agent[\\/]src[\\/]kiosk/i,
    ],
    frontend_paths: [
      /[\\/]kiosk[\\/]src[\\/]/i,
    ],
    expected_fs_tsx: 65,
  },

  web: {
    label: 'Web / POS (PWA)',
    purpose: 'Next.js web app at :3200 — POS terminal, customer self-service, wallet top-ups, session history.',
    repo_root: 'C:/Users/bono/racingpoint/racecontrol/web',
    scan_globs: [
      'web/src/**/*.{ts,tsx}',
    ],
    backend_paths: [
      /crates[\\/]racecontrol[\\/]src[\\/]api[\\/]customer/i,
      /crates[\\/]racecontrol[\\/]src[\\/]api[\\/]wallet/i,
      /crates[\\/]racecontrol[\\/]src[\\/]api[\\/]terminal/i,
    ],
    frontend_paths: [
      /[\\/]web[\\/]src[\\/]/i,
    ],
    expected_fs_tsx: 95,
  },

  'rc-agent': {
    label: 'rc-agent (pod binary)',
    purpose: 'Rust binary that runs on each pod — game launch, blanking, lock screen, kiosk browser mgmt, WS to server.',
    repo_root: 'C:/Users/bono/racingpoint/racecontrol/crates/rc-agent',
    scan_globs: [ 'crates/rc-agent/src/**/*.rs', 'crates/rc-agent/tests/**/*.rs' ],
    backend_paths: [
      /crates[\\/]rc-agent[\\/]/i,
    ],
    frontend_paths: [],
  },

  'rc-sentry': {
    label: 'rc-sentry (pod supervisor)',
    purpose: 'Watchdog binary that monitors rc-agent on each pod, performs deploys, executes remote commands.',
    repo_root: 'C:/Users/bono/racingpoint/racecontrol/crates/rc-sentry',
    scan_globs: [ 'crates/rc-sentry/src/**/*.rs', 'crates/rc-sentry-ai/src/**/*.rs' ],
    backend_paths: [
      /crates[\\/]rc-sentry[-a-z]*[\\/]/i,
    ],
    frontend_paths: [],
  },

  'comms-link': {
    label: 'comms-link (James↔Bono relay)',
    purpose: 'WebSocket relay between James (on-site) and Bono (VPS). Node.js + websocket.',
    repo_root: 'C:/Users/bono/racingpoint/comms-link',
    scan_globs: [ 'comms-link/**/*.{ts,tsx,js,mjs}' ],
    backend_paths: [ /comms-link[\\/]/i ],
    frontend_paths: [],
  },

  'whatsapp-bot': {
    label: 'WhatsApp Bot',
    purpose: 'Customer-facing WhatsApp bot — customer enquiries, booking flow, admin alerts via Claude API.',
    repo_root: 'C:/Users/bono/racingpoint/whatsapp-bot',
    scan_globs: [ 'whatsapp-bot/**/*.{ts,tsx,js,mjs,py}' ],
    backend_paths: [ /whatsapp-bot[\\/]/i ],
    frontend_paths: [],
  },

  'people-tracker': {
    label: 'People Tracker',
    purpose: 'YOLOv8 people-tracker — entry/exit counting on 3 cameras. FastAPI at :8095.',
    repo_root: 'C:/Users/bono/racingpoint/people-tracker',
    scan_globs: [ 'people-tracker/**/*.{py,js,ts}' ],
    backend_paths: [ /people-tracker[\\/]/i ],
    frontend_paths: [],
  },

  'marketing': {
    label: 'Marketing',
    purpose: 'Marketing campaigns, nudge pipelines, psychological retention logic.',
    repo_root: 'C:/Users/bono/racingpoint/marketing',
    scan_globs: [ 'marketing/**/*' ],
    backend_paths: [ /marketing[\\/]/i ],
    frontend_paths: [],
  },
};

// Helper: return all filesystem source files for a subsystem (excluding worktrees + build output)
export async function enumerateFilesystem(subsys) {
  const { globby } = await import('globby').catch(() => ({ globby: null }));
  // Fall back to filesystem walk if globby isn't available
  if (!globby) {
    const fs = await import('fs');
    const path = await import('path');
    const results = [];
    function walk(dir, depth) {
      if (depth > 8) return;
      let entries;
      try { entries = fs.readdirSync(dir, { withFileTypes: true }); }
      catch { return; }
      for (const ent of entries) {
        const p = path.join(dir, ent.name);
        // Exclude ephemeral/build dirs
        if (ent.isDirectory()) {
          if (/node_modules|\.next|worktree|dist|target|__pycache__/i.test(ent.name)) continue;
          walk(p, depth + 1);
        } else if (/\.(ts|tsx|rs|py)$/i.test(ent.name)) {
          results.push(p.replace(/\\/g, '/'));
        }
      }
    }
    walk(subsys.repo_root, 0);
    return results;
  }
  const out = [];
  for (const g of subsys.scan_globs) {
    const files = await globby([g], { cwd: subsys.repo_root, absolute: true, ignore: ['**/node_modules/**', '**/.next/**', '**/worktree*/**'] });
    out.push(...files);
  }
  return out;
}

// Classify a graph node into a subsystem (or 'uncategorized')
export function classifyNode(node, subsystems = SUBSYSTEMS) {
  const sf = (node.source_file || '').replace(/\\/g, '/');
  const matches = [];
  for (const [id, sub] of Object.entries(subsystems)) {
    for (const pat of sub.frontend_paths || []) {
      if (pat.test(sf)) { matches.push(`${id}:frontend`); break; }
    }
    for (const pat of sub.backend_paths || []) {
      if (pat.test(sf)) { matches.push(`${id}:backend`); break; }
    }
  }
  return matches.length === 0 ? ['uncategorized'] : matches;
}

export default SUBSYSTEMS;
