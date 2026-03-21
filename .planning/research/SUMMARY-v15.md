# Project Research Summary: v15.0 AntiCheat Compatibility

**Project:** v15.0 AntiCheat Compatibility — Racing Point eSports RaceControl
**Domain:** Anti-cheat safe mode integration for sim racing venue management software
**Researched:** 2026-03-21
**Confidence:** MEDIUM overall — stack HIGH (official MS/CA docs), architecture HIGH (direct codebase inspection), features MEDIUM, pitfall behaviors MEDIUM (ecosystem evidence; anti-cheat vendors do not publish detection criteria)

---

## Executive Summary

Racing Point runs rc-agent alongside kernel-mode anti-cheat systems on every pod: F1 25 uses EA Javelin (EAAC), iRacing uses Epic EOS, LMU uses Epic EOS via the EAC SDK, and EA WRC uses EAAC. These systems operate kernel drivers that monitor process behavior system-wide — not just the game process. rc-agent currently exhibits several behaviors that overlap with known cheat tooling signatures: a global WH_KEYBOARD_LL hook (identical API path to AutoHotKey), continuous process enumeration with kill capability, and unsigned binaries. If v13.0 Multi-Game Launcher deploys to customers before v15.0 is complete, customer account bans are a near-certain outcome on F1 25, iRacing, and LMU sessions.

The recommended approach is a behavioral safe mode built into rc-agent's existing AppState architecture. A new `safe_mode.rs` module holds a `SafeModeState` struct; entry is triggered by `LaunchGame` in `ws_handler.rs` for any game where `requires_safe_mode(sim) == true`; exit is triggered by the existing 30-second exit grace timer. The keyboard hook must be permanently replaced by registry-based Group Policy key suppression — not gated, replaced. Code signing with a Sectigo OV certificate must be in the build pipeline before any protected game is enabled for customers. These two changes (hook replacement + signing) eliminate the highest-risk signals before safe mode runtime gating handles the rest.

The critical risk to this milestone is the timing race: EAC initializes within 2-5 seconds of game launch, but poll-based game detection can miss that window. The fix is WMI `Win32_ProcessStartTrace` event subscription, which fires within milliseconds of process creation. On exit, a mandatory 30-second cooldown is required because EA Javelin continues monitoring after the game process exits. The safe mode must also be the startup default so rc-agent restarts mid-session do not expose the hook to an already-running anti-cheat driver. Windows Keyboard Filter is the preferred long-term replacement but requires Enterprise/Education SKU — pods on Windows 11 Pro must use GPO registry keys as the safe path.

---

## Key Findings

### Recommended Stack

v15.0 adds no new crates to the workspace. The net-new toolchain is `signtool.exe` (already on James's machine via Windows SDK) with a Sectigo OV code signing certificate (~$225/yr, or ~$249/yr for cloud key storage via SSL.com eSigner which is CI-compatible). The `windows` crate at 0.58+ (already in workspace) provides WMI COM calls for Keyboard Filter configuration if pods are ever on Enterprise SKU. Group Policy registry key writes use standard Win32 registry APIs already available.

**Core technologies:**

- `signtool.exe` + Sectigo OV certificate: sign `rc-agent.exe` and `rc-sentry.exe` post-build — eliminates the unsigned binary risk signal that contributes to EAC composite scoring
- WMI `Win32_ProcessStartTrace` subscription: event-driven game detection replacing poll-based detection — fires within milliseconds of process creation, closes the timing race with anti-cheat driver initialization
- Registry GPO key writes (`NoWinKeys=1`, `DisableTaskMgr=1`): replace `SetWindowsHookEx` for kiosk lockdown on Windows 11 Pro pods — no hook injection, no process-level visibility to anti-cheat
- `safe_mode.rs` new module: SafeModeState struct + exhaustive `requires_safe_mode(SimType)` match — compile error when a new game is added without updating the safe mode classification

**Critical version constraint:** Windows Keyboard Filter requires Enterprise or Education SKU. Pods on Windows 11 Pro must use GPO registry keys. Verify pod OS edition with `winver` before any Keyboard Filter work.

**Open question before implementation:** OV certificate HSM requirement — confirm with reseller whether the certificate ships with a physical USB token (SafeNet eToken) or cloud key storage. For the single build machine at the venue, a physical token is workable. Cloud key storage is required for any future CI pipeline.

### Expected Features

**Must have (table stakes — missing = customer bans):**

- TS-1: Protected game detection from `GameProfile.anti_cheat_tier` in TOML — foundation for all safe mode behavior
- TS-2: Safe mode state machine in `app_state.rs` — `Normal → SafeMode` on LaunchGame for protected games
- TS-3: Keyboard hook suspension on SafeMode entry — hook must be unregistered, not paused
- TS-4: GPO registry key application on SafeMode entry (`NoWinKeys=1`, `DisableTaskMgr=1`) — compensates for suspended hook on Windows 11 Pro pods
- TS-5: Process allowlist enforcement disabled in safe mode — no PID-based kill operations during protected game sessions
- TS-6: Telemetry reads gated per game — iRacing SDK reads always safe (iRacing staff confirmed), F1 25/WRC UDP always safe, LMU shared mem read-only safe, AC EVO feature-flagged off
- TS-7: Safe mode deactivation with 30-second cooldown after game exit and anti-cheat process gone
- TS-9: Pod 8 canary test sessions per game before fleet rollout

**Should have (raise quality above minimum viable):**

- TS-8: Code sign both binaries with OV certificate — reduces composite risk score before fleet deployment (P2; not a hard ban trigger alone but required for honest v15.0 claim)
- TS-10: Anti-cheat compatibility matrix in racecontrol.toml + ops reference doc
- D-1: Safe mode audit log per session at `C:\RacingPoint\safe-mode.log` — evidence for ban dispute appeals
- D-3: Per-game TOML anti-cheat profile driving safe mode logic (anti_cheat tier, subsystem toggles)
- D-4: Billing continuity integration test — safe mode activation must not corrupt billing_guard state

**Defer to v15.2:**

- D-2: ConspitLink signing audit — medium risk, requires test environment session with Process Monitor
- D-5: Anti-cheat process watchdog (detects unexpected AC exit during session, staff alert)
- D-6: `safe_mode` field in PodFleetStatus for dashboard visibility

### Architecture Approach

Safe mode integration is minimal-footprint: one new module (`safe_mode.rs`) and four modified files (`app_state.rs`, `ws_handler.rs`, `event_loop.rs`, `kiosk.rs`/`process_guard.rs`). No new crate is needed. `SafeModeState` lives in `AppState` so it survives WebSocket reconnections — the game keeps running through a transient disconnect and must stay in safe mode throughout. The `requires_safe_mode(sim)` function is an exhaustive match on `SimType` so a compile error fires when v13.0 adds a new game without updating the safe mode classification.

**Major components:**

1. `safe_mode.rs` (NEW) — SafeModeState struct, `enter()`/`exit()` transitions, `requires_safe_mode()` exhaustive match table
2. `ws_handler.rs` LaunchGame arm (MODIFIED) — calls `safe_mode.enter(sim)` before `game_process::launch()`; BillingStopped/SessionEnded arms call `safe_mode.exit()` as belt-and-suspenders
3. `event_loop.rs` exit grace path (MODIFIED) — calls `safe_mode.exit()` when exit_grace_timer fires (30s after game process death)
4. `kiosk.rs` + `process_guard.rs` kill-paths (MODIFIED) — check `state.safe_mode.active` before any kill operation; CRITICAL-tier kills (racecontrol.exe on a pod) exempt from safe mode gate
5. `event_loop.rs` telemetry branch (MODIFIED) — `shm_connect_allowed()` guard defers iRacing/LMU adapter `connect()` until 5 seconds after game is live

**Key architectural decision:** Safe mode is a positive-enable allowlist in safe mode, not a negative-disable blocklist. Only explicitly approved operations proceed during a protected game session: health endpoint polling, WebSocket keepalive, billing heartbeat, active-game UDP telemetry receive, session timer tick. Everything else is off by default and requires an explicit gate check.

### Critical Pitfalls

1. **SetWindowsHookEx keyboard hook — MUST be fully replaced, not gated.** Hook is installed at rc-agent startup. Even if safe mode disables it dynamically, the install/uninstall cycle happens while EAC is active if timing is wrong. Replace with GPO registry key writes (NoWinKeys, DisableTaskMgr). Full replacement is a prerequisite before v15.0 canary testing starts.

2. **Safe mode entry timing race — WMI event subscription required, not polling.** EAC initializes within 2-5 seconds of game launch. A 5-second poll interval means safe mode may engage after EAC has already fingerprinted rc-agent's hook. Use `Win32_ProcessStartTrace` WMI event subscription — fires milliseconds after process creation. Also: rc-agent startup must default to safe mode if any protected game process is already running.

3. **Safe mode exit timing race — 30-second cooldown mandatory.** EA Javelin and EAC continue monitoring 10-30 seconds after game process exits. Restoring the keyboard hook during this window is detectable. 30-second cooldown after game exit, then verify anti-cheat process absence before deactivation.

4. **Process guard calling OpenProcess near game PIDs.** The v12.1 Process Guard enumerates all processes and kills non-whitelisted entries. If the game PID is not yet on the whitelist when it launches, the guard may attempt `TerminateProcess()` on an anti-cheat-protected process. Gate entire process guard (all kill operations) behind safe mode. CRITICAL-tier guard (prevents racecontrol.exe on pods) is the only exception.

5. **Unsigned binaries — certificate is a prerequisite for fleet deployment.** An unsigned rc-agent.exe performing keyboard hooking + process enumeration + network listening matches the cheat tool profile signature. EA Javelin builds a hardware fingerprint per pod. A ban on Pod 1 can propagate to a customer's account permanently. Certificate procurement must happen during Phase 1, before canary testing.

6. **Shared memory telemetry must use MapViewOfFile, not ReadProcessMemory.** `ReadProcessMemory` on a game PID is an immediate hard ban trigger — EAC hooks this API. iRacing and LMU telemetry must use `OpenFileMapping` + `MapViewOfFile` on named shared memory objects. Never obtain a handle to the game process PID.

7. **Ollama on pods must not query GPU during protected game sessions.** Local Ollama (v8.0 LLM process classifier) makes CUDA syscalls to GPU memory. EA Javelin profiles GPU memory access patterns from non-game processes. Gate all Ollama queries behind safe mode. Set `OLLAMA_KEEP_ALIVE=0` on pods to free VRAM between queries.

8. **Safe mode incomplete coverage — build an allowlist, not a blocklist.** Behaviors from v4.0 (registry self-healing), v8.0 (LLM classifier), and v12.1 (process guard) are not automatically reviewed against safe mode. Design safe mode as an explicit positive allowlist of approved operations. Create a `safe_mode_subsystems` section in racecontrol.toml.

---

## Implications for Roadmap

Based on combined research, the milestone should be structured into 5 phases. The feature dependency chain is: detection (TS-1) drives state machine (TS-2), which gates all risky subsystems. Hook replacement (TS-3/TS-4) must be complete before any protected game is enabled — it cannot be deferred. Signing (TS-8) must be complete before fleet rollout.

### Phase 1: Behavior Audit and Certificate Procurement

**Rationale:** Cannot build safe mode correctly without knowing every risky behavior in the codebase. Cannot deploy to customers without signed binaries. Both are prerequisite, non-blocking work that unblocks all later phases.
**Delivers:** Exhaustive inventory of all risky rc-agent behaviors; code signing certificate procured and integrated into build pipeline; ConspitLink signing status audited; HKLM registry write paths cataloged.
**Addresses:** TS-8 (certificate), TS-10 (compatibility matrix foundation), Pitfall 5 (unsigned binaries), Pitfall 9 (ConspitLink audit), Pitfall 6 (registry write inventory)
**Avoids:** Shipping unsigned binaries to customers; missing a risky behavior when designing safe mode gates
**Research flag:** STANDARD PATTERNS — certificate procurement is well-documented; ConspitLink audit requires hands-on test session on Pod 8 (no research needed, execution needed)

### Phase 2: Keyboard Hook Replacement

**Rationale:** The `SetWindowsHookEx` hook is the single highest-risk behavior. It must be fully replaced before any EAC/EAAC game is launched on a pod. This is not a safe mode gate — it is a full replacement. Blocking dependency for all subsequent phases.
**Delivers:** `SetWindowsHookEx` removed from rc-agent source; GPO registry key writes (`NoWinKeys=1`, `DisableTaskMgr=1`) applied on safe mode entry and removed on exit; kiosk lockdown verified working without any hook on Pod 8
**Addresses:** TS-3 (hook suspension), TS-4 (GPO replacement), Pitfall 1 (keyboard hook the single highest-risk behavior)
**Avoids:** Hook install/uninstall cycle being visible to a running anti-cheat driver
**Research flag:** STANDARD PATTERNS — Win32 registry writes are well-documented; GPO key names are confirmed in FEATURES research

### Phase 3: Safe Mode State Machine

**Rationale:** With the hook replaced, the state machine can be built to gate remaining risky subsystems. WMI event subscription for game detection closes the timing race. The startup-default-to-safe-mode requirement must be in the design spec before implementation begins.
**Delivers:** `safe_mode.rs` module; `SafeModeState` in AppState; WMI `Win32_ProcessStartTrace` subscription for game detection; 30-second exit cooldown; process guard gated; Ollama queries gated; registry writes gated; safe mode startup default
**Addresses:** TS-1 (game detection), TS-2 (state machine), TS-5 (process kill gating), Pitfall 2 (process guard), Pitfall 3 (timing races), Pitfall 6 (registry writes), Pitfall 8 (Ollama), Pitfall 10 (allowlist model)
**Avoids:** Poll-based detection creating a 5-second window where EAC sees risky behaviors; safe mode missing non-obvious subsystems like Ollama or registry self-healing
**Research flag:** NEEDS DEEPER RESEARCH — WMI `Win32_ProcessStartTrace` subscription in Rust (windows crate 0.58 WMI event interfaces) needs implementation-level research; confirm subscription cleanup path to avoid memory leak (Pitfall performance trap)

### Phase 4: Telemetry Gating and Shared Memory Safety

**Rationale:** Telemetry adapters for iRacing and LMU use MapViewOfFile (correct approach confirmed in existing code) but need explicit timing gates. UDP telemetry sockets must be scoped to active game sessions. AC EVO must be feature-flagged off until anti-cheat status is confirmed.
**Delivers:** `shm_connect_allowed()` guard in event_loop.rs deferring adapter connect by 5s for protected games; UDP telemetry sockets opened only for the active game, closed on game exit; AC EVO telemetry feature-flagged off; per-game TOML anti-cheat profile (D-3); D-1 safe mode audit log
**Addresses:** TS-6 (telemetry read safety), TS-10 (compatibility matrix), D-1 (audit log), D-3 (per-game TOML profile), Pitfall 4 (shared memory vs ReadProcessMemory), Pitfall 7 (port listener profile)
**Avoids:** Opening shared memory handles during EAC driver initialization window; high-frequency mapping loop during game startup; UDP socket composite risk profile
**Research flag:** STANDARD PATTERNS — iRacing SDK shared memory interface is documented; rF2/LMU plugin SDK is documented; no new research needed

### Phase 5: Per-Game Canary Validation

**Rationale:** No simulation of anti-cheat behavior is reliable enough to confirm safety. Each game must be played in a full staff session on Pod 8 with real game accounts before any customer session runs. This is not optional.
**Delivers:** Signed binaries on Pod 8; staff test session per game (F1 25, iRacing, LMU — at minimum); Windows Event Log reviewed for conflicts; rc-agent logs reviewed for safe mode entry/exit timing; billing continuity verified during safe mode (D-4); anti-cheat compatibility matrix finalized
**Addresses:** TS-9 (Pod 8 validation), D-4 (billing continuity test), Pitfall 9 (ConspitLink runtime compatibility)
**Avoids:** First real customer session discovering ban-triggering behavior; deploying fleet without per-game evidence
**Research flag:** EXECUTION ONLY — test protocol is clear; no research needed; results determine whether any phase 3/4 rework is needed before fleet rollout

### Phase Ordering Rationale

- Phase 1 (Audit + Certificate) must be first because safe mode design depends on knowing all risky behaviors, and the certificate takes procurement time (1-5 business days delivery typical).
- Phase 2 (Hook Replacement) must be before Phase 3 because the state machine's hook-on/hook-off logic does not exist once the hook is fully replaced — Phase 3 only needs to apply/remove GPO keys.
- Phase 3 (State Machine) before Phase 4 because telemetry gating uses the safe mode state; without the state machine, the `shm_connect_allowed()` guard has nothing to check.
- Phase 5 (Validation) last because it validates Phases 2-4 together. If any phase produces unexpected results, Phase 5 is the catch.
- v13.0 Multi-Game Launcher MUST NOT deploy to customer pods until Phase 5 complete for each enabled game.

### Research Flags

Phases needing deeper research during planning:
- **Phase 3:** WMI `Win32_ProcessStartTrace` subscription in Rust via the `windows` crate — confirm event interface names, subscription cleanup path, error handling when WMI service is unavailable. This is the most technically novel piece of the milestone.

Phases with standard patterns (skip research-phase):
- **Phase 1:** Certificate procurement is vendor-driven; ConspitLink audit is hands-on test execution
- **Phase 2:** GPO registry key names and `SetWindowsHookEx` removal are well-documented
- **Phase 4:** iRacing SDK and rF2/LMU shared memory interfaces are documented; no new interfaces required
- **Phase 5:** Test protocol is execution, not research

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | signtool.exe and GPO registry keys are official Microsoft documentation. Certificate pricing verified via multiple reseller sources. OV HSM requirement is official CA/B Forum + DigiCert documentation. |
| Features | MEDIUM | TS-1 through TS-9 derived from codebase inspection (HIGH confidence on what exists) + anti-cheat behavior research (MEDIUM — vendors do not publish detection criteria). iRacing telemetry safety is HIGH (iRacing staff explicit confirmation). EA Javelin exact triggers are MEDIUM (EA blog + community). |
| Architecture | HIGH | Based on direct codebase inspection of rc-agent v11.0 source. Component modification list is exhaustive. SafeModeState design is derived from existing AppState patterns in the codebase. No speculative components. |
| Pitfalls | HIGH for EAC/Javelin general patterns, MEDIUM for AC EVO specifics | EAC kernel hook scanning, EA Javelin process profiling, and iRacing EOS memory protection are confirmed via academic analysis + official documentation. AC EVO anti-cheat system is unconfirmed (Early Access). ConspitLink driver behavior is unknown until audited. |

**Overall confidence:** MEDIUM-HIGH — the architecture and safe mode design are solid; the remaining uncertainty is in anti-cheat vendor-specific thresholds (intentionally opaque) and AC EVO's eventual enforcement posture.

### Gaps to Address

- **OV Certificate HSM delivery model:** Confirm with reseller before purchase whether the cert ships with physical USB token or cloud key storage. This affects whether signing can be automated in the build pipeline. If physical token only, signing must happen on James's machine manually on every release build.
- **Pod OS edition:** Verify all 8 pods are on Windows 11 Enterprise or Education before any Keyboard Filter work. If on Windows 11 Pro, GPO registry keys are the only safe path. Do not assume — check `winver` on each pod.
- **ConspitLink signing status:** Unknown until Phase 1 audit. If unsigned and using a kernel driver, this is a separate ban risk that safe mode cannot mitigate. Conspit contact and/or EAC allowlisting request may be needed.
- **AC EVO anti-cheat at full release:** AC EVO is in Early Access (v0.5.4, Jan 2026). No anti-cheat system confirmed. Treat as `anti_cheat_tier = "unknown"` and activate full safe mode. Reassess at v1.0 release — the shared memory telemetry feature flag must remain off until then.
- **EA Javelin exact exit cooldown duration:** Research indicates 10-30 seconds post-game monitoring. The 30-second cooldown is the conservative safe choice. Pod 8 canary testing should attempt to verify the actual minimum safe cooldown for F1 25 specifically.

---

## Sources

### Primary (HIGH confidence)

- [Windows Keyboard Filter — Microsoft Learn](https://learn.microsoft.com/en-us/windows/configuration/keyboard-filter/) — edition requirements (Enterprise/Education only), WMI configuration, Safe Mode limitation
- [DigiCert Code Signing Changes 2023](https://knowledge.digicert.com/alerts/code-signing-changes-in-2023) — HSM requirement for all code signing certs since Nov 2022
- [EAC kernel driver incompatibility — Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/3962392/easy-anti-cheat-driver-incompatible-with-kernel-mo) — "Forbidden Windows Kernel Modification" triggered by third-party kernel drivers
- iRacing staff confirmation (Randy Cassidy via XSimulator forum): "Use of the iRacing telemetry system will not cause EAC to trigger any issues"
- rc-agent v11.0 codebase (direct inspection) — component inventory, existing anti-cheat risk surface, AppState patterns

### Secondary (MEDIUM confidence)

- [FanControl EAC Issue #2104](https://github.com/Rem0o/FanControl.Releases/issues/2104) — unsigned/vulnerable driver adjacent to game caused EAC ban; user-mode signed binaries not flagged
- [OBS Capture Hook Certificate Update KB](https://obsproject.com/kb/capture-hook-certificate-update) — EAC resolves conflicts via signed binary cert updates; game devs must accept new cert hashes
- [EA AntiCheat deep-dive blog](https://www.ea.com/security/news/eaac-deep-dive) — kernel-mode driver, memory sandbox, shuts down on game exit; vague on specifics
- [LMU v1.2 EAC announcement](https://simulationdaily.com/news/le-mans-ultimate-v1-2-update/) — EAC "blocks access to write to memory locations and will crash the game if players start interfering"
- Academic anti-cheat survey: [arxiv.org/html/2408.00500v1](https://arxiv.org/html/2408.00500v1) — kernel-level AC handle scanning, hook detection, driver monitoring

### Tertiary (LOW confidence)

- [sslinsights.com — Code Signing Certificate Providers 2026](https://sslinsights.com/best-code-signing-certificate-providers/) — Sectigo OV pricing ~$225/yr, SSL.com eSigner ~$249/yr; reseller aggregator, pricing subject to change
- [PCGamingWiki AC EVO](https://www.pcgamingwiki.com/wiki/Assetto_Corsa_EVO) — no anti-cheat system listed as of 2026-03-21; Early Access status means this may change at full release
- AutoHotKey/EAC forum thread — SetWindowsHookEx flagged as AHK cheat tool; community source, confirmed by broader pattern evidence

---

*Research completed: 2026-03-21 IST*
*Ready for roadmap: yes*
*Prerequisite gate: v13.0 Multi-Game Launcher must not deploy to customer pods until v15.0 Phase 5 canary validation is complete for each enabled game*
