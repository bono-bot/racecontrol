#!/usr/bin/env node
// MMA Step 1 DIAGNOSE on §S-146 RCA for racecontrol#54
// Foundational-boundary escalation (billing) → 5-model OpenRouter, ≥3 vendor families
// Surface: MMA-PR54-RCA-DIAGNOSE-bono-2026-05-11

import fs from 'node:fs';

const KEY = fs.readFileSync('/root/comms-link/data/openrouter-key.txt', 'utf8').trim();
const SPEND_LEDGER = '/root/comms-link/data/openrouter-spend-bono.jsonl';
const RESULTS_DIR = '/tmp/mma-pr54-results';
fs.mkdirSync(RESULTS_DIR, { recursive: true });

const MODELS = [
  { id: 'deepseek/deepseek-r1-0528',           vendor: 'deepseek',  role: 'reasoner' },
  { id: 'qwen/qwen3-coder',                    vendor: 'qwen',      role: 'code_expert' },
  { id: 'nvidia/nemotron-3-super-120b-a12b',   vendor: 'nvidia',    role: 'sre' },
  { id: 'google/gemini-2.5-pro',               vendor: 'google',    role: 'generalist' },
  { id: 'moonshotai/kimi-k2.5',                vendor: 'moonshot',  role: 'reasoner_alt' },
];

const SURFACE = 'MMA-PR54-RCA-DIAGNOSE-bono-2026-05-11';

const RCA_CONTENT = fs.readFileSync(
  '/root/racecontrol/.planning/specs/v2/RCA/PR54-PACT-013-billing-paused-config-push-queue-20260511.md',
  'utf8'
);

const PROMPT = `You are an expert reviewer in a 5-model Multi-Model Audit (MMA) performing Step 1 DIAGNOSE on a Root-Cause Analysis (RCA) document for a racing-simulator software ecosystem.

Context:
- "RacingPoint" is a multi-pod racing-simulator venue with two AI pilots ("bono" / Linux VPS + "james" / Windows on-site). They coordinate via a sqlite + WebSocket comms-link daemon.
- "V2" is the current architectural target (replaces V1). Heavy doctrine: PACT framework + CGP gates + Captain (user) authority.
- §S-146 is a recently-ratified rule (2026-05-09): any V2 code change that inherits/calls into/reuses V1 schema/state must produce a written 5-section RCA before action. Foundational boundaries (billing/wallet/auth/pod-state-channel/WhatsApp identity/DB schema) escalate to MMA Step 1 DIAGNOSE on the RCA itself.
- §S-172 (2026-05-10) extends §S-146: when a V2 fix depends on shared infrastructure (delivery/transport/supervision/guard/observability/schema), run a 5-question mechanism-trust-check on the infrastructure surface first.
- §S-186 (2026-05-11) is a fast-lane carve-out for pre-§S-146 small bug-fixes (≤200 LOC, single-boundary, no schema/protocol change, bug-fix only) — gets a 3-section short-RCA instead of full 5-section. PR #54 does NOT qualify (feat-class, multi-boundary, protocol change).

The PR under review:
- racecontrol#54 — "feat(billing): route billing_paused via config_push_queue (PACT-013 Phase 1+2)"
- Removes PR #49's CoreToAgentMessage::BillingPaused/Resumed wire variants + routes through Phase 177 config_push_queue substrate (V2-P1-CONFIG-SERVICE).
- +128/-91 LOC across 5 files (protocol.rs, billing_session_lifecycle.rs, ws_handler.rs, CLAUDE.md, .gitignore).
- Bilaterally ratified as PACT-20260429-013 2026-04-29 (Phase 0+0.5 GREEN-LIGHT, all 3 substrate tables verified on .23 venue DB). CI all green 2026-04-30.
- Idle 12 days waiting for §S-146 retroactive RCA + Captain per-PR merge auth.

The RCA document below is what bono just authored (~12:30 IST 2026-05-11). Your job: Step 1 DIAGNOSE — find root-cause-class gaps, missing inherited issues, incorrect dispositions, or weak V2-alignment claims. Use the structured output format below.

================================================================================
RCA DOCUMENT START
================================================================================
${RCA_CONTENT}
================================================================================
RCA DOCUMENT END
================================================================================

Output requirements (use EXACT section markers):

=== Q1 — RCA-COMPLETENESS ===

A1.VERDICT: PASS / AGREE-WITH-CAVEATS / FAIL
A1.MISSING-INHERITED-ISSUES: list any V1-shape footguns or inherited issues the RCA failed to catalog (or "none" if §2 is complete). Be specific — file:line citations if you have them, otherwise concrete category names.
A1.MISSING-PAST-BUGS: list any past bugs at this boundary that the RCA's §3 disposition missed (or "none").
A1.V2-ALIGNMENT-GAPS: list any V2 doctrine anchors §4 should have cited but didn't (or "none").

=== Q2 — DELIVERY-SUBSTRATE-SOUNDNESS ===

The PR moves server→agent billing_paused control plane from an ad-hoc wire variant (PR #49 fire-and-forget) to the Phase 177 config_push_queue substrate (DB-persisted + seq_num + ack + audit_log). The RCA claims this is strictly stronger on every axis.

A2.AGREE-DISAGREE: AGREE / AGREE-WITH-CAVEATS / DISAGREE
A2.MISSED-FAILURE-MODES: list any failure modes of the new substrate path the RCA did not surface (e.g., what happens if the DB write succeeds but the WS send fails AND the pod never reconnects? what happens if seq_num overflows? what about row-state-machine inconsistency under partial commit?).
A2.STRONGER-THAN-PR49?-CONCRETE: name 2 concrete adversarial scenarios where the new path is provably stronger than PR #49's path (and 1 scenario, if any, where it might be weaker or equivalent).

=== Q3 — V1↔V2 BOUNDARY DISPOSITION ===

The RCA claims PR #54 is "V2→V2 substrate refactor" with V1 inheritance only at the upstream billing_session DB table (which the PR does not modify). The §3 past-bug review dispositions PR #49 / PACT-008 as "PATCHED-ONLY — closed by THIS PR."

A3.CORRECT-DISPOSITION?: YES / NO + reason
A3.RISK-OF-PATCH-V1-FORWARD-CLASSIFICATION: assess whether this PR is an example of the "patch V1 forward" antipattern §S-146 was authored to prevent. Concrete reasoning.
A3.HOT_RELOAD_FIELDS-ALLOWLIST-RETENTION: the RCA retains the hardcoded HOT_RELOAD_FIELDS allowlist as "kaizen-correct temporary V1 retention" with retire trigger "when ≥3 hot-reload fields exist." Is this justification sound?

=== Q4 — MERGE DISPOSITION RECOMMENDATION ===

A4.MERGE-DECISION: MERGE / MERGE-WITH-AMENDMENTS / BLOCK
A4.REQUIRED-PRE-MERGE-WORK: list ordered (or "none"). Be specific: integration test, Bono-VPS parity, fleet-roll sequence, additional RCA section, etc.
A4.RECOMMENDED-PR-COMMENT-TEXT: one paragraph (~150 words) the Captain could paste verbatim as the per-PR merge auth disposition.

=== Q5 — MMA-CONSENSUS-NOTE ===

A5.ONE-LINE-SUMMARY: <30 words on whether this RCA + PR meets §S-146 doctrine bar for foundational-boundary class.

=== END ===

Limit each answer to ~300 words. Be concrete, not abstract. Cite specific file:line or doctrine anchors where possible. State trade-offs explicitly.
`;

async function callModel(model) {
  const start = Date.now();
  try {
    const r = await fetch('https://openrouter.ai/api/v1/chat/completions', {
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${KEY}`,
        'Content-Type': 'application/json',
        'HTTP-Referer': 'https://racingpoint.in',
        'X-Title': SURFACE,
      },
      body: JSON.stringify({
        model: model.id,
        messages: [{ role: 'user', content: PROMPT }],
        max_tokens: 4000,
        temperature: 0.2,
      }),
    });
    const data = await r.json();
    const elapsed = Date.now() - start;
    const content = data?.choices?.[0]?.message?.content || '';
    const usage = data?.usage || {};
    const cost = data?.usage?.cost ?? 0;
    return { model, content, usage, cost, elapsed_ms: elapsed, ok: !!content, error: data?.error };
  } catch (e) {
    return { model, content: '', error: String(e), ok: false, elapsed_ms: Date.now() - start };
  }
}

const t0 = Date.now();
console.log(`MMA Step 1 DIAGNOSE | surface=${SURFACE} | models=${MODELS.length}`);
const results = await Promise.all(MODELS.map(callModel));
const elapsed = Date.now() - t0;

let totalCost = 0;
for (const r of results) {
  const file = `${RESULTS_DIR}/${r.model.vendor}.md`;
  const hdr = `# ${r.model.id}\n\nrole: ${r.model.role}\nvendor: ${r.model.vendor}\nok: ${r.ok}\nelapsed_ms: ${r.elapsed_ms}\ncost: ${r.cost}\nusage: ${JSON.stringify(r.usage)}\n\n---\n\n`;
  fs.writeFileSync(file, hdr + (r.content || `ERROR: ${r.error}`));
  totalCost += Number(r.cost) || 0;
  const ledgerEntry = {
    ts: new Date().toISOString(),
    surface: SURFACE,
    model: r.model.id,
    vendor: r.model.vendor,
    role: r.model.role,
    cost_usd: r.cost,
    tokens_in: r.usage?.prompt_tokens,
    tokens_out: r.usage?.completion_tokens,
    elapsed_ms: r.elapsed_ms,
    ok: r.ok,
    notes: 'MMA Step 1 DIAGNOSE on §S-146 RCA for racecontrol#54 PACT-013 Phase 1+2',
  };
  fs.appendFileSync(SPEND_LEDGER, JSON.stringify(ledgerEntry) + '\n');
}

const summary = {
  surface: SURFACE,
  elapsed_total_ms: elapsed,
  total_cost_usd: totalCost,
  models_ok: results.filter(r => r.ok).length,
  models_failed: results.filter(r => !r.ok).map(r => r.model.id),
  result_files: results.map(r => `${RESULTS_DIR}/${r.model.vendor}.md`),
};
fs.writeFileSync(`${RESULTS_DIR}/SUMMARY.json`, JSON.stringify(summary, null, 2));
console.log(JSON.stringify(summary, null, 2));
