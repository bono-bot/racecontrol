# Phase 5: Content Validation & Filtering - Research

**Researched:** 2026-03-14
**Domain:** Assetto Corsa content filesystem scanning, WebSocket protocol extension, per-pod catalog filtering
**Confidence:** HIGH

## Summary

Phase 5 transforms the static hardcoded catalog (325 cars, 50 tracks in `catalog.rs`) into a dynamic per-pod content system. Each rc-agent scans its pod's AC installation at startup, builds a `ContentManifest` describing which cars and tracks are installed (plus per-track AI capability and pit stall counts), and sends it to rc-core over the existing WebSocket protocol. rc-core caches each pod's manifest and filters catalog API responses so customers only see content that actually exists on their assigned pod.

The codebase is well-prepared for this change. The `AgentMessage` enum already uses `#[serde(tag = "type", content = "data")]` adjacently-tagged serialization with snake_case renaming -- adding a `ContentManifest` variant follows the established pattern (13 variants exist). The `AppState` already stores per-pod data in `RwLock<HashMap<String, T>>` maps (agent_senders, pod_deploy_states, etc.) -- a new `pod_manifests` map fits naturally. The catalog API endpoints (`customer_ac_catalog`, `list_ac_tracks`, `list_ac_cars`) currently take no pod context -- they need a `pod_id` query parameter to enable filtering.

**Primary recommendation:** Build three layers in sequence: (1) Content scanner module in rc-agent that walks `content/cars/` and `content/tracks/` producing a `ContentManifest` struct, (2) Protocol + state extension to send/receive/cache manifests, (3) Catalog API filtering + launch-time validation gate.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Scanning happens **on each pod (rc-agent)**, not centralized
- rc-agent scans AC content folders at **startup and on reconnect** to rc-core
- rc-agent sends a full **ContentManifest** to rc-core as a new WebSocket message (AgentMessage variant), sent after Connect handshake
- rc-core caches the manifest per-pod and uses it to filter catalog API responses
- AC installation path: `C:\Program Files (x86)\Steam\steamapps\common\assettocorsa`
- Content folders to scan: `content/cars/` (car IDs) and `content/tracks/` (track IDs + configs)
- Car entries: folder name (car ID) from `content/cars/`
- Track entries: folder name (track ID) + list of config subfolder names (layout variants)
- Per-config metadata: has_ai (bool -- ai/ folder exists with files), pit_count (Option<u32> -- from ui_track.json pitboxes field)
- Track configs checked independently for AI support
- Simple folder check for AI: ai/ folder exists with files
- Tracks without AI lines hide Race vs AI and Track Day session types entirely (not greyed out)
- Pit stall count parsed from ui/ui_track.json pitboxes field per track config
- Fallback: if ui_track.json missing or no pitboxes field, default to 19
- AI opponent slider max dynamically updates based on selected track's pit count minus 1
- Server-side filtering only -- rc-core filters catalog response per-pod before sending to PWA
- Invalid session types hidden entirely, not greyed out
- Launch-time validation gate: rc-core validates combo against pod manifest before sending LaunchGame
- If pod connects with empty manifest (scan failed), fall back to static hardcoded catalog (current catalog.rs)

### Claude's Discretion
- Manifest serialization format (compact vs verbose)
- Exact WebSocket message naming and structure
- How to handle track configs that are just empty default layouts
- Whether to scan car skins (not needed for validation, but could enrich display)
- Caching strategy for per-pod filtered catalogs

### Deferred Ideas (OUT OF SCOPE)
- Curated presets using validated content (Phase 7)
- Staff kiosk content configuration (Phase 8)
- Car skin scanning for richer display
- Content auto-download or sync between pods
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SESS-07 | Only valid session/mode combinations are presented (invalid options hidden) | Session type filtering based on AI line detection per track config; hide Race vs AI and Track Day when has_ai=false |
| CONT-01 | Customer can browse and select car from available catalog via PWA | Extend customer_ac_catalog API with pod_id parameter; filter ALL_CAR_IDS against pod's ContentManifest.cars |
| CONT-02 | Customer can browse and select track from available catalog via PWA | Extend customer_ac_catalog API with pod_id parameter; filter ALL_TRACK_IDS against pod's ContentManifest.tracks |
| CONT-04 | Invalid car/track/session combinations are filtered out before display | Server-side filtering in rc-core using cached manifest; launch-time validation gate as safety net |
| CONT-05 | Tracks without AI line data (ai/ folder) hide AI-related session types | has_ai boolean per track config from filesystem scan; catalog response includes per-track session type availability |
| CONT-06 | Track pit count limits maximum AI opponents shown for that track | pit_count from ui_track.json pitboxes field (string parsed to u32); default 19; slider max = pit_count - 1 |
| CONT-07 | Per-pod content scanning -- only show cars/tracks installed on the target pod | ContentManifest sent from rc-agent at startup/reconnect; rc-core caches per pod_id |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde + serde_json | (existing) | ContentManifest serialization | Already used for all protocol messages |
| tokio | (existing) | Async filesystem scanning via spawn_blocking | Already the async runtime |
| std::fs | stable | Synchronous directory walking (read_dir, metadata) | Simple filesystem ops, no external deps needed |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tracing | (existing) | Logging scan progress and errors | Already used throughout rc-agent |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| std::fs::read_dir | walkdir crate | Overkill -- scanning is 2 levels deep max, not recursive tree walk |
| Manual JSON parse | serde_json from_str | Already in deps, use it for ui_track.json parsing |

**Installation:**
No new dependencies needed. All required crates are already in the workspace.

## Architecture Patterns

### Recommended Project Structure
```
rc-agent/src/
  content_scanner.rs       # NEW: scan_ac_content() -> ContentManifest
  main.rs                  # After Register, send ContentManifest

rc-common/src/
  protocol.rs              # Add ContentManifest variant to AgentMessage
  types.rs                 # ContentManifest, TrackManifestEntry, CarManifestEntry structs

rc-core/src/
  state.rs                 # Add pod_manifests: RwLock<HashMap<String, ContentManifest>>
  catalog.rs               # Add get_filtered_catalog(manifest: Option<&ContentManifest>) -> Value
  ws/mod.rs                # Handle AgentMessage::ContentManifest -> store in AppState
  api/routes.rs            # customer_ac_catalog takes pod_id query param, uses filtered catalog
  game_launcher.rs         # (or auth/mod.rs) validate_launch_combo() before LaunchGame dispatch
```

### Pattern 1: Filesystem Scanner (rc-agent)
**What:** A synchronous function that walks AC content directories and produces a ContentManifest struct.
**When to use:** Called at startup and on WebSocket reconnect, before the main event loop.
**Example:**
```rust
// rc-agent/src/content_scanner.rs
use rc_common::types::{ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest};
use std::path::Path;

const AC_CONTENT_PATH: &str = r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\content";

pub fn scan_ac_content() -> ContentManifest {
    let content_path = Path::new(AC_CONTENT_PATH);
    let cars = scan_cars(&content_path.join("cars"));
    let tracks = scan_tracks(&content_path.join("tracks"));
    ContentManifest { cars, tracks }
}

fn scan_cars(cars_dir: &Path) -> Vec<CarManifestEntry> {
    // Read content/cars/ -> each subfolder = one car ID
    // Skip non-directories and hidden folders
}

fn scan_tracks(tracks_dir: &Path) -> Vec<TrackManifestEntry> {
    // For each track folder:
    //   1. Check for config subfolders (directories with data/ or ai/ inside)
    //   2. If no config subfolders, treat root as default config ""
    //   3. For each config: check ai/ existence and parse ui_track.json pitboxes
}
```

### Pattern 2: Protocol Extension (rc-common)
**What:** Add ContentManifest as a new AgentMessage variant following the existing tagged-enum pattern.
**When to use:** After Register message is sent, before entering the main event loop.
**Example:**
```rust
// In rc-common/src/protocol.rs - AgentMessage enum
/// Pod reports its installed AC content (cars, tracks, AI capabilities)
ContentManifest(ContentManifest),

// In rc-common/src/types.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentManifest {
    pub cars: Vec<CarManifestEntry>,
    pub tracks: Vec<TrackManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarManifestEntry {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackManifestEntry {
    pub id: String,
    pub configs: Vec<TrackConfigManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackConfigManifest {
    pub config: String,  // "" for default layout
    pub has_ai: bool,
    pub pit_count: Option<u32>,
}
```

### Pattern 3: Per-Pod Manifest Cache (rc-core)
**What:** Store ContentManifest per pod_id in AppState, used by catalog API filtering.
**When to use:** On receiving AgentMessage::ContentManifest in the WS handler.
**Example:**
```rust
// In AppState
pub pod_manifests: RwLock<HashMap<String, ContentManifest>>,

// In ws/mod.rs handler
AgentMessage::ContentManifest(manifest) => {
    if let Some(ref pod_id) = registered_pod_id {
        tracing::info!("Pod {} content manifest: {} cars, {} tracks",
            pod_id, manifest.cars.len(), manifest.tracks.len());
        state.pod_manifests.write().await.insert(pod_id.clone(), manifest);
    }
}
```

### Pattern 4: Filtered Catalog Response
**What:** Extend get_catalog() to accept an optional manifest filter.
**When to use:** In customer_ac_catalog and similar endpoints when pod_id is known.
**Example:**
```rust
// catalog.rs
pub fn get_filtered_catalog(manifest: Option<&ContentManifest>) -> Value {
    match manifest {
        None => get_catalog(), // fallback to full static catalog
        Some(m) => {
            let car_ids: HashSet<&str> = m.cars.iter().map(|c| c.id.as_str()).collect();
            let track_ids: HashSet<&str> = m.tracks.iter().map(|t| t.id.as_str()).collect();
            // Filter ALL_CAR_IDS and ALL_TRACK_IDS against manifest
            // Include has_ai and pit_count per track config in response
        }
    }
}
```

### Anti-Patterns to Avoid
- **Scanning during event loop:** Content scanning does blocking I/O. Use `tokio::task::spawn_blocking` if done after the agent's event loop starts, or do it synchronously before entering the loop.
- **Storing manifest in PodInfo:** PodInfo is sent on every heartbeat (5s). The manifest is static and large (~350 car IDs + 50 track entries). It should be stored separately, not in PodInfo.
- **Client-side filtering:** Never send full catalog and filter in PWA. Server-side only -- the PWA must never see options that would fail.
- **Scanning skins/data folders deeply:** Only scan what's needed for validation: car folder existence, track folder existence, ai/ folder presence, ui_track.json pitboxes. No recursive deep scan.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON parsing of ui_track.json | Manual string splitting | serde_json::Value or a typed struct | pitboxes is a string field ("15"), needs proper parsing |
| Track config detection | Simple "has any subfolder" check | Check for subfolders containing data/ or ai/ directories | Some tracks have non-config folders (skins, sfx) |
| Concurrent manifest access | Mutex or manual locking | RwLock<HashMap<String, ContentManifest>> | Established AppState pattern, reads >> writes |

**Key insight:** The filesystem scanning is straightforward (read_dir + metadata checks), but the tricky part is correctly detecting track configs vs. other subfolders, and handling the two valid track structures (default layout at root vs. named configs in subfolders).

## Common Pitfalls

### Pitfall 1: Track Config Detection Ambiguity
**What goes wrong:** Some tracks have subfolders that are NOT configs (e.g., `skins/`, `sfx/`, `extension/`). Treating every subfolder as a config produces invalid entries.
**Why it happens:** AC track folder structure allows arbitrary subfolders alongside config folders.
**How to avoid:** A config subfolder must contain at least one of: `data/` directory, `ai/` directory, or `models.ini` file. If a subfolder has none of these, it is not a config.
**Warning signs:** Manifest shows unexpected config names like "skins" or "sfx".

### Pitfall 2: Default Layout (No Config Subfolders)
**What goes wrong:** Tracks like `magione` or `trento-bondone` have no config subfolders -- their `data/`, `ai/`, and `ui/` folders are at the track root level. If the scanner only looks inside subfolders, these tracks show zero configs.
**Why it happens:** AC supports both single-layout tracks (root level) and multi-layout tracks (config subfolders).
**How to avoid:** If no valid config subfolders are found, check for `data/` at the track root. If present, create a default config entry with config="" (empty string). Check `ai/` at root for has_ai and `ui/ui_track.json` at root for pitboxes.
**Warning signs:** Popular tracks like Magione missing from manifest.

### Pitfall 3: pitboxes Is a String, Not an Integer
**What goes wrong:** `ui_track.json` stores pitboxes as `"pitboxes": "15"` (string), not `"pitboxes": 15` (integer). Direct deserialization to u32 fails silently.
**Why it happens:** AC's JSON format uses strings for many numeric fields (length, width, pitboxes).
**How to avoid:** Parse as `serde_json::Value`, extract as string, then `.parse::<u32>()`. Fall back to 19 on parse failure or missing field.
**Warning signs:** All tracks showing default pit count of 19.

### Pitfall 4: ui_track.json Location Varies by Config
**What goes wrong:** For multi-config tracks, `ui_track.json` lives at `ui/{config_name}/ui_track.json`, NOT inside the config subfolder itself.
**Why it happens:** AC separates runtime data (in config subfolder) from UI metadata (in ui/ subfolder).
**How to avoid:**
- Default layout (no configs): `{track}/ui/ui_track.json`
- Named config: `{track}/ui/{config_name}/ui_track.json`
**Warning signs:** ui_track.json not found for multi-config tracks.

### Pitfall 5: Stale Manifest After Content Change
**What goes wrong:** If staff installs new content while a pod is running, the manifest is stale until rc-agent restarts or reconnects.
**Why it happens:** Scanning only happens at startup/reconnect per the design decision.
**How to avoid:** This is acceptable for v1 -- staff can restart rc-agent on the pod to rescan. The existing fallback-to-static-catalog behavior provides a safety net.
**Warning signs:** Customer reports missing content that was just installed.

### Pitfall 6: Empty AI Folder
**What goes wrong:** Some mod tracks have an empty `ai/` folder (created by mod tools but no actual AI line files). Scanner reports has_ai=true but AC fails to race with AI.
**Why it happens:** Mod authors sometimes include empty placeholder directories.
**How to avoid:** Check that `ai/` folder contains at least one `.ai` or `.aip` file, not just that the directory exists. Use `read_dir()` and check `count > 0` or look for specific file extensions.
**Warning signs:** Race vs AI option shown but AI cars don't appear on track.

## Code Examples

Verified patterns from the existing codebase:

### Adding a New AgentMessage Variant (Established Pattern)
```rust
// Source: rc-common/src/protocol.rs (existing pattern)
// All variants follow this form:
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum AgentMessage {
    // ... existing variants ...

    /// Pod reports installed AC content at startup/reconnect
    ContentManifest(ContentManifest),
}
// Wire format: {"type":"content_manifest","data":{"cars":[...],"tracks":[...]}}
```

### Adding Per-Pod State in AppState (Established Pattern)
```rust
// Source: rc-core/src/state.rs (existing pattern -- pod_deploy_states)
pub pod_manifests: RwLock<HashMap<String, ContentManifest>>,

// In AppState::new():
pod_manifests: RwLock::new(HashMap::new()),
// No pre-population needed -- manifests arrive dynamically on connect
```

### Handling New AgentMessage in WS Handler (Established Pattern)
```rust
// Source: rc-core/src/ws/mod.rs (existing handler at line 117)
// Pattern: match on variant, extract pod_id, store in state, log activity
AgentMessage::ContentManifest(manifest) => {
    if let Some(ref pod_id) = registered_pod_id {
        let car_count = manifest.cars.len();
        let track_count = manifest.tracks.len();
        tracing::info!("Pod {} content manifest: {} cars, {} tracks", pod_id, car_count, track_count);
        log_pod_activity(&state, pod_id, "content", "Content Scanned",
            &format!("{} cars, {} tracks", car_count, track_count), "agent");
        state.pod_manifests.write().await.insert(pod_id.clone(), manifest);
    }
}
```

### Sending Message After Register (Established Pattern)
```rust
// Source: rc-agent/src/main.rs line 491-506
// After Register is sent successfully, send ContentManifest:
let manifest = content_scanner::scan_ac_content();
let manifest_msg = AgentMessage::ContentManifest(manifest);
let json = serde_json::to_string(&manifest_msg)?;
if ws_tx.send(Message::Text(json.into())).await.is_err() {
    // Handle send failure (reconnect)
}
```

### Filesystem Scanning (Pattern from detect_installed_games)
```rust
// Source: rc-agent/src/main.rs line 92 (detect_installed_games)
// The codebase already scans for installed software at startup.
// Content scanning follows the same pattern but walks directories.
fn scan_cars(cars_dir: &Path) -> Vec<CarManifestEntry> {
    let Ok(entries) = std::fs::read_dir(cars_dir) else {
        tracing::warn!("Cannot read cars directory: {:?}", cars_dir);
        return Vec::new();
    };
    entries.filter_map(|entry| {
        let entry = entry.ok()?;
        let metadata = entry.metadata().ok()?;
        if !metadata.is_dir() { return None; }
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden/system folders
        if name.starts_with('.') { return None; }
        Some(CarManifestEntry { id: name })
    }).collect()
}
```

### Parsing pitboxes from ui_track.json
```rust
fn parse_pit_count(track_dir: &Path, config: &str) -> Option<u32> {
    let ui_json_path = if config.is_empty() {
        track_dir.join("ui").join("ui_track.json")
    } else {
        track_dir.join("ui").join(config).join("ui_track.json")
    };

    let content = std::fs::read_to_string(&ui_json_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    // pitboxes is a STRING field in AC's ui_track.json: "pitboxes": "15"
    json.get("pitboxes")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u32>().ok())
}
```

### Catalog API with Pod Filtering
```rust
// Extend customer_ac_catalog to accept pod_id query parameter
#[derive(Debug, Deserialize)]
struct CatalogQuery {
    pod_id: Option<String>,
}

async fn customer_ac_catalog(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CatalogQuery>,
) -> Json<Value> {
    let manifest = if let Some(ref pod_id) = query.pod_id {
        state.pod_manifests.read().await.get(pod_id).cloned()
    } else {
        None
    };
    Json(catalog::get_filtered_catalog(manifest.as_ref()))
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Static hardcoded catalog (catalog.rs) | Per-pod dynamic content scanning | Phase 5 (this phase) | Customers only see installed content |
| No AI capability check | Per-track-config AI line detection | Phase 5 (this phase) | Race vs AI and Track Day hidden when no AI data |
| Fixed 19 AI max everywhere | Per-track pit count caps AI slider | Phase 5 (this phase) | Realistic AI limits per track |
| No launch validation | Pre-launch combo validation gate | Phase 5 (this phase) | Prevents invalid launches |

**Retained:**
- Static catalog remains as fallback when manifest is unavailable
- `FEATURED_TRACKS` and `FEATURED_CARS` arrays provide display names and categories
- `build_custom_launch_args()` continues to construct launch parameters (gains validation step before it)
- `effective_ai_cars()` in rc-agent caps at `MAX_AI_SINGLE_PLAYER` (19) -- now also constrained by track pit_count

## Open Questions

1. **Track IDs with spaces or special characters**
   - What we know: `ALL_TRACK_IDS` in catalog.rs includes `"shibuya-hachiko drift"` (with a space). This is a real folder name on Pod 8.
   - What's unclear: Whether AC handles this consistently across all code paths.
   - Recommendation: Scanner should handle this transparently -- just use the folder name as-is. The matching in get_filtered_catalog uses string equality.

2. **Default config naming convention**
   - What we know: Single-layout tracks have data/ai/ui at root (no config subfolder). Multi-layout tracks have named subfolders.
   - What's unclear: The exact string to use for the default config in the manifest.
   - Recommendation: Use empty string `""` for default config. This matches how AC represents default layouts internally and in server config (`TRACK_CONFIG=`).

3. **How PWA currently selects pod_id for catalog requests**
   - What we know: Customer books via PWA, gets assigned to a pod via reservation system.
   - What's unclear: At what point in the PWA flow the pod_id is known and available to pass to the catalog endpoint.
   - Recommendation: Customer browses catalog after reservation assigns them a pod. The `customer_active_reservation` endpoint returns the pod_id. If no reservation yet, serve unfiltered catalog (fallback).

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust test framework) |
| Config file | Cargo.toml per crate (existing) |
| Quick run command | `cargo test -p rc-common -p rc-agent -- --lib` |
| Full suite command | `cargo test -p rc-common -p rc-agent -p rc-core` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CONT-07 | Content scanner produces correct manifest from filesystem | unit | `cargo test -p rc-agent content_scanner -- -x` | Wave 0 |
| CONT-07 | ContentManifest serde roundtrip | unit | `cargo test -p rc-common protocol::tests::test_content_manifest_roundtrip -- -x` | Wave 0 |
| CONT-01 | Filtered catalog includes only manifest cars | unit | `cargo test -p rc-core catalog::tests::test_filtered_catalog_cars -- -x` | Wave 0 |
| CONT-02 | Filtered catalog includes only manifest tracks | unit | `cargo test -p rc-core catalog::tests::test_filtered_catalog_tracks -- -x` | Wave 0 |
| CONT-05 | Tracks without AI hide race/trackday session types | unit | `cargo test -p rc-core catalog::tests::test_no_ai_hides_sessions -- -x` | Wave 0 |
| CONT-06 | Pit count caps AI slider max | unit | `cargo test -p rc-core catalog::tests::test_pit_count_caps_ai -- -x` | Wave 0 |
| CONT-04 | Launch validation rejects invalid car/track/session | unit | `cargo test -p rc-core -- validate_launch -- -x` | Wave 0 |
| SESS-07 | Invalid session combos not presented | unit | Same as CONT-05 test | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common -p rc-agent -- --lib`
- **Per wave merge:** `cargo test -p rc-common -p rc-agent -p rc-core`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `rc-agent/src/content_scanner.rs` -- new module with test helpers for mock filesystem
- [ ] `rc-common/src/types.rs` -- ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest structs
- [ ] `rc-common/src/protocol.rs` -- ContentManifest variant + serde roundtrip test
- [ ] `rc-core/src/catalog.rs` -- get_filtered_catalog() + test module
- [ ] No framework install needed -- cargo test is already set up

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `rc-common/src/protocol.rs` -- AgentMessage enum structure, serde configuration
- Codebase analysis: `rc-common/src/types.rs` -- PodInfo struct, existing data patterns
- Codebase analysis: `rc-core/src/state.rs` -- AppState structure, RwLock<HashMap> pattern
- Codebase analysis: `rc-core/src/catalog.rs` -- get_catalog(), FEATURED_TRACKS/CARS, ALL_TRACK_IDS/CAR_IDS
- Codebase analysis: `rc-core/src/api/routes.rs` -- customer_ac_catalog, list_ac_tracks, list_ac_cars endpoints
- Codebase analysis: `rc-core/src/ws/mod.rs` -- handle_agent message dispatch pattern
- Codebase analysis: `rc-agent/src/main.rs` -- Register flow, detect_installed_games pattern
- Codebase analysis: `rc-agent/src/ac_launcher.rs` -- find_ac_dir(), effective_ai_cars(), MAX_AI_SINGLE_PLAYER

### Secondary (MEDIUM confidence)
- [Hagn's Site - ui_track.json](https://site.hagn.io/assettocorsa/modding/tracks/ui/ui_track-json) -- pitboxes is a string field, sets AI selection limit
- [Assetto Corsa Mods - Track layouts](https://assettocorsamods.net/threads/track-layouts-how-to.364/) -- config subfolder structure with ai/ and data/
- [OverTake.gg - Adding pit boxes](https://www.overtake.gg/threads/adding-more-start-boxes.182551/) -- pit stall count determines max AI

### Tertiary (LOW confidence)
- None -- all claims verified with codebase or official AC documentation

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all existing crates
- Architecture: HIGH -- follows established codebase patterns (AgentMessage variant, AppState map, API query params)
- Pitfalls: HIGH -- verified with AC documentation and codebase analysis of edge cases
- Content scanning: HIGH -- AC filesystem structure is well-documented and stable

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable domain -- AC content format has not changed in years)
