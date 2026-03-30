# Stack Research — v31.0 Autonomous Survival System

**Domain:** 3-Layer MI Independence (Rust/Axum Windows service monorepo additions)
**Researched:** 2026-03-30
**Confidence:** HIGH (all crate versions verified via crates.io/docs.rs; OpenRouter model data from official docs + March 2026 coverage; rc-watchdog/rc-agent Cargo.toml read directly)

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `goblin` | `"0.10"` | PE header parsing and validation | The de-facto standard Rust crate for cross-platform binary parsing. Exposes `DosHeader` (with `DOS_MAGIC`), `CoffHeader` (with `machine`, `time_date_stamp`), and `PE_MAGIC` constant. Fuzzed, zero-copy, no extra allocator. Latest: 0.10.5 released Feb 2026. Only add `features = ["pe"]` — skip ELF/Mach-o dead code in Windows service binary. |
| `sha2` | workspace `"0.10"` | SHA256 manifest verification | Already in workspace AND already used by rc-agent for OTA binary identity (OTA-10). Zero new dependency — zero Cargo.lock churn. |
| `reqwest` | workspace `"0.12"`, `blocking` feature | HTTP client in rc-watchdog for cross-layer reports and manifest fetch | rc-watchdog has NO tokio runtime — it is fully synchronous (confirmed by reading all source files). `reqwest::blocking` is already present in `rc-watchdog/Cargo.toml`. Use exact same pattern as the existing `reporter.rs` `send_crash_report()`. No new crate version needed. |
| `reqwest` | workspace `"0.12"`, `json` + async | HTTP client in rc-agent for AI diagnosis and existing allowlist polling | Already present via `http-client` feature. The watchdog-to-server cross-layer reporting path runs in rc-watchdog (blocking), not rc-agent. No change needed here. |
| `tokio` | workspace `"1"`, `features = ["rt", "rt-multi-thread"]` | Async runtime ONLY IF rc-watchdog needs async OpenRouter calls | NOT currently in rc-watchdog. Add ONLY if adding async OpenRouter diagnosis to the watchdog layer. If added: never use `#[tokio::main]` — `windows-service` owns the entry point. Use `tokio::runtime::Runtime::new()?.block_on(...)` inside the synchronous service handler, created once and held in service state. |

### New Crate Additions (Net New to Codebase)

| Crate | Version | Target Crate | Purpose | Why Not Alternatives |
|-------|---------|-------------|---------|----------------------|
| `goblin` | `{ version = "0.10", default-features = false, features = ["pe"] }` | `rc-watchdog/Cargo.toml` | Parse PE `MachineType`, `TimeDateStamp`, `SizeOfImage`, DOS magic to detect binary corruption, replacement, or foreign binary injection | `pe-parser` last released 2022, does not expose `time_date_stamp`. `object` crate targets DWARF/debug info — 3x compile weight for validation-only use. `goblin` is the ecosystem standard. |

### No-Change Dependencies (Already Present — Just Use Them)

| Library | Version | Where | What to Use |
|---------|---------|-------|-------------|
| `sha2` | workspace `0.10` | rc-watchdog already has it | `sha2::Sha256::digest(&file_bytes)` |
| `hex` | workspace `0.4` | rc-watchdog already has it | `hex::encode(hash)` for manifest comparison |
| `serde` + `serde_json` | workspace | everywhere | Serialize survival report payload |
| `toml` | workspace `0.8` | rc-watchdog already has it | Parse `release-manifest.toml` for expected SHA256 |
| `tracing` | workspace `0.1` | rc-watchdog already has it | Structured event logging |
| `anyhow` | workspace `1` | rc-watchdog already has it | Error propagation, no `.unwrap()` |
| `chrono` | workspace `0.4` | rc-watchdog already has it | Parse PE `TimeDateStamp` (Unix epoch u32), compare manifest age |
| `reqwest` | workspace `0.12` | rc-watchdog already has it | `reqwest::blocking::Client` for survival reports |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| `cargo test -p rc-watchdog` | Test PE validation logic and manifest comparison in isolation | No Windows service registration needed for unit tests |
| `touch crates/rc-watchdog/build.rs && cargo build --release --bin rc-watchdog` | Rebuild with new survival modules | Force GIT_HASH refresh — always touch build.rs after commit |

---

## Cargo.toml Changes Required

```toml
# crates/rc-watchdog/Cargo.toml — only line to add:
goblin = { version = "0.10", default-features = false, features = ["pe"] }

# Also add tokio ONLY if async OpenRouter calls are needed:
tokio = { workspace = true, features = ["rt", "rt-multi-thread"] }
```

```toml
# Workspace Cargo.toml — NO CHANGES NEEDED
# sha2, hex, serde, serde_json, toml, tracing, anyhow, chrono all already present
```

```toml
# crates/racecontrol/Cargo.toml — add IF server-side PE validation is added:
goblin = { version = "0.10", default-features = false, features = ["pe"] }
```

---

## OpenRouter Models — Top 5 for Survival System AI (March 2026)

The existing `openrouter.rs` in rc-agent already defines 5 models. The v31.0 survival system needs rc-watchdog to call OpenRouter DIRECTLY (bypassing rc-agent, which may be dead). Use these models and IDs:

| Role | Model | OpenRouter ID | Input / Output per 1M tokens | Context | Why This Model |
|------|-------|--------------|-------------------------------|---------|---------------|
| **Scanner** | Qwen3 235B | `qwen/qwen3-235b-a22b-2507` | ~$0.22 / $0.88 | 128K | Volume coverage, exhaustive enumeration. Already battle-tested in 139+ MMA findings at Racing Point. Fleet context-aware. Proven at Tier 3. |
| **Reasoner** | DeepSeek R1 | `deepseek/deepseek-r1-0528` | ~$0.55 / $2.19 | 128K | Chain-of-thought reasoning. Best for diagnosing "why did the binary get replaced" or "what sequence of events caused the watchdog to fail to restart." Finds absence bugs other models miss. |
| **Code Expert** | DeepSeek V3 | `deepseek/deepseek-v3-0324` | ~$0.38 / $1.42 | 128K | Rust/Windows code patterns, Session 0/1 separation, PE structure analysis. DeepSeek V3 matches frontier models at ~1/100th the cost. ID must match the pinned version — check against openrouter.rs. |
| **SRE / Ops** | Devstral 2 | `mistral/devstral-2` | ~$0.05 / $0.22 | 256K | Agentic operational gap detection, failure recovery, 123B MoE, SWE-bench proven. Ultra-low cost ($0.05/call) makes it safe for every crash event without budget concern. Use as Tier 3 in watchdog (before Qwen if cost is priority). |
| **Security** | Gemini 2.5 Pro | `google/gemini-2.5-pro-preview-03-25` | ~$1.25 / $10.00 | 1M | Config errors, credential exposure, auth boundary violations. Reserve for Tier 4 escalation only — expensive. Do NOT call from every crash event. |

**Estimated cost per watchdog AI call:**
- Tier 3 (Devstral only): ~$0.01-0.05 — safe for every crash
- Tier 4 (all 5 parallel): ~$3-5 — gate behind `budget_remaining > $5` check

### Management Key Pattern for Per-Pod Scoping

OpenRouter supports provisioning child API keys via management key API. Use this to give each pod watchdog a daily-capped key:

```
POST https://openrouter.ai/api/v1/keys
Authorization: Bearer <OPENROUTER_MANAGEMENT_KEY>
Content-Type: application/json

{
  "name": "rc-watchdog-pod-{N}",
  "limit": 10,
  "limit_reset": "daily"
}
```

Response includes `"key": "sk-or-v1-..."` (shown only once — store in pod's env or toml).

- `OPENROUTER_MANAGEMENT_KEY` lives on server only, never on pods
- Each pod watchdog's `OPENROUTER_KEY` is a child key with `limit: 10` daily cap
- Child keys use the same `/api/v1/chat/completions` endpoint as regular keys — zero code change in the API call layer
- Key provisioning can be automated at deploy time via `deploy-pod.sh`

---

## Cross-Layer HTTP Reporting Protocol

### Pattern: rc-watchdog Direct POST to racecontrol Server

rc-watchdog already implements this pattern in `reporter.rs`. Extend it with a new endpoint:

```
rc-watchdog (pod, synchronous)
  → POST http://192.168.31.23:8080/api/v1/pods/{pod_id}/survival-report
    (uses existing reqwest::blocking::Client, 5s timeout — same as send_crash_report)
  → racecontrol: store in SQLite, push to fleet dashboard, trigger WhatsApp if critical
```

**Why POST to server, not rc-agent:**
rc-agent is the process under survival monitoring — it may be dead when the watchdog fires. The server at `.23:8080` is the only durable target that is independent of the pod agent lifecycle.

**Survival report payload (extend WatchdogCrashReport or add new type in rc-common):**

```json
{
  "event_type": "binary_integrity_fail" | "manifest_mismatch" | "ai_diagnosis_complete" | "guardian_alert",
  "pod_id": "pod_3",
  "build_id": "5db7804d",
  "binary_sha256_actual": "abc123...",
  "binary_sha256_expected": "def456...",
  "pe_valid": true,
  "pe_machine": "x86_64",
  "pe_build_timestamp": 1743350400,
  "ai_diagnosis": {
    "tier": 3,
    "model": "mistral/devstral-2",
    "root_cause": "Binary replaced by stale version during incomplete OTA",
    "fix_action": "restart_service",
    "confidence": 0.87,
    "cost_usd": 0.03
  },
  "timestamp": "2026-03-30T12:00:00+05:30"
}
```

**HTTP client in rc-watchdog — use exactly this pattern from reporter.rs:**
```rust
let client = reqwest::blocking::Client::builder()
    .timeout(std::time::Duration::from_secs(5))
    .build()?;
client.post(&url).json(&report).send()?;
```

### External Guardian (James .27 + Bono VPS)

```
guardian binary (standalone, no service)
  → polls GET http://192.168.31.23:8080/api/v1/health every 60s
  → on failure: increment counter
  → on 3 consecutive failures (3 min total): POST survival-report + WhatsApp Uday
  → exponential backoff: 60s → 120s → 300s → 600s max
```

Guardian uses `reqwest::blocking` — simple polling loop, no tokio needed. Can be a new small binary in the workspace or an extension of `james_monitor.rs` (already exists in rc-watchdog).

---

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| `goblin` with `pe` feature only | `object` crate | Use `object` only if DWARF debug symbol extraction is also needed. For PE header validation only, `object` is 3x heavier with no benefit. |
| `goblin 0.10` | `pe-parser` | Never — `pe-parser` is unmaintained (last release 2022) and lacks `time_date_stamp` field. |
| `reqwest::blocking` in rc-watchdog | Add full tokio + async reqwest | Add tokio only if rc-watchdog gains long-running async tasks (e.g., gossip WebSocket subscription). For fire-and-forget HTTP reports and manifest polls, blocking is correct, simpler, and avoids the `windows-service` entry point conflict. |
| Per-pod child key (`limit: 10/day`) | Single shared OPENROUTER_KEY | Single key has no per-pod cap. One crashed pod in a diagnosis storm could burn the entire fleet budget. Child keys with daily reset are the correct production pattern. |
| Devstral 2 as Tier 3 in watchdog | MiMo v2 Pro | MiMo v2 Pro ($0.77/call) is still in the rc-agent Tier 4 stack. Devstral 2 ($0.05/call) is 15x cheaper — use it as the default Tier 3 for high-frequency watchdog calls. Keep MiMo in Tier 4 for ops diagnosis. |
| Direct POST to racecontrol server | Gossip via existing WebSocket | WebSocket gossip is for pod-to-server live operational state. Survival reports are crash events that must be durably stored even if the server restarts mid-report. HTTP POST + SQLite on server is the right persistence pattern. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `pe-parser` crate | Unmaintained since 2022. Missing `time_date_stamp`. | `goblin 0.10` with `features = ["pe"]` |
| `goblin` with default features | Pulls in ELF and Mach-o parsers — dead code on Windows, inflates binary size and compile time. | `goblin = { version = "0.10", default-features = false, features = ["pe"] }` |
| `#[tokio::main]` on rc-watchdog binary | `windows-service` controls the entry point via `service_dispatcher::start()`. `#[tokio::main]` conflicts and causes "cannot drop runtime in async context" panics. | `tokio::runtime::Runtime::new()?.block_on(...)` inside the service handler, created once in service state |
| `reqwest` async in rc-watchdog without explicit runtime | Async reqwest requires a tokio context. rc-watchdog service handler is synchronous. Calling async reqwest without a runtime panics immediately. | `reqwest::blocking` for all rc-watchdog HTTP calls |
| `tokio = { features = ["full"] }` in rc-watchdog | `full` features include io_uring, net, process, signal, time, macros — all unnecessary for a watchdog that only needs the executor. Inflates binary. | `features = ["rt", "rt-multi-thread"]` |
| SHA1 or MD5 for manifest comparison | Collision vulnerabilities. The OTA deploy pipeline (`deploy-pod.sh`, `deploy-server.sh`) already generates SHA256 manifests. Using a different algorithm creates a mismatch. | `sha2` (already in workspace) |
| Calling OpenRouter in the crash-restart hot path | OpenRouter calls add 500ms–30s latency per call. The rc-agent restart must be initiated IMMEDIATELY on crash detection. | Trigger restart first, then spawn a thread for AI diagnosis. Never block restart on AI. |
| `serde` feature on goblin | Not needed for validation — adds serde derives to all PE structs, inflating binary. | Omit — just read the fields directly from parsed structs |

---

## Stack Patterns by Variant

**PE validation in rc-watchdog (Layer 1 integrity check):**
```rust
// Add goblin to rc-watchdog/Cargo.toml
// goblin = { version = "0.10", default-features = false, features = ["pe"] }

use goblin::pe::{PE, header::{DOS_MAGIC, COFF_MACHINE_X86_64}};

fn validate_pe(path: &str) -> anyhow::Result<PeInfo> {
    let bytes = std::fs::read(path)?;
    let pe = PE::parse(&bytes)
        .map_err(|e| anyhow::anyhow!("PE parse failed for {}: {}", path, e))?;

    // Validate DOS magic (catches truncated or replaced files)
    if pe.header.dos_header.signature != DOS_MAGIC {
        anyhow::bail!("DOS magic mismatch in {}", path);
    }
    // Validate architecture (catch arm64 or 32-bit binaries deployed to x64 pods)
    if pe.header.coff_header.machine != COFF_MACHINE_X86_64 {
        anyhow::bail!("Wrong machine type: {:x}", pe.header.coff_header.machine);
    }
    Ok(PeInfo {
        machine: pe.header.coff_header.machine,
        time_date_stamp: pe.header.coff_header.time_date_stamp,
        // u32 Unix timestamp of when the binary was compiled
    })
}
```

**SHA256 manifest verification in rc-watchdog:**
```rust
// Zero new crates — sha2, hex, toml all in workspace

use sha2::{Sha256, Digest};

fn verify_sha256(binary_path: &str, expected_hex: &str) -> anyhow::Result<bool> {
    let bytes = std::fs::read(binary_path)?;
    let hash = Sha256::digest(&bytes);
    let actual_hex = hex::encode(hash);
    Ok(actual_hex == expected_hex)
}
```

**Async OpenRouter from rc-watchdog service (if needed):**
```rust
// Add tokio with features = ["rt", "rt-multi-thread"] to rc-watchdog/Cargo.toml
// Create runtime ONCE in service state, not on each call

struct ServiceState {
    async_rt: tokio::runtime::Runtime,
}

impl ServiceState {
    fn new() -> anyhow::Result<Self> {
        Ok(Self {
            async_rt: tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()?,
        })
    }

    fn diagnose_with_ai(&self, context: DiagnosisContext) -> anyhow::Result<Diagnosis> {
        // Block current service thread until diagnosis completes
        // Restart of rc-agent must have already been triggered BEFORE this call
        self.async_rt.block_on(async {
            openrouter_diagnose_tier3(&context).await
        })
    }
}
```

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| `goblin 0.10` | `rustc 1.85.0` (Rust 2024 edition) | Repo already uses edition 2024 (confirmed in workspace Cargo.toml). Compatible. |
| `goblin 0.10` | Windows x64 target | PE parsing is cross-platform — works on Windows build targets. No platform-specific deps. |
| `reqwest 0.12` (blocking) | No tokio in rc-watchdog | `reqwest::blocking` spawns its own internal thread pool. Safe without tokio. |
| `tokio 1` (if added to watchdog) | `windows-service 0.8` | `windows-service` does not require or conflict with tokio. Safe to coexist. |
| `sha2 0.10` + `hex 0.4` | Already in workspace | No version change. No lock file churn. |
| `goblin 0.10` + `serde 1` | Incompatible unless `goblin` `serde` feature enabled | Do NOT enable goblin's serde feature — read PE fields directly without derived serialization. |

---

## Sources

- `C:/Users/bono/racingpoint/racecontrol/crates/rc-watchdog/Cargo.toml` — confirmed: no tokio, `reqwest = { version = "0.12", features = ["json", "blocking"] }`, `winapi 0.3`, `windows-service 0.8`
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/Cargo.toml` — confirmed: `reqwest 0.12` optional, `sha2` workspace, `rusqlite 0.32`, `goblin` NOT present yet
- `C:/Users/bono/racingpoint/racecontrol/Cargo.toml` (workspace) — confirmed: `sha2 0.10`, `hex 0.4`, `serde 1`, `toml 0.8`, `chrono 0.4`, `anyhow 1`, `tokio 1`
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-watchdog/src/reporter.rs` — confirmed `reqwest::blocking::Client` pattern, 5s timeout, fire-and-forget
- `C:/Users/bono/racingpoint/racecontrol/crates/rc-agent/src/openrouter.rs` — confirmed 5-model stack, OPENROUTER_KEY env var, retry/backoff pattern
- [docs.rs/crate/goblin/latest](https://docs.rs/crate/goblin/latest) — version 0.10.5 (Feb 2026), Rust 2024 edition required — HIGH confidence
- [docs.rs/goblin/latest/goblin/pe/header/](https://docs.rs/goblin/latest/goblin/pe/header/index.html) — `DOS_MAGIC`, `PE_MAGIC`, `CoffHeader.time_date_stamp`, `COFF_MACHINE_X86_64` — HIGH confidence
- [openrouter.ai/docs/api/api-reference/api-keys/create-keys](https://openrouter.ai/docs/api/api-reference/api-keys/create-keys) — management key provisioning API format — HIGH confidence
- [TeamDay AI: Top OpenRouter Models March 2026](https://www.teamday.ai/blog/top-ai-models-openrouter-2026) — model pricing — MEDIUM confidence (third-party, use for estimates only; verify at openrouter.ai/models before budgeting)
- [reqwest::blocking docs](https://docs.rs/reqwest/latest/reqwest/blocking/index.html) — "must not execute within async runtime" constraint — HIGH confidence

---

*Stack research for: v31.0 Autonomous Survival System — 3-Layer MI Independence*
*Researched: 2026-03-30 IST*
