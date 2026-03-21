# Stack Research: E2E Process Guard (v12.1)

**Domain:** Windows process monitoring, registry audit, and whitelist enforcement daemon
**Researched:** 2026-03-21 IST
**Confidence:** HIGH (versions verified on crates.io; integration points verified against actual Cargo.toml files in repo)

---

## Existing Crates Already Present (No New Deps for These)

These are already in rc-agent and/or racecontrol Cargo.toml. Use them as-is.

| Crate | Version in Repo | Role in v12.1 |
|-------|----------------|---------------|
| `sysinfo` | `0.33` (both rc-agent and racecontrol) | Process enumeration: names, PIDs, exe paths. Process kill via `process.kill(Signal::Kill)`. Core of process audit loop. |
| `winapi` | `0.3` (rc-agent, windows-only) | Fallback `TerminateProcess` if sysinfo kill returns false. Features already include `processthreadsapi`. |
| `tokio` | `1` (workspace) | `tokio::time::interval` for the monitoring tick loop; `tokio::task::spawn` for background daemon task. |
| `tracing` | `0.1` (workspace) | Structured audit log entries (violation, action, timestamp, machine ID). |
| `serde` / `serde_json` | `1` (workspace) | Whitelist deserialization from TOML; audit event serialization for WS dispatch. |
| `toml` | `0.8` (workspace) | Parse `[process_guard]` section from racecontrol.toml with per-machine overrides. |
| `anyhow` | `1` (workspace) | Error propagation in guard loops — do not panic on a failed kill. |
| `chrono` | `0.4` (workspace) | IST timestamps on every audit log entry. |
| `dirs-next` | `2` (rc-agent) | Resolve `%APPDATA%` path for Startup folder scan portably. Already a dep. |

**Upgrade note on sysinfo:** Latest is `0.38.3` (released 2026-03-02). The existing codebase pins `0.33`. The API changed between 0.33 and 0.38 (System initialization and process iteration differ). Do NOT upgrade mid-milestone. Stay on `0.33` for v12.1. Schedule an upgrade as a separate phase if needed.

---

## New Crates Required

Two new crates are needed. Both are Windows-only, added under `[target.'cfg(windows)'.dependencies]`.

### Core Additions

| Crate | Add To | Version | Purpose | Why This One |
|-------|--------|---------|---------|-------------|
| `winreg` | rc-agent, racecontrol | `"0.55"` | Read, enumerate, and delete HKCU/HKLM Run key entries. Verified latest: 0.55.0 released 2025-01-12. | The canonical Windows registry crate for Rust. Direct bindings to Win32 `RegOpenKeyEx`, `RegEnumValue`, `RegDeleteValue`. 83K+ downloads. Active maintenance by gentoo90 since 2015. No COM overhead. Simpler API than the `windows` crate registry surface. |
| `netstat2` | rc-agent, racecontrol | `"0.11"` | Enumerate listening TCP/UDP sockets with owning PID. Current: 0.11.2. | Uses `GetExtendedTcpTable` / `GetExtendedUdpTable` (iphlpapi) directly — same kernel API as `netstat.exe -ano` but callable from Rust without shell exec or text parsing. Returns PID per socket, enabling cross-reference against the process whitelist. Specialized crate; sysinfo 0.33 does not expose per-socket PID on Windows. |

### Supporting Addition

| Library | Add To | Version | Purpose | When to Use |
|---------|--------|---------|---------|-------------|
| `walkdir` | rc-agent, racecontrol | `"2"` | Enumerate `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup` and `C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup` for unauthorized `.lnk` and `.exe` files. | Only needed for the Startup folder scan. Handles both user and system startup paths uniformly. If zero new deps is required here, `std::fs::read_dir` on a known flat path also works since both startup folders are non-recursive. Prefer `walkdir` for uniformity. |

---

## Cargo.toml Changes

### crates/rc-agent/Cargo.toml

Append to the existing `[target.'cfg(windows)'.dependencies]` block (line 61 in current file):

```toml
[target.'cfg(windows)'.dependencies]
winapi = { version = "0.3", features = ["processthreadsapi", "winnt", "handleapi", "winuser", "memoryapi", "basetsd", "synchapi", "errhandlingapi", "winerror", "wingdi", "libloaderapi"] }
winreg = "0.55"
netstat2 = "0.11"
walkdir = "2"
```

### crates/racecontrol/Cargo.toml

Add a new `[target.'cfg(windows)'.dependencies]` block (does not currently exist in racecontrol — add it):

```toml
[target.'cfg(windows)'.dependencies]
winreg = "0.55"
netstat2 = "0.11"
walkdir = "2"
```

### Root Cargo.toml (workspace)

Do NOT add these to `[workspace.dependencies]`. They are Windows-only and only needed in two crates. Keeping them in per-crate `[target.'cfg(windows)'.dependencies]` prevents cross-platform build failures if Bono ever compiles on Linux for the cloud components.

---

## Alternatives Considered

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `winreg 0.55` for registry reads | `windows-registry` crate (Microsoft) | Newer and from Microsoft, but far less real-world usage than `winreg`. `winreg` has 10x more adoption, clearer ergonomics, and 10 years of maintenance history. |
| `winreg 0.55` for registry reads | `wmi` crate + `Win32_StartupCommand` query | WMI adds COM initialization overhead and pulls in `wmi = "0.18"` + `windows = "0.58"` as a transitive dep. Overkill for reading a handful of Run keys. `winreg` is direct and fast. |
| `netstat2 0.11` for port audit | `sysinfo` network info | `sysinfo 0.33` exposes aggregate network interface statistics, not per-socket state or owning PID. Cannot determine which process owns port 8090 with sysinfo alone. |
| `netstat2 0.11` for port audit | `listeners` crate | Also purpose-built for port-to-process mapping. Fewer downloads than netstat2, slightly less battle-tested. Either works; netstat2 chosen for ecosystem maturity. |
| `netstat2 0.11` for port audit | Shell exec `netstat -ano` via rc-common exec | Text parsing is brittle (column alignment changes, localized output on non-English Windows). `netstat2` calls iphlpapi directly. Use the right tool. |
| `walkdir 2` for Startup folder | `std::fs::read_dir` | `read_dir` works fine since both Startup folders are flat (non-recursive). Use `std::fs::read_dir` if avoiding any new dep is a priority. `walkdir` is recommended only for consistency. |
| `sysinfo::Process::kill()` for termination | `winapi::TerminateProcess` directly | `sysinfo` already wraps `TerminateProcess` internally. Since `sysinfo 0.33` is already in both crates and `kill()` is the established pattern in `kiosk.rs`, use it. Fall through to raw `winapi::TerminateProcess` only if `kill()` returns false (e.g., process already dead but handle still held — known Windows edge case). |
| Shell exec `schtasks /delete /tn <name> /f` via rc-common for Scheduled Task removal | COM `ITaskService` via `windows = "0.58"` | The Task Scheduler COM interface requires the large `windows` crate and unsafe COM calls. `schtasks /delete` is a built-in Windows command and already the established rc-agent pattern for privileged Windows operations. No new dep. |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `wmi` crate | COM initialization + `windows = "0.58"` transitive dep (large). `Win32_Process` via WMI is slower than `sysinfo`. `Win32_StartupCommand` via WMI is slower than `winreg`. Adds >2MB to binary size. | `sysinfo` for processes; `winreg` for registry |
| `reg-watcher` crate | Event-driven registry change notifications via `RegNotifyChangeKeyValue`. Adds complexity with no benefit for a 5-10s audit poll loop — a rogue entry will be caught on the next tick. | Poll with `winreg` on each monitoring interval |
| `notify` crate (filesystem watcher) | Watching registry hive files on the filesystem does not reliably catch in-memory registry changes. Not suitable for registry monitoring. | `winreg` polling |
| `windows` crate (Microsoft, `windows = "0.58"`) | Conflicts with the existing `winapi = "0.3"` dependency already in rc-agent. Two different Win32 binding layers in one binary cause symbol conflicts. The `windows` crate also has a large surface area — adds significant compile time. | `winapi 0.3` already present |
| `psutil` / `rust-psutil` | Unmaintained (last release 2021). Superseded by `sysinfo`. | `sysinfo 0.33` (already in repo) |
| Upgrading `sysinfo` to `0.38.x` in this milestone | The 0.33 → 0.38 API includes breaking changes to `System::new_all` vs `refresh_all` semantics and process iteration. All existing code in `kiosk.rs`, `game_process.rs`, `self_test.rs` was written against 0.33. Upgrading mid-milestone risks regressions across those modules. | Stay on `0.33`. Schedule upgrade as a dedicated phase. |
| New standalone binary `rc-process-guard` | Adds another binary to build, deploy, and manage on 11 machines. The guard is a module, not a service — it runs inside rc-agent and racecontrol. No separate process. | `process_guard.rs` module inside each crate |

---

## Integration Architecture

The process guard runs as a background tokio task inside each deployment target. No new binary, no new service.

### rc-agent (all 8 pods)

New module: `crates/rc-agent/src/process_guard.rs`

- Spawned as `tokio::task::spawn(process_guard::run(state.clone()))` from `main.rs`, alongside the existing `event_loop` spawn.
- Runs a `tokio::time::interval(Duration::from_secs(10))` loop.
- On each tick: enumerate processes (sysinfo), enumerate listening ports (netstat2), enumerate Run keys (winreg), enumerate Startup folder entries (walkdir/read_dir).
- Compare each set against the whitelist loaded from `rc-agent.toml`.
- Kill violating processes via `sysinfo::Process::kill(Signal::Kill)`.
- Delete violating Run key entries via `winreg::RegKey::delete_value`.
- Delete violating Startup folder files via `std::fs::remove_file`.
- Delete violating Scheduled Task entries via shell exec `schtasks /delete /tn <name> /f` through `rc-common::exec`.
- Report each violation over the existing WS connection (new `AgentMessage::ProcessViolation` variant in rc-common).
- Write all violations to structured tracing log for audit.

### racecontrol (server .23)

New module: `crates/racecontrol/src/process_guard.rs`

- Same tick loop pattern, spawned from server's `main.rs`.
- Whitelist differs: server allows racecontrol, kiosk (port 3300), web dashboard (port 3200), postgres/sqlite, but not Steam or any pod-only binaries.
- Violations reported to its own audit log + forwarded as WS alert to connected staff kiosk.

### James machine (.27)

James runs the racecontrol binary. The racecontrol process_guard module covers James. James's whitelist is broader: includes Ollama (port 11434), webterm (port 9999).

Per-machine overrides in `racecontrol.toml`:

```toml
[process_guard]
poll_interval_secs = 10
approved_processes = ["racecontrol", "kiosk", "node"]
approved_ports = [8080, 3300, 3200]
approved_autostart = ["racecontrol-server", "kiosk"]

[process_guard.machine_overrides.james]
approved_processes = ["racecontrol", "ollama", "node", "python", "webterm"]
approved_ports = [8080, 11434, 9999]
```

For pod-specific overrides, rc-agent reads a `[process_guard]` section from its local `rc-agent.toml`, which is pushed from racecontrol during config sync (existing v10.0 mechanism).

---

## Process Kill Sequence

The recommended kill sequence for a violating process:

1. `sysinfo::Process::kill(Signal::Kill)` — wraps `TerminateProcess` internally.
2. If kill returns `false`: open handle via `winapi::um::processthreadsapi::OpenProcess(PROCESS_TERMINATE, ...)` and call `TerminateProcess` directly.
3. Log result (killed / failed / already dead) with PID, name, and IST timestamp.
4. Emit `AgentMessage::ProcessViolation` over WS with action taken.

Do not use `Signal::Term` — Windows does not have SIGTERM. `Signal::Kill` is the only cross-platform signal in sysinfo.

---

## Registry Run Key Enumeration Pattern

```rust
use winreg::enums::*;
use winreg::RegKey;

fn enumerate_run_keys() -> Vec<(String, String)> {
    let hives = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
        (HKEY_CURRENT_USER,  r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run"),
    ];
    let mut entries = Vec::new();
    for (hive, path) in hives {
        let root = RegKey::predef(hive);
        if let Ok(key) = root.open_subkey(path) {
            for (name, value) in key.enum_values().flatten() {
                if let winreg::types::RegValue { vtype: REG_SZ, bytes } = value {
                    let exe = String::from_utf16_lossy(
                        &bytes.chunks(2)
                            .map(|b| u16::from_le_bytes([b[0], b.get(1).copied().unwrap_or(0)]))
                            .collect::<Vec<_>>()
                    );
                    entries.push((name, exe));
                }
            }
        }
    }
    entries
}
```

Deletion: `key.delete_value(&name)?` — requires opening the key with write access (`open_subkey_with_flags(path, KEY_WRITE)`).

---

## Port Audit Pattern

```rust
use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};

fn listening_ports_with_pids() -> Vec<(u16, u32)> {
    let af = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let proto = ProtocolFlags::TCP;
    get_sockets_info(af, proto)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|si| {
            if let ProtocolSocketInfo::Tcp(tcp) = si.protocol_socket_info {
                if tcp.state == netstat2::TcpState::Listen {
                    return Some((tcp.local_port, si.associated_pids.into_iter().next().unwrap_or(0)));
                }
            }
            None
        })
        .collect()
}
```

---

## Version Compatibility

| Package | Resolves With | Notes |
|---------|---------------|-------|
| `winreg 0.55` | `windows-sys 0.59` (transitive) | No conflict with `winapi 0.3` — they bind different Win32 surface areas via different mechanisms |
| `netstat2 0.11` | `winapi 0.3` (transitive) | netstat2 uses winapi internally for iphlpapi. Same version already in repo — no conflict. |
| `walkdir 2` | All workspace deps | Pure Rust, no platform-native bindings. No conflicts. |
| `sysinfo 0.33` | `tokio 1`, `winapi` | Already proven across all 8 pods in production. |

---

## Sources

- [sysinfo on crates.io](https://crates.io/crates/sysinfo) — 0.38.3 latest; 0.33 confirmed in repo
- [Process::kill in sysinfo docs](https://docs.rs/sysinfo/latest/sysinfo/struct.Process.html) — Signal::Kill is only cross-platform signal
- [winreg on crates.io](https://crates.io/crates/winreg) — 0.55.0 confirmed latest (released 2025-01-12)
- [winreg-rs on GitHub](https://github.com/gentoo90/winreg-rs) — Run key enumeration and delete_value patterns
- [netstat2 on crates.io](https://crates.io/crates/netstat2) — 0.11.2 current; iphlpapi GetExtendedTcpTable confirmed
- [netstat2-rs on GitHub](https://github.com/ohadravid/netstat2-rs) — Windows GetExtendedTcpTable usage pattern
- [listeners on GitHub](https://github.com/GyulyVGC/listeners) — considered and rejected in favor of netstat2
- [TerminateProcess Win32 docs](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess) — fallback kill path
- [MITRE ATT&CK T1547.001](https://attack.mitre.org/techniques/T1547.001/) — Startup folder + Run key paths confirmed
- `crates/rc-agent/Cargo.toml` — read directly; sysinfo 0.33, winapi 0.3 features confirmed
- `crates/racecontrol/Cargo.toml` — read directly; sysinfo 0.33 confirmed, no existing windows-only dep block
- `Cargo.toml` (workspace) — read directly; confirmed tokio, tracing, serde, toml, anyhow versions

---

*Stack research for: v12.1 E2E Process Guard — Windows process monitoring and whitelist enforcement*
*Researched: 2026-03-21 IST*
