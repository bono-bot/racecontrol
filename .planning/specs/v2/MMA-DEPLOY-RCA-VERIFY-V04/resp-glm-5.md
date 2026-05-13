=== SCORES ===
1. substrate_evolution_validity: 3/5 — TTL-aware sentinels and SSH transport are real substrate improvements, but N=1 empirical anchor is weak evidence and the "4 NOT APPLICABLE" summary claim is actually 7 items with counting discrepancy.
2. cr_fl_x_closed: 3/5 — Sentinel-clear-before-wait correctly resolves the chicken-and-egg for normal flow, but crash-during-Step-7-wait leaves pod in crash-loop with no automatic rollback.
3. cr_fl_a_race_window: 3/5 — Confirmed-kill-loop covers normal watchdog poll timing, but Pod-1 degraded-class rapid crash-respawn during Step 6.5-7 window creates unanalyzed edge case.
4. cr_fl_e_prev_integrity: 2/5 — Canonical SHA file existence unconfirmed; `-dirty` suffix means running binary SHA differs from clean `c5f94e31` tip; recovery path unspecified.
5. cr_fl_f_flock_semantic: 2/5 — flock only blocks deploy-pod-agent.sh invocations, not deploy-pod.sh, manual SSH, or rc-sentry restart_service() — interlock is too narrow.
6. cr_fl_g_healthcheck: 3/5 — 4-axis check is comprehensive but heartbeat-only misses tracing-buffer-full silent degradation; 60s soak vs 5min Captain gap leaves ramp-exposed window.
7. operational_discipline: 3/5 — Canary discipline is sound but Captain-unavailability timeout missing; Pod 1 IP-drift mitigation is risk-register note not code.

OVERALL: 2.71/5 — BLOCK

---

## FLAWS ENUMERATED

### V04-FL-1
- **id:** V04-FL-1
- **severity:** P2
- **section:** 3 (mitigation map)
- **what's wrong:** Summary claims "4 NOT-APPLICABLE" but table shows 7 items (PV-FL-1,2,3,4,6 + CR-FL-C,D); counting discrepancy indicates incomplete review.
- **why it matters:** Documentation error undermines confidence in flaw analysis completeness.
- **proposed amendment:** Correct count to 7 NOT-APPLICABLE; verify each claim against actual substrate.

### V04-FL-2
- **id:** V04-FL-2
- **severity:** P1
- **section:** 2 (substrate evolution)
- **what's wrong:** PLAN claims Pod 8 deployed via deploy-pod-agent.sh but cites no commit/log evidence; if manual deploy, empirical anchor is wrong-class.
- **why it matters:** Entire substrate-evolution argument rests on Pod 8 being script-deployed; wrong methodology invalidates N=1 proof.
- **proposed amendment:** Add Step 0e: `git log --oneline scripts/deploy/deploy-pod-agent.sh` showing commit used for Pod 8 deploy; OR acknowledge uncertainty and weaken empirical claim.

### V04-FL-3
- **id:** V04-FL-3
- **severity:** P0
- **section:** 4 (CR-FL-E pre-swap check)
- **what's wrong:** PLAN references `scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256` but doesn't confirm file exists in substrate.
- **why it matters:** If file missing, CR-FL-E mitigation fails at runtime on first pod; entire deploy aborts.
- **proposed amendment:** Add preflight Step 0f: `test -f scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256 || { echo "ABORT: canonical SHA file missing"; exit 18; }`; if missing, generate via `git show c5f94e31:target/release/rc-agent.exe 2>/dev/null | sha256sum > scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256`.

### V04-FL-4
- **id:** V04-FL-4
- **severity:** P0
- **section:** 4 (CR-FL-E pre-swap check)
- **what's wrong:** Pods 1-7 run `c5f94e31-dirty` (uncommitted changes) but canonical reference is clean `c5f94e31` tip; SHA will mismatch even if pods are healthy.
- **why it matters:** Pre-swap SHA check would abort all 7 deploys on first pod; false-positive on degraded-prev-binary class.
- **proposed amendment:** Either (a) extract empirical SHA from a healthy pod via `certutil -hashfile` and use that as expected, OR (b) add `--skip-prev-sha-check` flag with explicit risk acceptance for `-dirty` class, OR (c) acknowledge that `-dirty` means unknown modifications and document manual SHA extraction step.

### V04-FL-5
- **id:** V04-FL-5
- **severity:** P1
- **section:** 4 (CR-FL-F concurrent interlock)
- **what's wrong:** `flock /tmp/deploy-pod-agent.lock` only blocks other deploy-pod-agent.sh invocations; does NOT block deploy-pod.sh, manual SSH commands, or rc-sentry `restart_service()` path.
- **why it matters:** Parallel-james session or Captain manual intervention could race with script; interlock is theater.
- **proposed amendment:** Add Step 0g: check `OTA_DEPLOYING` sentinel presence before any action; if present, abort with "deploy already in progress"; document that ALL deploy paths must check sentinel; OR add rc-sentry-level mutex via registry/file that all paths respect.

### V04-FL-6
- **id:** V04-FL-6
- **severity:** P1
- **section:** 5.A (timing analysis)
- **what's wrong:** If new rc-agent crashes during Step 7 wait, watchdog respawns (sentinel cleared), script exits 17, pod left in crash-loop with no automatic rollback