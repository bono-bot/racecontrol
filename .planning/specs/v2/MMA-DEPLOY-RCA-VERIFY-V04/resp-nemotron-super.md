=== SCORES ===
1. substrate_evolution_validity: 2 — The plan overstates substrate evolution; TTL-aware sentinels and SSH transport merely rearrange the userspace‑vs‑watchdog race without eliminating the core structural flaw identified in Rounds 1‑3.
2. cr_fl_x_closed: 3 — Clearing the sentinel before waiting for the watchdog reduces the chicken‑and‑egg window, but NTFS propagation delays after the sentinel clear (Step 6.5) still allow a watchdog poll to see the sentinel present and skip restart, recreating the deadlock.
3. cr_fl_a_race_window: 3 — The confirmed‑kill loop covers the watchdog poll interval, but a degraded previous‑binary that crashes quickly after respawn can cause a rapid respawn‑loop during the healthcheck window, leaving the script unable to distinguish a healthy respawn from a failing loop.
4. cr_fl_e_prev_integrity: 2 — Comparing the pre‑swap rc-agent.exe SHA to a canonical clean‑commit SHA ignores the "-dirty" state of the running binary on Pods 1‑7, guaranteeing false aborts and blocking the ramp unless the file is missing or the check is removed.
5. cr_fl_f_flock_semantic: 3 — A flock lock on the script file only prevents concurrent invocations of the same script; it does not block manual SSH, other deploy scripts, or rc‑sent‑initiated restarts, and the lock may persist if the SSH tunnel drops mid‑execution.
6. cr_fl_g_healthcheck: 3 — The 4‑axis check plus 60‑second soak assumes the PR #66 heartbeat thread is reliably invoked and that a single mtime advance within 60 s guarantees health; silent‑loop‑death can persist with a stalled heartbeat thread, and the window is too short to catch slower watchdog respawns or intermittent failures.
7. operational_discipline: 3 — Canary mode relies on a manual 5‑minute Captain observation gate (no automated timeout) and the DHCP‑IP‑drift mitigation for Pod 1 is only a risk‑register note, not implemented code, making the process fragile to human availability and network changes.

V04-FL-1
  id: V04-FL-1
  severity: P0
  section: 2
  what's wrong: The substrate‑evolution argument claims TTL‑aware sentinels, SSH transport, and Pod 8’s 4‑day stability eliminate the need for LockFileEx, but the fundamental race between a userspace deploy script and the service‑level watchdog remains; the script still must time its sentinel write/clear relative to watchdog polls, merely moving the race from rc‑sentry to deploy‑pod‑agent.sh.
  why it matters: If the script’s timing assumptions drift (e.g., due to load, latency, or NTFS jitter), the watchdog can miss the sentinel transition and either skip restart (deadlock) or respawn while the old binary is still present, leading to duplicate agents or failed deployments.
  proposed amendment: Abandon the script‑level timing approach and implement a kernel‑level mutual exclusion primitive (LockFileEx) that the watchdog checks before deciding to skip restart, guaranteeing that the sentinel state is observed atomically with the agent’s liveness.

V04-FL-2
  id: V04-FL-2
  severity: P0
  section: 3 (Step 6.5)
  what's wrong: After clearing the OTA_DEPLOYING sentinel (Step 6), the script sleeps 2 s then verifies the sentinel is gone. NTFS may delay the delete propagation beyond 2 s, allowing a watchdog poll that occurs during this window to still see the sentinel present and skip restart, reproducing the CR‑FL‑X chicken‑and‑egg deadlock.
  why it matters: A pod can be left with no rc-agent running and the watchdog refusing to start a new instance, requiring manual intervention to clear the sentinel and restart the agent.
  proposed amendment: Replace the fixed sleep with a retry loop that polls the sentinel’s existence (with a short backoff) until it is confirmed cleared or a timeout expires, and only then proceed to the watchdog‑wait phase. Additionally, flush the delete operation via `fsutil file setvaliddata` or equivalent to force metadata sync.

V04-FL-3
  id: V04-FL-3
  severity: P1
  section: 5.A
  what's wrong: The deterministic analysis assumes the watchdog’s reaction time is bounded by the poll interval, but a degraded previous‑binary (rc‑agent‑prev.exe) that crashes within seconds of respawn can cause the watchdog to trigger rapid respawn cycles during the script’s healthcheck window, making it impossible for the 4‑axis check to distinguish a healthy steady state from a failing loop.
  why it matters: The script may declare a pod healthy while the agent is actually crashing repeatedly, leading to a pod that appears healthy but is unstable and may fail later under load.
  proposed amendment: Extend the healthcheck to require not only a single heartbeat advance but a minimum number of advances (e.g., two advances spaced ≥30 s apart) and/or monitor the rc‑agent.exe crash count via the Windows Event Log or rc‑watchdog.log during the observation window, aborting if crashes exceed a threshold.

V04-FL-4
  id: V04-FL-4
  severity: P0
  section: 4 (pre‑swap prev binary integrity check)
  what's wrong: The script hashes the current rc-agent.exe and compares it to a canonical SHA for the clean `c5f94e31` commit. Pods 1‑7 are running `c5f94e31-dirty` (uncommitted changes), so their SHA will never match, causing the script to abort on every pod unless the canonical‑SHA file is missing or the check is bypassed.
  why it matters: The ramp cannot start because the integrity check falsely flags all target pods as having a corrupted binary, blocking deployment entirely.
  proposed amendment: Remove the strict equality check; instead, verify that the current binary is compatible with the OTA mechanism (e.g., check for the presence of the TTL‑aware sentinel handling code) or allow a configurable tolerance for dirty builds, recording the observed SHA for post‑hoc reconciliation rather than blocking on mismatch.

V04-FL-5
  id: V04-FL-5
  severity: P1
  section: 4 (flock interlock)
  what's wrong: Wrapping the script in `flock /tmp/deploy-pod-agent.lock` only prevents concurrent invocations of the same script on the same machine. It does not stop a parallel-james session from running the older `deploy-pod.sh`, a manual SSH command, or a rc‑sent‑initiated `restart_service()` that runs concurrently with the script’s atomic steps.
  why it matters: Two independent deployment mechanisms could interleave their sentinel writes, clears, and binary swaps, corrupting the OTA state and potentially leaving the pod with a half‑swapped binary or duplicate agents.
  proposed amendment: Replace the advisory flock with a system‑wide named mutex (e.g., a Windows named mutex created via `CreateMutex`) that any deployment tool must acquire before touching the OTA sentinel or binary, ensuring mutual exclusion across all deployment paths.

V04-FL-6
  id: V04-FL-6
  severity: P1
  section: 4 (Step 7b heartbeat soak)
  what's wrong: The 60‑second post‑success soak assumes the PR #66 heartbeat thread is reliably invoked and that a single mtime advance within that window proves the thread is alive. The commit `d6c623d7` may have added the heartbeat‑thread code without actually starting it, and a stalled tracing buffer could allow the heartbeat thread to run while the main agent loop is dead, yielding a false‑negative.
  why it matters: A pod could pass the healthcheck and soak while the agent’s main logic is stuck (silent‑loop‑death), leading to undetected degradation that only surfaces later under load.
  proposed amendment: Verify that the heartbeat thread is actually started by checking for its presence in the process’s thread list (e.g., via `wmic thread where (processid=<pid>) get threadid`) or by ensuring the heartbeat file advances at least twice with a ≥30 s gap during the observation window, and cross‑check with the absence of new entries in `panic.log` and `rc‑watchdog.log`.

V04-FL-7
  id: V04-FL-7
  severity: P2
  section: 5.C and risk register
  what's worth: The canary mode depends on a manual 5‑minute Captain observation gate with no automated timeout or health‑trend analysis, and the DHCP‑IP‑drift risk for Pod 1 is only mitigated by a note in the risk register, not by any code that dynamically resolves the IP before SSH.
  why it matters: If the Captain is delayed or unavailable, the ramp stalls indefinitely; if Pod 1’s IP has drifted and the script uses a stale SSH target, the deployment will fail silently, requiring manual troubleshooting.
  proposed amendment: Replace the manual gate with an automated health‑trend check that requires the canary pod to meet the 4‑axis criteria for a continuous 5‑minute window (or a configurable timeout with escalation). Additionally, add a pre‑pod‑loop step that resolves the pod’s current IP via a network‑map service or DNS and updates the SSH target dynamically.

V04-FL-8
  id: V04-FL-8
  severity: P2
  section: 4 (pre‑flight SHA file)
  what's wrong: The plan references `scripts/deploy/canonical-binaries/rc-agent-c5f94e31.sha256` but does not confirm that this file exists in the repository or is distributed to the runner; if missing, the SHA check will fail or be skipped, weakening the integrity guard.
  why it matters: Missing or tampered reference data could allow a binary with an unexpected SHA to proceed, undermining the prev‑binary integrity goal.
  proposed amendment: Ensure the canonical‑SHA file is version‑controlled and present; add an early‑exit error if the file cannot be read, and consider storing the expected SHA in the script itself (or retrieving it from a trusted server) to avoid file‑system dependencies.

V04-FL-9
  id: V04-FL-9
  severity: P1
  section: 4 (Step 6.5 sentinel‑cleared verify)
  what's wrong: The sentinel‑clear verification uses a static 2‑second sleep followed by a single existence check. NTFS metadata propagation can exceed this interval, especially under I/O load, causing either a false abort (sentinel still seen) or, worse, a false pass (sentinel still present but not detected) that lets the watchdog skip restart.
  why it matters: Incorrect sentinel state observation can lead to deadlock (watchdog skips) or to the agent being swapped while the watchdog still believes an OTA is in progress, resulting in undefined behavior.
  proposed amendment: Implement a retry loop with exponential backoff (e.g., up to 10 s total) that repeatedly checks for the sentinel’s absence and only proceeds after a confirmed clear, and flush the delete operation via `fsutil file setvaliddata` or a handle‑based flush to force immediate metadata sync.

V04-FL-10
  id: V04-FL-10
  severity: P2
  section: 4 (pre‑flight substrate check)
  what's wrong: The pre‑flight substrate compatibility check runs `git show <ref>:<path>` on James .27, assuming a local git checkout with the exact refs and that the repository is clean and up‑to‑date. If the runner lacks the repo, the refs are unavailable, or the working tree is dirty, the check may abort incorrectly or, worse, silently succeed on a mismatched version.
  why it matters: An incorrect substrate check could allow deployment to a pod running an older watchdog that lacks TTL‑aware sentinel handling, causing the watchdog to ignore the newly written sentinel and never respawn the agent.
  proposed amendment: Replace the git‑based check with a version query against the running binary itself (e.g., query a version resource or a known symbol) or retrieve the expected substrate metadata from a trusted server, ensuring the check reflects the actual binary on the target pod rather than a source‑tree assumption.