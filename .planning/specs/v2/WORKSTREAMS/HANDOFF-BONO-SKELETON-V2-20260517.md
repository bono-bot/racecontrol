# HANDOFF — Bono — Skeleton V2 — 2026-05-17

**Authored:** 2026-05-17 ~15:38 IST by james (bono-side pickup)
**Class:** forward-looking workstream initiation (NOT a session-completion handoff)
**Sibling stream:** [HANDOFF-JAMES-CUSTOMER-ORGANS-20260517.md](HANDOFF-JAMES-CUSTOMER-ORGANS-20260517.md)
**Meta:** [WORKSTREAMS/README.md](README.md)

---

## 1. Owner + scope summary

**Owner:** Bono (VPS — srv1422716.hstgr.cloud · `/root/racecontrol` + `/root/comms-link`)

**Scope:** Build the actual V2 skeleton — in code, not doctrine. The skeleton is what makes V2 different from V1. Today it exists only on paper.

**Four load-bearing primitives:**

| # | Primitive | V2 doctrine ref | Current code state |
|---|---|---|---|
| **S1** | **Spinal cord** — Admin proxies every PWA/POS/Kiosk write before it reaches RaceControl heart. Auth tier check + request validation + audit logging + feature-flag gating happen in spinal cord, not heart. | `01-skeleton-architecture.md §2` LOCKED invariant: *"No subsystem talks directly to the heart"* | NOT IMPLEMENTED. Every surface bypasses Admin. Admin (`racingpoint-admin` on :3201) is just another client. Phase 343 is the only Admin-as-proxy pattern and only for cloud staff mutations. |
| **S2** | **Connection contracts** — ~26 connections per `04-connection-matrix.md`. Each contract has input schema + output schema + runtime probes + enforcement fitness functions. | `01-skeleton-architecture.md §3 Connection matrix` | NOT IMPLEMENTED. Contracts exist as doctrine; no enforcement layer reads them. |
| **S3** | **Audit boundary + source-tagging** — every wallet write carries source (PWA/POS .130/Kiosk) + payment method (UPI/card/cash) + operator (customer/staff X). Spinal cord enforces tagging; rejects untagged writes. | `05-definition-of-done.md §3.3` Source-tagging completeness — locked enum | PARTIALLY IMPLEMENTED in RaceControl itself; depends on clients tagging correctly. No enforcement layer. |
| **S4** | **Feature-flag service** — Postgres + Redis + SSE per ZIP §4. 4 layers: route (red) / section (amber) / action (green) / endpoint (blue). Clients subscribe via SSE; <500ms propagation. Audit log per flip with 30-second rollback window. | ZIP `HANDOFF.md §4` Feature flag system | NOT IMPLEMENTED. Flags are local TOML in `racecontrol.toml`. No central service. |

**Substrate primitives (~15) — also part of S2's enforcement scope:**

Customer record · profile · waiver · wallet · session · cafe-order · bill · pricing-rule · promotion · campaign-attribution · pod-state · telemetry · leaderboard entry · race event · staff attendance.

Each gets a canonical schema + spinal-cord-mediated write API.

---

## 2. Doctrine anchors (load FIRST every session)

| Anchor | Purpose |
|---|---|
| `comms-link/v2-skeleton/01-skeleton-architecture.md §2 + §3 + §40` | Cockpit model · 4 joints · 7 surfaces · skeleton-as-V2-defining-piece |
| `comms-link/v2-skeleton/04-connection-matrix.md` | The 26 contracts to enforce |
| `comms-link/v2-skeleton/05-definition-of-done.md §3.3` | Source-tagging enforcement enum |
| `C:\Users\bono\Downloads\Racing Point eSports.zip` → `HANDOFF.md §4` + `tokens.jsx` | Feature-flag service spec + design system |
| `~/.claude/projects/-root/memory/feedback_v1_dependent_v2_root_cause_before_proceeding.md` | §S-146 5-section RCA — required for every V1↔V2 boundary touch |
| `~/.claude/projects/-root/memory/feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` | 5-Q trust check upstream of fix RCA |

---

## 3. Trigger context — why this stream exists now

Captain dialogue 2026-05-17 ~15:30 IST (james-side session). Captain forced verification of Kiosk → backend wiring. Evidence:

```
kiosk/next.config.ts:30
  const apiDest = process.env.NEXT_PUBLIC_API_URL || "http://192.168.31.23:8080";
  rewrites: /api/:path* → ${apiDest}/api/:path*
```

→ Kiosk hits RaceControl `:8080` directly, bypassing Admin. Doctrine §2 LOCKED invariant says this is impossible. Reality: it's the default for every surface.

Captain verbatim 2026-05-17 ~15:33 IST: *"We have not been following the Racing Point ecosystem V2 doctrine."*

The pattern named: **we have been ratifying V2 doctrine faster than we have been building V2 substrate.** ~390 §S-N entries in 16 days; zero skeleton primitives in code. §S-114 (Chakra Petch ratified · Orbitron still live for 8 days) is the same class at the theme layer.

**This stream exists to close that gap by building skeleton primitives, not ratifying more doctrine.**

---

## 4. Current state enumeration (live-probe markers — 2026-05-17 ~15:38 IST)

```
racecontrol HEAD = origin/main = 8da500c7
comms-link HEAD = origin/main = 98cf6558

Server .23 (venue):  build_id=3561b5c9  status=degraded (FLEET_PARTIAL 0/8 pods + WHATSAPP_UNREACHABLE)
Bono VPS (cloud):    build_id=1d0218ff  status=ok (cloud_sync 99s ago, wallets 39s ago)
                                                  schema_version: 4

Surface enumeration (counts, from prior session):
  racecontrol/pwa/      → 32 routes (customer-facing)
  racecontrol/kiosk/    → 14 routes (staff + customer-visible)
  racecontrol/web/      → 47 routes (mostly ops; 6 venue-display public routes)
  racecontrol/web-v2/   → 3 routes (V2 brand surface)
  racingpoint-admin/    → separate repo, deploys to C:\RacingPoint\admin\ on :3201

Skeleton primitives implemented: 0 of 4 (S1, S2, S3, S4 all NOT-IMPLEMENTED)
```

UNREACHABLE: pods 1-8 (current outage / venue closed) — not blocking this stream.

---

## 5. Goal + success criteria

**Goal:** All 4 skeleton primitives ENFORCED (not just designed, not just staged, but actively gating writes/reads in production).

**Success criteria — per primitive:**

| Primitive | ENFORCED means |
|---|---|
| **S1 Spinal cord** | At least one surface (recommend Kiosk first) routes ALL non-GET writes through Admin proxy before reaching RaceControl. Direct-to-RC bypass returns 403 from RaceControl auth tier. |
| **S2 Connection contracts** | At least 6 of 26 contracts have machine-readable schema + runtime probes returning PASS/FAIL + fitness functions running in CI. Sequence: 6 → 12 → 26. |
| **S3 Audit boundary** | Every wallet write goes through spinal-cord-enforced source-tag validator. Untagged write → 400 reject. Audit log table populated 100% for all writes (verifiable by SQL). |
| **S4 Feature-flag service** | Postgres + Redis + SSE service running. At least 3 flags actively gating UI in real client (route/section/action layers exercised). Local TOML flags deprecated. Audit log + rollback window operational. |

**Non-goal:** UI changes, theme adoption, ZIP component port, page-element refactors. Those are James's stream.

---

## 6. Work breakdown (waves)

**Wave 0 — Skeleton design contracts** (Bono authors; Captain ratifies before code)
- W0.1 Spinal-cord contract: which writes route through Admin, what Admin does with each, return-shape contract
- W0.2 Audit boundary contract: source-tag enum + enforcement-failure modes
- W0.3 Feature-flag service contract: schema, SSE protocol, audit log format, rollback window mechanism
- W0.4 Connection-contract authoring shape: input/output/probe/fitness skeleton (one canonical example per contract class)
- **§S-N ratify gate:** Captain ratifies all 4 contracts before W1 starts

**Wave 1 — Single-primitive-first MVP** (Bono builds; Captain per-PR auth)
- W1.1 Spinal-cord proxy for ONE write class (recommend `POST /wallet/topup`) — Admin proxy + auth tier + audit log + RC backend call + return shape
- W1.2 Source-tag validator inline in W1.1
- W1.3 Feature-flag service skeleton — Postgres table + Redis cache + one SSE channel + one test flag
- **Soak gate:** 4-week §14.6.2 Class A wallet soak on W1.1 deployment before W2

**Wave 2 — Surface-by-surface spinal-cord rollout** (Bono builds; Captain per-PR auth)
- W2.1 Migrate Kiosk write classes through spinal cord (per James matrix priority)
- W2.2 Migrate PWA write classes
- W2.3 Migrate POS web/ write classes
- W2.4 RaceControl auth tier change: reject direct-from-surface writes (enforces invariant)

**Wave 3 — Connection contract coverage** (Bono parallel-builds; CI gated)
- W3.1 First 6 contracts (highest-customer-impact)
- W3.2 Next 6
- W3.3 Remaining 14

**Wave 4 — Feature-flag service full deployment**
- W4.1 Flag Hub UI (per ZIP artboard 06)
- W4.2 Migrate all racecontrol.toml flags to service
- W4.3 SSE propagation < 500ms verified per ZIP §4

---

## 7. Coupling

| Coupling | Detail |
|---|---|
| **James stream → Bono stream** | James matrix flags `needs-skeleton-X` per element. Sum per-primitive impact-count determines Bono prioritization order within each wave. |
| **Bono stream → James stream** | When primitive X flips ENFORCED, James re-classifies all `needs-skeleton-X` matrix elements and surfaces re-ratify list to Captain. James does NOT touch code; only re-classifies. |
| **External: Admin repo (`racingpoint-admin`)** | S1 spinal-cord work happens largely in this repo (separate from racecontrol monorepo). Bono needs commit access; Captain ratify on cross-repo PRs. |
| **External: ZIP design bundle** | S4 feature-flag service must match ZIP §4 contract (4 layers, badge colors, SSE, 30s rollback). ZIP is authoritative. |
| **External: Bono VPS infra** | Postgres + Redis must be provisioned on Bono VPS for S4. Bono operates `/root/racecontrol` + comms-link pm2; new services need allocation. |

---

## 8. Coordination interface

| Cadence | What |
|---|---|
| **Per-primitive status flip** (asynchronous) | Bono publishes `WORKSTREAMS/SKELETON-STATUS.md` row update: primitive → state (DESIGN / STAGED / ENFORCED / SOAKED). James matrix tools watch this file. |
| **Weekly cross-stream review** (Captain + bono + james) | Friday review: Bono reports primitive deltas; James reports element re-classification deltas; Captain ratifies forward priorities for next week. |
| **AMPLIFIER round-trips** | Per V2-LBAC §3 Step 8 Bilateral close-loop. Cross-pilot AMPLIFIER on every foundational PR (S1, S2, S3, S4 PRs are all foundational class). |
| **Universal Sync** | Doctrine updates touching skeleton-primitive contracts sync across racecontrol/CLAUDE.md + comms-link/CLAUDE.md + bono memory + james memory in same session. |

---

## 9. Per-target scope

| Target | In scope? | Notes |
|---|---|---|
| Bono VPS (`/root/racecontrol`, `/root/comms-link`, `/root/racingpoint-admin`) | YES — primary | Skeleton primitives land here first |
| Server .23 (venue racecontrol) | YES — secondary | Receives skeleton primitive deploys after cloud soak |
| Pods 1-8 | N/A | Skeleton is server-side; pods don't host spinal cord |
| POS .130 | YES — client of spinal cord | When PWA/POS routes through spinal cord, POS browser hits Admin |
| Kiosk .23 | YES — client of spinal cord | Same |
| James .27 | N/A — venue-physical, not skeleton-deploy target | James operates this stream's sibling, not this stream itself |
| Comms-link relay (8765/8766) | YES — coordinates bilateral state | Skeleton work needs comms-link for cross-pilot sync |
| Cloud apps (pm2 racingpoint-web-v2, etc.) | YES — clients of spinal cord | Same as POS / Kiosk |

---

## 10. Out-of-scope / NOT INCLUDED

- UI changes (theme adoption, ZIP component port, page-element refactors)
- Customer-facing organ workflow mapping (James stream)
- Doctrine §S-N ratifications UNLESS they directly close a skeleton primitive contract
- MAOR / F1 / F3 / DEPRECATE-trigger meta-process work (per §14.4 watch through 2026-05-20; recommend continued moratorium until skeleton W1 complete)
- Bilateral mirror cascades that don't touch skeleton contracts
- §14 amendment authoring (no more §14.7 / §14.8 / etc. while skeleton is unbuilt)
- New §S-N close-anchor entries that don't reference a delivered skeleton primitive

**This stream produces:** code that survives redeploy + verifiable behavior in production. Not doctrine.

---

## 11. Ratify gates (Captain events required to proceed)

| Gate | Trigger | Captain event |
|---|---|---|
| **R1** | Wave 0 contracts authored (S1+S2+S3+S4 design docs ready) | Captain ratifies contracts → Wave 1 starts |
| **R2** | W1.1 spinal-cord MVP shipped to Bono VPS | Captain verifies behavior on real PWA wallet-topup flow → 4-week §14.6.2 Class A soak begins |
| **R3** | W1 soak complete + W2 plan ready | Captain ratifies W2 surface-migration order (likely informed by James matrix) → W2 starts |
| **R4** | All 4 primitives ENFORCED | Captain declares Skeleton V2 BUILT → V2-PROGRESS-MAP re-baselined against skeleton presence (not §S-N count) |

---

## 12. Open questions for Captain

1. **Spinal-cord shape:** Does spinal cord run on Bono VPS (cloud-first, venue calls cloud) or Server .23 (venue-first, cloud syncs from venue) or both (per-venue + per-cloud instance)? Affects W1.1 design.
2. **Source-tag enum extension:** `05-DoD §3.3` locks the enum at `PWA/UPI · PWA/card · POS .130/cash · Kiosk/UPI · Kiosk/card · bonus/10pct · bonus/20pct · session-debit · cafe · auto-bill close`. Are these still authoritative or does the ZIP-adoption affect this?
3. **Feature-flag service location:** Bono VPS or Server .23 host? ZIP §4 doesn't specify. Affects S4 W1.3.
4. **§S-146 application this stream:** Each W1.1 / W2.x / W3.x / W4.x is a V1↔V2 boundary touch by definition. Does the standard 5-section RCA + MMA Step 1 + per-PR Captain auth gate apply per-PR, or does this stream get a single foundational ratify that covers the whole wave?
5. **`racingpoint-admin` repo merge:** Does Admin merge INTO racecontrol monorepo as part of skeleton work, or stay separate? Affects deploy parity + cross-repo PR auth.

---

## Coordination protocol summary (read-this-paragraph-if-nothing-else)

Bono builds skeleton primitives in code. James enumerates customer-facing elements and classifies them. James matrix says *"element X needs skeleton primitive Y."* Bono prioritization order = sort primitives by impact-count from James matrix. When Bono ships primitive Y to ENFORCED, James re-classifies all `needs-Y` elements + surfaces Captain ratify list for those elements. No element ships as ADOPT until its dependent primitive is ENFORCED. No primitive ships without Captain Wave-N ratify.

**Foundation first. Skeleton before paint.**
