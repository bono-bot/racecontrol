# 5-Section RCA — Pod blank/idle screen: native GDI black → pod-display Idle (V2.0) web design

> **Class:** FOUNDATIONAL (pod-state-channel) · V1-dependent V2 change · display-affecting.
> **Trigger:** Captain `/goal` 2026-06-06 — *"replace the existing UI completely with the updated version… no overlapping of the existing design"* in the context of *"Pod Display → Idle (V2.0) should be the default blanking screen."*
> **Gates:** RCA (this doc) → MMA Step-1 DIAGNOSE (≥5 models, running) → H1 PLAN → pod-8 canary → on-screen visual verify → **per-PR Captain merge auth**.
> **Mechanism-trust-check:** `MECHANISM-TRUST/webidle-blank-20260606.json` — verdict FAIL (4/5) → this RCA engineers the 5 guarantees.
> **Baseline (probe-backed 2026-06-06 03:45 IST):** 8 pods uniform `rc-agent a826b100` ws+http True; pod-display serving `.23:3340` HTTP 200 ("RacingPoint · Pod Display"); heart `/api/v1/fleet/health` exposes a `displays` array (the behavioral-verify signal).

## §1 — Boundary map (paths + lines)
The change crosses these V1↔V2 seams in `crates/rc-agent/src/` + `crates/rc-common/src/`:
- **State machine:** `lock_screen.rs` — `LockScreenState` enum (`ScreenBlanked`, `ActiveSession`, `PinEntry`, `LaunchSplash`, …); `LockScreenManager` struct fields `browser_disabled` (POS-01 gate), `customer_self_service_mode`, `safe_mode_active`.
- **Idle entry:** `show_idle_state()` (L236-242) → `show_blank_screen()` (L571-595) sets `*state = ScreenBlanked` **after** a `window_alive || browser_disabled` gate (L588) — the non-atomic seam.
- **Game entry (hides idle):** `show_active_session()` (L438-459) → `hide_native_window()` (L454).
- **Native window API:** `native_lock/mod.rs` — `show()`/`hide()` (SW_HIDE, no destroy)/`is_alive()`/`request_show()` (SW_SHOW+SetForegroundWindow)/`request_repaint()`/`get_hwnd()`. The window is `WS_POPUP|WS_EX_TOPMOST` spanning the **entire virtual desktop** (all 3 Surround monitors) — the multi-monitor coverage the web path must preserve.
- **Legacy browser API (vestige):** `lock_screen.rs` L749-766 `launch_browser`/`close_browser`/`is_browser_alive` — thin wrappers now delegating to the native window; `edge_process_count` diagnostic; `ForceRelaunchBrowser` WS handler `ws_handler.rs` L1890-1907 (gated off during billing).
- **Transitions:** `ws_handler.rs` `BillingStarted`→`show_active_session()` (L317); `SessionEnded`→`show_idle_state()` (L1617/L1725).
- **Watchdog fallback:** `event_loop.rs` SAFETY-NET-01 — game process dies while state=ActiveSession → forces `show_idle_state()`.
- **Config:** `rc-common/src/config_schema.rs` `LockScreenConfig{ enabled, customer_self_service_mode }` (L202-222, defaults true/false); wired `main.rs` L1169-1175 (`set_browser_disabled`, `set_customer_self_service_mode`).
- **OS-launch primitive:** `game_process.rs` `launch_url` / `spawn_safe` (`cmd /C start`) — reusable to spawn the browser in Session 1.
- **Process guard:** pod allowlist `/api/v1/guard/whitelist/pod-N` (boot + 5-min re-fetch) — must allowlist the browser exe.
- **Transport (V2 surface):** pod-display Next.js `.23:3340/?pod_id=N` → admin-proxy `:3211` → heart; SSE `/api/v2/pods/state/stream` (401 until staff cookie); heart `displays` registration.

## §2 — Inherited-issue catalogue (the V1 Edge-blank era)
The pods **previously ran an Edge `--kiosk` web blank screen**, retired to the native GDI window (~commit `26d2acf1`, 2026-04-07). Failure modes that **WILL recur** if a browser blank returns unless designed around:
1. **Flicker (v17.0):** browser-watchdog killed+relaunched Edge every 30s + `location.reload()` every 5s → visible flicker on all pods; "declared fixed" 4× without looking at the screens (commits `c633ff66`, `905bc098`, `ecc832d1`).
2. **Multi-monitor bleed-through:** Edge `--kiosk` fullscreened the **primary monitor only** → the Windows desktop was visible on the other 2 of 3 Surround monitors (anchor `9825d69c`). The native window was built to span the full 7680×1440 virtual desktop.
3. **Session 0 vs 1:** a browser spawned from a Session-0 context silently fails to create windows (FIXED by Session-1 spawn `1e1ffbb2` + RCWatchdog `WTSQueryUserToken`).
4. **State-without-behavioral-verify:** `show_blank_screen()` set `state=ScreenBlanked` even when the Edge spawn silently failed (`edge_process_count: 0` while blanked) — proxy success, not render proof.
5. **Anti-cheat / z-order:** a fullscreen browser visible during game init could be flagged as an overlay; v1 kept it behind the game and failed the launch if z-order failed.
6. **Non-ACK control:** `ForceRelaunchBrowser` was fire-and-forget (lost on WS drop); process-guard empty allowlist once flagged/killed processes fleet-wide (28,749 false violations/day).

## §3 — Past-bug disposition
| Inherited issue | V1 status | Recurrence risk if browser returns | Mitigation the design MUST carry |
|---|---|---|---|
| Flicker (kill+relaunch + reload) | PATCHED→FIXED-by-native-migration | **95%** | ONE persistent instance; never kill/relaunch; refresh only on SSE-drop (app already backs off) |
| Multi-monitor bleed-through | FIXED-by-native (full-desktop span) | **HIGH** | New design must cover all 3 monitors with **zero desktop visible** — see §5 |
| Session 0 vs 1 | ROOT-CAUSED-AND-FIXED | MED | Spawn the browser **from rc-agent (Session 1)** only; never a new service/schtask |
| State-without-verify | PATCHED-ONLY | **VERY HIGH** | Behavioral render-verify (heart `displays` / WebView2 NavigationCompleted), never spawn-Ok; TTL fallback |
| Anti-cheat overlay | N/A in native | MED-HIGH | Browser **provably closed/hidden before AC launches**; black floor never desktop |
| Non-ACK control / guard | PATCHED | MED | Add browser exe to pod process-guard allowlist; deliver via rc-sentry atomic-swap; first-run flag-count check |

## §4 — V2-alignment delta
**V2 doctrine (pod = state-channel premise, ratified 2026-05-08):** the customer-facing pod surface should be the **pod-display State surface**, whose State #1 is **IDLE / WELCOMING** (welcome + wallet). Today the idle pod paints a **native black screen** — functionally correct but it is **not the V2.0 design**; it's the V1-era utility blank. **Gap:** the V2.0 Idle design exists and is served (`.23:3340`) but nothing renders it on the pod, so the pod's default surface diverges from the ratified V2 design. **This change moves the boundary toward V2 alignment** by making the V2.0 Idle the literal default pod screen — provided it does so without reintroducing the V1 Edge-blank failure class (§2/§3). It does **not** extend scope (display-only; money path untouched); it is the "complete replacement, no overlap" the Captain directed.

## §5 — V2-framed proposal  (MMA Step-1 DIAGNOSE: 4/4 consensus on Option (c) — see `MMA-DIAGNOSE-webidle-20260606.md`)
**Mechanism — Option (c): native black GDI floor (all 3 monitors) + persistent web Idle pane on top.**
1. **Floor (anti-bleed-through):** the native GDI window is an **always-present black backdrop spanning the full virtual desktop** (`SM_CXVIRTUALSCREEN × SM_CYVIRTUALSCREEN`) whenever the pod is idle — `HWND_BOTTOM`, re-asserted on `WM_WINDOWPOSCHANGING`. It is the *floor*, never the design surface, so any browser crash/refresh/startup gap reveals **black, never the Windows desktop**. This is what makes "no overlap of the old design" hold even under failure.
2. **Pane (the V2.0 design):** rc-agent launches **ONE persistent kiosk browser** (Edge `--kiosk` / WebView2) at `http://192.168.31.23:3340/?pod_id=pod-N`, **centered 1920×1080 on the primary monitor over the floor — NOT stretched** (stretching → 4:1 distortion + DPI artifacts). Z-order `HWND_TOP` over the floor, **not** system `HWND_TOPMOST` (anti-cheat). No kill/relaunch; no `location.reload` (app self-reconnects via `lib/api.ts` backoff); **debounce** idle/active ~500ms.
3. **Behavioral-verify (Q3):** the pod is "showing V2 Idle" only after the browser posts a **ready/DOMReady** message to rc-agent (local socket) — corroborated by heart `displays` registration for pod-N — within ~5s, then a **~2s heartbeat**; 2 misses → repaint black floor + restart. Never spawn-Ok.
4. **Idle→game (anti-cheat, Q1) — TERMINATE not hide:** the browser runs in a **Windows Job Object** (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`). On `BillingStarted`: kill the job and **wait for `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO`** (whole tree incl. the Chromium **GPU child** dead — a hidden HWND or lingering GPU D3D11/DXGI surface gets EAC-flagged), verify `count(msedge)==0`, **then** dispatch AC. Keep the black floor up until the **game window is foreground** (`SetWinEventHook EVENT_SYSTEM_FOREGROUND` + `DwmFlush`), then hide floor in-frame (no desktop flash). On `SessionEnded`: floor up → re-spawn browser → ready-ack → show.
5. **Auth (NEW gap, Q from MMA):** pod-display is **401 until a staff cookie**. Load `about:blank` first; provision auth (**PodDisplayMTLS** device cert per `apps/pod-display/SCAFFOLD-NOTES.md`, or an injected cookie) **before** navigating; **black-floor fallback on 401** — never a white error page.
6. **Spawn context + guard (Q from MMA + MTC Q5):** assert `ProcessIdToSessionId(GetCurrentProcessId())==1` before spawn; **add the browser exe to the pod process-guard allowlist**; **pin a Fixed-Version WebView2 runtime / Edge** + disable Evergreen auto-update (else mid-update file-lock → spawn loop). Deliver via the rc-agent fleet path (rc-sentry atomic swap + bat sync).
7. **Fail-safe (consensus):** if the browser won't die within timeout → **ABORT the game launch**, hold full-screen black floor, alert monitoring. Never launch AC with a browser possibly in memory (ban risk).
8. **Rollout:** new `LockScreenConfig.web_idle` flag **default OFF** (zero behavior change) → **pod-8 canary** with screen-capture frame-hash sampling + **on-screen visual verify** → fleet 1-7 → instant rollback (flag-off + `rc-agent-prev.exe`).

**Why (c):** (a) bare browser → desktop bleed-through on 2 of 3 monitors + on every refresh/crash → violates "no overlap"; (b) Rust/GDI re-creation → not the V2.0 web design, no live wallet → fails "the new design is the screen". **(c) is the only mechanism faithful to the V2.0 web design AND fail-safe to black (never the old desktop) AND flicker-free (single persistent instance).**

**V2 doctrine alignment:** moves the pod default surface to the ratified V2.0 pod-display State #1 (IDLE/WELCOMING) — `project_v2_pod_display_state_channel_premise.md`.

---
### Appendix A — MMA Step-1 DIAGNOSE consensus
4/4 models (deepseek-r1, qwen3-coder, nemotron-3-super, kimi-k2.5; gemini empty) → **Option (c)**. Full risk table + per-model quotes + z-order contract: `MMA-DIAGNOSE-webidle-20260606.md`. Spend $0.0336 → `openrouter-spend-bono.jsonl`. The DIAGNOSE materially hardened the design: Job-Object kill-verify before AC (anti-cheat GPU-child), centered-not-stretched, ready+heartbeat behavioral-verify, the 401/cookie gap, WebView2 version-pinning, and floor-hold-until-game-foreground.
