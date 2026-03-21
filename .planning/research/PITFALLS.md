# Pitfalls Research

**Domain:** Windows process monitoring / whitelist enforcement on gaming fleet (v12.1 E2E Process Guard)
**Researched:** 2026-03-21
**Confidence:** HIGH — drawn from direct incident record (Steam/leaderboard/watchdog/dev+prod incidents) + Windows internals knowledge + verified web sources

---

## Critical Pitfalls

### Pitfall 1: Keyword-Scoped Audit Instead of Whitelist-Inversion

**What goes wrong:**
The audit that triggered v12.1 is the canonical example. Steam, the leaderboard kiosk Edge instance, and the voice assistant watchdog were all missed because the audit searched for processes related to "racing/pod" keywords rather than enumerating everything running and checking it against a known-good list. Keyword scanning is an allowlist by accident — it only finds what you already know to look for.

**Why it happens:**
The natural tendency is to ask "is anything pod-related in the wrong place?" instead of "is everything running supposed to be running?" The audit felt complete because it caught several violations, but the framing was wrong from the start. String matching against suspected bad actors will always have gaps.

**How to avoid:**
The guard's logic must be: `running_processes - whitelist = violations`. Never the reverse. The whitelist is the source of truth. Anything not on it is a violation regardless of whether anyone thought to search for it. Implement deny-by-default: unknown process = violation, not unknown process = ignore.

**Warning signs:**
- Guard code contains string matching (`contains("steam")`, `contains("game")`) instead of set membership check
- Whitelist grows via "add what I know" rather than "enumerate what is running and approve selectively"
- Audit reports "no violations found" but no one can name every process on the machine

**Phase to address:**
Phase 1 (Whitelist Schema) — the data model must enforce deny-by-default. A `whitelist: Vec<ApprovedProcess>` where anything not in the set is a violation, never a blocklist.

---

### Pitfall 2: Killing Transient System Processes During Boot and Windows Updates

**What goes wrong:**
Windows spawns short-lived processes during startup, Windows Update, driver installation, and Defender scans. Examples: `MpCmdRun.exe` (Defender scan), `TiWorker.exe` (Windows Update), `msiexec.exe` (installer), `DismHost.exe` (DISM repair), `MusNotification.exe` (update notification), `wuauclt.exe`, `svchost.exe` with the `wuauserv` service. A process guard with a short scan cycle that kills anything not on the whitelist will terminate these mid-execution, corrupting update state, leaving drivers partially installed, or breaking Defender's signature database. Microsoft has confirmed svchost.exe/wuauserv crashes as a known Windows 11 24H2 issue — killing mid-process makes recovery harder.

**Why it happens:**
These processes are transient — they appear, do work, and exit. They are not in the "installed software" mental model so they get omitted from the whitelist. The guard sees them once and kills them before anyone realizes they should be allowed.

**How to avoid:**
- Build a two-tier whitelist: permanent processes (always allowed) and a system-process exclusion zone. Processes with image paths under `C:\Windows\System32\`, `C:\Windows\SysWOW64\`, or `C:\Program Files\Windows Defender\` that are signed by Microsoft default to ALERT-only, never auto-kill.
- Implement a kill grace period: flag for two consecutive scan cycles before killing. Transient Windows processes typically exit within 30-60 seconds without intervention. A process seen once and gone is never killed.
- Never kill processes whose parent is `TrustedInstaller.exe`, `wininit.exe`, or `services.exe` unless explicitly configured.

**Warning signs:**
- Windows Update silently fails or partially applies after guard deployment
- Defender reports outdated signatures (update was killed mid-download)
- Driver installs require multiple attempts

**Phase to address:**
Phase 1 (Whitelist Schema) — system process exclusion rules in the data model from day one. Phase 2 (Enforcement Logic) — kill grace period baked into the enforcement loop, not added later.

---

### Pitfall 3: PID Reuse Race — Killing the Wrong Process

**What goes wrong:**
The guard scans, sees `SomeProcess.exe` (PID 4821) that is not whitelisted, resolves to kill it. Between the scan and the `TerminateProcess()` call, PID 4821 exits naturally and Windows reassigns that PID to a new process — possibly `rc-agent.exe` or `racecontrol.exe`. The guard kills the new holder of the PID. This is not hypothetical: the killproc PID reuse race is a documented failure mode in OS process management, and Windows PID reuse is as fast as Linux's.

**Why it happens:**
Process enumeration (`CreateToolhelp32Snapshot`, WMI `Win32_Process`) returns a snapshot. The act of killing is a separate syscall. On a busy gaming machine with many short-lived processes (game shader compilers, audio helpers, overlay processes), the window between snapshot and kill can be 10-50ms — enough for PID reuse under load.

**How to avoid:**
Never kill by PID alone. Kill by PID + process name + creation time triple. Open a handle with `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE)`, verify the process name and creation time (via `GetProcessTimes()`) against the snapshot values, then call `TerminateProcess()` only if both match. Mismatch means the PID was reused — log and skip.

**Warning signs:**
- Kill function takes a `u32` pid with no additional identity verification
- rc-agent crashes with no error in its own logs (killed externally)
- Random pod disconnects correlating with guard scan cycle timing

**Phase to address:**
Phase 2 (Enforcement Logic) — the kill function must be: open handle, verify identity, kill or abort. This is correctness, not optional hardening.

---

### Pitfall 4: Self-Kill — Guard Terminates Itself or Its Parent Service

**What goes wrong:**
The process guard binary (`rc-process-guard.exe` or the guard module inside `rc-agent.exe`) appears in the process list. If its own name is not on the whitelist, or if it matches under a slightly different image name (renamed binary, different path, casing difference), it kills itself. For the embedded module case: the guard is a module inside `rc-agent.exe`. It scans, sees rc-agent is not on the whitelist (typo, wrong casing, path mismatch), triggers a kill of the containing process — which is itself.

**Why it happens:**
Self-reference is easy to overlook. The developer mentally excludes "us" but forgets to encode that exclusion. Windows file paths are case-insensitive but string equality is case-sensitive in Rust by default — `RC-Agent.exe` vs `rc-agent.exe` breaks a naive equality check.

**How to avoid:**
- Self-identity determined at runtime: `std::env::current_exe()` for canonical path, `std::process::id()` for own PID. Both excluded from kill candidates unconditionally, before whitelist lookup, regardless of whitelist contents.
- For the embedded module case: the PID of the containing process (`rc-agent`) is unconditionally excluded.
- All process name comparisons must use case-insensitive matching: `eq_ignore_ascii_case()` in Rust.
- The direct parent of the guard process is also excluded from kills.

**Warning signs:**
- Guard exits immediately after first scan with no error log
- rc-agent restarts unexpectedly correlated with guard scan cycle timing
- Guard never logs any violations (it died before it could report)

**Phase to address:**
Phase 2 (Enforcement Logic) — self-exclusion is the first filter applied before any whitelist logic runs.

---

### Pitfall 5: Auto-Start Entry Removed Without Per-Machine Context

**What goes wrong:**
This happened already: Edge kiosk (leaderboard) had its `HKLM\Run` entry on a pod instead of the server. The auto-start audit removes it as a violation. The leaderboard stops loading after pod restart and no one knows why — there is no error, the process just never starts. Registry entries do not re-add themselves the way processes can be restarted.

**Why it happens:**
Auto-start audit without per-machine context removes entries that are valid on one machine but invalid on another. The guard correctly identifies the entry as a violation on the pod, but the fix (remove it) silently breaks expected behavior with no recovery path unless someone knows to restore it.

**How to avoid:**
- Per-machine whitelist sections in `racecontrol.toml`. The server's `[machine.server]` section includes `Edge --kiosk leaderboard` in auto-start. Pod sections do not. The guard computes violations relative to the machine's own whitelist section.
- ALERT before REMOVE for auto-start entries. The kill-on-sight policy for processes does not extend to registry entries. Removing an auto-start entry is harder to detect and harder to recover from than killing a process. Auto-start enforcement should have a three-stage progression: LOG (default, configurable) → ALERT (after N cycles) → REMOVE (only with `autostart_enforcement = "remove"` explicitly set in config).
- Write a restore backup file (`autostart-removed-YYYYMMDD.toml`) before removing any auto-start entry.

**Warning signs:**
- Service or kiosk stops loading after pod restart without any deploy event
- Registry Run key count on a machine drops without a corresponding deploy commit
- Kiosk shows blank screen instead of expected content after reboot

**Phase to address:**
Phase 1 (Whitelist Schema) — per-machine `[machine.pod_N]` sections with explicit auto-start entries. Phase 3 (Auto-Start Audit) — LOG → ALERT → REMOVE progression implemented as three distinct enforcement modes.

---

### Pitfall 6: Watchdog Infinite Restart Loop Survives Kill

**What goes wrong:**
The voice assistant watchdog (`watchdog.cmd`) was an infinite restart loop. The guard kills the process. The watchdog restarts it within 200ms. The guard kills it again. This creates a kill-restart storm that consumes CPU, floods the violation log, and fires repeated alerts — but the underlying problem (watchdog.cmd in the Startup folder) is never addressed because process-level kill does not remove the auto-start source.

**Why it happens:**
Process-level kill treats the symptom (running process) without treating the cause (auto-start entry). A self-restarting process cannot be eliminated by killing alone.

**How to avoid:**
- Kill sequence must be: (1) identify auto-start source for this process name, (2) remove or disable auto-start entry, (3) kill the process. Kill without auto-start removal is futile for watchdogged processes.
- Implement restart storm detection: if the same process name is killed 3 or more times within 60 seconds, escalate to "auto-start audit required" and suppress further kills until the auto-start source is resolved. Alert staff with the specific auto-start location.
- The `Startup` folder (both `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup` and `C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup`) must be included in every auto-start audit alongside registry Run keys.

**Warning signs:**
- Same process name appears in kill log multiple times within 60 seconds
- CPU usage on the pod increases after guard deployment (kill-restart storm overhead)
- Alert rate for a single process does not decrease over time

**Phase to address:**
Phase 2 (Enforcement Logic) — kill sequence includes auto-start source lookup. Phase 3 (Auto-Start Audit) — Startup folders included alongside HKCU/HKLM Run and Scheduled Tasks.

---

### Pitfall 7: Interpreted Runtime Whitelisting — node.exe Covers Both Dev and Prod

**What goes wrong:**
On James's machine, `next dev` (dev server) and `next start` (prod server) both appear as `node.exe`. If `node.exe` is on the whitelist, both are allowed — and dev server left running alongside prod server causes port conflicts and inconsistent kiosk behavior. Process name whitelisting is insufficient for interpreted runtimes. This is the exact scenario that occurred: kiosk Next.js dev and prod server running simultaneously.

**Why it happens:**
Multiple `node.exe` instances are indistinguishable by name. The developer whitelists the runtime (`node.exe`) without constraining which invocations are permitted.

**How to avoid:**
- Whitelist entries for interpreted runtimes must include a `args_pattern` field. Example: `node.exe` is allowed only when args match `*next*start*` or `*server.js*`. Any `node.exe` with `*dev*` or `*--inspect*` in its command line on a pod is a violation.
- Add a `max_instances: 1` field for processes that should have exactly one copy running. Two `node.exe` instances when one is expected triggers a violation for the extra instance.
- Port audit supplements process audit: if port 3000 is bound on a pod (never approved for pods), that is a violation regardless of what process holds it. Port audit catches what process audit misses.
- Retrieve full command line via `QueryFullProcessImageName()` and the `NtQueryInformationProcess` / WMI `CommandLine` field.

**Warning signs:**
- Port already in use errors on startup (two servers competing)
- Kiosk loads inconsistently (requests sometimes hitting dev server)
- Process guard shows no violations despite known rogue process running

**Phase to address:**
Phase 1 (Whitelist Schema) — `args_pattern` and `max_instances` fields in the whitelist entry schema. Phase 4 (Port Audit) — port whitelist as a complementary enforcement layer.

---

### Pitfall 8: Cross-Machine Boundary Violation — Pod Binary Running on James

**What goes wrong:**
Standing rule #2: NEVER run pod binaries on James's PC. `rc-agent.exe` on James crashes the workstation. Without programmatic enforcement, the rule is only as strong as human memory. A developer running `rc-agent.exe` locally to test behavior, or a deploy script targeting the wrong machine, violates this rule silently until the crash.

**Why it happens:**
During development or debugging, running a binary locally seems natural. The rule exists because of past crashes but is not enforced at the system level.

**How to avoid:**
- Machine identity check built into the whitelist schema: each approved process entry carries an `allowed_machines` field specifying `["pod_*"]`, `["james"]`, or `["server"]`. The guard resolves machine identity at startup from hostname or static IP (read from config, not inferred) and filters the whitelist to only the entries for this machine.
- `rc-agent.exe` on the `james` machine is a CRITICAL violation with immediate alerting — no kill grace period, no two-cycle wait. Same for `ollama.exe` or `webterm.py`/`python.exe` on pods.
- Machine identity must be determined from the config (the `[machine]` section that names this machine), not from process introspection.

**Warning signs:**
- `rc-agent.exe` appears in James's process list
- `ollama.exe` appears in a pod's process list
- Deploy script output shows binary copied to wrong destination without any guard alert

**Phase to address:**
Phase 1 (Whitelist Schema) — `allowed_machines` is a mandatory field on every whitelist entry, not optional. Phase 2 (Enforcement Logic) — machine identity resolved at daemon startup, immutable for the run.

---

### Pitfall 9: Config Sync Lag Kills a Newly Deployed Process

**What goes wrong:**
Central `racecontrol.toml` is updated to add a new approved process (a new game launcher, a new monitoring tool). The config sync pushes to pods on the existing 30-second cycle. Between the update and the sync completing on all pods, the process guard on pods kills the new process as soon as it starts. The operator sees the process they just deployed being killed by the guard with no apparent reason, no error from the process itself.

**Why it happens:**
The guard runs continuously. Config sync is periodic. There is a 0–30 second window where the guard has stale whitelist config. If the new process starts within that window, it gets killed.

**How to avoid:**
- The kill grace period (two consecutive scan cycles) provides a natural buffer. If the scan cycle is 10 seconds and grace period requires two hits, a process must be non-whitelisted for 20 seconds before being killed. This overlaps with the 30-second sync window — meaning a process that starts simultaneously with a config update will usually survive to see the updated whitelist.
- Emit a `config_version` field (hash of the whitelist section) in the guard's heartbeat so the fleet dashboard can show which machines have current config.
- Document the deploy sequence: push config first, verify `config_version` matches on all machines (check via fleet dashboard), then deploy the new process binary.

**Warning signs:**
- Newly deployed process is killed within seconds of first start on a pod
- Kill log shows the same process killed exactly once then never again (survived after config synced)
- Alert fires for a process the operator just explicitly approved

**Phase to address:**
Phase 1 (Whitelist Schema) — config includes version hash. Phase 5 (Fleet Integration) — deploy sequence documentation specifies config-first, verify-sync, then binary.

---

### Pitfall 10: WMI Query Performance Overhead on Active Gaming Sessions

**What goes wrong:**
`SELECT * FROM Win32_Process` via WMI, or repeated `tasklist /FO CSV` calls in a polling loop, can spike CPU for 100-500ms per query. WMI's `Win32_Process` with no property filter enumerates all properties for all processes, forcing WMI provider host (`WmiPrvSE.exe`) to collect memory maps, handle tables, and string data for every process on the system. On a gaming pod running Assetto Corsa at 60fps with FFB active, any CPU spike causes frame drops and stutter visible to the customer.

**Why it happens:**
`SELECT *` queries are standard in scripts but unacceptable in a daemon running during active sessions. The developer reaches for the familiar WMI approach without measuring the overhead on a loaded gaming machine.

**How to avoid:**
- Use `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)` via WinAPI instead of WMI. This is a kernel-level snapshot with near-zero overhead — it is what Task Manager uses. In Rust, use the `windows` crate with `tlhelp32` directly.
- If WMI is used for any reason, project only needed columns: `SELECT Name, ProcessId, CreationDate FROM Win32_Process`, never `SELECT *`.
- Poll at 10-second intervals during active billing sessions, 30-second intervals during idle. A rogue process running for 30 seconds before detection is fine — it has already been running for hours before the guard existed.
- Run the scan loop on a thread with `THREAD_PRIORITY_BELOW_NORMAL` to yield to game processes.
- Benchmark requirement: each scan cycle must complete in under 20ms on a loaded gaming pod. This must be a passing test in the test suite, not a verbal commitment.

**Warning signs:**
- Guard code uses WMI `Win32_Process` queries in the polling loop
- CPU usage on pods increases by 2-5% after guard deployment
- Customers report frame stuttering that correlates with guard scan timing
- `WmiPrvSE.exe` shows elevated CPU in Task Manager on pods

**Phase to address:**
Phase 2 (Enforcement Logic) — API selection (Toolhelp32 vs WMI) is decided here with a benchmark test, not as an afterthought.

---

### Pitfall 11: Startup Order Race — Guard Kills Whitelisted Processes Before They Finish Starting

**What goes wrong:**
At pod boot: HKLM Run keys fire, `start-rcagent.bat` runs, and the process guard starts. The guard's first scan runs at second 3. `ConspitLink.exe` is whitelisted but takes 8-10 seconds to initialize its HID connection. The kiosk Edge instance takes 12-15 seconds to open and display. The guard's first scan sees neither and, depending on the kill grace period, may flag them for termination before they have had a chance to start.

**Why it happens:**
Slow-starting processes (HID devices, browser kiosks, network-dependent services) take longer to reach "running" state than the guard's initial scan window. The guard correctly interprets their absence as a violation of "required process not running" — but it is a false alarm during the startup race.

**How to avoid:**
- Implement a startup amnesty window: the guard does not enforce presence-based violations (required processes not yet running) for the first 60 seconds after boot. It scans and logs but does not alert or kill during this window.
- Per-process `startup_delay_s` field in the whitelist entry: the guard will not flag the absence of that process as a violation until N seconds after boot. `ConspitLink.exe: startup_delay_s: 30`, `msedge.exe (kiosk): startup_delay_s: 60`.
- Presence enforcement (required process must be running) is separate from absence enforcement (non-whitelisted process must not be running). Non-whitelisted processes can be flagged immediately. Required-but-absent processes need the startup delay.

**Warning signs:**
- Guard kill log shows a whitelisted process name in entries timestamped within 60 seconds of pod boot
- ConspitLink or Edge kiosk killed during pod startup, rc-agent pre-flight fails immediately after every reboot
- Operators have to manually restart ConspitLink after every reboot

**Phase to address:**
Phase 1 (Whitelist Schema) — `startup_delay_s` field on whitelist entries. Phase 2 (Enforcement Logic) — startup amnesty window is a first-class feature, not a workaround added after the first reboot failure.

---

### Pitfall 12: Process Path Not Verified — Masquerading Process Names

**What goes wrong:**
The whitelist allows `msedge.exe`. An attacker (or a careless software install) drops `msedge.exe` in `C:\Users\bono\Downloads\` and runs it. The guard sees `msedge.exe` on the whitelist, allows it. The real Microsoft Edge lives in `C:\Program Files (x86)\Microsoft\Edge\Application\`. Without path verification, any process can masquerade as a whitelisted process by using the same image name.

**Why it happens:**
`CreateToolhelp32Snapshot` returns only the image name (filename), not the full path. Developers use the filename for matching because it is immediately available, without realizing path verification requires an additional API call.

**How to avoid:**
- For all whitelist entries, use `QueryFullProcessImageName()` to get the full path, then match against the `allowed_path_prefix` in the whitelist entry. Example: `msedge.exe` is only allowed if its path starts with `C:\Program Files (x86)\Microsoft\Edge\` or `C:\Program Files\Microsoft\Edge\`.
- `allowed_path_prefix` is optional for processes with no known-stable install path, but required for system binaries and any security-relevant process (Edge kiosk, ConspitLink).

**Warning signs:**
- Whitelist matching uses only the filename component of the process image path
- No `QueryFullProcessImageName()` call in the guard code
- `allowed_path` field absent from whitelist entry schema

**Phase to address:**
Phase 1 (Whitelist Schema) — `allowed_path_prefix` field in the schema. Phase 2 (Enforcement Logic) — path verification in the process identity check alongside name and creation time.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Hardcode whitelist in source code instead of TOML | Simpler first implementation | Every whitelist change requires binary rebuild and redeploy to all 8 pods | Never — TOML from day one |
| Kill by process name only (no path check) | Easier to implement | Masquerading processes bypass guard; legitimate processes with same name get killed | Never for enforcement; acceptable for logging only |
| Single global whitelist (no per-machine sections) | Simpler data model | Cannot express "Ollama allowed on James, not pods" | Never — per-machine is a day-one requirement given the cross-machine boundary rule |
| Poll at 5-second interval regardless of session state | Simpler logic | Visible CPU spikes during active gaming sessions | Never on pods; acceptable on James and server |
| Alert-only mode as permanent state | Safe, no false-kill risk | Violations accumulate without resolution; guard becomes noise nobody reads | Acceptable for first 2 weeks post-deploy to tune whitelist thresholds |
| WMI `SELECT *` queries in polling loop | Familiar API | 100-500ms CPU spike per query on all pods | Never in the polling loop; acceptable for one-shot audit scripts |
| REMOVE auto-start entries immediately (no LOG → ALERT → REMOVE progression) | Faster enforcement | Silently breaks services on wrong machine with no recovery path | Never — progression is required |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| HKLM Run audit | Query `HKLM\Software\Microsoft\Windows\CurrentVersion\Run` only | Also query `HKCU\...\Run`, `HKLM\...\RunOnce`, `HKCU\...\RunOnce`, and both Startup folders (user and all-users) |
| Scheduled Tasks audit | Check Task Scheduler root folder only | Check all subfolders including `\Microsoft\Windows\*` — third-party software registers tasks in subfolders |
| Port audit | Parse `netstat` output | Use `GetExtendedTcpTable` / `GetExtendedUdpTable` via WinAPI for reliable PID-to-port mapping; `netstat` output format varies |
| Process path verification | Use `ImageName` from `PROCESSENTRY32` | `ImageName` is the filename only, not full path; use `QueryFullProcessImageName()` for path-based matching |
| Windows Service processes | Whitelist `svchost.exe` processes by name | All Windows Services appear as `svchost.exe`; whitelist by service name via SCM query (`EnumServicesStatusEx`), not by process name |
| ConspitLink identity | Name check only | Path must match `C:\RacingPoint\ConspitLink.exe`; a coincidentally named `ConspitLink.exe` elsewhere is a violation |
| Config sync integration | Guard reads config directly from disk on each scan | Reuse the existing `racecontrol.toml` TOML watcher from v10.0 (config sync infra); do not add a separate file watcher |
| rc-agent embedded module | Guard spawns `tokio::task` that calls `kill_process()` | The kill syscall blocks; use `tokio::task::spawn_blocking` for any WinAPI `TerminateProcess()` calls inside an async context |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| WMI `Win32_Process SELECT *` in polling loop | WmiPrvSE.exe CPU spike, game stutter | Use `CreateToolhelp32Snapshot` instead | Immediately on pods with active sessions |
| Logging every scan (including clean scans) to disk | Log grows to GB; disk I/O during gaming | Log only violations and state changes; rotate at 10MB | After ~1 week of continuous operation |
| Scanning all 8 pods sequentially from server | 8x scan time blocks server event loop | Parallel async scans with per-pod timeout | As soon as more than 2 pods are active |
| Calling `QueryFullProcessImageName()` for every process on every scan | Each call is a syscall; 100+ processes = 100+ syscalls per scan | Cache process identities between scans; only query new PIDs (ones not seen in previous snapshot) | On a pod with 100+ running processes |
| Kill verification loop without timeout | Guard hangs if `TerminateProcess()` is blocked (elevated process, protected process) | Set 500ms timeout on kill verification; log and escalate if process survives the kill attempt | Any process running with higher privilege than the guard |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Guard runs as standard user | Cannot kill processes owned by other users or running elevated; enforcement is incomplete | Guard must run as SYSTEM or the same administrative user as the processes it monitors; on pods this means the service context |
| Auto-start entries removed without backup | Silent change with no recovery path if an entry was incorrectly removed | Write `autostart-removed-YYYYMMDD.toml` before removing any auto-start entry; enables one-command restore |
| Whitelist stored in world-writable location | An attacker or rogue process modifies the whitelist to permit their process | `racecontrol.toml` on server is writable only by ADMIN; pods receive read-only config via the existing sync mechanism |
| Kill log contains full process command lines | Command lines may contain passwords, API keys, or tokens passed as args | Truncate command lines at 200 chars in logs; do not log environment variables |
| Guard binary path not verified | An attacker replaces the guard binary with a malicious one; the guard never detects violations | Not a v12.1 requirement but note: if the guard binary is in a user-writable path, it can be replaced without admin |

---

## "Looks Done But Isn't" Checklist

- [ ] **Deny-by-default:** Remove a known process from the whitelist and confirm the guard detects and kills it within two scan cycles
- [ ] **Per-machine enforcement:** Start `ollama.exe` on Pod 8 and confirm kill and alert fire; start `rc-agent.exe` on James and confirm CRITICAL alert fires
- [ ] **Auto-start all four sources:** Plant a test entry in each of HKCU Run, HKLM Run, user Startup folder, and Scheduled Tasks; confirm all four are detected
- [ ] **Auto-start LOG-before-REMOVE:** Add a non-whitelisted Run key; confirm the guard logs and alerts before any removal action
- [ ] **Self-exclusion:** Remove the guard binary from the whitelist; confirm the guard continues running and does not kill itself
- [ ] **Startup amnesty:** Reboot a pod and verify ConspitLink and Edge kiosk survive the first scan cycle without being killed
- [ ] **Kill grace period:** Start a short-lived Windows Update helper process; confirm it is not killed if it exits before the second scan cycle
- [ ] **PID reuse protection:** Kill function verifies process name and creation time before calling TerminateProcess; reviewed in code review
- [ ] **Watchdog storm detection:** Start an infinite-restart script; confirm storm detection fires after 3 kills within 60 seconds and escalates to auto-start audit
- [ ] **Config sync lag:** Update whitelist on server; start a new process on a pod within 5 seconds; confirm it survives the grace period and is not killed after sync completes
- [ ] **Performance benchmark:** Run a full scan cycle on an active gaming pod; confirm it completes in under 20ms (measured via tracing span)
- [ ] **Port audit:** Bind a test TCP server to a non-approved port; confirm guard detects and alerts

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Guard killed whitelisted process during startup | LOW | Restart the affected process manually; extend `startup_delay_s` in config and push config update |
| Guard removed auto-start registry entry incorrectly | MEDIUM | Restore from `autostart-removed-YYYYMMDD.toml` backup; re-add the Run key via the existing fleet exec endpoint |
| Kill-restart storm consuming CPU | LOW | Push `enforcement_mode = "alert_only"` in config to affected pod; investigate and remove auto-start source; re-enable enforcement after source removed |
| PID reuse killed rc-agent | HIGH | rc-agent self-restarts via Windows Service auto-restart; check billing session state on affected pod; fix creation-time verification in guard code |
| Whitelist config out of sync causing mass kills | MEDIUM | Push `enforcement_mode = "alert_only"` immediately to all pods; push correct config; verify `config_version` matches on all machines; re-enable enforcement |
| Guard killed itself | LOW | Guard exits; Windows Service auto-restart recovers it; fix self-exclusion logic before next deploy |
| Auto-start removed on wrong machine breaks service | MEDIUM | Restore from backup file; identify which machine section in TOML was missing the entry; add it and push config |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Keyword-scoped audit (whitelist inversion) | Phase 1 — Whitelist Schema | Schema uses deny-by-default `Vec<ApprovedProcess>`; no blocklist logic in codebase |
| Transient system processes killed | Phase 1 (system exclusion rules) + Phase 2 (kill grace period) | Run Windows Update on a pod with guard active; update completes successfully |
| PID reuse race | Phase 2 — Enforcement Logic | Kill function verifies name + creation time; code review confirms no PID-only kills |
| Self-kill | Phase 2 — Enforcement Logic | Remove guard from whitelist; guard continues running |
| Auto-start on wrong machine removed | Phase 1 (per-machine schema) + Phase 3 (auto-start audit) | Plant Run key on wrong machine; guard alerts but does not remove without `enforcement = "remove"` config |
| Watchdog infinite restart loop | Phase 2 (storm detection) + Phase 3 (auto-start audit) | Infinite-restart script triggers storm detection and escalates to auto-start audit |
| Dev + prod servers simultaneously | Phase 1 (args_pattern + max_instances) + Phase 4 (port audit) | Start `next dev` on James; guard detects via args_pattern or port conflict |
| Cross-machine binary violation | Phase 1 (allowed_machines) + Phase 2 (enforcement) | Start rc-agent.exe on James; CRITICAL alert fires within one scan cycle |
| Config sync lag kills new process | Phase 1 (config version hash) + Phase 5 (fleet integration) | New process survives 30-second sync window due to kill grace period |
| WMI performance on gaming pods | Phase 2 — Enforcement Logic | Benchmark test: scan cycle under 20ms; no CPU spike during AC session |
| Startup order kills slow-starting processes | Phase 1 (startup_delay_s) + Phase 2 (amnesty window) | Reboot pod; ConspitLink and Edge kiosk survive first scan cycle |
| Process name masquerading | Phase 1 (allowed_path_prefix) + Phase 2 (path verification) | Drop fake `msedge.exe` in Downloads; guard detects path mismatch and kills |

---

## Sources

- Direct incident record: Steam missed by keyword-scoped audit (v12.1 trigger incident, 2026-03-21)
- Direct incident record: Leaderboard kiosk HKLM Run entry on wrong machine (v12.1 trigger incident)
- Direct incident record: `watchdog.cmd` infinite restart loop surviving process kill (v12.1 trigger incident)
- Direct incident record: `next dev` and prod server running simultaneously on James (v12.1 trigger incident)
- `PROJECT.md`: v12.1 milestone context, standing rule #2 (no pod binaries on James), HKLM Run key architecture, existing config sync infra
- `CLAUDE.md`: Windows Service context, Session 0/1 boundary, ConspitLink VID:PID, static IP assignments, deploy rules
- [Windows Update svchost/wuauserv crash confirmation](https://www.windowslatest.com/2025/04/30/microsoft-confirms-windows-11-24h2-0x80240069-svchost-exe_wuauserv-crashes/) — Windows Latest (MEDIUM confidence — confirms transient svchost instability during updates)
- [WMI performance troubleshooting](https://learn.microsoft.com/en-us/troubleshoot/windows-server/system-management-components/scenario-guide-troubleshoot-wmi-performance-issues) — Microsoft Learn (HIGH confidence)
- [WMI Tasks: Performance Monitoring](https://learn.microsoft.com/en-us/windows/win32/wmisdk/wmi-tasks--performance-monitoring) — Microsoft Win32 docs (HIGH confidence)
- [PID reuse race condition](https://access.redhat.com/solutions/30695) — Red Hat (MEDIUM confidence — Linux origin, but PID reuse is OS-agnostic; Windows exhibits identical behavior)
- [SERVICE_DELAYED_AUTO_START_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winsvc/ns-winsvc-service_delayed_auto_start_info) — Microsoft Win32 docs (HIGH confidence — startup ordering)
- [Windows Sessions internals](https://brianbondy.com/blog/100/understanding-windows-at-a-deeper-level-sessions-window-stations-and-desktop) — Brian Bondy (MEDIUM confidence — Session 0/1 boundary, service process isolation)

---
*Pitfalls research for: Windows process monitoring / whitelist enforcement on gaming fleet (v12.1 E2E Process Guard)*
*Researched: 2026-03-21 IST*
