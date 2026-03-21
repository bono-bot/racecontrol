# Phase 84: iRacing Telemetry — Research

**Researched:** 2026-03-21 IST
**Domain:** iRacing IRSDK shared memory, Rust winapi, SimAdapter trait
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
All areas are Claude's discretion — see below.

### Claude's Discretion
- iRacing shared memory mapped file name: `Local\\IRSDKMemMapFileName`
- Use same `winapi::OpenFileMappingW` + `MapViewOfFile` pattern as AC adapter in `sims/assetto_corsa.rs`
- Double-buffer tick synchronization for reading shared memory safely
- YAML session info string parsing for track name, car, session type
- Session transition handling: re-open shared memory handle when session UID changes
- Pre-flight check: read `app.ini` for `irsdkEnableMem=1`, warn staff via tracing if missing (don't block launch)
- PlayableSignal integration: once this adapter exists, it replaces the 90s process fallback from Phase 82
- `poll_lap_completed()` returns `LapData` with `sim_type: SimType::IRacing`
- Follow F1 25 adapter structure: struct fields for tracking state, packet parsing methods, SimAdapter trait impl
- Follow AC adapter pattern for shared memory (OpenFileMappingW, MapViewOfFile)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TEL-IR-01 | iRacing shared memory read using winapi OpenFileMappingA during active sessions | irsdk_header layout, ShmHandle pattern from AC adapter, variable lookup mechanism |
| TEL-IR-02 | Session transitions handled — re-open shared memory handle between races | sessionUniqueID telemetry variable change detection, full disconnect/reconnect cycle |
| TEL-IR-03 | Lap times and sector splits extracted as LapCompleted events | LapCompleted + LapLastLapTime variables; sector splits via LapDistPct position tracking |
| TEL-IR-04 | Pre-flight check: verify irsdkEnableMem=1 in app.ini, warn if missing | app.ini at Documents\iRacing\app.ini, dirs-next crate already in deps |
</phase_requirements>

---

## Summary

iRacing exposes a Windows memory-mapped file at `Local\IRSDKMemMapFileName`. The shared memory begins with a fixed `irsdk_header` struct (48 bytes) that contains version, status bits, an offset to YAML session info, the count and offset of variable headers, and pointers to up to 4 rotating telemetry data buffers. Each buffer slot has a `tickCount` (monotonic tick for change detection) and a `bufOffset` (offset from shm start to that buffer row). Variable values inside a buffer row are located by scanning the `irsdk_varHeader` array for a named variable and reading from `bufOffset + varHeader.offset`.

The correct read pattern is a **double-buffer tick-lock**: read all three `varBuf[i].tickCount` values, pick the one with the highest tick, copy the row, re-read its tick to confirm it did not change during the copy (retry if it did). This avoids partial reads when iRacing writes the next frame at 60 Hz.

Session transitions (qualify -> race, practice -> qualify, next race) increment `sessionInfoUpdate` in the header and change `SessionUniqueID` in the telemetry variables. The shared memory handle itself stays open — the YAML session string at the existing offset is simply rewritten. The adapter must re-parse YAML and reset lap state on each UID change.

**Primary recommendation:** Implement `IracingAdapter` using the exact `ShmHandle`/`read_f32`/`read_i32` helper pattern from `assetto_corsa.rs`. Open one shared memory mapping (not three like AC). Use dynamic variable lookup (scan `irsdk_varHeader[]` by name), not fixed offsets. Detect laps via `LapCompleted` counter increment. No sector split variable exists in the public IRSDK — synthesize S3 from `LapLastLapTime - S1 - S2` using `LapDistPct` crossing thresholds or accept `None` for sector splits.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| winapi | 0.3 | OpenFileMappingW, MapViewOfFile, CloseHandle, UnmapViewOfFile | Already in Cargo.toml under `[target.'cfg(windows)'.dependencies]` |
| dirs-next | 2 | Resolve `Documents\iRacing\app.ini` path | Already in Cargo.toml; provides `dirs_next::document_dir()` |
| serde_yaml / manual YAML parse | — | Parse iRacing session info YAML string | **Do not add serde_yaml** — iRacing YAML is non-standard ISO-8859-1; use the existing manual search pattern (strstr-style scan for keys) |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| chrono | workspace | Timestamp LapData.created_at | All LapData construction |
| uuid | workspace | LapData.id generation | All LapData construction |
| tracing | workspace | Structured logging, pre-flight warnings | Throughout adapter lifecycle |
| anyhow | workspace | Error propagation | All Result returns |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Manual variable lookup loop | irsdk Rust crate (docs.rs/crate/iracing) | crate is dormant (2020, v0.4.1), adds dependency for something that is 30 lines of code |
| Manual YAML key scan | serde_yaml | iRacing's session string uses ISO-8859-1 + non-standard structure; serde_yaml rejects it without a custom deserializer; manual scan is simpler |
| Dynamic variable lookup | Pre-computed fixed offsets | Fixed offsets break if iRacing updates its SDK; dynamic lookup is session-safe |

**No new dependencies required.** All needed libraries are already in Cargo.toml.

---

## Architecture Patterns

### Recommended File Location
```
crates/rc-agent/src/sims/iracing.rs    -- new adapter
crates/rc-agent/src/sims/mod.rs        -- add: pub mod iracing;
crates/rc-agent/src/event_loop.rs      -- add IRacing arm to PlayableSignal dispatch
```

### irsdk_header Memory Layout (offset from shm start)
```
Offset  Size  Field
0       4     ver (i32) — API version
4       4     status (i32) — bitfield; bit 1 = irsdk_stConnected
8       4     tickRate (i32) — 60 or 360
12      4     sessionInfoUpdate (i32) — increments on YAML session string changes
16      4     sessionInfoLen (i32) — byte length of YAML string
20      4     sessionInfoOffset (i32) — offset from shm start to YAML string
24      4     numVars (i32) — count of telemetry variables
28      4     varHeaderOffset (i32) — offset to irsdk_varHeader array
32      4     numBuf (i32) — active buffer count (<=4)
36      4     bufLen (i32) — bytes per telemetry row
40      8     pad (2x i32)
48      16    varBuf[0] — { tickCount: i32, bufOffset: i32, pad: [i32;2] }
64      16    varBuf[1]
80      16    varBuf[2]
96      16    varBuf[3]
```
**Total header size: 112 bytes.** Status bit 1 (`status & 1`) = iRacing is connected.

### irsdk_varHeader Layout (per variable, starts at varHeaderOffset)
```
Offset  Size  Field
0       4     type (i32) — 0=char, 1=bool, 2=i32, 3=bitField, 4=f32, 5=f64
4       4     offset (i32) — offset from row start to variable value
8       4     count (i32) — array length (1 for scalar)
12      4     pad
16      32    name (char[32]) — null-terminated ASCII name
48      64    desc (char[64]) — description
112     32    unit (char[32]) — unit label
Total: 144 bytes per variable
```

### Pattern 1: Dynamic Variable Lookup
**What:** Scan the `irsdk_varHeader` array (numVars entries, each 144 bytes, starting at varHeaderOffset) for variables by name. Cache the offset for each variable needed.

**When to use:** At connect() time — build a lookup table. This handles SDK updates without hardcoded offsets.

**Example:**
```rust
// Source: irsdk_defines.h pattern + iRon/iracing.cpp structure
struct VarOffset {
    is_on_track: i32,
    lap_completed: i32,
    lap_last_lap_time: i32,
    session_unique_id: i32,
    speed: i32,
    throttle: i32,
    brake: i32,
    gear: i32,
    rpm: i32,
    lap_dist_pct: i32,
    session_state: i32,
    // types (to pick correct read_* helper)
    is_on_track_type: i32,   // 1 = bool
    lap_completed_type: i32, // 2 = i32
    lap_last_lap_time_type: i32, // 4 = f32
}

fn find_var_offset(shm_ptr: *const u8, header: &IrsdkHeader, name: &[u8]) -> Option<(i32, i32)> {
    // Returns (offset_in_row, type) for variable named `name`
    for i in 0..header.num_vars {
        let var_ptr = unsafe {
            shm_ptr.add(header.var_header_offset as usize + i as usize * 144)
        };
        let var_name = unsafe { std::slice::from_raw_parts(var_ptr.add(16), 32) };
        let null_end = var_name.iter().position(|&c| c == 0).unwrap_or(32);
        if &var_name[..null_end] == name {
            let offset = unsafe { std::ptr::read_unaligned(var_ptr.add(4) as *const i32) };
            let var_type = unsafe { std::ptr::read_unaligned(var_ptr as *const i32) };
            return Some((offset, var_type));
        }
    }
    None
}
```

### Pattern 2: Double-Buffer Tick-Lock Read
**What:** iRacing writes to rotating buffers at 60 Hz. Reading must pick the most-recently-written complete buffer.

**When to use:** Every call to `read_telemetry()`.

**Example:**
```rust
// Source: irsdk_defines.h + NickThissen/iRacingSdkWrapper pattern
fn read_latest_buffer_row<'a>(shm_ptr: *const u8, header: &IrsdkHeader) -> Option<i32> {
    // Find the buffer with the highest tickCount
    let mut best_tick = -1i32;
    let mut best_buf_idx = 0usize;
    for i in 0..(header.num_buf as usize).min(4) {
        let buf_offset = 48 + i * 16; // varBuf[i] starts at offset 48
        let tick = unsafe {
            std::ptr::read_unaligned(shm_ptr.add(buf_offset) as *const i32)
        };
        if tick > best_tick {
            best_tick = tick;
            best_buf_idx = i;
        }
    }
    // Return the bufOffset for the chosen slot
    let buf_offset_field = 48 + best_buf_idx * 16 + 4;
    let row_offset = unsafe {
        std::ptr::read_unaligned(shm_ptr.add(buf_offset_field) as *const i32)
    };
    Some(row_offset)
}
// Read a variable from the selected row:
// value_ptr = shm_ptr + row_offset + var_offset_in_row
```

### Pattern 3: Session Transition Detection
**What:** Track `SessionUniqueID` (i32 telemetry variable). When it changes between reads, re-parse YAML and reset lap state.

**When to use:** Every `read_telemetry()` call, compare to stored `last_session_uid`.

**Example:**
```rust
let current_uid = self.read_var_i32(shm, row_offset, self.var_offsets.session_unique_id);
if current_uid != self.last_session_uid && current_uid != 0 {
    tracing::info!("iRacing session transition: uid {} -> {}", self.last_session_uid, current_uid);
    self.last_session_uid = current_uid;
    self.last_lap_count = 0; // Reset to avoid false lap on new session
    self.sector_times = [None; 3];
    self.pending_lap = None;
    self.parse_session_yaml(shm, header); // Re-read track/car/session_type from YAML
}
```

### Pattern 4: YAML Session Info Parsing
**What:** iRacing stores a YAML string at `shm_ptr + sessionInfoOffset`. The string is ISO-8859-1. Parse by searching for key patterns like `TrackDisplayName:`, `CarScreenName:`, `SessionType:`.

**When to use:** In `connect()` and on session UID change.

**Example:**
```rust
// Simple key scan — do NOT use serde_yaml (ISO-8859-1 not handled)
fn extract_yaml_value(yaml: &str, key: &str) -> Option<String> {
    let search = format!("{}:", key);
    let start = yaml.find(&search)? + search.len();
    let rest = &yaml[start..];
    let trimmed = rest.trim_start_matches(' ');
    let end = trimmed.find('\n').unwrap_or(trimmed.len());
    let value = trimmed[..end].trim().trim_matches('"').to_string();
    if value.is_empty() { None } else { Some(value) }
}

// Keys used in iRacing YAML:
// "TrackDisplayName" -> track name
// "CarScreenName"    -> car model name
// "SessionType"      -> "Race", "Practice", "Qualify", "Time Trial", "Lone Qualify"
// "SubsessionId"     -> session ID (integer as string)
```

### Pattern 5: Lap Completion Detection
**What:** `LapCompleted` (i32) increments each time the player crosses the finish line. Same as AC's `COMPLETED_LAPS`. `LapLastLapTime` (f32, seconds) holds the previous lap time.

**When to use:** In `read_telemetry()`, compare `current_lap_completed` to `self.last_lap_count`.

**Example:**
```rust
let lap_completed = self.read_var_i32(shm, row, self.var_offsets.lap_completed);
let last_lap_time_s = self.read_var_f32(shm, row, self.var_offsets.lap_last_lap_time);

if lap_completed > self.last_lap_count && last_lap_time_s > 0.0 {
    let lap_time_ms = (last_lap_time_s * 1000.0) as u32;
    let lap_data = LapData {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: String::new(),
        driver_id: String::new(),
        pod_id: self.pod_id.clone(),
        sim_type: SimType::IRacing,
        track: self.current_track.clone(),
        car: self.current_car.clone(),
        lap_number: lap_completed as u32,
        lap_time_ms,
        sector1_ms: self.sector_times[0],
        sector2_ms: self.sector_times[1],
        sector3_ms: self.sector_times[2],
        valid: true, // iRacing invalidates laps server-side, not via a telemetry flag
        session_type: self.current_session_type,
        created_at: chrono::Utc::now(),
    };
    self.pending_lap = Some(lap_data);
    self.sector_times = [None; 3];
}
self.last_lap_count = lap_completed;
```

### Pattern 6: PlayableSignal Integration (IsOnTrack)
**What:** Replace the 90s process-based fallback for iRacing with an `IsOnTrack` signal from shared memory. When `IsOnTrack` transitions from false to true, emit `AcStatus::Live` via the same mechanism as AC and F1 25.

**When to use:** In `event_loop.rs` PlayableSignal dispatch block, add an IRacing arm that reads `adapter.read_iracing_is_on_track()` and fires billing when true.

**Implementation note:** Add a `read_is_on_track()` method to `IracingAdapter` (not on `SimAdapter` trait — same as `read_ac_status()` is AC-specific). event_loop checks it in the `Some(rc_common::types::SimType::IRacing)` match arm.

### Anti-Patterns to Avoid
- **Fixed struct offsets for variables:** Variable offsets change between iRacing SDK updates. Always scan the varHeader array by name.
- **Single-buffer read without tick check:** Reading without verifying tickCount before and after will produce torn reads at 60 Hz updates.
- **Opening shared memory handle every poll:** Open once at `connect()`. The handle remains valid across session transitions. Only re-parse YAML on UID change — do NOT close/reopen the mapping.
- **serde_yaml for session info:** iRacing's YAML uses ISO-8859-1 encoding and non-standard structure. Manual key scan is correct.
- **Assuming `Lap` is the lap counter:** Use `LapCompleted` (finished laps), not `Lap` (started laps). At start/finish combined tracks, `Lap` is always 1 ahead of `LapCompleted`.
- **Lap time unit confusion:** `LapLastLapTime` is in **seconds** (f32), not milliseconds. Multiply by 1000 when storing in `LapData.lap_time_ms`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| YAML key extraction | Full YAML parser | Manual key scan (5-line fn) | iRacing's YAML is ISO-8859-1 non-standard; the value is always a single line after "Key: " |
| Shared memory access | OS-specific mmap wrapper | winapi directly (same as AC adapter) | Already battle-tested in assetto_corsa.rs; no new abstraction needed |
| Variable offset cache | HashMap | Fixed struct with named fields per variable | Simpler, compile-time clear, no heap allocation for lookups at 60 Hz |
| app.ini path construction | Hard-coded path | `dirs_next::document_dir()` | Already in Cargo.toml; handles non-default Documents folder |

**Key insight:** iRacing's shared memory protocol is stable at the "named variable lookup" level even across SDK updates. The only thing that changes is variable offsets within a row — which is exactly what dynamic lookup handles.

---

## Common Pitfalls

### Pitfall 1: LapCompleted Resets to 0 on Session Transition
**What goes wrong:** New session (e.g., post-qualifying race start) resets `LapCompleted` to 0. If `last_lap_count` still holds the old value, every lap for the rest of the session is a false positive.
**Why it happens:** `SessionUniqueID` changes first, but `LapCompleted` may lag by 1-2 ticks. Reading in the wrong order causes a missed reset.
**How to avoid:** Reset `last_lap_count = 0` on every `SessionUniqueID` change, before the next telemetry comparison. Set it from `LapCompleted` on first valid read if transitioning mid-session.
**Warning signs:** Laps firing immediately at session start with 0ms times.

### Pitfall 2: iRacing Not Running = Null Shared Memory Handle
**What goes wrong:** `OpenFileMappingW` returns null if iRacing is not running. The adapter must return `Ok(None)` from `read_telemetry()`, not panic.
**Why it happens:** Unlike AC (which writes the file when AC.exe starts), iRacing only creates the mapping when the sim client is fully initialized.
**How to avoid:** Check `handle.is_null()` and return `Err` from `connect()`. event_loop retries `connect()` on next tick (existing behavior for all adapters).
**Warning signs:** `ERROR: failed to open shared memory` on pod startup — expected and recoverable.

### Pitfall 3: irsdkEnableMem Not Set
**What goes wrong:** iRacing requires `irsdkEnableMem=1` in `Documents\iRacing\app.ini` to activate shared memory telemetry. Without it, `OpenFileMappingW` always fails.
**Why it happens:** iRacing disables shared memory by default for performance. Many users never enable it.
**How to avoid:** Pre-flight check reads the app.ini file, scans for `irsdkEnableMem=1`. Warn via `tracing::warn!` if missing. Do not block launch — the warning reaches staff via log.
**Warning signs:** Persistent `connect()` failures even while iRacing is running.

### Pitfall 4: Torn Read at 60 Hz Update Boundary
**What goes wrong:** iRacing updates a buffer row atomically at 60 Hz. If you read the buffer mid-write, `LapCompleted` and `LapLastLapTime` come from different logical ticks.
**Why it happens:** Direct memory reads are not atomic at the application level.
**How to avoid:** Use the double-buffer tick-lock pattern: snapshot `tickCount` before and after copying the row. If ticks differ, retry with the next buffer slot.
**Warning signs:** Occasional `LapLastLapTime` = 0.0 on lap completion events.

### Pitfall 5: Sector Split Variables Do Not Exist in IRSDK
**What goes wrong:** Unlike AC and F1 25, iRacing does not expose sector split times as named telemetry variables. Searching the varHeader array for "Sector1Time" or similar yields nothing.
**Why it happens:** iRacing's sector times are only available in the session YAML (historical, not live), not in real-time telemetry.
**How to avoid:** Synthesize S3 as `lap_time_ms - S1_ms - S2_ms`. For S1/S2, use `LapDistPct` zone crossings to timestamp when the car passes 33% and 66% of the lap. Accept `None` for all three sector fields if tracking is not feasible in the initial implementation — `LapData.sector1/2/3_ms` are `Option<u32>`.
**Warning signs:** No sector data in leaderboard — this is expected for v1 iRacing adapter.

### Pitfall 6: app.ini Path on Non-English Windows
**What goes wrong:** On non-English Windows, the Documents folder is not `C:\Users\<user>\Documents`. Hard-coding the path breaks.
**Why it happens:** Windows localizes the Documents folder path.
**How to avoid:** Use `dirs_next::document_dir()` which calls `SHGetKnownFolderPath` internally. Already in Cargo.toml.
**Warning signs:** Pre-flight check always fails on pod despite app.ini being present.

---

## Code Examples

Verified patterns from the existing codebase and iRacing SDK reference:

### Shared Memory Open (TEL-IR-01)
```rust
// Source: assetto_corsa.rs connect() pattern adapted for single-mapping iRacing
#[cfg(windows)]
fn open_iracing_shm() -> Result<ShmHandle> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let name = "Local\\IRSDKMemMapFileName";
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
            anyhow::bail!("iRacing shared memory not found — is iRacing running?");
        }
        let ptr = winapi::um::memoryapi::MapViewOfFile(
            handle,
            winapi::um::memoryapi::FILE_MAP_READ,
            0, 0, 0,
        );
        if ptr.is_null() {
            winapi::um::handleapi::CloseHandle(handle);
            anyhow::bail!("MapViewOfFile failed for iRacing shm");
        }
        Ok(ShmHandle {
            _handle: handle,
            ptr: ptr as *const u8,
            _size: 0,
        })
    }
}
```

### Reading irsdk_header Fields
```rust
// Source: irsdk_defines.h struct layout (confirmed by node-iracing/lib/irsdk_defines.h)
struct IrsdkHeaderSnapshot {
    status: i32,           // offset 4
    session_info_update: i32, // offset 12
    session_info_len: i32, // offset 16
    session_info_offset: i32, // offset 20
    num_vars: i32,         // offset 24
    var_header_offset: i32, // offset 28
    num_buf: i32,          // offset 32
    buf_len: i32,          // offset 36
}

fn read_header(ptr: *const u8) -> IrsdkHeaderSnapshot {
    unsafe {
        IrsdkHeaderSnapshot {
            status: std::ptr::read_unaligned(ptr.add(4) as *const i32),
            session_info_update: std::ptr::read_unaligned(ptr.add(12) as *const i32),
            session_info_len: std::ptr::read_unaligned(ptr.add(16) as *const i32),
            session_info_offset: std::ptr::read_unaligned(ptr.add(20) as *const i32),
            num_vars: std::ptr::read_unaligned(ptr.add(24) as *const i32),
            var_header_offset: std::ptr::read_unaligned(ptr.add(28) as *const i32),
            num_buf: std::ptr::read_unaligned(ptr.add(32) as *const i32),
            buf_len: std::ptr::read_unaligned(ptr.add(36) as *const i32),
        }
    }
}

// is_connected check: status bit 1
fn is_iracing_active(status: i32) -> bool {
    status & 1 != 0
}
```

### Pre-flight app.ini Check (TEL-IR-04)
```rust
// Source: dirs_next::document_dir() already available (Cargo.toml)
pub fn check_iracing_shm_enabled() -> bool {
    let doc_dir = match dirs_next::document_dir() {
        Some(d) => d,
        None => {
            tracing::warn!("[iracing-preflight] Could not determine Documents folder");
            return false;
        }
    };
    let ini_path = doc_dir.join("iRacing").join("app.ini");
    let content = match std::fs::read_to_string(&ini_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "[iracing-preflight] Cannot read app.ini at {}: {} — irsdkEnableMem may not be set",
                ini_path.display(), e
            );
            return false;
        }
    };
    let enabled = content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "irsdkEnableMem=1"
            || trimmed.starts_with("irsdkEnableMem") && trimmed.contains('=') && {
                let val = trimmed.splitn(2, '=').nth(1).unwrap_or("0").trim();
                val == "1"
            }
    });
    if !enabled {
        tracing::warn!(
            "[iracing-preflight] irsdkEnableMem=1 not found in {} — iRacing telemetry will not work",
            ini_path.display()
        );
    }
    enabled
}
```

### SessionType Mapping from YAML
```rust
// iRacing YAML SessionType values (confirmed via iRon/iracing.h SessionType enum)
fn parse_session_type(yaml_session_type: &str) -> SessionType {
    match yaml_session_type.trim() {
        "Race" | "Sprint Race" => SessionType::Race,
        "Qualify" | "Lone Qualify" | "Open Qualify" => SessionType::Qualifying,
        "Lone Qualify" => SessionType::Qualifying,
        "Time Trial" => SessionType::Hotlap,
        _ => SessionType::Practice, // "Practice", "Warmup", unknown
    }
}
```

---

## Key Telemetry Variables (Verified)

| Variable Name | Type | Unit | Notes |
|---------------|------|------|-------|
| `IsOnTrack` | bool (type=1) | — | true = car on track, physics running |
| `LapCompleted` | i32 (type=2) | count | Finished lap counter — use this for lap detection |
| `Lap` | i32 (type=2) | count | Started lap (always LapCompleted+1 at finish line) |
| `LapLastLapTime` | f32 (type=4) | **seconds** | Previous lap time — multiply by 1000 for ms |
| `LapCurrentLapTime` | f32 (type=4) | seconds | Current in-progress lap estimate |
| `LapBestLapTime` | f32 (type=4) | seconds | Session best lap |
| `LapDistPct` | f32 (type=4) | 0.0-1.0 | Fraction of lap completed — use for S1/S2 zone detection |
| `SessionUniqueID` | i32 (type=2) | — | Changes on session transition |
| `SessionState` | i32 (type=3) | bitfield | 0=Invalid 1=GetInCar 2=Warmup 3=Parade 4=Racing 5=Checkered 6=CoolDown |
| `Speed` | f32 (type=4) | m/s | Multiply by 3.6 for km/h |
| `Throttle` | f32 (type=4) | 0.0-1.0 | |
| `Brake` | f32 (type=4) | 0.0-1.0 | |
| `SteeringWheelAngle` | f32 (type=4) | radians | |
| `Gear` | i32 (type=2) | — | -1=R, 0=N, 1-6 |
| `RPM` | f32 (type=4) | RPM | |

**Sector split variables:** Do NOT exist in iRacing real-time telemetry. `sector1_ms` and `sector2_ms` will be `None` in the initial implementation unless zone-crossing logic is added.

---

## PlayableSignal Integration

The existing `event_loop.rs` PlayableSignal dispatch block (lines 485-528) has a catch-all `Some(sim_type)` arm that fires `AcStatus::Live` after 90s for iRacing. Once the iRacing adapter exists, replace this with:

```rust
Some(rc_common::types::SimType::IRacing) => {
    if let Some(ref mut adapter) = state.adapter {
        if let Some(is_on_track) = adapter.read_iracing_is_on_track() {
            if is_on_track && matches!(conn.launch_state, LaunchState::WaitingForLive { .. }) {
                tracing::info!("[billing] iRacing IsOnTrack=true — emitting AcStatus::Live");
                let msg = AgentMessage::GameStatusUpdate {
                    pod_id: state.pod_id.clone(),
                    ac_status: AcStatus::Live,
                    sim_type: Some(rc_common::types::SimType::IRacing),
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

Add `fn read_iracing_is_on_track(&self) -> Option<bool>` on the `SimAdapter` trait with a default `None` implementation (same pattern as `read_ac_status`). Override in `IracingAdapter`.

---

## Integration Checklist

| File | Change |
|------|--------|
| `sims/mod.rs` | Add `pub mod iracing;` |
| `sims/iracing.rs` | New file — full adapter |
| `event_loop.rs` | Add `SimType::IRacing` arm to PlayableSignal dispatch; add `read_iracing_is_on_track()` call |
| `sims/mod.rs` (trait) | Add `fn read_iracing_is_on_track(&self) -> Option<bool> { None }` default |
| `ws_handler.rs` | Verify `f1_udp_playable_received` reset on sim switch (already present at line 263) |

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (standard) |
| Config file | none — workspace Cargo.toml |
| Quick run command | `cargo test -p rc-agent-crate sims::iracing` |
| Full suite command | `cargo test -p rc-agent-crate && cargo test -p rc-common` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TEL-IR-01 | Shared memory open returns error when not available (non-Windows or iRacing not running) | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_connect_no_shm` | ❌ Wave 0 |
| TEL-IR-02 | SessionUniqueID change resets lap count and re-fires pending state | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_session_transition_resets_lap` | ❌ Wave 0 |
| TEL-IR-03 | LapCompleted increment emits LapData with correct lap_time_ms (seconds to ms) | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_lap_completed_event` | ❌ Wave 0 |
| TEL-IR-03 | First packet safety — LapCompleted already >0 on connect does not fire false lap | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_first_packet_safety` | ❌ Wave 0 |
| TEL-IR-04 | check_iracing_shm_enabled returns false when app.ini missing | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_preflight_missing_ini` | ❌ Wave 0 |
| TEL-IR-04 | check_iracing_shm_enabled returns true when irsdkEnableMem=1 present | unit | `cargo test -p rc-agent-crate sims::iracing::tests::test_preflight_ini_enabled` | ❌ Wave 0 |

**Notes:**
- All tests are pure-Rust unit tests — no iRacing process needed. Tests construct synthetic shared memory buffers (like F1 25 tests build synthetic UDP packets).
- The `check_iracing_shm_enabled()` function takes an optional path parameter in tests (or use `tempfile` from dev-deps to write test app.ini).
- Windows-specific code (`#[cfg(windows)]`) is tested on Windows (pod build) — CI may skip on Linux.

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate sims::iracing`
- **Per wave merge:** `cargo test -p rc-agent-crate && cargo test -p rc-common`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/sims/iracing.rs` — file does not exist yet (entire adapter is Wave 0 work)
- [ ] Test module `sims::iracing::tests` — 6 tests listed above, all Wave 0

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed struct offsets (old irsdk clients) | Dynamic variable lookup by name | iRacing SDK ~2015+ | Adapter survives SDK updates without code changes |
| serde_yaml for session info | Manual key scan | Always required | ISO-8859-1 + non-standard YAML not parseable by standard libs |
| UDP telemetry (IRSDK had UDP option) | Shared memory only | iRacing removed UDP | Shared memory is the only supported real-time method |

---

## Open Questions

1. **Sector split data**
   - What we know: No `SectorXTime` variables exist in the iRacing telemetry variable list
   - What's unclear: Whether `LapDistPct` zone crossings (at 0.33/0.66) are close enough to actual sector boundaries to be useful
   - Recommendation: Initial implementation returns `None` for all three sector fields. A follow-up phase can add `LapDistPct` zone tracking once the adapter is stable. For Racing Point, lap time is the primary metric.

2. **IsOnTrack vs SessionState for PlayableSignal**
   - What we know: `IsOnTrack` = true when car is on track with physics active. `SessionState` = 4 (Racing) is also a valid "active" indicator.
   - What's unclear: Whether `IsOnTrack` fires before or after the loading screen clears.
   - Recommendation: Use `IsOnTrack` as the primary signal (matches what it says on the tin). Session state can be a fallback.

3. **Multiple players / car index**
   - What we know: `LapCompleted` is the player's own lap (not an array). iRacing telemetry is always player-centric for single-player variables.
   - What's unclear: N/A — Racing Point runs solo sessions, not multiplayer.
   - Recommendation: Use scalar variables (no car index needed).

---

## Sources

### Primary (HIGH confidence)
- `node-iracing/lib/irsdk_defines.h` (GitHub, meltingice) — irsdk_header, irsdk_varHeader, irsdk_varBuf struct layouts with byte offsets, SessionState enum
- `crates/rc-agent/src/sims/assetto_corsa.rs` — ShmHandle pattern, OpenFileMappingW, MapViewOfFile, disconnect pattern (project codebase)
- `crates/rc-agent/src/sims/f1_25.rs` — LapData construction, poll_lap_completed take semantics, test helper structure (project codebase)
- `NickThissen/iRacingSdkWrapper/TelemetryInfo.cs` (GitHub) — Verified variable names: IsOnTrack, Lap, LapCompleted, LapDistPct, SessionUniqueID, Speed, Throttle, Brake, SteeringWheelAngle, Gear, RPM, SessionState
- `sajax.github.io/irsdkdocs/telemetry/lapcompleted.html` — LapCompleted vs Lap distinction confirmed

### Secondary (MEDIUM confidence)
- WebSearch (2026): `irsdkEnableMem` location = `Documents\iRacing\app.ini`, verified across multiple sources (bandofothersgaming.com, simracingstudio.freshdesk.com, github.com/rosskevin/iracing)
- `lespalt/iRon/iracing.cpp` (GitHub) — session transition via `ir_SessionUniqueID` + `wasSessionStrUpdated()`, lap tracking via `CarIdxLapCompleted` (MEDIUM — C++ wrapper, may differ from raw SDK)
- WebSearch (2026): LapLastLapTime/LapBestLapTime are f32 in seconds (multiple implementations confirm)

### Tertiary (LOW confidence)
- Sector split variables absent: Inferred from absence in TelemetryInfo.cs and SDK variable surveys. Not explicitly documented as "not existing" in official docs.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies already in Cargo.toml; no new libraries needed
- Architecture (header layout): HIGH — verified against irsdk_defines.h (node-iracing reference)
- Architecture (variable names): HIGH — verified against iRacingSdkWrapper TelemetryInfo.cs
- Architecture (session transitions): MEDIUM — verified pattern from iRon C++ implementation
- Sector splits: LOW — absence confirmed by inspection but not by official "not supported" statement
- Pre-flight (app.ini path): HIGH — multiple independent sources confirm Documents\iRacing\app.ini

**Research date:** 2026-03-21 IST
**Valid until:** 2026-06-21 (iRacing SDK is stable; variable names rarely change)
