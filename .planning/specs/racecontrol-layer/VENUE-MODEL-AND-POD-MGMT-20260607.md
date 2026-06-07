# Venue-model transport attribute + pod-mgmt helper (Phase 3, 2026-06-07)

**Author:** bono · **Branch:** `feat/venue-model-pod-mgmt` (off `origin/main` `983ca527`)
**Captain direction (2026-06-07):** own (RP-Esports) venues touch pods via Tailscale SSH driven by RaceControl; sold (third-party) venues use the installer file. This phase makes that selectable per venue + gives a uniform helper.
**Pairs with:** Phase 2 (installer venue-type SSH provisioning) — racecontrol **PR #131**. Same `own`/`sold` axis; Phase 2 *provisions* SSH at install, Phase 3 *resolves + uses* the transport at runtime.

## What landed
- **`.planning/specs/racecontrol-layer/venue-registry.json`** — the declarative `venue_type` / `pod_transport` source of truth, keyed by `venue_id` (node roles per `VENUE-NODE-ROLE-TAXONOMY.md`). Data, not compiled per-venue — the binary stays identical across venues (`config.rs` invariant). Ships `rp-vlm` (own, 8 pods) + a `_template-sold`.
- **`scripts/pod-mgmt.mjs`** — venue → transport → pod resolver + a `list`/`resolve`/`exec` CLI. Pure resolution logic is exported + unit-tested.
- **`scripts/pod-mgmt.test.mjs`** — 9/9 green (`node --test`).

## Transport model
| venue_type | primary | fallback | reach |
|---|---|---|---|
| **own** | `tailscale-ssh` (control_node → pod, key-only) | `tailscale-rc-sentry` (pod `:8091/exec`, audited) | direct over Tailscale |
| **sold** | `heart-exec` (control_node → venue_heart `pod_exec` proxy → rc-sentry → pod) | `heart-exec` | via the venue heart (no direct tailnet) |

SSH is a **complement** to the audited rc-sentry / heart channels, not a replacement — the helper falls back to them and they remain the automated path.

## Verification
- `node --test scripts/pod-mgmt.test.mjs` → **9/9** (resolution, fallback, sold-heart, error cases) on the control node.
- **NOT tested:** the credential-gated `exec` dispatch — SSH needs the pod key bootstrapped (Phase 1, gated on the operator-supplied rc-sentry key); rc-sentry/heart need `RCSENTRY_SERVICE_KEY`.

## Scope + follow-ups
- This is **control-node tooling + config** (no heart prod change → no deploy). Proportionate bar: unit tests + PR (not a heart auth/schema change, so no second full RCA/MMA — it reads config + dispatches to existing audited channels).
- **Follow-ups (deferred, documented):** (1) optional in-binary `CapabilityManifest` venue model in the heart if the resolver should live server-side; (2) make `pod-ssh-bootstrap.sh` + `frontend-drift-audit.mjs` read this registry (DRY the pod-address map); (3) `canary`/`deploy`/`screenshot` ops once Phase 1 keys exist.
- **racecontrol bono-sole lane → Captain merges; not self-merged.**
