# HANDOFF — James — Customer-facing organ workflows — 2026-05-17

**Authored:** 2026-05-17 ~15:38 IST by james (self-stream pickup)
**Class:** forward-looking workstream initiation (NOT a session-completion handoff)
**Sibling stream:** [HANDOFF-BONO-SKELETON-V2-20260517.md](HANDOFF-BONO-SKELETON-V2-20260517.md)
**Meta:** [WORKSTREAMS/README.md](README.md)

---

## 1. Owner + scope summary

**Owner:** James (.27 — `C:\Users\bono\racingpoint\racecontrol` + venue-physical access to Server .23, Pods 1-8, POS .130, Kiosk .23)

**Scope:** Identify what is **possible** on each customer-facing organ — today, with V1 substrate, AND going forward as Bono delivers each skeleton primitive. Per-element closed-loop classification. No code touches; mapping + classification only.

**Three modules, in order:**

| # | Module | Definition | Route count |
|---|---|---|---|
| **M1** | **Kiosk** | All `kiosk/src/app/` routes — staff-facing + customer-visible | 14 |
| **M2** | **Billing** | Joint 2 cross-surface elements: PWA `/wallet/*` + POS `web/billing/*` + Kiosk top-up + auto-bill backend touchpoints. Elements belong to the joint where first encountered; no double-mapping in PWA module. | ~10-15 cross-surface elements |
| **M3** | **PWA** | All `pwa/src/app/` routes EXCEPT what was mapped in M2 | 32 minus M2 overlap |

**Per-element template** (ratified by Captain implicit acceptance 2026-05-17 — see this session log):

Metadata · Closed-loop workflow trace · Classification (4-status) · Feature flag (per ZIP §4) · Implementation gap · Decisions referenced.

**Classification taxonomy (5-status, including skeleton dependency):**

| Status | Means | Default disposition |
|---|---|---|
| **CLOSED-LOOP-V1** | Complete trace today on V1 substrate (direct-to-RaceControl). Works in production. | ADOPT-AS-IS (paint only) |
| **CLOSED-LOOP-PENDING-SKELETON** | Workflow traces end-to-end IF Bono skeleton primitive X is ENFORCED. Currently blocked by primitive X. | ADOPT after primitive X ENFORCED |
| **AMBIGUOUS-NEEDS-WORKFLOW** | Element exists but workflow incomplete (missing precondition / postcondition / error path / substrate write trace). | HIDE unless workflow completed in module-end ratify |
| **UNPLANNED-FEATURE** | ZIP designs it but no v2.0 doctrine exists (e.g. Lap Compare ghost-driver, Driver Class, Auto-refund, Auto-promote, Share-lap). | HIDE per ZIP §5 HELD pattern |
| **OUT-OF-V2-SCOPE** | Beyond v2.0 ship list per `03-principles §Deferral Roadmap` (Instagram, customer email, F1-themed coordination). | HIDE firmly |

---

## 2. Doctrine anchors (load FIRST every session)

| Anchor | Purpose |
|---|---|
| `comms-link/v2-skeleton/01-skeleton-architecture.md §Surfaces` | 7 surfaces — scope per organ |
| `comms-link/v2-skeleton/02-flows-and-roles.md` | Customer journey + multi-profile + demand-creation loop |
| `comms-link/v2-skeleton/05-definition-of-done.md §1.2 + §2 + §2.B` | Canonical day · failure modes · per-subsystem failure modes (~80 enumerated) |
| `~/.claude/projects/C--Users-bono/memory/project_v2_customer_workflows_consolidated_20260503.md` | 5 base + 6 missed scenarios + CR-1..CR-8 + 30-item v2.0 list |
| `C:\Users\bono\Downloads\Racing Point eSports.zip` — `tokens.jsx` + `HANDOFF.md` + 6 page artboards + `components.jsx` | Captain-commissioned 2026-05-02 design + architecture spec; authoritative theme + element design source |
| `~/.claude/projects/C--Users-bono/memory/feedback_ratify_vs_implementation_substrate_split_20260516.md` | Discipline anchor: ratify ≠ build. Every classification must verify against current code, not memory. |
| **Sibling stream:** [HANDOFF-BONO-SKELETON-V2-20260517.md](HANDOFF-BONO-SKELETON-V2-20260517.md) | Skeleton primitive status — read before classifying any element |

---

## 3. Trigger context — why this stream exists now

Same trigger as sibling Bono stream. Captain dialogue 2026-05-17 ~15:30 IST verified Kiosk → RaceControl direct bypass of Admin spinal cord. Doctrine §2 LOCKED invariant violated by every surface.

Captain verbatim 2026-05-17 ~15:33 IST: *"We have not been following the Racing Point ecosystem V2 doctrine."*

Captain workstream split direction 2026-05-17 ~15:38 IST: *"Bono will work on the Skeleton V2 [while] we work on the customer facing organ workflows and identify what is possible."*

**This stream identifies what's possible. Bono's stream builds what makes more possible.** The two together replace doctrine-ratification velocity with skeleton-build + workflow-clarity velocity.

---

## 4. Current state enumeration (live-probe markers — 2026-05-17 ~15:38 IST)

```
racecontrol HEAD = origin/main = 8da500c7
comms-link HEAD = origin/main = 98cf6558

Server .23 (venue):  build_id=3561b5c9  status=degraded (FLEET_PARTIAL 0/8 pods + WHATSAPP_UNREACHABLE)
Bono VPS (cloud):    build_id=1d0218ff  status=ok

Customer-facing surface enumeration:
  racecontrol/pwa/      → 32 routes (customer-facing, verified existence + functional spot-check on 25/32)
  racecontrol/kiosk/    → 14 routes (7 customer-visible + 7 staff-visible)
  racecontrol/web/      → 47 routes (mostly ops; 6 venue-display public routes)
  racecontrol/web-v2/   → 3 routes (V2 brand site)

ZIP design bundle:
  Location: C:\Users\bono\Downloads\Racing Point eSports.zip (2026-05-02, 17 design elements)
  Adoption: NOT YET — current canonical 10-ui-design-system.md is acknowledged-fallback
  Captain ratify: ADOPT (this session) — pending Module 1 ratify to operationalize

Skeleton primitives (from sibling stream): 0 of 4 ENFORCED
  → Every CLOSED-LOOP-PENDING-SKELETON element is blocked on Bono delivery
```

UNREACHABLE: pods 1-8 currently. Affects live-behavior verification for pod-state-channel elements; mapping continues without live verification, flagged as NOT-LIVE-VERIFIED.

---

## 5. Goal + success criteria

**Goal:** Every customer-facing interactive element in M1+M2+M3 has a ratified row in the matrix. Each row carries: workflow trace · classification · skeleton-dependency · disposition · feature flag · implementation gap.

**Success criteria — per module:**

| Module | Module-complete means |
|---|---|
| **M1 Kiosk** | All 14 routes mapped; all elements classified; AMBIGUOUS BATCH resolved by Captain; CLOSED-LOOP rows ready-for-ADOPT (gated on skeleton primitive readiness) |
| **M2 Billing** | All Joint 2 cross-surface elements mapped; cross-surface consistency reqs (2s wallet sync) named per element; source-tag completeness per `05-DoD §3.3` verified per element |
| **M3 PWA** | Remaining 32-routes-minus-M2-overlap mapped; UNPLANNED-FEATURE batch documented (likely large given ZIP scope > V2.0 scope); v2.x vs v2.1 deferral disposition surfaced |

**Module-end deliverable shape:**
- `V2-INTERACTIVE-ELEMENT-MAP/01-kiosk/<page>.md` — one per route (14 files)
- `V2-INTERACTIVE-ELEMENT-MAP/02-billing/<element-class>.md` — per cross-surface element class
- `V2-INTERACTIVE-ELEMENT-MAP/03-pwa/<page>.md` — one per route
- `V2-INTERACTIVE-ELEMENT-MAP/00-FORMAT-SPEC.md` — the per-element template (ratify before module work)
- `V2-INTERACTIVE-ELEMENT-MAP/README.md` — index + status dashboard + skeleton-dependency rollup

**Non-goal:** Code commits. Theme adoption. Component implementation. Page refactors. This stream is mapping + classification + recommendation only.

---

## 6. Work breakdown (waves)

**Wave 0 — Format ratify** (no module work yet)
- W0.1 Author `00-FORMAT-SPEC.md` — per-element template + 5-status classification + skeleton-dependency convention + AMBIGUOUS BATCH resolution protocol
- W0.2 Single-element worked example (recommend `pwa.wallet-topup.preset-grid` since it touches Joint 2 + has clear V2 doctrine backing)
- **R0 Captain ratify gate:** format locked before any module enumeration

**Wave 1 — Module 1 Kiosk** (14 routes)
- W1.1 Enumerate elements per route (from `kiosk/src/app/<route>/page.tsx` + ZIP `kiosk-screens.jsx` artboard)
- W1.2 Classify each element (5-status)
- W1.3 Trace closed-loop workflow for CLOSED-LOOP-V1 + CLOSED-LOOP-PENDING-SKELETON rows
- W1.4 Collect AMBIGUOUS BATCH + UNPLANNED-FEATURE BATCH per page
- W1.5 Flag skeleton dependencies per element (`needs-S1` / `needs-S2` / `needs-S3` / `needs-S4`)
- **R1.a Captain ratify gate:** CLOSED-LOOP map reviewed
- **R1.b Captain resolve gate:** AMBIGUOUS BATCH resolved (per element: complete workflow / replace / hide)

**Wave 2 — Module 2 Billing**
- Same shape as W1 but scope = cross-surface Joint 2 element class, not per-page
- Special attention to 2s wallet consistency rule + source-tag enum completeness
- **R2.a + R2.b gates**

**Wave 3 — Module 3 PWA**
- Same shape as W1 but scope = 32 PWA routes minus M2 overlap
- Expect large UNPLANNED-FEATURE batch (ZIP designs Lap Compare / Driver Class / Coaching / Share-lap which are not in v2.0)
- **R3.a + R3.b gates**

**Wave 4 — Cross-module synthesis**
- W4.1 Aggregate `needs-skeleton-X` rollup → publish to Bono stream for prioritization
- W4.2 Author cross-module dependency map (element X on PWA depends on element Y on Kiosk via skeleton primitive Z)
- W4.3 V2-PROGRESS-MAP re-baseline against new matrix (replaces §S-N-count metric with elements-ADOPTED metric)

**Wave 5 — Disposition queue for boundary cases**
- W5.1 PWA `/terminal` (security concern: customer-facing cmd.exe access) — Captain Q-DEC: gate / remove / keep
- W5.2 PWA `/ai` (customer-facing AI chat to Bono backend) — Captain Q-DEC: scope / defer / remove
- W5.3 PWA `/staff/diagnosis` (staff route on customer PWA) — Captain Q-DEC: move to Kiosk staff surface / remove
- W5.4 Web public routes (spectator/leaderboard-display/leaderboards/presenter/results/policy) — module assignment
- W5.5 Web-v2 (/, /privacy) — module assignment
- W5.6 WhatsApp message templates + Marketing redirect handlers — module assignment

---

## 7. Coupling

| Coupling | Detail |
|---|---|
| **Bono stream → this stream** | Every CLOSED-LOOP-PENDING-SKELETON element is gated on a Bono primitive flipping to ENFORCED. When that happens, this stream re-classifies + surfaces Captain ratify. |
| **This stream → Bono stream** | Matrix `needs-skeleton-X` rollup feeds Bono prioritization. Skeleton primitives that unblock more CLOSED-LOOP-PENDING-SKELETON elements ship first. |
| **External: ZIP design bundle** | Authoritative for element design + theme tokens + components.jsx primitives. Every element references ZIP artboard ID where applicable. |
| **External: live behavior verification** | Customer-day workflow trace requires Playwright probes on target browsers (POS browser, Kiosk Edge, James .27 chromium). NOT mocked. NOT memory-projected. |
| **External: V2-MASTER-STATE §S-N freeze** | This stream does NOT append §S-N entries unless they ratify-close a specific element or module. No meta-process §S-N. |

---

## 8. Coordination interface

| Cadence | What |
|---|---|
| **Read sibling status before module work** | Open `HANDOFF-BONO-SKELETON-V2-20260517.md` + `SKELETON-STATUS.md` (when Bono publishes it). Re-classify any `needs-X` rows where X has flipped ENFORCED since last session. |
| **Publish matrix delta per session** | Append `MATRIX-DELTA-YYYYMMDD.md` to WORKSTREAMS/ with: rows added, rows reclassified, AMBIGUOUS BATCH growth, skeleton-dependency-rollup delta. |
| **Weekly cross-stream review** (Friday, Captain + bono + james) | This stream reports element-count deltas; Bono reports primitive-status deltas; Captain ratifies priorities for next week. |
| **AMPLIFIER round-trips** | Cross-pilot AMPLIFIER not required for this stream (mapping is single-pilot work). Bono AMPLIFIER may be requested by Captain on specific module ratifies. |
| **Universal Sync** | NOT TRIGGERED by this stream unless an element classification surfaces a doctrine gap that requires CLAUDE.md updates. Default: this stream produces matrix files only, no CLAUDE.md edits. |

---

## 9. Per-target scope

| Target | In scope? | Notes |
|---|---|---|
| James .27 (`C:\Users\bono\racingpoint\racecontrol`) | YES — primary | Matrix authoring + Playwright probes |
| Server .23 (venue) | YES — live-behavior probe target | Playwright probes Kiosk pages on .23:3300; web pages on .23:3200 |
| Pods 1-8 | YES — for pod-state-channel customer-visible screens (spectator/blanking/preview-idle/preview-hud/pod-detail) | Live-behavior verification per pod when pods online |
| POS .130 | YES — Playwright probe from POS browser (NOT James .27 substitution) | Per H3 — if customer browser is POS, evidence must come from POS |
| Kiosk .23 (Edge browser on Server .23 console) | YES — live-behavior verification | Customer-visible screens render here |
| Bono VPS | N/A — this stream operates on James side | Bono is sibling stream |
| Comms-link relay (8765/8766) | N/A — coordination only | Used for cross-pilot sync messages, not mapping work |
| Cloud apps (racingpoint-web-v2 etc.) | YES — Playwright probe from external browser pointing at cloud | Cloud rendering ≠ venue rendering; both must be verified |

---

## 10. Out-of-scope / NOT INCLUDED

- ANY code commits (no `git commit` during this stream — pure mapping)
- ANY theme adoption work (Orbitron→Chakra Petch, token swaps, font wiring)
- ANY ZIP component port (components.jsx → shadcn/ui primitives)
- ANY page refactors or element implementation
- Skeleton primitive design (Bono's stream)
- Feature-flag service implementation (Bono's stream)
- Substrate schema changes (Bono's stream)
- Doctrine §S-N ratifications EXCEPT module-end ratify anchors
- Bilateral mirror cascades (this stream produces no CLAUDE.md edits by default)
- §14 amendment authoring
- MAOR / F1 / F3 / DEPRECATE-trigger meta-process engagement
- Web/* and Web-v2/* routes — those go to W5.4 / W5.5 disposition; not mapped in M1/M2/M3
- WhatsApp + Marketing organs — W5.6 disposition; not in M1/M2/M3
- Email + Instagram organs — deferred to v2.1 per `03-principles §Deferral Roadmap`; not mapped at all this stream

**This stream produces:** matrix files + module disposition + Captain ratify queue. Not code.

---

## 11. Ratify gates (Captain events required to proceed)

| Gate | Trigger | Captain event |
|---|---|---|
| **R0** | `00-FORMAT-SPEC.md` + single-element worked example ready | Captain ratifies format → Module 1 enumeration starts |
| **R1.a** | Module 1 (Kiosk) CLOSED-LOOP map authored | Captain reviews CLOSED-LOOP rows |
| **R1.b** | Module 1 AMBIGUOUS BATCH + UNPLANNED-FEATURE BATCH ready | Captain resolves per-element: complete-workflow / replace / hide |
| **R2.a + R2.b** | Module 2 (Billing) same gates | Same shape |
| **R3.a + R3.b** | Module 3 (PWA) same gates | Same shape |
| **R4** | Cross-module synthesis + V2-PROGRESS-MAP re-baseline ready | Captain ratifies new V2 metric (elements-ADOPTED replaces §S-N-count) |
| **R5** | Per-element ADOPT execution (out-of-scope this stream, separate execution stream after Bono primitive ENFORCED) | Captain per-PR auth on each element ADOPT PR |

---

## 12. Open questions for Captain

1. **W0.1 single-element worked example choice:** `pwa.wallet-topup.preset-grid` (recommend — Joint 2 + V2-doctrine-backed + medium complexity), OR `kiosk.spectator.<element>` (start with Module 1 directly), OR `pwa.dashboard.race-now-cta` (highest-customer-impact)?
2. **AMBIGUOUS BATCH default disposition:** HIDE (autonomous, I can hide without asking) OR Q-DEC (Captain decides per-element)? Last session this was an open question — surfacing again because module-end batching changes the resolution shape.
3. **CLOSED-LOOP-V1 dispositioning:** If an element works today on V1 substrate but the V2-target requires it routes through skeleton primitive X (not yet ENFORCED), is the disposition (a) ADOPT-AS-IS (paint only, V1 substrate, skeleton-migration deferred to W4 of Bono stream) or (b) HOLD-PENDING-SKELETON (no paint until Bono primitive X ENFORCED)?
4. **Cross-stream session frequency:** Daily check on Bono primitive status, or only weekly Friday review? Daily produces more re-classification churn; weekly may delay element ADOPT unlocks.
5. **W5 boundary disposition routing:** Should `/terminal` / `/ai` / `/staff/diagnosis` decisions surface during their parent module (M3 PWA) or as a dedicated W5 wave after all three modules? W5 is cleaner; in-module is faster.

---

## Coordination protocol summary (read-this-paragraph-if-nothing-else)

James enumerates customer-facing interactive elements and classifies each (5-status). Elements that need skeleton primitives flag `needs-S1` / `needs-S2` / `needs-S3` / `needs-S4`. James sends rollup to Bono stream → Bono prioritizes primitives by impact-count. When Bono primitive X flips ENFORCED, James re-classifies all `needs-X` elements + surfaces Captain ratify list for those elements. No element ADOPTS until its dependent primitive ENFORCED. No code touches this stream. Mapping only.

**Identify what's possible. Don't build until skeleton makes more possible.**
