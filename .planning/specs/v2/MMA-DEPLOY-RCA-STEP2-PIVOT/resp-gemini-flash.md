```json
{
  "pr_title": "fix(rc-sentry/rc-watchdog/deploy-pod): atomic deploy endpoint + watchdog deploy-aware mode (CF-1+CF-2+CF-9 bundle)",
  "selected_approach": {
    "cf1_atomicity": "server_side_mutex_in_exec_atomic_deploy",
    "cf2_sentinel": "internal_lifecycle_in_endpoint_with_ttl_json_and_mtime_fallback",
    "cf9_watchdog_aware": "deploy_state_query_with_extended_poll_interval"
  },
  "files_touched": [
    {
      "file": "crates/rc-sentry/src/main.rs",
      "kind": "edit",
      "loc_estimate": 150
    },
    {
      "file": "crates/rc-sentry/src/atomic_deploy.rs",
      "kind": "new",
      "loc_estimate": 200
    },
    {
      "file": "crates/rc-watchdog/src/service.rs",
      "kind": "edit",
      "loc_estimate": 100
    },
    {
      "file": "crates/rc-watchdog/src/rollback_manager.rs",
      "kind": "edit",
      "loc_estimate": 80
    },
    {
      "file": "crates/rc-watchdog/src/deploy_state.rs",
      "kind": "new",
      "loc_estimate": 70
    },
    {
      "file": "scripts/deploy-pod.sh",
      "kind": "edit",
      "loc_estimate": 50
    },
    {
      "file": "scripts/deploy-watchdog.sh",
      "kind": "new",
      "loc_estimate": 60
    },
    {
      "file": "crates/rc-sentry/tests/atomic_deploy_tests.rs",
      "kind": "new",
      "loc_estimate": 150
    },
    {
      "file": "crates/rc-watchdog/tests/deploy_aware_tests.rs",
      "kind": "new",
      "loc_estimate": 100
    }
  ],
  "actions": [
    {
      "id": "A1",
      "file": "crates/rc-sentry/src/atomic_deploy.rs",
      "kind": "new",
      "summary": "Implement `POST /exec_atomic_deploy` endpoint logic: acquire mutex, write OTA_DEPLOYING JSON, perform kill+rename, verify, clear sentinel, release mutex. Handle all failure paths with rollback and sentinel cleanup. Idempotent via `deploy_id`.",
      "loc_estimate": 200,
      "risk": "high",
      "risk_reason": "Core logic for atomicity, critical for correctness. Requires careful error handling and state management.",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "A2",
      "file": "crates/rc-sentry/src/main.rs",
      "kind": "edit",
      "summary": "Integrate `atomic_deploy.rs` module. Add `POST /exec_atomic_deploy` route. Implement process-wide `tokio::sync::Mutex` for deploy operations. Update `BLOCKED_PATTERNS` to allow `/exec_atomic_deploy`.",
      "loc_estimate": 100,
      "risk": "medium",
      "risk_reason": "Mutex implementation needs to be robust against deadlocks and panics. Route integration must be correct.",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "A3",
      "file": "crates/rc-sentry/src/main.rs",
      "kind": "edit",
      "summary": "Add `GET /deploy_state` endpoint to rc-sentry, returning current deploy status (e.g., `{'in_progress': true, 'deploy_id': '...'}` or `{'in_progress': false}`). This allows rc-watchdog to query directly.",
      "loc_estimate": 50,
      "risk": "low",
      "risk_reason": "Simple read-only endpoint, minimal risk.",
      "addresses_flaw": ["FL-CONV-2"]
    },
    {
      "id": "A4",
      "file": "crates/rc-watchdog/src/deploy_state.rs",
      "kind": "new",
      "summary": "Implement `DeployState` module for rc-watchdog. This module will encapsulate logic for querying rc-sentry's `/deploy_state` endpoint and parsing the response. Includes `auto_clear_ota_deploying_json` logic.",
      "loc_estimate": 70,
      "risk": "low",
      "risk_reason": "New module, isolated logic. Network calls need proper timeouts.",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-3"]
    },
    {
      "id": "A5",
      "file": "crates/rc-watchdog/src/service.rs",
      "kind": "edit",
      "summary": "Modify `rc-watchdog`'s main loop: BEFORE health check, query `rc-sentry`'s `/deploy_state`. If `in_progress`, extend `POLL_INTERVAL` to 30s and skip rollback evaluation. Expose `deploy_in_progress` via `/health`.",
      "loc_estimate": 80,
      "risk": "medium",
      "risk_reason": "Changes core watchdog behavior. Incorrect logic could lead to missed rollbacks or infinite loops.",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-9"]
    },
    {
      "id": "A6",
      "file": "crates/rc-watchdog/src/rollback_manager.rs",
      "kind": "edit",
      "summary": "Update `perform_rollback()` to use `DeployState` module for `OTA_DEPLOYING` check. Implement `auto_clear_ota_deploying_json` with TTL (300s), JSON parse failure fallback to mtime, and legacy bare `is_file()` fallback to mtime with bounded grace (60s).",
      "loc_estimate": 80,
      "risk": "medium",
      "risk_reason": "Sentinel logic is complex; incorrect implementation could lead to premature rollback or indefinite suppression.",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-3"]
    },
    {
      "id": "A7",
      "file": "scripts/deploy-watchdog.sh",
      "kind": "new",
      "summary": "Create `scripts/deploy-watchdog.sh` mirroring `deploy-sentry.sh` pattern. Handles downloading, stopping service, replacing binary, starting service, and verifying service status (`sc query RCWatchdog` for RUNNING with 30s timeout).",
      "loc_estimate": 60,
      "risk": "low",
      "risk_reason": "Standard deployment script, well-understood pattern.",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-5"]
    },
    {
      "id": "A8",
      "file": "scripts/deploy-pod.sh",
      "kind": "edit",
      "summary": "Update `scripts/deploy-pod.sh` to use `POST /exec_atomic_deploy` instead of the multi-step `/exec` chain. Pass `binary_url`, `expected_sha256`, `expected_build_id`, `deploy_id`, `timeout_secs`.",
      "loc_estimate": 50,
      "risk": "medium",
      "risk_reason": "Client-side script change, needs careful testing to ensure correct parameters are passed and error handling is robust.",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-4"]
    }
  ],
  "test_plan": [
    {
      "id": "T1",
      "kind": "unit",
      "what": "rc-sentry: `exec_atomic_deploy` happy path (kill+swap+verify)",
      "expected": "Returns success, binary is swapped, sentinel cleared.",
      "exercises_flaw": []
    },
    {
      "id": "T2",
      "kind": "unit",
      "what": "rc-sentry: `exec_atomic_deploy` failure during kill (e.g., binary locked)",
      "expected": "Returns failure, partial state rolled back, sentinel cleared.",
      "exercises_flaw": ["FL-CONV-1"]
    },
    {
      "id": "T3",
      "kind": "unit",
      "what": "rc-sentry: `exec_atomic_deploy` failure during rename (e.g., permissions)",
      "expected": "Returns failure, partial state rolled back, sentinel cleared.",
      "exercises_flaw": ["FL-CONV-1"]
    },
    {
      "id": "T4",
      "kind": "unit",
      "what": "rc-sentry: `exec_atomic_deploy` failure during verification (e.g., corrupted download)",
      "expected": "Returns failure, partial state rolled back, sentinel cleared.",
      "exercises_flaw": ["FL-CONV-1"]
    },
    {
      "id": "T5",
      "kind": "unit",
      "what": "rc-sentry: `exec_atomic_deploy` idempotency with same `deploy_id`",
      "expected": "Second call with same `deploy_id` returns success if first completed, or continues/retries if first failed mid-way.",
      "exercises_flaw": []
    },
    {
      "id": "T6",
      "kind": "integration",
      "what": "rc-sentry + rc-watchdog: Race scenario (artificial delay in `exec_atomic_deploy` kill+swap, watchdog polls)",
      "expected": "Watchdog queries `/deploy_state`, detects deploy in progress, extends `POLL_INTERVAL`, and does NOT trigger rollback.",
      "exercises_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "T7",
      "kind": "unit",
      "what": "rc-watchdog: `DeployState` with corrupted/partial OTA_DEPLOYING JSON file",
      "expected": "Falls back to mtime check with bounded grace window (60s), logs WARNING.",
      "exercises_flaw": ["FL-CONV-3"]
    },
    {
      "id": "T8",
      "kind": "unit",
      "what": "rc-watchdog: `DeployState` with legacy bare `OTA_DEPLOYING` file",
      "expected": "Falls back to mtime check with bounded grace window (60s), logs INFO.",
      "exercises_flaw": ["FL-CONV-2"]
    },
    {
      "id": "T9",
      "kind": "integration",
      "what": "scripts/deploy-watchdog.sh: `sc start` failure (e.g., service already running, or binary corrupted)",
      "expected": "Script detects failure via `sc query` and exits with error, providing clear diagnostic.",
      "exercises_flaw": ["FL-CONV-5"]
    },
    {
      "id": "T10",
      "kind": "live-pod",
      "what": "Backward compatibility: NEW `deploy-pod.sh` + OLD `rc-watchdog` (Pod 8 scenario)",
      "expected": "OLD `rc-watchdog` on Pod 8 should still read the `OTA_DEPLOYING` file via `is_file()` and suppress rollback indefinitely, as per its existing behavior. This confirms the new watchdog rollout ordering is critical.",
      "exercises_flaw": ["FL-CONV-2"]
    },
    {
      "id": "T11",
      "kind": "live-pod",
      "what": "Canary deploy on Pod 1: NEW `deploy-pod.sh` + NEW `rc-watchdog` + NEW `rc-sentry`",
      "expected": "Deploy completes successfully, `rc-agent` starts, `rc-watchdog` monitors correctly, no unexpected rollbacks.",
      "exercises_flaw": []
    }
  ],
  "rollout_plan": {
    "phase_1": "Deploy NEW `rc-watchdog` to Pods 1-4, 6-8 using `scripts/deploy-watchdog.sh`. Pod 5 is OUT-OF-SCOPE. For each pod: download, stop service, replace binary, start service, verify `sc query RCWatchdog` returns RUNNING within 30s. HALT on any failure. If a pod is transiently unreachable, retry after 15 minutes (max 3 retries) before escalating. No feature-flag gating for watchdog, as its new behavior is backward-compatible with old `rc-sentry` (it just won't get the `/deploy_state` info, but will still use `is_file()` for `OTA_DEPLOYING` if present).",
    "phase_2": "Update `scripts/deploy-pod.sh` in the deployment pipeline. Perform a canary deploy of a new `rc-agent` binary to Pod 1 (which now has the NEW `rc-watchdog`) using the updated `deploy-pod.sh` and the new `/exec_atomic_deploy` endpoint. Monitor Pod 1 for 5 minutes for stability (health checks, logs, no unexpected rollbacks).",
    "phase_3": "Fleet rollout of new `rc-agent` binary to Pods 2, 3, 4, 6, 7 sequentially, using the updated `deploy-pod.sh`. Pod 8 (still on PR #66 binary) will be deployed last to ensure maximum stability before touching the 'old watchdog' scenario. Each pod will have a 2-minute soak period. Pod 8 will then receive the new `rc-agent` binary. Monitor overall fleet health.",
    "rollback_plan": "If any phase fails: 1. Immediately halt further deployments. 2. For `rc-watchdog` rollout (Phase 1): Revert to previous `rc-watchdog` binary using `scripts/deploy-watchdog.sh` (which supports previous versions). 3. For `rc-agent` rollout (Phase 2/3): Revert `scripts/deploy-pod.sh` to its previous version. For any pods that received the new `rc-agent`, manually trigger a deploy of the previous stable `rc-agent` binary using the old `deploy-pod.sh` (or a specific rollback script if `exec_atomic_deploy` is deemed unstable). The `exec_atomic_deploy` endpoint itself has internal rollback mechanisms for partial failures. Monitor sentinel files for unexpected persistence. In case of mutex deadlock, manual intervention to restart `rc-sentry` might be required (this is a high-risk scenario to be mitigated by robust mutex implementation)."
  },
  "captain_q_decisions": [
    {
      "id": "PV-Q1",
      "question": "Should `rc-sentry`'s `/deploy_state` endpoint be authenticated?",
      "default_recommendation": "No, given it's an internal endpoint on a secured pod network and only provides status. Adding auth adds complexity and potential for rollout issues.",
      "rationale": "The `rc-sentry` and `rc-watchdog` communicate over localhost within the same pod. The risk of unauthorized access to this specific status endpoint is minimal compared to the overhead of implementing and managing authentication for it. The `/exec_atomic_deploy` endpoint itself will be secured by existing network policies and potentially client-side authentication if deemed necessary for broader `/exec` endpoints."
    },
    {
      "id": "PV-Q2",
      "question": "What is the maximum acceptable `timeout_secs` for `/exec_atomic_deploy`?",
      "default_recommendation": "300 seconds (5 minutes).",
      "rationale": "This aligns with the `OTA_DEPLOYING` sentinel TTL. A deploy should not take longer than 5 minutes. If it does, it indicates a deeper problem, and the client should time out and retry or escalate. This prevents indefinite blocking of the mutex."
    }
  ],
  "verify_post_deploy": [
    {
      "step": 1,
      "command": "for pod in $(seq 1 8); do ssh pod-$pod 'sc query RCWatchdog'; done",
      "pass_criterion": "All pods (1-4, 6-8) show RCWatchdog service as 'RUNNING'. Pod 5 is expected to be unreachable."
    },
    {
      "step": 2,
      "command": "scripts/deploy-pod.sh --target pod-1 --binary-id <new_agent_binary_id>",
      "pass_criterion": "Deployment completes successfully, `rc-agent` on pod-1 is running the new binary, and `rc-sentry` logs show successful `exec_atomic_deploy` completion. No `rc-watchdog` rollback events observed on pod-1."
    },
    {
      "step": 3,
      "command": "for pod in $(seq 1 8); do ssh pod-$pod 'curl -s localhost:8090/health | jq .deploy_in_progress'; done",
      "pass_criterion": "All pods show `false` for `deploy_in_progress` after successful deployment."
    },
    {
      "step": 4,
      "command": "for pod in $(seq 1 8); do ssh pod-$pod 'ls -l C:\\ProgramData\\rc-agent\\OTA_DEPLOYING.json'; done",
      "pass_criterion": "No `OTA_DEPLOYING.json` files exist on any pod after successful deployment."
    }
  ],
  "fl_conv_addressing": {
    "FL-CONV-1_sentinel_before_chain": "The `exec_atomic_deploy` endpoint on `rc-sentry` manages the entire lifecycle of the `OTA_DEPLOYING` sentinel internally. It writes the sentinel *before* the kill+swap, and clears it *after* successful verification or *after* any rollback on failure. This eliminates the client-side ordering hazard and the silent fleet death window.",
    "FL-CONV-2_pod_8_old_watchdog": "The rollout plan explicitly deploys the NEW `rc-watchdog` to all reachable pods (1-4, 6-8) *before* `scripts/deploy-pod.sh` is updated. This ensures that when the new `deploy-pod.sh` (using `/exec_atomic_deploy`) is introduced, all active `rc-watchdog` instances are deploy-aware. Pod 8, if it remains on its OLD `rc-watchdog` during the `rc-agent` rollout, will still correctly interpret the `OTA_DEPLOYING.json` file via its `is_file()` check and suppress rollback indefinitely, as per its existing behavior, preventing premature rollback. The new `rc-watchdog` will query `/deploy_state` for more robust detection.",
    "FL-CONV-3_json_parse_fail": "The `rc-watchdog`'s `DeployState` module (A4, A6) will explicitly handle JSON parse failures for `OTA_DEPLOYING.json`. If parsing fails, it will log a WARNING and fall back to using the file's modification timestamp (`mtime`) to infer a deploy in progress, with a bounded grace window (e.g., 60 seconds). This prevents indefinite suppression due to malformed JSON.",
    "FL-CONV-4_race_timing": "The `exec_atomic_deploy` endpoint acquires a process-wide `tokio::sync::Mutex` (A1, A2) at the very beginning of the deploy operation and holds it until the entire kill+swap+verify+sentinel-clear sequence is complete. This eliminates any race window between the kill and swap operations, as the server guarantees atomicity. The `rc-watchdog` queries `/deploy_state` (A5), which synchronously checks the mutex state, providing a definitive 'deploy in progress' status, removing any timing analysis dependency.",
    "FL-CONV-5_sc_start_fail": "The new `scripts/deploy-watchdog.sh` (A7) includes a post-start health check. After attempting `sc start RCWatchdog`, it will poll `sc query RCWatchdog` for up to 30 seconds, verifying the service transitions to 'RUNNING'. If it fails to reach 'RUNNING' within the timeout, the script will exit with an error, preventing silent failures. Additionally, documentation will be updated to recommend configuring Windows Service Recovery settings for `RCWatchdog` (e.g., restart on first/second failure)."
  },
  "loc_summary": {
    "prod": 710,
    "tests": 250,
    "total": 960,
    "pr_shape": "large"
  },
  "minority_dissent": "The `loc_estimate` for this plan is significantly higher than the 'gemini-original estimate' of ~200 LOC code + ~150 LOC tests. While the architectural change is substantial and addresses critical flaws, the current estimate of 710 LOC for production code and 250 LOC for tests pushes it into the 'large' PR category. This might introduce higher review burden and integration risk than initially anticipated. The increase is primarily due to the new `atomic_deploy.rs` module, the `deploy_state.rs` module, and the extensive modifications to `rc-watchdog`'s core logic. It's a necessary complexity given the scope, but should be acknowledged as a deviation from the 'smallest sustainable change' ideal."
}
```