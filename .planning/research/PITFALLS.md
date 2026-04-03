# Pitfalls Research

**Domain:** Game Intelligence System — adding per-pod game inventory, proactive combo validation, launch timeline telemetry, reliability scoring, and fleet game matrix dashboard to the existing Racing Point eSports venue management system (v41.0).
**Researched:** 2026-04-03
**Confidence:** HIGH — all pitfalls drawn from documented incidents in this exact codebase: CLAUDE.md standing rules, MEMORY.md incident log, git history, and direct code inspection of `crates/rc-agent/src/content_scanner.rs`, `tier_engine.rs`, `diagnostic_engine.rs`, `game_launcher.rs`, `steam_checks.rs`, `ws/mod.rs`, `fleet_health.rs`, `preset_library.rs`, `crates/rc-common/src/types.rs`, `protocol.rs`, `fleet_event.rs`. No hypothetical pitfalls.

---

## Critical Pitfalls

### Pitfall 1: Serde Silently Drops Unknown Fields — New Payload Fields Never Reach Agent

**What goes wrong:**
Adding new fields to `ContentManifest`, `AcLaunchParams`, or any struct sent over the WS protocol will silently produce the zero/default value on the receiving side if the receiver has an older binary that doesn't have the field. Worse: adding a field to the kiosk JSON payload that doesn't match the exact Rust field name in rc-common types means the selection is ignored with `{ok: true}` returned and the game launching with default config. This already caused the `ai_difficulty` / `ai_level` mismatch bug (kiosk sent `ai_difficulty: "easy"`, agent expected `ai_level: u32` — zero AI opponents launched with no error).

**Why it happens:**
Serde's default behavior is `#[serde(deny_unknown_fields)]` is NOT set, so extra fields from a newer sender are silently dropped by an older receiver. The inverse — a new receiver expecting a field the sender doesn't send — produces the struct's `Default::default()` value with no warning log.

**How to avoid:**
- Before adding any field to a WS protocol struct in `rc-common`, grep `buildLaunchArgs()` and all JSON payload constructors in kiosk/PWA/admin to verify the field name matches exactly (standing rule already exists for this).
- Add a `#[serde(deny_unknown_fields)]` annotation to new structs where forward-compat is not needed.
- After adding new protocol fields: verify the generated payload on a pod by reading back the actual file (race.ini, game state) — API success is NOT proof of correct config.
- Run `cargo test -p rc-common` to catch roundtrip failures before deploy.

**Warning signs:**
- Game launches successfully with `{ok: true}` but customer sees wrong car count, AI level, or track config.
- New field always shows its default value in logs despite kiosk sending a different value.
- `ContentManifest` for Steam games shows 0 entries despite games being installed.

**Phase to address:** Phase adding new WS protocol fields for game inventory (per-pod manifest extension, `InstalledGame` variants). Must verify roundtrip in tests AND on-pod behavior.

---

### Pitfall 2: `ok: true` Means WS Queued, Not Delivered — Launch Commands Silently Lost

**What goes wrong:**
The server returns `{ok: true}` from `/games/launch` when the WS message is queued for send, not when the agent receives and acknowledges it. If WS drops between queue and delivery, the launch command is lost. `GameTracker` stays in `Launching` permanently, blocking all future launches on that pod. v40.0 added ACK protocol (Phase 312) to fix this for the basic launch path, but any new WS messages introduced for v41.0 (inventory push, combo validation request, launch timeline events) will NOT have ACK wiring unless explicitly added.

**Why it happens:**
The ACK protocol (using `pending_command_acks` and `CoreMessage::command_id`) was retrofitted for `LaunchGame` in v40.0. New message types added in v41.0 will use the simpler fire-and-forget `tx.send(msg).await` path by default.

**How to avoid:**
- For any new server-to-agent message that requires confirmation (combo validation request, inventory rescan trigger), wire through `CoreMessage::wrap()` + `pending_command_acks` ACK pattern.
- For informational pushes from agent to server (inventory report, launch timeline event), no ACK needed — but add retry on reconnect: send latest state after each WS reconnect.
- Test: deliberately drop WS mid-operation and verify the system recovers (tracker unsticks, inventory resends).

**Warning signs:**
- Pod shows stale inventory after reconnect.
- Combo validation result never arrives but no error is logged.
- `GameTracker` state does not progress after a new trigger message is sent.

**Phase to address:** Phase adding launch timeline WS events and combo validation request/response messages.

---

### Pitfall 3: Single-Fetch-at-Boot Without Retry — Inventory Stale Forever After Server Restart

**What goes wrong:**
`content_scanner.rs` currently scans AC content at startup and sends a single `ContentManifest` WS message. If the server is down at boot, the manifest is never sent and `pod_manifests` in `AppState` stays empty for that pod indefinitely. When v41.0 adds Steam + non-Steam scanning, the same pattern will produce `GameManager` serving an empty game list to the kiosk until the pod is manually restarted.

This is the exact same pattern as the allowlist incident: "if the server is down at boot, pods get empty default but self-heal within 5 minutes once server is back" — but only because the allowlist already has the periodic re-fetch (commit `821c3031`). Content manifests do NOT have periodic re-fetch.

**Why it happens:**
Content scanning is triggered once at startup and on WS reconnect (per the module doc comment). WS reconnect does resend, but if the server restarts while pods are running, the pods don't automatically retrigger a scan — they wait for the next WS reconnect event, which may be hours away if the connection was already established.

**How to avoid:**
- Add a periodic re-fetch for the content manifest: re-scan and re-send on every WS reconnect (already triggered in the reconnect path) AND add a 24-hour periodic rescan for content changes (game installs/uninstalls are rare but happen).
- On the server side, when a pod reconnects (registers), proactively request a fresh manifest via `RequestContentScan` message.
- Never assume the first scan is authoritative.

**Warning signs:**
- Kiosk shows no games for a pod that was running when server restarted.
- `pod_manifests` map in server debug shows an entry from boot time that is hours old.
- Fleet game matrix dashboard shows zeroes for pods that were already running.

**Phase to address:** Phase 1 (content scanner extension). Must add rescan-on-reconnect and periodic rescan before this phase ships.

---

### Pitfall 4: Steam Library Path Hardcoding — Custom Library Locations Silently Missed

**What goes wrong:**
`steam_checks.rs` checks three hardcoded Steam library paths (`C:\...\steamapps`, `D:\SteamLibrary\steamapps`, `E:\SteamLibrary\steamapps`) for `appmanifest_*.acf` files. The code has a comment: "custom Steam library paths require full `libraryfolders.vdf` parsing." When `content_scanner.rs` is extended to scan Steam games for the full inventory, using the same hardcoded path list will silently miss games installed in custom library locations — the pod will appear to not have a game it actually has.

**Why it happens:**
Hardcoded paths are a common shortcut because they cover 90% of cases. But `libraryfolders.vdf` parsing is required for correctness. The existing code explicitly acknowledges this gap and returns a non-error to avoid blocking (standing rule: "not returning Err here — custom Steam library paths exist and we don't want to block").

**How to avoid:**
- Parse `C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf` to discover all registered Steam library paths before scanning.
- VDF format is simple key-value — a basic regex or line-by-line parser (no external crate needed).
- Fall back to hardcoded paths only if VDF is missing or unparseable, logging a warning.
- Test: install a game to `D:\SteamLibrary` on Pod 8, verify inventory scanner finds it.

**Warning signs:**
- Kiosk shows game unavailable on pod where game is visually installed (Steam library on D: or E: drive).
- Fleet game matrix shows inconsistent presence across pods that all have the same game.
- Reliability score shows zero launches for a game that customers have played.

**Phase to address:** Phase adding Steam library scanning to `content_scanner.rs`.

---

### Pitfall 5: Combo Validation at Boot Fires Before Agent Is Fully Initialized

**What goes wrong:**
Boot-time proactive combo validation cross-references presets against the content manifest. The manifest is built during boot scanning. But if validation is triggered before the WS connection is established (and thus before the server has pushed current presets), or before the Steam library scan completes, the validation runs against an empty or stale preset list and silently auto-disables combos that are actually valid.

This is analogous to the MAINTENANCE_MODE pitfall: a guard that fires during boot before its inputs are ready will produce incorrect results that persist indefinitely.

**Why it happens:**
Boot sequencing in `rc-agent` has multiple async tasks starting in parallel. There is no formal synchronization point that says "content scan done AND presets received AND Steam checks complete — now validate." The easy implementation is to trigger validation as soon as the manifest is built, which may be before presets arrive from the server.

**How to avoid:**
- Gate combo validation behind: (1) content manifest scan complete AND (2) first preset push from server received (or a 30s timeout with warning, not silent skip).
- Use a `tokio::sync::watch` or barrier pattern to sequence: `scan_complete → presets_loaded → validate`.
- If server is down at boot and presets can't be fetched, log a WARNING and defer validation rather than running on empty — same pattern as allowlist.
- Add a startup log event: "Combo validation deferred: waiting for preset push from server."

**Warning signs:**
- Combos show as invalid immediately after pod boot but become valid 30 seconds later.
- Admin receives WhatsApp alert about broken combos that were valid the day before.
- Validation runs with zero presets (log: "Validated 0 combos").

**Phase to address:** Phase adding proactive boot-time combo validation.

---

### Pitfall 6: DiagnosticTrigger Enum Addition Breaks Serde Deserialization of Old KB Entries

**What goes wrong:**
`DiagnosticTrigger` is serialized into the Knowledge Base (SQLite) as JSON. Adding new variants (`GameLaunchTimeout`, `CrashLoop`) without `#[serde(other)]` or handling for unknown variants means the KB deserialization will fail or panic when reading old entries stored under old variant names. This silently corrupts the KB replay on boot or produces `serde_json::Error` that gets swallowed.

**Why it happens:**
Enum serde in Rust has no forward-compat by default. A DB entry serialized as `"GameLaunchFail"` deserializes fine, but if someone added a new variant that shadows an old one, or if the variant serialization format changed, all historical KB entries for that trigger become unreadable.

**How to avoid:**
- Add `#[serde(other)]` to a catch-all variant on `DiagnosticTrigger` (e.g. `Unknown`) before adding new variants.
- Write a migration in `db.rs` that reads existing KB entries, verifies they deserialize, and repairs any that fail.
- In `tier_engine.rs`, `match trigger { DiagnosticTrigger::Unknown => { log warning; return NotApplicable } }`.
- Test: write a KB entry with an unknown trigger name, start the tier engine, verify it doesn't panic.

**Warning signs:**
- `knowledge_base.rs` logs errors at startup: "Failed to deserialize KB entry."
- Tier 1/2 hit rate drops after adding new trigger variants (stale KB entries silently excluded).
- KB solution count visible in admin dashboard drops after deploy.

**Phase to address:** Phase adding `GameLaunchTimeout` and `CrashLoop` trigger variants.

---

### Pitfall 7: Crash Loop Detection Already in `fleet_health.rs` — Parallel Implementation Will Diverge

**What goes wrong:**
`fleet_health.rs` already detects crash loops: `recent_count > 3 && uptime < 30s` sets `crash_loop: true` on the `PodFleetStatus`. The v41.0 plan says to "wire crash loop detection into Meshed Intelligence." If this is implemented as a new `CrashLoop` variant in `DiagnosticTrigger` that re-implements the detection logic, there will be two independent crash loop detectors that can disagree, producing duplicate alerts or conflicting state.

**Why it happens:**
Two teams (or two milestones) implementing the same detection independently without checking what already exists. The Meshed Intelligence tier engine runs in `rc-agent` (pod side), while `fleet_health.rs` runs in `racecontrol` (server side). Both have access to startup report data but via different paths.

**How to avoid:**
- Audit `fleet_health.rs` lines 379-390 BEFORE designing the `CrashLoop` trigger.
- The server-side detection in `fleet_health.rs` is the right place to emit the `FleetEvent` that triggers the agent-side `DiagnosticTrigger`.
- Wire: server detects crash loop → emits `FleetEvent::CrashLoopDetected { pod_id }` → WS push to agent → agent `DiagnosticTrigger::CrashLoop` → tier engine.
- Do NOT replicate the detection heuristic on both sides — single source of truth.

**Warning signs:**
- Agent receives a `CrashLoop` trigger but the server fleet health shows `crash_loop: false` (or vice versa).
- Uday receives two WhatsApp alerts for the same crash loop from different code paths.
- `crash_loop` flag in fleet health auto-clears but tier engine is still mid-diagnosis.

**Phase to address:** Phase adding crash loop wiring to Meshed Intelligence.

---

### Pitfall 8: WhatsApp Chain Failure Alert Goes Through Bono VPS Relay — Direct Path Will Fail

**What goes wrong:**
The v41.0 plan mentions "chain failure WhatsApp alerts." The existing `send_whatsapp()` function in `whatsapp_alerter.rs` calls the Evolution API directly. However, per MEMORY.md standing rule: "Promotions/deals/marketing must go via Bono VPS Evolution API, not venue tunnel." And per `whatsapp_escalation.rs`, Tier 5 escalation goes through `EscalationRequest` → server → Bono relay. If a new direct WhatsApp call is added in rc-agent for crash loop / chain failure, it will fail silently when the venue Evolution API is down (it's on Bono VPS, not venue).

**Why it happens:**
The correct alert path for agent-side events is: `agent emits EscalationRequest via WS → server receives → routes to whatsapp_escalation.rs → Bono relay`. The tempting shortcut is to call Evolution API directly from rc-agent using a hardcoded URL — this worked in an early prototype but the API endpoint moved to Bono VPS.

**How to avoid:**
- All WhatsApp alerts from rc-agent MUST route through `EscalationRequest` WS message → server → `whatsapp_escalation.rs` → Bono relay.
- Never add a direct HTTP client call to Evolution API in rc-agent.
- Verify in testing: disconnect Bono VPS relay, trigger a chain failure, verify the alert is queued (not lost) and delivered when relay reconnects.
- The `EscalationRequest` struct already exists in `rc-common/src/protocol.rs` — use it.

**Warning signs:**
- Chain failure WhatsApp alerts arrive when venue is online but not when Bono VPS is briefly down.
- Alert sent from rc-agent but Uday never receives it (no log in whatsapp_escalation.rs for the incident).
- Two WhatsApp messages for the same incident (direct + relay both fire).

**Phase to address:** Phase adding crash loop and chain failure WhatsApp alerts.

---

### Pitfall 9: Reliability Score Aggregates Across ALL Pods — Per-Pod Filtering Hidden in SQL

**What goes wrong:**
`list_presets_with_reliability()` in `preset_library.rs` computes reliability by averaging across ALL pods for a combo: `AVG(success_rate) FROM combo_reliability WHERE sim_type = ?`. A combo that works on 7 pods but crashes on 1 will have high average reliability but still fail for customers on that one pod. The kiosk game filter needs per-pod availability, not fleet-average — but the existing API returns only the fleet aggregate.

This same issue affects the "fleet game matrix" dashboard — if it queries fleet-aggregate reliability rather than per-pod, it will show "reliable" for combos that are actually broken on specific pods.

**Why it happens:**
Fleet-aggregate is the current design for the reliability score (it was the correct MVP). v41.0 adds per-pod inventory filtering, which requires per-pod reliability queries. The existing API endpoint and SQL query don't support this distinction.

**How to avoid:**
- Add `pod_id` parameter to the reliability query path for kiosk-facing endpoints.
- The `combo_reliability` table already has a `pod_id` column (unique index: `pod_id, sim_type, car, track`).
- For the fleet game matrix, show both: per-pod status AND fleet-aggregate score as separate columns.
- Do NOT change the existing fleet-aggregate query that powers the admin dashboard — additive only.

**Warning signs:**
- Kiosk on Pod 3 shows Forza available despite Forza not being installed on Pod 3 (uses fleet average which includes pods where it is installed).
- Reliability dashboard shows 85% success rate for a combo that always fails on Pod 7.
- `combo_reliability` queries show `pod_id = NULL` entries (fleet aggregate row was accidentally created).

**Phase to address:** Phase adding kiosk game filtering by pod availability and fleet game matrix.

---

### Pitfall 10: Content Manifest Only Covers AC — Non-AC Games Require Different Detection Logic

**What goes wrong:**
`ContentManifest` currently has `cars: Vec<CarManifestEntry>` and `tracks: Vec<TrackManifestEntry>` — it is AC-specific by design. When v41.0 extends `content_scanner.rs` to all games, adding `installed_games: Vec<InstalledGame>` as a new field is the natural approach. But the server stores manifests in `pod_manifests: RwLock<HashMap<String, ContentManifest>>` — if the type is extended, ALL downstream readers of `pod_manifests` must be audited for the cascade (standing rule: cascade updates are recursive).

Additionally, for non-AC games, "installed" means different things:
- Steam games: `appmanifest_<app_id>.acf` exists in a Steam library folder
- Non-Steam games (iRacing): presence of `iRacingSim64DX11.exe` in a known path
- Forza (Windows Store): presence of game package — no `.acf` file

Each detection method needs its own probe function.

**How to avoid:**
- Extend `ContentManifest` in `rc-common/src/types.rs` with `installed_games: Vec<InstalledGame>` field.
- Run `cargo test -p rc-common` to find all consumers via compile errors.
- Audit `ws/mod.rs` (line 920, `ContentManifest` handler), `catalog.rs`, any API routes that serialize `pod_manifests`.
- Write per-game detector functions in `content_scanner.rs` with explicit tests for each detection method.
- For Windows Store games (Forza), use `winreg` or file path probing — NOT Steam ACF check.

**Warning signs:**
- `cargo build` succeeds but runtime panics on manifest deserialization (new field not present in old JSON).
- Server-side code that reads `manifest.cars` compiles but produces incorrect results because the new game type uses `installed_games` instead.
- Forza always shows as "not installed" even when it is.

**Phase to address:** Phase 1 (content scanner extension). Cascade audit is mandatory before shipping.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Single-fetch-at-boot for content manifest | Simpler code | Stale game list after server restart | Never — use periodic re-fetch pattern (already exists for allowlist, feature flags) |
| Fleet-average reliability only (no per-pod) | Simpler SQL | Kiosk shows unavailable games on specific pods | Only in MVP; per-pod needed before kiosk filtering goes live |
| Hardcoded Steam library paths | Works for 90% of installs | Silently misses custom library locations | Only as fallback after VDF parse fails |
| Combining crash loop detection in both fleet_health.rs and DiagnosticTrigger | Faster | Two disagreeing detectors, duplicate alerts | Never — wire fleet_health as the single source |
| Skip `#[serde(other)]` on new DiagnosticTrigger variants | Less code | KB deserialization panics on old entries | Never for enums stored in DB |
| Direct Evolution API call from rc-agent | Simpler | Alert path silently fails when Bono VPS is down | Never — always route through EscalationRequest |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| content_scanner → WS protocol → server pod_manifests | Extend ContentManifest type without auditing all downstream consumers | Extend type, compile, let compiler find all readers; cascade-audit each |
| DiagnosticTrigger new variants → KB persistence | Add variants without serde forward-compat | Add `#[serde(other)]` variant first, then add new variants |
| crash_loop detection → tier_engine | Re-implement detection in rc-agent, bypassing fleet_health | Emit FleetEvent from server fleet_health, push to agent via WS as a trigger |
| WhatsApp alerts from rc-agent | Direct HTTP call to Evolution API | Route through EscalationRequest → server → whatsapp_escalation.rs → Bono relay |
| combo_reliability queries for kiosk filtering | Use fleet-aggregate AVG (no pod_id filter) | Query per-pod rows using existing pod_id column |
| Steam library scan | Hardcoded paths C/D/E | Parse libraryfolders.vdf first, fall back to hardcoded |
| Boot-time combo validation | Trigger as soon as manifest scan completes | Gate on: manifest complete AND presets received from server |
| Launch timeline events via WS | Fire-and-forget new WS message type | Resend on reconnect; for commands needing confirmation, use ACK pattern |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Full filesystem scan on every WS reconnect | Pod WS reconnects cause 2-5s freeze (scanning thousands of AC car folders) | Cache manifest in memory; rescan only on explicit trigger or 24h timer | Any pod with large AC install (500+ cars) |
| Reliability query per-preset in a loop (N+1) | `/api/v1/presets` response time grows linearly with preset count | Batch reliability query: one JOIN across all presets, not one query per preset | >50 presets |
| Launch timeline events logged to SQLite per event | High-frequency launch events fill launch_events table | Keep `launch_events` schema but use JSONL dual-write for high-frequency timeline data; SQLite for aggregation | >100 launches/day |
| Fleet game matrix loading all pod manifests | Admin page loads all 8 manifests on every render | Cache fleet matrix in server memory, invalidate on ContentManifest receive | N/A at 8 pods, but bad pattern |
| Re-validating all combos on every content scan | Boot takes 30+ seconds validating thousands of AC combos | Validate only combos that changed (compare manifest diff, not full rescan) | AC installs with 200+ presets |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Exposing full content manifest via public API | Information disclosure: reveals pod hardware and software configuration to customers on venue WiFi | Game availability endpoint returns only `available: bool` per game, not full manifest. Full manifest stays internal. |
| Allowing kiosk to request arbitrary game inventory scans | DoS: customer-triggered filesystem scan on every kiosk interaction | Inventory scan is triggered server-side only (agent push); kiosk only reads cached availability |
| Storing car/track names from manifest into preset validation without sanitization | INI injection: a malicious mod folder name `ferrari\n[RACE]` could corrupt race.ini | Whitelist validation (standing rule: INI injection whitelist for car/track names) — already exists in v27.0 |
| Unguarded `/api/v1/games/fleet-matrix` endpoint returning pod details | Information disclosure: exposes per-pod capability to unauthorized callers | Route behind staff JWT (not public) — pod capability data is operational, not customer-facing |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Showing game as "unavailable" when inventory scan hasn't completed yet | Customer sees empty game list on first boot, assumes nothing is installed | Show "Checking availability..." spinner for up to 30s on boot; fall back to all-games-visible if scan times out |
| Showing reliability score as % to customers | Customers avoid a game with 70% reliability that is perfectly fine today | Reliability score is staff/admin only; kiosk only shows available/unavailable |
| Kiosk flickering when pod availability changes mid-session | Customer sees game list change while browsing | Debounce kiosk availability updates: apply new manifest only between sessions, not during active browsing |
| Displaying "Game unavailable on this pod" with no alternative | Customer walk-away | Show alternative available pod: "Available on Pod 5 — ask staff" |
| Crash loop alert to Uday with no actionable context | Uday calls staff who don't know which pod or what to do | Alert format: "Pod 3 crash loop — 5 restarts in 3 min. Last error: acs.exe segfault. Recommend reboot." |

---

## "Looks Done But Isn't" Checklist

- [ ] **Content scanner extended:** verify by SSH to a pod and checking `GET /api/v1/fleet/manifest/pod-3` returns Steam games, not just AC content.
- [ ] **Kiosk game filter:** verify on the ACTUAL kiosk browser (not curl) — open `/kiosk` on Pod 3 and confirm Forza is absent if not installed. `curl` proves the API; the kiosk proves the UI.
- [ ] **Combo validation at boot:** verify validation log appears AFTER "Preset push received" log, not before. Check timing of log entries on Pod 8.
- [ ] **Crash loop WhatsApp alert:** trigger a crash loop on Pod 8 (3 fast restarts) and verify Uday receives a WhatsApp message with pod number and context. `fleet_health.crash_loop: true` is a proxy; the alert arriving is the behavior.
- [ ] **Reliability dashboard:** open admin dashboard in browser from James's machine (NOT from server itself) and verify per-pod scores render. Static file serving issues are only visible from a remote browser.
- [ ] **DiagnosticTrigger new variants:** deploy, then read back KB entries from SQLite and verify they deserialize without error. `cargo test` proves struct, not DB roundtrip.
- [ ] **Steam VDF parsing:** install a game to a non-default Steam library path on Pod 8 and verify it appears in the inventory. Hardcoded path check is the trap.
- [ ] **WhatsApp chain failure path:** disconnect Bono VPS relay (`CTRL+C comms-link`) and trigger a Tier 5 escalation. Verify alert is queued and delivered when relay reconnects. Alert during relay-down is the failure mode.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Serde field mismatch (kiosk sent wrong field name) | LOW — kiosk redeploy only | Update field name in kiosk `buildLaunchArgs()`, rebuild and deploy kiosk Next.js app. No Rust rebuild needed. |
| Stale inventory after server restart | LOW — automatic | Pod re-sends ContentManifest on next WS reconnect. If periodic rescan is implemented, self-heals within 24h. Manual: fleet exec `RequestContentScan` via server. |
| Crash loop detector conflict (double alerts) | LOW | Disable new detector, keep fleet_health.rs as single source. One-line config change. |
| DiagnosticTrigger serde failure on KB entries | MEDIUM | Write a one-time migration script: read all KB entries, skip those that fail deserialization with a WARNING, write back. Or clear KB entirely if entries are few. |
| WhatsApp alert going to wrong path (direct vs relay) | LOW | Change rc-agent to emit EscalationRequest instead of direct HTTP call. Rust rebuild + fleet deploy. |
| Boot combo validation auto-disabled correct combos | MEDIUM | Re-enable combos via admin preset library UI. Add boot sequencing gate to prevent recurrence. |
| Steam library scan missed custom path | LOW | Add VDF parsing. Rebuild rc-agent, deploy to fleet. Manual: admin re-enables affected presets. |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Serde silently drops new fields | Phase adding WS protocol extension (ContentManifest + InstalledGame) | `cargo test -p rc-common` roundtrip tests + on-pod file readback |
| `ok: true` means queued not delivered | Phase adding launch timeline WS events | Drop-WS-mid-operation test; verify resend on reconnect |
| Single-fetch-at-boot, no retry | Phase 1: content scanner extension | Restart server while pods are running; verify game list repopulates within 24h without pod restart |
| Steam library hardcoded paths | Phase 1: content scanner extension | Install game to D: drive on Pod 8; verify scanner finds it |
| Boot combo validation fires before presets loaded | Phase adding proactive combo validation | Check log ordering: "Presets received" before "Combo validation complete" |
| DiagnosticTrigger enum serde compat | Phase adding CrashLoop/GameLaunchTimeout variants | DB roundtrip test: write old variant to SQLite, upgrade, read back — must not error |
| Crash loop detection duplication | Phase wiring crash loop to Meshed Intelligence | grep for duplicate detection logic; single FleetEvent emission path |
| WhatsApp via wrong path | Phase adding crash loop/chain failure alerts | Bono VPS relay-down test; verify alert queued not lost |
| Fleet-average reliability, not per-pod | Phase adding kiosk game filtering | Open kiosk on Pod 3; verify game absent if not installed on Pod 3 specifically |
| ContentManifest type cascade | Phase 1: content scanner extension (type extension) | `cargo build` after type change; fix ALL compiler errors before proceeding |

---

## Sources

- `crates/rc-agent/src/content_scanner.rs` — AC-only scanner, no Steam or non-Steam support
- `crates/rc-agent/src/steam_checks.rs` — hardcoded Steam library paths, acknowledged VDF gap (line 301-310)
- `crates/rc-agent/src/diagnostic_engine.rs` — DiagnosticTrigger enum, existing trigger set
- `crates/rc-agent/src/tier_engine.rs` — WhatsApp escalation via EscalationRequest (line 2525-2590), existing WS escalation path
- `crates/racecontrol/src/game_launcher.rs` — ACK protocol (line 440-467), fire-and-forget history
- `crates/racecontrol/src/fleet_health.rs` — existing crash_loop detection (lines 379-390, 531-538)
- `crates/racecontrol/src/preset_library.rs` — fleet-aggregate reliability SQL (no pod_id filter)
- `crates/racecontrol/src/ws/mod.rs` — ContentManifest handler (line 920-928), single insert on receive
- `crates/racecontrol/src/state.rs` — pod_manifests: HashMap (line 189)
- `crates/rc-common/src/protocol.rs` — EscalationRequest (line 135), ContentManifest (line 147)
- CLAUDE.md standing rules: serde field mismatch (ai_difficulty/ai_level incident), ok:true desync (GameTracker stuck), process guard empty allowlist (single-fetch-at-boot), Session 0 launch failure, WhatsApp routing (Bono VPS not venue), cascade updates (recursive), DiagnosticTrigger serde risk (enum in DB), boot sequencing (MAINTENANCE_MODE fires before inputs ready)
- MEMORY.md: v40.0 WS ACK Protocol, v41.0 constraints, known pitfalls list (serde, ok:true, Session 0, process guard, single-fetch, manual fix regression)

---
*Pitfalls research for: Game Intelligence System (v41.0) — game inventory, combo validation, launch telemetry, reliability scoring*
*Researched: 2026-04-03*
