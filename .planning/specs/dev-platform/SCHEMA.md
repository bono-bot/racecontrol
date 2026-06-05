# SCHEMA — Dev-Platform Registry (P0)

> **Phase:** P0 (repo-native registry) of [`DEV-PLATFORM-DESIGN.md`](./DEV-PLATFORM-DESIGN.md) §8. Hand-maintained, git-tracked, zero new infra.
> **Authored:** 2026-06-05 (bono). **Extended 2026-06-06:** framework-aware (DMAIC | DMADV) + Frontend-UI signal + `health`/`gate_state`/`linked_release_id`/`rollout_targets`, reconciling the Captain's Console prototype (see [`../racecontrol-layer/CONSOLE-DEV-MGMT-DESIGN.md`](../racecontrol-layer/CONSOLE-DEV-MGMT-DESIGN.md)). **Status:** P0 active.
> **What this is:** the contract for two registry files — [`apps.yaml`](./apps.yaml) (Application registry) and [`developments.yaml`](./developments.yaml) (Development registry) — that together track the RacingPoint **product** portfolio's development. A **Development** ≡ a Console **"Initiative."**

---

## Two entities (design §3)

- **Application** — a product surface we ship. Carries **metrics** (dev/process + product/CTQ) and **toolchain/dependencies**. Lives in `apps.yaml`.
- **Development** *(= Console "Initiative")* — a **DMAIC- or DMADV-tracked** work-item. **DMAIC** = improve an existing app · **DMADV** = an entirely new app/product. Carries the framework's lifecycle + a Frontend-UI signal + a derived gate-state. Tagged to the app(s) it touches. Lives in `developments.yaml`.

Relationship is many-to-many: a Development touches ≥1 App (`developments[].apps[]`); an App lists its active Developments (`apps[].active_developments[]`). The two id-spaces cross-reference by slug.

---

## P0 principle — sources, not stale values

P0 is hand-maintained. To avoid memory-projected/stale values, **every probe-able metric records its SOURCE (the command/endpoint), and its `value` stays `TBD (P1 auto-fill)`** until P1 wires the live pull. Only facts that are stable + dated may carry a literal value (with `as_of`). CTQ numeric targets are **`TBD-Captain`** — never invented here (design §5, §10.4).

**Data-source class** (design §5): `probe` 🟢 (automatable) · `partial` 🟠 (probe + manual) · `manual` 🔴 (human/Captain entry).

---

## Phase model — DMAIC or DMADV (design §4)

Status vocabulary (machine-clean; maps to the Index legend):

| YAML value | Index legend | Meaning |
|---|---|---|
| `done` | ✅ | phase complete |
| `in_phase` | 🟡 | actively in this phase |
| `not_started` | 🔴 | not begun |
| `gated` | ⛔ | blocked (freeze or dependency) |
| `frozen` | ❄️ | whole item frozen |

**Framework** (per development; exactly one phase-block is populated):
- **DMAIC** (improve existing) — `D` (Define) · `M` (Measure) · `A` (Analyze) · `Improve` · `Control`. **Gate phase = Control.**
- **DMADV** (new product) — `D` (Define) · `M` (Measure) · `A` (Analyze) · `Design` · `Verify`. **Gate phase = Verify.**

`gate_state` is **derived** from the gate phase: `gate-clean` iff Control/Verify is `done` with evidence; `blocked` if a phase is `gated`/`frozen` or `health: block`; else `open`.

**Freeze-gate:** for a `freeze_status: frozen` development, D/M/A may be `done|in_phase` (planning allowed) but the gate-side phases (**Design/Verify** for DMADV, **Improve/Control** for DMAIC) MUST be `gated` until Captain-unfreeze + first-INR pass. The registry records state; it never unfreezes.

---

## Application record schema (`apps.yaml`)

```yaml
- id: <slug>                      # stable, unique
  name: <str>
  product_line: A | B | C         # A=RaceControl Captain's Console (separate product)
                                  # B=Ecosystem V2 (per-venue) · C=cloud surface (candidate)
  repo: <repo name>
  path: <path within repo>
  role: <one line>
  owner: bono | Captain | operator
  framework: <str>
  build: <command>
  test: <command>
  deploy_target: <str>
  toolchain: [<str>, ...]
  dependencies: [<app-id|@rp/pkg|API>, ...]
  dev_metrics:                    # source-recorded; value TBD until P1
    <metric>: { source: <cmd/endpoint>, class: probe|partial|manual, value: "TBD (P1)" }
  ctq_metrics:                    # product quality targets — TBD-Captain
    <metric>: { source: <...>, class: <...>, target: "TBD-Captain" }
  active_developments: [<dev-id>, ...]
  freeze_status: live | frozen
  evidence_anchors: [<PR/SHA/path>, ...]
```

## Development record schema (`developments.yaml`)

```yaml
- id: <slug>                       # = Console "Initiative" id
  title: <str>
  apps: [<app-id>, ...]           # apps this development touches
  framework: DMAIC | DMADV        # DMAIC = improve existing app · DMADV = new app/product
  # exactly ONE phase-block, per framework:
  dmaic: { D: <status>, M: <status>, A: <status>, Improve: <status>, Control: <status> }
  dmadv: { D: <status>, M: <status>, A: <status>, Design: <status>, Verify: <status> }
  current_phase: <phase (+ note)>
  freeze_status: unfrozen | frozen | in_flight   # deferred?
  lifecycle: active | shipped | archived   # where-in-life: active=current · shipped=live-to-customers (finished) · archived=retired
  health: on | risk | block       # initiative health (Console) — distinct from gate_state
  gate_state: open | gate-clean | blocked   # DERIVED from the gate phase (Control/Verify)
  ui:                             # Frontend-UI signal (design-led program lens)
    need: new | update | none
    status: live | canvas | design | todo | tbd | na
    surfaces: <named surfaces (verified vs Racing Point V3.1.html)>
  linked_release_id: <release_id | null>    # @rp/contracts release this ships in
  rollout_targets: [<tenant_id | "all-venues">, ...]
  ctq: [<str | "TBD-Captain">, ...]
  owner: bono | Captain           # James redundant (§S-448); operator = bono
  evidence_anchors: [<PR/SHA/path>, ...]
```

> **Back-compat:** existing records may carry only `dmadv{}` and no `framework` — readers treat a missing `framework` as `DMADV`. New/edited records SHOULD set `framework` explicitly. `health`/`gate_state`/`lifecycle`/`ui`/`linked_release_id`/`rollout_targets` are optional (defaults: `health: on`, `gate_state: open`, `lifecycle: active`, no `ui`). **`lifecycle` distinguishes *current* (`active`) from *finished* (`shipped`) developments** — the Console's Current/Finished views read it.

---

## Sync — source-class + the `Development:` trailer (P1/P2, design 2026-06-06)

Every field is one of three sync-classes; the auto-refresh generator (DEV-PLATFORM-DESIGN §11 P1) writes **only the 🟢 set** and **never overwrites 🔴**:

- **🟢 auto** — `evidence_anchors` (PRs/SHAs/CI), `dev_metrics` values, staleness-vs-HEAD, deployed `build_id`: pulled from `gh run list` / `gh pr list` / `git log <deployed>..HEAD` / `/fleet/health` + SWAPLOG into `registry-live.json`.
- **🔴 manual** — the DMAIC/DMADV **phase pointer** (`dmaic{}`/`dmadv{}`/`current_phase`) + `ctq` targets: human/Captain judgment, no probe exists (DEV-PLATFORM-DESIGN §5). Surfaced 🟠 as "verify manually" + the header `stale_at`.
- **derived** — `gate_state` (gate phase done + CI-green + merged) · `lifecycle: shipped` (deployed SHA + reached a venue ring).

**IDE→development link (LOCKED, Captain 2026-06-06):** commits that advance a development carry a **`Development: <id>` git trailer** in the commit body (e.g. `Development: initiative-money-path-reliability`). The generator greps `git log --grep='Development:'` / `gh pr list` to auto-attach the PR/SHA/CI to that development and derive `gate_state`/`lifecycle`. Greppable across both repos; survives squash-merge (it's in the body). **Triggers (P2):** post-merge git hook (immediate) + nightly cron + a SessionStart freshness check vs `stale_at` (mirrors the V2-PROGRESS-MAP cadence).

## Governance (design §9)

- **Owner/maintainer:** bono (`racecontrol/**` is bono-sole, §S-450). Captain owns CTQ targets + freeze + product-boundary calls.
- **Refresh (P0):** manual. Header `stale_at` per file + SessionStart freshness check. P2 adds cron/post-merge auto-refresh.
- **Push:** the V2-PROGRESS-MAP autonomous-push standing rule does **not** cover these files yet → commits stay **Captain-gated** until an analogous rule is authorized (design §10.5).
- **Mutability:** registries are current-state (mutable); the append-only audit trail is the §S-N ledger via `developments[].evidence_anchors`.

## Validation

```bash
python3 -c "import yaml,sys; [yaml.safe_load(open(f)) for f in ('apps.yaml','developments.yaml')]; print('YAML OK')"
# cross-check: every developments[].apps[] id exists in apps.yaml; every apps[].active_developments[] exists in developments.yaml
```
