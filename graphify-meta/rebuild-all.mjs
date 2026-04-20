#!/usr/bin/env node
// graphify-meta/rebuild-all.mjs
// One-shot rebuild of the entire meta-map pipeline. Runs all 5 stages in order:
//   1. slice-submodules.mjs  (filter racecontrol/graph.json into 8 sub-slices + HTML)
//   2. api-edges.mjs         (HTTP URL coupling)
//   3. db-edges.mjs          (SQL/Prisma shared-table coupling)
//   4. ws-edges.mjs          (WS enum-variant coupling)
//   5. build-meta.mjs        (assemble meta-graph + meta.html + META_REPORT.md)
//
// Idempotent, no external API calls, <2 seconds. Invoke manually or wire into a
// post-commit hook when slice graphs change.
//
// Exit 0 on success, 1 on any stage failure.

import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const stages = [
  'slice-submodules.mjs',
  'api-edges.mjs',
  'db-edges.mjs',
  'ws-edges.mjs',
  'fs-edges.mjs',
  'struct-edges.mjs',
  'import-edges.mjs',
  'cochange-edges.mjs',  // depends on meta-graph to compute hidden_coupling diff; run before build-meta
  'build-meta.mjs',
];

let failed = false;
for (const stage of stages) {
  const start = Date.now();
  const r = spawnSync(process.execPath, [path.join(__dirname, stage)], {
    cwd: __dirname,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const elapsed = Date.now() - start;
  if (r.status === 0) {
    // Tail the last line of stdout — each stage ends with a one-line summary
    const lines = (r.stdout || '').trim().split('\n').filter(Boolean);
    const tail = lines[lines.length - 1] || '(no output)';
    console.log(`[OK  ] ${stage.padEnd(22)} ${elapsed}ms  ${tail.slice(0, 80)}`);
  } else {
    console.log(`[FAIL] ${stage.padEnd(22)} ${elapsed}ms  exit=${r.status}`);
    console.log(`       stderr: ${(r.stderr || '').slice(0, 300)}`);
    failed = true;
  }
}

process.exit(failed ? 1 : 0);
