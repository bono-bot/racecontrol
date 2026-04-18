#!/usr/bin/env node
// Unit tests for scripts/diagnose/bundle-helper.js.
//
// Run: node scripts/diagnose/bundle-helper.test.js
// Exits 0 on all-pass, 1 on any failure.

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const {
  filesForClasses,
  extractErrorTerms,
  grepNarrow,
  applyTokenCap,
  annotate,
  prepareQueryRelevantBundle,
  estimateTokens,
} = require('./bundle-helper.js');

let pass = 0;
let fail = 0;
function run(name, fn) {
  try {
    fn();
    console.log(`ok  ${name}`);
    pass += 1;
  } catch (e) {
    console.error(`FAIL ${name}`);
    console.error(`     ${e.message}`);
    fail += 1;
  }
}

// ─── Helpers ───────────────────────────────────────────────────────────────

function makeTempRepo() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'bundle-test-'));
  return root;
}

function writeFile(root, rel, content) {
  const abs = path.join(root, rel);
  fs.mkdirSync(path.dirname(abs), { recursive: true });
  fs.writeFileSync(abs, content);
}

function writeClassMap(root, map) {
  const mapPath = path.join(root, 'class-to-globs.json');
  fs.writeFileSync(mapPath, JSON.stringify({ classes: map }));
  return mapPath;
}

// ─── extractErrorTerms ─────────────────────────────────────────────────────

run('extractErrorTerms: quoted strings', () => {
  const terms = extractErrorTerms('the "game process exited" error on pod_3');
  assert(terms.includes('game process exited'), 'missing quoted term');
});

run('extractErrorTerms: exit code N', () => {
  const terms = extractErrorTerms('game crashed with exit code 1');
  assert(terms.some((t) => t.toLowerCase().includes('exit')), 'missing exit-code term');
});

run('extractErrorTerms: 0x hex code', () => {
  const terms = extractErrorTerms('process terminated with code 0xC0000005');
  assert(terms.some((t) => t.includes('0xC0000005') || t.includes('0xc0000005')), 'missing hex term');
});

run('extractErrorTerms: backticked identifier', () => {
  const terms = extractErrorTerms('`steam_checks.rs` never retries the dialog');
  assert(terms.includes('steam_checks.rs'), 'missing backticked term');
});

run('extractErrorTerms: keyword-list hit', () => {
  const terms = extractErrorTerms('orphan process blocks relaunch');
  assert(terms.includes('orphan'), 'missing orphan keyword');
});

run('extractErrorTerms: empty query → empty', () => {
  assert.deepEqual(extractErrorTerms(''), []);
  assert.deepEqual(extractErrorTerms(null), []);
});

// ─── filesForClasses ───────────────────────────────────────────────────────

run('filesForClasses: dedupes across classes', () => {
  const root = makeTempRepo();
  const mapPath = writeClassMap(root, {
    'class-a': ['src/shared.rs', 'src/a.rs'],
    'class-b': ['src/shared.rs', 'src/b.rs'],
  });
  const out = filesForClasses(['class-a', 'class-b'], mapPath);
  assert.deepEqual(out, ['src/shared.rs', 'src/a.rs', 'src/b.rs']);
});

run('filesForClasses: unknown class skipped', () => {
  const root = makeTempRepo();
  const mapPath = writeClassMap(root, { 'known': ['src/k.rs'] });
  const out = filesForClasses(['unknown', 'known'], mapPath);
  assert.deepEqual(out, ['src/k.rs']);
});

// ─── grepNarrow ────────────────────────────────────────────────────────────

run('grepNarrow: keeps matching file, excludes non-matching', () => {
  const root = makeTempRepo();
  writeFile(root, 'src/hit.rs', 'fn foo() { steam_checks::dismiss_dialog(); }');
  writeFile(root, 'src/miss.rs', 'fn bar() { println!("hello"); }');
  const { kept, excluded } = grepNarrow(
    ['src/hit.rs', 'src/miss.rs'],
    ['steam_checks'],
    { repoRoot: root }
  );
  assert.equal(kept.length, 1);
  assert.equal(kept[0].path, 'src/hit.rs');
  assert.deepEqual(excluded.not_matched, ['src/miss.rs']);
});

run('grepNarrow: not_found for missing file', () => {
  const root = makeTempRepo();
  const { kept, excluded } = grepNarrow(['src/ghost.rs'], ['x'], { repoRoot: root });
  assert.equal(kept.length, 0);
  assert.deepEqual(excluded.not_found, ['src/ghost.rs']);
});

run('grepNarrow: too_large when > 50KB', () => {
  const root = makeTempRepo();
  writeFile(root, 'src/big.rs', 'x'.repeat(60_000));
  const { kept, excluded } = grepNarrow(['src/big.rs'], ['x'], { repoRoot: root });
  assert.equal(kept.length, 0);
  assert.deepEqual(excluded.too_large, ['src/big.rs']);
});

run('grepNarrow: counts multiple matches', () => {
  const root = makeTempRepo();
  writeFile(root, 'src/multi.rs', 'foo foo foo bar');
  const { kept } = grepNarrow(['src/multi.rs'], ['foo', 'bar', 'missing'], {
    repoRoot: root,
  });
  assert.equal(kept.length, 1);
  assert.deepEqual(kept[0].matches, ['foo', 'bar']);
  assert.equal(kept[0].match_count, 2);
});

run('grepNarrow: empty terms → include all readable', () => {
  const root = makeTempRepo();
  writeFile(root, 'src/any.rs', 'content');
  const { kept } = grepNarrow(['src/any.rs'], [], { repoRoot: root });
  assert.equal(kept.length, 1);
  assert.equal(kept[0].matches, null);
});

// ─── applyTokenCap ─────────────────────────────────────────────────────────

run('applyTokenCap: sorts by match_count, includes until budget exhausted', () => {
  const files = [
    { path: 'a.rs', content: 'x'.repeat(4000), match_count: 1 }, // 1000 tokens
    { path: 'b.rs', content: 'x'.repeat(4000), match_count: 3 }, // 1000 tokens
    { path: 'c.rs', content: 'x'.repeat(4000), match_count: 2 }, // 1000 tokens
  ];
  const { included, truncated, tokens_estimated } = applyTokenCap(files, 2500);
  // Budget fits 2 files (2000 tokens). Highest match_count first = b, then c.
  assert.deepEqual(included.map((f) => f.path), ['b.rs', 'c.rs']);
  assert.deepEqual(truncated, ['a.rs']);
  assert.equal(tokens_estimated, 2000);
});

run('applyTokenCap: zero budget → all truncated', () => {
  const files = [{ path: 'a.rs', content: 'xxxx', match_count: 1 }];
  const { included, truncated } = applyTokenCap(files, 0);
  assert.equal(included.length, 0);
  assert.deepEqual(truncated, ['a.rs']);
});

// ─── annotate ─────────────────────────────────────────────────────────────

run('annotate: adds why_selected with class and matches', () => {
  const files = [
    { path: 'a.rs', content: 'x', matches: ['foo', 'bar'] },
  ];
  const out = annotate(files, ['class-a']);
  assert.match(out[0].why_selected, /class=class-a/);
  assert.match(out[0].why_selected, /foo/);
});

// ─── Orchestrator end-to-end ──────────────────────────────────────────────

run('prepareQueryRelevantBundle: E2E smoke with temp repo', () => {
  const root = makeTempRepo();
  writeFile(root, 'src/steam_checks.rs', 'fn dismiss() { /* vguiPopupWindow */ }');
  writeFile(root, 'src/unrelated.rs', 'fn other() {}');
  const mapPath = writeClassMap(root, {
    'steam-dialog': ['src/steam_checks.rs', 'src/unrelated.rs'],
  });

  const bundle = prepareQueryRelevantBundle(
    'pod_3 `vguiPopupWindow` dialog visible after 60s',
    ['steam-dialog'],
    { repoRoot: root, mapPath }
  );

  assert.equal(bundle.files.length, 1, 'expected only the matching file');
  assert.equal(bundle.files[0].path, 'src/steam_checks.rs');
  assert.deepEqual(bundle.excluded_reasons.not_matched, ['src/unrelated.rs']);
  assert(bundle.tokens_estimated > 0);
  assert.deepEqual(bundle.symptom_classes_used, ['steam-dialog']);
  assert(bundle.terms_extracted.includes('vguiPopupWindow'));
});

run('prepareQueryRelevantBundle: empty classes → empty bundle', () => {
  const root = makeTempRepo();
  const mapPath = writeClassMap(root, {});
  const bundle = prepareQueryRelevantBundle('any query', [], {
    repoRoot: root,
    mapPath,
  });
  assert.equal(bundle.files.length, 0);
  assert.deepEqual(bundle.candidate_paths, []);
});

// ─── estimateTokens sanity ────────────────────────────────────────────────

run('estimateTokens: proportional to length', () => {
  assert.equal(estimateTokens(''), 0);
  assert.equal(estimateTokens('xxxx'), 1);
  assert.equal(estimateTokens('x'.repeat(100)), 25);
});

console.log(`\n${pass} passed, ${fail} failed`);
process.exit(fail > 0 ? 1 : 0);
