#!/usr/bin/env node
/**
 * Fleet Intelligence Weekly Report — v26.0 Phase 228
 *
 * Queries mesh intelligence data and sends a formatted report to Uday via WhatsApp.
 * Run weekly via cron: Sunday 10:00 IST
 *
 * Usage: node scripts/fleet-report.js [--dry-run]
 */

const RC_URL = process.env.RC_URL || 'http://localhost:8080';
const EVOLUTION_URL = process.env.EVOLUTION_URL || 'http://localhost:8081';
const EVOLUTION_INSTANCE = process.env.EVOLUTION_INSTANCE || 'RacingPoint';
const EVOLUTION_KEY = process.env.EVOLUTION_APIKEY || '';
const UDAY_NUMBER = '917981264279';

async function fetchJson(url) {
  const res = await fetch(url, { signal: AbortSignal.timeout(10000) });
  if (!res.ok) throw new Error(`${url}: ${res.status}`);
  return res.json();
}

async function generateReport() {
  // Fetch mesh data
  const [stats, solData, incData] = await Promise.all([
    fetchJson(`${RC_URL}/api/v1/mesh/stats`),
    fetchJson(`${RC_URL}/api/v1/mesh/solutions?limit=100`),
    fetchJson(`${RC_URL}/api/v1/mesh/incidents?limit=100`),
  ]);

  const solutions = solData.solutions || [];
  const incidents = incData.incidents || [];

  // Calculate metrics
  const totalSolutions = stats.total_solutions || 0;
  const byStatus = stats.by_status || {};

  // MTTR (Mean Time To Resolve) — only for resolved incidents
  const resolved = incidents.filter(i => i.time_to_resolve_secs != null);
  const avgTTR = resolved.length > 0
    ? Math.round(resolved.reduce((sum, i) => sum + i.time_to_resolve_secs, 0) / resolved.length)
    : null;

  // Total diagnosis cost
  const totalCost = incidents.reduce((sum, i) => sum + (i.cost || 0), 0);

  // Resolution by tier
  const tierCounts = {};
  for (const inc of resolved) {
    const tier = inc.resolved_by_tier || 'unknown';
    tierCounts[tier] = (tierCounts[tier] || 0) + 1;
  }

  // New solutions this week
  const weekAgo = new Date(Date.now() - 7 * 24 * 60 * 60 * 1000).toISOString();
  const newThisWeek = solutions.filter(s => s.created_at > weekAgo);
  const promotedThisWeek = solutions.filter(s => s.status === 'fleet_verified' && s.updated_at > weekAgo);
  const hardenedThisWeek = solutions.filter(s => s.status === 'hardened' && s.updated_at > weekAgo);

  // Top problems
  const problemCounts = {};
  for (const inc of incidents) {
    problemCounts[inc.problem_key] = (problemCounts[inc.problem_key] || 0) + 1;
  }
  const topProblems = Object.entries(problemCounts)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5);

  // Format report
  const lines = [
    `*Fleet Intelligence Report*`,
    `Week ending ${new Date().toLocaleDateString('en-IN', { timeZone: 'Asia/Kolkata', dateStyle: 'medium' })}`,
    ``,
    `*Knowledge Base*`,
    `Total solutions: ${totalSolutions}`,
    `  Candidate: ${byStatus.candidate || 0}`,
    `  Fleet verified: ${byStatus.fleet_verified || 0}`,
    `  Hardened: ${byStatus.hardened || 0}`,
    `  New this week: ${newThisWeek.length}`,
    `  Promoted: ${promotedThisWeek.length}`,
    `  Hardened: ${hardenedThisWeek.length}`,
    ``,
    `*Incidents*`,
    `Total: ${incidents.length}`,
    `Resolved: ${resolved.length}`,
    `MTTR: ${avgTTR != null ? `${avgTTR}s` : 'N/A'}`,
    `Total diagnosis cost: $${totalCost.toFixed(2)}`,
    ``,
    `*Resolution by Tier*`,
  ];

  const tierLabels = {
    deterministic: 'Tier 1 (Deterministic)',
    knowledge_base: 'Tier 2 (KB Lookup)',
    single_model: 'Tier 3 (Single Model)',
    multi_model: 'Tier 4 (Multi-Model)',
    human: 'Tier 5 (Human)',
  };
  for (const [tier, count] of Object.entries(tierCounts).sort((a, b) => b[1] - a[1])) {
    lines.push(`  ${tierLabels[tier] || tier}: ${count}`);
  }

  if (topProblems.length > 0) {
    lines.push('', '*Top Problems*');
    for (const [key, count] of topProblems) {
      lines.push(`  ${key}: ${count}x`);
    }
  }

  lines.push('', `_Generated ${new Date().toLocaleString('en-IN', { timeZone: 'Asia/Kolkata' })}_`);

  return lines.join('\n');
}

async function sendWhatsApp(message) {
  if (!EVOLUTION_KEY) {
    console.log('No EVOLUTION_APIKEY set — printing report instead:\n');
    console.log(message);
    return;
  }

  const url = `${EVOLUTION_URL}/message/sendText/${EVOLUTION_INSTANCE}`;
  const res = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'apikey': EVOLUTION_KEY,
    },
    body: JSON.stringify({
      number: UDAY_NUMBER,
      text: message,
    }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`WhatsApp send failed: ${res.status} ${body}`);
  }
  console.log('Report sent to Uday via WhatsApp');
}

async function main() {
  const dryRun = process.argv.includes('--dry-run');

  try {
    const report = await generateReport();

    if (dryRun) {
      console.log('=== DRY RUN — Report Preview ===\n');
      console.log(report);
      return;
    }

    await sendWhatsApp(report);
  } catch (err) {
    console.error('Fleet report error:', err.message);
    process.exit(1);
  }
}

main();
