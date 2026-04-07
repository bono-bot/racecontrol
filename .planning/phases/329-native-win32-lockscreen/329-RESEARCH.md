# Phase 329: Native Win32 Lock Screen - Research

**Researched:** 2026-04-07
**Domain:** Win32 GUI / GDI text rendering / Rust FFI / multi-monitor window management
**Confidence:** HIGH

## Summary

The current lock screen implementation in `lock_screen.rs` (3169 lines) uses Microsoft Edge in `--app` mode with a local HTTP server on port 18923 serving rendered HTML. This causes ~300MB RAM overhead, Edge session restore bugs (SNSS files), Startup Boost process counting confusion, `Stdio::null` spawn failures, and complex crash recovery logic. The entire file is dominated by Edge lifecycle management, HTML template generation, and browser workarounds.

The project already has a **proven native Win32 GDI rendering pattern** in `overlay.rs` (1370+ lines) that creates a `WS_POPUP | WS_EX_TOPMOST` window with `winapi` crate, runs a dedicated Win32 message loop thread, uses `CreateFontW` + `DrawTextW` + `FillRect` for all rendering, and repaints via `WM_TIMER` at 200ms intervals. This overlay uses ~2-5MB RAM and has zero browser dependencies.

**Primary recommendation:** Replicate the `overlay.rs` Win32 window pattern for the lock screen. Use the existing `winapi` crate (0.3.9, already in Cargo.toml) for window management and GDI rendering. Use `CreateFontW` with Montserrat (embedded TTF via `AddFontResourceEx`) for brand-compliant text. Do NOT introduce new GUI frameworks (egui, slint, iced, winit) -- they add 5-30MB of dependencies and complexity for a problem already solved in this codebase.

## Project Constraints (from CLAUDE.md)

### Locked Decisions (from project CLAUDE.md standing rules)
- Static CRT linking: `.cargo/config.toml` has `+crt-static` -- no vcruntime dependency on pods
- rc-agent MUST run in Session 1 (interactive desktop) -- GUI operations require this
- NVIDIA Surround = single virtual desktop (7680x1440) -- `get_virtual_screen_bounds()` via `GetSystemMetrics(SM_XVIRTUALSCREEN/SM_CXVIRTUALSCREEN)`
- Brand colors: Racing Red `#E10600`, Asphalt Black `#1A1A1A`, Gunmetal Grey `#5A5A5A`
- Font: Montserrat (body), Enthocentric (headers)
- Never restart explorer.exe on pods -- breaks NVIDIA Surround
- Visual verification mandatory for display-affecting deploys
- Deploy via SCP + reboot (not exec+schtasks) for display changes
- Pod 8 canary first for all deploys

### Claude's Discretion
- Whether to use GDI or Direct2D for rendering (research recommends GDI -- see below)
- How to handle PIN input (keyboard hook vs raw input vs WM_CHAR)
- State machine architecture (reuse LockScreenState enum or create new)
- Whether to embed Montserrat TTF or install it system-wide

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `winapi` | 0.3.9 | Win32 FFI bindings | Already in Cargo.toml with required features. overlay.rs uses it successfully. |
| `windows` | 0.58 | DXGI/D3D11 (already used) | Only for DXGI capture; NOT for lock screen rendering |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `qrcode` | 0.13 | QR code generation | Already in Cargo.toml, used for QR display state |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Raw winapi GDI | Direct2D via `windows` crate | Better antialiasing and GPU accel, but massive API surface increase. GDI is sufficient for static text on 7680x1440. overlay.rs proves GDI works. |
| Raw winapi GDI | `egui` + `eframe` | Immediate-mode GUI framework. Adds ~15MB to binary, OpenGL/wgpu dependency, completely different architecture from existing code. |
| Raw winapi GDI | `slint` | Declarative UI toolkit. Adds ~10MB, requires `.slint` markup files, GPL-licensed for open source (commercial license needed). |
| Raw winapi GDI | `iced` | Elm-inspired GUI. Adds ~20MB, wgpu dependency, complex event model for simple screens. |
| Raw winapi GDI | `winit` + custom renderer | Window creation only (no rendering). Would still need GDI/D2D for drawing. Extra abstraction with no benefit over raw winapi for a single-window app. |

**Installation:**
No new dependencies required. All needed crates already in `Cargo.toml`. May need to add `wingdi` and `winuser` features to `winapi` (already present: `processthreadsapi, winnt, handleapi, winuser, memoryapi, basetsd, synchapi, errhandlingapi, winerror, wingdi, libloaderapi`).

## Architecture Patterns

### Recommended Project Structure
```
crates/rc-agent/src/
  lock_screen.rs          # Existing file -- refactored
  lock_screen/            # NEW module directory
    mod.rs                # LockScreenManager + state machine + public API
    window.rs             # Win32 window creation, message loop, state rendering
    painter.rs            # GDI painting for each state (blanked, PIN, timer, summary, etc.)
    font.rs               # Font embedding (AddFontResourceEx) + GDI font cache
    keyboard.rs           # WM_CHAR/WM_KEYDOWN handler for PIN entry
    qr.rs                 # QR code rendering via GDI (pixel grid)
    http_server.rs        # Retained: /health, /countdown-warning endpoints (no HTML)
```

### Pattern 1: Dedicated Win32 Thread with Shared State (from overlay.rs)
**What:** Spawn a dedicated `std::thread` that owns the Win32 window and runs `GetMessageW` loop. Shared state via `Arc<Mutex<LockScreenState>>` -- the main tokio runtime writes state changes, the Win32 thread reads them on `WM_TIMER` repaint.
**When to use:** Always -- this is the proven pattern in this codebase.
**Example:**
```rust
// Source: overlay.rs lines 1060-1176
pub fn spawn_lock_screen_window(
    state: Arc<Mutex<LockScreenState>>,
    hwnd_slot: Arc<Mutex<Option<isize>>>,
) {
    std::thread::spawn(move || {
        // RegisterClassExW, CreateWindowExW with WS_POPUP | WS_EX_TOPMOST
        // SetTimer for periodic repaint
        // GetMessageW loop
    });
}
```

### Pattern 2: State-Driven Repaint (from overlay.rs HudComponent)
**What:** On each `WM_TIMER` tick, read the current `LockScreenState`, match on the variant, and paint the appropriate screen. The window is created once and never destroyed between state transitions -- only the paint content changes.
**When to use:** All state transitions (blanked -> PIN entry -> active session -> summary).
**Example:**
```rust
// WM_PAINT handler
match &*state {
    LockScreenState::ScreenBlanked => paint_blanked(hdc, &res),
    LockScreenState::PinEntry { .. } => paint_pin_entry(hdc, &res, &data),
    LockScreenState::ActiveSession { .. } => paint_active_session(hdc, &res, &data),
    LockScreenState::SessionSummary { .. } => paint_summary(hdc, &res, &data),
    // ... etc
}
```

### Pattern 3: HWND_TOPMOST + Virtual Screen Spanning
**What:** Use `get_virtual_screen_bounds()` (already exists at line 34) to get full virtual desktop dimensions. Create window with `CreateWindowExW(WS_EX_TOPMOST, ..., vx, vy, vw, vh)`. No need for post-creation `SetWindowPos` retry loop since the window is created at the right size from birth (unlike Edge which fights resizing).
**When to use:** Window creation.
**Example:**
```rust
let (vx, vy, vw, vh) = get_virtual_screen_bounds();
let hwnd = CreateWindowExW(
    WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
    class_name.as_ptr(),
    wide("Racing Point").as_ptr(),
    WS_POPUP | WS_VISIBLE,
    vx, vy, vw, vh,  // spans all monitors from birth
    std::ptr::null_mut(), std::ptr::null_mut(),
    hinstance, state_ptr as LPVOID,
);
```

### Pattern 4: Font Embedding via AddFontResourceEx
**What:** Bundle Montserrat TTF files in the binary (via `include_bytes!`) or in `C:\RacingPoint\fonts\`. At startup, write to temp file and call `AddFontResourceEx(path, FR_PRIVATE, 0)` to make the font available to this process only (no system-wide install needed). `FR_PRIVATE` means other processes cannot see the font.
**When to use:** Startup, before creating GDI fonts.
**Example:**
```rust
// Embed font at compile time
const MONTSERRAT_REGULAR: &[u8] = include_bytes!("../assets/Montserrat-Regular.ttf");
const MONTSERRAT_BOLD: &[u8] = include_bytes!("../assets/Montserrat-Bold.ttf");

fn install_embedded_font(data: &[u8], name: &str) -> bool {
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, data).ok();
    let wide_path = wide(path.to_str().unwrap());
    unsafe { AddFontResourceExW(wide_path.as_ptr(), FR_PRIVATE, std::ptr::null_mut()) > 0 }
}
```

### Anti-Patterns to Avoid
- **Do NOT use `WS_EX_NOACTIVATE` for the lock screen window.** Unlike the overlay HUD which must not steal focus, the lock screen MUST receive keyboard focus for PIN entry. Use `WS_EX_TOPMOST` without `WS_EX_NOACTIVATE`.
- **Do NOT create a new window for each state transition.** Create once, repaint on timer. Window creation/destruction causes visible flicker on 7680x1440.
- **Do NOT use Direct2D for this.** It requires D3D11 device creation, swap chains, render targets -- massive complexity for static text screens. GDI with double-buffering (as in overlay.rs `CreateCompatibleBitmap` + `BitBlt`) eliminates flicker and is sufficient.
- **Do NOT use `LoadCursorW(NULL, IDC_ARROW)` on the blanked state.** When blanked, set cursor to invisible (load a blank cursor or `SetCursor(NULL)` + `ShowCursor(FALSE)`) so the mouse pointer doesn't show on the black screen.
- **Do NOT spawn Edge or any browser as fallback.** The entire point is to eliminate browser dependencies.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| QR code generation | Custom QR encoder | `qrcode` crate (already dep) | QR encoding is complex with error correction levels |
| Font metrics/layout | Manual character width tables | `GetTextExtentPoint32W` (GDI API) | GDI handles kerning, ligatures, font metrics natively |
| Double-buffered painting | Manual buffer management | `CreateCompatibleDC` + `CreateCompatibleBitmap` + `BitBlt` pattern | Already proven in overlay.rs (line 1280-1306) |
| Virtual screen bounds | Hardcoded 7680x1440 | `GetSystemMetrics(SM_XVIRTUALSCREEN)` etc. | Already exists in lock_screen.rs line 34, handles Surround failure detection |
| Thread-safe HWND communication | Custom channels | `PostMessageW` with custom `WM_APP+N` messages | Win32 standard for cross-thread window communication |

**Key insight:** The overlay.rs file is a complete, working, production-tested implementation of every Win32 GUI pattern needed for the lock screen. The only additions are: (1) full-screen instead of 96px bar, (2) keyboard input, (3) more paint states, (4) font embedding. The window lifecycle, GDI resource management, double-buffering, and thread architecture are copy-paste.

## Common Pitfalls

### Pitfall 1: GDI Handle Leak
**What goes wrong:** Each `CreateFont`, `CreateSolidBrush`, `CreatePen` allocates a GDI handle. Windows has a per-process limit of ~10,000. If handles are created in the paint loop without `DeleteObject`, the process runs out of GDI handles and all rendering fails silently.
**Why it happens:** Easy to forget `DeleteObject` when painting dynamic content (e.g., colored text per state).
**How to avoid:** Use the `GdiResources` cache pattern from overlay.rs (line 142-207). Create all static resources once in `WM_CREATE`, free in `WM_DESTROY` via `Drop`. For per-frame dynamic resources, use the `TempBrush` RAII wrapper pattern (line 210-228). overlay.rs already has GDI handle drift monitoring (line 1203-1224) -- replicate this.
**Warning signs:** Text stops rendering, windows draw blank, `gdi_handle_count()` increasing over time.

### Pitfall 2: Keyboard Focus Loss
**What goes wrong:** Other processes (ConspitLink, game launchers) steal focus from the lock screen window, making PIN entry stop working.
**Why it happens:** Windows focus is cooperative. Any process can call `SetForegroundWindow`.
**How to avoid:** Use a `WM_TIMER` check (every 500ms) to call `GetForegroundWindow` and if it's not our HWND, call `SetForegroundWindow(our_hwnd)`. Only do this when in PIN entry state -- during ActiveSession, the game should have focus. This is similar to `enforce_kiosk_foreground()` in the current lock_screen.rs (line 931+).
**Warning signs:** Customer types PIN but nothing appears on screen.

### Pitfall 3: WM_CHAR vs WM_KEYDOWN for PIN Entry
**What goes wrong:** Using `WM_KEYDOWN` for text input requires manual scancode-to-character translation and doesn't handle dead keys, IME, or keyboard layouts correctly.
**Why it happens:** `WM_KEYDOWN` gives virtual key codes, not characters.
**How to avoid:** Use `WM_CHAR` for character input (digits 0-9 for PIN). Use `WM_KEYDOWN` only for special keys (Backspace, Enter, Escape). `TranslateMessage` in the message loop converts `WM_KEYDOWN` to `WM_CHAR` automatically.
**Warning signs:** Wrong characters entered, keyboard layout issues.

### Pitfall 4: NVIDIA Surround False Detection
**What goes wrong:** `GetSystemMetrics(SM_CXVIRTUALSCREEN)` returns 1024x768 when NVIDIA Surround has failed (usually after explorer restart).
**Why it happens:** Surround collapse is not recoverable without reboot.
**How to avoid:** Already handled in `get_virtual_screen_bounds()` (line 52-59) with a warning log when dimensions are below 1920x1080. The native window should still create at whatever size is reported -- a small lock screen is better than no lock screen. Log the anomaly clearly.
**Warning signs:** Lock screen only covers part of the display.

### Pitfall 5: Thread Safety of Window Handles
**What goes wrong:** Calling `SetWindowPos`, `InvalidateRect`, or `SendMessage` from the tokio runtime thread instead of the window's owning thread causes undefined behavior or deadlock.
**Why it happens:** Win32 windows have thread affinity -- most operations must happen on the thread that created the window.
**How to avoid:** Use `PostMessageW` (async, thread-safe) to send custom messages (`WM_APP + N`) from tokio tasks to the window thread. The window thread processes them in its message loop. overlay.rs uses this pattern with `hwnd_slot: Arc<Mutex<Option<isize>>>` to store the HWND for cross-thread access.
**Warning signs:** Lock screen freezes, doesn't update, or crashes with access violation.

### Pitfall 6: Font Not Found Fallback
**What goes wrong:** `CreateFontW("Montserrat", ...)` silently falls back to the system default font (MS Shell Dlg) if Montserrat is not installed.
**Why it happens:** GDI never errors on font creation -- it always returns a font handle, using substitution if the requested face is unavailable.
**How to avoid:** Embed Montserrat TTFs in the binary, install via `AddFontResourceEx` at startup, verify with `EnumFontFamiliesExW` that "Montserrat" is available before creating fonts. Log a warning if fallback triggers.
**Warning signs:** Text renders in a different font than expected.

## Code Examples

### Example 1: Full-Screen Lock Screen Window Creation
```rust
// Source: adapted from overlay.rs lines 1095-1160 + lock_screen.rs lines 34-63
use winapi::um::winuser::*;
use winapi::shared::windef::*;

fn create_lock_screen_window(
    hinstance: winapi::shared::minwindef::HINSTANCE,
    class_name: &[u16],
    state_ptr: *mut LockWindowState,
) -> HWND {
    let (vx, vy, vw, vh) = get_virtual_screen_bounds();

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(lock_wnd_proc),
        hInstance: hinstance,
        // Hide cursor when blanked -- load blank cursor or set to null
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszClassName: class_name.as_ptr(),
        ..unsafe { std::mem::zeroed() }
    };
    unsafe { RegisterClassExW(&wc); }

    // WS_EX_TOPMOST: stays above all windows
    // WS_POPUP: no title bar, no border
    // NO WS_EX_NOACTIVATE: must receive keyboard focus for PIN entry
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST,
            class_name.as_ptr(),
            wide("Racing Point Lock Screen").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            vx, vy, vw, vh,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            state_ptr as winapi::shared::minwindef::LPVOID,
        )
    };
    hwnd
}
```

### Example 2: State-Driven Paint Handler
```rust
// Source: pattern from overlay.rs WM_PAINT handler (line 1229-1236)
unsafe fn paint_lock_screen(
    hwnd: HWND,
    state: &LockScreenState,
    res: &LockGdiResources,
    screen_w: i32,
    screen_h: i32,
) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);

    // Double-buffer: paint to memory DC, then BitBlt to screen
    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, screen_w, screen_h);
    let old_bmp = SelectObject(mem_dc, mem_bmp as *mut _);

    // Fill background: Asphalt Black #1A1A1A
    let bg_rect = RECT { left: 0, top: 0, right: screen_w, bottom: screen_h };
    FillRect(mem_dc, &bg_rect, res.brush_asphalt);

    match state {
        LockScreenState::ScreenBlanked => {
            // Pure black -- background fill is sufficient
            // Optionally render centered Racing Point logo
        }
        LockScreenState::PinEntry { driver_name, pin_buffer, pin_error, .. } => {
            paint_pin_entry(mem_dc, res, screen_w, screen_h, driver_name, pin_buffer, pin_error);
        }
        LockScreenState::ActiveSession { driver_name, remaining_seconds, .. } => {
            paint_active_session(mem_dc, res, screen_w, screen_h, driver_name, *remaining_seconds);
        }
        LockScreenState::SessionSummary { .. } => {
            paint_summary(mem_dc, res, screen_w, screen_h, state);
        }
        // ... other states
        _ => {}
    }

    // Blit to screen
    BitBlt(hdc, 0, 0, screen_w, screen_h, mem_dc, 0, 0, SRCCOPY);
    SelectObject(mem_dc, old_bmp);
    DeleteObject(mem_bmp as *mut _);
    DeleteDC(mem_dc);
    EndPaint(hwnd, &ps);
}
```

### Example 3: Keyboard Input for PIN Entry
```rust
// WM_CHAR handler for numeric PIN input
WM_CHAR => {
    let ch = wparam as u8 as char;
    let ws = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut LockWindowState);

    if let LockScreenState::PinEntry { ref mut pin_buffer, .. } = *ws.state.lock().unwrap() {
        match ch {
            '0'..='9' if pin_buffer.len() < 6 => {
                pin_buffer.push(ch);
                InvalidateRect(hwnd, std::ptr::null(), FALSE);
                if pin_buffer.len() == 6 {
                    // Auto-submit: send PIN to event channel
                    let pin = pin_buffer.clone();
                    let _ = ws.event_tx.blocking_send(LockScreenEvent::PinEntered { pin });
                }
            }
            _ => {}
        }
    }
    0
}
WM_KEYDOWN => {
    let vk = wparam as i32;
    let ws = &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut LockWindowState);

    if let LockScreenState::PinEntry { ref mut pin_buffer, .. } = *ws.state.lock().unwrap() {
        match vk {
            VK_BACK if !pin_buffer.is_empty() => {
                pin_buffer.pop();
                InvalidateRect(hwnd, std::ptr::null(), FALSE);
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
```

### Example 4: QR Code Rendering via GDI
```rust
// Render QR code as a grid of filled rectangles
fn paint_qr_code(hdc: HDC, qr_data: &str, center_x: i32, center_y: i32, module_size: i32) {
    use qrcode::QrCode;
    let code = match QrCode::new(qr_data.as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let modules = code.to_colors();
    let width = code.width() as i32;
    let total = width * module_size;
    let start_x = center_x - total / 2;
    let start_y = center_y - total / 2;

    let white_brush = CreateSolidBrush(0x00FFFFFF);
    let black_brush = CreateSolidBrush(0x00000000);

    // White background for QR
    let bg = RECT {
        left: start_x - module_size,
        top: start_y - module_size,
        right: start_x + total + module_size,
        bottom: start_y + total + module_size,
    };
    FillRect(hdc, &bg, white_brush);

    for (i, color) in modules.iter().enumerate() {
        if *color == qrcode::types::Color::Dark {
            let x = (i as i32 % width) * module_size + start_x;
            let y = (i as i32 / width) * module_size + start_y;
            let r = RECT { left: x, top: y, right: x + module_size, bottom: y + module_size };
            FillRect(hdc, &r, black_brush);
        }
    }

    DeleteObject(white_brush as *mut _);
    DeleteObject(black_brush as *mut _);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Edge `--kiosk` mode (single monitor) | Edge `--app` + `SetWindowPos(HWND_TOPMOST)` | 2026-03-23 (commit 4044af7b) | Fixed multi-monitor spanning but still uses Edge |
| Edge default profile | Dedicated Edge profile + SNSS cleanup | 2026-03-31 (MMA consensus) | Reduced session restore bugs but still complex |
| `close_browser()` kills all Edge | Track child PID, skip Startup Boost processes | 2026-04 | Reduced false relaunch skips |

**Deprecated/outdated:**
- Edge `--edge-kiosk-type=fullscreen`: Only fullscreens to primary monitor, incompatible with NVIDIA Surround.
- HTML-based lock screen: ~300MB RAM overhead, 3169 lines of browser lifecycle management code, SNSS bugs, Startup Boost confusion.

## GDI vs Direct2D Decision

**Recommendation: Use GDI (not Direct2D) for this phase.**

| Factor | GDI | Direct2D |
|--------|-----|----------|
| Already proven in codebase | Yes (overlay.rs, 1370+ lines) | No |
| Dependencies needed | `winapi` 0.3 (already in Cargo.toml) | `windows` 0.58 + D3D11 + DXGI features |
| Complexity | Low: CreateFont, DrawText, FillRect | High: D3D11Device, ID2D1Factory, IDWriteFactory, HwndRenderTarget, swap chains |
| Text quality | Good for large text (24pt+), adequate for 7680x1440 | Subpixel antialiasing, ClearType |
| Performance | CPU-only, negligible for 1 repaint/sec | GPU-accelerated via D3D11 |
| Font embedding | `AddFontResourceEx` + `CreateFontW` | `IDWriteFontFileLoader` custom impl |
| Risk | Zero -- exact pattern exists in overlay.rs | Medium -- new API surface, untested in this project |

**Why GDI is sufficient:** The lock screen repaints at most once per second (countdown timer). It renders static text, rectangles, and a QR code. There are no animations, gradients, or complex graphics. GDI with double-buffering (which overlay.rs already does) produces zero-flicker output. At 7680x1440, GDI font rendering at 24pt+ is visually indistinguishable from DirectWrite.

**When to upgrade to Direct2D:** If future requirements include animations (fade transitions between states), rounded rectangles, gradient backgrounds, or sub-14pt text that needs ClearType quality. This can be done as a separate phase without changing the window/state architecture.

## Keyboard Input Strategy

**Recommendation: WM_CHAR for character input, WM_KEYDOWN for special keys.**

The lock screen PIN entry accepts 4-6 digit numeric codes. The input model:

1. `TranslateMessage(&msg)` in the message loop converts `WM_KEYDOWN` to `WM_CHAR`
2. `WM_CHAR` handler filters for digits `'0'..='9'`, appends to `pin_buffer: String`
3. `WM_KEYDOWN` handler catches `VK_BACK` (backspace) and `VK_RETURN` (submit)
4. On reaching 6 digits OR pressing Enter, send `LockScreenEvent::PinEntered` via the existing `mpsc::Sender`
5. PIN display shows dots/asterisks (not actual digits) for security

**Focus enforcement:** A `WM_TIMER` callback (every 500ms) checks if the lock screen has focus. If not, and the current state is PinEntry, call `SetForegroundWindow(our_hwnd)`. This replaces the current `enforce_kiosk_foreground()` function.

**Block keyboard shortcuts:** Handle `WM_SYSKEYDOWN` to block Alt+Tab, Alt+F4, Win key while lock screen is active (except during ActiveSession when the game has focus). Use `SetWindowsHookEx(WH_KEYBOARD_LL)` only if `WM_SYSKEYDOWN` is insufficient.

## Font Embedding Strategy

**Montserrat is NOT installed on pods or James machine.** The current HTML lock screen loads it from Google Fonts CDN (`fonts.googleapis.com`). For native rendering:

**Recommended approach:** Embed TTF files at compile time via `include_bytes!`.

1. Download Montserrat-Regular.ttf and Montserrat-Bold.ttf (SIL Open Font License)
2. Place in `crates/rc-agent/assets/fonts/`
3. Use `include_bytes!("../assets/fonts/Montserrat-Bold.ttf")` to embed in binary
4. At lock screen init, write to `%TEMP%\rp-montserrat-bold.ttf` and call `AddFontResourceExW(path, FR_PRIVATE, 0)`
5. At shutdown, call `RemoveFontResourceExW`

**Binary size impact:** Montserrat-Regular.ttf is ~90KB, Montserrat-Bold.ttf is ~90KB. Total: ~180KB added to binary. Negligible compared to current 15MB rc-agent binary.

**Fallback:** If `AddFontResourceEx` fails, fall back to "Segoe UI" (available on all Windows 11 machines, already used in overlay.rs).

## Migration Path

The refactoring can be done incrementally:

1. **Phase 329a:** Create `NativeLockScreen` module with Win32 window, supporting only `ScreenBlanked` state. Wire it into `LockScreenManager` behind a feature flag or config toggle. Keep Edge code intact.
2. **Phase 329b:** Add PIN entry rendering + keyboard input. Test on Pod 8 canary.
3. **Phase 329c:** Add all remaining states (ActiveSession, SessionSummary, BetweenSessions, LaunchSplash, etc.)
4. **Phase 329d:** Remove Edge code path, HTTP server HTML templates, and all browser lifecycle management. Keep HTTP server for `/health` and `/countdown-warning` endpoints only.

**Rollback strategy:** The config toggle allows reverting to Edge at any point. The `browser_disabled` field in `LockScreenManager` already exists for POS devices -- reuse this pattern.

## HTTP Server Retention

The existing HTTP server on port 18923 serves two purposes:
1. **HTML pages for Edge** (to be removed)
2. **API endpoints** (`/health`, `/countdown-warning`, `/state`) used by monitoring and the overlay

**Keep the HTTP server for API endpoints only.** Remove all HTML template generation (~1500 lines of embedded HTML/CSS/JS). The `/health` endpoint returns JSON and is used by `verify-pod-screen.js` and fleet monitoring. The `/countdown-warning` endpoint is used by the overlay HUD.

## Effort Estimate

| Component | Estimated Lines | Complexity | Risk |
|-----------|----------------|------------|------|
| Window creation + message loop | ~150 | Low (copy overlay.rs pattern) | Low |
| GDI resource cache + font embedding | ~100 | Low | Low |
| ScreenBlanked paint | ~20 | Trivial | None |
| PinEntry paint + keyboard | ~200 | Medium (keyboard focus management) | Medium |
| ActiveSession / Timer paint | ~80 | Low | Low |
| SessionSummary / BetweenSessions paint | ~150 | Low | Low |
| QR code GDI rendering | ~60 | Low (algorithm above) | Low |
| StartupConnecting / Disconnected / ConfigError / Lockdown / MaintenanceRequired | ~200 | Low (text-only screens) | Low |
| LaunchSplash / AwaitingAssistance paint | ~80 | Low | Low |
| Focus enforcement + cursor hiding | ~50 | Low | Low |
| HTTP server cleanup (remove HTML) | -1500 | Low (deletion) | Low |
| Edge lifecycle removal | -800 | Low (deletion) | Low |
| **Total new code** | **~1090** | | |
| **Total deleted code** | **~2300** | | |
| **Net change** | **-1210 lines** | | |

**Estimated effort:** 3-4 focused implementation sessions. The highest risk is keyboard focus management on pods with ConspitLink competing for input.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | workspace Cargo.toml |
| Quick run command | `cargo test -p rc-agent -- lock_screen` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command |
|--------|----------|-----------|-------------------|
| LOCK-01 | Window spans 7680x1440 | manual | Visual verify on Pod 8 |
| LOCK-02 | PIN entry accepts digits | unit | `cargo test -p rc-agent -- pin_entry` |
| LOCK-03 | State transitions repaint correctly | unit | `cargo test -p rc-agent -- lock_screen_state` |
| LOCK-04 | No Edge process after migration | integration | `tasklist /FI "IMAGENAME eq msedge.exe"` on pod |
| LOCK-05 | RAM usage < 10MB for lock screen | manual | `tasklist /FI "IMAGENAME eq rc-agent.exe"` before/after |
| LOCK-06 | Font renders as Montserrat | manual | Visual verify screenshot |

### Wave 0 Gaps
- [ ] Unit tests for PIN buffer state machine (char append, backspace, submit, overflow)
- [ ] Unit tests for state-driven paint dispatch (each LockScreenState variant calls correct painter)
- [ ] Integration test: font embedding success verification

## Open Questions

1. **Enthocentric font for headers**
   - What we know: CLAUDE.md lists "Enthocentric (headers)" as brand font. Current HTML lock screen uses Montserrat for everything.
   - What's unclear: Is Enthocentric actually used anywhere in the lock screen? Is there a TTF file available?
   - Recommendation: Use Montserrat Bold for all text initially. Add Enthocentric later if Uday requests it. Keeps font embedding simple (2 files not 4).

2. **Logo rendering**
   - What we know: Current HTML renders an SVG Racing Point logo inline. GDI cannot render SVG natively.
   - What's unclear: Is there a PNG or BMP version of the logo?
   - Recommendation: Convert logo SVG to a 256x256 PNG, embed via `include_bytes!`, render via `StretchBlt` from a loaded bitmap. Or render "RACING POINT" text using Montserrat Bold with the same styling as the SVG (red "RACING", white "POINT").

3. **Cursor behavior during game session**
   - What we know: During ActiveSession, the lock screen is hidden and the game has focus. The overlay HUD uses `WS_EX_NOACTIVATE` so it never steals focus.
   - What's unclear: Should the lock screen window be hidden (via `ShowWindow(SW_HIDE)`) during game sessions, or just painted transparent?
   - Recommendation: Use `ShowWindow(hwnd, SW_HIDE)` when transitioning to Hidden state, `ShowWindow(hwnd, SW_SHOW)` when any visible state is set. This matches the current behavior where `close_browser()` kills Edge entirely during active sessions.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| winapi crate | Window creation, GDI | Yes | 0.3.9 | -- |
| windows crate | Not needed for lock screen | Yes | 0.58 | -- |
| Montserrat TTF | Brand font rendering | No (not installed) | -- | Embed in binary + AddFontResourceEx |
| NVIDIA Surround | 7680x1440 virtual desktop | Yes (on pods) | -- | Falls back to primary monitor dims |
| Rust stable | Compilation | Yes | 1.93.1 | -- |

**Missing dependencies with no fallback:** None.

**Missing dependencies with fallback:**
- Montserrat font: Not installed system-wide. Fallback: embed TTF in binary (recommended approach).

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/overlay.rs` -- Complete working Win32 GDI window implementation in this codebase. Lines 1-1370+. Production-tested on 8 pods.
- `crates/rc-agent/src/lock_screen.rs` -- Current Edge-based implementation (3169 lines). State machine, HTTP server, Edge lifecycle.
- `DEBUG-BLANKING-SCREEN.md` -- History of multi-monitor spanning approaches tested (6 failed, 1 working).
- `Cargo.toml` -- winapi 0.3 features already available: winuser, wingdi, libloaderapi.
- `.cargo/config.toml` -- Static CRT linking confirmed.

### Secondary (MEDIUM confidence)
- [Microsoft Win32 GDI documentation](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/Gdi/index.html) -- GDI API reference for Rust windows crate
- [Direct2D text rendering](https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-and-directwrite) -- Compared Direct2D vs GDI for decision
- [winapi crate](https://github.com/retep998/winapi-rs) -- Verified v0.3.9 is current stable

### Tertiary (LOW confidence)
- VMS CBlankingDlg reference (from user context) -- MFC/GDI+ implementation, ~5MB RAM. Cannot verify independently but consistent with expected Win32 window overhead.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- using existing crates already in the project, proven patterns in overlay.rs
- Architecture: HIGH -- direct replication of overlay.rs pattern, well-understood Win32 APIs
- Pitfalls: HIGH -- based on production experience documented in DEBUG-BLANKING-SCREEN.md and CLAUDE.md standing rules
- Font embedding: MEDIUM -- AddFontResourceEx is documented Win32 API but untested in this project
- Effort estimate: MEDIUM -- dependent on ConspitLink focus competition behavior during PIN entry

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable Win32 APIs, unlikely to change)
