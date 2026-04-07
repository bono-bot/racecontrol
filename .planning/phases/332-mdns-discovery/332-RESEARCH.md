# Phase 332: mDNS Auto-Discovery - Research

**Researched:** 2026-04-07
**Domain:** mDNS/Zeroconf service discovery (Rust, Windows 11 LAN)
**Confidence:** HIGH

## Summary

The mDNS auto-discovery feature is **already fully implemented** in the codebase. Both the server advertiser (`crates/racecontrol/src/mdns.rs`) and the agent discoverer (`crates/rc-agent/src/mdns_discovery.rs`) exist, are wired into their respective `main.rs` files, and are gated behind config flags (`server.mdns_enabled` and `core.mdns_enabled`, both defaulting to `true`).

The implementation uses `mdns-sd = "0.12"` (locked in Cargo.lock at 0.12.0). The service type is `_racecontrol._tcp.local.`. The server registers with TXT records (`build_id`, `venue_id`, `version`). The agent browses with a 5-second timeout, overrides `core.url` if found, and falls back to TOML config otherwise.

**What is NOT implemented (the actual work for Phase 332):**
1. **Re-discovery on WS disconnect** -- mDNS discovery runs only once at startup. If the server IP changes (DHCP) or the server moves, the agent keeps retrying the stale IP forever. The reconnection loop (main.rs line 1948+) never re-runs mDNS.
2. **mdns-sd version upgrade** -- 0.12.0 is 7 major versions behind (latest: 0.19.0). Multiple breaking changes exist between 0.12 and 0.19.
3. **Windows firewall verification** -- no automated check that UDP 5353 multicast is allowed on the Private network profile.

**Primary recommendation:** Phase 332 should focus on (1) adding mDNS re-discovery to the reconnect loop after N failed attempts, (2) optionally upgrading mdns-sd, and (3) adding a pre-flight check for mDNS readiness.

## Standard Stack

### Core
| Library | Version (locked) | Latest | Purpose | Status |
|---------|-----------------|--------|---------|--------|
| mdns-sd | 0.12.0 | 0.19.0 | mDNS-SD service discovery | Already in Cargo.toml for both `racecontrol` and `rc-agent` |

### Version Upgrade Analysis (0.12 -> 0.19)

| Version | Breaking Change | Impact on Existing Code |
|---------|----------------|------------------------|
| 0.14.0 | `HostnameResolutionEvent::AddressesFound` uses `ScopedIp` instead of `IpAddr` | Not used in our code -- no impact |
| 0.15.0 | `ServiceEvent::ServiceData` merged into `ServiceResolved` | Our code only matches `ServiceResolved` -- compatible |
| 0.17.0 | Loopback interfaces enabled by default; new `set_interfaces()` API | May change behavior on Windows; needs testing |
| 0.18.0 | Removed `reuseport` default feature | Transparent on Windows |
| 0.19.0 | `ScopedIpV4` includes `interface_ids` in Eq/Hash | Not used directly -- no impact |

**Upgrade recommendation:** MEDIUM priority. The existing 0.12.0 works. Upgrade to 0.19 is safe based on API surface analysis (we only use `ServiceDaemon::new()`, `.browse()`, `.register()`, `ServiceInfo::new()`, `ServiceEvent::ServiceResolved`). However, test on Windows after upgrading -- 0.17's loopback-by-default could cause the daemon to bind extra interfaces.

**Installation:** Already in Cargo.toml. No new dependencies needed.

## Architecture Patterns

### Existing Implementation Structure
```
crates/racecontrol/src/
  mdns.rs                    # Server: start_advertiser() -> Option<ServiceDaemon>
  main.rs:1244               # Calls mdns::start_advertiser() at startup

crates/rc-agent/src/
  mdns_discovery.rs          # Agent: discover_server() -> Option<String>
  main.rs:736                # Calls discover_server() at startup, overrides core.url

crates/rc-common/src/
  config_schema.rs:110       # CoreConfig.mdns_enabled (agent, default: true)

crates/racecontrol/src/
  config.rs:159              # ServerConfig.mdns_enabled (server, default: true)
```

### Gap: Re-Discovery on Disconnect

**Current flow:**
1. Agent starts -> mDNS browse (5s timeout) -> override `core.url` or use TOML
2. Agent enters reconnect loop with `primary_url` and `failover_url`
3. On disconnect, agent retries the SAME URL (lines 1948-2037)
4. **Server IP changes -> agent stuck forever retrying old IP**

**Required flow:**
1. Same as above for initial connection
2. On disconnect, after N failed reconnect attempts (e.g., 5), re-run mDNS discovery
3. If mDNS finds a different IP, update `active_url` and `primary_url`
4. Continue reconnect loop with new URL

### Pattern: mDNS Re-Discovery in Reconnect Loop

```rust
// In the reconnection loop (main.rs ~line 2015), after reconnect failures:
if reconnect_attempt > 0 && reconnect_attempt % 5 == 0 && config.core.mdns_enabled {
    tracing::info!(target: LOG_TARGET, "Re-running mDNS discovery after {} failed reconnects...", reconnect_attempt);
    let rediscovered = tokio::task::spawn_blocking(mdns_discovery::discover_server)
        .await
        .unwrap_or(None);
    if let Some(new_url) = rediscovered {
        let new_authed_url = format!("{}{}", new_url, ws_psk_suffix);
        if new_authed_url != *active_url.read().await {
            tracing::info!(target: LOG_TARGET, "mDNS: server moved to {} (was {})", new_url, active_url.read().await);
            *active_url.write().await = new_authed_url;
            // Reset attempt counter since we have a new target
            reconnect_attempt = 0;
        }
    }
}
```

### Anti-Patterns to Avoid
- **Running mDNS browse on every reconnect attempt:** `spawn_blocking` + mDNS daemon creation is expensive. Only re-discover every Nth failure.
- **Replacing TOML URL permanently:** mDNS override should be runtime-only. The TOML `core.url` stays as fallback. If mDNS stops working, agent can still connect via the configured URL.
- **Holding the ServiceDaemon alive in the agent:** The agent's `discover_server()` correctly creates and shuts down the daemon per browse. Do NOT keep a long-running daemon -- it would conflict with Windows' built-in mDNS responder (dnscache).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| mDNS protocol | Raw UDP multicast on 224.0.0.251:5353 | `mdns-sd` crate | RFC 6762/6763 compliance, DNS record types, TTL management |
| Service discovery | Custom broadcast protocol | `_racecontrol._tcp.local.` standard | Interoperable, debuggable with standard tools (dns-sd, Avahi) |

## Common Pitfalls

### Pitfall 1: Windows mDNS Daemon Conflict
**What goes wrong:** Windows 11 has a built-in mDNS responder (DNS Client service / dnscache). Two mDNS responders on the same machine can conflict on UDP 5353.
**Why it happens:** `mdns-sd` creates its own socket on port 5353. Windows' dnscache is already listening.
**How to avoid:** The `mdns-sd` crate uses `SO_REUSEADDR`/`SO_REUSEPORT` to share the port. This works in practice on Windows but can cause intermittent failures. The existing 5s timeout + TOML fallback handles this correctly.
**Warning signs:** `ServiceDaemon::new()` returns an error about address in use.

### Pitfall 2: mDNS Only at Startup = Stale IP
**What goes wrong:** Server DHCP lease changes, server reboots on different IP, or server migrates. Agent keeps retrying old IP.
**Why it happens:** `discover_server()` runs once at startup (main.rs:736-743). The reconnect loop never re-discovers.
**How to avoid:** Add mDNS re-discovery to the reconnect loop (the primary work item for this phase).
**Warning signs:** Agent log shows repeated "Failed to connect to core" with the same IP, while server is healthy on a different IP.

### Pitfall 3: spawn_blocking Deadlock Risk
**What goes wrong:** `mdns_sd::ServiceDaemon` uses internal sync channels. Calling it from an async context without `spawn_blocking` blocks the tokio runtime.
**Why it happens:** The `recv_timeout` call in `discover_server()` blocks the current thread.
**How to avoid:** Always use `tokio::task::spawn_blocking()` (already done correctly in the existing code).
**Warning signs:** Agent hangs for 5 seconds during startup.

### Pitfall 4: Windows Firewall Blocks UDP 5353
**What goes wrong:** mDNS browse returns nothing despite server advertising correctly.
**Why it happens:** Windows Firewall on "Public" network profile blocks inbound UDP 5353 by default. "Private" profile allows it via built-in rule.
**How to avoid:** Ensure pods are on "Private" network profile (they should be on 192.168.31.0/24 LAN). Add a pre-flight check that tests mDNS availability.
**Warning signs:** mDNS browse always times out on specific machines.

### Pitfall 5: Server DHCP Reservation Missing
**What goes wrong:** Server IP changes on DHCP renewal, breaking all pod connections simultaneously.
**Why it happens:** CLAUDE.md already notes: "Server DHCP reservation needed: MAC 10-FF-E0-80-B1-A7 -> 192.168.31.23" -- this is listed as a current blocker.
**How to avoid:** (1) Set DHCP reservation on router, (2) mDNS re-discovery provides resilience even without reservation.
**Warning signs:** All 8 pods disconnect simultaneously after router reboot.

## Code Examples

### Existing Server Advertiser (verified from source)
```rust
// crates/racecontrol/src/mdns.rs
// Registers _racecontrol._tcp.local. with TXT records: build_id, venue_id, version
pub fn start_advertiser(port: u16, build_id: &str, venue_id: &str) -> Option<ServiceDaemon> {
    let daemon = ServiceDaemon::new()?;  // error handling omitted for brevity
    let service_info = ServiceInfo::new(
        SERVICE_TYPE,        // "_racecontrol._tcp.local."
        INSTANCE_NAME,       // "RaceControl Server"
        &host_fqdn,          // "{COMPUTERNAME}.local."
        "",                  // auto-detect IPs
        port,                // 8080
        properties,          // build_id, venue_id, version
    )?;
    daemon.register(service_info)?;
    Some(daemon)
}
```

### Existing Agent Browser (verified from source)
```rust
// crates/rc-agent/src/mdns_discovery.rs
pub fn discover_server() -> Option<String> {
    let daemon = ServiceDaemon::new().ok()?;
    let receiver = daemon.browse(SERVICE_TYPE).ok()?;
    // Loop with 5s deadline, return first ServiceResolved with IPv4 address
    // Returns: Some("ws://192.168.31.23:8080/ws/agent") or None
}
```

### Proposed: Re-Discovery Helper
```rust
// Add to crates/rc-agent/src/mdns_discovery.rs
/// Re-discover server with a shorter timeout (3s) for use during reconnection.
/// Returns a new WS URL only if it differs from the current URL.
pub fn rediscover_server(current_url: &str) -> Option<String> {
    let result = discover_server_with_timeout(Duration::from_secs(3));
    match result {
        Some(ref url) if url != current_url => result,
        _ => None, // Same URL or not found -- no change
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| mdns-sd 0.12 | mdns-sd 0.19 | 2026-04-04 | 7 major versions behind; 0.19 has better IPv6, interface control, memory optimization |
| mDNS at startup only | mDNS re-discovery on reconnect | Phase 332 | Handles DHCP changes, server migrations |
| No mDNS pre-flight | Pre-flight check for mDNS | Phase 332 | Detects firewall/network issues at boot |

## Open Questions

1. **mdns-sd version upgrade priority**
   - What we know: 0.12.0 works, 0.19.0 has breaking changes but our API surface is compatible
   - What's unclear: Whether 0.17+'s loopback-by-default causes issues on Windows 11
   - Recommendation: Keep 0.12 for this phase, upgrade in a separate phase with canary testing on Pod 8

2. **Re-discovery frequency**
   - What we know: Every reconnect attempt is too aggressive (daemon creation overhead). Never re-discovering is the current bug.
   - What's unclear: Optimal frequency for the Racing Point LAN (8 pods, 1 server)
   - Recommendation: Every 5 failed attempts (roughly every 30-60 seconds given backoff delays)

3. **Server DHCP reservation**
   - What we know: Listed as a current blocker in CLAUDE.md. mDNS is a workaround, not a fix.
   - What's unclear: Whether Uday has router admin access to set the reservation
   - Recommendation: mDNS re-discovery provides resilience regardless; DHCP reservation is a separate ops task

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| mdns-sd crate | mDNS discovery | Yes | 0.12.0 (Cargo.lock) | TOML `core.url` static config |
| Windows mDNS (dnscache) | UDP 5353 multicast | Yes (built-in Win11) | N/A | mdns-sd crate handles its own socket |
| Private network profile | UDP 5353 firewall rule | Assumed yes (LAN) | N/A | Manual firewall rule |

**Missing dependencies with no fallback:** None -- mDNS is best-effort with TOML fallback already built in.

## Effort Estimate

| Work Item | Effort | Risk |
|-----------|--------|------|
| Add mDNS re-discovery to reconnect loop | Small (30-50 lines) | LOW -- isolated change in main.rs reconnect section |
| Add pre-flight check for mDNS | Small (20-30 lines) | LOW -- read-only diagnostic |
| Test on Pod 8 canary | Medium (manual verification) | LOW |
| Optional: Upgrade mdns-sd 0.12 -> 0.19 | Medium (API surface review + testing) | MEDIUM -- breaking changes at 0.14, 0.15, 0.17 |

**Total estimate:** 1-2 hours for core work (re-discovery + pre-flight). Add 1 hour for optional version upgrade.

## Project Constraints (from CLAUDE.md)

Key standing rules that apply to this phase:
- **No `.unwrap()` in production Rust** -- use `?`, `.ok()`, or match (already followed in existing mDNS code)
- **Static CRT** -- `.cargo/config.toml` `+crt-static` (no new DLL dependencies)
- **Test before upload** -- `cargo test` + deploy to Pod 8 first
- **`touch build.rs` before release builds** -- after new commits
- **Boot Resilience pattern** -- periodic re-fetch, not single-fetch-at-boot (this is exactly what Phase 332 fixes for mDNS)
- **Never hold a lock across `.await`** -- mDNS uses `spawn_blocking`, not async; `active_url` RwLock must be held briefly
- **Session 1 requirement** -- mDNS doesn't affect this (no GUI operations)
- **Cascade updates** -- if reconnect logic changes, update error catalog + service reference docs

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/mdns.rs` -- existing server advertiser implementation
- `crates/rc-agent/src/mdns_discovery.rs` -- existing agent browser implementation
- `crates/rc-agent/src/main.rs:732-746` -- mDNS startup integration
- `crates/racecontrol/src/main.rs:1242-1253` -- mDNS server startup integration
- `crates/rc-common/src/config_schema.rs:92-124` -- CoreConfig with mdns_enabled field
- `Cargo.lock` -- mdns-sd pinned at 0.12.0

### Secondary (MEDIUM confidence)
- [mdns-sd CHANGELOG.md](https://github.com/keepsimple1/mdns-sd/blob/main/CHANGELOG.md) -- version history and breaking changes
- [mdns-sd crates.io](https://crates.io/crates/mdns-sd) -- latest version 0.19.0
- [Microsoft mDNS in the Enterprise](https://techcommunity.microsoft.com/t5/networking-blog/mdns-in-the-enterprise/ba-p/3275777) -- Windows mDNS behavior
- `.planning/research-v26.1/STACK.md:29-32` -- previous research on mdns-sd Windows caveats

### Tertiary (LOW confidence)
- Windows 11 mDNS intermittent failures (community reports, no Microsoft confirmation)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- mdns-sd already in Cargo.toml and working
- Architecture: HIGH -- existing implementation fully reviewed, gap clearly identified
- Pitfalls: HIGH -- Windows mDNS caveats documented from prior research (v26.1) and verified against current code

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable domain, 30-day validity)
