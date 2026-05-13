# MMA Step 4 VERIFY Round 4 — v0.4 hardened-script PLAN — Adversarial Consensus Synthesis

- **Run:** 2026-05-13 ~15:05 IST
- **Models:** z-ai/glm-5 (zhipu, reasoner) + kwaipilot/kat-coder-pro-v2 (kwai, code_expert) + nvidia/nemotron-3-super-120b-a12b (nvidia, SRE) — 3 distinct vendor families, all fresh-to-this-RCA
- **Wall:** 159s
- **Cost:** $0.0309 (cumulative MMA-day ~$0.83 / $5)
- **Captain trigger:** disposition (A) verbatim 2026-05-13 ~14:48 IST: "drive Step 4 VERIFY to PASS on a v0.4 plan ... or commit to the LockFileEx pivot direction with a fresh PLAN. Authorize ~$0.05-0.10 MMA spend."
- **Override:** `MMA_FORCE_DUPLICATE=1` with reason "Round 4 VERIFY on v0.4 hardened-script PLAN — distinct architecture (Option Y) from Rounds 1-3 (LockFileEx-direction)"

## VERDICT: BLOCK (4th consecutive on rc-agent-fleet-deploy RCA)

| Round | PLAN | Avg score | Verdict |
|-------|------|-----------|---------|
| 1 | CONSENSUS-PLAN (atomic-chain + sentinel-respecting watchdog code change) | 2.12/5 | BLOCK |
| 2 | PIVOT (server-side mutex /exec_atomic_deploy endpoint) | 1.75/5 | BLOCK |
| 3 | BRIDGE PV2-OPT-E (manual atomic chain + existing OTA_DEPLOYING sentinel) | 2.28/5 | BLOCK |
| **4** | **v0.4 Option Y (hardened deploy-pod-agent.sh + 6 script additions)** | **2.62/5** | **BLOCK** |

## Per-model scoring

| Dimension | glm-5 | kat-coder-pro | nemotron-super | AVG |
|-----------|-------|---------------|----------------|-----|
| 1. substrate_evolution_validity | 3 | 2 | 2 | 2.33 |
| 2. cr_fl_x_closed | 3 | 3 | 3 | 3.00 |
| 3. cr_fl_a_race_window | 3 | 2 | 3 | 2.67 |
| 4. cr_fl_e_prev_integrity | 2 | 2 | 2 | 2.00 |
| 5. cr_fl_f_flock_semantic | 2 | 2 | 3 | 2.33 |
| 6. cr_fl_g_healthcheck | 3 | 3 | 3 | 3.00 |
| 7. operational_discipline | 3 | 3 | 3 | 3.00 |
| **OVERALL** | **2.71** | **2.43** | **2.71** | **2.62** |

`OVERALL 2.62 << 4.0 PASS = BLOCK`

## Pattern recognition: 4 consecutive BLOCKs on this RCA

| Round | Approach axis | Layer | Avg | Distinct flaw classes caught |
|-------|---------------|-------|-----|------------------------------|
| 1 | CMD `&` chain via /exec | Client-side script | 2.12 | sentinel-before-chain, suppression-indefinite, JSON-parse, race-timing, sc-start |
| 2 | Tokio mutex in rc-sentry | Server-side rust | 1.75 | Tokio-cancellation, rc-sentry SPOF, Phase-1 circular dep, OLD-sentry 404, chaos tests, mutex-poisoning |
| 3 | Manual bridge via SSH + OTA_DEPLOYING | Operator-level | 2.28 | watchdog-skip-restart (SF-05), NTFS-flush, rollback-after-auto-clear, rc-sentry SPOF (still!), Pod 1 prev integrity, INBOX-not-interlock |
| 4 | Hardened script (SSH + TTL-aware sentinels + flock + 4-axis healthcheck + 60s soak) | Script-level | 2.62 | substrate-evolution N=1 weak, CR-FL-E broken-in-practice (-dirty mismatch + canonical SHA missing), flock-too-narrow, NTFS-propagation-still-not-flushed, heartbeat-single-advance-not-proof |

**Convergent inference (Round 3 deepseek-r1 prediction validated)**: 4 architecturally-different approaches all caught by adversarial gates with **different specific flaws each time** confirms the structural diagnosis. The fundamental problem is an unprivileged userspace process trying to win a race against a service-level watchdog. Adding more checkpoints, retries, and sentinel-discipline at any userspace layer (client script, server endpoint, operator procedure, or hardened script) merely rearranges where the race manifests — it does not eliminate it.

Quoting nemotron-super V04-FL-1: *"The substrate-evolution argument claims TTL‑aware sentinels, SSH transport, and Pod 8's 4‑day stability eliminate the need for LockFileEx, but the fundamental race between a userspace deploy script and the service‑level watchdog remains; the script still must time its sentinel write/clear relative to watchdog polls, merely moving the race from rc‑sentry to deploy‑pod‑agent.sh."*

Quoting kat-coder-pro CQ7: *"A 40-60 LOC script change does NOT solve the structural 'userspace process trying to win race against service-level watchdog' pattern. It merely adds more checkpoints and retries within the same race paradigm. Round 3's LockFileEx recommendation was to move the interlock to kernel level, which is the only way to guarantee atomicity."*

Quoting glm-5 CQ7: *"Without kernel-level mutual exclusion, every userspace-coordination scheme is vulnerable to NTFS jitter, OS scheduling delays, and watchdog poll-interval variance. v0.4 reduces the probability of failure but does not eliminate the failure class."*

## Convergent P0/P1 flaws (≥2 models)

| ID | Severity | Section | Flaw | Models |
|----|----------|---------|------|--------|
| **V04-FL-CONV-E** | **P0** | 4 (CR-FL-E pre-swap SHA check) | Canonical SHA file `scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256` existence unverified AND `-dirty` suffix means running binary SHA differs from clean `c5f94e31` tip → CR-FL-E gate will reject ALL pods unless file is missing/skipped | 3/3 unanimous |
| **V04-FL-CONV-N=1** | **P0** | 2 (substrate evolution) | Pod-8 N=1 empirical anchor cannot distinguish design-works from luck; 1/8 success rate could be timing-dodge | 3/3 unanimous |
| **V04-FL-CONV-X** | **P0** | 4 (Step 6.5 sentinel-clear verify) | NTFS metadata propagation can exceed `sleep 2`; CR-FL-X chicken-and-egg deadlock can recur even with sentinel-clear-before-wait pattern | 3/3 unanimous |
| **V04-FL-CONV-FLOCK** | **P1** | 4 (CR-FL-F flock interlock) | flock only blocks same-script invocations; does NOT block parallel-james deploy-pod.sh / manual SSH / rc-sentry restart_service — interlock is theater | 3/3 unanimous |
| **V04-FL-CONV-HB** | **P1** | 4 (Step 7b heartbeat soak) | 60s soak with single-mtime-advance accepts heartbeat-thread-alive-while-main-loop-dead; needs content-monotonic-counter verification + ≥2 advances with ≥30s gap + cross-check with absence of new panic.log/rc-watchdog.log entries | 3/3 unanimous |
| **V04-FL-CONV-MAOR** | **P1** | 9 (MAOR sequencing) | V2-LBAC §14.1 places REVIEW before CLOSE; Step 4 VERIFY IS the REVIEW pre-gate. MAOR should run on the PLAN itself before VERIFY, not after EXECUTE. v0.4's sequencing is backwards. | 2/3 (kat-coder explicit CQ5, glm-5 implicit) |
| **V04-FL-CONV-F1** | **P1** | 8 (F1 SCOPE GATE deferral) | G-F1-5 DEFERRED but V2-LBAC §14.2 requires ALL 5 gates pass; kaizen-min carve-out invalid for foundational pod-state-channel boundary | 2/3 (kat-coder explicit CQ4, glm-5 implicit) |
| **V04-FL-CONV-CAPTAIN** | **P2** | 5.C (canary discipline) | 5min Captain observation gate has no automated timeout / fallback; Captain-unavailable scenario stalls deploy indefinitely | 3/3 unanimous |
| **V04-FL-CONV-DHCP** | **P2** | 10 (Pod 1 IP drift) | DHCP-drift mitigation is risk-register note not code; v0.4 §4 diff does NOT include dynamic IP resolution before SSH | 2/3 (glm-5 + nemotron explicit) |

## CQ answers (unanimous across models)

| CQ | Question | Convergent answer |
|----|----------|-------------------|
| CQ1 | substrate-evolution argument validity | **INSUFFICIENT** — TTL-aware sentinels + SSH are real improvements but don't solve userspace-vs-watchdog race; N=1 anchor weak (3/3 models) |
| CQ2 | Pod-8 empirical methodology verified? | **UNVERIFIED** — PLAN cites no commit log / deploy record for Pod 8 deploy method; if manual, empirical claim is wrong-class (3/3 models) |
| CQ3 | concurrent-pilot-interlock scope adequate? | **NO** — flock too narrow; doesn't block deploy-pod.sh, manual SSH, rc-sentry restart_service (3/3 models) |
| CQ4 | G-F1-5 deferral risk | **F1-NON-COMPLIANT** — V2-LBAC §14.2 requires ALL 5 gates; kaizen-min carve-out invalid for foundational boundary (2/3 explicit, 1/3 implicit) |
| CQ5 | MAOR Tier-1 batch sequencing | **BACKWARDS** — REVIEW should run on PLAN before VERIFY, not after EXECUTE (2/3 explicit) |
| CQ6 | heartbeat-mtime as silent-loop-death gate | **UNVERIFIED** — no evidence PR #66 heartbeat thread actually starts on rc-agent startup; commit `d6c623d7` could be dead code; needs live verification on Pod 8 (3/3 models) |
| CQ7 | script-level vs binary-level fix robustness | **REARRANGES, NOT SOLVES** — v0.4 moves the userspace race from rc-sentry to deploy-pod-agent.sh; LockFileEx kernel-level mutex is the only structurally correct answer (3/3 unanimous) |

## Recommended disposition options (V04-OPT series)

| Verb | Action | Cost | Wall | Pods 1-7 exposure |
|------|--------|------|------|-------------------|
| **`V04-OPT-A`** | Amend v0.4 PLAN with all 9 convergent flaws (V04-FL-CONV-*) + re-run Step 4 VERIFY (Round 5) at ~$0.03 | ~$0.03 | ~45min author + ~3min verify | continues |
| **`V04-OPT-B`** *(STRONG — convergent across 4 rounds)* | **Commit to LockFileEx pivot direction for next session.** Full Step 1-2-3-4 cycle (multi-session Wave-class change). Accept Pods 1-7 silent-loop-death exposure indefinitely OR resolve via reboot-and-stay-on-OLD until LockFileEx ships. | ~$0.20-0.30 (Step 1 + 2 + 4) next session | next session 2-3h | indefinite until LockFileEx ships |
| **`V04-OPT-C`** | Single-pilot Pod 2 ONLY with v0.4 script + Captain real-time observation; accept 2.62/5 risk; observe 24h; do NOT ramp; if pilot survives N=2 evidence | $0 | ~30min + 24h | 1 pod (Pod 2) under observation |
| **`V04-OPT-D`** | HYBRID: address top-3 convergent P0s (V04-FL-CONV-E + CONV-N=1 + CONV-X) only; defer P1/P2; re-run Step 4 VERIFY (Round 5) at ~$0.03 | ~$0.03 | ~30min + 3min | continues |
| **`V04-OPT-E`** | DEFER fleet rollout indefinitely; Pod 8 stable + Pod-Control implementation wave will land atomic-deploy as part of pod-state-channel V2 rewrite per `project_v2_pod_control_doctrine_deployment_plan_20260510.md` Wave 5b | $0 | next planning cycle | indefinite |

**RECOMMEND `V04-OPT-B`**: 4 consecutive BLOCKs at 2.12 → 1.75 → 2.28 → 2.62 with each round catching architecturally-different flaws is the strongest possible signal that the LockFileEx pivot was the correct reading of the RCA from Round 3 deepseek-r1's reasoning chain. Round 4's substrate-evolution argument is empirically convincing for closing some flaws (TTL-aware sentinels, SSH-not-/exec) but the convergent verdict across 4 fresh-vendor adversarial panels is unanimous: this is a structural problem requiring kernel-level mutual exclusion, not script-level hardening.

**Secondary recommendation `V04-OPT-E`** is the kaizen-min disposition if Pod-Control Wave 5b is on the near-term roadmap — defers the immediate fleet-rollout urgency in favor of the architecturally-correct V2 rewrite. Pod 8 has been stable on PR #66 for 4 days; Pods 1-7 are healthy on `c5f94e31-dirty` (PRE-PR-66 but no active silent-loop-death symptoms today). No incident pressure forces rollout.

## Composes-with

- §S-146 V1↔V2 RCA doctrine (foundational pod-state-channel boundary) — 4 rounds of MMA application complete; V2-aligned remediation gated on Captain disposition
- §S-150 PR #66 silent-loop-death merged `d6c623d7` (Pod 8 canary stable 4d)
- §S-159 pre-MMA-duplicate-check hook — 4 successful gates with explicit reasons
- §S-166 model-role-fit code enforcement — 4/4 vendor families per round, no role-fit violations
- §S-172 Mechanism-Trust 5-Q — deploy-pod FAIL 5/5 stands; fix RCA remediation BLOCKED 4× pending Captain pivot direction
- §S-186 pre-§S-146 small-fix fast-lane — DOES NOT APPLY
- §S-220 MAOR v0.1 — v0.4 sequencing flagged (V04-FL-CONV-MAOR P1); future PLANs must run MAOR-on-PLAN before VERIFY
- §S-221 F1 SCOPE GATE — v0.4 flagged F1-non-compliant (V04-FL-CONV-F1 P1); G-F1-5 must complete before any EXECUTE-class action on this surface
- §S-221 F3 ACCOUNTING REFORM — this RCA is ENGINEERING-IN-FLIGHT until ANY of {LockFileEx ships / Pod-Control Wave 5b lands / v0.4-amended Round 5 PASSes}; do NOT count toward V2.0 closure
- V2-LBAC v0.1 §3 closed-loop cascade — currently in REVIEW step (Step 4.5); FIX phase (Step 4) blocked pending Captain disposition pick
- `feedback_grep_all_behavior_paths_before_planning_20260509.md` — applied via v0.4 §3 CR-FL-H mitigation BUT 3/3 models still found V04-FL-CONV-X (NTFS propagation timing) suggesting the rule is necessary-but-insufficient; consider extending to "grep all OS-primitive timing assumptions"
- per-PR Captain auth rule — applies to ALL pivot dispositions (V04-OPT-A through E)
