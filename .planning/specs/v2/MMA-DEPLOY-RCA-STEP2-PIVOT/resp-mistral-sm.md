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
    {"file": "crates/rc-sentry/src/atomic_deploy.rs", "kind": "new", "loc_estimate": 180},
    {"file": "crates/rc-watchdog/src/service.rs", "kind": "edit", "loc_estimate": 80},
    {"file": "crates/rc-watchdog/src/health.rs", "kind": "new", "loc_estimate": 60},
    {"file": "crates/rc-watchdog/src/rollback_manager.rs", "kind": "edit", "loc_estimate": 90},
    {"file": "scripts/deploy-watchdog.sh", "kind": "new", "loc_estimate": 50},
    {"file": "scripts/deploy-pod.sh", "kind": "edit", "loc_estimate": 40},
    {"file": "tests/atomic_deploy_integration.rs", "kind": "new", "loc_estimate": 150}
  ],
  "actions": [
    {
      "id": "A1",
      "file": "crates/rc-sentry/src/main.rs",
      "kind": "edit",
      "summary": "Add POST /exec_atomic_deploy endpoint to rc-sentry",
      "loc_estimate": 120,
      "risk": "medium",
      "risk_reason": "New endpoint with mutex management and complex state transitions",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-3", "FL-CONV-4"]
    },
    {
      "id": "A2",
      "file": "crates/rc-sentry/src/atomic_deploy.rs",
      "kind": "new",
      "summary": "Implement atomic deploy core logic with mutex and sentinel management",
      "loc_estimate": 180,
      "risk": "high",
      "risk_reason": "Core atomicity guarantee implementation with multiple failure modes",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-3", "FL-CONV-4"]
    },
    {
      "id": "A3",
      "file": "crates/rc-watchdog/src/service.rs",
      "kind": "edit",
      "summary": "Update watchdog to query deploy state and adjust behavior",
      "loc_estimate": 80,
      "risk": "low",
      "risk_reason": "Read-only changes to existing health check logic",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-9"]
    },
    {
      "id": "A4",
      "file": "crates/rc-watchdog/src/health.rs",
      "kind": "new",
      "summary": "Add /health endpoint exposing deploy state and startup phase",
      "loc_estimate": 60,
      "risk": "low",
      "risk_reason": "New read-only endpoint with simple state exposure",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-9"]
    },
    {
      "id": "A5",
      "file": "crates/rc-watchdog/src/rollback_manager.rs",
      "kind": "edit",
      "summary": "Update rollback logic to handle deploy-aware mode and sentinel parsing",
      "loc_estimate": 90,
      "risk": "medium",
      "risk_reason": "Changes to core rollback decision logic with new failure modes",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-3", "FL-CONV-9"]
    },
    {
      "id": "A6",
      "file": "scripts/deploy-watchdog.sh",
      "kind": "new",
      "summary": "Add deployment script for rc-watchdog updates",
      "loc_estimate": 50,
      "risk": "low",
      "risk_reason": "New deployment script following existing patterns",
      "addresses_flaw": ["FL-CONV-2"]
    },
    {
      "id": "A7",
      "file": "scripts/deploy-pod.sh",
      "kind": "edit",
      "summary": "Update to use /exec_atomic_deploy endpoint",
      "loc_estimate": 40,
      "risk": "low",
      "risk_reason": "Simple endpoint substitution with same parameters",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "A8",
      "file": "tests/atomic_deploy_integration.rs",
      "kind": "new",
      "summary": "Add integration tests for atomic deploy endpoint",
      "loc_estimate": 150,
      "risk": "low",
      "risk_reason": "Test-only changes with no production impact",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-2", "FL-CONV-3", "FL-CONV-4", "FL-CONV-5"]
    }
  ],
  "test_plan": [
    {
      "id": "T1",
      "kind": "unit",
      "what": "Test atomic deploy mutex contention handling",
      "expected": "Second request fails immediately with deploy_in_progress error",
      "exercises_flaw": ["FL-CONV-4"]
    },
    {
      "id": "T2",
      "kind": "integration",
      "what": "Test race condition between deploy and watchdog poll",
      "expected": "Watchdog correctly detects deploy in progress and extends interval",
      "exercises_flaw": ["FL-CONV-1", "FL-CONV-2"]
    },
    {
      "id": "T3",
      "kind": "live-pod",
      "what": "Test JSON parse failure handling during deploy",
      "expected": "Endpoint falls back to mtime check and completes deploy",
      "exercises_flaw": ["FL-CONV-3"]
    },
    {
      "id": "T4",
      "kind": "live-pod",
      "what": "Test backward compatibility with old watchdog on Pod 8",
      "expected": "Deploy succeeds with new endpoint, old watchdog skips rollback",
      "exercises_flaw": ["FL-CONV-2"]
    },
    {
      "id": "T5",
      "kind": "unit",
      "what": "Test sc-start failure handling in deploy-watchdog.sh",
      "expected": "Script fails fast with clear error message",
      "exercises_flaw": ["FL-CONV-5"]
    }
  ],
  "rollout_plan": {
    "phase_1": {
      "description": "Deploy rc-watchdog to all reachable pods (1-4,6-8) using deploy-watchdog.sh",
      "criteria": "All pods report RUNNING status via sc query",
      "halt_on_failure": true
    },
    "phase_2": {
      "description": "Update deploy-pod.sh to use /exec_atomic_deploy and canary on Pod 1",
      "criteria": "Pod 1 stable for 5 minutes with new binary",
      "halt_on_failure": true
    },
    "phase_3": {
      "description": "Sequential rollout to remaining pods (2,3,4,6,7) with 10 minute stability checks",
      "criteria": "All pods report healthy with new binary",
      "halt_on_failure": false
    },
    "rollback_plan": {
      "immediate": "Rollback rc-watchdog on affected pods using deploy-watchdog.sh with previous binary",
      "deferred": "If atomic deploy fails mid-operation, endpoint will automatically rollback and clear sentinel",
      "watchdog_fallback": "If new watchdog causes issues, old watchdog binary can be redeployed via phase 1 rollback"
    }
  },
  "captain_q_decisions": [
    {
      "id": "PV-Q1",
      "question": "Should we proceed with the atomic endpoint approach despite its complexity?",
      "default_recommendation": "Proceed with the atomic endpoint approach as it addresses the root cause of the race conditions that blocked the previous plan.",
      "rationale": "The server-side atomicity guarantee eliminates timing dependencies and provides a single point of control for the deploy operation. The complexity is justified by the P0 failures in the previous approach."
    },
    {
      "id": "PV-Q2",
      "question": "Is the 300s TTL for OTA_DEPLOYING sufficient for worst-case deploy times?",
      "default_recommendation": "Use 600s TTL with dynamic adjustment based on binary size and network conditions.",
      "rationale": "The original 300s was based on MAINTENANCE_MODE which may not account for large binary transfers. A longer TTL with automatic extension would be safer."
    }
  ],
  "verify_post_deploy": [
    {
      "step": 1,
      "command": "curl -X POST http://<pod-ip>:8091/exec_atomic_deploy -d '{\"binary_url\":\"...\",\"expected_sha256\":\"...\",\"deploy_id\":\"test123\"}'",
      "pass_criterion": "Returns success with swap_completed_at timestamp"
    },
    {
      "step": 2,
      "command": "curl http://<pod-ip>:8091/deploy_state",
      "pass_criterion": "Returns deploy_in_progress: false and no OTA_DEPLOYING file"
    },
    {
      "step": 3,
      "command": "curl http://<pod-ip>:8091/health",
      "pass_criterion": "Returns startup_phase: false and deploy_in_progress: false"
    }
  ],
  "fl_conv_addressing": {
    "FL-CONV-1_sentinel_before_chain": "The atomic endpoint manages the OTA_DEPLOYING sentinel internally as part of the single atomic operation, eliminating the client-side ordering hazard that caused silent fleet death.",
    "FL-CONV-2_pod_8_old_watchdog": "Rollout plan requires rc-watchdog deployment to all pods BEFORE changing deploy-pod.sh format. The new watchdog implements deploy-aware mode that works with both new and old sentinel formats via mtime fallback.",
    "FL-CONV-3_json_parse_fail": "Endpoint implements explicit policy: log WARNING, fall back to mtime check with 60s grace window, and deny rollback if mtime within window. This matches the proven pattern from auto_clear_maintenance_mode_json.",
    "FL-CONV-4_race_timing": "Server-side mutex in /exec_atomic_deploy holds the deploy lock across the entire swap operation, eliminating all timing dependencies between kill, copy, and move operations.",
    "FL-CONV-5_sc_start_fail": "deploy-watchdog.sh includes post-start health check (sc query RCWatchdog with 30s timeout) and documents Windows Service Recovery settings. Failure triggers immediate rollback to previous binary."
  },
  "loc_summary": {"prod": 490, "tests": 150, "total": 640, "pr_shape": "medium"},
  "minority_dissent": "The atomic endpoint approach introduces a new single point of failure (rc-sentry) that wasn't present in the original design. If rc-sentry crashes during a deploy, the endpoint's rollback mechanism may not complete, leaving the pod in a partially deployed state. While the endpoint includes comprehensive error handling, a crash during the critical section could still cause issues. Consider adding a watchdog-specific health check that monitors the atomic deploy endpoint's state and triggers emergency rollback if the endpoint becomes unresponsive during a deploy."
}
```