# Phase 331-01: spawn_safe() Migration - Research

**Researched:** 2026-04-07
**Domain:** Rust process spawning, Windows creation_flags, FreeConsole() stdio invalidation
**Confidence:** HIGH

## Summary

The `spawn_safe()` module already exists in `rc-common/src/spawn_safe.rs` and has been widely adopted across most of rc-agent. The 4 duplicate `hidden_cmd()` functions have already been deleted. However, **20 `Command::new` call sites remain unmigrated** across 8 files. These fall into three categories: (1) tokio async `Command::new` that cannot use the sync `spawn_safe()`, (2) game/Edge launches that intentionally need visible windows or special creation_flags, and (3) a few oversights from the initial migration.

**Primary recommendation:** The plan needs updating -- the scope is smaller than originally estimated (20 remaining sites, not 76). The key challenge is the tokio async `Command::new` sites which use `tokio::process::Command`, not `std::process::Command`. A `spawn_safe_async()` variant may be needed, or these sites should be manually annotated.

## Current State (Post-Initial Migration)

### spawn_safe() Already Exists and Works
- **File:** `crates/rc-common/src/spawn_safe.rs` (83 lines)
- **Exported via:** `crates/rc-common/src/lib.rs` line 16: `pub mod spawn_safe;`
- **Two functions:** `spawn_safe()` (all stdio null) and `spawn_safe_capture()` (stdin null, stdout/stderr default)
- **Both set:** `CREATE_NO_WINDOW` (0x08000000) on Windows via `#[cfg(windows)]`
- **Tests:** 4 unit tests, all passing

### hidden_cmd() -- ALREADY DELETED
The research from 331-RESEARCH.md listed 4 duplicate `hidden_cmd()` in ac_launcher.rs, lock_screen.rs, ai_debugger.rs, and game_process.rs. **All 4 have been deleted.** `grep -rn "fn hidden_cmd" crates/rc-agent/src/` returns zero results.

### Adoption Status
**Already migrated (using spawn_safe/spawn_safe_capture):** 18 files, ~100+ call sites
- ac_launcher.rs, ai_debugger.rs, diagnostic_engine.rs, dxgi_capture.rs, event_loop.rs, ffb_controller.rs, firewall.rs, game_doctor.rs, game_process.rs, knowledge_base.rs, kiosk.rs, lock_screen.rs, main.rs, pre_flight.rs, process_guard.rs, safe_mode.rs, self_heal.rs, self_monitor.rs (partially), self_test.rs, session_enforcer.rs (partially), startup_cleanup.rs, steam_checks.rs, tier_engine.rs (partially)

## Remaining Unmigrated Command::new Call Sites (20 total)

### Category A: Tokio Async Command (5 sites) -- CANNOT use sync spawn_safe()
These use `tokio::process::Command` not `std::process::Command`. The existing `spawn_safe()` returns `std::process::Command`.

| File | Line | Command | Current Flags | Issue |
|------|------|---------|---------------|-------|
| `debug_server.rs` | 220 | `tokio::process::Command::new("powershell")` | CREATE_NO_WINDOW manually set | Missing Stdio::null on stdin |
| `debug_server.rs` | 251 | `tokio::process::Command::new(tool)` (Linux screenshot) | None | Linux-only, non-production |
| `ws_handler.rs` | 114 | `tokio::process::Command::new("cmd")` | CREATE_NO_WINDOW manually set | Missing Stdio::null on stdin |
| `remote_ops.rs` | 972 | `tokio::process::Command::new("cmd")` | CREATE_NO_WINDOW manually set | Missing Stdio::null on stdin |
| `remote_ops.rs` | 1393 | `tokio::process::Command::new("cmd")` | CREATE_NO_WINDOW manually set | Missing Stdio::null on stdin |

**Note:** `remote_ops.rs` line 1111 (`run_ps()`) also uses `tokio::process::Command::new("powershell")` with CREATE_NO_WINDOW -- same pattern.

**Options:**
1. **Add `spawn_safe_async()` to rc-common** returning `tokio::process::Command` -- this requires adding `tokio` as a dependency to rc-common (currently has zero external deps)
2. **Manually add Stdio::null to each async site** -- keeps rc-common dependency-free, 6 sites to fix
3. **Leave as-is with `// ASYNC: manual flags` comments** -- these already have CREATE_NO_WINDOW, only missing stdin null

**Recommendation:** Option 2 -- manually add `.stdin(std::process::Stdio::null())` to each async site. Adding tokio to rc-common just for this is overkill. These 6 sites already have CREATE_NO_WINDOW correctly set; they just need stdin null for FreeConsole() safety.

### Category B: Visible Window Launches (5 sites) -- MUST NOT use spawn_safe()
These intentionally need visible windows or special creation_flags.

| File | Line | Command | Flags | Why Special |
|------|------|---------|-------|-------------|
| `ac_launcher.rs` | 691 | `Command::new(ac_dir.join("acs.exe"))` | DETACHED_PROCESS \| CREATE_NEW_PROCESS_GROUP + all Stdio::null | SP game launch -- needs DETACHED not CREATE_NO_WINDOW |
| `ac_launcher.rs` | 708 | `Command::new("cmd")` (MP bat) | CREATE_NO_WINDOW \| CREATE_NEW_PROCESS_GROUP + all Stdio::null | MP game launch via bat |
| `ac_launcher.rs` | 1689 | `Command::new(ac_dir.join("acs.exe"))` | NONE | Legacy/fallback acs.exe launch -- **MISSING all flags and Stdio::null** |
| `tier_engine.rs` | 2497 | `Command::new(edge_path)` (msedge.exe) | NONE | Edge kiosk launch -- **VISIBLE window needed, but MISSING Stdio::null** |
| `game_process.rs` | 321 | `Command::new(exe_path)` (generic game) | NONE | Generic game launch -- **MISSING all flags and Stdio::null** |

**Issues found:**
1. `ac_launcher.rs:1689` -- launches acs.exe with NO Stdio::null and NO creation_flags. This will fail after FreeConsole() with os error 6. Needs at minimum Stdio::null + DETACHED_PROCESS.
2. `tier_engine.rs:2497` -- launches Edge with NO Stdio::null. Needs Stdio::null (Edge handles its own window). Same pattern as the primary Edge launch in lock_screen.rs.
3. `game_process.rs:321` -- generic game exe launch with NO Stdio::null and NO creation_flags. Needs at minimum Stdio::null + DETACHED_PROCESS for game isolation.

### Category C: Self-Restart/Special (2 sites) -- Need DETACHED_PROCESS specifically
| File | Line | Command | Current Flags | Issue |
|------|------|---------|---------------|-------|
| `self_monitor.rs` | 417 | `std::process::Command::new("powershell")` | DETACHED_PROCESS only | Correct -- needs DETACHED not CREATE_NO_WINDOW |
| `remote_ops.rs` | 876 | `tokio::process::Command::new("cmd")` (detached exec) | DETACHED_PROCESS only | Correct -- intentionally detached |

These are **correctly configured** and should NOT be changed to spawn_safe() because:
- DETACHED_PROCESS (0x8) is different from CREATE_NO_WINDOW (0x08000000)
- spawn_safe() sets CREATE_NO_WINDOW which is wrong for self-restart
- MMA-P2 comment at remote_ops.rs:878 explicitly says "Use only DETACHED_PROCESS"

### Category D: Non-Windows / Linux-only (4 sites) -- Low priority
| File | Line | Command | Notes |
|------|------|---------|-------|
| `game_process.rs` | 305 | `Command::new("xdg-open")` | `#[cfg(not(target_os = "windows"))]` |
| `game_process.rs` | 411 | `Command::new("xdg-open")` | `#[cfg(not(target_os = "windows"))]` |
| `game_process.rs` | 518 | `Command::new("kill")` | `#[cfg(not(target_os = "windows"))]` |
| `session_enforcer.rs` | 123 | `Command::new("kill")` | `#[cfg(not(target_os = "windows"))]` |

These are behind `#[cfg(not(target_os = "windows"))]` and only run on Linux dev environments. FreeConsole() is Windows-only so these are safe. **No action needed.**

### Category E: Second nvidia-smi with special priority (1 site)
| File | Line | Command | Current Flags | Issue |
|------|------|---------|---------------|-------|
| `predictive_maintenance.rs` | 610 | `std::process::Command::new("nvidia-smi")` | BELOW_NORMAL_PRIORITY_CLASS (0x4000) | Needs both BELOW_NORMAL + CREATE_NO_WINDOW, and Stdio handling |

**Issue:** This uses BELOW_NORMAL_PRIORITY_CLASS but not CREATE_NO_WINDOW. It also does not set Stdio::null on stdin. The `spawn_safe_capture()` sets CREATE_NO_WINDOW but not BELOW_NORMAL_PRIORITY_CLASS. Options:
1. Use `spawn_safe_capture("nvidia-smi")` and lose the priority hint (acceptable -- nvidia-smi is fast)
2. Keep manual but add Stdio::null for stdin safety
3. Add a `spawn_safe_capture_low_priority()` variant (overkill)

**Recommendation:** Replace with `spawn_safe_capture("nvidia-smi")` -- the priority class is a minor optimization. nvidia-smi completes in <100ms. The first nvidia-smi call at line 254 already uses `spawn_safe_capture`.

## Action Summary

| Action | Sites | Files |
|--------|-------|-------|
| Add Stdio::null to async tokio Command sites | 6 | debug_server.rs, ws_handler.rs, remote_ops.rs (3 sites) |
| Fix ac_launcher.rs:1689 -- add Stdio::null + DETACHED_PROCESS | 1 | ac_launcher.rs |
| Fix tier_engine.rs:2497 -- add Stdio::null | 1 | tier_engine.rs |
| Fix game_process.rs:321 -- add Stdio::null + DETACHED_PROCESS | 1 | game_process.rs |
| Replace predictive_maintenance.rs:610 with spawn_safe_capture | 1 | predictive_maintenance.rs |
| Add `// VISIBLE:` or `// DETACHED:` comments on intentional exceptions | 5 | ac_launcher.rs, tier_engine.rs, self_monitor.rs, remote_ops.rs |
| Leave as-is (Linux-only, non-production) | 4 | game_process.rs, session_enforcer.rs |
| **Total changes needed** | **10** | **6 files** |

## Architecture Patterns

### spawn_safe() Usage Rules (Already Established)
```
spawn_safe(prog)         -- fire-and-forget: taskkill, reg, netsh, powershell fire-and-forget
spawn_safe_capture(prog) -- need output: tasklist, netstat, nvidia-smi, powershell queries, reg queries
Command::new(prog)       -- VISIBLE windows: Edge, game exe, ConspitLink (with Stdio::null + custom flags)
tokio::process::Command  -- async contexts with manual Stdio::null + CREATE_NO_WINDOW
```

### Windows Creation Flags Reference
| Flag | Value | Use Case |
|------|-------|----------|
| CREATE_NO_WINDOW | 0x08000000 | Background utilities (default in spawn_safe) |
| DETACHED_PROCESS | 0x00000008 | Games, self-restart (process outlives parent) |
| CREATE_NEW_PROCESS_GROUP | 0x00000200 | Games (prevents CTRL_CLOSE cascade) |
| BELOW_NORMAL_PRIORITY_CLASS | 0x00004000 | Low-priority background tasks |

**Rule:** CREATE_NO_WINDOW and DETACHED_PROCESS are mutually exclusive in behavior. When combined, CREATE_NO_WINDOW is silently ignored. Use DETACHED_PROCESS for anything that must survive parent exit.

### Anti-Patterns to Avoid
- **spawn_safe() for visible processes:** CREATE_NO_WINDOW prevents Edge/games from showing
- **spawn_safe() for self-restart:** DETACHED_PROCESS is needed, not CREATE_NO_WINDOW
- **Missing Stdio::null after FreeConsole():** Causes os error 6 (ERROR_INVALID_HANDLE)
- **Adding tokio to rc-common:** rc-common has zero external deps -- keep it that way

## Common Pitfalls

### Pitfall 1: tokio::process::Command vs std::process::Command
**What goes wrong:** `spawn_safe()` returns `std::process::Command`. Passing it where `tokio::process::Command` is expected causes a compile error -- but the TEMPTATION is to convert between them, which doesn't work cleanly.
**How to avoid:** For async sites, manually set Stdio::null + creation_flags inline. Keep the pattern consistent with a comment: `// ASYNC: manual spawn_safe pattern (tokio::process::Command)`

### Pitfall 2: ac_launcher.rs:1689 -- the forgotten acs.exe launch
**What goes wrong:** This is a fallback acs.exe launch path (after CM failure in MP mode) with NO Stdio::null and NO creation_flags. After FreeConsole(), this WILL fail with os error 6.
**How to avoid:** Add the same Stdio::null + DETACHED_PROCESS pattern as the primary launch at line 691.

### Pitfall 3: game_process.rs:321 -- generic game launch
**What goes wrong:** Generic `Command::new(exe_path)` for non-AC games with no Stdio::null or creation_flags.
**How to avoid:** Add Stdio::null + DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP. Games need DETACHED to survive independently.

## Verification Strategy

After migration, run these checks:

```bash
# 1. All tests pass
cargo test -p rc-common -- spawn_safe
cargo test -p rc-agent

# 2. Release build succeeds
cargo build --release --bin rc-agent

# 3. Count remaining bare Command::new (should be ~9: 5 visible/detached + 4 Linux-only)
grep -rn "Command::new" crates/rc-agent/src/ | grep -v spawn_safe | grep -v "// VISIBLE" | grep -v "// DETACHED" | grep -v "// ASYNC" | grep -v "// LINUX" | grep -v "#\[cfg(not"

# 4. All async sites have Stdio::null
grep -A5 "tokio::process::Command::new" crates/rc-agent/src/ | grep -c "Stdio::null"
```

## Sources

### Primary (HIGH confidence)
- `crates/rc-common/src/spawn_safe.rs` -- full source read (83 lines)
- `grep "Command::new" crates/rc-agent/src/` -- 20 remaining sites identified and categorized
- `grep "spawn_safe" crates/rc-agent/src/` -- 100+ already-migrated sites confirmed
- `grep "creation_flags" crates/rc-agent/src/` -- 10 manual flag sites analyzed
- Each unmigrated file read at the relevant lines for full context

### Secondary (MEDIUM confidence)  
- 331-RESEARCH.md -- original research (some counts now stale after partial migration)
- 331-02-SUMMARY.md -- confirms rollback/sentry cleanup already done
- CLAUDE.md standing rules on process spawning, DETACHED_PROCESS, Session 0/1

## Metadata

**Confidence breakdown:**
- Current state mapping: HIGH -- grep verified, every remaining site read in context
- Migration plan: HIGH -- mechanical changes with clear patterns
- tokio async decision: HIGH -- adding tokio to rc-common is wrong; manual pattern is correct
- Bug identification (3 missing Stdio::null sites): HIGH -- these are real bugs waiting to happen

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable domain)

## Key Delta from 331-RESEARCH.md

The original research estimated 76 Command::new sites needing migration. The ACTUAL current state shows most migration is already done. Only **10 sites need changes** across **6 files**. The plan (331-01-PLAN.md) needs to be adjusted:
- Task 1 (create spawn_safe module): **ALREADY DONE** -- skip entirely
- Task 2 (migrate 76 sites + delete hidden_cmd): **90% done** -- only 10 remaining sites need fixes
- The 4 hidden_cmd() copies: **ALREADY DELETED**
- New work: fix 3 bug sites (missing Stdio::null on game launches), add Stdio::null to 6 async sites, replace 1 nvidia-smi call
