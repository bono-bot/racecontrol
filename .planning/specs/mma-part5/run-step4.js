#!/usr/bin/env node
// MMA Part 5 Step 4 VERIFY runner — 3 parallel adversarial models review SHIPPED code
// against DESIGN-SPEC + Step 1 findings. Produces: verify/<short-name>.json + summary.
//
// Models must be DIFFERENT from Steps 1+2 (diversity rule). Steps 1-3 used:
//   deepseek-r1-0528, deepseek-chat-v3-0324, qwen3-coder, nemotron-3-super,
//   gemini-2.5-flash, grok-code-fast-1, codex-mini.
// Step 4 uses: kimi-k2 (Moonshot), mistral-medium (Mistral), mimo-v2-pro (Xiaomi).

const fs = require("fs").promises;
const fsSync = require("fs");
const https = require("https");
const path = require("path");

const ROOT = "C:/Users/bono/racingpoint/racecontrol";
const OUT_DIR = path.join(ROOT, ".planning/specs/mma-part5/verify");
const KEY_FILE = path.join(ROOT, "data/openrouter-mma-key.txt");

const API_KEY = fsSync.readFileSync(KEY_FILE, "utf8").trim();
if (!API_KEY) {
  console.error("ERROR: empty API key");
  process.exit(2);
}

const MODELS = [
  {
    id: "moonshotai/kimi-k2-0905",
    short: "kimi",
    price_prompt: 0.60,
    price_completion: 2.50,
    role: "reasoner-adversarial",
  },
  {
    id: "mistralai/mistral-medium-3.1",
    short: "mistral",
    price_prompt: 0.40,
    price_completion: 2.00,
    role: "code-expert-adversarial",
  },
  {
    id: "xiaomi/mimo-v2-pro",
    short: "mimo",
    price_prompt: 0.15,
    price_completion: 0.60,
    role: "sre-adversarial",
  },
];

async function loadContext() {
  const spec = await fs.readFile(
    path.join(ROOT, ".planning/specs/PATTERN-I-PART-5-HTTP-FALLBACK.md"),
    "utf8"
  );
  const findings = await fs.readFile(
    path.join(ROOT, ".planning/specs/mma-part5/FINDINGS-STEP1.md"),
    "utf8"
  );
  // Full session_end_fallback.rs (Commit 5 — HTTP fallback module)
  const module = await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/session_end_fallback.rs"),
    "utf8"
  );
  // ws_handler.rs lines 2325-2570 — Part 5 Commit 4 extraction (apply_session_ended + dedup)
  const wsh_full = await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/ws_handler.rs"),
    "utf8"
  );
  const wsh_part5 = wsh_full.split("\n").slice(2324, 2570).join("\n");
  // Commit 6 — fleet_health_api.rs insertions (86-line additive patch)
  const fha_full = await fs.readFile(
    path.join(ROOT, "crates/racecontrol/src/fleet_health_api.rs"),
    "utf8"
  );
  const fha_part5 = fha_full.split("\n").slice(100, 220).join("\n");

  return { spec, findings, module, wsh_part5, fha_part5 };
}

const JSON_SCHEMA = `{
  "verdict": "PASS|CONDITIONAL_PASS|BLOCK",
  "score_out_of_5": 0.0,
  "coverage_of_step1_findings": [
    { "finding_id": "C1|C2|...|C9|D3|D6|D9|D11|D13",
      "addressed_in_code": true|false,
      "evidence": "file:line or quote",
      "residual_risk": "none|low|medium|high",
      "note": "optional" }
  ],
  "new_defects_found": [
    { "defect_id": "your-choice, e.g. V-1",
      "severity": "P0|P1|P2|P3",
      "category": "correctness|race|resource|security|observability|deploy",
      "file_line": "crates/rc-agent/src/...:NN",
      "description": "1-3 sentences on what is wrong",
      "exploit_or_trigger": "how to reproduce or when it fires",
      "suggested_fix": "1-2 sentence fix hint",
      "confidence_1_to_5": 4 }
  ],
  "deploy_readiness": {
    "can_ship_as_is": true|false,
    "must_fix_before_ship": ["defect ids that must be addressed"],
    "nice_to_have": ["defect ids that can ship with follow-up commit"]
  },
  "adversarial_summary": "3-5 sentence executive summary of your strongest concern"
}`;

function buildPrompt(ctx) {
  return `You are an adversarial senior Rust reviewer performing MMA Step 4 VERIFY for Racing Point Pattern I Part 5. Your job: find defects the Step 1-3 consensus missed in the SHIPPED code.

This is NOT a sanity check. The Step 1 findings were addressed by the implementation — your task is to look for what those 9 consensus findings MISSED. Specifically target:
- Trace-level bugs (specific variable values at specific lines) rather than abstract patterns
- Concurrency/ordering bugs (lock ordering, cancellation, mpsc/watch channel behaviour)
- Failure injection bugs (what if reqwest timeout fires AFTER status check but BEFORE body parse?)
- Subtle semantics (saturating_duration_since edge cases, Instant monotonicity on Windows, reqwest default redirect behaviour, tokio::spawn lifecycle)
- Deploy/rollback bugs not covered by D3
- Test coverage gaps (what the 11 unit tests in session_end_fallback.rs + 4 in pattern_i_part5_tests do NOT exercise)

Emit output as STRICT JSON matching this schema:
${JSON_SCHEMA}

Scoring rubric (score_out_of_5):
- 5.0 = all Step 1 findings addressed, zero new defects, test coverage adequate
- 4.0 = all addressed, ≤2 P2 defects, ≤1 P1 defect → CONDITIONAL_PASS
- 3.0 = ≥1 P0 defect OR ≥3 P1 defects → BLOCK
- 2.0 = multiple P0s, systemic issue → BLOCK
- 1.0 = implementation does not match spec intent → BLOCK

MMA protocol: verdict PASS or CONDITIONAL_PASS with score ≥4.0 unblocks deploy. Lower score = deploy blocked.

Output ONLY the JSON. No prose before or after.

=== DESIGN-SPEC (PATTERN-I-PART-5-HTTP-FALLBACK.md, current, post-Step-1) ===
${ctx.spec}

=== STEP 1 DIAGNOSE FINDINGS (FINDINGS-STEP1.md) ===
${ctx.findings}

=== SHIPPED CODE: crates/rc-agent/src/session_end_fallback.rs (Commit 5 — HTTP fallback module, 362 LOC) ===
${ctx.module}

=== SHIPPED CODE: crates/rc-agent/src/ws_handler.rs lines 2325-2570 (Commit 4 — SessionEndOrigin + apply_session_ended + dedup guard) ===
${ctx.wsh_part5}

=== SHIPPED CODE: crates/racecontrol/src/fleet_health_api.rs lines 101-220 (Commit 6 — stuck_session_candidate flag, D3 rollback detection) ===
${ctx.fha_part5}

=== CONTEXT ===
- Commit 2 (be664a04) added AppState-level dedup + session-id freshness scaffolding.
- Commit 3 (29d5cfa2) added characterisation test for FMS reset.
- Commit 7 (d35342bf) added rc_common::url::http_base_from_ws helper.
- Fleet state 2026-04-21: 4 pods online on a13942f2 (pre-Part-5), 0 pods have Part 5 yet.
- Deploy is gated on this Step 4 VERIFY outcome + 4 offline pods returning + explicit user auth.
- Sibling Part 4 (dead-man's-switch) is NOT coded — if Part 5 alone cannot cover the silent-loop-death class, call that out.
`;
}

function callModel(model, prompt) {
  return new Promise((resolve) => {
    const body = JSON.stringify({
      model: model.id,
      messages: [{ role: "user", content: prompt }],
      max_tokens: 8000,
      temperature: 0.1,
    });

    const req = https.request(
      {
        hostname: "openrouter.ai",
        path: "/api/v1/chat/completions",
        method: "POST",
        headers: {
          "Authorization": `Bearer ${API_KEY}`,
          "Content-Type": "application/json",
          "Content-Length": Buffer.byteLength(body),
          "HTTP-Referer": "https://racingpoint.in",
          "X-Title": "MMA-Part5-Step4-Verify",
        },
        timeout: 240000,
      },
      (res) => {
        let data = "";
        res.on("data", (chunk) => (data += chunk));
        res.on("end", () => {
          try {
            const parsed = JSON.parse(data);
            if (parsed.error) {
              resolve({ ok: false, status: res.statusCode, error: parsed.error.message || JSON.stringify(parsed.error), raw: data });
            } else if (!parsed.choices || !parsed.choices[0]) {
              resolve({ ok: false, status: res.statusCode, error: "no choices in response", raw: data });
            } else {
              resolve({
                ok: true,
                status: res.statusCode,
                content: parsed.choices[0].message.content,
                usage: parsed.usage || {},
                finish_reason: parsed.choices[0].finish_reason,
              });
            }
          } catch (e) {
            resolve({ ok: false, status: res.statusCode, error: `parse error: ${e.message}`, raw: data.slice(0, 500) });
          }
        });
      }
    );

    req.on("timeout", () => {
      req.destroy();
      resolve({ ok: false, status: 0, error: "timeout_240s" });
    });
    req.on("error", (err) => {
      resolve({ ok: false, status: 0, error: `request error: ${err.message}` });
    });

    req.write(body);
    req.end();
  });
}

function estimateCost(model, usage) {
  const pt = usage.prompt_tokens || 0;
  const ct = usage.completion_tokens || 0;
  return (pt / 1e6) * model.price_prompt + (ct / 1e6) * model.price_completion;
}

function extractJson(content) {
  try { return { ok: true, parsed: JSON.parse(content.trim()), raw: content }; } catch (_) { }
  const fence = content.match(/```(?:json)?\s*\n([\s\S]*?)\n```/);
  if (fence) {
    try { return { ok: true, parsed: JSON.parse(fence[1]), raw: content }; } catch (_) { }
  }
  const firstBrace = content.indexOf("{");
  if (firstBrace >= 0) {
    let depth = 0, inString = false, escape = false;
    for (let i = firstBrace; i < content.length; i++) {
      const c = content[i];
      if (escape) { escape = false; continue; }
      if (c === "\\" && inString) { escape = true; continue; }
      if (c === '"') { inString = !inString; continue; }
      if (inString) continue;
      if (c === "{") depth++;
      else if (c === "}") {
        depth--;
        if (depth === 0) {
          const candidate = content.slice(firstBrace, i + 1);
          try { return { ok: true, parsed: JSON.parse(candidate), raw: content }; } catch (_) { }
          break;
        }
      }
    }
  }
  return { ok: false, error: "could not extract JSON", raw: content };
}

async function main() {
  await fs.mkdir(OUT_DIR, { recursive: true });
  const ctx = await loadContext();
  const prompt = buildPrompt(ctx);
  console.error(`[mma-step4] prompt size: ${prompt.length} chars, approx ${Math.ceil(prompt.length / 4)} tokens`);

  const BUDGET = 0.80;
  const PER_MODEL_CAP = 0.35;
  let running_total = 0;

  const results = [];
  const promises = MODELS.map(async (m) => {
    const t0 = Date.now();
    console.error(`[mma-step4] dispatch ${m.short} (${m.id})`);
    let res = await callModel(m, prompt);
    if (!res.ok && (res.error === "timeout_240s" || (res.status && res.status >= 500))) {
      console.error(`[mma-step4] retry ${m.short} — first attempt: ${res.error}`);
      res = await callModel(m, prompt);
    }
    const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
    const r = { model: m, result: res, elapsed_sec: elapsed };
    if (res.ok) {
      const cost = estimateCost(m, res.usage);
      r.cost_usd = cost;
      console.error(`[mma-step4] done ${m.short} — ${elapsed}s, $${cost.toFixed(4)}, tokens: p=${res.usage.prompt_tokens} c=${res.usage.completion_tokens}`);
      if (cost > PER_MODEL_CAP) {
        console.error(`[mma-step4] WARN single-model cost ${m.short} $${cost.toFixed(4)} exceeds cap $${PER_MODEL_CAP}`);
      }
      const extracted = extractJson(res.content);
      r.json_ok = extracted.ok;
      r.verdict = extracted.ok ? extracted.parsed : null;

      const outPath = path.join(OUT_DIR, `${m.short}.json`);
      if (extracted.ok) {
        await fs.writeFile(outPath, JSON.stringify(extracted.parsed, null, 2));
      } else {
        await fs.writeFile(outPath, JSON.stringify({
          __malformed: true,
          __error: extracted.error,
          __model: m.id,
          __raw_content: res.content,
        }, null, 2));
      }
      await fs.writeFile(path.join(OUT_DIR, `${m.short}.meta.json`), JSON.stringify({
        model: m.id,
        short: m.short,
        role: m.role,
        elapsed_sec: elapsed,
        prompt_tokens: res.usage.prompt_tokens,
        completion_tokens: res.usage.completion_tokens,
        cost_usd: cost,
        finish_reason: res.finish_reason,
        json_ok: extracted.ok,
        json_error: extracted.ok ? null : extracted.error,
      }, null, 2));
    } else {
      console.error(`[mma-step4] FAIL ${m.short} — ${elapsed}s, status=${res.status}, error=${res.error}`);
      await fs.writeFile(path.join(OUT_DIR, `${m.short}.json`), JSON.stringify({
        __failed: true,
        __model: m.id,
        __status: res.status,
        __error: res.error,
        __raw: (res.raw || "").slice(0, 1000),
      }, null, 2));
    }
    return r;
  });

  const settled = await Promise.all(promises);
  for (const r of settled) results.push(r);
  running_total = results.filter(r => r.result.ok).reduce((s, r) => s + r.cost_usd, 0);

  const summary = {
    step: "step4_verify",
    total_cost_usd: running_total,
    budget_remaining_usd: BUDGET - running_total,
    ok_count: results.filter(r => r.result.ok).length,
    fail_count: results.filter(r => !r.result.ok).length,
    json_ok_count: results.filter(r => r.result.ok && r.json_ok).length,
    per_model: results.map(r => ({
      short: r.model.short,
      id: r.model.id,
      role: r.model.role,
      ok: r.result.ok,
      json_ok: r.result.ok ? r.json_ok : false,
      elapsed_sec: r.elapsed_sec,
      status: r.result.status,
      error: r.result.ok ? null : r.result.error,
      cost_usd: r.cost_usd || 0,
      prompt_tokens: r.result.ok ? r.result.usage.prompt_tokens : 0,
      completion_tokens: r.result.ok ? r.result.usage.completion_tokens : 0,
      verdict: r.result.ok && r.json_ok ? r.verdict.verdict : null,
      score: r.result.ok && r.json_ok ? r.verdict.score_out_of_5 : null,
    })),
  };
  console.log(JSON.stringify(summary, null, 2));
}

main().catch((e) => {
  console.error("[mma-step4] fatal:", e);
  process.exit(1);
});
