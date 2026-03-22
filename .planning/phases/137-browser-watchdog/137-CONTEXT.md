# Phase 137: Browser Watchdog - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Add a browser watchdog loop to rc-agent's LockScreenManager that polls Edge process liveness every 30s, detects stacking (>5 msedge.exe), and auto-relaunches. Fix close_browser() to kill ALL Edge/WebView2 processes. Gate all taskkill behind anti-cheat safe mode check.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure infrastructure phase.

Key constraints from standing rules:
- Standing rule #10: recovery systems must not fight each other (browser watchdog vs rc-sentry vs pod_monitor)
- Standing rule #74 (CLAUDE.md): cargo test must not execute real system commands — auto-fix functions need #[cfg(test)] guards
- Anti-cheat safe mode (v15.0): any taskkill must check safe_mode_active AtomicBool before executing

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `LockScreenManager` in crates/rc-agent/src/lock_screen.rs — owns browser_process: Option<Child>, launch_browser(), close_browser()
- `safe_mode_active: Arc<AtomicBool>` already on LockScreenManager (line 159)
- `get_virtual_screen_bounds()` for multi-monitor Edge positioning
- `FindWindowA("Chrome_WidgetWin_1", NULL)` + `MoveWindow()` for window forcing (lines 635-654)

### Established Patterns
- Event loop tick intervals in event_loop.rs (e.g., 30s maintenance retry at line 106)
- `tokio::time::interval` for periodic tasks
- `std::process::Command::new("taskkill")` for process killing (used in ai_debugger.rs fix functions)
- `#[cfg(test)]` guards on all system command calls (standing rule from C section)

### Integration Points
- `lock_screen.rs:576` launch_browser() — spawns Edge with kiosk flags
- `lock_screen.rs:690` close_browser() — currently only kills self.browser_process child
- `event_loop.rs` — main loop where the watchdog tick would be added
- `app_state.rs` — shared state accessible from event loop

</code_context>

<specifics>
## Specific Ideas

Incident context: Pod 6/7 had 25 stacked Edge processes (13 msedge + 12 msedgewebview2). close_browser() only killed the spawned child, not stale processes from prior crashes. Edge died but rc-agent didn't notice because there's no browser liveness check.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>
