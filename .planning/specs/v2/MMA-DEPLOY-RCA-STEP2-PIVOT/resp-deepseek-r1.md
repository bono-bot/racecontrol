```json
{
  "pr_title": "fix(rc-sentry/rc-watchdog/deploy-pod): atomic deploy endpoint + watchdog deploy-aware mode (CF-1+CF-2+CF-9 bundle)",
  "selected_approach": {
    "cf1_atomicity": "server_side_mutex_in_exec_atomic_deploy",
    "cf2_sentinel": "internal_lifecycle_in_endpoint_with_ttl_json_and_mtime_fallback",
    "cf9_watchdog_aware": "deploy_state_file_check_with_extended_poll_interval"
  },
  "files_touched": [
    {"file": "crates/rc-sentry/src/main.rs", "kind": "edit", "loc_estimate": 120},
    {"file": "crates/rc-watchdog/src/rollback_manager.rs", "kind": "edit", "loc_estimate": 70},
    {"file": "crates/rc-watchdog/src/service.rs", "kind": "edit", "loc_estimate": 30},
    {"file": "crates/rc-agent/src/http_handlers/health.rs", "kind": "edit", "loc_estimate": 15},
    {"file": "scripts/deploy-pod.sh", "kind": "edit", "loc_estimate": 25},
    {"file": "scripts/deploy-watchdog.sh", "kind": "new", "loc_estimate": 40},
    {"file": "tests/e2e/atomic_deploy.rs", "kind": "new", "loc_estimate": 90}
  ],
  "actions": [
    {
      "id": "A1",
      "file": "crates/rc-sentry/src/main.rs",
      "kind": "edit",
      "summary": "Implement POST /exec_atomic_deploy endpoint with mutex, sentinel lifecycle, and atomic swap",
      "loc_estimate": 120,
      "risk": "medium",
      "risk_reason": "Concurrent access handling and filesystem rollback logic",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-3", "FL-CONV-4"]
    },
    {
      "id": "A2",
      "file": "crates/rc-watchdog/src/rollback_manager.rs",
      "kind": "edit",
      "summary": "Add auto_clear_ota_deploying_json() and is_deploy_in_progress() with TTL/mtime fallback",
      "loc_estimate": 40,
      "risk": "low",
      "risk_reason": "Mirrors proven maintenance_mode pattern (BUG-71)",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-3"]
    },
    {
      "id": "A3",
      "file": "crates/rc-watchdog/src/service.rs",
      "kind": "edit",
      "summary": "Integrate deploy state checks in poll loop (line 234-559), extend interval to 30s during deploy",
      "loc_estimate": 30,
      "risk": "medium",
      "risk_reason": "Polling logic modification in live system",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-9"]
    },
    {
      "id": "A4",
      "file": "crates/rc-agent/src/http_handlers/health.rs",
      "kind": "edit",
      "summary": "Add startup_phase/graceful_shutdown_in_progress fields to /health response",
      "loc_estimate": 15,
      "risk": "low",
      "risk_reason": "Additive API change, backward compatible",
      "addresses_flaw": ["CF-9"]
    },
    {
      "id": "A5",
      "file": "scripts/deploy-watchdog.sh",
      "kind": "new",
      "summary": "Create watchdog deploy script mirroring deploy-sentry.sh pattern",
      "loc_estimate": 40,
      "risk": "low",
      "risk_reason": "Derivative work from existing deploy patterns",
      "addresses_flaw": ["FL-CONV-2", "FL-CONV-5"]
    },
    {
      "id": "A6",
      "file": "scripts/deploy-pod.sh",
      "kind": "edit",
      "summary": "Replace multi-step /exec calls with single /exec_atomic_deploy (line 138+)",
      "loc_estimate": 25,
      "risk": "high",
      "risk_reason": "Core deploy mechanism change",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "A7",
      "file": "tests/e2e/atomic_deploy.rs",
      "kind": "new",
      "summary": "End-to-end tests: race conditions, JSON corruption, backward compat",
      "loc_estimate": 90,
      "risk": "low",
      "risk_reason": "Test-only changes",
      "addresses_flaw": ["FL-CONV-1", "FL-CONV-2", "FL-CONV-3"]
    }
  ],
  "test_plan": [
    {
      "id": "T1",
      "kind": "integration",
      "what": "Inject 20s delay during swap + watchdog poll",
      "expected": "Watchdog skips rollback, extends poll interval to 30s",
      "exercises_flaw": ["FL-CONV-1", "FL-CONV-4"]
    },
    {
      "id": "T2",
      "kind": "unit",
      "what": "Corrupt OTA_DEPLOYING.json with invalid content",
      "expected": "auto_clear falls back to mtime check, clears if >60s old",
      "exercises_flaw": ["FL-CONV-3"]
    },
    {
      "id": "T3",
      "kind": "live-pod",
      "what": "New deploy-pod.sh + old watchdog (Pod 8 config)",
      "expected": "Old watchdog ignores JSON (bare is_file() true), suppresses rollback",
      "exercises_flaw": ["FL-CONV-2"]
    },
    {
      "id": "T4",
      "kind": "integration",
      "what": "Simulate sc start failure during watchdog deploy",
      "expected": "deploy-watchdog.sh retries 3x with 10s delays, fails after timeout",
      "exercises_flaw": ["FL-CONV-5"]
    }
  ],
  "rollout_plan": {
    "phase_1": "Deploy new rc-watchdog to Pods 1-4,6-8 via deploy-watchdog.sh. Verify with 'sc query RCWatchdog' and 30s health check timeout. HALT on any failure.",
    "phase_2": "Update deploy-pod.sh to use /exec_atomic_deploy. Canary test on Pod 1 (new watchdog) with 5min soak.",
    "phase_3": "Sequential rollout to Pods 2,3,4,6,7. Pod 8 last (retain PR #66 binary until final phase).",
    "rollback_plan": "Revert to previous deploy-pod.sh version + manual watchdog rollback via 'sc stop' + binary replacement"
  },
  "captain_q_decisions": [
    {
      "id": "PV-Q1",
      "question": "Should watchdog use HTTP endpoint instead of file for deploy state?",
      "default_recommendation": "No - file-based avoids HTTP dependency during rc-agent downtime",
      "rationale": "rc-sentry may be busy holding mutex; file I/O is synchronous and matches maintenance_mode pattern"
    }
  ],
  "verify_post_deploy": [
    {
      "step": 1,
      "command": "curl -sSf podX:8091/deploy_state | jq .in_progress",
      "pass_criterion": "Returns false during non-deploy periods"
    },
    {
      "step": 2,
      "command": "stat -c %Y OTA_DEPLOYING.json",
      "pass_criterion": "File mtime <60s during active deploys"
    }
  ],
  "fl_conv_addressing": {
    "FL-CONV-1_sentinel_before_chain": "Sentinel write/clear encapsulated in atomic endpoint - no client-side ordering gap",
    "FL-CONV-2_pod_8_old_watchdog": "Watchdog deployed first; old watchdog sees JSON as regular file (suppresses rollback identically to legacy behavior)",
    "FL-CONV-3_json_parse_fail": "Fallback to mtime check with 60s grace window + auto-clear",
    "FL-CONV-4_race_timing": "Server mutex guarantees swap atomicity; watchdog checks are synchronous to filesystem state",
    "FL-CONV-5_sc_start_fail": "deploy-watchdog.sh includes retry loop + 'sc query' verification with 30s timeout"
  },
  "loc_summary": {"prod": 340, "tests": 90, "total": 430, "pr_shape": "medium"},
  "minority_dissent": "Potential mutex deadlock if endpoint crashes mid-deploy. Mitigation: mutex uses timeout (suggest 300s) and watchdog fallback to mtime clears stale locks."
}
```