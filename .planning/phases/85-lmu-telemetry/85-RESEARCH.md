# Phase 85: LMU Telemetry - Research

**Researched:** 2026-03-21 IST
**Domain:** rFactor 2 shared memory (Windows named shared memory, fixed C struct layout)
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
None — all areas are Claude's discretion.

### Claude's Discretion (all areas)
- rFactor 2 shared memory mapped files: `$rFactor2SMMP_Telemetry$`, `$rFactor2SMMP_Scoring$`
- Use same `winapi::OpenFileMappingW` + `MapViewOfFile` pattern as iRacing and AC adapters
- rF2 Scoring struct has lap times and sector splits (unlike iRacing which lacks sectors)
- Session transition handling: detect session change via scoring data, reset lap tracking
- PlayableSignal: use rF2 driving flag from telemetry (replaces 90s process fallback)
- `poll_lap_completed()` returns `LapData` with `sim_type: SimType::LeMansUltimate`
- Follow iRacing adapter structure closely — same shared memory pattern, similar wiring
- `read_is_on_track()` trait override inside `impl SimAdapter` (not inherent method — learned from Phase 84 checker)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TEL-LMU-01 | LMU shared memory read using rF2 shared memory plugin ($rFactor2SMMP_*) | rF2 plugin exposes two named file maps; `OpenFileMappingW` + `MapViewOfFile` is the established pattern already used for iRacing and AC |
| TEL-LMU-02 | Lap times and sector splits extracted from rF2 scoring data | `rF2VehicleScoring.mLastLapTime` (f64, seconds), `mLastSector1` (f64), `mLastSector2` (f64, cumulative) — S3 derived as `mLastLapTime - mLastSector2` |
| TEL-LMU-03 | Each completed lap emits LapCompleted with sim_type=LMU | `mTotalLaps` (i16) increments on completion; `SimType::LeMansUltimate` already defined in rc-common/src/types.rs |
</phase_requirements>

---

## Summary

Le Mans Ultimate (LMU) uses the rFactor 2 shared memory plugin (`rF2SharedMemoryMapPlugin`) to expose telemetry via named Windows file maps. Two maps are relevant: `$rFactor2SMMP_Scoring$` (5 Hz, lap times + sector splits + session state) and `$rFactor2SMMP_Telemetry$` (50 Hz, vehicle inputs + RPM + speed). The plugin must be installed in LMU's plugins folder — it is not a built-in feature; it ships with the game via Steam and is active by default.

The rF2 data structures use **fixed C struct layout** — not a dynamic variable table like iRacing. Fields are at predictable byte offsets. The Scoring buffer contains `rF2VehicleScoring` records (one per vehicle, up to 128), from which the player's vehicle is identified by `mIsPlayer == 1`. Lap completion is detected via `mTotalLaps` (i16) incrementing. Sector times are in `mLastSector1` (f64, S1 only), `mLastSector2` (f64, S1+S2 cumulative), with S3 derived as `mLastLapTime - mLastSector2`. All times are in seconds and must be converted to milliseconds (multiply by 1000, cast to u32).

Each buffer starts with version fields (`mVersionUpdateBegin` / `mVersionUpdateEnd`, both u32). A torn-read guard reads `mVersionUpdateBegin`, copies data, reads `mVersionUpdateEnd` — if equal, data is consistent. This replaces iRacing's double-buffer tick-lock with a simpler version equality check.

**Primary recommendation:** Implement `LmuAdapter` as a near-clone of `IracingAdapter`, replacing the variable-lookup approach with fixed struct offsets. Open both `$rFactor2SMMP_Scoring$` and `$rFactor2SMMP_Telemetry$` file maps. Read scoring at each `read_telemetry()` call (100ms polling interval is faster than 5Hz update rate — harmless, reads same data). Use `mControl == 0` (local player) or `mIsPlayer == 1` to identify the player vehicle.

---

## Standard Stack

### Core (all already in workspace — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| winapi | 0.3 | `OpenFileMappingW`, `MapViewOfFile`, `UnmapViewOfFile`, `CloseHandle` | Already used for iRacing and AC adapters |
| uuid | 1.x | `LapData.id` generation | Workspace dependency |
| chrono | 0.4 | `LapData.created_at` | Workspace dependency |
| tracing | 0.1 | Structured logging | Workspace dependency |

**No new crate dependencies required.** The rF2 adapter follows the exact same `winapi` shared memory pattern as the existing iRacing and AC adapters.

**Installation:** No `cargo add` needed — all dependencies already in workspace `Cargo.toml`.

---

## Architecture Patterns

### Recommended File Structure

```
crates/rc-agent/src/sims/
├── mod.rs          — add: pub mod lmu;
├── lmu.rs          — NEW: LmuAdapter (this phase)
├── iracing.rs      — Reference: closest pattern
└── assetto_corsa.rs — Reference: original ShmHandle pattern
```

### Pattern 1: Fixed-Offset Struct Reading (rF2 differs from iRacing)

**What:** rF2 uses a fixed C struct layout — offsets never change. No variable lookup table needed.

**When to use:** Always — this is the only approach for rF2 shared memory.

**Key difference from iRacing:** iRacing has a dynamic variable table scanned at connect time. rF2 has a fixed struct where fields are always at the same byte offsets.

```rust
// rF2 scoring buffer layout (at the start of $rFactor2SMMP_Scoring$):
// Offset 0:   mVersionUpdateBegin (u32) — incremented before write
// Offset 4:   mVersionUpdateEnd   (u32) — incremented after write
// Offset 8:   mBytesUpdatedHint   (i32)
// Offset 12:  rF2ScoringInfo      (variable-size struct)
//             └─ mNumVehicles     (i32, at some fixed offset within ScoringInfo)
//             └─ mSession         (i32, session type)
//             └─ mGamePhase       (u8)
//             └─ ...
// After ScoringInfo: rF2VehicleScoring array[128]

// Source: TheIronWolfModding/rF2SharedMemoryMapPlugin rF2Data.cs
```

**Critical approach for this phase:** Rather than computing precise C struct offsets from scratch (error-prone), use the C# `rF2Data.cs` from the plugin as the ground truth for field layout, then replicate key offsets in Rust. The key fields needed per vehicle are:

```rust
// rF2VehicleScoring struct (368 bytes per vehicle)
// Source: rF2Data.cs, TheIronWolfModding/rF2SharedMemoryMapPlugin
struct Rf2VehicleScoring {
    // ... (earlier fields)
    m_best_lap_time: f64,     // best lap time in seconds (negative = none)
    m_last_lap_time: f64,     // last completed lap time in seconds
    m_cur_sector1: f64,       // current in-progress S1 time
    m_cur_sector2: f64,       // current in-progress S1+S2 time
    m_best_sector1: f64,      // best S1
    m_best_sector2: f64,      // best S1+S2 cumulative
    m_last_sector1: f64,      // last completed S1 time
    m_last_sector2: f64,      // last completed S1+S2 cumulative
    m_lap_start_et: f64,      // elapsed time when this lap started
    m_total_laps: i16,        // laps completed (lap counter)
    m_sector: i8,             // 0=S3/finish, 1=S1, 2=S2
    m_control: i8,            // -1=none, 0=local player, 1=local AI, 2=remote, 3=replay
    m_in_pits: u8,            // 1 if between pit entrance and pit exit
    m_is_player: u8,          // 1 if this is the player's vehicle
    m_finish_status: i8,      // 0=none, 1=finished, 2=dnf, 3=dq
    // ...
}
```

### Pattern 2: Version-Field Torn-Read Guard

**What:** rF2 uses a begin/end version pair at offset 0 of each buffer. Read begin, copy data, read end. If begin == end, the read is consistent.

**Example:**
```rust
// Source: rF2SharedMemoryMapPlugin README, TheIronWolfModding
fn read_scoring_safe(ptr: *const u8) -> bool {
    let begin = unsafe { std::ptr::read_unaligned(ptr as *const u32) };
    // ... read your data fields ...
    let end = unsafe { std::ptr::read_unaligned(ptr.add(4) as *const u32) };
    begin == end  // true = consistent read
}
```

**Retry:** Retry up to 3 times on torn reads, same as the iRacing double-buffer retry.

### Pattern 3: Lap Completion via mTotalLaps

**What:** `mTotalLaps` (i16) on the player's `rF2VehicleScoring` record increments when a lap is crossed. When `mTotalLaps > last_lap_count`, a lap completed.

**Sector derivation:**
```rust
// Source: TransitionTracker.cs, TheIronWolfModding/rF2SharedMemoryMapPlugin
let s1_ms = (veh.m_last_sector1 * 1000.0) as u32;
let s2_ms = ((veh.m_last_sector2 - veh.m_last_sector1) * 1000.0) as u32;
let s3_ms = ((veh.m_last_lap_time - veh.m_last_sector2) * 1000.0) as u32;
let lap_time_ms = (veh.m_last_lap_time * 1000.0) as u32;
```

**Guard:** Only emit a lap when `m_last_lap_time > 0.0` (negative or zero means no valid time).

### Pattern 4: Player Vehicle Identification

```rust
// mIsPlayer == 1 is the primary check.
// mControl == 0 is secondary confirmation (local human player).
// Source: rF2Data.cs
let is_player = veh.m_is_player == 1;
```

### Pattern 5: PlayableSignal (replaces 90s process fallback)

The CONTEXT.md specifies using an rF2 "driving flag" from telemetry. Based on research, the cleanest signal available is `mGamePhase >= 4` (Countdown or later) combined with `mIsPlayer == 1` found in scoring. The scoring buffer is available at 5 Hz as soon as LMU is in-session.

Implement `read_is_on_track()` trait override that returns `Some(true)` when:
- Scoring buffer is readable (version fields valid)
- Player vehicle found (`m_is_player == 1`)
- `m_game_phase >= 4` (Countdown=4, GreenFlag=5, FullCourseYellow=6)

This is wired into `event_loop.rs` in the `Some(SimType::LeMansUltimate)` match arm, following the iRacing pattern exactly.

### Pattern 6: Session Transition Detection

**mSession** field in `rF2ScoringInfo` changes between sessions. Track `last_session_type: i32` and reset lap state when it changes. Alternative: track `mGamePhase` transitions from SessionOver/SessionStopped (7/8) back to Countdown (4) as a session boundary signal.

### Anti-Patterns to Avoid

- **Hard-coding offsets without verification:** The C struct offsets must be cross-checked against `rF2Data.cs`. The vehicle array layout (368 bytes per vehicle) can drift between plugin versions.
- **Blocking on scoring rate:** Scoring updates at 5 Hz. Polling at 100 ms (10 Hz) is fine — you'll often read the same data twice. Do not block waiting for a version change.
- **Opening only Scoring:** Telemetry buffer is needed for speed/RPM/throttle/brake for `TelemetryFrame`. Open both maps.
- **Not handling m_last_lap_time <= 0:** rF2 uses negative values to signal "no valid lap time." Always guard.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Shared memory IPC | Custom named pipe or socket | `winapi::OpenFileMappingW` | rF2 only speaks named file maps |
| Struct deserialization | serde derive or bytemuck | Manual `read_unaligned` with verified offsets | No dependency on rF2 SDK; offsets from C# source |
| Lap timing validation | Custom timer accumulation | `mLastLapTime > 0.0` guard from rF2 | rF2 signals invalid time with <= 0 |
| Torn-read protection | Mutex or channel | mVersionUpdateBegin/End equality check | Plugin's own documented synchronization mechanism |

**Key insight:** rF2's fixed struct layout means there is no runtime lookup needed — just verified byte offsets. The plugin does all the heavy lifting of converting the internal ISI game structures into mapped memory.

---

## Common Pitfalls

### Pitfall 1: Plugin Not Installed / Not Loaded

**What goes wrong:** `OpenFileMappingW` returns null; `GetLastError()` returns ERROR_FILE_NOT_FOUND. The file maps don't exist until the rF2SharedMemoryMapPlugin DLL is loaded by LMU.

**Why it happens:** LMU ships with the plugin but it must be in the correct plugins folder and not disabled. The CustomPluginVariables.JSON `UnsubscribedBuffersMask` setting can also suppress specific buffers (Scoring=2, Weather=128).

**How to avoid:** `connect()` must return a clear error: "LMU shared memory not found — rF2SharedMemoryMapPlugin may not be loaded." The adapter should retry on the next telemetry poll cycle (same as iRacing pattern — connect() is called by the event loop when adapter is not connected).

**Warning signs:** `connect()` always returns `Err` even with LMU running.

### Pitfall 2: Vehicle Array Index vs Player Flag

**What goes wrong:** Reading vehicle[0] assuming it's the player, getting AI data instead.

**Why it happens:** rF2 puts vehicles in session order, not player-first order.

**How to avoid:** Always scan the vehicle array for `m_is_player == 1`. If not found, return `Ok(None)` from `read_telemetry()`.

### Pitfall 3: Sector Time Cumulative vs Differential

**What goes wrong:** Using `mLastSector2` directly as S2 time, producing wildly wrong splits.

**Why it happens:** `mLastSector2` is cumulative (S1 + S2), not S2 alone.

**How to avoid:** Always derive: S2 = `mLastSector2 - mLastSector1`, S3 = `mLastLapTime - mLastSector2`.

### Pitfall 4: First-Packet Safety (same as iRacing)

**What goes wrong:** `mTotalLaps` is already 3 when we first open the buffer mid-session. We emit 3 spurious laps.

**Why it happens:** Adapter connecting mid-session.

**How to avoid:** Same `first_read` flag as iRacing: on first successful read, snapshot `last_lap_count = m_total_laps`, set `first_read = false`, do NOT emit a lap.

### Pitfall 5: read_is_on_track() Must Be Trait Override, Not Inherent Method

**What goes wrong:** Implementing `read_is_on_track_from_shm()` as an inherent method without overriding the trait method. `dyn SimAdapter` trait dispatch calls the default `None` implementation.

**Why it happens:** Rust trait objects dispatch through the vtable. The trait method `read_is_on_track()` defaults to `None`. An inherent method is not visible through `dyn SimAdapter`.

**How to avoid:** Follow iRacing's exact pattern — add `fn read_is_on_track(&self) -> Option<bool>` inside `impl SimAdapter for LmuAdapter` that delegates to the inherent method.

### Pitfall 6: Struct Size / Offset Drift Between Plugin Versions

**What goes wrong:** Offsets computed from an old version of `rF2Data.cs` don't match the deployed plugin DLL version.

**Why it happens:** Plugin updates occasionally change struct layout.

**How to avoid:** Document which plugin version the offsets were sourced from. Use `rF2Data.cs` from the master branch of TheIronWolfModding/rF2SharedMemoryMapPlugin as the canonical source. Verify with a live LMU session dump before shipping.

---

## Code Examples

Verified patterns from official sources:

### Opening rF2 Shared Memory Buffer
```rust
// Source: existing iRacing and AC adapters in this codebase
// Pattern is identical — only the name string changes
#[cfg(windows)]
fn open_lmu_scoring_shm() -> Result<ShmHandle> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let name = "$rFactor2SMMP_Scoring$";
    let wide: Vec<u16> = OsStr::new(name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = winapi::um::memoryapi::OpenFileMappingW(
            winapi::um::memoryapi::FILE_MAP_READ,
            0,
            wide.as_ptr(),
        );
        if handle.is_null() {
            anyhow::bail!(
                "OpenFileMappingW failed for LMU scoring shm (error={}) — \
                 is LMU running with rF2SharedMemoryMapPlugin loaded?",
                winapi::um::errhandlingapi::GetLastError()
            );
        }
        let ptr = winapi::um::memoryapi::MapViewOfFile(
            handle,
            winapi::um::memoryapi::FILE_MAP_READ,
            0, 0, 0,
        );
        if ptr.is_null() {
            winapi::um::handleapi::CloseHandle(handle);
            anyhow::bail!("MapViewOfFile failed for LMU scoring shm");
        }
        Ok(ShmHandle { _handle: handle, ptr: ptr as *const u8, _size: 0 })
    }
}
```

### Version-Field Torn-Read Guard
```rust
// Source: README.md — TheIronWolfModding/rF2SharedMemoryMapPlugin
// Returns true if read is consistent (not torn)
#[cfg(windows)]
fn is_consistent_read(ptr: *const u8) -> bool {
    // mVersionUpdateBegin at offset 0, mVersionUpdateEnd at offset 4
    let begin = unsafe { std::ptr::read_unaligned(ptr as *const u32) };
    let end   = unsafe { std::ptr::read_unaligned(ptr.add(4) as *const u32) };
    begin == end
}
```

### Sector Time Derivation
```rust
// Source: TransitionTracker.cs — TheIronWolfModding/rF2SharedMemoryMapPlugin
fn sector_times_ms(last_lap_s: f64, last_s1_s: f64, last_s2_cumul_s: f64)
    -> (Option<u32>, Option<u32>, Option<u32>)
{
    if last_lap_s <= 0.0 || last_s1_s <= 0.0 || last_s2_cumul_s <= 0.0 {
        return (None, None, None);
    }
    let s1 = (last_s1_s * 1000.0) as u32;
    let s2 = ((last_s2_cumul_s - last_s1_s) * 1000.0) as u32;
    let s3 = ((last_lap_s - last_s2_cumul_s) * 1000.0) as u32;
    (Some(s1), Some(s2), Some(s3))
}
```

### Lap Completion Detection (core loop)
```rust
// Source: TransitionTracker.cs — lap detection pattern
// veh = &rF2VehicleScoring for the player vehicle
if veh.m_total_laps > self.last_lap_count && !self.first_read {
    if veh.m_last_lap_time > 0.0 {
        let lap_time_ms = (veh.m_last_lap_time * 1000.0) as u32;
        let (s1, s2, s3) = sector_times_ms(
            veh.m_last_lap_time,
            veh.m_last_sector1,
            veh.m_last_sector2,
        );
        self.pending_lap = Some(LapData {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: String::new(),
            driver_id: String::new(),
            pod_id: self.pod_id.clone(),
            sim_type: SimType::LeMansUltimate,
            track: self.current_track.clone(),
            car: self.current_car.clone(),
            lap_number: veh.m_total_laps as u32,
            lap_time_ms,
            sector1_ms: s1,
            sector2_ms: s2,
            sector3_ms: s3,
            valid: true,
            session_type: self.current_session_type,
            created_at: Utc::now(),
        });
    }
    self.last_lap_count = veh.m_total_laps;
}
```

### event_loop.rs Integration (LMU PlayableSignal arm)
```rust
// In the game_check_interval match arm — follows iRacing pattern exactly
// Source: crates/rc-agent/src/event_loop.rs lines 513-530
Some(rc_common::types::SimType::LeMansUltimate) => {
    if let Some(ref adapter) = state.adapter {
        if let Some(true) = adapter.read_is_on_track() {
            if matches!(conn.launch_state, LaunchState::WaitingForLive { .. }) {
                tracing::info!("[billing] LMU IsOnTrack=true — emitting AcStatus::Live");
                let msg = AgentMessage::GameStatusUpdate {
                    pod_id: state.pod_id.clone(),
                    ac_status: AcStatus::Live,
                    sim_type: Some(rc_common::types::SimType::LeMansUltimate),
                };
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = ws_tx.send(Message::Text(json.into())).await;
                }
                conn.launch_state = LaunchState::Live;
            }
        }
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Process-based 90s fallback for LMU PlayableSignal | `read_is_on_track()` via `mGamePhase` from scoring | Phase 85 | Billing starts when player is on track, not 90s after process detection |
| No sector splits for LMU | Full S1/S2/S3 from rF2 scoring struct | Phase 85 | Leaderboard shows sector splits |

**iRacing vs LMU shared memory differences:**
- iRacing: dynamic variable table (scan headers at connect time, offsets vary per SDK version)
- rF2/LMU: fixed C struct layout (offsets are constant, no scan needed)
- iRacing: no sector splits in standard telemetry variables
- rF2/LMU: sector splits natively available in `rF2VehicleScoring`

---

## Integration Points Summary

| File | Change | Notes |
|------|--------|-------|
| `crates/rc-agent/src/sims/mod.rs` | Add `pub mod lmu;` | One line |
| `crates/rc-agent/src/main.rs` | Add `use sims::lmu::LmuAdapter;` + `SimType::LeMansUltimate => Some(Box::new(LmuAdapter::new(pod_id.clone())))` | In the adapter creation match (lines ~407-410) |
| `crates/rc-agent/src/event_loop.rs` | Add `Some(SimType::LeMansUltimate)` arm in PlayableSignal dispatch | Between iRacing arm and catch-all (line ~531) |
| `crates/rc-agent/src/sims/lmu.rs` | New file: `LmuAdapter` struct + `impl SimAdapter` | Full implementation |

`SimType::LeMansUltimate` is **already defined** in `rc-common/src/types.rs` (verified: line 14). No changes needed to `rc-common`.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | `.cargo/config.toml` (static CRT, existing) |
| Quick run command | `cargo test -p rc-agent sims::lmu` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | Notes |
|--------|----------|-----------|-------------------|-------|
| TEL-LMU-01 | connect() fails without LMU running | unit | `cargo test -p rc-agent sims::lmu::tests::test_connect_no_shm` | Can run on CI — no LMU required |
| TEL-LMU-01 | connect() returns Err when shm not found | unit | same test | Always fails on non-Windows or without plugin |
| TEL-LMU-02 | Sector derivation: S1/S2/S3 from cumulative fields | unit | `cargo test -p rc-agent sims::lmu::tests::test_sector_derivation` | Pure math — no SHM needed |
| TEL-LMU-02 | Sector times None when mLastLapTime <= 0.0 | unit | `cargo test -p rc-agent sims::lmu::tests::test_sector_guard` | Pure logic |
| TEL-LMU-03 | mTotalLaps increment fires LapData with sim_type=LMU | unit | `cargo test -p rc-agent sims::lmu::tests::test_lap_completed_event` | Mirrors iRacing test pattern |
| TEL-LMU-03 | First-packet safety: no false lap on connect | unit | `cargo test -p rc-agent sims::lmu::tests::test_first_packet_safety` | Mirrors iRacing test |
| All | Session transition resets lap state | unit | `cargo test -p rc-agent sims::lmu::tests::test_session_transition_resets_lap` | Mirrors iRacing test |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent sims::lmu`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/sims/lmu.rs` — contains all unit tests (created in Wave 1, tests in same file per iRacing pattern)

*(No separate test file needed — Rust convention is `#[cfg(test)] mod tests` inside the module file)*

---

## Open Questions

1. **Exact byte offsets for rF2VehicleScoring fields**
   - What we know: Field names and types from `rF2Data.cs` (C# marshaling structs). Vehicle record is 368 bytes. Key fields: `mIsPlayer` (u8), `mControl` (i8), `mTotalLaps` (i16), `mLastLapTime` (f64), `mLastSector1` (f64), `mLastSector2` (f64).
   - What's unclear: Exact byte offset of each field within the 368-byte struct without access to the raw C++ headers (`rF2State.h` is not in the public plugin repo; only the C# marshaling struct is).
   - Recommendation: Read `rF2Data.cs` carefully — C# `[StructLayout(LayoutKind.Sequential)]` with explicit field ordering and `MarshalAs` attributes gives the byte layout. Compute offsets from field order + types (8-byte alignment for f64). Cross-verify by running the reference C# monitor against a live LMU session (or use the community Rust implementations as secondary reference).

2. **rF2ScoringInfo size (offset to vehicle array)**
   - What we know: `rF2Scoring` starts with version header (12 bytes: 4+4+4), then `rF2ScoringInfo`, then vehicle array.
   - What's unclear: Exact byte size of `rF2ScoringInfo` — needed to compute where the vehicle array starts.
   - Recommendation: Use the C# `rF2Data.cs` struct definition to compute size via `Marshal.SizeOf`. Document the computed offset as a named constant with a comment referencing the source version.

3. **LMU-specific plugin version and any LMU deviations from rF2**
   - What we know: LMU ships `rF2SharedMemoryMapPlugin` via Steam. Community reports confirm it works (SimHub, LMU Trace app, DR Sim Manager all use it successfully).
   - What's unclear: Whether LMU's bundled plugin version has any struct deviations from the TheIronWolfModding public repo.
   - Recommendation: Low risk — multiple third-party tools use the same structs successfully with LMU. Treat as identical to rF2. If offsets produce garbage data on first test, check the bundled DLL version.

---

## Sources

### Primary (HIGH confidence)
- [TheIronWolfModding/rF2SharedMemoryMapPlugin — rF2Data.cs](https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin/blob/master/Monitor/rF2SMMonitor/rF2SMMonitor/rF2Data.cs) — struct field names, types, vehicle record structure
- [TheIronWolfModding/rF2SharedMemoryMapPlugin — TransitionTracker.cs](https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin/blob/master/Monitor/rF2SMMonitor/rF2SMMonitor/TransitionTracker.cs) — lap detection logic, sector derivation, session transition detection
- [TheIronWolfModding/rF2SharedMemoryMapPlugin — README.md](https://github.com/TheIronWolfModding/rF2SharedMemoryMapPlugin/blob/master/README.md) — buffer names, version synchronization, subscription masks
- Existing codebase: `crates/rc-agent/src/sims/iracing.rs` — ShmHandle pattern, connect/disconnect lifecycle, first_read safety, test patterns
- Existing codebase: `crates/rc-agent/src/sims/assetto_corsa.rs` — original ShmHandle struct
- Existing codebase: `crates/rc-agent/src/event_loop.rs` — PlayableSignal dispatch pattern (lines 487-550)

### Secondary (MEDIUM confidence)
- [Le Mans Ultimate Telemetry Wiki](https://lemansultimate.wiki.gg/wiki/Telemetry) — confirms rF2 plugin is the telemetry mechanism for LMU
- [LMU Trace](https://lmutrace.com/) — third-party tool using same rF2 shared memory with LMU (confirms compatibility)
- [DR Sim Manager LMU page](https://docs.departedreality.com/dr-sim-manager/general/sources/le-mans-ultimate) — another tool confirming rF2 plugin works for LMU

### Tertiary (LOW confidence — for awareness only)
- Community forum discussion: early reports of plugin compatibility issues are outdated (2023 LMU launch era); current LMU ships the plugin bundled.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — identical to existing iRacing adapter, no new dependencies
- Architecture: HIGH — rF2Data.cs and TransitionTracker.cs provide direct field and logic reference
- Pitfalls: HIGH — all pitfalls are either from existing code lessons (first_read, trait override) or clearly documented rF2 quirks (cumulative sector fields, mLastLapTime <= 0 guard)
- Struct byte offsets: MEDIUM — field types/names confirmed, exact offsets require careful computation from C# layout

**Research date:** 2026-03-21 IST
**Valid until:** 2026-06-21 (rF2 plugin struct layout is stable; changes are rare and versioned)
