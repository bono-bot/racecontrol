# MMA Step 4 VERIFY — Round 4 adversarial gate on v0.4 deploy-mechanism PLAN

You are an adversarial reviewer in a 4-step Multi-Model Audit. Your job is to **find every flaw** in v0.4 PLAN before it touches production. Score below 4.0/5 = BLOCK.

## Context — 3 consecutive BLOCKs on this RCA

Three prior rounds have BLOCKED on rc-agent fleet deploy-mechanism plans:

| Round | Approach | Avg | Key flaws found |
|-------|----------|-----|-----------------|
| 1 | CMD `&` chain in single /exec (CONSENSUS-PLAN) | 2.12/5 | FL-CONV-1..5: sentinel-before-chain · suppression indefinite · JSON parse · race timing · sc-start |
| 2 | Tokio Mutex + /exec_atomic_deploy server-side endpoint (PIVOT) | 1.75/5 | PV-FL-1..6: Tokio cancellation · rc-sentry SPOF · Phase-1 circular dep · OLD-sentry 404 · chaos tests · mutex poisoning |
| 3 | Manual operational bridge using `OTA_DEPLOYING` sentinel (BRIDGE PV2-OPT-E) | 2.28/5 | CR-FL-X chicken-and-egg SF-05 · CR-FL-A poll-interval race · CR-FL-B NTFS durability · CR-FL-C double-rollback race · CR-FL-D rc-sentry SPOF · CR-FL-E prev binary integrity · CR-FL-F concurrent interlock · CR-FL-G healthcheck depth · CR-FL-H preflight gate scope |

Round 3 deepseek-r1 reasoning chain concluded: *"3 consecutive BLOCKs each catching architecturally-different flaws is the strongest possible signal that LockFileEx kernel-level mutual exclusion is the only sustainable answer ... an unprivileged userspace process trying to win a race against a service-level watchdog. None of the 3 approaches solves that race; they just rearrange where it manifests."*

## v0.4's structural divergence from Round-3 LockFileEx-recommendation

**v0.4 explicitly REJECTS the LockFileEx-direction** based on substrate evolution since 2026-05-09. Key claims you must adversarially examine:

1. **Substrate evolved**: `rc_common::survival_types::SentinelKind::OtaDeploying` now has TTL-aware structured JSON (`started_at`/`ttl_secs`/`action_id`/`layer`); `service.rs:280-298` SF-05 logic uses `any_sentinel_active() + check_sentinel()` with `remaining_secs()` instead of mtime-based `is_file()`. PLAN claims this closes 4 of 8 Round-3 flaws.

2. **deploy-pod-agent.sh substrate**: a NEWER script (vs the deploy-pod.sh audited Round 1-3) uses SSH transport (not rc-sentry /exec), thereby eliminating Round-2 PV-FL-2 + Round-3 CR-FL-D rc-sentry SPOF. PLAN claims this script is the 90%-correct base.

3. **Pod 8 empirical anchor**: PR #66 binary `8e378f4d` deployed via deploy-pod-agent.sh on 2026-05-09, has held stable 4+ days across 2 reboots (~7h current uptime). PLAN claims this is empirical N=1 proof the script's design works.

4. **Therefore**: LockFileEx-from-scratch (~250-350 LOC of foundational pod-state-channel boundary code) is overengineering. v0.4 ships 40-60 LOC of script-level hardening to close remaining 6 flaws.

**Adversarial question for you**: is this substrate-evolution argument valid, or is v0.4 falling into the same pattern as Round 1-3 (rearranging userspace race conditions without solving them)?

## Hard architectural facts (verified preflight 2026-05-13 14:44 IST)

1. **Live fleet state**: Server .23 racecontrol = `61999f58`; Pods 1-7 at `c5f94e31-dirty` (ws=True http=True uptime ~6.9h all healthy); Pod 8 at `8e378f4d` (~7h uptime, healthy, ws=True). Pod 9 = POS at `e7e01ae3`.
2. **Pod 1 IP DHCP-drifted**: canonical `.89` → was `.16` on 2026-05-12 → now `.31.3`. PLAN places Pod 1 LAST in ramp and adds dig-resolve in §10 risk register.
3. **OLD `c5f94e31-dirty` binary substrate**: contains `survival_types::OtaDeploying` + TTL-aware `any_sentinel_active()` (deploy-pod-agent.sh:82 writes structured JSON matching this schema, and Pod 8 deploy via this script empirically succeeded).
4. **RCWatchdog Windows Service**: spawns rc-agent.exe in Session 1 via `WTSQueryUserToken` + `CreateProcessAsUser`. POLL_INTERVAL = 10s per `service.rs:56`.
5. **PR #66 heartbeat behavior**: `<log_dir>/rc-agent-heartbeat.txt` mtime advances every 30s via dedicated OS thread (commit `d6c623d7`); silent-loop-death detected externally by mtime > 60s.
6. **Server fleet-health endpoint**: `GET http://192.168.31.23:8080/api/v1/fleet/health` returns per-pod `pods[]` array with `build_id`/`ws_connected`/`http_reachable`/`last_seen`/`uptime_secs`.

## The PLAN under review

<PLAN>
{{PLAN_CONTENT}}
</PLAN>

## Your task

For each scoring dimension below, give a 1-5 score with one-sentence justification. Then enumerate every concrete flaw you find. Then conclude with overall verdict.

Flaws must include:
- **id:** V04-FL-N
- **severity:** P0 (BLOCK), P1 (must-fix-before-canary), P2 (must-fix-before-ramp), P3 (nice-to-fix)
- **section:** which section of the PLAN (e.g., 3, 4, 5.A)
- **what's wrong:** specific failure mode
- **why it matters:** consequence in production
- **proposed amendment:** concrete change to address

## Scoring dimensions (1=fatal, 5=excellent)

1. **Substrate-evolution argument validity** — is §2's claim that TTL-aware sentinels + SSH-not-/exec + Pod-8-empirical-anchor actually warrants rejecting Round-3's LockFileEx-recommendation? Or is v0.4 missing what Round 1-3 were trying to teach? (Check the validity of "4 flaws NOT APPLICABLE" claims — are they really substrate-divergent, or is v0.4 disguising the same flaws under different transport?)

2. **CR-FL-X chicken-and-egg actually closed** — §3 claims sentinel-clear-post-swap (Step 6) eliminates the SF-05 skip-restart deadlock. Verify by walking through the timeline: T=0 sentinel set, T=1 taskkill rc-agent, T=2 watchdog poll cycle (sees no rc-agent + sentinel-active → skips), T=3 binary swap done, T=4 sentinel cleared, T=5 watchdog poll cycle (sees no rc-agent + no sentinel → respawns). What if T=5 watchdog poll arrives BEFORE T=4 (clock-skew, fs-buffer)? What if rc-agent respawns and immediately crashes during Step 7 wait — does it loop into MAINTENANCE_MODE before 4-axis check completes?

3. **CR-FL-A poll-interval race window deterministic analysis** — §5.A claims worst-case watchdog reacts 10s after taskkill, and confirmed-kill-loop (Step 4) covers 5-15s. Trace: what if Pod 1's degraded prev-binary class makes rc-agent crash within 3s of respawn during Step 7? Could that crash + watchdog respawn loop happen during Step 6.5 sentinel-cleared-verify, leaving inconsistent state?

4. **CR-FL-E prev binary integrity gate adequacy** — v0.4 hashes `rc-agent.exe` BEFORE swap and compares to canonical `c5f94e31-dirty` expected SHA from `scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256`. (A) Does that file actually exist? PLAN doesn't say. (B) What if the canonical-OLD SHA differs from what's running on Pods 1-7 because they're at `c5f94e31-dirty` (note the "-dirty" suffix = uncommitted changes) and the canonical reference is the clean `c5f94e31` tip? (C) What's the recovery if SHA mismatch on Pod 1 (degraded class)?

5. **CR-FL-F flock interlock semantic** — v0.4 wraps script in `flock /tmp/deploy-pod-agent.lock`. Does flock survive the SSH commands inside the script (they're shell subprocesses)? Does flock block a parallel SSH-direct command issued by user/parallel-james? Does the lock release if SSH connection drops mid-script?

6. **CR-FL-G 4-axis healthcheck + 60s soak adequacy** — §3 + Step 7b describe 4 axes (build_id + tasklist + ws_connected + heartbeat-mtime) + 60s post-soak. (A) What's the minimum mtime advance count to declare heartbeat-healthy (1 advance? 2?). (B) What if rc-agent's tracing buffer is full but heartbeat thread still alive — silent-loop-death partial state — does 4-axis catch it? (C) Why 60s soak and not the full 5min Captain-observation? Could 60s miss anything?

7. **Operational discipline** — is canary-mode (§5.C) actually different from how Round 3 was structured? Captain observation gate vs automated 5min advance — what changes if Captain is unavailable / takes hours to observe? Pod 1 LAST disposition — is the DHCP-IP-drift risk handled in code, or is it a § note that requires manual care?

## Specific challenge questions (must address)

- **CQ1 (substrate divergence):** v0.4 §2 claims "Pod 8 4-day stability is strong empirical signal that the existing design class works at N=1." But N=1 is by definition weak evidence — what's the prior probability that Pod 8 succeeded by chance vs by design? If 7/8 pods broke on the SAME class of deploy (deploy-pod.sh) and 1 pod succeeded on deploy-pod-agent.sh, is the "different script" the explanation, or did Pod 8 just happen to dodge the race?

- **CQ2 (Pod-8-empirical-anchor methodology):** Was Pod 8's deploy actually via `deploy-pod-agent.sh`, or via a manual operator-driven sequence? The PLAN claims script-deployed but doesn't cite commit/log evidence. If manual, the empirical claim is wrong-class.

- **CQ3 (concurrent-pilot-interlock scope):** flock prevents 2 invocations of deploy-pod-agent.sh from same machine. Does NOT prevent: (a) parallel-james session running deploy-pod.sh (different script); (b) Captain running manual SSH; (c) rc-sentry initiating restart via `restart_service()` path concurrent with script's atomic chain. Is this interlock too narrow?

- **CQ4 (G-F1-5 deferral risk):** PLAN §8 admits G-F1-5 (composes-with §S-146 V1↔V2 RCA gate) is "DEFERRED — full RCA section deferred to Step 3 EXECUTE artifact." Per V2-LBAC §14.2 F1 SCOPE GATE, the ENTIRE row should be `ENGINEERING-IN-FLIGHT` until ALL 5 gates pass, NOT classify as `TEST-SCAFFOLDED`. Is the kaizen-min carve-out valid here, or is v0.4 itself an F1-anti-pattern (claiming ENGINEERING-IN-FLIGHT status without full substrate verification)?

- **CQ5 (MAOR Tier-1 batch deferral):** PLAN §9 says MAOR runs "after Step 3 EXECUTE drafts script changes" — i.e., AFTER this VERIFY pass. But V2-LBAC §14.1 inserts REVIEW between FIX and CLOSE; this Step 4 VERIFY IS the REVIEW step's pre-gate. Is the VERIFY-first then MAOR-later sequencing correct, or should MAOR-on-PLAN-itself happen before VERIFY?

- **CQ6 (heartbeat-mtime as silent-loop-death gate):** PR #66's panic-hook + heartbeat-thread is supposed to write `<log_dir>/rc-agent-heartbeat.txt` every 30s. What if PR #66's commit `d6c623d7` only writes the heartbeat THREAD code but doesn't actually invoke it on startup (commit message says "40 lines added BEFORE tracing init at line 693" — could be code-only-not-invoked)? Has anyone confirmed the heartbeat file actually exists on Pod 8 4 days after deploy?

- **CQ7 (script-level vs binary-level fix robustness):** Does a 40-60 LOC script change really address the "userspace process trying to win race against service-level watchdog" pattern that Round 3 identified as the structural root cause? Or does this v0.4 just move the rearrangement-of-userspace-races down one layer (from rc-sentry to deploy-pod-agent.sh)?

## Output format

Begin with:
```
=== SCORES ===
1. substrate_evolution_validity: <1-5> — <one sentence>
2. cr_fl_x_closed: <1-5> — <one sentence>
3. cr_fl_a_race_window: <1-5> — <one sentence>
4. cr_fl_e_prev_integrity: <1-5> — <one sentence>
5. cr_fl_f_flock_semantic: <1-5> — <one sentence>
6. cr_fl_g_healthcheck: <1-5> — <one sentence>
7. operational_discipline: <1-5> — <one sentence>

OVERALL: <average>/5 — <PASS if ≥4.0 else BLOCK>
```

Then enumerate every flaw with the format above. Then answer each CQ1..CQ7 explicitly with concrete reasoning. Then conclude with one line: **VERDICT: PASS / BLOCK** + amendment summary if BLOCK.

Be ruthlessly adversarial. Prior 3 rounds at 2.12, 1.75, 2.28 caught real flaws — find the ones still hiding. This is Round 4; if BLOCK, the pattern strongly suggests v0.4 itself is the wrong architectural direction and LockFileEx pivot must be ratified for next session despite multi-session cost.
