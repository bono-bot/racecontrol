# Phase 16: Firewall Auto-Config - Research

**Researched:** 2026-03-15
**Domain:** Windows Firewall (netsh advfirewall) automation from Rust via std::process::Command
**Confidence:** HIGH

## Summary

Phase 16 moves firewall rule management from the CRLF-sensitive `fix-firewall.bat` into Rust code that
runs inside `rc-agent` on every startup. The root cause of Pods 1/3/4 being unreachable was that batch
files written on Windows may carry CRLF line endings that silently corrupt netsh commands when deployed
from a non-Windows staging environment. Moving the same `netsh advfirewall` calls into
`std::process::Command` in Rust eliminates this permanently.

The implementation is a single new file `crates/rc-agent/src/firewall.rs` with one public function
`configure()` that is called in `main.rs` before `remote_ops::start(8090)`. The function: (1) deletes
any existing rules with the same names to prevent accumulation (idempotency), then (2) adds fresh rules
with `profile=any`. On failure it logs a warning and continues — a blocked port is better than a crashed
agent.

rc-agent runs as a regular GUI process in Session 1, not as a SYSTEM service. On Windows 11, modifying
firewall rules with `netsh advfirewall firewall add` requires administrator privileges. The pods already
run rc-agent via an elevated session (install.bat ran as admin, the Run key inherits that context on
gaming PCs). If elevation is absent, netsh exits with error code 1 and a logged warning — the agent must
not panic.

**Primary recommendation:** Add `firewall.rs` with a synchronous `configure()` function using
`std::process::Command` + `CREATE_NO_WINDOW`. Call it from `main()` before `remote_ops::start(8090)`.
Return a `FirewallResult` enum (Configured / AlreadyCorrect / Failed(String)) for testable logic.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FW-01 | rc-agent configures firewall rules (ICMP + TCP 8090) in Rust on every startup | `firewall::configure()` called in main() before remote_ops::start — uses netsh via std::process::Command |
| FW-02 | Firewall rules use profile=any and are idempotent (no duplicate accumulation) | Delete-then-add pattern; `delete rule name=X` is a no-op when rule absent (exit 0) |
| FW-03 | Firewall configuration runs before HTTP server bind (confirms port 8090 reachable) | Call order in main.rs: configure() → remote_ops::start(8090) → tracing::info!("Firewall configured") |
</phase_requirements>

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| std::process::Command | stdlib | Synchronous subprocess execution | No extra dep; same pattern as ai_debugger.rs auto-fix functions |
| tracing | workspace | Structured logging | Already used project-wide |
| winapi (existing) | 0.3 | CREATE_NO_WINDOW flag on Windows | Already in Cargo.toml under `[target.'cfg(windows)'.dependencies]` |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::io | stdlib | Capture stdout/stderr from netsh | Always — netsh error output is in stderr |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| std::process::Command (sync) | tokio::process::Command (async) | Sync is correct here — this runs at startup before the tokio runtime's full event loop, and a 100ms netsh call blocking main() for a few hundred ms is acceptable. Also matches ai_debugger.rs pattern for process invocation. |
| netsh advfirewall | Windows Firewall COM API (INetFwRules) | COM requires unsafe + extra winapi features; netsh is simpler, already used in fix-firewall.bat, exit codes are reliable |
| netsh advfirewall | PowerShell New-NetFirewallRule | PowerShell startup is 200–500ms; netsh is 30–80ms. No new deps. |

**No new dependencies needed.** The existing Cargo.toml already has everything required.

## Architecture Patterns

### New File

```
crates/rc-agent/src/
├── firewall.rs          # NEW — contains configure(), FirewallResult, unit tests
├── main.rs              # MODIFIED — adds `mod firewall;` + call before remote_ops::start
└── ... (existing)
```

### Pattern 1: Delete-Then-Add (Idempotent Rule Management)

**What:** Delete by name first (silently), then add fresh. This is the exact same logic in `fix-firewall.bat`
but in Rust.

**When to use:** Any time a named rule must exist exactly once.

**Why delete first, not check-then-add:** `netsh advfirewall firewall show rule` returns exit 1 when a
rule does not exist, and exit 0 when it does — but the output format has changed across Windows builds.
Delete-first is simpler and equally safe: deleting a non-existent rule returns exit 0 on Windows 11.

**Verified behavior** (from fix-firewall.bat production use):
- `netsh advfirewall firewall delete rule name="X"` — exit 0 whether or not rule exists
- `netsh advfirewall firewall add rule name="X" ...` — exit 0 on success, exit 1 if not admin

```rust
// Source: fix-firewall.bat production patterns + std::process::Command docs
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub enum FirewallResult {
    Configured,
    Failed(String),
}

pub fn configure() -> FirewallResult {
    // Step 1: Delete stale rules (idempotent — exit 0 even if absent)
    run_netsh(&["advfirewall", "firewall", "delete", "rule",
               "name=RacingPoint-ICMP"]);
    run_netsh(&["advfirewall", "firewall", "delete", "rule",
               "name=RacingPoint-RemoteOps"]);

    // Step 2: Add ICMP echo-request (ping), all profiles
    let icmp_ok = run_netsh(&[
        "advfirewall", "firewall", "add", "rule",
        "name=RacingPoint-ICMP",
        "protocol=icmpv4:8,any",
        "dir=in",
        "action=allow",
        "profile=any",
        "enable=yes",
    ]);

    // Step 3: Add TCP 8090 (remote ops), all profiles
    let tcp_ok = run_netsh(&[
        "advfirewall", "firewall", "add", "rule",
        "name=RacingPoint-RemoteOps",
        "protocol=TCP",
        "localport=8090",
        "dir=in",
        "action=allow",
        "profile=any",
        "enable=yes",
    ]);

    if icmp_ok && tcp_ok {
        FirewallResult::Configured
    } else {
        FirewallResult::Failed("One or more netsh rules failed — likely not running as admin".into())
    }
}

fn run_netsh(args: &[&str]) -> bool {
    let mut cmd = Command::new("netsh");
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    match cmd.output() {
        Ok(out) => out.status.success(),
        Err(e) => {
            tracing::warn!("[firewall] netsh exec failed: {}", e);
            false
        }
    }
}
```

### Pattern 2: Call Order in main.rs

**What:** Insert firewall configure call at a specific point in the startup sequence.

**Current startup order (relevant section):**
```
line ~309: remote_ops::start(8090)    ← HTTP server binds port 8090
```

**Required order after Phase 16:**
```
firewall::configure() result → tracing::info!("Firewall configured")
remote_ops::start(8090)      ← port now guaranteed open in firewall
```

**FW-03 success criterion:** Log line "Firewall configured" MUST appear before the remote_ops bind log
line `"Remote ops server listening on http://..."`. This ordering is guaranteed by calling
`firewall::configure()` synchronously in main() before the `remote_ops::start(8090)` call.

### Pattern 3: Rule Names

Use `RacingPoint-ICMP` and `RacingPoint-RemoteOps` (not `AllowICMP` / `RCAgent` from the old batch
file). Reasons:
1. Namespaced names are less likely to collide with other software
2. Old rule names from the batch file (AllowICMP, RCAgent, PodAgent) will coexist harmlessly — Rust
   code only manages its own named rules

**The old batch file rules do NOT need to be cleaned up** — they provide a safety net on pods that
haven't been fully migrated. Once Phase 16 is deployed, the Rust-managed rules take over.

### Anti-Patterns to Avoid

- **Async netsh calls:** Don't use `tokio::process::Command` here. The function is called before the
  async subsystems are started and must complete before continuing. A 100–200ms synchronous call at
  startup is fine.
- **Checking rule existence before deleting:** `show rule` output is locale-sensitive and format varies.
  Delete-first is simpler and equally correct.
- **Panicking on failure:** If the pod isn't running as admin, firewall config fails — but the agent
  should still run. Log a warning, return `Failed`, continue.
- **Using profile=domain,private,public separately:** The `profile=any` shorthand is equivalent and
  cleaner. Confirmed in netsh help output.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Idempotency check | Parse `netsh show` output to detect existing rules | Delete-then-add pattern | Output format varies by locale and Windows version; delete is always safe |
| Admin detection | Check token elevation before running netsh | Just run it, handle exit code | Simpler; the code path is the same either way |
| Rule verification | Query rules after adding to confirm | Trust exit code 0 | netsh exit 0 = success on Windows; verification adds 2 more subprocess calls for no benefit |

## Common Pitfalls

### Pitfall 1: CREATE_NO_WINDOW is Windows-only

**What goes wrong:** Compile error on non-Windows if `creation_flags()` is called unconditionally.

**Why it happens:** `CommandExt::creation_flags` is a Windows-only extension trait.

**How to avoid:** Wrap in `#[cfg(windows)]` — both the `use` import and the `.creation_flags()` call.
The existing `remote_ops.rs` shows the exact pattern to copy:
```rust
#[cfg(windows)]
cmd.creation_flags(CREATE_NO_WINDOW);
```

**Warning signs:** Build fails on non-Windows CI or if the codebase is ever tested on Linux.

### Pitfall 2: netsh delete of absent rule returns exit 0 on Windows 11

**What goes wrong:** Developer may assume delete returns non-zero when rule doesn't exist, and write
check-before-delete logic.

**Why it happens:** On Windows 11, `netsh advfirewall firewall delete rule name=X` returns exit 0 and
prints "There are no rules matching the specified criteria." to stdout. This is the correct and expected
behavior.

**How to avoid:** Don't check return code of the delete step — always proceed to add regardless.

### Pitfall 3: rc-agent may not have admin privileges

**What goes wrong:** netsh exits with error code 1, rule not created, port 8090 stays blocked.

**Why it happens:** rc-agent runs via the HKLM Run key (`start-rcagent.bat`), which starts in the user's
session at login. If the user account on the pod is a standard (non-admin) account, the process lacks
the elevation needed by netsh to modify firewall rules.

**Reality check on pods:** All 8 pods use an admin-level local account (per deployment history — the
install.bat ran as admin and set the Run key). So in practice this works. But the code must not crash.

**How to avoid:** Return `FirewallResult::Failed(...)` and log a warning, do NOT use `unwrap()` or
`expect()` on the netsh result. The agent starts normally regardless.

### Pitfall 4: Rule name collisions from old batch file rules

**What goes wrong:** Old rules named `AllowICMP` and `RCAgent` still exist alongside new
`RacingPoint-ICMP` / `RacingPoint-RemoteOps`. This is fine — extra allow rules don't hurt — but
someone may try to delete the old ones in the same function, causing complexity.

**How to avoid:** Leave old rules alone. They're additive. Let them be cleaned up manually or via the
next physical install. Rust code only manages rules it owns (by name prefix `RacingPoint-`).

### Pitfall 5: Testing firewall code without admin privileges

**What goes wrong:** Unit tests that call `firewall::configure()` directly will fail on developer
machines or CI that don't have admin rights.

**How to avoid:** Structure the module so the logic (building args, parsing results) is testable
separately from the actual `std::process::Command` execution. Tests verify:
- Rule names are correct strings
- Profile=any is present in the args
- Both rules are attempted (ICMP + TCP)
- Failure path returns `FirewallResult::Failed(...)` not a panic

The actual netsh subprocess call is tested via Pod 8 canary deploy, not unit tests.

## Code Examples

### Complete firewall.rs module

```rust
// Source: fix-firewall.bat production patterns, ai_debugger.rs Command pattern
//! Firewall auto-configuration — ensures ICMP echo + TCP 8090 are open on every startup.
//! Runs synchronously before the HTTP server binds.

use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const RULE_ICMP: &str = "RacingPoint-ICMP";
const RULE_TCP: &str = "RacingPoint-RemoteOps";

#[derive(Debug, PartialEq)]
pub enum FirewallResult {
    Configured,
    Failed(String),
}

/// Configure firewall rules for ICMP echo and TCP 8090.
/// Idempotent: deletes existing rules by name before adding fresh ones.
/// Non-fatal: logs warning and returns Failed on permission error.
pub fn configure() -> FirewallResult {
    tracing::info!("[firewall] Applying firewall rules (profile=any)...");

    // Delete stale rules — exit 0 even if absent on Windows 11
    run_netsh(&["advfirewall", "firewall", "delete", "rule",
               &format!("name={}", RULE_ICMP)]);
    run_netsh(&["advfirewall", "firewall", "delete", "rule",
               &format!("name={}", RULE_TCP)]);

    // Add ICMP echo-request (ping), all profiles
    let icmp_ok = run_netsh(&[
        "advfirewall", "firewall", "add", "rule",
        &format!("name={}", RULE_ICMP),
        "protocol=icmpv4:8,any",
        "dir=in",
        "action=allow",
        "profile=any",
        "enable=yes",
    ]);

    // Add TCP 8090 (remote ops), all profiles
    let tcp_ok = run_netsh(&[
        "advfirewall", "firewall", "add", "rule",
        &format!("name={}", RULE_TCP),
        "protocol=TCP",
        "localport=8090",
        "dir=in",
        "action=allow",
        "profile=any",
        "enable=yes",
    ]);

    match (icmp_ok, tcp_ok) {
        (true, true) => {
            tracing::info!("[firewall] Firewall configured — ICMP + TCP 8090 open (profile=any)");
            FirewallResult::Configured
        }
        _ => {
            let msg = "netsh failed — agent may lack admin privileges. Port 8090 may be blocked.";
            tracing::warn!("[firewall] {}", msg);
            FirewallResult::Failed(msg.to_string())
        }
    }
}

/// Returns true if netsh exited with code 0.
fn run_netsh(args: &[&str]) -> bool {
    let mut cmd = Command::new("netsh");
    cmd.args(args);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    match cmd.output() {
        Ok(out) => {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!("[firewall] netsh {:?} failed: {}", args, stderr.trim());
            }
            out.status.success()
        }
        Err(e) => {
            tracing::warn!("[firewall] failed to spawn netsh: {}", e);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_names_are_namespaced() {
        assert!(RULE_ICMP.starts_with("RacingPoint-"));
        assert!(RULE_TCP.starts_with("RacingPoint-"));
    }

    #[test]
    fn test_rule_names_are_distinct() {
        assert_ne!(RULE_ICMP, RULE_TCP);
    }

    #[test]
    fn test_firewall_result_failed_is_not_configured() {
        let r = FirewallResult::Failed("test".into());
        assert_ne!(r, FirewallResult::Configured);
    }
}
```

### main.rs call site (before remote_ops::start)

```rust
// In main() — BEFORE remote_ops::start(8090)
match firewall::configure() {
    firewall::FirewallResult::Configured => {
        tracing::info!("Firewall configured");
    }
    firewall::FirewallResult::Failed(msg) => {
        tracing::warn!("Firewall config failed: {} — continuing anyway", msg);
    }
}

// Remote ops HTTP server (merged pod-agent) — port 8090
remote_ops::start(8090);
tracing::info!("Remote ops server started on port 8090");
```

### Verification commands (from server, via pod-agent /exec)

```
# Verify rules were created with profile=any
netsh advfirewall firewall show rule name="RacingPoint-ICMP"
netsh advfirewall firewall show rule name="RacingPoint-RemoteOps"

# Idempotency check: count rules before and after restart
netsh advfirewall firewall show rule name=all | findstr /C:"RacingPoint-"
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| fix-firewall.bat (run manually or at install) | firewall::configure() in Rust at every startup | Phase 16 | CRLF bug impossible; rules re-applied after every reboot automatically |
| netsh in batch: `profile` parameter omitted | `profile=any` explicit | Phase 16 | Rules apply on domain, private, AND public profiles |
| Rule names: AllowICMP, RCAgent, PodAgent | RacingPoint-ICMP, RacingPoint-RemoteOps | Phase 16 | Namespaced — no collisions, easy to identify |

**Deprecated/outdated:**
- `fix-firewall.bat`: Replaced by Rust code. File can be kept on pendrive for emergency manual use but
  is no longer the authoritative source of firewall config.
- `install.bat` Step 10 `netsh` line: The single `netsh` call in install.bat (`add rule name="RCAgent"
  ...`) sets no profile and does not add ICMP. After Phase 16 deploys, this line becomes redundant
  (rc-agent will apply its own rules on first run). It should be removed from install.bat in a follow-up
  to avoid confusion, but is not blocking.

## Open Questions

1. **Profile behavior on fresh Windows installs**
   - What we know: `profile=any` covers domain, private, and public per netsh documentation
   - What's unclear: Whether Windows Defender's "first run" wizard resets rules on profile change
   - Recommendation: The delete-then-add pattern on every startup handles this regardless — rules are
     always fresh

2. **Admin level on pods post-Phase 19 (Watchdog Service)**
   - What we know: Currently rc-agent runs in Session 1 via the logged-in admin account
   - What's unclear: Phase 19 will introduce rc-watchdog.exe as a SYSTEM service that starts rc-agent.
     If rc-agent is started by SYSTEM it may run with or without admin token depending on `CreateProcessAsUser` parameters
   - Recommendation: This is a Phase 19 concern. For Phase 16, the existing startup path is sufficient.
     Add a comment in firewall.rs noting this dependency.

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (cargo test) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p rc-agent firewall` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FW-01 | configure() calls netsh for ICMP + TCP 8090 | unit | `cargo test -p rc-agent firewall` | Wave 0 (new file) |
| FW-02 | Rule names are distinct, namespaced; delete before add pattern | unit | `cargo test -p rc-agent firewall` | Wave 0 (new file) |
| FW-02 | Idempotency — no duplicate accumulation | manual-only | `netsh advfirewall show rule name=all \| findstr RacingPoint-` run 10x | N/A |
| FW-03 | configure() result logged before remote_ops bind | manual-only | Check rc-agent.log line ordering after reboot | N/A |

Note on manual tests: FW-02 idempotency and FW-03 log ordering require a real pod with admin rights.
They cannot be exercised in unit tests on a developer machine without netsh. These are verified during
the Pod 8 canary deploy step.

### Sampling Rate

- **Per task commit:** `cargo test -p rc-agent firewall`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green + Pod 8 canary verification before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/rc-agent/src/firewall.rs` — new module; covers FW-01 and FW-02 unit assertions
  - Tests needed: rule names namespaced, names distinct, FirewallResult::Failed != Configured
  - Actual netsh execution is NOT tested in unit tests (requires admin + Windows)

*(No other gaps — existing test infrastructure (cargo test, serial_test, axum TestClient) covers all
other aspects of rc-agent)*

## Sources

### Primary (HIGH confidence)

- `D:\pod-deploy\fix-firewall.bat` — production netsh commands that have been verified working on pods
- `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\ai_debugger.rs` lines 200-237 — std::process::Command pattern for synchronous subprocess calls in this codebase
- `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\remote_ops.rs` lines 36-41 — CREATE_NO_WINDOW pattern in this codebase
- `C:\Users\bono\racingpoint\racecontrol\crates\rc-agent\src\main.rs` lines 309-311 — exact call site for remote_ops::start(8090)

### Secondary (MEDIUM confidence)

- `.planning/STATE.md` — decision: "Firewall: Move entirely to Rust (std::process::Command calling netsh) — eliminate CRLF-sensitive batch files permanently"
- `.planning/REQUIREMENTS.md` — FW-01, FW-02, FW-03 definitions

### Tertiary (LOW confidence)

- None — all findings grounded in codebase inspection and project decisions

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — uses only existing deps (std, tracing, winapi already in Cargo.toml)
- Architecture: HIGH — pattern copied directly from ai_debugger.rs + fix-firewall.bat
- Pitfalls: HIGH — most pitfalls already encountered in production (CRLF, admin, duplicate rules)

**Research date:** 2026-03-15
**Valid until:** 2026-06-15 (stable — netsh advfirewall API hasn't changed in 10+ years)
