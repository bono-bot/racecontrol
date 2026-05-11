#!/usr/bin/env node
/**
 * Focused MMA Cross-System Bridge audit for PACT-001 Phase 1 wire-up.
 *
 * Scope: cirs_lookup HTTP handler ↔ TypeScript UI ↔ POS .130 surface bridge.
 * Branch: feat/pact-001-phase-1-wireup HEAD 12632519.
 *
 * Out-of-scope codebase areas (auth fundamentals, billing, fleet, agent) — the
 * full-codebase MMA at scripts/multi-model-audit.js covers those at $4-7. This
 * script targets just the Phase 1 delta at ~$0.25-0.50.
 *
 * 5 models stratified ≥3 vendor families, max 2 per vendor; ≥1 reasoner +
 * ≥1 code-expert + ≥1 SRE per the Unified MMA Protocol v3.0 stratification.
 *
 * Output: audit/results/mma-pact001-phase1-{ts}/{model}.json with raw findings.
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
const OUT_DIR = path.join(__dirname, "results", `mma-pact001-phase1-${TS}`);
fs.mkdirSync(OUT_DIR, { recursive: true });

// Stratified 5-model selection per Unified MMA Protocol v3.0:
//   2× DeepSeek (reasoner + code-expert)
//   1× Qwen (code-expert)
//   1× Xiaomi (SRE)
//   1× Google (generalist)
// = 4 vendor families, max 2 per vendor (DeepSeek), ≥1 of each required role.
const MODELS = [
  { id: "deepseek/deepseek-r1-0528", role: "reasoner", vendor: "deepseek" },
  { id: "deepseek/deepseek-v3.2-exp", role: "code-expert", vendor: "deepseek" },
  { id: "qwen/qwen3-coder", role: "code-expert", vendor: "qwen" },
  { id: "xiaomi/mimo-v2-pro", role: "sre", vendor: "xiaomi" },
  { id: "google/gemini-2.5-flash", role: "generalist", vendor: "google" },
];

const PROMPT = `You are auditing a cross-system bridge in a Rust+TypeScript monorepo.

CONTEXT
-------
PACT-001 Phase 1 wire-up: customer-identity-resolution (CIRS) HTTP surface for POS staff terminal.

Wire path:
  POS staff terminal (Edge kiosk on Windows)
    → web-v2 Next.js page (/v2/pos/lookup)
    → fetch POST /api/v1/cirs/lookup (relative URL — Next.js proxy assumed)
    → racecontrol Rust/Axum :8080 cirs_lookup_handler
    → v2-db SQLite (customers + wallets + sessions + cirs_lookup_audit)

Auth: staff session JWT cookie + role check; non-privileged action (no PIN re-entry).
Idle-timeout: 30 min from token iat (NOT sliding window — see KNOWN CONCERNS).

KEY INVARIANTS
--------------
1. record_lookup discipline: every CIRS HTTP invocation MUST write a cirs_lookup_audit row
   regardless of Found/NotFound/Error outcome. SHA256 input hash for phone/QR/NFC; NULL for walk_in_guest.
2. ProfilePreview wire field is "balance_credits" NOT "wallet_balance_credits" (§AMEND-1.A).
3. PrivilegedAction enum at racecontrol/src/auth/privileged_actions.rs has 12 variants in 4 categories.
   TS mirror at web-v2/src/lib/types/privileged-action.ts must serialize to identical snake_case strings.
4. Indian-mobile-prefix WARN gate at UI layer ONLY (digit[0] ∈ {6,7,8,9}); substrate stays permissive
   for E.164 international customers. UI strips non-ASCII digits via JS /\\D/ before substrate call.
5. M3 (QR) and M4 (NFC) plumbed-disabled via FeatureFlag — handler returns 501.
6. Single shared wallet per family (Wallet Framing C); balance_credits is family-pool not per-profile.
7. Walk-In Guest 1+2: customer_id=NULL on substrate call; record_lookup writes input_method='walk_in_guest_id' result='not_found'.

ALREADY-SHIPPED TEST COVERAGE
-----------------------------
- 1070 cargo tests (was 1069; +1 perf smoke this PR) — debug-build PASS
- 23 vitest tests on PrivilegedAction TS↔Rust mirror invariants + ManagerPill render-state
- 5 Playwright E2E specs covering phone Found / NotFound / Arabic-Indic / Walk-In Guest 1 / WARN gate
- Perf smoke: p95=188us against 10K-row dataset (DoD §1.2 budget = 500ms)

KNOWN CONCERNS (already self-flagged, NOT what we want from you)
----------------------------------------------------------------
A. Cross-layer naming drift: TS ProfilePreview uses { full_name, arrival_count_30d } but Rust
   ProfilePreview is { name, arrival_history_count_30d }. TS missing primary_phone, last_visit_ts,
   discount_ineligible. This is a pre-existing wire-format mismatch.
B. Idle-timeout uses claims.iat (issue-time), not last-activity time. Captain §S-82 Q3 said
   "sliding window" — implementation is fixed-window-from-issue. Sliding refresh is deferred to
   "Session 6 PR-readiness" per inline comment.
C. Synthetic single-element profiles[] used because customer_profiles table not yet migrated
   (Q-CUSTOMER-PROFILES-1 dispositioned (a) by Captain — known reversible).

INPUT — TYPESCRIPT WIRE TYPES (web-v2/src/lib/types/cirs-lookup.ts)
--------------------------------------------------------------------
\`\`\`typescript
export type CirsLookupRequest =
  | { method: "phone"; phone: string }
  | { method: "qr_payload"; qr_payload: string }
  | { method: "nfc_tag_id"; nfc_tag_id: string }
  | { method: "walk_in_guest_id"; walk_in_guest_id: 1 | 2 };

export interface ProfileSummary {
  profile_id: string;
  full_name: string;
  is_default: boolean;
  discount_ineligible: boolean;
}

export interface ProfilePreview {
  customer_id: string;
  full_name: string;
  balance_credits: number;
  arrival_count_30d: number;
  profiles: ProfileSummary[];
}

export type CirsLookupResponse =
  | { status: "found"; profile: ProfilePreview }
  | { status: "not_found"; reason: string }
  | { status: "error"; code: string; message: string };

export function isIndianMobilePrefix(phone: string): boolean {
  if (phone.length === 0) return true;
  const first = phone[0];
  return first === "6" || first === "7" || first === "8" || first === "9";
}
\`\`\`

INPUT — RUST WIRE STRUCT (cirs_lookup.rs excerpt)
--------------------------------------------------
\`\`\`rust
pub struct ProfilePreview {
    pub customer_id: String,
    pub primary_phone: String,
    pub name: String,                       // <-- TS expects "full_name"
    pub profiles: Vec<ProfileSummary>,
    pub balance_credits: i64,
    pub last_visit_ts: Option<String>,
    pub arrival_history_count_30d: u32,     // <-- TS expects "arrival_count_30d"
    pub discount_ineligible: bool,
}

pub struct ProfileSummary {
    pub profile_id: String,
    pub name: String,                       // <-- TS expects "full_name"
    pub is_default: bool,
    pub discount_ineligible: bool,
}

// Discriminated tag for wire format — matches TS via serde rename_all
#[derive(...)]
#[serde(tag = "method", rename_all = "snake_case")]
pub use v2_db::cirs::LookupInput as CirsLookupRequest;
// LookupInput::Phone { phone }, ::QrPayload { payload },
// ::NfcTagId { tag_id }, ::WalkInGuestId { guest_id: u8 }
\`\`\`

NOTE: TS sends \`{ method: "qr_payload", qr_payload: string }\` but Rust expects
\`LookupInput::QrPayload { payload: String }\`. Field names differ ("qr_payload" vs "payload").
Same for nfc_tag_id ("nfc_tag_id" vs "tag_id") and walk_in_guest_id ("walk_in_guest_id" vs "guest_id").

INPUT — IDLE TIMEOUT IMPL (auth/middleware.rs)
------------------------------------------------
\`\`\`rust
pub(crate) fn is_idle_expired(claims: &StaffClaims, idle_timeout_secs: u64) -> bool {
    if idle_timeout_secs == 0 {
        return false;
    }
    let now = chrono::Utc::now().timestamp();
    let iat = claims.iat as i64;
    let elapsed = now.saturating_sub(iat);
    elapsed > idle_timeout_secs as i64
}
\`\`\`

INPUT — POS PAGE FETCH (web-v2/src/app/pos/lookup/page.tsx excerpt)
--------------------------------------------------------------------
\`\`\`typescript
async function callLookup(body: CirsLookupRequest): Promise<CirsLookupResponse> {
  try {
    const res = await fetch("/api/v1/cirs/lookup", {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!res.ok) {
      return {
        status: "error",
        code: \`HTTP_\${res.status}\`,
        message: "Lookup endpoint returned … Proxy wiring may be pending",
      };
    }
    return (await res.json()) as CirsLookupResponse;
  } catch (e) {
    return { status: "error", code: "NETWORK_ERROR", message: ... };
  }
}
\`\`\`

YOUR JOB
--------
Find P0 (deploy-blocker / customer-impact) and P1 (PR-blocker / regression-class) bugs in this
cross-system bridge that the human review process and 1098 existing tests may have MISSED. Focus on:

  1. Cross-layer wire-format mismatches (besides the already-flagged ones above)
  2. Auth boundary gaps (token replay, missing role check, missing rate limit, CSRF on POST)
  3. Lock ordering / async safety (locks held across .await in axum handlers)
  4. Concurrent-modification semantics (double-submit, race between record_lookup and pool)
  5. Blast-radius miscalculation (one user's lookup affecting another's audit row, etc)
  6. DPDP / privacy gaps (phone number in logs, error responses, audit hash collisions)
  7. JSON shape edge cases (null handling, unknown fields, numeric overflow)
  8. POS .130 deployment-time gotchas (basePath /v2 doubling, Next.js proxy, CORS)

OUTPUT FORMAT — strict JSON, no prose, no markdown:
\`\`\`json
{
  "model_self_id": "<your model id>",
  "findings": [
    {
      "id": "F1",
      "severity": "P0|P1|P2",
      "class": "wire-format|auth|lock|race|blast-radius|privacy|json-edge|deploy",
      "title": "<one-line>",
      "evidence": "<file:line or specific code reference>",
      "fix_sketch": "<smallest reversible fix>",
      "confidence": 0.0-1.0
    }
  ],
  "ship_recommendation": "SHIP_AS_IS | FIX_BEFORE_PR | NEEDS_DESIGN_DECISION"
}
\`\`\`

Max 8 findings. If no P0/P1 found, output \`"findings": []\` and SHIP_AS_IS.
Do NOT report on the 3 KNOWN CONCERNS (A/B/C) above — those are already in the disposition queue.
Be terse. JSON only. No surrounding markdown fence.`;

async function callModel(model) {
  const reqBody = JSON.stringify({
    model: model.id,
    messages: [{ role: "user", content: PROMPT }],
    max_tokens: 3000,
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
          "X-Title": "MMA PACT-001 Phase 1 Cross-Bridge Audit",
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
            // Strip markdown fences if model wrapped output
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
  console.log(`MMA PACT-001 Cross-Bridge Audit @ ${TS}`);
  console.log(`Models: ${MODELS.map((m) => m.id).join(", ")}`);
  console.log(`Output dir: ${OUT_DIR}`);
  console.log("");

  // Run all 5 models in parallel — they're independent.
  const results = await Promise.all(MODELS.map(callModel));

  let totalCost = 0;
  for (const r of results) {
    const fname = r.model.replace(/[/:]/g, "_");
    fs.writeFileSync(path.join(OUT_DIR, `${fname}.json`), JSON.stringify(r, null, 2));
    const promptToks = r.usage?.prompt_tokens || 0;
    const compToks = r.usage?.completion_tokens || 0;
    // Rough OpenRouter pricing: ~$0.15 / 1M input, ~$0.60 / 1M output (varies)
    const cost = (promptToks * 0.0000015 + compToks * 0.000006);
    totalCost += cost;
    const findings = (() => {
      try {
        return JSON.parse(r.content || "{}").findings?.length || 0;
      } catch (e) {
        return "parse-fail";
      }
    })();
    console.log(
      `[${r.ok ? "OK" : "ERR"}] ${r.model.padEnd(40)} ` +
        `findings=${findings} ` +
        `tokens=${promptToks}/${compToks} ` +
        `elapsed=${r.elapsed_ms}ms ` +
        `~$${cost.toFixed(4)}`,
    );
    if (!r.ok && r.error) console.log(`    error: ${r.error}`);
  }
  console.log("");
  console.log(`Total estimated cost: ~$${totalCost.toFixed(4)}`);
  console.log(`Reports written to: ${OUT_DIR}`);
})();
