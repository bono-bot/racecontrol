---
status: investigating
trigger: "staff-launch-pin-grid-block — staff fires game launch from kiosk staff page, Pod 4 display stays on customer PIN grid instead of showing game or launching state"
created: 2026-04-11T00:00:00+05:30
updated: 2026-04-11T00:00:00+05:30
---

## Current Focus
<!-- OVERWRITE on each update - reflects NOW -->

hypothesis: H7-CONFIRMED — For non-AC games (F1 25, iRacing, LMU, Forza, FH5), ws_handler.rs calls show_launch_splash() on LaunchGame, which calls show_native_window() re-showing the lock screen. For AC games, close_browser() is called after ac_launcher::launch_ac() returns (line 767), hiding the lock screen. For non-AC games there is NO equivalent close_browser() call after game process starts. The splash/PIN-grid stays on screen permanently.
test: reading code path for F1 25 (non-AC) vs AC to confirm no close_browser call exists in generic branch
expecting: confirmed no close_browser in lines 843-1158 of ws_handler.rs
next_action: verify fix location — add close_browser() after non-AC game process starts, analogous to AC path at line 767

## Symptoms
<!-- Written during gathering, then IMMUTABLE -->

expected: After staff selects Pod 4 + F1 25 + 30-min tier on http://192.168.31.23:3300/kiosk/staff and fires launch, Pod 4's physical display leaves idle/PIN screen and shows "Launching..." or F1 25 itself. Kiosk staff page shows per-pod launch status indicator.

actual: Pod 4's physical display remains stuck on the customer-self-service PIN grid (numeric keypad UI from customer-PIN flow). No visible indication anywhere — pod display, kiosk staff page — confirming whether F1 25 launched behind the PIN grid.

errors: None observed. Problem is silence — no error, no status, no transition. Both "successful launch hidden behind a screen" and "launch silently failed" present identically.

reproduction:
1. Open http://192.168.31.23:3300/kiosk/staff in browser
2. Select Pod 4 (IP 192.168.31.88)
3. Select game F1 25 + 30-minute time tier
4. Click Launch
5. Observe Pod 4's physical display — PIN grid still showing, no transition

started: Unknown — likely never worked correctly for non-AC games via staff-launch flow, or regressed during v46.0/v47.0 deploys (2026-04-11)

## Eliminated
<!-- APPEND only - prevents re-investigating -->

- hypothesis: H1 — kiosk staff button fires nothing
  evidence: The staff page source (staff/page.tsx handleGameLaunch) shows the button calls api.startBilling then api.launchGame. The UI transitions to "live_session" panelMode after launch. The issue is on the pod-side display, not the kiosk UI.
  timestamp: 2026-04-11T~00:10+05:30

- hypothesis: H2 — server receives but doesn't forward (feature flag / 409)
  evidence: The lock screen splash IS shown (show_launch_splash called at line 1009), which means LaunchGame DID arrive at rc-agent. If the server never forwarded it, no lock screen transition would happen at all. The bug is that the splash stays visible, not that nothing happened.
  timestamp: 2026-04-11T~00:15+05:30

- hypothesis: H3 — pod rc-agent never receives WS message
  evidence: Same as H2 — show_launch_splash fires on LaunchGame receipt, so rc-agent receives the message.
  timestamp: 2026-04-11T~00:15+05:30

- hypothesis: H5 — F1 25 launches but Edge kiosk window stays on top
  evidence: There is no Edge kiosk window on pods in the current architecture. Pods use the rc-agent native Win32 lock screen (NativeLockScreen). Edge was the old architecture; CLAUDE.md confirms current pods use native lock screen.
  timestamp: 2026-04-11T~00:15+05:30

- hypothesis: H8 — Pod 4 on stale build
  evidence: Not the root cause. The code path analysis shows the missing close_browser() is a design gap in the non-AC launch branch, not a stale build issue. All builds that have the generic non-AC launch path (lines 843-1159) have this bug.
  timestamp: 2026-04-11T~00:20+05:30

## Evidence
<!-- APPEND only - facts discovered -->

- timestamp: 2026-04-11T~00:10+05:30
  checked: kiosk/src/components/PodKioskView.tsx + kiosk/src/app/pod/[number]/page.tsx
  found: No PIN grid in the Next.js kiosk app. The "PIN grid" is the rc-agent native Win32 lock screen window (LockScreenState::PinEntry or LaunchSplash), hosted by rc-agent's embedded lock screen server on port 18923, NOT the kiosk Next.js app.
  implication: Bug is in rc-agent lock screen state management, not in kiosk frontend.

- timestamp: 2026-04-11T~00:15+05:30
  checked: crates/rc-agent/src/ws_handler.rs LaunchGame handler (lines 449-1162)
  found: For AC games (AssettoCorsa branch, line 609-842): after ac_launcher::launch_ac() returns with a game pid, close_browser() is called (line 767) which calls hide_native_window() — hides the lock screen. For ALL non-AC games (generic branch, line 843-1159): show_launch_splash() is called (line 1009, calls show_native_window()), then game is launched, but close_browser() is NEVER called. The lock screen (now in LaunchSplash state) stays visible indefinitely.
  implication: Root cause: missing close_browser() in the non-AC game launch branch for the case where game process is confirmed started.

- timestamp: 2026-04-11T~00:18+05:30
  checked: ws_handler.rs BillingStarted handler (line 222-380), show_active_session() in lock_screen.rs
  found: BillingStarted calls show_active_session() (line 312) which calls hide_native_window(). BUT in the staff flow, billing is started FIRST (api.startBilling before api.launchGame in staff/page.tsx handleGameLaunch). So BillingStarted arrives BEFORE LaunchGame. When LaunchGame arrives later, show_launch_splash() re-shows the native window — overriding the hide from BillingStarted. Confirmed sequence: (1) BillingStarted → hide, (2) LaunchGame → show_launch_splash → show, (3) game starts → [AC: close_browser → hide] vs [non-AC: nothing → stays shown].
  implication: The fix must call close_browser() (hide) after the non-AC game process is confirmed started, analogous to the AC branch at line 767.

- timestamp: 2026-04-11T~00:20+05:30
  checked: All callers of close_browser() in rc-agent: ws_handler.rs lines 767, 1849
  found: Line 767: AC-only, after launch_ac() returns with pid. Line 1849: ForceRelaunchBrowser handler (close + relaunch). No close_browser() call exists anywhere in the non-AC game launch path (lines 1031-1158).
  implication: Single-location fix: add close_browser() after non-AC game process confirmed started (line ~1075 for direct launch with pid, and deferred via ws_exec_result_tx for GAME-07 Steam URL launch path).

## Resolution
<!-- OVERWRITE as understanding evolves -->

root_cause: |
  In crates/rc-agent/src/ws_handler.rs, the LaunchGame handler has two branches:
  (1) AssettoCorsa (lines 609-842): after launch_ac() returns with pid, calls state.lock_screen.close_browser() at line 767 — CORRECTLY hides the native Win32 lock screen.
  (2) All other sims — F1 25, iRacing, LMU, Forza, FH5 (lines 843-1159): calls show_launch_splash() at line 1009 which calls show_native_window() — but NEVER calls close_browser(). For F1 25 (Steam URL launch via use_steam=true), the game window is detected asynchronously by a tokio::spawn task that confirms the window after ~10-60s but also never calls close_browser(). The lock screen stays visible permanently over the game window.
  Secondary issue: LaunchGame arrives AFTER BillingStarted in the staff flow. BillingStarted calls show_active_session() which hides the window. But show_launch_splash() in LaunchGame immediately re-shows it. The AC branch closes it AFTER the game starts. The non-AC branch has no such close.
  Third issue: staff/page.tsx does not consume `launches` (Phase 368 LaunchStatusCard from useKioskSocket) — the per-pod launch status indicator exists in the debug page only, not on the staff page.

fix: |
  1. app_state.rs: Add lock_screen_hide_tx/rx mpsc channel fields (for GAME-07 async signaling).
  2. main.rs: Create the channel, populate AppState fields.
  3. ws_handler.rs direct-pid path (line ~1076): Add state.lock_screen.close_browser() immediately after game pid confirmed.
  4. ws_handler.rs GAME-07 path: Clone lock_screen_hide_tx before tokio::spawn; send () after game window confirmed.
  5. event_loop.rs select!: Add arm for lock_screen_hide_rx.recv() → state.lock_screen.close_browser().
  Fix 1 is for the pod display. Fix for the staff page status indicator is a separate follow-up (add launches consumption to staff/page.tsx or KioskPodCard).

verification:
files_changed:
  - crates/rc-agent/src/app_state.rs
  - crates/rc-agent/src/main.rs
  - crates/rc-agent/src/ws_handler.rs
  - crates/rc-agent/src/event_loop.rs
