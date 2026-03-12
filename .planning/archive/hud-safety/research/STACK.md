# Stack Research: Racing HUD Redesign + Wheelbase FFB Safety

**Date:** 2026-03-11
**Author:** James Vowles (research pass)
**Feeds into:** HUD milestone roadmap, FFB safety kill-switch feature

---

## Table of Contents

1. [OpenFFBoard USB HID FFB Protocol](#1-openffboard-usb-hid-ffb-protocol)
2. [Win32 GDI / DirectWrite Overlay Rendering](#2-win32-gdi--directwrite-overlay-rendering)
3. [hidapi Rust Crate — Write Protocol](#3-hidapi-rust-crate--write-protocol)
4. [AC Shared Memory — acpmf_graphics Fields](#4-ac-shared-memory--acpmf_graphics-fields)
5. [Synthesis: Recommended Approaches](#5-synthesis-recommended-approaches)

---

## 1. OpenFFBoard USB HID FFB Protocol

### 1.1 Device Identification

- **VID:** `0x1209`  **PID:** `0xFFB0`
- Registered on pid.codes (open-source VID pool).
- Presents to Windows as a **DirectInput force-feedback gamepad** (USB HID PID class).
- The Conspit Ares 8Nm is built on OpenFFBoard firmware, so the protocol applies directly.

### 1.2 Two Separate HID Interfaces

OpenFFBoard exposes **two independent HID interfaces** on the same USB device:

| Interface | Purpose | Report ID |
|-----------|---------|-----------|
| HID PID (gamepad) | Standard DirectInput FFB — games write here | Standard PID reports (see 1.4) |
| Vendor command interface | OpenFFBoard-specific config/control | `0xA1` |

**The vendor interface is what rc-agent should use for a safety kill-switch.** Games drive the PID interface themselves via DirectInput/XInput.

### 1.3 Vendor Command Interface (Report 0xA1)

The vendor report has the following packed binary layout (little-endian):

```
Byte  0     : Report ID = 0xA1
Byte  1     : Type (u8)
                  0 = write
                  1 = request (read)
                  2 = info
                  3 = writeAddr  (two-value write)
                  4 = requestAddr
                 10 = ACK (device reply)
                 13 = notFound
                 14 = notification
                 15 = err
Bytes 2-3   : ClassID (u16 LE) — target subsystem
Byte  4     : Instance (u8)   — which instance (usually 0)
Bytes 5-8   : Command / CmdID (u32 LE)
Bytes 9-16  : Data / Value (i64 LE)
Bytes 17-24 : Address (i64 LE) — used with writeAddr/requestAddr
```

Total payload: **25 bytes** (+ 1 byte report ID = 26 bytes sent to hidapi).

**Class IDs relevant to FFB safety:**

| ClassID | Class Name | Notes |
|---------|-----------|-------|
| `0x00`  | System    | Global: reset, save, etc. |
| `0x01`  | Main      | Top-level mode selection |
| `0xA1`  | FFBWheel  | The active FFB wheel class |
| `0xA02` | Effects   | Per-effect gain control |

**Key Commands for FFB Safety (ClassID = 0xA1, FFBWheel):**

| Command Name | CmdID (dec) | Description |
|-------------|-------------|-------------|
| `ffbactive`  | `0x00`     | Read/write FFB active flag. Write 0 to disable FFB. |
| `estop`      | `0x0A`     | Emergency stop. Write 1 to engage e-stop (motor torque = 0). |
| `hidsendspd` | `0x09`     | Gamepad HID update rate in Hz. |

**Effects gain class (ClassID = 0xA02):**

| Command | Effect |
|---------|--------|
| `spring` | Spring effect gain |
| `friction` | Friction effect gain |
| `damper` | Damper effect gain |
| `inertia` | Inertia effect gain |

Writing gain = 0 to all effects reduces output force contribution from those effects to zero, but the cleanest safety kill is the `estop` command.

### 1.4 USB HID PID Standard Reports (used by games, not rc-agent)

The OpenFFBoard firmware implements the **USB HID Physical Interface Device (PID) v1.0** standard. This is what DirectInput uses automatically — rc-agent does not need to speak this protocol for normal operation. However, understanding it is needed if we ever want to inject a "zero torque" constant-force override.

Standard PID report IDs (may vary by descriptor implementation, but conventionally):

| Report ID | Name | Direction | Purpose |
|-----------|------|-----------|---------|
| 1 | Set Effect | Host→Device | Create/modify an effect block |
| 2 | Set Envelope | Host→Device | Attack/fade envelope |
| 3 | Set Condition | Host→Device | Spring/damper condition |
| 4 | Set Periodic | Host→Device | Sine/square wave |
| 5 | Set Constant Force | Host→Device | Magnitude of constant force |
| 6 | Set Ramp Force | Host→Device | Ramp start/end |
| 7 | Custom Force Data | Host→Device | Custom waveform |
| 11 | Device Control | Host→Device | Enable/disable/stop actuators |
| 12 | Device Gain | Host→Device | Global gain |
| 1 (IN) | PID State | Device→Host | Effect status |
| 2 (IN) | Block Load | Device→Host | Effect block index assigned |

**Set Constant Force Report (ID 5) byte layout:**
```
Byte 0: Report ID = 5
Byte 1: Effect Block Index (1..40)
Bytes 2-3: Magnitude (i16 LE, range -10000..+10000, 0 = center/zero)
```

**Device Control Report (ID 11) byte layout — the software emergency stop:**
```
Byte 0: Report ID = 11
Byte 1: Control value:
    1 = Enable Actuators
    2 = Disable Actuators   <-- this is the safe "motor off" command
    3 = Stop All Effects
    4 = Device Reset
    5 = Device Pause
    6 = Device Continue
```

**Safety approach via HID PID (alternative to vendor interface):**
```
// Send Device Control = Disable Actuators (bytes: [11, 2])
device.write(&[0x00, 11, 2]).unwrap();  // 0x00 = no report ID prefix (or use 11 directly)
```

### 1.5 Recommended Safety Kill Implementation

**For rc-agent FFB kill-switch, use the Vendor Interface (0xA1) `estop` command:**

```rust
use hidapi::HidApi;

const VID: u16 = 0x1209;
const PID: u16 = 0xFFB0;

fn send_vendor_command(
    device: &hidapi::HidDevice,
    type_: u8,
    class_id: u16,
    instance: u8,
    cmd: u32,
    data: i64,
) -> hidapi::HidResult<()> {
    let mut report = [0u8; 26]; // 1 (report id) + 25 payload
    report[0] = 0xA1;           // Report ID
    report[1] = type_;
    report[2] = (class_id & 0xFF) as u8;
    report[3] = (class_id >> 8) as u8;
    report[4] = instance;
    let cmd_bytes = cmd.to_le_bytes();
    report[5..9].copy_from_slice(&cmd_bytes);
    let data_bytes = data.to_le_bytes();
    report[9..17].copy_from_slice(&data_bytes);
    // addr bytes 17-24 stay 0
    device.write(&report)?;
    Ok(())
}

fn ffb_emergency_stop(device: &hidapi::HidDevice) -> hidapi::HidResult<()> {
    // ClassID=0xA1 (FFBWheel), Instance=0, CmdID=0x0A (estop), Data=1
    send_vendor_command(device, 0, 0x00A1, 0, 0x0A, 1)
}

fn ffb_disable(device: &hidapi::HidDevice) -> hidapi::HidResult<()> {
    // ClassID=0xA1, CmdID=0x00 (ffbactive), Data=0 (disable)
    send_vendor_command(device, 0, 0x00A1, 0, 0x00, 0)
}
```

**Open the correct HID interface:** The OpenFFBoard enumerates multiple HID interfaces. Filter by usage page `0xFF00` (vendor-defined) to land on the command interface, not the gamepad:

```rust
let api = HidApi::new()?;
let device = api
    .device_list()
    .filter(|d| d.vendor_id() == VID && d.product_id() == PID)
    .filter(|d| d.usage_page() == 0xFF00)
    .next()
    .ok_or(hidapi::HidError::HidApiError { message: "not found".into() })?
    .open_device(&api)?;
```

### 1.6 Notes and Caveats

- The HID command interface is only active when the FFBWheel class is loaded (default on Conspit Ares).
- `estop` is **not persistent** across USB reconnect. rc-agent must re-send on reconnect if billing session is inactive.
- The PID Device Control report (ID 11, value 2 = Disable Actuators) is an alternative path through the gamepad interface. This is what Windows DirectInput games would send, so it may be blocked by the OS HID driver claiming the interface. The vendor interface is more reliable from userspace.
- Confirmed: OpenFFBoard configurator Python source uses `struct.unpack('<H', data[2:4])` for ClassID and `struct.unpack('<q', data[9:17])` for Data — confirming the little-endian layout above.

---

## 2. Win32 GDI / DirectWrite Overlay Rendering

### 2.1 Current State Assessment

The existing `overlay.rs` already implements a functional Win32 GDI overlay with:
- `WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED` window
- `WS_POPUP | WS_VISIBLE` styles (borderless from creation)
- `SetLayeredWindowAttributes(hwnd, 0, 240, LWA_ALPHA)` — 94% opacity
- Double-buffered GDI via `CreateCompatibleDC` / `BitBlt`
- `CLEARTYPE_QUALITY` (quality=5) in `CreateFontW`
- `TIMER_ID` repaint every 200ms
- 6-section layout: Session | Current Lap | Gear+Speed | Prev | Best | Lap#
- F1-style sector color coding (purple/green/yellow)

**This is already solid Win32 GDI.** The redesign question is: do we stay with GDI or migrate to DirectWrite/Direct2D?

### 2.2 GDI Limitations for HUD Redesign

| Limitation | Impact |
|-----------|--------|
| ClearType only works on light text on dark — no sub-pixel on alpha composited windows | Text looks slightly fuzzy at small sizes on transparency |
| No native path/curve drawing with AA | Rounded corners, arcs (rev counter arc) require workarounds |
| No per-pixel alpha compositing beyond LWA_ALPHA global | Can't do per-element fade/glow effects |
| Font metrics via `GetTextExtentPoint32` are coarse | Precise layout (centering at px level) requires manual correction |
| `TextOutW` does not wrap | Manual line-break logic required |
| 1-pixel precision only | No sub-pixel positioning for smooth animation |

**For AC Essentials-style HUD (bold numbers, monospaced telemetry, no complex curves):** GDI is sufficient and simpler. Stay with GDI if the design stays flat-rectangle with no arcs.

**For a modern HUD with a rev counter arc, rounded cards, or animated elements:** Direct2D + DirectWrite is the correct upgrade path.

### 2.3 DirectWrite / Direct2D Migration Path

**Architecture: Layered Window + DC Render Target**

```
CreateWindowExW(WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, ...)
  └── WM_PAINT
        └── BeginPaint → GetDC
              └── ID2D1DCRenderTarget::BindDC(hdc, &rect)
                    ├── ID2D1SolidColorBrush for fills
                    ├── IDWriteTextLayout for text
                    └── ID2D1RenderTarget::DrawGeometry for arcs
```

**Key Rust crate:** `windows` crate (microsoft/windows-rs), feature flags needed:

```toml
[dependencies]
windows = { version = "0.58", features = [
    "Win32_Graphics_Direct2D",
    "Win32_Graphics_Direct2D_Common",
    "Win32_Graphics_DirectWrite",
    "Win32_Graphics_Gdi",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
] }
```

**DirectWrite text rendering setup:**

```rust
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Direct2D::*;

// Factory creation
let dw_factory: IDWriteFactory =
    unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };

// Text format (replaces CreateFontW)
let text_format = unsafe {
    dw_factory.CreateTextFormat(
        w!("Segoe UI"),
        None,
        DWRITE_FONT_WEIGHT_BOLD,
        DWRITE_FONT_STYLE_NORMAL,
        DWRITE_FONT_STRETCH_NORMAL,
        22.0,           // size in DIPs (not pixels — multiply by DPI/96)
        w!("en-us"),
    )?
};
```

**Anti-aliasing modes:**

```rust
// For text (ClearType or grayscale)
render_target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE);
// For geometry (arcs, rounded rects)
render_target.SetAntialiasMode(D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
```

**Rev counter arc example (AC Essentials-style):**

```rust
// Draw arc for RPM gauge (0..270 degrees sweep)
let center = D2D_POINT_2F { x: 100.0, y: 50.0 };
let radius = 40.0;
let sweep = rpm_fraction * 270.0_f32.to_radians();
let arc_segment = D2D1_ARC_SEGMENT {
    point: D2D_POINT_2F {
        x: center.x + radius * sweep.cos(),
        y: center.y + radius * sweep.sin(),
    },
    size: D2D_SIZE_F { width: radius, height: radius },
    rotationAngle: 0.0,
    sweepDirection: D2D1_SWEEP_DIRECTION_CLOCKWISE,
    arcSize: if sweep > std::f32::consts::PI {
        D2D1_ARC_SIZE_LARGE
    } else {
        D2D1_ARC_SIZE_SMALL
    },
};
```

### 2.4 Font Recommendations for Racing HUD

| Use Case | Font | Size | Weight | Notes |
|----------|------|------|--------|-------|
| Main lap time | "Segoe UI" or "Arial" | 22-28pt | Bold | Proportional OK for times |
| Gear number | "Consolas" or "Segoe UI" | 36pt | Bold | Large, instant read |
| Speed | "Segoe UI" | 18pt | Bold | kmh is always 3 digits |
| Sector times | "Consolas" | 12pt | Regular | Monospace prevents jitter as digits change |
| Labels (SESSION, BEST) | "Segoe UI" | 10pt | Regular | Small caps style |
| Session timer | "Consolas" | 22pt | Bold | Monospace prevents layout shift MM:SS |

**Key rule:** Use monospace ("Consolas") for any number that changes digit count or can change width between frames. Proportional fonts cause the surrounding layout to shift when "9" becomes "10" etc. For lap times this matters — `1:23.456` vs `1:09.012` have different widths in Segoe UI.

**AC Essentials style reference:**
- Black background, 2-3px red accent stripe at bottom
- Large white numbers, small grey labels above
- Sector times shown inline below lap time
- No rounded corners, flat rectangular sections

**ClearType quality flag (already in code):** `CLEARTYPE_QUALITY = 5` in `CreateFontW` lfQuality. This is correct. Do not use `DEFAULT_QUALITY = 0` or `ANTIALIASED_QUALITY = 4` — ClearType is sharper.

### 2.5 Click-Through Behaviour

Current code uses `WM_MOUSEACTIVATE → MA_NOACTIVATE`. This prevents focus steal but the window still receives mouse clicks.

**To make fully click-through (mouse events pass to game underneath):**

```rust
// In CreateWindowExW: add WS_EX_TRANSPARENT to extended style
WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT
```

Or respond to `WM_NCHITTEST` with `HTTRANSPARENT (-1)` for fine-grained control (e.g., allow click on specific zones).

**Note:** `WS_EX_TRANSPARENT` applies the transparent style to the entire window — once set, mouse messages are never delivered to the HUD window. The current `MA_NOACTIVATE` approach is better if we ever want a click zone (e.g., "click to extend session" button).

### 2.6 Decision Matrix

| Feature Needed | Use GDI | Use Direct2D+DirectWrite |
|---------------|---------|--------------------------|
| Flat rectangular HUD (current style) | YES | Overkill |
| Crisp text at 200% DPI (4K monitors) | GDI struggles | YES — DIP units handle DPI |
| Rev counter arc / gauge | Need bitmap or manual hack | YES — native geometry |
| Per-element alpha/glow | No | YES |
| Animation (smooth transitions) | Hard | YES |
| Complexity cost | Low | Medium — COM boilerplate |

**Recommendation:** For the immediate HUD redesign milestone, **stay on GDI** but add the following improvements:
1. Swap `"Segoe UI"` to `"Consolas"` for all time/number values.
2. Increase font sizes (22→28 for main lap, 32→42 for gear).
3. Add a thin `#E10600` bottom border (already there).
4. Add driver name in top-right corner (already in `OverlayData`).

**If a rev-counter arc is required:** port to Direct2D. Budget 2-3 days for the migration — the window creation stays the same, only the WM_PAINT handler changes.

---

## 3. hidapi Rust Crate — Write Protocol

### 3.1 Crate Selection

**Recommended crate:** `hidapi` v2.x by ruabmbua (wraps libhidapi C library)

```toml
[dependencies]
hidapi = { version = "2.6", features = ["windows-native"] }
```

The `windows-native` feature uses the Windows HID backend directly (no libusb required — avoids driver conflicts on Windows).

### 3.2 Core API

```rust
use hidapi::{HidApi, HidDevice, HidError};

// Enumerate and open
let api = HidApi::new()?;
let device = api.open(0x1209, 0xFFB0)?;

// OR open with usage page filter (for multi-interface devices like OpenFFBoard):
let device = api
    .device_list()
    .find(|d| {
        d.vendor_id() == 0x1209
        && d.product_id() == 0xFFB0
        && d.usage_page() == 0xFF00  // vendor interface
    })
    .ok_or(HidError::HidApiError { message: "OpenFFBoard not found".into() })?
    .open_device(&api)?;
```

### 3.3 Write (Output Report)

```rust
// HidDevice::write() — sends an Output report
// First byte MUST be the Report ID (or 0x00 for single-report devices)
// Returns number of bytes written, or error

let report: [u8; 26] = build_vendor_report(...);
let n = device.write(&report)?;
assert_eq!(n, 26);
```

**Critical rule:** The byte buffer passed to `write()` always starts with the Report ID byte. For OpenFFBoard vendor reports, that is `0xA1`. For a device with a single report (no numbered reports), use `0x00`.

### 3.4 Feature Report

```rust
// HidDevice::send_feature_report() — uses SET_REPORT control transfer
// First byte is Report ID, same as write()
device.send_feature_report(&[0xA1, ...])?;

// HidDevice::get_feature_report() — GET_REPORT
let mut buf = [0u8; 26];
buf[0] = 0xA1;  // Report ID to request
let n = device.get_feature_report(&mut buf)?;
```

Feature reports go over the control endpoint (EP0), Output reports go over the interrupt OUT endpoint. For OpenFFBoard vendor commands, `write()` to the interrupt endpoint is preferred (matches configurator behaviour).

### 3.5 Read (Input Report)

```rust
// Non-blocking read
device.set_blocking_mode(false)?;
let mut buf = [0u8; 26];
match device.read(&mut buf) {
    Ok(n) if n > 0 => { /* parse ACK */ }
    Ok(_) => { /* no data */ }
    Err(e) => eprintln!("read error: {e}"),
}

// Blocking read with timeout (ms)
let n = device.read_timeout(&mut buf, 100)?;  // 100ms timeout
```

### 3.6 Complete Safety Kill Sequence (Rust)

```rust
use hidapi::HidApi;

const VID: u16 = 0x1209;
const PID: u16 = 0xFFB0;
const VENDOR_USAGE_PAGE: u16 = 0xFF00;

pub struct WheelbaseGuard {
    device: hidapi::HidDevice,
}

impl WheelbaseGuard {
    pub fn open() -> Result<Self, hidapi::HidError> {
        let api = HidApi::new()?;
        let info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == VID
                    && d.product_id() == PID
                    && d.usage_page() == VENDOR_USAGE_PAGE
            })
            .ok_or(hidapi::HidError::HidApiError {
                message: "Conspit wheelbase not found".into(),
            })?;
        let device = info.open_device(&api)?;
        Ok(Self { device })
    }

    /// Send emergency stop — motor torque → 0 immediately.
    pub fn emergency_stop(&self) -> Result<(), hidapi::HidError> {
        self.vendor_write(0, 0x00A1, 0, 0x0A, 1) // estop = 1
    }

    /// Re-enable FFB after e-stop (call when session resumes).
    pub fn enable_ffb(&self) -> Result<(), hidapi::HidError> {
        self.vendor_write(0, 0x00A1, 0, 0x0A, 0) // estop = 0
    }

    fn vendor_write(
        &self,
        type_: u8,
        class_id: u16,
        instance: u8,
        cmd: u32,
        data: i64,
    ) -> Result<(), hidapi::HidError> {
        let mut report = [0u8; 26];
        report[0] = 0xA1;
        report[1] = type_;
        report[2..4].copy_from_slice(&class_id.to_le_bytes());
        report[4] = instance;
        report[5..9].copy_from_slice(&cmd.to_le_bytes());
        report[9..17].copy_from_slice(&data.to_le_bytes());
        self.device.write(&report)?;
        Ok(())
    }
}

impl Drop for WheelbaseGuard {
    fn drop(&mut self) {
        // Safety: always try to stop FFB on drop (session end, crash, etc.)
        let _ = self.emergency_stop();
    }
}
```

### 3.7 Threading Considerations

- `HidDevice` is not `Send` in hidapi v2.x. Keep the device handle on a dedicated thread.
- Use a `mpsc::channel` to send kill commands from the billing logic to the HID thread.
- HID writes are synchronous and fast (<1ms on USB FS). No async needed.

```rust
// In rc-agent main or billing module:
let (ffb_tx, ffb_rx) = std::sync::mpsc::channel::<FfbCommand>();

std::thread::spawn(move || {
    let Ok(guard) = WheelbaseGuard::open() else {
        tracing::warn!("Wheelbase not found — FFB safety disabled");
        return;
    };
    while let Ok(cmd) = ffb_rx.recv() {
        match cmd {
            FfbCommand::Kill => { let _ = guard.emergency_stop(); }
            FfbCommand::Enable => { let _ = guard.enable_ffb(); }
        }
    }
});

// On billing end:
ffb_tx.send(FfbCommand::Kill).ok();
```

---

## 4. AC Shared Memory — acpmf_graphics Fields

### 4.1 Memory Mapped File Access

Assetto Corsa writes telemetry to three Windows memory-mapped files:

| File Name | Rust MMF name | Contents |
|-----------|---------------|----------|
| `Local\acpmf_physics` | Physics: suspension, tyre, G-forces | Per-frame physics |
| `Local\acpmf_graphics` | **Session timing, lap data, race state** | 200ms cadence |
| `Local\acpmf_static` | Car/track static info, max RPM | Written at session start |

**Rust access pattern:**

```rust
use windows::Win32::System::Memory::*;
use windows::Win32::Foundation::*;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

unsafe fn open_ac_graphics() -> Option<*const AcGraphics> {
    let name: Vec<u16> = OsStr::new("Local\\acpmf_graphics")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let handle = OpenFileMappingW(FILE_MAP_READ.0, false, name.as_ptr());
    if handle.is_invalid() { return None; }
    let ptr = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0);
    if ptr.Value.is_null() { return None; }
    Some(ptr.Value as *const AcGraphics)
}
```

### 4.2 acpmf_graphics (SPageFileGraphic) — Complete Field List

Struct alignment: `#[repr(C, packed(4))]` (matches `#pragma pack(4)` in the SDK).

```rust
#[repr(C, packed(4))]
pub struct AcGraphics {
    // --- Packet header ---
    pub packet_id: i32,               // offset 0: incrementing sequence number

    // --- Session state ---
    pub status: i32,                  // 4: AC_STATUS: 0=OFF 1=REPLAY 2=LIVE 3=PAUSE
    pub session: i32,                 // 8: AC_SESSION_TYPE: 0=Practice 1=Qualify 2=Race ...

    // --- Formatted time strings (UTF-16, 15 chars each = 30 bytes each) ---
    pub current_time: [u16; 15],      // 12: current lap time string "1:23.456"
    pub last_time: [u16; 15],         // 42: last lap time string
    pub best_time: [u16; 15],         // 72: best lap time string
    pub split: [u16; 15],             // 102: split string (sector split)

    // --- Integer timing (milliseconds) ---
    pub completed_laps: i32,          // 132: laps completed this session
    pub position: i32,                // 136: position in race (1-based)
    pub i_current_time: i32,          // 140: current lap time ms (live, updates continuously)
    pub i_last_time: i32,             // 144: last completed lap time ms
    pub i_best_time: i32,             // 148: best lap time ms

    // --- Session time ---
    pub session_time_left: f32,       // 152: seconds remaining in timed session (-1 if laps-based)
    pub distance_traveled: f32,       // 156: meters traveled this lap

    // --- Pit status ---
    pub is_in_pit: i32,               // 160: 0=track 1=pit lane
    pub current_sector_index: i32,    // 164: 0=S1, 1=S2, 2=S3 (sector the car is currently IN)
    pub last_sector_time: i32,        // 168: last sector split time ms
    pub number_of_laps: i32,          // 172: total laps in session (race length), -1 if timed

    // --- Tyre compound string ---
    pub tyre_compound: [u16; 33],     // 176: active compound name (wide string)

    // --- Camera/replay ---
    pub replay_time_multiplier: f32,  // 242: replay speed multiplier
    pub normalized_car_position: f32, // 246: 0..1 normalized track position

    // --- Active cars (multiplayer/AI) ---
    pub car_coordinates: [f32; 3],    // 250: x,y,z of the player car (NOT used for HUD)
}
```

**Byte offsets verified against Python ctypes implementations** (ac_dashboard, deltabar, acc-extension-apps/sim_info.py). The struct is packed(4) so there is no padding between the u16 arrays and i32 fields.

### 4.3 Fields for HUD Use

| Field | Type | HUD Use |
|-------|------|---------|
| `i_current_time` | `i32` (ms) | Live lap timer — poll every frame |
| `i_last_time` | `i32` (ms) | Previous lap time |
| `i_best_time` | `i32` (ms) | Personal best this session |
| `last_sector_time` | `i32` (ms) | Last completed sector time |
| `current_sector_index` | `i32` | Which sector (0/1/2) — use to assign sector times |
| `session_time_left` | `f32` (s) | Session countdown (-1 = lap count mode) |
| `completed_laps` | `i32` | Lap counter |
| `status` | `i32` | Gate to skip rendering when `status != 2 (LIVE)` |
| `is_in_pit` | `i32` | Show "PIT" indicator in HUD |

### 4.4 Sector Time Reconstruction

AC does not expose S1/S2/S3 separately in acpmf_graphics. It only provides `last_sector_time` and `current_sector_index`. The pattern to reconstruct sector splits:

```rust
// In overlay state, track:
let mut sector_times: [Option<u32>; 3] = [None, None, None];
let mut last_sector_index: i32 = -1;

// On each poll (200ms):
let g = read_ac_graphics();
let new_sector = g.current_sector_index;  // 0, 1, or 2

if new_sector != last_sector_index && last_sector_index >= 0 {
    // Sector changed: last_sector_time is the time for last_sector_index
    sector_times[last_sector_index as usize] = Some(g.last_sector_time as u32);
}
last_sector_index = new_sector;

// On lap completion (i_last_time changes):
// Sector 2 (S3) time = i_last_time - sector_times[0] - sector_times[1]
let s3 = if let (Some(s1), Some(s2)) = (sector_times[0], sector_times[1]) {
    let total = g.i_last_time as u32;
    if total > s1 + s2 { Some(total - s1 - s2) } else { None }
} else {
    None
};
```

**Alternative:** Use `acpmf_physics.last_ff` or the UDP telemetry (port 9996 for AC) which does provide live sector splits. The UDP protocol is already implemented in `crates/rc-agent/src/sims/`.

### 4.5 AC Shared Memory vs UDP Telemetry for Sector Times

| Source | Sector Times | Update Rate | Reliability |
|--------|-------------|-------------|-------------|
| acpmf_graphics | Indirect (reconstruct) | 200ms | Accurate but requires reconstruction |
| AC UDP (port 9996) | Direct `sector_time` per lap event | Per-lap packet | Simpler, already implemented |

**Recommendation:** For sector times in the HUD overlay, use the existing UDP ingestion in `rc-agent` (already parsing AC telemetry at 9996) rather than adding a second shared memory reader. Only add acpmf_graphics polling if you need data the UDP protocol doesn't provide (e.g., `status`, `is_in_pit`, `session_time_left`).

### 4.6 acpmf_physics — Additional Fields (if needed)

```rust
#[repr(C, packed(4))]
pub struct AcPhysics {
    pub packet_id: i32,
    pub gas: f32,         // 0..1 throttle
    pub brake: f32,       // 0..1 brake
    pub fuel: f32,        // liters
    pub gear: i32,        // 0=R, 1=N, 2=1st, ...
    pub rpms: i32,        // current RPM
    pub steer_angle: f32, // degrees
    pub speed_kmh: f32,   // speed in km/h
    // ... 50+ more physics fields
}
```

Fields `gear`, `rpms`, `speed_kmh` are in acpmf_physics — but the UDP stream already provides all of these to the existing telemetry pipeline.

### 4.7 acpmf_static — Session Setup (read once)

```rust
#[repr(C, packed(4))]
pub struct AcStatic {
    pub sm_version: [u16; 15],       // shared mem version
    pub ac_version: [u16; 15],       // AC version string
    pub number_of_sessions: i32,
    pub num_cars: i32,
    pub car_model: [u16; 33],        // car identifier
    pub track: [u16; 33],            // track identifier
    pub player_name: [u16; 33],
    pub player_surname: [u16; 33],
    pub player_nick: [u16; 33],
    pub sector_count: i32,           // number of sectors (usually 3)
    pub max_torque: f32,
    pub max_power: f32,
    pub max_rpm: i32,                // IMPORTANT: use this for RPM bar scaling
    pub max_fuel: f32,
    // ... more car class info
}
```

`max_rpm` from acpmf_static is the correct redline for RPM bar scaling. The current code hardcodes `18000` — this should come from acpmf_static at session start.

---

## 5. Synthesis: Recommended Approaches

### 5.1 HUD Redesign (Milestone 1)

**Approach: Stay on GDI, targeted improvements only**

1. Swap all time/number fonts from `"Segoe UI"` to `"Consolas"` (monospace prevents layout jitter).
2. Increase gear font: 32→42pt, lap time: 22→28pt.
3. Fetch `max_rpm` from acpmf_static at session start — replace hardcoded `18000`.
4. Fetch `session_time_left` from acpmf_graphics to show timed session countdown (currently unused).
5. Add `is_in_pit` badge (already have `isInPit` in data model — just not displayed).
6. Add driver name display in top-left section (it's in `OverlayData::driver_name` but not shown).

**New shared memory module:** `crates/rc-agent/src/ac_shm.rs` — single file, reads acpmf_graphics + acpmf_static via `OpenFileMappingW` + `MapViewOfFile`. Poll at 200ms (same as repaint interval — one goroutine).

**If Direct2D is desired later:** The window creation code in `overlay.rs` stays unchanged. Only `paint_hud()` is replaced. The HWND-based structure is the same entry point for a `ID2D1DCRenderTarget`.

### 5.2 FFB Safety Kill-Switch (Milestone 2)

**Approach: Vendor HID interface, dedicated thread**

1. Add crate dependency: `hidapi = { version = "2.6", features = ["windows-native"] }` to `rc-agent/Cargo.toml`.
2. Create `crates/rc-agent/src/wheelbase_guard.rs` — implement `WheelbaseGuard` with `open()`, `emergency_stop()`, `enable_ffb()`.
3. Wire to billing events in `rc-agent/src/main.rs`:
   - `BillingState::Ended | BillingState::Idle` → send `FfbCommand::Kill`
   - `BillingState::Active` → send `FfbCommand::Enable`
4. The guard's `Drop` impl fires `emergency_stop()` on process exit — this is the crash-safe path.
5. On USB reconnect, the watchdog thread should re-open and re-send the current state.

**Important:** The Conspit Ares may share its USB hub with the gaming PC's USB controller. If the pod restarts mid-session, the `HidDevice` will error. Implement reconnect with exponential backoff (500ms → 1s → 2s → …up to 30s).

### 5.3 Crate Dependencies Summary

For `crates/rc-agent/Cargo.toml`:

```toml
[dependencies]
# Existing
winapi = { version = "0.3", features = ["wingdi", "winuser", "libloaderapi", "memoryapi"] }

# New for FFB safety
hidapi = { version = "2.6", features = ["windows-native"] }

# If migrating to Direct2D (future)
# windows = { version = "0.58", features = [
#     "Win32_Graphics_Direct2D",
#     "Win32_Graphics_Direct2D_Common",
#     "Win32_Graphics_DirectWrite",
#     "Win32_Graphics_Gdi",
#     "Win32_UI_WindowsAndMessaging",
#     "Win32_Foundation",
#     "Win32_System_Memory",           # for MapViewOfFile
# ] }
```

**Note on winapi vs windows-rs:** The existing code uses `winapi` crate. Adding `windows` crate for new features is fine — both can coexist. But the `memoryapi` feature needs to be added to winapi for `OpenFileMappingW` / `MapViewOfFile` if we keep using winapi for the shared memory module.

```toml
winapi = { version = "0.3", features = [
    "wingdi", "winuser", "libloaderapi",
    "memoryapi",     # add this for MapViewOfFile / OpenFileMappingW
    "handleapi",     # for CloseHandle
] }
```

### 5.4 Open Questions for Roadmap

1. **Rev counter arc:** Do we want it? If yes, budget Direct2D migration (2-3 days). If no, GDI improvements are 0.5 days.
2. **acpmf_graphics polling vs UDP:** Confirm whether sector times from UDP are accurate enough or if we need the shared memory `last_sector_time` field for better precision.
3. **FFB kill scope:** Does the kill apply to all 8 pods simultaneously (via pod-agent relay) or just the per-pod rc-agent? Probably per-pod since the wheelbase is local USB.
4. **estop persistence:** Should we write the `estop` state to the toml config so it survives rc-agent restart? Probably not — default is FFB active, estop only during billing gap.
5. **Multi-interface handling:** On some systems, Windows may claim the OpenFFBoard PID gamepad interface exclusively via the HID class driver, but the vendor interface (`usage_page=0xFF00`) should always remain accessible from userspace.

---

## Sources

- [OpenFFBoard GitHub](https://github.com/Ultrawipf/OpenFFBoard)
- [OpenFFBoard Commands Wiki](https://github.com/Ultrawipf/OpenFFBoard/wiki/Commands)
- [OpenFFBoard Doxygen — HID Class Driver](https://ultrawipf.github.io/OpenFFBoard/doxygen/group___class_driver___h_i_d.html)
- [pid.codes — VID 0x1209 PID 0xFFB0](https://pid.codes/1209/FFB0/)
- [hidapi Rust docs — HidDevice](https://docs.rs/hidapi/latest/hidapi/struct.HidDevice.html)
- [hidapi-rs GitHub](https://github.com/ruabmbua/hidapi-rs)
- [USB HID PID v1.0 Specification](https://www.usb.org/sites/default/files/documents/pid1_01.pdf)
- [Linux hid-pidff.c — zanppa fork](https://github.com/zanppa/hid-pidff)
- [Linux universal-pidff](https://github.com/JacKeTUs/universal-pidff)
- [AC Shared Memory Reference — assettocorsamods.net](https://assettocorsamods.net/threads/doc-shared-memory-reference.58/)
- [AC sim_info.py — ac_dashboard](https://github.com/ev-agelos/ac_dashboard/blob/master/sim_info.py)
- [AC sim_info.py — acc-extension-apps](https://github.com/ac-custom-shaders-patch/acc-extension-apps/blob/master/apps/python/AccExtHelper/sim_info.py)
- [AC sim_info.py — deltabar](https://github.com/jamessanford/assetto-corsa-deltabar/blob/master/deltabar/deltabar_lib/sim_info.py)
- [AC SimInfo Docs Markdown](https://github.com/dcratliff19/AC-SimInfo-Docs-Markdown)
- [Microsoft Learn — ClearType Antialiasing](https://learn.microsoft.com/en-us/windows/win32/gdi/cleartype-antialiasing)
- [microsoft/windows-docs-rs — Direct2D](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/Direct2D/index.html)
- [microsoft/windows-docs-rs — DirectWrite](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/DirectWrite/index.html)
- [aero-overlay — ReactiioN1337](https://github.com/ReactiioN1337/aero-overlay)
- [hudhook — veeenu (Rust overlay framework)](https://github.com/veeenu/hudhook)
- [What USB protocol do racing wheels use?](https://www.overtake.gg/threads/what-usb-protocol-do-racing-wheels-use-for-force-feedback.263162/)
