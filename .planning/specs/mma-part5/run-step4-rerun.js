#!/usr/bin/env node
// MMA Part 5 Step 4 VERIFY — RE-RUN against post-fix commit 49eb2821.
//
// Reads source files via `git show 49eb2821:<path>` so the working tree
// (detached HEAD on an unrelated commit) is never touched. Same 3 models
// as the original Step 4 run (kimi / mistral / mimo) to enable direct
// delta comparison of verdicts. Output dir: verify-rerun/.

const fs = require("fs").promises;
const fsSync = require("fs");
const https = require("https");
const path = require("path");
const { execFileSync } = require("child_process");

const ROOT = "C:/Users/bono/racingpoint/racecontrol";
const OUT_DIR = path.join(ROOT, ".planning/specs/mma-part5/verify-rerun");
const KEY_FILE = path.join(ROOT, "data/openrouter-mma-key.txt");
const FIX_COMMIT = "49eb2821";

const API_KEY = fsSync.readFileSync(KEY_FILE, "utf8").trim();
if (!API_KEY) {
  console.error("ERROR: empty API key");
  process.exit(2);
}

function gitShow(commit, filePath) {
  // Use execFileSync (argv — no shell quoting) to read any path safely.
  return execFileSync(
    "git",
    ["show", `${commit}:${filePath}`],
    { cwd: ROOT, encoding: "utf8", maxBuffer: 32 * 1024 * 1024 }
  );
}

// NOTE: original run had all 3 models; mimo crashed extractJson on null content.
// This is a re-dispatch for mimo ONLY. kimi.json + mistral.json already on disk
// from first run — leaving untouched.
const MODELS = [
  { id: "xiaomi/mimo-v2-pro",           short: "mimo",    price_prompt: 0.15, price_completion: 0.60, role: "sre-adversarial" },
];

async function loadContext() {
  // DESIGN-SPEC + FINDINGS-STEP1 from fix commit (same snapshot as the fix)
  const spec = gitShow(FIX_COMMIT, ".planning/specs/PATTERN-I-PART-5-HTTP-FALLBACK.md");
  const findings = gitShow(FIX_COMMIT, ".planning/specs/mma-part5/FINDINGS-STEP1.md");
  const step4_prior = gitShow(FIX_COMMIT, ".planning/specs/mma-part5/FINDINGS-STEP4.md");

  // POST-FIX SOURCE (the whole point of this re-run)
  const module = gitShow(FIX_COMMIT, "crates/rc-agent/src/session_end_fallback.rs");
  const wsh_full = gitShow(FIX_COMMIT, "crates/rc-agent/src/ws_handler.rs");
  const wsh_part5 = wsh_full.split("\n").slice(2324, 2570).join("\n");
  const fha_full = gitShow(FIX_COMMIT, "crates/racecontrol/src/fleet_health_api.rs");
  const fha_part5 = fha_full.split("\n").slice(100, 220).join("\n");
  // V-C cache field — pull the failure_monitor.rs struct excerpt
  const fms_full = gitShow(FIX_COMMIT, "crates/rc-agent/src/failure_monitor.rs");
  const fms_struct = fms_full.split("\n").slice(38, 120).join("\n");

  return { spec, findings, step4_prior, module, wsh_part5, fha_part5, fms_struct };
}

const JSON_SCHEMA = `{
  "verdict": "PASS|CONDITIONAL_PASS|BLOCK",
  "score_out_of_5": 0.0,
  "prior_defects_status": [
    { "defect_id": "V-A|V-B|V-C|...",
      "fix_addresses_root_cause": true|false,
      "evidence": "file:line or quote from post-fix code",
      "residual_risk": "none|low|medium|high",
      "note": "does the fix go far enough? any half-measures?" }
  ],
  "new_defects_found": [
    { "defect_id": "your-choice, e.g. W-1",
      "severity": "P0|P1|P2|P3",
      "category": "correctness|race|resource|security|observability|deploy",
      "file_line": "crates/rc-agent/src/...:NN",
      "description": "1-3 sentences",
      "exploit_or_trigger": "how it fires",
      "suggested_fix": "1-2 sentences",
      "confidence_1_to_5": 4 }
  ],
  "deploy_readiness": {
    "can_ship_as_is": true|false,
    "must_fix_before_ship": ["defect ids"],
    "nice_to_have": ["defect ids"]
  },
  "adversarial_summary": "3-5 sentence executive summary"
}`;

function buildPrompt(ctx) {
  return `You are an adversarial Rust reviewer performing MMA Step 4 RE-RUN for Racing Point Pattern I Part 5. The prior Step 4 run returned verdict BLOCK with 3 consensus defects (V-A, V-B, V-C). The author has shipped a fix commit at 49eb2821 — your job NOW is to assess whether the fix actually addresses each defect's root cause, and whether the fix introduced new defects.

Focus shift vs the original Step 4 run:
- Prior Step 4 asked: "what did the implementation miss?"
- This re-run asks: "does the fix hold up adversarially? is V-A/V-B/V-C now closed at the root cause, or is it papered over?"

For each prior defect (V-A reqwest timeout-during-parse race, V-B Client-per-call, V-C synth zeros), explicitly rate whether the fix:
- Closes the root cause (residual_risk: none)
- Addresses the symptom but leaves the class open (residual_risk: low/medium)
- Fails to address at all (residual_risk: high)

Also look for NEW defects introduced BY the fix itself (W-1, W-2, ...). Example classes to probe:
- Does the OnceLock HTTP_CLIENT carry cross-test contamination?
- Does the bytes+from_slice split introduce a new error path that's also silent?
- Does the driving_seconds cache race with SessionStarted? Does reset_fms_for_session_end get called atomically with the synth or can there be a window where cache survives a session-end?
- Test coverage: do the 4 new unit tests actually exercise the fix paths, or are they surface-level?

Emit STRICT JSON matching this schema:
${JSON_SCHEMA}

Scoring rubric (score_out_of_5):
- 5.0 = all prior defects closed at root cause, zero new defects, test coverage adequate
- 4.0+ = all prior addressed cleanly, ≤2 P2 new, ≤1 P1 new → PASS / CONDITIONAL_PASS
- 3.0 = ≥1 P0 OR ≥3 P1 new OR ≥1 prior defect still unaddressed → BLOCK
- 2.0 = systemic issues → BLOCK

MMA protocol: ≥4.0 unblocks deploy. Lower = blocked.

Output ONLY the JSON. No prose before or after.

=== DESIGN-SPEC (PATTERN-I-PART-5-HTTP-FALLBACK.md, unchanged from prior Step 4) ===
${ctx.spec}

=== STEP 1 FINDINGS (unchanged) ===
${ctx.findings}

=== PRIOR STEP 4 FINDINGS (the defects being re-evaluated) ===
${ctx.step4_prior}

=== POST-FIX SOURCE @ 49eb2821: crates/rc-agent/src/session_end_fallback.rs ===
${ctx.module}

=== POST-FIX SOURCE @ 49eb2821: crates/rc-agent/src/failure_monitor.rs lines 39-120 (FailureMonitorState struct + Default + reset helper — V-C cache field) ===
${ctx.fms_struct}

=== SOURCE @ 49eb2821 (unchanged): crates/rc-agent/src/ws_handler.rs lines 2325-2570 (Commit 4 extraction) ===
${ctx.wsh_part5}

=== SOURCE @ 49eb2821 (unchanged): crates/racecontrol/src/fleet_health_api.rs lines 101-220 (Commit 6 flag) ===
${ctx.fha_part5}

=== CONTEXT ===
- Fix commit 49eb2821 (+179 / -23 LOC across 2 files). Changes: (1) module-level OnceLock HTTP_CLIENT; (2) bytes() with explicit 5s tokio::time::timeout + serde_json::from_slice decode; (3) session_last_known_driving_seconds cache on FailureMonitorState, populated in still-live branch, read in synth branch. Plus 4 new unit tests and 1 updated D11 characterisation.
- total_laps and best_lap_ms remain hardcoded 0/None — BillingSessionInfo lacks them. Tracked as P3 follow-up.
- C3 (T2 cancellation) was verified as already-addressed (tokio::select! RAII pattern); no change in this fix.
`;
}

function callModel(model, prompt) {
  return new Promise((resolve) => {
    const body = JSON.stringify({
      model: model.id,
      messages: [{ role: "user", content: prompt }],
      max_tokens: 24000,  // bumped from 12000 — mimo hit exact 12000 cap last run, returned null content
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
          "X-Title": "MMA-Part5-Step4-ReRun",
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

    req.on("timeout", () => { req.destroy(); resolve({ ok: false, status: 0, error: "timeout_240s" }); });
    req.on("error", (err) => resolve({ ok: false, status: 0, error: `request error: ${err.message}` }));
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
  // Null-guard: OpenRouter returns content=null when finish_reason=length cuts at max_tokens.
  if (!content || typeof content !== "string") {
    return { ok: false, error: "null or non-string content (likely hit max_tokens cap — finish_reason=length)", raw: "" };
  }
  try { return { ok: true, parsed: JSON.parse(content.trim()), raw: content }; } catch (_) {}
  const fence = content.match(/```(?:json)?\s*\n([\s\S]*?)\n```/);
  if (fence) { try { return { ok: true, parsed: JSON.parse(fence[1]), raw: content }; } catch (_) {} }
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
      else if (c === "}") { depth--; if (depth === 0) {
        const candidate = content.slice(firstBrace, i + 1);
        try { return { ok: true, parsed: JSON.parse(candidate), raw: content }; } catch (_) {}
        break;
      }}
    }
  }
  return { ok: false, error: "could not extract JSON", raw: content };
}

async function main() {
  await fs.mkdir(OUT_DIR, { recursive: true });
  const ctx = await loadContext();
  const prompt = buildPrompt(ctx);
  console.error(`[step4-rerun] prompt size: ${prompt.length} chars, approx ${Math.ceil(prompt.length / 4)} tokens`);

  const BUDGET = 0.80;
  const results = [];
  const promises = MODELS.map(async (m) => {
    const t0 = Date.now();
    console.error(`[step4-rerun] dispatch ${m.short} (${m.id})`);
    let res = await callModel(m, prompt);
    if (!res.ok && (res.error === "timeout_240s" || (res.status && res.status >= 500))) {
      console.error(`[step4-rerun] retry ${m.short} — ${res.error}`);
      res = await callModel(m, prompt);
    }
    const elapsed = ((Date.now() - t0) / 1000).toFixed(1);
    const r = { model: m, result: res, elapsed_sec: elapsed };
    if (res.ok) {
      const cost = estimateCost(m, res.usage);
      r.cost_usd = cost;
      console.error(`[step4-rerun] done ${m.short} — ${elapsed}s, $${cost.toFixed(4)}, tokens: p=${res.usage.prompt_tokens} c=${res.usage.completion_tokens}`);
      const extracted = extractJson(res.content);
      r.json_ok = extracted.ok;
      r.verdict = extracted.ok ? extracted.parsed : null;
      const outPath = path.join(OUT_DIR, `${m.short}.json`);
      if (extracted.ok) {
        await fs.writeFile(outPath, JSON.stringify(extracted.parsed, null, 2));
      } else {
        await fs.writeFile(outPath, JSON.stringify({
          __malformed: true, __error: extracted.error, __model: m.id, __raw_content: res.content,
        }, null, 2));
      }
      await fs.writeFile(path.join(OUT_DIR, `${m.short}.meta.json`), JSON.stringify({
        model: m.id, short: m.short, role: m.role, elapsed_sec: elapsed,
        prompt_tokens: res.usage.prompt_tokens, completion_tokens: res.usage.completion_tokens,
        cost_usd: cost, finish_reason: res.finish_reason,
        json_ok: extracted.ok, json_error: extracted.ok ? null : extracted.error,
      }, null, 2));
    } else {
      console.error(`[step4-rerun] FAIL ${m.short} — ${elapsed}s, status=${res.status}, error=${res.error}`);
      await fs.writeFile(path.join(OUT_DIR, `${m.short}.json`), JSON.stringify({
        __failed: true, __model: m.id, __status: res.status, __error: res.error,
        __raw: (res.raw || "").slice(0, 1000),
      }, null, 2));
    }
    return r;
  });

  const settled = await Promise.all(promises);
  for (const r of settled) results.push(r);
  const running_total = results.filter(r => r.result.ok).reduce((s, r) => s + r.cost_usd, 0);

  const summary = {
    step: "step4_verify_rerun",
    fix_commit: FIX_COMMIT,
    total_cost_usd: running_total,
    budget_remaining_usd: BUDGET - running_total,
    ok_count: results.filter(r => r.result.ok).length,
    fail_count: results.filter(r => !r.result.ok).length,
    json_ok_count: results.filter(r => r.result.ok && r.json_ok).length,
    per_model: results.map(r => ({
      short: r.model.short, id: r.model.id, role: r.model.role,
      ok: r.result.ok, json_ok: r.result.ok ? r.json_ok : false,
      elapsed_sec: r.elapsed_sec, status: r.result.status,
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

main().catch((e) => { console.error("[step4-rerun] fatal:", e); process.exit(1); });
