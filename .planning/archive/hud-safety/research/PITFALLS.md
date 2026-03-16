# Racing HUD & Wheelbase FFB — Implementation Pitfalls

**Scope:** Subsequent milestone — Racing HUD redesign (HUD-01 through HUD-06) and FFB safety system (FFB-01, FFB-02)
**Codebase baseline:** `overlay.rs` (848 lines), `assetto_corsa.rs` (417 lines), `driving_detector.rs` (282 lines), `main.rs` (1416 lines), `ac_launcher.rs` (987 lines)
**Last Updated:** 2026-03-11

---

## How to Read This Document

Each pitfall is formatted as:

```
### P-XX: Title
Observed in / Risk to: <which requirement(s) it threatens>
Warning signs: what you will see when this has gone wrong
Prevention: what to do to avoid it
Phase: which implementation phase must address it (Design / Build / Test / Deploy)
```

Pitfalls are grouped by the five research topics requested. Where a pitfall directly maps to an existing code path in our codebase, the relevant file and line range is cited.

---

## 1. Win32 GDI Overlay Pitfalls

### P-01: GDI Object Leak Per Paint Cycle
**Observed in / Risk to:** HUD-01, HUD-04 (every WM_PAINT creates fonts, pens, brushes)

Win32 GDI maintains a finite per-process handle table (default ~10,000 objects). Every `CreateFontW`, `CreateSolidBrush`, `CreatePen`, `CreateCompatibleBitmap`, or `CreateCompatibleDC` call allocates a handle. If any one is not `DeleteObject`/`DeleteDC`'d before the function returns — including on early-return paths — the handle leaks. At 200ms repaint intervals with 8 objects per paint, a single missed `DeleteObject` exhausts the GDI table in under an hour and causes invisible WM_PAINT silently failing.

**Current exposure (overlay.rs:709-728):** The current paint routine deletes all 8 fonts and the DC/bitmap correctly at the end of `paint_hud()`. However the `font_badge` created at line 698 for the "INV" badge is a mid-function allocation. If we add more conditional mid-function `CreateFontW` calls during the HUD redesign (e.g. per-sector colored fonts, arc drawing brushes), each new branch must explicitly delete its handle before returning or before `BitBlt`.

**Warning signs:**
- GDI handle count in Task Manager (Details → select columns → GDI Objects) climbing steadily
- Overlay stops rendering after 30–60 minutes of continuous use
- `CreateCompatibleBitmap` returns NULL silently (WM_PAINT exits via early return)
- No crash, no log — the overlay just goes blank

**Prevention:**
- Audit every new `Create*` call: pair it with `DeleteObject`/`DeleteDC` unconditionally (do not rely on fall-through — use a cleanup section before `BitBlt`)
- Cache fonts across paints: create font handles once during `WM_CREATE`, store them as fields in the state struct, delete only in `WM_DESTROY`. This eliminates per-paint allocation entirely
- Add a GDI object counter: log `GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS)` every 60 seconds to detect slow leaks early

**Phase:** Build — must be addressed during any font/brush addition

---

### P-02: Flicker from Incorrect Double-Buffer Teardown Order
**Observed in / Risk to:** HUD-01, HUD-04 (layout redesign changes paint order)

The current implementation correctly uses a double-buffer: draw to `mem_dc`, then `BitBlt` to `hdc` at line 722. Flicker occurs when:
1. The old bitmap is selected back *before* `BitBlt` — swapping it prematurely causes `BitBlt` to copy garbage
2. `DeleteObject(mem_bmp)` is called *before* `SelectObject(mem_dc, old_bmp)` — deleting the currently-selected object corrupts the DC
3. Calling `InvalidateRect(hwnd, NULL, TRUE)` instead of `FALSE` — the `TRUE` flag erases the client area with the background brush before WM_PAINT fires, causing a single-frame white flash

**Current exposure (overlay.rs:721-728):** The teardown order is correct (`BitBlt` → `SelectObject` old → `DeleteObject` bmp → `DeleteDC`). Risk increases if the paint routine is restructured during HUD-01 redesign and the teardown sequence is accidentally reordered.

**Warning signs:**
- Visible white or black flash at the HUD position at each repaint
- Flicker worsens at higher game frame rates (because the game is compositing underneath more frequently)
- Flicker disappears if repaint interval is increased (confirms timing, not rendering logic)

**Prevention:**
- Keep the teardown block as a distinct section after all drawing is done — never interleave drawing and cleanup
- `InvalidateRect(hwnd, NULL, FALSE)` — the `FALSE` tells Windows not to erase background (line 385 currently uses FALSE correctly — do not change this)
- Do not call `BeginPaint`/`EndPaint` in a timer handler — only in WM_PAINT. Our current design is correct; don't move paint code into WM_TIMER

**Phase:** Build

---

### P-03: DPI Scaling Causing Blurry Fonts or Misaligned Layout
**Observed in / Risk to:** HUD-01, HUD-04 (larger fonts at specific pixel sizes)

On Windows 11 with high-DPI displays (common on gaming monitors), GDI text is rendered at the physical pixel size requested but positioned using logical coordinates. If the process is DPI-unaware, Windows applies bitmap scaling to the entire window, making all text blurry. If the process is per-monitor DPI-aware but the font sizes in `CreateFontW` are specified as logical units, font metrics are scale-inconsistent between monitors.

The pods use fixed 1920×1080 displays at 100% scaling, but James's machine (RTX 4070) may run at a different DPI for testing. If the overlay is tested on James's machine at 125% DPI and looks fine, it may render with doubled spacing on a 100% pod display.

**Warning signs:**
- Font text appears blurry or slightly smeared on the overlay
- Layout sections misalign horizontally when moving window between monitors
- `GetSystemMetrics(SM_CXSCREEN)` returns logical pixels, not physical — so `bar_w` calculation is wrong on DPI-scaled displays

**Prevention:**
- Declare DPI awareness in the manifest or call `SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` before creating any windows
- Use `GetDpiForWindow(hwnd)` after window creation and scale font sizes: `font_size_px = (logical_size * dpi) / 96`
- For the pods (all 1920×1080 at 100%): DPI is 96, so logical == physical. Confirm this assumption with `GetDpiForWindow` in a startup log line so we detect if a pod ever gets rescaled
- Never hardcode pixel positions — compute all x/y offsets from `w`, `h`, and `GetDpiForWindow`

**Phase:** Design — DPI strategy must be decided before drawing any text at specific sizes

---

### P-04: HWND_TOPMOST Losing to Game Fullscreen Exclusive Mode
**Observed in / Risk to:** HUD-01 through HUD-06 (overlay visibility)

Assetto Corsa in fullscreen exclusive mode (D3D exclusive fullscreen) owns the display adapter entirely. Win32 `WS_EX_TOPMOST` windows cannot appear on top of an exclusive fullscreen D3D application — the game's Present() call unconditionally replaces the framebuffer. Our overlay is only guaranteed to be visible when AC runs in borderless windowed or windowed mode.

The current implementation at overlay.rs line 203 calls `SetWindowPos(HWND_TOPMOST, ...)` periodically from `enforce_topmost()`. This is called from the main loop. However, when AC switches from windowed → fullscreen (which happens if the player hits Alt+Enter or if CSP launches in fullscreen), the overlay disappears and `enforce_topmost` cannot recover it.

**Warning signs:**
- Overlay visible at AC launch, disappears after 2-3 seconds (when AC goes fullscreen)
- Overlay reappears when Alt+Tab out of game
- No error in logs — `SetWindowPos` succeeds but has no visible effect

**Prevention:**
- Confirm AC is configured to launch in borderless windowed mode on all pods — check `video.ini` setting `FULLSCREEN=0` and `BORDERLESS=1`. This is a one-time pod setup check, not a code fix
- Add a startup log line that reads `video.ini` and warns if `FULLSCREEN=1` is detected
- If fullscreen exclusive mode is ever needed: use a layered window with `WS_EX_LAYERED` + DWM composition instead of GDI — but this is out of scope for this milestone
- The current `enforce_topmost()` call interval should remain at ~1s or less to catch any spontaneous window reordering by the OS

**Phase:** Design — fullscreen mode must be locked to borderless-windowed before HUD implementation starts

---

### P-05: `CS_HREDRAW | CS_VREDRAW` Causing Full Repaint on Any Resize
**Observed in / Risk to:** HUD-01 (window geometry changes during redesign)

The current WNDCLASSEX uses `CS_HREDRAW | CS_VREDRAW` (overlay.rs line 287). These flags force a full WM_PAINT on any horizontal or vertical resize. For a static 96px-tall bar this is irrelevant. However, if the HUD redesign introduces a taller window (e.g. a centered gear display at 200px height), and the window is ever resized — even programmatically — the resulting double-repaint produces a single-frame flash.

More critically: if `CreateWindowExW` is called with `WS_THICKFRAME` or `WS_SIZEBOX` by mistake (e.g. when copying a window style template), users can drag-resize the overlay, causing continuous repaints and GDI stress.

**Warning signs:**
- Brief flicker when the overlay window first appears
- CPU usage spikes during rapid resize operations

**Prevention:**
- Keep `WS_POPUP | WS_VISIBLE` only — no resize chrome (current implementation is correct)
- Remove `CS_HREDRAW | CS_VREDRAW` if the window size is truly fixed — replace with `0` to suppress resize repaints. For a fixed-geometry HUD overlay, these flags add no value
- Set window size once in `CreateWindowExW` and never call `SetWindowPos` with new dimensions after that

**Phase:** Build

---

### P-06: Font Face Fallback Silently Using Wrong Typeface
**Observed in / Risk to:** HUD-01 (Montserrat/Enthocentric brand fonts)

`CreateFontW` accepts a face name but silently falls back to a system default if the requested font is not installed. If Montserrat or Enthocentric (Racing Point brand fonts) are not installed on the pod, GDI will render in a generic sans-serif without any error or warning. The overlay will still appear functional but with wrong typography.

**Warning signs:**
- HUD text does not match brand typography in screenshots
- No log output about font missing
- Font looks like Arial or Tahoma instead of Montserrat

**Prevention:**
- At startup, call `EnumFontFamiliesEx` to verify required font faces are installed. Log a warning if not found
- For the HUD redesign: "Segoe UI" (current default) is always present on Windows 11 and is acceptable fallback. Do not depend on Montserrat/Enthocentric being installed on pods unless they are pre-installed in the pod image
- If brand fonts are required: bundle them with the deploy kit and install via the Windows Font API (`AddFontResourceEx`) at rc-agent startup, remove with `RemoveFontResourceEx` on shutdown

**Phase:** Design

---

## 2. HID FFB Pitfalls

### P-07: Wrong Output Report Format Causing Silent Device Malfunction
**Observed in / Risk to:** FFB-01, FFB-02 (zero-force HID write)

The OpenFFBoard USB HID FFB protocol uses the USB HID Physical Interface Device (PID) class — specifically the `SET_EFFECT`, `SET_ENVELOPE`, `SET_CONDITION`, `DEVICE_CONTROL` reports defined in the USB PID spec. The report IDs and field layouts are defined by the HID report descriptor embedded in the device firmware. If you write a raw byte buffer with incorrect report ID or field ordering, the device either ignores it silently or enters an undefined state. A wrong `DEVICE_CONTROL` opcode (e.g. opcode 6 = "device reset" vs opcode 1 = "enable actuators") sent as a "zero force" command is not recoverable without power cycling.

The Conspit Ares uses OpenFFBoard firmware. Different firmware versions (pre-1.10, 1.10.x, 1.11+) have different FFB report implementations. The OpenFFBoard project has changed its HID descriptor multiple times. A zero-force report valid for firmware 1.10 may not apply correctly on 1.11 because the effect instance mapping changed.

**Warning signs:**
- Wheelbase does not respond to HID write (silent ignore)
- Wheelbase jerks or spins in one direction after write (wrong effect applied)
- `hidapi::write()` returns `Ok(report_size)` even when device ignores the payload
- Device LED changes color or blinks unexpectedly after write

**Prevention:**
- Before implementing FFB write: confirm the exact firmware version on the Conspit Ares units using OpenFFBoard's USB configuration tool. Note the version in the codebase as a comment
- Do not hand-craft raw FFB PID reports. Use the `DEVICE_CONTROL` report with opcode `DC_DISABLE_ACTUATORS` (opcode 2) — this is the safest "zero force" command in the USB PID class and is firmware-version stable because it is part of the USB spec, not an OpenFFBoard extension
- Alternatively, use the `SET_EFFECT` report to create a constant-force effect with magnitude 0 — safer than disabling actuators because it leaves the FFB subsystem in a known state
- Prototype the write on a single pod (Pod 8) and verify with a force gauge or by holding the wheel by hand while the command is sent
- Read the HID report descriptor from the device at startup: `HidDevice::get_report_descriptor()` — parse it to confirm expected report IDs before writing

**Phase:** Build — must prototype and verify on hardware before integrating into cleanup path

---

### P-08: Blocking HID Write on the Main Async Thread
**Observed in / Risk to:** FFB-01, FFB-02 (zero-force must complete before taskkill)

`hidapi::HidDevice::write()` is a synchronous blocking call. The rc-agent main loop is Tokio async. Calling `write()` directly from an async context without `spawn_blocking` blocks the Tokio executor thread, potentially stalling the entire event loop for the duration of the write (typically 1-50ms depending on USB host controller).

More critically: if the device is disconnected when `write()` is called, `hidapi` on Windows via the Win32 `WriteFile` API can block for up to 5 seconds on a stale handle before returning an error.

**Warning signs:**
- rc-agent event loop appears to stall briefly during session end
- `BillingStopped` WebSocket message is processed late (observable as delayed lock screen appearance)
- Watchdog kills rc-agent thinking it's hung, racing with the FFB write

**Prevention:**
- Wrap the FFB write in `tokio::task::spawn_blocking`: `spawn_blocking(move || device.write(&report)).await`
- Set a timeout on the spawn_blocking future: `tokio::time::timeout(Duration::from_millis(100), spawn_blocking(...)).await`. If the write doesn't complete in 100ms, log an error and proceed to taskkill anyway
- Acquire the HID device handle before the write (it's already open for reading in the HID monitor task). Share the device handle via `Arc<Mutex<HidDevice>>` between the read loop and the write path — but see P-09 for the race condition this introduces

**Phase:** Build

---

### P-09: Race Condition Between HID Read Loop and FFB Write
**Observed in / Risk to:** FFB-01 (zero-force sent while read is active)

The current HID monitor (`main.rs:1253-1331`) holds the `HidDevice` handle exclusively in a loop calling `dev.read_timeout(&mut buf, 10)`. If we introduce a FFB write path that also needs the device handle, we have a race: the read loop holds the handle, and the write must wait for it — or we get concurrent access on a non-thread-safe type.

`hidapi::HidDevice` does not implement `Sync`. Accessing it from two threads simultaneously on Windows via `WriteFile` + `ReadFile` can work (USB pipes are separate) but `hidapi`'s Rust binding does not expose separate read/write handles — it wraps a single `HANDLE`.

**Warning signs:**
- `hidapi` panics or returns `INVALID_HANDLE` during write while read is active
- Write appears to succeed (returns byte count) but wheelbase FFB state does not change

**Prevention:**
- Design option A (recommended): `Arc<Mutex<HidDevice>>`. The read loop acquires and releases the lock per read call (10ms intervals). The FFB write acquires the lock, writes, releases. Contention is 10ms maximum. The Mutex ensures serial access
- Design option B: Use two separate `hidapi::HidApi::open()` calls — one for reading, one for writing. On Windows, USB HID devices support multiple open handles. Verify this works with the Conspit/OpenFFBoard device before relying on it
- Design option C: Send the zero-force through the Conspit Link application's configuration API (if it exposes one) instead of directly via HID — eliminates the handle sharing problem entirely
- Whichever option is chosen: test by sending a write while the read loop is active under load (driving in-game). If the write is dropped, the Mutex approach is needed

**Phase:** Design — handle ownership must be decided before writing any FFB code

---

### P-10: OpenFFBoard Firmware Version Differences in FFB Report Layout
**Observed in / Risk to:** FFB-01, FFB-02

OpenFFBoard has made breaking changes to its HID descriptor in multiple releases. In pre-1.10 firmware, the constant-force effect used report ID 1. In 1.10+, the USB PID class was partially reimplemented with different report IDs and different actuator mapping. The Conspit Ares ships with a vendor-customized OpenFFBoard firmware — the version may differ from the upstream OpenFFBoard release.

**Warning signs:**
- FFB write succeeds (hidapi returns expected byte count) but device behavior is unchanged
- Different pods respond differently to the same write (if pods have different firmware installed)

**Prevention:**
- Run `hidapi.get_manufacturer_string()` and `hidapi.get_product_string()` at startup and log both — these may include firmware version
- Compare the HID report descriptors across pods using `get_report_descriptor()` — if they differ, you have firmware version heterogeneity
- If firmware versions differ across pods: implement a firmware-version dispatch table with separate report builders per version
- Coordinate with Conspit to get the exact firmware version and HID descriptor for the Ares 8Nm units delivered to Racing Point

**Phase:** Build — before implementing write, enumerate and document firmware versions on all 8 pods

---

### P-11: Device Disconnect During FFB Write at Session End
**Observed in / Risk to:** FFB-01, FFB-02

Session end involves: (1) zero-force FFB write, (2) `taskkill /IM acs.exe /F`. AC exit can cause USB re-enumeration on some systems because AC's DirectInput exclusive mode is released — triggering a brief USB disconnect/reconnect cycle on the wheelbase. If the FFB write coincides with this re-enumeration, the write fails with an error that looks like a device disconnect.

A worse scenario: `enforce_safe_state()` calls `taskkill` immediately. If FFB write is in flight when the AC process dies, AC's DirectInput handle release causes the wheelbase to snap to its last force state — which could be full-lock from the last racing moment.

**Warning signs:**
- HID write error in logs at session end: "device not found" or "write failed"
- Wheelbase snaps to a non-zero position after session ends
- Error occurs intermittently (only when AC was in an active FFB effect at shutdown)

**Prevention:**
- Send the zero-force write *before* any taskkill call with a hard guarantee: `ffb_zero().await?; kill_game();`. This is the ordering already planned in PROJECT.md (FFB-01 requirement: "before killing game process")
- Add a 50ms sleep after the FFB write before the taskkill to allow the write to complete at the USB protocol level before the game's DirectInput context is destroyed
- If the write fails (device error), log and proceed with taskkill — do not abort the session cleanup because of an FFB write failure
- In `enforce_safe_state()` (ac_launcher.rs:956): add FFB zero as step 0 before game process kill

**Phase:** Build

---

## 3. Racing HUD Data Accuracy Pitfalls

### P-12: AC `lastSectorTime` Is Per-Sector, Not Cumulative — But Only Available at the Transition Moment
**Observed in / Risk to:** HUD-02 (sector times), HUD-03 (lap times)

AC's `lastSectorTime` (graphics offset 168) contains the time for the sector *that just completed* at the moment of the `currentSectorIndex` transition. This field is **not** continuously valid — it holds the previous sector's time only during the frame when `currentSectorIndex` increments. On the next read (potentially 100ms later if we're polling at 10Hz), it may already be overwritten by a new value or zeroed.

The current implementation (assetto_corsa.rs:263-268) correctly detects the transition by comparing `current_sector != self.last_sector_index`. However, if the read_telemetry loop misses a transition (e.g. the adapter is disconnected and reconnected, or the polling interval is too slow), the sector time is permanently lost.

The confusion is compounded because the AC Python reference implementation (`sim_info.py`) reads this field continuously — Python's ctypes just re-reads the struct every 10ms. Our polling at 10Hz (100ms intervals) could theoretically miss a sub-100ms sector transition window, though in practice AC holds `lastSectorTime` stable for at least one full polling cycle.

**Warning signs:**
- S1, S2, or S3 showing `--.-` after the sector visually completed on-track
- Sector time appears for one frame then disappears (polling captured the transition but then cleared it)
- Wrong sector time (from a different sector) stored — happens if two transitions occur between polls

**Prevention:**
- Increase polling frequency for the graphics shared memory to 50ms (20Hz) during active sessions. Physics already updates per-frame but graphics is ~10Hz — 50ms poll is still within AC's graphics update window
- Store `last_sector_time` from the previous poll and compare: if `current_sector == last_sector_index` but `last_sector_time != previous_last_sector_time`, a sector time update occurred within the same sector index — this is AC updating `lastSectorTime` after a pit stop sector, handle it explicitly
- On adapter reconnect (after a disconnect mid-session), reset `last_sector_index = -1` and `sector_times = [None; 3]` — already done at connect(), but verify reconnect path also resets these

**Phase:** Build

---

### P-13: `iCurrentTime` Resets to 0 at Lap Boundary — HUD Shows 0ms Briefly
**Observed in / Risk to:** HUD-03 (current lap timer)

AC's `iCurrentTime` (graphics offset 140) tracks the in-progress lap timer in milliseconds. At the exact moment a lap completes, AC resets this field to 0 before incrementing `completedLaps`. Our polling reads both fields in the same `read_telemetry()` call, but because the shared memory is updated by AC asynchronously, we may read `iCurrentTime = 0` and `completedLaps` still showing the old count. This produces a single-frame display of "0:00.000" on the HUD before the lap counter increments.

**Warning signs:**
- Brief flash of `0:00.000` in the current lap timer at the finish line
- Lap counter and lap timer briefly out of sync

**Prevention:**
- In `update_telemetry()` (overlay.rs:144), guard against `lap_time_ms == 0` when `current_lap_number > 0`: if the previous frame had a non-zero lap time and the current frame reads 0, treat it as the lap boundary transition and hold the previous value for one frame
- Alternatively: when `completedLaps` increments, do not update `current_lap_time_ms` from `iCurrentTime` for 2 poll cycles — wait for `iCurrentTime` to rise above 0 before pushing it to the HUD

**Phase:** Build

---

### P-14: Off-by-One on Lap Count at Session Start
**Observed in / Risk to:** HUD-06 (lap counter), HUD-03 (lap times)

AC initializes `completedLaps` to 0. On the first lap, the driver crosses the start/finish line and `completedLaps` increments to 1, recording the first lap time. However, AC also briefly shows `completedLaps = 0` at race start even before the first lap is driven. The current code sets `self.last_lap_count = 0` at connect and only records a lap when `completed_laps > self.last_lap_count && self.last_lap_count > 0` (assetto_corsa.rs:276).

The `&& self.last_lap_count > 0` guard prevents recording a "lap" when AC first initializes with `completedLaps = 1` on session join (some server configs pre-set this to 1). But if the driver genuinely completes their first lap from 0→1, the guard `last_lap_count > 0` would block it.

**Warning signs:**
- First lap of a session is never recorded in the database
- Lap counter shows LAP 1 when driver is on their second physical lap
- `completedLaps` jumps from 0 to 2 if the driver completes the formation lap (some tracks have a pre-start lap)

**Prevention:**
- Remove the `&& self.last_lap_count > 0` guard — it incorrectly suppresses the first lap
- Instead, guard against invalid lap times: if `last_lap_time_ms < 10_000` (less than 10 seconds, clearly not a real lap), skip recording but still update `last_lap_count`
- Set `self.last_lap_count` to `completedLaps` at connect time (not 0), so the first increment from the current state is treated as a genuine lap completion

**Phase:** Build

---

### P-15: Sector Color Logic Incorrect on First Lap (No Previous Lap)
**Observed in / Risk to:** HUD-02 (F1-style sector color coding)

The current `sector_color()` function (overlay.rs:814-847) handles the case where `prev_ms.is_none() && best_ms.is_none()` by returning purple ("it IS the best by definition"). This is logically correct for the very first lap ever driven. However, there is a subtle bug: on the *second* lap, `best_ms` is set from the first lap, but the second lap's S1 time is being compared against `best_ms` which is the *entire best lap's S1*, not the best S1 across all laps separately.

More critically: the function receives `prev_ms` as the *previous lap's sector time*, but during a session, `data.previous_lap` is only updated when a lap completes — not when a sector completes. So mid-lap on the second lap, S1 has completed but `data.previous_lap` is still None (no lap has completed yet in the session). This causes S1 to show purple even if it is slower than the driver's historical best.

**Warning signs:**
- All sectors show purple on the first lap of every session (correct)
- S1 always shows purple on the second lap even when it is a slow sector (incorrect)

**Prevention:**
- Maintain per-sector bests separately from per-lap bests: `best_sector1_ms: Option<u32>`, `best_sector2_ms: Option<u32>`, `best_sector3_ms: Option<u32>` tracked independently in `OverlayData`
- Update per-sector bests whenever a sector completes (not just when a lap completes)
- Pass per-sector best to `sector_color()` instead of pulling from `best_lap.sectorN_ms`

**Phase:** Build

---

### P-16: Lap Time Drift — `iCurrentTime` vs Real Elapsed Time
**Observed in / Risk to:** HUD-03 (lap timer), HUD-05 (session time sync)

AC's `iCurrentTime` is driven by AC's internal simulation clock, which can drift from wall-clock time if:
- AC is paused (ESC menu): `iCurrentTime` stops advancing, wall-clock does not
- AC is in replay mode: `iCurrentTime` may advance at variable speed
- AC's physics rate drops below real-time (CPU/GPU overload): `iCurrentTime` may fall behind

HUD-05 specifically calls for syncing the HUD timer with `iSessionTime` rather than billing countdown. If we use `iCurrentTime` for the lap timer and `iSessionTime` for the session timer, these clocks can diverge when AC stutters.

**Warning signs:**
- Lap timer on HUD shows 1:23.500 but the in-game timer shows 1:24.1
- Session timer drifts from expected remaining time after game stutter
- `iCurrentTime` returns negative values briefly during AC menu transitions (menu time is subtracted from lap time on some AC versions)

**Prevention:**
- Use `iCurrentTime` exclusively for lap timing — it is the ground truth that matches AC's lap recording
- For session time (HUD-05), use `sessionTimeLeft` (f32, offset 152) from the graphics struct, which is always in sync with AC's session clock
- Guard against negative `iCurrentTime`: `lap_time_ms = if raw > 0 { raw as u32 } else { 0 }` — already done implicitly via `as u32` cast but explicit guard is clearer
- Never attempt to compute session time by subtracting lap times — use `sessionTimeLeft` directly

**Phase:** Build

---

### P-17: Game Restart During Session Resets Telemetry State Without Notifying rc-agent
**Observed in / Risk to:** HUD-02, HUD-03, HUD-06 (all lap/sector/timer data)

If a customer restarts the race (via AC menu → Restart Race) during an active billing session, AC resets `completedLaps` to 0 and `iCurrentTime` to 0. From rc-agent's perspective, `completedLaps` drops from (say) 3 back to 0. The current code does not handle a decrease in `completedLaps` — `last_lap_count` remains 3, and the next lap completion increments to 1, which is not `> self.last_lap_count (3)`, so laps 1-3 of the new race are silently skipped.

The HUD would continue showing `LAP 4` (from `completed_laps = 1`, which equals `lap_number: completed_laps` in TelemetryFrame) but the overlay's `best_lap` retains times from the previous race stint — causing cross-race pollution of sector colors.

**Warning signs:**
- After a race restart: lap counter jumps to an incorrect number
- Best lap and sector colors reference times from the previous stint
- Laps from the restarted race are not recorded in the database

**Prevention:**
- Detect `completedLaps` decreasing: `if completed_laps < self.last_lap_count { self.handle_session_reset(); }`
- `handle_session_reset()`: reset `last_lap_count`, `sector_times = [None; 3]`, `last_sector_index = -1`
- In `overlay.rs`: clear `previous_lap`, `best_lap`, and all `live_sector*_ms` when a session reset is detected — expose a `reset_lap_state()` method on `OverlayManager`
- Emit a `SessionReset` agent message to racecontrol so billing logic can handle it (lap records from the restarted stint should not be merged with the old stint)

**Phase:** Build

---

## 4. Safety System Pitfalls

### P-18: FFB Cleanup Skipped When Process Is Killed Before Cleanup Runs
**Observed in / Risk to:** FFB-01, FFB-02

The current session end flow (`main.rs:751-782`) receives `SessionEnded` from racecontrol via WebSocket, calls `overlay.deactivate()`, then `game.stop()`, then `enforce_safe_state()` via `spawn_blocking`. The FFB zero-force write (not yet implemented) would need to occur *before* `game.stop()` / `taskkill`.

Risk: if rc-agent is killed by the watchdog (e.g., pod-agent detects rc-agent crash and restarts it) at the moment between billing end and the FFB write, the FFB write never executes. The game keeps running with its last FFB state, and the next rc-agent instance starts fresh with no knowledge of the pending FFB write.

Similarly: if racecontrol loses the WebSocket connection to rc-agent (network blip) and force-ends the billing on the racecontrol side, rc-agent may never receive `SessionEnded` — and thus never sends the FFB zero command.

**Warning signs:**
- Wheelbase has force applied after session should have ended
- rc-agent crash log shows no FFB write in the log before process exit
- Session ends on racecontrol side but pod still has game running with active FFB

**Prevention:**
- Implement FFB zero as a synchronous operation before any process kill — in `enforce_safe_state()` (ac_launcher.rs:956), add an FFB write step as the very first action, before taskkill
- This means FFB zero must work even from a fresh restart of rc-agent (i.e., it cannot depend on the HID device handle that was open in the previous session)
- On rc-agent startup: always send FFB zero as part of initialization, regardless of whether a session is active. This recovers from the "killed mid-cleanup" scenario
- Register a Windows console control handler (`SetConsoleCtrlHandler`) to catch CTRL_CLOSE and `TerminateProcess` signals and attempt FFB zero before exit — this is best-effort since `TerminateProcess` cannot be intercepted, but `CTRL_BREAK` can

**Phase:** Design — the startup-zero behavior must be designed before the write path is built

---

### P-19: Async Cleanup Timeout Race with Watchdog
**Observed in / Risk to:** FFB-01, FFB-02

The watchdog in rc-agent (main.rs:1388-1414) uses `tasklist` to check if `pod-agent.exe` and `ConspitLink2.0.exe` are running, and restarts them if not. The watchdog has a 10-second check interval based on memory notes. If the FFB cleanup path takes >10 seconds (e.g. HID write blocks, or there are multiple cleanup steps), the watchdog could restart a dependent process while cleanup is in progress, causing undefined state.

Specifically: if the FFB write blocks for 5 seconds (USB timeout), and the watchdog fires at 3 seconds and restarts ConspitLink2.0, ConspitLink may re-acquire the HID device handle just as our write is completing — causing our write to fail with "device busy."

**Warning signs:**
- ConspitLink restarts unexpectedly during session cleanup
- FFB write error: "device already opened" or "access denied"
- Cleanup log shows correct sequence, but FFB state on device is wrong

**Prevention:**
- FFB zero write must complete in <200ms. If the write takes longer than 200ms, something is wrong (device disconnect, USB issue) — abort the write and proceed with cleanup
- Set the HID write timeout aggressively: `hidapi::write()` on Windows calls `WriteFile` with an overlapped I/O structure — configure USB write to complete in one USB frame (1ms) or fail
- Coordinate watchdog with cleanup state: set an atomic flag `cleanup_in_progress` at session end; watchdog skips ConspitLink restart check while this flag is set; clear flag after cleanup completes

**Phase:** Build

---

### P-20: ConspitLink Interfering with Direct HID Writes
**Observed in / Risk to:** FFB-01, FFB-02

ConspitLink2.0 is the Conspit-provided wheelbase driver/configuration application. It maintains its own open HID handle to the wheelbase for configuration and monitoring. On Windows, a HID device can be opened by multiple processes simultaneously — but only one process should be writing FFB output reports. If both ConspitLink and rc-agent write FFB output reports concurrently, the device receives interleaved FFB commands with undefined behavior.

ConspitLink may also maintain an FFB "keep-alive" — periodically writing a heartbeat effect to prevent the device's timeout-safe-mode from engaging. If rc-agent sends a `DEVICE_CONTROL: DC_DISABLE_ACTUATORS` and ConspitLink immediately sends a "re-enable" keep-alive, the zero-force command is undone.

**Warning signs:**
- FFB zero write succeeds but wheelbase immediately returns to force within 100ms
- ConspitLink log shows "device re-enabled" or "effect restarted" after rc-agent write
- Wheelbase FFB stops working entirely (ConspitLink and rc-agent writes conflict and corrupt device state)

**Prevention:**
- Do not send `DC_DISABLE_ACTUATORS` if ConspitLink is managing the device — use a `SET_EFFECT` with magnitude 0 instead. This creates a zero-force constant effect that overrides any existing effects without disabling the device globally
- Alternative: close ConspitLink during the FFB write window. This is consistent with the established pattern of minimizing ConspitLink during sessions (ac_launcher.rs uses `minimize_conspit_window()`). Add a `close_conspit_link_briefly()` function: close it, send FFB zero, wait 200ms, reopen it
- Research whether OpenFFBoard's USB PID implementation respects effect priority — if a "host application" effect with higher priority can be created by rc-agent that overrides ConspitLink's effects

**Phase:** Design — the ConspitLink interaction must be resolved before FFB write is implemented

---

### P-21: No FFB Zero on Unexpected Process Exit (SIGKILL Equivalent)
**Observed in / Risk to:** FFB-02 (crash/unexpected exit scenario)

When `taskkill /F` kills rc-agent, Windows terminates the process immediately — no Rust `Drop` implementations run, no cleanup code executes. The HID device handle is closed by Windows (releasing the file handle), but this does not send any FFB output report to the device. The device retains its last active FFB state.

This is the critical gap: `enforce_safe_state()` calls taskkill on game processes, but if rc-agent itself crashes or is killed, there is no executor left to send the FFB zero.

**Warning signs:**
- rc-agent crash in logs, no "FFB zero sent" log line before crash
- Wheelbase has force applied minutes after game ended

**Prevention:**
- The only reliable defense is initializing FFB zero at rc-agent startup (P-18 prevention, item 3). Every rc-agent boot sends FFB zero unconditionally — this recovers from the previous-instance crash scenario within the watchdog restart window (typically 10 seconds)
- Consider a small standalone watchdog companion: a separate minimal executable (`ffb-guard.exe`) that runs independently, watches for rc-agent to exit unexpectedly, and immediately sends FFB zero. This is complex but provides true crash-time protection
- For this milestone: implement startup-zero only. Document that the gap between crash and next rc-agent startup (watchdog interval) is the risk window, and accept it as a known limitation

**Phase:** Design — scope decision needed from Uday: startup-zero only (simpler) or ffb-guard.exe (comprehensive)

---

## 5. AC Shared Memory Pitfalls

### P-22: Stale Shared Memory After Game Exit — Reading Garbage Values
**Observed in / Risk to:** HUD-02, HUD-03, HUD-05, HUD-06 (all AC telemetry)

AC's shared memory files (`acpmf_physics`, `acpmf_graphics`, `acpmf_static`) are created by `acs.exe` when it starts and are not explicitly destroyed when it exits. The memory-mapped region persists in the Windows kernel as long as any process has an open handle to it. This means:

After `taskkill /IM acs.exe /F`, the shared memory still exists and is still accessible. Our `read_telemetry()` will continue returning "valid" data — but the data is frozen at AC's last write before shutdown. `iCurrentTime` is frozen, `completedLaps` is frozen, and the `STATUS` field (offset 4) may still show `AC_STATUS = 2` (LIVE) even though the game is dead.

If the HUD is still active when this stale data is read, it shows a frozen lap timer and frozen sector times — confusing the customer and potentially recording duplicate "laps" if our code misinterprets the frozen data.

**Warning signs:**
- HUD continues to show data (including lap timer) after game has been killed
- `disconnect()` not called, adapter still `connected = true` after taskkill
- Same lap time recorded multiple times in the database

**Prevention:**
- Check the `STATUS` field on every `read_telemetry()` call. If `STATUS == 0` (AC_OFF), treat it as "game exited" and return `Ok(None)` — which triggers the "no telemetry" path in main.rs
- In `ac_launcher.rs` cleanup functions: call `adapter.disconnect()` *before* taskkill, then call taskkill. The disconnect unmaps the view (`UnmapViewOfFile`) and closes the handle. After that, even if the shared memory file still exists, our code no longer reads it (current `enforce_safe_state()` does not disconnect the adapter — this is a gap)
- Add an `ac_is_running()` helper that checks the STATUS field: `STATUS == 0` means AC exited. Call this check at the top of `read_telemetry()` and return early if AC is not live

**Phase:** Build

---

### P-23: Shared Memory Not Unmapped Causing Access Violation After Process Exit
**Observed in / Risk to:** Reliability (rc-agent stability after session end)

The `ShmHandle` struct in `assetto_corsa.rs` wraps a raw pointer (`ptr: *const u8`). The `disconnect()` method calls `UnmapViewOfFile(h.ptr)` and `CloseHandle(h._handle)`. If `disconnect()` is not called before `AssettoCorsaAdapter` is dropped (e.g. in an early return or panic path), the handles leak and the raw pointer becomes a dangling reference.

More critically: if `read_telemetry()` is called concurrently from another thread while `disconnect()` is executing on the main thread (a race condition), `read_telemetry()` may dereference a pointer to already-unmapped memory — causing an access violation (Windows: STATUS_ACCESS_VIOLATION), which on Windows in Rust with `#[allow(unsafe_op_in_unsafe_fn)]` will terminate the process without unwinding.

**Warning signs:**
- rc-agent crashes with no Rust panic message (access violation = process abort, not panic)
- Crash occurs during session end cleanup
- Memory-mapped handle count in Process Explorer stays high after sessions

**Prevention:**
- Implement `Drop` for `ShmHandle` that calls `UnmapViewOfFile` + `CloseHandle`. This ensures the handles are released even if `disconnect()` is never called:
  ```rust
  #[cfg(windows)]
  impl Drop for ShmHandle {
      fn drop(&mut self) {
          unsafe {
              winapi::um::memoryapi::UnmapViewOfFile(self.ptr as *const _);
              winapi::um::handleapi::CloseHandle(self._handle);
          }
      }
  }
  ```
- Mark `read_telemetry()` as non-concurrent: it is already called from a single-threaded loop in the sim adapter task, but add a comment documenting this assumption
- Always call `adapter.disconnect()` before session cleanup in `enforce_safe_state()` and `cleanup_after_session()`

**Phase:** Build — `Drop` impl should be added as part of the milestone, not deferred

---

### P-24: `lastSectorTime` Is Per-Sector, Not a Running Total (AC Documentation Confusion)
**Observed in / Risk to:** HUD-02 (sector time display accuracy)

The AC shared memory documentation (and some community implementations) is ambiguous about whether `lastSectorTime` contains the sector time for the just-completed sector or whether it is a cumulative time up to the end of that sector. Some implementations interpret it as cumulative (adding S1+S2 to get S2's split) while others interpret it as per-sector (each sector's individual time).

Based on AC's Python reference implementation (`sim_info.py`), `lastSectorTime` is the time for the *just-completed sector only* — not cumulative. Our current implementation treats it as per-sector (assetto_corsa.rs:267), which is correct. However, the comment on line 12 says "ms for the sector just completed" — this should be verified against an actual AC session before shipping.

**Warning signs:**
- S2 time appears larger than expected (would indicate cumulative interpretation being used)
- Sum of S1 + S2 + S3 does not equal the lap time (would indicate per-sector is correct)

**Prevention:**
- Add a validation check in `poll_lap_completed()`: verify that `sector1_ms + sector2_ms + sector3_ms ≈ lap_time_ms` (within 100ms tolerance for AC's own rounding). Log a warning if the sum is off by more than 200ms
- Document the confirmed interpretation ("per-sector, not cumulative") with a specific test track and lap time that was verified

**Phase:** Test — validate with a real AC session on a track with 3 sectors

---

### P-25: `isValidLap` Offset Is Approximate — May Read Wrong Field
**Observed in / Risk to:** HUD-02 (sector colors), lap recording accuracy

The comment at assetto_corsa.rs line 87-89 explicitly states: `IS_VALID_LAP: usize = 180 — approximate — may need correction`. The actual offset for `isValidLap` in the AC graphics struct is deeper in the struct (community reports suggest it is at approximately offset 1408+ in the full SPageFileGraphic with Unicode strings). Offset 180 may read a different field — possibly part of the tyre compound wchar string (which starts at offset 176 per the struct layout).

If offset 180 reads garbage from the tyreCompound wchar, our validity check `is_valid != 0` is always true (since non-null wchar bytes are non-zero), meaning every lap is recorded as valid regardless of cut penalties.

**Warning signs:**
- Laps with track limit violations recorded as valid
- `is_valid` never reads 0 in logs, regardless of whether cuts occurred

**Prevention:**
- Verify the `IS_VALID_LAP` offset using a Python script (`sim_info.py`) that reads the struct with the `ctypes` binding — this gives the definitive offset from the actual SPageFileGraphic C struct
- If offset 180 is wrong: find the correct offset using `ctypes.offsetof(SPageFileGraphic, 'isValidLap')` in Python
- Until verified, treat `isValidLap` as unreliable and consider reading from `lastSectorTime[2]` (S3 completion) as a proxy for lap validity — if S3 time is present and reasonable, the lap is likely valid
- Add a TODO comment and a test that verifies validity behavior for a known cut lap

**Phase:** Test — must be verified against real AC data before HUD sector colors are considered production-ready

---

## Pitfall Cross-Reference by Requirement

| Requirement | Pitfalls to Address |
|-------------|---------------------|
| HUD-01 (layout redesign) | P-01, P-02, P-03, P-04, P-05, P-06 |
| HUD-02 (sector times, colors) | P-12, P-15, P-24, P-25 |
| HUD-03 (lap times) | P-13, P-14, P-16, P-22 |
| HUD-04 (RPM/font size) | P-01, P-03 |
| HUD-05 (session time sync) | P-16 |
| HUD-06 (lap counter) | P-14, P-17 |
| FFB-01 (zero-force on session end) | P-07, P-08, P-09, P-10, P-11, P-18, P-20 |
| FFB-02 (zero-force on crash/exit) | P-18, P-19, P-21 |
| Reliability (all) | P-22, P-23 |

---

## Pitfall Priority Summary

### Must-fix Before Any FFB Write Code Is Written (Design Phase)
- **P-07** — Verify exact report format/firmware version for zero-force command
- **P-09** — Decide HID handle ownership architecture (read vs write concurrency)
- **P-18** — Startup-zero behavior designed and agreed
- **P-20** — ConspitLink coexistence strategy decided
- **P-04** — Confirm pods are locked to borderless-windowed AC mode

### Must-fix During Build (Cannot Ship Without)
- **P-01** — GDI font handle caching (no per-paint allocation)
- **P-08** — spawn_blocking for HID write with timeout
- **P-11** — FFB write before taskkill ordering
- **P-12** — Sector time polling frequency (50ms minimum)
- **P-17** — Session reset detection (`completedLaps` decrease)
- **P-22** — STATUS field check in read_telemetry
- **P-23** — Drop impl for ShmHandle

### Verify During Test on Real Hardware
- **P-14** — Off-by-one on lap count (first lap)
- **P-24** — Confirm per-sector vs cumulative interpretation with sum check
- **P-25** — Verify IS_VALID_LAP offset with Python ctypes

### Accept as Known Limitation for This Milestone
- **P-21** — FFB zero on SIGKILL (startup-zero is sufficient for now; ffb-guard.exe deferred)
- **P-13** — Momentary 0ms lap time at boundary (cosmetic, 200ms window)

---

*Generated: 2026-03-11*
*Author: James Vowles (Research phase for Racing HUD & FFB safety milestone)*
