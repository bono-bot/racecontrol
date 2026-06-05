# 🛠️ DEV-PLATFORM-DESIGN — Ecosystem Application-Development Platform (DMADV-tracked)

> **Status:** DRAFT for Captain review · **Authored:** 2026-06-05 (bono) · **Scope:** design/architecture only — no platform code is built by this doc.
> **`/goal` (Captain, 2026-06-04):** *"Make an application-developing platform that tracks existing and future developments and tracks all metrics and tools needed to develop the applications we create for the ecosystem,"* using **DMADV** (Define · Measure · Analyze · Design · Verify) to track each item's progress.
> **Locked decisions (AskUserQuestion 2026-06-04):** (1) design-first, build later; (2) scope = **product apps only**; (3) per-entry tracks all four — DMADV phase · dev/process metrics · product/CTQ metrics · toolchain+dependencies.
> **Captain constraint (2026-06-04):** **RaceControl Captain's Console is a SEPARATE product** — its own product line; the dev-platform is a dev tool and is **never hosted inside a sold product.**
> **Method:** built on a 3-agent read-only sweep of `rp-v2-apps`, `racecontrol`, `comms-link` + direct dir/CI probes (catalog cross-checked against actual `apps/`·`crates/`·`packages/` listings on 2026-06-05). Every data source named below was probe-confirmed to exist.
> **Companions (do not duplicate):** [`../v2/ECOSYSTEM-V2-INDEX.md`](../v2/ECOSYSTEM-V2-INDEX.md) · [`../v2/V2-PROGRESS-MAP.md`](../v2/V2-PROGRESS-MAP.md) · `comms-link/V2-MASTER-STATE.md`.

---

## §1 — Vision & purpose

A single **development command-center** for the RacingPoint **product portfolio**: one place to see, for every product application we ship (existing *and* future), where each piece of development sits in its **DMADV** lifecycle, what **metrics** prove it's healthy, and what **tools/dependencies** are needed to build it.

**Problem it solves.** Today V2.0 first-INR is tracked three ways (gap map, progress grid, decision ledger — see §7), but there is **no per-application development-lifecycle view across the whole portfolio**. Forward/V2.1+ work lives loosely in prose; dev-health (CI, coverage, staleness) and product-health (KPIs) are scattered across endpoints, SQL, and CI logs with no rollup; toolchain/dependency coupling is undocumented. A builder cannot answer "what DMADV phase is X in, is it healthy, and what does it depend on?" from one surface.

**Non-goals (explicit).**
- **Not a sold product.** This is internal engineering tooling. It is **never** hosted inside RaceControl Captain's Console or any customer/operator-facing product (§2, §8).
- **Not a 4th status tracker that duplicates the 3.** It *ingests/links* them (§7), it does not restate them.
- **Not customer- or operator-facing.** Audience = builders (bono + Captain; future on-site operator).
- **Not auto-deciding scope.** It records DMADV state; it does not unfreeze anything (the scope-freeze is unchanged — §4).

**Two entities it tracks** (the key model choice — §3): **Applications** (the portfolio surfaces — carry metrics + toolchain) and **Developments** (DMADV-tracked work-items / "Index Items" — carry the DMADV lifecycle, each tagged to the app(s) it touches).

---

## §2 — Product taxonomy (catalog membership)

The platform tracks **product apps only**. Membership is classified below against the **verified** repo listing (probed 2026-06-05: 16 `rp-v2-apps/apps/`, 12 `racecontrol/crates/`, 11 `packages/`). Three product lines:

### Line A — RaceControl Captain's Console *(SEPARATE PRODUCT)*
The tier-1 **HQ / vendor control-plane** product (§S-449 outer-Console layer; `console.racecontrol.in`). **Distinct** from the venue `captain-console` app (Line B) and from this dev-platform.
- **`racecontrol-console`** (rp-v2-apps) — the Console product surface (tenant directory, deployment dashboard, access grant/revoke).
- *Supporting infra (tracked as infra-deps of Line A, not standalone product entries):* `deploy-controller-bono`, `deploy-agent-james` (release composition/signing + per-venue apply — the Console's distribution backbone).

> ⚠️ **Naming guard (per the naming-disambiguation doctrine):** "RaceControl Captain's Console" (Line A, HQ product) ≠ venue `captain-console` app (Line B). Never merge them. Console-product boundary + canonical name to be **confirmed by Captain** (§10).

### Line B — Ecosystem V2 *(per-venue sellable unit)*
The venue product the customer/venue buys.
- **Venue/web apps:** `pod-display`, `pos`, `pos130`, `staff-tablet`, `kiosk`, `pwa`, `launch-portal`, `chef-display`, `captain-console` (venue).
- **RaceControl Rust products:** `racecontrol` (heart), `rc-agent` (pod), `rc-installer` (release-trust + installer).

### Line C — Cloud product surfaces *(candidate — Captain to confirm, §10)*
- `cloud-dashboard` (`/root/racingpoint-cloud-dashboard`, :3600), `api-gateway` (`/root/racingpoint-api-gateway`, :3000). Separate repos; observability/routing surfaces. Flagged because their "product vs internal-ops" status is a Captain call.

### Membership classification (every probed dir accounted for — no silent omissions)

| Dir | Verdict | Line / reason |
|---|---|---|
| racecontrol-console | ✅ tracked | Line A (Console product) |
| pod-display · pos · pos130 · staff-tablet · kiosk · pwa · launch-portal · chef-display · captain-console | ✅ tracked | Line B venue apps |
| crates: racecontrol · rc-agent · rc-installer | ✅ tracked | Line B (Rust products) |
| cloud-dashboard · api-gateway | 🟨 candidate | Line C (Captain-confirm) |
| admin-proxy-bono · admin-proxy-james · bono-internal-relay | ❌ excluded | internal coordination/auth infra |
| deploy-controller-bono · deploy-agent-james | ❌ excluded as standalone | infra backbone of Line A (tracked as its deps) |
| mock-heart | ❌ excluded | dev-only sandbox |
| crates: rc-sentry · rc-sentry-ai · rc-watchdog · weekly-report | ❌ excluded | internal ops/utility binaries |
| crates: rc-common · rc-guardian · rc-process-guard · rc-process-manager · v2-db | ❌ excluded | libraries (tracked as **dependencies**) |
| packages/*: contracts · db · tokens · ui · billing-engine · proxy-core · wallet-client · wallet-ui · release-manifest · flag-sdk · events | ❌ excluded as entries | shared libs (tracked as **dependencies**) |
| comms-link · MCP servers · discord-bot · whatsapp-bot | ❌ excluded | internal tooling/coordination |

**Net product catalog:** **1 (Line A) + 12 (Line B) + 2 candidate (Line C) = ~13–15 tracked applications.** Libraries/packages appear only as dependency edges.

---

## §3 — Entity / data model

Two linked entities (the design's core abstraction):

### A. Application (portfolio entry) — carries metrics + toolchain
```jsonc
{
  "id": "pod-display",                 // stable slug
  "name": "Pod Display",
  "product_line": "A | B | C",         // §2
  "repo": "rp-v2-apps | racecontrol | racingpoint-cloud-dashboard | ...",
  "path": "apps/pod-display",
  "role": "per-pod customer-visible state surface",
  "owner": "bono | Captain | (future operator)",
  "framework": "Next.js 16 | Rust/Axum | Node",
  "build": "next build | cargo build --release --bin <x>",
  "test":  "vitest | tsc --noEmit | cargo test -p <crate>",
  "deploy_target": "Server .23 :3340 | Pods :8090 | Bono VPS :NNNN",
  "toolchain": ["pnpm","Next 16","@rp/ui","@rp/contracts", "..."],
  "dependencies": ["@rp/contracts","@rp/tokens","racecontrol(API)"],  // §5
  "dev_metrics":  { /* §5 dim 2 — probe-sourced */ },
  "ctq_metrics":  { /* §5 dim 3 — TBD-Captain where undecided */ },
  "active_developments": ["dev-multiplayer","dev-leaderboard"],       // → entity B
  "evidence_anchors": ["PR #34 2eaaf94","fleet/health build_id"]
}
```

### B. Development (DMADV work-item = "Index Item") — carries the DMADV lifecycle
```jsonc
{
  "id": "dev-cross-venue-leaderboard",
  "title": "Cross-venue AC leaderboard",
  "apps": ["racecontrol","racecontrol-console","admin-proxy-james"],  // touches → entity A
  "dmadv": { "D":"✅", "M":"🟡", "A":"✅", "Design":"✅", "Verify":"⛔" },  // §4 legend
  "current_phase": "Verify (deploy-gated)",
  "freeze_status": "UNFROZEN (Captain /goal 2026-06-04) | ❄️ frozen",
  "ctq": ["PII fields exposed = 0 [enforced]", "query p95 latency = TBD-Captain"],
  "owner": "bono",
  "evidence_anchors": ["rc #124 20065a6d","rp #35 430537e"]
}
```

**Relationship:** Application ↔ Development is many-to-many (a development can touch several apps; an app hosts several developments). Metrics + toolchain roll up on **Applications**; DMADV progress rolls up on **Developments**. The portfolio view = Apps × current-development-phase heatmap.

---

## §4 — DMADV lifecycle model

| Phase | Means (for a Development) | Marked ✅ when | Freeze rule |
|---|---|---|---|
| **D — Define** | problem/opportunity, scope, CTQs (critical-to-quality must-haves), success criteria, owner | scope + CTQ list exist | ✅ allowed under freeze (planning) |
| **M — Measure** | the KPIs/targets that prove the CTQs + baseline | targets named or explicit `TBD-Captain` | ✅ allowed under freeze |
| **A — Analyze** | options + trade-offs + risk + **F1 substrate gate** (does the code substrate exist?) + V1↔V2 RCA if foundational | an option chosen with rationale | ✅ allowed under freeze |
| **D — Design** | detailed design + **build** (contracts, specs, code/PRs) → maps to ENGINEERING-IN-FLIGHT→merged | code merged | ⛔ **gated** by freeze (build) |
| **V — Verify** | validation vs CTQs — tests, MAOR, e2e, pilot/canary, deploy-verify → DONE | validated on a real target | ⛔ **gated** by freeze |

**Freeze-gate rule:** a Development not Captain-unfrozen may advance **D→M→A** (planning), but **Design-build + Verify show ⛔** until (a) Captain unfreezes it AND (b) the first-INR bar passes. Captain-unfrozen items (multiplayer, pod-display error-screens, cross-venue leaderboard, grace-countdown — /goal 2026-06-04) may proceed all phases. **This platform does not unfreeze anything — it only records state.**

**Legend (reuse Index §1A, verbatim):** `✅ complete · 🟡 in phase · 🔴 not started · ⛔ gated · ❄️ frozen` · CTQ/metric `TBD` = undecided.
**Mapping to existing taxonomy:** the **Design** phase maps onto the `V2-PROGRESS-MAP` row taxonomy (NOT-STARTED → ENGINEERING-IN-FLIGHT → DONE; `TEST-SCAFFOLDED` = a Verify-phase sub-state per §S-221 F3 reform) so the platform reuses, not replaces, that vocabulary.

---

## §5 — Four tracked dimensions + data-source map

Each field is classified by how it's sourced: 🟢 probe-automatable · 🟠 partial (probe + manual) · 🔴 manual/Captain-entry. (All probes below were confirmed present in this repo.)

### Dim 1 — DMADV phase/progress
| Field | Source | Auto |
|---|---|---|
| Current DMADV phase + per-phase status | manual / Captain + bono entry (no probe exists) | 🔴 |
| Gate evidence (H1–H5, MAOR verdict, F1) | hook audit logs `~/.claude/state/*.jsonl` + commit-body `MAOR-REVIEW:` | 🟠 |
| Blocked-by / freeze | manual annotation (mirrors ROADMAP `DEPENDS-ON`) | 🔴 |

### Dim 2 — Dev/process metrics
| Field | Source | Auto |
|---|---|---|
| CI status / pass-rate | `gh run list -R <repo> --branch main` (real: racecontrol `ci.yml`,`contract-tests.yml`,`deploy.yml`,`e2e-tests.yml`,`quality-gate.yml`; rp-v2-apps `proxy-core-integration.yml`,`doctrine-name-collision-comms-link.yml`) | 🟢 |
| Build/deploy state (build_id, uptime) | `GET /api/v1/fleet/health` + `SWAPLOG.md` (present, 78 KB) | 🟢 |
| Open/merged PRs per app | `gh pr list` + `git log -- <app-path>` | 🟢 |
| Staleness vs HEAD | `git log <deployed_build_id>..HEAD -- <path>` | 🟢 |
| Contract parity | `pnpm run check-parity` (≈676 assertions) | 🟢 |
| Code coverage % | **not instrumented** (no nyc/tarpaulin) — **gap** | 🔴 |

### Dim 3 — Product/CTQ metrics
| Field | Source | Auto |
|---|---|---|
| Pod health score (0–100) | `GET /api/v1/fleet/intelligence` (Phase 366) | 🟢 |
| Session success-rate / revenue | `billing_sessions` SQL (`sqlite-racecontrol`) | 🟢 |
| Latency (launch→loading-complete) | session-event timestamp delta (`/heart/sessions/{id}/loading-complete`) | 🟠 |
| Error rate by category | audit_log SQL / `violation_count_24h` | 🟠 |
| **CTQ targets (SLAs, budgets)** | **`TBD-Captain`** — not invented | 🔴 |

### Dim 4 — Toolchain + dependencies
| Field | Source | Auto |
|---|---|---|
| Per-app toolchain | `package.json` / `Cargo.toml` (static) | 🟢 |
| Shared-package consumption | `pnpm ls` / `grep "@rp/"` / `cargo metadata` | 🟢 |
| DB schema version | `ls crates/v2-db/migrations/` | 🟢 |
| Cross-repo coupling (heart↔agent↔proxy) | manual topology map (no auto tool; graphify corpus could feed) | 🟠 |

**Design implication:** the platform is mostly **wiring a readout over instrumentation that already exists** (🟢 majority). The 🔴 items — DMADV phase, CTQ targets, coverage — are **human-entry forms** (or new instrumentation, deferred). A future automated refresh pulls 🟢 live and surfaces 🟠 as "verify manually."

---

## §6 — Application catalog (scaffold)

### Line A — RaceControl Captain's Console (separate product)
| App | Framework | Build / Test | Deploy | Current maturity |
|---|---|---|---|---|
| racecontrol-console | Next.js 16 | `next build` / `vitest` + `integration:pg` | Bono VPS `console.racecontrol.in` | Phase-K Foundation; HQ console live HTTP 200; extras → Console V2 (frozen) |

### Line B — Ecosystem V2 (per-venue product)
| App | Framework | Build / Test | Deploy | Current maturity |
|---|---|---|---|---|
| pod-display | Next.js 16 | `next build` / `tsc` | Pods :3340 | V3-UI built; server-lost screen merged (#34); Ph2 error-screens in-flight |
| pos / pos130 | Next.js 16 | `next build` / `vitest`+`playwright` | Server .23 :3200/:3130 | built; cash top-up + cafe |
| staff-tablet | Next.js 16 | `next build` / `vitest`+`playwright` | .23:3201 | built; live grace countdown |
| kiosk | Next.js 16 | `next build` / `vitest` | Venue :3300 | built; V2-theme migration PRs open (James) |
| pwa | Next.js 16 | `next build` / `vitest`+`playwright` | Bono VPS :3300 | built; registration + wallet |
| launch-portal | Next.js 16 | `next build` / `vitest` | .23 :3360 | built; 8-pod launch grid |
| chef-display | Next.js 16 | `next build` / `tsc` | Venue :3350 | V0.1 kitchen panel |
| captain-console (venue) | Next.js 16 | `next build` / `vitest`+`playwright` | Bono VPS :3210 | built; Reason-Class Monitor + pricing |
| racecontrol (heart) | Rust/Axum | `cargo build --release --bin racecontrol` / `cargo test -p racecontrol-crate` | Server .23 :8080 (`690a8616`) | heart-V2 surface live; first-INR money/launch merged, flag-OFF |
| rc-agent | Rust | `cargo build --release --bin rc-agent` / `cargo test -p rc-agent-crate` | Pods :8090 (`a826b100`) | fleet-uniform 8/8 |
| rc-installer | Rust | `cargo build --release --bin rc-installer --features installer-bin` | web-distributed | trust-core (ed25519+sha256) |

### Line C — candidate (Captain-confirm)
| App | Repo | Deploy |
|---|---|---|
| cloud-dashboard | racingpoint-cloud-dashboard | Bono VPS :3600 |
| api-gateway | racingpoint-api-gateway | Bono VPS :3000 |

### Development registry — seed (DMADV-tracked "Index Items")
Carried over + generalized from the V2.1+/future enumeration. Columns = D · M · A · Design · Verify · current · freeze.

| # | Development | Apps touched | D | M | A | Des | V | Current | Freeze |
|---|---|---|---|---|---|---|---|---|---|
| 1 | Multiplayer racing | racecontrol, launch-portal | ✅ | 🟡 | 🟡 | 🟡 (S0 #126) | 🔴 | Analyze/Design | UNFROZEN |
| 2 | Pod-display error screens Ph2 | pod-display, racecontrol | ✅ | 🟡 | 🟡 | 🟡 (Ph1 #34) | 🔴 | Design | UNFROZEN |
| 3 | Cross-venue AC leaderboard | racecontrol, racecontrol-console | ✅ | 🟡 (PII=0; latency TBD) | ✅ | ✅ (#124/#35) | ⛔ (deploy) | Verify | UNFROZEN |
| 4 | Grace-countdown symmetry | pod-display, staff-tablet | ✅ | 🟡 | ✅ | 🟡 (#25) | 🔴 | Design | UNFROZEN (UX-exception) |
| 5 | Per-game leaderboards | racecontrol | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ |
| 6 | Multi-tenant control plane (Halo) | racecontrol-console, racecontrol | ✅ | 🔴 | 🟡 (8 blockers) | ⛔ | ⛔ | Define/Analyze | ❄️ |
| 7 | Console V2+ (Ring6/7/brand) | racecontrol-console | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ |
| 8 | Customer-email/messaging (Wati) | pwa, racecontrol | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ |
| 9 | V1 decommissioning | (fleet) | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ |
| 10 | Walk-in registration Ph2 | pos, pwa | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ |
| 11 | Incident RESOLVE staff UI | staff-tablet, captain-console | ✅ | 🔴 | 🟡 | 🟡 (VIEW #36) | 🔴 | Design (partial) | in-flight |
| 12 | Refund/manual-adjust UI | captain-console, pos | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ |

*(CTQ targets marked TBD are Captain-owned; no numeric SLA invented here.)*

---

## §7 — Relationship to existing trackers (subsume none)

| Tracker | Role | This platform's relationship |
|---|---|---|
| `V2-PROGRESS-MAP.md` | 13-layer metric grid (% closed) | **Ingest** Design-phase row status from it |
| `comms-link/V2-MASTER-STATE.md` | §S-N append-only decision ledger | **Link** — each Development cites the §S-N that ratified it |
| `ECOSYSTEM-V2-INDEX.md` | first-INR nav + gap map (V2.0) | **Reference** — first-INR gaps for Line B apps |

One integration touchpoint each (no duplication): a Part-3 source-map row in the Index pointing here; a back-link from each Development to its §S-N; the platform's Design-phase status derived from (not re-keyed against) the Progress-Map. The 3 trackers stay authoritative for their lens; this platform is the **per-app/per-development lifecycle lens** they lack.

---

## §8 — Form-factor & phased build roadmap

**End-state:** a **standalone dev-platform** surface (own app/registry), audience = builders. **HARD RULE: never hosted inside a sold product** — not RaceControl Captain's Console (Line A), not any Line B/C app. (Rationale: a dev tool inside a sold product couples internal tooling to customer release cadence + leaks dev internals into a vendor surface. This is the structural guard against the console-conflation the Captain flagged.)

**Phased build (each phase independently shippable; design-first means we stop here until Captain says go):**
- **P0 — Repo-native registry** *(recommended first build).* `apps.yaml` + `developments.yaml` + `SCHEMA.md` under `.planning/specs/dev-platform/`, hand-maintained, encoding §3's two entities. Zero new infra; lives in git; immediately useful. *(This is the smallest thing that delivers the /goal.)*
- **P1 — Read-only readout.** A generator that fills the 🟢 fields live (the §5 probes: `gh run list`, `/fleet/health`, `/fleet/intelligence`, `check-parity`, `package.json`/`Cargo.toml`) into a rendered `REGISTRY.md` / JSON. No UI yet.
- **P2 — Automation.** cron + post-merge hook auto-refresh (mirror the V2-PROGRESS-MAP cadence §9), staleness bound.
- **P3 — Standalone app/dashboard.** New surface (own port; e.g. Bono VPS), reusing staff-JWT auth — only if P0–P2 prove the readout valuable.

---

## §9 — Governance

- **Ownership:** `racecontrol/**` is bono-sole (§S-450 LANE-CONTRACT) → bono authors/maintains; Captain owns CTQ targets + scope/freeze + the product-boundary calls (§10).
- **Refresh cadence (proposed, mirrors V2-PROGRESS-MAP):** P2 onward = nightly + post-merge; until then manual with a **stale-at** marker in the registry header + a SessionStart freshness check.
- **Autonomous-push:** the V2-PROGRESS-MAP autonomous-push standing rule does **not** yet cover this doc/registry → **commits stay Captain-gated** until Captain authorizes an analogous standing rule (proposed in §10).
- **Append vs edit:** the registry is mutable (current-state); Development → §S-N links provide the append-only audit trail via the existing ledger.

---

## §10 — Open decisions for Captain

1. **RaceControl Captain's Console boundary + canonical name** — confirm Line A = the `racecontrol-console`/HQ product, and the exact product name to use (so the catalog never re-conflates it with the venue `captain-console`).
2. **Line C in scope?** — track `cloud-dashboard` + `api-gateway` as products, or treat as internal ops?
3. **Build go/no-go + starting phase** — proceed to **P0 repo-native registry** now, or hold at design?
4. **CTQ targets** — who sets the per-app/per-development numeric targets (latency SLAs, error/leak budgets, success-rate floors)? They are `TBD-Captain` throughout until set.
5. **Autonomous-push standing rule** for the dev-platform registry (mirror V2-PROGRESS-MAP), or keep every commit Captain-gated?
6. **Coverage instrumentation** — add nyc/tarpaulin to CI (the one real dev-metric gap), or leave coverage untracked for now?

---

### Verification anchors for this doc
Catalog cross-checked against probed dirs (16 apps / 12 crates / 11 packages, 2026-06-05) — every dir classified in §2. CI workflows, `/fleet/health`, `/fleet/intelligence`, `check-parity`, SWAPLOG all probe-confirmed present. No numeric CTQ target is invented (all `TBD-Captain`). The 3 existing trackers are ingest/link/reference only (§7).
