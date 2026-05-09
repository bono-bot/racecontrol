> **⛔ STATUS — BLOCKED-AT-STEP-4-VERIFY (2026-05-09 ~20:24 IST)**
>
> MMA Step 4 VERIFY adversarial gate scored this PLAN at **2.12/5 overall** (PASS threshold = 4.0); both completed models returned BLOCK.
>
> **DO NOT author PR commits from this PLAN until Captain dispositions amendments.** See `../MMA-DEPLOY-RCA-STEP4/CONSENSUS-VERIFY.md` for:
> - 5 convergent P0/P1 flaws (FL-CONV-1 sentinel-before-chain silent-fleet-death · FL-CONV-2 Pod-8-OLD-watchdog-suppresses-indefinitely · FL-CONV-3 JSON parse fail policy unspecified · FL-CONV-4 race timing analysis missing · FL-CONV-5 sc-start-failure unhandled)
> - Sonnet structural critique: `single_exec_chain` is "fragile by construction"; gemini's deferred `new_atomic_endpoint` is structurally correct alternative
> - 5 disposition options (A respin Step 2 / B pivot to atomic endpoint / C amend inline / D halt rollout / E manual bridge)
>
> Captain Q-DECISION required before any code is written from this PLAN.

---

# MMA Step 2 PLAN — CF-1 + CF-2 bundle — CONSENSUS

**Captain-authorized**: 2026-05-09 ~19:50 IST ("authorize MMA Step 2 PLAN for CF-1+CF-2+CF-9 bundle" → narrowed to CF-1+CF-2 per kaizen-min recommendation)
**Consumes**: `MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md` (CF-1 5/5 + CF-2 5/5)
**Models**: 5 vendor-disjoint (deepseek-r1 / grok-code-fast / mimo-v2-pro / gemini-2.5-flash / kimi-k2.5)
**Vendor families**: 5 (deepseek, xai, xiaomi, google, moonshot) ≥3 ✓
**Roles**: ≥1 reasoner (r1, kimi) + ≥1 code expert (grok) + ≥1 SRE (mimo) ✓
**Wall time**: 222.8s (kimi/r1 longest; grok+gemini under 11s)
**Cost**: $0.0474 / $5 budget (all 5 valid responses)
**Hooks**: pre-mma-duplicate-check passed (Step 2 ≠ Step 1 same RCA in 60-min window)

---

## §1 — Scope decision (consensus)

| Dimension | CF-1 approach | CF-2 approach |
|---|---|---|
| **5/5 (gemini dissent on CF-1 only)** | `single_exec_chain` via Windows CMD `&` operator (4/5) | `ttl_json_body` mirroring `auto_clear_maintenance_mode_json` (5/5) |
| **Dissent (gemini)** | `new_atomic_endpoint` (`/exec_atomic_deploy`) — more code (~200 LOC) but bypasses BLOCKED_PATTERNS concerns entirely | — |

**Selected**: `single_exec_chain` for CF-1 (kaizen-min, 4/5 consensus, leverages allowed single `&`); `ttl_json_body` for CF-2 (5/5 unanimous, mirrors proven BUG-71 pattern).

**Rationale**: New endpoint (gemini's variant) is over-engineered for CF-1 alone — it introduces a structural change to rc-sentry that's better justified when bundled with CF-9 (watchdog deploy-aware) in a future PR. Smallest reversible bundle = single-chain + JSON sentinel.

---

## §2 — Files touched (frequency across 5 models)

| File | Models | Action class |
|---|---|---|
| `crates/rc-watchdog/src/rollback_manager.rs` | **5/5** | TTL JSON parsing + auto-clear helper + perform_rollback suppression check refactor |
| `scripts/deploy-pod.sh` | **5/5** | Atomic /exec chain + JSON sentinel set/clear |
| `crates/rc-watchdog/src/service.rs` (a.k.a. main loop) | **3/5** | Periodic `auto_clear_ota_deploying_json(300)` invocation |
| `crates/rc-sentry/src/main.rs` | 2/5 (gemini, mimo dissent) | NOT NEEDED for selected approach |
| `crates/rc-sentry/src/lib.rs` | 1/5 (gemini) | NOT NEEDED |

---

## §3 — Action plan (consensus-merged, 7 steps)

| ID | File | Kind | Summary | LOC | Risk | Risk reason |
|---|---|---|---|---|---|---|
| **A1** | `crates/rc-watchdog/src/rollback_manager.rs` | edit | Add `auto_clear_ota_deploying_json(max_age_secs: u64) -> bool` mirroring `auto_clear_maintenance_mode_json` (JSON timestamp_epoch + mtime fallback) | ~30 | low | Mirrors proven BUG-71 pattern |
| **A2** | `crates/rc-watchdog/src/rollback_manager.rs` | edit | Replace bare `ota_deploying.is_file()` check (line 121) with TTL-aware JSON check; preserve mtime fallback for legacy bare-file sentinels (Pod 8 backward compat) | ~15 | medium | Modifies rollback suppression — clock skew or malformed JSON could permit rollback during active deploy. Mitigated by mtime fallback. |
| **A3** | `crates/rc-watchdog/src/service.rs` | edit | Call `auto_clear_ota_deploying_json(OTA_TTL_SECS)` in main poll loop alongside `auto_clear_maintenance_mode_json` | ~5 | low | Adds one function call to existing cleanup routine |
| **A4** | `crates/rc-watchdog/src/rollback_manager.rs` | edit | Define `OTA_TTL_SECS = 300` constant; document semantics | ~3 | low | Constant definition; matches MAINTENANCE_MODE 30-min equivalent scaled to OTA window |
| **A5** | `scripts/deploy-pod.sh` | edit | Replace 3 separate /exec calls (lines 170-188) with single atomic chain: `taskkill /F /IM rc-agent.exe & del /Q rc-agent-prev.exe & ren rc-agent.exe rc-agent-prev.exe & ren rc-agent-new.exe rc-agent.exe & echo SWAPPED` (single `&`, NOT `&&` — BLOCKED_PATTERNS-safe) | ~25 | medium | Long single command; set timeout_ms ≥30000 in curl payload to handle slow copies |
| **A6** | `scripts/deploy-pod.sh` | edit | Replace bare-file `OTA_DEPLOYING` sentinel write (line 167) with JSON body: `echo {"timestamp_epoch":$(date +%s)} > C:\RacingPoint\OTA_DEPLOYING` | ~5 | low | Adds epoch write before atomic chain; sentinel removed post-healthcheck (existing flow) |
| **A7** | `crates/rc-watchdog/src/rollback_manager.rs` | new (test) | Add unit tests: `test_auto_clear_ota_json_fresh_preserved`, `test_auto_clear_ota_json_expired_removed`, `test_auto_clear_ota_legacy_bare_mtime_fallback` | ~50 | low | Isolated tests; no prod impact |

**Total**: ~133 LOC code + ~50 LOC tests = ~183 LOC. PR shape: SMALL-MEDIUM (≤200 LOC = comfortable single-PR review).

---

## §4 — Test plan (consensus-merged)

| ID | Kind | What | Expected |
|---|---|---|---|
| **T1** | unit | `auto_clear_ota_deploying_json(300)` with fresh JSON `{timestamp_epoch: now-30}` | Returns `false`; file remains |
| **T2** | unit | `auto_clear_ota_deploying_json(300)` with expired JSON `{timestamp_epoch: now-400}` | Returns `true`; file removed |
| **T3** | unit | `auto_clear_ota_deploying_json(300)` with legacy bare-file sentinel (mtime 400s old) | Falls back to mtime; returns `true`; file removed |
| **T4** | integration | Mock rc-sentry `/exec`; run `deploy-pod.sh` against it | Receives exactly **one** POST containing `taskkill ... & del ... & ren ... & ren ...` chain; OTA_DEPLOYING JSON written before POST |
| **T5** | live-pod | Deploy to **Pod 1** (NOT Pod 8 — canary stands) using new script + new rc-watchdog | New binary persists; watchdog log shows `OTA_DEPLOYING sentinel present — suppressing rollback` then sentinel auto-clear; **zero rollback events** for new binary across 5-min observation window |

---

## §5 — Rollback plan

If PR breaks Pods 1-7 fleet rollout:
1. `git revert <pr-merge-commit>` on `feat/v2-wave-1-w1-s1-billing-service` (or whichever branch hosts the merge)
2. Rebuild rc-watchdog + redeploy to affected pods (rc-watchdog is a Windows Service — install via existing rc-sentry `/exec` invocation: `sc stop RCWatchdog & copy /Y rc-watchdog-prev.exe rc-watchdog.exe & sc start RCWatchdog`)
3. Old rc-watchdog handles bare-file OTA_DEPLOYING (backward-compat path of new code retains this — so partial rollback may not even be needed)
4. If JSON sentinel exists from new flow but old watchdog reads it: bare `is_file()` check still works; OTA suppression still fires (just no TTL). Safe.
5. Manual cleanup if needed: `del C:\RacingPoint\OTA_DEPLOYING` via rc-sentry `/exec`

**Zero-blast-radius rollback**: backward compat preserved both directions (new code reads JSON OR mtime; old code reads bare file).

---

## §6 — Verify post-deploy (per-pod, after Pod 1 canary success)

| Step | Command / Observation | Pass criteria |
|---|---|---|
| 1 | `curl http://192.168.31.89:8090/health \| jq .build_id` (Pod 1) | Matches deployed hash (e.g., `8e378f4d` for PR #66 redeploy verification) |
| 2 | rc-sentry `/exec`: `dir C:\RacingPoint\OTA_DEPLOYING` | File **absent** (cleared post-healthcheck) OR if present, `type` shows valid JSON with recent timestamp_epoch |
| 3 | rc-sentry `/exec`: `dir C:\RacingPoint\rc-agent-prev.exe` | File exists (rollback binary preserved) |
| 4 | rc-watchdog log scan (Windows Event Log or local file): `findstr "OTA_DEPLOYING\|rollback" C:\RacingPoint\logs\rc-watchdog.log` | Contains `OTA_DEPLOYING sentinel present — suppressing rollback` during deploy window; **zero `Rollback complete:`** entries within 5min of deploy |
| 5 | Repeat steps 1-4 sequentially for Pods 2-7 (NOT POS — out of scope; NOT Pod 8 — canary stands) | All pods report new build_id; no rollback events |

**Blast radius**: `fleet_pods` (Pods 1-7; Pod 8 untouched as canary).

---

## §7 — Captain Q-DECISIONs (consensus open questions)

| ID | Question | Default recommendation | Source models |
|---|---|---|---|
| **CF12-Q1** | OTA_DEPLOYING TTL value: 300s vs 600s? | **300s** (matches MAINTENANCE_MODE pattern; 4/5 default) | gemini (300), kimi (300), grok (300), mimo (300) — deepseek (600) outlier |
| **CF12-Q2** | TTL configurable via env var, or fixed constant? | **Fixed constant `OTA_TTL_SECS = 300`** (mirrors MAINTENANCE_MODE which has no env override; reduce surface) | mimo flagged; default kaizen-min |
| **CF12-Q3** | Should new rc-watchdog read AND honor old bare-file sentinel during rollout window? | **YES** — `auto_clear_ota_deploying_json` already includes mtime fallback (consistent with MAINTENANCE_MODE pattern); zero migration risk | All 5 models implicitly via mtime fallback inclusion |
| **CF12-Q4** | rc-watchdog deploy mechanism for this PR? Service stop+copy+start via rc-sentry `/exec`, OR separate rc-watchdog deploy script? | **Reuse `scripts/deploy-sentry.sh` pattern** if exists, OR add `scripts/deploy-watchdog.sh` mirror — single `/exec` chain: `sc stop RCWatchdog & ... & sc start RCWatchdog` | Not directly addressed; my synthesis-judgment based on single-author session RCA Issue 9 lessons |
| **CF12-Q5** | Pre-merge MMA Step 4 VERIFY adversarial gate REQUIRED, or proceed direct to Step 3 EXECUTE? | **Step 4 RECOMMENDED** (~$0.04 — adversarial models score this PLAN; first beneficiary of pre-MMA-duplicate-check hook §S-159 since no Step 4 in this RCA's last 60min) | Default per Protocol v3.0 |

---

## §8 — Dependencies / sequencing

- **No dependency on CF-3 (dry-run+preflight)** — that's a wrapper around this PR; this PR is mergeable independently
- **No dependency on CF-9 (watchdog deploy-aware)** — orthogonal change to watchdog poll-rate; can ship as second PR after this one lands
- **Composes-with `project_session_pr66_deploy_session_rca_20260509.md`** — directly addresses Issue 6 (kill+swap race) + Issue 9 (manual atomic-chain failed because OTA sentinel omitted) from that RCA
- **Prerequisite for unblocking Pods 1-7**: this PR must land + rc-watchdog must redeploy to Pods 1-7 BEFORE re-attempting PR #66 fleet rollout via `deploy-pod.sh`. Pod 8 stays on PR #66 binary `8e378f4d` throughout (no touch).

---

## §9 — Minority/dissent capture

- **gemini-2.5-flash** proposed `new_atomic_endpoint` (`/exec_atomic_deploy`) for CF-1 — adds ~200 LOC vs ~25 for chain approach. **Disposition: DEFER** — better justified when bundled with CF-9 (watchdog deploy-aware) in subsequent PR; preserves option without committing now.
- **deepseek-r1** proposed 600s TTL vs consensus 300s — minor; per default recommendation 300s.
- **mimo-v2-pro** proposed `blast_radius: single_pod` — likely cautious-correct for canary-first rollout; my synthesis adopts fleet_pods because Pod 1 canary success unblocks Pods 2-7.

---

## §10 — What I observed (CGP H3 evidence)

**BEHAVIOR**: Authored Step 2 PROMPT.md (5784 chars), invoked 5 OpenRouter models in parallel, parsed JSON responses, clustered findings into 7-action consensus PLAN.

**RAW OUTPUT**:
- `.planning/specs/v2/MMA-DEPLOY-RCA-STEP2/resp-<5-models>.txt` (per-model PLAN JSON, 4304-6712 chars each)
- `.planning/specs/v2/MMA-DEPLOY-RCA-STEP2/results.json` (full results + metadata)
- `comms-link/data/openrouter-spend-james.jsonl` (this Step 2 entry appended)

**WHERE**: Models invoked from James .27 (this terminal); responses written to local files. No live-pod testing yet (Step 3 EXECUTE territory).

**NOT TESTED**:
- T1-T7 of §4 — none yet executed (planning artifact only)
- Whether atomic chain actually executes within rc-sentry timeout_ms — needs T4 mock + T5 live-pod
- Whether new rc-watchdog binary deploys cleanly via `sc stop/start RCWatchdog & copy & sc start` chain — needs separate deploy-sentry/watchdog test
- Whether Pod 8 backward-compat actually works (canary on OLD rc-watchdog reading JSON-format sentinel from new deploy script) — needs explicit cross-version test
- MMA Step 4 VERIFY adversarial gate not yet run — Captain Q-DECISION CF12-Q5
- Bono cross-check / AMPLIFIER — single-pilot synthesis (not MMA-substitute-pilot territory; bilateral discipline pending)
- Independence assumption — 4/5 models converge on `single_exec_chain` because CLAUDE.md "Remote deploy sequence" is in the prompt; consensus may reflect prompt-anchoring bias

---

## §11 — Open items for Captain (next-step decision)

1. **Approve PLAN as-is** → I author the PR (commits A1-A7) then run live Pod 1 test (T5) before PR-open
2. **Approve PLAN + run MMA Step 4 VERIFY first** (~$0.04) → adversarial models score this PLAN; first beneficiary of `pre-mma-duplicate-check` hook pre-Step-4 path
3. **Amend PLAN** — disposition for any of CF12-Q1..Q5 different from defaults
4. **Defer to bono AMPLIFIER** — INBOX notify for cross-pilot review before Step 3 EXECUTE (adds ~24h CHALLENGE-AMEND window per PACT cascade L0)
5. **Bundle with CF-9** after all — re-open Step 2 to include watchdog deploy-aware mode in same PR (rejects kaizen-min posture; ~+150 LOC, +1 file)

---

## §12 — Spend log entry (appended)

```json
{
  "timestamp": "2026-05-09T14:18:35Z",
  "ts_ist": "2026-05-09 19:48 IST",
  "pilot": "james",
  "session_purpose": "MMA Step 2 PLAN — CF-1+CF-2 bundle (atomic swap + TTL OTA sentinel) — deploy-mechanism RCA",
  "mma_step": "PLAN",
  "models": ["deepseek/deepseek-r1-0528","x-ai/grok-code-fast-1","xiaomi/mimo-v2-pro","google/gemini-2.5-flash","moonshotai/kimi-k2.5"],
  "vendor_families": ["deepseek","xai","xiaomi","google","moonshot"],
  "valid_responses": 5,
  "total_responses": 5,
  "total_cost_usd": 0.0474,
  "anchor": ".planning/specs/v2/MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md",
  "consumes_step1": ".planning/specs/v2/MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md (CF-1 5/5 + CF-2 5/5)",
  "authorization": "Captain G33 2026-05-09 ~19:50 IST",
  "consensus_actions": 7,
  "consensus_tests": 5,
  "captain_q_decisions": 5,
  "minority_dissent": "gemini new_atomic_endpoint (DEFER); deepseek 600s TTL (REJECT for 300s default)"
}
```

---

— james / 2026-05-09 ~19:55 IST · MMA Step 2 PLAN complete · 5/5 valid · 7-action consensus · 5 Captain Q-DECISIONs queued · awaits disposition on §11
