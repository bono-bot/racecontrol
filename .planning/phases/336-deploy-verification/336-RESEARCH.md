# Phase 336: Deploy Verification & E2E Automation - Research

**Researched:** 2026-04-07
**Domain:** Deploy pipeline verification, E2E billing/game lifecycle testing, fleet parity
**Confidence:** HIGH

## Summary

The Racing Point racecontrol project has a growing set of deploy and verification scripts, but critical gaps remain. Today's incident (4 pods with broken blanking after deploy, `edge_process_count=0`) proves the current verification is insufficient. The existing `tests/e2e/deploy/verify.sh` checks WS connectivity, build_id consistency, and installed games -- but does NOT check blanking state, Edge process count, Session context, or visual correctness. The existing `scripts/pod-verify.sh` DOES check Session context and Edge count, but it is not integrated into the deploy pipeline and is not run automatically after deploys.

The billing E2E test (`tests/e2e/api/billing.sh`) creates a trial session and validates the billing gate, but does NOT test the full lifecycle: wallet topup, game launch, AC process verification, session stop, and refund calculation. A game launch E2E exists (`tests/e2e/api/launch.sh`) but runs independently of billing -- it does not verify the billing-to-launch-to-stop-to-refund flow end-to-end.

**Primary recommendation:** Merge `pod-verify.sh` behavioral checks INTO `deploy/verify.sh` as mandatory post-deploy gates, add a new `tests/e2e/api/full-lifecycle.sh` that exercises the complete billing+launch+stop+refund flow on Pod 8 canary, and create a `deploy-all-verify.sh` master script that ensures parity across ALL targets (Server, 8 pods, POS, Cloud).

## Project Constraints (from CLAUDE.md)

Critical directives that constrain implementation:

- **Pod 8 is the canary** -- test on Pod 8 first, then fleet
- **rc-agent MUST run in Session 1** -- any deploy verification must check Console session, not Services
- **Behavioral verification for blanking** -- `edge_process_count > 0` at `:18924/debug` is the ONLY reliable check
- **DEPLOY PARITY** -- every local deploy MUST also deploy to cloud (Bono VPS)
- **ALL target enumeration from MEMORY.md** -- Server (.23), Pods 1-8, POS (.20), Cloud (Bono VPS)
- **Never conclude "powered off" from single failed probe** -- use multi-probe approach
- **Visual verification for display-affecting deploys** -- screenshots required
- **Financial flow E2E: trace actual currency values** through complete flows
- **Verify the EXACT behavior path, not proxies** -- health endpoints and build IDs are necessary but NOT sufficient
- **Static CRT builds** -- `.cargo/config.toml` `+crt-static`
- **Git Bash JSON: write payloads to file, then `curl -d @file`** -- bash escaping mangles backslashes
- **NEVER run pod binaries on James's PC**

## Standard Stack

### Core (already in project)
| Tool | Purpose | Why Standard |
|------|---------|--------------|
| bash + curl + python3 | E2E test scripting | All tests use this pattern, `lib/common.sh` provides shared infrastructure |
| `tests/e2e/lib/common.sh` | PASS/FAIL/SKIP counters, summary_exit | Already used by smoke.sh, deploy/verify.sh, api/billing.sh |
| `tests/e2e/lib/pod-map.sh` | Pod IP lookup from pod_id | Already used by deploy/verify.sh, api/launch.sh |
| `scripts/visual-verify.js` | Screenshot capture + pixel sampling | Node.js, uses `:18924/screenshot` debug endpoint |
| `scripts/pod-verify.sh` | Session context + Edge count + blanking | Behavioral verification (not proxy) |

### Supporting
| Tool | Purpose | When to Use |
|------|---------|-------------|
| `scripts/check-alive.sh` | Multi-probe connectivity (ping + HTTP + SSH) | Pre-deploy target reachability |
| `deploy-staging/deploy-server.sh` | Server deploy with rollback | Server binary swap |
| `scripts/deploy-pod-agent.sh` | Pod agent deploy with SHA256 verification | Pod binary swap |

## Architecture Patterns

### Current Verification Landscape

```
scripts/
  check-alive.sh          # Multi-probe: ping + HTTP + SSH (per-target)
  pod-verify.sh           # Behavioral: Session context + edge count + blanking (all 8 pods)
  visual-verify.js        # Screenshot capture + pixel analysis (all 8 pods)
  deploy-pod-agent.sh     # Single-pod deploy with build_id verify

tests/e2e/
  smoke.sh                # API endpoint reachability (status codes only)
  deploy/verify.sh        # Post-deploy: server health, sentry, binary size, WS, build_id, games
  api/billing.sh          # Billing gate rejection + trial session create
  api/launch.sh           # Per-game launch + state lifecycle
  api/session-lifecycle.sh # Session create + end_reason schema
  lib/common.sh           # Shared PASS/FAIL/SKIP infrastructure
  lib/pod-map.sh          # Pod IP mapping
```

### Gap Analysis: What Exists vs. What's Missing

| Check | Exists Where | Automated After Deploy? | Notes |
|-------|-------------|------------------------|-------|
| Server health (:8080) | deploy/verify.sh Gate 0,4 | YES | |
| rc-sentry reachable (:8091) | deploy/verify.sh Gate 1 | YES (canary only) | |
| Binary size non-zero | deploy/verify.sh Gate 2 | YES (canary only) | |
| Kiosk :3300 serving | deploy/verify.sh Gate 3 | YES | |
| WS connected (8 pods) | deploy/verify.sh Gate 5 | YES | |
| build_id consistent | deploy/verify.sh Gate 6 | YES | |
| installed_games non-empty | deploy/verify.sh Gate 7 | YES (canary only) | |
| **Session context (Console vs Services)** | pod-verify.sh | **NO** -- not in deploy pipeline | **CRITICAL GAP** |
| **edge_process_count > 0** | pod-verify.sh | **NO** -- not in deploy pipeline | **CRITICAL GAP -- today's incident** |
| **lock_screen_state=screen_blanked** | pod-verify.sh | **NO** -- not in deploy pipeline | |
| **Visual screenshot non-black** | visual-verify.js | **NO** -- manual `--visual` flag | |
| **Billing+launch+stop+refund flow** | Partial (billing.sh + launch.sh separate) | **NO** -- no integrated flow | |
| **POS reachable + build_id** | NOWHERE | **NO** | POS always forgotten |
| **Cloud build_id matches venue** | NOWHERE | **NO** | Parity not checked |
| **Server build_id matches expected** | deploy-server.sh step 7 | Only during server deploy | Not in fleet verify |

### Recommended Architecture: Post-Deploy Verification Pipeline

```
tests/e2e/
  deploy/
    verify.sh              # ENHANCED: merge pod-verify.sh behavioral checks
    verify-parity.sh       # NEW: cross-target parity (server, pods, POS, cloud)
    verify-blanking.sh     # NEW: dedicated blanking+visual check (all 8 pods)
  api/
    full-lifecycle.sh      # NEW: billing create -> launch -> verify AC -> stop -> refund
  run-post-deploy.sh       # NEW: master orchestrator (runs all verify scripts)
```

### Pattern: Post-Deploy Gate Script

Every deploy script (`deploy-pod-agent.sh`, `deploy-server.sh`) should call the verification pipeline at the end:

```bash
# After deploy completes:
echo "Running post-deploy verification..."
bash tests/e2e/deploy/verify.sh
DEPLOY_EXIT=$?
if [ $DEPLOY_EXIT -ne 0 ]; then
    echo "POST-DEPLOY VERIFICATION FAILED — do NOT mark deployed"
    exit 1
fi
```

### Pattern: Blanking Behavioral Check (from pod-verify.sh)

This is the exact check that catches today's incident:

```bash
# Source: scripts/pod-verify.sh lines 71-87
debug=$(curl -s --connect-timeout 3 "http://$ip:18924/debug" 2>/dev/null)
edge=$(echo "$debug" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('edge_process_count',0))" 2>/dev/null)
lock=$(echo "$debug" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('lock_screen_state','unknown'))" 2>/dev/null)

if [ "$edge" -gt 0 ] 2>/dev/null; then
    pass "Pod $num: edge_process_count=$edge, lock=$lock"
else
    fail "Pod $num: edge_process_count=$edge (blanking broken)"
fi
```

### Pattern: Full Billing Lifecycle Test

The E2E flow that does NOT exist today but is needed:

```bash
# 1. Auth: get staff JWT
TOKEN=$(curl -s -X POST "${BASE_URL}/auth/admin-login" \
    -H "Content-Type: application/json" \
    -d '{"pin":"..."}' | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))")

# 2. Create test driver (or use existing test driver)
# POST /drivers with test data

# 3. Wallet topup
curl -s -X POST "${BASE_URL}/wallet/${DRIVER_ID}/topup" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d '{"amount_paise": 100000, "method": "cash", "staff_id": "test"}'

# 4. Start billing session
SESSION_ID=$(curl -s -X POST "${BASE_URL}/billing/start" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"pod_id\":\"pod-8\",\"driver_id\":\"${DRIVER_ID}\",\"pricing_tier_id\":\"tier_30min\"}" \
    | python3 -c "import sys,json; print(json.load(sys.stdin).get('billing_session_id',''))")

# 5. Launch game (AC)
curl -s -X POST "${BASE_URL}/games/launch" \
    -H "Authorization: Bearer ${TOKEN}" \
    -H "Content-Type: application/json" \
    -d "{\"pod_id\":\"pod-8\",\"sim_type\":\"assetto_corsa\"}"

# 6. Poll for game state = Running/Launching (max 60s)
# 7. Verify acs.exe running on pod via debug endpoint or SSH

# 8. Stop billing (early end)
curl -s -X POST "${BASE_URL}/billing/${SESSION_ID}/stop" \
    -H "Authorization: Bearer ${TOKEN}"

# 9. Verify refund: wallet balance should reflect pro-rated refund
# 10. Cleanup: verify pod returns to blanking state
```

### API Endpoints Required for E2E

| Endpoint | Method | Auth | Purpose |
|----------|--------|------|---------|
| `/auth/admin-login` | POST | Public | Get staff JWT token |
| `/drivers` | POST | Staff JWT | Create test driver |
| `/wallet/{driver_id}/topup` | POST | Staff JWT | Add funds |
| `/wallet/{driver_id}` | GET | Staff JWT | Check balance |
| `/billing/start` | POST | Staff JWT | Start billing session |
| `/billing/active` | GET | Staff JWT | Check active sessions |
| `/games/launch` | POST | Staff JWT | Launch game on pod |
| `/games/active` | GET | Staff JWT | Check running games |
| `/billing/{id}/stop` | POST | Staff JWT | End session early |
| `/billing/sessions/{id}` | GET | Staff JWT | Get session details with refund |

### Debug Endpoint (Pod-Level, No Auth Required from .23/.27)

| Endpoint | Port | Returns |
|----------|------|---------|
| `GET /status` (or `/debug`) | 18924 | `lock_screen_state`, `edge_process_count`, `pod_number`, `last_launch_error` |
| `GET /screenshot` | 18924 | PNG image of pod screen |

**IP-restricted:** Only localhost, .23 (server), .27 (James) can access.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Pod IP mapping | Hardcoded maps in each script | `tests/e2e/lib/pod-map.sh` | Already exists, DRY |
| PASS/FAIL reporting | Custom echo patterns | `tests/e2e/lib/common.sh` | Already exists, consistent output |
| Screenshot capture | Custom Win32 code | `:18924/screenshot` debug endpoint | Already deployed on all pods |
| Pod connectivity probing | Single ping | `scripts/check-alive.sh` | Multi-probe prevents false negatives |
| Visual analysis | Manual screenshot review | `scripts/visual-verify.js` | Pixel sampling + size analysis automated |

## Common Pitfalls

### Pitfall 1: Proxy Verification (TODAY'S INCIDENT)
**What goes wrong:** Deploy checks build_id + WS connected, declares success. Blanking is broken on 4 pods.
**Why it happens:** `build_id` and `ws_connected` are proxy metrics. They prove the binary is running, NOT that it works correctly.
**How to avoid:** ALWAYS check behavioral state: `edge_process_count > 0` at `:18924/debug` for every pod after deploy.
**Warning signs:** `lock_screen_state: "screen_blanked"` with `edge_process_count: 0` is the impossible state that signals broken blanking.

### Pitfall 2: Session 0 Silently Breaks Everything
**What goes wrong:** rc-agent starts in Session 0 (Services context). All GUI operations fail silently -- Edge, games, overlays, blanking.
**Why it happens:** schtasks or Windows Service restarts rc-agent as SYSTEM in Session 0.
**How to avoid:** After every deploy, verify `tasklist /V /FO CSV | findstr rc-agent` shows `Console` session, not `Services`.
**Warning signs:** Health endpoint returns 200, build_id matches, but `edge_process_count = 0`.

### Pitfall 3: POS and Cloud Always Forgotten
**What goes wrong:** Deploy verification runs on server + 8 pods. POS (.20) and Cloud (Bono VPS) are never checked.
**Why it happens:** Fleet health endpoint only shows connected pods. POS and Cloud are separate infrastructure.
**How to avoid:** Explicit target enumeration from MEMORY.md in every verification script: Server, Pods 1-8, POS, Cloud.
**Warning signs:** POS showing stale build_id. Cloud `racingpoint.cloud` returning old UI.

### Pitfall 4: Billing E2E Creates Orphan Sessions
**What goes wrong:** E2E test creates a billing session on Pod 8, test fails midway, session left active. Next customer on Pod 8 can't start.
**Why it happens:** No cleanup trap in test script.
**How to avoid:** `trap cleanup EXIT` that always calls `billing/{id}/stop` (already in `session-lifecycle.sh` -- copy pattern).
**Warning signs:** `billing/active` shows a stale test session on Pod 8.

### Pitfall 5: Git Bash JSON Escaping
**What goes wrong:** JSON payloads with nested quotes or backslashes get mangled by bash.
**Why it happens:** Git Bash on Windows strips/doubles backslashes differently than Linux bash.
**How to avoid:** Write JSON to a temp file using the Write tool, then `curl -d @file`. Already a standing rule.

### Pitfall 6: SSH Connection Limits During Fleet Verify
**What goes wrong:** Parallel SSH to all 8 pods + server + POS hits SSH MaxSessions or connection limits.
**Why it happens:** Default SSH MaxStartups is 10 on Windows OpenSSH. Sequential checks take 3-5 min.
**How to avoid:** Serial checks with 1s sleep between pods, OR use HTTP-based checks (`:18924/debug`, `:8090/health`) instead of SSH where possible.

## Code Examples

### Existing: pod-verify.sh Behavioral Check (VERIFIED -- scripts/pod-verify.sh)

```bash
# Per-pod: Session context check via SSH
session=$(ssh -o ConnectTimeout=3 $ssh_target "tasklist /V /FO CSV /NH | findstr rc-agent" 2>/dev/null)
if echo "$session" | grep -q "Services"; then
    fail "Pod $num: rc-agent in SESSION 0 (Services) -- GUI broken"
elif echo "$session" | grep -q "Console"; then
    pass "Pod $num: rc-agent in Console Session 1"
fi

# Per-pod: Debug endpoint check (blanking state)
debug=$(curl -s --connect-timeout 3 "http://$ip:18924/debug" 2>/dev/null)
edge=$(echo "$debug" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('edge_process_count',0))" 2>/dev/null)
```

### Existing: Deploy Verify Fleet WS Check (VERIFIED -- tests/e2e/deploy/verify.sh)

```bash
# Uses fleet/health API, parses with python3
FLEET_RESP=$(curl -s --max-time 10 "${BASE_URL}/fleet/health" 2>/dev/null)
# Python extracts connected/disconnected pod lists
```

### Existing: Billing Session Create (VERIFIED -- tests/e2e/api/billing.sh)

```bash
BILL_RESP=$(curl -s --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d "{\"pod_id\": \"${POD_ID}\", \"driver_id\": \"driver_test_trial\", \"pricing_tier_id\": \"tier_trial\"}" \
    "${BASE_URL}/billing/start" 2>/dev/null)
```

### Debug Server Status Response (VERIFIED -- crates/rc-agent/src/debug_server.rs)

```json
{
  "pod": "Pod-8",
  "pod_number": 8,
  "lock_screen_state": "screen_blanked",
  "edge_process_count": 2,
  "debug_server": "ok",
  "last_launch_error": null
}
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | bash + curl + python3 (E2E scripts) |
| Config file | `tests/e2e/lib/common.sh` (shared infrastructure) |
| Quick run command | `bash tests/e2e/smoke.sh` |
| Full suite command | `bash tests/e2e/run-all.sh` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DVER-01 | Post-deploy blanking check on all 8 pods | integration | `bash tests/e2e/deploy/verify.sh` (enhanced) | Partial -- needs blanking gates |
| DVER-02 | Session context verification (Console not Services) | integration | `bash tests/e2e/deploy/verify.sh` (enhanced) | Partial -- exists in pod-verify.sh only |
| DVER-03 | Cross-target parity (server, pods, POS, cloud) | integration | `bash tests/e2e/deploy/verify-parity.sh` | Wave 0 |
| DVER-04 | Full billing lifecycle (topup->start->launch->stop->refund) | e2e | `bash tests/e2e/api/full-lifecycle.sh` | Wave 0 |
| DVER-05 | Visual verification (screenshot non-black) | e2e | `bash scripts/pod-verify.sh --visual` | Exists |
| DVER-06 | Deploy pipeline integration (auto-run after deploy) | integration | Deploy scripts call verify | Needs wiring |

### Wave 0 Gaps
- [ ] `tests/e2e/deploy/verify-parity.sh` -- cross-target parity verification (server, 8 pods, POS, cloud)
- [ ] `tests/e2e/api/full-lifecycle.sh` -- complete billing+launch+stop+refund E2E on Pod 8
- [ ] Enhanced `tests/e2e/deploy/verify.sh` -- add blanking + session context gates from pod-verify.sh

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| bash (Git Bash) | All scripts | Yes | Git Bash on Windows | -- |
| curl | HTTP checks | Yes | via Git Bash | -- |
| python3 | JSON parsing | Yes | Available | -- |
| ssh | Pod Session check | Yes | OpenSSH | HTTP debug endpoint for most checks |
| Node.js | visual-verify.js | Yes | v22.22.0 | -- |
| Debug endpoint (:18924) | Blanking + screenshot | Yes (all pods) | Deployed in rc-agent | -- |
| Fleet health API | WS + build_id check | Yes | Server :8080 | -- |

**Missing dependencies with no fallback:** None.

## Open Questions

1. **Admin PIN for E2E tests**
   - What we know: `admin_login` requires a PIN. Billing start/stop/topup require staff JWT.
   - What's unclear: Should the E2E test use a real admin PIN or should we create a dedicated test auth bypass?
   - Recommendation: Use an env var `E2E_ADMIN_PIN` set at test time. Do NOT hardcode the PIN in test scripts. The existing billing.sh skips auth by using public billing/start endpoint.

2. **Test driver for billing E2E**
   - What we know: `billing.sh` uses `driver_test_trial` with `tier_trial`. Wallet topup needs a driver with a wallet.
   - What's unclear: Does `driver_test_trial` have a wallet? Can we topup a trial driver?
   - Recommendation: Create a dedicated `driver_e2e_test` with wallet at test start, or verify the trial tier does not require wallet funds.

3. **POS SSH access**
   - What we know: POS PC SSH is down per MEMORY.md ("SSH down, D3 audit 2026-04-06"). rc-agent :8090 is running.
   - What's unclear: Can we verify POS state via HTTP health endpoint alone?
   - Recommendation: Use `curl http://192.168.31.20:8090/health` (or Tailscale IP) for POS. Session context check via SSH is blocked until SSH is restored.

4. **Cloud verification method**
   - What we know: Cloud racecontrol on Bono VPS. Bono relay available via comms-link.
   - What's unclear: Best way to check cloud build_id -- direct HTTP to `racingpoint.cloud:8080` or Bono relay exec?
   - Recommendation: Direct `curl https://racingpoint.cloud/api/v1/health` for build_id comparison.

## Sources

### Primary (HIGH confidence)
- `scripts/deploy-pod-agent.sh` -- current deploy flow, SHA256 verification, build_id check
- `scripts/pod-verify.sh` -- behavioral verification: Session context, edge_process_count, blanking
- `scripts/check-alive.sh` -- multi-probe connectivity pattern
- `tests/e2e/deploy/verify.sh` -- current post-deploy gates (7 gates)
- `tests/e2e/api/billing.sh` -- billing lifecycle test pattern
- `tests/e2e/api/launch.sh` -- game launch test pattern with state polling
- `tests/e2e/api/session-lifecycle.sh` -- session lifecycle with cleanup trap
- `crates/rc-agent/src/debug_server.rs` -- debug endpoint: lock_screen_state, edge_process_count, screenshot
- `crates/racecontrol/src/api/routes.rs` -- all API routes for billing, games, wallets, auth

### Secondary (MEDIUM confidence)
- CLAUDE.md standing rules -- deploy parity, behavioral verification, Session 1 requirement
- MEMORY.md -- target enumeration, POS status, Cloud status

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all tools already exist in the project, just need wiring
- Architecture: HIGH -- gap analysis based on reading actual source code of all existing scripts
- Pitfalls: HIGH -- drawn directly from today's incident and CLAUDE.md standing rules that document past failures

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable -- infrastructure, not fast-moving libraries)
