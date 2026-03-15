# Stack Research

**Domain:** Pod Fleet Self-Healing — Windows Service, Firewall, Registry, Fleet Dashboard
**Researched:** 2026-03-15
**Confidence:** HIGH (existing codebase verified directly; new crates verified via crates.io, GitHub, official docs)

---

> **Note:** This file covers v4.0 Pod Fleet Self-Healing stack additions ONLY.
> The v3.0 Leaderboard/Telemetry stack and all prior additions remain unchanged.
> This research answers four specific questions: Windows Service wrapper, Firewall management,
> Registry management, and Fleet health dashboard transport.

---

## Context: What Already Exists (Do Not Re-add)

| Technology | Version | Role |
|------------|---------|------|
| Rust/Axum | 0.8 | rc-agent HTTP server on port 8090 (remote_ops.rs) |
| tokio | 1 (full) | Async runtime in both rc-core and rc-agent |
| tokio-tungstenite | 0.26 | WebSocket client in rc-agent → rc-core |
| axum | 0.8 | Both rc-core (port 8080) and rc-agent (port 8090) |
| winapi | 0.3 | Already in rc-agent for Windows process management |
| serde, serde_json | 1 | Serialization throughout |
| tracing, tracing-appender | 0.1/0.2 | Structured logging |
| anyhow, thiserror | 1/2 | Error handling |
| Next.js kiosk | 16.1.6 | Staff dashboard on port 3300 |
| useKioskSocket | existing | WebSocket hook connecting to `ws://...:8080/ws/dashboard` |

**Critical existing constraint from PROJECT.md:**
> "No new dependencies: Use existing crate deps where possible (tokio, reqwest, serde, chrono, tracing)"

Every new crate added below is justified by a capability gap — no existing dep covers it.

---

## New Stack Additions Required

### 1. Windows Service: `windows-service` crate

**Recommendation: `windows-service = "0.8"` (native Rust ServiceMain)**

Do NOT use NSSM, WinSW, or shawl as external wrappers.

**Why `windows-service` over NSSM:**

NSSM has not been updated since 2017. Its last release (2.24) predates Windows 11. It ships as an external binary that must be bundled separately, added to the deploy kit, and installed idempotently on every pod. Most critically, NSSM wraps the process externally — it cannot participate in the Rust shutdown sequence, cannot signal the existing `tokio_util::CancellationToken` pattern, and cannot report structured startup errors back to rc-core over the WebSocket before dying. The Feb 2026 David Hamann article on writing Windows services in Rust (https://davidhamann.de/2026/02/28/writing-a-windows-service-in-rust/) confirms the `windows-service 0.8` crate integrates cleanly with a tokio runtime and `CancellationToken`.

**Why `windows-service` over `sc.exe`:**

`sc.exe create` installs a service but provides no service control loop — the binary still needs to call `StartServiceCtrlDispatcher`. Without a `ServiceMain`, Windows kills the process in ~30 seconds with error 1053 ("service did not respond"). `sc.exe` is only the installer; `windows-service` provides the runtime protocol.

**Why `windows-service` over `shawl`/`WinSW`:**

Shawl is Rust-written but externally wraps any executable — same limitation as NSSM regarding structured shutdown and error reporting. WinSW is .NET-based, requiring .NET runtime on pods. Both are correct choices for apps you cannot modify; rc-agent is our own code.

**Session 0 vs Session 1 — THE critical gotcha:**

All Windows services run in Session 0. rc-agent currently shows a GUI (lock screen overlay using WINAPI). A service in Session 0 cannot create visible windows in Session 1 (the logged-in user session). This is a hard OS constraint on Windows Vista+ — there is no "allow interact with desktop" checkbox that works on Windows 10/11.

**Architecture to handle Session 0/1 split:**
- The Windows Service (`windows-service` crate) runs the non-GUI logic in Session 0: WebSocket, remote_ops, billing, heartbeat, HID polling
- A separate lightweight Session 1 helper process (`rc-agent-ui.exe`) handles ONLY the lock screen overlay and kiosk window
- The service spawns `rc-agent-ui.exe` via `CreateProcessAsUser` into the active user's session, or uses a named pipe / local WebSocket for IPC
- If rc-agent currently must show UI, the split is mandatory. If GUI responsibility can move to the kiosk (already runs in Session 1), eliminate `rc-agent-ui.exe` entirely

**Recommendation:** Move lock screen to kiosk (it runs in Session 1 via HKLM Run already). The service handles all non-UI logic. This eliminates the Session 0/1 split entirely and is the cleanest path.

**Tokio integration pattern (HIGH confidence — mullvad/windows-service-rs GitHub + Hamann 2026 article):**

```rust
// main.rs — dual-mode: service or console depending on launch context
fn main() -> anyhow::Result<()> {
    // When launched by SCM, run as service
    if std::env::args().any(|a| a == "--service") {
        service_dispatcher::start("rc-agent", ffi_service_main)?;
    } else {
        // Direct console launch (dev/debug mode)
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(run_agent())?;
    }
    Ok(())
}

define_windows_service!(ffi_service_main, windows_service_main);

fn windows_service_main(args: Vec<OsString>) {
    // tokio runtime created HERE, not in main()
    // because service_dispatcher::start blocks the main thread
    let rt = tokio::runtime::Runtime::new().unwrap();

    let shutdown_token = CancellationToken::new();
    let token_clone = shutdown_token.clone();

    let status_handle = service_control_handler::register("rc-agent", move |ctrl| {
        match ctrl {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                token_clone.cancel();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    }).unwrap();

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        ..ServiceStatus::default()
    }).unwrap();

    rt.block_on(run_agent_with_shutdown(shutdown_token));

    status_handle.set_service_status(ServiceStatus {
        current_state: ServiceState::Stopped,
        ..ServiceStatus::default()
    }).unwrap();
}
```

**Service installation from Rust (no sc.exe dependency):**

The `windows-service` crate includes `ServiceManager` and `Service` types for programmatic install/uninstall. Call `ServiceManager::local_computer(None, ServiceManagerAccess::CREATE_SERVICE)` from a one-time `rc-agent --install` subcommand. No external installer script needed.

**`windows-service` crate details:**
- Crate: `windows-service` on crates.io
- Current version: 0.8.0 (9 total published versions; 2.8M downloads)
- Maintained by: Mullvad VPN (production-grade, used in their Windows VPN client)
- GitHub: https://github.com/mullvad/windows-service-rs
- No additional Windows SDK dependencies beyond what winapi already provides

**Cargo.toml addition (rc-agent, Windows-only):**

```toml
[target.'cfg(windows)'.dependencies]
windows-service = "0.8"
tokio-util = { version = "0.7", features = ["rt"] }  # for CancellationToken
```

Note: `tokio-util` with `CancellationToken` is the shutdown coordination primitive. Verify whether it's already in the workspace — if not, add it here. `CancellationToken` is the recommended pattern per the Tokio docs for cooperative task cancellation.

---

### 2. Firewall Management: `std::process::Command` calling `netsh` — no new crate

**Recommendation: Call `netsh advfirewall firewall` via `std::process::Command`. No new crate.**

This directly satisfies the project constraint "no new dependencies where existing deps cover it."

**Why NOT `windows_firewall` crate (0.3.0, lhenry-dev):**

The `windows_firewall` crate (https://crates.io/crates/windows_firewall, current version 0.3.0) wraps the Windows Firewall COM API. It is maintained by a single developer with low download count and no notable production adoption. The v4.0 requirement is narrow: add two rules on startup (ICMP + TCP 8090), idempotently. A 20-line function using `std::process::Command` is more auditable, less risky to upgrade, and requires no COM initialization in the tokio runtime.

**Why NOT Windows Filtering Platform (WFP) API via `wfp` crate:**

WFP is the kernel-level packet filtering layer. It is appropriate for building firewalls, VPN clients, and network monitors. For adding named application firewall rules, WFP is the wrong abstraction layer — that's the job of the Windows Firewall service, which netsh controls. The `wfp` crate (https://crates.io/crates/wfp) is unmaintained (last commit 2021).

**Why NOT `winfw-rs`:**

`marirs/winfw-rs` is abandoned (last commit 2021, no crates.io release).

**Why netsh is correct for this use case:**

The CRLF-damaged batch file issue that caused the Mar 15 incident is a git/text-mode issue with `.bat` files, not a problem with netsh itself. Moving the netsh calls into Rust `Command::new("netsh")` eliminates the CRLF vector entirely since Rust strings have no line-ending ambiguity. The idempotency problem (duplicate rules) is solved by checking rule existence before adding.

**Implementation pattern (idempotent — no duplicate rules):**

```rust
// src/firewall.rs — new module in rc-agent
use std::process::Command;
use anyhow::Result;
use tracing::{info, warn};

const RULE_ICMP: &str = "RacingPoint-ICMP";
const RULE_TCP_8090: &str = "RacingPoint-RemoteOps-8090";

pub fn ensure_firewall_rules() -> Result<()> {
    ensure_rule(
        RULE_ICMP,
        &["protocol=icmpv4", "dir=in", "action=allow"],
    )?;
    ensure_rule(
        RULE_TCP_8090,
        &["protocol=TCP", "dir=in", "localport=8090", "action=allow"],
    )?;
    Ok(())
}

fn rule_exists(name: &str) -> bool {
    // netsh exits 0 and prints the rule if it exists; exits non-zero if not found
    Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={name}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn ensure_rule(name: &str, extra_args: &[&str]) -> Result<()> {
    if rule_exists(name) {
        info!("Firewall rule '{}' already exists — skipping", name);
        return Ok(());
    }
    let mut cmd = Command::new("netsh");
    cmd.args(["advfirewall", "firewall", "add", "rule", &format!("name={name}")]);
    for arg in extra_args {
        cmd.arg(arg);
    }
    let out = cmd.output()?;
    if out.status.success() {
        info!("Firewall rule '{}' created", name);
    } else {
        warn!(
            "Failed to create firewall rule '{}': {}",
            name,
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}
```

**Administrator privilege note:** netsh firewall commands require elevation. When rc-agent runs as a Windows Service with `LocalSystem` account, it has the required privilege automatically. During console development, run with admin rights.

**No new Cargo.toml entry needed.** `std::process::Command` is in the standard library.

---

### 3. Registry Management: `winreg = "0.55"` — one new crate

**Recommendation: `winreg = "0.55"`**

**Why winreg over existing `winapi` crate:**

The project already has `winapi = "0.3"` with several feature flags. winapi exposes the raw `RegOpenKeyExW`, `RegSetValueExW`, etc. FFI functions — usable but requires manual `HKEY` lifecycle management, `WCHAR` conversion, and `RegCloseKey` on drop. Writing this correctly without winreg is ~150 lines of unsafe FFI for what winreg provides in 10 lines of safe Rust.

**Why NOT adding winapi registry features:**

`winapi = "0.3"` with features `["winreg", "minwindef"]` would expose the raw registry API. This is viable but produces fragile code. The `winreg` crate wraps exactly these same APIs with proper RAII handles and serde serialization. Given the project already tolerates crate dependencies (hidapi, mdns-sd, sysinfo), adding winreg for a clean registry API is justified.

**Why NOT `windows-registry` crate (microsoft/windows-rs):**

`windows-registry` is part of the `windows` crate ecosystem (Microsoft official). It requires pulling in `windows-targets`, `windows-implement`, etc. — a much heavier dependency tree than `winreg = "0.55"`. For two registry operations (read + write HKLM Run key and config path), winreg is proportionate.

**winreg details:**
- Crate: `winreg` on crates.io (https://crates.io/crates/winreg)
- Current version: 0.55.0 (released 2025-01-12)
- Maintained by: gentoo90 (https://github.com/gentoo90/winreg-rs)
- Downloads: actively maintained, widely used (>7M downloads)
- No build.rs, no C dependencies — pure Rust + winapi bindings

**Cargo.toml addition (rc-agent, Windows-only):**

```toml
[target.'cfg(windows)'.dependencies]
winreg = "0.55"
```

**Usage pattern for self-healing config:**

```rust
// src/config_healer.rs — new module in rc-agent
#[cfg(windows)]
use winreg::{RegKey, enums::*};
use anyhow::Result;
use tracing::{info, warn};

const RUN_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
const RCAGENT_VALUE: &str = "RCAgent";
const RCAGENT_BAT: &str = r"C:\RacingPoint\start-rcagent.bat";

/// Verify the HKLM Run key exists and points to the correct batch file.
/// Re-create if missing or wrong.
pub fn ensure_startup_registry() -> Result<()> {
    #[cfg(windows)]
    {
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let run_key = hklm.open_subkey_with_flags(RUN_KEY, KEY_READ | KEY_WRITE)?;

        let current: Result<String, _> = run_key.get_value(RCAGENT_VALUE);
        match current {
            Ok(val) if val == RCAGENT_BAT => {
                info!("Startup registry key OK");
            }
            Ok(val) => {
                warn!("Startup registry key wrong: '{}' — repairing", val);
                run_key.set_value(RCAGENT_VALUE, &RCAGENT_BAT)?;
                info!("Startup registry key repaired");
            }
            Err(_) => {
                warn!("Startup registry key missing — creating");
                run_key.set_value(RCAGENT_VALUE, &RCAGENT_BAT)?;
                info!("Startup registry key created");
            }
        }
    }
    Ok(())
}
```

---

### 4. Fleet Health Dashboard: Extend Existing Kiosk WebSocket — no new library

**Recommendation: Add a new `/ws/fleet` endpoint in rc-core (Axum) and a new `/fleet` page in the existing kiosk (Next.js 16.1.6). Zero new libraries.**

**Why NOT a new standalone dashboard app:**

The kiosk already runs at port 3300. It already has `useKioskSocket` connecting to `/ws/dashboard` and rendering all 8 pods in real time. Uday already accesses it from his phone via `http://192.168.31.27:3300`. Adding a new `/fleet` route to the same Next.js app is a single new page file — no new deployment, no new port, no new process.

**Why NOT SSE (Server-Sent Events) for the fleet dashboard:**

SSE is ideal when the server pushes unidirectional event streams to a read-only dashboard. The fleet dashboard is NOT read-only — it needs two-way communication: Uday presses "restart Pod 3" and a command must flow back to rc-core. The existing `/ws/dashboard` already handles bidirectional pod commands (billing, game launch, lock, power). The fleet health view is an extension of the same WebSocket, not a new protocol.

**Why NOT polling from the kiosk:**

Pod state changes within seconds (crash → restart → reconnect). Polling at 2s intervals would produce acceptable UX but wastes HTTP requests when there are already persistent WebSocket connections. The WebSocket already pushes pod state on every change.

**Implementation approach:**

Option A (simpler): Add fleet health fields to the existing `/ws/dashboard` `pod_state` message. The control page already receives these. The `/fleet` kiosk page subscribes to `useKioskSocket` just like `/control` does — same hook, different view.

Option B (separate endpoint): Add `/ws/fleet` as a read-only WebSocket for the fleet health view. Justified if the control page's WebSocket message volume is too high for a simple health view, or if Uday accesses `/fleet` on a different device from where staff use `/control`.

**Recommendation: Option A.** Add fleet-relevant fields to existing pod state messages already flowing on `/ws/dashboard`. Create a new kiosk page at `/fleet` that uses `useKioskSocket`. No new endpoints, no new libraries, no new protocol.

**What the `/fleet` page shows per pod:**
- Service status (running as service / not a service / unknown)
- WebSocket connectivity (connected / disconnected, duration)
- Firewall rules status (present / missing, last verified)
- Config file health (present / missing / repaired)
- Last heartbeat timestamp
- Recent startup error (if any, last N lines)
- Deploy status (idle / deploying / verifying / rolled-back)

**This data already flows on the WebSocket** from pod_state messages. The missing pieces (service status, firewall status, config health) are NEW fields that rc-agent will report as part of v4.0 startup diagnostics — added to the `AgentMessage::Status` type in rc-common.

**No new npm packages required.** The kiosk already has:
- `next 16.1.6` — routing and server components
- `react 19.2.3` — UI rendering
- `tailwindcss 4.x` — styling
- `useKioskSocket` — WebSocket hook with pod state

**Mobile optimization note:** Uday accesses the kiosk from his phone. The existing `/control` page renders pod cards in a responsive grid. The `/fleet` page should use a compact list layout (pod number + status indicators) rather than cards, to fit 8 pods on a phone screen without scrolling.

---

## Recommended Stack Summary (New Additions Only)

### rc-agent Cargo.toml (Windows targets only)

| Crate | Version | Purpose | Why New |
|-------|---------|---------|---------|
| `windows-service` | 0.8 | ServiceMain protocol, SCM registration, install/uninstall | winapi doesn't have ServiceMain; NSSM abandoned |
| `winreg` | 0.55 | Registry read/write (HKLM Run key, config path check) | winapi raw FFI requires ~150 lines of unsafe for 10 lines of safe winreg |
| `tokio-util` | 0.7 | `CancellationToken` for coordinated async shutdown | Required for graceful service stop propagation to all tokio tasks |

**No new crate for firewall.** `std::process::Command` + `netsh` is sufficient and already available.

### kiosk (Next.js — zero new npm packages)

| Addition | Type | What |
|----------|------|------|
| `/fleet` page | New route file | Fleet health dashboard (pod service/firewall/config/deploy status) |
| Extended `Pod` type | Type update | Add `service_status`, `firewall_ok`, `config_ok`, `last_startup_error` fields |

**Zero new npm packages.** All fleet dashboard features use the existing WebSocket hook and Tailwind CSS.

---

## Installation

```toml
# crates/rc-agent/Cargo.toml — add to [target.'cfg(windows)'.dependencies]
windows-service = "0.8"
winreg = "0.55"
tokio-util = { version = "0.7", features = ["rt"] }
```

```bash
# No npm changes needed
# Kiosk: add src/app/fleet/page.tsx using existing useKioskSocket hook
```

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `windows-service 0.8` | NSSM (external binary) | Abandoned 2017; can't participate in Rust shutdown; can't report structured errors over WebSocket; must be bundled separately on pendrive |
| `windows-service 0.8` | `shawl` (Rust wrapper) | External wrapper — same Session 0 problem; can't signal CancellationToken |
| `windows-service 0.8` | `sc.exe` only | sc.exe installs the service but provides zero service control loop; binary would be killed by SCM after 30s without ServiceMain |
| `std::process::Command` netsh | `windows_firewall 0.3` crate | Low adoption; single-developer; COM API overkill for two rules; netsh is 20 lines, zero new dep |
| `std::process::Command` netsh | `wfp` crate (WFP API) | Kernel-level packet filtering — wrong abstraction for named application rules; crate abandoned 2021 |
| `winreg 0.55` | `winapi` raw registry | Correct but ~150 lines of unsafe FFI for HKEY lifecycle management vs 10 lines of safe Rust with winreg |
| `winreg 0.55` | `windows-registry` (microsoft/windows-rs) | Much heavier dependency tree; same functionality at 10x the transitive crate count |
| Extend existing `/ws/dashboard` | New `/ws/fleet` SSE endpoint | SSE is read-only; fleet commands (restart, redeploy) need bidirectional. WebSocket already established and working. |
| Extend existing `/ws/dashboard` | New standalone fleet app | New deployment, new port, new process — unnecessary when kiosk already runs and is accessible from Uday's phone |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| NSSM | Last release 2017; abandoned; cannot participate in Rust async shutdown or report errors over WebSocket; CRLF-safe deploy is irrelevant if the wrapper is the problem | `windows-service 0.8` crate |
| `windows_firewall` / `winfw-rs` crates | Both are low-adoption, low-maintenance; solving a 2-rule idempotent problem with a COM wrapper is disproportionate | `std::process::Command` netsh |
| `wfp` crate | Kernel-level API for application-layer firewall rules; abandoned 2021 | `std::process::Command` netsh |
| Session 0 GUI in the service | Hard OS constraint on Windows 10/11: services cannot display windows to user sessions. Attempting this causes blank screens or crashes | Move all GUI (lock screen) to the kiosk which runs in Session 1 |
| `windows-registry` crate (microsoft/windows-rs ecosystem) | Pulls in 10+ transitive crates for two registry operations | `winreg 0.55` |
| New standalone fleet dashboard (separate Next.js app, separate port) | New deployment surface; Uday already has kiosk URL; splitting adds ops complexity | Extend existing kiosk with `/fleet` page |
| Polling-based fleet status in Next.js | 2s polls waste connections when WebSocket already pushes on change; adds latency spikes | Extend existing `/ws/dashboard` WebSocket messages with fleet health fields |

---

## Critical Windows-Specific Gotchas

### Session 0 Isolation (CRITICAL)
**All Windows Services run in Session 0.** Session 0 is non-interactive — it has no display, no keyboard, no ability to show windows to the user in Session 1. The rc-agent lock screen currently uses WINAPI to create a window. If rc-agent is converted to a Windows Service without architectural changes, the lock screen will be invisible. This is not a configuration issue — it is an OS security boundary that cannot be bypassed on Windows 10/11.

**Mitigation:** Move the lock screen responsibility to the kiosk (`/lock` page), which already runs in Session 1 via the HKLM Run key. The kiosk navigates to `/lock` when it receives a `lock_screen` WebSocket event from rc-core.

### ServiceMain Thread vs Main Thread
The `service_dispatcher::start()` call BLOCKS the calling thread until the service stops. The tokio runtime must be created INSIDE `windows_service_main()`, not in `main()`. Creating the runtime in `main()` before calling `service_dispatcher::start()` will result in the runtime being torn down when `start()` returns on service stop, causing task cancellation before graceful shutdown completes.

### Startup Error Reporting Window
When a service fails to start within 30 seconds of the SCM registering it, Windows kills the process. If rc-agent startup (config load, WebSocket connect) takes >30s, the service is killed. Set service status to `StartPending` immediately at the top of `windows_service_main`, then update to `Running` after initialization completes. Report startup errors to rc-core over WebSocket before the status becomes `Running`, using the existing `AgentMessage` protocol.

### netsh Requires Elevation
`netsh advfirewall` commands require administrator rights. When rc-agent runs as a Windows Service under `LocalSystem`, it has these rights automatically. During development/console mode, the executable must be run as Administrator. Consider detecting at runtime and logging a clear warning rather than silently failing.

### winreg Key Access Flags
`RegKey::open_subkey()` opens with `KEY_READ` only. For writing (ensure_startup_registry), open with `KEY_READ | KEY_WRITE`. Opening HKLM with write access requires administrator rights — same elevation requirement as netsh. As a service, this is automatic.

### tokio-util version alignment
`tokio-util` must match the major tokio version. With `tokio = "1"` in the workspace, use `tokio-util = "0.7"` (the current stable companion to tokio 1.x). Do not use `tokio-util = "1"` — it does not exist as of March 2026.

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `windows-service 0.8` | Rust 1.93.1, tokio 1, Windows 11 | Maintained by Mullvad VPN; production tested on Windows Server and Windows 11 |
| `winreg 0.55` | Rust 1.93.1, winapi 0.3 | Released 2025-01-12; does not conflict with existing winapi 0.3 features |
| `tokio-util 0.7` | tokio 1 (workspace) | Stable companion to tokio 1.x; provides CancellationToken |
| netsh (std::process::Command) | Windows 10/11, Windows Server 2016+ | No Rust version dependency; pre-installed on all Windows 11 pods |
| Kiosk `/fleet` page | Next.js 16.1.6, React 19.2.3, Tailwind 4 | Uses only existing hooks and components; no new npm packages |

---

## Sources

- windows-service crate: https://crates.io/crates/windows-service (0.8.0, maintained by Mullvad VPN) — HIGH confidence (crates.io official)
- windows-service-rs GitHub: https://github.com/mullvad/windows-service-rs — HIGH confidence (official source)
- Writing a Windows Service in Rust (David Hamann, Feb 2026): https://davidhamann.de/2026/02/28/writing-a-windows-service-in-rust/ — HIGH confidence (recent, detailed)
- Tokio ServiceMain integration: https://users.rust-lang.org/t/tokio-app-as-windows-service/44207 — MEDIUM confidence (community, verified against official tokio docs)
- Session 0 isolation official docs: https://learn.microsoft.com/en-us/windows/win32/services/interactive-services — HIGH confidence (Microsoft Learn)
- Session 0 Windows 11 no-workaround: https://learn.microsoft.com/en-us/answers/questions/27517/is-there-any-workaround-in-win10-to-allow-service — HIGH confidence (Microsoft Q&A)
- winreg crate: https://crates.io/crates/winreg (0.55.0, released 2025-01-12) — HIGH confidence (crates.io official)
- winreg GitHub: https://github.com/gentoo90/winreg-rs — HIGH confidence
- windows_firewall crate: https://crates.io/crates/windows_firewall (0.3.0) — MEDIUM confidence (low adoption, single developer)
- wfp crate (abandoned): https://crates.io/crates/wfp — HIGH confidence it is abandoned (last commit 2021)
- netsh advfirewall docs: https://learn.microsoft.com/en-us/troubleshoot/windows-server/networking/netsh-advfirewall-firewall-control-firewall-behavior — HIGH confidence (Microsoft Learn)
- axum SSE official example: https://github.com/tokio-rs/axum/blob/main/examples/sse/src/main.rs — HIGH confidence (official axum repo)
- Existing codebase verified:
  - `crates/rc-agent/Cargo.toml` — confirmed winapi 0.3, tokio full, axum 0.8, no windows-service
  - `kiosk/src/hooks/useKioskSocket.ts` — confirmed WebSocket hook at `/ws/dashboard`, bidirectional, handles all pod events
  - `kiosk/package.json` — confirmed Next.js 16.1.6, zero charting libraries, Tailwind 4
  - `crates/rc-agent/src/main.rs` — confirmed tokio runtime in main, no ServiceMain
  - `Cargo.toml` workspace — confirmed constraint: existing deps preferred

---

*Stack research for: RaceControl v4.0 — Pod Fleet Self-Healing*
*Researched: 2026-03-15*
