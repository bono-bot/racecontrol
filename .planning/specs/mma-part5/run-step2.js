#!/usr/bin/env node
// MMA Part 5 Step 2 PLAN runner — 5 parallel OpenRouter models produce JSON fix plans
// Produces: plans/<short-name>.json + per-model metadata + cost summary to stdout.

const fs = require("fs").promises;
const fsSync = require("fs");
const https = require("https");
const path = require("path");

const ROOT = "C:/Users/bono/racingpoint/racecontrol";
const OUT_DIR = path.join(ROOT, ".planning/specs/mma-part5/plans");
const KEY_FILE = path.join(ROOT, "data/openrouter-mma-key.txt");

const API_KEY = fsSync.readFileSync(KEY_FILE, "utf8").trim();
if (!API_KEY) {
  console.error("ERROR: empty API key");
  process.exit(2);
}

// Per MMA rule: ≥3 vendor families, ≥1 reasoner, ≥2 code_expert, ≥1 SRE.
// Vendor families: deepseek×2 (max 2 per family OK), nvidia, google, openai = 4 families.
const MODELS = [
  {
    id: "deepseek/deepseek-r1-0528",
    short: "r1",
    price_prompt: 0.50,
    price_completion: 2.18,
    role: "reasoner",
  },
  {
    id: "deepseek/deepseek-chat-v3-0324",
    short: "v3",
    price_prompt: 0.27,
    price_completion: 1.10,
    role: "code_expert",
  },
  {
    id: "nvidia/nemotron-3-super-120b-a12b",
    short: "nemotron",
    price_prompt: 0.10,
    price_completion: 0.50,
    role: "sre",
  },
  {
    id: "google/gemini-2.5-flash",
    short: "gemini",
    price_prompt: 0.30,
    price_completion: 2.50,
    role: "generalist",
  },
  {
    id: "openai/gpt-5.1-codex-mini",
    short: "codex-mini",
    price_prompt: 0.25,
    price_completion: 2.00,
    role: "code_expert",
  },
];

// --- Load context bundle ---
async function loadContext() {
  const spec = await fs.readFile(
    path.join(ROOT, ".planning/specs/PATTERN-I-PART-5-HTTP-FALLBACK.md"),
    "utf8"
  );
  const findings = await fs.readFile(
    path.join(ROOT, ".planning/specs/mma-part5/FINDINGS-STEP1.md"),
    "utf8"
  );

  // billing_views.rs lines 326-331 (active_billing_sessions handler)
  const bv = (await fs.readFile(
    path.join(ROOT, "crates/racecontrol/src/api/billing_views.rs"),
    "utf8"
  )).split("\n").slice(325, 332).join("\n");

  // ws_handler.rs lines 370-450 (SessionEnded arm - refactor target)
  const wsh = (await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/ws_handler.rs"),
    "utf8"
  )).split("\n").slice(369, 453).join("\n");

  // main.rs lines 2060-2230 (reconnect loop context)
  const mainrs = (await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/main.rs"),
    "utf8"
  )).split("\n").slice(2059, 2230).join("\n");

  // ws_state.rs (full file, already shipped in part 1)
  const wss = await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/ws_state.rs"),
    "utf8"
  );

  // failure_monitor.rs lines 35-90 (FailureMonitorState struct + Default impl)
  const fms = (await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/failure_monitor.rs"),
    "utf8"
  )).split("\n").slice(34, 90).join("\n");

  return { spec, findings, bv, wsh, mainrs, wss, fms };
}

const JSON_SCHEMA = `{
  "plan_id": "string (your choice, e.g. 'plan-r1-v1')",
  "strategy": "one-paragraph summary of your approach",
  "commit_order": [
    { "seq": 1, "title": "...", "files_touched": ["path/to/file.rs"],
      "changes_summary": "...", "risk": "low|medium|high", "rollback": "how to undo" }
  ],
  "key_design_decisions": [
    { "question": "...", "decision": "...", "rationale": "...", "alternatives_considered": "..." }
  ],
  "test_plan": [
    { "test_name": "...", "type": "unit|integration|characterisation", "file": "tests/...", "coverage": "..." }
  ],
  "risk_matrix": [
    { "risk": "...", "likelihood": "low|medium|high", "impact": "low|medium|high", "mitigation": "..." }
  ],
  "deploy_plan": {
    "build_targets": [],
    "swap_order": [],
    "rollback_procedure": "...",
    "verification_cues": []
  },
  "open_questions": []
}`;

function buildPrompt(ctx) {
  return `You are a senior Rust engineer preparing a concrete implementation plan for Racing Point eSports Pattern I Part 5.

The spec and Step-1 findings below describe a change to rc-agent. Your task: emit a structured JSON plan that could be executed mechanically. DO NOT write code — write the PLAN.

Required JSON schema:
${JSON_SCHEMA}

Constraints:
- All 10 must-include spec updates MUST be addressed (60s grace, AppState guard, shared url helper, pod_id filter, X-Service-Key, status gate, CancellationToken, char-for-char refactor, refresh_summary upgrade, fallback_version telemetry)
- Commit sequence must be ordered to preserve compilability at each step
- Characterisation test MUST be sequenced before the refactor commit (D11)
- Max 5 commits total — if you need more, merge adjacent ones
- Each commit under 200 LoC change where possible (smallest-reversible principle)

Output ONLY the JSON. No prose before or after.

=== DESIGN-SPEC (PATTERN-I-PART-5-HTTP-FALLBACK.md, post-Step-1) ===
${ctx.spec}

=== STEP 1 DIAGNOSE FINDINGS (FINDINGS-STEP1.md) ===
${ctx.findings}

=== SOURCE: crates/racecontrol/src/api/billing_views.rs lines 326-331 (server endpoint) ===
${ctx.bv}

=== SOURCE: crates/rc-agent/src/ws_handler.rs lines 370-450 (existing WS SessionEnded arm — refactor target) ===
${ctx.wsh}

=== SOURCE: crates/rc-agent/src/main.rs lines 2060-2230 (reconnect loop — T1 fires post-register) ===
${ctx.mainrs}

=== SOURCE: crates/rc-agent/src/ws_state.rs (already shipped in part 1 — for reference) ===
${ctx.wss}

=== SOURCE: crates/rc-agent/src/failure_monitor.rs lines 35-90 (FailureMonitorState struct + Default) ===
${ctx.fms}
`;
}

// --- HTTPS POST to OpenRouter with timeout ---
function callModel(model, prompt) {
  return new Promise((resolve) => {
    const body = JSON.stringify({
      model: model.id,
      messages: [{ role: "user", content: prompt }],
      max_tokens: 16000,
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
          "X-Title": "MMA-Part5-Step2",
        },
        timeout: 180000,
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
      resolve({ ok: false, status: 0, error: "timeout_180s" });
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

// --- Extract JSON from model output (may be wrapped in code fences or have prose) ---
function extractJson(content) {
  // Try direct parse first
  try { return { ok: true, parsed: JSON.parse(content.trim()), raw: content }; } catch (_) { }
  // Try ```json ... ``` code-fenced
  const fence = content.match(/```(?:json)?\s*\n([\s\S]*?)\n```/);
  if (fence) {
    try { return { ok: true, parsed: JSON.parse(fence[1]), raw: content }; } catch (_) { }
  }
  // Try finding the first { and matching } with naive brace balancing
  const firstBrace = content.indexOf("{");
  if (firstBrace >= 0) {
    let depth = 0;
    let inString = false;
    let escape = false;
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
  const ctx = await loadContext();
  const prompt = buildPrompt(ctx);
  console.error(`[mma-step2] prompt size: ${prompt.length} chars, approx ${Math.ceil(prompt.length / 4)} tokens`);

  const BUDGET = 1.50;
  const PER_MODEL_CAP = 0.40;
  let running_total = 0;

  const results = [];
  // Launch all 5 in parallel (budget check post-hoc)
  const promises = MODELS.map(async (m) => {
    const t0 = Date.now();
    console.error(`[mma-step2] dispatch ${m.short} (${m.id})`);
    let res = await callModel(m, prompt);
    if (!res.ok && (res.error === "timeout_180s" || (res.status && res.status >= 500))) {
      console.error(`[mma-step2] retry ${m.short} — first attempt: ${res.error}`);
      res = await callModel(m, prompt);
    }
    const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
    const r = { model: m, result: res, elapsed_sec: elapsed };
    if (res.ok) {
      const cost = estimateCost(m, res.usage);
      r.cost_usd = cost;
      console.error(`[mma-step2] done ${m.short} — ${elapsed}s, $${cost.toFixed(4)}, tokens: p=${res.usage.prompt_tokens} c=${res.usage.completion_tokens}`);
      if (cost > PER_MODEL_CAP) {
        console.error(`[mma-step2] WARN single-model cost ${m.short} $${cost.toFixed(4)} exceeds cap $${PER_MODEL_CAP}`);
      }

      // Extract JSON plan
      const extracted = extractJson(res.content);
      r.json_ok = extracted.ok;
      r.plan = extracted.ok ? extracted.parsed : null;

      // Write raw JSON (pretty-printed if parseable, else raw content)
      const outPath = path.join(OUT_DIR, `${m.short}.json`);
      if (extracted.ok) {
        await fs.writeFile(outPath, JSON.stringify(extracted.parsed, null, 2));
      } else {
        // Save raw as .json with a note wrapper
        await fs.writeFile(outPath, JSON.stringify({
          __malformed: true,
          __error: extracted.error,
          __model: m.id,
          __raw_content: res.content,
        }, null, 2));
      }
      // Also write a metadata file
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
      console.error(`[mma-step2] FAIL ${m.short} — ${elapsed}s, status=${res.status}, error=${res.error}`);
      const outPath = path.join(OUT_DIR, `${m.short}.json`);
      await fs.writeFile(outPath, JSON.stringify({
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
    })),
  };
  console.log(JSON.stringify(summary, null, 2));
}

main().catch((e) => {
  console.error("[mma-step2] fatal:", e);
  process.exit(1);
});
