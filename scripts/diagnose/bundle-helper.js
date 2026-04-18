#!/usr/bin/env node
// L1 retrieval helper — Commit 3 of MMA-TRIAGE-ROUTER spec.
//
// When the triage router returns AUDIT_DEEP (symptom class matched, no
// oracle hit) or RESEARCH_NEW with a hint, the MMA caller can use
// prepareQueryRelevantBundle() instead of bundling whole directories.
//
// Problem: the 2026-04-17 MMA audits handed models 50+ files per crate
// as context. Keyword-similarity retrieval drowned in distractor and
// produced hallucinated file paths, env vars, and bug IDs.
//
// Strategy: given symptom classes, look up candidate files from
// class-to-globs.json, grep them for error strings extracted from the
// query, sort by match density, cap at token budget, annotate each file
// with why_selected so the model stays oriented.
//
// Spec: .planning/specs/MMA-TRIAGE-ROUTER.md (Commit 3 of 3).
//
// Exports:
//   filesForClasses(classes) -> string[]
//   extractErrorTerms(query) -> string[]
//   grepNarrow(files, terms) -> { kept, excluded }
//   prepareQueryRelevantBundle(query, symptomClasses, options?) -> bundle

const fs = require('node:fs');
const path = require('node:path');

const REPO_ROOT = path.resolve(__dirname, '..', '..');
const DEFAULT_TOKEN_BUDGET = 20_000;
const MAX_FILE_BYTES = 50_000;

function loadJson(p) {
  return JSON.parse(fs.readFileSync(p, 'utf8'));
}

// ─── Step 1: symptom class → candidate file paths ─────────────────────────

function filesForClasses(classes, mapPath) {
  const mapFile = mapPath || path.join(__dirname, 'class-to-globs.json');
  const map = loadJson(mapFile).classes || {};
  const seen = new Set();
  const out = [];
  for (const cls of classes) {
    const paths = map[cls];
    if (!Array.isArray(paths)) continue;
    for (const p of paths) {
      if (!seen.has(p)) {
        seen.add(p);
        out.push(p);
      }
    }
  }
  return out;
}

// ─── Step 2: extract error-string terms from the query ─────────────────────

// Pull out things that are likely to appear verbatim in source:
// - quoted strings
// - "exit code N" / "os error N" / "0x..."
// - backticked identifiers
// - known keyword tokens (game names, process names)
function extractErrorTerms(query) {
  const terms = new Set();
  const q = query || '';

  // Quoted ("..." or '...')
  for (const m of q.matchAll(/["']([^"']{3,60})["']/g)) {
    terms.add(m[1]);
  }
  // Error codes (named: "exit code N", "os error N", etc.)
  for (const m of q.matchAll(/\b(exit|os error|error)\s+(code\s+)?(-?\d+|0x[0-9a-fA-F]+)\b/gi)) {
    terms.add(m[0]);
  }
  // Standalone hex codes (6+ hex digits — typical Windows exception codes)
  for (const m of q.matchAll(/\b(0x[0-9a-fA-F]{6,})\b/g)) {
    terms.add(m[1]);
  }
  // Backticked identifiers
  for (const m of q.matchAll(/`([^`]{2,60})`/g)) {
    terms.add(m[1]);
  }
  // Known process/game tokens (case-insensitive)
  const KEYWORDS = [
    'F1_25.exe', 'acs.exe', 'AssettoCorsa', 'iRacing',
    'steam dialog', 'vguiPopupWindow', 'EA Anti-Cheat',
    'orphan', 'taskkill', 'python.ini', 'apps-default.ini',
    'GetExitCodeProcess', 'session_enforcer', 'ai_debugger',
    'spawn_safe', 'pre_flight', 'steam_checks',
  ];
  const lower = q.toLowerCase();
  for (const k of KEYWORDS) {
    if (lower.includes(k.toLowerCase())) terms.add(k);
  }
  return [...terms];
}

// ─── Step 3: grep each candidate, keep only matching files ─────────────────

function readFileSafe(repoRelPath, repoRoot) {
  try {
    const abs = path.join(repoRoot || REPO_ROOT, repoRelPath);
    const stat = fs.statSync(abs);
    if (!stat.isFile()) return null;
    if (stat.size > MAX_FILE_BYTES) {
      return { path: repoRelPath, content: null, size: stat.size, tooLarge: true };
    }
    return { path: repoRelPath, content: fs.readFileSync(abs, 'utf8'), size: stat.size };
  } catch (e) {
    return null;
  }
}

function grepNarrow(files, terms, opts = {}) {
  const repoRoot = opts.repoRoot || REPO_ROOT;
  const kept = [];
  const excluded = { not_found: [], too_large: [], not_matched: [] };
  for (const p of files) {
    const f = readFileSafe(p, repoRoot);
    if (!f) {
      excluded.not_found.push(p);
      continue;
    }
    if (f.tooLarge) {
      excluded.too_large.push(p);
      continue;
    }
    if (terms.length === 0) {
      // No terms to narrow by — include everything with matched=null.
      kept.push({ ...f, matches: null, match_count: 0 });
      continue;
    }
    const lower = f.content.toLowerCase();
    const matches = terms.filter((t) => lower.includes(t.toLowerCase()));
    if (matches.length === 0) {
      excluded.not_matched.push(p);
      continue;
    }
    kept.push({ ...f, matches, match_count: matches.length });
  }
  return { kept, excluded };
}

// ─── Step 4: token-budget cap ──────────────────────────────────────────────

function estimateTokens(text) {
  return Math.ceil(text.length / 4);
}

function applyTokenCap(files, budget) {
  // Sort by match_count descending so dense matches come first.
  const sorted = [...files].sort((a, b) => (b.match_count || 0) - (a.match_count || 0));
  const included = [];
  const truncated = [];
  let total = 0;
  for (const f of sorted) {
    const tokens = estimateTokens(f.content);
    if (total + tokens <= budget) {
      included.push(f);
      total += tokens;
    } else {
      truncated.push(f.path);
    }
  }
  return { included, truncated, tokens_estimated: total };
}

// ─── Step 5: annotate each file with why_selected ──────────────────────────

function annotate(files, symptomClasses) {
  return files.map((f) => {
    const whyParts = [];
    whyParts.push(`class=${symptomClasses.join(',')}`);
    if (f.matches && f.matches.length > 0) {
      whyParts.push(`matched=[${f.matches.slice(0, 5).join(', ')}]`);
    }
    return {
      path: f.path,
      content: f.content,
      why_selected: whyParts.join(' '),
    };
  });
}

// ─── Public: orchestrator ──────────────────────────────────────────────────

function prepareQueryRelevantBundle(query, symptomClasses, options = {}) {
  const budget = options.tokenBudget || DEFAULT_TOKEN_BUDGET;
  const mapPath = options.mapPath;

  const candidatePaths = filesForClasses(symptomClasses || [], mapPath);
  const terms = extractErrorTerms(query);
  const { kept, excluded } = grepNarrow(candidatePaths, terms, options);
  const { included, truncated, tokens_estimated } = applyTokenCap(kept, budget);
  const annotated = annotate(included, symptomClasses || []);

  return {
    files: annotated,
    tokens_estimated,
    symptom_classes_used: symptomClasses || [],
    terms_extracted: terms,
    candidate_paths: candidatePaths,
    excluded_reasons: { ...excluded, token_cap: truncated },
  };
}

module.exports = {
  filesForClasses,
  extractErrorTerms,
  grepNarrow,
  applyTokenCap,
  annotate,
  prepareQueryRelevantBundle,
  estimateTokens,
};

// ─── CLI entry: print a bundle preview (for manual inspection) ────────────

if (require.main === module) {
  const args = process.argv.slice(2);
  if (args.length === 0) {
    console.error('usage: node scripts/diagnose/bundle-helper.js <symptom-class>[,<class>...] [query]');
    console.error('  e.g. node scripts/diagnose/bundle-helper.js steam-dialog "pod_3 steam dialog 60s"');
    process.exit(64);
  }
  const classes = args[0].split(',').map((s) => s.trim()).filter(Boolean);
  const query = args.slice(1).join(' ');
  const bundle = prepareQueryRelevantBundle(query, classes);
  const preview = {
    files_included: bundle.files.map((f) => ({
      path: f.path,
      why: f.why_selected,
      bytes: f.content.length,
    })),
    tokens_estimated: bundle.tokens_estimated,
    candidate_paths: bundle.candidate_paths,
    excluded: bundle.excluded_reasons,
    terms_extracted: bundle.terms_extracted,
  };
  console.log(JSON.stringify(preview, null, 2));
}
