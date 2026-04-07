# Phase 331: Process Architecture Cleanup - Research

**Researched:** 2026-04-07
**Domain:** Windows process management, service architecture, Rust Command spawning
**Confidence:** HIGH

## Summary

The current rc-agent restart architecture has THREE competing restart mechanisms that evolved organically across 43+ milestones. The good news: rc-sentry has ALREADY been neutered (BUG-001/BUG-002 fix in tier1_fixes.rs) -- it logs "DELEGATED to rc-watchdog" and does nothing. The schtask `StartRCAgent` is only used as a last-resort fallback in documentation/playbooks. The primary restart authority is ALREADY the RCWatchdog Windows service via `WTSQueryUserToken` + `CreateProcessAsUser` (session.rs).

The real cleanup work is: (1) remove dead code paths in rc-sentry that still reference restart, (2) formalize the watchdog as THE single authority by removing schtask references, (3) consolidate 4 duplicate `hidden_cmd()` helpers into a single `spawn_safe()` in rc-common, and (4) decide whether to keep or remove the rollback_manager binary rename logic.

**Primary recommendation:** This is a safe cleanup phase. The hardest part (making watchdog the primary restart authority) was already done. What remains is removing dead code, consolidating helpers, and removing binary renames from rollback_manager.

## Current Restart Flow (As-Is)

### 1. RCWatchdog Service (PRIMARY -- already working correctly)
- **File:** `crates/rc-watchdog/src/service.rs` (685 lines)
- **Mechanism:** Windows SCM service, polls every 5s, detects rc-agent death via tasklist + health endpoint
- **Restart:** `session::spawn_in_session1()` -- uses `WTSQueryUserToken` + `CreateProcessAsUser` to spawn `rc-agent.exe` directly in Session 1
- **CRITICAL FIX ALREADY DONE:** As of recent commit, spawns `rc-agent.exe` directly, NOT `start-rcagent.bat` (regression test in session.rs enforces this)
- **Deconfliction:** Checks sentry breadcrumb file (30s grace), max 6 deferrals before taking over
- **Binary validation:** SHA256 manifest check every 10 polls, triggers rollback on mismatch
- **Restart loop detection:** 3+ restarts in 10 min triggers MMA diagnosis

### 2. rc-sentry restart_service() (ALREADY NEUTERED)
- **File:** `crates/rc-sentry/src/tier1_fixes.rs` lines 442-473
- **Status:** DELEGATED -- logs a message and does NOTHING else
- **Dead code:** Still has breadcrumb cleanup (`del sentry-restart-breadcrumb.txt`)
- **References in:** cognitive_gate.rs playbooks mention `StartRCAgent` schtask as fallback
- **peer_channel.rs:** Test fixtures still reference `schtasks /Run /TN StartRCAgent`

### 3. self_monitor.rs relaunch_self() (SECONDARY -- sentry-aware)
- **File:** `crates/rc-agent/src/self_monitor.rs` (573 lines)
- **Triggers:** WS disconnected 5+ min, CLOSE_WAIT flood on :8090
- **Mechanism:** If sentry alive -> write GRACEFUL_RELAUNCH sentinel + `process::exit(0)` (watchdog handles restart). If sentry dead -> PowerShell fallback (90MB leak per restart)
- **Cap:** MAX_RESTARTS = 5, then refuses to restart
- **This is actually fine as-is** -- it's already deferring to watchdog via exit(0)

### 4. StartRCAgent schtask (BOOT ONLY)
- **Registered on:** All 8 pods
- **Purpose:** Runs `start-rcagent.bat` at boot (HKLM Run key)
- **Problem:** Running it at runtime puts rc-agent in Session 0 (SYSTEM context)
- **Used by:** Documentation, playbooks, peer_channel.rs test fixtures, cognitive_gate.rs fallback strings
- **Should NOT be removed** -- it's the boot mechanism. But references to running it at runtime should be cleaned.

### 5. start-rcagent.bat (BOOT SCRIPT)
- **File:** `scripts/deploy/start-rcagent.bat` (86 lines)
- **Purpose:** Bloatware cleanup, sentinel clearing, Edge session wipe, binary swap, power settings, then `start "" /D C:\RacingPoint rc-agent.exe`
- **MUST stay** -- it's the boot-time cleanup + binary swap mechanism
- **MUST NOT be used by watchdog** -- the bat's `taskkill /F /IM rc-agent.exe` kills a running agent

## rollback_manager.rs Analysis

### What It Does
- **File:** `crates/rc-watchdog/src/rollback_manager.rs` (485 lines)
- **Binary rename:** `rc-agent.exe` -> `rc-agent-failed.exe`, `rc-agent-prev.exe` -> `rc-agent.exe`
- **Depth tracking:** Max 3 rollbacks before entering MAINTENANCE_MODE
- **WhatsApp alerts:** Sends to Bono via comms-link on rollback
- **State:** Persisted in `C:\RacingPoint\rollback-state.json`
- **MAINTENANCE_MODE:** JSON sentinel with epoch timestamp, auto-cleared after 30 min

### Where It's Called
1. `service.rs` line 314: Binary validation failure triggers rollback
2. `service.rs` line 529: Health poll failure after restart triggers rollback (if 2+ failures)
3. `service.rs` line 353: `confirm_healthy()` resets rollback state when agent is alive
4. `service.rs` lines 262-269: Maintenance mode check + auto-clear every poll cycle
5. `service.rs` line 418: `RollbackState::load()` for restart loop context

### Should Binary Rename Stay or Go?

**Arguments for REMOVING binary rename:**
- VMS (reference architecture) never renames binaries
- `start-rcagent.bat` already handles binary swap (hash-based: `rc-agent-????????*.exe` -> `rc-agent.exe`)
- Renaming to `rc-agent-failed.exe` pollutes the filesystem
- Creates confusing tasklist output (BUG-66: `output_contains_agent()` must exclude `-failed` and `-prev`)

**Arguments for KEEPING binary rename:**
- Prevents a known-bad binary from being restarted in a crash loop
- Preserves forensic evidence (the failed binary is still on disk)
- The bat only runs at boot -- between boots, the watchdog is the only restart mechanism
- Removing it means a bad binary gets restarted indefinitely until reboot

**Recommendation:** KEEP the rollback mechanism but simplify it. Remove the `rc-agent-failed.exe` rename (just use `rc-agent-prev.exe` which already exists from the deploy flow). The rollback depth + MAINTENANCE_MODE is genuinely useful crash loop protection that VMS doesn't need (VMS has dedicated ops teams, we have autonomous AI).

## spawn_safe() Helper Analysis

### Current State: 4 Duplicate `hidden_cmd()` Helpers
| File | Has Stdio::null? | Has creation_flags? | Visibility |
|------|-------------------|---------------------|------------|
| `ac_launcher.rs` | NO | YES | `pub(crate)` |
| `lock_screen.rs` | stdin only | YES | `fn` (private) |
| `ai_debugger.rs` | NO | YES | `fn` (private) |
| `game_process.rs` | NO | YES | `fn` (private) |

### Scale of the Problem
- **76 `Command::new` call sites** in rc-agent/src/
- **15 `Stdio::null` usages** scattered across files (only where bugs were hit)
- **38 `creation_flags(0x08000000)` usages** (CREATE_NO_WINDOW)
- **Many call sites have NEITHER** Stdio::null nor creation_flags

### Why This Matters
`FreeConsole()` at startup (main.rs line 800) detaches rc-agent from its parent console. After that, inherited stdio handles are INVALID. Any `Command::new()` that doesn't set `Stdio::null()` risks `ERROR_INVALID_HANDLE (os error 6)`. This has caused bugs repeatedly -- each time fixed by adding Stdio::null to the specific call site.

### Recommended Approach: `spawn_safe()` in rc-common
```rust
// crates/rc-common/src/spawn_safe.rs
use std::process::{Command, Stdio};

/// Create a Command pre-configured for the FreeConsole() environment:
/// - Stdio::null() on stdin/stdout/stderr (prevents os error 6)
/// - CREATE_NO_WINDOW on Windows (prevents console flash)
///
/// Use for ALL background utilities (taskkill, reg, powershell, netstat, etc.)
/// Do NOT use for processes that need visible windows (Edge, games).
pub fn spawn_safe(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null())
       .stdout(Stdio::null())
       .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Like spawn_safe() but captures stdout/stderr for reading output.
/// Still sets stdin to null and CREATE_NO_WINDOW.
pub fn spawn_safe_capture(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    // stdout and stderr left as default (piped by .output())
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    cmd
}
```

**Two variants needed:**
1. `spawn_safe()` -- all stdio null, for fire-and-forget (taskkill, reg, etc.)
2. `spawn_safe_capture()` -- stdin null, stdout/stderr default, for `.output()` calls

**Migration:** Replace all 4 `hidden_cmd()` copies + all bare `Command::new()` that set creation_flags manually. The 76 call sites break down into:
- ~40 that need `spawn_safe()` (fire-and-forget like taskkill)
- ~30 that need `spawn_safe_capture()` (need to read output like tasklist, netstat, nvidia-smi)
- ~6 that need special handling (Edge browser launch, game launch -- visible windows)

## Risk Analysis

### If We Remove schtask Restart Path
**Risk:** If RCWatchdog service dies, there is NO automatic restart mechanism until reboot.
**Mitigation:** 
1. RCWatchdog is a Windows service with SCM `SERVICE_FAILURE_ACTIONS` -- SCM auto-restarts it
2. rc-sentry can detect watchdog death and alert staff
3. The schtask STILL EXISTS for boot -- we're only removing runtime invocation references
4. self_monitor's PowerShell fallback still works as emergency last resort

**Verdict:** LOW risk. The schtask is already broken at runtime (Session 0 issue). Removing references to running it at runtime is actually fixing documentation, not removing capability.

### If We Remove Binary Rename from rollback_manager
**Risk:** A bad binary loops forever between boots (watchdog keeps restarting it).
**Mitigation:** Restart loop detection (3 in 10 min) triggers MAINTENANCE_MODE which stops all restarts. MMA diagnosis runs. WhatsApp alert sent.
**Verdict:** MEDIUM risk but the MAINTENANCE_MODE + MMA path is sufficient. The rename adds complexity (BUG-66 exclusion logic in output_contains_agent) for marginal benefit.

**Recommendation:** Keep the MAINTENANCE_MODE + restart loop detection. Remove only the `rc-agent.exe -> rc-agent-failed.exe` rename step. Keep `rc-agent-prev.exe` (deploy flow uses it). This simplifies rollback_manager while keeping crash loop protection.

### If spawn_safe() Migration Misses a Call Site
**Risk:** os error 6 on the missed call site.
**Mitigation:** 
1. `grep -rn "Command::new" crates/rc-agent/src/ | grep -v spawn_safe` finds stragglers
2. The error is non-fatal (the specific command fails, agent stays alive)
3. Tests exist for many of these code paths

**Verdict:** LOW risk. The migration is mechanical and verifiable via grep.

## Architecture Patterns

### Recommended Final State
```
Restart Authority Hierarchy:
1. RCWatchdog Windows Service (PRIMARY)
   - Polls every 5s
   - WTSQueryUserToken + CreateProcessAsUser -> Session 1
   - Binary validation + rollback
   - Restart loop detection + MMA
   
2. self_monitor.rs (SECONDARY, inside rc-agent)
   - WS dead 5min / CLOSE_WAIT flood
   - exit(0) -> watchdog detects -> restart
   - PowerShell fallback ONLY if sentry unreachable
   
3. start-rcagent.bat (BOOT ONLY)
   - HKLM Run key, runs at login
   - Bloatware cleanup + binary swap + start

4. REMOVED: rc-sentry restart path (already neutered, clean up dead code)
5. REMOVED: Runtime schtask invocation (keep schtask for boot only)
6. REMOVED: Binary rename to rc-agent-failed.exe
```

### Anti-Patterns to Avoid
- **Running schtasks at runtime:** Always produces Session 0 rc-agent, breaks all GUI
- **Using start-rcagent.bat for restart:** The bat's taskkill kills a running agent
- **Bare Command::new() without Stdio::null():** Will fail after FreeConsole()

## Common Pitfalls

### Pitfall 1: Removing sentry breadcrumb handling from watchdog
**What goes wrong:** If old sentry binaries are still deployed on some pods, they might write breadcrumbs. Watchdog must still handle stale breadcrumbs gracefully.
**How to avoid:** Keep the breadcrumb check in watchdog but add a log note that it's legacy. Don't remove the check -- it's cheap and prevents double-restart.

### Pitfall 2: spawn_safe() for processes that need visible windows
**What goes wrong:** CREATE_NO_WINDOW prevents Edge, games, ConspitLink from showing
**How to avoid:** Clearly document that spawn_safe() is for background utilities only. Keep separate `Command::new()` for Edge launch (lock_screen.rs line 891) and game launch (ac_launcher.rs line 567).

### Pitfall 3: Removing rollback too aggressively
**What goes wrong:** Bad binary deploys loop forever until reboot
**How to avoid:** Keep MAINTENANCE_MODE + restart loop detection. Only remove the rename step.

### Pitfall 4: Breaking the regression test in session.rs
**What goes wrong:** `test_watchdog_must_not_use_start_rcagent_bat` fails
**How to avoid:** Don't change the command construction in session.rs

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process spawning helpers | Per-module hidden_cmd() | Shared spawn_safe() in rc-common | 4 duplicates, inconsistent Stdio::null |
| Restart coordination | Custom breadcrumb/sentinel files | Windows SCM service recovery | SCM is battle-tested, zero code to maintain |
| Crash loop detection | Custom timestamp tracking | Keep current (it works) | The restart_timestamps + MAINTENANCE_MODE is already solid |

## Effort Estimate

| Task | Files Changed | Effort | Risk |
|------|---------------|--------|------|
| Create spawn_safe() in rc-common | 1 new file + Cargo.toml | Small | LOW |
| Migrate ~70 Command::new call sites to spawn_safe | ~15 files in rc-agent | Medium | LOW (mechanical) |
| Remove 4 duplicate hidden_cmd() | 4 files | Small | LOW |
| Clean up rc-sentry dead restart code | 2-3 files in rc-sentry | Small | LOW |
| Remove binary rename from rollback_manager | 1 file | Small | MEDIUM |
| Update schtask references in playbooks/docs | 3-4 files | Small | LOW |
| Tests | spawn_safe unit tests + verify existing pass | Small | LOW |
| **Total** | **~25 files** | **1-2 sessions** | **LOW overall** |

## Sources

### Primary (HIGH confidence)
- `crates/rc-watchdog/src/service.rs` -- full watchdog poll loop, 685 lines read
- `crates/rc-watchdog/src/session.rs` -- WTSQueryUserToken spawn, 268 lines read
- `crates/rc-watchdog/src/rollback_manager.rs` -- full rollback logic, 485 lines read
- `crates/rc-agent/src/self_monitor.rs` -- self-restart logic, 573 lines read
- `crates/rc-agent/src/main.rs` -- FreeConsole() at line 800
- `crates/rc-sentry/src/tier1_fixes.rs` -- DELEGATED restart, confirmed neutered
- `scripts/deploy/start-rcagent.bat` -- boot script, 86 lines read
- `grep` audit: 76 Command::new sites, 15 Stdio::null, 38 creation_flags, 4 hidden_cmd duplicates

### Secondary (MEDIUM confidence)
- CLAUDE.md standing rules on Session 0/1, deploy protocol, bat file conventions
- DEBUG-RESTART-ISSUE.md in rc-sentry -- documents the 5 restart methods tested

## Metadata

**Confidence breakdown:**
- Restart flow mapping: HIGH -- read all source files, verified current state
- spawn_safe() design: HIGH -- pattern exists in 4 copies, just needs consolidation
- Rollback removal safety: MEDIUM -- MAINTENANCE_MODE is tested but removing rename is a behavior change
- Effort estimate: HIGH -- counted actual call sites via grep

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable domain, unlikely to change)
