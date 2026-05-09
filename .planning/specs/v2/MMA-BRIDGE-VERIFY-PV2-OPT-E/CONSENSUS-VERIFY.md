# MMA Step 4 VERIFY — PV2-OPT-E Bridge — Adversarial Consensus Synthesis

- **Run:** 2026-05-09 ~22:50 IST
- **Models:** deepseek-r1-0528 + qwen3-coder + mistral-small-2603 (3 vendors, all Tier-1, all role-fit-validated per §S-166)
- **Wall:** 73.7s
- **Cost:** $0.0214 (cumulative MMA-day ~$0.80 / $5)
- **Captain trigger:** `mma-bridge-verify` 2026-05-09 ~22:40 IST
- **Override:** `MMA_FORCE_DUPLICATE=1` with reason "new RCA scope (operational bridge, distinct from prior Step 4 code-PR rounds)"

## VERDICT: BLOCK (3rd consecutive on rc-agent-fleet-deploy RCA)

| Round | PLAN | Avg score | Verdict |
|-------|------|-----------|---------|
| 1 | CONSENSUS-PLAN (atomic-chain + sentinel-respecting watchdog code change) | 2.12/5 | BLOCK |
| 2 | PIVOT (server-side mutex /exec_atomic_deploy endpoint) | 1.75/5 | BLOCK |
| **3** | **BRIDGE PV2-OPT-E (manual atomic chain + existing OTA_DEPLOYING sentinel)** | **2.28/5** | **BLOCK** |

## Per-model scoring

| Model | race_corr | sentinel | rollback | fm_cov | op_disc | concur | OVERALL |
|-------|-----------|----------|----------|--------|---------|--------|---------|
| deepseek-r1 | 1 | 3 | 2 | 1 | 3 | 2 | **2.00/5** |
| qwen3-coder | 2 | 3 | 2 | 3 | 4 | 2 | **2.67/5** |
| mistral-sm4 | 2 | 2 | 2 | 3 | 3 | 1 | **2.17/5** |
| **AVG** | **1.67** | **2.67** | **2.00** | **2.33** | **3.33** | **1.67** | **2.28/5** |

`OVERALL 2.28 << 4.0 PASS = BLOCK`

## CRITICAL P0 FINDING (deepseek-r1, surfaced via reasoning channel)

### **CR-FL-X — Sentinel suppresses watchdog RESTART, not just rollback**

> "The watchdog does not restart rc-agent when sentinel is set, so the deploy fails." — deepseek-r1

- **Source:** `crates/rc-watchdog/src/service.rs:288` — `tracing::info!("OTA_DEPLOYING active (action_id={}) — skipping restart cycle (SF-05)")`
- **Behavior:** When OTA_DEPLOYING file present, watchdog main loop SKIPS the restart cycle entirely (not just suppresses the rollback path at `rollback_manager.rs:121`)
- **Consequence (chicken-and-egg):**
  1. Bridge sets sentinel (4.2)
  2. Bridge taskkills rc-agent (4.3 atomic chain start)
  3. Watchdog observes: rc-agent gone + sentinel present → **SKIPS RESTART**
  4. Atomic chain copies new binary → file is in place but no process running
  5. Bridge polls /healthz → **TIMEOUT** (no process to respond)
  6. Bridge fires rollback (4.6) → re-sets sentinel + reverse atomic chain → STILL no restart (sentinel still active)
  7. Sentinel TTL=300s expires → watchdog finally allowed to restart → restarts whatever binary is now in place
- **Bridge's preflight gate is incomplete:** it checks `rollback_manager.rs` only, not `service.rs` SF-05 skip-restart logic
- **Severity:** P0 BLOCK — bridge architecture is dead-on-arrival if this behavior is in OLD c5f94e31 binary

**This was the PLAN's most consequential unverified assumption: that the watchdog would happily respawn the binary as long as we held the sentinel for the rollback path. That assumption is wrong if SF-05 also gates restart.**

## Convergent flaws (≥2 models)

| ID | Severity | Section | Flaw | Models |
|----|----------|---------|------|--------|
| **CR-FL-A** | P0 | 4.3 | Watchdog poll interval N unknown; race window between taskkill and copy unaddressed; even ping-delays inadequate | deepseek-r1 + qwen + mistral (3/3) |
| **CR-FL-B** | P0 | 4.2 | 1s flush insufficient for NTFS durability without `FlushFileBuffers`/`fsutil file flush`; watchdog may read empty/corrupt sentinel | qwen + mistral (2/3) |
| **CR-FL-C** | P0 | 4.6 | Rollback re-sets sentinel BEFORE checking if it auto-cleared at 300s; if T=305s, double-rollback race | deepseek-r1 + qwen + mistral (3/3) |
| **CR-FL-D** | P0 | 4.4 | rc-sentry SPOF for both atomic chain delivery AND healthcheck polling; no fallback if rc-sentry dies between sentinel-set and chain | deepseek-r1 + qwen + mistral (3/3) |
| **CR-FL-E** | P0 | 4.1 | Pod 1 prev binary integrity unverified (degraded-from-prior-failed-deploy); no MD5 match against archived OLD; rollback could restore corrupt binary | qwen + mistral (2/3) |
| **CR-FL-F** | P1 | 6, 11 | Concurrent parallel-james session interlock missing; INBOX notify ≠ hard interlock; no file/registry semaphore | qwen + mistral (2/3) |
| **CR-FL-G** | P1 | 4.4 | Healthcheck `build_id == 8e378f4d AND ws_connected == True` insufficient; no rc-agent.exe process liveness check | mistral (1/3 but high signal) |
| **CR-FL-H** | P1 | 3 | Preflight gate checks `rollback_manager.rs` only, not `service.rs` SF-05 skip-restart logic — directly enables CR-FL-X | mistral PV-BR-FL-07 + deepseek-r1 reasoning chain (2/3) |

## CQ answers (challenge questions)

| CQ | Question | Convergent answer |
|----|----------|-------------------|
| CQ1 | Watchdog poll interval N? | **NOT SPECIFIED in PLAN** — fundamental gap. Worst-case races unaddressed (3/3 models). |
| CQ2 | 1s flush enough on NTFS? | **NO** — needs FlushFileBuffers/fsutil (2/3 models, both substantively). |
| CQ3 | rc-sentry dies between sentinel-set and chain? | **PLAN goes silent** — Tailscale SSH "fallback" is informal, not procedural (3/3 models). |
| CQ4 | Rollback at T=305s after auto-clear? | **Double rollback race** unaddressed; PLAN has no sentinel-existence pre-check (3/3 models). |
| CQ5 | Pod 1 prev binary corrupt? | **No recovery path** beyond "HALT + Captain"; bridge is stuck (2/3 models explicitly). |
| CQ6 | OLD c5f94e31 rc-sentry endpoint coverage? | **Unverified** — PLAN assumes parity with current rc-sentry without test (2/3 models). |
| CQ7 | INBOX notify sufficient for collision? | **NO** — needs hard interlock (file lock, registry semaphore) (3/3 models unanimous). |

## Pattern recognition: 3 consecutive BLOCKs

| Round | Approach | Avg | Distinct flaw classes caught |
|-------|----------|-----|-----------------------------|
| 1 (CONSENSUS-PLAN) | Sentinel-respecting watchdog code + atomic chain | 2.12 | sentinel-before-chain, suppression, JSON parse, race timing, sc-start |
| 2 (PIVOT) | Server-side mutex /exec_atomic_deploy endpoint | 1.75 | Tokio Mutex cancellation, rc-sentry SPOF, Phase 1 circular dep, Pod 8 OLD-sentry 404, chaos tests, mutex poisoning |
| 3 (BRIDGE) | Manual atomic chain + existing sentinel | 2.28 | watchdog-skip-restart (SF-05), NTFS flush, rollback-after-auto-clear, rc-sentry SPOF (still!), Pod 1 prev integrity, INBOX-not-interlock |

**Inference (model-corroborated):** The deploy-mechanism RCA's solution space genuinely requires the kernel-level mutual exclusion that PV2-OPT-B Win32 LockFileEx provides. Each of 3 rounds catches different structural flaws because each approach has the same underlying architectural problem: an unprivileged userspace process trying to win a race against a service-level watchdog. None of the 3 approaches solves that race; they just rearrange where it manifests.

## Revised disposition options (PV3 series)

| Verb | Action | Cost | Time | Pods 1-7 exposure |
|------|--------|------|------|-------------------|
| **`PV3-OPT-A`** | Amend bridge PLAN with all 8 convergent flaws (CR-FL-A..H) + re-run Step 4 VERIFY adversarial (Round 4) | ~$0.04 | ~30min author + 2min verify | continues |
| **`PV3-OPT-B`** *(STRONG)* | **Pivot to PV2-OPT-B Win32 LockFileEx for next session.** Accept Pods 1-7 silent-loop-death exposure for ~tonight. Pod 8 canary stable; 6 pods serve customers degraded. | $0 | next session | ~tonight (8-12h) |
| **`PV3-OPT-C`** | Single-pilot Pod 2 ONLY with bridge as written + accept the 2.28/5 risk; observe 24h; do NOT ramp; if pilot survives, revisit | $0 | ~30min | continues for 5 pods |
| **`PV3-OPT-D`** | HYBRID: ship CR-FL-A+B+C+D+E+H amendments only (skip CR-FL-F+G as P1); re-run Step 4 VERIFY (Round 4) at ~$0.05 | ~$0.05 | ~30min author + 2min verify | continues |
| **`PV3-OPT-E`** | Halt bridge entirely; document 3-BLOCK pattern as DEFER-TO-LOCKFILEEX + log G9 candidate "MMA-iteration-on-architecturally-flawed-RCA produces multi-round BLOCK without convergence" | $0 | ~5min | continues |

**RECOMMEND `PV3-OPT-B`**: 3 consecutive BLOCKs at 2.12 → 1.75 → 2.28 with each round catching DIFFERENT structural flaws is the strongest possible signal that the LockFileEx pivot was the correct reading of the RCA. Spending another iteration on the bridge is throwing good money after bad. Accept ~tonight's exposure; ship LockFileEx properly next session.

**Composes-with**: §S-146 V1↔V2 RCA doctrine · §S-150 PR #66 silent-loop-death · §S-159 pre-MMA-duplicate-check hook (3rd successful gate this RCA) · §S-166 model-role-fit code enforcement · sentinel discipline rule PROMOTE-NOW-ACTIVE · per-PR Captain auth.
