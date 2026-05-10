---
title: HALO-MI Design Skeleton v0.1
class: substrate · operational · cross-pilot-affecting
authored: 2026-05-10 IST · bono-LEAD per §S-182.6 #4
gates-on: Captain Q-DECISION (Q-DEC-AUDIT-5) + james AMPLIFIER pre-FILE pass
parent: project_mi_mission_statement_mini_jaeger_20260509.md (V2-MASTER-STATE §S-170)
sibling: project_mi_wave4_readiness_and_asterisk_removal_20260509.md
---

# HALO-MI Design Skeleton v0.1

**Status**: DRAFT-PRE-AMPLIFIER · substrate-class · authored under Captain "uptime → V2 dev" implicit directive 2026-05-10 ~20:05 IST + §S-192.7 item 5 substrate-class autonomous-eligible carve-out

**Closes bravo-slice item 5** (Q-DEC-AUDIT-5 HALO-MI joint design review per PACT-DRAFT-bravo-slice-20260510 §2) at v0.1 design level — concrete enough for Captain Q-DECISION + james AMPLIFIER, deliberately not over-specified for v0.1.

## §0 — Provenance + scope

**Trigger**: §S-170.16 HALO-as-substrate-for-MI architectural finding (msg=35903 outbound 2026-05-10 00:39 IST) is AMPLIFIER-PENDING. v0.1 design-skeleton converts the finding into a reviewable artifact.

**In-scope**:
- HALO ↔ MI organ relationship (peer? substrate? sensor-feed?)
- 12 SK + 8 BK catalog → HALO probe mapping (extending halo-pact-map.json 16→36)
- mesh_kb.db ↔ halo-findings.jsonl integration semantics
- G-1..G-5 asterisk-removal HALO-traceability

**Out-of-scope** (deferred):
- Implementation code (V2 foundational PR class — Captain-explicit-auth required per MMA §2 always-Captain-explicit set scenario 4)
- Specific probe regex / detection patterns (case-by-case post-design)
- Wave 1 PR-D wallet wrapper composition (sibling Wave 1 work; bravo-slice item 2 gates on james AMPLIFIER on PACT-024 §A)

## §1 — Four-layer architectural overlay

The mini-Jaeger frame (per parent §2) establishes 4 architectural layers. HALO position in each:

| Layer | MI position | HALO position | Relationship |
|---|---|---|---|
| **Anatomical** (v2-skeleton/01:35) | Nervous system / six senses / autopilot fallback | Threat detection organ; finds bugs, doesn't fix them (line 31) | Peer organs; HALO feeds MI sensory input via `connection_halo_mi` (auxiliary, 04-connection-matrix.md:125-127) |
| **Cognitive** (PART 51 segment-B) | Curriculum-learner consuming PACT taxonomy | Pattern-detector emitting structured findings | HALO findings = candidate kaiju observations for MI; MI classifies (SK/BK), HALO does not |
| **Doctrinal** (`feedback_mi_functional_doctrine_boundary.md`) | Functional-only; never authors doctrine | Functional-only; never authors fixes | Both organs constrained to functional layer; doctrine is PACT-only |
| **Operational** (mini-Jaeger §S-170) | Mini-Jaeger handling small kaiju autonomously | Threat sensor feeding the mini-Jaeger | HALO = the *eyes* of the mini-Jaeger; MI = the *judgment + response* |

**Architectural choice (v0.1 proposal)**: HALO and MI are PEER ORGANS connected by `connection_halo_mi`, NOT a substrate-of relationship. HALO operates upstream (detection), MI operates downstream (classification + response). The §S-170.16 framing "HALO-as-substrate-for-MI" is RECAST as "HALO-feeds-MI-sensory-input"; no substrate-of inheritance, just connection-class data flow.

**Rationale**: substrate-of would conflate detection (HALO concern) with classification (MI concern); peer + connection preserves separation-of-concerns and lets either evolve independently.

## §2 — `connection_halo_mi` contract (mechanical-muscle spec)

Per parent connection-matrix line 125-127, `connection_halo_mi` is between HALO (threat detection) and MI (nervous system). v0.1 contract:

**Input** (HALO → MI):
| Field | Type | Notes |
|---|---|---|
| `finding_id` | UUID | Unique per HALO emission |
| `detector_class` | enum | probe taxonomy: pod-watchdog · mesh-agent-noise · schema-drift · billing-anomaly · deployment-state · etc. |
| `signal` | JSON | Raw probe output |
| `severity_hint` | enum (info/warn/critical) | HALO's hint, MI re-classifies |
| `timestamp` | IST | Wall-clock fetched from environment |
| `pact_attribution` | string \| null | halo-pact-map.json reference, if mapped |

**Output** (MI → HALO):
| Field | Type | Notes |
|---|---|---|
| `finding_id` | UUID ref | Reference to HALO emission |
| `kaiju_class` | enum (SK-N / BK-N) | MI classifier verdict |
| `disposition` | enum | handled-silently / dashboard-update / pilot-escalation / captain-escalation / awaiting-confidence |
| `mesh_kb_log_row_id` | int FK | kaiju_classification_log table per Wave 4 §1.2 |
| `mi_confidence` | float [0, 1] | Pattern-match similarity score |

**Invariants**:
- HALO never auto-classifies (severity_hint is hint, not verdict)
- MI never modifies HALO findings (read-only consumer)
- Bidirectional but not symmetric — MI may request HALO to re-probe a class; HALO may not request MI to re-classify
- finding_id is the load-bearing FK; halo-findings.jsonl + kaiju_classification_log share it as the join column

## §3 — Small-kaiju → HALO-probe mapping (extends halo-pact-map 16→36)

Per §S-170.17 proposed extension. Each SK class gets one or more HALO probes. v0.1 mapping (one-to-many; some probes detect multiple SK classes):

| SK class | HALO probe(s) | Status | Detection signal |
|---|---|---|---|
| SK-1 routine-empty-window | `pod-utilization-rolling-sample` | NEW | Utilisation < threshold N≥4 historical samples within 30d |
| SK-2 known-fleet-monitor-noise | `mesh-agent-watchdog` (existing) + `pod-8091-recurring-unreachable` (NEW) | EXISTS+NEW | Pod :8091 recurring + never escalated to actual outage in N days |
| SK-3 wallet-HOLD-reconcile | `wallet-hold-vs-final-delta` | NEW (gates Wave 1 PR-D) | abs(provisional - final) < ±tolerance |
| SK-4 capacity-baseline-drift | `capacity-baseline-rolling-stddev` | NEW | Today vs last N same-day-of-week within ±std-dev |
| SK-5 stock-threshold-breach | `consumables-low-water-mark` | NEW (Wave 4+ scope) | Coffee/oil < threshold |
| SK-6 repeat-customer-arrival | `customer-history-lookup` (existing customer module) | EXISTS | Customer with prior session_count > 0 walks in |
| SK-7 daily-revenue-rollup | `revenue-aggregation-eod` (existing/cron) | EXISTS | End-of-day timestamp + aggregate compute |
| SK-8 split-rate-billing | `cross-window-billing-detect` | NEW | Session crosses rate-window boundary mid-flight |
| SK-9 bilateral-live-sync-below-alarm | `live-sync-volume-monitor` (existing) | EXISTS | james↔bono volume < alarm threshold |
| SK-10 routine-pm2-restart | `pm2-restart-correlation` | NEW | PM2 restart within deploy_window of last deploy |
| SK-11 idle-timeout-normal-pattern | `pod-idle-timeout-detector` (existing) | EXISTS | Customer leaves + pod auto-pauses + bill closes |
| SK-12 manager-pill-normal-pattern | `staff-pin-telemetry-class` | NEW (gates on Wave 4 staff_pin_telemetry table per §S-122) | Override within auditable bounds |

**Probe-coverage gap (v0.1 finding)**: 8 of 12 SK classes need NEW HALO probes (SK-1, SK-2 partial, SK-3, SK-4, SK-5, SK-8, SK-10, SK-12). Wave 4 schema enables 3 of those (SK-3 via revenue_paise_*, SK-12 via staff_pin_telemetry, partial SK-4 via empty_window_events). Remaining 5 need new probe authoring post-Wave-4-land. Captain Q-DECISION: probe authoring sequence and timing.

## §4 — Big-kaiju → HALO-detection-class taxonomy

BK classes are ALWAYS escalations — HALO's role is to surface, not to detect-and-classify. Mapping different in nature: HALO detects the *signal*, MI gates on confidence, BK escalation fires when MI confidence drops:

| BK class | HALO detection role | MI gate | Escalation target |
|---|---|---|---|
| BK-1 novel-failure-pattern | Emits novel-pattern signal (no detector_class match) | Confidence below threshold → BK-1 fires | WhatsApp Captain + ping pilots |
| BK-2 doctrine-edge-case | Emits doctrine-conflict signal (e.g., overlapping rate windows) | Halts pre-classification | Captain (PACT territory) |
| BK-3 anomalous-demand-no-driver | Emits demand-spike signal without campaign attribution | Cannot attribute → BK-3 | Intelligence report Captain |
| BK-4 customer-complaint-interpretation | OUT-OF-HALO-SCOPE (comes via comms or POS path) | Receives via separate channel | Pilots — judgment |
| BK-5 mi-confidence-below-threshold | Emits any signal | Similarity-score < threshold | "I don't know" → Captain |
| BK-6 sustained-drift-detection | Emits drift-signal (rolling-window) | Halts proposals | Pilots; doctrine recovery |
| BK-7 cross-pilot-coordination-question | OUT-OF-HALO-SCOPE (bilateral-class) | Direct bilateral surface | Pilots |
| BK-8 captain-challenge-amend-on-prior-mi | OUT-OF-HALO-SCOPE (post-hoc) | Re-classification event | Both pilots; weight update |

**HALO scope boundary**: HALO detects 5 of 8 BK classes (BK-1, 2, 3, 5, 6). BK-4, 7, 8 are out-of-HALO-scope by class definition (interpretation / coordination / post-hoc). Captain Q-DECISION: confirm OUT-OF-HALO-SCOPE boundary or recast as "HALO not-yet-extended-to."

## §5 — `mesh_kb.db` ↔ `halo-findings.jsonl` integration schema

Per parent Wave 4 readiness §1.2 + halo-runner.js operational state (PM2; halo-findings.jsonl 26,227 lines). v0.1 ingestion flow:

```
halo-runner.js (PM2 24/7)
  → halo-findings.jsonl (append-only, source-of-truth)
    → halo-ingest-cron (NEW; */5 * * * *)
      → mesh_kb.db.kaiju_classification_log INSERT (classified_at NULL)
        ↓
        MI classifier reads kaiju_classification_log WHERE classified_at IS NULL
        ↓
        MI runs §S-170.5 5-step ladder
        ↓
        UPDATE kaiju_classification_log SET kaiju_class, disposition, mi_confidence, classified_at
        ↓
        IF disposition IN (pilot-escalation, captain-escalation):
          → mesh_kb.db.escalations INSERT
          → bilateral msg / WhatsApp out-channel
          ↓
          IF disposition == captain-escalation AND no Captain ack within N min:
            → escalations.escalation_state = pending-acknowledge
            → reminder loop (separate concern)
```

**Schema overlap (v0.1 proposal)**:
- `halo-findings.jsonl` = source-of-truth append-only (already exists; do not touch)
- `mesh_kb.db.kaiju_classification_log` = derived state with MI annotations (queryable; new in Wave 4 per parent §1.2)
- `mesh_kb.db.escalations` = derived from kaiju_classification_log filtered on disposition (new in Wave 4)

**Integration invariants**:
- halo-findings.jsonl append-only; mesh_kb derived; can rebuild mesh_kb from JSONL replay (DR primitive)
- kaiju_classification_log foreign-keys halo-findings.jsonl row by `finding_id`
- MI never writes to halo-findings.jsonl; only reads via classifier consumer
- halo-ingest-cron is idempotent — replaying same JSONL row produces same kaiju_classification_log row (UNIQUE on finding_id)

## §6 — G-1..G-5 asterisk-removal HALO-traceability

Per sibling `project_mi_wave4_readiness_and_asterisk_removal_20260509.md` §2.1. Which HALO substrate elements satisfy each gate:

| Gate | What it requires | HALO contribution |
|---|---|---|
| **G-1** Wave 4 MI Ingestion landed | mesh_kb.db schema regenerated with kaiju_classification_log + pact_corpus_index | halo-ingest-cron must be authored + deployed; halo-findings.jsonl piped into kaiju_classification_log |
| **G-2** ≥4 weeks operational data | kaiju_classification_log row count > 4*7*24 events | halo-findings.jsonl already has 26,227 lines pre-Wave 4 schema; backfill ingestion needed once kaiju_classification_log live |
| **G-3** Drift alarm fired + recovered ≥2× | F7 self-monitoring drift detection | HALO probe `mi-prediction-error-rolling` (NEW) detects drift; recovery flow in MI |
| **G-4** Captain ratify pilot-bandwidth-preserved | §S-170.6 metric #1 measurable | kaiju_classification_log outcomes computable; HALO contributes raw event count denominator |
| **G-5** Zero false-negative escalations on big-kaiju 30d | Captain CHALLENGE-AMEND count = 0 | HALO ensures BK-1, 2, 3, 5, 6 detection coverage; BK-4, 7, 8 are out-of-HALO-scope (gated on MI's other input channels) |

**Gating risk (v0.1 finding)**: G-5 depends on HALO covering BK-1, 2, 3, 5, 6 with confidence-floor — if HALO mis-detects (e.g., misses novel-pattern signal), MI doesn't escalate, Captain CHALLENGE-AMEND fires post-hoc, G-5 30d counter resets to 0. **HALO probe-coverage = G-5 leading indicator.** Recommend: monthly false-negative audit comparing MI auto-handled events vs Captain CHALLENGE-AMEND count; rolling 30d zero-target gates "MI stable" declaration.

## §7 — Open questions deferred

1. **Captain Q-DECISION on §1 architectural choice** — peer-organs-with-connection (proposed v0.1) vs HALO-as-substrate-for-MI (original §S-170.16 framing). v0.1 proposes peer for separation-of-concerns; Captain disposition gates §2-§6 detail.
2. **james AMPLIFIER on §2 connection_halo_mi contract** — input/output schema acceptance OR challenge with mods. Bilateral pre-FILE pass per L1 charter 24h silent-AGREE OR substantive disposition.
3. **Probe-authoring sequence** — 8 NEW probes needed for SK coverage. Recommendation: SK-3 wallet-HOLD-reconcile gates Wave 1 PR-D first; SK-1 empty-window gates Wave 2; SK-12 manager-pill gates Wave 4 staff_pin_telemetry; remaining 5 post-Wave-4-land.
4. **halo-ingest-cron implementation timing** — gates on Wave 4 schema land. Pre-author code (substrate-class spec authorable now) OR defer until schema solid (avoid implementation drift). Recommendation: spec now, code post-schema-land.
5. **§3 mapping completeness** — 12 SK classes have proposed probes; coverage gaps explicit. Captain accept gap-acknowledgment OR amend mapping.
6. **§4 BK-4/7/8 out-of-HALO-scope** — confirm explicit out-of-scope boundary, or recast as HALO not-yet-extended-to scope?
7. **Probe-coverage measurement (G-5 leading indicator)** — monthly false-negative audit recommended; Captain confirms cadence + Captain-stake on threshold-cross discipline.
8. **§3.3 catalog-growth (per parent §S-170)** — when SK-13+ class added, halo-pact-map extension protocol — automatic OR Captain-ratify-gated? Recommendation: Captain-ratify-gated (consistent with parent §S-170.3 catalog growth rule N≥2 within 30d AND Captain ratifies PACT).

## §8 — Composes-with

| Memory / substrate | Relationship |
|---|---|
| `project_mi_mission_statement_mini_jaeger_20260509.md` (parent §S-170) | Parent mission anchor; this skeleton implements §S-170.16/17 HALO-as-substrate finding (recast as peer-organs-with-connection) |
| `project_mi_wave4_readiness_and_asterisk_removal_20260509.md` | Sibling forward-looking; §6 traceability honors G-1..G-5 |
| `comms-link/v2-skeleton/01-skeleton-architecture.md:31, 35` | HALO + MI organ definitions |
| `comms-link/v2-skeleton/04-connection-matrix.md:125-127` | connection_halo_mi (auxiliary); this skeleton specifies the auxiliary connection contract |
| V2-MASTER-STATE §S-170.16 + §S-170.17 (AMPLIFIER-PENDING) | The pending findings this skeleton converts to reviewable substrate |
| `racecontrol/halo/` (charter PACT-20260426-040) | HALO operational substrate (halo-runner.js + 35+ probes + 6 catalogs + halo-findings.jsonl 26,227 lines) |
| `feedback_mi_functional_doctrine_boundary.md` | F8 functional-only invariant honored |
| `comms-link/.planning/draft-pacts/PACT-DRAFT-pact-as-teacher-mi-student-curriculum.md` | mi-tag taxonomy that §3 maps populate |
| V2-MASTER-STATE §S-184 + §S-191 | bravo-slice PACT (item 5 closure substrate) |
| `feedback_apply_recommendations_autonomously_20260510.md` | Authoring discipline (substrate-class autonomous-eligible per §S-192.7 deferral lifted post-V2-clarity-prereq) |

---

## Anchor signatures

- Captain "uptime → V2 dev" implicit directive 2026-05-10 ~20:05 IST
- bravo-slice item 5 substrate-class autonomous-eligible per §S-192.7
- bono-LEAD lead-claim retention per §S-182.6 #4
- Wave 0.5 launch event (HALO-MI joint design review) gated james-LEAD per §S-182.6 #4 — this v0.1 skeleton is bono-side prep for that event, not james-LEAD pre-emption
- §S-170 parent mission anchor + §S-170.16/17 AMPLIFIER-PENDING extension targets

— bono · 2026-05-10 IST · v0.1 substrate-class draft · bravo-slice item 5 PRE-AMPLIFIER artifact · 8 sections · ~210 lines · gates on Captain Q-DECISION (Q-DEC-AUDIT-5) §1 architectural choice + james AMPLIFIER pre-FILE pass on §2 contract · §S-N V2-MASTER-STATE entry deferred to next turn (H2 separation)
