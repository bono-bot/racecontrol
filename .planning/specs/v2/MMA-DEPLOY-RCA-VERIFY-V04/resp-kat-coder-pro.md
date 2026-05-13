=== SCORES ===
1. substrate_evolution_validity: 2 — N=1 empirical anchor is weak; SSH transport eliminates rc-sentry SPOF but does not solve the fundamental userspace-race-against-watchdog pattern
2. cr_fl_x_closed: 3 — sentinel-clear-post-swap closes the chicken-and-egg logically, but fs-buffer + clock-skew edge cases remain unaddressed
3. cr_fl_a_race_window: 2 — confirmed-kill-loop covers watchdog poll interval but ignores degraded-prev-binary crash-during-Step7 scenario
4. cr_fl_e_prev_integrity: 2 — canonical-OLD SHA file existence unverified; "-dirty" suffix mismatch not handled; Pod1 degraded class recovery undefined
5. cr_fl_f_flock_semantic: 2 — flock blocks same-script invocations only; does not block parallel SSH, manual Captain commands, or rc-sentry restart_service
6. cr_fl_g_healthcheck: 3 — 4-axis + 60s soak is thorough but heartbeat-mtime threshold undefined; silent-loop-death partial state may evade detection
7. operational_discipline: 3 — canary-mode discipline is sound but Captain-unavailable scenario not handled; DHCP-drift mitigation is manual-note not code

OVERALL: 2.43/5 — BLOCK

---

## FLAWS

### V04-FL-1 (P0)
- **section:** 2, 5.C
- **what's wrong:** N=1 empirical anchor (Pod 8) is used as proof-of-design, but N=1 cannot distinguish luck from design. 7/8 pods broke on deploy-pod.sh; 1 succeeded on deploy-pod-agent.sh. The prior probability that Pod 8 succeeded by chance (e.g., timing alignment, no concurrent activity) is non-trivial.
- **why it matters:** If Pod 8 succeeded by luck, v0.4's entire substrate-evolution argument collapses and the 7-pod-break will repeat.
- **proposed amendment:** Require N≥3 empirical validation before fleet ramp: deploy to Pods 2,5,8 (3 distinct network segments) via canary-mode, observe 24h stability each, before ramping remaining 5.

### V04-FL-2 (P0)
- **section:** 4, Step 3.5
- **what's wrong:** `fsutil file flush` is called on the sentinel file, but `fsutil` requires Administrator privileges on Windows. The script runs via SSH as a standard user (rc-agent install directory is typically `C:\ProgramData\...` or `C:\RacingPoint\` with admin-only write). The flush will fail silently (stderr redirected to /dev/null in the script).
- **why it matters:** Sentinel durability is not actually verified; NTFS buffer may not be flushed, leading to CR-FL-B re-emergence (stale sentinel read by watchdog).
- **proposed amendment:** Remove `2>/dev/null` from fsutil call; capture exit code and abort if flush fails. Alternatively, use PowerShell `Flush-FileBuffers` with -ErrorAction Stop.

### V04-FL-3 (P0)
- **section:** 4, CR-FL-E pre-swap check
- **what's wrong:** The canonical-OLD SHA file (`scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256`) is referenced but its existence is not verified in the PLAN. If the file is missing, the script will fail with `cat: ...: No such file or directory` and `EXPECTED_PREV_SHA` will be empty, causing all pods to abort with exit 12.
- **why it matters:** Deploy halts on first pod due to missing reference file — operational blocker.
- **proposed amendment:** Add Step 0c: `test -f scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256 || { echo "ABORT: canonical OLD SHA file missing"; exit 20; }`

### V04-FL-4 (P0)
- **section:** 4, CR-FL-E pre-swap check
- **what's wrong:** Pods 1-7 are at `c5f94e31-dirty` (uncommitted changes), but the canonical reference is the clean `c5f94e31` tip. The SHA of the dirty binary will NOT match the clean SHA, causing all pods to abort with exit 12.
- **why it matters:** The "-dirty" suffix indicates local modifications; the canonical SHA gate will reject every pod, making the deploy impossible without manual override.
- **proposed amendment:** Either (a) generate canonical SHA from the actual dirty binary that was deployed (query Pod 8's running binary SHA as the reference), or (b) add `--force` flag to bypass this check for degraded-prev-binary class with Captain approval.

### V04-FL-5 (P0)
- **section:** 4, Step 7b 60s soak
- **what's wrong:** The heartbeat-mtime advance check uses `dir /T:W` to get the last-write time. If the heartbeat thread writes every 30s, a 60s soak should see exactly 2 advances. But the script only checks `INITIAL_HB != FINAL_HB` — a single advance (30s) would pass, but what if the heartbeat thread is stuck writing the same timestamp repeatedly (e.g., file handle leak, write cached)? The check would pass falsely.
- **why it matters:** Silent-loop-death could manifest as heartbeat file being written but with stale content; mtime would advance but the process is dead inside.
- **proposed amendment:** Read the actual content of the heartbeat file (e.g., a monotonic counter or timestamp inside the file) and verify it advances, not just the mtime.

### V04-FL-6 (P1)
- **section:** 4, CR-FL-F flock interlock
- **what's wrong:** `flock /tmp/deploy-pod-agent.lock` only prevents concurrent invocations of deploy-pod-agent.sh from the same machine. It does NOT prevent: (a) a parallel-james session using a different script (deploy-pod.sh), (b) Captain running manual SSH commands, (c) rc-sentry's `restart_service()` path triggering during the deploy window.
- **why it matters:** Concurrent interference from any of these paths could corrupt the deploy state (e.g., rc-sentry restarts rc-agent mid-swap).
- **proposed amendment:** Add a cross-script semaphore: write a well-known file `C:\RacingPoint\.deploy-in-progress` before starting, check for it in rc-sentry's restart path (requires rc-sentry code change — but that's a Wave-class change). Alternatively, document the interlock limitation in the risk register and require INBOX broadcast before deploy start.

### V04-FL-7 (P1)
- **section:** 4, Step 7 4-axis healthcheck
- **what's wrong:** Axis 2 (`tasklist /FI "IMAGENAME eq rc-agent.exe"`) confirms a process named rc-agent.exe exists, but does NOT verify it's the NEW binary (could be the OLD binary respawned by watchdog before sentinel clear). Axis 1 checks build_id via fleet-health, but fleet-health may have caching delay.
- **why it matters:** Script could declare success while the pod is still running the old binary.
- **proposed amendment:** Add Axis 2b: `ssh "${POD_HOST}" "certutil -hashfile ${INSTALL_DIR}\\rc-agent.exe SHA256" | grep -v "SHA256\|CertUtil" | tr -d '[:space:]'` and compare to `$LOCAL_SHA` (the new binary's SHA).

### V04-FL-8 (P1)
- **section:** 5.A, timing analysis
- **what's wrong:** The analysis assumes watchdog poll interval is exactly 10s. But `service.rs:56` defines `POLL_INTERVAL = Duration::from_secs(10)` — this is the target, but the actual poll may be delayed by system load, GC pauses, or other watchdog duties. Worst-case could be 15-20s.
- **why it matters:** If watchdog poll is delayed beyond the script's confirmed-kill-loop (15s max), the sentinel could be cleared before watchdog sees it, creating a window where watchdog sees "no rc-agent + no sentinel" and respawns OLD binary.
- **proposed amendment:** Increase confirmed-kill-loop timeout to 30s (10 retries × 3s) to cover worst-case watchdog delay. Add a post-swap `sleep 5` before sentinel clear to ensure watchdog has seen the sentinel-active state.

### V04-FL-9 (P1)
- **section:** 4, Step 6.5 sentinel-cleared verify
- **what's wrong:** The script does `sleep 2` then checks if sentinel is cleared. But NTFS file deletion is not always immediately visible across network shares or under heavy I/O. The 2s delay may be insufficient.
- **why it matters:** Script could proceed to Step 7 assuming sentinel is cleared, but watchdog still sees it → watchdog skips restart → deploy hangs.
- **proposed amendment:** Replace fixed `sleep 2` with a polling loop: check sentinel existence every 0.5s for up to 10s; abort if still present after 10s.

### V04-FL-10 (P2)
- **section:** 5.C, canary discipline
- **what's wrong:** The canary-mode requires Captain observation for 5min before ramp. If Captain is unavailable (e.g., offline, in meeting), the deploy stalls indefinitely. There's no automated fallback or timeout.
- **why it matters:** Operational deadlock — deploy cannot proceed without human intervention.
- **proposed amendment:** Add `--auto-ramp` flag that skips Captain observation and proceeds after 5min automated soak (with full 4-axis checks every 60s). Require explicit Captain approval to use `--auto-ramp`.

### V04-FL-11 (P2)
- **section:** 10, risk register
- **what's wrong:** The "Pod 1 IP drift" risk is marked HIGH but the mitigation is a manual note ("pre-pod-loop dig pod${N} via Network-Map-resolve script"). There's no code implementation in the script diff.
- **why it matters:** Manual mitigation is error-prone; if the operator forgets, Pod 1 deploy fails.
- **proposed amendment:** Add code to the script: before per-pod loop, resolve each pod's IP via `nslookup pod${N}.local` or query the fleet-health API for `last_seen` IP; update `POD_IPS` array dynamically.

### V04-FL-12 (P2)
- **section:** 8, F1 SCOPE GATE
- **what's wrong:** G-F1-5 is DEFERRED, but per V2-LBAC §14.2, ALL 5 gates must pass for F1 status. The PLAN claims "ENGINEERING-IN-FLIGHT" but is missing a critical substrate verification.
- **why it matters:** The deploy is proceeding without full substrate verification — violates V2-LBAC doctrine.
- **proposed amendment:** Complete the §S-146 RCA before Step 3 EXECUTE, or reclassify the deploy as "TEST-SCAFFOLDED" (not ENGINEERING-IN-FLIGHT) until G-F1-5 passes.

### V04-FL-13 (P3)
- **section:** 4, Step 7 4-axis healthcheck
- **what's wrong:** The healthcheck queries fleet-health API twice per iteration (build_id and ws_connected) with separate curl calls. This doubles the load on the server and introduces a race where build_id and ws_connected could be from different poll cycles.
- **why it matters:** Minor inefficiency and potential inconsistency in healthcheck data.
- **proposed amendment:** Cache the entire fleet-health JSON once per iteration and parse both fields from the same snapshot.

---

## CHALLENGE QUESTIONS

### CQ1 (substrate divergence)
v0.4's N=1 empirical anchor is statistically weak. The prior probability that Pod 8 succeeded by chance is non-negligible: if the race condition has a 1/8 chance of not triggering (given 7/8 pods failed), then Pod 8 succeeding by luck is entirely plausible. The "different script" explanation is possible but unproven. **Verdict:** substrate-evolution argument is not sufficiently supported by evidence.

### CQ2 (Pod-8-empirical-anchor methodology)
The PLAN claims Pod 8 was deployed via `deploy-pod-agent.sh` but does not cite a commit log or deployment record. If the deploy was manual (operator-driven SSH sequence), the empirical claim is wrong-class — it proves manual procedure works, not that the script works. **Verdict:** unverified claim; requires commit/log evidence.

### CQ3 (concurrent-pilot-interlock scope)
flock is too narrow. It does not block: (a) parallel-james using a different script, (b) Captain manual SSH, (c) rc-sentry restart_service. The interlock should be a cross-component semaphore (e.g., a well-known file or registry key checked by all actors). **Verdict:** interlock is insufficient for production safety.

### CQ4 (G-F1-5 deferral risk)
Per V2-LBAC §14.2, F1 SCOPE GATE requires ALL 5 gates to pass. Deferring G-F1-5 means the deploy is NOT F1-compliant. The kaizen-min carve-out is not valid for a foundational pod-state-channel boundary change. **Verdict:** v0.4 is F1-non-compliant; must complete §S-146 RCA before EXECUTE.

### CQ5 (MAOR Tier-1 batch deferral)
V2-LBAC §14.1 places REVIEW between FIX and CLOSE. Step 4 VERIFY is the REVIEW pre-gate. MAOR should run on the PLAN itself before VERIFY, not after EXECUTE. The current sequencing is backwards. **Verdict:** MAOR-on-PLAN should happen before this VERIFY round.

### CQ6 (heartbeat-mtime as silent-loop-death gate)
PR #66's commit `d6c623d7` adds heartbeat thread code, but there's no evidence the thread is actually spawned on rc-agent startup. The commit message says "40 lines added BEFORE tracing init at line 693" — this could be dead code. Has anyone confirmed the heartbeat file exists on Pod 8? **Verdict:** unverified assumption; requires live verification on Pod 8.

### CQ7 (script-level vs binary-level fix robustness)
A 40-60 LOC script change does NOT solve the structural "userspace process trying to win race against service-level watchdog" pattern. It merely adds more checkpoints and retries within the same race paradigm. Round 3's LockFileEx recommendation was to move the interlock to kernel level, which is the only way to guarantee atomicity. **Verdict:** v0.4 is rearranging userspace races, not solving them.

---

**VERDICT: BLOCK** — v0.4 fails to address the fundamental architectural flaw identified across 4 consecutive adversarial rounds. The substrate-evolution argument is weak (N=1, unverified empirical claim), critical gates (CR-FL-E, CR-FL-F, G-F1-5) are incomplete or incorrect, and the script-level approach continues to rearrange userspace races rather than solving them at the kernel level. Recommendation: commit to LockFileEx pivot direction for next session despite multi-session cost.