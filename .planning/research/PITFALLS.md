# Pitfalls Research

**Domain:** Adding anti-cheat compatibility to existing game-adjacent management software — v15.0 AntiCheat Compatibility
**Researched:** 2026-03-21
**Confidence:** HIGH for EAC/Javelin/iRacing-EOS behaviors (confirmed via official docs, academic analysis, and community evidence), MEDIUM for per-game specifics (AC EVO/LMU anti-cheat strictness level), LOW for exact ban thresholds (intentionally not published by anti-cheat vendors)

---

## Critical Pitfalls

### Pitfall 1: SetWindowsHookEx Keyboard Hook — The Single Highest-Risk Behavior on the Pods

**What goes wrong:**
The existing Phase 78 kiosk lock (implemented to block Win key, Alt+Tab, and other escape keys) uses `SetWindowsHookEx` with `WH_KEYBOARD_LL` (low-level keyboard hook). This is one of the classic detection targets of every major anti-cheat system. EAC-EOS, EA Javelin, and iRacing-EOS all operate kernel-mode drivers that can enumerate system-wide hooks registered via `SetWindowsHookEx`. A hook installed by rc-agent (an unsigned binary) targeting all keyboard input will appear to anti-cheat scanners as a keylogger or input manipulation tool — which is precisely how aimbots and macro cheats work. AutoHotkey scripts, which use the same API, are routinely blocked by EAC and flagged by EA Javelin. The risk is not hypothetical: DisplayFusion (a legitimate multi-monitor tool used by millions) had to specifically implement per-process hook exemption logic and a game-detection folder scanner to avoid triggering anti-cheat systems with its own `SetWindowsHookEx` usage.

**Why it happens:**
`SetWindowsHookEx(WH_KEYBOARD_LL, ...)` is the simplest, most-documented Win32 way to intercept keyboard input globally. It is the obvious first-implementation choice for kiosk lockdown. The mistake is not knowing that EAC sees the hook from the kernel side — it does not need to be injected into the game process to be detected.

**How to avoid:**
Replace `SetWindowsHookEx` entirely before deploying any anti-cheat protected game. The replacement approach is policy-based kiosk lockdown that does not install any system-wide hooks:

1. **Group Policy keyboard filter:** Use `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer` `NoWinKeys=1` (disables Win key without hooking) combined with Edge kiosk mode flags already in use.
2. **Keyboard filter driver (Windows Kiosk Mode):** For commercial kiosk use, Windows 10/11 Pro includes a keyboard filter via `Assigned Access` (kiosk mode) that blocks keys at the shell level without requiring any user-mode hook. This is the anti-cheat-safe approach.
3. **Focus-enforcement instead of hooking:** rc-agent can periodically check if the game window has focus and restore it if not, without hooking keyboard events. This is a polling approach (safe) vs. hook approach (dangerous).

The `SetWindowsHookEx` call must be removed from rc-agent's codebase — not conditioned, not paused — fully replaced. The hook is installed at rc-agent startup and persists for the session, meaning it is present when the game and anti-cheat are running even if rc-agent is otherwise in safe mode.

**Warning signs:**
- Any call to `SetWindowsHookEx` with `WH_KEYBOARD_LL`, `WH_KEYBOARD`, `WH_MOUSE_LL`, or `WH_CBT` in rc-agent source.
- rc-agent logs "keyboard hook installed" or similar at startup.
- A customer reports iRacing or F1 25 refusing to launch or banning immediately after first session.

**Phase to address:**
Phase 1 of v15.0 (Behavior Audit) — identify all hook calls. Phase 2 (Safe Replacement) — replace before any protected game is enabled for customers. This is a prerequisite, not an optimization.

---

### Pitfall 2: Process Monitor + Taskkill Allowlist Enforcement — Process Injection Suspicion

**What goes wrong:**
The v12.1 E2E Process Guard milestone (currently planned) intends to continuously monitor all processes and auto-kill anything not on the whitelist. This behavior — enumerate running processes, compare against a list, call `TerminateProcess()` on violations — is structurally identical to what anti-cheat-bypass tools do. EAC's kernel driver uses `PsSetCreateProcessNotifyRoutine` (a Windows kernel callback) to track every process creation and termination event. EA Javelin performs stateful VAD scanning and monitors thread creation. A process like rc-agent that calls `CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS)` in a tight loop every few seconds will appear as a process scanner to these systems.

The additional danger: if the allowlist enforcement logic ever calls `OpenProcess(PROCESS_TERMINATE, FALSE, gamePid)` on a game PID that is NOT on the allowlist (because it was just launched and not yet recognized), it will attempt to terminate an anti-cheat-protected process. EAC protects its own game process with `PROCESS_PROTECTION_LEVEL_ANTI_CHEAT_LIGHT` or higher. Attempting `TerminateProcess()` on a protected process will be detected, logged, and potentially trigger an immediate ban.

**Why it happens:**
The allowlist enforcer logic naturally needs to check all processes to find violations. Nobody thinks to ask "can I call `OpenProcess` on a process that happens to be protected by a kernel-mode anti-cheat?"

**How to avoid:**
The v12.1 Process Guard must be gated behind safe mode and must never run while any protected game is active. Specifically:

1. rc-agent must detect game launch (by process name or game handle) and enter safe mode **before** the game's anti-cheat driver initializes (which happens within the first few seconds of game startup).
2. While in safe mode, the allowlist enforcement subsystem is completely suspended — no `CreateToolhelp32Snapshot`, no `OpenProcess` on any PID.
3. Process guard resumes only after the game process has fully exited and a cooldown period of 10 seconds has elapsed (allowing anti-cheat driver to unload).
4. The safe mode flag must be stored in `AppState` with an `AtomicBool` or `Arc<RwLock<SafeModeState>>` so all subsystems can check it without spawning handles.

**Warning signs:**
- v12.1 process guard runs continuously regardless of whether a game is active.
- rc-agent source has a loop that calls `CreateToolhelp32Snapshot` on a fixed timer with no safe mode gate.
- Process guard calls `OpenProcess` without first checking whether the target PID is a known protected game process.

**Phase to address:**
Phase 3 of v15.0 (Auto Safe Mode Implementation) — safe mode must be designed and implemented before v12.1 Process Guard is deployed. Do not deploy v12.1 on pods until safe mode is proven on Pod 8 canary.

---

### Pitfall 3: Safe Mode Timing Race — Anti-Cheat Sees the Hook Before Safe Mode Engages

**What goes wrong:**
rc-agent watches for game process launch to enter safe mode. The watch is implemented as a poll: every N seconds, check if a protected game process is running, if yes, enter safe mode and disable hooks/process monitor. The race condition: EAC initializes its kernel driver within the first 2-5 seconds of the game executable starting. If rc-agent's poll interval is 5 seconds, EAC may have already scanned the system and fingerprinted rc-agent's hooks before rc-agent detects the game and enters safe mode.

The reverse race is equally dangerous: rc-agent detects game exit and immediately leaves safe mode, re-installing hooks and restarting the process guard. But EAC (and especially EA Javelin) does not unload its kernel driver the moment the game exits — it may remain active for 10-30 seconds after game exit, continuing to monitor the system during that window. If rc-agent reinstalls the keyboard hook during this window, EAC can still see it.

**Why it happens:**
Poll-based game detection has inherent latency. The "enter safe mode immediately after detecting game" approach does not account for anti-cheat pre-initialization. The "leave safe mode immediately after game exits" approach does not account for anti-cheat post-game monitoring.

**How to avoid:**
Use WMI event subscription (`Win32_ProcessStartTrace`) or `ReadDirectoryChangesW` on the game's executable directory instead of polling. WMI process start events fire within milliseconds of process creation — before the game has finished its own initialization and before EAC has scanned. This eliminates the entry-side race.

For exit-side race: implement a mandatory 30-second cooldown after game process exit before safe mode is deactivated. During this 30-second window, rc-agent remains in safe mode (hooks off, process guard suspended). After 30 seconds with no game process detected, safe mode deactivates.

Safe mode must also be the **startup default** on pods. If rc-agent starts and detects that a game process is already running (because rc-agent crashed mid-session and restarted), it must enter safe mode immediately before restoring any other subsystem.

**Warning signs:**
- Safe mode detection is implemented as a `tokio::time::interval` poll at 5+ second intervals.
- Safe mode deactivation happens immediately after game exit detection with no cooldown.
- rc-agent startup does not check for already-running game processes during initialization.
- Log shows "entering safe mode" appearing seconds after "game process detected" — gap indicates polling latency.

**Phase to address:**
Phase 3 of v15.0 (Auto Safe Mode) — WMI event subscription approach and the 30-second exit cooldown must be in the design spec before implementation. The startup-default-to-safe-mode requirement must be in the spec explicitly.

---

### Pitfall 4: Shared Memory Telemetry Readers — Benign Use, Malicious Signature

**What goes wrong:**
The planned iRacing and LMU telemetry adapters read data from memory-mapped files (`iRacing` SDK uses a named shared memory file, rFactor 2/LMU uses a plugin with shared memory). Reading memory-mapped files is legitimate and the iRacing SDK explicitly supports third-party telemetry readers via this mechanism. However, the risk is in **how** the reader is implemented. The wrong approach is to use `ReadProcessMemory()` (which reads another process's virtual address space directly) — this is the same API that cheat tools use and is detected by all major anti-cheat systems.

The correct approach (OpenFileMapping + MapViewOfFile on a named shared memory object) is explicitly supported by the iRacing SDK and is not the same as ReadProcessMemory. The pitfall is implementing it incorrectly — especially if a developer reaches for `ReadProcessMemory` because they're more familiar with it, or if a third-party crate abstracts both methods under the same interface.

A secondary risk: the telemetry reader should only be active while in safe mode bypass state for telemetry (i.e., only activate after the game has signaled it is ready to share data). For iRacing specifically, the shared memory header contains a session status field. Reading shared memory before the game has initialized it returns garbage, and a tight loop doing so can produce high-frequency memory mapping calls that look suspicious.

**Why it happens:**
`ReadProcessMemory` is the first search result when developers look up "read another process memory Windows." The distinction between ReadProcessMemory (process address space, requires PROCESS_VM_READ handle) and MapViewOfFile (named shared memory, no handle to game process required) is not obvious. Getting it wrong triggers EAC/Javelin.

**How to avoid:**
Use `OpenFileMapping` + `MapViewOfFile` on the named shared memory objects:
- iRacing: `"Local\\IRSDKMemMapFileName"` — explicitly documented in iRacing SDK
- rFactor 2/LMU: `"$rFactor2SMMP_HWControlInput$"` and related named objects — documented in rF2 plugin SDK

Never call `ReadProcessMemory`, `WriteProcessMemory`, or `OpenProcess(PROCESS_VM_READ, ...)` on any game process PID. The telemetry reader must be a pure shared-memory consumer with no handle to the game process itself.

Gate telemetry activation: only open the shared memory mapping after verifying the iRacing session status header shows `irsdk_stReady` or equivalent. For LMU, wait for the plugin-published "simulation active" flag. This prevents the high-frequency mapping loop during game initialization.

**Warning signs:**
- Telemetry reader implementation uses `ReadProcessMemory` or requires obtaining a handle to the game PID.
- Telemetry reader activates as soon as rc-agent starts, regardless of whether a game is running.
- A Rust crate being used for shared memory access requires `OpenProcess` as a prerequisite.

**Phase to address:**
Phase 4 of v15.0 (Shared Memory Telemetry Gating) — review all telemetry adapter implementations for ReadProcessMemory vs MapViewOfFile distinction before any adapter is deployed to pods. This applies to both iRacing (Phase within v13.0) and LMU adapters.

---

### Pitfall 5: Unsigned rc-agent.exe and rc-sentry.exe — Kernel-Level Visibility Risk

**What goes wrong:**
EA Javelin (F1 25) runs a kernel driver that profiles process behavior. An unsigned executable performing keyboard hooking, process enumeration, and network listening is a stronger behavioral match to cheat tools than a signed, identified executable doing the same things. Code signing does not make anti-cheat ignore the behavior — but it provides a second signal that moderates the risk score. More critically: Windows Defender SmartScreen and EAC may warn or block at launch when an unsigned binary is detected running alongside a protected game, especially if the binary's behavior profile is unusual (network listeners, hooks).

The actual high-severity scenario: EA Javelin builds a hardware fingerprint (motherboard serial, disk ID, MAC, GPU) for each machine. If rc-agent's unsigned binary + keyboard hook behavior triggers a false positive report for F1 25 on one pod, **all 8 pods share the same hardware profile** from a customer account perspective. A ban on Pod 1 can propagate to the customer's account and follow them to other venues. This is not a recoverable situation short of EAC appeal.

**Why it happens:**
Code signing certificates cost money (~$200-500/year for OV certificates, more for EV). Developers defer "we'll sign it later." Meanwhile, the unsigned binary ships to production and runs alongside anti-cheat protected games.

**How to avoid:**
Obtain a code signing certificate before v15.0 ships. Microsoft's Trusted Signing service launched in 2024 provides a lower-cost path (approximately $9.99/month) for signing Windows executables. Both `rc-agent.exe` and `rc-sentry.exe` must be signed before any anti-cheat protected game is enabled for customer play. The signing step must be part of the build pipeline — not a manual step done before deployment — otherwise unsigned binaries will be deployed when the manual step is forgotten.

Additionally, sign the binaries with a Subject that includes the company name ("Racing Point eSports" or similar) so that if a report is investigated by an anti-cheat team, the binary has an identifiable legitimate purpose.

**Warning signs:**
- `rc-agent.exe` is deployed to pods without a digital signature (check with `Get-AuthenticodeSignature .\rc-agent.exe` in PowerShell).
- Build pipeline does not include a `signtool.exe` step.
- v15.0 milestone begins without a certificate purchase order in place.

**Phase to address:**
Phase 1 of v15.0 (Behavior Audit) — certificate procurement timeline. Phase 2 (Safe Replacement) — signing integrated into build pipeline. v15.0 must not reach canary testing on Pod 8 without signed binaries.

---

### Pitfall 6: HKLM Run Key Modification — Persistence Pattern Detection

**What goes wrong:**
rc-agent currently uses `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run\RCAgent` to start `start-rcagent.bat` at system boot. Modifying HKLM Run keys is a well-known persistence technique (MITRE ATT&CK T1547.001) and is monitored by both anti-cheat systems and Windows Defender. The specific risk: if rc-agent's self-healing logic repairs a missing Run key entry while a game is running, the registry write happens while the anti-cheat kernel driver is active and watching for registry persistence attempts.

EA Javelin and EAC both monitor for run key writes as indicators of persistent cheat installation. While the Run key was set up at deployment time, any runtime modification (repair, update) during an active game session triggers the same detection signature.

**Why it happens:**
rc-agent's self-healing (from v4.0 HEAL-01/HEAL-04) writes the Run key if it finds it missing. This is correct behavior for reliability, but must not occur while a game is running. Developers implementing the self-healing don't think to check the safe mode state before performing a registry write.

**How to avoid:**
All registry write operations in rc-agent must be gated behind the safe mode check. Specifically:
- Run key repair: allowed only when no game is running (outside safe mode).
- Any registry write under `HKLM` or `HKCU`: must fail gracefully and schedule a retry for after game exit if in safe mode.
- The USB mass storage lockdown (Group Policy) must be applied once at deployment and never re-applied at runtime during a game session.

USB lockdown via Group Policy writes to `HKLM\SYSTEM\CurrentControlSet\Services\USBSTOR` (setting `Start=4` to disable the driver). This is exactly the kind of system-level registry modification that anti-cheat scanners watch for. Apply it once at pod setup, never again at runtime.

**Warning signs:**
- rc-agent's self-healing code does not check safe mode state before calling registry write APIs.
- Log shows "repaired Run key" events occurring during billing sessions (which may coincide with game sessions).
- USB lockdown enforcement is re-applied on every rc-agent startup rather than being idempotent at first run only.

**Phase to address:**
Phase 1 of v15.0 (Behavior Audit) — catalog all registry write paths. Phase 3 (Safe Mode) — gate all registry writes behind safe mode check.

---

### Pitfall 7: Multiple Port Listeners as Network Hooking Suspicion

**What goes wrong:**
rc-agent maintains three listening sockets: `:8090` (HTTP remote_ops), `:18923`, and `:18925`. Additionally, the telemetry UDP listeners (`:9996` for AC, `:20777` for F1, `:6789` for iRacing, `:5555` for LMU) must be open while games are running. This is 7 open listening ports active during a game session. EA Javelin's kernel driver monitors network activity and has behavioral heuristics for processes with unusual listener profiles (multiple ports, raw socket access, or high-frequency UDP reads). While having listening ports is not itself a ban trigger, the combination of: unsigned binary + keyboard hook + multiple ports + process enumeration creates a composite risk score that is more likely to trip EAC/Javelin heuristics than any single behavior.

The specific risk for UDP telemetry: a tight UDP read loop on `:20777` (F1 25 telemetry) that runs at 60Hz (reading 16KB+ per second) while F1 25 + EA Javelin is running looks like a game data exfiltration channel from the anti-cheat's perspective.

**Why it happens:**
Each listener was added for a legitimate purpose (HTTP for fleet management, UDP for telemetry). Nobody considered the combined network listener profile as an anti-cheat risk factor.

**How to avoid:**
The HTTP remote_ops listener (`:8090`) is necessary for fleet management and cannot be removed. The UDP telemetry listeners can be scoped:
- Open UDP telemetry port only when the corresponding game is actively running and in safe mode bypass state for telemetry.
- Close the telemetry UDP socket when the game exits (within the safe mode exit cooldown).
- Do not pre-open telemetry ports at rc-agent startup "just in case" a game might be launched later.

The port listener profile combined with other risk behaviors should be addressed by eliminating the high-risk behaviors first (keyboard hook, process enumeration). The listeners alone — especially if the binary is signed and the other high-risk behaviors are absent — are unlikely to be a standalone ban trigger.

**Warning signs:**
- UDP telemetry sockets are bound at rc-agent startup regardless of which game is running.
- rc-agent holds 5+ UDP listeners open simultaneously during a racing session.
- Telemetry reader does not close its socket when the game exits.

**Phase to address:**
Phase 4 of v15.0 (Shared Memory Telemetry Gating) — telemetry socket lifecycle tied to game state. Also addressed by eliminating the keyboard hook (reduces composite risk score even if port profile stays the same).

---

### Pitfall 8: Ollama on Pods — GPU Memory Contention and Anti-Cheat Profiling Conflict

**What goes wrong:**
v8.0 deployed Ollama (`qwen3:0.6b`) on all 8 pods for the LLM process classifier. Ollama occupies GPU VRAM when a model is loaded (qwen3:0.6b is ~600MB of VRAM). Racing games, especially F1 25 and AC EVO, also require significant VRAM. On pods with RTX 3060/3070 class cards (8-12GB VRAM), this may not be a direct crash risk but will affect frame rates. More relevant to anti-cheat: Ollama starts an HTTP server on `:11434` by default and uses CUDA/cuDNN libraries that make syscalls to GPU memory via the NVIDIA driver. These CUDA syscalls from a non-game process, while a game is running, can trigger GPU monitoring heuristics in anti-cheat systems that profile GPU memory access patterns for cheats.

The specific scenario: a customer session where rc-agent queries local Ollama (for process classification) mid-game will cause Ollama to load or query the model, generating GPU CUDA activity from a non-game process during active gameplay with anti-cheat running.

**Why it happens:**
Ollama was deployed for background LLM inference tasks (process classification). Nobody considered that GPU CUDA syscalls from Ollama would be visible to kernel-mode anti-cheat monitoring GPU activity.

**How to avoid:**
Ollama must not be queried on pods while an anti-cheat protected game is running. Specifically:
- LLM-based process classification (dynamic kiosk allowlist from v8.0) must be suspended in safe mode.
- Ollama's HTTP server on pods should bind to `127.0.0.1` only (already the default), not `0.0.0.0`.
- Consider setting `OLLAMA_KEEP_ALIVE=0` on pods so the model unloads from VRAM immediately after each query, freeing GPU memory for the game.
- The Ollama service on pods does not need to be stopped during gameplay — but no queries should be made to it while a protected game is running.

The James `.27` Ollama (`:11434`) is used for Tier 3 crash analysis and is remote to pods — this is safe (network call to a different machine, no local GPU interaction).

**Warning signs:**
- rc-agent's process classification loop queries local Ollama regardless of whether a game is running.
- `OLLAMA_KEEP_ALIVE` is not set on pods, leaving the model loaded in VRAM during gameplay.
- Pods have less than 8GB of VRAM available after Ollama model load.

**Phase to address:**
Phase 3 of v15.0 (Auto Safe Mode) — suspend LLM process classification queries in safe mode. Phase 4 — evaluate `OLLAMA_KEEP_ALIVE=0` setting for pods.

---

### Pitfall 9: ConspitLink Third-Party Software — Unsigned Wheel Driver Conflict

**What goes wrong:**
ConspitLink (the Conspit Ares wheelbase management software) runs on every pod as a mandatory hardware interface. Like rc-agent, ConspitLink is a third-party, potentially unsigned binary that communicates with the HID device (USB VID:0x1209 PID:0xFFB0). Anti-cheat systems that scan the process list for suspicious software (both EAC and EA Javelin do this) may flag ConspitLink if: (a) it is unsigned, (b) it has a kernel-mode driver component that conflicts with the anti-cheat driver, or (c) it uses WinUSB/HID API calls in patterns that look like a peripheral manipulation tool (which some cheats use to inject fake mouse/keyboard input).

The Conspit Ares being an OpenFFBoard-based wheelbase means it uses standard HID gamepad drivers. HID input from a gamepad is not a classic cheat vector and is generally allowlisted by anti-cheat. However, the force feedback side — where ConspitLink writes FFB data to the device — involves Windows `WriteFile` calls on the HID endpoint. These writes from an unsigned binary during active gameplay are a potential heuristic trigger.

**Why it happens:**
ConspitLink is shipped by the hardware vendor and its signing status is outside Racing Point's control. Nobody checks whether third-party peripheral software is signed or on anti-cheat allowlists.

**How to avoid:**
1. Check ConspitLink's signing status: `Get-AuthenticodeSignature ConspitLink.exe` in PowerShell. If unsigned, contact Conspit to request a signed build or check if a signed version is available.
2. Verify ConspitLink's driver: `Get-WindowsDriver -Online | Where-Object {$_.Driver -like '*conspit*'}`. If a custom kernel driver is installed, it must be WHQL-signed to avoid anti-cheat kernel driver conflicts.
3. For iRacing specifically: iRacing's forums have documented FFB driver compatibility. Check whether OpenFFBoard-based wheelbases are on iRacing's tested hardware list.
4. If ConspitLink is unsigned and cannot be signed: file whitelisting requests with EAC (https://www.easy.ac/en-US/) and EA's developer portal. Commercial venues can request allowlisting for management software.

**Warning signs:**
- `Get-AuthenticodeSignature ConspitLink.exe` returns "NotSigned" or "HashMismatch".
- iRacing or F1 25 crashes or bans on first session — ConspitLink was the only non-standard process running.
- ConspitLink installs a kernel-mode driver that appears in Device Manager under a non-standard vendor.

**Phase to address:**
Phase 1 of v15.0 (Behavior Audit) — audit ConspitLink signing status and driver components. Phase 5 (Per-Game Validation) — test ConspitLink compatibility on Pod 8 canary before rolling out.

---

### Pitfall 10: Safe Mode Incomplete Coverage — "I Disabled the Obvious Things" Failure

**What goes wrong:**
Safe mode is implemented to disable the keyboard hook and process guard. Testing shows no immediate ban. The implementation ships. Six weeks later, a customer gets banned during an iRacing session because the self-healing code repaired a missing registry Run key mid-session (Pitfall 6), or because a UDP telemetry socket was still open (Pitfall 7), or because the Ollama process classifier fired once during the game (Pitfall 8). The ban was caused by a behavior that was not on the safe mode checklist.

This is the "I disabled the obvious things" failure mode: safe mode addresses the behaviors the developer was aware of, but misses edge cases, background subsystems, and interactions between behaviors that individually look safe but combine to trip an anti-cheat heuristic.

**Why it happens:**
Safe mode checklists are built from the known-risky behaviors at design time. Behaviors added in earlier milestones (v4.0 self-healing, v8.0 LLM classifier, v12.1 process guard) are not automatically reviewed against the safe mode checklist.

**How to avoid:**
Build safe mode as a positive-enable allowlist, not a negative-disable blocklist. In safe mode, rc-agent's behavior is restricted to only the explicitly approved operations:
- Health endpoint polling (HTTP GET localhost:8090/health)
- WebSocket keepalive ping/pong to racecontrol server
- Billing heartbeat (read-only, no process inspection)
- UDP telemetry receive (only the port for the active game, read-only)
- Session timer tick

Everything else is off by default in safe mode. Behaviors are re-enabled explicitly when:
1. They are proven safe with anti-cheat (tested on Pod 8).
2. They are gated behind safe mode exit cooldown.

Create a `safe_mode_subsystems` config section in racecontrol.toml that explicitly lists which subsystems are enabled in safe mode. This makes the allowlist visible, versioned, and auditable.

**Warning signs:**
- Safe mode is implemented as a series of `if !safe_mode { do_risky_thing() }` guards scattered across modules.
- No centralized safe mode subsystem registry exists.
- A code review of safe mode does not immediately reveal the complete list of behaviors disabled.
- A new subsystem is added in a later milestone without explicitly documenting its safe mode behavior.

**Phase to address:**
Phase 3 of v15.0 (Auto Safe Mode) — design the allowlist model from the start. Phase 6 (Validation Matrix) — before each game is approved for customer use, run the full subsystem checklist against that game's anti-cheat.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Keep `SetWindowsHookEx`, just disable it in safe mode | Faster to gate than replace | Hook is installed/uninstalled dynamically — anti-cheat sees the install event at game launch if timing is wrong | Never — full replacement required, not gating |
| Poll for game process at 5s intervals for safe mode detection | Simple implementation | 5-second window where anti-cheat is active before safe mode engages — high risk on fast machines | Never — WMI event subscription required for entry-side |
| Skip code signing until "later" | Saves ~$120/year | Unsigned binary + hooks = highest composite risk score; cannot be recovered after first ban | Never — certificate is a prerequisite for v15.0 |
| Apply safe mode exit after 5 seconds instead of 30 | Faster recovery of kiosk features | EA Javelin kernel driver stays active 10-30s post-game; hooks re-installed during this window | Only if confirmed via Pod 8 testing that specific game's AC unloads faster |
| Leave Ollama running at full VRAM during games | Simpler ops | VRAM contention + GPU CUDA activity from non-game process during anti-cheat monitoring | Acceptable only if VRAM available > game requirement + 1GB buffer |
| Use `ReadProcessMemory` for telemetry because it's simpler | Easier initial implementation | Immediate EAC detection — `ReadProcessMemory` on game PID is a hard ban trigger | Never — `MapViewOfFile` on named shared memory is the correct approach |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| iRacing EOS anti-cheat + shared memory | Open shared memory mapping before iRacing session starts | Check `irsdk_isConnected` header field; only read when `irsdk_stReady` state is active |
| EAC-EOS (LMU/iRacing) + process list read | Call `CreateToolhelp32Snapshot` for process monitoring during game session | Full suspension of any process enumeration; use WMI start event subscription instead for game detection |
| EA Javelin (F1 25) + keyboard hook | Disable hook when game is detected running | Remove hook entirely; replace with policy-based key blocking that does not use SetWindowsHookEx |
| EA Javelin + registry write | Write Run key repair while game is running | Gate all registry writes behind safe mode; defer repairs to post-game exit |
| ConspitLink (wheelbase) + anti-cheat | Assume HID peripheral software is always allowed | Verify ConspitLink signing status; check for kernel driver components; request EAC allowlisting if unsigned |
| Ollama local LLM on pod + GPU game | Query Ollama for process classification mid-game | Suspend all Ollama queries in safe mode; set `OLLAMA_KEEP_ALIVE=0` to prevent VRAM reservation |
| Safe mode exit timing + anti-cheat driver | Exit safe mode immediately after game process terminates | 30-second cooldown after game exit before safe mode deactivates |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| UDP telemetry socket open at startup for all games | 5+ UDP sockets bound simultaneously during gameplay | Open telemetry socket only for the active game on launch, close on exit | Any game session — composite port listener profile raises anti-cheat risk score |
| Ollama model loaded in VRAM during entire customer session | Frame rate drops on games with high VRAM requirements; potential CUDA contention | Set `OLLAMA_KEEP_ALIVE=0`; model unloads immediately after each query | Pods with < 10GB VRAM when running F1 25 or AC EVO |
| WMI game detection subscription never cleaned up | Memory leak in WMI subscription after rc-agent runs for days | Unsubscribe from WMI event on game exit; verify subscription count stays at 1 | After rc-agent restarts several times without full process restart |
| Safe mode check as a deep-stack function call | Code in subsystems calls a remote async check to determine safe mode state | Store safe mode as `AtomicBool` in AppState — O(1) read with no async overhead | Any subsystem that needs to check safe mode in a tight loop |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Shipping unsigned rc-agent.exe alongside kernel-mode anti-cheat | Customer ban, venue reputation damage, hardware fingerprint ban propagating across pods | Code signing certificate obtained before v15.0 deployment |
| Reporting ban to customer as "the game crashed" | Confusion, failed appeal timeline, permanent account loss | Establish a ban response protocol: immediate Uday alert, game suspended on that pod, written incident report for customer EAC appeal |
| No canary testing per game before customer rollout | First real customer session discovers ban-triggering behavior | Pod 8 canary test session per game (staff only) before enabling that game for customers |
| No anti-cheat compatibility matrix maintained | Future code changes break previously-working safe mode | Keep `docs/anti-cheat-matrix.md` updated — every subsystem listed with its safe mode status per anti-cheat system |

---

## "Looks Done But Isn't" Checklist

- [ ] **Keyboard hook replacement:** Verify `SetWindowsHookEx` does not appear anywhere in rc-agent source. Search for `WH_KEYBOARD_LL`, `WH_KEYBOARD`, `SetWindowsHookEx`, `UnhookWindowsHookEx` in all Rust and script files.
- [ ] **Code signing:** Run `Get-AuthenticodeSignature rc-agent.exe` on a freshly built Pod 8 binary. Confirm it returns "Valid" not "NotSigned".
- [ ] **Safe mode entry timing:** Start a test game on Pod 8 and watch logs. Confirm "entering safe mode" log appears BEFORE the game process has been running for 3 seconds (WMI event path, not poll path).
- [ ] **Safe mode exit cooldown:** Kill test game on Pod 8. Confirm "exiting safe mode" log does not appear for at least 30 seconds after game process death.
- [ ] **Process guard suspended in safe mode:** In safe mode, verify no `CreateToolhelp32Snapshot` calls appear in process monitor while game is running.
- [ ] **Registry write suspended in safe mode:** Force a Run key deletion while game is running. Verify rc-agent schedules the repair for post-game, not immediately.
- [ ] **UDP telemetry socket lifecycle:** Verify only the telemetry port for the active game is bound during a session. No stale sockets from other games are open.
- [ ] **ConspitLink signing:** Run `Get-AuthenticodeSignature ConspitLink.exe`. If NotSigned, do not proceed to anti-cheat validation testing without EAC allowlist request filed.
- [ ] **Ollama suspended in safe mode:** During a test game session, confirm no Ollama HTTP requests are made to `:11434`. Check Ollama access log on the pod.
- [ ] **iRacing telemetry uses MapViewOfFile:** Code review confirms no `ReadProcessMemory` or `OpenProcess(PROCESS_VM_READ)` in any telemetry adapter.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Customer receives EAC ban during session | HIGH | Immediately suspend that game on all pods; document the rc-agent behavior active at ban time; submit EAC appeal at easy.ac with timeline; do not re-enable game until root cause identified and fixed |
| EA Javelin ban (F1 25) | HIGH | Same as EAC — suspend F1 25 across all pods; EA Javelin appeals at help.ea.com; note that EA Javelin's reported 99%+ accuracy means false positive appeals are more likely to succeed than EAC appeals |
| iRacing account ban | HIGH | iRacing operates its own anti-cheat (EOS variant); ban affects customer's existing iRacing subscription; appeal via iRacing support with venue documentation |
| Safe mode race condition (hook installed before safe mode) | MEDIUM | Increase safe mode detection sensitivity (poll at 1s until WMI event subscription is implemented); accept false positives (safe mode fires for non-protected games) rather than missing the window |
| ConspitLink flagged by anti-cheat | MEDIUM | Fallback: run game without ConspitLink active (keyboard/mouse input only); contact Conspit support for signed build or EAC allowlisting |
| Code signed but certificate expired | LOW | Certificate expiry does not retroactively invalidate signed binaries if timestamp countersigned; but new builds will be unsigned — renew certificate before expiry; add calendar reminder 30 days before expiry |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| SetWindowsHookEx keyboard hook | v15.0 Phase 2 — Safe Replacement | `grep -r "SetWindowsHookEx"` returns zero results in rc-agent source |
| Process guard + taskkill alongside game | v15.0 Phase 3 — Auto Safe Mode | Process guard loop has safe mode gate; verified via log analysis during test game session |
| Safe mode timing race (entry + exit) | v15.0 Phase 3 — Auto Safe Mode | WMI event subscription, 30s cooldown, startup default verified on Pod 8 canary |
| Shared memory telemetry using ReadProcessMemory | v15.0 Phase 4 — Telemetry Gating | Code review: zero `ReadProcessMemory` calls in any adapter; telemetry socket lifecycle tied to game state |
| Unsigned rc-agent.exe / rc-sentry.exe | v15.0 Phase 1 — Audit (cert procurement) | `Get-AuthenticodeSignature` returns Valid on all deployed binaries before any game testing begins |
| HKLM registry writes during game session | v15.0 Phase 3 — Auto Safe Mode | Registry write code audited; all writes gated; USB lockdown is one-time-at-setup only |
| Multiple port listeners composite risk | v15.0 Phase 4 — Telemetry Gating | UDP sockets opened/closed per game; listener count during session is 1 HTTP + 1 UDP max |
| Ollama CUDA activity during game + AC | v15.0 Phase 3 — Auto Safe Mode | No Ollama requests during test game session confirmed via Ollama access log |
| ConspitLink unsigned / driver conflict | v15.0 Phase 1 — Audit + Phase 5 Validation | Signing status checked; EAC allowlist request filed if needed; tested on Pod 8 |
| Safe mode incomplete coverage | v15.0 Phase 3 — Auto Safe Mode design | Allowlist model documented in code; all subsystems have explicit safe mode behavior |

---

## Per-Game Anti-Cheat Risk Reference

| Game | Anti-Cheat | AC Type | Keyboard Hook Risk | Process Scan Risk | Shared Memory Risk | Registry Write Risk |
|------|------------|---------|-------------------|------------------|-------------------|---------------------|
| F1 25 | EA Javelin | Kernel-mode, continuous scan | CRITICAL — Javelin monitors input hooks | HIGH — process profiling active | MEDIUM — no shared memory needed | HIGH — Javelin watches persistence APIs |
| iRacing | EOS (Epic EAC variant) | Kernel-mode | HIGH — same EAC-EOS kernel driver as other EAC games | HIGH — EAC enumerates process list | LOW — iRacing SDK explicitly supports named shared memory readers | MEDIUM — EAC watches run key modifications |
| LMU (rFactor2-based) | EasyAntiCheat | Kernel-mode (EAC standard) | HIGH — same risk as any EAC game | HIGH | LOW — rF2 shared memory plugin is official and supported | MEDIUM |
| AC EVO (Kunos) | None currently | User-mode at best (per community reports as of early 2026) | LOW — no kernel driver to trigger | LOW | LOW | LOW |
| EA WRC | EasyAntiCheat | Kernel-mode (EAC standard) | HIGH | HIGH | MEDIUM — no official shared memory API | MEDIUM |

Note on AC EVO confidence: The absence of anti-cheat in AC EVO is based on community reports (LOW confidence) — Kunos may add anti-cheat in a future update during Early Access. Treat AC EVO as the safest game for initial testing, but do not assume permanent safety.

---

## Sources

- [EA Javelin Anti-Cheat Progress Report](https://www.ea.com/security/news/anticheat-progress-report) — EA official (HIGH confidence — confirms hundreds of specific detections, kernel-mode operation, internal and external cheat detection)
- [EA Javelin Anti-Cheat Installation Guide](https://help.ea.com/en/articles/platforms/pc-ea-anticheat/) — EA official (HIGH confidence — confirms kernel driver, F1 25 uses Javelin)
- [iRacing Anti-Cheat Migration to EOS](https://support.iracing.com/support/solutions/articles/31000173103-anticheat-not-installed-uninstalling-eac-and-installing-eos-) — iRacing official support (HIGH confidence — confirms iRacing migrated from EAC to EOS)
- [iRacing 2024 Season 2 Patch 4 Anti-cheat update](https://support.iracing.com/support/solutions/articles/31000173098-2024-season-2-patch-4-release-notes-2024-05-01-02-) — iRacing official (HIGH confidence — confirms EOS rollout completed May 2024)
- [LMU EasyAntiCheat implementation](https://steamcommunity.com/app/2399420/discussions/0/693122391565596967/) — Steam community, developer response (MEDIUM confidence — confirms LMU uses EAC)
- [LMU V1.2 Anti-Cheat Improvement](https://simulationdaily.com/news/le-mans-ultimate-v1-2-update/) — Simulation Daily (MEDIUM confidence — confirms active EAC improvement December 2025)
- [AC EVO anti-cheat community discussion](https://steamcommunity.com/app/3058630/discussions/0/756141976595742745/) — Steam community (LOW confidence — community reports only, no official Kunos confirmation)
- [If It Looks Like a Rootkit and Deceives Like a Rootkit](https://dl.acm.org/doi/fullHtml/10.1145/3664476.3670433) — ACM academic paper 2024 (HIGH confidence — peer-reviewed analysis of kernel anti-cheat detection methods including VAD scanning, process callbacks, and handle monitoring)
- [EAC Kernel Driver Incompatibility](https://learn.microsoft.com/en-us/answers/questions/3962392/easy-anti-cheat-driver-incompatible-with-kernel-mo) — Microsoft Learn (HIGH confidence — confirms EAC kernel-mode scope and driver loading behavior)
- [How Easy Anti-Cheat Actually Works](https://tateware.com/blog/easy-anti-cheat-how-it-works.html) — TateWare technical analysis 2026 (MEDIUM confidence — confirms memory scanning, hardware fingerprinting, driver monitoring)
- [DisplayFusion anti-cheat hook disabling](https://www.displayfusion.com/Discussions/View/disable-all-hooksaccess-primarily-because-of-game-anticheat/?ID=289100c7-e6d6-49a9-8a49-5578b2079f55) — Binary Fortress official support (HIGH confidence — confirms anti-cheat visibility of SetWindowsHookEx from legitimate background apps; confirms per-process hook exemption approach)
- [DisplayFusion anti-cheat game folder scanning improvement](https://www.displayfusion.com/ChangeLog/) — Binary Fortress changelog (HIGH confidence — confirms DisplayFusion had to implement game detection to avoid hooking game processes)
- [iRacing SDK shared memory documentation](https://sajax.github.io/irsdkdocs/) — iRacing SDK docs (HIGH confidence — confirms named shared memory `Local\\IRSDKMemMapFileName`, explicitly supports third-party readers)
- [Cheat Engine detection by EAC](https://forum.cheatengine.org/viewtopic.php?t=610934) — Cheat Engine community (MEDIUM confidence — confirms EAC detects OpenProcess-based memory reading tools)
- [EAC false positive on ASUS Aura Sync driver](https://steamcommunity.com/discussions/forum/0/3874842132568325313/) — Steam community (MEDIUM confidence — confirms legitimate background apps with kernel components trigger EAC)
- [Microsoft Trusted Signing service](https://support.microsoft.com/en-us/topic/kb5022661-windows-support-for-the-trusted-signing-formerly-azure-code-signing-program-4b505a31-fa1e-4ea6-85dd-6630229e8ef4) — Microsoft official (HIGH confidence — confirms Azure Trusted Signing as lower-cost code signing path, ~$9.99/month)
- [MITRE ATT&CK T1547.001 Run Key Persistence](https://attack.mitre.org/techniques/T1547/001/) — MITRE ATT&CK (HIGH confidence — confirms anti-cheat and security tools monitor HKLM Run key modifications as persistence indicator)
- Direct codebase knowledge: rc-agent Phase 78 keyboard hook, v4.0 self-healing Run key repair, v8.0 Ollama LLM classifier, v12.1 Process Guard design, PROJECT.md v15.0 milestone constraints

---
*Pitfalls research for: Anti-Cheat Compatibility — adding safe mode to existing game management software on 8 sim racing pods*
*Researched: 2026-03-21 IST*
