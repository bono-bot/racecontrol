# §S-146 RCA — Silent `deny_unknown_fields` fallback class on cross-boundary game-launch contract

**Status:** DRAFT-PENDING-MMA-STEP-1-CAPTAIN-AUTH
**Authored:** james · 2026-05-16 ~12:55 IST (07:25 UTC)
**Anchor:** §S-388 (this session close-anchor) · MAOR datapoint #2 Tier-2 WSH-1 + non-AC sim sibling exposure
**Class:** §S-146 V1↔V2 RCA · foundational-boundary (cross-boundary serialization contract · kiosk↔rc-agent↔racecontrol seam) · MMA Step 1 DIAGNOSE required
**Captain framing:** Game Launching = MOST IMPORTANT V2 feature (Captain 2026-05-16 12:41 IST)
**Composes-with:** §S-146 parent · CLAUDE.md "Cross-Boundary Serialization" standing rule (Phase 62 / MI gap P2 historical anchor) · `feedback_v1_v2_gap_flow_plan_20260514.md` doctrine B1 (substrate-pointer convention extends to services)

---

## §1 — Boundary map

The cross-boundary serialization contract spans **kiosk JSON → server relay → WS protocol → rc-agent deserialization → ac_launcher consumption**. The `deny_unknown_fields` rejection point is at the agent-side deserialization step. The silent fallback class **neutralizes the contract** by catching the rejection and substituting hardcoded defaults.

### Primary site — `crates/rc-agent/src/ws_handler.rs:684-711`

```rust
let params: ac_launcher::AcLaunchParams = match &launch_args {
    Some(args) => serde_json::from_str(args).unwrap_or_else(|e| {
        tracing::warn!(target: LOG_TARGET, "Failed to parse AC launch_args, using defaults (car=ks_ferrari_sf15t, track=spa): {}", e);
        ac_launcher::AcLaunchParams {
            car: "ks_ferrari_sf15t".to_string(), track: "spa".to_string(),
            // ... 20+ hardcoded defaults ...
            ai_level: 87, session_type: "practice".to_string(), ai_cars: Vec::new(),
            // ...
        }
    }),
    None => ac_launcher::AcLaunchParams { /* same defaults */ },
};
```

**Contract anchor:** `crates/rc-agent/src/ac_launcher.rs:123-133`:

```rust
/// `#[serde(deny_unknown_fields)]` is enforced to fail fast on field name drift
/// ...
/// `test_deny_unknown_fields_rejects_drift` verifies this.
#[serde(deny_unknown_fields)]
pub struct AcLaunchParams { ... }
```

Plus `crates/rc-agent/src/ac_launcher.rs:2473-2481` — Phase 62 / MI gap P2 contract test:

```rust
/// Phase 62 / MI gap P2: deny_unknown_fields contract test.
#[test]
fn test_deny_unknown_fields_rejects_drift() {
    let json = ...; // contains ai_difficulty (typo for ai_level)
    let result: Result<AcLaunchParams, _> = serde_json::from_str(json);
    assert!(result.is_err(), "deny_unknown_fields must reject ai_difficulty (typo for ai_level)");
}
```

**The contract:** unknown-field rejection MUST fail-fast to surface kiosk↔agent drift.
**The breach:** `unwrap_or_else` catches the rejection silently and substitutes Ferrari SF15T on Spa.

### Sibling exposure — non-AC sim paths · `crates/rc-agent/src/ws_handler.rs:866-1198`

For F1 25 / iRacing / LMU / Forza / ACE / ACR / ForzaHorizon5: `launch_args` is assigned directly to `game_config.args` as `Option<String>` (L879) — **NO sim-specific typed struct deserialization**, **NO `deny_unknown_fields` enforcement**, **NO field-validation**. Raw string forwarded as-is to `game_process::GameProcess::launch()`. Whether downstream parsers in each sim's launcher apply contract discipline is per-sim and NOT validated at the WS-handler boundary.

**ACE specific** — `crates/rc-agent/src/ws_handler.rs:966-983` calls `write_evo_config(&evo_args, &dir)` with raw String. Whether AC EVO's config writer applies `deny_unknown_fields` is in `crates/rc-agent/src/sims/assetto_corsa_evo.rs` — adapter file does NOT use serde (confirmed by expanded MAOR review). The config-write path is in a separate module not in this RCA's scope.

### Cross-boundary chain

```
kiosk (TypeScript) — buildLaunchArgs() composes JSON object with field names
       ↓ HTTP POST /api/v1/games/launch with launch_args field
racecontrol (server) — game_launch.rs forwards launch_args as Value to WS
       ↓ CoreToAgentMessage::LaunchGame { launch_args: Option<String>, ... }
rc-agent (pod) — ws_handler.rs:684-711 attempts AcLaunchParams::from_str
       ↓ on serde error → silent fallback to ks_ferrari_sf15t/spa defaults
ac_launcher.rs — launch_ac(&params) writes race.ini with WHATEVER params it received
```

**Failure mode:** kiosk adds a new field (e.g. `weather_intensity`) or drifts an existing one (e.g. `ai_difficulty` instead of `ai_level`); contract test `test_deny_unknown_fields_rejects_drift` PASSES on the agent side because the test directly invokes the deserializer; PRODUCTION path catches the rejection and silently launches Spa+SF15T. Customer is billed for their selection, gets unrelated config.

---

## §2 — Inherited-issue catalogue

### V1 failure-mode anchor — Phase 62 / MI gap P2 + CLAUDE.md "Cross-Boundary Serialization"

> *"2026-03-26 — Two critical bugs: (1) kiosk sent `ai_difficulty: "easy"` (string) but agent expected `ai_level: u32` (numeric). AI was always Semi-Pro. (2) kiosk sent `ai_count: 5` but agent expected `ai_cars: Vec<AiCarSlot>`. Zero AI opponents appeared. Both undetected because game launched successfully and no error was logged anywhere. Audit Protocol Phase 62 added to catch this class of bug."*

The contract test was added **explicitly** to catch this class. The contract test PASSES in CI. The production path NEUTRALIZES the contract via `unwrap_or_else`. The defense-in-depth was structurally undermined.

### V1 process-mess catalogue (categories touched)

Per `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md`:
- **Category A** (audit-blind proxy checking) — direct hit. WARN log without error-broadcast IS proxy-class signal; ground-truth (customer got wrong car/track) is invisible to the server.
- **Category D** (cross-process schema drift) — direct hit. Kiosk and agent compile from different module versions during rolling deploy windows; the contract test is the schema-drift guard; silent fallback defeats it.
- **Category F** (silent-fallback class) — parent class. `unwrap_or_default` / `unwrap_or_else` / `?` with `.ok()` are the canonical anti-patterns.

### Past-bug catalogue at this boundary (60-day window)

| Anchor | Class | Disposition |
|---|---|---|
| 2026-03-26 `ai_difficulty`→`ai_level` (string vs u32) | identical pattern | PATCHED-ONLY — contract test added; production silent-fallback path NOT removed |
| 2026-03-26 `ai_count`→`ai_cars` (count vs Vec) | identical pattern | PATCHED-ONLY — same as above |
| WSH-3 billing_rates/game_limits ConfigPush silent no-op (ws_handler.rs:1756-1792) | sibling — `accepted: true` on no-op | UNRESOLVED — same fail-silent class on different surface |
| WSH-5 StopGame `game.stop()` failure unreported (ws_handler.rs:1259) | sibling — failure logged not broadcast | UNRESOLVED — same class on stop surface |
| §S-220 first-MAOR PII fixture in tests (real-looking `9876543210`) | sibling class — defense doctrine cited but violated | ROOT-CAUSED-AND-FIXED in §S-220 |
| GLO-1 stale TODO "no trial concept" (Tier-1 finding) | sibling — code says one thing, comment says another | UNRESOLVED — separate item |

### Phase 62 contract test efficacy gap

The test at `ac_launcher.rs:2477` invokes `serde_json::from_str::<AcLaunchParams>` and asserts `result.is_err()`. The test verifies the deserializer rejects unknown fields. The test does NOT verify the **production code path** invokes the deserializer in a way that surfaces the rejection. The `unwrap_or_else` swallows the rejection at the call site — exactly the path the test does not cover. **Test coverage is necessary but not sufficient when the production wrapper has its own error-handling that defeats the contract.**

---

## §3 — Past-bug review (per-site disposition)

### Site A (ws_handler.rs:684-711 AC AcLaunchParams) — **NEVER-PATCHED-IN-PRODUCTION · CRITICAL**

- Contract test was added in Phase 62. Production silent-fallback shipped same era.
- WARN-level log exists but is observability-only, not actionable.
- No `GameStateUpdate::Error` sent to server → server has no idea the launch ran wrong config.
- Customer billed for selection, gets defaults.

**Disposition:** UNRESOLVED. Contract intact in test; defeated in production.

### Site B (non-AC sims · raw String pass-through) — **STRUCTURAL · IMPORTANT**

- No typed struct deserialization at WS boundary → no `deny_unknown_fields` contract to enforce.
- Per-sim downstream parsers may or may not enforce contracts.
- Silent-failure class exists by absence-of-contract rather than by contract-defeat.

**Disposition:** UNRESOLVED — per-sim audit needed; out-of-scope for primary fix but flagged for sibling RCA.

### Site C (ACE config write · write_evo_config) — **DEFERRED · NOTE**

- Cross-reference into `crates/rc-agent/src/sims/assetto_corsa_evo.rs` confirms adapter does NOT use serde — no `deny_unknown_fields` available there.
- Config write path in separate module (not in RCA scope).

**Disposition:** DEFERRED to separate RCA on ACE config-write contract class.

---

## §4 — V2-alignment delta

V2 doctrine for cross-boundary contracts (per V2-MASTER-STATE §S-N + customer workflows + Wallet Framing C precedent):

1. **Contract test ≠ production-path test.** Phase 62 contract test verified the deserializer; the production code path must INVOKE the contract in a way that surfaces the rejection. V2 must enforce **production-path contract tests** — tests that exercise the actual code path from WS-receive → params struct → ac_launcher invocation, with assertion that unknown-field WS payload produces server-observable error.

2. **`unwrap_or_else` with hardcoded defaults is a V1-anti-pattern.** Per CLAUDE.md "Anti-patterns blocked" + Cross-Boundary Serialization rule. V2 substrate convention: cross-boundary deserialization failure MUST propagate as typed error to caller; caller decides disposition (retry / error / abort). No silent defaulting.

3. **Substrate-pointer convention (B1 doctrine flip, 2026-05-14):** the `deny_unknown_fields` annotation on `AcLaunchParams` IS the canonical contract. CLAUDE.md should annotate the launch-params surface with `(canonical: crates/rc-agent/src/ac_launcher.rs:132 AcLaunchParams)` so derived artifacts don't drift the field list.

4. **Per-sim contract parity.** All sim-specific `LaunchParams` structs should follow the same `#[serde(deny_unknown_fields)]` discipline. Sibling exposure (Site B non-AC sims) reveals V1 substrate gap: raw String pass-through is V1-shaped, V2 requires typed contracts at every cross-process seam.

5. **Foundation/strategy/config separation:** the contract-enforcement IS foundation-class. Currently it sits at the strategy layer (per-sim handler arm decides whether to enforce). V2 alignment: hoist contract enforcement to foundation (single helper `parse_launch_params<T: DeserializeOwned + DenyUnknownFields>(args)` that returns typed error; strategy layer dispatches on error class).

**Substrate-pointer:** Canonical cross-boundary contract = `AcLaunchParams` at `crates/rc-agent/src/ac_launcher.rs:132` + Phase 62 contract test at `:2473-2481`. Production wrapper at `crates/rc-agent/src/ws_handler.rs:684-711` IS the violation site. CLAUDE.md "Cross-Boundary Serialization" rule IS the doctrine; production path bypasses it.

---

## §5 — V2-framed proposal (NOT autonomously executed — MMA Step 1 + Captain auth required)

### Stage 1 — Mechanism trust check (5 questions)

1. **Atomic primitives?** — Single-PR scope on `ws_handler.rs:684-711` replacement. **YES.**
2. **TTL-bounded sentinels?** — N/A.
3. **Behavioral-verify success?** — Behavior = "kiosk JSON with unknown field produces `GameStateUpdate::Error` reaching server within 5s, AND no acs.exe spawn". Test from real kiosk browser, NOT curl from James .27. **NEEDS-HARNESS-AT-KIOSK.**
4. **Single-target dry-run?** — Pod 8 canary. **YES.**
5. **Guard contracts?** — `clippy::result_map_unit_fn` or custom lint on `unwrap_or_else` returning hardcoded defaults — not standard lint. **NO — hook-enforcement candidate** (`pre-bash-detect-silent-fallback-on-deny-unknown-fields.js` design class).

### Stage 2 — MMA Step 1 DIAGNOSE (Captain auth verb needed)

5-model OpenRouter consensus. Captain verb form:
*"James, authorize MMA Step 1 DIAGNOSE on the deny_unknown_fields silent-fallback RCA · 5-model OpenRouter · $5 budget · output to .planning/specs/v2/RCA/game-launch/MMA-STEP-1-CONSENSUS-DENY-UNKNOWN.md"*

### Stage 3 — Fix proposal sketch (subject to MMA consensus)

**Primary site fix:**
Replace `unwrap_or_else` with explicit error propagation. Mirror the pattern at L638-656 (Game Doctor failure path):

```rust
let params: ac_launcher::AcLaunchParams = match &launch_args {
    Some(args) => match serde_json::from_str(args) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(target: LOG_TARGET, "AC launch_args rejected by deny_unknown_fields (field drift?): {}", e);
            let error_info = GameLaunchInfo {
                pod_id: state.pod_id.clone(),
                sim_type: launch_sim,
                game_state: GameState::Error,
                error_message: Some(format!("launch_args contract violation: {}", e)),
                /* ... */
            };
            if let Ok(json_str) = serde_json::to_string(&AgentMessage::GameStateUpdate(error_info)) {
                let _ = ws_tx.send(Message::Text(json_str.into())).await;
            }
            return Ok(HandleResult::Continue);
        }
    },
    None => {
        // Explicit None case — server bug or test, not field-drift. Decide: error or accept defaults?
        // V2 disposition: error. Server MUST always send launch_args for AC.
        tracing::error!(target: LOG_TARGET, "AC LaunchGame missing launch_args");
        // emit Error GameStateUpdate ... return
    }
};
```

**Sibling fix (non-AC sims) — DEFERRED to per-sim RCA:**
Each non-AC sim's `LaunchParams` (or equivalent) needs `deny_unknown_fields` + production-path enforcement. Scope = separate RCA per sim. Foundational helper `parse_launch_params<T>()` would land in `rc-common` and centralize enforcement.

**Production-path contract test:**
Add integration test that sends a `LaunchGame` WS message with an unknown field and asserts the agent responds with `GameStateUpdate::Error` (not silent fallback + Running).

### Stage 4 — V2-doctrine alignment statement (required on PR per V1↔V2 RCA rule)

`V2 doctrine alignment: Cross-Boundary Serialization standing rule (CLAUDE.md) + Phase 62 contract test efficacy gap closed via production-path test · substrate-pointer convention applied to AcLaunchParams (canonical: ac_launcher.rs:132) · foundation/strategy separation (§AMEND-3.II D12) by hoisting deserialization to foundation helper.`

### Verify-by

- V-RCA2-1: Send WS LaunchGame with payload `{ ..., ai_difficulty: "easy" }` (unknown field) → agent emits `GameStateUpdate::Error` within 5s; acs.exe NOT spawned. Test from real kiosk-class JSON, not crafted Rust unit test.
- V-RCA2-2: Contract test `test_deny_unknown_fields_rejects_drift` still passes (existing behavior preserved).
- V-RCA2-3: New integration test `test_production_path_rejects_unknown_field` covers the WS→ws_handler→error-broadcast path.
- V-RCA2-4: Server-side dashboard receives the Error update with structured error_message including the field-drift detail.
- V-RCA2-5: Server .23 + Pod 8 canary verify; fleet rollout under §S-345 soak-in-parallel.

### NOT in scope (deferred to sibling RCAs/items)

- Non-AC sims raw-String pass-through (Site B · separate per-sim RCA · ~6 sibling RCAs needed)
- ACE config-write contract (Site C · separate ACE RCA)
- WSH-3 billing_rates/game_limits ConfigPush silent no-op (separate cross-boundary RCA)
- WSH-5 StopGame failure unreported (separate fix · same fail-silent family)
- General hook design for `unwrap_or_else`-with-defaults detection (composes-with §S-146 enforcement RCA)

---

## §6 — Stale-at + closure tracking

**Stale-at:** 2026-08-16 (90d) OR MMA Step 1 fires OR Captain disposition on Stage 2-4 · whichever first.

**Closure mechanism:** RCA closes when (V-RCA2-1 ∧ V-RCA2-2 ∧ V-RCA2-3 ∧ V-RCA2-4 ∧ V-RCA2-5) AND §S-N+ ledger anchor references this file with `RCA-CLOSED-VERIFIED` tag.

**Captain-stake gates (forward queue):**
- D-RCA2-1: MMA Step 1 budget+models ratify
- D-RCA2-2: per-PR merge auth for ws_handler.rs:684-711 fix-PR (cross-boundary contract class)
- D-RCA2-3: per-PR merge auth for production-path integration test addition
- D-RCA2-4: foundation helper extraction PR (rc-common::parse_launch_params) — optional class-level fix · separate Captain auth

---

End of RCA-2026-05-16-deny-unknown-fields-silent-fallback.
