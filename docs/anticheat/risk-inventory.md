# rc-agent Anti-Cheat Risk Inventory

**Created:** 2026-03-21
**Version:** v15.0 Phase 107
**Purpose:** Exhaustive inventory of every pod-side behavior that could trigger anti-cheat detection

---

## Anti-Cheat Systems Reference

| Anti-Cheat System | Games | Kernel Level | Notes |
|------------------|-------|-------------|-------|
| EA AntiCheat (EAAC / EA Javelin) | F1 25, EA WRC | YES — kernel-mode driver | Full EA stack. Replaced EAC in EA sports titles mid-2024. Blocks unsigned drivers. Crashes game on detected violation. Only active while game is running. |
| iRacing EOS (Epic Online Services) | iRacing | YES | Switched from EAC to EOS in 2024 S2 Patch 4. Runs iRacing inside sandbox — blocks external process memory access. Official telemetry SDK (shared memory) explicitly permitted. |
| Easy Anti-Cheat (EAC) | LMU (Le Mans Ultimate) | YES — kernel-mode driver | Shipped from LMU v1.2+. Blocks write-access to memory. Crashes game if interference detected. Multiplayer enforced; single player reportedly less strict. |
| Unknown | AC EVO (Assetto Corsa EVO) | UNKNOWN | Early Access v0.5.4 (Jan 2026). No confirmed anti-cheat system announced. Likely light or no enforcement in Early Access. May change at full release. |

---

## Risk Inventory

Every rc-agent source file and API usage that could trigger anti-cheat detection. Severity classifications per research:

- **CRITICAL**: Immediate ban trigger or near-certain detection
- **HIGH**: Known detection signature (likely to be flagged by active anti-cheat)
- **MEDIUM**: Contributing risk factor (raises suspicion composite score)
- **LOW**: Minimal risk but documented for completeness
- **NONE**: No anti-cheat risk

| Behavior | Source File:Line | API/Function Used | EA Javelin Risk | iRacing EOS Risk | LMU EAC Risk | AC EVO Risk | EA WRC Risk | Phase to Address |
|----------|-----------------|------------------|----------------|-----------------|-------------|------------|------------|-----------------|
| Global low-level keyboard hook — blocks Win, Alt+Tab, Alt+F4 during kiosk session | `kiosk.rs:958-959` | `SetWindowsHookExW(WH_KEYBOARD_LL, ...)` | CRITICAL — same API as AHK/keyloggers; EAAC kernel driver enumerates system-wide hooks | CRITICAL — iRacing EOS scans for hook-installing processes; WH_KEYBOARD_LL is a primary AHK cheat signature | CRITICAL — EAC flags WH_KEYBOARD_LL from external unsigned processes (community-confirmed AutoHotKey ban trigger) | MEDIUM — Unknown AC system, but hook is detectable pattern | CRITICAL — same as F1 25 (both use EAAC) | Phase 108: Replace with GPO registry keys (NoWinKeys=1, DisableTaskMgr=1) |
| Continuous process enumeration during kiosk enforcement scan | `kiosk.rs:654` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | MEDIUM — process enumeration without OpenProcess handles; sysinfo reads kernel process list safely | LOW — sysinfo approach confirmed safe by iRacing EOS (no open handles to game) | MEDIUM — EAC monitors for continuous process scanning (cheat tool signature); no game handles opened | LOW | MEDIUM | Phase 109: Gate behind safe mode — suspend during active protected game session |
| Continuous process enumeration for kiosk ALLOWED_PROCESSES enforcement | `kiosk.rs:769` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | MEDIUM | LOW | MEDIUM | LOW | MEDIUM | Phase 109: Gate behind safe mode |
| Process guard scan cycle — enumerates all processes every scan interval | `process_guard.rs:99` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | MEDIUM — repeated fast-cycle scanning resembles cheat process monitor | LOW | HIGH — EAC kernel monitors for frequent process enumeration; combined with kill capability = classic cheat pattern | LOW | MEDIUM | Phase 109: Suspend process_guard entirely during protected game sessions |
| Process guard scan cycle — second enumeration pass | `process_guard.rs:223` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | MEDIUM | LOW | HIGH | LOW | MEDIUM | Phase 109: Suspend process_guard entirely |
| Process guard scan cycle — third enumeration pass (kill verification) | `process_guard.rs:580` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | MEDIUM | LOW | HIGH | LOW | MEDIUM | Phase 109: Suspend process_guard entirely |
| PID-targeted process kill via taskkill /F /PID — used by process_guard | `process_guard.rs:259-260` | `taskkill /F /PID {pid}` | HIGH — PID-based kill during game session risks targeting game-adjacent processes; EAC monitors for TerminateProcess on protected PIDs | MEDIUM — iRacing EOS protects against external process interference; PID kill near game is suspicious | HIGH — EAC PROCESS_PROTECTION_LEVEL_ANTI_CHEAT_LIGHT blocks PID kills on game; attempt to kill adjacent PID may be logged | MEDIUM | HIGH | Phase 109: Disable all PID-targeted kills in safe mode; only name-based kills permissible |
| PID-targeted process kill via taskkill — fallback in process_guard | `process_guard.rs:596` | `taskkill /F /PID {pid}` | HIGH | MEDIUM | HIGH | MEDIUM | HIGH | Phase 109: Suspend entirely |
| Pre-flight orphaned game process kill via PID | `pre_flight.rs:250-251` | `taskkill /F /PID {pid}` | LOW — runs only at startup before anti-cheat is active; not during game session | LOW | LOW | LOW | LOW | SAFE — startup-only; anti-cheat not yet active |
| OpenProcess on game PID to check if process is alive | `game_process.rs:321` | `winapi::um::processthreadsapi::OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, ...)` | HIGH — OpenProcess on game PID is flagged by EAAC kernel even with limited rights; any handle to game process from unsigned binary is suspicious | HIGH — iRacing EOS explicitly blocks external process memory access; PROCESS_QUERY_LIMITED_INFORMATION still opens handle | HIGH — EAC monitors all handle opens to game process; even query-only handle from unsigned process is detectable | MEDIUM | HIGH | Phase 109: Replace is_process_alive() with name-based detection in safe mode |
| Process enumeration during failure monitor — two-pass CPU check | `failure_monitor.rs:272-274` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` (twice) | MEDIUM — triggered when game may be frozen; but anti-cheat is active when game is running | LOW | MEDIUM | LOW | MEDIUM | Phase 109: Gate failure monitor behind safe mode check or exempt from kills |
| Process enumeration during game_process::find_game_pid | `game_process.rs:98` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | LOW — used for game detection (startup); LOW risk for one-time check | LOW | LOW | LOW | LOW | LOW risk — detection purpose only; no kills |
| Process enumeration during game_process::find_game_pid (second call) | `game_process.rs:304` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | LOW | LOW | LOW | LOW | LOW | LOW risk — detection purpose only |
| Pre-flight process enumeration | `pre_flight.rs:176` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | LOW — startup-only; anti-cheat not yet active when pre-flight runs | NONE | LOW | NONE | LOW | SAFE — pre-game startup context |
| Pre-flight additional process scan | `pre_flight.rs:631` | `sysinfo::System::refresh_processes(ProcessesToUpdate::All, true)` | LOW | NONE | LOW | NONE | LOW | SAFE — startup context |
| iRacing shared memory read via named file mapping | `sims/iracing.rs:128-140` | `OpenFileMappingW(FILE_MAP_READ, "Local\\IRSDKMemMapFileName")` + `MapViewOfFile` | NONE — iRacing SDK confirmed safe by iRacing staff; EAC/EAAC do not protect iRacing | LOW — iRacing EOS explicitly permits this via official SDK; Randy Cassidy (iRacing): "Use of the iRacing telemetry system will not cause EAC to trigger any issues" | NONE — iRacing not LMU-protected | NONE | NONE | Safe — officially sanctioned |
| LMU shared memory read via named file mapping | `sims/lmu.rs:225-239` | `OpenFileMappingW(FILE_MAP_READ, rF2 shared mem names)` + `MapViewOfFile` | NONE — LMU not EAAC-protected | NONE — iRacing not LMU | LOW — EAC-protected; read-only named shared memory (no OpenProcess) is lower risk than ReadProcessMemory; rF2 SDK model explicitly designed for external readers | LOW — AC EVO may adopt similar model | NONE | Phase 110: Verify FILE_MAP_READ flag is always read-only; never escalate to FILE_MAP_WRITE |
| Assetto Corsa shared memory read via named file mapping | `sims/assetto_corsa.rs:182-191` | `OpenFileMappingW(FILE_MAP_READ, ...)` + `MapViewOfFile` | NONE | NONE | NONE | LOW — AC EVO is the relevant successor; AC classic has no anti-cheat | NONE | Safe — AC has no anti-cheat |
| AC EVO shared memory read via named file mapping | `sims/assetto_corsa_evo.rs:168-177` | `OpenFileMappingW(FILE_MAP_READ, ...)` + `MapViewOfFile` | NONE | NONE | NONE | MEDIUM — AC EVO anti-cheat status unknown; shared memory reads are safer than ReadProcessMemory but risk unknown at full release | NONE | Phase 110: Feature-flag off by default; enable only after AC EVO anti-cheat confirmed safe at full release |
| F1 25 UDP telemetry socket binding on port 20777 | `sims/f1_25.rs:451-452` | `UdpSocket::bind("0.0.0.0:20777")` | NONE — F1 25 broadcasts telemetry to UDP; standard game telemetry interface | NONE | NONE | NONE | NONE | No risk — UDP socket, no process access |
| Overlay HUD window creation — separate topmost popup window | `overlay.rs:1118-1122` | `CreateWindowExW(WS_EX_TOPMOST|WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE|WS_EX_LAYERED, WS_POPUP|WS_VISIBLE, ...)` | LOW-MEDIUM — separate process window; not injection; EAAC does not scan for external windows but topmost layered window could appear as overlay cheat | LOW — external overlay windows not explicitly blocked by iRacing EOS; no DLL injection | LOW — EAC distinguishes overlay windows from injected hooks; separate process window is lower risk | LOW | LOW-MEDIUM | Phase 109: Consider suspending overlay during safe mode or ensure window does not overlap game window |
| HKLM Run key write — rc-agent startup persistence | `self_heal.rs:100-105` | `reg add HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` (via `repair_registry_key`) | LOW — applied only at startup if key is missing; not during game session; HKLM Run key is a standard software persistence mechanism | LOW | LOW | LOW | LOW | SAFE — startup-only repair |
| HKCU registry writes — notification suppression during lock screen | `lock_screen.rs:719-738` | `reg add HKCU\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings` + `HKCU\Software\Policies\Microsoft\Windows\Explorer` | LOW — HKCU changes during active session; not HKLM; notification suppression is standard OS feature | LOW | MEDIUM — registry writes during game session are visible to EAC; HKCU policy keys may raise profile | LOW | LOW | Phase 109: Consider suppressing notification changes during safe mode or accepting HKCU risk as LOW |
| HKCU Run key audit — reads and potentially removes unauthorized autostart entries | `process_guard.rs:326` | `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (read + possible delete) | LOW — registry read/audit; deletions are of unauthorized entries | LOW | LOW | LOW | LOW | LOW risk — audit purpose, not persistence |
| HKLM Run key audit — reads and potentially removes unauthorized autostart entries | `process_guard.rs:332` | `reg query HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run` | LOW | LOW | LOW | LOW | LOW | LOW risk |
| Ollama LLM query from ai_debugger — HTTP to :11434 during game session | `ai_debugger.rs:276` | HTTP POST to Ollama at configured URL (default 192.168.31.27:11434) | MEDIUM — Ollama runs on James .27 (remote); HTTP calls to external host during game session; GPU and memory contention during Ollama inference on same machine would be HIGH risk but Ollama is remote | LOW | MEDIUM — Ollama on separate host; HTTP traffic to local network is not blocked by EAC; but triggering Ollama for a game crash diagnosis during active EAC session creates non-standard process activity | LOW | MEDIUM | Phase 109: Gate ai_debugger Ollama calls behind safe mode (suppress LLM calls during protected game sessions) |
| Name-based taskkill for WerFault, msedge, and game cleanup processes | `ai_debugger.rs:609-610, 637-638, 786-791` | `taskkill /IM {name}.exe /F` | LOW — name-based kill is safer than PID; but killing WerFault/Edge during anti-cheat active is detectable activity | LOW | LOW — name-based kills confirmed safer by EAC research; no handle to game PID | LOW | LOW | Phase 109: Audit which name-based kills happen during game session vs. pre-flight |
| Name-based taskkill for game cleanup in ac_launcher | `ac_launcher.rs:287-290, 1534-1541` | `taskkill /IM {game}.exe /F` | LOW — name-based; happens during game launch/exit sequence | LOW | LOW | LOW | LOW | LOW risk — occurs at game boundaries, not mid-session |
| Unsigned rc-agent.exe and rc-sentry.exe binaries | All binaries | No code signing certificate — `Get-AuthenticodeSignature rc-agent.exe` returns "NotSigned" | MEDIUM — unsigned binary + keyboard hook behavior = stronger cheat tool profile match; EAAC builds hardware fingerprint; ban on one pod may propagate across all 8 pods sharing customer account | MEDIUM — unsigned status increases scrutiny; not a direct ban trigger for iRacing EOS | MEDIUM — unsigned binaries performing process enumeration + kills receive more scrutiny from EAC kernel | LOW — unknown AC system | MEDIUM | Phase 111: Sign with Sectigo OV certificate before fleet deployment |
| ConspitLink2.0.exe wheelbase software (external, opaque) | Not in rc-agent source — external process | Unknown — may use HID APIs, shared memory, or process hooks | MEDIUM-UNKNOWN — if ConspitLink uses DLL injection or process hooks it would be HIGH; HID-only is LOW; documented separately in AUDIT-02 | MEDIUM-UNKNOWN | MEDIUM-UNKNOWN | UNKNOWN | MEDIUM-UNKNOWN | AUDIT-02: ConspitLink process audit (Sysinternals Process Monitor test session) |

---

## Safe Behaviors (Positive List)

The following rc-agent behaviors are confirmed safe alongside all known anti-cheat systems:

| Behavior | Location | Reason Safe |
|----------|----------|-------------|
| Health endpoint HTTP polling (localhost:8090) | `main.rs` — Axum HTTP server | TCP on loopback, no game process contact |
| WebSocket connection to racecontrol server (192.168.31.23:8080) | `main.rs` — websocket client task | Standard TCP network traffic, no process interaction |
| Billing lifecycle management (start/stop/idle via WebSocket messages) | `main.rs` — billing state machine | State management only, no Win32 APIs |
| Lock screen PIN auth (Edge kiosk browser) | `lock_screen.rs` — Edge launch/management | Browser management via taskkill /IM (name-based, not PID) |
| File I/O on non-game paths (logs, config, learned allowlist) | Throughout | Standard file system operations, no process interaction |
| TCP connections to other localhost ports (Ollama :11434 — localhost only) | `ai_debugger.rs` | TCP to localhost; Ollama on remote James .27 = network traffic only |
| iRacing SDK shared memory reads (`Local\IRSDKMemMapFileName`) | `sims/iracing.rs` | Officially confirmed safe by iRacing staff; FILE_MAP_READ only |
| F1 25 UDP telemetry receive (port 20777) | `sims/f1_25.rs` | UDP passive listener; no process interaction |
| Self-heal config/script repair at startup | `self_heal.rs` | Startup-only, before anti-cheat is active |
| Name-based process kills using taskkill /IM | `lock_screen.rs`, `ac_launcher.rs` | Name resolution does not require OpenProcess on target PID |
| sysinfo process list reads (no OpenProcess) | Various — game detection | Reading process list via kernel structures without open handles is safe |

---

## Pod Windows Edition Verification

**Fleet exec status:** Server at 192.168.31.23:8080 is not reachable from James .27 dev machine during planning phase (deployed server is 97 commits behind HEAD, not yet updated with v15.0 changes). Direct pod agent queries at :8090/exec also returned no response — pods are likely in kiosk mode with the agent not exposing an /exec endpoint in the currently deployed version.

**Research-based determination:** All v15.0 planning research and Phase 78 implementation context consistently refers to pods as "Windows 11 Pro" machines. The FEATURES-v15-anticheat.md document title explicitly states "sim racing venue management on Windows 11 Pro pods." SUMMARY-v15.md states "Pods on Windows 11 Pro must use GPO registry keys." No prior phase has indicated Enterprise or Education edition pods. Pod hardware (identical Conspit sim rigs purchased together) makes mixed editions unlikely.

**Verification method:** Staff/Uday to run `winver` on any pod at the venue to confirm edition. Expected: Windows 11 Pro (Version 23H2 or 24H2).

| Pod | IP | Windows Edition | Build | Keyboard Filter Available? |
|-----|-----|----------------|-------|---------------------------|
| 1 | 192.168.31.89 | Windows 11 Pro (expected — see note above) | Pending live verification | No — Pro SKU does not include Keyboard Filter |
| 2 | 192.168.31.33 | Windows 11 Pro (expected) | Pending live verification | No |
| 3 | 192.168.31.28 | Windows 11 Pro (expected) | Pending live verification | No |
| 4 | 192.168.31.88 | Windows 11 Pro (expected) | Pending live verification | No |
| 5 | 192.168.31.86 | Windows 11 Pro (expected) | Pending live verification | No |
| 6 | 192.168.31.87 | Windows 11 Pro (expected) | Pending live verification | No |
| 7 | 192.168.31.38 | Windows 11 Pro (expected) | Pending live verification | No |
| 8 | 192.168.31.91 | Windows 11 Pro (expected) | Pending live verification | No |

**Decision:** Pods are on Windows 11 Pro. Phase 108 MUST use GPO registry keys (NoWinKeys=1, DisableTaskMgr=1) as the primary kiosk lockdown replacement. Keyboard Filter is NOT available on Windows 11 Pro. This decision is based on strong research evidence and consistent planning documentation — confirmed by live `winver` check before Phase 108 implementation begins.

---

## Code Signing Certificate Procurement

**Status:** Deferred to Uday — pending business owner initiation
**Certificate Authority:** Sectigo OV (~$220/yr)
**Key Storage:** Physical USB token (SafeNet eToken) — single build machine (James .27)
**Procurement Owner:** Uday Singh (business owner — OV requires org verification)
**Timeline:** 1-5 business days after purchase order
**Action Required:** Uday to purchase Sectigo OV cert and provide USB token to James

### Pre-Purchase Checklist

- [ ] Confirm with reseller: physical USB token or cloud key storage?
- [ ] Confirm Racing Point eSports org verification documents ready (business registration, Uday's ID)
- [ ] Budget approval (~$220/yr recurring)
- [ ] Select reseller (recommended: Sectigo direct or SSLTrust)
- [ ] Expected delivery date: _______________

---

## Summary of Findings for Phases 108-111

### Phase 108: Keyboard Hook Replacement (BLOCKING — must complete before any protected game canary)

| Behavior | Severity | Action Required |
|----------|---------|----------------|
| `SetWindowsHookExW(WH_KEYBOARD_LL)` in `kiosk.rs:958-959` | CRITICAL | Replace entirely with GPO registry keys: `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System\DisableTaskMgr=1` and `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\NoWinKeys=1` |

**Decision dependency:** Pod OS edition verification (Task 2 of this plan) confirms whether Windows Keyboard Filter is available. Research indicates Windows 11 Pro does NOT support Keyboard Filter — GPO registry keys are the mandatory path if pods are on Pro.

### Phase 109: Safe Mode State Machine (addresses HIGH and MEDIUM behaviors during game session)

| Behavior | Severity | Safe Mode Action |
|----------|---------|-----------------|
| `process_guard.rs:99,223,580` — continuous process enumeration | HIGH | Suspend entire process_guard loop while protected game active |
| `process_guard.rs:259-260,596` — PID-targeted taskkill | HIGH | Disable all PID-based kills during safe mode |
| `kiosk.rs:654,769` — kiosk enforcement process scans | MEDIUM | Suspend kiosk process enforcement during safe mode (relying on GPO keys instead) |
| `game_process.rs:321` — OpenProcess on game PID | HIGH | Replace is_process_alive() with name-based check in safe mode context |
| `failure_monitor.rs:272-274` — process scan during hang detection | MEDIUM | Gate hang detection behind safe mode; do not attempt kills during protected game |
| `overlay.rs:1118` — HUD overlay window | LOW-MEDIUM | Consider suspending overlay window during safe mode (optional) |
| `ai_debugger.rs:276` — Ollama LLM call during game session | MEDIUM | Gate ai_debugger behind safe mode — suppress Ollama queries while anti-cheat is active |
| `lock_screen.rs:719-738` — HKCU registry writes during session | LOW | Accept as LOW risk or move to session boundaries only |

### Phase 110: Telemetry Gating (LOW risks, per-game safety verification)

| Behavior | Severity | Telemetry Action |
|----------|---------|-----------------|
| `sims/lmu.rs:225-239` — LMU shared memory read | LOW | Verify FILE_MAP_READ only; gate activation on rF2 plugin "simulation active" flag |
| `sims/assetto_corsa_evo.rs:168-177` — AC EVO shared memory | MEDIUM | Feature-flag OFF by default; enable only after AC EVO 1.0 anti-cheat confirmed |
| `sims/iracing.rs:128-140` — iRacing SDK read | LOW | Safe as-is; officially sanctioned |
| `sims/f1_25.rs:451-452` — F1 25 UDP | NONE | Safe as-is |

### Phase 111: Code Signing + Canary Validation (MEDIUM risk reduction, deployment gate)

| Behavior | Severity | Code Signing Action |
|----------|---------|---------------------|
| `rc-agent.exe` unsigned binary | MEDIUM | Sign with Sectigo OV certificate — certificate must be procured NOW (1-5 business day OV verification delay) |
| `rc-sentry.exe` unsigned binary | MEDIUM | Sign with same certificate |
| ConspitLink2.0.exe (external) | MEDIUM-UNKNOWN | Audit separately (AUDIT-02); verify ConspitLink signature status |

**Phase 111 BLOCKED until:** Sectigo OV certificate is in hand (see Certificate Procurement section above).
