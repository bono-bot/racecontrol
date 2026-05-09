```json
{
  "findings": [
    {
      "id": "F-1",
      "category": "design",
      "severity": "P0",
      "title": "Non-atomic kill+swap sequence allows race with rc-watchdog respawn",
      "evidence": "Issue #6, deploy-pod.sh steps 5-8 use 3 separate /exec calls; rc-watchdog/src/service.rs polls every 5-10s",
      "root_cause": "Kill and binary swap are decoupled across multiple HTTP roundtrips, allowing rc-watchdog to respawn old binary before new one is swapped in",
      "structural_fix": "Combine kill+swap+start into single atomic /exec call with proper sentinel coordination",
      "verify": "Deploy to one pod with new atomic script; confirm no rollback triggered and binary matches expected SHA",
      "novel": false
    },
    {
      "id": "F-2",
      "category": "process",
      "severity": "P0",
      "title": "Missing preflight validation on staging server setup",
      "evidence": "Issue #1 - orphan HTTP server served wrong directory; deploy proceeded silently",
      "root_cause": "No verification that staging server is correctly bound to expected port and serving correct content before initiating fleet deploy",
      "structural_fix": "Add mandatory staging server health check to deploy orchestration that validates port binding and file content hashes",
      "verify": "Run deploy with intentionally misconfigured staging server; verify preflight check blocks deployment",
      "novel": false
    },
    {
      "id": "F-3",
      "category": "design",
      "severity": "P0",
      "title": "Sentinel file coordination missing during atomic deploy",
      "evidence": "Issue #9 - perform_rollback() triggered during manual atomic deploy because OTA_DEPLOYING was not set",
      "root_cause": "Atomic deploy scripts don't properly coordinate with watchdog's rollback suppression mechanism",
      "structural_fix": "Mandate OTA_DEPLOYING sentinel creation/deletion as part of any atomic deploy sequence",
      "verify": "Deploy with sentinel coordination; verify watchdog doesn't trigger rollback during deploy",
      "novel": false
    },
    {
      "id": "F-4",
      "category": "observability",
      "severity": "P1",
      "title": "Silent failures in background task execution",
      "evidence": "Issue #7 - bg task EPERM crashes leave 0 byte output but report 'completed exit 0'",
      "root_cause": "Background task execution doesn't properly capture and report spawn failures, leading to false success reports",
      "structural_fix": "Implement proper error propagation in background task execution with explicit failure detection",
      "verify": "Intentionally cause bg task spawn failure; verify error is properly reported rather than masked as success",
      "novel": false
    },
    {
      "id": "F-5",
      "category": "design",
      "severity": "P1",
      "title": "SHA256 verification command contains blocked patterns",
      "evidence": "Issue #3 - '| findstr' pattern blocked by rc-sentry BLOCKED_PATTERNS; Issue #4 - misdiagnosed root cause",
      "root_cause": "Verification commands use shell patterns that trigger security filters, causing silent 403 failures",
      "structural_fix": "Refactor SHA verification to use approved command patterns or whitelist legitimate verification commands",
      "verify": "Run SHA verification command through /exec; confirm it executes without 403 response",
      "novel": false
    },
    {
      "id": "F-6",
      "category": "process",
      "severity": "P0",
      "title": "No single-pod dry-run validation before fleet deployment",
      "evidence": "Issue #5 - burned 7 pods cycling same failure; no validation on single target first",
      "root_cause": "Deployment process lacks mandatory single-target validation step before fleet rollout",
      "structural_fix": "Implement mandatory single-pod dry-run phase with full validation before fleet deployment",
      "verify": "Attempt fleet deploy without single-pod validation; verify process is blocked",
      "novel": false
    },
    {
      "id": "F-7",
      "category": "design",
      "severity": "P1",
      "title": "JSON escaping inconsistencies cause command corruption",
      "evidence": "Issue #2 - \\R invalid JSON escape; different encoding methods produce different bytes",
      "root_cause": "Multiple encoding layers (heredoc/printf/Python json.dump) create inconsistent escaping behavior",
      "structural_fix": "Standardize on single reliable JSON encoder (jq) for all command encoding",
      "verify": "Send complex command with special characters through /exec; verify exact command received matches intent",
      "novel": false
    },
    {
      "id": "F-8",
      "category": "environment",
      "severity": "P1",
      "title": "Orphan processes accumulate and cause handle exhaustion",
      "evidence": "Issue #7, #10 - bash/python orphans accumulate; handle exhaustion contributes to failures",
      "root_cause": "Background processes not properly cleaned up; no resource leak detection/prevention",
      "structural_fix": "Implement process lifecycle management with automatic cleanup and resource leak detection",
      "verify": "Monitor process table during deployment; confirm no orphan accumulation",
      "novel": false
    },
    {
      "id": "F-9",
      "category": "observability",
      "severity": "P1",
      "title": "Timing misattribution obscures true failure causes",
      "evidence": "Issue #11 - Pod 5 outage timing misinterpreted as during deploy but actually 4h gap",
      "root_cause": "Lack of cross-system timeline correlation makes it impossible to establish true causality",
      "structural_fix": "Implement unified timestamp logging and cross-system correlation dashboard",
      "verify": "Correlate events across systems using unified timestamps; confirm accurate timeline reconstruction",
      "novel": true
    },
    {
      "id": "F-10",
      "category": "design",
      "severity": "P1",
      "title": "Missing manifest-trust verification between staging and deployment",
      "evidence": "Issue #1 - wrong binary served but SHA caught it; Issue #2 - JSON escaping could corrupt binaries",
      "root_cause": "No end-to-end content integrity verification from staging server to pod deployment",
      "structural_fix": "Implement manifest-based trust chain with cryptographic signatures from build to deployment",
      "verify": "Attempt to deploy tampered binary; verify manifest validation blocks deployment",
      "novel": true
    },
    {
      "id": "F-11",
      "category": "design",
      "severity": "P2",
      "title": "Modal UI dialogs can block pod availability",
      "evidence": "Issue #8 - 'select app to open .dll' dialog blocked Windows shell input for ~4h",
      "root_cause": "Windows UI interactions can interfere with headless deployment operations without detection",
      "structural_fix": "Implement headless mode enforcement and UI interaction detection/alerting mechanism",
      "verify": "Trigger modal dialog condition; verify system alerts rather than silently blocking",
      "novel": true
    },
    {
      "id": "F-12",
      "category": "design",
      "severity": "P1",
      "title": "Health check polling window creates false positive restart detection",
      "evidence": "Issue #9 - health_poll fails 2+ times triggering rollback even during legitimate deploy",
      "root_cause": "Fixed polling interval creates race window where legitimate restarts appear as failures",
      "structural_fix": "Implement deploy-aware health check suppression with explicit coordination protocol",
      "verify": "Deploy with health check coordination; confirm no false positive rollback triggers",
      "novel": true
    }
  ],
  "missed_in_session_rca": [
    "F-9",
    "F-10",
    "F-11",
    "F-12"
  ],
  "recommended_priority_order": [
    "F-1",
    "F-2", 
    "F-6",
    "F-3",
    "F-5",
    "F-7",
    "F-12",
    "F-4",
    "F-8",
    "F-10",
    "F-9",
    "F-11"
  ]
}
```