# MMA Step-1 DIAGNOSE — Heart-V2 Real Pod-State Bridge (seam decision)

- **Date:** 2026-06-02 · **Author:** bono · **Channel:** OpenRouter (Captain Phase-3 directive)
- **Runner:** `/tmp/mma-heart-bridge-diagnose.mjs` · **Results:** `/tmp/mma-heart-bridge-results/`
- **Models:** 5 stratified, **4 vendor families responded** (deepseek-r1 timed out at 180s) — qwen3-coder, nemotron-3-super (nvidia), gemini-2.5-pro (google), kimi-k2.5 (moonshot). ≥3 vendor families ✓.
- **Cost:** $0.053 (ledger `openrouter-spend-bono.jsonl`, step `MMA-STEP1-DIAGNOSE`).
- **Note:** google + moonshot maxed `max_tokens` on an exhaustive D1 failure-mode list and did not reach D2-D4; nvidia + qwen answered all four. The binding output is the **unanimous D1 failure-mode + D3 billing-safety consensus** plus the **2/2 seam lean away from pure Option A**.

## Consensus — failure modes (unanimous 4/4)
| Failure mode | Severity | Mechanism |
|---|---|---|
| Lock-order deadlock | CRITICAL | Option A adds `state.heart.write()` to the V1 hot path that already holds `active_games.write()`; the 15s reconciler takes them in the opposite order → cyclic wait freezes the fleet. |
| Split-brain / stuck pod | CRITICAL | A dropped/aborted heart mutation leaves the panel "running" while the pod is crashed/idle → billed-but-unusable, blocks reallocation. |
| Zombie-Running resurrection | CRITICAL | A late `GameStateUpdate(Running)` after `Idle` re-lights a freed pod (re-grants green-light) if `window_secs` is not respected. |
| Transient-Error frees a billed pod | CRITICAL | `Error` is noisy (~11 emit sites); mapping it to `end` would stop billing/free a pod mid-session. |
| Dropped-WS terminal miss | IMPORTANT | Fire-and-forget WS push lost → heart never sees the terminal transition (zombie pod) until the reconciler backstop. |
| Double-broadcast | IMPORTANT | Hot-path send + reconciler send → duplicate SSE frames / UI flicker. |
| Idempotency violation | IMPORTANT/MINOR | Repeated `Running` re-grants green-light / double-`end` panics on double-free. |

## Consensus — billing-safety mapping (unanimous, D3)
- `Running` → `promote_to_running` (idempotent; green-light only after `verified_running`).
- `Error` → `mark_crashed` — **billing-NEUTRAL**: never set/clear `green_light_at`; keep the session for reconciliation.
- `Idle` → `end(sid, "game_exit")` — clears green-light, frees pod, stops billing.
- **Green-light rule:** the bridge may set `green_light_at` ONLY on `Running` with `verified_running` already true; NEVER on `Error`; cleared only on `Idle`/`end`.
- **Zombie-guard:** before acting on `Running`, verify the heart session still exists (not ended/crashed) AND the update is within `window_secs` (game_launcher.rs:142) — else discard.

## SEAM DECISION → **Option B (reconciler-based crash/exit detection)**
Both models that reached D2 (nvidia, qwen) rejected pure Option A; nvidia recommended decoupling via an async task, qwen recommended "A for Running, B for Error/Idle." Resolving against CLAUDE.md "smallest reversible fix first" + the unanimous #1 CRITICAL:

**Crash/exit (Idle/Error) detection goes in the existing reconciler, NOT the V1 hot path.** Rationale:
1. Eliminates the #1 unanimous CRITICAL (lock-order deadlock) **by construction** — the V1 `handle_game_state_update` hot path is left byte-unchanged (no new `state.heart` lock there).
2. The reconciler (`reconcile_heart_green_light_once`, 15s) already reads `active_games` and writes `state.heart` in the correct lock order — it is the natural, already-safe seam.
3. `Running`→promote (billing onset) is ALREADY handled by the launch dispatch-poll (confirm-before-bill) + the existing reconciler — so the only genuinely NEW logic is Idle/Error detection.
4. Latency: tighten the reconcile interval (15s → ~2s) and/or event-wake the reconciler on WS receipt via a lightweight notify flag (nvidia) so panel latency on crash/exit is ≤ a couple seconds, not 15s.

**Reconciler diff (per nvidia/qwen):** snapshot `active_games.read()` (pod→GameState) + `heart.read()` (pod→session); for each heart session whose pod is Idle/absent in active_games or Error → `end`/`mark_crashed`; for each Running-in-agent missing green-light → idempotent `promote_to_running`. All under correct lock order, never across `.await`.

## Binding constraints carried into implementation
- MUST-NOT-CHANGE: confirm-before-bill; SSE wire shape + `deny_unknown_fields`; flag `heart_v2_real_launch` default OFF until cutover; the per-tick admin-proxy metering (reads heart only); reconciler retained as backstop.
- OUT-OF-SCOPE (confirmed): Postgres `pod_state_projection` durable read-model; pod-display CUSTOMER error screens (V2.1-frozen).
- Highest-value tests (D4): (1) launch→Running→Idle full lifecycle; (2) transient Error keeps green-light + session alive; (3) dropped terminal WS → reconciler detects + ends; (4) zombie late-Running does not resurrect; (5) lock-order stress / concurrency = no deadlock.
