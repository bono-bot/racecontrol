```json
{
  "findings": [
    {
      "id": "F-1",
      "category": "design",
      "severity": "P0",
      "title": "Non-atomic deployment sequence vulnerable to watchdog race condition",
      "evidence": "Issue 6: deploy-pod.sh performs kill/copy/move in 3 separate /exec calls. Watchdog polls every 5-10s and can respawn old binary between steps. CLAUDE.md prescribes single /exec atomic chain.",
      "root_cause": "Multi-roundtrip deployment allows watchdog to intervene between steps, violating atomicity. The system lacks a coordinated protocol to prevent concurrent health checks during deployment.",
      "structural_fix": "Implement atomic deployment chain in single /exec call: kill → copy → move → start → verify. Add OTA_DEPLOYING sentinel file BEFORE any changes and clear AFTER verification. Ensure watchdog's health_poller respects OTA_DEPLOYING to suppress rollback during deployment.",
      "verify": "Deploy to test pod with watchdog polling enabled. Confirm binary swap completes in <5s and watchdog does not respawn old binary. Check OTA_DEPLOYING sentinel is set during deployment and cleared post-success.",
      "novel": true
    },
    {
      "id": "F-2",
      "category": "design",
      "severity": "P1",
      "title": "Sentinel file discipline gaps allow rollback during deployment",
      "evidence": "Issue 9: Manual atomic-chain swap failed on Pod 1 because perform_rollback() triggered despite deployment. OTA_DEPLOYING sentinel was omitted. Pod 8 worked when sentinel was set and held for 15s+.",
      "root_cause": "No enforced protocol for sentinel file lifecycle. rc-watchdog's perform_rollback() checks OTA_DEPLOYING but deploy scripts do not consistently set/clear it. Race between sentinel clearance and watchdog health polling.",
      "structural_fix": "Add sentinel discipline to deploy-pod.sh: set OTA_DEPLOYING before any changes, hold for duration of deployment (e.g., 30s), clear only after successful verification. Add watchdog health polling delay after sentinel clearance to avoid premature rollback.",
      "verify": "Deploy with OTA_DEPLOYING set. Confirm watchdog does not rollback during deployment. Verify sentinel is cleared only after health checks pass.",
      "novel": true
    },
    {
      "id": "F-3",
      "category": "design",
      "severity": "P1",
      "title": "BLOCKED_PATTERNS in rc-sentry too restrictive and inconsistent",
      "evidence": "Issue 3: SHA filter `| findstr /v hash | findstr /v Cert` contains pipe-space `| ` which is in BLOCKED_PATTERNS. Issue 2: JSON escaping inconsistencies due to shell-layer escaping. Issue 4: Misdiagnosis of root cause without checking BLOCKED_PATTERNS.",
      "root_cause": "BLOCKED_PATTERNS list is overly broad and not aligned with legitimate Windows shell commands. No mechanism to whitelist safe patterns. JSON escaping is delegated to multiple layers (heredoc, printf, Python) without standardization.",
      "structural_fix": "Refine BLOCKED_PATTERNS to exclude safe Windows shell patterns (e.g., `| findstr`, `&&`, `||` when used in controlled contexts). Add jq-based JSON encoding for /exec calls. Document allowed patterns in rc-sentry/src/main.rs and add unit tests for /exec payload validation.",
      "verify": "Deploy with SHA filter containing `| findstr`. Confirm /exec succeeds. Add unit test for /exec payload validation with jq-encoded strings. Verify no false positives in BLOCKED_PATTERNS.",
      "novel": true
    },
    {
      "id": "F-4",
      "category": "process",
      "severity": "P1",
      "title": "Missing preflight and dry-run validation for deploy scripts",
      "evidence": "Issue 5: No dry-test of patched script on single target before fleet deployment. Burned 7 pods cycling same failure. Issue 11: Misinterpreted Pod 5 outage timing due to single-source observability.",
      "root_cause": "No formal preflight or dry-run process for deploy scripts. Deployments are validated only in production, leading to cascading failures. Observability relies on single source (last_seen) without cross-referencing.",
      "structural_fix": "Implement preflight.sh: validate script syntax, test SHA filter, verify /exec payloads, check sentinel discipline, and simulate deployment on single pod. Add dry-run mode to deploy-pod.sh (--dry-run) that skips actual deployment but validates all steps. Enhance observability with multiple sources (pod heartbeats, watchdog logs, sentinel files).",
      "verify": "Run preflight.sh on all deploy scripts. Deploy to test pod in dry-run mode. Confirm all validations pass. Monitor Pod 5 outage with multiple sources (heartbeat, watchdog logs, network pings).",
      "novel": true
    },
    {
      "id": "F-5",
      "category": "design",
      "severity": "P1",
      "title": "HTTP staging server silent failure due to port binding",
      "evidence": "Issue 1: Orphan HTTP server PID 53024 from prior session served from wrong directory. Deploy-pod.sh's python3 -m http.server failed to bind port but proceeded silently. Pods downloaded stale binary.",
      "root_cause": "No health check or port binding validation for HTTP staging server. Server startup is fire-and-forget with no confirmation of readiness. No retry logic for failed binds.",
      "structural_fix": "Add health check endpoint to HTTP staging server (e.g., /health). Modify deploy-pod.sh to validate server readiness before download. Implement retry logic with exponential backoff for failed binds. Add server PID tracking and cleanup on restart.",
      "verify": "Kill HTTP server and restart. Confirm deploy-pod.sh fails to download and retries. Verify server PID is cleaned up and restarted correctly.",
      "novel": true
    },
    {
      "id": "F-6",
      "category": "design",
      "severity": "P1",
      "title": "Orphan process accumulation due to silent crashes",
      "evidence": "Issue 7: Silent crashes (uv_spawn EPERM) leave 0-byte output but bg harness reports exit 0. Multiple bash/python orphans accumulate. Issue 10: Multiple orphans contribute to EPERM errors.",
      "root_cause": "No process lifecycle management for background tasks. Subshells and orphaned processes are not reaped. No error handling for EPERM or other spawn failures. Harness assumes success if exit code is 0, even with no output.",
      "structural_fix": "Implement process reaping in rc-sentry and deploy scripts. Add error handling for spawn failures (EPERM, EACCES). Use job objects or process groups to manage background tasks. Enhance harness to validate output size and stderr logs. Add timeout for background tasks.",
      "verify": "Spawn multiple background tasks with intentional errors. Confirm harness detects failures and reaps processes. Verify no orphan processes accumulate over multiple deployments.",
      "novel": true
    },
    {
      "id": "F-7",
      "category": "design",
      "severity": "P2",
      "title": "Modal dialog blocking shell input on Pod 5",
      "evidence": "Issue 8: Pod 5 modal 'select app to open .dll' dialog blocked Windows shell input for ~4h, causing LAN unresponsiveness.",
      "root_cause": "No mechanism to prevent or recover from modal dialogs. Windows shell is unresponsive during modal state. No health check detects GUI interaction requirements.",
      "structural_fix": "Add GUI automation to detect and dismiss modal dialogs (e.g., AutoIt or PowerShell scripts). Integrate with watchdog to restart rc-agent if GUI interaction is detected. Add health check for shell responsiveness (e.g., echo test).",
      "verify": "Simulate modal dialog on test pod. Confirm watchdog detects and dismisses dialog. Verify rc-agent remains responsive.",
      "novel": true
    },
    {
      "id": "F-8",
      "category": "observability",
      "severity": "P1",
      "title": "Insufficient cross-system observability for deployment state",
      "evidence": "Issue 11: Misinterpreted Pod 5 outage timing due to single-source observability (last_seen). Issue 9: Manual atomic-chain swap failure not detected until post-deployment verification.",
      "root_cause": "Observability relies on single source (last_seen) without cross-referencing watchdog logs, sentinel files, or rc-agent health. No real-time deployment state dashboard. No alerts for sentinel file presence or watchdog rollback triggers.",
      "structural_fix": "Add deployment state tracking: OTA_DEPLOYING sentinel, watchdog rollback triggers, rc-agent health, and binary version. Implement real-time dashboard with alerts for abnormal states (e.g., OTA_DEPLOYING >30s, watchdog rollback). Integrate with racecontrol.exe for fleet-wide state.",
      "verify": "Deploy to test pod. Confirm dashboard shows OTA_DEPLOYING during deployment and clears post-success. Set up alert for OTA_DEPLOYING >30s and verify alert triggers.",
      "novel": true
    },
    {
      "id": "F-9",
      "category": "design",
      "severity": "P1",
      "title": "Non-idempotent binary swap and lack of manifest trust",
      "evidence": "Issue 1: Stale binary downloaded due to wrong staging directory. Issue 6: Old binary respawned during swap. No manifest or version file to verify binary integrity.",
      "root_cause": "No manifest or version file to validate binary authenticity and version. Binary swap is not idempotent (rc-agent-prev.exe may not exist). No checksum or signature validation beyond SHA256.",
      "structural_fix": "Add manifest file (e.g., C:\RacingPoint\rc-agent.manifest) with build_id, SHA256, and version. Validate manifest before swap. Ensure rc-agent-prev.exe exists before swap (create if missing). Add signature validation (e.g., Authenticode) for binaries.",
      "verify": "Deploy with manifest file. Confirm SHA256 and build_id match expected. Verify rc-agent-prev.exe exists and swap is idempotent. Test signature validation.",
      "novel": true
    },
    {
      "id": "F-10",
      "category": "coordination",
      "severity": "P1",
      "title": "Missing bilateral consistency protocol between rc-agent and rc-watchdog",
      "evidence": "Issue 6: Watchdog respawns old binary during deployment. Issue 9: Watchdog rolls back despite deployment. No protocol to coordinate state between rc-agent and rc-watchdog during deployment.",
      "root_cause": "No bilateral protocol to signal deployment in progress or suppress health checks. rc-watchdog acts independently without awareness of deployment state. No heartbeat or state synchronization.",
      "structural_fix": "Implement bilateral protocol: rc-agent sets OTA_DEPLOYING sentinel during deployment and clears it post-success. rc-watchdog respects OTA_DEPLOYING to suppress rollback and health checks. Add heartbeat synchronization between rc-agent and rc-watchdog.",
      "verify": "Deploy with bilateral protocol enabled. Confirm watchdog does not rollback during deployment. Verify heartbeat synchronization.",
      "novel": true
    }
  ],
  "missed_in_session_rca": [
    "F-1: Non-atomic deployment sequence vulnerable to watchdog race condition",
    "F-2: Sentinel file discipline gaps allow rollback during deployment",
    "F-3: BLOCKED_PATTERNS in rc-sentry too restrictive and inconsistent",
    "F-4: Missing preflight and dry-run validation for deploy scripts",
    "F-5: HTTP staging server silent failure due to port binding",
    "F-6: Orphan process accumulation due to silent crashes",
    "F-7: Modal dialog blocking shell input on Pod 5",
    "F-8: Insufficient cross-system observability for deployment state",
    "F-9: Non-idempotent binary swap and lack of manifest trust",
    "F-10: Missing bilateral consistency protocol between rc-agent and rc-watchdog"
  ],
  "recommended_priority_order": [
    "F-1",
    "F-2",
    "F-3",
    "F-4",
    "F-5",
    "F-8",
    "F-9",
    "F-10",
    "F-6",
    "F-7"
  ]
}
```