# Phase 6: Mid-Session Controls - Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Customers can adjust driving assists (transmission, ABS, TC, SC) and force feedback intensity while actively driving, via PWA — without restarting the session. Only controls that can change instantly mid-drive are offered. FFB changes go direct to the OpenFFBoard wheelbase via HID. Assist changes use whatever AC mechanism supports instant application. Controls that can't change instantly are excluded from the UI entirely.

</domain>

<decisions>
## Implementation Decisions

### When Changes Take Effect
- Changes must be **instant while driving** — no pit stop or restart required
- If research shows a specific assist (ABS, TC, SC, transmission) cannot change instantly mid-drive, **skip that assist entirely** — don't offer it in the UI
- Only show controls that actually work mid-session. No greyed-out or disabled toggles.
- FFB intensity changes go **direct to the OpenFFBoard wheelbase via HID** — bypass AC's INI entirely. FfbController already has the HID interface open.
- Assist changes need research into AC's runtime behavior — shared memory writes, CSP console commands, or AC API if available

### FFB Intensity Model
- **Percentage slider from 10% to 100%** — no presets, just a clean slider
- Minimum floor: **10%** (prevents zero FFB which feels like a broken wheel)
- Initial slider position **matches the launch setting** — if customer launched with medium (70%), slider starts at 70
- **500ms debounce** on slider changes — only send HID command after 500ms of no movement to prevent flooding the wheelbase
- FFB gain sent via OpenFFBoard HID vendor commands (extends FfbController beyond safety-only usage)

### PWA Control Placement
- **Bottom sheet / drawer** on the active session screen — pull up to reveal, dismiss to hide
- Opened via a **gear icon button** on the session screen (visible, discoverable)
- Changes apply **instantly on toggle** — no "Apply" button needed
- Drawer shows **actual pod state** when opened — PWA queries rc-core for current assist/FFB values, not cached last-sent values
- Toggles for assists (ON/OFF), slider for FFB intensity

### Change Feedback to Driver
- **Both HUD overlay AND PWA confirmation** — double confirmation, no ambiguity
- HUD overlay: brief toast at **top-center** of pod screen (e.g., "ABS: OFF" or "FFB: 85%")
- Toast duration: **3 seconds**
- Toast behavior: **replace** (latest change wins) — no stacking or queueing
- PWA shows confirmation inline next to the control that changed
- Uses existing overlay system from Phase 3 (billing timer overlay)

### Claude's Discretion
- Exact WebSocket message types for mid-session control commands
- How to read current assist state from the pod (shared memory, INI read, or cached in rc-agent)
- OpenFFBoard HID command for setting FFB gain (vs safety estop which is already implemented)
- Overlay toast styling and animation (fade in/out, color scheme)
- Whether to send individual assist changes or batch them

</decisions>

<specifics>
## Specific Ideas

- The bottom sheet pattern is familiar from mobile apps — customers already know how to swipe up/down
- FFB slider should feel responsive even with debounce — update the UI position immediately, debounce only the HID send
- "Only show what works" is the key UX principle — if AC can't hot-swap ABS, customers never know it was considered. Clean.
- Phase 2 decided assists are independent of difficulty tiers — this phase treats each assist as a standalone toggle
- The existing set_transmission() and set_ffb() functions in ac_launcher.rs are INI-based and require restart — Phase 6 needs to go beyond this for instant changes

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `FfbController` (ffb_controller.rs): HID interface to OpenFFBoard — currently safety-only (zero_force, estop). Extend with set_gain() for intensity control.
- `set_transmission()` (ac_launcher.rs:323): Existing INI-based transmission change — reference for what to replace with instant approach
- `set_ffb()` (ac_launcher.rs:374): Existing INI-based FFB preset change — replace with HID-direct for Phase 6
- `AcAids` struct (ac_launcher.rs:202): abs, tc, stability, autoclutch, ideal_line — defines the assist model
- Overlay system (Phase 3): HUD overlay for billing timer — extend with toast notification capability
- `AssettoCorsaAdapter` (sims/assetto_corsa.rs): Shared memory reader — can read current AC state

### Established Patterns
- WebSocket protocol: AgentMessage/CoreMessage adjacently-tagged enums — add new variants for mid-session commands
- OpenFFBoard HID: send_vendor_cmd() sends 26-byte report — extend with gain command
- INI writing: write_assists_ini(), write_assists_section() — reference but not sufficient for instant changes

### Integration Points
- rc-core receives change request from PWA API → sends CoreMessage to rc-agent via WebSocket
- rc-agent receives CoreMessage → applies change (HID for FFB, TBD mechanism for assists)
- rc-agent confirms change → sends AgentMessage back to rc-core → PWA gets confirmation
- Overlay on pod screen shows toast notification independently
- PWA active session screen needs gear icon + bottom sheet UI component

</code_context>

<deferred>
## Deferred Ideas

- Mid-session AI difficulty adjustment (change AI_LEVEL while racing) — separate capability, not in Phase 6 scope
- Weather/condition changes mid-session — future feature
- Camera angle presets from PWA — out of scope

</deferred>

---

*Phase: 06-mid-session-controls*
*Context gathered: 2026-03-14*
