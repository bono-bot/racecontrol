# Stack Research

**Domain:** Anti-cheat compatible Windows pod management (sim racing venue)
**Researched:** 2026-03-21
**Confidence:** MEDIUM overall — code signing toolchain HIGH (official CA docs), Keyboard Filter HIGH (official MS docs), anti-cheat detection behavior MEDIUM (derived from ecosystem evidence, no official EAC API)

---

## Context: What This Research Covers

v15.0 AntiCheat Compatibility is a hardening milestone on top of an existing validated Rust/Axum stack. This file covers ONLY the net-new toolchain additions needed:

1. Code signing toolchain for `rc-agent.exe` and `rc-sentry.exe`
2. Safe keyboard lockdown replacement for `SetWindowsHookEx`
3. Anti-cheat detection behavior model (what triggers EAC, iRacing EOS, etc.)
4. Testing approach for per-game compatibility validation

Existing stack (Rust 1.93.1, Axum, Tokio, windows-service, winapi/windows crates) is NOT re-researched here.

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `signtool.exe` (Windows SDK) | Ships with Windows SDK 10.0.26100+ (Win11 24H2 SDK) | Sign `rc-agent.exe` and `rc-sentry.exe` post-build | Industry-standard Authenticode signer. Already available on James's machine (Windows SDK). Zero new install. Produces signed PE that EAC validates. |
| Sectigo OV Code Signing Certificate | 1-year max (CA/B Forum cap since Feb 2026) | Authenticode certificate for Racing Point eSports org | ~$225/yr via resellers (ssl2buy, cheapsslsecurity). OV is sufficient for venue-internal binaries. EV (~$280/yr) gives instant SmartScreen reputation but requires HSM hardware token — unnecessary complexity for venue-only distribution. |
| Windows Keyboard Filter (built-in OS feature) | Windows 10 Enterprise 1607+ / Windows 11 Enterprise/Education | Replace `SetWindowsHookEx` global keyboard hook for kiosk lockdown | Built-in Optional Feature configured via WMI — no third-party kernel driver installed. Suppresses Ctrl+Alt+Delete, Win+L, Alt+F4 etc. at OS level without appearing in the game process hook chain. This is the architectural difference vs. SetWindowsHookEx: Keyboard Filter does not inject into the game process's hook list. |
| Windows Local Group Policy (built-in) | All Windows 11 editions | Suppress Win key hotkeys and system shortcuts for kiosk user account | Zero-risk complement to Keyboard Filter. `User Configuration > Administrative Templates > Windows Components > File Explorer > Turn off Windows key hotkeys`. No drivers, no hooks, applies per-user. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `signtool-rs` | 0.1.x (crates.io) | Rust wrapper that auto-locates `signtool.exe` in Windows SDK without hardcoding SDK version path | Use in `build.rs` or deploy script to sign release builds. Eliminates fragile path like `C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe` that breaks on SDK updates. |
| `windows` crate (microsoft/windows-rs) | 0.58+ (already in workspace) | WMI COM calls to configure Keyboard Filter rules (`WEKF_PredefinedKey`, `WEKF_CustomKey`) | Use in rc-agent's safe-mode initialization to enable/configure Keyboard Filter via `Windows::Win32::System::Wmi`. Replaces the Phase 78 `SetWindowsHookEx` module. |
| `wmi` crate | 0.13.x | Higher-level serde-based WMI query wrapper | Optional alternative to raw windows-rs WMI COM. Use only if raw CIM calls become verbose. Not currently in workspace — adds one new Cargo.lock entry. |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `signtool.exe` | Sign `.exe` after `cargo build --release` | Path: `C:\Program Files (x86)\Windows Kits\10\bin\10.0.26100.0\x64\signtool.exe`. Always use `/fd SHA256 /tr http://timestamp.sectigo.com/rfc3161 /td SHA256`. The RFC 3161 timestamp is mandatory — it makes the signature valid after the cert expires (otherwise signatures break at cert renewal). |
| DISM (built-in) | Enable Keyboard Filter optional feature on pods | `Dism /online /Enable-Feature /FeatureName:Client-KeyboardFilter /NoRestart` — requires reboot. Run once per pod during setup, before any game sessions. |
| Pod 8 canary | Manual anti-cheat compatibility validation | Run each protected game on Pod 8 with signed rc-agent and Keyboard Filter active. Observe: game launches, EAC does not block, Windows Event Log shows no driver conflicts. Document per game. |

---

## Installation

```bash
# [target: pod] Enable Keyboard Filter once per pod (requires admin + reboot)
Dism /online /Enable-Feature /FeatureName:Client-KeyboardFilter /NoRestart
# Reboot, then configure suppressed keys via WMI:
```

```powershell
# [target: pod] Configure Keyboard Filter rules after reboot
$ns = "root\standardcimv2\embedded"
# Suppress Ctrl+Alt+Delete, Win+L, Win key (breakout escape)
$suppress = @("Ctrl+Alt+Del", "Win+L", "Win", "Alt+F4", "Ctrl+Shift+Esc")
foreach ($key in $suppress) {
    $rule = Get-CimInstance -Namespace $ns -ClassName WEKF_PredefinedKey |
            Where-Object { $_.Id -eq $key }
    if ($rule) {
        $rule.Enabled = $true
        Set-CimInstance -InputObject $rule
    }
}
```

```bash
# [target: james] Sign rc-agent.exe after cargo build --release
signtool.exe sign /fd SHA256 \
  /tr http://timestamp.sectigo.com/rfc3161 \
  /td SHA256 \
  /n "Racing Point eSports" \
  target\release\rc-agent.exe

# Verify signature
signtool.exe verify /pa /v target\release\rc-agent.exe
```

```toml
# Cargo.toml — add to rc-agent build-dependencies if automating signing
[build-dependencies]
signtool-rs = "0.1"
```

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| Sectigo OV Code Signing (~$225/yr) | DigiCert OV (~$370/yr) | DigiCert when enterprise support SLA is needed. Functionally identical for Authenticode. |
| Sectigo OV (~$225/yr) | Any EV certificate (~$280-560/yr) | EV gives instant SmartScreen reputation bar and is worth it for public software distribution. For venue-only pods, OV is sufficient. EV requires FIPS 140-2 Level 2 HSM (physical token or cloud HSM), adding friction to the deploy workflow. |
| Windows Keyboard Filter (built-in) | `SetWindowsHookEx(WH_KEYBOARD_LL)` — current Phase 78 implementation | SetWindowsHookEx is acceptable on machines that never run anti-cheat games. On pods running F1 25 (EAC), iRacing (EOS), LMU (EOS), or BattlEye games, global low-level keyboard hooks appear in the game process's hook chain and are flagged as potential input-injection vectors. |
| Windows Keyboard Filter (built-in) | Third-party kiosk software (Netkiosk, FrontFace) | Never use third-party kiosk products on gaming pods — they install their own kernel drivers, which is worse than SetWindowsHookEx. |
| Group Policy key suppression | AutoHotkey or Python scripting for key blocking | AHK and scripting runtimes inject into processes. Instant EAC detection. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `SetWindowsHookEx(WH_KEYBOARD_LL)` for kiosk lockdown | Global low-level hooks appear in every process's hook chain, including EAC-protected games. Input hook APIs are prime cheating vectors (input injection, macro automation) — EAC and BattlEye specifically monitor hook chain for external entries. | Windows Keyboard Filter (WMI-configured, no hook injection) + Group Policy |
| Unsigned `rc-agent.exe` / `rc-sentry.exe` | EAC validates signatures of co-running processes. The FanControl/WinRing0 issue confirms: unsigned binaries running alongside an EAC game are the primary trigger for flagging. OBS resolved EAC conflicts by publishing a signed capture hook certificate. Signing is the single highest-value change in v15.0. | Authenticode signing via signtool.exe + Sectigo OV cert |
| Any kernel-mode driver for venue management | Kernel drivers are the #1 trigger for EAC "Forbidden Windows Kernel Modification" and BattlEye kernel integrity errors. WinRing0 (used by FanControl) is the canonical example: even legitimate hardware monitoring drivers are flagged because they expose exploitable memory access primitives. rc-agent has zero need for kernel drivers. | Stay entirely in user-mode. All current rc-agent operations (process management, registry writes, WebSocket, HTTP) are user-mode and must remain so. |
| `ReadProcessMemory` / `WriteProcessMemory` on game processes | EAC hooks these Win32 APIs in user-mode and catches callers instantly. Any process calling these on an EAC-protected game PID is immediately flagged regardless of signature. | Use official game telemetry APIs only: iRacing MemMapFile, rF2/LMU shared memory plugin, F1 25 UDP, EA WRC UDP — these are explicitly sanctioned. |
| Debug APIs (`OpenProcess(PROCESS_VM_READ)`, `DebugActiveProcess`) | Same detection mechanism as ReadProcessMemory. rc-sentry v11.2 already correctly avoids this by using HTTP health polling. | HTTP polling `localhost:8090/health` (already implemented) |
| AutoHotkey, AHK2, Python scripting runtimes | Spawn additional processes, may inject hooks. EAC watches for scripting runtimes that could wrap game input. | Native Rust Win32 calls only |
| DLL injection into game processes | Instant account ban regardless of signing. Even OBS's signed game capture hook requires per-game EAC whitelisting — and the whitelisting is done by the game developer, not by the software author. rc-agent has no legitimate reason to ever inject into a game. | Read telemetry from external interfaces only |
| Continuous process enumeration while anti-cheat games run | Polling `CreateToolhelp32Snapshot` or `sysinfo::processes()` continuously while EAC is active is flagged as process monitoring (a cheating technique). Snapshot is safe for one-time startup detection; NOT safe as a continuous 1-second loop beside a live EAC game. | Health endpoint polling + safe mode that suspends process enumeration during protected game sessions |

---

## Stack Patterns by Variant

**For keyboard lockdown (kiosk account on pods):**
- Enable Keyboard Filter via DISM on first pod setup (one-time, requires reboot)
- Configure suppressed keys via WMI from rc-agent's initialization path (or deploy-time PowerShell script)
- Remove the `SetWindowsHookEx` call from the Phase 78 keyboard hook module
- Group Policy `Turn off Windows key hotkeys` as belt-and-suspenders for the kiosk user account

**For code signing (deploy pipeline):**
- Buy Sectigo OV certificate (1-year, digital key delivery — confirm HSM requirement with reseller before purchase)
- Add `signtool.exe` call to the deploy script immediately after `cargo build --release`
- Sign both `rc-agent.exe` and `rc-sentry.exe`
- Use RFC 3161 timestamp server (`http://timestamp.sectigo.com/rfc3161`) — mandatory for long-term validity

**For telemetry shared memory (v13.0 readiness):**
- iRacing: open `Local\IRSDKMemMapFileName` (official iRacing SDK MemMapFile) — sanctioned API, not process memory
- LMU/rF2: open rF2 shared memory plugin mappings (`$rFactor2SMMP_*`) — official plugin API
- Gate: open shared memory handles only after the game process is confirmed running AND billing session is active. Close handles on game exit. Never hold persistent open handles to MemMapFiles while EAC is initializing.

**For anti-cheat safe mode detection:**
- One-time `CreateToolhelp32Snapshot` on game launch event to identify which game launched (safe — snapshot, not attach)
- On protected game detected: disable any keyboard hook (N/A post-Keyboard Filter migration), suspend continuous process enumeration, defer shared memory open until after EAC init completes (~30s after game launch)
- On game exit: resume normal operations, close all shared memory handles

---

## Anti-Cheat Detection Behavior Model

Derived from: FanControl EAC issue #2104, OBS Capture Hook Certificate KB, EAC kernel driver conflict reports on Microsoft Q&A, iRacing EOS migration support notes. Confidence: MEDIUM (ecosystem evidence; no official EAC detection criteria document exists publicly).

| rc-agent Behavior | Detection Risk | Rationale | Verdict |
|-------------------|---------------|-----------|---------|
| Unsigned `rc-agent.exe` running alongside EAC game | HIGH | EAC checks signatures of co-running processes. Unsigned = suspicious. Primary root cause of FanControl flagging. | Fix: sign both binaries |
| `SetWindowsHookEx(WH_KEYBOARD_LL)` global hook | MEDIUM-HIGH | Hook appears in game process hook chain. Known input-injection vector. EAC and BattlEye scan hook chains. | Fix: migrate to Keyboard Filter |
| One-time `CreateToolhelp32Snapshot` for game detection | LOW | Snapshot-based, user-mode, same API Windows Task Manager uses. Does not attach to game process. | Safe as-is |
| Continuous `CreateToolhelp32Snapshot` loop while EAC running | MEDIUM | Continuous enumeration mimics cheat process monitoring behavior. | Safe with gate: suspend in safe mode |
| HTTP polling `localhost:8090/health` | NONE | Network call to local port, no process interaction whatsoever. | Safe, keep as-is |
| Shared memory MemMapFile read (official game API) | LOW with gating | Official game SDK interface. Risk window: opening handle during EAC initialization. Gate: open after game fully loaded. | Safe with gate |
| `OpenProcess(PROCESS_VM_READ)` on game PID | CRITICAL | EAC hooks this API. Instant flag. rc-agent does not do this — must stay that way. | Never do |
| Registry writes (HKLM Run, HKCU) | NONE | EAC does not monitor registry writes. Existing rc-agent behavior. | Safe |
| Port listener `0.0.0.0:8090` | NONE | TCP listener. Not monitored by EAC. | Safe |
| Windows Service hosting rc-agent | NONE | Standard Windows service mechanism, separate session from game. | Safe |
| Keyboard Filter (built-in Windows feature) | NONE | Microsoft OS feature. Not flagged by EAC. Distinct from third-party kernel drivers. | Safe |

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| Sectigo OV cert (2026+) | Windows Authenticode (all versions) | Max 459-day validity since Feb 2026 CA/B Forum rule. Budget annual renewal. |
| Windows Keyboard Filter | Windows 10 Enterprise 1607+ / Win11 Enterprise/Education | NOT available on Windows 11 Home or Pro (Home only). Pods must be on Enterprise or Education SKU. Verify with `winver` before implementing. |
| `signtool.exe` SHA256 | Windows Vista+ verifiers | SHA1 deprecated for Authenticode since 2020. Use `/fd SHA256 /td SHA256` everywhere. |
| `windows` crate 0.58 | Rust 1.70+ | Already in workspace. WMI interfaces under `Windows::Win32::System::Wmi`. |
| `CreateToolhelp32Snapshot` | All Windows versions | Safe for process detection. Part of standard Win32 API surface. |

---

## Open Question: OV Certificate HSM Key Storage (Verify Before Purchase)

Since November 2022, CA/B Forum requires all code signing private keys — including OV — to be stored on a FIPS 140-2 Level 2 hardware token or cloud HSM equivalent. Some CAs ship a physical USB token (SafeNet eToken) with OV certs; others offer cloud-based signing (DigiCert KeyLocker, SSL.com eSigner).

**Before buying:** confirm with the reseller whether the OV cert ships with:
- A **physical USB token** — signing can only happen on the machine with the token plugged in (workable for James's machine as sole build machine), OR
- **Cloud key storage** — CI-compatible, no physical token required, ~$50/yr more

For a single build machine at the venue, a physical token is workable. For any future CI pipeline, prefer cloud key storage (SSL.com eSigner at ~$249/yr for OV+cloud is the most CI-friendly option).

---

## Sources

- [Windows Keyboard Filter — Microsoft Learn](https://learn.microsoft.com/en-us/windows/configuration/keyboard-filter/) — official docs updated March 2025. Edition requirements, WMI configuration, Safe Mode limitation. HIGH confidence.
- [FanControl EAC Issue #2104](https://github.com/Rem0o/FanControl.Releases/issues/2104) — real-world evidence that unsigned kernel driver is the primary EAC trigger; user-mode signed binaries are not flagged. MEDIUM confidence (community issue tracker).
- [OBS Capture Hook Certificate Update KB](https://obsproject.com/kb/capture-hook-certificate-update) — confirms EAC resolves conflicts via signed binary certificate updates; game devs must opt-in to new cert hashes. MEDIUM confidence.
- [sslinsights.com — Code Signing Certificate Providers 2026](https://sslinsights.com/best-code-signing-certificate-providers/) — pricing verified across multiple reseller sites. LOW-MEDIUM confidence (reseller aggregator).
- [DigiCert Code Signing Changes 2023](https://knowledge.digicert.com/alerts/code-signing-changes-in-2023) — HSM requirement for all code signing certs from Nov 2022. HIGH confidence (official CA source).
- [signtool-rs crate](https://github.com/SecSamDev/signtool-rs) — Rust wrapper for signtool.exe auto-location. LOW confidence on production maturity (small crate, use as convenience shim only).
- [iRacing EOS migration notes](https://support.iracing.com/support/solutions/articles/31000173103-anticheat-not-installed-uninstalling-eac-and-installing-eos-) — iRacing moved from Kamu EAC to Epic EOS EAC in 2024/2025. MEDIUM confidence.
- [EAC kernel driver incompatibility — Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/3962392/easy-anti-cheat-driver-incompatible-with-kernel-mo) — EAC "Forbidden Windows Kernel Modification" triggered by third-party kernel drivers. HIGH confidence.
- GlobalSign SignTool guide — confirmed signtool.exe SHA256 + RFC 3161 timestamp command syntax. HIGH confidence.

---

*Stack research for: v15.0 AntiCheat Compatibility — Racing Point eSports RaceControl*
*Researched: 2026-03-21*
