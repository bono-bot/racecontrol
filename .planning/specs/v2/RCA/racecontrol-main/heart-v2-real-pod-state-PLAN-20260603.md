# PLAN — Gap-2 Heart-V2 real-launch delivery integrity (I2 launch_args + cutover/flag)

- **Author:** bono · **Date:** 2026-06-03 IST · **Branch base:** `feat/heart-v2-real-pod-state` off `origin/main` `66a02154`.
- **Authorization:** Captain selected "Gap-2 launch-dispatch first" (2026-06-03) → author this PLAN for per-PR review. **Execution (code build), merge, I5 flag-flip, and I4 operator cutover all still require per-PR §S-146 named-surface Captain auth + (I4/I5) operator/Captain action. This PLAN is design only.**
- **Upstream gates discharged:** MTC (`MECHANISM-TRUST/heart-v2-real-pod-state-20260602.json` = PASS-WITH-MITIGATIONS) · 5-section RCA (`heart-v2-real-pod-state-bridge-20260602.md`) · MMA Step-1 DIAGNOSE (`...-DIAGNOSE-20260602.md` → Option B). This PLAN consumes them; it does not re-open them.

## Reframe from code-verification (2026-06-03) — narrows the remaining work
The RCA's I3 (the Option-B reconciler bridge) is **already implemented + tested at the branch base** (`origin/main 66a02154`); the GATE-0 branch adds only docs (2 commits, 3 files). Verified present:
- `heart_v2.rs:475 mark_crashed` (billing-neutral crash mutator) · `:861 reconcile_heart_green_light_once` · `:422 reconcile_green_light`.
- Tests `:1868 reconcile_crash_marks_interrupted_billing_neutral` · `:1900 reconcile_exit_frees_pod` · `:1916 reconcile_promote_matches_across_hyphen_underscore_keys` · `:1939 reconcile_flag_off_ignores_crash_exit` — these map 1:1 to the DIAGNOSE's priority tests (crash→neutral, exit→free, promote, flag-off).

**∴ remaining Gap-2 code = I2 (`launch_args`) only; then I4 (cutover, operator) + I5 (flag-flip, Captain).** This must be re-confirmed by whoever executes (read `reconcile_heart_green_light_once` body to confirm it actually propagates Running/Error/Idle, not just exists).

## The one remaining code gap (verified)
`heart_v2.rs:799` `dispatch_launch_to_agent(&state, &pod_id, sim_type, None, ctx)` → `launch_args = None`; `:795 duration_minutes: None` (`// TODO follow-up: derive from the V2 tier`); `:768` "car/track content is deferred". So even with `heart_v2_real_launch` ON, the rc-agent boots with no car/track/session → **bill can move without a real game = the delivery-integrity break.**

## Tasks

### T1 — I2-core: populate `launch_args` + `duration_minutes` at `launch_real`
- **Edit:** `crates/racecontrol/src/api/heart_v2.rs` `launch_real` (~L769-799): replace the `None` launch_args with `build_launch_args(&req)` and `duration_minutes: None` with `tier_to_duration(&req.tier)`.
- **Input available:** `LaunchReq { pod_id, household_id, profile_id, tier, game, lobby_id, preset_id }` (`heart_v2.rs:142`).
- **Validation contract:** `AcLauncher::validate_args` (`game_launcher.rs:176-181`) only checks the string is **valid JSON** — it does NOT enforce car/track. So the *binding* contract is the **rc-agent's `CoreToAgentMessage::LaunchGame { launch_args }` parser**, NOT validate_args.
  - **SUB-TASK T1a (must do before coding T1):** read the rc-agent launch handler (`crates/rc-agent/src/ws_handler.rs` + AC launch path) to pin the EXACT fields it consumes from `launch_args` (car / track / session_type / preset reference). Author `build_launch_args` to emit exactly those. Do NOT invent fields the agent ignores (serde silently drops unknowns — CLAUDE.md cross-boundary-serialization rule).

### T2 — I2-helper: `build_launch_args` + `tier_to_duration` (pure, unit-tested)
- `fn build_launch_args(req: &LaunchReq) -> Option<String>` — AC single-player first-INR scope. **OPEN DESIGN DECISION (Captain/author):** where do car/track come from? Options:
  - **(A) preset passthrough** — `launch_args = {"preset_id": req.preset_id}`; the rc-agent resolves it via its AC preset library (`LoadAcPreset`/`AcPresetLoaded {track, car_class}` already exist in the protocol). Cleanest if presets are provisioned per pod. Requires `preset_id` to be populated by the proxy.
  - **(B) known-good default** — a fixed first-INR AC-SP car+track (e.g. one provisioned car/track) when `preset_id` is absent, so first-INR works before the V2 game catalog exists. The `heart_v2.rs:836` `heart_game_to_sim_type` comment already flags "a real V2 game catalog (overlaps the preset surface) is a follow-up."
  - **Recommendation:** (A) when `preset_id` present, else (B) default — covers both. MP/lobby launch_args = **V2.1-FROZEN**.
- `fn tier_to_duration(tier: &str) -> Option<u32>` — derive the agent session duration from the V2 tier (`tier_1_full_skeleton` / `tier_2_desktop_workaround`). Confirm whether V2 metering wants a duration cap at all (the per-minute autobill tick + the 402 launch-gate already bound spend; duration may stay `None` if the agent treats `None` as "until stopped"). **OPEN: confirm intended semantics with Captain — do NOT cap the session shorter than the wallet allows.**
- Unit tests: build_launch_args(AC-SP w/ preset) → expected JSON; (no preset) → default; validate_args passes on both; MP tier → frozen/None.

### T3 — I4: close the silent mock-heart fallback (fail-closed)
- **Edit:** `apps/admin-proxy-james/src/m5-handlers.ts:17` — `RACECONTROL_HEART_URL` currently defaults to `http://127.0.0.1:8090` (mock-heart) if unset → panels silently show mock state (MTC Q5 / RCA §2.6). Change to **throw in prod if unset** (fail-closed); keep the mock default only under an explicit dev flag. *(admin-proxy-james is a money-path foundational surface → per-PR auth.)*
- **Operator (not bono):** set `.23` env `RACECONTROL_HEART_URL=http://127.0.0.1:8080` (the real venue heart, not :8090 mock).

### T4 — Verify (before any flag flip)
- Re-run the I3 reconciler tests (already present) + new T2 unit tests: `cargo test -p racecontrol-crate heart_v2` + the launch_args helper tests.
- DIAGNOSE priority lifecycle tests (1) launch→Running→Idle (2) transient Error keeps green-light+session (3) dropped terminal WS → reconciler ends (4) zombie late-Running no resurrect (5) lock-order stress — confirm still green.
- **Pod-8 canary (operator + bono):** with the flag ON for pod-8 only, launch real AC-SP → confirm the rig boots the **correct car/track** (not a blank/default-wrong sim) and that `verified_running` → green-light → per-minute `session_debit.*` debit moves. Evidence = the actual rig screen + a wallet_ledger row, per H3 (health 200 ≠ game launched).

### T5 — I5: activation (Captain)
- Captain `config_push` `heart_v2_real_launch` = ON **after** T1-T4 land + Pod-8 canary passes. Prod unchanged until then.

## Sequencing & dependencies
T1a (agent-parser read) → T1+T2 (launch_args build) → T3 (cutover guard) ∥ operator env → T4 (tests + canary) → T5 (Captain flag). I2 is independent of I3 (already on main) but both must be live before I5.

## Invariants preserved (from RCA/DIAGNOSE — do NOT regress)
confirm-before-bill (green-light only on real `verified_running`) · billing-neutral crash (`mark_crashed` never sets/clears `green_light_at`) · no double-spend / no free session · never hold a lock across `.await` (snapshot→drop→await) · `#[serde(deny_unknown_fields)]` on heart wire structs · idempotent transitions · flag default OFF until I5.

## Gates before EXECUTE / merge / deploy
- §S-146: RCA + DIAGNOSE + MTC discharged (above). **Per-PR named-surface Captain auth required to (a) build/commit the I2 code, (b) merge to `replit/coordinator`/main, (c) I4 m5-handlers change.** A generic "proceed" does NOT satisfy this (foundational money-path + pod-state-channel boundary).
- MAOR review between FIX and CLOSE (pre-push-maor gate) on the I2 cascade.
- Cross-boundary serialization check (T1a): every `launch_args` field must have a matching rc-agent consumer (grep the agent parser) — serde drops unknowns silently.

## Open questions for Captain
1. **launch_args car/track source** (T2): preset-passthrough (A), known-good default (B), or A-then-B? Is `preset_id` reliably populated by the proxy/launch-portal for first-INR AC-SP?
2. **duration_minutes from tier** (T2): cap the agent session, or leave `None` (wallet 402-gate + autobill tick already bound spend)? Don't cap shorter than the wallet allows.
3. Authorize T1+T2 build (bono, racecontrol lane) under per-PR §S-146 — or hold for the car/track decision first?
