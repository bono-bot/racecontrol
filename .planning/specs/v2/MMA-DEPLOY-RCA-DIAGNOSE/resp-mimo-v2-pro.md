```json
{
  "findings": [
    {
      "id": "F-1",
      "category": "design",
      "severity": "P0",
      "title": "Non-atomic deploy sequence creates race condition with watchdog",
      "evidence": "Issues 6, 9; deploy-pod.sh steps 5-10 are separate /exec calls; watchdog polls every 5-10s and respawns old binary between kill and swap; CLAUDE.md prescribes single /exec atomic chain but script violates it",
      "root_cause": "Multi-step HTTP exec calls allow watchdog to observe missing rc-agent and respawn old binary before swap completes; no inter-process coordination mechanism exists",
      "structural_fix": "Rewrite deploy-pod.sh to use single /exec call with atomic chain: `set sentinel → kill → copy → move → start → verify → clear sentinel` in one command; add watchdog-aware deploy mode that temporarily extends POLL_INTERVAL or uses a shared mutex",
      "verify": "Simulate watchdog with 1s polling during deploy; confirm old binary never respawns; measure deploy success rate across 100 test deployments",
      "novel": false
    },
    {
      "id": "F-2",
      "category": "design",
      "severity": "P0",
      "title": "Sentinel discipline gap allows rollback during deployment",
      "evidence": "Issue 9; watchdog's perform_rollback() suppressed only if OTA_DEPLOYING exists; deploy script omitted sentinel in atomic chain; Pod 8 succeeded because sentinel was set 15s+ before clear",
      "root_cause": "Rollback suppression depends on ephemeral file presence without lease/timeout mechanism; deploy script can clear sentinel before watchdog's health check completes",
      "structural_fix": "Implement sentinel with TTL (e.g., `OTA_DEPLOYING:expires=1715270400`); watchdog checks both existence AND expiration; add `DEPLOY_LOCK` registry key with mutex for cross-process coordination",
      "verify": "Kill watchdog during deploy with sentinel set; confirm no rollback occurs; test sentinel expiration after 60s triggers rollback as expected",
      "novel": false
    },
    {
      "id": "F-3",
      "category": "observability",
      "severity": "P1",
      "title": "Silent failure of HTTP server binding masks wrong binary serving",
      "evidence": "Issue 1; python3 http.server failed to bind (port taken) but proceeded; pods downloaded stale 12.4MB binary; only SHA verify caught it",
      "root_cause": "No preflight check that staging server is running from correct directory; background process errors are swallowed; no manifest/hash verification before download",
      "structural_fix": "Add preflight: `curl -s http://192.168.31.27:18889/manifest.json | jq -e '.binary_hash == \"780d8b1a234f...\"'`; implement health endpoint on staging server that reports serving directory and binary hash; add deploy script validation step",
      "verify": "Start wrong server, run preflight; confirm script aborts with clear error; test with correct server passes",
      "novel": false
    },
    {
      "id": "F-4",
      "category": "process",
      "severity": "P1",
      "title": "No dry-run/preflight validation before fleet deployment",
      "evidence": "Issue 5; no single-target test after patch; burned 7 pods on same failure; SHA filter patch still contained blocked pattern",
      "root_cause": "Deploy process lacks mandatory canary stage; no automated validation of command syntax against rc-sentry BLOCKED_PATTERNS",
      "structural_fix": "Add `--canary POD` flag to deploy script that runs full sequence on one pod with enhanced logging; create `validate-exec-syntax.sh` that checks commands against BLOCKED_PATTERNS regex before sending; require canary success before fleet rollout",
      "verify": "Introduce syntax error in test command; confirm canary fails and blocks fleet; test valid command passes canary",
      "novel": false
    },
    {
      "id": "F-5",
      "category": "design",
      "severity": "P1",
      "title": "Error-swallowing in exec endpoint and background tasks",
      "evidence": "Issues 3, 7; rc-sentry returns 403 for blocked patterns but deploy script doesn't check HTTP status; bg tasks report exit 0 despite EPERM crashes; 0-byte outputs",
      "root_cause": "No error propagation from rc-sentry to deploy script; background harness doesn't capture stderr; exit code doesn't reflect command success",
      "structural_fix": "Modify rc-sentry to return JSON with `success: false, error: \"blocked pattern\"`; deploy script checks `jq -e '.success'` after each exec; background harness captures stderr and checks output size > 0 for critical commands",
      "verify": "Send blocked pattern command; confirm script aborts with descriptive error; test EPERM scenario returns non-zero exit",
      "novel": false
    },
    {
      "id": "F-6",
      "category": "environment",
      "severity": "P1",
      "title": "Resource exhaustion from orphaned processes degrades stability",
      "evidence": "Issues 7, 10; bash/python orphans accumulate; handle exhaustion causes EPERM; Pod 5 unresponsiveness possibly related",
      "root_cause": "No process cleanup between deploy sessions; background tasks not properly reaped; Windows job objects not used to contain child processes",
      "structural_fix": "Wrap deploy in Windows job object with `TerminateJobObject` on exit; add session start cleanup: `taskkill /F /IM python.exe /FI \"SESSIONNAME eq $SESSION\"`; implement process tree killing for background harness",
      "verify": "Run 10 deploys; check for orphaned processes with `tasklist`; confirm no handle leaks via `handle.exe -p`",
      "novel": false
    },
    {
      "id": "F-7",
      "category": "observability",
      "severity": "P2",
      "title": "No cross-source timing correlation for outage diagnosis",
      "evidence": "Issue 11; misinterpreted Pod 5 outage timing; asserted 'during deploy' but server last_seen showed 4h gap",
      "root_cause": "No centralized logging with timestamps from all components (rc-agent, watchdog, rc-sentry, racecontrol); manual correlation error-prone",
      "structural_fix": "Implement structured logging to central collector (e.g., Windows Event Log → Fluentd → Elasticsearch); add correlation ID to all deploy operations; create dashboard showing pod status timeline with deploy markers",
      "verify": "Simulate outage; confirm logs from all sources show consistent timeline; test correlation ID traces through system",
      "novel": false
    },
    {
      "id": "F-8",
      "category": "design",
      "severity": "P1",
      "title": "Watchdog health polling doesn't account for deployment windows",
      "evidence": "Architecture description; watchdog polls every 5-10s; no awareness of ongoing deployment; can trigger rollback mid-swap even with sentinel if timing aligns",
      "root_cause": "Watchdog operates independently; no protocol to pause health checks during deployment; health_poller runs immediately after spawn without backoff",
      "structural_fix": "Add deployment-aware mode to watchdog: when `OTA_DEPLOYING` exists, increase POLL_INTERVAL to 30s and skip rollback; implement exponential backoff for health checks after restart (10s, 20s, 40s)",
      "verify": "Set sentinel, kill agent; confirm watchdog waits 30s before respawn; test health check backoff sequence",
      "novel": true
    },
    {
      "id": "F-9",
      "category": "coordination",
      "severity": "P1",
      "title": "No bilateral consistency check between control server and pods",
      "evidence": "Issue 1; racecontrol thought deploy was proceeding but pods had wrong binary; no verification that download matched expected hash before proceeding",
      "root_cause": "Deploy script doesn't report download hash back to control server; no two-phase commit protocol for fleet deploys",
      "structural_fix": "Implement pre-commit phase: pods download and report hash to racecontrol; control server waits for all pods to report correct hash before issuing swap commands; add rollback if any pod reports mismatch",
      "verify": "Introduce hash mismatch on one pod; confirm control server aborts deploy for all pods; test successful pre-commit proceeds to swap",
      "novel": true
    },
    {
      "id": "F-10",
      "category": "design",
      "severity": "P2",
      "title": "BLOCKED_PATTERNS interferes with legitimate Windows commands",
      "evidence": "Issue 3; `| findstr` contains pipe-space which is blocked; certutil filter needed but rejected; security vs functionality tradeoff",
      "root_cause": "Overly broad regex patterns; no allowlist for known-safe command patterns; security model is deny-first without exceptions",
      "structural_fix": "Refactor BLOCKED_PATTERNS to use parser instead of regex; implement allowlist for specific commands (e.g., `certutil -hashfile ... | findstr ...`); add per-command risk scoring",
      "verify": "Test 20 legitimate Windows commands; confirm none blocked; test 20 injection attempts all blocked",
      "novel": false
    },
    {
      "id": "F-11",
      "category": "process",
      "severity": "P2",
      "title": "No idempotent deploy operations",
      "evidence": "Architecture; deploy script copies/moves files without checking current state; re-running deploy after partial failure may have unpredictable results",
      "root_cause": "Operations are imperative not declarative; no state reconciliation; no check for existing rc-agent-new.exe before download",
      "structural_fix": "Convert to declarative model: `ensure_binary_hash(path, expected_hash)` downloads only if missing or hash mismatch; add state machine for deploy phases with resume capability",
      "verify": "Kill deploy mid-download; restart script; confirm it resumes from correct phase; test with already-correct binary skips download",
      "novel": true
    },
    {
      "id": "F-12",
      "category": "observability",
      "severity": "P2",
      "title": "No deploy progress tracking or timeout enforcement",
      "evidence": "Issues 4, 8; misdiagnosed root cause due to incomplete information; Pod 5 offline 4h without alerting; no overall deploy timeout",
      "root_cause": "No per-pod deploy state tracking; no heartbeat from deploy script to control server; no timeout for individual steps or overall operation",
      "structural_fix": "Add deploy state file per pod: `DEPLOY_STATE:phase=downloading,started=1715270400`; control server polls state and alerts on timeout > 5min; implement step timeouts with cleanup on expiry",
      "verify": "Simulate stuck download; confirm timeout triggers after 5min and cleans up; test normal deploy completes before timeout",
      "novel": false
    }
  ],
  "missed_in_session_rca": [
    "Watchdog's health polling lacks deployment awareness (F-8)",
    "No bilateral consistency between control server and pods (F-9)",
    "Deploy operations are not idempotent (F-11)",
    "Resource exhaustion from orphaned processes affects stability (F-6)",
    "BLOCKED_PATTERNS design is overly restrictive without allowlisting (F-10)"
  ],
  "recommended_priority_order": ["F-1", "F-2", "F-9", "F-4", "F-5", "F-8", "F-3", "F-6", "F-11", "F-10", "F-7", "F-12"]
}
```