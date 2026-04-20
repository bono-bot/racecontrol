#!/usr/bin/env node
// MMA Step 1 DIAGNOSE runner — 5 parallel OpenRouter models
// Produces: responses/<short-name>.md + cost/usage summary to stdout.
// No deps: uses https core module + fs/promises.

const fs = require("fs").promises;
const fsSync = require("fs");
const https = require("https");
const path = require("path");

const ROOT = "C:/Users/bono/racingpoint/racecontrol";
const OUT_DIR = path.join(ROOT, ".planning/specs/mma-part5/responses");
const KEY_FILE = path.join(ROOT, "data/openrouter-mma-key.txt");

const API_KEY = fsSync.readFileSync(KEY_FILE, "utf8").trim();
if (!API_KEY) {
  console.error("ERROR: empty API key");
  process.exit(2);
}

// Pricing per million tokens (prompt / completion) — USD.
// Values pulled from OpenRouter public pricing table 2026-04-20 (approximate).
const MODELS = [
  {
    id: "deepseek/deepseek-r1-0528",
    short: "deepseek-r1",
    price_prompt: 0.50,
    price_completion: 2.18,
    role: "reasoner",
  },
  {
    id: "qwen/qwen3-coder",
    short: "qwen3-coder",
    price_prompt: 0.30,
    price_completion: 1.20,
    role: "code_expert",
  },
  {
    id: "nvidia/nemotron-3-super-120b-a12b",
    short: "nemotron-super",
    price_prompt: 0.10,
    price_completion: 0.50,
    role: "sre",
  },
  {
    id: "google/gemini-2.5-flash",
    short: "gemini-flash",
    price_prompt: 0.30,
    price_completion: 2.50,
    role: "generalist",
  },
  {
    id: "x-ai/grok-code-fast-1",
    short: "grok-code",
    price_prompt: 0.20,
    price_completion: 1.50,
    role: "code_expert",
  },
];

// --- Load + assemble prompt payload ---
async function loadContext() {
  const spec = await fs.readFile(
    path.join(ROOT, ".planning/specs/PATTERN-I-PART-5-HTTP-FALLBACK.md"),
    "utf8"
  );

  // billing_views.rs lines 326-331 (active_billing_sessions handler)
  const bv = (await fs.readFile(
    path.join(ROOT, "crates/racecontrol/src/api/billing_views.rs"),
    "utf8"
  )).split("\n").slice(325, 332).join("\n");

  // ws_handler.rs lines 370-450 (SessionEnded arm)
  const wsh = (await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/ws_handler.rs"),
    "utf8"
  )).split("\n").slice(369, 453).join("\n");

  // main.rs lines 2060-2230 (reconnect loop context)
  const mainrs = (await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/main.rs"),
    "utf8"
  )).split("\n").slice(2059, 2230).join("\n");

  // ws_state.rs (entire file)
  const wss = await fs.readFile(
    path.join(ROOT, "crates/rc-agent/src/ws_state.rs"),
    "utf8"
  );

  return { spec, bv, wsh, mainrs, wss };
}

function buildPrompt(ctx) {
  return `You are an expert Rust / distributed systems auditor. Review the following DESIGN-SPEC for a proposed change to rc-agent (a Windows-native tokio-based agent on racing simulator pods). The change adds an HTTP fallback path so rc-agent can self-end a session when WebSocket misses the SessionEnded frame.

ENUMERATE all failure modes this patch could introduce or fail to address. Cover four axes:
(a) Trace-level bugs — variable-level, timing-sensitive, race conditions, ordering
(b) Architecture bugs — idempotency, authority semantics, cross-system ordering
(c) Integration bugs with existing reconnect/WS-state/overlay/blank_timer/failure_monitor code
(d) Deployment / rollback / observability bugs

For each finding, output:
  FINDING #N: <one-line title>
  SEVERITY: P0 | P1 | P2 | P3
  AXIS: a | b | c | d
  DESCRIPTION: <what the bug is, under 150 words>
  EVIDENCE: <cite file:line or spec section that motivates the concern>
  PROPOSED MITIGATION: <under 80 words>

Produce 8-15 findings. Be concrete. Do NOT restate the spec. Do NOT give general advice. Every finding must name a specific scenario, input, or code path.

=== DESIGN-SPEC (PATTERN-I-PART-5-HTTP-FALLBACK.md) ===
${ctx.spec}

=== SOURCE: crates/racecontrol/src/api/billing_views.rs lines 326-331 (server endpoint) ===
${ctx.bv}

=== SOURCE: crates/rc-agent/src/ws_handler.rs lines 370-450 (existing WS SessionEnded arm — the refactor target) ===
${ctx.wsh}

=== SOURCE: crates/rc-agent/src/main.rs lines 2060-2230 (reconnect loop — where T1 fires post-register) ===
${ctx.mainrs}

=== SOURCE: crates/rc-agent/src/ws_state.rs (diagnostic ws_state module, full file) ===
${ctx.wss}
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
          "X-Title": "MMA-Part5-Step1",
        },
        timeout: 120000,
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
      resolve({ ok: false, status: 0, error: "timeout_120s" });
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

async function main() {
  const ctx = await loadContext();
  const prompt = buildPrompt(ctx);
  console.error(`[mma-step1] prompt size: ${prompt.length} chars, approx ${Math.ceil(prompt.length / 4)} tokens`);

  const results = await Promise.all(
    MODELS.map(async (m) => {
      const t0 = Date.now();
      console.error(`[mma-step1] dispatch ${m.short} (${m.id})`);
      let res = await callModel(m, prompt);
      // Retry once if first attempt failed (timeout or transient error)
      if (!res.ok && (res.error === "timeout_120s" || (res.status && res.status >= 500))) {
        console.error(`[mma-step1] retry ${m.short} — first attempt: ${res.error}`);
        res = await callModel(m, prompt);
      }
      const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
      const r = { model: m, result: res, elapsed_sec: elapsed };
      if (res.ok) {
        const cost = estimateCost(m, res.usage);
        r.cost_usd = cost;
        console.error(`[mma-step1] done ${m.short} — ${elapsed}s, $${cost.toFixed(4)}, tokens: p=${res.usage.prompt_tokens} c=${res.usage.completion_tokens}`);
        if (cost > 1.0) {
          console.error(`[mma-step1] WARN cost > $1.00 for ${m.short} — ${cost.toFixed(4)}`);
        }
        const header = `# ${m.short} (${m.id})\n\n- elapsed: ${elapsed}s\n- prompt_tokens: ${res.usage.prompt_tokens}\n- completion_tokens: ${res.usage.completion_tokens}\n- estimated_cost_usd: ${cost.toFixed(4)}\n- finish_reason: ${res.finish_reason}\n\n---\n\n${res.content}\n`;
        await fs.writeFile(path.join(OUT_DIR, `${m.short}.md`), header);
      } else {
        console.error(`[mma-step1] FAIL ${m.short} — ${elapsed}s, status=${res.status}, error=${res.error}`);
        await fs.writeFile(
          path.join(OUT_DIR, `${m.short}.md`),
          `# ${m.short} (${m.id}) — FAILED\n\n- elapsed: ${elapsed}s\n- status: ${res.status}\n- error: ${res.error}\n\n${res.raw ? "## Raw response (truncated)\n\n```\n" + (res.raw || "").slice(0, 1000) + "\n```\n" : ""}`
        );
      }
      return r;
    })
  );

  // Summary to stdout (machine-readable JSON on one line)
  const summary = {
    total_cost_usd: results.filter(r => r.result.ok).reduce((s, r) => s + r.cost_usd, 0),
    ok_count: results.filter(r => r.result.ok).length,
    fail_count: results.filter(r => !r.result.ok).length,
    per_model: results.map(r => ({
      short: r.model.short,
      id: r.model.id,
      ok: r.result.ok,
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
  console.error("[mma-step1] fatal:", e);
  process.exit(1);
});
