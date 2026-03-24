# Stack Research

**Domain:** AI-driven watchdog recovery — v17.1 additions to existing Rust/Axum fleet ops platform (8 Windows 11 pods).
**Researched:** 2026-03-25
**Confidence:** HIGH (codebase read directly; versions confirmed against existing Cargo.toml; integration points traced through rc-sentry/rc-agent/rc-common source)

---

## Context: What Already Exists (Do Not Re-research)

The v17.1 milestone is NOT starting from scratch. Substantial watchdog infrastructure already ships:

| Component | Location | Status |
|-----------|----------|--------|
| Watchdog FSM (Healthy/Suspect/Crashed, 3-poll hysteresis) | `rc-sentry/src/watchdog.rs` | DONE |
| Tier 1 deterministic fixes (kill zombies, port wait, close wait, config repair, shader cache) | `rc-sentry/src/tier1_fixes.rs` | DONE |
| RestartTracker (3-in-10min escalation + 5s/15s/30s/60s/5min backoff) | `rc-sentry/src/tier1_fixes.rs` | DONE |
| Pattern memory (debug-memory-sentry.json, instant_fix, hit_count pruning) | `rc-sentry/src/debug_memory.rs` | DONE |
| Tier 3 Ollama (TcpStream raw HTTP, fire-and-forget, qwen2.5:3b) | `rc-sentry/src/ollama.rs` | DONE |
| Recovery authority registry (ProcessOwnership, RecoveryAuthority enum) | `rc-common/src/recovery.rs` | DONE |
| RecoveryDecision JSONL log (timestamp, machine, process, authority, action, reason) | `rc-common/src/recovery.rs` | DONE |
| EscalatingBackoff state machine (30s/2m/10m/30m) | `rc-common/src/watchdog.rs` | DONE |
| Spawn verification (schtasks /Run + 20s health poll loop) | `rc-sentry/src/tier1_fixes.rs` | DONE |
| Graceful restart sentinel (GRACEFUL_RELAUNCH, RCAGENT_SELF_RESTART) | `rc-sentry/src/tier1_fixes.rs` | DONE |
| MAINTENANCE_MODE sentinel (3 restarts in 10min = stop all restarts) | `rc-sentry/src/tier1_fixes.rs` | DONE |
| rc-watchdog Windows Service (pod mode) + James daemon (standalone mode) | `rc-watchdog/src/` | DONE |
| windows-service 0.8 | `rc-watchdog/Cargo.toml` | DONE |
| rc-agent ai_debugger (Tier 1/2/3 for game crashes, billing-gated destructive fixes) | `rc-agent/src/ai_debugger.rs` | DONE |
| Feature flags (sentry-flags.json, kill_watchdog_restart kill switch) | `rc-sentry/src/watchdog.rs` | DONE |

**Implication:** The core 4-tier architecture and most low-level machinery already exists. v17.1 is integration and consolidation, not greenfield implementation.

---

## What v17.1 Actually Needs (Gap Analysis)

Reading the PROJECT.md v17.1 goals against the existing code:

| Goal | Gap | Status |
|------|-----|--------|
| Replace rc-sentry blind restart with AI healer pattern memory + graduated response | Pattern memory + Tier 1 exist; need to wire Tier 2 (memory lookup) into the crash handler loop before Tier 3 Ollama | PARTIAL |
| Merge pod_monitor server-side WoL + restart into AI healer with context-aware recovery | pod_monitor.rs exists on racecontrol server; RecoveryAuthority::PodHealer exists; coordination logic not yet written | PARTIAL |
| Replace james_watchdog.ps1 blind 2min service check with AI debugger + pattern memory | rc-watchdog james_monitor.rs runs the check loop; Ollama query missing from james monitor path | PARTIAL |
| Single recovery authority per machine, prevent fighting between self_monitor / rc-sentry / pod_monitor / WoL | ProcessOwnership registry exists in rc-common; not yet enforced at runtime (ownership check not called before restart) | PARTIAL |
| Distinguish crash vs deliberate shutdown | GRACEFUL_RELAUNCH and RCAGENT_SELF_RESTART sentinels exist; pod_monitor does not yet check sentinels before WoL | GAP |

---

## Recommended Stack (New Additions for v17.1)

### No New Crates Required

All capability gaps can be closed using existing dependencies. This is the key finding of this research. Adding crates for v17.1 would be over-engineering — the primitives are already present.

**Existing stack covers everything:**

| Need | Existing Solution |
|------|------------------|
| Process supervision (spawn + verify) | `tier1_fixes::restart_service()` + `verify_service_started()` — already polls health endpoint for 20s post-spawn |
| Crash pattern memory/matching | `debug_memory::DebugMemory::instant_fix()` + `derive_pattern_key()` — already in rc-sentry |
| Tier 2 memory lookup before Tier 3 LLM | `debug_memory.rs` API is ready; just not called from `handle_crash()` before `ollama` |
| Graduated response (Tier 1 → 2 → 3 → 4) | Tier 1 in `tier1_fixes.rs`, Tier 3 in `ollama.rs`; needs wiring into main crash handler |
| Recovery authority coordination | `recovery::ProcessOwnership` + `RecoveryAuthority` enum — registry exists |
| Windows service lifecycle | `windows-service = "0.8"` already in rc-watchdog/Cargo.toml |
| Deliberate shutdown detection | `GRACEFUL_RELAUNCH` + `RCAGENT_SELF_RESTART` sentinels already exist |
| Crash diagnosis reporting to server | `CrashDiagResult` type in rc-common; fleet API endpoints in racecontrol |
| Alert escalation (Tier 4) | `bono_alert.rs` in rc-watchdog; WhatsApp via existing comms-link relay |

---

## Integration Points: What to Wire Up

### 1. Wire Tier 2 Into rc-sentry Crash Handler

**Location:** `crates/rc-sentry/src/tier1_fixes.rs` — `handle_crash()` function.

The current `handle_crash()` runs Tier 1 fixes then goes straight to `restart_service()`. Before calling Ollama (Tier 3), it should check pattern memory:

```rust
// Current flow (abbreviated):
handle_crash(ctx, tracker) → [tier1 fixes] → restart_service()

// v17.1 flow:
handle_crash(ctx, tracker)
  → [tier1 fixes]
  → derive_pattern_key(ctx) → DebugMemory::instant_fix(key)
       → if known pattern: apply recorded fix (Tier 2 instant replay)
       → if unknown: query_ollama(ctx) async (Tier 3)
            → on result: record fix in DebugMemory
  → restart_service()
```

**No new types needed.** `CrashIncident.fix_type` already stores the fix type string. Map fix_type strings to the existing `fix_*` function dispatch.

### 2. Enforce Recovery Authority Before Any Restart

**Location:** `crates/rc-common/src/recovery.rs` — `ProcessOwnership` registry.

The registry exists but is not enforced at call sites. Each recovery authority (rc-sentry, pod_healer, james_monitor) should call `ownership.owner_of(process)` before attempting a restart. If a different authority owns the process, log and skip — do not fight.

**Pattern:**

```rust
// In rc-sentry tier1_fixes.rs before restart_service():
if let Some(owner) = ownership.owner_of("rc-agent.exe") {
    if owner != RecoveryAuthority::RcSentry {
        // Another authority is handling this — skip our restart
        return (results, false);
    }
}
```

The `ProcessOwnership` struct is not `Sync` by itself. Wrap in `Arc<Mutex<ProcessOwnership>>` in AppState-equivalents where shared across threads (rc-sentry is pure std threads, so `std::sync::Mutex` — already in use for other guards in the codebase).

### 3. Sentinel Check in pod_monitor Before WoL

**Location:** `crates/racecontrol/src/` — pod_monitor or pod_healer module.

pod_monitor currently uses `RecoveryAuthority::PodHealer`. Before sending WoL or triggering restart, it should check for sentinel files on the pod via rc-sentry `/files` endpoint (already exists as a 6th endpoint on rc-sentry):

- `C:\RacingPoint\MAINTENANCE_MODE` — pod deliberately stopped, do NOT WoL
- `C:\RacingPoint\GRACEFUL_RELAUNCH` — self-monitor restart in progress, skip WoL
- `C:\RacingPoint\rcagent-restart-sentinel.txt` — deploy restart in progress, skip WoL

This is the missing "distinguish crash vs deliberate shutdown" logic. No new API needed — rc-sentry already serves `/files` which can read sentinel paths.

### 4. James Monitor Tier 3 Path

**Location:** `crates/rc-watchdog/src/james_monitor.rs`

The james_monitor runs a 2-min check loop (confirmed in main.rs). Currently it checks services are alive. v17.1 adds:

1. On failure detection: check james debug memory (`C:\Users\bono\racingpoint\recovery-log.jsonl`) for past incidents
2. If no known pattern: query Ollama at 192.168.31.27:11434 (same host — low latency, no network dependency)
3. Log decision to `RECOVERY_LOG_JAMES` constant already defined in recovery.rs

The Ollama query path is already implemented in `rc-sentry/src/ollama.rs`. Extract `query_crash()` into `rc-common` or use it directly from rc-watchdog by referencing rc-sentry source. The cleaner approach: move `ollama.rs` content to `rc-common/src/ollama.rs` so both rc-sentry and rc-watchdog share it without code duplication.

**Cargo change needed:**

```toml
# crates/rc-common/Cargo.toml — add (for ollama module):
# No new crates — TcpStream HTTP is already used in rc-common exec.rs
# ollama.rs uses std::net::TcpStream + serde_json — both already in rc-common
```

### 5. Tier 4 Escalation: WhatsApp Alert

**Existing path:** `bono_alert.rs` in rc-watchdog uses the comms-link relay at `localhost:8766`.

This is already the Tier 4 escalation path for james_monitor. For pod-side escalation (rc-sentry reaching Tier 4), the path is: rc-sentry → fleet API on racecontrol (`POST /api/v1/fleet/alert`) → racecontrol → comms-link relay → WhatsApp. This indirect path is correct — pods do not have direct comms-link access.

No new crates. The fleet API alert endpoint may need to be added to racecontrol if not already present (verify against existing routes before adding).

---

## Confirmed Existing Crates (rc-watchdog Cargo.toml)

| Crate | Version | Purpose |
|-------|---------|---------|
| windows-service | 0.8 | Windows SYSTEM service lifecycle (SCM registration, stop signal handling) |
| reqwest | 0.12 blocking | HTTP crash reports to racecontrol fleet API |
| winapi 0.3 | wtsapi32, processthreadsapi, winbase, handleapi, securitybaseapi, userenv, winnt, errhandlingapi | Session 1 process launch (launching GUI processes from SYSTEM service context) |

The `winapi` features already selected in rc-watchdog cover all Windows process supervision needs for v17.1 — no additional winapi features needed.

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `sysinfo` for process inspection in watchdog | Anti-cheat constraint — EAC and iRacing ban process enumeration APIs; sysinfo uses CreateToolhelp32Snapshot | Health endpoint polling (already used) — pure TCP, anti-cheat safe |
| `tokio` in rc-sentry | rc-sentry is deliberately pure std::net — zero shared runtime deps with the process it's watching; tokio in both creates crash coupling | std::thread + std::sync::mpsc (already in place) |
| A new "watchdog coordinator" binary | Would require new port, new process, new auth, new deploy; coordination is a data problem not a process problem | rc-common ProcessOwnership registry + sentinel files (already exist) |
| External pattern matching library (regex, etc.) | Pattern keys are derived from structured fields (panic message, exit code, startup phase) — substring matching is sufficient and anti-cheat safe | `String::contains()` + `derive_pattern_key()` (already in debug_memory.rs) |
| `anyhow` in rc-sentry | rc-sentry currently has no anyhow dep — error handling is manual match; adding anyhow for a minor ergonomics win would increase binary size | Manual `match` + `tracing::warn!` (established pattern) |
| Persistent SQLite for pattern memory | debug-memory.json atomic write is already implemented, simple, and sufficient for 50 patterns with hit_count pruning | debug-memory-sentry.json (already in debug_memory.rs) |
| HTTP client library (reqwest) in rc-sentry | rc-sentry uses raw TcpStream HTTP on purpose — no tokio runtime, no TLS, minimal binary; reqwest would pull in tokio + hyper | std::net::TcpStream HTTP (established pattern in watchdog.rs and ollama.rs) |
| Separate crash log aggregator | 8 pods, each with local JSONL log, served via existing rc-sentry /files endpoint | RecoveryLogger JSONL (already in rc-common/recovery.rs) |

---

## Cargo.toml Changes for v17.1

The only Cargo change needed is moving the Ollama module to rc-common for sharing:

```toml
# crates/rc-common/Cargo.toml — no new external dependencies
# ollama.rs uses: serde_json (workspace), std::net::TcpStream — both present
# Move src from rc-sentry/src/ollama.rs → rc-common/src/ollama.rs
# Add pub mod ollama; to rc-common/src/lib.rs
# Update rc-sentry to use rc_common::ollama::query_crash()
# rc-watchdog uses rc-common (workspace) — gets ollama module for free
```

**Total new crates for v17.1: 0**

---

## Session 1 Process Launch (Critical Windows Constraint)

The existing rc-watchdog `winapi` setup already handles the primary Windows constraint for v17.1: launching GUI processes (Edge, lock screen) from a SYSTEM service context requires a Session 1 token. This is already solved in `rc-watchdog/src/session.rs` via `WTSQueryUserToken` + `CreateProcessAsUser`.

For v17.1, any NEW process launches triggered by AI recovery actions (e.g., relaunching Edge, relaunching lock screen) must use the same `session.rs` path — do not use `std::process::Command` directly from the watchdog service context for GUI processes.

**Rule:** `std::process::Command` = console/background processes (rc-agent, schtasks). Session 1 launch = GUI processes (Edge, lock screen). Mixing these causes the "launches but nothing appears on screen" failure mode.

---

## Recovery Coordination Protocol (No New Crate — Sentinel + JSONL)

The v17.1 "single recovery authority per machine" requirement is solved by two existing mechanisms:

**Sentinel files** (coarse lock — prevents two systems restarting simultaneously):

```
C:\RacingPoint\GRACEFUL_RELAUNCH        — rc-agent self_monitor restart in progress
C:\RacingPoint\rcagent-restart-sentinel.txt — deploy restart in progress
C:\RacingPoint\MAINTENANCE_MODE         — all restarts blocked
```

**RecoveryDecision JSONL** (audit trail — what happened and why):

```
C:\RacingPoint\recovery-log.jsonl       — pod + server decisions (RECOVERY_LOG_POD)
C:\Users\bono\racingpoint\recovery-log.jsonl — james decisions (RECOVERY_LOG_JAMES)
```

**ProcessOwnership registry** (runtime — prevents authority conflicts):

```rust
// rc-common::recovery::ProcessOwnership
// Checked before any restart action
// Single source of truth: "who owns rc-agent.exe?"
```

These three together provide the coordination without any new infrastructure.

---

## Version Compatibility

| Package | Version | Notes |
|---------|---------|-------|
| windows-service | 0.8 | Existing; handles Windows SCM contract correctly |
| winapi | 0.3 | Existing; WTSQueryUserToken for Session 1 launch |
| reqwest | 0.12 blocking | Existing in rc-watchdog; blocking feature required (no tokio in watchdog) |
| serde/serde_json | 1 (workspace) | debug-memory.json, recovery-log.jsonl serialization |
| chrono | 0.4 (workspace) | RecoveryDecision UTC timestamps |
| tracing | 0.1 (workspace) | All structured logging |

No version conflicts — all additions use existing workspace versions.

---

## Sources

- `crates/rc-sentry/src/` (watchdog.rs, tier1_fixes.rs, debug_memory.rs, ollama.rs, main.rs) — read directly: HIGH confidence
- `crates/rc-common/src/` (recovery.rs, watchdog.rs, exec.rs) — read directly: HIGH confidence
- `crates/rc-watchdog/Cargo.toml` and `src/main.rs` — read directly: HIGH confidence
- `crates/rc-agent/src/ai_debugger.rs` — read directly: HIGH confidence
- PROJECT.md v17.1 milestone goals — read directly: HIGH confidence
- CLAUDE.md standing rules (spawn verification, non-interactive context, Session 1 constraint) — read directly: HIGH confidence

---

*Stack research for: v17.1 Watchdog-to-AI Migration — AI-driven recovery on Windows, 8-pod fleet*
*Researched: 2026-03-25 IST*
