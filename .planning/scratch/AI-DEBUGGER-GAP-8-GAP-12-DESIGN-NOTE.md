# AI Debugger — GAP-8 + GAP-12 design options

Author: James (Claude Opus 4.7) · 2026-04-19 · status: DESIGN ONLY, no code changes

These two gaps from the 2026-04-16 AI Debugger safety overhaul handoff were
explicitly flagged as non-autonomous: GAP-8 needs an architecture decision and
GAP-12 was "defer until real query traffic justifies the change." This note
captures the options so the decision has a paper trail.

---

## GAP-8 — Two parallel debug systems with no coordination

### Background

The rc-agent has two independent autonomous debugging subsystems:

- **System A — `ai_debugger.rs`** (2,113 lines): synchronous crash-event handler,
  15 keyword-matched fix patterns + pattern memory + Ollama/OpenRouter.
  State: `C:\RacingPoint\debug-memory.json`.
- **System B — `diagnostic_engine.rs` (830) + `tier_engine.rs` (~2,860 after
  GAP-9)**: background 5-second anomaly poller, Tier 0-2 deterministic + Tier
  3/4 MMA decision gate.
  State: `C:\RacingPoint\knowledge-base.db`.

They share neither state nor coordination. Under a game-crash storm both can
fire fixes at the same pod simultaneously with conflicting actions (e.g.
System A calls `fix_kill_stale_game()` while System B's Tier 1 is mid-restart).

### Option 8α — Explicit priority + mutex

**Change:** Add a per-pod `RecoveryLock` (async tokio::sync::Mutex). Both
systems acquire it before executing any `spawn_safe` / `taskkill` / process
kill. System A (synchronous crash-triggered) has implicit priority because it
fires first; System B polls every 5s and will find the lock held. Add a
per-pod `recovery_in_progress_since: Option<Instant>` timestamp so System B
can time-out the lock after 60s (guard against a crashed System A holding
forever).

**Pros:** smallest diff. No system-level merge. Existing tests unaffected.
**Cons:** serialises recoveries. Two independent crash classes on the same
pod would queue one behind the other (rare but possible during deploy storms).
**Effort:** ~2-4 hours. Risk: LOW.

### Option 8β — Single entrypoint, sub-strategies

**Change:** Make `ai_debugger.rs` the sole fix-executor. `tier_engine.rs`
becomes a classifier+recommender that emits `DiagnosticSuggestion` events
which `ai_debugger.rs` serialises through its existing `try_auto_fix()` +
pattern-memory pipeline. All kills, taskkills, restarts route through a
single function. Quarantine TTL, billing-guard, launch-epoch already exist
there — Tier 0-4 gets them for free.

**Pros:** true single source of truth. All billing/validation/quarantine
logic applies to Tier fixes too. No duplicate pattern memories.
**Cons:** large refactor. Breaks current tier_engine behaviour tests. Changes
the DiagnosticEvent → fix dispatch path that Phase 229 established.
**Effort:** ~2-3 days including MMA audit + fleet re-verification.
Risk: MEDIUM-HIGH (touches ~8 files, >40 tests).

### Option 8γ — Side-by-side with broadcast-aware gating

**Change:** Each system publishes a `FixInFlight {pod_id, fix_type, started_at}`
message to the existing broadcast channel. Before executing, each system
checks the channel for in-flight fixes on the same pod and waits/aborts if
the fix overlaps (same `fix_type` family — e.g. "taskkill_game" or
"restart_service"). No shared mutex, no merger — just coordination via the
existing pub/sub.

**Pros:** preserves both systems' independence. No refactor. Failure mode
is graceful degradation (broadcast slow → fixes still execute, just not
coordinated).
**Cons:** coordination is best-effort, not guaranteed. Race window between
"check for in-flight" and "publish mine". Can't detect stalls cleanly.
**Effort:** ~1 day. Risk: LOW-MEDIUM.

### Recommendation

**Option 8α first** (mutex) for the immediate safety win, then re-evaluate.
8β is the right long-term design but should come after the billing-gate +
launch-epoch model is proven stable for another 30 days of customer traffic.
Don't bundle 8α and 8β in the same commit — the first is a fence, the
second is a rebuild.

**Decision owner:** Uday + architecture review. Not autonomous-safe.

---

## GAP-12 — Pattern key too coarse (`SimType:exit_code`)

### Background

`DebugMemory::pattern_key()` builds keys like `AssettoCorsa:-1`. This
collapses genuinely-different failures into one bucket:

- AC crash because of stale mod list
- AC crash because of corrupt telemetry.ini
- AC crash because of wheelbase disconnect during load
- AC crash because of OOM in 64-car multiplayer

…all key to `AssettoCorsa:-1`. The pattern memory then recommends the
fix learned from whichever variant happened last, regardless of fit.

### Option 12α — Error-context fingerprint

**Change:** Add a 4-char xxhash suffix computed from the first 100 chars of
`error_context`. New keys: `AssettoCorsa:-1:a3f2`. Identical contexts →
identical fingerprints → same key. Variants get separate keys.

**Pros:** structurally correct — each root cause gets its own pattern.
**Cons:** (a) breaks 7 unit tests that assert exact legacy keys like
`AssettoCorsa:-1`. (b) invalidates the entire existing `debug-memory.json`
on every pod (keys don't match). (c) Slows learning — each new context
variant starts at success_count=0.
**Effort:** ~3-4 hours including test updates + on-disk migration shim.
Risk: MEDIUM.

### Option 12β — Error-context classifier extension

**Change:** Extend the existing keyword classifier (the "Priority 2" branch
in `pattern_key()`) to ALSO enrich the exit-code branch. Detect well-known
error-context substrings and append them as structured sub-reasons:
`AssettoCorsa:-1:mod_load`, `AssettoCorsa:-1:wheelbase_disconnect`, etc.

**Pros:** deterministic, readable keys. Tests can be updated incrementally.
No hashing. Humans can grep debug-memory.json meaningfully.
**Cons:** only helps for classes we predict. Novel variants still collide.
Classification logic needs review as new games ship.
**Effort:** ~2 hours for the initial classifier set + tests.
Risk: LOW.

### Option 12γ — Defer (handoff's original recommendation)

**Change:** None. Accept current coarseness. Wait for real James query
traffic to reveal which exit-code buckets actually collapse real variants
before paying the refactor cost.

**Pros:** zero work. Other safety rails (launch-epoch, quarantine TTL,
billing-guard, validation window) already mitigate the worst downstream
effects — a wrong fix applied from a collapsed bucket gets quarantined
within 3 fires/5min.
**Cons:** coarseness remains; future incidents may hit the same issue.
**Effort:** 0. Risk: LOW (we already ship this).

### Recommendation

**Option 12γ for now.** The Pod 8 47-kill-loop incident that triggered the
overhaul was caused by missing safety rails, not by coarseness — coarseness
just made the wrong fix easy to pick. With quarantine TTL + launch-epoch +
billing-guard in place, a bad suggestion self-limits within minutes.
Re-visit when (a) a real customer incident traces to wrong-bucket fix
selection, or (b) debug-memory.json accumulates >50 entries and we have
actual query traffic to profile which keys over-collapse.

If 12 becomes active, prefer **12β over 12α** — readable keys beat opaque
fingerprints when an operator greps debug-memory.json during a live incident.

**Decision owner:** defer to next AI Debugger tuning session.

---

## Why these are not shipped autonomously

- GAP-8: merges or coordinates two top-level subsystems. Touches billing
  guards, process-kill paths, and the dispatch model for Phase 229. Failure
  mode is "customer's running game gets killed during a race" — identical
  to the Pod 8 47-kill-loop. MMA-required.
- GAP-12: breaks existing on-disk state format and seven tests. Low
  severity relative to the mitigations already in place. Handoff explicitly
  said "defer until we have real James query traffic to justify the change."

Both fall under CGP v4.3 H1 "design decision before action" — autonomous
execution would violate the gate. Documenting the options is the correct
autonomous step.
