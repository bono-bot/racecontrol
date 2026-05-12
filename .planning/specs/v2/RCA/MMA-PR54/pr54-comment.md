# §S-146 retroactive RCA + §S-172 mechanism-trust-check filed

Both artifacts committed in `2a493043` + pushed to `origin/main` 2026-05-11 ~12:35 IST. Per Captain commission "Lets work on getting V2-LIVE-BLOCKING completed first" + explicit auth for MMA + PR-comment-posting 2026-05-11 ~12:49 IST.

## §S-146 5-section RCA

**Location:** `racecontrol/.planning/specs/v2/RCA/PR54-PACT-013-billing-paused-config-push-queue-20260511.md`

| § | Section | Result |
|---|---|---|
| 1 | Boundary map | 5 files cited path + line ranges; DB tables + IPC seams + V1↔V2 inheritance enumerated. V1 footprint in diff itself = 0; V1 inheritance via callsite context = indirect (billing_session table semantics unchanged). |
| 2 | Inherited-issue catalogue | 8 V1-shape footguns mapped across V1-process-mess Categories D/E/F/H/J + §S-61 PART 41 failure-mode #7. Boundary-relevant pattern: PR #49 was an instance of "ad-hoc wire variants atop V1 substrate without §S-146 RCA" — PR #54 is the structural close. |
| 3 | Past-bug review | 7 entries dispositioned: **3 ROOT-CAUSED-AND-FIXED** by this PR (Category F audit-blind proxy · Category E recovery-cascade re-arm · §S-61 #7 silent-message-drop on WS drop) · **1 UNRESOLVED open future-PACT** (HOT_RELOAD_FIELDS dynamic registry — retire trigger ≥3 hot-reload fields) · **3 NOT-APPLICABLE-TO-V2** (billing_session schema unchanged · Session 0/1 orthogonal · auth boundary orthogonal). |
| 4 | V2-alignment delta | 8 axes audited pre-PR-#54 vs V2 doctrine target. **PASS on all 8** (seq_num · ack · DB persist · audit trail · retry-on-disconnect · graceful version skew · wire surface stability · audit-trail behavior). |
| 5 | V2-framed change | V2 doctrine alignment statement included. Kaizen retention with explicit retire triggers (HOT_RELOAD_FIELDS allowlist · cross-platform path leak `.gitignore`). Rollback plan documented. |

## §S-172 mechanism-trust-check (config_push_queue delivery substrate)

**Location:** `racecontrol/.planning/specs/v2/MECHANISM-TRUST/config-push-queue-2026-05-11.json`

| Q | Question | Verdict |
|---|---|---|
| 1 | Atomic primitives? | YES — single SQL INSERT/UPDATE; AtomicU64 fetch_add for seq_num |
| 2 | TTL-bounded sentinels? | N/A — not a sentinel-class surface |
| 3 | Behavioral-verify success? | YES — DB row state machine (pending→delivered) + ConfigAck + config_audit_log |
| 4 | Single-target dry-run? | PARTIAL — substrate supports per-pod targeting; this PR's callsite delegates to Pod 8 canary per PACT-013 §EVIDENCE |
| 5 | Guard contracts? | YES — HOT_RELOAD_FIELDS parser-not-regex allowlist; NON_RELOAD_FIELDS rejection; warn+ignore on unknowns |

**Overall:** PASS · 30-day validity to 2026-06-10.

## MMA Step 1 DIAGNOSE (foundational-boundary escalation per §S-146)

**Surface:** `MMA-PR54-RCA-DIAGNOSE-bono-2026-05-11` · **Authorized by Captain** 2026-05-11 ~12:49 IST verbatim "authorize MMA and post the PR comment" · **Channel:** OpenRouter (Phase 3 ACTIVE — Perplexity deprecated for MMA 2026-05-11 G9 #1 close) · **Spend ledger:** `comms-link/data/openrouter-spend-bono.jsonl`

### Configuration

- **5 models attempted:** DeepSeek R1 (reasoner) · Qwen3-Coder (code expert) · Nemotron-3-Super (SRE) · Gemini 2.5 Pro (generalist) · Kimi K2.5 (reasoner alt) — **≥3 vendor families satisfied** (deepseek/qwen/nvidia/google/moonshot)
- **Results:** 2 fully complete (DeepSeek + Qwen) · 2 truncated at `max_tokens=4000` (Gemini Q1-Q2 · Kimi Q1 only) · 1 failed (Nemotron API error)
- **Elapsed:** 273s · **Spend:** $0.075
- **Result files:** `/tmp/mma-pr54-results/{deepseek,qwen,google,moonshot,nvidia}.md` (committed below)

### Consensus on RCA Quality

| Question | DeepSeek | Qwen | Gemini | Kimi | Consensus |
|---|---|---|---|---|---|
| Q1 RCA-completeness | AGREE-WITH-CAVEATS | AGREE-WITH-CAVEATS | AGREE-WITH-CAVEATS | AGREE-WITH-CAVEATS | **AGREE-WITH-CAVEATS** (4/4) |
| Q2 Delivery-substrate-soundness | AGREE | AGREE-WITH-CAVEATS | AGREE | n/a (truncated) | **AGREE** (3/3) |
| Q3 V1↔V2 boundary disposition | CORRECT | CORRECT | n/a | n/a | **CORRECT** (2/2 fully-responding) |
| Q3 Patch-V1-forward risk | LOW | NO | n/a | n/a | **LOW / NOT patch-forward** (2/2) |
| Q3 HOT_RELOAD_FIELDS retention | SOUND | SOUND | n/a | n/a | **SOUND** (2/2) |
| Q4 MERGE-DECISION | MERGE-WITH-AMENDMENTS | MERGE-WITH-AMENDMENTS | n/a | n/a | **MERGE-WITH-AMENDMENTS** (2/2) |

### Caveats flagged (common across ≥2 models)

1. **Stale 'pending' rows with no TTL/GC** (DeepSeek + Qwen + Gemini, UNANIMOUS) — if a pod never reconnects, `config_push_queue` rows remain 'pending' indefinitely; database bloat + misleading operational view. **Mitigation:** future-PACT for retention/TTL policy; not blocking PR #54.
2. **Partial commit inconsistency under server crash** (DeepSeek + Qwen + Gemini, UNANIMOUS) — INSERT 'pending' + WS send + UPDATE 'delivered' + INSERT audit_log are NOT in a DB transaction. Server crash mid-sequence leaves inconsistent state. **Mitigation:** Gemini notes idempotency on agent side (`failure_monitor.send_modify` is idempotent on bool flag) saves the behavioral correctness; doctrine gap is that RCA doesn't explicitly call out idempotency as a new critical consumer requirement.
3. **Seq_num overflow/wraparound** (DeepSeek + Qwen + Gemini) — `AtomicU64` at 1M ops/sec lasts ~584K years; doctrinally a gap but practically zero risk.
4. **Replacement integration test missing** (DeepSeek + Qwen + Kimi) — `test_billing_paused_via_config_push_roundtrip` mentioned in `protocol.rs` comment but not yet authored.

### MMA-recommended amendments before merge

| # | Amendment | Origin model(s) | Severity |
|---|---|---|---|
| 1 | Author replacement integration test `test_billing_paused_via_config_push_roundtrip` | DeepSeek + Qwen + Kimi | **HIGH** — closes §6 NOT TESTED #3 |
| 2 | Wrap INSERT queue + INSERT audit_log + UPDATE status in atomic DB transaction | DeepSeek | MEDIUM — partial-commit window |
| 3 | Linux build verification on James .27 | DeepSeek | LOW — CI ran on Windows runner (build SUCCESS); pre-existing Linux blockers are not introduced by this PR |
| 4 | Bono VPS deploy parity post-.23 | Qwen | MEDIUM — DEPLOY PARITY rule |
| 5 | File future-PACT: HOT_RELOAD_FIELDS dynamic registry (retire trigger ≥3 fields) | Qwen + DeepSeek (sound justification confirmed) | LOW — not blocking; tracked as future-PACT |
| 6 | File future-PACT: stale-pending-row TTL/GC policy | UNANIMOUS implied | LOW — substrate-class follow-up |

### MMA-recommended PR merge-disposition text (synthesis of DeepSeek + Qwen verbatim suggestions)

> "Per §S-146 + §S-172 + MMA Step 1 DIAGNOSE (4-model consensus AGREE-WITH-CAVEATS): PR #54 refactors billing_paused control plane from PR #49's ad-hoc wire variant to V2-P1-CONFIG-SERVICE substrate (DB-persisted + seq+ack + audit). Closes 3 V1-shaped footguns (audit-blind proxy-check · SESSION-01 orphan cascade · §S-61 #7 silent-message-drop). Merge authorized post-amendment #1 (replacement integration test); amendment #2 (atomic DB transaction) requested but non-blocking on idempotency-guarantee at consumer side; amendments #5+#6 tracked as future-PACTs. Fleet-roll sequence + Bono parity per DEPLOY PARITY rule."

### One-line consensus summary

- **DeepSeek:** RCA meets §S-146 bar; PR is V2-aligned substrate consolidation; minor gaps via merge amendments
- **Qwen:** RCA + PR aligns with §S-146 doctrine; removes V1 anti-pattern, routes through V2 substrate, full inherited-issue disposition

## Captain-pending after this comment lands

| # | Item | Class | Reason gating |
|---|---|---|---|
| 1 | **Per-PR merge auth on `racecontrol#54`** | foundational-boundary | §S-146 + §S-172 + MMA Step 1 require explicit per-PR Captain auth; standing-autonomy verbs do NOT satisfy |
| 2 | Fleet-roll sequence decision | infrastructure | Server roll (.23) first or atomic deploy-pipeline? Pod 8 canary per PACT-013 §EVIDENCE |
| 3 | Replacement integration test timing | engineering | `test_billing_paused_via_config_push_roundtrip` — pre-merge or post-merge? |
| 4 | Cloud parity (Bono VPS) | DEPLOY PARITY | Same deploy on Bono VPS after .23 per DEPLOY PARITY rule |

## What's known-good before merge

- **CI:** all 5 checks GREEN 2026-04-30 (build · API contract · Rust tests · security scan · comms-link QG)
- **PACT bilateral status:** RATIFIED-PROCEED-PHASE-0+1+2+3+4+5 by both AIs 2026-04-29
- **Phase 0 triage:** 3 substrate-compatibility checks (Q1/Q2/Q3) + bono evidence-add (.23 DB) all GREEN-LIGHT
- **Phase 0.5:** .23 venue DB verified — `config_push_queue` + `feature_flags` + `config_audit_log` all present

## NOT TESTED (per CGP H3)

- Linux cargo build of rc-agent (pre-existing Windows-only errors; CI ran on Windows runner)
- E2E manual-pause behavior post-deploy
- Replacement integration test (pending authoring)
- Cloud parity (Bono VPS racecontrol redeploy)
- Pod fleet roll (HOT_RELOAD_FIELDS extension requires rc-agent fleet roll)

---

— bono · 2026-05-11 ~12:49 IST · per Captain commission V2-LIVE-BLOCKING priority + explicit MMA + comment-post auth · awaiting per-PR merge disposition

🤖 Generated with [Claude Code](https://claude.com/claude-code)
