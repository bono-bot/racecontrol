# Phase 67: Config Sync - Research

**Researched:** 2026-03-20 IST
**Domain:** Config file change detection, sanitization, and push via comms-link sync_push
**Confidence:** HIGH — all findings from direct source inspection, no guesswork

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SYNC-01 | racecontrol.toml changes detected via sha2 hash and pushed to Bono via comms-link sync_push | Hash mechanism and sync_push path fully documented; `sha2` not yet in workspace (needs adding); comms-link relay `/relay/sync` endpoint already exists |
| SYNC-02 | Config payload is sanitized (credentials/local paths stripped) before push | Credential fields catalogued from config.rs; sanitization is a JavaScript transform on james/index.js side; exact fields to strip documented |
| SYNC-03 | Bono applies received config to cloud racecontrol (pod definitions, billing rates, game catalog) | Bono's sync_push handler already forwards to rc-core via `httpPost`; cloud racecontrol needs a `/api/v1/config/apply` or similar endpoint to receive and apply config; existing `/sync/push` is for laps/drivers — not config |
</phase_requirements>

---

## Summary

Phase 67 adds a new config synchronization channel: when `racecontrol.toml` on server .23 changes, a sanitized snapshot of the venue-specific config sections is pushed to Bono's cloud racecontrol so failover has an up-to-date config.

The key insight is that there are **two distinct mechanisms** to implement:

1. **James side (Node.js in comms-link):** A file watcher on `C:\RacingPoint\racecontrol.toml` that computes a SHA-256 hash (no `sha2` crate needed — Node.js has `crypto.createHash`), diffs on change, sanitizes the payload (strips credentials and Windows paths), and sends a `sync_push` message via the existing comms-link WebSocket.

2. **Bono/cloud side:** The `sync_push` message arriving at Bono is already forwarded to cloud racecontrol via `httpPost` to `RC_CORE_URL/sync/push`. However, the existing `/sync/push` endpoint handles only laps/drivers/track_records. A new config payload key (`config_snapshot`) must be recognized by cloud racecontrol's `/sync/push` handler to apply the venue config sections.

**Primary recommendation:** All hash/watch/sanitize logic goes in the James side (comms-link, Node.js). No new Rust crates needed for hashing — use Node.js `crypto`. The Rust side needs only a small addition to `sync_push` to handle a `config_snapshot` key.

---

## Standard Stack

### Core (already in place — no new dependencies)

| Library/Tool | Version | Purpose | Status |
|---|---|---|---|
| Node.js `crypto.createHash` | built-in | SHA-256 hash of racecontrol.toml | Already used in logbook-watcher.js (same pattern) |
| Node.js `fs/promises.readFile` + `fs.watch` | built-in | File watching and reading | Already used in logbook-watcher.js |
| comms-link `sync_push` MessageType | v2.0 (shipping) | Push config delta to Bono | Already in protocol.js; james/index.js already POSTs sync_push via `/relay/sync` |
| comms-link `/relay/sync` HTTP endpoint | James relay server | Venue racecontrol triggers sync | Already implemented in james/index.js (line 422-426) |
| `toml` npm package | Not yet installed | Parse racecontrol.toml on James side | Need to check if already in comms-link package.json |
| Rust `toml` crate | 0.8 (workspace) | Parse toml on cloud side | Already in workspace.dependencies |

### No New Rust Crates Needed

The sha2 crate mentioned in prior research notes is for Rust-side hashing. This phase does NOT need Rust-side hashing — the file watcher runs in Node.js (James side). `crypto.createHash('sha256')` is identical to sha2 in behavior.

### Config Sections to Sync (SAFE — no credentials)

From `config.rs` analysis, these sections are safe to include in the sync payload:

| Section | Fields to Sync | Why Safe |
|---------|---------------|----------|
| `[venue]` | `name`, `location`, `timezone` | Metadata only |
| `[pods]` | `count`, `discovery`, `healer_enabled`, `healer_interval_secs`, `static[]` | Pod topology |
| `[branding]` | `primary_color`, `theme` | UI config |
| `[integrations.whatsapp]` | `enabled` (NOT `contact`) | Feature flag only |
| Billing rates (see note) | `rate_30min`, `rate_60min`, `trial_secs` | These are NOT in racecontrol.toml currently |

**Critical finding on billing rates:** Looking at `racecontrol.toml` and `config.rs`, billing rates (₹700/30min, ₹900/60min) are NOT stored in `racecontrol.toml` — they are hardcoded in `billing_guard.rs` or stored in the database. The requirements say "billing rates" but the config file does not contain them. The planner must decide how to handle this: either (a) scope SYNC-03 to pod definitions only for now, or (b) add a `[billing]` section to racecontrol.toml in this phase.

**Recommendation:** Scope the sync to what IS in racecontrol.toml (`[venue]`, `[pods]`, `[branding]`). Document billing rates as a follow-on concern.

### Fields to STRIP (NEVER send to cloud)

From `racecontrol.toml` inspection, strip all of these:

| Field | Reason |
|-------|--------|
| `[auth].jwt_secret` | Signing key — would allow forging tokens |
| `[auth].evolution_api_key` | WhatsApp API credential |
| `[auth].evolution_url` | Internal service URL |
| `[auth].evolution_instance` | Internal config |
| `[cloud].terminal_secret` | Auth shared secret |
| `[cloud].terminal_pin` | Admin PIN |
| `[cloud].api_url` | Venue-specific URL (not needed by cloud) |
| `[cloud].comms_link_url` | Venue-internal localhost URL |
| `[bono].relay_secret` | Shared secret |
| `[bono].webhook_url` | Tailscale-specific URL |
| `[bono].tailscale_bind_ip` | Venue-specific IP |
| `[gmail].*` | OAuth credentials |
| `[database].path` | Local Windows path |
| `[server].host` / `[server].port` | Local binding — cloud has its own |
| `[ac_server].acserver_path` | Local Windows path (`C:\Program Files\...`) |
| `[ac_server].data_dir` | Local path |
| `[bono].enabled` | Venue relay flag, not relevant to cloud |

### Game Catalog

The requirements mention "game catalog" in SYNC-03. There is no `[games]` section in `racecontrol.toml` and no game catalog struct in `config.rs`. The game list is likely stored in the database or hardcoded. **The planner should note this as an open question** — either game catalog is out of scope for this phase, or a `[games]` section needs to be added to toml.

---

## Architecture Patterns

### Pattern 1: File Watcher on James Side (Node.js)

The existing `logbook-watcher.js` already implements exactly the pattern needed: poll-based file watching with SHA-256 hash comparison and event emission on change.

**Key reference:** `C:/Users/bono/racingpoint/comms-link/james/logbook-watcher.js` — implements `LogbookWatcher` with:
- `fs.watch` + polling fallback
- `createHash('sha256').update(content).digest('hex')` for change detection
- Event emission (`changed`, `error`) when hash differs

**Recommended approach:** Create a new `ConfigWatcher` class (or reuse `LogbookWatcher` with different config) that:
1. Reads `racecontrol.toml` on an interval (poll, 30s)
2. Computes SHA-256 of file contents
3. On hash change: parse TOML, apply sanitization, send `sync_push` via `/relay/sync`

**Why poll not `fs.watch`?** `fs.watch` on Windows is unreliable for files on NTFS (missed events, spurious events). The logbook-watcher uses poll as the reliable path. Same here.

### Pattern 2: Relay via Existing `/relay/sync` Endpoint

James's relay server already has a `POST /relay/sync` route (james/index.js line 422-426) that:
1. Accepts a JSON payload
2. Calls `client.send('sync_push', payload)`
3. Returns `{ ok: true, sent: bool }`

This is how racecontrol (cloud_sync.rs line 220) already sends lap/driver data. Config sync uses the same path — James watches the file and POSTs to `/relay/sync` directly.

**No new relay route needed.** The config watcher just calls `httpPost('http://localhost:8766/relay/sync', sanitizedPayload)` (note: James relay is on port 8766, not 8765 which is Bono's WS port).

**Wait — port clarification:** From james/index.js:
- `wsUrl = process.env.COMMS_URL || 'ws://localhost:8765'` — connects TO Bono
- `relayPort = parseInt(process.env.RELAY_PORT, 10) || 8766` — James's LOCAL HTTP relay

So the config watcher calls `http://localhost:8766/relay/sync` (James's own relay) to inject a sync_push into the comms-link WebSocket.

### Pattern 3: Bono Forwards sync_push to Cloud RC

Already implemented in `bono/index.js` (line 221-231):
```javascript
if (msg.type === 'sync_push') {
  httpPost(`${rcCoreUrl}/sync/push`, JSON.stringify(msg.payload), {
    'x-terminal-secret': terminalSecret,
  }).then(...)
}
```

This is unchanged. The config snapshot arrives in `msg.payload` alongside any lap/driver data (or by itself — `sync_push` accepts partial payloads).

### Pattern 4: Cloud racecontrol Handles Config Key in sync_push

The existing `sync_push` handler in `routes.rs` (line 7077) processes `body.get("laps")`, `body.get("track_records")`, etc. Adding config sync requires adding a `body.get("config_snapshot")` branch that applies venue config to the cloud racecontrol's in-memory state.

**Design decision for planner:** The cloud racecontrol does not need to rewrite its own TOML file — it just needs to update runtime values. Three options:
- Option A: Expose a `POST /api/v1/config/apply` endpoint (separate from `/sync/push`)
- Option B: Add `config_snapshot` handling inside the existing `/sync/push` handler
- Option C: Use the existing `/sync/push` handler but update an `AppState` field (e.g., `venue_config_override: Arc<Mutex<Option<VenueConfigSnapshot>>>`)

**Recommended:** Option B — add `config_snapshot` to the existing `/sync/push` handler. Least surface area, consistent with how laps/drivers work. The handler reads `body.get("config_snapshot")` and updates `AppState` fields that the venue sync endpoint exposes.

### Recommended Project Structure Changes

```
comms-link/
├── james/
│   ├── config-watcher.js     # NEW: watches racecontrol.toml, emits changes
│   ├── config-sanitizer.js   # NEW: strips credentials/paths from toml payload
│   └── index.js              # MODIFY: instantiate ConfigWatcher + wire to relay
└── shared/
    └── protocol.js           # NO CHANGE: sync_push already exists

crates/racecontrol/src/
├── api/routes.rs             # MODIFY: add config_snapshot branch in sync_push handler
└── config.rs                 # NO CHANGE (only Rust structs, not modified)
```

### Anti-Patterns to Avoid

- **Do NOT use `fs.watch` as the primary watcher on Windows** — unreliable. Use poll with setInterval.
- **Do NOT send the raw TOML string** — parse it first (TOML → JSON), then sanitize as JSON before pushing.
- **Do NOT add sha2 crate** — not needed; Node.js `crypto` handles this.
- **Do NOT modify `racecontrol.toml` itself** — this phase is read-only on the venue config file.
- **Do NOT use the existing cloud sync timer** (the 30s cloud_sync.rs loop) — config sync is triggered by file change, not by a time interval.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| File change detection | Custom inotify/FSEvents wrapper | Poll with `setInterval` + SHA-256 compare | Same pattern as logbook-watcher.js; `fs.watch` is unreliable on Windows NTFS |
| TOML parsing in Node.js | Custom TOML parser | `toml` npm package or built-in `JSON.parse` after racecontrol exposes config as JSON | A proper TOML parser handles edge cases (multiline strings, datetime, etc.) |
| Config snapshot delivery | New WebSocket message type | Existing `sync_push` MessageType + `/relay/sync` relay | Already wired end-to-end; adding a new message type creates unnecessary protocol complexity |
| Credential detection | Heuristic string matching | Explicit field-name allowlist (only allow known-safe fields) | Allowlist is safer than denylist — new credential fields added to toml don't accidentally leak |

---

## Common Pitfalls

### Pitfall 1: Billing Rates Not in racecontrol.toml

**What goes wrong:** SYNC-03 mentions "billing rates" but billing rates (`[billing]` section) do not exist in `racecontrol.toml`. The 30min/₹700, 60min/₹900 rates are not config-file-managed.

**Why it happens:** Requirements were written based on future intent; current toml doesn't have billing config.

**How to avoid:** Either (a) scope SYNC-03 to pod definitions only for this phase, and note billing rates as Phase 68 work, OR (b) add `[billing]` to toml in this phase as part of task 1. The planner should make this explicit — it's a non-trivial decision.

**Warning signs:** If you try to sync billing rates without first adding them to toml, the sync payload will be empty of the most important data.

### Pitfall 2: Game Catalog Not in racecontrol.toml

**What goes wrong:** Similar to billing rates — "game catalog" is not a config file concept in the current system.

**Why it happens:** Requirements reference future desired state.

**How to avoid:** Document as out of scope for Phase 67 unless explicitly adding a `[games]` section is part of this phase's scope.

### Pitfall 3: TOML Parse Failure on Partial/In-Progress Write

**What goes wrong:** racecontrol.toml is being written (by a human or script) when the watcher reads it. Partial TOML → parse failure → exception in config-watcher.js.

**Why it happens:** Non-atomic file writes on Windows.

**How to avoid:** Wrap TOML parse in try/catch. On parse failure, log warning and skip this cycle — the next poll will get the complete file. Do NOT crash or stop the watcher.

**Warning signs:** `TOML parse error` in James logs immediately followed by a successful parse 30s later.

### Pitfall 4: Sending sync_push When Nothing Changed

**What goes wrong:** Watcher polls every 30s and sends sync_push even when nothing changed, flooding comms-link.

**Why it happens:** Forgetting to compare hashes before sending.

**How to avoid:** Store `lastHash` in watcher state. Only send if `newHash !== lastHash`. The logbook-watcher pattern already does this correctly.

### Pitfall 5: Race Condition — Config Watcher Sends While Cloud RC Restarts

**What goes wrong:** Cloud racecontrol is in the middle of restarting (failover mode) when config_snapshot arrives in sync_push. The HTTP POST to `/sync/push` returns a non-200.

**Why it happens:** Failover window.

**How to avoid:** The existing sync_push bono-forwarding code already handles this with a simple error log (no retry). This is acceptable — the next config push (triggered by the next toml change or on reconnect) will carry the full current config.

### Pitfall 6: Credential Leak via Nested/Unexpected Fields

**What goes wrong:** Using a denylist (strip known secrets) rather than an allowlist (only include known-safe fields). A future `[auth].new_secret` would slip through the denylist.

**Why it happens:** Denylist feels natural but is always incomplete.

**How to avoid:** Use an allowlist in `config-sanitizer.js`. Build the payload by explicitly selecting fields:

```javascript
// SAFE: allowlist approach
const snapshot = {
  venue: { name: parsed.venue.name, location: parsed.venue.location, timezone: parsed.venue.timezone },
  pods: { count: parsed.pods?.count, discovery: parsed.pods?.discovery },
  branding: { primary_color: parsed.branding?.primary_color, theme: parsed.branding?.theme },
};
// Result contains ONLY the fields we explicitly listed — nothing else can leak
```

---

## Code Examples

Verified patterns from existing source:

### LogbookWatcher Hash Pattern (reference — same approach for ConfigWatcher)

```javascript
// Source: C:/Users/bono/racingpoint/comms-link/james/logbook-watcher.js (confirmed)
// SHA-256 of file contents for change detection
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';

async function computeHash(filePath) {
  const content = await readFile(filePath, 'utf8');
  return {
    hash: createHash('sha256').update(content).digest('hex'),
    content,
  };
}
```

### Relay/Sync POST (how cloud_sync.rs sends lap data — same path for config)

```javascript
// Source: james/index.js lines 422-426 (relay/sync endpoint)
// James relay server receives this POST and calls client.send('sync_push', payload)
const response = await fetch('http://localhost:8766/relay/sync', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    config_snapshot: {
      venue: { name: 'RacingPoint', location: '...', timezone: 'Asia/Kolkata' },
      pods: { count: 8, discovery: true },
      branding: { primary_color: '#E10600', theme: 'dark' },
      _meta: { pushed_at: Date.now(), hash: '...', source: 'james' },
    }
  }),
});
```

### Bono Forward to Cloud RC (already works — confirmed from bono/index.js line 221)

```javascript
// Source: C:/Users/bono/racingpoint/comms-link/bono/index.js line 221-231
// This code already runs when sync_push arrives. No changes needed to bono/index.js.
if (msg.type === 'sync_push') {
  httpPost(`${rcCoreUrl}/sync/push`, JSON.stringify(msg.payload), {
    'x-terminal-secret': terminalSecret,
  }).then((resp) => {
    if (resp.statusCode !== 200) {
      console.error(`[SYNC] rc-core rejected push: ${resp.statusCode} ${resp.body}`);
    }
  });
}
```

### sync_push Handler Extension (cloud racecontrol — routes.rs pattern to follow)

```rust
// Source: routes.rs line 7090-7120 pattern (add config_snapshot branch)
// Add after existing laps/track_records processing:
if let Some(config_snap) = body.get("config_snapshot") {
    // Apply venue config snapshot to AppState
    // Only update the fields we trust (pod count, branding, venue name)
    if let Some(pod_count) = config_snap.get("pods").and_then(|p| p.get("count")).and_then(|c| c.as_u64()) {
        // Update AppState or log — the exact update mechanism depends on AppState structure
        tracing::info!("Config sync: pod_count={}", pod_count);
    }
    total += 1;
}
```

### Config Sanitizer Allowlist Pattern (config-sanitizer.js)

```javascript
// Source: design decision — no existing source, but based on SYNC-02 requirement
// Use allowlist, NOT denylist
export function sanitizeConfig(parsed) {
  return {
    venue: {
      name: parsed.venue?.name ?? 'RacingPoint',
      location: parsed.venue?.location ?? '',
      timezone: parsed.venue?.timezone ?? 'Asia/Kolkata',
    },
    pods: {
      count: parsed.pods?.count ?? 8,
      discovery: parsed.pods?.discovery ?? true,
      healer_enabled: parsed.pods?.healer_enabled ?? true,
    },
    branding: {
      primary_color: parsed.branding?.primary_color ?? '#E10600',
      theme: parsed.branding?.theme ?? 'dark',
    },
    // _meta: tracking fields — not from config, added by watcher
    _meta: {
      source: 'james-config-watcher',
      pushed_at: Date.now(),
    },
  };
  // NOTHING ELSE — no auth, no cloud, no database, no bono relay secrets
}
```

### TOML Parsing in Node.js

The `toml` npm package is standard for Node.js TOML parsing. Check if it's already in comms-link's package.json:

```bash
# Check from comms-link dir
cat package.json | grep toml
# If not present: npm install toml
```

Alternative: Since racecontrol itself already parses the toml, another approach is to call `GET http://localhost:8080/api/v1/venue` or `GET http://localhost:8080/api/v1/config` which may already expose venue config as JSON — no TOML parsing needed in Node.js. **Check if such an endpoint exists** before adding the `toml` npm package.

---

## TOML npm Package Verification

```bash
# Verify package exists and is current:
npm view toml version
# Expected: 3.0.0 (stable since 2020 — LOW churn)
```

---

## State of the Art

| Old Approach | Current Approach | Notes |
|--------------|-----------------|-------|
| Push entire toml file | Push sanitized JSON snapshot | Only safe fields, structured |
| File watcher via `fs.watch` | Poll with SHA-256 comparison | Reliable on Windows NTFS |
| New message type for config | Reuse `sync_push` with `config_snapshot` key | Minimal protocol changes |

---

## Open Questions

1. **Billing rates location**
   - What we know: `racecontrol.toml` has no `[billing]` section. Rates (₹700/30min, ₹900/60min) are in CLAUDE.md but not in the toml file.
   - What's unclear: Are rates in the DB, hardcoded in `billing_guard.rs`, or somewhere else?
   - Recommendation: Planner should include a task to grep for rate constants before deciding scope. If rates are hardcoded, add `[billing]` to toml as part of this phase.

2. **Game catalog**
   - What we know: No `[games]` section in `racecontrol.toml`, no `GamesConfig` in `config.rs`.
   - What's unclear: Is game catalog a DB concept or a config concept?
   - Recommendation: Treat as out of scope for SYNC-03 for this phase. Document the gap. SYNC-03 can be satisfied with pod definitions alone.

3. **Cloud racecontrol AppState mutability**
   - What we know: AppState is `Arc<AppState>` with `Arc<Pool<Sqlite>>`. Interior mutability requires `Mutex` or `RwLock`.
   - What's unclear: Whether AppState has a mechanism for runtime config updates or if this requires a new field.
   - Recommendation: Add `venue_config_override: Arc<RwLock<Option<VenueConfigSnapshot>>>` to AppState in the new struct. Simple and thread-safe.

4. **Does a config-as-JSON endpoint already exist?**
   - What we know: `GET /api/v1/sync/status` returns config metadata (line 7540+). `GET /api/v1/fleet/health` returns pod status.
   - What's unclear: Whether any endpoint returns `[venue]` + `[pods]` as JSON (which would avoid needing TOML parsing in Node.js).
   - Recommendation: Search routes.rs for `/venue` or `/config` endpoints. If one exists, use it — simpler than adding TOML npm dep.

5. **60-second push requirement**
   - What we know: SYNC-01 requires sync_push within 60s of toml edit. A 30s poll interval means worst case is 30s (edit at second 0 of interval → detected at second 30 → pushed immediately).
   - What's unclear: Nothing — 30s poll satisfies the 60s SLA comfortably.
   - Recommendation: Use 30s poll interval.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Node.js built-in test runner (`node:test`) — matches existing comms-link tests |
| Config file | None (no jest/vitest config in comms-link) |
| Quick run command | `node --test test/config-watcher.test.js` |
| Full suite command | `node --test test/*.test.js` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| SYNC-01 | Config watcher detects toml hash change and sends sync_push within 60s | Manual smoke | Edit `C:\RacingPoint\racecontrol.toml`, observe James logs for `[CONFIG-SYNC] pushed config snapshot` within 60s | N/A — manual |
| SYNC-01 | Config watcher does NOT send when file unchanged | Unit | `node --test test/config-watcher.test.js` | ❌ Wave 0 |
| SYNC-02 | Sanitizer strips jwt_secret, terminal_secret, relay_secret, passwords | Unit | `node --test test/config-sanitizer.test.js` | ❌ Wave 0 |
| SYNC-02 | Sanitizer output contains no Windows paths (`C:\`) | Unit | same test file | ❌ Wave 0 |
| SYNC-03 | Bono forwards sync_push with config_snapshot to cloud rc | Manual smoke | Check Bono VPS logs for `[SYNC]` after triggering sync | N/A — manual |
| SYNC-03 | Cloud racecontrol logs received config_snapshot pod_count | Manual smoke | Check cloud rc logs after sync | N/A — manual |

### Sampling Rate
- **Per task commit:** `node --test test/config-watcher.test.js test/config-sanitizer.test.js`
- **Per wave merge:** `node --test test/*.test.js`
- **Phase gate:** Unit tests green + manual smoke tests pass

### Wave 0 Gaps
- [ ] `comms-link/test/config-watcher.test.js` — covers SYNC-01 (hash change detection, no-change guard, parse error recovery)
- [ ] `comms-link/test/config-sanitizer.test.js` — covers SYNC-02 (allowlist enforcement, credential field absence, no Windows paths)
- [ ] `toml` npm package if not already in package.json — `npm install toml` in comms-link dir (or confirm endpoint-based approach avoids this)

---

## Sources

### Primary (HIGH confidence)
- `C:/Users/bono/racingpoint/comms-link/shared/protocol.js` — MessageType.sync_push confirmed
- `C:/Users/bono/racingpoint/comms-link/james/index.js` lines 422-426 — `/relay/sync` POST endpoint confirmed; line 245-256 — how sync_push is forwarded to rc-core
- `C:/Users/bono/racingpoint/comms-link/bono/index.js` lines 221-231 — Bono already forwards sync_push to cloud rc via httpPost
- `C:/Users/bono/racingpoint/racecontrol/racecontrol.toml` — exact fields, confirmed which contain credentials
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/config.rs` — complete Config struct with all sections and fields
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/cloud_sync.rs` line 220 — confirms relay URL format `/relay/sync`
- `C:/Users/bono/racingpoint/racecontrol/crates/racecontrol/src/api/routes.rs` lines 7077-7090 — sync_push handler structure confirmed
- `C:/Users/bono/racingpoint/comms-link/james/logbook-watcher.js` — SHA-256 file hash pattern (identical to what ConfigWatcher needs)

### Secondary (MEDIUM confidence)
- `toml` npm package (https://www.npmjs.com/package/toml) — version 3.0.0, stable; training knowledge

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all confirmed from direct source inspection
- Architecture: HIGH — relay path end-to-end traced through source code
- Sanitization fields: HIGH — config.rs read directly, all fields enumerated
- Billing/game catalog gap: HIGH confidence that the gap EXISTS, LOW confidence on resolution approach
- Cloud-side AppState mutability: MEDIUM — AppState structure not fully read, but `Arc<RwLock<T>>` is standard Axum pattern

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (comms-link v2.0 API stable; config.rs structure stable)
