# REGISTRY — Dev-Platform live readout (P1, GENERATED)

> **GENERATED FILE — do not hand-edit.** Regenerate: `python3 scripts/dev-platform/build_registry.py`
> **Generated:** 2026-06-05 01:37 IST (UTC 2026-06-04T20:07:33Z) on `srv1422716` · P1 of [`DEV-PLATFORM-DESIGN.md`](./DEV-PLATFORM-DESIGN.md) §8.
> **Source registries (hand-maintained):** [`apps.yaml`](./apps.yaml) · [`developments.yaml`](./developments.yaml). Probe values below are LIVE; failures show `unavailable`; venue/auth probes are DEFERRED (see end).

**Portfolio:** 15 product apps · 12 DMADV developments (7 frozen).

## Applications (live probes)

| App | Line | Last commit | CI (latest) | Repo open PRs | Active devs |
|---|---|---|---|---|---|
| `racecontrol-console` | A | 2026-06-01 | failure | 15 | 2 |
| `pod-display` | B | 2026-05-29 | failure | 15 | 2 |
| `pos` | B | 2026-05-29 | failure | 15 | 2 |
| `pos130` | B | 2026-05-29 | failure | 15 | 0 |
| `staff-tablet` | B | 2026-05-29 | failure | 15 | 2 |
| `kiosk` | B | 2026-05-29 | failure | 15 | 0 |
| `pwa` | B | 2026-05-29 | failure | 15 | 2 |
| `launch-portal` | B | 2026-05-29 | failure | 15 | 1 |
| `chef-display` | B | 2026-05-29 | failure | 15 | 0 |
| `captain-console` | B | 2026-06-01 | failure | 15 | 2 |
| `racecontrol-heart` | B | 2026-06-04 | in_progress | 20 | 7 |
| `rc-agent` | B | 2026-05-12 | in_progress | 20 | 1 |
| `rc-installer` | B | 2026-05-31 | in_progress | 20 | 0 |
| `cloud-dashboard` *(candidate)* | C | — | no-runs | 0 | 0 |
| `api-gateway` *(candidate)* | C | — | no-runs | 0 | 0 |

*CI/open-PRs are repo-level (monorepo single pipeline) attributed to each app in that repo.*

## Developments — DMADV board

| Development | D | M | A | Des | V | Current phase | Freeze |
|---|:-:|:-:|:-:|:-:|:-:|---|---|
| Multiplayer racing | ✅ | 🟡 | 🟡 | 🟡 | 🔴 | Analyze/Design — lobby.rs ~95%; S0 preflight_acserver merged | live |
| Pod-display error screens (Phase 2) | ✅ | 🟡 | 🟡 | 🟡 | 🔴 | Design — Ph1 server-lost merged; Ph2 updating/OTA + crash pending heart display_message | live |
| Cross-venue AC leaderboard | ✅ | 🟡 | ✅ | ✅ | ⛔ | Verify — deploy-gated (Server-.23 deploy of both binaries) | live |
| Grace-countdown symmetry (customer vs staff) | ✅ | 🟡 | ✅ | 🟡 | 🔴 | Design — PR #25 (pod.live_timers) | live |
| Per-game leaderboards (generalize AC pattern) | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ frozen |
| Multi-tenant control plane (Halo) | ✅ | 🔴 | 🟡 | ⛔ | ⛔ | Define/Analyze — design verified; 8 blockers B1-B8 | ❄️ frozen |
| Console V2+ (Ring6 Releases / Ring7 Billing / brand-pack) | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define — P3 prototype in Drive | ❄️ frozen |
| Customer email & messaging (Wati / WhatsApp BSP) | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ frozen |
| V1 decommissioning | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define — V1-DECOMMISSION-INVENTORY exists | ❄️ frozen |
| Walk-in registration (Phase 2) | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define | ❄️ frozen |
| Incident RESOLVE staff UI (VIEW half merged) | ✅ | 🔴 | 🟡 | 🟡 | 🔴 | Design (partial) — read/VIEW half merged; RESOLVE half + cadence open | in-flight |
| Refund / manual-adjust / dispute UI | 🟡 | 🔴 | 🔴 | ⛔ | ⛔ | Define — apology-credit is the V2.0 path | ❄️ frozen |

Legend: ✅ done · 🟡 in-phase · 🔴 not-started · ⛔ gated · ❄️ frozen.

## Deferred probes (not runnable from this host — design §5 🟠/🔴)

| Metric | Source | Why deferred |
|---|---|---|
| build_id / fleet state | `GET http://192.168.31.23:8080/api/v1/fleet/health` | venue .23 + pods (0/8 off) |
| pod health score | `GET /api/v1/fleet/intelligence` | venue .23 + staff JWT |
| contract parity | `pnpm run check-parity` | needs rp-v2-apps install / CI runner |
| revenue / session success | `billing_sessions SQL` | venue/cloud DB + auth |
| code coverage % | `(not instrumented)` | no nyc/tarpaulin in CI yet |

These land when P1 runs from a venue-reachable/authed context (or P2 automation wires CI secrets + a venue probe relay).

