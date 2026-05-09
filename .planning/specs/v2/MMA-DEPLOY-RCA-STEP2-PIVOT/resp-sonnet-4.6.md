```json
{
  "pr_title": "fix(rc-sentry/rc-watchdog/deploy-pod): atomic deploy endpoint + watchdog deploy-aware mode (CF-1+CF-2+CF-9 bundle)",
  "selected_approach": {
    "cf1_atomicity": "server_side_mutex_in_exec_atomic_deploy",
    "cf2_sentinel": "internal_lifecycle_in_endpoint_with_ttl_json_and_mtime_fallback",
    "cf9_watchdog_aware": "deploy_state_query_with_extended_poll_interval"
  },
  "files_touched": [
    {"file": "crates/rc-sentry/src/main.rs", "kind": "edit", "loc_estimate": 145},
    {"file": "crates/rc-sentry/src/atomic_deploy.rs", "kind": "new", "loc_estimate": 210},
    {"file": "crates/rc-watchdog/src/service.rs", "kind": "edit", "loc_estimate": 55},
    {"file": "crates/rc-watchdog/src/rollback_manager.rs", "kind": "edit", "loc_estimate": 60},
    {"file": "crates/rc-watchdog/src/deploy_state.rs", "kind": "new", "loc_estimate": 80},
    {"file": "scripts/deploy-pod.sh", "kind": "edit", "loc_estimate": 25},
    {"file": "scripts/deploy-watchdog.sh", "kind": "new", "loc_estimate": 70},
    {"file": "crates/rc-sentry/src/atomic_deploy_tests.rs", "kind": "new", "loc_estimate": 110},
    {"file": "crates/rc-watchdog/src/deploy_state_tests.rs", "kind": "new", "loc_estimate": 65}
  ],
  "actions": [
    {
      "id": "A1",
      "file": "crates/rc-sentry/src/atomic_deploy.rs",
      "kind": "new",
      "summary": "New module implementing POST /exec_atomic_deploy handler. Declares a process-wide tokio::sync::Mutex<Option<ActiveDeploy>> named DEPLOY_MUTEX (lazy_static or once_cell). Handler signature: async fn exec_atomic_deploy(State(app): State<AppState>, Json(req): Json<AtomicDeployRequest>) -> Json<AtomicDeployResponse>. AtomicDeployRequest fields: {binary_url: String, expected_sha256: String, expected_build_id: String, deploy_id: String, timeout_secs: u32}. AtomicDeployResponse: {success: bool, deploy_id: String, swap_completed_at: Option<String>, error: Option<DeployErrorKind>, sentinel_cleared: bool}. DeployErrorKind enum: MutexContention | SentinelWriteFailed | KillFailed | CopyFailed | SwapFailed | Sha256Mismatch | AlreadyCompleted | Timeout. Logic flow: (1) try_lock DEPLOY_MUTEX — if contention, return MutexContention immediately (fail-fast, no queue). (2) Check idempotency: if active deploy has same deploy_id and swap_completed_at is Some, return AlreadyCompleted with cached result — prevents double-swap on client timeout-retry. (3) Write OTA_DEPLOYING JSON sentinel (see A2 for schema) — if write fails, release mutex, return SentinelWriteFailed. (4) Call taskkill /F /IM rc-agent.exe — capture exit code; if non-zero AND rc-agent.exe process still visible in process list, return KillFailed + clear sentinel. (5) Copy new binary to rc-agent.exe.new (temp name avoids partial-overwrite of live path). (6) Rename rc-agent.exe → rc-agent.exe.prev (atomic on same volume). (7) Rename rc-agent.exe.new → rc-agent.exe. Steps 5-7 wrapped in a closure; on any error: attempt restore rc-agent.exe.prev → rc-agent.exe, clear sentinel, return SwapFailed. (8) Verify rc-agent.exe exists + sha256 matches expected_sha256. (9) Clear OTA_DEPLOYING sentinel. (10) Record swap_completed_at = Utc::now().to_rfc3339(). (11) Release mutex. Return success response. NOTE: rc-sentry does NOT restart rc-agent — watchdog handles that via its normal respawn path, which is now safe because sentinel is cleared only after swap is verified.",
      "loc_estimate": 210,
      "risk": "high",
      "risk_reason": "Core atomicity guarantee lives here. Mutex scope must span steps 3-10 without await points that could be cancelled by tokio runtime without running cleanup. Use tokio::select! with timeout_secs to bound total operation. Ensure MutexGuard is held across all file ops — no .await between lock acquisition and release except inside the select! arm.",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-3", "FL-CONV-4"]
    },
    {
      "id": "A2",
      "file": "crates/rc-sentry/src/atomic_deploy.rs",
      "kind": "new",
      "summary": "Implement write_ota_deploying_sentinel() and clear_ota_deploying_sentinel() and auto_clear_ota_deploying_json() mirroring the proven auto_clear_maintenance_mode_json() pattern from rollback_manager.rs. Sentinel JSON schema: {deploy_id: String, started_at: ISO8601, ttl_secs: u32, build_id: String, pid: u32}. Default TTL: 300s (matching MAINTENANCE_MODE TTL). write_ota_deploying_sentinel() atomically writes via temp-file + rename (same pattern as BUG-71 fix). auto_clear_ota_deploying_json() called by watchdog's deploy_state.rs (A5): parse JSON → check started_at + ttl_secs vs now; if expired, delete sentinel and log WARNING. Parse failure policy (FL-CONV-3): log WARNING('OTA_DEPLOYING sentinel parse failed, falling back to mtime') → stat sentinel file mtime → if mtime within 60s, treat as deploy-in-progress; if mtime older than 60s, treat as stale + delete sentinel. Legacy bare-file fallback (no JSON content): same mtime check with 60s window. The 60s mtime window is intentionally shorter than 300s TTL to bound the blast radius of a corrupted sentinel.",
      "loc_estimate": 0,
      "risk": "low",
      "risk_reason": "Pattern is proven (BUG-71). Only new element is the 60s mtime fallback window — value is a captain decision (PV-Q2).",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-2", "FL-CONV-3"]
    },
    {
      "id": "A3",
      "file": "crates/rc-sentry/src/main.rs",
      "kind": "edit",
      "summary": "Register two new routes on the axum router: POST /exec_atomic_deploy → atomic_deploy::exec_atomic_deploy handler; GET /deploy_state → atomic_deploy::get_deploy_state handler. get_deploy_state returns {in_progress: bool, deploy_id: Option<String>, started_at: Option<String>, ttl_secs: Option<u32>} — reads from DEPLOY_MUTEX state without blocking (try_lock; if locked, in_progress=true). Also add BLOCKED_PATTERNS guard exemption: /exec_atomic_deploy must NOT be in BLOCKED_PATTERNS (line 722 guard). Verify existing /exec route is unchanged — backward compat for any tooling not yet migrated. AppState must include Arc<Mutex<Option<ActiveDeploy>>> for shared state between handler and get_deploy_state. LOC estimate covers route registration + AppState extension + BLOCKED_PATTERNS audit.",
      "loc_estimate": 145,
      "risk": "medium",
      "risk_reason": "BLOCKED_PATTERNS at line 722 could silently block the new endpoint if pattern matching is substring-based. Must audit pattern list. AppState extension is additive — no existing handler changes.",
      "addresses_flaw": ["FL-CONV-4"]
    },
    {
      "id": "A4",
      "file": "crates/rc-watchdog/src/deploy_state.rs",
      "kind": "new",
      "summary": "New module: DeployStateChecker. Primary method: fn check_deploy_in_progress(&self) -> DeployCheckResult. Strategy: (1) HTTP GET rc-sentry:8091/deploy_state with 2s timeout — if reachable and in_progress=true, return DeployInProgress{deploy_id, started_at}. (2) If rc-sentry unreachable (connection refused, timeout): fall back to reading OTA_DEPLOYING sentinel file directly from filesystem. (3) Sentinel file read: attempt JSON parse → apply TTL check → apply mtime fallback per A2 policy. (4) If sentinel absent: return DeployNotInProgress. DeployCheckResult enum: DeployInProgress{deploy_id: String, grace_until: Instant} | DeployNotInProgress | CheckFailed{reason: String}. On CheckFailed: log WARNING, treat as DeployNotInProgress (fail-open toward rollback — conservative). NOTE: fail-open is intentional: if watchdog cannot determine deploy state, it should not suppress rollback indefinitely, because that risks masking a real crash. This is a deliberate policy choice surfaced as PV-Q3.",
      "loc_estimate": 80,
      "risk": "medium",
      "risk_reason": "Fail-open policy (CheckFailed → allow rollback) is conservative but correct. The alternative (fail-closed → suppress rollback) risks indefinite suppression if sentry is down. Captain should confirm this policy (PV-Q3).",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-3", "FL-CONV-4"]
    },
    {
      "id": "A5",
      "file": "crates/rc-watchdog/src/service.rs",
      "kind": "edit",
      "summary": "Modify poll loop in service.rs (lines 234-559). At top of each poll cycle, BEFORE health check evaluation: call DeployStateChecker::check_deploy_in_progress(). If DeployInProgress: set next_poll_interval = 30s, skip rollback evaluation entirely (continue loop), log INFO('deploy in progress for deploy_id={}, skipping health check'). If DeployNotInProgress or CheckFailed: proceed with existing health check logic at existing POLL_INTERVAL. Also update /health response struct to include: startup_phase: String (values: 'starting'|'running'|'degraded'), graceful_shutdown_in_progress: bool, deploy_in_progress: bool (mirrors DeployStateChecker last result). startup_phase and graceful_shutdown_in_progress are existing internal state — expose them. deploy_in_progress is populated from last DeployStateChecker result cached between polls.",
      "loc_estimate": 55,
      "risk": "medium",
      "risk_reason": "Modifying the poll loop is the highest-blast-radius change in rc-watchdog. The 30s extended interval must not interact with the existing 2x-failure rollback counter — counter must be RESET (not merely paused) when deploy_in_progress transitions from true to false, to avoid a stale failure count triggering immediate rollback post-deploy.",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-4", "FL-CONV-5"]
    },
    {
      "id": "A6",
      "file": "crates/rc-watchdog/src/rollback_manager.rs",
      "kind": "edit",
      "summary": "Lines 121-128: replace bare is_file() OTA_DEPLOYING check with DeployStateChecker::check_deploy_in_progress() call (reuse module from A4). This ensures perform_rollback() uses the same TTL-aware + mtime-fallback logic as the poll loop, not the legacy bare is_file() that caused FL-CONV-2. Lines 174-187 (binary removal) and line 190 (prev→current restoration): no change — rollback logic itself is correct per Step 1 DIAGNOSE. Add explicit log line at rollback entry: 'perform_rollback() called; deploy_in_progress={}'.",
      "loc_estimate": 60,
      "risk": "high",
      "risk_reason": "This is the FL-CONV-2 fix for the OLD watchdog path. If this edit ships on Pod 8 BEFORE the new sentinel JSON format is in use, there is no regression (DeployStateChecker handles both legacy bare-file and new JSON). But if OLD rc-watchdog (pre-this-edit) is on Pod 8 when new deploy-pod.sh runs, Pod 8 watchdog will read bare is_file() and suppress indefinitely. Rollout ordering (A8) is the mitigation.",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-3"]
    },
    {
      "id": "A7",
      "file": "scripts/deploy-pod.sh",
      "kind": "edit",
      "summary": "Replace the multi-step /exec chain (kill + copy + move as 3 separate HTTP roundtrips) with a single POST /exec_atomic_deploy call to rc-sentry:8091. New flow: (1) Compute sha256 of local binary. (2) POST {binary_url, expected_sha256, expected_build_id, deploy_id=$(uuidgen), timeout_secs=120} to http://${POD_IP}:8091/exec_atomic_deploy. (3) Parse JSON response: if success=true, log and continue. If success=false, log error.error + exit 1. (4) Poll rc-agent:8090/health for RUNNING state with 30s timeout (10 x 3s intervals) — this is the post-swap startup verification. (5) Remove the SHA filter line at line 138 containing ' | ' (CF-4 fix is out of scope for this PR but the line should be audited — add TODO comment). Client-side sentinel write is REMOVED entirely. deploy_id is generated client-side (uuidgen) and echoed to log for correlation with sentry logs.",
      "loc_estimate": 25,
      "risk": "low",
      "risk_reason": "Client becomes a thin caller. The only new client-side logic is sha256 computation (standard) and response parsing. Idempotency on retry is handled server-side via deploy_id.",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "A8",
      "file": "scripts/deploy-watchdog.sh",
      "kind": "new",
      "summary": "New script mirroring scripts/deploy-sentry.sh pattern. Arguments: POD_IP, WATCHDOG_BINARY_PATH. Steps: (1) SCP new rc-watchdog.exe to pod staging dir. (2) sc stop RCWatchdog on pod (via SSH or WinRM). (3) Wait for SERVICE_STOPPED: poll 'sc query RCWatchdog' for STOPPED state, 15s timeout, 3s interval. (4) Copy staged binary over existing rc-watchdog.exe. (5) sc start RCWatchdog. (6) Poll 'sc query RCWatchdog' for RUNNING state, 30s timeout, 3s interval (FL-CONV-5 fix). If RUNNING not reached within 30s: log ERROR, attempt sc stop, restore previous binary from .prev backup, sc start, exit 1. (7) HTTP GET http://${POD_IP}:8091/health — verify watchdog health endpoint responds 200 within 10s. (8) Log success with pod IP and binary sha256. Windows Service Recovery settings: configure via 'sc failure RCWatchdog reset=86400 actions=restart/5000/restart/10000/restart/30000' — document in script header comment. Script exits non-zero on any step failure — caller (rollout runbook) must HALT fleet rollout on first failure.",
      "loc_estimate": 70,
      "risk": "medium",
      "risk_reason": "sc stop/start on a live pod has a brief window where watchdog is not running. During this window, if rc-agent crashes, no rollback occurs. Window is bounded by sc stop + binary copy + sc start latency (~10-20s typical). Acceptable because: (a) we are not deploying rc-agent simultaneously, (b) rc-agent is stable on all pods at this phase.",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-5"]
    },
    {
      "id": "A9",
      "file": "crates/rc-sentry/src/atomic_deploy_tests.rs",
      "kind": "new",
      "summary": "Unit + integration tests. T1-T4 live here (see test_plan). Uses tempdir for sentinel file ops. Mocks taskkill via injectable KillStrategy trait (prod impl calls taskkill; test impl returns configurable exit codes). Tests the mutex contention path by spawning two concurrent deploy requests with same deploy_id (expect second returns MutexContention). Tests idempotency: complete a deploy, retry with same deploy_id, expect AlreadyCompleted with same swap_completed_at.",
      "loc_estimate": 110,
      "risk": "low",
      "risk_reason": "Test-only code. KillStrategy trait injection is the only prod-code surface added for testability — keep it behind #[cfg(test)] or a feature flag.",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-3", "FL-CONV-4"]
    },
    {
      "id": "A10",
      "file": "crates/rc-watchdog/src/deploy_state_tests.rs",
      "kind": "new",
      "summary": "Unit tests for DeployStateChecker (T5-T7). Tests: (1) Valid JSON sentinel within TTL → DeployInProgress. (2) Corrupted JSON sentinel within 60s mtime → DeployInProgress (mtime fallback). (3) Corrupted JSON sentinel older than 60s mtime → DeployNotInProgress + sentinel deleted. (4) rc-sentry unreachable + valid sentinel → DeployInProgress (filesystem fallback). (5) rc-sentry unreachable + no sentinel → DeployNotInProgress. (6) Expired TTL sentinel (started_at + ttl_secs < now) → DeployNotInProgress + sentinel deleted.",
      "loc_estimate": 65,
      "risk": "low",
      "risk_reason": "Pure unit tests with tempdir. No prod risk.",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-3"]
    }
  ],
  "test_plan": [
    {
      "id": "T1",
      "kind": "integration",
      "what": "Race scenario: inject artificial 500ms sleep between taskkill and rename inside exec_atomic_deploy (via KillStrategy mock that sleeps). Spawn a goroutine simulating watchdog poll calling perform_rollback() during the sleep window. Assert: rollback is suppressed because DEPLOY_MUTEX is held; swap completes; sentinel is cleared; rc-agent.exe contains new binary.",
      "expected": "perform_rollback() returns early with 'deploy in progress' log line. swap_completed_at is set. sentinel file absent post-deploy.",
      "exercises_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "T2",
      "kind": "unit",
      "what": "JSON parse failure: write a sentinel file containing '{invalid json' to tempdir. Call DeployStateChecker::check_deploy_in_progress() with mtime set to 30s ago (within 60s window). Assert DeployInProgress returned. Repeat with mtime 90s ago. Assert DeployNotInProgress returned and sentinel file deleted.",
      "expected": "Within-window: DeployInProgress. Outside-window: DeployNotInProgress + file deleted + WARNING logged.",
      "exercises_flaw": ["FL-CONV-3"]
    },
    {
      "id": "T3",
      "kind": "unit",
      "what": "sc start failure simulation: deploy-watchdog.sh with mock sc that returns STOPPED after start attempt. Assert script exits non-zero, logs ERROR, restores previous binary, attempts sc start of previous binary.",
      "expected": "Script exit code 1. Previous rc-watchdog.exe restored. Rollback sc start attempted.",
      "exercises_flaw": ["FL-CONV-5"]
    },
    {
      "id": "T4",
      "kind": "integration",
      "what": "Backward compat: OLD rc-watchdog (pre-A6, bare is_file() at line 121) running against NEW deploy-pod.sh that calls /exec_atomic_deploy. Simulate by running old rollback_manager binary against a pod where /exec_atomic_deploy is in progress. Assert: old watchdog reads bare sentinel file via is_file() → suppresses rollback (this is the KNOWN behavior during transition window, not a regression). Assert: after deploy completes and sentinel is cleared, old watchdog resumes normal poll. This test documents the transition window risk rather than eliminating it — elimination requires A8 rollout ordering.",
      "expected": "Old watchdog suppresses rollback during deploy (acceptable). Resumes after sentinel cleared. No permanent suppression.",
      "exercises_flaw": ["FL-CONV-2"]
    },
    {
      "id": "T5",
      "kind": "live-pod",
      "what": "Pod 1 canary post-rc-watchdog deploy (Phase 2 of rollout). Deploy new rc-watchdog to Pod 1 via deploy-watchdog.sh. Verify sc query shows RUNNING. Verify GET /health returns deploy_in_progress field. Then run deploy-pod.sh targeting Pod 1 with new binary. Monitor watchdog logs for 'deploy in progress, skipping health check' during swap window. Verify rc-agent comes up on new binary. Verify no spurious rollback.",
      "expected": "Pod 1 stable on new binary within 5min soak. Watchdog logs show deploy-aware skip. No rollback triggered. /health shows startup_phase='running' post-deploy.",
      "exercises_flaw": ["FL-CONV-1", "FL-CONV-2", "FL-CONV-4"]
    },
    {
      "id": "T6",
      "kind": "unit",
      "what": "Idempotency: call exec_atomic_deploy handler twice with same deploy_id. First call completes successfully. Second call (simulating client timeout-retry) returns AlreadyCompleted with identical swap_completed_at. Assert binary is not swapped a second time.",
      "expected": "Second response: {success: false, error: 'AlreadyCompleted', swap_completed_at: <same as first>, sentinel_cleared: true}.",
      "exercises_flaw": ["FL-CONV-4"]
    },
    {
      "id": "T7",
      "kind": "integration",
      "what": "TTL expiry during long deploy: set TTL to 5s in test config. Inject 10s sleep in swap step. Assert auto_clear_ota_deploying_json() fires and deletes sentinel at ~5s. Assert watchdog (polling at 30s extended interval) detects sentinel gone at next poll and resumes normal health checks. Assert deploy handler returns Timeout error and clears partial state.",
      "expected": "Sentinel deleted at TTL. Watchdog resumes normal poll. Deploy returns Timeout. rc-agent.exe.prev restored if swap was partial.",
      "exercises_flaw": ["FL-CONV-1", "FL-CONV-3"]
    }
  ],
  "rollout_plan": {
    "phase_1": "rc-watchdog deploy via deploy-watchdog.sh to Pods 1-4, 6-8 sequentially (Pod 5 OUT-