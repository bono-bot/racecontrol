# Console — Application & Development Management (DMAIC / DMADV) — Build Brief

> **Audience:** Claude Code (implementer). **Author:** bono. **Date:** 2026-06-06 IST. **Status:** design brief, pre-build — **build DEFERRED until the first-INR money path closes** (V2 scope-freeze; this Console is vendor-internal tooling, not on the first-INR critical path).
> **Build target:** `rp-v2-apps/apps/racecontrol-console/` (the Tier-1 vendor HQ — `console.racecontrol.in`, Next.js 16, port 3220). **NOT** the venue-side `captain-console` (a distinct product — see §10).
> **Reconciled against:** the Captain's Console clickable prototype (`captain-console-for-bono/`: `Captain's Console.html` + 12 `.jsx` + `HANDOFF-CAPTAIN-CONSOLE-FOR-BONO.md` + `WORKFLOWS.md`, read 2026-06-06). This brief folds in the prototype's vocabulary ("Initiative"), 5-surface shape, and Frontend-UI signal. The JSX are the visual reference; this is the build contract.
> **Lane note:** authored under bono-sole `racecontrol/.planning`. The *implementation* lands in `rp-v2-apps` (a separate lane) — copy/reference this brief there when building.
> **Consumes (does not redefine):** the dev-platform registry at `racecontrol/.planning/specs/dev-platform/{SCHEMA.md,apps.yaml,developments.yaml,registry-live.json}`.
> **Captain decisions locked 2026-06-06:** (1) gate = **advisory picture + Config toggle** (release-store hard-block ships warn-only, flippable per-ring); (2) **design now, build after first-INR**; (3) billing remit = **TRACK + recovery-status only** (payment processing = separate §S-146 build).

---

## §1 — Purpose & scope

`console.racecontrol.in` is the **vendor-internal HQ control plane** — used by the **vendor / HQ / billing seat** (RacingPoint staff / Captain). This module is **App & Development Management**: a single place where every application we ship and every **Initiative** (an update we intend to roll out to venues) is tracked through a **Six Sigma change-management cycle** — **DMAIC** (improve an existing process) or **DMADV** (design a new one) — and where the vendor sees the deployment + subscription state of the fleet.

**The one-sentence goal:** *no update reaches a venue without a clear, current, evidence-backed picture that it is safe to ship* — and the Console makes that picture legible before the Captain (or, later, an agent) acts.

- **DMAIC** = Define · Measure · Analyze · **Improve** · Control — **changes to existing apps** (Ecosystem V2 apps, RC Installer, racecontrol heart / rc-agent, pod-display). The common case.
- **DMADV** = Define · Measure · Analyze · **Design** · Verify — **entirely new apps/products**.

**Five surfaces** (the prototype's nav; §8): **Command** (triage deck) · **Develop** (the DMAIC/DMADV board) · **Venues** (deployment + onboarding + billing-seat) · **Audit** (append-only F5 log) · **Config** (governance + thresholds + reset).

**Audience boundary (load-bearing):** the Console UI is the **Captain's personal command surface — single-identity-gated to `usingh@racingpoint.in` only** (NOT broad RacingPoint staff, NOT operator-visible; mechanism in §8/§13). Venue operators never see it — they get their own Ecosystem V2 admin panel + a self-serve subscription page. **bono is the data-layer maintainer** — it keeps the registry current via git (the `Development:` trailer + the generator, §15), *not* via the Console UI. The Console is where the Captain tracks, gates, monitors, and bills.

**Two money paths — do NOT conflate (see §11):** the *customer↔venue* money path (a driver pays the venue for sim time — the first-INR bar, surfaces on the pod/kiosk) is distinct from the *vendor↔venue* subscription path (a venue pays RacingPoint — the Console's billing seat). This Console touches the **vendor↔venue** path only.

**Non-goals** are in §10. **Build is deferred** (Captain decision 2) — this brief is the contract for when Claude Code picks it up post-first-INR.

---

## §2 — Where it lives + reuse map (do NOT rebuild what exists)

Mostly assembly — the rollout + fleet machinery already exists in `racecontrol-console`. Reuse:

| Existing asset (`rp-v2-apps/apps/racecontrol-console/`) | What it gives you | How this module uses it |
|---|---|---|
| `lib/release-store.ts` | the venue-rollout **ring ladder** (`canary → early → general`) + soak windows + per-tenant approval/promotion | **Extend** its promotion path with the (advisory-default) DMAIC/DMADV precondition (§6). Do NOT build a new ladder. |
| `lib/release-store-pg.ts` | Postgres-backed variant | Mirror any schema change here for parity. |
| `lib/readers.ts` | reads markdown/registry artifacts (B1–B9 board) into the UI | **Pattern to copy** for reading `developments.yaml`/`apps.yaml`/`registry-live.json` (P0). |
| `lib/db.ts` | control-plane **SQLite** schema init (better-sqlite3); tier-1-only | Add the `initiatives` + `initiative_releases` tables here (P1). |
| `lib/release-auth.ts` + `control_plane_audit` | `ControlPlaneCaptainJWT` verification + append-only audit | Gate mutations; every phase transition / gate flip / deploy → `control_plane_audit` (the prototype's "F5" log). |
| `app/page.tsx` | `Card`, `rowStyle`, `listStyle` inline-style patterns | Reuse for the board + cards. |
| `@rp/tokens` | design tokens — `RP.red/ink/inkDim/card/asphalt`, `SP_PX`, `RADIUS`, `ELEV`; Chakra Petch / Montserrat / JetBrains Mono | Style everything with these; the prototype's `tokens.jsx`/`components.jsx` map here — **do not fork** (extend at the shadcn/Tailwind handoff). |
| `@rp/contracts` (`release.ts`) | `ReleaseClass`, `ReleaseRing` (`canary\|early\|general`), `ReleaseManifest`, `Approve/PromoteReleaseRing*`, `PROMOTE_DEEPLINK_PARAM_*` | The `linked_release_id` an Initiative links to is one of these; reuse the ring enum + promote deeplink. |
| `HQ_VENUES` fleet dataset (shared with RaceControl HQ) | venue `{name,id,city,region,sub,seats[used,licensed],mrr,invoice,online,pods[up,total],faults,uptime,seen,version,vstate,dogfood}` | The Venues surface (§8) + billing seat (§11) read this. **Do not duplicate the fleet store.** |
| existing routes `/tenants`, `/access`, `/releases/[id]/promote` | nav + page-shell + form patterns | Match their structure for the new routes. |

**Existing rollout ladder semantics (reuse as-is):** a release advances `canary → early → general`; venue exposure increases each ring; soak windows gate promotion. **`general` (and `early`) are the venue-facing rings** — that is where the DMAIC/DMADV gate *surfaces its picture* and, when the Config toggle is on, *bites* (§6).

---

## §3 — Entities

Two entities, both already in the dev-platform registry — this module surfaces them and adds the DMAIC arm, the Frontend-UI signal, and the gate.

### Application
What we ship. Source of truth: `dev-platform/apps.yaml`. **Consume as-is.**

### Initiative *(prototype term; ≡ dev-platform "Development"; my earlier "Change")*
**One update we intend to roll out.** The unit the DMAIC/DMADV board tracks. Extends the dev-platform `Development` record (see `dev-platform/SCHEMA.md`):

```yaml
- id: <slug>                       # e.g. initiative-money-path-reliability
  title: <str>
  apps: [<app-id>, ...]            # which Application(s) this update touches
  framework: DMAIC | DMADV         # DMAIC = update to existing app; DMADV = new app/product
  # exactly one phase-block is populated, per framework:
  dmaic: { D: <status>, M: <status>, A: <status>, Improve: <status>, Control: <status> }
  dmadv: { D: <status>, M: <status>, A: <status>, Design: <status>, Verify: <status> }
  current_phase: <phase + short note>
  lifecycle: active | shipped | archived     # active=current · shipped=finished (live) · archived=retired (§14)
  health: on | risk | block         # initiative health (prototype) — distinct from gate_state
  gate_state: open | gate-clean | blocked   # DERIVED from the gate phase (Control / Verify)
  ui:                                # the Frontend-UI signal (§4a) — the design-led program lens
    need: new | update | none        # NEW UI / UI UPDATE / NO FRONTEND·BACKEND
    status: live | canvas | design | todo | tbd | na   # LIVE / ON CANVAS / NEEDS DESIGN / TBD
    surfaces: <named surfaces, verified vs Racing Point V3.1.html>
  linked_release_id: <release_id | null>     # the @rp/contracts release this Initiative ships in
  rollout_targets: [<tenant_id|"all-venues">, ...]
  owner: bono | Captain              # James redundant (§S-448); operator=bono
  ctq: [<str | "TBD-Captain">, ...]  # the measurable quality target(s)
  evidence_anchors: [<PR/SHA/endpoint/doc path>, ...]
```

`status` vocabulary (dev-platform): `done` ✅ · `in_phase` 🟡 · `not_started` 🔴 · `gated` ⛔ · `frozen` ❄️.

**Two distinct axes (keep both):** `health` = the Captain's at-a-glance read of the initiative (on track / at risk / blocked, prototype). `gate_state` = the *derived* readiness for venue sync (clean iff the gate phase is `done` with evidence). A `health: block` initiative **cannot advance a phase** (prototype W2); a `gate_state != gate-clean` initiative shows a warning on the promote page and — toggle-on — blocks promotion (§6).

**Relationship:** an Initiative touches ≥1 Application; an Application lists its active Initiatives (`apps.yaml[].active_developments[]`); an Initiative links to ≤1 release. The release ↔ initiative link is the spine of the gate (§6).

---

## §4 — The dedicated DMAIC page (for updates to existing apps)

DMAIC improves an **existing** process. Per Initiative, the page captures + displays five phases. Each phase: **status**, **required fields**, **gate criteria** ("done"), **evidence anchors** — rendered as the prototype's **phase ladder** (each phase a node, marked CLEARED / IN PROGRESS / pending) + per-phase `{artifact, exit-criterion}` (the prototype's `pd[]`).

| Phase | What it answers | Required fields | "Done" criteria |
|---|---|---|---|
| **Define** | What problem, for whom, to what goal? | problem · venue/customer requirement · goal + **CTQ** · target app(s) · scope | a stated CTQ + named target app(s) |
| **Measure** | Current baseline? | baseline metric(s) + **data source** (probe/partial/manual) · window · current value | a real baseline with a cited source (not memory-projected) |
| **Analyze** | Root cause of the defect/variation? | root cause(s) · evidence (logs/RCA/SHA) · contributing factors | ≥1 root cause with evidence |
| **Improve** | Solution shipped to address the cause? | the change (PR/commit/SHA) · what changed · expected effect on the CTQ | merged solution linked |
| **Control** | How do we sustain the gain? **(GATE PHASE)** | monitoring in place (dashboard/alert/metric) · gate-clean checks (tests green, parity, no regression) · rollback plan · post-rollout metric target | monitoring named + gate-clean evidence → sets `gate_state: gate-clean` |

**The CTQ card (DMAIC, prototype):** a `metric:{label, baseline → current → target, good:'low'}` with a reduction bar — the data-driven Six Sigma signal. Numeric targets are **`TBD-Captain`** until set; never invented.

**Control is the pre-sync gate** for existing-app updates. **Data-source honesty (dev-platform doctrine):** Measure/Control values record their **source** + **class** — `probe` 🟢 / `partial` 🟠 / `manual` 🔴.

### §4a — The Frontend-UI signal (every Initiative)

Because this is a design-led program, every Initiative declares its frontend-UI footprint — a **row badge** + a **roll-up count** on the board + a **drawer panel** ("FRONTEND UI LAYER") naming the exact surfaces (verified against `Racing Point V3.1.html`):

- `ui.need`: **NEW UI** · **UI UPDATE** · **NO FRONTEND · BACKEND**
- `ui.status`: **ON CANVAS** / **LIVE** / **NEEDS DESIGN** / **TBD**
- `ui.surfaces`: the named surfaces (e.g. money-path → `g1-hud`, `g3-billing-banner`, `m1-billing-recovery`).

This is how the Captain knows, across the whole pipeline, what design work each update needs and whether it's drawn.

---

## §5 — The DMADV page (for entirely new apps/products)

Identical treatment; Define · Measure · Analyze are the same shape as DMAIC.

| Phase | What it answers | Required fields | "Done" |
|---|---|---|---|
| **Define** | new product goal + requirements | goal · customer/venue requirements · CTQ · scope | CTQ + scope |
| **Measure** | what the design must hit | target metrics + acceptance thresholds + source | quantified targets |
| **Analyze** | options / risks | design options · risks · chosen direction | a chosen direction with rationale |
| **Design** | the built design | architecture/spec links · build PRs/SHAs · contracts | design built + linked |
| **Verify** | meets requirements? **(GATE PHASE)** | verification evidence vs each CTQ · tests · sign-off | every CTQ verified → `gate_state: gate-clean` |

**Verify is the pre-sync gate** for new products (the DMADV analogue of Control).

---

## §6 — The pre-rollout gate (advisory + Config toggle — Captain decision 1)

**The picture, always. The block, by toggle.** The prototype already gates *clearing* a phase (W6 Config "require sign-off to clear Control/Verify") and refuses to advance a `health: block` initiative (W2). This module adds the missing **deploy consequence**: the release ↔ initiative link, surfaced on the promote page and — when enabled — enforced.

**Rule (default = advisory):** when a release is promoted toward a venue-facing ring (`early`/`general`), the Console computes the **readiness picture** — every linked Initiative, its framework, current phase, `gate_state`, and the blocking reason if not clean — and renders it on `/releases/[id]/promote`. **By default this is a warning, not a block.**

**Implement by extending `lib/release-store.ts`** (and `release-store-pg.ts`) — a precondition on the existing promotion path, do NOT fork the ladder:

```ts
// in promote(release_id, from_ring, to_ring) of release-store.ts:
if (isVenueFacing(to_ring)) {                         // 'early' / 'general'
  const initiatives = initiativesForRelease(release_id);     // via initiative_releases (§7)
  const notClean = initiatives.filter(i => i.gate_state !== 'gate-clean');
  const picture = notClean.map(i => ({ id: i.id, framework: i.framework, current_phase: i.current_phase }));
  if (notClean.length > 0) {
    if (gatePolicyFor(to_ring) === 'hard') {          // Config toggle, default 'advisory'
      return blockPromotion({ reason: 'dmaic-dmadv-gate', detail: picture });
    }
    return warnPromotion({ reason: 'dmaic-dmadv-gate', detail: picture });  // advisory default
  }
}
// canary: always advisory (in-house soak).
```

- **Default policy (Decision 1):** **advisory at every ring** — the readiness picture shows, deploy is never machine-blocked. **Per-ring `gatePolicyFor(ring)` is a Config toggle** (§ Config governance): the Captain flips `early`/`general` to **hard** once the initiative metadata is trusted. No rebuild to tighten.
- The **promote page** renders the readiness picture either way — "the clear and updated picture for every update we roll out."
- Every gate evaluation (warn or block) → `control_plane_audit` (actor, release_id, initiative ids, verdict).

**Why safe + cheap:** the ladder/soak/approval already exist; the gate is a *read* of `initiative.gate_state` inserted as a precondition. No new distribution path, no money-path touch.

---

## §7 — Data model / schema

### P0 (read-only): consume the registry
No new tables. Read `dev-platform/registry-live.json` (or `developments.yaml` + `apps.yaml`) via the `lib/readers.ts` pattern. The DMAIC arm renders when `framework: DMAIC`; the `ui{}` signal renders the badge + panel.

**Registry extension** (authored in `dev-platform/SCHEMA.md` + `developments.yaml`, this pass): add `framework`, the `dmaic{}` block, `ui{}`, `health`, `gate_state`, `linked_release_id`, `rollout_targets` to the Development record. Existing DMADV records keep working (`framework: DMADV`).

### P1 (editable): control-plane SQLite (`lib/db.ts`)

```sql
CREATE TABLE IF NOT EXISTS initiatives (
  id              TEXT PRIMARY KEY,
  title           TEXT NOT NULL,
  framework       TEXT NOT NULL CHECK (framework IN ('DMAIC','DMADV')),
  apps_json       TEXT NOT NULL,           -- JSON array of app ids
  phases_json     TEXT NOT NULL,           -- {D,M,A,Improve,Control} or {D,M,A,Design,Verify} + per-phase {artifact,exit}
  current_phase   TEXT NOT NULL,
  health          TEXT NOT NULL DEFAULT 'on'  CHECK (health     IN ('on','risk','block')),
  gate_state      TEXT NOT NULL DEFAULT 'open' CHECK (gate_state IN ('open','gate-clean','blocked')),
  ui_json         TEXT,                    -- {need,status,surfaces}
  owner           TEXT,
  ctq_json        TEXT,
  evidence_json   TEXT,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL,
  created_by      TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS initiative_releases (   -- link: which Initiatives ship in which release
  initiative_id TEXT NOT NULL REFERENCES initiatives(id),
  release_id    TEXT NOT NULL REFERENCES releases(release_id),
  PRIMARY KEY (initiative_id, release_id)
);
```

`gate_state` is **derived** on every phase write: `gate-clean` iff the framework's gate phase (Control / Verify) is `done` with required evidence; else `open`; `blocked` if a phase is `gated`/`frozen` or `health = block`. Mirror in `release-store-pg.ts`.

---

## §8 — The five surfaces & routes (new, in `racecontrol-console`)

The prototype's nav, mapped to routes. Reuse `Card`/`rowStyle`/`listStyle` + `@rp/tokens`; phase status uses the registry glyphs (✅🟡🔴⛔❄️). **Auth: the entire Console is single-identity-gated to `usingh@racingpoint.in`** (Google Workspace OAuth → server-side email-allowlist → `ControlPlaneCaptainJWT`; §13) — no operator-visible reads.

| Nav · route | Renders | Prototype source |
|---|---|---|
| **Command** · `/` | triage deck — stat tiles (active initiatives, net-new vs improve, **shipped this cycle**, **finished all-time**, what's shipping, venues live/on-latest, pods) + **release-train rollup** (mini phase rails) + **"Needs-a-decision" queue** (at-risk/blocked initiatives + offline/past-due venues, deep-linking into Develop/Venues). Header **Weekly review** → audited export. | `CapOverview` (W1) |
| **Develop** · `/initiatives` | **the board** — Initiatives as rows/cards with area kicker, framework chip, 5-phase rail, **Frontend-UI badge**, current phase, `health`. Filter `All / DMADV / DMAIC` + by app / gate_state, and a **`Current │ Finished │ Archived`** lifecycle segment (Current=`active`, Finished=`shipped`, §14). | `CapDevelopment` (W2) |
| · `/initiatives/[id]` | **the DMAIC/DMADV tracker** — 5-phase stepper (§4/§5 fields + the `{artifact,exit}` ladder), the **Frontend UI layer** panel, the **CTQ card** (DMAIC), `gate_state` + a **"ready to sync to venues"** badge, **Advance phase** (→ `PHASE_ADVANCE`/`SHIPPED` audit), **Open in canvas** (deep-link `Racing Point V3.1.html` / `RaceControl HQ.html`; backend-only → "Not on canvas"). A `health:block` initiative refuses to advance. | drawer (W2) |
| · `/initiatives/new` | create — **framework first** (existing app → DMAIC · new product → DMADV) + name + area + owner + target + **Frontend-UI impact** → prepends in Define → `INITIATIVE_CREATE`. | `NewInitiative` (W3) |
| **Apps** · `/apps`, `/apps/[id]` | Application registry from `apps.yaml` (name, product_line, owner, freeze_status, # active Initiatives, dev/CTQ metrics, data-source classed). | (brief addition) |
| **Venues** · `/venues`, `/venues/[id]` | deployment + onboarding + **billing seat** (§11) — table (status dot, subscription pill, V2 version on-latest/behind, pods, uptime) + version-coverage + the **onboarding rail** (§12) + venue drawer (tenant id, location, `sub`/`mrr`/`invoice`/seats/pods/faults/uptime, **View in HQ Fleet**, **Push <release>** → `VENUE_DEPLOY`). Header **Provision venue** → **Mint & email installer** (§12). | `CapVenues` (W4) |
| **Audit** · `/audit` | append-only **F5 log** (`control_plane_audit`) — newest-first, `timestamp · actor · surface · event · detail · F5-id`, severity-coloured, filter `All/Develop/Venues/Config`, immutable, **Export** (audited). | `CapAudit` (W5) |
| **Config** · `/config` | governance (§13) — identity & access, **framework governance** (the §6 gate toggles), risk/SLA thresholds, connected systems, **Reset console state**. | `CapConfig` (W6) |

**"No dead ends" UX doctrine (prototype):** every action navigates, deep-links, or fires an **audited toast carrying its F5 id**. `CapStore` is the single writer; `useCap()` subscribes the surfaces so the board/overview/venues/audit update live. (Production: `control_plane_audit` is the F5 sink; see `INTERACTION-MAP.md` / the M5 AuditLogViewer.)

---

## §9 — Build phasing for Claude Code (each independently shippable)

> **Build deferred until first-INR closes** (Captain decision 2). Phasing for when it's picked up:

- **P0 — Read-only picture.** Routes `/`, `/initiatives`, `/initiatives/[id]`, `/apps`, `/venues`, `/audit` rendering from the dev-platform registry + `HQ_VENUES` via the `readers.ts` pattern. Add the registry fields first (done this pass). **Ships the "clear, current picture"** with zero new tables, zero enforcement.
- **P1 — Editable records.** `initiatives` + `initiative_releases` tables (`lib/db.ts`); `/initiatives/new` + phase editing; every write audited; `gate_state` derived; the billing-seat TRACK surface (§11) + onboarding rail (§12). (pg parity.)
- **P2 — Gate enforcement (toggle).** Wire the §6 precondition into `release-store.ts` promotion (advisory default; Config toggle to hard). Render the readiness picture + reason on `/releases/[id]/promote`.
- **P3 — Agentic tier** (Annex D) — post-first-INR; wires existing substrates (graphify, MMA/MAOR/Workflow, the hook stack, the AI healer).

Tests: vitest unit (phase/gate-state derivation, advisory-vs-hard policy) + the existing pg-route integration pattern for the promotion precondition.

---

## §10 — Out of scope / non-goals

- **Not** the venue-side `captain-console` (port 3210) — operator-facing, a distinct product (naming-guarded). This is vendor-internal.
- **Not payment PROCESSING** — the Console is the billing *seat* (TRACK + recovery-status, §11), but charging venues / payment-gateway / invoice-capture is a **§S-146 foundational build** (RCA + Captain per-PR auth, post-first-INR), NOT this module. *(Revises the earlier blanket "not payment collection" non-goal per Captain decision 3.)*
- **Does not duplicate** the dev-platform registry — it **consumes** it.
- **Does not build a new rollout mechanism** — it **extends** `release-store.ts`.
- **No money-path / pod-state-channel / auth-schema changes** (§S-146 foundational boundaries handled elsewhere).

---

## §11 — Billing / subscription seat (TRACK — Captain decision 3)

The Console is the **vendor / HQ / billing seat**. **Two money paths, kept distinct in the schema so a venue-dunning workflow can never point at a driver's wallet:**

| | Customer ↔ Venue (in-venue) | **Vendor ↔ Venue (this Console)** |
|---|---|---|
| Who pays whom | driver pays the venue for sim time | venue operator pays RacingPoint (vendor) |
| Money path | the **first-INR bug-free bar** (register→topup→launch→tick-debit→bill) | MRR / invoice / per-pod licensing |
| Surfaces | `g1-hud` · `g3-billing-banner` · `m1-billing-recovery` — on the **pod/kiosk** (`Racing Point V3.1.html`) | the **Console billing seat** — `sub`/`mrr`/`invoice`/`seats[used,licensed]` on `HQ_VENUES` |
| Scope | V2.0 first-INR (FROZEN at the bar) | this section |

**TRACK (in scope, ~P1, read/observe):** surface `sub`/`mrr`/`invoice`/`seats[used,licensed]`/licensed-pods from `HQ_VENUES`; subscription-status pills (Paid / Trial-30d); past-due / dunning **status**; a billing-recovery **workflow view** (what's owed + recovery steps — *display*, not charge). Fits the "clear picture" intent.

**TRANSACT (deferred, §S-146 foundational, post-first-INR):** actual payment **processing** — charging venues, gateway integration, invoice capture, auto-dunning that moves money. Requires a 5-section RCA + Captain per-PR auth before any build. **NOT designed here** — captured as a future foundational item.

**Existing subscription substrate to reuse:** `subscription.yaml` v0.1.0-draft (bono-authored 2026-05-30, pending Captain ratify) · POD-ENTITLEMENT · Paid/Trial-30d plans · per-pod licensing (initiative) · the rc-installer redeem packet's `licensed_pods`.

---

## §12 — Onboarding / provisioning rail + rc-installer packet

The Venues surface owns venue onboarding (the prototype's right-rail). A 5-step rail: **Token issued → Redeemed → Server up → Pods enrolling → Live**.

**Provision venue** (header modal): name · city · plan (Paid / Trial-30d) · licensed-pods stepper · a live **credential-packet preview**:

```
{ venue_id, install_token, .wgconf, licensed_pods }
```

This is the **exact contract `rc-installer` redeems** (see the rc-installer trust core: `crates/rc-installer/src/{signature_verifier,manifest,profile}.rs` — ed25519+sha256 verification, `Profile::{Server,Pod}`). **Mint & email installer** issues the token + emails the installer (reusing the **already-live vendor-console installer-email infra** — `racingpoint-gmail` = bono@racingpoint.in, `project_console_installer_email_live_20260601`), adds the venue to the rail at *Token issued*, and audits `VENUE_PROVISION`. See `hq-deploy.jsx` / `VENUE-DEPLOY-WORKFLOW-FOR-CLAUDE-CODE.md` in the prototype bundle.

---

## §13 — Config governance

The prototype's `CapConfig`, mapped to production:
- **Identity & access** — the **single allowlisted email `usingh@racingpoint.in`** (Google Workspace OAuth → server-side allowlist → `ControlPlaneCaptainJWT`, reuse `release-auth.ts`); decision alerts. **No add-user flow in V1** — the allowlist is a one-entry constant; broadening it is a deliberate future change.
- **Framework governance** — the §6 gate toggles: per-ring `gatePolicyFor(ring)` (advisory ⟷ hard) + "require sign-off to clear DMADV·Verify / DMAIC·Control." Bind to real Six Sigma tollgate approvals (`CAP_PHASES` defines both ladders).
- **Risk & SLA thresholds** — stall window (initiative health → risk), onboarding SLA target, rollback budget.
- **Connected systems** — links to RaceControl HQ, the V3.1 canvas, the F5 sink.
- **Reset console state** — restores the seed (demo).

---

## §14 — Lifecycle: monitoring *current* vs *finished* developments

Every Initiative carries a **`lifecycle`** axis — and the Console keeps **three orthogonal axes**: `lifecycle` (where in life) · `gate_state` (ready?) · `freeze_status` (deferred?).

| `lifecycle` | Meaning | Where it shows |
|---|---|---|
| `active` | in-flight = **current** | Develop "Current" segment · Command "active initiatives" |
| `shipped` | gate-clean **AND** the linked release reached a venue ring (live to customers) = **finished** | Develop "Finished" segment · "Shipped this cycle / Finished all-time" tiles |
| `archived` | superseded / cancelled / decommissioned (kept for the record) | Develop "Archived" segment |

`lifecycle: shipped` is **derived** (the gate phase clears + the linked release deploys); the `SHIPPED` F5 event is the audit trail of the transition — **the event answers *when* it shipped; `lifecycle` answers *what is currently* shipped** (a queryable state, not a log scrape). **Pre-first-INR the Finished view is near-empty *by design*** — nothing is live-to-customer until the bug-free bar passes; an empty Finished list is the honest signal, not a defect.

## §15 — Sync: keeping the registry current with IDE development

The Console reads **`registry-live.json`** (auto-refreshed) + **`developments.yaml`** (the hand-curated contract). Only the *objective* state auto-syncs from IDE work; the *judgment* state stays human-entered (DEV-PLATFORM-DESIGN §5; the `dev-platform/SCHEMA.md` Sync section).

| Class | Fields | Source |
|---|---|---|
| **🟢 auto** | evidence (PRs/SHAs/CI), staleness, deployed `build_id`, dev_metrics | `gh run list` / `gh pr list` / `git log <deployed>..HEAD` / `/fleet/health` + SWAPLOG → generator → `registry-live.json` |
| **🔴 manual** | the DMAIC/DMADV **phase pointer** + CTQ targets | Captain via the Console "Advance phase" (no probe exists) — the generator **never overwrites** |
| **derived** | `gate_state` · `lifecycle: shipped` | computed from merged + CI-green + deployed |

**IDE→development link (Captain-locked 2026-06-06):** commits that advance an Initiative carry a **`Development: <id>` git trailer** in the commit body; the generator greps `git log --grep='Development:'` / `gh pr list` to auto-attach the PR/SHA/CI + derive `gate_state`/`lifecycle`. Works across both repos; survives squash-merge. **Triggers (P2):** post-merge git hook (immediate) + nightly cron + a SessionStart freshness check vs `stale_at`.

**Two doors, one audited contract:** the Captain advances a phase via the Console UI (single-identity-gated, §8); **bono** maintains the registry via git (the trailer + the generator). Both write the same `developments.yaml` + audit trail — so locking the UI to `usingh@racingpoint.in` does not block bono's maintenance role.

---

## Appendix A — Why DMAIC is the primary lens
Most updates we roll out are **improvements to already-shipped processes** (a heart fix, an rc-installer hardening, a pod-display tweak) → **DMAIC**. Only an *entirely new* app/product is **DMADV**. Both converge on one gate semantic — *"is the last phase (Control / Verify) clean?"* — which one `release-store` precondition enforces. `/initiatives/new` makes the routing explicit so the correct 5-phase template is always used.

## Appendix B — `policy`: gate strictness (Captain decision 1)
**Default = advisory at every ring** (show the picture, never block). Per-ring `gatePolicyFor(ring)` is a **Config toggle** — flip `early`/`general` to **hard** once initiative metadata is trusted. One-line change, no rebuild: `gatePolicyFor` reads the Config flag. (Alternatives the toggle covers: fully-advisory · hard-at-venue-rings · hard-at-every-ring.)

## Appendix C — References (consume / link)
- Registry: `racecontrol/.planning/specs/dev-platform/{SCHEMA.md,apps.yaml,developments.yaml,REGISTRY.md,registry-live.json}`.
- Prototype bundle: `captain-console-for-bono/` (Drive) — `HANDOFF-CAPTAIN-CONSOLE-FOR-BONO.md`, `WORKFLOWS.md`, `captain-development.jsx`, `captain-venues.jsx`, `captain-config.jsx`, `captain-store.jsx`, `hq-shell.jsx`, `tokens.jsx`, `components.jsx`, `hq-deploy.jsx`, `VENUE-DEPLOY-WORKFLOW-FOR-CLAUDE-CODE.md`.
- Rollout model: `.bono-staging/HANDOFF-MULTI-VENUE-MULTI-TENANT-N-ALTITUDE-20260531.md` (Ring-6 venue-wave releases).
- Layer context: `racecontrol/.planning/specs/v2/ECOSYSTEM-V2-INDEX.md` (L3/first-INR) + the RaceControl-layer program.
- App structure: `rp-v2-apps/apps/racecontrol-console/` (`lib/*`, `app/*`, `package.json`).

---

## Annex D — Agentic tier (P3+, post-first-INR)

> Captain 2026-06-06 "consider this also in design." **Disposition: design captured now, built post-first-INR.** Most of it is *wiring substrates we already run* into the Console — not new infra. Each pattern carries a doctrine constraint.

| Gap → agentic solution | Already-running substrate | Disposition |
|---|---|---|
| **G1 Agentic troubleshooting UI** — agent reads fault logs, suggests remediation in the venue drawer (Tool-Use) | heart `/api/v1/fleet/health` + `/fleet/intelligence` (composite health score + time-of-day failure patterns) · rc-sentry · the **AI healer** (`rc-watchdog` + Ollama) | P3. The venue/initiative drawer surfaces the healer's diagnosis + **suggests** (read-only) remediation. Agent proposes; never auto-executes on the fleet. |
| **G2 Multi-agent deploy verify** — Executor (provision the rc-installer packet) → Validator/Judge (dynamic tests on the deployment) → Reporter (append F5) | **MMA** (OpenRouter VERIFY) + **MAOR** review + the **Workflow tool** (Executor/Validator/Reporter = a workflow pipeline) · the F5 audit + Mint-&-email packet (§12) | P3. Reuse all three. **Constraint:** the Validator runs against **canary/soak only**, never live money-path pods (§S-146); the Executor's autonomous push is **gated by the HITL checkpoint** (G3a). |
| **G3a HITL / state-managed interruption** — pause before the push; operator approves to resume | **= the §6 gate + Captain decision 1** + the *money-path turn-ons are Captain-RESERVED, not agent-authable* rule + the safety classifier | **Already core doctrine.** HITL at the money path is **MANDATORY, not optional polish** — it is the gate. |
| **G3b Sandboxed execution** — Wasm + Kubernetes Policy-Enforcement-Layer validating generated actions vs framework governance | the **PreToolUse hook stack** (lane-guard · canonical-ref-guard · pre-build-canonical-ref-guard · harness-auth-gate) + `deploy-server.sh` safety + canary/soak/MAINTENANCE_MODE | **Adopt the PRINCIPLE, DEFER the mechanism.** Wasm+K8s is over-engineered for our substrate (Windows pods + 1 VPS + Next.js; no K8s; no agent-authored scripts hitting the fleet). The hooks + Config "framework governance" ARE the policy-enforcement layer. Revisit Wasm/K8s only if we ever run agent-authored scripts against live venues (not planned). |
| **G4 Knowledge graph / GraphRAG** — multi-hop fleet queries ("V2 2.1.0 in EU, high fault on money-path pods") | **graphify** — 13 graph-MCP servers incl. `graphify-racecontrol` (`query_graph`/`get_neighbors`/`shortest_path`/`god_nodes`/`get_community`/`graph_stats`) | P3, strongest reuse. Wire `HQ_VENUES` + fault logs into graphify; the Console's agent does multi-hop reasoning via the existing graph MCP. **Zero new graph infra.** |

**Doctrine guardrails on the whole tier:** (1) **scope-freeze** — entire tier is post-first-INR; must not expand the first-INR path. (2) **money-path safety** — autonomous fleet DEPLOY is Captain-reserved, not agent-authable; Validator/Judge agents read-only or canary, never live money-path; HITL is the hard gate. (3) **mechanism-trust-check** — any agent driving shared deploy/transport infra passes the 5-question MTC first. (4) **reuse-don't-rebuild** — graphify · MMA/MAOR/Workflow · the hook stack · AI healer + fleet-intelligence all exist; Wasm/K8s is the one piece deliberately NOT built.

### Workflow methodology — which agentic pattern per operation

A **portfolio matched to each operation's STAKES × SHAPE** — not one pattern everywhere. The cost ladder (deterministic → classify → fan-out → generate-filter → tournament → adversarial) tracks **blast radius**; the expensive patterns are reserved for the money path.

| Console agentic op | Pattern | Why |
|---|---|---|
| Pre-rollout gate / deploy-verify (G2) — "safe to ship to a venue?" | **Adversarial verification** | money-path stakes; N skeptics try to REFUTE "safe", kill on majority-refute; = MMA VERIFY + MAOR + HITL. The one place agents earn their cost. |
| Gating / routing (gate_state→block/warn/allow · framework DMAIC/DMADV · fault-type→remediation) | **Classify & act** | deterministic rules — the control spine; don't over-agent. |
| Command triage / Needs-a-decision / GraphRAG fleet queries (G4) | **Fan out & synthesize** | read across N venues/initiatives in parallel → synthesize the rollup / multi-hop answer. |
| Remediation suggestions for a stalled venue (G1) | **Generate & filter** | generate candidate fixes → filter to safe/applicable (read-only suggest). |
| DMADV Analyze (options/risks → chosen direction) | **Tournament** | N design approaches → score → winner (design-phase, not daily). |
| Onboarding rail · soak windows · sync refresh | **Loop until done** | poll until Live / gate-clean / registry-fresh. |

**Spine at rest = classify-and-act; apex guarding the money-path = adversarial verification.** No agentic-washing — the cheapest pattern that fits the blast radius.
