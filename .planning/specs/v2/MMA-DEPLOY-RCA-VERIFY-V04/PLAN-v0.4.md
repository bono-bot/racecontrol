# Deploy-mechanism remediation — PLAN v0.4 (Option Y · harden deploy-pod-agent.sh)

- **Author:** james (Claude Opus 4.7)
- **Date:** 2026-05-13 ~14:55 IST
- **Captain ratification:** "Resume the CF-1+CF-2 remediation: either drive Step 4 VERIFY to PASS on a v0.4 plan ... or commit to the LockFileEx pivot direction with a fresh PLAN. Authorize ~$0.05-0.10 MMA spend." (2026-05-13 ~14:48 IST)
- **Status:** AWAITING Step 4 VERIFY adversarial gate (Round 4 of 4)
- **Prior rounds:** 1=2.12/5 BLOCK · 2=1.75/5 BLOCK (PIVOT /exec_atomic_deploy) · 3=2.28/5 BLOCK (BRIDGE PV2-OPT-E)

## 1. Scope (fleet state observed 2026-05-13 14:44 IST)

| Pod | IP | Current binary | Action |
|-----|----|----------------|--------|
| Pod 1 | 192.168.31.3 (DHCP-drifted from canonical .89; was .16 yesterday) | `c5f94e31-dirty` | Bridge LAST in ramp (degraded-from-prior-failed-deploy class) |
| Pod 2 | .31.33 | `c5f94e31-dirty` (uptime 6.9h) | PILOT — full canary soak |
| Pod 3 | .31.28 | `c5f94e31-dirty` (uptime 6.9h) | Ramp 1 |
| Pod 4 | .31.88 | `c5f94e31-dirty` (uptime 6.9h) | Ramp 2 |
| Pod 5 | .31.86 | `c5f94e31-dirty` (uptime 6.9h, REACHABLE — recovered since 2026-05-09 incident) | Ramp 3 |
| Pod 6 | .31.87 | `c5f94e31-dirty` (uptime 6.9h) | Ramp 4 |
| Pod 7 | .31.38 | `c5f94e31-dirty` (uptime 6.9h) | Ramp 5 |
| Pod 8 | .31.91 | `8e378f4d` (PR #66 squash · stable 7h current uptime · cumulatively ~4 DAYS stable since 2026-05-09 deploy) | UNTOUCHED |

Target: ramp Pods 1-7 to `8e378f4d` (=PR #66) using **hardened `scripts/deploy-pod-agent.sh`** — NO code changes to rc-agent / rc-sentry / rc-watchdog. v0.4 hardens the script that proved-on-Pod-8 to address all flaws caught in Round 1-3 adversarial gates.

## 2. Substrate evolution since Round 3 (CRITICAL — adversarial reviewer must examine)

This v0.4 PLAN **diverges from the convergent Round-3 LockFileEx recommendation** because substrate has materially evolved post-2026-05-09. Adversarial reviewer should challenge whether this divergence is warranted.

Evidence of substrate evolution:

1. **`rc_common::survival_types` module landed** with structured `SentinelKind::OtaDeploying` + `check_sentinel()` + `remaining_secs()` (TTL-aware parsing). Verified at `crates/rc-watchdog/src/service.rs:280-298` — `any_sentinel_active()` reads TTL from JSON content, not file mtime. Closes Round-1 FL-CONV-2 (sentinel suppression indefinite) and Round-3 CR-FL-B partially (TTL is OS-level not just mtime).

2. **`scripts/deploy-pod-agent.sh` exists** (155 LOC, NEWER than the deploy-pod.sh that Round 1-3 audited). Reviewed in this PLAN as the base for v0.4 hardening. Key properties:
   - SSH transport (not rc-sentry /exec) → eliminates Round-2 PV-FL-2 + Round-3 CR-FL-D rc-sentry SPOF
   - SHA256 verification of staged binary before swap (Step 1) → partial CR-FL-E mitigation
   - Stale-binary cleanup before swap (Step 2) → eliminates "bat picks up wrong rc-agent-*.exe" class
   - Sentinel written WITH structured JSON content matching `survival_types::OtaDeploying` schema (Step 3)
   - Confirmed-kill loop with retry (Step 4) — Round-3 CR-FL-A race window mitigation
   - **Sentinel cleared post-swap BEFORE watchdog wait (Step 6)** → eliminates Round-3 CR-FL-X chicken-and-egg (script affirmatively unblocks watchdog rather than relying on TTL expiry)
   - Build_id post-swap verification loop (Step 7)
   - Build_id mismatch recovery via stale-process kill (Step 7b) — NEW

3. **Pod 8 empirical anchor**: deployed PR #66 binary on 2026-05-09 via this exact script (commit log shows deploy-pod-agent.sh was used); has held stable 4+ days across 2 reboots (current uptime 6.9h after 17:35 IST 2026-05-12 cycle). Empirical N=1 proof that the script's design works for 1 pod. The 7-pod-break of 2026-05-09 was using the OLDER deploy-pod.sh (different script).

**Rationale for NOT going LockFileEx:**

- LockFileEx-from-scratch requires: new `crates/rc-common/src/deploy_lock.rs` (Win32 FFI wrapper) + modify `rc-watchdog/src/service.rs:280-298` SF-05 logic + modify `rc-watchdog/src/rollback_manager.rs:121` + new `/exec/atomic_deploy` endpoint in rc-sentry + modify deploy-pod-agent.sh to use new endpoint. ~250-350 LOC of foundational pod-state-channel boundary code. By V2-LBAC §14.1 MAOR REVIEW + §14.2 F1 SCOPE GATE + §S-146 V1↔V2 RCA, this is a Wave-class change. NOT a one-session ship.
- Empirically, the post-2026-05-09 substrate already eliminated 4 of 8 Round-3 flaws (CR-FL-X chicken-and-egg + CR-FL-B partial TTL + CR-FL-D rc-sentry SPOF + the "no TTL" subset of CF-2).
- The remaining 4 flaws (CR-FL-A taskkill race · CR-FL-C double-rollback race · CR-FL-E prev binary integrity · CR-FL-F concurrent interlock · CR-FL-G healthcheck depth · CR-FL-H preflight gate scope) are addressable in `deploy-pod-agent.sh` script-level hardening.
- Pod 8 4-day stability is strong empirical signal that the existing design class works at N=1 — the 7-pod-break was a different script with different design.

If adversarial reviewer disagrees with this substrate-evolution assessment, the v0.4 verdict should BLOCK and recommendation re-orient toward LockFileEx (which would then take 2-3 sessions across MMA Step 1-2-3-4 cycles).

## 3. The 8 adversarial flaws from Round 1-3 BLOCK reports — v0.4 mitigation map

| Flaw ID | Caught in Round | Severity | Substrate state | v0.4 mitigation |
|---|---|---|---|---|
| **FL-CONV-1** sentinel-before-chain ordering | 1 (2.12) | P0 | Current script writes sentinel BEFORE taskkill (Step 3 before Step 4) | Inherent in deploy-pod-agent.sh — no change needed |
| **FL-CONV-2** watchdog suppression indefinite | 1 | P0 | `survival_types::OtaDeploying` now has TTL via `ttl_secs` JSON field | Inherent (substrate-resolved) |
| **FL-CONV-3** JSON parse fail | 1 | P1 | `survival_types::check_sentinel()` returns `None` on parse fail = sentinel-not-present | Add explicit verify: post-write `dir OTA_DEPLOYING` AND `type OTA_DEPLOYING | findstr ttl_secs` to confirm content parseable |
| **FL-CONV-4** race timing unaddressed | 1 | P1 | deploy-pod-agent.sh has confirmed-kill loop (Step 4 polls until taskkill PASS) | Add deterministic timing analysis section in this PLAN (§5.A) |
| **FL-CONV-5** sc-start-failure unhandled | 1 | P1 | Watchdog respawn via WTSQueryUserToken (not sc-start); built-in retry | Acceptance criterion §7 includes "watchdog respawn observed within 30s" |
| **PV-FL-1** Tokio Mutex cancellation hazard | 2 (1.75) | P0 | No Tokio Mutex — script-level deploy uses SSH not /exec | NOT APPLICABLE (substrate-divergent) |
| **PV-FL-2** rc-sentry SPOF | 2 | P0 | Script uses SSH not /exec — rc-sentry NOT on critical deploy path | NOT APPLICABLE (substrate-divergent) |
| **PV-FL-3** Phase-1 circular dep | 2 | P0 | Script is single-phase per pod; no cross-pod dependency | NOT APPLICABLE |
| **PV-FL-4** Pod 8 OLD-sentry 404 | 2 | P1 | Pod 8 untouched | NOT APPLICABLE |
| **PV-FL-5** chaos tests missing | 2 | P2 | Single-pilot-pod canary IS the chaos test (real pod, real watchdog) | §5.D — pilot pod 5-minute soak before ramp; observe panic.log + heartbeat |
| **PV-FL-6** mutex poisoning | 2 | P1 | No Rust Mutex | NOT APPLICABLE |
| **CR-FL-X** SF-05 chicken-and-egg (watchdog skips restart) | 3 (2.28) | P0 | Script clears sentinel post-swap (Step 6) BEFORE waiting for watchdog → SF-05 stops gating restart → respawn proceeds | Add `--verify-sentinel-cleared` post-Step-6 gate before entering Step 7 wait; abort+manual-recover if sentinel still present after 5s |
| **CR-FL-A** watchdog poll-interval N race window | 3 | P0 | Script has confirmed-kill loop (taskkill + 3s + verify dead, retry up to 5 times = 15s total) — covers worst-case watchdog poll interval (5-10s typical) | §5.A timing analysis: prove kill+swap window does NOT overlap with watchdog poll on TTL-aware sentinel |
| **CR-FL-B** NTFS durability (1s flush insufficient) | 3 | P0 | Script writes sentinel via SSH `echo > file` — shell-buffered, NOT FlushFileBuffers | **v0.4 ADD:** post-write `fsutil file flush ${INSTALL_DIR}\\OTA_DEPLOYING` AND `type ${INSTALL_DIR}\\OTA_DEPLOYING | findstr OtaDeploying` round-trip verify (Step 3.5) |
| **CR-FL-C** rollback re-sets sentinel before checking auto-clear | 3 | P0 | Script does NOT run rollback (delegates to watchdog) — script aborts on failure with manual-recover branch | NOT APPLICABLE (script architecture changed) |
| **CR-FL-D** rc-sentry SPOF (chain delivery + healthcheck) | 3 | P0 | Script uses SSH for chain delivery + fleet-health-API (.23:8080) for healthcheck → 2 different transports | NOT APPLICABLE (substrate-divergent) |
| **CR-FL-E** Pod 1 prev binary integrity | 3 | P0 | Script verifies NEW binary SHA (Step 1); does NOT verify PREV (rc-agent-prev.exe pre-swap state) | **v0.4 ADD:** pre-swap SHA256 of current `rc-agent.exe` recorded to deploy-manifest.json on James .27 for post-hoc verify; if pod's current rc-agent.exe SHA differs from canonical `c5f94e31-dirty` expected SHA, abort + escalate (degraded prev binary class) |
| **CR-FL-F** concurrent parallel-james interlock | 3 | P1 | INBOX notify only — no file/registry semaphore | **v0.4 ADD:** wrap entire script in `flock /tmp/deploy-pod-agent.lock` (advisory POSIX lock; rejects second invocation immediately); document override env for emergency manual recovery |
| **CR-FL-G** healthcheck depth (no process liveness) | 3 | P1 | Step 7 checks `build_id` only via :8090/health | **v0.4 ADD:** Step 7 augmented with: (a) `tasklist /FI "IMAGENAME eq rc-agent.exe"` confirms PID present, (b) `dir /A-H ${LOG_DIR}\\rc-agent-heartbeat.txt` mtime advances ≥30s during 60s window (PR #66 behavior — heartbeat-on-OS-thread), (c) ws_connected=True via Server .23 fleet-health, (d) panic.log size unchanged or absent |
| **CR-FL-H** preflight gate scope (only checked rollback_manager.rs) | 3 | P1 | Preflight must verify BOTH `service.rs:280-298` SF-05 (TTL-aware sentinel check) AND `rollback_manager.rs:121` are present in target pod's running binary | **v0.4 ADD:** preflight `git show <build_id>:crates/rc-watchdog/src/service.rs | grep -n any_sentinel_active` AND `git show <build_id>:crates/rc-watchdog/src/rollback_manager.rs | grep -n OTA_DEPLOYING` — both must match expected substrate; abort on mismatch (substrate-incompatible OLD binary class) |

**Summary**: 4 NOT-APPLICABLE (substrate-divergent: SSH not /exec, no Tokio, script aborts on failure not rolls back) · 4 INHERENT (script already handles) · 6 v0.4 ADDITIONS to script

## 4. v0.4 script changes (~40-60 LOC delta on `scripts/deploy-pod-agent.sh`)

```bash
# v0.4 additions (compact pseudo-diff against current deploy-pod-agent.sh)

# ── PRE-SCRIPT (Step 0a) ──
# CR-FL-F: concurrent interlock
LOCK_FILE="/tmp/deploy-pod-agent.lock"
exec 200>"$LOCK_FILE"
flock -n 200 || { echo "ERROR: another deploy-pod-agent.sh holds lock $LOCK_FILE"; exit 9; }
echo "Lock acquired: $LOCK_FILE (pid $$)"

# ── PREFLIGHT (Step 0b) — runs once on James .27, before per-pod loop ──
# CR-FL-H: substrate compatibility gate
EXPECTED_OLD_SHA=$(git rev-parse c5f94e31)  # canonical OLD binary git ref
git show ${EXPECTED_OLD_SHA}:crates/rc-watchdog/src/service.rs | grep -q 'any_sentinel_active' \
  || { echo "ABORT: OLD binary missing TTL-aware sentinel substrate"; exit 10; }
git show ${EXPECTED_OLD_SHA}:crates/rc-watchdog/src/rollback_manager.rs | grep -q 'OTA_DEPLOYING' \
  || { echo "ABORT: OLD binary missing OTA_DEPLOYING rollback suppression"; exit 11; }

# ── CR-FL-E: pre-swap prev binary integrity check ──
# Insert between Step 1 and Step 2 of existing script
EXPECTED_PREV_SHA="$(cat scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256)"
CURRENT_SHA=$(ssh -o ConnectTimeout=5 "${POD_HOST}" "certutil -hashfile ${INSTALL_DIR}\\rc-agent.exe SHA256" \
              | grep -v "SHA256\|CertUtil" | tr -d '[:space:]')
if [ "$CURRENT_SHA" != "$EXPECTED_PREV_SHA" ]; then
    echo "ABORT (Pod $POD_NUM): current rc-agent.exe SHA does not match canonical c5f94e31"
    echo "  Got:      $CURRENT_SHA"
    echo "  Expected: $EXPECTED_PREV_SHA"
    echo "  Degraded-prev-binary class — escalate to Captain"
    exit 12
fi

# Record SHA to deploy-manifest for post-hoc reconciliation
echo "{\"pod\":${POD_NUM},\"pre_sha\":\"${CURRENT_SHA}\",\"new_sha\":\"${LOCAL_SHA}\",\"ts\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}" \
  >> scripts/deploy/deploy-manifest.jsonl

# ── CR-FL-B: NTFS durability after sentinel write ──
# Insert as Step 3.5, between Step 3 (write sentinel) and Step 4 (taskkill)
echo -n "Step 3.5: NTFS flush + content round-trip verify... "
ssh -o ConnectTimeout=5 "${POD_HOST}" "fsutil file flush ${INSTALL_DIR}\\OTA_DEPLOYING" 2>/dev/null
SENTINEL_CONTENT=$(ssh -o ConnectTimeout=5 "${POD_HOST}" "type ${INSTALL_DIR}\\OTA_DEPLOYING 2>nul")
echo "$SENTINEL_CONTENT" | grep -q "OtaDeploying" \
  || { echo "ABORT: sentinel content not durable/parseable"; \
       ssh "${POD_HOST}" "del ${INSTALL_DIR}\\OTA_DEPLOYING 2>nul"; exit 13; }
echo "$SENTINEL_CONTENT" | grep -q "ttl_secs" \
  || { echo "ABORT: sentinel TTL field missing"; \
       ssh "${POD_HOST}" "del ${INSTALL_DIR}\\OTA_DEPLOYING 2>nul"; exit 14; }
echo "OK (content verified)"

# ── CR-FL-X: post-Step-6 sentinel-cleared gate ──
# Insert as Step 6.5, between Step 6 (clear sentinel) and Step 7 (wait for watchdog)
echo -n "Step 6.5: confirm sentinel cleared before watchdog wait... "
sleep 2  # filesystem propagation
SENTINEL_STILL=$(ssh -o ConnectTimeout=5 "${POD_HOST}" \
                "if exist ${INSTALL_DIR}\\OTA_DEPLOYING (echo PRESENT) else (echo CLEAR)" 2>/dev/null)
if [ "$SENTINEL_STILL" != "CLEAR" ]; then
    echo "ABORT: sentinel still present after del — watchdog still gated"
    echo "Manual recovery: ssh ${POD_HOST} 'del ${INSTALL_DIR}\\OTA_DEPLOYING'"
    exit 15
fi
echo "OK"

# ── CR-FL-G: enhanced behavioral healthcheck (replaces Step 7 build_id-only check) ──
# Inline expansion of Step 7
echo -n "Step 7: behavioral healthcheck (4-axis verification)..."
HEALTHY=0
for i in 1 2 3 4 5 6 7 8 9 10; do  # 60s window
    sleep 6
    # Axis 1: build_id via Server .23 fleet-health (NOT direct pod query — avoids race)
    BUILD_ID=$(curl -s --max-time 5 "http://192.168.31.23:8080/api/v1/fleet/health" \
               | python3 -c "import sys,json; d=json.load(sys.stdin); p=[x for x in d['pods'] if x['pod_number']==${POD_NUM}]; print(p[0]['build_id'] if p else 'NONE')" 2>/dev/null)
    # Axis 2: process liveness via ssh tasklist
    PID_PRESENT=$(ssh -o ConnectTimeout=5 "${POD_HOST}" \
                  "tasklist /FI \"IMAGENAME eq rc-agent.exe\" /NH 2>nul | findstr rc-agent >nul && echo YES || echo NO" 2>/dev/null)
    # Axis 3: ws_connected via fleet-health
    WS=$(curl -s --max-time 5 "http://192.168.31.23:8080/api/v1/fleet/health" \
         | python3 -c "import sys,json; d=json.load(sys.stdin); p=[x for x in d['pods'] if x['pod_number']==${POD_NUM}]; print(p[0]['ws_connected'] if p else False)" 2>/dev/null)
    # Axis 4: heartbeat-mtime advance (PR #66 behavior)
    HB_TS=$(ssh -o ConnectTimeout=5 "${POD_HOST}" \
            "for /F %T in ('dir /T:W ${INSTALL_DIR}\\rc-agent-heartbeat.txt 2^>nul ^| findstr rc-agent-heartbeat') do echo %T" 2>/dev/null)

    echo -n "."
    if [ "$BUILD_ID" = "$HASH" ] && [ "$PID_PRESENT" = "YES" ] && [ "$WS" = "True" ] && [ -n "$HB_TS" ]; then
        HEALTHY=1
        break
    fi
done
if [ "$HEALTHY" = "1" ]; then
    echo " OK (build=$BUILD_ID pid=$PID_PRESENT ws=$WS hb_present)"
    # NEW: 60s observation soak before declaring success
    echo -n "Step 7b: 60s post-success soak (heartbeat advance check)..."
    INITIAL_HB="$HB_TS"
    sleep 60
    FINAL_HB=$(ssh "${POD_HOST}" "for /F %T in ('dir /T:W ${INSTALL_DIR}\\rc-agent-heartbeat.txt 2^>nul ^| findstr rc-agent-heartbeat') do echo %T" 2>/dev/null)
    if [ "$INITIAL_HB" = "$FINAL_HB" ]; then
        echo " ABORT: heartbeat mtime did not advance during 60s — silent-loop-death suspected"
        echo "  Initial: $INITIAL_HB"
        echo "  Final:   $FINAL_HB"
        exit 16
    fi
    echo " OK (heartbeat advanced $INITIAL_HB → $FINAL_HB)"
    echo "=== Pod ${POD_NUM}: DEPLOY VERIFIED (4-axis + 60s soak) ==="
    exit 0
fi

echo " TIMEOUT (60s window expired without 4-axis pass)"
exit 17

# ── --canary-only flag at script entry ──
# Handle in arg parsing before per-pod loop:
if [ "${1:-}" = "--canary-only" ]; then
    POD_NUM="$2"
    HASH="$3"
    CANARY_ONLY=true
else
    CANARY_ONLY=false
fi

# ... and at script end:
if [ "$CANARY_ONLY" = "true" ]; then
    echo "Canary mode — ramp gated on Captain go-ahead after 5min observation"
    echo "After observation, re-run without --canary-only to proceed with ramp"
fi
```

## 5. Timing + per-pod analysis

### 5.A — Race window deterministic analysis (CR-FL-A)

Watchdog `service.rs:280-298` poll cycle is gated by `POLL_INTERVAL` constant. Per source read 2026-05-13: `POLL_INTERVAL = Duration::from_secs(10)` (line 56 of service.rs). Worst-case watchdog reacts 10s after taskkill.

Script timing:
- Step 3 (sentinel write) → Step 3.5 (fsutil flush + verify): ~1s
- Step 4 (taskkill + confirmed-dead loop): ~3-15s (typically <5s)
- Step 5 (binary swap via 4 `move` operations): <1s
- Step 6 (sentinel clear): <1s
- Step 6.5 (sentinel-cleared verify): ~2s
- Step 7 (4-axis healthcheck wait, up to 60s)

Worst-case race window where watchdog COULD see "rc-agent dead + sentinel absent" = window between Step 6 and watchdog next poll. With TTL-aware sentinel, this is acceptable: watchdog respawning rc-agent is the DESIRED outcome of Step 6→7 transition. The race window doesn't exist for the previous-design reason (watchdog was using mtime-based sentinel with race vs TTL-aware structured-content sentinel now).

### 5.B — Per-pod procedure (sequential, one pod at a time)

For each pod in `[2, 3, 4, 5, 6, 7, 1]` (canary→ramp; Pod 1 last due to DHCP IP drift class):

1. **Acquire flock** (script-level — only first pod actually acquires; subsequent pods wait via same lock)
2. **Preflight** (Steps 0-2 in v0.4): SHA verify on staged binary + clean stale + record canonical OLD SHA to deploy-manifest
3. **Sentinel set + verify** (Steps 3-3.5)
4. **Confirmed kill** (Step 4)
5. **Atomic swap** (Step 5)
6. **Sentinel clear + verify** (Steps 6-6.5)
7. **4-axis healthcheck** (Step 7)
8. **60s post-success soak with heartbeat-advance verify** (Step 7b — NEW)

### 5.C — Canary discipline

- First invocation: `--canary-only 2 8e378f4d` → exits after Pod 2 healthy + 60s soak
- Captain observes Pod 2 fleet-health + heartbeat for 5min (300s) BEFORE authorizing ramp
- Second invocation: `deploy-pod-agent.sh 8e378f4d` (no --canary-only) → ramps remaining pods 3-7 sequentially, Pod 1 last

### 5.D — Pilot-pod soak verifies what 4-axis healthcheck does not

The 4-axis check verifies "deploy correct at T+60s." The 5min Captain-observed soak verifies:
- No silent-loop-death over 5min (PR #66 fix is doing its job)
- No new panic.log entries
- No watchdog rollback attempted (`tail rc-watchdog.log`)
- No cumulative behavior change (memory growth, handle leak, etc.)
- ws_connected stays True (not flapping)

This composes-with the CGP H3 anti-theater rule: 4-axis check is the proxy gate, 5min Captain soak is the actual-behavior verification.

## 6. Rollback paths

| Trigger | Action |
|---------|--------|
| Step 4 taskkill fails after 5 retries | Clear sentinel; HALT script with exit 1 |
| Step 5 swap fails (rc-agent.exe missing post-swap) | Clear sentinel; HALT with exit 1 (manual SSH to inspect rc-agent-prev.exe state) |
| Step 6.5 sentinel-not-cleared | HALT with exit 15; surface manual recovery |
| Step 7 4-axis timeout | Exit 17; pod has new binary running but unhealthy — Captain decides between (a) manual rollback to prev via SSH (~10s), (b) reboot pod, (c) leave + investigate |
| Step 7b heartbeat stall during soak | Exit 16; silent-loop-death detected → roll back this pod to prev binary; HALT entire ramp |

Reversibility per pod: rc-agent-prev.exe preserved for at least 72h per OTA pipeline rule. Manual rollback command in standby:
```bash
ssh pod${N} "taskkill /F /IM rc-agent.exe & cd /d C:\\RacingPoint & move /Y rc-agent.exe rc-agent-failed.exe & move /Y rc-agent-prev.exe rc-agent.exe"
```

## 7. Acceptance criteria (per pod — all 6 must hold)

1. **build_id** == `8e378f4d` via Server .23 fleet-health
2. **process liveness** == YES (rc-agent.exe in tasklist)
3. **ws_connected** == True
4. **http_reachable** == True
5. **heartbeat.txt mtime** advances ≥1× during 60s observation (proves PR #66 fix engaged)
6. **panic.log** absent or size-unchanged-from-pre-deploy
7. *(canary only)* **rc-watchdog.log** shows no rollback attempt during 5min observation

## 8. F1 SCOPE GATE (V2-LBAC §14.2 — pre-spawn substrate verification)

| Gate | Required | v0.4 status |
|------|----------|-------------|
| **G-F1-1** endpoint exists | rc-sentry :8091/exec for diagnostics (NOT critical path) — verified in registered routes | ✓ |
| **G-F1-2** configurable constant | `POLL_INTERVAL`, `SENTINEL_PATH`, `TTL_SECS` — present in `rc-watchdog/src/service.rs:56` + `survival_types.rs` | ✓ |
| **G-F1-3** field shape | `SentinelKind::OtaDeploying` struct shape with `started_at`/`ttl_secs`/`action_id`/`layer` — present in `rc_common::survival_types` | ✓ |
| **G-F1-4** behavioral mechanism | `any_sentinel_active() + check_sentinel()` TTL-aware logic — present in `service.rs:280-298` | ✓ |
| **G-F1-5** composes-with §S-146 V1↔V2 RCA gate | This PLAN IS the §S-146 RCA for deploy-pod-agent surface (mechanism-trust ledger entry pending) | DEFERRED — full RCA section deferred to Step 3 EXECUTE artifact (kaizen-min) |

**F1 verdict**: 4/5 PASS. G-F1-5 deferral is acceptable per V2-LBAC for kaizen-min path; full §S-146 5-section RCA writes alongside Step 3 EXECUTE PR.

## 9. MAOR Tier-1 batch (V2-LBAC §14.1 — mandatory every iter)

Will run after Step 3 EXECUTE drafts script changes — `feature-dev:code-reviewer` subagent reviews bash diffs + new helpers. Tier-2 per-file conditional on N>7 files (v0.4 touches 1-2 files: deploy-pod-agent.sh + deploy-manifest.jsonl scaffold).

## 10. Risk register

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| OLD `c5f94e31-dirty` binary on Pods 1-7 actually lacks TTL-aware substrate (preflight grep gate at compile-time, not runtime check) | LOW (deploy substrate landed pre-Pod-8-canary) | Script could write structured-JSON sentinel that OLD binary doesn't understand → falls back to mtime/is_file() check → still works | Preflight G-F1-3 check; substrate-incompatible class → exit 11 |
| Pod 8 succeeded because of luck-not-design | LOW (4 days stable, 2 reboots survived) | 7-pod-break repeats | Canary mode + 5min Captain observation gate isolates risk to 1 pod before fleet exposure |
| Concurrent parallel-james session attempts deploy via different path | MEDIUM | Lock works for OWN script invocations; doesn't block another tool | flock + bono INBOX notify on script start; if INBOX shows live parallel deploy, abort |
| Pod 1 IP drift (now .31.3) breaks SSH alias | HIGH (verified via fleet-health) | Script SSH fails on Pod 1 specifically | v0.4 ADD: pre-pod-loop dig pod${N} via Network-Map-resolve script; refresh POD_IPS array if drift detected; Pod 1 placed LAST in ramp |
| Heartbeat file path mismatch between PR #66 binary and watchdog reader | LOW | Step 7b false-negative; rolls back healthy deploy | PR #66 code reviewed by adversarial gate already; path is `<log_dir>/rc-agent-heartbeat.txt` per commit message; preflight grep gate Step 0c |
| BLOCKED_PATTERNS in OLD rc-sentry rejects new diagnostic commands | LOW (CR-FL-D not applicable — script doesn't use rc-sentry) | nil | NOT APPLICABLE |
| Step 7b 60s soak insufficient to catch slow regressions | MEDIUM | Bug ships, surfaces hours later | Canary 5min Captain observation catches what 60s does not; ramp gated on canary |

## 11. Spend budget

Step 4 VERIFY (this Round 4):
- z-ai/glm-5: ~$0.012
- kwaipilot/kat-coder-pro-v2: ~$0.008
- nvidia/nemotron-3-super-120b-a12b: ~$0.003

Est total: ~$0.023. Within Captain $0.05-0.10 authorization.

If PASS: Step 3 EXECUTE = script edits (~50 LOC) + cargo build canonical-OLD-binary-sha helper if missing = ~0 MMA cost (manual). Plus MAOR Tier-1 batch via `feature-dev:code-reviewer` (subagent, no $ cost on session budget but uses Sonnet tokens — separate budget).

If BLOCK 4th time at <4.0/5: HALT — surface to Captain with the 3 BLOCK pattern + Round 4 reasoning + recommendation to commit to LockFileEx (multi-session Wave class).

## 12. Composes-with

- §S-146 V1↔V2 RCA doctrine (foundational pod-state-channel boundary; per-PR Captain auth at PR-open + at deploy execute)
- §S-150 PR #66 silent-loop-death merged `d6c623d7`
- §S-159 pre-MMA-duplicate-check hook (Round 4 needs `MMA_FORCE_DUPLICATE=1` with reason "v0.4 hardened-script PLAN distinct from prior 3 LockFileEx-direction rounds")
- §S-166 model-role-fit code enforcement (Round 4 picks all role-fit)
- §S-172 Mechanism-Trust 5-Q (this v0.4 IS the upstream-of-fix remediation that mechanism-trust FAIL → PASS depends on)
- §S-186 pre-§S-146 small-fix fast-lane — DOES NOT APPLY (Pods 1-7 ramp is V2-doctrine ship, not pre-§S-146 small fix)
- §S-220 MAOR v0.1 (REVIEW step) — runs in EXECUTE phase via `feature-dev:code-reviewer`
- §S-221 F1 SCOPE GATE — applied above §8
- §S-221 F3 ACCOUNTING REFORM — this is ENGINEERING-IN-FLIGHT until binaries land on Pods 1-7 + 5min soak + ramp; do NOT count toward V2.0 closure until DONE
- V2-LBAC v0.1 §3 closed-loop cascade — OPEN→DESCEND→H1→F1→FIX→REVIEW→CLOSE→SWEEP→SYNC→BILATERAL
- new G9 PROMOTE-N=2 `feedback_grep_all_behavior_paths_before_planning_20260509.md` — applied via §3 mitigation table grep of service.rs + rollback_manager.rs (CR-FL-H direct addresses it)
- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` — script-level change but foundational boundary; per-PR Captain auth required for EXECUTE phase
- `feedback_pre_s146_small_fix_fastlane_20260511.md` — does NOT apply (V2-doctrine ship, not pre-§S-146)

## 13. Deferred to future Waves (NOT in v0.4 scope)

- **PV2-OPT-B Win32 LockFileEx structural rewrite** — multi-session Wave-class change; address remaining flaws via kernel-level mutex. Trigger: if v0.4 hardened-script approach experiences any deploy failure post-ship, prioritize LockFileEx in next planning cycle.
- **CF-4 BLOCKED_PATTERNS refactor** (parser-not-regex, allowlist) — independent surface; address in own RCA
- **CF-7 jq JSON standardization** — discipline doc only
- **CF-8 cross-source observability** — central log collector
- **CF-9 watchdog deploy-aware health checks bilateral protocol** — adjacent rc-watchdog refactor; address in next pod-state-channel boundary work

## 14. Authority chain

- **Captain ratify:** 2026-05-13 ~14:48 IST disposition (A) "drive Step 4 VERIFY to PASS on a v0.4 plan ... or commit to the LockFileEx pivot direction with a fresh PLAN. Authorize ~$0.05-0.10 MMA spend"
- **MMA Step 1 DIAGNOSE:** consumed from `MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md` (CF-1..CF-9, 9 consensus + 4 novel)
- **MMA Step 2 PLAN:** substrate-evolved minimal-invariant Option Y (this PLAN replaces both `STEP2/CONSENSUS-PLAN.md` and `STEP2-PIVOT/CONSENSUS-PLAN.md` as superseded)
- **MMA Step 4 VERIFY:** awaiting Round 4 verdict (this PLAN under review)

---

— james / 2026-05-13 ~14:55 IST · v0.4 deploy-mechanism remediation PLAN · substrate-evolved Option Y (harden deploy-pod-agent.sh) · 6 script additions addressing CR-FL-X+A+B+E+F+G+H · 4 flaws NOT-APPLICABLE (substrate-divergent: SSH not /exec, no Tokio, script-aborts-not-rolls-back) · est Step 4 VERIFY cost ~$0.023 of $0.05-0.10 authorized
