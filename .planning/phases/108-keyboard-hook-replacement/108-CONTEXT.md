# Phase 108: Keyboard Hook Replacement - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

This phase removes the SetWindowsHookEx global keyboard hook from rc-agent's active code path and replaces it with GPO registry key writes for kiosk lockdown. The hook code is preserved behind a Cargo feature flag for emergency rollback. After this phase, no hook install/uninstall cycle is visible to anti-cheat systems.

</domain>

<decisions>
## Implementation Decisions

### Hook Replacement Strategy
- Block Win key via GPO registry: `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\NoWinKeys=1`
- Block Task Manager via GPO registry: `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System\DisableTaskMgr=1`
- Alt+Tab cannot be fully blocked via GPO alone — accept this limitation. `hide_taskbar(true)` (already exists) makes Alt+Tab less useful since there's nothing to switch to.
- Apply registry keys on kiosk lockdown start (`set_lockdown(true)`), remove on lockdown release (`set_lockdown(false)`) — same lifecycle as current hook

### Cleanup Approach
- Keep SetWindowsHookEx code behind a Cargo feature flag (e.g., `keyboard-hook`) for emergency rollback
- Default build does NOT include the feature — hook code is dead by default
- Feature flag documented in README/CLAUDE.md for ops reference
- All call sites (`install_keyboard_hook()`, `remove_keyboard_hook()`) gated behind `#[cfg(feature = "keyboard-hook")]`

### Claude's Discretion
- Exact Cargo feature flag name (suggested: `keyboard-hook`)
- Registry key write implementation details (winreg crate vs raw Win32 API)
- Error handling for registry write failures
- Test structure for verifying lockdown works without hook

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/rc-agent/src/kiosk.rs:956` — `install_keyboard_hook()` with SetWindowsHookExW
- `crates/rc-agent/src/kiosk.rs:973` — `remove_keyboard_hook()` with UnhookWindowsHookEx
- `crates/rc-agent/src/kiosk.rs:891` — `keyboard_hook_proc()` callback
- `crates/rc-agent/src/kiosk.rs:983` — `hide_taskbar()` already exists and works
- `crates/rc-agent/src/kiosk.rs:467` — call site in `set_lockdown(true)`
- `crates/rc-agent/src/kiosk.rs:482` — call site in `set_lockdown(false)`

### Established Patterns
- `#[cfg(windows)]` / `#[cfg(not(windows))]` split for platform-specific code
- `pub use windows_impl::` re-exports at module level
- `tracing::info!` with LOG_TARGET for kiosk events

### Integration Points
- `set_lockdown(true/false)` in kiosk.rs is the hook lifecycle manager — registry keys go here
- `Cargo.toml` for rc-agent needs the feature flag definition
- Phase 109 (safe mode) will NOT need to manage the hook since it's gone from default builds

</code_context>

<specifics>
## Specific Ideas

- Phase 107 audit confirmed: all 8 pods are Windows 11 Pro — Keyboard Filter is NOT available
- GPO registry keys are the only viable path
- The existing `hide_taskbar(true)` call already prevents meaningful Alt+Tab usage
- Consider also blocking Ctrl+Shift+Esc (direct Task Manager shortcut) — DisableTaskMgr registry key handles this

</specifics>

<deferred>
## Deferred Ideas

- Windows Keyboard Filter integration (requires Enterprise/Education SKU upgrade) — deferred to v15.1
- Full Alt+Tab blocking without hooks — no known safe method exists

</deferred>
