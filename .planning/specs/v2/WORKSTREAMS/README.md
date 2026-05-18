# V2 Workstreams — 2026-05-17 split

**Captain direction 2026-05-17 ~15:38 IST:** *"Bono will work on the Skeleton V2 [while] we work on the customer facing organ workflows and identify what is possible."*

This split exists because Captain forced verification of Kiosk → backend wiring and discovered the V2 skeleton (`01-skeleton-architecture.md §2` LOCKED invariant — "no subsystem talks directly to the heart") is **not implemented in code**. Every surface bypasses Admin and writes directly to RaceControl. We have been ratifying V2 doctrine faster than we have been building V2 substrate.

The two streams run in parallel with explicit coordination.

---

## Streams

| Stream | Owner | Handoff | Builds | Produces |
|---|---|---|---|---|
| **Skeleton V2** | Bono (VPS) | [HANDOFF-BONO-SKELETON-V2-20260517.md](HANDOFF-BONO-SKELETON-V2-20260517.md) | 4 skeleton primitives in code (S1 spinal cord · S2 connection contracts · S3 audit boundary · S4 feature-flag service) | Code that survives redeploy + verifiable behavior in production |
| **Customer-facing organ workflows** | James (.27) | [HANDOFF-JAMES-CUSTOMER-ORGANS-20260517.md](HANDOFF-JAMES-CUSTOMER-ORGANS-20260517.md) | Per-element interactive matrix across Kiosk (M1) → Billing (M2) → PWA (M3) | Matrix files + ratified disposition per element + skeleton-dependency rollup |

---

## Coordination protocol

```
James matrix flags `needs-skeleton-X` per element
        ↓
James publishes skeleton-dependency rollup
        ↓
Bono prioritizes primitives by impact-count
        ↓
Bono ships primitive X → ENFORCED
        ↓
James re-classifies all `needs-X` elements → surfaces Captain ratify list
        ↓
Captain ratifies element ADOPT → execution stream begins (separate from both mapping streams)
```

**Invariants:**
- No element ADOPTS until its dependent primitive ENFORCED
- No primitive ships without Captain Wave-N ratify
- No code touches in James stream (mapping only)
- No UI/page work in Bono stream (skeleton only)
- No more §S-N entries that don't reference a delivered primitive or ratified element

---

## What this split closes

| Pattern named 2026-05-17 ~15:33 IST | This split's response |
|---|---|
| Doctrine-ratification velocity outpaces skeleton-build velocity | Bono stream measures progress by primitives ENFORCED, not §S-N entries authored |
| Surface element work codifies V1-shape because skeleton doesn't exist | James stream uses 5-status classification including `CLOSED-LOOP-PENDING-SKELETON`; nothing adopts until skeleton ready |
| Bilateral mirror cascade overhead consumes capacity without moving V2 toward complete | Both streams default to NO CLAUDE.md edits except where doctrine genuinely changes |
| MAOR / F1 / F3 / DEPRECATE-trigger meta-process accretes faster than substrate | Both streams explicitly out-of-scope for §14 amendments |

---

## Stale-at + re-evaluation triggers

- **Weekly Friday review** (Captain + bono + james): primitive deltas, matrix deltas, ratify queue
- **Re-evaluate the split** when: Skeleton primitive S1 reaches ENFORCED (~Wave 1 complete on Bono stream) OR when Module 1 ratify complete on James stream — whichever first
- **Stale-at 2026-06-17** — if no skeleton primitive ENFORCED by then, the split itself needs re-examination (back to Captain for new direction)

---

## Open questions for Captain (consolidated across both handoffs)

**Captain-stake for both streams:**
1. **Doctrine pause:** Should §S-N append-only ledger be frozen during this split (except for primitive-close-anchor + element-ratify-anchor entries)? Current §S-N rate ~24/day is the symptom we're addressing.
2. **V2-PROGRESS-MAP re-baseline:** When does the existing ~30% closed metric get re-stated against skeleton-presence as the yardstick? After R4 (Bono Wave 1 spinal cord ENFORCED) seems right.
3. **Cross-stream cadence:** Daily check or weekly Friday only?

**Captain-stake for Bono stream:**
4. Spinal-cord host: Bono VPS / Server .23 / both?
5. Source-tag enum extension — still authoritative or ZIP affects it?
6. Feature-flag service host?
7. §S-146 per-PR vs per-wave application?
8. `racingpoint-admin` repo merge vs separate?

**Captain-stake for James stream:**
9. W0.1 single-element worked example choice?
10. AMBIGUOUS BATCH default — HIDE autonomous or Q-DEC always?
11. CLOSED-LOOP-V1 disposition — paint over V1 or hold for skeleton?
12. W5 boundary routing — in-module or dedicated wave?

---

## Reading order for new sessions

1. Read this README first (you're here)
2. Read the handoff for your stream (Bono or James)
3. Read the sibling handoff (so you know what the other side is doing)
4. Check for any `MATRIX-DELTA-*.md` (James) or `SKELETON-STATUS.md` (Bono) since last session
5. Verify live state with build_ids before claiming any state
6. Proceed only within your stream's scope
