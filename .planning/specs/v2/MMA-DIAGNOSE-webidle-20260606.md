# MMA Step-1 DIAGNOSE — rc-agent web-idle blank screen (foundational pod-state-channel)

**Date:** 2026-06-06 · **Channel:** OpenRouter (Captain Phase-3 doctrine) · **Spend:** $0.0336 → `openrouter-spend-bono.jsonl` · **Raw:** `/tmp/mma-webidle-results/`
**Models (4/5 OK, 4 vendor families):** deepseek-r1-0528 ✓ · qwen3-coder ✓ · nvidia nemotron-3-super-120b ✓ · moonshot kimi-k2.5 ✓ · google gemini-2.5-pro **EMPTY (0 tok, $0)** — excluded. Threshold ≥3 vendor families: **MET**.

## Q4 — Mechanism choice: **CONSENSUS 4/4 → Option (c)**
Native GDI **black floor spanning the full 7680×1440 virtual desktop** (all 3 Surround monitors), with a **single persistent browser** (Edge `--kiosk` / WebView2) **centered on top** showing the pod-display Idle. Rejected: (a) bare browser → desktop bleed-through on side monitors + on every refresh/crash; (b) native Rust/GDI re-creation → not the V2.0 web design, can't live-mirror wallet, "6-month fidelity effort" (nemotron). **(c) is the only option that is faithful to the web design AND fails safe to black (never the old desktop) AND avoids the v17.0 flicker.**

## Consensus root-cause risks → mitigations (folded into RCA §5)
1. **Multi-monitor bleed-through (CRITICAL, 4/4):** browsers clamp to the primary monitor → desktop on the other 2. → Black GDI floor spans `SM_CXVIRTUALSCREEN × SM_CYVIRTUALSCREEN`; web pane **centered 1920×1080 on the primary, NOT stretched** (stretch → 4:1 distortion + DPI artifacts).
2. **Anti-cheat: TERMINATE, not hide (CRITICAL, 3/4):** a hidden browser HWND stays in the DWM z-order tree (EAC scans it) and the Chromium **GPU child** holds a D3D11/DXGI surface ~30s → overlay flag. → Launch browser in a **Windows Job Object** (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`); on BillingStarted, kill the job and **wait for `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO`** (whole tree incl. GPU child dead) **before** launching AC. Verify `count(msedge)==0`.
3. **State-without-behavioral-verify (HIGH, 4/4):** don't set "idle visible" until the browser confirms render. → Browser JS posts a **"ready"/DOMReady** message to rc-agent (local socket) within ~5s + **heartbeat every ~2s**; 2 missed → paint black floor + restart. (= MTC Q3.)
4. **Session-0 silent failure (CRITICAL/HIGH, 4/4):** assert `ProcessIdToSessionId(GetCurrentProcessId())==1` before spawn; abort+restart if not. (rc-agent already Session-1 via RCWatchdog.)
5. **Flicker on SSE reconnect / startup (CRITICAL/HIGH, 4/4):** single persistent instance; **no kill/relaunch, no `location.reload`**; show the browser only after DOMContentLoaded; black floor covers any reload/white-flash; **debounce** idle/active transitions ~500ms.
6. **401/cookie → white error page (MED, 2/4 — NEW gap):** pod-display is 401 until a staff cookie. → Load `about:blank` first; provision auth (PodDisplayMTLS device cert per SCAFFOLD-NOTES, or injected cookie) **before** navigating; black-floor fallback on 401 (`WebResourceResponseReceived`).
7. **WebView2 Evergreen auto-update crash loop (nemotron top-risk):** pin a **Fixed Version** WebView2 runtime + disable auto-update (or pin Edge); else mid-update file locks → spawn loop.
8. **Black-flash during launch (nemotron Race-2):** keep the black floor visible until the **game window is foreground** (`SetWinEventHook EVENT_SYSTEM_FOREGROUND`), then hide it in the same frame (`DwmFlush`) — no desktop flash between floor-hide and game-show.
9. **Fallback discipline (consensus):** if the browser won't die within timeout → **ABORT the game launch**, hold the full-screen black floor, alert monitoring; never launch AC with a browser possibly in memory (ban risk).

## Z-order contract (consensus)
Idle: `[web pane] > [black floor] > [desktop]` (floor `HWND_BOTTOM` re-asserted on `WM_WINDOWPOSCHANGING`; pane `HWND_TOP` over floor, **not** system `HWND_TOPMOST` — avoids anti-cheat). Game: browser **process-terminated** (job zero) → floor held until game foreground → floor hidden in-frame → `[game] > [floor hidden]`.

## Rollout (consensus)
`web_idle` config flag **default OFF** (zero behavior change) → **pod-8 canary** with screen-capture frame-hash sampling (flicker/bleed detection) + on-screen visual verify → fleet → instant rollback via flag-off + `rc-agent-prev.exe`.
