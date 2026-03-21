# Phase 86: AC EVO Telemetry - Research

**Researched:** 2026-03-21
**Domain:** Rust shared memory adapter (SimAdapter trait), AC EVO undocumented telemetry
**Confidence:** MEDIUM — codebase patterns HIGH, EVO telemetry API LOW (early access, undocumented)

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **Reuse existing AC adapter's shared memory struct layout** (ACC format: `acpmf_physics`, `acpmf_graphics`, `acpmf_static`)
- AC EVO is built on ACC's engine — likely uses same or similar shared memory layout
- If fields are populated: extract lap times, sector splits, emit LapCompleted
- If fields are empty/zero: log warning, adapter reports no telemetry, billing continues via process fallback
- Feature-flagged: adapter only activates when `SimType::AssettoCorsaEvo` is the current game
- Never panic on unpopulated fields — all reads check for zero/default values
- If shared memory map doesn't exist (EVO doesn't use ACC format): `connect()` returns Ok but `is_connected = false`
- Tracing warns once per session, not per poll cycle
- Game launch and billing work regardless of telemetry success

### Claude's Discretion (all implementation details)
- Whether to create a new EVO-specific adapter file or extend the existing AC adapter with EVO support
- Exact shared memory map names to try (ACC names vs potential EVO-specific names)
- PlayableSignal: use physics data (non-zero speed/RPM) or separate IsOnTrack equivalent
- `read_is_on_track()` trait override for EVO
- How to distinguish AC1 vs EVO if both use similar shared memory names

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TEL-EVO-01 | AC EVO shared memory is read using ACC-format struct layout when data is available | AssettoCorsaAdapter patterns — ShmHandle, struct offsets, read_f32/read_i32/read_wchar_string helpers fully reusable |
| TEL-EVO-02 | If telemetry fields are unpopulated or API changes, adapter logs warning and continues without crashing | Warn-once pattern: `connected = false` on connect failure + zero-check guards in read path; billing falls through to 90s process fallback |
| TEL-EVO-03 | When lap data is available, emitted as LapCompleted with sim_type = AC_EVO | `LapData { sim_type: SimType::AssettoCorsaEvo, ... }` — same struct as AC1, different enum variant |
</phase_requirements>

---

## Summary

AC EVO is an Unreal Engine 5 sim in Early Access (released January 2025). It is built on Kunos's ACC engine lineage. Community research and motion sim software developers confirm EVO uses the same `acpmf_physics`, `acpmf_graphics`, `acpmf_static` named shared memory map format as AC1 — but with a critical caveat: **only the physics struct appears reliably populated**. Graphics and static structs may be empty, zero, or not yet implemented by Kunos.

This means `completedLaps`, `iLastTime`, `currentSectorIndex`, and `lastSectorTime` (all in the graphics struct) are likely zero or unreliable. Lap detection via the graphics `COMPLETED_LAPS` counter may not work. The physics struct fields (speed, RPM, gear, throttle, brake) appear functional and are used by motion sims.

The implementation decision is to attempt the same struct layout as AC1, gate all lap-detection logic on non-zero values, and degrade gracefully to the existing 90-second process-based billing fallback for PlayableSignal. This is the correct strategy given EVO's Early Access state.

**Primary recommendation:** Create a new `assetto_corsa_evo.rs` adapter file that reuses AC1's ShmHandle, read helpers, and struct offsets wholesale, but wraps all graphics/static reads in zero-guards, warns once on first zero-state detection, and uses physics data (speed > 0 OR rpm > 0) as the PlayableSignal / `read_is_on_track()` heuristic.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `winapi` | (already in Cargo.toml) | `OpenFileMappingW`, `MapViewOfFile`, `CloseHandle` | Same pattern used by AC1, iRacing, LMU adapters |
| `anyhow` | (already in Cargo.toml) | Error propagation | Project-wide standard |
| `tracing` | (already in Cargo.toml) | Structured logging, warn-once pattern | Project-wide standard |
| `uuid` | (already in Cargo.toml) | `LapData.id` generation | Used by all other lap-emitting adapters |
| `chrono` | (already in Cargo.toml) | `LapData.created_at` timestamp | Used by all other lap-emitting adapters |
| `rc-common` | workspace | `SimType::AssettoCorsaEvo`, `LapData`, `TelemetryFrame` | Shared types crate |

No new dependencies required. All libraries already present in rc-agent's Cargo.toml.

### Supporting
None beyond existing dependencies.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| New `assetto_corsa_evo.rs` file | Extend `assetto_corsa.rs` with EVO variant | New file is cleaner — avoids `sim_type` branching inside methods, no risk of AC1 regression |
| ACC map names (`acpmf_*`) | EVO-specific map names (unknown) | ACC names confirmed by community; EVO-specific names unknown |
| Physics-based PlayableSignal | Separate `IsOnTrack` field | No IsOnTrack in physics struct; speed/RPM >0 is reliable "player is driving" signal |

**Installation:** No new packages needed.

---

## Architecture Patterns

### Recommended Project Structure
```
crates/rc-agent/src/sims/
├── assetto_corsa.rs        # Existing AC1 adapter (DO NOT MODIFY)
├── assetto_corsa_evo.rs    # New EVO adapter (this phase)
├── f1_25.rs
├── iracing.rs
├── lmu.rs
└── mod.rs                  # Add: pub mod assetto_corsa_evo;
```

### Pattern 1: ShmHandle Reuse
**What:** The `ShmHandle` struct (Windows HANDLE + mapped ptr) is private to `assetto_corsa.rs`. The EVO adapter must define its own copy, or the AC adapter must re-export it. The recommended approach is to duplicate the small struct in the new file — it's 4 lines of code.

**When to use:** Every Windows shared memory adapter does this. iRacing and LMU also define their own shm handle wrappers.

**Example:**
```rust
// Mirrors assetto_corsa.rs pattern exactly
#[cfg(windows)]
struct ShmHandle {
    _handle: winapi::shared::ntdef::HANDLE,
    ptr: *const u8,
    _size: usize,
}

#[cfg(windows)]
unsafe impl Send for ShmHandle {}
#[cfg(windows)]
unsafe impl Sync for ShmHandle {}
```

### Pattern 2: Graceful connect() — Ok but not connected
**What:** When EVO shared memory maps don't exist or can't be opened, `connect()` returns `Ok(())` with `self.connected = false` rather than `Err(...)`. This prevents the event loop from logging persistent errors. The event loop retries `connect()` on every telemetry tick when `is_connected()` is false.

**When to use:** Best-effort adapters where telemetry failure is expected/acceptable.

**Example:**
```rust
fn connect(&mut self) -> Result<()> {
    match open_shm("Local\\acpmf_physics") {
        Ok(h) => {
            self.physics_handle = Some(h);
            self.connected = true;
            tracing::info!("[EVO] connected to shared memory (physics)");
        }
        Err(e) => {
            // EVO may not have populated shared memory yet — not an error
            if !self.warned_no_shm {
                tracing::warn!("[EVO] shared memory not available: {} — telemetry disabled, billing via process fallback", e);
                self.warned_no_shm = true;
            }
            // connected stays false — billing continues via 90s process fallback
        }
    }
    Ok(()) // Never return Err — caller treats Err as hard failure
}
```

### Pattern 3: Zero-guard reads on graphics struct
**What:** Community confirms graphics/static structs are often zero/unpopulated in EVO Early Access. Every read from these structs must check for zero before acting.

**When to use:** All lap detection, sector tracking, and static data reads.

**Example:**
```rust
let completed_laps = Self::read_i32(graphics, graphics::COMPLETED_LAPS) as u32;
// Zero-guard: if graphics is unpopulated, completed_laps will be 0 forever
if completed_laps > self.last_lap_count && self.last_lap_count > 0 {
    let lap_ms = Self::read_i32(graphics, graphics::I_LAST_TIME);
    if lap_ms > 0 {
        // Only emit if we have a real time
        self.pending_lap = Some(build_lap_data(lap_ms as u32));
    }
}
```

### Pattern 4: Warn-once per session
**What:** If telemetry fields are consistently zero, log one warning and set a flag. Do not warn on every 100ms poll cycle.

**Example:**
```rust
struct AssettoCorsaEvoAdapter {
    // ...
    warned_empty_graphics: bool,
}

// In read_telemetry():
if completed_laps == 0 && lap_time_ms == 0 && !self.warned_empty_graphics {
    tracing::warn!("[EVO] graphics shared memory appears empty — lap detection disabled. EVO may not yet expose this data.");
    self.warned_empty_graphics = true;
}
```

### Pattern 5: PlayableSignal via read_is_on_track()
**What:** EVO uses the 90s process-based fallback for billing (see event_loop.rs line 554). Optionally, the adapter can implement `read_is_on_track()` using physics data (speed > 5 km/h OR rpm > 500) to fire PlayableSignal earlier and more accurately than the 90s fallback.

**When to use:** If physics struct is reliably populated, this gives better billing accuracy.

**Example:**
```rust
fn read_is_on_track(&self) -> Option<bool> {
    #[cfg(windows)]
    {
        let ph = self.physics_handle.as_ref()?;
        let speed = Self::read_f32(ph, physics::SPEED_KMH);
        let rpm = Self::read_i32(ph, physics::RPMS);
        Some(speed > 5.0 || rpm > 500)
    }
    #[cfg(not(windows))]
    None
}
```

### Pattern 6: Adapter registration in main.rs
**What:** The match arm in `main.rs` around line 398 must be extended with an `AssettoCorsaEvo` arm. Currently it falls through to `_ => None` (heartbeat-only mode).

**Example:**
```rust
SimType::AssettoCorsaEvo => Some(Box::new(AssettoCorsaEvoAdapter::new(
    pod_id.clone(),
))),
```

### Anti-Patterns to Avoid
- **Returning Err from connect() on missing shm:** The event loop calls `connect()` on every tick when not connected — returning Err triggers repeated warn logs. Return Ok with connected=false instead.
- **Panicking on zero lap time:** `LapData.lap_time_ms` must be > 0. Guard every lap emission with `if lap_ms > 0`.
- **Modifying assetto_corsa.rs:** AC1 works. Do not touch it. Create a new file.
- **Single warn_once flag for all issues:** Use separate bool flags for "no shm", "empty graphics", etc. so each distinct failure logs exactly once.
- **Calling disconnect() on empty reads:** Empty/zero telemetry is not a disconnect event. Only call disconnect on actual Err from read_telemetry.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Windows shared memory open | Custom FFI | `winapi::um::memoryapi::OpenFileMappingW` + `MapViewOfFile` | Already used by AC1, iRacing, LMU — proven pattern |
| Unaligned struct reads | Casting raw ptr directly | `std::ptr::read_unaligned` | Struct fields may not be aligned; all existing adapters use this |
| UUID generation for LapData | Custom ID scheme | `uuid::Uuid::new_v4().to_string()` | All adapters do this; consistent format |
| Warn throttling | Custom rate limiter | Simple bool flag `warned_X: bool` per failure type | LMU and iRacing use same pattern |

**Key insight:** The entire SharedMemory infrastructure already exists. The EVO adapter is ~80% copy-paste from AC1 with zero-guard additions and sim_type changed.

---

## Common Pitfalls

### Pitfall 1: Graphics struct all-zeros
**What goes wrong:** `completed_laps` never increments, `i_last_time` always 0, lap detection never fires.
**Why it happens:** EVO Early Access only populates the physics struct. Kunos has not yet implemented the graphics output page.
**How to avoid:** All lap detection branches are gated on `lap_ms > 0`. Log warn-once on first detection. Never emit a LapCompleted with `lap_time_ms = 0`.
**Warning signs:** `completed_laps` stays 0 across multiple laps. `speed_kmh` changes (physics works) but no laps emit.

### Pitfall 2: Shared memory maps may not exist
**What goes wrong:** `OpenFileMappingW` returns null handle. `connect()` fails hard.
**Why it happens:** EVO may not create the named mappings at all if the feature isn't active in this build.
**How to avoid:** All three `open_shm()` calls are individually optional. Only physics is required for any telemetry. Graphics and static can fail silently.
**Warning signs:** connect() error logged at startup — expected behavior, not a bug.

### Pitfall 3: Shared memory names may differ from AC1
**What goes wrong:** AC1 uses `Local\acpmf_physics`. EVO might use different names (e.g., `Local\acevo_physics` or `Local\AC2_physics`).
**Why it happens:** EVO is a separate product on a different Steam App ID. Kunos may namespace differently.
**How to avoid:** Try AC1 names first (confirmed by community as working for motion sims). If all fail, log the exact names tried so it's debuggable.
**Warning signs:** `OpenFileMappingW` fails for all three names simultaneously from day one.

### Pitfall 4: Stale last_lap_count on connect
**What goes wrong:** First poll detects a false lap (stale `completed_laps` from previous session).
**Why it happens:** Graphics struct may have leftover data from prior session if EVO doesn't clear it on launch.
**How to avoid:** Snapshot `completed_laps` on connect (same as AC1 does at line 226). Set `self.last_lap_count = initial_laps`.

### Pitfall 5: disconnect() called on every Err from empty telemetry
**What goes wrong:** Adapter connects, immediately gets zero telemetry, `read_telemetry` returns Err, event loop calls `disconnect()`, then tries to `connect()` again every 100ms forever.
**Why it happens:** Event loop at event_loop.rs:188-191 calls `adapter.disconnect()` on any Err from `read_telemetry`.
**How to avoid:** `read_telemetry()` must return `Ok(None)` when data is zero/empty, not `Err`. Only return `Err` for genuine read failures (null handle, etc.).

### Pitfall 6: mod.rs registration missing
**What goes wrong:** `cargo test` fails with "unresolved module" error.
**Why it happens:** New adapter file needs `pub mod assetto_corsa_evo;` in `sims/mod.rs`, and import in `main.rs`.
**Warning signs:** Compile error referencing assetto_corsa_evo module not found.

---

## Code Examples

Verified patterns from existing codebase:

### Open shared memory (from assetto_corsa.rs)
```rust
// Source: crates/rc-agent/src/sims/assetto_corsa.rs connect()
fn open_shm(name: &str) -> Result<ShmHandle> {
    let wide_name: Vec<u16> = OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let handle = winapi::um::memoryapi::OpenFileMappingW(
            winapi::um::memoryapi::FILE_MAP_READ,
            0,
            wide_name.as_ptr(),
        );
        if handle.is_null() {
            anyhow::bail!("Failed to open shared memory: {}", name);
        }
        let ptr = winapi::um::memoryapi::MapViewOfFile(
            handle,
            winapi::um::memoryapi::FILE_MAP_READ,
            0, 0, 0,
        );
        if ptr.is_null() {
            winapi::um::handleapi::CloseHandle(handle);
            anyhow::bail!("Failed to map view of: {}", name);
        }
        Ok(ShmHandle { _handle: handle, ptr: ptr as *const u8, _size: 0 })
    }
}
```

### Emit LapCompleted with correct sim_type
```rust
// Source: pattern from assetto_corsa.rs, sim_type changed to AssettoCorsaEvo
let lap_data = LapData {
    id: uuid::Uuid::new_v4().to_string(),
    session_id: String::new(),
    driver_id: String::new(),
    pod_id: self.pod_id.clone(),
    sim_type: SimType::AssettoCorsaEvo,  // TEL-EVO-03: must be AssettoCorsaEvo
    track: self.current_track.clone(),
    car: self.current_car.clone(),
    lap_number: completed_laps,
    lap_time_ms: lap_ms,
    sector1_ms: self.sector_times[0],
    sector2_ms: self.sector_times[1],
    sector3_ms: self.sector_times[2],
    valid: is_valid != 0,
    session_type: rc_common::types::SessionType::Practice,
    created_at: Utc::now(),
};
```

### Disconnect pattern (close handles)
```rust
// Source: assetto_corsa.rs disconnect()
fn disconnect(&mut self) {
    #[cfg(windows)]
    {
        if let Some(h) = self.physics_handle.take() {
            unsafe {
                winapi::um::memoryapi::UnmapViewOfFile(h.ptr as *const _);
                winapi::um::handleapi::CloseHandle(h._handle);
            }
        }
        // repeat for graphics_handle, static_handle
    }
    self.connected = false;
}
```

### Struct offsets for physics (from assetto_corsa.rs — reuse unchanged)
```rust
mod physics {
    pub const GAS: usize = 4;
    pub const BRAKE: usize = 8;
    pub const GEAR: usize = 16;
    pub const RPMS: usize = 20;
    pub const SPEED_KMH: usize = 28;
}
mod graphics {
    pub const STATUS: usize = 4;
    pub const COMPLETED_LAPS: usize = 132;
    pub const CURRENT_SECTOR_INDEX: usize = 164;
    pub const I_CURRENT_TIME: usize = 140;
    pub const I_LAST_TIME: usize = 144;
    pub const LAST_SECTOR_TIME: usize = 168;
}
```

### event_loop.rs: EVO falls through to 90s process fallback (existing behavior)
```rust
// Source: event_loop.rs line 552-568
// EVO is handled by the `Some(sim_type)` catch-all arm — 90s process fallback already works.
// If read_is_on_track() is implemented, add an explicit arm like IRacing/LMU.
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| AC1/ACC: all three structs populated | EVO Early Access: only physics reliably populated | Jan 2025 EA launch | Lap detection via graphics struct unreliable |
| AC1: `completedLaps` counter reliable | EVO: `completedLaps` may stay 0 | EVO EA current state | Must zero-guard all lap emission |
| Process-based 90s billing fallback (other sims) | Physics-based `read_is_on_track()` (speed/RPM) | This phase | Earlier, more accurate billing trigger for EVO |

**Deprecated/outdated:**
- Assuming all three AC shared memory structs are populated: graphics and static are empty in EVO as of Early Access builds.

---

## Open Questions

1. **Exact shared memory map names in EVO**
   - What we know: Community confirms `Local\acpmf_physics` works for motion sims
   - What's unclear: Whether `Local\acpmf_graphics` and `Local\acpmf_static` use same names or are absent entirely
   - Recommendation: Try AC1 names. If `acpmf_physics` succeeds but others fail, proceed with physics-only mode. Log which names succeeded/failed at connect time.

2. **Whether Kunos has published or plans to publish EVO telemetry docs**
   - What we know: No official EVO telemetry documentation found as of research date
   - What's unclear: When EVO 1.0 ships, struct layouts may change
   - Recommendation: The feature-flag approach handles this — if EVO breaks, flag off with no code change.

3. **Whether `is_valid` / `STATUS` fields work in EVO**
   - What we know: STATUS (offset 4 in graphics) = AcStatus enum. If graphics is empty, this will read 0 = AC_OFF.
   - What's unclear: EVO may never reach AC_LIVE status via shared memory
   - Recommendation: Do not use EVO's `read_ac_status()` for PlayableSignal — use `read_is_on_track()` via physics instead.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`#[test]` + `#[cfg(test)]`) |
| Config file | none — Cargo workspace standard |
| Quick run command | `cargo test -p rc-agent sims::assetto_corsa_evo` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TEL-EVO-01 | Struct offsets match AC1 (reused constants) | unit | `cargo test -p rc-agent sims::assetto_corsa_evo::tests::test_offset_constants` | Wave 0 |
| TEL-EVO-01 | connect() without EVO running returns Ok (not Err) | unit | `cargo test -p rc-agent sims::assetto_corsa_evo::tests::test_connect_no_shm` | Wave 0 |
| TEL-EVO-02 | read_telemetry() returns Ok(None) when handles are None | unit | `cargo test -p rc-agent sims::assetto_corsa_evo::tests::test_read_telemetry_no_handles` | Wave 0 |
| TEL-EVO-02 | No LapCompleted emitted when lap_ms == 0 | unit | `cargo test -p rc-agent sims::assetto_corsa_evo::tests::test_no_lap_on_zero_time` | Wave 0 |
| TEL-EVO-03 | LapData has sim_type = AssettoCorsaEvo | unit | `cargo test -p rc-agent sims::assetto_corsa_evo::tests::test_lap_sim_type` | Wave 0 |
| TEL-EVO-02 | Gear conversion (-1=R, 0=N, 1=1st) | unit | `cargo test -p rc-agent sims::assetto_corsa_evo::tests::test_gear_conversion` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent sims::assetto_corsa_evo`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/sims/assetto_corsa_evo.rs` — covers TEL-EVO-01, TEL-EVO-02, TEL-EVO-03 (file doesn't exist yet)
- [ ] `pub mod assetto_corsa_evo;` in `sims/mod.rs` — needed before cargo can compile test target

*(All tests live in the new adapter file's `#[cfg(test)] mod tests` block — standard pattern matching all other sim adapters.)*

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/sims/assetto_corsa.rs` — AC1 adapter, all struct offsets, ShmHandle pattern, connect/disconnect, lap detection logic
- `crates/rc-agent/src/sims/mod.rs` — SimAdapter trait definition with all method signatures
- `crates/rc-agent/src/event_loop.rs` lines 160-192, 487-569 — telemetry interval, poll_lap_completed, PlayableSignal dispatch, 90s fallback for EVO
- `crates/rc-agent/src/main.rs` lines 398-418 — adapter creation match, EVO falls through to None (heartbeat-only)
- `crates/rc-common/src/types.rs` — SimType::AssettoCorsaEvo confirmed defined

### Secondary (MEDIUM confidence)
- Steam Community discussion "Telemetry Data :: Assetto Corsa EVO" — confirms physics struct populated, graphics/static empty, lap times not yet available
- SimTools EVO plugin page (simtools.us, uploaded Jan 2025) — confirms limited telemetry, game status not available in early version
- Multiple community reports: motion sims (SimHub, SimTools) use ACC settings for EVO, confirming same shared memory architecture

### Tertiary (LOW confidence)
- Assumed shared memory map names (`Local\acpmf_physics` etc.) work for EVO — confirmed by motion sim community but not by official Kunos documentation
- EVO 2026 state: no official changelog found; telemetry status may have improved since Jan 2025 EA launch

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies already in project, identical to AC1 adapter
- Architecture: HIGH — SimAdapter trait, event_loop integration, LapData struct all fully understood from source
- EVO shared memory API: LOW — undocumented, Early Access, confirmed partially broken (graphics/static empty)
- Pitfalls: MEDIUM — derived from codebase analysis + community reports, not official docs

**Research date:** 2026-03-21 IST
**Valid until:** 2026-04-21 (30 days — EVO is Early Access, API may change on any update)
