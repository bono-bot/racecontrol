# Phase 5: Blanking Screen Protocol - Context

**Gathered:** 2026-03-13
**Status:** Ready for planning

<domain>
## Phase Boundary

Pod screens show only branded Racing Point UI to customers at all times — no Windows desktop, taskbar, file explorer, error popups, or system dialogs visible. PIN auth works identically across all entry points (pod lock screen, kiosk, PWA). All error popups (WerFault, game crashes, ConspitLink, Windows Update) are suppressed. Desktop hiding prevents any system UI leaking through during game transitions.

</domain>

<decisions>
## Implementation Decisions

### Anti-cheat compatibility
- **Moderate approach:** Suppress WerFault during gameplay (already reactive in auto-fix). No NEW process manipulation during active game sessions beyond what's already happening
- rc-agent already kills WerFault.exe reactively via ai_debugger.rs auto-fix — this is acceptable during gameplay
- No proactive polling loop that kills popups every N seconds during active sessions — anti-cheat risk
- Between sessions (no billing active): aggressive suppression is safe — kill all error dialogs, manage windows freely
- **Test requirement:** Before deploying, test all three online games (iRacing, F1 25, LMU) with rc-agent running to verify no anti-cheat kicks or bans from current WerFault killing + Edge kiosk behavior
- Test all three games in sequence before claiming Phase 5 complete

### Dialog suppression strategy
- WerFault.exe: Already handled reactively by auto-fix. Continue this approach during gameplay
- Between sessions: Proactive sweep of known popup processes (WerFault, crash dialogs, update prompts) during cleanup_after_session and enforce_safe_state
- ConspitLink messages: Managed by 10s watchdog in main loop — no change needed
- Windows Update prompts: Suppress via Group Policy on pods (defer updates, disable restart notifications)
- "Application has stopped working" dialogs: Add to the suppress list alongside WerFault

### Lock screen coverage and transitions
- **Game launch:** Show branded splash screen ("Preparing your session...") between lock screen close and game window visible. Customer sees Racing Point branding during shader compilation delay, CM loading, etc.
- **Game exit (session end):** Launch Edge kiosk lock screen FIRST (covers screen), THEN kill the game process. Customer never sees desktop during transition
- **Game crash:** Same as game exit — lock screen first, then cleanup. The crash dialog (WerFault) gets killed by the cleanup step
- Lock screen uses Edge kiosk fullscreen mode (already implemented) — covers entire screen

### Desktop hiding
- **Taskbar:** Hide via registry on all pods (not just auto-hide — fully hidden). Requires reboot to apply
- **File Explorer:** Disable file browser windows via Group Policy or registry. If customer alt-tabs, they see only Edge lock screen window behind the game
- **Keyboard shortcuts:** Block Win key, Alt+Tab, Ctrl+Esc on pods via registry/Group Policy. Prevents customers from leaving fullscreen games
- **Recovery:** Admin login required to undo lockdown. All changes applied via pod-agent /exec during deploy
- These are one-time pod setup changes, not rc-agent runtime code

### PIN auth unification
- **Single shared function:** One `validate_pin()` handles all 3 callers (pod lock screen, staff kiosk, customer PWA). Callers pass source context (pod|kiosk|pwa) for logging only
- **No rate limiting:** PINs are 4-digit, change per session, and venue is physical. Brute force risk is negligible
- **Error messages:** Claude's discretion — just ensure identical messages across all 3 surfaces
- **Response time:** Claude's discretion — optimize as needed, just don't let it feel sluggish (PERF-02 says 1-2s target)

### Claude's Discretion
- Exact error message text for wrong PIN (just make it identical everywhere)
- PIN response time optimization approach (within 1-2s target)
- Branded splash screen design during game launch
- Specific registry keys for taskbar hiding and keyboard lockdown
- Whether to use Group Policy objects or direct registry edits for pod lockdown
- Dialog suppression implementation details (polling interval between sessions, process list)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `lock_screen.rs` LockScreenManager: Full lifecycle management — show_blank_screen(), show_pin_screen(), launch_browser(), close_browser(), suppress_notifications()
- `ai_debugger.rs` fix_kill_error_dialogs(): Reactive WerFault.exe killing via taskkill — proven safe pattern
- `ac_launcher.rs` cleanup_after_session() + enforce_safe_state(): Kill game processes, dismiss error dialogs, minimize background windows, bring lock screen to foreground
- `auth/mod.rs` validate_pin() + validate_pin_kiosk(): Two separate validation functions that need unification
- Edge kiosk mode: --kiosk --edge-kiosk-type=fullscreen — already deployed on all pods

### Established Patterns
- Edge kiosk fullscreen for lock screen display (lock_screen.rs)
- suppress_notifications() via Focus Assist registry key (lock_screen.rs)
- PowerShell process manipulation for foreground window control (ac_launcher.rs)
- pod-agent /exec for remote command execution on pods (field is `cmd` not `command`)
- Phase 1 ConfigError: Generic messages only — no technical details to customers

### Integration Points
- Lock screen state transitions in main.rs event loop (lines 807-1250)
- Game launch sequence in ac_launcher.rs (launch → wait for window → close lock screen)
- Session end flow in billing.rs (end billing → cleanup → show lock screen)
- Pod setup via deploy scripts (deploy-staging/, install.bat)

</code_context>

<specifics>
## Specific Ideas

- Branded splash screen during game launch — Racing Point colors (#E10600 red, #1A1A1A black), Enthocentric font header
- Lock screen before game kill on session end — no desktop flash, ever
- Test iRacing, F1 25, and LMU with rc-agent's WerFault killing before deploying Phase 5 to verify anti-cheat safety
- Registry-based taskbar hiding (not just auto-hide) — permanent until admin reverts

</specifics>

<deferred>
## Deferred Ideas

- Shell replacement (replace explorer.exe with rc-agent) — too risky for recovery if rc-agent crashes. Revisit if registry+GP lockdown proves insufficient
- USB mass storage lockdown — already tracked in MEMORY.md TODOs, separate from screen protocol

</deferred>

---

*Phase: 05-blanking-screen-protocol*
*Context gathered: 2026-03-13*
