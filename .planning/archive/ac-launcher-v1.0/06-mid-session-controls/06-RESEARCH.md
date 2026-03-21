# Phase 6: Mid-Session Controls - Research

**Researched:** 2026-03-14
**Domain:** AC runtime assist modification, OpenFFBoard HID gain control, Windows keyboard simulation, overlay toast notifications
**Confidence:** HIGH

## Summary

Mid-session controls require three distinct technical approaches: (1) AC driving assists (ABS, TC, transmission) are changeable mid-session via built-in keyboard shortcuts that AC already supports -- Ctrl+A cycles ABS (off, 1-4), Ctrl+T cycles TC (off, 1-4), Ctrl+G toggles auto-shifter. The rc-agent can send these via Windows `SendInput` API. (2) Stability control has NO keyboard shortcut and NO runtime toggle in AC -- it is an INI-only pre-session setting. Per the user's decision ("if research shows a specific assist cannot change instantly mid-drive, skip it entirely"), stability control MUST be excluded from the UI. (3) FFB intensity goes direct to the OpenFFBoard wheelbase via HID vendor commands, using Axis class (0xA01) power command (CMD 0x00) which sets overall force strength as a 16-bit value.

The existing codebase already has: WebSocket protocol variants (SetTransmission, SetFfb) in protocol.rs, API routes (`POST /pods/{pod_id}/transmission` and `/pods/{pod_id}/ffb`) in rc-core, agent-side handling in main.rs, and INI-writing functions. Phase 6 replaces the INI-writing approach with instant runtime approaches: SendInput for assists, HID for FFB. The overlay system (native Win32 GDI) needs toast notification support added. The PWA active session page needs a bottom sheet with toggles and slider.

**Primary recommendation:** Use SendInput keyboard simulation for ABS/TC/transmission (proven AC keyboard shortcuts), direct HID for FFB gain, exclude stability control entirely, extend existing overlay with toast capability.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Changes must be instant while driving -- no pit stop or restart required
- If research shows a specific assist cannot change instantly mid-drive, skip that assist entirely -- don't offer it in the UI
- Only show controls that actually work mid-session. No greyed-out or disabled toggles.
- FFB intensity changes go direct to the OpenFFBoard wheelbase via HID -- bypass AC's INI entirely. FfbController already has the HID interface open.
- Assist changes need research into AC's runtime behavior -- shared memory writes, CSP console commands, or AC API if available
- Percentage slider from 10% to 100% -- no presets, just a clean slider
- Minimum floor: 10% (prevents zero FFB which feels like a broken wheel)
- Initial slider position matches the launch setting -- if customer launched with medium (70%), slider starts at 70
- 500ms debounce on slider changes -- only send HID command after 500ms of no movement to prevent flooding the wheelbase
- FFB gain sent via OpenFFBoard HID vendor commands (extends FfbController beyond safety-only usage)
- Bottom sheet / drawer on the active session screen -- pull up to reveal, dismiss to hide
- Opened via a gear icon button on the session screen (visible, discoverable)
- Changes apply instantly on toggle -- no "Apply" button needed
- Drawer shows actual pod state when opened -- PWA queries rc-core for current assist/FFB values, not cached last-sent values
- Toggles for assists (ON/OFF), slider for FFB intensity
- Both HUD overlay AND PWA confirmation -- double confirmation, no ambiguity
- HUD overlay: brief toast at top-center of pod screen (e.g., "ABS: OFF" or "FFB: 85%")
- Toast duration: 3 seconds
- Toast behavior: replace (latest change wins) -- no stacking or queueing
- PWA shows confirmation inline next to the control that changed
- Uses existing overlay system from Phase 3 (billing timer overlay)

### Claude's Discretion
- Exact WebSocket message types for mid-session control commands
- How to read current assist state from the pod (shared memory, INI read, or cached in rc-agent)
- OpenFFBoard HID command for setting FFB gain (vs safety estop which is already implemented)
- Overlay toast styling and animation (fade in/out, color scheme)
- Whether to send individual assist changes or batch them

### Deferred Ideas (OUT OF SCOPE)
- Mid-session AI difficulty adjustment (change AI_LEVEL while racing) -- separate capability, not in Phase 6 scope
- Weather/condition changes mid-session -- future feature
- Camera angle presets from PWA -- out of scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| DIFF-06 | Customer can toggle transmission auto/manual mid-session while driving | AC supports Ctrl+G to toggle auto-shifter mid-session. Use SendInput to simulate this keystroke. Readable via shared memory `autoShifterOn` at physics offset 264. |
| DIFF-07 | Customer can toggle ABS on/off mid-session | AC supports Ctrl+A to cycle ABS levels (off, 1-4). Use SendInput for Ctrl+A. Readable via shared memory `abs` (float) at physics offset 252. |
| DIFF-08 | Customer can toggle traction control on/off mid-session | AC supports Ctrl+T to cycle TC levels (off, 1-4). Use SendInput for Ctrl+T. Readable via shared memory `tc` (float) at physics offset 204. |
| DIFF-09 | Customer can toggle stability control on/off mid-session | CANNOT BE IMPLEMENTED. AC has no keyboard shortcut or runtime mechanism for stability control. It is INI-only, pre-session. Per user decision: skip entirely, do not show in UI. |
| DIFF-10 | Customer can adjust force feedback intensity mid-session | OpenFFBoard Axis class (0xA01) power command (CMD 0x00) sets overall force strength via HID. Extend FfbController with set_gain(). 500ms debounce, 10-100% range mapped to 16-bit HID value. |
</phase_requirements>

## Standard Stack

### Core (already in codebase)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| winapi | 0.3 | SendInput for keyboard simulation, Win32 overlay | Already a dependency; winuser feature includes SendInput |
| hidapi | 2 | OpenFFBoard HID vendor commands | Already used by FfbController for estop |
| tokio-tungstenite | 0.26 | WebSocket agent-core communication | Already used for all CoreToAgentMessage transport |
| serde/serde_json | workspace | Protocol serialization | Already used for all message types |

### Supporting (already in codebase)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| dirs-next | 2 | AC Documents path resolution | Reading assists.ini for initial state |
| Next.js | (PWA) | Customer-facing PWA | Bottom sheet UI, active session controls |
| Tailwind CSS | (PWA) | Styling | Bottom sheet drawer, toggle switches, slider |

### No New Dependencies Needed
The existing Cargo.toml already includes all required crates. The winapi dependency with "winuser" feature provides `SendInput`, `INPUT`, `KEYBDINPUT`, `KEYEVENTF_KEYUP`, `VK_CONTROL`. No new npm packages are needed for the PWA -- bottom sheet, toggles, and sliders are achievable with Tailwind CSS and standard React state.

**Installation:**
```bash
# No new packages needed -- all dependencies already present
cargo test -p rc-agent   # verify existing tests pass
cargo test -p rc-common  # verify protocol tests pass
```

## Architecture Patterns

### How AC Assists Change Mid-Session

**Key finding (HIGH confidence):** Assetto Corsa has built-in keyboard shortcuts for mid-session assist changes:

| Assist | Keyboard Shortcut | Behavior | Shared Memory Field | Offset |
|--------|-------------------|----------|---------------------|--------|
| ABS | Ctrl+A / Ctrl+Shift+A | Cycles: off, 1, 2, 3, 4 | `abs` (float) | 252 |
| Traction Control | Ctrl+T / Ctrl+Shift+T | Cycles: off, 1, 2, 3, 4 | `tc` (float) | 204 |
| Auto Shifter | Ctrl+G | Toggles on/off | `autoShifterOn` (int) | 264 |
| Stability Control | **NONE** | **INI-only, pre-session** | `aidStability` (static) | N/A |

**Sources:** [Steam guide (keyboard commands)](https://steamcommunity.com/sharedfiles/filedetails/?id=457373702), [AC shared memory reference](https://assettocorsamods.net/threads/doc-shared-memory-reference.58/)

**Critical finding on DIFF-09:** Stability control has no keyboard shortcut and no runtime mechanism. The `aidStability` value is in the Static shared memory (set once per session). One community report confirms it always reads as 0 regardless of actual setting. Per the locked user decision, this MUST be excluded from the UI.

### How FFB Gain Changes Via HID

**OpenFFBoard Axis Power Command:**
- Class: `0xA01` (Axis)
- CMD: `0x00` (power -- "Overall force strength")
- Data: 16-bit value (i64 in the HID report, but meaningful range is 16b)
- Read/Write: both supported (CMD_TYPE_WRITE=0 for set, CMD_TYPE_READ=1 for get)

The existing `send_vendor_cmd()` in `ffb_controller.rs` sends to class `0x00A1` (FFBWheel) for estop. For gain control, we need to send to class `0x0A01` (Axis) with CMD `0x00` (power).

**Mapping percentage to HID value:**
- The power value is 16-bit (0-65535)
- 10% = 6553, 100% = 65535
- Formula: `value = (percentage as i64 * 65535) / 100`
- The existing `send_vendor_cmd()` method takes `cmd_id: u32` and `data: i64` -- we change `CLASS_FFBWHEEL` to `CLASS_AXIS` for gain commands

**Sources:** [OpenFFBoard Commands wiki](https://github.com/Ultrawipf/OpenFFBoard/wiki/Commands), [OpenFFBoard Configurator guide](https://github.com/Ultrawipf/OpenFFBoard/wiki/Configurator-guide)

### SendInput Pattern for Keyboard Simulation

```rust
// Source: winapi crate docs + Windows SendInput documentation
use winapi::um::winuser::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT,
    KEYEVENTF_KEYUP, VK_CONTROL,
};

/// Send a Ctrl+key combination to the foreground window (AC).
/// AC must be the foreground window for this to work.
fn send_ctrl_key(vk_key: u16) {
    unsafe {
        let mut inputs: [INPUT; 4] = std::mem::zeroed();

        // Ctrl down
        inputs[0].type_ = INPUT_KEYBOARD;
        *inputs[0].u.ki_mut() = KEYBDINPUT {
            wVk: VK_CONTROL as u16,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };

        // Key down
        inputs[1].type_ = INPUT_KEYBOARD;
        *inputs[1].u.ki_mut() = KEYBDINPUT {
            wVk: vk_key,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };

        // Key up
        inputs[2].type_ = INPUT_KEYBOARD;
        *inputs[2].u.ki_mut() = KEYBDINPUT {
            wVk: vk_key,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };

        // Ctrl up
        inputs[3].type_ = INPUT_KEYBOARD;
        *inputs[3].u.ki_mut() = KEYBDINPUT {
            wVk: VK_CONTROL as u16,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };

        SendInput(4, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}
```

### Data Flow: PWA to Pod

```
PWA (customer taps toggle)
  |
  | POST /pods/{pod_id}/assists  (new unified endpoint)
  | body: { "abs": true, "tc": false, "transmission": "manual", "ffb_percent": 85 }
  v
rc-core (validates, sends to agent)
  |
  | CoreToAgentMessage::SetAssists { ... } or SetFfbGain { percent: u8 }
  v
rc-agent (applies change)
  |
  |-- Assists: SendInput(Ctrl+A / Ctrl+T / Ctrl+G) to AC window
  |-- FFB: send_vendor_cmd(CLASS_AXIS, CMD_POWER, gain_value) to wheelbase
  |
  | Read shared memory to confirm actual state
  | Send overlay toast
  |
  | AgentMessage::AssistStateUpdate { pod_id, abs, tc, auto_shifter, ffb_percent }
  v
rc-core (updates cached state, responds to PWA)
  |
  | Response: { "ok": true, "abs": 1, "tc": 0, "auto_shifter": true, "ffb_percent": 85 }
  v
PWA (updates UI with confirmed state)
```

### Recommended Message Design (Claude's Discretion)

**Individual commands, not batched.** Each assist toggle is independent. Reasons:
- Simpler error handling (one change fails, others succeed)
- SendInput needs spacing between keystrokes (can't blast 3 Ctrl+combos simultaneously)
- FFB goes to HID, assists go to SendInput -- different transports

**New protocol variants:**

```rust
// CoreToAgentMessage additions:
SetAssist {
    assist_type: String, // "abs", "tc", "transmission"
    enabled: bool,       // true = on/auto, false = off/manual
},
SetFfbGain {
    percent: u8,         // 10-100
},
QueryAssistState,        // Request current state from agent

// AgentMessage additions:
AssistChanged {
    pod_id: String,
    assist_type: String,
    enabled: bool,
    confirmed: bool,     // true if shared memory confirms the change
},
FfbGainChanged {
    pod_id: String,
    percent: u8,
},
AssistState {            // Response to QueryAssistState
    pod_id: String,
    abs: u8,             // 0=off, 1-4=level
    tc: u8,              // 0=off, 1-4=level
    auto_shifter: bool,
    ffb_percent: u8,
},
```

### Reading Current Assist State (Claude's Discretion)

**Recommended approach: Read AC shared memory.**

The rc-agent already has `AssettoCorsaAdapter` with open shared memory handles. Add reading:

| Field | Memory | Offset | Type | Interpretation |
|-------|--------|--------|------|----------------|
| `abs` | physics | 252 | float | 0.0 = off, >0 = active level |
| `tc` | physics | 204 | float | 0.0 = off, >0 = active level |
| `autoShifterOn` | physics | 264 | i32 | 0 = manual, 1 = auto |

For FFB percent: not in shared memory. Cache the last-sent HID value in rc-agent state. On startup, read from `controls.ini` [FF] GAIN= value as initial.

### Overlay Toast (Claude's Discretion)

Extend `OverlayData` with:
```rust
toast_message: Option<String>,
toast_until: Option<std::time::Instant>,
```

In the paint routine, if `toast_message.is_some()` and `Instant::now() < toast_until`:
- Draw a centered rectangle at top-center of the HUD bar
- White text on semi-transparent dark background
- Text like "ABS: OFF" or "FFB: 85%"
- After 3 seconds, clear the toast

The 200ms repaint interval already handles auto-clearing. When a new toast arrives, it replaces the old one (per user decision: latest change wins, no stacking).

**Styling:** Use Racing Red (#E10600) background for the toast badge, white bold text. This matches the existing overlay aesthetic and makes it highly visible during gameplay.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Keyboard simulation | Raw PostMessage/WM_KEYDOWN | `SendInput` via winapi | SendInput is the correct Win32 API for synthetic input; PostMessage/WM_KEYDOWN is unreliable with games that use DirectInput |
| AC assist change | Writing assists.ini while running | Keyboard shortcuts (Ctrl+A/T/G) via SendInput | AC does NOT hot-reload INI files mid-session; keyboard shortcuts are the game's own built-in mechanism |
| FFB gain control | AC's controls.ini GAIN= | Direct OpenFFBoard HID axis power | AC's GAIN requires restart; HID is instant and bypasses game entirely |
| Bottom sheet UI | Custom CSS animation | CSS `transform: translateY()` with transition | Standard mobile bottom sheet pattern with Tailwind |
| Slider debounce | Custom timer logic | `setTimeout`/`clearTimeout` pattern | Standard JS debounce; 500ms per user decision |

**Key insight:** The critical insight for this phase is that AC's INI files are read at launch and NOT hot-reloaded. The existing `set_transmission()` and `set_ffb()` functions in ac_launcher.rs write INIs but changes only apply on next launch. Phase 6 MUST use runtime mechanisms (keyboard shortcuts for assists, HID for FFB).

## Common Pitfalls

### Pitfall 1: Stability Control Cannot Change Mid-Session
**What goes wrong:** Implementing a stability control toggle that writes to INI but has no effect on the running game. Customer toggles it, nothing happens, trust is broken.
**Why it happens:** Stability control has no keyboard shortcut in AC. Unlike ABS/TC/transmission, it can only be set pre-session via assists.ini.
**How to avoid:** Per user decision: exclude stability control entirely from the UI. DIFF-09 should be marked as "cannot implement -- excluded by design."
**Warning signs:** If someone tries to add a stability toggle, check shared memory -- `aidStability` is in Static (set once), not Physics (updated per-frame).

### Pitfall 2: SendInput Requires Foreground Window
**What goes wrong:** Ctrl+A/T/G sent via SendInput goes to whatever window is in focus, not AC.
**Why it happens:** `SendInput` sends to the foreground window. If the overlay or lock screen steals focus, the keystrokes miss AC.
**How to avoid:** Before sending keyboard input, verify AC (acs.exe) is the foreground window. Use `GetForegroundWindow()` + `GetWindowThreadProcessId()` to check. If AC is not foreground, call `SetForegroundWindow()` or `BringWindowToTop()` first. The existing `bring_game_to_foreground()` function in ac_launcher.rs already does this.
**Warning signs:** Assist toggles work sometimes but not always. Look for overlay/lock-screen window stealing focus.

### Pitfall 3: ABS/TC Cycles Through Levels, Not Simple Toggle
**What goes wrong:** Customer expects a simple ON/OFF toggle, but Ctrl+A sends them through 5 states (off, 1, 2, 3, 4).
**Why it happens:** AC's ABS and TC have multiple levels. Ctrl+A/Ctrl+Shift+A cycle up/down through all levels.
**How to avoid:** For the simple toggle UX the user specified (ON/OFF): read current state from shared memory, determine if it's off (0) or on (any level). If toggling ON, send Ctrl+A once (goes to level 1). If toggling OFF from level 3, send Ctrl+Shift+A three times (cycling down through 2, 1, off). Alternative simpler approach: always send enough Ctrl+Shift+A to reach 0 (off), then if turning ON send one Ctrl+A. Read shared memory after each send to confirm.
**Warning signs:** Customer presses "OFF" but sees "ABS: 3" because only one cycle was sent.

### Pitfall 4: FFB HID Class ID Confusion
**What goes wrong:** Sending the gain command to class 0x00A1 (FFBWheel) instead of class 0x0A01 (Axis). The estop command works on FFBWheel, but power/gain is on Axis.
**Why it happens:** The existing code uses `CLASS_FFBWHEEL: u16 = 0x00A1` for safety commands. The gain command needs `CLASS_AXIS: u16 = 0x0A01`.
**How to avoid:** Add a new constant `CLASS_AXIS: u16 = 0x0A01` and a new method `set_gain()` that uses it. Keep `zero_force()` and `estop()` on FFBWheel class unchanged.
**Warning signs:** HID write succeeds but FFB doesn't change. The wheelbase ignores commands sent to the wrong class.

### Pitfall 5: 500ms Debounce Flooding Prevention
**What goes wrong:** User rapidly slides FFB from 50% to 100%, generating 50+ HID commands that flood the wheelbase USB interface.
**Why it happens:** Slider `onChange` fires on every pixel of movement. Without debounce, each change triggers a WebSocket message and HID write.
**How to avoid:** Debounce in the PWA (500ms timer, reset on each slider move, only send on timer expiry). The PWA should update the slider position visually immediately (local state) but only POST to rc-core after 500ms of no movement.
**Warning signs:** Wheelbase becomes unresponsive during rapid slider movement; HID write errors in rc-agent logs.

### Pitfall 6: Overlay Toast Flicker With GDI Repaint
**What goes wrong:** Toast appears and disappears rapidly on each 200ms repaint cycle, causing flicker.
**Why it happens:** The GDI paint routine clears the entire window on each repaint. If toast rendering isn't consistent, it flickers.
**How to avoid:** Use `Instant::now() < toast_until` check consistently. Store the toast message and expiry in the shared OverlayData behind the Mutex. The paint routine reads it atomically -- either the toast is visible or not, never half-rendered.
**Warning signs:** Rapid blinking of toast text during the 3-second display period.

## Code Examples

### Extending FfbController with set_gain()

```rust
// Source: OpenFFBoard wiki Commands page + existing ffb_controller.rs pattern

/// Axis class ID (little-endian u16) -- for gain/power commands
const CLASS_AXIS: u16 = 0x0A01;

/// Axis command: power (overall force strength)
const CMD_POWER: u32 = 0x00;

impl FfbController {
    /// Set FFB gain as a percentage (10-100).
    /// Sends to OpenFFBoard Axis class power command.
    /// Returns Ok(true) if sent, Ok(false) if device not found.
    pub fn set_gain(&self, percent: u8) -> Result<bool, String> {
        let percent = percent.clamp(10, 100);
        let device = match self.open_vendor_interface() {
            Some(dev) => dev,
            None => return Ok(false),
        };

        // Map percentage to 16-bit HID value
        let value = (percent as i64 * 65535) / 100;

        // Send to Axis class (0x0A01), not FFBWheel class
        let mut buf = [0u8; 26];
        buf[0] = REPORT_ID;
        buf[1] = CMD_TYPE_WRITE;
        buf[2..4].copy_from_slice(&CLASS_AXIS.to_le_bytes());
        buf[4] = 0; // instance
        buf[5..9].copy_from_slice(&CMD_POWER.to_le_bytes());
        buf[9..17].copy_from_slice(&value.to_le_bytes());
        buf[17..25].copy_from_slice(&0i64.to_le_bytes());

        device
            .write(&buf)
            .map(|_| {
                tracing::info!("FFB: gain set to {}% (HID value: {})", percent, value);
                true
            })
            .map_err(|e| format!("HID write failed: {}", e))
    }
}
```

### SendInput for Ctrl+A (ABS toggle)

```rust
// Source: winapi crate SendInput docs + AC keyboard reference
#[cfg(windows)]
pub fn toggle_ac_abs() {
    use winapi::um::winuser::*;

    unsafe {
        let mut inputs: [INPUT; 4] = std::mem::zeroed();

        // Ctrl down
        inputs[0].type_ = INPUT_KEYBOARD;
        *inputs[0].u.ki_mut() = KEYBDINPUT {
            wVk: VK_CONTROL as u16,
            ..std::mem::zeroed()
        };
        // 'A' down (VK_A = 0x41)
        inputs[1].type_ = INPUT_KEYBOARD;
        *inputs[1].u.ki_mut() = KEYBDINPUT {
            wVk: 0x41,
            ..std::mem::zeroed()
        };
        // 'A' up
        inputs[2].type_ = INPUT_KEYBOARD;
        *inputs[2].u.ki_mut() = KEYBDINPUT {
            wVk: 0x41,
            dwFlags: KEYEVENTF_KEYUP,
            ..std::mem::zeroed()
        };
        // Ctrl up
        inputs[3].type_ = INPUT_KEYBOARD;
        *inputs[3].u.ki_mut() = KEYBDINPUT {
            wVk: VK_CONTROL as u16,
            dwFlags: KEYEVENTF_KEYUP,
            ..std::mem::zeroed()
        };

        SendInput(4, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}
```

### Reading Assist State from AC Shared Memory

```rust
// Source: AC shared memory reference + existing assetto_corsa.rs pattern
// Physics struct offsets (pack=4):
mod physics_assists {
    pub const TC: usize = 204;              // float, 0.0=off, >0=active
    pub const ABS: usize = 252;             // float, 0.0=off, >0=active
    pub const AUTO_SHIFTER_ON: usize = 264; // i32, 0=manual, 1=auto
}

// Read in AssettoCorsaAdapter:
pub fn read_assist_state(&self) -> Option<AssistState> {
    let ph = self.physics_handle.as_ref()?;
    let abs_val = Self::read_f32(ph, physics_assists::ABS);
    let tc_val = Self::read_f32(ph, physics_assists::TC);
    let auto_shifter = Self::read_i32(ph, physics_assists::AUTO_SHIFTER_ON);

    Some(AssistState {
        abs: if abs_val > 0.0 { (abs_val as u8).max(1) } else { 0 },
        tc: if tc_val > 0.0 { (tc_val as u8).max(1) } else { 0 },
        auto_shifter: auto_shifter != 0,
    })
}
```

### PWA Bottom Sheet (Tailwind CSS)

```tsx
// Pattern: CSS transform-based bottom sheet
const [sheetOpen, setSheetOpen] = useState(false);

<button onClick={() => setSheetOpen(true)} className="fixed bottom-20 right-4 ...">
  <GearIcon />
</button>

<div className={`fixed inset-x-0 bottom-0 z-50 transform transition-transform duration-300
  ${sheetOpen ? 'translate-y-0' : 'translate-y-full'}`}>
  <div className="bg-rp-card border-t border-rp-border rounded-t-2xl p-6">
    {/* Drag handle */}
    <div className="w-12 h-1 bg-rp-grey/50 rounded-full mx-auto mb-4" />

    {/* ABS Toggle */}
    <div className="flex items-center justify-between mb-4">
      <span className="text-white">ABS</span>
      <Toggle checked={absOn} onChange={() => toggleAssist('abs')} />
    </div>

    {/* TC Toggle */}
    <div className="flex items-center justify-between mb-4">
      <span className="text-white">Traction Control</span>
      <Toggle checked={tcOn} onChange={() => toggleAssist('tc')} />
    </div>

    {/* Transmission Toggle */}
    <div className="flex items-center justify-between mb-4">
      <span className="text-white">Auto Transmission</span>
      <Toggle checked={autoTrans} onChange={() => toggleAssist('transmission')} />
    </div>

    {/* FFB Slider */}
    <div className="mt-6">
      <span className="text-white text-sm">Force Feedback: {ffbPercent}%</span>
      <input type="range" min="10" max="100" value={ffbPercent}
        onChange={(e) => handleFfbChange(Number(e.target.value))}
        className="w-full mt-2 accent-rp-red" />
    </div>
  </div>
</div>
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Write assists.ini + restart | AC keyboard shortcuts (Ctrl+A/T/G) via SendInput | Always available in AC | Instant mid-session changes, no restart |
| AC controls.ini GAIN= | Direct OpenFFBoard HID power command | Phase 6 | Bypasses game entirely, instant response |
| Preset FFB (light/medium/strong) | Percentage slider (10-100%) | Phase 6 | Granular control vs 3 fixed presets |

**Existing code to replace/extend:**
- `set_transmission()` in ac_launcher.rs -- currently writes INI. Phase 6 adds `toggle_ac_transmission()` using SendInput for instant effect. Keep INI write for future launches.
- `set_ffb()` in ac_launcher.rs -- currently writes controls.ini. Phase 6 adds `FfbController::set_gain()` for instant HID. Keep INI write for future launches.
- `CoreToAgentMessage::SetTransmission` / `SetFfb` -- currently trigger INI writes. Replace handler with SendInput/HID calls.

## Open Questions

1. **OpenFFBoard power value range**
   - What we know: The power command is 16-bit ("16b" per wiki), sent as i64 in the HID report
   - What's unclear: Whether 65535 = 100% or if there's a different scaling. The Conspit Ares firmware may use a different range than stock OpenFFBoard.
   - Recommendation: On first deploy to Pod 8, test by sending known values (6553=10%, 32767=50%, 65535=100%) and verifying subjective FFB strength. If the range doesn't feel right, adjust the mapping. Read current value first with CMD_TYPE_READ to establish baseline.

2. **ABS/TC level mapping in shared memory**
   - What we know: The shared memory `abs` and `tc` fields are floats. Community docs say they exist.
   - What's unclear: Exact float values for each level (is level 1 = 1.0, level 4 = 4.0? Or normalized 0.0-1.0?). The community report about `aidStability` always being 0 raises questions about reliability.
   - Recommendation: On Pod 8, manually set ABS to each level (off, 1, 2, 3, 4) and log the shared memory float value. This is a 5-minute test that resolves the question.

3. **SendInput and anti-cheat/game focus**
   - What we know: AC is not an online competitive game in this context (LAN venue). SendInput should work. AC already accepts keyboard input for these shortcuts.
   - What's unclear: Whether CSP (Custom Shaders Patch) or Content Manager intercepts or modifies keyboard handling.
   - Recommendation: Test on Pod 8 before rolling out. If SendInput doesn't work, fallback to writing INI + showing "change takes effect next session" message.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p rc-agent -- --test-threads=1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DIFF-06 | Transmission toggle via SendInput | unit | `cargo test -p rc-agent -- toggle_transmission -x` | Wave 0 |
| DIFF-07 | ABS toggle via SendInput | unit | `cargo test -p rc-agent -- toggle_abs -x` | Wave 0 |
| DIFF-08 | TC toggle via SendInput | unit | `cargo test -p rc-agent -- toggle_tc -x` | Wave 0 |
| DIFF-09 | Stability control excluded | unit | `cargo test -p rc-agent -- stability_excluded -x` | Wave 0 |
| DIFF-10 | FFB gain via HID | unit | `cargo test -p rc-agent -- set_gain -x` | Wave 0 |
| N/A | Protocol serialization for new messages | unit | `cargo test -p rc-common -- mid_session -x` | Wave 0 |
| N/A | Toast overlay data management | unit | `cargo test -p rc-agent -- toast -x` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent && cargo test -p rc-common`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `rc-agent/src/ac_launcher.rs` -- tests for SendInput helper functions (buffer format, not actual keypress)
- [ ] `rc-agent/src/ffb_controller.rs` -- test for `set_gain()` HID buffer format (like existing `test_vendor_cmd_buffer_format`)
- [ ] `rc-common/src/protocol.rs` -- serialization tests for new SetAssist/SetFfbGain/AssistState variants
- [ ] `rc-agent/src/overlay.rs` -- tests for toast data management (set, expire, replace)
- [ ] `rc-agent/src/sims/assetto_corsa.rs` -- tests for assist state reading (offset correctness)

## Sources

### Primary (HIGH confidence)
- [Steam Guide: AC Keyboard Commands](https://steamcommunity.com/sharedfiles/filedetails/?id=457373702) -- Ctrl+A (ABS), Ctrl+T (TC), Ctrl+G (auto-shifter) confirmed
- [AC Shared Memory Reference (assettocorsamods.net)](https://assettocorsamods.net/threads/doc-shared-memory-reference.58/) -- Physics struct with abs, tc, autoShifterOn fields
- [AC telemetry struct reference (koscielniak.pro)](https://koscielniak.pro/knowledge/others/ac-telemetry.html) -- Full SPageFilePhysics layout with offsets
- [OpenFFBoard Commands Wiki](https://github.com/Ultrawipf/OpenFFBoard/wiki/Commands) -- Axis class 0xA01, power CMD 0x00
- [OpenFFBoard Configurator Guide](https://github.com/Ultrawipf/OpenFFBoard/wiki/Configurator-guide) -- Power is 16-bit total force
- [Windows SendInput documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput) -- Official API reference

### Secondary (MEDIUM confidence)
- [Stability control shared memory issue (assettocorsamods.net)](https://assettocorsamods.net/threads/stability-control-shared-memory-value-not-set.2969/) -- Confirms aidStability reads as 0
- [Steam discussion: changing shift type in-game](https://steamcommunity.com/app/244210/discussions/0/1648791520829970105/) -- Confirms Ctrl+G works mid-session
- [SendInput Rust example (GitHub Gist)](https://gist.github.com/littletsu/d1c1b512d6843071144b7b89109a8de2) -- Rust pattern for SendInput

### Tertiary (LOW confidence)
- OpenFFBoard power value range (65535) -- inferred from "16b" notation in wiki, needs Pod 8 validation
- AC shared memory abs/tc float value mapping -- needs Pod 8 validation to confirm exact values per level

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies already in codebase, no new crates needed
- Architecture (assists via keyboard): HIGH -- AC keyboard shortcuts are officially documented and community-verified
- Architecture (FFB via HID): MEDIUM -- OpenFFBoard command structure verified from wiki, but Conspit Ares power range needs Pod 8 testing
- Architecture (stability exclusion): HIGH -- no keyboard shortcut exists, multiple sources confirm
- Pitfalls: HIGH -- based on codebase analysis and AC community documentation
- Shared memory offsets: MEDIUM -- calculated from struct layout, confirmed by one external source, needs Pod 8 validation

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable -- AC and OpenFFBoard are mature platforms)
