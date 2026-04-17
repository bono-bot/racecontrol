# Phase 413: Service key provisioning + deploy-server.sh hardening - Context

**Gathered:** 2026-04-18
**Status:** Ready for planning
**Source:** Session discussion (conversational context-capture, not formal discuss-phase)

<domain>
## Phase Boundary

**In scope:**
- Eliminate manual HKLM `RCAGENT_SERVICE_KEY` provisioning on pods (8 pods + POS)
- Fix 3 concrete factors in `deploy-server.sh` that caused the 2026-04-18 03:13 IST deploy abort
- Audit remaining manual-state service-key provisioning paths on pods

**Out of scope:**
- Changing the mesh service-key rotation cadence or lifecycle
- Redesigning `rc-sentry` /exec auth (uses `SENTRY_KEY` — different concern, user-managed)
- Cloud (Bono VPS) deploy script parity — separate concern, follow-up phase
- Any changes to `pods.sentry_service_key` value itself — just how it's propagated

**Hard deliverable:** rc-agent's `check_audit_known_issues()` at [ai_debugger.rs:778](crates/rc-agent/src/ai_debugger.rs#L778) returns `Some(match)` on a real crash whose symptoms are in `audit_known_issues` (seeded 2026-04-18 with 42 entries). Today it returns None fleet-wide because of Gap 4 (pod HKLM key ≠ server TOML key).
</domain>

<decisions>
## Implementation Decisions (locked by user this session)

### Option Z chosen — fetch-at-boot from server (not Options X or Y)

User explicitly rejected Option X (manual `setx /M` script) and Option Y (separate `MESH_SERVICE_KEY` field) in favor of Option Z because:
- X is not permanent — manual state drifts, gets wiped by reimage, requires per-pod re-execution
- Y duplicates config surface unnecessarily
- Z reuses the existing `rc_common::boot_resilience::spawn_periodic_refetch` pattern already used for `feature_flags` ([main.rs:1604](crates/rc-agent/src/main.rs#L1604)) and the process-guard allowlist ([commit 821c3031](../../../..)). Key lives only in server TOML; survives key rotation; no per-pod provisioning.

### Bootstrap auth = pod-IP allowlist via `network_middleware`

Pods fetching the key don't have it yet (that's the point). Bootstrap can't use a credential the pod doesn't have. Solution: gate the new fetch route by source IP via existing `network_middleware` (already in place per SEC-06 per MEMORY.md: "Network middleware (pod IP classification, 403 enforcement)"). LAN + pod-IP range is the trust boundary.

**Acceptable because** allowlist fetches today (`/api/v1/config/kiosk-allowlist`) are already LAN-gated with the same pattern. Mesh key is not higher sensitivity than that.

### Key fetch cache strategy

- **Type:** `Arc<RwLock<Option<String>>>` in rc-agent global state (mirrors how feature_flags is shared).
- **Lifetime:** fetched at boot; refreshed every 5 minutes via `spawn_periodic_refetch`.
- **Failure behavior:** on fetch failure, keep last-known-good value. On cold boot with failed fetch, cache stays None — consumers bail to current "no key" behavior (same as today's env-var-unset case). Self-heals on next tick.
- **Consumers:** all 3 read the cache; on None, fall back to env var for test compatibility.

### Consumers to rewire (3 total)

| Consumer | File | Line | Today | After |
|---|---|---|---|---|
| Tier 0 mesh oracle | `crates/rc-agent/src/ai_debugger.rs` | 779 | `std::env::var("RCAGENT_SERVICE_KEY")` | cache.read() → fall back to env |
| `require_service_key` middleware | `crates/rc-agent/src/remote_ops.rs` | 165 | `std::env::var("RCAGENT_SERVICE_KEY")` | cache.read() → fall back to env |
| csv_lap_fallback push | `crates/rc-agent/src/ws_handler.rs` | 431 | `std::env::var("RCAGENT_SERVICE_KEY")` | cache.read() → fall back to env |

The env fallback matters only for unit tests (which already `set_var("RCAGENT_SERVICE_KEY", "test-secret-key")` — see `remote_ops.rs:1898` and below). Production env is unset; cache is the source of truth.

### deploy-server.sh — 3 concrete factor fixes

All three were identified from live evidence on 2026-04-18 03:13 IST abort. Each has a minimal diff.

**Factor 1: Extend schtasks disable list.** Current list at [deploy-server.sh:205](scripts/deploy-server.sh#L205): `{StartRCOnBoot, StartRCTemp}`. Extended list: add `RCWatchdog`, `RaceControlStartup`, `StartRCDirect`, `StartRaceControl`, `StartRCWatchdog`, `StartFrontendWatchdog`. Evidence: `schtasks /Query /FO CSV` on server .23 returned 8 RC-related tasks; `RCWatchdog` fired during the aborted deploy at 03:13:00. Must also be re-enabled in the success path (line ~260) AND in the rollback block (line ~327).

**Factor 2: Unify deploy sentinel on `OTA_DEPLOYING`.** Current: `deploy-server.sh:210` writes `DEPLOY_IN_PROGRESS`; `start-racecontrol-watchdog.ps1:61` checks `OTA_DEPLOYING`. Fix: change the write side to `OTA_DEPLOYING` (the PS watchdog is the more durable convention — it ships as part of the runtime). Also update the delete lines in the re-enable block and rollback block.

**Factor 3: Replace WINDOWTITLE filter with WMIC commandline match.** Current: `taskkill /F /IM powershell.exe /FI "WINDOWTITLE eq *watchdog*"`. Problem: `start-racecontrol.bat:26` launches PS via `start "" /B powershell ...` — empty `""` leaves window title unset, filter misses. Fix: `wmic process where "name='powershell.exe' and commandline like '%start-racecontrol-watchdog.ps1%'" delete`.

### MMA audit is mandatory before deploy

Per CLAUDE.md standing rule ("MMA audit is MANDATORY for new cross-system bridges"). This phase crosses ≥2 system boundaries (server route + rc-agent + pod network middleware). Must run 5-model MMA with ≥3 vendor families. Budget: $5. Block deploy until score ≥4.0.

### POS gets same treatment as pods

POS (.130) also runs rc-agent (confirmed this session, PID 6308). Deploy the new rc-agent binary to POS + server too. POS LAN IP must be on `network_middleware` pod-IP allowlist. Verify LAN-IP classification list includes 192.168.31.130.

### Scope boundary on key rotation mechanism

This phase does NOT change how keys are ROTATED — only how they are PROPAGATED. Rotation today = edit `C:\RacingPoint\racecontrol.toml` + restart racecontrol. After this phase = same, but pods self-heal within 5 min instead of needing manual HKLM update on each pod.

### Claude's Discretion

- Exact route path name (`/api/v1/pods/mesh-service-key` vs `/api/v1/config/mesh-service-key`) — pick the path that groups with existing pod-fetch patterns; match whatever convention `/config/kiosk-allowlist` follows.
- Cache module location (`crates/rc-agent/src/mesh_key_cache.rs` vs expanding an existing config module) — planner's call.
- Whether to rename the env var (it's legacy-named `RCAGENT_SERVICE_KEY`; the cache represents the same value) — keep the name for test compatibility unless there's a strong reason.
- Test structure — whatever mirrors `feature_flags::FeatureFlags::fetch_from_server` tests most closely.
</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Boot resilience pattern (the template to copy)
- `crates/rc-common/src/boot_resilience.rs` — `spawn_periodic_refetch` — full impl + 3 unit tests showing the pattern
- `crates/rc-agent/src/main.rs` around line 1595-1617 — existing feature_flags use of `spawn_periodic_refetch`, shows boot-time wire-up

### Consumer sites (to rewire)
- `crates/rc-agent/src/ai_debugger.rs:773-804` — `check_audit_known_issues()`, including the comment at 775-777 documenting the original MMA-v29 gap
- `crates/rc-agent/src/remote_ops.rs:157-177` — `require_service_key` middleware with constant-time comparison (keep that property)
- `crates/rc-agent/src/remote_ops.rs:1895-2010` — existing tests for `require_service_key` — MUST stay green after refactor
- `crates/rc-agent/src/ws_handler.rs:426-440` — csv_lap_fallback service-key use (comment at 430 says "matches sentry_service_key on server" — that's the intent this phase enforces)

### Server route pattern
- `crates/racecontrol/src/api/routes.rs` — find where `/config/kiosk-allowlist` and `/guard/whitelist/pod-{N}` are registered (both in `public_routes` per MEMORY.md). New route goes next to them.
- `crates/racecontrol/src/api/mesh_intelligence.rs:297-345` — `mesh_audit_check_service` / `mesh_audit_seed_service` handlers — these show the `X-Service-Key` validation pattern but more importantly show how service-key-gated routes are registered. The NEW route is NOT service-key-gated — it's pod-IP-gated — so it goes elsewhere in routes.rs.

### Network middleware (pod-IP gating)
- Search `crates/racecontrol/src/api/` for `network_middleware` or pod-IP classification. Must understand what list of IPs is trusted today. 192.168.31.0/24 at minimum; pod-specific entries likely.

### deploy-server.sh
- `scripts/deploy-server.sh` — full file. Read end-to-end before editing. Factor 1 touches lines ~197-212 (disable block), ~255-262 (re-enable in success path), ~322-332 (rollback path). Factor 2 touches the same 3 locations. Factor 3 touches only the disable block.
- `scripts/deploy/start-racecontrol-watchdog.ps1:61` — the sentinel name that Factor 2 unifies on

### Evidence (session findings)
- `~/.claude/projects/C--Users-bono/memory/session_handoff_20260418_mi_seed_verification_and_gap4.md` — full evidence trail for Gap 4 + 3 deploy factors
- `~/.claude/projects/C--Users-bono/memory/session_handoff_20260418_mi_seed_short_token.md` — earlier gaps 1-3 + where `5f80fc6a` deployed

### Standing rules
- Root `CLAUDE.md` Standing Rules section — especially:
  - Rebuild + redeploy after functional code commits — ALL apps, not just Rust
  - Cross-Process Updates — changing a feature? Update ALL: rc-agent, racecontrol, etc.
  - MMA audit is MANDATORY for new cross-system bridges
  - No `.unwrap()` in production Rust
  - Boot Resilience: No single-fetch-at-boot without retry (this phase upholds that)
</canonical_refs>

<specifics>
## Specific Ideas

### Deploy target list (enumerate fleet-wide)

Per CGP Gate H4, any fleet-wide claim needs per-target enumeration. Deploy targets for the rc-agent binary change:
- Server .23 (hosts racecontrol server — gets server binary change)
- Pods 1-8 (192.168.31.33/28/38/86/87/88/89/91 — all get rc-agent binary)
- POS .130 (also runs rc-agent)
- Cloud (Bono VPS) — server binary change deploys there too per DEPLOY PARITY rule

### Success test (goal-backward verification)

The one test that proves this phase's goal:
1. Pick any pod (e.g., pod 3)
2. Ensure HKLM `RCAGENT_SERVICE_KEY` is deleted (`reg delete ... /v RCAGENT_SERVICE_KEY /f`) — prove we no longer depend on it
3. Restart rc-agent (let it boot with no env var)
4. Trigger a known crash pattern (e.g., AC launch with missing python.ini — matches seeded ZL-2)
5. Observe `check_audit_known_issues()` log the `AUDIT KNOWN ISSUE matched` line with ZL-2

### Pre-flight for POS IP classification

Before merging: grep `network_middleware` (or whatever classifier) to confirm `192.168.31.130` (POS) is on the pod allowlist. If not, add it in the SAME phase — otherwise POS rc-agent will 403 the fetch and stay Tier-0-dark.

### Unit test for cache fallback to env

The Option Z cache + env-fallback pattern needs a test:
- Set env, cache empty → function returns env value
- Set env, cache has value → function returns cache value (cache wins)
- Clear env, cache empty → function returns None (both consumers must handle gracefully)

Mirror structure of `test_service_key_permissive_mode_no_key_set` at `remote_ops.rs:1949`.
</specifics>

<deferred>
## Deferred Ideas

### Deferred from this phase to future work

- **Cloud (Bono VPS) key parity** — cloud has its own `racecontrol.toml`, its own `pods.sentry_service_key`, its own rc-agent consumers (if any). The phase targets venue fleet. Cloud gets the code change via DEPLOY PARITY rule but cloud-side rc-agent consumers aren't in scope to audit.
- **Remove RCAGENT_SERVICE_KEY env var entirely** — keeping env as fallback for test compatibility this phase. A follow-up phase can remove the env path after a few weeks of production cache use.
- **rc-sentry service key** — uses a similar pattern but is deploy-tool-facing (user sets `SENTRY_KEY` env before running deploy scripts). Not in scope. If the user later wants unified key management, that's a separate phase.
- **Key rotation cadence** — currently manual via TOML edit. Could move to automated rotation in a future phase, but that's a bigger design change.
- **MMA audit of the Option Z design before code** — handled inside this phase's execution per the MMA mandate; NOT a deferred item.

### Explicitly NOT this phase

- Changing `audit_known_issues` schema, match semantics, or pattern specificity ordering (the GLC-1 vs INV-9 quirk is noted for separate follow-up)
- Changing how the seeder runs (seeder works fine; 42/42 verified this session)
- Cloud seed — separate decision pending with Uday per INBOX entry
</deferred>

---

*Phase: 413-service-key-provisioning-deploy-server-sh-hardening-option-z-respawn-race-fixes*
*Context gathered: 2026-04-18 via session discussion (no formal discuss-phase)*
