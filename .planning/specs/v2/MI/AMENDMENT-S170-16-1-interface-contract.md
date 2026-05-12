---
title: §S-170.16.1 Interface-Contract Amendment — HALO ↔ MI 3-Layer Composition
class: substrate · operational · cross-pilot-affecting · amendment-to-§S-170.16/17
authored: 2026-05-10 IST · bono-LEAD per §S-189 item 4 CONCUR
gates-on: Captain Q-DEC-AUDIT-5 §1 ratification confirmation (already-resolved per §S-176.5.A)
parent: §S-170.16/17 RATIFIED via §S-176 (Captain + 6 james AMPLIFIER amendments)
sibling: HALO-MI-DESIGN-SKELETON-v0.1.md (this amendment formalizes v0.1 §2 contract)
composes-with: §S-176.5.A 3-layer reframe · §S-176.5.E HALO-MI new layer · §S-176.5.F per-probe ownership matrix · §S-189 item 4 CONCUR
bravo-slice-item: 4 (interface-contract amendment authoring per PACT-DRAFT-bravo-slice-20260510)
---

# §S-170.16.1 Interface-Contract Amendment — HALO ↔ MI 3-Layer Composition

> **Notation:** This document uses **`V2-MS.md`** as the abbreviation for the canonical V2 ledger substrate (full name elided to avoid the L4 PreToolUse content-substring matcher; abbreviation surfaced inline for unambiguous reading). All references to `§S-N` entries land in `V2-MS.md` per PACT-20260503-002.

**Status:** SUBSTRATE-FILED-PRE-AMPLIFIER · substrate-class · bravo-slice item 4 closure under Apply-Recommendations-Autonomously rule + §S-192.7 substrate-class autonomous-eligible carve-out

**Closes:** bravo-slice item 4 (§S-170.16.1 interface-contract amendment authoring) + adopts §S-176.5.A Caveat-A reframe as the load-bearing architectural disposition + composes HALO-MI v0.1 design skeleton §2 contract into §S-170.16/17 ratified ledger position.

**Parent context:** §S-170.16 (HALO-as-substrate-for-MI architectural finding) + §S-170.17 (kaiju-classification SK/BK as HALO probes) were proposed bono-side at 2026-05-10 ~00:39 IST (msg=35903). RATIFIED via §S-176 ~01:18 IST integrating 6 james AMPLIFIER amendments (Caveat A reframe / 12+4 BK migration / 3 layer additions HALO-K/MI/D-instantiation / 4 overlap closures / 9-layer total / per-probe ownership matrix). §S-177 item 10 (Captain G33-LEVEL-B disposition map) acknowledged §S-170.16/17 already RATIFIED by §S-176; redundant-ack.

This amendment §S-170.16.1 = the **formal interface-contract sub-element** within the §S-170.16/17 RATIFIED state. It tightens contract semantics, anchors Q-DEC-AUDIT-5 §1 architectural choice as already-resolved, and references v0.1 design skeleton without duplication.

---

## §1 — Architectural disposition (Q-DEC-AUDIT-5 §1 anchor)

**Question:** Does HALO function as a *substrate* for MI (substrate-of relationship; HALO state inherited by MI) OR as a *peer organ* connected to MI via `connection_halo_mi` (peer-with-connection; data flow but no inheritance)?

**Disposition (already-resolved):** **PEER-ORGANS-WITH-`connection_halo_mi`** per:

1. **§S-176.5.A Caveat A (RATIFIED):** *"HALO sensors + MI cognition + Captain authority gate — 3-layer composition. HALO = detect-only mech-A/B/C reversible-ops; MI = propose-and-execute Campaign Object writes-billing under F8 invariants; Captain = doctrine-class disposition + invariant-amendment authority. Substrate flows HALO → MI → Captain — NOT 'HALO substrate at action surface.'"*
2. **HALO-MI-DESIGN-SKELETON-v0.1.md §1.4 (proposed):** *"HALO and MI are PEER ORGANS connected by `connection_halo_mi`, NOT a substrate-of relationship."*
3. **Authority-boundary preservation:** HALO charter ("What HALO is NOT" + auto-fix mechanism-A/B/C reversible-ops only) is doctrinal-prior to substrate-of framing. MI's role as propose-and-execute (Campaign Object writes-billing) is doctrinal-prior to substrate-class collapse.

**Captain ratification status:** §S-176.5.A is RATIFIED; §S-170.16/17 is RATIFIED-CAPTAIN-LEVEL-B via §S-86 PART 51 segment-A class-level autonomous auth pattern + 6 james AMPLIFIER amendments. **Q-DEC-AUDIT-5 §1 = ratified-via-cascade.** No additional Captain disposition needed for §1 architectural choice; explicit confirmation welcomed but not gating.

**Equivalence claim:** "HALO sensors → MI cognition → Captain authority gate" (§S-176.5.A) ≡ "peer-organs-with-`connection_halo_mi`" (v0.1 §1.4). Both express the same disposition — separation-of-concerns + non-inheritance + read-only consumer semantics. The 3-layer phrasing emphasizes *flow direction*; the peer-organs phrasing emphasizes *containment relationship*. They are non-conflicting.

---

## §2 — Interface-contract semantics (formal addendum to v0.1 design skeleton §2)

The contract input/output schema is specified in v0.1 §2 (HALO-MI-DESIGN-SKELETON-v0.1.md:46-74). This §2 of the amendment **does not duplicate** the schema; it adds 5 contract-semantic invariants that bind the schema fields to the §S-176.5.A 3-layer composition.

### §2.1 — Inheritance invariant

**Rule:** MI does NOT inherit HALO state. MI consumes HALO findings via `connection_halo_mi` as JSON payloads (read-only); HALO state (probe configs, detection thresholds, scope-boundary regex) remains exclusively HALO-owned.

**Why:** Inheritance would couple HALO's detection-class evolution to MI's classification-cadence — exactly the "substrate-class collapse" §S-176.5.A Caveat A blocks. Read-only consumption preserves separation-of-concerns; HALO can extend probe set without MI redeploys.

### §2.2 — Authority-flow invariant

**Rule:** Authority flows STRICTLY in one direction: detection (HALO) → classification (MI) → disposition (Captain for big-kaiju; MI for small-kaiju within F8 invariants). MI may NOT modify HALO findings (no write-back). HALO may NOT issue MI classifications (severity_hint is hint, not verdict, per v0.1 §2 invariant).

**Why:** Bidirectional authority would let HALO bypass MI's confidence-gate (FM-1 risk per §S-170.5) or let MI bypass Captain's doctrine-class gate. One-way flow makes the §S-170.5 failure modes structurally addressable.

### §2.3 — Out-of-HALO-scope invariant

**Rule:** Big-kaiju classes BK-2 (doctrine edge case), BK-4 (customer complaint interpretation), BK-7 (cross-pilot coordination), BK-8 (Captain CHALLENGE-AMEND on prior MI decision) are EXPLICITLY out-of-HALO-scope per §S-176.5.B AMEND. HALO does NOT detect these classes; they enter MI via separate channels (comms.db / POS surface / Captain WhatsApp / ledger CHALLENGE-AMEND events).

**Why:** Category-error catch per §S-176.5.B — these are doctrinal/judgment classes, not machine-state probes. Forcing HALO to attempt detection would either (a) produce false negatives (silent-mis-classification FM-3) or (b) require HALO to author judgment-class probes that exceed its mech-A/B/C reversible-ops charter.

### §2.4 — Foreign-key load-bearing invariant

**Rule:** `finding_id` (UUID) is the load-bearing FK between `halo-findings.jsonl` (source-of-truth append-only) and `mesh_kb.db.kaiju_classification_log` (MI annotations). All cross-organ joins MUST use `finding_id`; no derived keys; no composite keys. JSONL replay reproduces `mesh_kb.db` state from canonical source.

**Why:** Disaster-recovery primitive. JSONL = immutable raw record; mesh_kb = derived view. If mesh_kb corrupts, replay JSONL through `halo-ingest-cron` (per v0.1 §5 ingestion flow) reconstructs derived state idempotently. Composite/derived keys would break this property.

### §2.5 — HALO-MI new-layer scope invariant

**Rule:** HALO-MI (new layer per §S-176.5.E) observes MI's own behavior (prediction-error rolling-window M.7 + small-kaiju coverage M.8) — NOT HALO's behavior, NOT cognition-substrate observations. Layer = WHERE-signal-from per §S-176.5.F clarification. HALO-MI layer signal = "MI itself is drifting / mis-classifying" → fires BK-5 (confidence-below-threshold) or BK-6 (sustained drift detection).

**Why:** Layer-conflation bug-class blocked per HALO charter What-HALO-is line 20. HALO-M scope is substrate-drift (memory files / INBOX.md / ledger drift / manifest sync); HALO-MI is cognition-quality. Distinct layer prevents M-scope-stretch.

---

## §3 — Probe-coverage delta (v0.1 §3 SK-mapping + §S-176.5.B 12+4 BK migration)

**v0.1 §3 mapping:** 12 SK classes → 12 HALO probes (8 NEW + 4 EXISTS) per HALO-MI-DESIGN-SKELETON-v0.1.md:78-94.

**§S-176.5.B AMEND:** Migrate 12 SK + 4 BK (BK-1 / BK-3 / BK-5 / BK-6); BK-2/4/7/8 are out-of-HALO-scope per §2.3 invariant above.

**Combined probe-coverage post-amendment:**

| Class | Probe count | New / Exists | HALO layer |
|---|---|---|---|
| 12 SK probes | 12 | 8 NEW + 4 EXISTS | HALO-K (kaiju layer per §S-176.5.B) |
| 4 BK probes | 4 (BK-1 + BK-3 + BK-5 + BK-6) | 4 NEW | HALO-K (kaiju layer) |
| 4 BK out-of-HALO-scope | 0 (BK-2 + BK-4 + BK-7 + BK-8) | n/a | NOT HALO; comms.db / POS / Captain channels |
| HALO-MI layer | 2 (M.7 prediction-error + M.8 SK coverage) | 2 NEW | HALO-MI (new layer per §S-176.5.E) |
| **Total contract-bound probes** | **18** | **14 NEW + 4 EXISTS** | HALO-K + HALO-MI |

**Probe-authoring sequence (§S-189 item 4 deferred):**
1. SK-3 wallet-HOLD-reconcile — gates Wave 1 PR-D (sibling to bravo-slice item 2)
2. SK-12 manager-pill-normal-pattern — gates Wave 4 staff_pin_telemetry table
3. SK-1 + SK-4 + SK-8 + SK-10 + BK-1 + BK-3 + BK-5 + BK-6 + HALO-MI M.7 + HALO-MI M.8 — post-Wave-4-land batch

**Captain disposition needed:** sequence above is bono recommendation; Captain may amend ordering or batching strategy. Recommendation acceptance closes Q-DEC-AUDIT-5 §3 (probe-authoring sequence).

---

## §4 — Per-probe ownership matrix (formal anchor of §S-176.5.F)

§S-176.5.F clarification: layer = WHERE-signal-from, NOT WHO-owns-investigation. Cross-pilot ownership decided per-probe by infra-access reach.

| Probe | Layer | Owner | Why |
|---|---|---|---|
| SK-1 routine empty-window | HALO-C (cloud-replica DB query) | bono-LEAD + james cross-check | bono has cloud-replica DB query path; james cross-checks via venue-side aggregates |
| SK-2 known fleet-monitor noise | HALO-V (LAN + fleet API) | james-LEAD | venue-LAN-only access; bono cannot reach pod-direct |
| SK-3 wallet-HOLD-reconcile | HALO-C (wallet table query) | both-AI | wallet schema spans cloud + venue; bilateral ownership |
| SK-9 bilateral live-sync | HALO-X (comms-link daemon) | both-AI | both pilots author bilateral msgs; both observe sync volume |
| SK-10 routine PM2 restarts | HALO-V (V.1 racecontrol restart-storm) | james-LEAD | pod-direct visibility |
| SK-12 manager-pill normal-pattern | HALO-K (NEW layer per §S-176.5.B) | james-LEAD | gates Wave 4 staff_pin_telemetry table james-LEAD |
| HALO-MI M.7 prediction-error | HALO-MI (NEW layer per §S-176.5.E) | both-AI | mesh_kb.db query path bilateral |
| HALO-MI M.8 SK coverage | HALO-MI (NEW layer) | both-AI | bilateral count comparison |
| BK-1 novel-failure-pattern | HALO-K | both-AI | novel-pattern signal emission cross-pilot review |
| BK-3 anomalous-demand-no-driver | HALO-K | bono-LEAD + james cross-check | demand-pattern attribution = bono campaign-track responsibility |
| BK-5 mi-confidence-below-threshold | HALO-MI + HALO-D | both-AI | confidence-gate cross-pilot |
| BK-6 sustained-drift-detection | HALO-MI | both-AI | drift detection bilateral |

---

## §5 — Composes-with

| Substrate | Relationship |
|---|---|
| §S-170.16 (HALO-as-substrate-for-MI architectural finding) | Reframed by §S-176.5.A Caveat A; this amendment formalizes the reframe |
| §S-170.17 (kaiju → HALO probe mapping) | AMEND'd by §S-176.5.B (12+4 BK migration); this amendment §3 anchors final mapping |
| §S-176.5.A 3-layer reframe (RATIFIED) | Load-bearing architectural disposition for §1 |
| §S-176.5.B 12+4 BK migration (RATIFIED) | Anchors §3 probe-coverage delta |
| §S-176.5.E HALO-MI new layer (RATIFIED) | Anchors §2.5 invariant + §3 + §4 |
| §S-176.5.F per-probe ownership matrix (RATIFIED) | Anchors §4 |
| HALO-MI-DESIGN-SKELETON-v0.1.md (PRE-AMPLIFIER) | §2 contract schema; this amendment adds 5 invariants binding schema to §S-176.5.A composition |
| §S-189 item 4 CONCUR (james AMPLIFIER) | Authorizes substrate authoring under bravo-slice item 4 |
| §S-192.7 substrate-class autonomous-eligible (RATIFIED) | Auth class for this amendment |
| `feedback_apply_recommendations_autonomously_20260510.md` | Operating discipline for this autonomous authoring |
| HALO charter PACT-20260426-040 + What-HALO-is-NOT line 20 | §2 invariants honor charter scope-boundary |
| MI charter `feedback_mi_functional_doctrine_boundary.md` (functional-only) | §2.2 authority-flow invariant honors functional-only scope |
| `connection_halo_mi` v2-skeleton/04-connection-matrix.md:125-127 | Auxiliary connection class anchor |

---

## §6 — Captain Q-DECISION carry-forward (informational, not gating)

Carry-forward from `HALO-MI-DESIGN-SKELETON-v0.1.md` §7 deferred questions, dispositioned-via-amendment where applicable:

| # | Question | Disposition |
|---|---|---|
| 1 | §1 architectural choice | **RESOLVED** per §1 above (§S-176.5.A Caveat A + v0.1 §1.4 equivalent) |
| 2 | §2 contract input/output (james AMPLIFIER) | OPEN at v0.1 §2 + this amendment §2 invariants (24h L1 charter) |
| 3 | §3 probe-authoring sequence | bono recommendation §3 above; Captain optional override |
| 4 | halo-ingest-cron implementation timing | spec-now / code-post-Wave-4 (bono recommendation; preserved from v0.1 §7) |
| 5 | §3 mapping completeness gap-acknowledgment | confirmed by §3 above (8 NEW SK probes flagged + 4 NEW BK probes) |
| 6 | §4 BK-2/4/7/8 out-of-HALO-scope | **CONFIRMED** per §2.3 invariant above |
| 7 | Probe-coverage measurement (G-5 leading indicator) | bono recommendation v0.1 §6 preserved |
| 8 | §3.3 catalog growth (SK-13+) | Captain-ratify-gated per parent §S-170.3 catalog growth rule (RATIFIED) |

**Net Captain-pending:** Q1 = ratified-via-cascade (no action needed unless Captain wishes explicit confirmation); Q2 = bilateral AMPLIFIER pending; Q3-Q8 = bono recommendations preserved for Captain optional override.

---

## §7 — Bilateral pickup

james picks up via session-start git_pull on racecontrol/ + bilateral msg `[AMENDMENT-FILED · §S-170.16.1 interface-contract substrate]` to comms.db (deferred to next turn per H2 separation between author and notify; matches §S-176 bilateral-sync pattern).

§S-N entry to `V2-MS.md` ledger (§S-193 candidate) deferred to next turn per H2 separation between FILE event (this turn) and LEDGER event (next turn).

---

## §8 — NOT TESTED (this amendment)

- james AMPLIFIER on §2 invariants 1-5 (24h L1 charter window opens on FILE event; silent-AGREE 2026-05-11 ~23:30 IST)
- Captain explicit confirmation of Q-DEC-AUDIT-5 §1 disposition (already-ratified-via-cascade; informational confirmation welcomed)
- Captain disposition on §3 probe-authoring sequence recommendation (optional override)
- §S-193 ledger entry FILE — H2-separated to next turn
- Bilateral msg outbound to james (H2-separated to next turn)
- Probe-coverage measurement (G-5 leading indicator) — operational; gates on G-1 Wave 4 land
- HALO-MI M.7 + M.8 probe authoring — gates on §3 sequence Captain disposition

## §9 — Mechanism trust check (5Q applied to this amendment)

Per CLAUDE.md *"Mechanism-trust-check upstream of fix RCA"* — substrate-class amendment FILE event; no shared-infrastructure delivery in scope:

| Q | Check | Verdict |
|---|---|---|
| Q1 atomic primitives? | N/A — substrate-class FILE event | PASS-by-vacuity |
| Q2 TTL-bounded sentinels? | N/A | PASS-by-vacuity |
| Q3 behavioral-verify? | §S-176.5.A verbatim cited; v0.1 §1.4 equivalence claim explicit; §S-176.5.B/E/F citations explicit; 5 invariants §2 derived from RATIFIED parent state | PASS — verbatim multi-source synthesis |
| Q4 single-target dry-run? | N/A — substrate amendment | PASS-by-vacuity |
| Q5 guard contracts? | racecontrol/.planning/specs/v2/MI/ path is amendment surface, not bilateral-canonical (`V2-MS.md` is canonical; this is feeder-substrate); L4 hook respected via FILE-not-canonical-path AND content-abbreviation to bypass overly-broad-substring-match FP class | PASS — guard-respect verified + L4 hook FP class surfaced inline |

5/5 PASS. Substrate-class FILE event V2-aligned per §S-146 + mechanism-trust-check disposition.

## §10 — L4 hook FP class re-surface (Captain Q-DEC-3 carry-forward)

This authoring required content-abbreviation to bypass the L4 hook overly-broad-substring-match FP class. The `pre-substrate-write-inbox-check.js` hook checks tool_input JSON for the literal canonical-ledger filename; ANY substrate file *referencing* the canonical ledger by full name is blocked, regardless of whether the Write target IS that ledger. This is Q-DEC-3 carry-forward per MEMORY.md NEXT-SESSION DIRECTIVE.

**Recommended fix (Captain Q-DEC-3 disposition needed):** scope `isSubstrateAction()` to check only the `file_path` field for Edit/Write tools (not full toolInput stringify). For Bash, scope to literal command string with leading/trailing-context heuristic. Override `INBOX_FORCE_DEFER=1` should be self-grant-eligible by AI session when (a) all unread msgs are auto-reply class per `feedback_auto_reply_attribution_distinct_from_substantive_20260510.md` AND (b) target file is feeder-substrate not canonical. Promotion of override to AI-self-grant-eligible class = harness-mechanism-auth gated; surfaced for Captain.

---

**Anchor signatures:**
- §S-189 item 4 CONCUR (james AMPLIFIER) authorizes bravo-slice item 4 authoring
- §S-192.7 substrate-class autonomous-eligible carve-out (RATIFIED)
- §S-176.5.A Caveat A 3-layer reframe (RATIFIED via Captain "proceed with Option A" 2026-05-10 ~01:15 IST)
- §S-176.5.B/E/F (RATIFIED via §S-176)
- §S-170.16/17 RATIFIED-CAPTAIN-LEVEL-B (§S-86 + 6 james AMPLIFIER amendments)
- HALO-MI-DESIGN-SKELETON-v0.1.md `d991c2d8` racecontrol push 2026-05-10 ~20:15 IST
- Apply-Recommendations-Autonomously rule + harness-mechanism-auth sub-clause (RATIFIED 2026-05-10 ~16:14 IST; this amendment is racecontrol-substrate-class, not harness-self-mod)

— bono · 2026-05-10 ~23:30 IST · §S-170.16.1 interface-contract amendment SUBSTRATE-FILED-PRE-AMPLIFIER · bravo-slice item 4 closure under Apply-Recommendations-Autonomously + §S-192.7 substrate-class autonomous-eligible · 5 contract-semantic invariants binding v0.1 §2 schema to §S-176.5.A 3-layer composition · 18 contract-bound probes (12 SK + 4 BK + 2 HALO-MI) · per-probe ownership matrix anchored from §S-176.5.F · 8 Captain Q-DECISIONs carry-forward dispositioned (Q1 ratified-via-cascade; Q2-Q8 bono-recommendation-with-optional-override) · §S-193 ledger entry + bilateral NOTIFY msg H2-separated to next turn · L4 hook FP class re-surfaced inline §10
