# Feature Research: v15.0 AntiCheat Compatibility

**Domain:** Anti-cheat safe mode and compatibility hardening for a sim racing venue management agent running alongside EAC, EOS, EA AntiCheat, and Kunos-protected games.
**Researched:** 2026-03-21
**Confidence:** MEDIUM (WebSearch + official EAC/EA docs + sim racing community sources; specific per-game ban triggers are partially opaque by design — anti-cheat vendors deliberately avoid publishing full detection lists)

---

## Context: The Problem This Milestone Solves

rc-agent currently runs on all 8 pods with behaviors that overlap with known cheat tooling:

| Existing rc-agent Behavior | Cheat Tooling Analog | Anti-Cheat Risk |
|---------------------------|---------------------|-----------------|
| `SetWindowsHookEx` WH_KEYBOARD_LL global hook | AutoHotKey, keyloggers | HIGH — EAC flags AHK at kernel level; Discord overlay-style hooks also trigger |
| Process allowlist enforcement (monitors + kills processes) | Process monitors, sandbox escape detectors | MEDIUM — killing processes by PID requires `OpenProcess`; EAC scans for open handles to game process |
| USB mass storage lockdown via Group Policy | USB-based cheat injectors | LOW — Group Policy is OS-level, not detectable as a cheat tool |
| Registry modifications (HKLM Run key, Edge hardening) | Persistence mechanisms | LOW — applying at startup, not during gameplay |
| Shared memory telemetry reads (iRacing irsdk, rF2 shared mem) | Memory readers | VARIES — iRacing explicitly permits SDK reads; EAC-protected games depend on how the shared mem is accessed |
| Unsigned rc-agent.exe and rc-sentry.exe binaries | Unsigned cheat tools | MEDIUM — EAC does not ban based on unsigned binaries alone, but unsigned status reduces trust with security software and increases scan scrutiny |
| ConspitLink wheelbase software running during games | Third-party overlays, HID injectors | MEDIUM — depends entirely on what ConspitLink does: shared memory reads and standard HID are safe; process injection is not |

The milestone goal: customers must not be banned for playing F1 25, iRacing, LMU, AC EVO, or EA WRC on RaceControl-managed pods.

---

## Per-Game Anti-Cheat System Reference

Understanding what each game uses is the prerequisite for knowing what triggers are relevant.

| Game | Anti-Cheat System | Kernel Level? | Active When | Notes |
|------|------------------|--------------|-------------|-------|
| F1 25 | EA AntiCheat (EAAC) | YES — kernel-mode driver | Game running | Replaced EAC in EA sports titles mid-2024. Full stack ownership by EA. Blocks unsigned drivers. Crashes game on detected violation. |
| iRacing | Epic Online Services (EOS) anti-cheat | YES | Game running | Switched from EAC to EOS in 2024 S2 Patch 4. Runs iRacing inside a sandbox — blocks external process memory access to simulation. Official telemetry SDK (shared memory) is explicitly permitted. |
| LMU (Le Mans Ultimate) | Easy Anti-Cheat (EAC) | YES — kernel-mode driver | Multiplayer sessions (single player reportedly less enforced) | EAC shipped from LMU v1.2 onwards. Blocks write-access to memory. Crashes game if interference detected. |
| AC EVO (Assetto Corsa EVO) | Unknown / early access — no confirmed AC system as of 2026-03-21 | UNKNOWN | Unknown | EA v0.5.4 (Jan 2026): shared memory improved to match ACC. No public anti-cheat announcement. Likely light or no enforcement in Early Access. Low risk currently, may change at full release. |
| EA WRC | EA AntiCheat (EAAC) | YES — kernel-mode driver | Game running | Same as F1 25. EA anticheat implemented from WRC v1.9.0 (June 2024). Shuts down when game exits. |

---

## Feature Landscape

### Table Stakes (Must Build — Missing = Customer Accounts Get Banned)

Features that must exist before v13.0 Multi-Game Launcher deploys to customers. Without these, running
any EAC/EAAC-protected game on a RaceControl-managed pod risks a ban.

| # | Feature | Why Expected | Complexity | Existing Dependency |
|---|---------|--------------|------------|---------------------|
| TS-1 | **Protected game detection in rc-agent** | Every safe-mode behavior depends on knowing a protected game is running. rc-agent must detect game launch (already monitors game processes) and additionally identify which anti-cheat system is active. Detection: watch for the anti-cheat process or check the game executable name against a per-game config. | LOW | `game_process.rs` already tracks running game. Add `anti_cheat_tier` field to `GameProfile` in TOML: `none / eac / eaac / eos`. |
| TS-2 | **Auto safe mode activation: disable risky subsystems on protected game launch** | When a protected game starts, rc-agent must automatically disable all behaviors that overlap with cheat tooling. This is the central "safe mode" concept. Activation must be automatic — staff cannot be expected to toggle it manually per session. | MEDIUM | New `SafeModeState` enum in `app_state.rs`. `Normal` → `SafeMode { game: GameId, anti_cheat: AntiCheatTier }` on game launch. All risky subsystems check this flag before acting. |
| TS-3 | **Keyboard hook suspension during protected game sessions** | SetWindowsHookEx WH_KEYBOARD_LL is a known EAC detection trigger. AutoHotKey uses the same mechanism and gets banned in games like Rust, Fortnite, and F1 via EAC/EAAC. The current hook blocks Win key, Alt+Tab, etc. This MUST be suspended while a protected game runs. Kiosk lockdown must be re-established via a safer mechanism. | MEDIUM | `keyboard_hook.rs` (Phase 78). Add `suspend()` / `resume()` methods callable from the safe mode state machine. Resume only after game process exits and anti-cheat shuts down. |
| TS-4 | **Replace keyboard hook with policy-based kiosk lockdown** | Suspending the hook means Win key and Alt+Tab become active during gameplay — players could exit the game, access the desktop, open Task Manager. The replacement must block these keys without using SetWindowsHookEx. Two approaches: (a) Windows Keyboard Filter (IoT Enterprise feature — not available on standard Win 11 Pro), (b) Group Policy `DisableTaskMgr` + `NoWinKeys` registry values (safe, no hooks, process-agnostic). | HIGH | Keyboard Filter is NOT available on Windows 11 Pro (requires IoT Enterprise LTSC). Registry-based Group Policy values are the viable path for the current pod OS. These survive anti-cheat scrutiny because they are OS-level policy, not user-mode injection. Requires testing: GPO keys applied per-session may not take effect without user logoff. |
| TS-5 | **Process allowlist enforcement gated behind safe mode** | rc-agent's process monitor kills non-whitelisted processes. Killing a process by PID requires `OpenProcess` internally (via taskkill /PID or equivalent). EAC scans for open handles to the game process. If the game PID ever appears in a kill operation, it could be flagged. During safe mode: process killing must stop entirely or restrict to rc-agent/rc-sentry only (never touch game PIDs). | MEDIUM | `process_monitor.rs`. Add guard: `if safe_mode_active && is_game_related_process(pid) { skip }`. Better: disable all auto-kill during safe mode, rely on the whitelist audit running only after game exits. |
| TS-6 | **Shared memory telemetry gated to known-safe reads** | iRacing's official SDK explicitly permits external shared memory reads — iRacing staff confirmed "use of the iRacing telemetry system will not cause EAC to trigger any issues." rF2/LMU shared memory is the same pattern (memory-mapped file, read-only). EAC/EAAC-protected games (F1 25, EA WRC): telemetry is UDP, not shared memory — no risk from the telemetry adapter. Risk level: iRacing = LOW (officially sanctioned), LMU = LOW (same SDK model, no write access), F1 25/WRC = NO RISK (UDP, no process access), AC EVO = UNKNOWN (use with caution until anti-cheat situation confirmed). | LOW | Telemetry adapter trait (v13.0). Add `safe_mode_read_allowed: bool` to each game's telemetry config. iRacing: `true` always. F1 25/WRC: `true` always (UDP). AC EVO: feature-flagged `false` until confirmed safe. |
| TS-7 | **Safe mode deactivation after game exit and anti-cheat shutdown** | Safe mode must deactivate cleanly after the game and its anti-cheat process both exit. EAC/EAAC shut down when the game exits — confirmed by EA documentation. iRacing EOS: same behavior. Deactivation: restore keyboard hook (or re-apply GPO values), re-enable process monitor, log the transition. | LOW | Game exit detection already exists in `event_loop.rs`. Extend to also wait for anti-cheat process exit (`EasyAntiCheat_EOS.exe`, `EASYANTICHEAT.EXE`, `EAAntiCheat.GameService.exe`) before restoring normal mode. Poll for AC process absence with 5s timeout. |
| TS-8 | **Code sign rc-agent.exe and rc-sentry.exe** | Unsigned binaries are not directly banned by EAC/EAAC, but: (1) unsigned binaries attract heavier scrutiny from security tools that interact with the same kernel space as anti-cheat drivers, (2) EAAC specifically monitors "unauthorized drivers" and the unsigned status of adjacent processes is a contributing signal, (3) the FanControl case study shows that unsigned/vulnerable drivers used by adjacent software trigger EAC detection. Code signing eliminates this as a contributing factor and is standard practice for software distributed to customers. | LOW (operationally — cost ~$200-400/year for OV cert; technical effort is low) | `rc-agent.exe` and `rc-sentry.exe` binaries. Sign with a real OV (Organization Validation) code signing certificate. Self-signed certs do NOT satisfy this — they are flagged the same as unsigned by EAAC. |
| TS-9 | **Anti-cheat compatibility validation on Pod 8 canary** | All safe mode behaviors must be tested against each game before fleet rollout. Pod 8 is the established canary. Validation: launch each protected game, verify no anti-cheat crash, verify billing + lock screen still work in safe mode, verify game exit restores normal mode. | MEDIUM | Pod 8 canary deployment is the established pattern (standing rule). Create a test checklist: one session per game, observe rc-agent logs, confirm no EAC/EAAC violation detected. |
| TS-10 | **Anti-cheat compatibility matrix documentation** | Staff and future developers must know which behaviors are safe per game. A machine-readable matrix in `racecontrol.toml` and a human-readable ops reference. Documents both current safe behaviors and rationale for disabled behaviors per game. | LOW | New `[anti_cheat]` section in `racecontrol.toml`. Anti-cheat matrix in `.planning/` as ops reference. |

### Differentiators (Raise Quality Above Minimum Viable Compliance)

Features that improve the implementation beyond "probably won't get banned" to "verifiably safe."

| # | Feature | Value Proposition | Complexity | Notes |
|---|---------|-------------------|------------|-------|
| D-1 | **Safe mode audit log entry per session** | Every time safe mode activates or deactivates, log the event with timestamp, game ID, anti-cheat tier, and which subsystems were suspended. Provides post-incident evidence if a ban dispute arises. "Pod 3 ran F1 25 at 14:22, keyboard hook suspended 14:22-15:47, restored 15:47." | LOW | Append to `C:\RacingPoint\safe-mode.log`. Same pattern as crash-sentry.log. |
| D-2 | **ConspitLink process behavior audit** | ConspitLink software runs alongside games to manage the Ares 8Nm wheelbase. Its internal behavior is opaque — it may use shared memory, HID APIs, or low-level process hooks that are safe or unsafe depending on implementation. A one-time audit of ConspitLink's loaded DLLs and system calls (using Process Monitor during a test session) should be documented. | MEDIUM | Use Sysinternals Process Monitor in a test environment. Check: does ConspitLink open handles to game processes? Does it inject DLLs? Does it install hooks? If it only uses HID driver communication (standard hardware API), it is EAC-safe. Document findings. |
| D-3 | **Per-game TOML anti-cheat profile with risk override** | racecontrol.toml should carry explicit per-game anti-cheat config: which AC system is active, which subsystems to suspend in safe mode, whether shared memory telemetry is permitted. This makes adding a new game (v13.0+) safe by default — new games default to `anti_cheat_tier = "unknown"` which triggers maximum safe mode. | LOW | Extends v13.0 `GameProfile` TOML structure. Example: `[games.f1_25] anti_cheat = "eaac" safe_mode_hooks = false safe_mode_process_kill = false telemetry_type = "udp"` |
| D-4 | **Graceful billing on safe mode activation** | If safe mode activates mid-session (game launched after billing started), billing must continue correctly. The safe mode transition must not interrupt the billing guard or cause a false session end. Explicit test case: start billing → launch F1 25 → verify safe mode activates → verify billing continues → game exits → verify safe mode deactivates → billing ends normally on PIN entry. | MEDIUM | `billing_guard.rs` already tracks session state independently of game state. Safe mode state machine must be independent from billing lifecycle. Integration test to verify no crosstalk. |
| D-5 | **Anti-cheat process watchdog during game session** | Detect if the anti-cheat process crashes or exits unexpectedly during a game session (without the game also exiting). This is a signal that the anti-cheat system encountered something it didn't like — a potential precursor to a ban. Alert staff: "EAC process exited unexpectedly on Pod 4 during F1 25 session." | MEDIUM | Poll for anti-cheat process presence (same process name watchlist as TS-7). If AC process disappears but game is still running → log warning + staff alert. Does not terminate session — that is for staff judgment. |
| D-6 | **Safe mode state visible in fleet health dashboard** | Staff kiosk shows pod states. Adding `safe_mode: bool` and `protected_game: Option<String>` to `PodFleetStatus` lets staff see at a glance which pods are in restricted mode. Useful for diagnosing reports of "kiosk locked up" during game sessions. | LOW | Extend `PodFleetStatus` struct in rc-common. `safe_mode: bool`. No new endpoints needed — server aggregates pod health via existing fleet poll. |

### Anti-Features (Commonly Proposed, Actively Harmful)

| Anti-Feature | Why Requested | Why Problematic | Alternative |
|--------------|---------------|-----------------|-------------|
| **Permanently disable keyboard hook for all games** | Simplifies code — no need for safe mode toggle | Win key and Alt+Tab become permanently active during ALL sessions, including unprotected games. Customers access desktop, task manager, other apps. Venue security regresses to pre-Phase 78 state. | Suspend hook only during protected game sessions (TS-3). Restore immediately after game exit. |
| **Kill all processes before launching a protected game** | "Ensures clean state" before EAC starts scanning | Killing processes by PID requires OpenProcess internally. If any game-adjacent process (anti-cheat helper, prior game residue) is targeted by PID rather than name, the PID lookup could interact with anti-cheat detection. Also destroys ConspitLink, breaking wheelbase for the session. | Kill by process name (safe), not by PID. Use existing pre-flight checks (v11.1) which are already billing-safe. |
| **Process Monitor running during game session for debug** | Useful for diagnosing anti-cheat issues during testing | Sysinternals Process Monitor attaches a kernel filter driver that intercepts all process/file/registry events system-wide. This is exactly the behavior EAC and EAAC scan for. Running it during a real customer session with a protected game would almost certainly trigger detection. | Use Process Monitor ONLY in isolated test sessions on Pod 8 with no real account logged into the game. Never during customer play. |
| **Injecting a DLL to replace the keyboard hook** | "More reliable" key blocking without SetWindowsHookEx | DLL injection is a primary vector for cheats. EAC monitors for injected DLLs in all processes. Even a legitimately-purposed DLL injected via CreateRemoteThread or AppInit_DLLs will be detected. | Group Policy registry keys for key blocking. No DLL injection required or appropriate. |
| **Using WMI queries to enumerate game processes during session** | "Safer than OpenProcess" for process discovery | WMI queries for process information (`SELECT * FROM Win32_Process`) still issue system calls that anti-cheat drivers can observe. WMI is a known vector for process enumeration by cheats. The risk is lower than OpenProcess, but not zero. | Use `sysinfo::System::processes()` (Rust crate already in use) which reads from `/proc`-equivalent kernel structures. This is the approach already validated as EAC-safe in v11.2 research. |
| **Whitelisting rc-agent with EAC via developer API** | "Official solution" — register rc-agent as trusted software | EAC whitelisting requires a relationship with Epic Games as a game developer. Racing Point is not a game developer. This path is not available for venue management software. Even if available, it would be per-game (LMU, not all EAC games). | Behavioral compatibility: don't do what EAC flags. No whitelist needed if rc-agent does not engage in flaggable behaviors. |
| **Running LMU/F1 25 in offline/no-anti-cheat mode** | "Bypass the problem entirely" | Offline mode disables competitive features — no online racing, no leaderboard sync, no iRacing championship participation. This defeats the venue's value proposition (competitive racing, lap time records). | Build safe mode correctly. Customers play normally, accounts are protected, venue competitive features are preserved. |

---

## Feature Dependencies

```
TS-1 (detect protected game launch)
    └──triggers──> TS-2 (activate safe mode)
                       ├──suspends──> TS-3 (keyboard hook suspended)
                       ├──gates──> TS-5 (process kill disabled)
                       └──gates──> TS-6 (telemetry reads, per-game safety)

TS-3 (hook suspended)
    └──requires replacement──> TS-4 (GPO-based key blocking)
                                    └──must not conflict with──> TS-3 (don't restore hook if GPO is active)

TS-2 (safe mode active)
    └──deactivates on──> TS-7 (game exit + AC process exit)
                              └──restores──> TS-3 (hook), TS-5 (process kill), TS-6 (telemetry)

TS-8 (code signing)
    └──independent prerequisite for deployment, no runtime dependency

TS-9 (Pod 8 validation)
    └──validates──> TS-2, TS-3, TS-4, TS-5, TS-6, TS-7

D-2 (ConspitLink audit)
    └──informs──> TS-6 (is ConspitLink telemetry access safe?)
    └──informs──> TS-2 (should ConspitLink be suspended in safe mode?)

D-4 (billing continuity during safe mode)
    └──validates that──> TS-2 (safe mode activation) does not corrupt billing_guard state

D-3 (per-game TOML profile)
    └──drives──> TS-1 (which anti-cheat tier maps to which game)
    └──drives──> TS-2 (which subsystems to suspend per game)
```

### Dependency Notes

- **TS-4 is a dependency of TS-3, not a replacement**: The hook must be suspended (TS-3) AND the GPO keys must be applied (TS-4) together. Suspending the hook without applying GPO leaves Win key + Alt+Tab open. Applying GPO without suspending the hook still leaves the hook detectable by EAC. Both must happen atomically on safe mode activation.
- **TS-7 must wait for anti-cheat process exit, not just game exit**: EAC/EAAC processes run briefly after the game exits to finalize state. Restoring the keyboard hook before AC has fully shut down could create a detection window. 5s poll for AC process absence is the safe approach.
- **TS-8 (code signing) is a prerequisite for fleet deployment, not a runtime feature**: It must be completed before signed binaries are deployed. It does not gate safe mode functionality, but it reduces overall anti-cheat risk surface for the deployed pods.
- **AC EVO telemetry (TS-6) is feature-flagged**: AC EVO is in Early Access (v0.5.4 as of early 2026). No confirmed anti-cheat system. Shared memory improved in v0.5 update. Telemetry adapter should exist but be gated `feature = "ac_evo_telemetry"` until anti-cheat situation is confirmed at or near full release.

---

## MVP Definition

### Launch With (v15.0 core — before v13.0 deploys to customers)

Minimum behaviors needed to prevent customer account bans on any supported game.

- [ ] **TS-1** — Protected game detection from `GameProfile.anti_cheat_tier` in TOML. Detect by game process name + optional AC process presence.
- [ ] **TS-2** — Safe mode state machine in `app_state.rs`. `Normal → SafeMode` on game launch if `anti_cheat_tier != none`. Log transition.
- [ ] **TS-3** — Keyboard hook suspended on `SafeMode` entry. Hook process-name-checked to confirm it terminates before game anti-cheat starts scanning.
- [ ] **TS-4** — GPO registry keys applied on `SafeMode` entry: `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System\DisableTaskMgr=1` and `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\NoWinKeys=1`. Removed on `Normal` restore.
- [ ] **TS-5** — Process allowlist enforcement disabled in `SafeMode`. No PID-based kills during protected game sessions. Resume after `Normal` restore.
- [ ] **TS-6** — Telemetry reads enabled per game config: iRacing SDK reads always safe, F1 25/WRC UDP always safe, LMU shared mem reads (read-only) safe, AC EVO feature-flagged off.
- [ ] **TS-7** — Safe mode deactivation after game exit + AC process gone (5s poll). Restore hook, GPO keys removed, process monitor re-enabled.
- [ ] **TS-9** — Pod 8 canary test sessions: F1 25, iRacing, LMU each played for 5+ minutes in a real session with real game account. Observe for anti-cheat violations or game crashes.

### Add After Core Validation (v15.1)

- [ ] **TS-8** — Code sign rc-agent.exe and rc-sentry.exe with OV certificate before final fleet rollout.
- [ ] **TS-10** — Anti-cheat compatibility matrix in racecontrol.toml + ops reference doc.
- [ ] **D-1** — Safe mode audit log (`C:\RacingPoint\safe-mode.log`) with per-session activation/deactivation records.
- [ ] **D-3** — Per-game TOML anti-cheat profile (anti_cheat tier, subsystem toggles, telemetry type) replacing hardcoded logic.
- [ ] **D-4** — Billing continuity integration test: safe mode activation during live billing session.

### Future Consideration (v15.2 / post-v13.0 feedback)

- [ ] **D-2** — ConspitLink process audit (Sysinternals Process Monitor test session on Pod 8).
- [ ] **D-5** — Anti-cheat process watchdog (detects unexpected AC exit during session, alerts staff).
- [ ] **D-6** — `safe_mode` field in PodFleetStatus for fleet health dashboard visibility.
- [ ] **AC EVO full anti-cheat assessment** — re-evaluate when AC EVO leaves Early Access; enable telemetry feature flag if safe.

---

## Feature Prioritization Matrix

| Feature | Customer Protection Value | Implementation Cost | Priority |
|---------|--------------------------|---------------------|----------|
| TS-1 Protected game detection | HIGH — foundation for everything | LOW — process name match vs TOML profile | P1 |
| TS-2 Safe mode state machine | HIGH — enables all subsystem gating | MEDIUM — new state in AppState, transition logic | P1 |
| TS-3 Keyboard hook suspension | HIGH — direct EAC/EAAC detection trigger | LOW — add suspend()/resume() to existing hook | P1 |
| TS-4 GPO key blocking replacement | HIGH — required to maintain kiosk integrity without the hook | HIGH — GPO reg keys + testing Win 11 Pro behavior | P1 |
| TS-5 Process kill gating | HIGH — PID-based kills near game process are risky | MEDIUM — add safe_mode guard to process_monitor | P1 |
| TS-6 Telemetry read safety | MEDIUM — reads are already low-risk; gate AC EVO | LOW — add `safe_mode_read_allowed` field to telemetry config | P1 |
| TS-7 Safe mode deactivation | HIGH — restoring hooks before AC exits is a risk | LOW — poll for AC process exit, 5s timeout | P1 |
| TS-9 Pod 8 canary validation | HIGH — confirms no false assumptions | MEDIUM — manual test sessions, observe logs | P1 |
| TS-8 Code signing | MEDIUM — reduces risk surface, not a hard ban trigger | LOW (tech) / MEDIUM (cost ~$200-400/yr OV cert) | P2 |
| TS-10 Compatibility matrix docs | MEDIUM — ops reference, reduces future mistakes | LOW | P2 |
| D-1 Safe mode audit log | MEDIUM — evidence for ban disputes | LOW | P2 |
| D-3 Per-game TOML anti-cheat profile | MEDIUM — makes adding new games safe by default | LOW | P2 |
| D-4 Billing continuity test | MEDIUM — correctness validation | MEDIUM — integration test | P2 |
| D-2 ConspitLink audit | MEDIUM — ConspitLink is opaque, risk unknown | MEDIUM — test environment setup | P3 |
| D-5 AC process watchdog | LOW — early warning, not prevention | MEDIUM — process polling loop | P3 |
| D-6 Fleet dashboard safe_mode field | LOW — visibility only | LOW — struct field + server handler | P3 |

---

## Per-Game Anti-Cheat Behavior Reference

### EAC (Easy Anti-Cheat) — LMU, some iRacing versions

**What it scans for (MEDIUM confidence — academic analysis + community reports):**
- Open handles to the game process from external processes (`OpenProcess` with any access rights)
- Memory read/write operations targeting the game (`ReadProcessMemory`, `WriteProcessMemory`, `VirtualQueryEx`)
- Injected DLLs or manually-mapped code in game process memory
- User-mode hooks (`SetWindowsHookEx` WH_KEYBOARD_LL, WH_MOUSE_LL detected as AHK-like tools)
- Kernel drivers that are unsigned, vulnerable, or contain memory read/write functions (`InpOutx64`, `WinRing0` — the FanControl false positive case)
- Suspicious thread creation in kernel/user mode not associated with loaded modules

**What is confirmed safe alongside EAC:**
- Standard Windows process listing via `sysinfo::System::processes()` (no handle to game process)
- UDP network traffic (game telemetry, health heartbeats)
- TCP connections to other localhost ports (rc-agent :8090 health endpoint)
- File I/O on non-game paths (log files, config files)
- Shell commands by process name (`taskkill /F /IM process.exe` — uses name resolution, not direct PID handle)
- iRacing official shared memory SDK reads (iRacing staff confirmed explicitly)
- OBS Studio (whitelisted by EAC when certificate is current — requires game developer to accept new cert)

**What triggers EAC detection (MEDIUM confidence):**
- `SetWindowsHookEx` WH_KEYBOARD_LL or WH_MOUSE_LL from any external process — flagged as AHK/cheat input manipulation
- Adjacent service processes using vulnerable drivers (LightingService.exe with Asus/Gigabyte RGB drivers caused widespread EAC bans in Apex Legends, Fortnite — requires only that the vulnerable driver is loaded, not that it accesses the game)
- Process running inside a sandbox or virtual machine (sandbox detection is a first-class EAC feature)

### EA AntiCheat (EAAC) — F1 25, EA WRC

**What it scans for (MEDIUM confidence — EA deep-dive blog + community reports):**
- Kernel drivers that are unsigned, deny-listed, or incompatible with Windows HVCI/Secure Boot policies
- External processes interacting with F1 25 or EA WRC process memory ("any attempt by an external process to touch F1 25's memory is instantly blocked")
- DLL replacement in the game directory (hash sum checks on critical DLLs like dxgi.dll)
- Redirected function calls (hook detection on system interrupt tables)

**What is confirmed safe alongside EAAC:**
- EAAC "only runs when a game with EA anticheat protection included is running. All anti-cheat processes shut down when the game does." (EA official)
- EAAC "claims not to monitor applications that are not connected to EA games" (EA blog — LOW confidence, this is a PR statement)
- UDP-based telemetry (F1 25 outputs telemetry to UDP port 20777 — standard, no process access)

**What triggers EAAC detection (MEDIUM confidence):**
- Adjacent kernel drivers that are unsigned or flagged as vulnerable
- Processes attempting to read/write F1 25 game memory
- Game file modifications (DLL swaps, config tampering)

### iRacing EOS (Epic Online Services) anti-cheat

**What it scans for (MEDIUM confidence — iRacing support docs + SimTools forum confirmation):**
- External programs "hacking into and modifying the simulation" — memory read/write to iRacing process
- Modification of iRacing installation files
- Replacement of system-level components used by iRacing

**What is confirmed safe alongside iRacing EOS:**
- iRacing official telemetry SDK (shared memory, read-only): explicitly confirmed safe by iRacing staff. "Use of the iRacing telemetry system will not cause EAC to trigger any issues." This confirmation was from iRacing's Randy Cassidy regarding SimTools (motion platform software).
- Multiple simultaneous SDK readers (JRT, dashboard apps, motion platforms, bass shakers)
- Third-party apps running alongside iRacing that do not access iRacing process memory

**What triggers iRacing EOS detection (LOW confidence — limited public documentation):**
- Modifying iRacing process memory directly
- Running inside a sandbox environment
- Modifying iRacing installation files

### Kunos AC / AC EVO

**Status: UNKNOWN — Early Access as of 2026-03-21 (MEDIUM confidence)**

AC EVO v0.5.4 (January 2026) improved shared memory output to match ACC quality. No confirmed anti-cheat system deployment announced. The original Assetto Corsa (AC) has no anti-cheat and is extensively modded. ACC had a proprietary server-side check system (file checksums, not kernel-level). AC EVO is expected to receive more robust anti-cheat at or after full release (Fall 2025 original target, now later).

**Conservative approach for rc-agent:**
- Treat AC EVO as `anti_cheat_tier = "unknown"` which activates full safe mode
- Shared memory telemetry: feature-flagged off until anti-cheat situation confirmed
- Revisit at AC EVO 1.0 release

---

## Safe Mode Behavioral Contract

What rc-agent does and does not do when `SafeMode` is active:

| Subsystem | Normal Mode | Safe Mode | Notes |
|-----------|------------|-----------|-------|
| Keyboard hook (WH_KEYBOARD_LL) | ACTIVE — blocks Win, Alt+Tab, etc. | SUSPENDED | Hook unregistered for the duration. |
| GPO kiosk keys (DisableTaskMgr, NoWinKeys) | Varies | APPLIED | Compensates for suspended hook. |
| Process allowlist enforcement (kill non-whitelisted) | ACTIVE | DISABLED — no kills | No PID-based operations during protected game. |
| Health endpoint polling (:8090) | ACTIVE | ACTIVE | Safe — no game process contact. |
| UDP telemetry reads (F1 25, WRC) | ACTIVE | ACTIVE | Safe — UDP, no process access. |
| iRacing SDK shared memory read | ACTIVE when game running | ACTIVE | Explicitly confirmed safe by iRacing. |
| LMU rF2 shared memory read | ACTIVE when game running | ACTIVE | Read-only, same model as iRacing SDK. |
| AC EVO shared memory read | FEATURE FLAGGED | FEATURE FLAGGED OFF | Await confirmation at full release. |
| Billing lifecycle (start/stop/idle) | ACTIVE | ACTIVE — unchanged | Safe mode must not affect billing. |
| Lock screen (PIN auth) | ACTIVE | ACTIVE — unchanged | Game session = customer session, lock screen still enforces exit. |
| WebSocket to server | ACTIVE | ACTIVE | TCP, no game process contact. |
| ConspitLink (external process) | RUNNING | RUNNING | Hardware FFB — do not kill. Audit separately (D-2). |
| Process monitoring (sysinfo process list) | ACTIVE | ACTIVE — read only, no kills | Reading process list without open handles is safe. |

---

## Sources

- **EAC detection mechanisms (MEDIUM confidence):**
  - Academic analysis: [arxiv.org/html/2408.00500v1](https://arxiv.org/html/2408.00500v1) — kernel-level anti-cheat survey; confirms handle scanning, hook detection, driver monitoring
  - AutoHotKey/SetWindowsHookEx EAC flags: [AutoHotKey community](https://www.autohotkey.com/boards/viewtopic.php?t=38423) — confirmed EAC flags WH_KEYBOARD_LL hooks as AHK cheat tools
  - FanControl false positive case: [GitHub Rem0o/FanControl.Releases #2104](https://github.com/Rem0o/FanControl.Releases/issues/2104) — vulnerable adjacent driver (WinRing0) triggered EAC without touching the game process
  - LightingService.exe EAC bans: [EA Forums](https://answers.ea.com/t5/Technical-Issues/Easy-Anti-Cheat-game-security-violation-detected-000000D/td-p/7825729) — Asus/Gigabyte RGB service caused Game Security Violation across multiple EAC games

- **iRacing EOS and telemetry safety (HIGH confidence):**
  - iRacing staff confirmation: [XSimulator forum thread](https://www.xsimulator.net/community/threads/will-iracing-new-anti-cheat-software-work-with-simtools.7273/) — Randy Cassidy (iRacing): "Use of the iRacing telemetry system will not cause EAC to trigger any issues."
  - iRacing SDK documentation: [sajax.github.io/irsdkdocs](https://sajax.github.io/irsdkdocs/) — shared memory model, multiple simultaneous readers supported
  - iRacing transition to EOS: [iRacing support 2024 S2 Patch 4](https://support.iracing.com/support/solutions/articles/31000173098-2024-season-2-patch-4-release-notes-2024-05-01-02-)

- **EA AntiCheat (EAAC) — F1 25 and WRC (MEDIUM confidence):**
  - EA WRC EAAC announcement: [EA News](https://www.ea.com/news/ea-anticheat) — kernel-mode driver, full-stack EA ownership, active only when game runs
  - EA EAAC deep-dive: [ea.com/security/news/eaac-deep-dive](https://www.ea.com/security/news/eaac-deep-dive) — confirms memory sandbox, driver monitoring; vague on specifics
  - EAAC incompatible driver resolution: [Windows Forum](https://windowsforum.com/threads/resolve-ea-anticheat-incompatible-driver-on-windows-11-quick-guide.378203/) — HVCI/Secure Boot incompatible drivers blocked by EAAC

- **LMU EAC (MEDIUM confidence):**
  - LMU v1.2 update: [Simulation Daily](https://simulationdaily.com/news/le-mans-ultimate-v1-2-update/) — EAC "blocks access to write to memory locations and will crash the game if players start interfering"
  - LMU EAC community: [Le Mans Ultimate community forums](https://community.lemansultimate.com/index.php?threads/easy-anti-cheat-activation.14662/) — EAC activation confirmed from v1.2+

- **AC EVO (LOW confidence — early access, limited anti-cheat documentation):**
  - AC EVO v0.5 shared memory: [assettocorsa.gg](https://assettocorsa.gg/assetto-corsa-evo-release-0-5-out-now/) — shared memory improved in Release 0.5
  - PCGamingWiki AC EVO: [pcgamingwiki.com/wiki/Assetto_Corsa_EVO](https://www.pcgamingwiki.com/wiki/Assetto_Corsa_EVO) — no anti-cheat system listed

- **Windows kiosk lockdown alternatives (HIGH confidence):**
  - Microsoft Keyboard Filter: [learn.microsoft.com/windows/configuration/keyboard-filter](https://learn.microsoft.com/en-us/windows/configuration/keyboard-filter/) — confirmed IoT Enterprise only, NOT available on Windows 11 Pro
  - GPO DisableTaskMgr: [mdmandgpanswers.com](https://www.mdmandgpanswers.com/blogs/view-blog/how-to-disable-windows-shortcut-keystrokes-using-group-policy-and-intune) — registry-based, no hooks, OS-level policy

---

*Feature research for: v15.0 AntiCheat Compatibility (rc-agent safe mode for sim racing venue management on Windows 11 Pro pods)*
*Researched: 2026-03-21*
