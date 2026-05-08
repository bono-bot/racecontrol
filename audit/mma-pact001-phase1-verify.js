#!/usr/bin/env node
/**
 * MMA VERIFY step for PACT-001 Phase 1 wire-up Session 7 EXECUTE Path A.
 *
 * Per Unified MMA Protocol v3.0 Step 4: 3 adversarial models distinct from
 * Steps 1-3 (DIAGNOSE), score the executed fix on 1-5 scale, mean ≥4.0 = PASS.
 *
 * DIAGNOSE roster (mma-pact001-phase1-cross-bridge.js):
 *   DeepSeek R1 0528 + DeepSeek V3.2 + Qwen3 Coder + MiMo v2 Pro + Gemini 2.5 Flash
 * VERIFY roster (this script — fully distinct vendor families):
 *   GPT-5.4 Nano (OpenAI — reasoner)
 *   Grok Code Fast 1 (xAI — code-expert)
 *   Mistral Medium 3.1 (Mistral — generalist)
 *
 * Vendor diversity: 3 vendors, none overlap DIAGNOSE pool. ≥1 reasoner +
 * ≥1 code-expert per protocol stratification.
 *
 * Scope: confirm Path A fix at commit 8043f6b3 actually resolves P0 C1
 * (Walk-In Guest field name mismatch) without introducing new defects.
 */
const fs = require("fs");
const path = require("path");
const https = require("https");

const KEY_PATH = path.join(__dirname, "..", "data", "openrouter-mma-key.txt");
const KEY = (process.env.OPENROUTER_KEY || fs.readFileSync(KEY_PATH, "utf8"))
  .trim();
if (!KEY.startsWith("sk-or-v")) {
  throw new Error("OPENROUTER_KEY missing or malformed");
}

const TS = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
const OUT_DIR = path.join(__dirname, "results", `mma-pact001-phase1-verify-${TS}`);
fs.mkdirSync(OUT_DIR, { recursive: true });

const MODELS = [
  { id: "openai/gpt-5.4-nano", role: "reasoner", vendor: "openai" },
  { id: "x-ai/grok-code-fast-1", role: "code-expert", vendor: "xai" },
  { id: "mistralai/mistral-medium-3.1", role: "generalist", vendor: "mistral" },
];

const PROMPT = `You are an adversarial verifier for an executed bug fix. You did NOT participate in
the original DIAGNOSE step (different model pool). Your job: independently
score the fix on a 1-5 scale across 5 dimensions, and flag any new bugs the
fix may have introduced.

ORIGINAL BUG (P0 deploy-blocker, 3/3 DIAGNOSE consensus)
--------------------------------------------------------
TS web-v2 sent JSON requests with method-name-mirrored inner field names:
  { method: "qr_payload",    qr_payload: string }
  { method: "nfc_tag_id",    nfc_tag_id: string }
  { method: "walk_in_guest_id", walk_in_guest_id: 1 | 2 }

But the Rust racecontrol handler deserializes via v2_db::cirs::LookupInput
whose canonical wire fields are:
  { method: "qr_payload",       payload:  string }
  { method: "nfc_tag_id",       tag_id:   string }
  { method: "walk_in_guest_id", guest_id: 1 | 2 }

Existing serde test at cirs_lookup.rs:482 confirms canonical Rust shape:
  assert_eq!(json, r#"{"method":"walk_in_guest_id","guest_id":1}"#)

Walk-In Guest is a shipped customer-touch path. M3 (qr_payload) and M4
(nfc_tag_id) are plumbed-disabled (501) — Walk-In Guest is the live one.

PATH A FIX (commit 8043f6b3)
----------------------------
1. web-v2/src/lib/types/cirs-lookup.ts — discriminated-union variants:
   Before:
     | { method: "qr_payload"; qr_payload: string }
     | { method: "nfc_tag_id"; nfc_tag_id: string }
     | { method: "walk_in_guest_id"; walk_in_guest_id: 1 | 2 };
   After:
     | { method: "qr_payload"; payload: string }
     | { method: "nfc_tag_id"; tag_id: string }
     | { method: "walk_in_guest_id"; guest_id: 1 | 2 };

2. web-v2/src/app/pos/lookup/page.tsx — handleWalkInSelect body:
   Before: lookup({ method: "walk_in_guest_id", walk_in_guest_id: id });
   After:  lookup({ method: "walk_in_guest_id", guest_id: id });

3. web-v2/tests/e2e/cirs-lookup.spec.ts — Playwright mock-route inspection:
   Before: if (body?.method === "walk_in_guest_id" && body?.walk_in_guest_id === 1)
   After:  if (body?.method === "walk_in_guest_id" && body?.guest_id === 1)

VERIFICATION RUNS POST-FIX
--------------------------
- npx tsc --noEmit            -> exit 0 (clean)
- npm test (vitest)            -> 23/23 pass in 1.75s
- npx playwright test          -> 5/5 pass in 5.4s
  (incl. Walk-In Guest spec exercising the new guest_id field via mock-route)

YOUR JOB
--------
Score the fix on each dimension 1-5 (1=poor, 5=excellent):

  D1. CORRECTNESS — Does the fix actually resolve P0 C1 (TS request now
       deserializes successfully on the Rust handler)?
  D2. REVERSIBILITY — Is this the smallest reversible change? (Path A is
       TS-only; Path B would require Rust serde rename in v2-db Phase 0
       MERGED substrate — larger blast radius.)
  D3. COMPLETENESS — Are there other call sites or test mocks that still
       send the OLD field names? (Search the diff context for any leftover
       "walk_in_guest_id:" / "qr_payload:" / "nfc_tag_id:" inside JSON object
       literals — the variant METHOD discriminator strings stay the same;
       only inner field names change.)
  D4. RISK — Did the fix introduce any new defects? Type holes, broken
       runtime invariants, accidental wire-format drift, removed tests, etc.
  D5. TEST COVERAGE — Are the gates adequate? (vitest covers TS shape;
       Playwright exercises the Walk-In Guest flow with mocked backend;
       Rust serde tests cover the canonical shape; real-wire integration
       between TS and Rust is NOT tested by this fix — that's Session 8
       deploy verify scope.)

Plus identify any NEW bugs the fix may have introduced (P0/P1 only, no
nitpicks). Mention if you spot any place that still uses the OLD field
names.

OUTPUT — strict JSON, no prose, no markdown fence:
{
  "model_self_id": "<your model id>",
  "scores": {
    "D1_correctness": 1-5,
    "D2_reversibility": 1-5,
    "D3_completeness": 1-5,
    "D4_risk": 1-5,
    "D5_test_coverage": 1-5
  },
  "mean": <average>,
  "verdict": "PASS | FAIL | NEEDS_DESIGN_DECISION",
  "new_findings": [
    {
      "id": "V1",
      "severity": "P0|P1|P2",
      "title": "<one-line>",
      "evidence": "<file:line or specific code reference>",
      "fix_sketch": "<smallest reversible fix>"
    }
  ],
  "notes": "<one sentence summary>"
}

PASS gate: mean ≥ 4.0. If new P0 found, override to FAIL regardless of mean.
Be honest. Adversarial = look for what the original DIAGNOSE missed.`;

async function callModel(model) {
  const reqBody = JSON.stringify({
    model: model.id,
    messages: [{ role: "user", content: PROMPT }],
    max_tokens: 1500,
    temperature: 0.2,
  });
  const startedAt = Date.now();
  return new Promise((resolve) => {
    const req = https.request(
      {
        hostname: "openrouter.ai",
        path: "/api/v1/chat/completions",
        method: "POST",
        headers: {
          Authorization: `Bearer ${KEY}`,
          "Content-Type": "application/json",
          "Content-Length": Buffer.byteLength(reqBody),
          "HTTP-Referer": "https://racingpoint.in",
          "X-Title": "MMA PACT-001 Phase 1 VERIFY",
        },
        timeout: 90000,
      },
      (res) => {
        let raw = "";
        res.on("data", (c) => (raw += c));
        res.on("end", () => {
          const elapsed_ms = Date.now() - startedAt;
          try {
            const json = JSON.parse(raw);
            const content = json.choices?.[0]?.message?.content || "";
            const usage = json.usage || {};
            const clean = content
              .replace(/^```(?:json)?\s*/i, "")
              .replace(/\s*```\s*$/i, "")
              .trim();
            resolve({
              ok: res.statusCode === 200,
              status: res.statusCode,
              model: model.id,
              role: model.role,
              vendor: model.vendor,
              elapsed_ms,
              usage,
              content: clean,
              raw_first_2k: raw.slice(0, 2000),
            });
          } catch (e) {
            resolve({
              ok: false,
              status: res.statusCode,
              model: model.id,
              error: e.message,
              raw_first_2k: raw.slice(0, 2000),
              elapsed_ms,
            });
          }
        });
      },
    );
    req.on("error", (e) =>
      resolve({ ok: false, model: model.id, error: e.message }),
    );
    req.on("timeout", () => {
      req.destroy();
      resolve({ ok: false, model: model.id, error: "TIMEOUT 90s" });
    });
    req.write(reqBody);
    req.end();
  });
}

(async () => {
  console.log(`MMA PACT-001 VERIFY @ ${TS}`);
  console.log(`Adversarial models: ${MODELS.map((m) => m.id).join(", ")}`);
  console.log("");

  const results = await Promise.all(MODELS.map(callModel));
  let totalCost = 0;
  const scores = [];

  for (const r of results) {
    const fname = r.model.replace(/[/:]/g, "_");
    fs.writeFileSync(path.join(OUT_DIR, `${fname}.json`), JSON.stringify(r, null, 2));
    const promptToks = r.usage?.prompt_tokens || 0;
    const compToks = r.usage?.completion_tokens || 0;
    const cost = promptToks * 0.0000015 + compToks * 0.000006;
    totalCost += cost;

    let mean = "?",
      verdict = "?",
      newFindings = "?";
    try {
      const parsed = JSON.parse(r.content || "{}");
      mean = parsed.mean ?? "no-mean";
      verdict = parsed.verdict ?? "no-verdict";
      newFindings = (parsed.new_findings || []).length;
      if (typeof parsed.mean === "number") scores.push(parsed.mean);
    } catch (e) {
      mean = `parse-fail: ${e.message}`;
    }

    console.log(
      `[${r.ok ? "OK" : "ERR"}] ${r.model.padEnd(38)} ` +
        `mean=${mean} verdict=${verdict} new=${newFindings} ` +
        `tokens=${promptToks}/${compToks} ~$${cost.toFixed(4)}`,
    );
    if (!r.ok && r.error) console.log(`    error: ${r.error}`);
  }

  const aggregateMean =
    scores.length > 0
      ? scores.reduce((a, b) => a + b, 0) / scores.length
      : NaN;
  const passGate = scores.length >= 2 && aggregateMean >= 4.0;

  console.log("");
  console.log(
    `Aggregate mean: ${aggregateMean.toFixed(2)} (n=${scores.length}/3 valid)`,
  );
  console.log(`PASS gate (≥4.0): ${passGate ? "YES" : "NO"}`);
  console.log(`Total estimated cost: ~$${totalCost.toFixed(4)}`);
  console.log(`Reports written to: ${OUT_DIR}`);
})();
