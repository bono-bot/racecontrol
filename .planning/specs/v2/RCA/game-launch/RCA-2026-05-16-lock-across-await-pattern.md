# §S-146 RCA — Lock-across-await pattern class on game-launch foundational surface

**Status:** DRAFT-PENDING-MMA-STEP-1-CAPTAIN-AUTH
**Authored:** james · 2026-05-16 ~12:50 IST (07:20 UTC)
**Anchor:** §S-388 (this session close-anchor) · MAOR datapoint #2 Tier-2 GLS-1/GLS-2 + expanded MAOR STOP-1/RELAUNCH-1
**Class:** §S-146 V1↔V2 RCA · foundational-boundary (billing-adjacent state machine · agent_senders cross-pilot channel) · MMA Step 1 DIAGNOSE required before fix-PR
**Captain framing:** Game Launching = MOST IMPORTANT V2 feature (Captain 2026-05-16 12:41 IST)
**Composes-with:** §S-146 parent doctrine · mechanism-trust-check upstream (v27.0 MMA P1 trust verification) · §S-186 small-fix fast-lane — NOT eligible (multi-boundary)

---

## §1 — Boundary map

The pattern class spans **4 distinct lock-across-await sites** in **3 files** all touching foundational-boundary surfaces. Cited at file:line precision per CGP H3.

### Site A — `crates/racecontrol/src/game_launcher_state.rs:253-270`

```rust
let mut timers = state.billing.active_timers.write().await;   // L253: write guard
if let Some(timer) = timers.get_mut(pod_id)
    && timer.status == BillingSessionStatus::Active && timer.driving_seconds < 120 {
        timer.driving_seconds = 0;
        timer.started_at = Some(Utc::now());
        let _ = sqlx::query(
            "UPDATE billing_sessions SET driving_seconds = 0, started_at = ? WHERE id = ?"
        )
        .bind(Utc::now().to_rfc3339())
        .bind(&timer.session_id)
        .execute(&state.db).await;                            // L268-269: .await while guard alive
    }
// L271: implicit guard drop at end of if-block
```

**Lock held:** `state.billing.active_timers` (tokio `RwLock<HashMap<pod_id, BillingTimer>>`)
**Across:** `sqlx::query(...).execute(&state.db).await` (SQLite UPDATE round-trip)
**Boundary:** billing↔game-launch seam. Triggered on every `Running` GameStateUpdate for AC during initial launch window (driving_seconds < 120).

### Site B — `crates/racecontrol/src/game_launcher_state.rs:422-433` (inside spawned RE-relaunch task at L364)

```rust
let senders = state_clone.agent_senders.read().await;         // L422: read guard
if let Some(tx) = senders.get(&pod_id_owned) {
    let _ = tx
        .send(CoreMessage::wrap(...))
        .await;                                                // L432: .await while guard alive
}
drop(senders);                                                 // L434: post-hoc drop
```

**Lock held:** `state.agent_senders` (tokio `RwLock<HashMap<pod_id, mpsc::Sender>>`)
**Across:** `tx.send(CoreMessage::wrap(LaunchGame { ... })).await` (mpsc channel send)
**Boundary:** server→pod cross-process channel. Triggered on Race Engineer auto-relaunch (CRASH-04 path).

### Site C — `crates/racecontrol/src/game_launcher_ops_stop.rs:73-82`

```rust
let senders = state.agent_senders.read().await;               // L73: read guard
if let Some(tx) = senders.get(pod_id) {
    if let Err(e) = tx.send(stop_msg).await {                 // L75: .await while guard alive
        state.pending_command_acks.write().await.remove(...);
    }
} else { ... }
drop(senders);                                                 // L82
```

**Lock held:** `state.agent_senders`
**Across:** `tx.send(stop_msg).await` + conditional `pending_command_acks.write().await` (nested lock acquisition while outer guard alive — compound class)
**Boundary:** `/games/stop` HTTP route + auto-stop on billing-end path.

### Site D — `crates/racecontrol/src/game_launcher_ops_relaunch.rs:69-79`

```rust
let senders = state.agent_senders.read().await;               // L69: read guard
let tx = senders.get(pod_id).ok_or("Pod not connected")?;
tx.send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame { ... }))
    .await                                                     // L77: .await while guard alive
    .map_err(|e| ...)?;
```

**Lock held:** `state.agent_senders`
**Across:** `tx.send(...).await` (LaunchGame channel send)
**Boundary:** `/games/relaunch/{pod_id}` HTTP route (staff-initiated manual relaunch) + Race Engineer relaunch sibling path.

### Counter-anchor (correct pattern in same file family) — `game_launcher_support.rs:227-230`

```rust
let sender_opt = {
    let senders = state.agent_senders.read().await;
    senders.get(&pod_id).cloned()
};  // guard dropped — then await
```

**Significance:** the snapshot-then-drop pattern was applied correctly in `support.rs` but NOT propagated to sibling `ops_stop.rs` / `ops_relaunch.rs` during v49.0 extraction. Systematic extraction-time discipline gap, not isolated drift.

---

## §2 — Inherited-issue catalogue

### V1 failure-mode anchor — v27.0 MMA P1 (CLAUDE.md Code Quality section)

> *"Never hold a lock across `.await` — whether `std::sync::RwLock`, `tokio::sync::RwLock`, or `Mutex`. Clone/snapshot the data, drop the guard in a tight `{ }` block, THEN iterate or perform async work. ... v27.0 MMA — 5/5 models flagged lock-across-await as P1 deadlock/starvation risk. Pattern exists in 10+ places across codebase from earlier milestones."*

The v27.0 MMA pass (cited in CLAUDE.md) identified this exact class as P1 with 5/5 model consensus. The doctrine landed; the lateral sweep to fix all instances did not complete. Sites B/C/D are documented in `game_launcher_support.rs:227-230` as the canonical pattern; sibling ops files were not converted.

### V1 process-mess catalogue (categories touched)

Per `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md`:
- **Category C** (lock-discipline cascade) — direct hit. `agent_senders` is V1-shaped cross-process IPC channel inherited as-is into V2 substrate.
- **Category G** (broadcast storm class) — sibling. Lock starvation under 8-pod concurrent launch storm produces the same "all pods stall" effect as the v17.0 browser-watchdog flicker class.

### Past-bug catalogue at this boundary (60-day window)

| Anchor | Class | Disposition |
|---|---|---|
| v27.0 MMA P1 — lock-across-await | identical pattern class | PATCHED-ONLY-LATERALLY-INCOMPLETE — doctrine landed in CLAUDE.md; v49.0 module extraction did not propagate fix to all sibling files |
| §S-61 PART 41 V1 failure-mode #C — lock-discipline cascade | parent class | UNRESOLVED — no hook-enforcement; recurrence rate ≥1/30d per §S-146 enforcement RCA |
| §S-272 racecontrol restart cascade (2026-05-12) | downstream effect of lock contention class | ROOT-CAUSED-AND-FIXED (restart-storm mitigation) but root cause (lock starvation on cold-start) deferred |
| GameTracker stuck-in-Launching (CLAUDE.md Crash Loop section) | sibling failure mode at same boundary | PATCHED-ONLY (5s ACK timeout) — root cause (no `Launching → Error` transition timeout) UNRESOLVED |

---

## §3 — Past-bug review (per-site disposition)

### Site A (billing-adjacent · sqlx UPDATE) — **NEW · NEVER-DETECTED · CRITICAL**

- Never surfaced in prior MMA pass (file extracted from `game_launcher.rs` in v49.0 module split; v27.0 audit predates this file).
- Path runs on every AC `Running` event during initial-launch window. Triggered ~once per customer-session start.
- Blast radius: any concurrent `active_timers` read (`/billing/status` GET, billing-tick timer, session-end) stalls for SQLite UPDATE wall-clock duration.

**Disposition:** UNRESOLVED — newly identified · billing-adjacent foundational.

### Site B (agent_senders + spawned task · channel send) — **NEW · NEVER-DETECTED · CRITICAL**

- RE relaunch path (CRASH-04). The `drop(senders)` at L434 is post-hoc, after the await — defensive intent visible but mechanism wrong.
- Blast radius: blocks `agent_senders` writers (agent reconnect after pod restart) during a window when pod IS crashing — worst-case ordering.

**Disposition:** UNRESOLVED — newly identified · foundational cross-pilot channel.

### Site C (ops_stop · agent_senders + pending_command_acks compound) — **NEW · NEVER-DETECTED · IMPORTANT**

- Compound lock acquisition (read guard on outer, write guard on inner across await).
- v49.0 extraction-time gap — sibling `support.rs` has correct pattern.

**Disposition:** UNRESOLVED — newly identified · extraction-time discipline gap.

### Site D (ops_relaunch · agent_senders · channel send) — **NEW · NEVER-DETECTED · IMPORTANT**

- Same class as Site C. v49.0 extraction sibling gap.

**Disposition:** UNRESOLVED — newly identified · extraction-time discipline gap.

### Class disposition

The **doctrine** ("no lock across await") is ROOT-CAUSED-AND-FIXED (CLAUDE.md anchor). The **lateral application** is PATCHED-ONLY — the lateral sweep was not exhaustive, and module extraction (v49.0) re-introduced the class. Per `feedback_s146_enforcement_rca_20260510.md`: text-only rules carry ≥1 repeat-violation per 30d; hook-enforced rules carry zero. This RCA is the 30d recurrence evidence — class persisted through 2 module reorgs.

---

## §4 — V2-alignment delta

V2 doctrine for shared-state coordination (per `comms-link/v2-skeleton/01-skeleton-architecture.md` §40 + Pod-Control Doctrine §S-171):

1. **Snapshot-then-await** at all lock acquisitions touching channels/DB/external I/O. The `support.rs:227-230` pattern is canonical.
2. **No compound lock acquisition** across await points (Site C extra-violates this — read + write nested).
3. **State-channel coupling** must use `watch::Receiver` or `mpsc` send-without-holding-lock — the Pod-Control Doctrine establishes pod = experience surface; `agent_senders` IS the experience-surface state-channel. Lock contention on this channel = direct experience-surface impact.
4. **Foundation/strategy/config separation** (§AMEND-3.II D12): the lock-discipline IS foundation-class concern. Strategy-class (RE relaunch logic, AC timer-sync logic) sits ABOVE it. Sites A-D conflate foundation with strategy by holding foundation-locks across strategy-async-ops.

**Gap:** V2 substrate inherits V1-shaped `agent_senders` Arc<RwLock<HashMap>> from racecontrol→pods topology. V2 has NOT yet converted this to a state-channel-pattern primitive. The lock-across-await class persists because the V1 substrate's shape encourages it (acquire-by-key lookup encourages holding the guard for the send).

**Substrate-pointer:** Canonical lock-discipline doctrine = CLAUDE.md Code Quality > "Never hold a lock across `.await`" (v27.0 MMA P1 ratify) + this RCA filing. No code-side state-channel primitive ratified yet; deferred to V2 Pod-Control Doctrine implementation wave 3-4 per `project_v2_pod_control_doctrine_deployment_plan_20260510.md`.

---

## §5 — V2-framed proposal (NOT autonomously executed — MMA Step 1 + Captain auth required)

### Stage 1 — Mechanism trust check (5 questions per `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md`)

Before fix authoring, verify the delivery mechanism for the fix:

1. **Atomic primitives?** — Each fix-site is a single atomic edit (no multi-step pipeline). Single-PR scope. **YES.**
2. **TTL-bounded sentinels?** — No sentinel involved; pure code change. **N/A (clean).**
3. **Behavioral-verify success?** — Behavior test = "concurrent billing tick + game launch on same pod does not stall the tick under load". Requires synthetic load harness. Build_id flip alone is NOT sufficient. **NEEDS-HARNESS.**
4. **Single-target dry-run?** — Pod 8 canary post-deploy (CLAUDE.md "Test before upload" rule). **YES.**
5. **Guard contracts?** — Lock-discipline has NO code-enforced guard (rust-analyzer doesn't warn; clippy::await_holding_lock IS available but not enabled in workspace Cargo.toml). **NO — gate-fail.** Q5 FAIL → mechanism-trust-check RCA on lint-enforcement is prerequisite to fix RCA per `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md`.

### Stage 2 — MMA Step 1 DIAGNOSE (Captain auth verb needed)

5-model OpenRouter consensus on root causes. Budget ~$5. Slots per UNIFIED-MMA-PROTOCOL.md:

- Reasoner: Claude Opus 4.7 (or DeepSeek R1, or Kimi K2.6)
- Code Expert: Claude Sonnet 4.6 (or DeepSeek V4-Pro, or Qwen3-Coder-Plus)
- SRE/Ops: MiMo v2.5-Pro (or Nemotron-3-Super-120b)
- Domain Specialist: Rust-async deadlock specialist
- Generalist: Gemini 2.5 Pro

Min 3 vendor families. Captain verb form: *"James, authorize MMA Step 1 DIAGNOSE on the lock-across-await pattern-class RCA · 5-model OpenRouter · $5 budget · output to .planning/specs/v2/RCA/game-launch/MMA-STEP-1-CONSENSUS.md"*

### Stage 3 — Fix proposal sketch (NOT a commitment; subject to MMA consensus)

**Per-site fixes (snapshot-then-await pattern):**

- **Site A:** scope billing field snapshot in inner block → drop guard → execute DB query. ~12 LOC delta.
- **Site B:** clone `tx` out of guard in inner block → drop guard → send. ~6 LOC delta.
- **Site C:** clone `tx` + handle missing case → drop guard → send + nested write-lock. ~10 LOC delta.
- **Site D:** clone `tx` + early-return on missing → drop guard → send. ~8 LOC delta.

**Class-level fix (PREFERRED if MMA Step 2 PLAN converges on it):**
Extract a `AgentSenderRegistry::send_to(pod_id, msg)` helper that snapshots-internally and returns `Result<(), SendError>`. All 4 sites collapse to a single line. Removes class entirely + prevents recurrence via single chokepoint.

**Hook-enforcement (composes-with mechanism-trust §Q5):**
Enable `clippy::await_holding_lock` in workspace `Cargo.toml` `[lints]` table with `deny` level. Catches future instances at compile time. Per §S-146 enforcement RCA — text-only rule recurrence rate dictates hook-enforcement is the correct closure.

### Stage 4 — V2-doctrine alignment statement (required on PR per V1↔V2 RCA rule)

`V2 doctrine alignment: Pod-Control Doctrine §S-171 root + reliability buttress · agent_senders = experience-surface state-channel · lock-discipline is foundation-class concern (§AMEND-3.II D12) · v27.0 MMA P1 lateral sweep completion + hook-enforcement closes the recurrence class.`

### Verify-by

- V-RCA1-1: All 4 sites converted to snapshot-then-await OR class-level helper. `grep -nE '\.(read|write)\(\)\.await' crates/racecontrol/src/game_launcher_*.rs` returns ZERO instances of guard-followed-by-other-await in same scope.
- V-RCA1-2: `clippy::await_holding_lock = "deny"` enabled in workspace `Cargo.toml`. CI passes.
- V-RCA1-3: Synthetic 8-pod concurrent launch + billing-tick harness shows no tick-stall under load (test added to integration suite).
- V-RCA1-4: Server .23 + Bono VPS both on new build_id post-deploy. Pod 8 canary verify before fleet.

### NOT in scope (deferred to downstream RCAs/items)

- GameTracker stuck-in-Launching timeout (sibling failure mode; separate item — Tier-1 finding L7)
- JSON injection in `events_json` crash_detail (GLS-6 · separate sub-RCA)
- billing_rates/game_limits silent no-op ConfigPush (WSH-3 · separate RCA on cross-boundary contract class)
- Hardcoded staff phone number (SUPPORT-1 · config-extraction class · separate item)
- Hardcoded `.23:8090` SwitchController probe (WSH-7 · config-extraction class · separate item)

---

## §6 — Stale-at + closure tracking

**Stale-at:** 2026-08-16 (90d) OR MMA Step 1 fires OR Captain disposition on Stage 2-4 sequence · whichever first.

**Closure mechanism:** This RCA closes when (V-RCA1-1 ∧ V-RCA1-2 ∧ V-RCA1-3 ∧ V-RCA1-4) AND a §S-N+ ledger anchor on V2-MASTER-STATE references this file with `RCA-CLOSED-VERIFIED` tag.

**Captain-stake gates (forward queue):**
- D-RCA1-1: MMA Step 1 budget+models ratify
- D-RCA1-2: per-PR merge auth for Site A fix-PR (billing-adjacent)
- D-RCA1-3: per-PR merge auth for Sites B+C+D fix-PR (or class-level helper PR)
- D-RCA1-4: workspace Cargo.toml clippy lint enablement (foundation/strategy boundary class)

---

End of RCA-2026-05-16-lock-across-await-pattern.
