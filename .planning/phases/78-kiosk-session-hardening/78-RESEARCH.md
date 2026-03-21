# Phase 78: Kiosk & Session Hardening - Research

**Researched:** 2026-03-21
**Domain:** Windows kiosk lockdown (Edge/Chrome flags, keyboard hooks, registry, Group Policy) + session-scoped token lifecycle + network source tagging
**Confidence:** HIGH

## Summary

Phase 78 addresses the physical attack surface of the pod machines and ties kiosk security to billing session lifecycle. The codebase already has a **substantial kiosk foundation**: `kiosk.rs` includes a 400+ entry process allowlist, an LLM-based unknown process classifier, temp-allow with TTL, lockdown mode, and a working low-level keyboard hook that blocks Win key, Alt+Tab, Alt+F4, Alt+Esc, and Ctrl+Esc. The `lock_screen.rs` launches Edge in `--kiosk` mode with several `--disable-*` flags. The `pod-lockdown.ps1` script handles taskbar auto-hide, Win key blocking via registry NoWinKeys, and Windows Update suppression.

What is **missing** and needs to be built: (1) Edge/Chrome hardening flags for dev tools, extensions, file:// protocol, and incognito mode; (2) Sticky Keys / Filter Keys / Toggle Keys registry disable; (3) USB mass storage disable via USBSTOR service/registry; (4) server-side route protection so kiosk IP ranges cannot access admin routes; (5) session-scoped kiosk tokens that tie pod unlock state to active billing; (6) security anomaly detection that auto-pauses billing and sends WhatsApp alerts; (7) network source tagging middleware that classifies requests by origin (wired pod, WiFi customer, WAN cloud).

**Primary recommendation:** Extend the existing `pod-lockdown.ps1` script with Sticky Keys and USBSTOR registry entries, add missing Edge flags to `lock_screen.rs`, implement a `SessionToken` variant in the `CoreToAgentMessage` enum for session-scoped locking, and add network source classification middleware to the Axum router.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| KIOSK-01 | Chrome kiosk flag lockdown -- disable dev tools, extensions, file:// protocol | Edge launched from lock_screen.rs already uses --kiosk; needs --disable-extensions, --disable-dev-tools-extensions, --disable-file-system. See "Browser Flag Hardening" section. |
| KIOSK-02 | Block keyboard shortcuts (Win+R, Alt+Tab, Ctrl+Alt+Del, Alt+F4) via low-level keyboard hook | Keyboard hook already exists in kiosk.rs windows_impl. Blocks Win/Alt+Tab/Alt+F4/Alt+Esc/Ctrl+Esc. Ctrl+Alt+Del cannot be intercepted by user-mode hooks -- must use registry SAS disable. See "Keyboard Hook Gaps" section. |
| KIOSK-03 | Disable USB mass storage on pod machines via Group Policy | Registry key HKLM\SYSTEM\CurrentControlSet\Services\USBSTOR\Start = 4 disables USB mass storage. Add to pod-lockdown.ps1. See "USB Mass Storage" section. |
| KIOSK-04 | Disable Sticky Keys and accessibility escape vectors via registry | Registry keys under HKCU\Control Panel\Accessibility. See "Accessibility Escape Vectors" section. |
| KIOSK-05 | PWA route protection -- kiosk cannot access admin routes | Network source tagging (KIOSK-07) enables this. Staff routes reject requests from pod IP range. See "Route Protection" section. |
| KIOSK-07 | Network source tagging -- different trust levels for wired LAN, WiFi, WAN | ConnectInfo<SocketAddr> already extracted in Axum (into_make_service_with_connect_info). Add middleware that tags request source. See "Network Source Tagging" section. |
| SESS-04 | Session-scoped kiosk tokens -- kiosk locks when billing session ends | New CoreToAgentMessage::SessionToken variant. Server issues on BillingStarted, agent validates, revokes on SessionEnded. See "Session Token Lifecycle" section. |
| SESS-05 | Automated session pause on security anomaly with WhatsApp alert | KioskLockdown agent message already exists. Server side needs to auto-pause billing on receipt + call whatsapp_alerter. See "Anomaly Detection" section. |
</phase_requirements>

## Standard Stack

### Core (Already in Project)

| Library | Version | Purpose | Status |
|---------|---------|---------|--------|
| winapi | 0.3 | SetWindowsHookEx, keyboard hook, window manipulation | Already in rc-agent Cargo.toml |
| sysinfo | latest | Process enumeration for kiosk enforcement | Already in rc-agent |
| reqwest | latest | HTTP client for LLM classification, server API calls | Already in rc-agent |
| axum | latest | Server-side middleware for route protection + source tagging | Already in racecontrol |
| tower | latest | Middleware layer composition | Already in racecontrol |
| rc-common | local | AgentMessage/CoreToAgentMessage protocol types | Already shared between crates |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| uuid | (already dep) | Generate session-scoped tokens | For SESS-04 token generation |
| chrono | (already dep) | Token expiry timestamps | For SESS-04 token TTL |

No new dependencies are needed. All required functionality is achievable with existing crates.

## Architecture Patterns

### Pattern 1: Pod Lockdown via Registry Script (KIOSK-03, KIOSK-04)

**What:** Extend `deploy/pod-lockdown.ps1` with new registry entries for USB mass storage and Sticky Keys. Deploy via rc-agent `/exec` to all pods.

**Why this approach:** The script already exists and is idempotent. Registry changes persist across reboots. No Rust code changes needed for the OS-level lockdown -- keep it in the deploy script.

**Registry entries to add:**

```powershell
# USB mass storage disable (KIOSK-03)
# Start = 4 means "Disabled" for the USBSTOR driver service
Set-ItemProperty -Path 'HKLM:\SYSTEM\CurrentControlSet\Services\USBSTOR' -Name 'Start' -Value 4 -Type DWord

# Sticky Keys disable (KIOSK-04)
# Flags = "506" disables the 5x Shift shortcut that opens Sticky Keys dialog
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\StickyKeys' -Name 'Flags' -Value '506' -Type String

# Filter Keys disable (KIOSK-04)
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\Keyboard Response' -Name 'Flags' -Value '122' -Type String

# Toggle Keys disable (KIOSK-04)
Set-ItemProperty -Path 'HKCU:\Control Panel\Accessibility\ToggleKeys' -Name 'Flags' -Value '58' -Type String
```

**Undo support must be added** for each new entry (matching the existing `-Undo` pattern).

**Confidence:** HIGH -- these are well-documented Windows registry values. The Flags values disable the "keyboard shortcut to turn on" feature while leaving the accessibility feature itself available through Settings.

### Pattern 2: Browser Flag Hardening in lock_screen.rs (KIOSK-01)

**What:** Add security flags to the Edge `--kiosk` launch in `lock_screen.rs`.

**Current flags (line 554-566):**
```
--kiosk, --edge-kiosk-type=fullscreen, --no-first-run, --no-default-browser-check,
--disable-notifications, --disable-popup-blocking, --disable-infobars,
--disable-session-crashed-bubble, --disable-component-update,
--autoplay-policy=no-user-gesture-required, --suppress-message-center-popups
```

**Flags to add:**
```
--disable-extensions                    # Prevents extension install/sideload
--disable-dev-tools                     # Blocks F12 / Ctrl+Shift+I
--disable-translate                     # Prevents translate bar (escape vector)
--disable-features=FileSystemAPI        # Blocks file:// protocol access
--disable-file-system                   # Additional file system access block
--incognito                             # Prevents history/cache persistence (no local storage leak)
--disable-pinch                         # Prevents zoom gesture escape
--disable-print-preview                 # Prevents print dialog (file save escape)
--no-experiments                        # Disables chrome://flags access
--disable-background-networking         # Prevents background update checks
--block-new-web-contents                # Prevents popups opening new windows
```

**Important:** Edge kiosk mode (`--edge-kiosk-type=fullscreen`) already blocks the URL bar. Adding `--disable-dev-tools` is the critical missing flag -- F12 is the #1 kiosk escape vector for tech-savvy customers.

**Note on `--disable-dev-tools`:** In Chromium-based browsers, `--kiosk` mode already disables some DevTools access, but `--disable-dev-tools` provides explicit enforcement. Edge may use `--disable-dev-tools-extension` as the actual flag. Both should be included for defense in depth.

**Confidence:** HIGH for core flags (--disable-extensions, --disable-dev-tools). MEDIUM for --disable-features=FileSystemAPI (Edge-specific behavior may differ from Chrome).

### Pattern 3: Keyboard Hook Enhancement (KIOSK-02)

**What:** The keyboard hook in `kiosk.rs` already blocks Win, Alt+Tab, Alt+F4, Alt+Esc, Ctrl+Esc. Additional keys to block:

```rust
// Block F12 (DevTools -- defense in depth, browser flag should also block)
if vk == winuser::VK_F12 as u32 {
    return 1;
}
// Block Ctrl+Shift+I (DevTools alternate)
if vk == 0x49 /* I */ {
    let ctrl = unsafe { winuser::GetAsyncKeyState(winuser::VK_CONTROL) } < 0;
    let shift = unsafe { winuser::GetAsyncKeyState(winuser::VK_SHIFT) } < 0;
    if ctrl && shift {
        return 1;
    }
}
// Block Ctrl+Shift+J (Console)
if vk == 0x4A /* J */ {
    let ctrl = unsafe { winuser::GetAsyncKeyState(winuser::VK_CONTROL) } < 0;
    let shift = unsafe { winuser::GetAsyncKeyState(winuser::VK_SHIFT) } < 0;
    if ctrl && shift {
        return 1;
    }
}
// Block Ctrl+L (URL bar -- defense in depth)
if vk == 0x4C /* L */ && unsafe { winuser::GetAsyncKeyState(winuser::VK_CONTROL) } < 0 {
    return 1;
}
```

**Ctrl+Alt+Del limitation:** Windows does NOT allow user-mode keyboard hooks to intercept Ctrl+Alt+Del. This is a Secure Attention Sequence (SAS) handled by the kernel. The only ways to block it:
1. Registry: `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System\DisableTaskMgr = 1` -- blocks Task Manager but Ctrl+Alt+Del menu still appears.
2. Registry: `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\DisableCAD = 1` -- disables the Ctrl+Alt+Del requirement for login but does NOT prevent the SAS screen.
3. Group Policy: "Remove Task Manager" via `HKCU\Software\Microsoft\Windows\CurrentVersion\Policies\System\DisableTaskMgr = 1`.

**Recommendation:** Add `DisableTaskMgr = 1` to `pod-lockdown.ps1`. The Ctrl+Alt+Del screen will still appear but Task Manager will be grayed out. This is the standard kiosk approach -- complete Ctrl+Alt+Del suppression requires a kernel driver (out of scope).

**Confidence:** HIGH -- well-documented Windows API behavior. The limitation on Ctrl+Alt+Del is a kernel security feature by design.

### Pattern 4: Network Source Tagging Middleware (KIOSK-07)

**What:** Axum middleware that classifies incoming requests by source IP into trust tiers.

**Network layout (from CLAUDE.md):**
- Pods (wired LAN): 192.168.31.28, .33, .38, .86, .87, .88, .89, .91
- Server: 192.168.31.23
- POS PC: 192.168.31.20
- James workstation: 192.168.31.27
- WiFi customers: 192.168.31.* (any other IP on subnet)
- Cloud (Bono VPS): external IP

**Trust tiers:**
```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestSource {
    Pod,        // Known pod IPs -- agent-level trust
    Staff,      // Server, James, POS PC -- admin trust
    Customer,   // Any other 192.168.31.* -- customer only
    Cloud,      // External IPs -- cloud sync trust
}
```

**Implementation approach:**
```rust
// Middleware extracts ConnectInfo<SocketAddr> (already available via into_make_service_with_connect_info)
async fn classify_source(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    mut req: Request,
    next: Next,
) -> Response {
    let source = match addr.ip() {
        IpAddr::V4(ip) => {
            let octets = ip.octets();
            if octets[0] == 192 && octets[1] == 168 && octets[2] == 31 {
                match octets[3] {
                    28 | 33 | 38 | 86 | 87 | 88 | 89 | 91 => RequestSource::Pod,
                    20 | 23 | 27 => RequestSource::Staff,
                    _ => RequestSource::Customer,
                }
            } else {
                RequestSource::Cloud
            }
        }
        _ => RequestSource::Cloud,
    };
    req.extensions_mut().insert(source);
    next.run(req).await
}
```

**Important considerations:**
- Pod IPs are currently DHCP-assigned but stable (MAC-based persistence on the TP-Link router). If IPs change, the middleware needs updating. A configuration-based approach (pod IPs in `racecontrol.toml`) is more robust than hardcoding.
- `127.0.0.1` / `::1` (localhost) should map to Staff -- used by services on the same machine.

**Confidence:** HIGH -- ConnectInfo extraction already works in the codebase (used by rate limiting).

### Pattern 5: PWA Route Protection (KIOSK-05)

**What:** Use the `RequestSource` from KIOSK-07 to reject pod-origin requests to admin routes.

The route structure already separates `staff_routes()` from other tiers. Add a middleware layer to `staff_routes()`:

```rust
fn staff_routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        // ... existing routes ...
        .layer(axum::middleware::from_fn(require_staff_source))  // NEW
        .layer(axum::middleware::from_fn(require_staff_jwt))     // existing
}

async fn require_staff_source(req: Request, next: Next) -> Response {
    let source = req.extensions().get::<RequestSource>().copied();
    match source {
        Some(RequestSource::Pod) => {
            // Pods should never access admin routes
            (StatusCode::FORBIDDEN, "Pod source not allowed on admin routes").into_response()
        }
        _ => next.run(req).await,
    }
}
```

**Edge case:** The kiosk PWA on pods makes API calls to `/api/v1/auth/kiosk/validate-pin` and `/api/v1/kiosk/*` routes. These are currently in `staff_routes()`. They need to either:
1. Move to a separate `kiosk_routes()` tier that accepts Pod source, OR
2. Be exempted from the source check

**Recommendation:** Create a `kiosk_routes()` function for the handful of kiosk-facing endpoints (kiosk/validate-pin, kiosk/experiences, kiosk/settings, kiosk/pod-launch-experience, kiosk/book-multiplayer). These need Pod source but NOT admin-level operations.

**Confidence:** HIGH -- the route tier structure already supports this pattern.

### Pattern 6: Session-Scoped Kiosk Tokens (SESS-04)

**What:** When billing starts, the server generates a session token, sends it to the agent via WebSocket, and the agent validates it. When billing ends, the server revokes the token, and the agent locks the kiosk.

**Lifecycle:**
1. Server: `billing::start_billing_session()` generates `session_token = Uuid::new_v4().to_string()`
2. Server: sends `CoreToAgentMessage::SessionToken { token, expires_at }` to agent via existing WebSocket
3. Agent: stores token in `KioskManager`, uses it to gate kiosk unlock state
4. Server: on billing end/pause, sends `CoreToAgentMessage::SessionTokenRevoked { token }`
5. Agent: clears token, transitions to lock screen

**Protocol additions to `rc-common/src/protocol.rs`:**
```rust
// In CoreToAgentMessage enum:
SessionToken {
    billing_session_id: String,
    token: String,
    expires_at: u64,  // Unix timestamp
},
SessionTokenRevoked {
    billing_session_id: String,
},
```

**Agent-side implementation:**
- `KioskManager` gains `active_session_token: Option<String>` field
- On `SessionToken` message: store token, deactivate lock screen (kiosk stays active for process enforcement but browser shows session UI)
- On `SessionTokenRevoked` or `SessionEnded`: clear token, show lock screen
- Periodic check: if `active_session_token.is_some()` and `expires_at` has passed, auto-lock (defense against server crash)

**Existing flow integration:** The server already sends `BillingStarted` and `SessionEnded` messages via WebSocket. The session token piggybacks on the same flow -- either embed in `BillingStarted` or send as a separate message immediately after.

**Recommendation:** Embed the token in the existing `BillingStarted` message as an additional field rather than a new message type. Add `session_token: Option<String>` to `BillingStarted`. This minimizes protocol changes.

**Confidence:** HIGH -- the WebSocket message flow already handles billing lifecycle events.

### Pattern 7: Security Anomaly Detection + Auto-Pause (SESS-05)

**What:** When `KioskLockdown` fires (expired process approval, rejected process), the server should:
1. Auto-pause the active billing session on that pod
2. Send a WhatsApp alert to Uday via the existing `whatsapp_alerter.rs`

**Current state:** `KioskLockdown` messages arrive at the server's `ws/mod.rs` handler (line 634) but only log a pod activity entry. No billing action is taken.

**Implementation:**
```rust
// In ws/mod.rs KioskLockdown handler:
AgentMessage::KioskLockdown { pod_id, reason } => {
    tracing::warn!("[kiosk] Pod {} LOCKDOWN: {}", pod_id, reason);
    log_pod_activity(&state, &pod_id, "kiosk", "Kiosk Lockdown", &reason, "rc-bot");

    // NEW: Auto-pause billing on this pod
    if let Some(session_id) = get_active_billing_session_for_pod(&state, &pod_id).await {
        let pause_reason = format!("Security anomaly: {}", reason);
        billing::pause_billing(&state, &session_id, &pause_reason).await;
        tracing::warn!("[kiosk] Billing session {} auto-paused due to lockdown on pod {}", session_id, pod_id);
    }

    // NEW: WhatsApp alert
    let alert_msg = format!(
        "SECURITY ALERT -- Pod {} LOCKDOWN\nReason: {}\nBilling auto-paused. Check admin dashboard.",
        pod_id, reason
    );
    send_whatsapp(&state.config, &alert_msg).await;
}
```

**WhatsApp integration:** The `send_whatsapp()` function in `whatsapp_alerter.rs` is currently private to that module. It needs to be made `pub(crate)` or extracted to a shared utility. The function already handles Evolution API authentication and error handling.

**Rate limiting alerts:** Add a debounce (e.g., 1 alert per pod per 5 minutes) to prevent alert storms if a process keeps respawning and triggering lockdown repeatedly.

**Confidence:** HIGH -- all building blocks exist (KioskLockdown message, whatsapp_alerter, billing pause).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| USB mass storage block | Custom Rust USB monitor | Windows registry USBSTOR\Start=4 | OS-level enforcement, persists across reboots, 1 line of PowerShell |
| Sticky Keys disable | Custom keyboard filter | Registry Flags values | OS handles it natively, no daemon needed |
| Ctrl+Alt+Del suppression | Kernel driver / keyboard filter driver | Registry DisableTaskMgr + kiosk browser on top | Kernel drivers are unsafe, hard to deploy, unnecessary for cafe threat model |
| Session token crypto | Custom token format | UUID v4 (already in deps) | Tokens are validated server-side over a trusted WebSocket, not bearer tokens -- UUID is sufficient |
| IP-based source classification | External firewall rules | Axum middleware with ConnectInfo | Application-level is sufficient for same-subnet classification, easier to update than firewall rules |

## Common Pitfalls

### Pitfall 1: Keyboard Hook Dropped on Timeout

**What goes wrong:** Windows low-level keyboard hooks have a timeout -- if the hook callback takes too long (default: LowLevelHooksTimeout, typically 300ms-1000ms), Windows silently removes the hook. The kiosk loses all keyboard blocking.

**Why it happens:** The hook callback in `kiosk.rs` is simple (just vkCode checks), so this is unlikely. But if future changes add logging or I/O inside the callback, the timeout will hit.

**How to avoid:** Keep the hook callback under 10ms. Never add tracing, file I/O, or network calls inside `keyboard_hook_proc`. The current implementation is correct -- keep it that way.

**Warning signs:** Customers suddenly able to use Win+R or Alt+Tab after the pod has been running for hours.

### Pitfall 2: Edge --kiosk Flag Ignored on Existing Profile

**What goes wrong:** Edge's `--kiosk` flag may be ignored if Edge is already running with the same user profile. The new window opens in normal mode, not kiosk mode.

**Why it happens:** Chromium-based browsers share a single process per profile. If an Edge process is already running (e.g., from a previous session that wasn't fully killed), the new `--kiosk` launch sends a message to the existing process, which ignores the kiosk flags.

**How to avoid:** `lock_screen.rs` already kills all Edge processes before launching (close_browser() calls `taskkill /F /IM msedge.exe`). This is the correct approach. Verify it runs before every launch.

**Warning signs:** Edge opens but is not fullscreen/kiosk, URL bar visible, DevTools accessible.

### Pitfall 3: USBSTOR Disable Breaks Peripherals

**What goes wrong:** Setting USBSTOR\Start=4 disables ALL USB mass storage, including USB hubs with storage endpoints. Some USB racing wheels or pedals may present as composite devices with a storage component.

**Why it happens:** The USBSTOR driver handles all USB Mass Storage Class devices. It does not distinguish between flash drives and composite devices with mass storage interfaces.

**How to avoid:** Test on all pod hardware before fleet deployment. The Conspit Ares wheelbases use custom HID (VID:0x1209 PID:0xFFB0) -- not USB mass storage, so they should be unaffected. Test by plugging in a USB stick after applying the registry change -- it should not appear in Explorer.

**Warning signs:** Wheelbase firmware update fails, game controller not recognized after USBSTOR change.

### Pitfall 4: Session Token Race on Reconnect

**What goes wrong:** Agent disconnects from WebSocket, reconnects. The server re-sends `BillingStarted` with a new session token, but the agent's old token is still valid. If timing is wrong, the agent might lock the kiosk briefly between disconnect and re-auth.

**How to avoid:** On WebSocket reconnect, the server already sends the current state. Add session token to the reconnect state payload. Agent should check for active billing session on reconnect before locking.

**Warning signs:** Customer sees lock screen flash briefly during a WebSocket reconnect during an active session.

### Pitfall 5: Source Tagging Breaks When Pod IP Changes

**What goes wrong:** A pod gets a different DHCP IP after a router reboot. The source tagging middleware classifies it as "Customer" instead of "Pod". Kiosk-specific API calls start returning 403.

**How to avoid:** Store pod IPs in `racecontrol.toml` configuration rather than hardcoding. The server already knows pod IPs from the `pods` table / fleet health. Use the registered pod IP list dynamically:
```rust
// Check against registered pod IPs from AppState
let pod_ips = state.pods.read().await;
if pod_ips.values().any(|p| p.ip == addr.ip().to_string()) {
    RequestSource::Pod
}
```

**Warning signs:** Kiosk PWA returns 403 on pod-launch-experience or validate-pin calls.

## Code Examples

### Sticky Keys Registry Disable (KIOSK-04)

```powershell
# Source: Microsoft Docs - Accessibility Registry Keys
# Flags value breakdown:
#   506 = 0x1FA = Sticky Keys available but keyboard shortcut (5x Shift) DISABLED
#   Original default: 510 = 0x1FE = keyboard shortcut ENABLED

# Sticky Keys
$sk = 'HKCU:\Control Panel\Accessibility\StickyKeys'
Set-ItemProperty -Path $sk -Name 'Flags' -Value '506' -Type String

# Filter Keys
$fk = 'HKCU:\Control Panel\Accessibility\Keyboard Response'
Set-ItemProperty -Path $fk -Name 'Flags' -Value '122' -Type String

# Toggle Keys
$tk = 'HKCU:\Control Panel\Accessibility\ToggleKeys'
Set-ItemProperty -Path $tk -Name 'Flags' -Value '58' -Type String
```

### USB Mass Storage Disable (KIOSK-03)

```powershell
# Source: Microsoft - USB storage disable via registry
# Start values: 3 = Manual (default), 4 = Disabled
$usb = 'HKLM:\SYSTEM\CurrentControlSet\Services\USBSTOR'
Set-ItemProperty -Path $usb -Name 'Start' -Value 4 -Type DWord

# Undo:
# Set-ItemProperty -Path $usb -Name 'Start' -Value 3 -Type DWord
```

### Task Manager Disable (supplements KIOSK-02)

```powershell
# Source: Microsoft Group Policy - DisableTaskMgr
$sys = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Policies\System'
New-Item -Path $sys -Force -ErrorAction SilentlyContinue | Out-Null
Set-ItemProperty -Path $sys -Name 'DisableTaskMgr' -Value 1 -Type DWord
```

### Session Token in BillingStarted (SESS-04)

```rust
// In rc-common/src/protocol.rs -- extend BillingStarted:
BillingStarted {
    billing_session_id: String,
    driver_name: String,
    allocated_seconds: u32,
    session_token: Option<String>,  // NEW: session-scoped kiosk token
},
```

### Network Source Classification (KIOSK-07)

```rust
// Source: project codebase pattern (ConnectInfo already used in rate_limit.rs)
use axum::extract::ConnectInfo;
use std::net::{IpAddr, SocketAddr};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestSource {
    Pod,
    Staff,
    Customer,
    Cloud,
}

async fn classify_source(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Response {
    let source = classify_ip(addr.ip(), &state).await;
    req.extensions_mut().insert(source);
    next.run(req).await
}
```

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|------------------|--------|
| Chrome `--kiosk` alone | `--kiosk` + `--disable-dev-tools` + `--disable-extensions` + profile isolation | `--kiosk` alone still allows F12/extensions on some Chromium versions |
| Group Policy Editor (gpedit.msc) | Direct registry keys | Same effect, but registry works on Windows Home editions (pods may not have Pro) |
| Full keyboard filter driver (WFCO) | SetWindowsHookEx WH_KEYBOARD_LL | Hook is user-mode, no driver install needed, sufficient for cafe threat model |
| Static API tokens for kiosk | Session-scoped tokens tied to billing | Prevents "session ended but kiosk still unlocked" window |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust native) |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p rc-agent -- kiosk` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| KIOSK-01 | Edge flags include --disable-dev-tools, --disable-extensions | unit | `cargo test -p rc-agent -- lock_screen` | Needs new test |
| KIOSK-02 | Keyboard hook blocks F12, Ctrl+Shift+I, Ctrl+L | manual-only | Physical test on pod | N/A (Windows hook requires desktop session) |
| KIOSK-03 | USBSTOR registry set to 4 | manual-only | `reg query HKLM\SYSTEM\CurrentControlSet\Services\USBSTOR /v Start` on pod | N/A (registry script) |
| KIOSK-04 | Sticky/Filter/Toggle Keys shortcuts disabled | manual-only | Press Shift 5x on pod -- no dialog appears | N/A (registry script) |
| KIOSK-05 | Pod IP rejected from staff routes | integration | `cargo test -p racecontrol -- route_protection` | Needs new test |
| KIOSK-07 | RequestSource correctly classified by IP | unit | `cargo test -p racecontrol -- classify_source` | Needs new test |
| SESS-04 | BillingStarted includes session_token, agent locks on revoke | unit | `cargo test -p rc-common -- session_token` | Needs new test |
| SESS-05 | KioskLockdown triggers billing pause + WhatsApp | integration | `cargo test -p racecontrol -- kiosk_lockdown_pause` | Needs new test |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent -- kiosk && cargo test -p racecontrol -- route`
- **Per wave merge:** Full suite
- **Phase gate:** Full suite green before verify

### Wave 0 Gaps
- [ ] Unit test for Edge kiosk flags (verify args list in lock_screen.rs)
- [ ] Unit test for RequestSource classification by IP
- [ ] Unit test for session token serde round-trip in protocol.rs
- [ ] Integration test for KioskLockdown -> billing pause flow

## Open Questions

1. **Pod IP stability**
   - What we know: Pods have stable DHCP assignments (MAC-based persistence on TP-Link router). Server .23 has a known DHCP issue (reservation blocked by firmware bug).
   - What's unclear: Whether pod IPs ever change on router reboot.
   - Recommendation: Use dynamic lookup from pods table rather than hardcoded IPs. The server already tracks pod IPs via WebSocket registration.

2. **Edge vs Chrome DevTools flag name**
   - What we know: Chrome uses `--disable-dev-tools` or `--auto-open-devtools-for-tabs`. Edge is Chromium-based but may have different flag names.
   - What's unclear: Whether Edge honors `--disable-dev-tools` specifically.
   - Recommendation: Include both `--disable-dev-tools` and `--disable-dev-tools-extension`. Test on pod Edge.

3. **Windows edition on pods**
   - What we know: James workstation is Windows 11 Pro. Pod OS edition unknown.
   - What's unclear: Whether pods run Pro or Home. Home lacks gpedit.msc.
   - Recommendation: Use direct registry keys (not GPO editor) -- works on all Windows editions.

## Sources

### Primary (HIGH confidence)
- **Codebase audit** -- kiosk.rs (917 lines), lock_screen.rs, pod-lockdown.ps1, protocol.rs, ws/mod.rs, whatsapp_alerter.rs, routes.rs
- **SECURITY-AUDIT.md** (Phase 75) -- endpoint inventory, auth state, PII audit
- **FEATURES.md** -- kiosk escape vectors research
- **PITFALLS.md** -- kiosk hardening pitfalls catalog

### Secondary (MEDIUM confidence)
- Microsoft Docs: Accessibility registry flags, USBSTOR service, DisableTaskMgr policy
- Chromium source: `--kiosk`, `--disable-dev-tools`, `--disable-extensions` flag documentation
- Microsoft Edge kiosk mode documentation: `--edge-kiosk-type` flag variants

### Tertiary (LOW confidence)
- Sticky Keys Flags numeric values (506, 122, 58) -- derived from community documentation, should be verified on test pod before fleet deploy

## Metadata

**Confidence breakdown:**
- Browser flag hardening: HIGH - well-documented Chromium flags, existing kiosk.rs foundation
- Keyboard hook enhancement: HIGH - existing hook works, additions are straightforward vkCode checks
- Registry lockdown (USB, Sticky Keys): HIGH for approach, MEDIUM for exact flag values (verify on pod)
- Network source tagging: HIGH - ConnectInfo already in use, IP classification is deterministic
- Session-scoped tokens: HIGH - existing WebSocket message flow, well-understood UUID token pattern
- Anomaly detection + billing pause: HIGH - all building blocks exist (KioskLockdown, billing pause, WhatsApp alerter)

**Research date:** 2026-03-21
**Valid until:** 2026-04-20 (stable domain -- Windows kiosk patterns change slowly)
