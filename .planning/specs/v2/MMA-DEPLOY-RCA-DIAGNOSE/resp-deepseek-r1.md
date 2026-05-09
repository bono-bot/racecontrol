```json
{
  "findings": [
    {
      "id": "F-1",
      "category": "design",
      "severity": "P0",
      "title": "Non-atomic deploy sequence enables watchdog interference",
      "evidence": "Issue 6 (watchdog wins race during multi-step /exec calls), Issue 9 (rollback triggered despite manual atomic chain)",
      "root_cause": "Deploy sequence spans multiple network roundtrips without coordination with watchdog health checks",
      "structural_fix": "Implement single /exec atomic chain per CLAUDE.md: combine kill-swap-start in one script with OTA_DEPLOYING sentinel covering entire operation",
      "verify": "Deploy to test pod while artificially reducing watchdog poll interval; monitor process tree continuity via racecontrol",
      "novel": false
    },
    {
      "id": "F-2",
      "category": "observability",
      "severity": "P1",
      "title": "Silent failure modes in critical paths",
      "evidence": "Issue 1 (HTTP server fail-to-bind swallowed), Issue 3 (403 blocked by rc-sentry not detected), Issue 7 (EPERM reported as success)",
      "root_cause": "Missing error propagation: bg tasks lack status checks, /exec calls don't validate exit codes or output length",
      "structural_fix": "Add: 1) Port conflict check before HTTP server start, 2) HTTP status/exit code validation for all /exec responses, 3) Output length verification in bg task harness",
      "verify": "Inject failures (port conflict, blocked cmd, EPERM) during test deploy; verify script fails fast with actionable errors",
      "novel": false
    },
    {
      "id": "F-3",
      "category": "design",
      "severity": "P0",
      "title": "Inadequate deploy-watchdog coordination protocol",
      "evidence": "Issue 9 (rollback triggered despite successful deploy due to missing sentinel coverage during health checks)",
      "root_cause": "OTA_DEPLOYING sentinel doesn't persist through watchdog health check window (3×10s polls)",
      "structural_fix": "Extend sentinel retention until new agent survives watchdog health checks (min 30s). Add version handshake: watchdog ignores rollback if new build_id matches target",
      "verify": "Deploy test binary with artificial 40s startup delay; verify watchdog doesn't rollback while OTA_DEPLOYING exists",
      "novel": false
    },
    {
      "id": "F-4",
      "category": "process",
      "severity": "P1",
      "title": "Missing preflight validation suite",
      "evidence": "Issue 5 (no dry-run), Issue 1 (wrong binary served), Issue 11 (misdiagnosed pod state)",
      "root_cause": "No systematic checks for: binary integrity pre-deploy, pod readiness state, or environmental conflicts",
      "structural_fix": "Implement pre-deploy checklist: 1) Binary hash validation on staging server, 2) Pod liveness/version baseline, 3) Port/process conflict scan",
      "verify": "Run preflight against misconfigured test environment (wrong binary, occupied port, offline pod); verify abort with diagnostics",
      "novel": false
    },
    {
      "id": "F-5",
      "category": "design",
      "severity": "P1",
      "title": "Unsafe command filtering without override mechanism",
      "evidence": "Issue 3 (blocked SHA256 filter), Issue 4 (no-op patch), rc-sentry/src/main.rs:722 BLOCKED_PATTERNS",
      "root_cause": "Static blocklist prohibits essential admin operations with no trusted-path bypass",
      "structural_fix": "Add signed-command whitelist: allow privileged IPs (.23/.27) to bypass filters when requests contain HMAC-signed commands",
      "verify": "Execute blocked SHA256 verification command from whitelisted IP with valid HMAC; confirm 200 OK",
      "novel": false
    },
    {
      "id": "F-6",
      "category": "design",
      "severity": "P1",
      "title": "Resource leakage from unmanaged bg processes",
      "evidence": "Issue 7 (EPERM crashes), Issue 10 (orphan accumulation), Issue 1 (stale HTTP server)",
      "root_cause": "No process lifecycle tracking: bg tasks lack reaping mechanism and PID tracking",
      "structural_fix": "Implement process supervisor: track PIDs of spawned tasks with SIGCHLD-like handler. Add resource caps (max bg procs per host)",
      "verify": "Run 20 concurrent bg tasks; verify no orphans remain after completion and EPERM errors surface properly",
      "novel": false
    },
    {
      "id": "F-7",
      "category": "design",
      "severity": "P2",
      "title": "Vulnerable binary distribution mechanism",
      "evidence": "Issue 1 (stale binary served over HTTP), scripts/deploy-pod.sh step 1 (unauthenticated download)",
      "root_cause": "Lack of cryptographic chain-of-trust from build to execution",
      "structural_fix": "Replace HTTP with HTTPS + code signing: 1) Serve binaries via TLS, 2) Verify signature with embedded root cert before execution",
      "verify": "Attempt deploy with tampered binary; verify pod rejects invalid signature before swap",
      "novel": true
    },
    {
      "id": "F-8",
      "category": "observability",
      "severity": "P1",
      "title": "Insufficient deploy state tracking",
      "evidence": "Issue 11 (misdiagnosed outage timing), Issue 9 (uncertain sentinel state)",
      "root_cause": "No centralized ledger of: sentinel timestamps, binary versions per pod, or watchdog interventions",
      "structural_fix": "Add state API to rc-sentry: expose active sentinels/last deploy. Log watchdog actions to central telemetry",
      "verify": "Query state API during deploy; verify sentinel timestamps match operations. Check telemetry for watchdog rollback events",
      "novel": true
    }
  ],
  "missed_in_session_rca": [
    "Binary trust gap (HTTP without signing)",
    "State tracking gap (no sentinel/version API)",
    "Rollback safety (single backup version)"
  ],
  "recommended_priority_order": ["F-1", "F-3", "F-5", "F-2", "F-4", "F-6", "F-8", "F-7"]
}
```