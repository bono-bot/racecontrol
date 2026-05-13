# §S-241 Agent-2 Class-Audit Findings: thiserror Display-impl PII-leak class

**Cascade:** §S-241 iter5
**Audit target class:** sibling instances of `cirs.rs:23-31` PII-leak pattern across racecontrol thiserror enums
**Scope:** AUDIT ONLY — no code modifications
**Date:** 2026-05-13 IST

## Summary

- **6 enums audited** (cirs.rs out-of-scope as Agent 1's cascade target)
- **17 total `#[error(...)]` variants** enumerated
- **0 HIGH-RISK-CONFIRMED** Variant-D PII-leak instances found
- **2 POTENTIAL-RISK** Variant-D variants found (FleetHealer SSH-error / IsolationFailed) — interpolated value is internal SSH stderr, NOT user-supplied request input; reach to HTTP response surface NOT confirmed
- **0 sibling thiserror enums discovered beyond the brief** (grep `thiserror::Error` returns exactly the 7 enums named in the brief; no additional sibling enums in `/root/racecontrol/crates/**/*.rs`)

Key finding: the cirs.rs leak class is **anomalous** in the racecontrol codebase. The other 6 thiserror enums either (a) interpolate structured numeric/enum fields (LOW-RISK), (b) interpolate static literals (NO-RISK), (c) wrapper-only via `#[from]` (delegated risk), or (d) interpolate internal-only data (SSH stderr, config TOML content, version strings — not user-supplied request input).

## Findings table

| File:Line | Variant Name | Display Pattern | Source-of-Interpolation | Reach (HTTP/WS/Log) | Risk Class | Recommended Structural Fix |
|---|---|---|---|---|---|---|
| `/root/racecontrol/crates/rc-process-manager/src/registry.rs:24` | `RegistryError::Io` | `#[error("registry file read error: {0}")]` | `#[from] std::io::Error` (wrapper-only) | Internal — `Registry::load_from_path` boot only; not consumed outside crate (grep found only re-export at `lib.rs:20`) | Variant A — NO-RISK (boot-time, no HTTP reach) | None — internal error, never reaches user surface |
| `/root/racecontrol/crates/rc-process-manager/src/registry.rs:27` | `RegistryError::Parse` | `#[error("registry TOML parse error: {0}")]` | `#[from] toml::de::Error` (wrapper-only) | Internal — boot config parse only | Variant A — NO-RISK | None |
| `/root/racecontrol/crates/rc-process-manager/src/registry.rs:30` | `RegistryError::SchemaVersion` | `#[error("schema_version mismatch: expected {expected}, got {found}")]` | Structured `u32` fields from on-disk TOML | Internal — boot panic only | Variant C — NO-RISK (numeric, internal) | None |
| `/root/racecontrol/crates/rc-process-manager/src/registry.rs:33` | `RegistryError::DuplicateEntry` | `#[error("duplicate entry for {0:?}")]` | `ManagedProcess` enum (build-time constant set) | Internal — boot only | Variant C — NO-RISK (closed enum) | None |
| `/root/racecontrol/crates/rc-process-manager/src/registry.rs:36` | `RegistryError::MissingEntry` | `#[error("no registry entry for {0:?}")]` | `ManagedProcess` enum | Internal — `Registry::entry()` not used in HTTP path | Variant C — NO-RISK | None |
| `/root/racecontrol/crates/rc-process-manager/src/manager.rs:10` | `ManagerError::NotImplemented` | `#[error("manager scaffold only — spawn/kill API lands in W3")]` | Static literal | W1 stub; never returned at runtime | Variant B — NO-RISK | None |
| `/root/racecontrol/crates/v2-db/src/lib.rs:23` | `Error::Sqlx` | `#[error("sqlx: {0}")]` | `#[from] sqlx::Error` (wrapper-only) | Reaches `cirs_lookup.rs:284` `CirsError::Sqlx(_)` arm which returns generic `internal_error` JSON without `format!("{e}")` interpolation (sibling Agent 1's code-path also redacts). DB pool boot at `lib.rs:32`. | Variant A — LOW-RISK (sqlx::Error Display CAN include bind values under some feature flags — see Out-of-scope note) | None for this audit; track as separate sqlx Display feature-flag audit class |
| `/root/racecontrol/crates/v2-db/src/lib.rs:26` | `Error::Migrate` | `#[error("migrate: {0}")]` | `#[from] sqlx::migrate::MigrateError` | Boot-only | Variant A — NO-RISK (boot, no user input) | None |
| `/root/racecontrol/crates/racecontrol/src/fleet_healer.rs:73` | `FleetHealerError::UnknownPod` | `#[error("Unknown pod number: {0}")]` | `u32` pod number (internal config / API path param) | `FleetHealerOrchestrator::heal_pod` is **NOT called from any HTTP handler** (only `survival_report_handler` and `fleet_healer_routes` use Json; orchestrator is dead/internal code-path per grep). Used in `fleet_healer_diagnosis.rs:58`. | Variant C — NO-RISK (numeric, no current HTTP reach) | None |
| `/root/racecontrol/crates/racecontrol/src/fleet_healer.rs:76` | `FleetHealerError::SshTimeout` | `#[error("SSH timeout on {pod_id} after {timeout_secs}s")]` | `pod_id: String` (operator-supplied / config), `timeout_secs: u64` | Internal — orchestrator currently has no HTTP surface | Variant C — NO-RISK (pod_id is fleet-internal identifier, not PII) | None |
| `/root/racecontrol/crates/racecontrol/src/fleet_healer.rs:79` | `FleetHealerError::SshExecFailed` | `#[error("SSH execution failed on {pod_id}: {error}")]` | `pod_id` + `error: String` from `e.to_string()` of remote SSH stderr (`fleet_healer_diagnosis.rs:80`) | Internal — log + audit trail only via `fleet_healer_repair.rs:141 error: Some(e.to_string())`; no current HTTP response reach | Variant D — **POTENTIAL-RISK** (SSH stderr may contain hostnames / paths / pod-internal data; NOT user-supplied request input; reach to external surface NOT confirmed) | If a future HTTP handler returns this error via `format!("{e}")`, redact `{error}` field. Track as deferred-risk; no fix required this iter. |
| `/root/racecontrol/crates/racecontrol/src/fleet_healer.rs:82` | `FleetHealerError::IsolationFailed` | `#[error("Pod isolation failed on {pod_id}: {error}")]` | Same as SshExecFailed | Internal — `fleet_healer_repair.rs:322` constructs; no HTTP return path | Variant D — **POTENTIAL-RISK** (same class as SshExecFailed) | Same as above |
| `/root/racecontrol/crates/racecontrol/src/fleet_healer.rs:85` | `FleetHealerError::BillingActive` | `#[error("Billing session active on {pod_id} — repair not permitted")]` | `pod_id: String` (fleet-internal) | Internal | Variant C — NO-RISK | None |
| `/root/racecontrol/crates/racecontrol/src/fleet_healer.rs:88` | `FleetHealerError::CommandBlocked` | `#[error("SSH command blocked on {pod_id}: {reason}")]` | `pod_id` + `reason: String` (internal SSH allowlist deny-reason constant, `fleet_healer_diagnosis.rs:51`) | Internal | Variant C — NO-RISK (`reason` is from internal constants, not user input) | None |
| `/root/racecontrol/crates/rc-common/src/verification.rs:28` | `VerificationError::InputParseError` | `#[error("input parse failed at step '{step}': raw value = {raw_value}")]` | `step` (internal step name) + `raw_value: String` — sourced from `config/mod.rs:208` (TOML config content), `rc-agent/src/config.rs:433` (agent config TOML), `pod_healer_diagnostics.rs:28` (internal status strings), `process_guard.rs:63` (machine whitelist). **NOT** sourced from HTTP request bodies. | Boot-time config validation + diagnostic ring buffer; logged at WARN. No HTTP response surface returns `VerificationError` via `format!("{e}")`. | Variant D — **LOW-RISK** (raw_value IS user-supplied in the broad sense — boot config file content — but boot-only; no per-request user input flows into this variant) | None — boot-time errors are operator-visible by design; not a PII class |
| `/root/racecontrol/crates/rc-common/src/verification.rs:31` | `VerificationError::TransformError` | `#[error("transform failed at step '{step}': raw value = {raw_value}")]` | Same class as InputParseError (boot config) | Same as above | Variant D — LOW-RISK | None |
| `/root/racecontrol/crates/rc-common/src/verification.rs:34` | `VerificationError::DecisionError` | `#[error("decision failed at step '{step}': raw value = {raw_value}")]` | `process_guard.rs:90` whitelist decision-step input (internal pod state) | Same as above | Variant D — LOW-RISK | None |
| `/root/racecontrol/crates/rc-common/src/verification.rs:37` | `VerificationError::ActionError` | `#[error("action failed at step '{step}': raw value = {raw_value}")]` | `rc-sentry/src/tier1_fixes.rs:335` action result (internal) | Same as above | Variant D — LOW-RISK | None |
| `/root/racecontrol/crates/rc-common/src/survival_types.rs:372` | `DiagnosisError::BudgetExhausted` | `#[error("budget exhausted: daily limit ${0:.2} reached")]` | `f64` budget config value | **Unused outside definition** — grep `DiagnosisError` returns zero hits outside survival_types.rs | Variant C — NO-RISK (numeric, unused) | None |
| `/root/racecontrol/crates/rc-common/src/survival_types.rs:374` | `DiagnosisError::ApiUnreachable` | `#[error("api unreachable after {0} attempts")]` | `u32` retry count | Unused | Variant C — NO-RISK | None |
| `/root/racecontrol/crates/rc-common/src/survival_types.rs:376` | `DiagnosisError::Timeout` | `#[error("diagnosis timeout after {0}s")]` | `u64` seconds | Unused | Variant C — NO-RISK | None |
| `/root/racecontrol/crates/rc-common/src/survival_types.rs:378` | `DiagnosisError::Other` | `#[error("{0}")]` | `String` — completely open passthrough | Unused (dead code currently) | Variant D — **POTENTIAL-RISK if revived** (`Other(String)` is the same pattern shape as `CirsError::InvalidPhone(String)`; if a future caller constructs `DiagnosisError::Other(user_input)` and reach-paths to HTTP, this is a new leak class) | If revived: redact or hash-prefix; add caller-site contract that `Other` must not carry user-supplied request data |

## Out-of-scope notes

- **`v2-db::Error::Sqlx` (`#[from] sqlx::Error`)**: `sqlx::Error` Display impl may include parameter bind values when compiled with certain feature flags (e.g. `log-statements`). Separate audit class — not addressed in this iter. The cirs.rs Agent-1 fix at `cirs_lookup.rs:288` `format!("{e}")` reach is for `CirsError::InvalidPhone/AmbiguousPhone`, not for `CirsError::Sqlx(_)` which the existing match arm at `cirs_lookup.rs:284` already handles separately with a generic `internal_error` JSON response (no Display interpolation).
- **`#[error("{0:?}")]` Debug-format vs Display**: `RegistryError::DuplicateEntry/MissingEntry` use `{0:?}` which uses Debug not Display. `ManagedProcess` enum is build-time constant set (no user input). NO-RISK.
- **`SurvivalReport` JSON echo at `fleet_healer.rs:119`**: `survival_report_handler` deserializes `SurvivalReport` from request body and writes the full JSON back into audit log via `serde_json::to_string(&report)` — separate audit class (full request body echoed to audit-log, not Display-impl class). Out-of-scope for this iter; flag for future "request-body audit-log echo" class audit.
- **`heal_pod_diagnostics.rs` / `pod_healer_diagnostics.rs`**: these use `VerificationError` but for internal pod-status string parsing (e.g. `"200"` from curl output) — not customer-facing PII. LOW-RISK already covered above.

## Recommended next steps

1. **No iter5 cascade fix required for this class** — sibling-instance audit returns 0 HIGH-RISK-CONFIRMED. Agent 1's `cirs.rs` fix is the singular instance of the request-body PII-leak class in the current codebase.
2. **Defensive recommendation (deferred-low-priority):** add a `// SAFETY: do not interpolate request-body data` comment-contract above `DiagnosisError::Other(String)` (`survival_types.rs:378`) and any future `String`-payload thiserror variant. Pattern-prevention rather than current-fix.
3. **Track 2 POTENTIAL-RISK items** for next cascade iteration: `FleetHealerError::SshExecFailed` + `FleetHealerError::IsolationFailed` — if the `FleetHealerOrchestrator` HTTP wire-up lands in a future phase, audit the response-body construction site at that time to ensure the `error: String` field (which carries SSH stderr) is NOT echoed via `format!("{e}")` into a JSON response.
4. **Separate "sqlx::Error Display feature-flag" audit class** for v2-db::Error::Sqlx — confirm sqlx crate is not built with `log-statements` or similar that would surface bind values in Display output. Defer to dedicated audit.
5. **Separate "request-body audit-log echo" audit class** for `survival_report_handler` JSON echo at `fleet_healer.rs:119` — out-of-scope for thiserror class but worth a sibling-cascade.

**Estimated diff size if all (currently zero) HIGH-RISK fixes were applied:** 0 LOC.

**Estimated diff size for defensive comment-contracts (recommendation #2):** ~5 LOC across 1 file (`survival_types.rs`).
