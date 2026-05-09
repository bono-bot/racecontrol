```json
{
  "pr_title": "fix(rc-sentry/rc-watchdog/deploy-pod): atomic deploy endpoint + watchdog deploy-aware mode (CF-1+CF-2+CF-9 bundle)",
  "selected_approach": {
    "cf1_atomicity": "server_side_mutex_in_exec_atomic_deploy",
    "cf2_sentinel": "internal_lifecycle_in_endpoint_with_ttl_json_and_mtime_fallback",
    "cf9_watchdog_aware": "deploy_state_query_with_extended_poll_interval"
  },
  "files_touched": [
    {"file": "crates/rc-sentry/src/main.rs", "kind": "edit", "loc_estimate": 120},
    {"file": "crates/rc-watchdog/src/service.rs", "kind": "edit", "loc_estimate": 60},
    {"file": "crates/rc-watchdog/src/rollback_manager.rs", "kind": "edit", "loc_estimate": 80},
    {"file": "scripts/deploy-pod.sh", "kind": "edit", "loc_estimate": 25},
    {"file": "scripts/deploy-watchdog.sh", "kind": "new", "loc_estimate": 40},
    {"file": "tests/test_atomic_deploy.rs", "kind": "new", "loc_estimate": 150}
  ],
  "actions": [
    {
      "id": "A1",
      "file": "crates/rc-sentry/src/main.rs",
      "kind": "edit",
      "summary": "Add POST /exec_atomic_deploy endpoint with process-wide deploy mutex (tokio::sync::Mutex), internal sentinel lifecycle, and idempotent deploy_id handling",
      "loc_estimate": 120,
      "risk": "medium",
      "risk_reason": "New endpoint with mutex contention handling; must ensure mutex is not held across await points that could block health checks",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "A2",
      "file": "crates/rc-sentry/src/main.rs",
      "kind": "edit",
      "summary": "Add GET /deploy_state endpoint returning {deploy_in_progress: bool, deploy_id: string|null, started_at: ISO8601|null, ttl_remaining_secs: int}",
      "loc_estimate": 25,
      "risk": "low",
      "risk_reason": "Read-only endpoint; no state mutation",
      "addresses_flaw": ["FL-CONV-2"]
    },
    {
      "id": "A3",
      "file": "crates/rc-watchdog/src/service.rs",
      "kind": "edit",
      "summary": "Modify poll_cycle() to query rc-sentry /deploy_state before health check; extend POLL_INTERVAL to 30s and skip rollback_evaluation when deploy_in_progress=true",
      "loc_estimate": 40,
      "risk": "medium",
      "risk_reason": "Watchdog must handle rc-sentry unreachability during deploy (fallback to direct sentinel read with mtime check)",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-3"]
    },
    {
      "id": "A4",
      "file": "crates/rc-watchdog/src/rollback_manager.rs",
      "kind": "edit",
      "summary": "Add auto_clear_ota_deploying_json() mirroring BUG-71 pattern: 300s TTL, JSON parse failure→mtime fallback, legacy bare-file→mtime fallback; integrate into rollback trigger logic",
      "loc_estimate": 60,
      "risk": "low",
      "risk_reason": "Proven pattern from maintenance_mode; explicit fallback chain",
      "addresses_flaw": ["FL-CONV-3"]
    },
    {
      "id": "A5",
      "file": "crates/rc-watchdog/src/rollback_manager.rs",
      "kind": "edit",
      "summary": "Update perform_rollback() to check deploy_in_progress via new is_deploy_in_progress() helper (queries /deploy_state or reads sentinel); skip rollback if true",
      "loc_estimate": 20,
      "risk": "low",
      "risk_reason": "Simple guard clause; no logic change to rollback itself",
      "addresses_flaw": ["FL-CONV-2"]
    },
    {
      "id": "A6",
      "file": "scripts/deploy-pod.sh",
      "kind": "edit",
      "summary": "Replace 3-step /exec chain with single curl POST to /exec_atomic_deploy; pass binary_url, expected_sha256, deploy_id (timestamp-based); handle JSON response",
      "loc_estimate": 25,
      "risk": "low",
      "risk_reason": "Script simplification; server handles atomicity",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "A7",
      "file": "scripts/deploy-watchdog.sh",
      "kind": "new",
      "summary": "New script mirroring deploy-sentinel.sh pattern: downloads rc-watchdog binary, verifies SHA, stops service, copies binary, starts service, polls sc query for RUNNING with 30s timeout",
      "loc_estimate": 40,
      "risk": "medium",
      "risk_reason": "Must handle sc start failure with retry; document Windows Service Recovery settings",
      "addresses_flaw": ["FL-CONV-5"]
    },
    {
      "id": "A8",
      "file": "tests/test_atomic_deploy.rs",
      "kind": "new",
      "summary": "Integration tests: race scenario (inject delay + watchdog poll), JSON parse failure, idempotent deploy_id, mutex contention, sentinel TTL expiry during long deploy",
      "loc_estimate": 150,
      "risk": "low",
      "risk_reason": "Test-only; exercises all failure modes",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-2", "FL-CONV-3", "FL-CONV-4", "FL-CONV-5"]
    }
  ],
  "test_plan": [
    {
      "id": "T1",
      "kind": "unit",
      "what": "Mutex contention: two concurrent /exec_atomic_deploy calls with different deploy_ids",
      "expected": "Second call returns 409 Conflict immediately (fail-fast)",
      "exercises_flaw": ["FL-CONV-4"]
    },
    {
      "id": "T2",
      "kind": "integration",
      "what": "Race scenario: inject 2s delay in kill+swap sequence; trigger watchdog poll during swap",
      "expected": "Watchdog sees deploy_in_progress=true, extends poll interval, does not trigger rollback",
      "exercises_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "T3",
      "kind": "integration",
      "what": "JSON parse failure: write corrupted sentinel (partial JSON) then call /exec_atomic_deploy",
      "expected": "Endpoint logs WARNING, deletes corrupted file, writes new sentinel, proceeds with deploy",
      "exercises_flaw": ["FL-CONV-3"]
    },
    {
      "id": "T4",
      "kind": "integration",
      "what": "Idempotent retry: client timeout after swap but before response; retry same deploy_id",
      "expected": "Second call returns success with original swap_completed_at (no double-swap)",
      "exercises_flaw": ["FL-CONV-1"]
    },
    {
      "id": "T5",
      "kind": "live-pod",
      "what": "Backward compat: deploy new rc-watchdog to Pod 1, then run old deploy-pod.sh (multi-step /exec)",
      "expected": "Old deploy still works; new watchdog reads legacy sentinel via mtime fallback",
      "exercises_flaw": ["FL-CONV-2"]
    },
    {
      "id": "T6",
      "kind": "live-pod",
      "what": "sc start failure: deploy rc-watchdog to Pod 3 with intentionally broken binary",
      "expected": "deploy-watchdog.sh detects sc query != RUNNING, retries 3x, then reports failure",
      "exercises_flaw": ["FL-CONV-5"]
    },
    {
      "id": "T7",
      "kind": "live-pod",
      "what": "Canary soak: Pod 1 with new rc-watchdog + new deploy-pod.sh; deploy 3 test binaries sequentially",
      "expected": "All deploys succeed; watchdog never triggers rollback; /health shows deploy_in_progress=false between deploys",
      "exercises_flaw": ["FL-CONV-1", "FL-CONV-2"]
    }
  ],
  "rollout_plan": {
    "phase_1": "rc-watchdog deploy via deploy-watchdog.sh to Pods 1-4, 6-8 (Pod 5 OUT-OF-SCOPE); per-pod sc query verify; HALT on any failure",
    "phase_2": "deploy-pod.sh updated to use /exec_atomic_deploy; canary on Pod 1 with new rc-watchdog; 5min stability soak",
    "phase_3": "fleet rollout Pods 2,3,4,6,7 sequential; Pod 8 last (canary stays on existing PR #66 binary until phase 3)",
    "rollback_plan": "If phase 1 fails: revert rc-watchdog via deploy-watchdog.sh with OLD binary (keep copy in /opt/rc/rollback/). If phase 2 fails: revert deploy-pod.sh to multi-step /exec chain. If phase 3 fails: halt rollout, investigate Pod 8 last-known-good binary (PR #66) as fleet fallback."
  },
  "captain_q_decisions": [
    {
      "id": "PV-Q1",
      "question": "Should /exec_atomic_deploy accept binary_url (HTTP download) or assume binary already present