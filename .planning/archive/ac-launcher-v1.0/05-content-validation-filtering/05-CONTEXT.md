# Phase 5: Content Validation & Filtering - Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Ensure every car, track, and session option shown to customers is guaranteed valid on their specific pod. rc-agent scans the pod's AC install at startup, sends a content manifest to rc-core over WebSocket, and rc-core filters the catalog API response per-pod. Tracks without AI lines hide AI session types. Pit stall count caps the AI slider. Invalid combos are impossible to select. Curated presets are Phase 7. Staff kiosk configuration is Phase 8.

</domain>

<decisions>
## Implementation Decisions

### Content Scanning Architecture
- Scanning happens **on each pod (rc-agent)**, not centralized
- rc-agent scans AC content folders at **startup and on reconnect** to rc-core
- rc-agent sends a full **ContentManifest** to rc-core as a new WebSocket message (AgentMessage variant), sent after Connect handshake
- rc-core caches the manifest per-pod and uses it to filter catalog API responses
- Pods are **mostly identical but may have occasional differences** (test mods, missing content) — per-pod filtering is a real requirement, not just a safety net
- AC installation path: `C:\Program Files (x86)\Steam\steamapps\common\assettocorsa`
- Content folders to scan: `content/cars/` (car IDs) and `content/tracks/` (track IDs + configs)

### Manifest Contents
- **Car entries:** folder name (car ID) from `content/cars/`
- **Track entries:** folder name (track ID) + list of config subfolder names (layout variants)
- **Per-config metadata:** has_ai (bool — `ai/` folder exists with files), pit_count (Option<u32> — from ui_track.json pitboxes field)
- Track configs checked **independently** for AI support (e.g. spa/gp may have AI, spa/drift may not)

### AI Line Detection
- Simple folder check: `content/tracks/{track_id}/{config}/ai/` folder exists with files → AI capable
- No deep validation of .ai file contents — folder existence is sufficient (AC itself uses the same check)
- Tracks without AI lines hide **Race vs AI** and **Track Day** session types entirely (not greyed out)

### Pit Stall Count
- Parse `ui/ui_track.json` for pitboxes count per track config
- **Fallback:** if ui_track.json missing or no pitboxes field → default to **19** (AC max single-player)
- AI opponent slider max **dynamically updates** based on selected track's pit count minus 1 (for player)

### Invalid Combo Handling
- **Server-side filtering only** — rc-core filters catalog response per-pod before sending to PWA
- PWA only receives valid options — impossible to select something broken
- Invalid session types (no AI → no Race vs AI/Track Day) are **hidden entirely**, not greyed out
- **Launch-time validation gate:** rc-core validates combo against pod manifest before sending LaunchGame — safety net against API misuse or stale PWA cache

### Scan Failure Fallback
- If a pod connects with empty manifest (scan failed), **fall back to static hardcoded catalog** (current catalog.rs)
- Customer can still browse and attempt launch — better than showing nothing

### Claude's Discretion
- Manifest serialization format (compact vs verbose)
- Exact WebSocket message naming and structure
- How to handle track configs that are just empty default layouts
- Whether to scan car skins (not needed for validation, but could enrich display)
- Caching strategy for per-pod filtered catalogs

</decisions>

<specifics>
## Specific Ideas

- ContentManifest as new AgentMessage variant fits the existing protocol pattern (GameStatusUpdate, FfbZeroed, etc.)
- The manifest should be lightweight — just IDs + metadata, not full display names (rc-core has the display name mapping in catalog.rs)
- Per-config AI check is important because AC tracks like Nordschleife have multiple layouts where only some have AI lines
- Dynamic slider max gives immediate feedback — customer sees "Max 15 AI" and understands the track's limits

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `catalog.rs` (rc-core): Static catalog with 325 cars, 50 tracks, display names, categories — becomes the fallback/reference for filtering
- `get_catalog()` function: Returns JSON with tracks/cars/categories — needs to accept pod_id parameter for per-pod filtering
- `ALL_CAR_IDS` / `ALL_TRACK_IDS`: Hardcoded reference lists — useful as fallback when manifest unavailable
- `AgentMessage` enum (protocol.rs): Existing WebSocket protocol — add ContentManifest variant
- `detect_installed_games()` (main.rs): Pattern for scanning installed content at startup

### Established Patterns
- WebSocket message flow: rc-agent sends typed messages → rc-core dispatches by variant
- Catalog API: `/ac/content/tracks`, `/ac/content/cars`, `/customer/ac/catalog` — extend with pod_id query param
- `build_custom_launch_args()`: Constructs launch params — add validation step before this

### Integration Points
- rc-agent main.rs: After WebSocket connect, send ContentManifest
- rc-core ws/mod.rs: Handle ContentManifest → store in AppState per pod
- rc-core api/routes.rs: Filter catalog responses using stored manifest
- rc-core auth/mod.rs or booking flow: Validate combo before LaunchGame dispatch

</code_context>

<deferred>
## Deferred Ideas

- Curated presets using validated content (Phase 7)
- Staff kiosk content configuration (Phase 8)
- Car skin scanning for richer display (not needed for validation)
- Content auto-download or sync between pods (ops concern, not software)

</deferred>

---

*Phase: 05-content-validation-filtering*
*Context gathered: 2026-03-14*
