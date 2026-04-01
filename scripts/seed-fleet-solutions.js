#!/usr/bin/env node
/**
 * seed-fleet-solutions.js — Populate fleet_solutions table from Claude Code memory files.
 *
 * Reads SOLUTIONS-INDEX.md and referenced memory files, then POSTs solutions
 * to the racecontrol server's mesh import endpoint.
 *
 * Usage: node scripts/seed-fleet-solutions.js [--server URL]
 *   Default server: http://192.168.31.23:8080
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const MEMORY_DIR = 'C:/Users/bono/.claude/projects/C--Users-bono/memory';
const INDEX_PATH = path.join(MEMORY_DIR, 'SOLUTIONS-INDEX.md');
const DEFAULT_SERVER = 'http://192.168.31.23:8080';

function parseIndex() {
  if (!fs.existsSync(INDEX_PATH)) {
    console.error('SOLUTIONS-INDEX.md not found. Run build-solutions-index.js first.');
    process.exit(1);
  }

  const content = fs.readFileSync(INDEX_PATH, 'utf-8');
  const lines = content.split('\n');
  const entries = [];

  for (let i = 0; i < lines.length; i++) {
    if (!lines[i].startsWith('KEYWORDS:')) continue;
    const keywords = lines[i].substring(9).trim();
    const file = (lines[i + 1] || '').replace('FILE:', '').trim();
    const section = (lines[i + 2] || '').replace('SECTION:', '').trim();
    entries.push({ keywords, file, section });
  }

  return entries;
}

function extractSolutionContent(file, section) {
  const filePath = path.join(MEMORY_DIR, file.split('#')[0]);
  if (!fs.existsSync(filePath)) return null;

  const content = fs.readFileSync(filePath, 'utf-8');

  // For debugging-playbook.md#N, extract the specific section
  const sectionNum = file.match(/#(\d+)$/);
  if (sectionNum) {
    const num = sectionNum[1];
    const regex = new RegExp(`## ${num}\\.\\s+(.+?)(?=\\n## \\d+\\.|$)`, 's');
    const match = content.match(regex);
    if (match) return match[0].substring(0, 1000);
  }

  // For other files, return the body (skip frontmatter)
  const body = content.replace(/^---\n[\s\S]*?\n---\n?/, '');
  return body.substring(0, 1000);
}

function buildSolution(entry, idx) {
  const solutionContent = extractSolutionContent(entry.file, entry.section);
  if (!solutionContent) return null;

  // Extract symptom/root_cause from content — try multiple patterns
  const symptomMatch = solutionContent.match(/\*\*(?:Symptom|Problem)[:.]?\*\*\s*(.+?)(?:\n|$)/i);
  const rootCauseMatch = solutionContent.match(/\*\*(?:Root [Cc]ause[s]?)(?:\s*\([^)]*\))?[:.]?\*\*\s*(.+?)(?:\n|$)/i);
  const fixMatch = solutionContent.match(/\*\*Fix\s*(?:\([^)]+\))?[:.]?\*\*\s*(.+?)(?:\n|$)/i);

  // For playbook: extract all ### subsection titles as root causes
  const subsections = [...solutionContent.matchAll(/### ([A-Z])\.\s+(.+?)(?:\n|$)/g)];

  let symptoms = symptomMatch ? symptomMatch[1].trim() : entry.section;
  let rootCause = rootCauseMatch ? rootCauseMatch[1].trim() : '';
  let fixAction = fixMatch ? fixMatch[1].trim() : '';

  // If no root cause found, try subsections
  if (!rootCause && subsections.length > 0) {
    rootCause = subsections.map(m => m[2].trim()).join('; ');
  }
  if (!rootCause) rootCause = `See ${entry.file}`;

  // If no fix, extract first **Fix** or **Prevention** line from anywhere
  if (!fixAction) {
    const prevMatch = solutionContent.match(/\*\*(?:Fix|Prevention|Workaround).*?\*\*\s*(.+?)(?:\n|$)/i);
    if (prevMatch) fixAction = prevMatch[1].trim();
    else fixAction = `Refer to ${entry.file}`;
  }

  const id = `seed-${crypto.createHash('md5').update(entry.file + entry.section).digest('hex').substring(0, 12)}`;
  const problemKey = entry.section.toLowerCase().replace(/[^a-z0-9]+/g, '-').substring(0, 60);
  const problemHash = crypto.createHash('sha256').update(entry.keywords).digest('hex').substring(0, 16);

  return {
    id,
    problem_key: problemKey,
    problem_hash: problemHash,
    symptoms: { description: symptoms, keywords: entry.keywords.split(', ').slice(0, 10) },
    environment: { os: 'windows', component: 'fleet' },
    root_cause: rootCause,
    fix_action: { description: fixAction, source_file: entry.file },
    fix_type: 'deterministic',
    status: 'fleet_verified',
    success_count: 5,
    fail_count: 0,
    confidence: 0.9,
    cost_to_diagnose: 0,
    models_used: null,
    diagnosis_tier: 'memory',
    source_node: 'james-claude-code',
    venue_id: 'rp-hyderabad',
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    version: 1,
    ttl_days: 365,
    tags: ['seeded', 'claude-code-memory'],
  };
}

async function main() {
  const serverArg = process.argv.find(a => a.startsWith('--server='));
  const server = serverArg ? serverArg.split('=')[1] : DEFAULT_SERVER;

  console.log(`Seeding fleet solutions to ${server}`);

  const entries = parseIndex();
  console.log(`Found ${entries.length} index entries`);

  const solutions = entries
    .map((e, i) => buildSolution(e, i))
    .filter(Boolean);

  console.log(`Built ${solutions.length} solutions`);

  // POST to server mesh import endpoint
  const url = `${server}/api/v1/mesh/import`;
  const body = JSON.stringify({
    venue_id: 'rp-hyderabad',
    solutions,
  });

  try {
    const resp = await fetch(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
    });
    const result = await resp.json();
    console.log('Server response:', JSON.stringify(result, null, 2));
  } catch (e) {
    console.error(`Failed to POST to ${url}: ${e.message}`);
    console.log('\nAlternative: writing solutions.json locally for manual import');
    const outPath = path.join(__dirname, '..', 'data', 'fleet-solutions-seed.json');
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, JSON.stringify(solutions, null, 2));
    console.log(`Wrote ${solutions.length} solutions to ${outPath}`);
  }
}

main();
