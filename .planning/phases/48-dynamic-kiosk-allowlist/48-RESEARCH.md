# Phase 48: Dynamic Kiosk Allowlist - Research

**Researched:** 2026-03-19
**Domain:** Rust/Axum API + SQLite schema + Next.js admin UI + local Ollama LLM classification
**Confidence:** HIGH

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ALLOW-01 | GET /api/v1/config/kiosk-allowlist returns merged allowlist (hardcoded + DB additions) | sqlx pattern from billing_rates; route follows /kiosk/* prefix convention |
| ALLOW-02 | Admin panel has Kiosk Allowlist section for staff add/remove | settings/page.tsx is the correct insertion point; uses same api.* pattern |
| ALLOW-03 | rc-agent picks up new process within 5 minutes without restart or redeploy | HTTP poll loop in AgentConfig + KioskConfig is the right layer; kiosk.rs allowed_set_snapshot() needs server-fetched set injection |
| ALLOW-04 | Unknown process triggers local LLM classification (ALLOW/BLOCK/ASK) before kill | query_ollama() pattern in ai_debugger.rs is the template; must integrate into enforce_process_whitelist_blocking() |
| ALLOW-05 | No false lockdowns when a Windows system process runs on any pod | Hardcoded ALLOWED_PROCESSES baseline is never removed; server-fetched list is additive only |
</phase_requirements>

---

## Summary

Phase 48 eliminates the #1 manual intervention at Racing Point: every new Windows service, driver, or tool triggers a false kiosk lockdown requiring a code change, rebuild, and redeploy to all 8 pods. The 70+ hardcoded entries in `kiosk.rs` have been patched at least 3 times in the past two weeks (commits c53cd03, b9245a6, 61e7b0b) with ~45 missing processes added reactively.

The solution is three-layered: (1) a SQLite-backed API on the server that staff manage via the admin panel, (2) an rc-agent HTTP poll every 5 minutes that fetches and merges the server list with the hardcoded baseline, and (3) a local LLM (rp-debug / qwen3:0.6b, already installed on all 8 pods by Phase 47) that classifies genuinely unknown processes as ALLOW/BLOCK/ASK before any action is taken. The hardcoded baseline is never removed — it is the safety net; the server list is purely additive.

The existing infrastructure makes this tractable: the `kiosk_settings` key-value table and `billing_rates` CRUD pattern show exactly how to add a new named table; the `CoreToAgentMessage::ApproveProcess` and `AgentMessage::ProcessApprovalRequest` WebSocket messages are already wired in `protocol.rs`; and `query_ollama()` in `ai_debugger.rs` is the exact HTTP call pattern to reuse for LLM classification. The admin panel (`kiosk/src/app/settings/page.tsx`) is the correct home for the new Allowlist section.

**Primary recommendation:** Add a `kiosk_allowlist` table to SQLite with a matching CRUD API, have rc-agent poll `GET /api/v1/config/kiosk-allowlist` on startup and every 5 minutes, inject the result into `allowed_set_snapshot()` via a `OnceLock<Mutex<HashSet>>`, and wire LLM classification into `enforce_process_whitelist_blocking()` before any temp-allow/reject action.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| sqlx | Already in Cargo.toml (racecontrol) | Async SQLite CRUD | All existing tables use this; migrate() pattern in db/mod.rs |
| axum | Already in Cargo.toml | HTTP route handlers | All API routes use axum extractors |
| reqwest | Already in AppState.http_client | rc-agent HTTP fetch | OnceLock client already used for Ollama in ai_debugger.rs |
| serde_json | Already in both crates | JSON serialization | Used throughout; `json!()` macro for API responses |
| tokio | Already runtime | Async tasks, intervals | `tokio::time::interval` for 5-minute poll |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| OnceLock<Mutex<HashSet>> | std | Server-fetched allowlist in rc-agent | Static global matching existing `learned_allowlist()` pattern in kiosk.rs |
| tracing | Already in both crates | Logging fetch/classify events | All modules use this; keep consistent |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| HTTP poll (5 min) | WebSocket push (CoreToAgentMessage) | Push is lower latency but more complex; poll matches the phase goal of "within 5 minutes" with zero new protocol messages |
| kiosk_allowlist table | kiosk_settings key-value (comma-separated) | Separate table enables proper CRUD, individual timestamps, and clean DELETE by process name |
| LLM inline in enforce loop | Separate async task | Enforce loop runs in spawn_blocking; LLM call is async HTTP; must be deferred via mpsc or tokio::spawn from caller |

**Installation:** No new dependencies needed. All required libraries are already present in both crates.

---

## Architecture Patterns

### Recommended Project Structure

```
crates/racecontrol/src/
├── db/mod.rs                    # +kiosk_allowlist table migration (additive ALTER IF NOT EXISTS pattern)
├── api/routes.rs                # +3 route handlers: GET/POST/DELETE /config/kiosk-allowlist
└── (no new files needed)

crates/rc-agent/src/
├── kiosk.rs                     # +server_allowlist() OnceLock + refresh_from_server() + LLM classifier call
└── main.rs                      # +allowlist_poll_loop() task + wire into KioskConfig

kiosk/src/app/settings/
└── page.tsx                     # +Allowlist section (list + add input + delete button)

tests/e2e/api/
└── kiosk-allowlist.sh           # CRUD test + 5-min pickup verification
tests/e2e/browser/
└── allowlist.spec.ts            # Playwright admin panel test
```

### Pattern 1: DB Table Migration (additive, idempotent)

**What:** Add `kiosk_allowlist` table in the existing `migrate()` function using `CREATE TABLE IF NOT EXISTS`.
**When to use:** Any new persistent data. Follow existing billing_rates pattern exactly.

```rust
// Source: crates/racecontrol/src/db/mod.rs — billing_rates pattern
sqlx::query(
    "CREATE TABLE IF NOT EXISTS kiosk_allowlist (
        id TEXT PRIMARY KEY,
        process_name TEXT NOT NULL UNIQUE,
        added_by TEXT NOT NULL DEFAULT 'staff',
        notes TEXT,
        created_at TEXT DEFAULT (datetime('now'))
    )"
)
.execute(pool)
.await?;

sqlx::query(
    "CREATE INDEX IF NOT EXISTS idx_kiosk_allowlist_name ON kiosk_allowlist(process_name)"
)
.execute(pool)
.await?;
```

Key decisions:
- `process_name UNIQUE` — prevents duplicate entries; staff adding twice is a no-op
- `id TEXT PRIMARY KEY` — UUID string, consistent with all other tables
- No foreign keys — allowlist is configuration, not relational data

### Pattern 2: API Route Handlers

**What:** Three handlers following the exact style of billing_rates handlers in routes.rs.
**When to use:** All CRUD for kiosk_allowlist.

```rust
// Source: crates/racecontrol/src/api/routes.rs — billing_rates pattern

// In api_routes():
.route("/config/kiosk-allowlist", get(list_kiosk_allowlist).post(add_kiosk_allowlist_entry))
.route("/config/kiosk-allowlist/:name", delete(delete_kiosk_allowlist_entry))

async fn list_kiosk_allowlist(State(state): State<Arc<AppState>>) -> Json<Value> {
    // SELECT id, process_name, added_by, notes, created_at FROM kiosk_allowlist ORDER BY process_name
    // Returns { "allowlist": [...], "hardcoded_count": ALLOWED_PROCESSES.len() }
}

async fn add_kiosk_allowlist_entry(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // INSERT OR IGNORE INTO kiosk_allowlist (id, process_name, added_by, notes)
    // body: { "process_name": "foo.exe", "notes": "optional" }
}

async fn delete_kiosk_allowlist_entry(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Json<Value> {
    // DELETE FROM kiosk_allowlist WHERE process_name = ?
}
```

Critical: The GET endpoint should return BOTH the server-managed entries AND the hardcoded count so the admin UI can show the full picture. It does NOT need to return hardcoded names (they are baked into the binary).

### Pattern 3: rc-agent Server-Fetched Allowlist (OnceLock)

**What:** A static OnceLock mirroring the existing `learned_allowlist()` pattern in kiosk.rs. Fetched on startup and refreshed every 5 minutes.
**When to use:** The rc-agent needs this at kiosk enforcement time without blocking.

```rust
// Source: crates/rc-agent/src/kiosk.rs — learned_allowlist() pattern

/// Server-fetched allowlist — processes approved via admin panel.
/// Refreshed every 5 minutes via allowlist_poll_loop() in main.rs.
fn server_allowlist() -> &'static Mutex<HashSet<String>> {
    static SERVER: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SERVER.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Replace the server allowlist atomically.
pub fn set_server_allowlist(names: Vec<String>) {
    if let Ok(mut set) = server_allowlist().lock() {
        *set = names.into_iter().map(|s| s.to_lowercase()).collect();
    }
}

// In allowed_set_snapshot(), add after learned_allowlist:
if let Ok(server) = server_allowlist().lock() {
    set.extend(server.iter().cloned());
}
```

**Poll loop in main.rs:**

```rust
// Source: pattern from self_monitor.rs interval loop

async fn allowlist_poll_loop(core_http_url: String, client: reqwest::Client) {
    let mut interval = tokio::time::interval(Duration::from_secs(300)); // 5 minutes
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        interval.tick().await;
        match fetch_server_allowlist(&client, &core_http_url).await {
            Ok(names) => {
                let count = names.len();
                kiosk::set_server_allowlist(names);
                tracing::info!("[allowlist] Updated: {} entries from server", count);
            }
            Err(e) => {
                tracing::warn!("[allowlist] Fetch failed (will retry in 5 min): {}", e);
                // Failure is non-fatal — hardcoded baseline still enforced
            }
        }
    }
}

async fn fetch_server_allowlist(client: &reqwest::Client, base_url: &str) -> anyhow::Result<Vec<String>> {
    // GET {base_url}/api/v1/config/kiosk-allowlist
    // Parse response.allowlist[].process_name
    // Timeout: 10s (same as other rc-agent HTTP calls)
}
```

The `core_http_url` is derived from `config.core.url` (WebSocket URL) by replacing `ws://` with `http://` and stripping the `/ws/agent` path.

### Pattern 4: LLM Process Classification

**What:** Before placing an unknown process in `temp_allowlist`, ask the local rp-debug model. Classification result: ALLOW (add to server list via WebSocket), BLOCK (existing flow), or ASK (existing temp_allow flow).
**When to use:** Only when process has reached `WARN_BEFORE_ACTION_COUNT` threshold.

```rust
// Source: crates/rc-agent/src/ai_debugger.rs — query_ollama() pattern

pub async fn classify_process(
    ollama_url: &str,
    process_name: &str,
    exe_path: &str,
) -> ProcessVerdict {
    let prompt = format!(
        "You are a Windows process security classifier for a sim racing venue kiosk. \
        Classify this process: name='{}', path='{}'. \
        Rules: Windows system processes, GPU drivers, audio services, Realtek, \
        Gigabyte/AORUS, NVIDIA, AMD, Tailscale, RustDesk = ALLOW. \
        Unknown browser helpers, random updaters = ASK. \
        Keyloggers, screen capture, remote access not from allowlist = BLOCK. \
        Reply with exactly one word: ALLOW, BLOCK, or ASK.",
        process_name, exe_path
    );
    // Call query_ollama() with 10s timeout
    // Parse first word of response
}

pub enum ProcessVerdict { Allow, Block, Ask }
```

**Integration point in enforce_process_whitelist_blocking():**

The `enforce_process_whitelist_blocking()` function is called from a `tokio::task::spawn_blocking` context. LLM classification is async HTTP, so it cannot be called inline. The solution: `enforce_process_whitelist_blocking()` returns a new `PendingClassification` vec alongside existing results. The async caller fires `tokio::spawn(classify_process(...))` for each and handles the verdict:
- ALLOW: calls `kiosk::approve_process()` + sends `AgentMessage::ProcessApprovalRequest` to server for persistence
- BLOCK: calls `kiosk::reject_process()` — lockdown
- ASK: existing temp_allow + `AgentMessage::ProcessApprovalRequest` flow

### Anti-Patterns to Avoid

- **Blocking the enforce loop on LLM:** The 100-300ms `sysinfo::refresh_processes()` already makes this loop expensive. LLM adds 2-5s. Always defer classification to an async tokio::spawn task.
- **Replacing hardcoded baseline:** Server list is additive only. Hardcoded ALLOWED_PROCESSES must never be deleted or bypassed. If server is unreachable, enforcement continues with baseline only.
- **Using kiosk_settings key-value table for allowlist:** It lacks CRUD semantics. A comma-separated value cannot be atomically deleted per-entry.
- **Storing full ALLOWED_PROCESSES in DB:** 70+ hardcoded entries baked into the binary are the immutable safety net. DB stores only what staff added beyond that.
- **Calling fetch_server_allowlist() from within kiosk enforcement:** The enforcement runs in spawn_blocking. HTTP fetch is async. Always decouple via the poll loop + static OnceLock.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Atomic set update | Custom RWLock + Vec dance | `OnceLock<Mutex<HashSet>>` + full replacement | Already proven in learned_allowlist() in kiosk.rs |
| HTTP retry on fetch failure | Custom backoff | Non-fatal warn + next 5-min tick | fetch failure leaves existing set intact — safe default |
| Process name normalization | Complex regex | `.to_lowercase()` only | All existing kiosk code uses lowercase comparison |
| LLM response parsing | JSON schema parsing | First-word extraction from plain text | rp-debug model (qwen3:0.6b) produces single-word answers reliably |
| Admin UI table | Custom React table | Inline list with delete button matching existing experience list pattern | settings/page.tsx already has this pattern for experiences |

**Key insight:** The existing kiosk.rs architecture (static OnceLock, HashSet merging in `allowed_set_snapshot()`) was designed to accommodate exactly this kind of additive injection. The server-fetched list is a fourth layer alongside static, dynamic game, learned, and temp.

---

## Common Pitfalls

### Pitfall 1: Derive the HTTP URL from the WebSocket URL Incorrectly
**What goes wrong:** rc-agent's `config.core.url` is `ws://192.168.31.23:8080/ws/agent`. Naively replacing `ws://` gives the wrong path for the API.
**Why it happens:** WebSocket URL has a path suffix; HTTP API is at the root.
**How to avoid:** Strip the path, replace scheme: `ws://192.168.31.23:8080/ws/agent` → `http://192.168.31.23:8080/api/v1/config/kiosk-allowlist`.
**Warning signs:** 404 errors on the allowlist fetch — check the URL construction logic first.

### Pitfall 2: LLM Classification Blocks Enforce Loop
**What goes wrong:** `query_ollama()` is async and takes 2-30s. If called from within `enforce_process_whitelist_blocking()`, the blocking thread pool starves.
**Why it happens:** `spawn_blocking` threads are meant for blocking I/O (sysinfo), not async HTTP.
**How to avoid:** Return `pending_classification: Vec<ProcessName>` from `enforce_process_whitelist_blocking()`. Caller (async context) fires `tokio::spawn(classify_process(...))` for each.
**Warning signs:** rc-agent async runtime hangs; WS heartbeats stop; lock screen freezes.

### Pitfall 3: Server Unreachable at Boot = Empty Allowlist
**What goes wrong:** Pod boots before server is up. First poll at t=0 fails. rc-agent proceeds with only hardcoded baseline for 5 minutes.
**Why it happens:** Normal boot sequence.
**How to avoid:** Non-fatal — hardcoded baseline is comprehensive. Log at WARN, not ERROR. Do NOT block rc-agent startup on allowlist fetch.
**Warning signs:** Excessive WARN logs at boot — confirm they resolve after the next tick.

### Pitfall 4: process_name UNIQUE Constraint Causes Confusing Errors
**What goes wrong:** Staff tries to add `svchost.exe` (already hardcoded). Server returns a 409 or INSERT OR IGNORE silently succeeds with 0 rows.
**Why it happens:** The hardcoded list is not in the DB — there's no conflict — but staff may expect an error.
**How to avoid:** The POST handler should check if the name is in the hardcoded ALLOWED_PROCESSES list and return a clear message: `"already in baseline allowlist — no action needed"`. The DB UNIQUE constraint only prevents DB duplicates.
**Warning signs:** Staff reporting "I added it but nothing changed" — the process was already in the baseline.

### Pitfall 5: LLM Returns Multi-Line Response
**What goes wrong:** qwen3:0.6b occasionally prepends reasoning before the verdict word.
**Why it happens:** Small models sometimes disobey single-word constraints.
**How to avoid:** Extract the first occurrence of `ALLOW`, `BLOCK`, or `ASK` (case-insensitive) anywhere in the response, not just the first line. Default to `ASK` if none found.
**Warning signs:** `ProcessVerdict::Ask` for processes that should be ALLOW — check LLM raw response in logs.

---

## Code Examples

Verified patterns from existing codebase:

### Existing OnceLock Mutex Pattern (kiosk.rs)

```rust
// Source: crates/rc-agent/src/kiosk.rs:38-44
fn learned_allowlist() -> &'static Mutex<HashSet<String>> {
    static LEARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    LEARNED.get_or_init(|| {
        let set = load_learned_allowlist().unwrap_or_default();
        Mutex::new(set)
    })
}
```

Server allowlist follows this exactly but without the file-load initializer (starts empty, filled by poll loop).

### Existing allowed_set_snapshot() Merge (kiosk.rs lines 538-560)

```rust
// Source: crates/rc-agent/src/kiosk.rs:538-560
pub fn allowed_set_snapshot(&self) -> HashSet<String> {
    let mut set: HashSet<String> = ALLOWED_PROCESSES
        .iter()
        .map(|s| s.to_lowercase())
        .chain(self.allowed_extra.iter().cloned())
        .collect();

    // Add learned allowlist
    if let Ok(learned) = learned_allowlist().lock() {
        set.extend(learned.iter().cloned());
    }

    // Add temporarily allowed processes (within TTL)
    if let Ok(temp) = temp_allowlist().lock() {
        for (name, entry) in temp.iter() {
            if entry.added_at.elapsed().as_secs() < TEMP_ALLOW_TTL_SECS {
                set.insert(name.clone());
            }
        }
    }

    set
}
```

Add `server_allowlist()` injection immediately after the `learned_allowlist()` block.

### Existing Ollama Query Pattern (ai_debugger.rs)

```rust
// Source: crates/rc-agent/src/ai_debugger.rs — query_ollama call site
match query_ollama(&config.ollama_url, &config.ollama_model, &prompt).await {
    Ok(suggestion) => { /* handle */ }
    Err(e) => { tracing::warn!("[rc-bot] Ollama query failed: {}", e); }
}
```

Classification uses the same `query_ollama()` function with a shorter, constrained prompt. ollama_url is `http://127.0.0.1:11434` (already configured on all 8 pods per Phase 47).

### Existing Route Registration Pattern (routes.rs)

```rust
// Source: crates/racecontrol/src/api/routes.rs:70-71 — billing_rates pattern
.route("/billing/rates", get(list_billing_rates).post(create_billing_rate))
.route("/billing/rates/{id}", put(update_billing_rate).delete(delete_billing_rate))

// Phase 48 follows same pattern:
.route("/config/kiosk-allowlist", get(list_kiosk_allowlist).post(add_kiosk_allowlist_entry))
.route("/config/kiosk-allowlist/:name", delete(delete_kiosk_allowlist_entry))
```

Note: Use `:name` (process_name) not `:id` (UUID) for DELETE — staff will want to delete by process name from the UI.

### tokio::time::interval Pattern (self_monitor.rs style)

```rust
// Source: pattern established in self_monitor.rs and close-wait fix
let mut interval = tokio::time::interval(Duration::from_secs(300));
interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
loop {
    interval.tick().await;
    // do work
}
```

First tick fires immediately at t=0 (boot fetch), then every 5 minutes.

### Admin Panel API Call Pattern (kiosk/src/lib/api.ts style)

The Next.js admin panel uses an `api.*` object. New methods to add:

```typescript
// Pattern from existing api.listExperiences() / api.deleteExperience()
async listKioskAllowlist(): Promise<{ allowlist: AllowlistEntry[] }> {
  const res = await fetch(`${API_BASE}/config/kiosk-allowlist`);
  return res.json();
}

async addKioskAllowlistEntry(processName: string, notes?: string): Promise<void> {
  await fetch(`${API_BASE}/config/kiosk-allowlist`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ process_name: processName, notes }),
  });
}

async deleteKioskAllowlistEntry(processName: string): Promise<void> {
  await fetch(`${API_BASE}/config/kiosk-allowlist/${encodeURIComponent(processName)}`, {
    method: "DELETE",
  });
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Kill unknown processes immediately | Warn-then-allow (3 sightings) + temp_allow + WebSocket approval | 61e7b0b | Eliminated instant lockdowns; created approval flow |
| Manual ALLOWED_PROCESSES patches | Dynamic server-fetched list (Phase 48) | Phase 48 | Zero rebuilds for new processes; staff self-service |
| Cloud Ollama (removed) | Local Ollama on each pod (qwen3:0.6b, rp-debug) | Phase 47 | Offline AI diagnosis; no internet dependency |

**Deprecated/outdated:**
- Patch-rebuild-redeploy cycle for every new process: replaced by Phase 48 admin panel
- `learned-allowlist.json` local file: still valid for pod-local approvals; Phase 48 server list is authoritative source of truth above it

---

## Open Questions

1. **Should ProcessApprovalRequest WS message also auto-add to DB?**
   - What we know: The existing flow sends `ProcessApprovalRequest` to server for staff to approve/reject. Server sends `ApproveProcess`/`RejectProcess` back.
   - What's unclear: With Phase 48, staff can pre-add via admin panel. But runtime approvals via WS still exist. Should a WS ApproveProcess also write to the DB so it survives across restarts?
   - Recommendation: YES — when the server processes `AgentMessage::ProcessApprovalRequest` and staff approves via dashboard, write to `kiosk_allowlist` DB. This closes the loop: approved-at-runtime processes become permanent without another admin panel visit.

2. **How to expose the LLM classifier verdict to the server dashboard?**
   - What we know: The current `ProcessApprovalRequest` message shows the process to staff. LLM classification adds context.
   - What's unclear: Should the `ProcessApprovalRequest` include `llm_verdict: Option<String>` so the dashboard can show "AI says: ALLOW"?
   - Recommendation: Add `#[serde(default)] pub llm_verdict: Option<String>` to `ProcessApprovalRequest` in protocol.rs. Backward-compatible (serde default = None).

3. **KioskConfig poll interval: hardcoded 5 min or TOML-configurable?**
   - What we know: 5 minutes satisfies the "within 5 minutes" success criterion. Other intervals in rc-agent are hardcoded constants.
   - Recommendation: Hardcoded constant `ALLOWLIST_REFRESH_SECS: u64 = 300` in kiosk.rs. Can be made TOML-configurable in a future phase if needed — YAGNI for now.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | bash (lib/common.sh) + Playwright (allowlist.spec.ts) |
| Config file | tests/e2e/lib/common.sh (already exists from Phase 41) |
| Quick run command | `bash tests/e2e/api/kiosk-allowlist.sh` |
| Full suite command | `bash tests/e2e/run-all.sh` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| ALLOW-01 | GET /api/v1/config/kiosk-allowlist returns JSON with allowlist array | smoke (curl) | `bash tests/e2e/api/kiosk-allowlist.sh` | No — Wave 0 |
| ALLOW-02 | Admin panel renders Allowlist section, add/remove works | browser (Playwright) | `npx playwright test allowlist.spec.ts` | No — Wave 0 |
| ALLOW-03 | rc-agent picks up new process within 5 minutes | integration (curl + wait) | `bash tests/e2e/api/kiosk-allowlist.sh` | No — Wave 0 |
| ALLOW-04 | LLM classification fires for unknown processes | unit (cargo nextest) | `cargo nextest run -p rc-agent-crate test_classify_process` | No — Wave 0 |
| ALLOW-05 | No false lockdowns: hardcoded baseline unchanged | unit (cargo nextest) | `cargo nextest run -p rc-agent-crate test_hardcoded_baseline_intact` | No — Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo nextest run -p rc-agent-crate` (unit tests only, <30s)
- **Per wave merge:** `bash tests/e2e/api/kiosk-allowlist.sh`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `tests/e2e/api/kiosk-allowlist.sh` — CRUD test (ALLOW-01, ALLOW-03): POST entry, GET verify, wait 5 min for pod pickup, DELETE
- [ ] `tests/e2e/browser/allowlist.spec.ts` — Playwright test (ALLOW-02): admin panel UI CRUD
- [ ] Unit tests in `crates/rc-agent/src/kiosk.rs` — `test_server_allowlist_injected`, `test_hardcoded_baseline_intact`, `test_classify_process_verdict_parsing`
- [ ] Unit tests in racecontrol-crate — `test_kiosk_allowlist_crud` covering insert/select/delete SQL queries

---

## Sources

### Primary (HIGH confidence)

- `crates/rc-agent/src/kiosk.rs` — Full source read; `ALLOWED_PROCESSES`, `KioskManager`, `allowed_set_snapshot()`, `learned_allowlist()`, `TempAllowEntry`, `enforce_process_whitelist_blocking()`
- `crates/rc-agent/src/ai_debugger.rs` — `query_ollama()`, `PodErrorContext`, `build_prompt()` patterns
- `crates/rc-agent/src/main.rs` — `AgentConfig`, `KioskConfig`, startup sequence
- `crates/racecontrol/src/db/mod.rs` — `migrate()`, table schema patterns, billing_rates as CRUD template
- `crates/racecontrol/src/api/routes.rs` — Route registration, billing_rates handler pattern, kiosk_settings handler
- `crates/racecontrol/src/state.rs` — `AppState` structure, http_client, existing fields
- `crates/rc-common/src/protocol.rs` — `AgentMessage::ProcessApprovalRequest`, `CoreToAgentMessage::ApproveProcess`/`RejectProcess`, existing WS message format
- `crates/racecontrol/src/config.rs` — `Config` struct, how to add new config sections
- `kiosk/src/app/settings/page.tsx` — Admin panel insertion point, existing patterns

### Secondary (MEDIUM confidence)

- `.planning/ROADMAP.md` — Phase 48 goals and success criteria (authoritative)
- `.planning/STATE.md` — Protocol decisions, backward-compat patterns established in prior phases

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries are already present in the codebase; no new dependencies
- Architecture: HIGH — patterns are directly derived from existing working code (billing_rates CRUD, learned_allowlist OnceLock, query_ollama)
- Pitfalls: HIGH — pitfalls identified from direct code analysis (enforce_process_whitelist_blocking is spawn_blocking, URL construction, LLM response variability)
- Test strategy: HIGH — follows established Phase 41-47 shell + Playwright convention

**Research date:** 2026-03-19
**Valid until:** 2026-04-19 (stable codebase; patterns won't change)
