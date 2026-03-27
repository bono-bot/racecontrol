#!/usr/bin/env node
// scripts/cross-model-analysis.js — Cross-reference findings from multiple model audits
//
// Usage: node scripts/cross-model-analysis.js
// Reads: audit/results/*-audit-YYYY-MM-DD/
// Output: audit/results/cross-model-report-YYYY-MM-DD/

const fs = require('fs');
const path = require('path');

const REPO_ROOT = path.resolve(__dirname, '..');
const RESULTS_DIR = path.join(REPO_ROOT, 'audit', 'results');
const dateStr = new Date().toISOString().split('T')[0];

// ─── Find all audit directories for today ────────────────────────────────────
function findAuditDirs() {
  const dirs = fs.readdirSync(RESULTS_DIR, { withFileTypes: true })
    .filter(d => d.isDirectory() && d.name.endsWith(`-audit-${dateStr}`))
    .map(d => ({
      name: d.name.replace(`-audit-${dateStr}`, ''),
      path: path.join(RESULTS_DIR, d.name)
    }));
  return dirs;
}

// ─── Parse findings from a model's report ────────────────────────────────────
function parseFindingsFromFile(filePath, modelName) {
  const content = fs.readFileSync(filePath, 'utf-8');
  const findings = [];

  // Match finding blocks — look for SEVERITY + CATEGORY + FILE + FINDING patterns
  const lines = content.split('\n');
  let current = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();

    // Detect severity line
    const sevMatch = line.match(/\*?\*?SEVERITY\*?\*?:?\s*(P[123])/i);
    if (sevMatch) {
      if (current && current.finding) findings.push(current);
      current = { model: modelName, severity: sevMatch[1].toUpperCase(), lineNum: i };
      continue;
    }

    if (!current) continue;

    const catMatch = line.match(/\*?\*?CATEGORY\*?\*?:?\s*(\S+)/i);
    if (catMatch) { current.category = catMatch[1].replace(/[*`]/g, '').toLowerCase(); continue; }

    const fileMatch = line.match(/\*?\*?FILE\*?\*?:?\s*[`]?([^\s`*]+)[`]?/i);
    if (fileMatch) { current.file = fileMatch[1].replace(/[`*]/g, ''); continue; }

    const lineMatch = line.match(/\*?\*?LINE\*?\*?:?\s*~?(\d+)/i);
    if (lineMatch) { current.approxLine = parseInt(lineMatch[1]); continue; }

    const findMatch = line.match(/\*?\*?FINDING\*?\*?:?\s*(.+)/i);
    if (findMatch) { current.finding = findMatch[1].replace(/\*\*/g, '').trim(); continue; }

    const impactMatch = line.match(/\*?\*?IMPACT\*?\*?:?\s*(.+)/i);
    if (impactMatch) { current.impact = impactMatch[1].replace(/\*\*/g, '').trim(); continue; }

    const fixMatch = line.match(/\*?\*?FIX\*?\*?:?\s*(.+)/i);
    if (fixMatch) { current.fix = fixMatch[1].replace(/\*\*/g, '').trim(); continue; }
  }

  if (current && current.finding) findings.push(current);
  return findings;
}

// ─── Similarity score between two findings ───────────────────────────────────
function similarity(a, b) {
  // Same file is a strong signal
  if (!a.file || !b.file) return 0;

  const fileA = a.file.split('/').pop();
  const fileB = b.file.split('/').pop();
  if (fileA !== fileB) return 0;

  // Same file — check line proximity
  let score = 0.4;
  if (a.approxLine && b.approxLine && Math.abs(a.approxLine - b.approxLine) < 30) {
    score += 0.3;
  }

  // Check keyword overlap in finding text
  if (a.finding && b.finding) {
    const wordsA = new Set(a.finding.toLowerCase().split(/\s+/).filter(w => w.length > 4));
    const wordsB = new Set(b.finding.toLowerCase().split(/\s+/).filter(w => w.length > 4));
    const intersection = [...wordsA].filter(w => wordsB.has(w));
    const union = new Set([...wordsA, ...wordsB]);
    if (union.size > 0) {
      score += 0.3 * (intersection.length / union.size);
    }
  }

  return score;
}

// ─── Group similar findings across models ────────────────────────────────────
function groupFindings(allFindings) {
  const groups = [];
  const assigned = new Set();

  for (let i = 0; i < allFindings.length; i++) {
    if (assigned.has(i)) continue;

    const group = [allFindings[i]];
    assigned.add(i);

    for (let j = i + 1; j < allFindings.length; j++) {
      if (assigned.has(j)) continue;
      if (allFindings[i].model === allFindings[j].model) continue; // different models only

      if (similarity(allFindings[i], allFindings[j]) > 0.5) {
        group.push(allFindings[j]);
        assigned.add(j);
      }
    }

    groups.push(group);
  }

  return groups;
}

// ─── Main ────────────────────────────────────────────────────────────────────
function main() {
  const auditDirs = findAuditDirs();
  console.log(`Found ${auditDirs.length} audit directories for ${dateStr}:`);
  auditDirs.forEach(d => console.log(`  - ${d.name} (${d.path})`));

  if (auditDirs.length < 2) {
    console.error('Need at least 2 model audit directories to cross-reference.');
    process.exit(1);
  }

  // Parse all findings
  const allFindings = [];
  const modelStats = {};

  for (const dir of auditDirs) {
    const reportPath = path.join(dir.path, 'FULL-AUDIT-REPORT.md');
    if (!fs.existsSync(reportPath)) {
      console.warn(`  Skipping ${dir.name}: no FULL-AUDIT-REPORT.md`);
      continue;
    }

    const findings = parseFindingsFromFile(reportPath, dir.name);
    console.log(`  ${dir.name}: ${findings.length} findings parsed`);
    allFindings.push(...findings);

    modelStats[dir.name] = {
      total: findings.length,
      p1: findings.filter(f => f.severity === 'P1').length,
      p2: findings.filter(f => f.severity === 'P2').length,
      p3: findings.filter(f => f.severity === 'P3').length,
    };
  }

  console.log(`\nTotal findings across all models: ${allFindings.length}`);

  // Group similar findings
  const groups = groupFindings(allFindings);

  // Classify groups
  const consensus = groups.filter(g => g.length >= 3).sort((a, b) => {
    const sevOrder = { P1: 0, P2: 1, P3: 2 };
    return (sevOrder[a[0].severity] || 3) - (sevOrder[b[0].severity] || 3);
  });
  const twoModels = groups.filter(g => g.length === 2).sort((a, b) => {
    const sevOrder = { P1: 0, P2: 1, P3: 2 };
    return (sevOrder[a[0].severity] || 3) - (sevOrder[b[0].severity] || 3);
  });
  const unique = groups.filter(g => g.length === 1).sort((a, b) => {
    const sevOrder = { P1: 0, P2: 1, P3: 2 };
    return (sevOrder[a[0].severity] || 3) - (sevOrder[b[0].severity] || 3);
  });

  // Generate report
  const outDir = path.join(RESULTS_DIR, `cross-model-report-${dateStr}`);
  fs.mkdirSync(outDir, { recursive: true });

  let report = `# Cross-Model Audit Report — Racing Point eSports\n\n`;
  report += `**Date:** ${new Date().toISOString()}\n`;
  report += `**Models:** ${auditDirs.map(d => d.name).join(', ')}\n`;
  report += `**Total Findings:** ${allFindings.length} (across ${auditDirs.length} models)\n`;
  report += `**Grouped:** ${groups.length} unique issues\n\n`;

  // Model comparison table
  report += `## Model Comparison\n\n`;
  report += `| Model | Total | P1 | P2 | P3 |\n|---|---|---|---|---|\n`;
  for (const [model, stats] of Object.entries(modelStats)) {
    report += `| ${model} | ${stats.total} | ${stats.p1} | ${stats.p2} | ${stats.p3} |\n`;
  }

  // Summary
  report += `\n## Finding Categories\n\n`;
  report += `| Category | Count | Description |\n|---|---|---|\n`;
  report += `| **Consensus (3+ models)** | ${consensus.length} | High confidence — multiple models agree |\n`;
  report += `| **Two models agree** | ${twoModels.length} | Medium confidence — corroborated by 2 |\n`;
  report += `| **Unique (1 model only)** | ${unique.length} | Most valuable — what other models missed |\n`;

  // Consensus findings
  report += `\n---\n\n## Consensus Findings (${consensus.length}) — 3+ Models Agree\n\n`;
  for (let i = 0; i < consensus.length; i++) {
    const group = consensus[i];
    const rep = group[0]; // representative
    const models = group.map(f => f.model).join(', ');
    report += `### C${i + 1}. [${rep.severity}] ${rep.file || 'unknown'}\n`;
    report += `**Models:** ${models}\n`;
    report += `**Finding:** ${rep.finding || 'N/A'}\n`;
    report += `**Impact:** ${rep.impact || 'N/A'}\n`;
    report += `**Fix:** ${rep.fix || 'N/A'}\n\n`;
  }

  // Two-model findings
  report += `\n---\n\n## Two-Model Findings (${twoModels.length}) — Corroborated\n\n`;
  for (let i = 0; i < twoModels.length; i++) {
    const group = twoModels[i];
    const rep = group[0];
    const models = group.map(f => f.model).join(', ');
    report += `### T${i + 1}. [${rep.severity}] ${rep.file || 'unknown'}\n`;
    report += `**Models:** ${models}\n`;
    report += `**Finding:** ${rep.finding || 'N/A'}\n\n`;
  }

  // Unique findings — THE GOLD
  report += `\n---\n\n## Unique Findings (${unique.length}) — Single Model Only\n\n`;
  report += `> These are the most valuable findings — issues caught by only one model that all others missed.\n`;
  report += `> They need Opus review to separate real issues from false positives.\n\n`;

  const uniqueByModel = {};
  for (const group of unique) {
    const f = group[0];
    if (!uniqueByModel[f.model]) uniqueByModel[f.model] = [];
    uniqueByModel[f.model].push(f);
  }

  for (const [model, findings] of Object.entries(uniqueByModel)) {
    report += `### ${model} — ${findings.length} unique findings\n\n`;
    for (let i = 0; i < findings.length; i++) {
      const f = findings[i];
      report += `${i + 1}. **[${f.severity}] ${f.category || '?'}** — ${f.file || '?'}:${f.approxLine || '?'}\n`;
      report += `   ${f.finding || 'N/A'}\n\n`;
    }
  }

  // Write report
  const reportPath = path.join(outDir, 'CROSS-MODEL-REPORT.md');
  fs.writeFileSync(reportPath, report);

  // Also write raw JSON for programmatic use
  const jsonPath = path.join(outDir, 'findings.json');
  fs.writeFileSync(jsonPath, JSON.stringify({
    date: dateStr,
    models: Object.keys(modelStats),
    modelStats,
    totalFindings: allFindings.length,
    groupedIssues: groups.length,
    consensus: consensus.map(g => ({ models: g.map(f => f.model), finding: g[0] })),
    twoModels: twoModels.map(g => ({ models: g.map(f => f.model), finding: g[0] })),
    unique: unique.map(g => g[0]),
    allFindings
  }, null, 2));

  console.log(`\n=== CROSS-MODEL ANALYSIS COMPLETE ===`);
  console.log(`Report: ${reportPath}`);
  console.log(`JSON:   ${jsonPath}`);
  console.log(`\nConsensus (3+): ${consensus.length}`);
  console.log(`Two models:    ${twoModels.length}`);
  console.log(`Unique:        ${unique.length}`);
}

main();
