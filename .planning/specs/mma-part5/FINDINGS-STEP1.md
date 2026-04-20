# MMA Part 5 Step 1 DIAGNOSE — Findings

**Run:** 2026-04-20 19:57 IST
**Budget:** $0.0417 / $3.00 used
**Models:** 5/5 completed, 0 timeouts, 0 retries
**Raw responses:** `responses/{deepseek-r1,qwen3-coder,nemotron-super,gemini-flash,grok-code}.md`
**Runner:** `run-step1.js` / stderr: `step1.stderr.log`

## Per-model

| Model | Role | Elapsed | Tokens (p/c) | Cost | Findings |
|---|---|---|---|---|---|
| deepseek-r1-0528 | reasoner | 198.6s | 9493 / 4680 | $0.0149 | 12 |
| qwen3-coder | code_expert | 13.8s | 8538 / 1571 | $0.0044 | 10 |
| nemotron-3-super | SRE | 37.4s | 9439 / 8682 | $0.0053 | 10 |
| gemini-2.5-flash | generalist | 24.6s | 10273 / 3691 | $0.0123 | 14 |
| grok-code-fast-1 | code_expert | 12.6s | 8445 / 1983 | $0.0047 | 12 |

Vendor families covered: deepseek, qwen, nvidia, google, xai (5 — exceeds MMA min 3).
Roles covered: ≥1 reasoner, ≥2 code_expert, ≥1 SRE — meets MMA diversity rule.

## Consensus findings (≥3/5)

| # | Theme | Flags | Max Sev | Axis | Mitigation (one-line) |
|---|---|---|---|---|---|
| C1 | WS-real-after-HTTP-synth race: dedup guard zeros real stats forever | 4/5 | P1 | b,c | Second-apply path must `refresh_summary_card(real_stats)` only; skip lifecycle side-effects |
| C2 | 60s grace missing from algorithm body → false synth during session-start window | 3/5 | **P0** | a,b | Add `active_billing_session_id_set_at: Instant` to FailureMonitorState; gate synth on elapsed ≥ 60s |
| C3 | T2 periodic tick ignores shutdown signal → spurious synth during graceful exit | 4/5 | P1 | a,c | Plumb `CancellationToken` into T2 spawn; wrap tick in `tokio::select!` |
| C4 | `conn.current_driver_name` cleared at ws_handler.rs:376,413 → blank summary in synth | 3/5 | P2 | c | Server response carries `driver_name`; use that, not conn cache |
| C5 | X-Service-Key header absent → if route actually gates, fallback silent-401s forever | 3/5 | P1 | c | Follow remote_ops.rs auth pattern; pre-merge probe confirming /billing/active is genuinely public |
| C6 | pod_id filter missing → session-id-only membership check breaks if IDs recycle | 3/5 | P1 | b | Filter server_sessions by `pod_id == state.config.pod.number` before membership check |
| C7 | URL derivation duplicates ws_handler:423-429; JWT query-string compounds divergence | 3/5 | P1 | b,c | Extract `rc_common::url::http_base_from_ws` helper; unit-test 4 variants |
| C8 | Zeroed `total_laps`/`driving_seconds` mislead customer + corrupt offline-lap analytics | 3/5 | P2 | b,c | Cache `session_last_known` in FailureMonitorState; flag synth summaries with `synth=true` |
| C9 | blank_timer re-arm race: spec calls "benign", grok-code disputes at P1 | 4/5 | P1 | a,c | Reset blank_timer only on first-apply; skip on second-apply via `last_applied_session_end` |

## Dissenting P0 — 1 (escalated)

**D3 — Rollback regression class** (grok-code #11, P0, axis d). Rolling back to pre-patch binary silently re-introduces the stuck-session class with zero alert. Deploy pattern retains `racecontrol-prev.exe` 72h per standing rule → live risk.

**Mitigation:** Emit structured `fallback_version=part5_v1` log on every T1 fire and every synth. Server-side composite rule in `/fleet/health`: pods lacking `fallback_version` AND holding `active_session_id` without recent SessionEnded > 15 min → flag as "stuck-session candidate, pre-patch binary suspected".

**Accepted** — cheap, aligns with existing SWAPLOG + build_id culture, structural deploy-observability improvement.

## Dissenting P1s folded in (1-2 model flags, high-value-low-cost)

- **D2 / D9** (gemini-flash): reqwest returns `Ok` for 5xx bodies. Add explicit `if !resp.status().is_success() { return; }` before JSON parse. Cheap correctness fix.
- **D6** (nemotron-super): `last_applied_session_end` MUST live on `AppState`, not `ConnectionState` — the latter is rebuilt every reconnect loop iteration, wiping the dedup guard.
- **D11** (qwen3-coder): Extracted `apply_session_ended` must ALSO reset `inactivity_monitor` and `crash_recovery` — missing either makes the extracted fn non-equivalent to the inlined arm (silent regression on refactor).
- **D13** (gemini-flash): Pin `BillingSessionInfo` JSON shape in `rc_common` shared types. rc-agent is currently parsing server JSON blind — a field rename breaks fallback silently.

## Spec updates required before Step 2 PLAN (10 must-include)

1. 60s grace check into algorithm body (C2)
2. Move `last_applied_session_end` to AppState (D6)
3. Shared `http_base_from_ws(url)` helper in rc_common (C7)
4. pod_id filter in algorithm (C6)
5. X-Service-Key header + pre-merge un-keyed probe (C5)
6. Explicit status-code gate before JSON parse (D9)
7. CancellationToken plumbed into T2 spawn (C3)
8. `apply_session_ended` must reset blank_timer + inactivity_monitor + crash_recovery atomically (D11)
9. `refresh_summary_card(real_stats)` upgrade path for WsReal arriving after HttpSynth (C1 + C4 + C8)
10. `fallback_version=part5_v1` telemetry marker on every T1 + synth (D3)

## Proceed decision

**Proceed to Step 2 PLAN** after spec updates (1)-(10) land. No Step 1 iteration.

Findings distributed evenly across axes (a: 6 themes, b: 8, c: 9, d: 6). No blind spot. 9 consensus themes, all actionable within one rc-agent commit. Spec-level merge of updates → Step 2 validates plan against updated spec → Step 3 implements.

Budget remaining: $2.96 for iteration headroom.
