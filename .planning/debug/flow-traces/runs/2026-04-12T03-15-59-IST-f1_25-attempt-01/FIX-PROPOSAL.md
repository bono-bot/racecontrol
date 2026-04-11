# Fix Proposal — F1 25 PlayableSignal Gating

**Linked report:** `DIVERGENCE-REPORT.md` (this folder)
**Status:** PROPOSAL ONLY — no commits, awaiting user approval
**Bug class:** Cross-boundary signal split (1 signal used for 2 distinct purposes)

---

## What needs to change in one sentence

F1 25's sim adapter must emit a "game is alive and reachable" signal for the launch verifier on **any** parsed CarTelemetry packet, while keeping the existing "customer is actively driving" signal gated on `speed_kmh > 0` for billing purposes.

---

## Fix Option A — Add a second signal type, route separately (RECOMMENDED)

**Change:** Add `DetectorSignal::UdpReachable` variant. Emit it on any parsed CarTelemetry packet from F1 25. Route `UdpReachable` to the launch_verifier; keep `UdpActive` (gated on `speed > 0`) routed to billing.

**Files affected:**
1. `crates/rc-agent/src/driving_detector.rs` — add `UdpReachable` to `DetectorSignal` enum + `process_signal` handler
2. `crates/rc-agent/src/sims/f1_25.rs` — emit `UdpReachable` on every parsed CarTelemetry packet (separate from the `UdpActive` block)
3. Wherever `DetectorSignal::UdpActive` is consumed for launch verification (need to grep — possibly `event_loop.rs` or `launch_verifier.rs` or `failure_monitor.rs`) — switch to consuming `UdpReachable`
4. **No changes needed for AC/iRacing/LMU/ACE** — they use shared memory connectivity, not these signals

**Diff sketch (conceptual, NOT applied):**
```rust
// driving_detector.rs
pub enum DetectorSignal {
    HidActive,
    HidIdle,
    HidDisconnected,
    UdpActive,        // game is being PLAYED (speed > 0) — billing trigger
    UdpReachable,     // game is RUNNING (any packet received) — launch verifier trigger  ← NEW
    UdpIdle,
}

// f1_25.rs read_packet (around line 506)
// First emission — fires on EVERY parsed packet
if let Some(ref tx) = self.signal_tx {
    let _ = tx.try_send(DetectorSignal::UdpReachable);
}
// Second emission — keep existing speed gate
if self.speed_kmh > 0 {
    if let Some(ref tx) = self.signal_tx {
        let _ = tx.try_send(DetectorSignal::UdpActive);
    }
}
```

**Pros:**
- Clean architectural separation: launch detection ≠ billing trigger
- Defensive comment intent preserved (no false billing-start from menu packets)
- Symmetric with what AC/SHM games already do implicitly (SHM connect = "alive", lap data = "playing")
- Smallest behavioral surface area — only adds a new signal, doesn't change existing ones
- Easy to roll back: revert one commit

**Cons:**
- Touches 3-4 files (enum + adapter + consumer + tests)
- Need to verify the consumer side wires `UdpReachable` to launch_verifier and not somewhere else by accident — this is the "cross-boundary serialization" risk and needs careful verification before commit
- Need new tests for both paths

**Blast radius:** Pod-side only (rc-agent). Server doesn't change. Pod TOMLs don't change. Other games unaffected (they don't use these signals).

**Test plan:**
1. Unit test: F1 25 adapter emits `UdpReachable` on any packet, `UdpActive` only on speed > 0 packet
2. Integration test: launch_verifier reaches verified state on `UdpReachable`
3. Live regression: re-run this exact trace on Pod 4 — should see green-flag launch (game stays running) instead of BILL-13 cancel
4. Negative test: verify billing still doesn't start charging during F1 25 menu navigation

**Recommendation: this is the right fix.** It addresses the root cause architecturally and matches the developer's original defensive intent (which is correct).

---

## Fix Option B — Single-emission with packet-count threshold

**Change:** Keep `UdpActive` as the only signal but emit it after N consecutive parsed packets (e.g. 30 packets ≈ 0.5 seconds at 60 Hz), regardless of speed.

**Pros:**
- Smallest possible code change (single file, single block)
- No new enum variants

**Cons:**
- Conflates "alive" and "playing" semantically — same bug class as today, just papered over
- Re-introduces the menu-billing-start risk the original fix was trying to avoid
- The 30-packet threshold doesn't actually filter menu vs gameplay — both produce packets at the same rate
- Future-fragile

**Recommendation: avoid.** Doesn't fix the architectural issue.

---

## Fix Option C — Increase `default_launch_timeout_per_attempt` for F1 25

**Change:** Add per-game timeout overrides in config. F1 25 = 600s, others = 180s.

**Pros:**
- Single config change, no code change
- Reversible by config push
- Doesn't touch sim adapter

**Cons:**
- Requires the customer to actually start driving within 600 seconds — still possible to fail
- Adds dead time on legitimate launch failures (10 minutes before "fail" becomes visible to staff)
- Doesn't fix the underlying classification problem
- Customers in long menu sessions (track selection, qualifying setup) still hit it
- Doesn't help BILL-13 — billing session still cancels at the timeout

**Recommendation: ship as a TEMPORARY mitigation if Option A takes more than a session, but not as the permanent fix.** This is a workaround, not a fix.

---

## Fix Option D — Process+window detection for launch verification

**Change:** Use Win32 `GetForegroundWindow` + tasklist for `F1_25.exe` as the launch-verified signal. If F1_25.exe is the foreground window for ≥5 seconds, fire launch-verified.

**Pros:**
- Most accurate "game launched" signal — matches what the customer actually sees
- Decouples from telemetry entirely

**Cons:**
- Adds Win32 API dependency to the launch detection path (currently it's all telemetry-based)
- Window focus can flicker during loading (Steam overlay, EA login, etc.)
- More complex to test
- Scope creep beyond the smallest reversible fix

**Recommendation: future architectural improvement, not the fix for this bug.**

---

## Recommended path forward

**Option A** is the right fix. Here's the proposed sequence:

1. **YOU APPROVE** the Option A approach (or pick a different one)
2. I grep all consumers of `DetectorSignal::UdpActive` to find where the launch verifier reads it (so I know exactly which consumer to switch to `UdpReachable`)
3. I show you the consumer call site BEFORE writing the code
4. I write the patch (4 files: enum, adapter, consumer, test)
5. I run `cargo test -p rc-agent-crate` locally
6. I deploy the new rc-agent binary to **Pod 4 only** (canary, not fleet)
7. **You re-run the same trace** — fire F1 25 from kiosk staff on Pod 4
8. We confirm Pod 4 shows F1 25 staying running, no BILL-13 cancel, no "Fail to start" error
9. If green: deploy to remaining 7 pods + Bono VPS + commit + push + LOGBOOK + Bono notify
10. If red: revert binary on Pod 4, re-investigate, do not deploy fleet-wide

**Estimated risk:** LOW. Option A only adds a new signal variant — existing signals are untouched. Worst case the new signal misroutes and the launch_verifier doesn't pass — same as today's bug, no regression.

**Universal sync targets:**
- Pod 1-8 rc-agent binary (deploy after Pod 4 canary verifies)
- No server changes required
- No cloud (Bono VPS) changes required
- No POS changes required
- No frontend changes required

---

## What I need from you

**Pick one and tell me:**

1. **"Go A"** — proceed with Option A as outlined above
2. **"Go A but verify consumer first"** — grep call sites, show me, then proceed
3. **"Go C as temp mitigation"** — bump F1 25 launch_timeout to 600s, ship today, do A later
4. **"Different option"** — describe what
5. **"Stop, let me think"** — pause; we already have the divergence proof, no rush

I lean toward (2) — verify the consumer call site before writing code. The risk class for this fix is "wrong signal routed to wrong consumer", and one extra grep buys high confidence. But you have context about how often Bug 2 hits real customers that I don't — if it's burning revenue every hour, "Go A" is fine and I'll be careful with the grep during implementation.

**No code changes happen until you respond.**
