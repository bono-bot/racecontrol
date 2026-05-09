# MMA Step 1 DIAGNOSE — rc-agent fleet deploy mechanism RCA — CONSENSUS

**Captain-requested**: 2026-05-09 ~19:30 IST ("Do RCA of deployment and use MMA")
**Anchor**: 11-issue session RCA at `~/.claude/projects/C--Users-bono/memory/project_session_pr66_deploy_session_rca_20260509.md`
**Models**: 5 vendor-disjoint (deepseek-r1 / qwen3-coder / mimo-v2-pro / gemini-2.5-flash / mistral-small-2603)
**Vendor families**: 5 (deepseek, qwen, xiaomi, google, mistral) ≥3 ✓
**Roles**: ≥1 reasoner (r1) + ≥1 code expert (qwen3-coder) + ≥1 SRE (mimo) ✓
**Wall time**: 86.6s max (parallel)
**Cost**: $0.0409 / $5 budget (deepseek $0.0146 + gemini $0.0103 + mimo $0.0117 + qwen $0.0024 + mistral $0.0020)
**Valid responses**: 5/5
**Hooks**: pre-mma-duplicate-check passed (different session_purpose from today's W1-S5/W3 batches)

---

## §1 — Consensus matrix

5 models, 12 distinct themes (clusters of similar findings across models).

| Theme | Title | Consensus | Severity | Novel | Net (post-triage) |
|---|---|---|---|---|---|
| **A** | Non-atomic kill+swap race vs watchdog | **5/5** ★★★ | **P0** | No | CONSENSUS-P0 |
| **B** | OTA_DEPLOYING sentinel discipline gap | **5/5** ★★★ | **P0** | No | CONSENSUS-P0 |
| **C-i** | Missing single-target dry-run before fleet | **5/5** ★★★ | **P0/P1** | No | CONSENSUS-P0 |
| **C-ii** | Missing staging-server preflight (port-bind/serving-dir) | **5/5** ★★★ | **P1** | No | CONSENSUS-P1 |
| **D** | BLOCKED_PATTERNS overly restrictive (`"\| "` blocks legit cmds) | **5/5** ★★★ | **P0/P1** | No | CONSENSUS-P1 |
| **E** | Silent failure modes (HTTP 403, EPERM-as-success, exit-0-with-0-bytes) | **5/5** ★★★ | **P1** | No | CONSENSUS-P1 |
| **F** | Orphan bg process accumulation → handle exhaustion | **5/5** ★★★ | **P1** | No | CONSENSUS-P1 |
| **G** | JSON escaping standardization needed (jq is reliable) | **4/5** ★★ | **P1** | No | CONSENSUS-P1 |
| **H** | Cross-source timing correlation for incident analysis | **5/5** ★★★ | **P1/P2** | No | CONSENSUS-P1/P2 |
| **I** | Modal dialog / GUI interaction can block pod | **3/5** ★ | **P1/P2** | Mostly novel | CONSENSUS-P2 |
| **J** | Watchdog deploy-aware health checks (extend POLL_INTERVAL on OTA_DEPLOYING) | **4/5** ★★ | **P0/P1** | **NOVEL** (3 of 4 models) | CONSENSUS-NOVEL-P1 |
| **L** | Two-phase commit / bilateral consistency (control server ↔ pods) | **2/5** | **P1** | NOVEL | MINORITY-P1 |
| **M** | Idempotent deploy operations (state-machine / declarative) | **2/5** | **P1/P2** | NOVEL | MINORITY-P1/P2 |
| **N** | Manifest trust gap (cryptographic chain build → exec) | **3/5** ★ | **P1/P2** | NOVEL | CONSENSUS-NOVEL-P2 |
| **O** | build_id verification BEFORE swap (not just SHA) | 1/5 | P2 | NOVEL | SINGLETON |
| **P** | Implicit trust in start-rcagent.bat | 1/5 | P2 | NOVEL | SINGLETON |
| **Q** | Deploy progress tracking + timeout enforcement | 2/5 | P2 | Mostly novel | MINORITY-P2 |
| **K** | HTTPS + code signing instead of plain HTTP | 1/5 | P2 | NOVEL | SINGLETON |

**Counts**: 9 CONSENSUS (5/5 or 4/5) · 1 CONSENSUS-NOVEL · 4 MINORITY (2/5) · 4 SINGLETON

---

## §2 — Top P0/P1 findings (priority order, all CONSENSUS)

### CF-1 [Theme A] — Non-atomic kill+swap race vs RCWatchdog [P0, 5/5]

**Root cause:** `deploy-pod.sh` (steps 5-10) issues kill, copy-to-prev, and move-new as 3 separate `/exec` HTTP roundtrips. RCWatchdog polling (5-10s) wins the race; OLD binary respawns before NEW swap completes.

**Structural fix (consensus across 5 models):**
- Combine kill+swap into SINGLE `/exec` chain: `taskkill /F /IM rc-agent.exe & del /Q rc-agent-prev.exe & ren rc-agent.exe rc-agent-prev.exe & ren rc-agent-new.exe rc-agent.exe`.
- Refactor `deploy-pod.sh` deploy_pod() to use this canonical pattern (already documented in CLAUDE.md "Remote deploy sequence" but not implemented in script).
- (gemini variant): create new `/exec/atomic_swap` endpoint in rc-sentry that handles the chain server-side + manages OTA_DEPLOYING internally.

**Verify:** Deploy to test pod with watchdog polling enabled; confirm swap completes < 5s and watchdog does not respawn old binary.

---

### CF-2 [Theme B] — OTA_DEPLOYING sentinel discipline gap [P0, 5/5]

**Root cause:** Watchdog's `perform_rollback()` is suppressed only by presence of `C:\RacingPoint\OTA_DEPLOYING` sentinel file. The sentinel has no TTL, no lease semantics, and is managed external to the swap operation. Inline manual deploys (Pod 1) often omit it; even canonical script clears it before health-poll completes.

**Structural fix (consensus):**
- (mimo) Sentinel with TTL: write `OTA_DEPLOYING` containing `expires=<epoch+60s>`; watchdog checks both existence AND expiration.
- (5/5) Integrate sentinel set/clear into the atomic deploy primitive (CF-1) so it can never be omitted.
- (mimo, mistral) Implement bilateral mutex: rc-agent + rc-watchdog share state via shared file or registry key.

**Verify:** Deploy with sentinel set; confirm watchdog skips rollback throughout the boot window; test sentinel auto-expiry after 60s triggers normal behavior.

---

### CF-3 [Theme C-i + C-ii] — Missing dry-run + staging-server preflight [P0, 5/5]

**Root cause:** No mandatory single-target validation before fleet rollout (Issue 5 burned 7 pods). HTTP staging server starts fire-and-forget; silent port-bind failure is undetected (Issue 1).

**Structural fix (consensus):**
- Add `--canary <pod>` flag to deploy script that runs full sequence on one pod with enhanced logging; require canary success before fleet rollout.
- Add staging-server preflight: `curl -s http://192.168.31.27:18889/manifest.json | jq -e '.binary_hash == ...'` OR `lsof -i :18889` to confirm correct PID owns port.
- (mistral) Add `/health` endpoint to staging server reporting serving directory + binary hash.

**Verify:** Introduce port conflict OR wrong staging dir → preflight aborts with clear error.

---

### CF-4 [Theme D] — BLOCKED_PATTERNS overly restrictive [P1, 5/5]

**Root cause:** rc-sentry's `BLOCKED_PATTERNS` (rc-sentry/src/main.rs:722) includes `"| "` (pipe-space) which blocks legitimate Windows commands like `certutil ... | findstr ...`. Returns silent 403 without diagnostic detail. Patterns are deny-first without allowlist or override mechanism.

**Structural fix (consensus):**
- Refactor BLOCKED_PATTERNS to use a parser instead of regex (gemini, mimo).
- Allowlist known-safe command shapes (e.g., `certutil ... | findstr ...`) (deepseek, mimo).
- (deepseek, novel) Add HMAC-signed command bypass for trusted IPs (.23/.27).
- (mistral) Document allowed patterns + add unit tests for /exec payload validation.
- (gemini, novel) Add `/exec/validate` endpoint that pre-checks commands without executing — returns blocked-pattern diagnostic.

**Verify:** Deploy a SHA filter using `| findstr` from valid IP — confirm executes (not 403). Test 20 injection attempts — confirm all blocked.

---

### CF-5 [Theme E] — Silent failure propagation gaps [P1, 5/5]

**Root cause:** Multiple silent-failure paths: rc-sentry returns 403 but deploy script doesn't check HTTP status; bg task harness reports `exit 0` despite EPERM crash + 0-byte output; staging server failed bind is swallowed.

**Structural fix (consensus):**
- Modify rc-sentry to return JSON `{success: false, error: "blocked pattern"}` (mimo).
- Deploy script checks `jq -e '.success'` after each /exec.
- Bg harness validates output size > 0 for critical commands.
- (deepseek) Add HTTP status code validation on every /exec response.

**Verify:** Send blocked pattern command → script aborts with descriptive error. Test EPERM scenario → returns non-zero exit.

---

### CF-6 [Theme F] — Orphan bg process accumulation → handle exhaustion [P1, 5/5]

**Root cause:** Background tasks (bash subshells, python http.server) leave orphans across deploy sessions. Handle exhaustion contributes to subsequent EPERM failures (Issue 7). No process lifecycle management.

**Structural fix (consensus):**
- (mimo) Wrap deploy in Windows job object with `TerminateJobObject` on exit.
- Session-start cleanup: `taskkill /F /IM python.exe /FI "WINDOWTITLE eq *http.server*"`.
- (mistral) Implement process reaping in rc-sentry + deploy scripts.
- (gemini) Use `wait` for background processes; add cleanup routine at script end.

**Verify:** Run 10 sequential deploys → check for orphans with `tasklist`; confirm no handle leaks.

---

### CF-7 [Theme G] — Standardize JSON encoding on jq [P1, 4/5]

**Root cause:** Multiple JSON encoding methods (heredoc/printf/Python json.dump) produce inconsistent escaping; `\R` invalid JSON escape blocked rc-sentry.

**Structural fix (consensus):** Standardize on `jq -nc --arg cmd 'literal' '{cmd:$cmd}'` for all rc-sentry /exec payloads. Document in CLAUDE.md Comms section.

**Verify:** Test commands with backslashes/quotes/special chars → confirm correct execution.

---

### CF-8 [Theme H] — Cross-source timing correlation [P1, 5/5]

**Root cause:** Single-source observability (e.g., racecontrol last_seen alone) led to misinterpreting Pod 5 outage as "during deploy" when it was 4h later (Issue 11). No unified timeline.

**Structural fix (consensus):**
- Add structured logging to central collector (Windows Event Log → Fluentd → ES per gemini; or simpler central jsonl per mimo).
- Add correlation ID to all deploy operations.
- (mistral) Real-time dashboard with deploy state per pod + alerts for OTA_DEPLOYING > 30s.

**Verify:** Simulate outage; confirm central logs reconstruct timeline consistently across all sources.

---

### CF-9 [Theme J] — Watchdog deploy-aware health checks [P1, 4/5, **NOVEL**]

**Root cause (novel — not in session RCA):** Watchdog's `health_poller::poll_agent_health()` is independent of deploy state. Fixed polling interval creates race window where legitimate restarts appear as failures, triggering rollback even with sentinel.

**Structural fix (consensus):**
- (mimo) Add deployment-aware mode: when `OTA_DEPLOYING` exists, watchdog increases POLL_INTERVAL to 30s and skips rollback.
- (mistral) Bilateral protocol — rc-agent signals state ("starting up", "healthy", "graceful_shutdown") via /health endpoint; watchdog interprets state-aware.
- (qwen3) Implement deploy-aware health check suppression with explicit coordination protocol.
- (gemini) Enhance /health to expose `startup_phase` + `graceful_shutdown_in_progress`; watchdog interprets intelligently.

**Verify:** Set sentinel + kill agent → confirm watchdog waits 30s before respawn → test health check backoff sequence.

---

## §3 — Minority/Singleton findings worth tracking

| ID | Title | Models | Disposition |
|---|---|---|---|
| L | Two-phase commit between control server + pods | 2/5 | DEFER — useful for V2 architecture, not urgent for current deploy fix |
| M | Idempotent deploy operations | 2/5 | DEFER — composes with CF-1 (atomic primitive) |
| N | Manifest trust gap (signed binaries) | 3/5 | KEEP-MINORITY — fold into V2 security debt ledger |
| O | build_id verify BEFORE swap | 1/5 | DEFER — current SHA verify is sufficient near-term |
| P | start-rcagent.bat trust gap | 1/5 | DEFER — bat is well-tested in production |
| Q | Deploy progress + timeout enforcement | 2/5 | KEEP-MINORITY — composes with CF-8 observability |
| K | HTTPS + code signing | 1/5 | DEFER — V2 security sprint scope |

---

## §4 — Recommended P0 priority order (composite of all 5 models' rankings)

1. **CF-1 + CF-2 (atomic swap + sentinel discipline)** — single PR; can't do one without the other
2. **CF-3 (dry-run + staging preflight)** — wraps CF-1 with safety
3. **CF-9 (watchdog deploy-aware health)** — coordinated change to rc-watchdog (related to CF-2)
4. **CF-4 (BLOCKED_PATTERNS refactor)** — unblocks SHA filter + future deploy ops
5. **CF-5 (silent-failure propagation)** — orthogonal to atomic flow
6. **CF-6 (orphan process cleanup)** — Windows-environmental hardening
7. **CF-7 (jq JSON standard)** — discipline doc + script conversion
8. **CF-8 (cross-source observability)** — longer-horizon investment

---

## §5 — Findings that MMA caught but session RCA missed

Per cross-model `missed_in_session_rca`:

1. **CF-9 — Watchdog deploy-aware health checks** (3 models flagged this NOVEL) — single-author RCA noted "rollback fires when health fails 2x" but didn't propose **changing watchdog** to be deploy-aware (just stay-set-sentinel-longer). MMA proposes structural fix on the rc-watchdog side.

2. **CF-4 — BLOCKED_PATTERNS deserves design refactor** — single-author RCA noted "fix the script's filter" but MMA proposes refactor on rc-sentry side (allowlist + parser + validate endpoint).

3. **N — Manifest trust gap** (3 models) — session RCA mentions release-manifest.toml exists but doesn't propose end-to-end signing chain.

4. **L — Bilateral consistency / 2PC** (2 models) — session RCA assumes single-pod control flow; doesn't consider control-server-coordinated fleet 2PC.

5. **M — Idempotent deploy ops** (2 models) — current scripts are imperative; declarative state-machine model would be more robust.

These findings represent the value-add of MMA over single-author RCA.

---

## §6 — What I observed (CGP H3 evidence)

**BEHAVIOR**: 5 OpenRouter API calls completed; per-model JSON responses parsed and clustered into 18 themes; consensus matrix derived by counting models per theme.

**RAW OUTPUT**: `resp-<model>.md` (5 files) at `racecontrol/.planning/specs/v2/MMA-DEPLOY-RCA-DIAGNOSE/`; `meta-<model>.json` per-model usage; `openrouter-spend-james.jsonl` appended.

**WHERE**: 5 models invoked from James .27 (this terminal); responses written to local files.

**NOT TESTED**:
- MMA Step 2 PLAN — would design fix plans for CF-1..CF-9 (NOT YET RUN)
- MMA Step 3 EXECUTE — would apply smallest fix per consensus (NOT YET RUN)
- MMA Step 4 VERIFY — adversarial models scoring the proposed fixes (NOT YET RUN)
- Whether any of the proposed fixes WORK — pure analysis output, no implementation
- Cross-validation with bono — bono not consulted (single-pilot synthesis)
- Independence assumption — models may share training-data biases (e.g., 4 of 5 converge on jq because it's a common SO answer)

---

## §7 — Open items for Captain

1. **Authorize MMA Step 2 PLAN** for top-3 P0 findings (CF-1+CF-2 bundle + CF-3) — would design specific PRs with risk + rollback. Estimated cost ~$0.05.
2. **Disposition for novel findings** (CF-9 watchdog deploy-aware + N manifest trust + L 2PC) — accept into V2 substrate or DEFER?
3. **PR sequencing decision** — CF-1+CF-2 bundle first (highest leverage, also unblocks fleet deploy of PR #66), then CF-3 wrapping?
4. **Promote CANDIDATE-N1 G9s** from session RCA (G9 #1 dry-test-on-1-target, G9 #2 grep-error-source, G9 #4 sentinel-discipline-with-atomic-chain, G9 #3 cross-reference-timing) — at least 2 of these now have N=2 evidence (1 in session, 1 in MMA cross-vendor consensus).

---

## §8 — Composes-with

- `project_session_pr66_deploy_session_rca_20260509.md` — single-author 11-issue RCA (this MMA confirms 9 themes + adds 4 missed)
- `project_pod5_offline_during_deploy_rca_20260509.md` — Pod 5 outage RCA (CF-8 cross-source timing was learned here)
- `project_silent_loop_death_v1v2_rca_20260509.md` — PR #66 fix RCA (this MMA explains why fleet deploy of PR #66 is blocked)
- CLAUDE.md "Deploy" + "Comms" sections — multiple proposed amendments (jq standard, dry-run rule, atomic-chain pattern with sentinel)
- `feedback_canonical_deploy_path_vs_symmetry_projected_user_candidate_n1.md` — predates this MMA; confirmed canonical path itself has bugs CF-1, CF-4

---

## §9 — Spend log entry (appended to openrouter-spend-james.jsonl)

```json
{
  "timestamp": "2026-05-09T14:08:42Z",
  "ts_ist": "2026-05-09 19:38 IST",
  "pilot": "james",
  "session_purpose": "MMA Step 1 DIAGNOSE — rc-agent fleet deployment mechanism RCA (Captain-requested)",
  "mma_step": "DIAGNOSE",
  "models": ["deepseek/deepseek-r1-0528","qwen/qwen3-coder","xiaomi/mimo-v2-pro","google/gemini-2.5-flash","mistralai/mistral-small-2603"],
  "vendor_families": ["deepseek","qwen","xiaomi","google","mistral"],
  "valid_responses": 5,
  "total_responses": 5,
  "total_cost_usd": 0.0409,
  "anchor": "MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md",
  "consensus_findings": 9,
  "novel_findings": 4
}
```

— james / 2026-05-09 ~19:40 IST · MMA Step 1 DIAGNOSE complete · 5/5 valid responses · 9 consensus findings (CF-1..CF-9) · awaits Captain disposition on Step 2 PLAN authorization
