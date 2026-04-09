# 362-01 SUMMARY — Post-Launch Config Verification (Layer 3)

**Phase:** 362 — Post-Launch Config Verification
**Plan:** 362-01 (retroactive — no PLAN.md was written pre-execution)
**Status:** SHIPPED
**Build:** `a9b5eaa3`
**Shipped:** 2026-04-09
**Target:** All 8 pods (Pod 1-8)
**Canary:** Pod 8 — visually confirmed by user on-site
**Requirements closed:** GLD-B-01, GLD-B-02, GLD-B-03, GLD-B-04, GLD-B-05

## What shipped

### SessionConfig struct + per-sim shared-memory readers (GLD-B-01)

Added a common `SessionConfig` struct in `crates/rc-common/src/protocol.rs` carrying: `ai_count`, `session_type`, `car_id`, `track_id`, `track_config`, `fuel_remaining`, and adapter-specific fields.

Implemented `read_session_config()` on all 5 sim adapters:

| Adapter | File | Source | Key offsets / fields |
|---------|------|--------|----------------------|
| Assetto Corsa | `crates/rc-agent/src/sims/assetto_corsa.rs` | Static shared memory + RC plugin | AI count @ offset 64 (i32), session type @ offset 20, track config @ offset 168 (wchar[33]), fuel @ offset 336 |
| Assetto Corsa Evo | `crates/rc-agent/src/sims/assetto_corsa_evo.rs` | AC Evo shared memory (compat layout) | Same layout as AC with adapter-specific offsets |
| F1 25 | `crates/rc-agent/src/sims/f1_25.rs` | UDP Packet 4 (participant list) + Session packet | Participant count = non-empty entries in packet, game_mode @ offset 181, grid pos in lap data @ offset 32 |
| iRacing | `crates/rc-agent/src/sims/iracing.rs` | Shared memory variable lookup | `CarsCount` by name |
| LMU | `crates/rc-agent/src/sims/lmu.rs` | LMU shared memory | Adapter-specific layout (built, runtime verification deferred to GLD-G-05) |

### verify_launch_config() Stage 5 (GLD-B-02)

Added a new Stage 5 `verify_launch_config()` in `crates/rc-agent/src/launch_verifier.rs`. Pipeline now runs:

1. Process alive
2. Main window present
3. Game state = InRace / Practice / Hotlap
4. Telemetry feed active
5. **(NEW)** SessionConfig matches AcLaunchParams

Stage 5 compares requested vs actual with:

- Session type: normalized enum comparison with semantic equivalence table (kiosk "trackday" ↔ SHM "practice" mapped explicitly)
- Car/track: fuzzy match on normalized folder-name vs display-name (fixes AC folder-name vs kiosk display-name mismatch)
- AI count: exact match tolerance ±1 (to absorb AC's one-based-vs-zero-based counter drift)

### ConfigMismatchDetected WS + server handler + admin broadcast (GLD-B-03)

New WS message in `crates/rc-common/src/protocol.rs`:

```rust
ConfigMismatchDetected {
    pod_id: u32,
    session_id: String,
    requested: SessionConfig,
    actual: SessionConfig,
    mismatched_fields: Vec<String>,
    severity: MismatchSeverity,  // Warning | Critical
}
```

Server handler in `crates/racecontrol/src/ws/mod.rs`:

1. Logs the mismatch at WARN with structured fields
2. Broadcasts to admin dashboard over SSE channel (`/admin/events` subscribers)
3. Fires WhatsApp alert via comms-link relay (`POST /relay/exec/run` with `send_whatsapp_alert` command and a templated message)
4. Persists to `config_mismatches` table for audit trail

### Atomic race.ini write + readback + AI car content validation (GLD-B-04)

In `crates/rc-agent/src/ac_launcher.rs`:

- Race.ini write changed from direct `File::create().write_all()` to a temp-file-then-rename atomic write (`write_atomic` helper).
- After write, reads back the file and parses it to verify the on-disk content matches what was intended (catches quota/permission/partial-write edge cases).
- AI car content validation: before launch, checks that every AI car slot in `AcLaunchParams.ai_cars` corresponds to a car folder that exists on the pod (prevents the class of bug where kiosk sends an AI car the pod doesn't have installed).

### Session type + car/track normalization + 1s sleep removal (GLD-B-05)

- Removed 1 second of cumulative sleep from the launch path (were added as stability workarounds but Phase 362's direct readback made them unnecessary).
- Session type normalization: added `SessionType::normalize()` that maps the kiosk's `trackday|endurance|quickrace|practice|hotlap` enum to the per-sim canonical enum.
- Car/track fuzzy matcher: lowercase + strip whitespace + `_/- ` equivalence + common suffix strip (`_dlc`, `_gt3`).

## Files changed

| File | Change |
|------|--------|
| `crates/rc-common/src/protocol.rs` | Added `SessionConfig`, `ConfigMismatchDetected`, `MismatchSeverity` |
| `crates/rc-agent/src/sims/mod.rs` | SimAdapter trait gained `read_session_config()` method |
| `crates/rc-agent/src/sims/assetto_corsa.rs` | AC shared-memory offset reader |
| `crates/rc-agent/src/sims/assetto_corsa_evo.rs` | AC Evo adapter (built, runtime verification deferred) |
| `crates/rc-agent/src/sims/f1_25.rs` | F1 25 UDP Packet 4 participant parser |
| `crates/rc-agent/src/sims/iracing.rs` | iRacing `CarsCount` by-name reader |
| `crates/rc-agent/src/sims/lmu.rs` | LMU adapter (built, runtime verification deferred) |
| `crates/rc-agent/src/launch_verifier.rs` | Stage 5 `verify_launch_config()` |
| `crates/rc-agent/src/event_loop.rs` | ConfigMismatchDetected event wiring |
| `crates/rc-agent/src/ac_launcher.rs` | Atomic write, readback, AI car validation |
| `crates/racecontrol/src/ws/mod.rs` | Server handler: log, broadcast, WhatsApp, persist |

## Evidence (H3 compliant)

**Behavior tested (on Pod 8 canary, 2026-04-09):**
- Compiled release build with `GIT_HASH=a9b5eaa3` — verified via `curl http://192.168.31.91:8090/health | jq .build_id`
- `read_session_config()` on AC returned non-zero AI count after live launch of a practice session with 5 AI cars on Spa — log showed `session_config={ai_count: 5, session_type: Practice, car_id: "bmw_m4_gt4", track_id: "spa", ...}`
- Verify Stage 5 passed for a matching launch (`ConfigVerified` log line present)
- Kiosk "trackday" session_type → SHM "practice" — normalization verified via AC Pod 8 with SessionType::Trackday request matching SessionType::Practice from SHM read
- Car/track fuzzy match: kiosk display name "BMW M4 GT4" matched AC folder name `bmw_m4_gt4` after normalization
- Atomic race.ini write: inspected by hand on Pod 8, `race.ini.tmp.XXXX` files gone post-launch, final `race.ini` byte-identical to intended content
- Binary sha256: `4fa26c40d82d5e36bdcc3f5b22c396adc77079990c875d15da26a06d269e823b` (26.4 MB) uniform across all 8 pods

**Where tested:** Build on James `.27`, staged via deploy-staging HTTP server, downloaded to Pod 1-8, canary visual verification on Pod 8 by user on-site. Fleet-wide deployment verified via `/api/v1/fleet/health` build_id match across all 8 pods.

**Not tested (tracked as GLD-G-05 in Phase 367):**
1. **Deliberate mismatch → WhatsApp alert E2E** — no synthetic test has been run where a wrong AI count is injected and the full chain (rc-agent → server → comms-link → WhatsApp → staff phone) is verified with a real alert received. The code path is compiled and unit-tested but never fired end-to-end in anger.
2. **Assetto Corsa Evo runtime verification** — adapter is built and compiles but no live AC Evo launch has exercised the shared-memory reader since ACR is rarely used.
3. **LMU runtime verification** — same as ACR, built but no live LMU launch has exercised the reader.
4. **8-pod concurrent-mismatch load test** — no scenario has forced mismatches on multiple pods simultaneously to verify the server handler doesn't drop events under concurrent pressure.

These gaps are intentionally surfaced as a single Phase 367 deliverable (`GLD-G-05`) so they cannot silently fall off the backlog.

## Deploy manifest

| Artifact | Target | Method | Verification |
|----------|--------|--------|--------------|
| `rc-agent.exe` (hash `a9b5eaa3`, 26.4 MB) | Pods 1-8 | Staged HTTP download → bat-swap on restart | `build_id` match on `/health` endpoint all 8 pods |
| Cloud parity | Bono VPS | — | N/A — rc-agent runs on pods only, no cloud binary |
| Frontend | Admin dashboard | — | Handled separately (ConfigMismatchDetected consumer lives in admin /events SSE) |
| Database migration | Server | Automatic (idempotent) | `config_mismatches` table created at startup |

## Permanence gate

- ✅ Source code in git (`a9b5eaa3` tagged in `racingpoint/racecontrol` main branch)
- ✅ Binary SHA256 recorded and uniform fleet-wide
- ✅ Atomic file writes survive reboot
- ✅ No manual server edits required
- ✅ Deploy script (`deploy-staging/rc-agent fleet push`) is the permanent path

## Cascade updates

- ✅ `crates/rc-common` types used by rc-agent AND racecontrol (single source of truth)
- ✅ Admin dashboard consumes `ConfigMismatchDetected` via `/admin/events` SSE
- ⚠️  OpenAPI spec NOT updated to include the new WS message type (deferred to GLD-G-05)
- ⚠️  Contract tests NOT added for the new WS message (deferred to GLD-G-05)
- ⚠️  `shared-types` TS package NOT updated (deferred to GLD-G-05)

## Plan checkbox sync

- [x] 362-01-PLAN — SessionConfig + read_session_config on 5 adapters (shipped in `a9b5eaa3`)
- [x] 362-02-PLAN — verify_launch_config Stage 5 + ConfigMismatchDetected WS + admin broadcast (shipped in `a9b5eaa3`)
- [x] 362-03-PLAN — Atomic race.ini write + AI car content validation (shipped in `a9b5eaa3`)

Requirements closed: GLD-B-01, GLD-B-02, GLD-B-03, GLD-B-04, GLD-B-05.

## Commit reference

Built from git hash `a9b5eaa3`. This phase summary was written 2026-04-09 ~20:45 IST retroactively after the milestone was opened and STATE.md was swapped from v47.0 to v46.0.
