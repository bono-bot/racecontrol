# BUG: Lock Screen Not Fullscreen on NVIDIA Surround Triple Monitors

**Date:** 2026-03-23
**Severity:** HIGH — customers see desktop on 2 of 3 monitors
**Affects:** Pods 5, 6, 7 confirmed. Likely all 8 pods.
**Status:** ROOT CAUSE IDENTIFIED, needs code fix in lock_screen.rs

---

## Symptom

Lock screen Edge kiosk window opens on ONE monitor (center, 2560x1440)
instead of spanning all 3 monitors (7680x1440 virtual desktop via NVIDIA Surround).

Customers see:
- Left monitor: partial dark background or desktop
- Center monitor: kiosk page in a window (not fullscreen)
- Right monitor: desktop wallpaper with "RACE SIMS" logo

## Root Cause

`lock_screen.rs:launch_browser()` (line 576-625) sets:
```
--kiosk URL --edge-kiosk-type=fullscreen
--window-position=0,0 --window-size=7680,1440
```

But Edge `--edge-kiosk-type=fullscreen` fullscreens to the **PRIMARY MONITOR only**,
ignoring `--window-size`. The `--window-position` and `--window-size` flags are
overridden by kiosk mode's own fullscreen behavior.

The code has a `MoveWindow` call after 3s (line 606 comment) to resize, but
Edge kiosk mode fights against external window resizing.

## What Was Tried (During Audit)

1. **Manual Edge launch with --kiosk + fullscreen flags** — opens on one monitor only
2. **Explorer restart to apply taskbar auto-hide** — BROKE NVIDIA Surround (resolution
   dropped to 1024x768). Required pod reboot to restore.
3. **Kill overlay processes** — killed rc-agent accidentally. Required schtasks restart.
4. **Screenshot verification** — confirmed the issue is real (not a proxy check artifact)

## Standing Rule Violations Found

1. **"Never restart explorer.exe on pods with NVIDIA Surround"** — this should be a
   standing rule. Explorer restart disrupts GPU display configuration.
2. **"Smallest reversible fix"** — should have tested on one pod before applying to 3.
3. **Taskbar auto-hide was already ON** — the screenshots showed it because PowerShell
   CopyFromScreen triggers a focus change. The "fix" was unnecessary.

## Hypotheses for Fix (Parallel Session)

**H1: Don't use --edge-kiosk-type=fullscreen. Use regular maximized window.**
- Launch Edge without `--kiosk`: `msedge.exe --start-maximized --window-position=0,0 --window-size=7680,1440 URL`
- Then use Win32 API `SetWindowPos` to force the window to cover all monitors
- This avoids kiosk mode's single-monitor fullscreen behavior

**H2: Use --app mode instead of --kiosk**
- `--app=URL` creates a chromeless window that respects `--window-size`
- Combined with `--window-position=0,0 --window-size=7680,1440` may span all monitors

**H3: Post-launch Win32 MoveWindow with retry loop**
- Current code does MoveWindow once after 3s
- May need: find Edge window by title, call `SetWindowPos(HWND_TOP, 0, 0, 7680, 1440, SWP_SHOWWINDOW)` repeatedly until it sticks
- Edge kiosk may reset position — need to check if `--kiosk` is fighting the resize

**H4: Two-step: launch normal, then F11 fullscreen**
- Launch Edge normally spanning all monitors
- Send F11 keystroke via SendKeys to enter browser fullscreen (spans all monitors)
- This is how users manually go fullscreen on surround setups

## Code Paths

- `crates/rc-agent/src/lock_screen.rs:576` — `launch_browser()`
- `crates/rc-agent/src/lock_screen.rs:600` — `get_virtual_screen_bounds()`
- `crates/rc-agent/src/lock_screen.rs:931` — `enforce_kiosk_foreground()`
- `crates/rc-agent/src/lock_screen.rs:18923` — built-in HTTP server port

## Approaches Tested (2026-03-23)

| # | Approach | Result |
|---|---------|--------|
| 1 | `--kiosk --edge-kiosk-type=fullscreen` (original) | Single monitor only |
| 2 | `--app=URL --start-fullscreen --window-size=7680,1440` | Still single monitor |
| 3 | `--app` + `MoveWindow(0,0,7680,1440)` after 3s | Returns OK, doesn't visually span |
| 4 | `--app` + `PostMessage(WM_KEYDOWN, VK_F11)` | Edge ignores posted keys |
| 5 | `--app` + `keybd_event(VK_F11)` | F11 not received in --app mode |
| 6 | `--app` + `SetWindowPos` via PowerShell | Dark background but dialog overlay |

## Untested Approaches

- **H6:** Normal Edge (no --kiosk/--app) + MoveWindow + F11 — manual F11 DOES span surround
- **H7:** Native Win32 fullscreen window + WebView2 — full control, biggest change
- **H8:** PowerShell `SendKeys('{F11}')` — targets focused app specifically
- **H9:** Edge Group Policy kiosk — enterprise kiosk may handle surround

## Playwright Screenshot Debug Methodology

Tool: `C:/Users/bono/verify-pod-screen.js`
```bash
node verify-pod-screen.js <pod_ip> [output_path]
```
Checks: lock screen HTML, popup processes, resolution, centering.

**Screenshot capture + download flow:**
1. Write `capture-screen.ps1` via rc-agent `/write`
2. Execute via rc-agent `/exec`
3. Write `serve-png.ps1` via `/write` (PowerShell HTTP one-shot)
4. Execute serve, download via `curl -o`
5. Resize via Playwright for viewing

## Deploy via Tailscale (Safe Method — No Display Disruption)

```bash
# SCP binary + bat (no process kill)
scp User@<tailscale_ip>:"C:/RacingPoint/rc-agent-new.exe" rc-agent.exe
scp -T User@<tailscale_ip>:"C:\\RacingPoint\\start-rcagent.bat" start-rcagent.bat
# Reboot (bat swaps binary, NVIDIA Surround restores)
ssh User@<tailscale_ip> "shutdown /r /t 5 /f"
```

**CRITICAL: Use Write tool for bat files, not bash heredoc.** Git Bash converts `nul` → `/dev/null`.

## DO NOT

- **DO NOT restart explorer.exe on pods** — breaks NVIDIA Surround (1024x768)
- **DO NOT taskkill WindowsTerminal** — hosts rc-agent/rc-sentry processes
- **DO NOT taskkill overlay processes in bulk** — may kill rc-agent children
- **DO NOT use screenshot as sole diagnostic** — CopyFromScreen triggers taskbar
- **DO NOT use bash heredoc for bat files** — Git Bash converts nul→/dev/null
- **DO NOT deploy rc-agent via exec+schtasks** — restart breaks NVIDIA Surround. Use Tailscale SCP + reboot
- **DO NOT kill Edge processes remotely** — triggers NVIDIA Surround collapse
