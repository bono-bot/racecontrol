# SCHEMA — Dev-Platform Registry (P0)

> **Phase:** P0 (repo-native registry) of [`DEV-PLATFORM-DESIGN.md`](./DEV-PLATFORM-DESIGN.md) §8. Hand-maintained, git-tracked, zero new infra.
> **Authored:** 2026-06-05 (bono). **Status:** P0 active.
> **What this is:** the contract for two registry files — [`apps.yaml`](./apps.yaml) (Application registry) and [`developments.yaml`](./developments.yaml) (Development registry) — that together track the RacingPoint **product** portfolio's development.

---

## Two entities (design §3)

- **Application** — a product surface we ship. Carries **metrics** (dev/process + product/CTQ) and **toolchain/dependencies**. Lives in `apps.yaml`.
- **Development** — a DMADV-tracked work-item (a feature/capability = an "Index Item"). Carries the **DMADV lifecycle**. Tagged to the app(s) it touches. Lives in `developments.yaml`.

Relationship is many-to-many: a Development touches ≥1 App (`developments[].apps[]`); an App lists its active Developments (`apps[].active_developments[]`). The two id-spaces cross-reference by slug.

---

## P0 principle — sources, not stale values

P0 is hand-maintained. To avoid memory-projected/stale values, **every probe-able metric records its SOURCE (the command/endpoint), and its `value` stays `TBD (P1 auto-fill)`** until P1 wires the live pull. Only facts that are stable + dated may carry a literal value (with `as_of`). CTQ numeric targets are **`TBD-Captain`** — never invented here (design §5, §10.4).

**Data-source class** (design §5): `probe` 🟢 (automatable) · `partial` 🟠 (probe + manual) · `manual` 🔴 (human/Captain entry).

---

## DMADV phase model (design §4)

Status vocabulary (machine-clean; maps to the Index legend):

| YAML value | Index legend | Meaning |
|---|---|---|
| `done` | ✅ | phase complete |
| `in_phase` | 🟡 | actively in this phase |
| `not_started` | 🔴 | not begun |
| `gated` | ⛔ | blocked (freeze or dependency) |
| `frozen` | ❄️ | whole item frozen |

Phases: `D` (Define) · `M` (Measure) · `A` (Analyze) · `Design` · `Verify`.
**Freeze-gate:** for a `freeze_status: frozen` development, D/M/A may be `done|in_phase` (planning allowed) but `Design`/`Verify` MUST be `gated` until Captain-unfreeze + first-INR pass. The registry records state; it never unfreezes.

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
- id: <slug>
  title: <str>
  apps: [<app-id>, ...]           # apps this development touches
  dmadv: { D: <status>, M: <status>, A: <status>, Design: <status>, Verify: <status> }
  current_phase: <Define|Measure|Analyze|Design|Verify (+note)>
  freeze_status: unfrozen | frozen | in_flight
  ctq: [<str | "TBD-Captain">, ...]
  owner: bono | Captain | operator | James
  evidence_anchors: [<PR/SHA/path>, ...]
```

---

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
