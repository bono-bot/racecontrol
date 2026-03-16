# Phase 11: Customer Experience Polish - Research

**Researched:** 2026-03-14
**Domain:** Rust/Axum rc-agent HTML lock screen, WebSocket protocol, Next.js kiosk settings UI
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BRAND-01 | Lock screen displays the Racing Point logo prominently | PAGE_SHELL already has `.logo` class and `RACING POINT` text — needs visual logo image embedded as inline SVG or Base64 PNG |
| BRAND-02 | Staff can set a dynamic or static wallpaper for the blanking/lock screen from the kiosk dashboard — visible on pod within 10 seconds | `kiosk_settings` key-value table + `SettingsUpdated` broadcast + `lock_screen.rs` HTML rendering; wallpaper URL stored as setting, pods poll every 3s |
| BRAND-03 | A branded loading screen shown before game launch — no desktop visible | `LockScreenState::LaunchSplash` already exists and is rendered; needs richer branding (logo image, better copy) |
| SESS-01 | After session, pod displays lap times, top speed, best lap | `SessionSummary` state exists with laps + best lap; `top_speed_kmh` field must be added to `SessionEnded` protocol message and tracked in rc-agent from `TelemetryFrame.speed_kmh` |
| SESS-02 | After session, pod displays race position (if AI/multiplayer) | Race position must be tracked in rc-agent from AC shared memory or telemetry; added as optional field on `SessionEnded` |
| SESS-03 | Session results remain on screen until new session initiated | `blank_timer` fires after 15 seconds and blanks screen — must be disabled or made indefinite; new session (BillingStarted / ShowPinLockScreen) dismisses results naturally |
</phase_requirements>

---

## Summary

Phase 11 is pure enhancement to existing infrastructure — no new architecture needed. The lock screen system in `lock_screen.rs` already handles all states (LaunchSplash, SessionSummary, BetweenSessions, etc.) via an Edge browser in kiosk mode auto-reloading every 3 seconds. The three BRAND requirements upgrade what already renders; the three SESS requirements extend the data pipeline and persistence.

**The key constraint for SESS-03:** The current `blank_timer` arms after `SessionEnded` and fires after 15 seconds, blanking the screen. To keep results on screen indefinitely, the blank_timer must be disarmed on `SessionEnded` (or its countdown made much larger). The screen naturally transitions when core sends `ShowPinLockScreen`, `BlankScreen`, or `ClearLockScreen` — which happens when staff starts a new session.

**The key constraint for SESS-01/SESS-02:** `SessionEnded` in the protocol carries `total_laps`, `best_lap_ms`, and `driving_seconds` but NOT `top_speed_kmh` or `race_position`. These must be (a) tracked locally in rc-agent during the session from telemetry, and (b) added as optional fields to the `SessionEnded` message in `rc-common/protocol.rs` so racecontrol can include them when it fires the message.

**Primary recommendation:** Work in three layers — protocol extension (rc-common), rc-agent tracking + rendering, kiosk settings UI. Keep each layer independently testable.

---

## Standard Stack

### Core (all already in use — no new dependencies)

| Component | Location | Version/Type | Purpose |
|-----------|----------|-------------|---------|
| `lock_screen.rs` | `crates/rc-agent/src/` | Rust/Tokio | HTTP server on :18923, HTML rendering, state machine |
| `protocol.rs` | `crates/rc-common/src/` | Rust/Serde | `SessionEnded` message, `SettingsUpdated` message |
| `kiosk_settings` table | SQLite via racecontrol | key-value store | Persistent venue settings broadcast to all pods |
| `SettingsUpdated` message | WebSocket protocol | existing | Push settings from core to all connected rc-agents |
| `api/routes.rs` | `crates/racecontrol/src/` | Axum | `update_kiosk_settings` broadcasts to all agents |
| `settings/page.tsx` | `kiosk/src/app/settings/` | Next.js | Existing kiosk settings page for staff |

### No New Dependencies

No new Rust crates or npm packages are needed for any Phase 11 requirement. The entire feature set is achievable within the existing stack:

- Branding: inline CSS + Base64 SVG logo in `lock_screen.rs` HTML templates
- Wallpaper URL: stored in `kiosk_settings` table, rendered via CSS `background-image`
- Session stats: tracked as local variables in rc-agent's main event loop
- Results persistence: remove `blank_timer` arming on `SessionEnded`

---

## Architecture Patterns

### Pattern 1: Settings Key → Pod HTML Rendering (BRAND-02)

**What:** Staff writes a setting via kiosk UI → `update_kiosk_settings` saves to SQLite → `broadcast_settings` calls `state.broadcast_settings(&map)` → sends `SettingsUpdated` to all connected agents → rc-agent's `SettingsUpdated` handler updates a local variable → next page render uses the variable in HTML CSS.

**Already works for:** `kiosk_lockdown_enabled`, `screen_blanking_enabled`

**How to apply for wallpaper:**
```rust
// In rc-agent main loop, SettingsUpdated handler:
if let Some(v) = settings.get("lock_screen_wallpaper_url") {
    lock_screen.set_wallpaper_url(v.clone());
}
```

The 3-second JS auto-reload in all lock screen pages guarantees the new wallpaper shows within 3 seconds of the agent receiving the update. The spec says 10 seconds — 3s satisfies it.

**Where wallpaper is rendered:** `page_shell()` function in `lock_screen.rs`. Add a CSS `background-image` to the `body` style when a wallpaper URL is set.

### Pattern 2: Telemetry Tracking in Main Loop (SESS-01/SESS-02)

**What:** rc-agent already receives `TelemetryFrame` events from the UDP sim adapters every ~100ms. The `OverlayData` struct already tracks `speed_kmh`. The main loop needs two additional accumulators that reset at session start and are read at session end.

```rust
// In main loop state variables (alongside blank_timer, etc.):
let mut session_max_speed_kmh: f32 = 0.0;
let mut session_race_position: Option<u32> = None;

// In telemetry processing branch:
if frame.speed_kmh > session_max_speed_kmh {
    session_max_speed_kmh = frame.speed_kmh;
}
// AC position comes from shared memory (AcStatus); F1 from F1 telemetry protocol
```

**Reset trigger:** `BillingStarted` message → reset both accumulators to zero/None.

**Read trigger:** `SessionEnded` handler → pass to `show_session_summary()`.

### Pattern 3: Protocol Extension for Session Stats (SESS-01/SESS-02)

The `SessionEnded` message in `rc-common/protocol.rs` currently carries:

```rust
SessionEnded {
    billing_session_id: String,
    driver_name: String,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
},
```

Add optional fields with backward-compatible serde defaults:

```rust
SessionEnded {
    billing_session_id: String,
    driver_name: String,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    top_speed_kmh: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    race_position: Option<u32>,
},
```

**Why optional:** racecontrol constructs `SessionEnded` from its billing database, which does NOT currently store top speed or race position. Rather than plumbing these values through racecontrol, the agent should track them locally and override the display with its own values. This is the simpler approach: racecontrol sends `None` for both new fields; rc-agent substitutes its own tracked values.

**Alternative approach (simpler):** rc-agent ignores the protocol fields entirely for now and uses ONLY its locally tracked accumulators. This avoids any racecontrol changes. The `show_session_summary()` function signature gets two new optional parameters. This is the recommended approach for Phase 11.

### Pattern 4: SESS-03 — Persistent Results Screen

**Current behavior:** `SessionEnded` handler in `main.rs` line 1084:
```rust
blank_timer.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(15));
blank_timer_armed = true;
```
This blanks the screen 15 seconds after session ends.

**Required behavior:** Results stay until staff/customer starts a new session. Transitions are:
- `ShowPinLockScreen` — replaces results with PIN screen (staff assigned customer)
- `BlankScreen` — explicit blank command from kiosk dashboard
- `ClearLockScreen` — explicit clear
- `BillingStarted` — new session starting

**Fix:** Remove the two `blank_timer` arming lines from the `SessionEnded` handler. The screen remains on `SessionSummary` state until one of the above messages arrives. The existing `LaunchSplash` state covers the game-loading transition for the NEXT session.

**Risk:** If no new session is ever started, the results screen stays forever. This is the desired behavior per SESS-03.

### Pattern 5: BRAND-01 — Logo on Lock Screen

**Current state:** The `PAGE_SHELL` template renders:
```html
<div class="logo">RACING POINT</div>
<div class="tagline">May the Fastest Win.</div>
```

This is text-only with the `.logo` CSS class (font-size 2.8em, letter-spacing 6px, color #E10600). The spec says "Racing Point logo prominently" — this means a proper logo mark, not just styled text.

**Approach:** Embed the Racing Point logo as an inline Base64 PNG or SVG in the `PAGE_SHELL` constant. No external URL needed (pods may have no internet). The logo can be a simple SVG checkered-flag or "RP" wordmark.

**Constraint:** The lock screen HTTP server on :18923 only serves the root path `/` — no static file serving. Inline Base64 or inline SVG is the correct approach. A `<link>` to Google Fonts is already present (works on LAN if internet available; falls back to system font if not).

### Pattern 6: BRAND-03 — Loading Screen Polish

**Current state:** `LaunchSplash` already exists:
```rust
LockScreenState::LaunchSplash {
    driver_name: String,
    message: String,
}
```

And `render_launch_splash_page()` renders a branded spinner with "PREPARING YOUR SESSION" header. The test `launch_splash_renders_branded_html` already verifies Racing Point red `#E10600` is present.

**What's missing:** The current splash uses text-only header. BRAND-03 requires the Racing Point logo image, not just the text. Same fix as BRAND-01: add the inline SVG/PNG logo to the `LaunchSplash` render function.

### Anti-Patterns to Avoid

- **External image URLs in lock screen HTML:** The lock screen runs offline-capable. Never use `<img src="http://...">` for the logo. Always inline SVG or Base64.
- **Storing wallpaper files on pods:** BRAND-02 stores a URL (can be a data URI or a LAN URL like `http://192.168.31.23:8080/...`). Never store binary image files in pod config.
- **Re-arming blank_timer in SessionSummary:** SESS-03 requires indefinite display. Do not add any auto-dismiss logic to `SessionSummary`.
- **Adding new `LockScreenState` variants:** All three brand requirements work within existing states. `LaunchSplash` is already correct for BRAND-03. `SessionSummary` is correct for SESS-01/SESS-02/SESS-03.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Logo rendering | Custom image server on :18923 | Inline SVG or Base64 in HTML template string | Lock screen server only serves `/` — no routing for `/logo.png` |
| Wallpaper distribution | File transfer to pods | URL string in `kiosk_settings` key-value + CSS `background-image` | Already-working broadcast infrastructure handles it in <3s |
| Race position tracking | AC shared memory parsing in new module | Use `TelemetryFrame` already arriving in main loop + AC shared memory reader already in `sims/assetto_corsa.rs` | Telemetry is already deserialized every 100ms |
| Settings persistence | New database table | `kiosk_settings` key-value table already exists | `INSERT ... ON CONFLICT DO UPDATE` already implemented |
| Broadcast on settings change | Custom pub/sub | `state.broadcast_settings()` already called by `update_kiosk_settings` | Sends `SettingsUpdated` to all agents automatically |

---

## Common Pitfalls

### Pitfall 1: blank_timer Auto-Blanks Before Customer Reads Results
**What goes wrong:** Current code arms 15s blank timer in `SessionEnded`. Customer doesn't have 15 seconds to read results.
**Why it happens:** Phase 5 designed auto-blank as a convenience feature for the next customer.
**How to avoid:** Remove `blank_timer_armed = true` from `SessionEnded` handler. Results stay until staff action.
**Warning signs:** If tests check for `blank_timer_armed` being true after `SessionEnded`, update those tests.

### Pitfall 2: Wallpaper URL Containing Spaces or Special Characters
**What goes wrong:** CSS `background-image: url(...)` breaks if the URL has unescaped spaces.
**Why it happens:** Staff pastes URLs with spaces from browser.
**How to avoid:** In the HTML renderer, URL-encode or wrap in quotes: `background-image: url('{{WALLPAPER_URL}}')`. HTML-escape the URL value to prevent injection.

### Pitfall 3: Race Position Not Available for Practice Sessions
**What goes wrong:** AC practice mode has no race position (always 0 or null from telemetry).
**Why it happens:** Race position is only meaningful in race sessions.
**How to avoid:** `race_position: Option<u32>` — only display the stat card when `Some(pos)`. Hide it in the HTML when None.

### Pitfall 4: Top Speed Accumulator Not Reset on New Session
**What goes wrong:** If `session_max_speed_kmh` is not reset on `BillingStarted`, the value from a previous session bleeds into the summary.
**Why it happens:** Accumulator variable persists across sessions in the main loop.
**How to avoid:** Reset `session_max_speed_kmh = 0.0` and `session_race_position = None` in the `BillingStarted` handler.

### Pitfall 5: Wallpaper URL Rendered in ScreenBlanked State
**What goes wrong:** The `ScreenBlanked` state renders a pure-black page. If wallpaper is applied to ALL states including blanked, it breaks the blank screen purpose.
**Why it happens:** Applying wallpaper CSS globally in `PAGE_SHELL`.
**How to avoid:** Apply wallpaper only to idle states (StartupConnecting, Disconnected, Hidden) — not to `ScreenBlanked`. Pass wallpaper as a parameter to `page_shell()` with a flag.

### Pitfall 6: Backward Compatibility on Protocol Extension
**What goes wrong:** If `top_speed_kmh` is added to `SessionEnded` WITHOUT `#[serde(default)]`, racecontrol binary (which doesn't know about it yet) will fail to deserialize incoming messages.
**Why it happens:** serde's default behavior for missing fields is an error.
**How to avoid:** Always use `#[serde(default, skip_serializing_if = "Option::is_none")]` for new optional fields. Existing racecontrol binary does not need to be redeployed for the protocol change to be safe.

---

## Code Examples

### Example 1: Wallpaper URL in page_shell
```rust
// Source: lock_screen.rs (existing pattern, enhanced)
fn page_shell_with_wallpaper(title: &str, content: &str, wallpaper_url: Option<&str>) -> String {
    let bg_style = match wallpaper_url {
        Some(url) if !url.is_empty() => format!(
            "background: linear-gradient(135deg, #1A1A1A 0%, #222222 50%, #1A1A1A 100%);\
             background-image: url('{}'); background-size: cover; background-position: center;",
            html_escape(url)
        ),
        _ => "background: linear-gradient(135deg, #1A1A1A 0%, #222222 50%, #1A1A1A 100%);".to_string(),
    };
    // Replace body style in PAGE_SHELL with bg_style
    PAGE_SHELL
        .replace("{{TITLE}}", title)
        .replace("{{CONTENT}}", content)
        .replace("{{BG_STYLE}}", &bg_style)
}
```

### Example 2: Top Speed Tracking in Main Loop
```rust
// In the telemetry processing arm of the select! macro:
// (TelemetryFrame arrives from adapter.next_frame())
if frame.speed_kmh > session_max_speed_kmh {
    session_max_speed_kmh = frame.speed_kmh;
}
```

### Example 3: Reset Accumulators on BillingStarted
```rust
CoreToAgentMessage::BillingStarted { billing_session_id, driver_name, allocated_seconds } => {
    // ... existing code ...
    session_max_speed_kmh = 0.0;        // NEW: reset for this session
    session_race_position = None;        // NEW: reset for this session
}
```

### Example 4: Extended show_session_summary Call
```rust
// SessionEnded handler — pass locally tracked stats:
lock_screen.show_session_summary(
    driver_name,
    total_laps,
    best_lap_ms,
    driving_seconds,
    Some(session_max_speed_kmh),   // NEW: f32 km/h
    session_race_position,          // NEW: Option<u32>
);
// Do NOT arm blank_timer (remove those two lines)
```

### Example 5: Session Summary HTML with Top Speed + Position
```rust
// In render_session_summary_page():
let speed_card = if let Some(spd) = top_speed_kmh {
    format!(r#"<div class="stat-item">
        <div class="stat-value">{:.0}</div>
        <div class="stat-label">Top Speed km/h</div>
    </div>"#, spd)
} else { String::new() };

let position_card = if let Some(pos) = race_position {
    let suffix = match pos { 1 => "st", 2 => "nd", 3 => "rd", _ => "th" };
    format!(r#"<div class="stat-item">
        <div class="stat-value">{}{}</div>
        <div class="stat-label">Race Position</div>
    </div>"#, pos, suffix)
} else { String::new() };
```

### Example 6: Kiosk Settings UI — Wallpaper Input (Next.js)
```tsx
// In kiosk/src/app/settings/page.tsx (existing pattern):
<div>
  <label className="text-sm text-rp-grey">Lock Screen Wallpaper URL</label>
  <input
    type="url"
    value={settings?.lock_screen_wallpaper_url ?? ""}
    onChange={(e) => handleSettingChange("lock_screen_wallpaper_url", e.target.value)}
    placeholder="http://192.168.31.23:8080/wallpaper.jpg or leave blank"
    className="w-full mt-1 px-3 py-2 bg-rp-card border border-rp-border rounded text-sm"
  />
  <p className="text-xs text-rp-grey mt-1">Change is visible on pods within 10 seconds</p>
</div>
```

---

## State of the Art

| Old State | Current State | Impact for Phase 11 |
|-----------|--------------|---------------------|
| Auto-blanks after 15s | Must stay indefinitely | Remove blank_timer arming in SessionEnded |
| Text-only logo in PAGE_SHELL | Need image logo | Add inline SVG/Base64 to PAGE_SHELL |
| LaunchSplash exists (text only) | Need logo in splash | Same fix as above — reuse same inline logo |
| `SessionEnded` carries 5 fields | Need top_speed + position | Add 2 optional fields with `#[serde(default)]` |
| Wallpaper: not implemented | Need URL → CSS background | New `kiosk_settings` key + SettingsUpdated handler |

---

## Open Questions

1. **Racing Point Logo Asset**
   - What we know: No logo file exists in the repo. Only text with CSS styling.
   - What's unclear: Do we have an actual SVG logo or PNG?
   - Recommendation: Create a simple inline SVG checkered-flag or text-based wordmark in Racing Red (#E10600). Can be replaced later when asset exists.

2. **Race Position Source for AC**
   - What we know: `TelemetryFrame` does not carry position. AC shared memory has a position field.
   - What's unclear: Is position tracked in `sims/assetto_corsa.rs` adapter?
   - Recommendation: Add max-speed tracking first (easy). Position for AC requires checking `sims/assetto_corsa.rs` for shared memory position field. If not available, ship SESS-02 as "shown only when telemetry provides position" (F1 25 adapter may provide it).

3. **Wallpaper URL Scope**
   - What we know: `kiosk_settings` is global (all pods get same value via broadcast).
   - What's unclear: Should wallpaper be per-pod or venue-wide?
   - Recommendation: Venue-wide for Phase 11. Per-pod is a future enhancement.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (`#[test]`, `#[tokio::test]`) |
| Config file | none — inline tests in source files |
| Quick run command | `cargo test -p rc-common && cargo test -p racecontrol-crate` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

Note: rc-agent is a binary crate (no `lib.rs`). Tests are inline in `lock_screen.rs` via `#[cfg(test)] mod tests`. Run with `cargo test --bin rc-agent`.

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BRAND-01 | PAGE_SHELL contains logo element (SVG/img) | unit | `cargo test --bin rc-agent logo` | ❌ Wave 0 — add test in `lock_screen.rs` |
| BRAND-02 | wallpaper URL appears in rendered HTML as CSS background | unit | `cargo test --bin rc-agent wallpaper` | ❌ Wave 0 |
| BRAND-02 | SettingsUpdated with `lock_screen_wallpaper_url` updates agent state | unit | `cargo test -p rc-common settings_updated` | ❌ Wave 0 |
| BRAND-03 | LaunchSplash renders logo element | unit | `cargo test --bin rc-agent launch_splash_has_logo` | ❌ Wave 0 (existing test checks #E10600 but not logo img) |
| SESS-01 | SessionSummary page contains top speed stat card | unit | `cargo test --bin rc-agent session_summary_top_speed` | ❌ Wave 0 |
| SESS-01 | Top speed tracks max from telemetry frames | unit | local variable logic test | ❌ Wave 0 |
| SESS-02 | SessionSummary page contains race position stat card when Some | unit | `cargo test --bin rc-agent session_summary_position` | ❌ Wave 0 |
| SESS-02 | Race position card hidden when None | unit | `cargo test --bin rc-agent session_summary_no_position` | ❌ Wave 0 |
| SESS-03 | SessionSummary does NOT reload or auto-dismiss | unit | `cargo test --bin rc-agent session_summary_no_autoblank` | ❌ Wave 0 |
| SESS-03 | SessionSummary page reload interval absent (or very long) | unit | check HTML does not contain `location.reload` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common`
- **Per wave merge:** `cargo test -p rc-common && cargo test --bin rc-agent`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] Tests for BRAND-01: `logo_in_page_shell`, `logo_in_launch_splash`
- [ ] Tests for BRAND-02: `wallpaper_url_renders_in_css`, `wallpaper_empty_uses_default_bg`
- [ ] Tests for SESS-01: `session_summary_shows_top_speed`, `session_summary_hides_top_speed_when_zero`
- [ ] Tests for SESS-02: `session_summary_shows_race_position`, `session_summary_hides_position_when_none`
- [ ] Tests for SESS-03: `session_summary_no_auto_reload_script`

All tests go in `crates/rc-agent/src/lock_screen.rs` under `#[cfg(test)] mod tests` (existing pattern, 15+ tests already there).

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/lock_screen.rs` — full lock screen state machine, HTTP server, HTML templates (read in full)
- `crates/rc-agent/src/main.rs` — SessionEnded handler, blank_timer, BillingStarted, SettingsUpdated (read lines 450–1500)
- `crates/rc-common/src/protocol.rs` — SessionEnded, SettingsUpdated, CoreToAgentMessage definitions (read in full)
- `crates/racecontrol/src/api/routes.rs` — set_pod_screen, get/update_kiosk_settings, lockdown broadcast pattern (read in full)
- `kiosk/src/app/settings/page.tsx` — existing settings UI pattern (read)
- `kiosk/src/lib/api.ts` — API client patterns (read)

### Secondary (MEDIUM confidence)
- Rust serde documentation: `#[serde(default, skip_serializing_if = "Option::is_none")]` — verified pattern already used in existing `TelemetryFrame` and `SessionEnded` fields in same file

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all implementation is within existing files, no new dependencies
- Architecture: HIGH — patterns verified directly in source code
- Pitfalls: HIGH — blank_timer behavior verified in main.rs, protocol extension pattern verified in existing code

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable codebase, no external dependencies)
