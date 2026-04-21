# V2 Foundation — Multi-Phase Rebuild Roadmap

**Status:** DRAFT — awaiting Uday sign-off
**Created:** 2026-04-21
**Owner:** James (on-site) + Bono (VPS)
**Goal:** Evolve Racing Point Ecosystem from v1 (organically grown) to v2 (architecturally sound) **without** greenfield rewrite, **without** stalling v1 bug-fix velocity, and **with** per-phase isolation so any phase can be paused or rolled back without affecting others.

---

## Why this exists

Accumulated structural debt catalogued from memory + open-patterns ledger:

| Pain | Evidence | Class |
|---|---|---|
| Config split across HKLM / `*.ini` / DB / WS cache / `mesh_kb.db` | Gap 4 (HKLM fetch), DB-2 (SQL migration), python.ini Zero Laps | **state-drift** |
| State derived, not primary | Pattern I silent-loop-death, Zero Laps 0-of-fleet lap persist | **no event ledger** |
| Untyped API surface | Phase 445 retroactive typing across 47 `.ts` files | **contract drift** |
| Mixed supervisors | schtask + HKLM Run + pm2 + RCWatchdog + watchdog-pos.ps1 | **supervision drift** |
| Mixed kiosk runtimes | Chrome (POS 2026-04-20) + Edge orphan leftovers + python.ini AC bridge | **runtime drift** |
| Server-patch regressions | `deploy-kiosk-server.ps1` re-regressed 2026-04-20 (fixed 2026-04-12) | **CI drift** |
| Repo fragmentation | racecontrol + admin + pwa + comms-link + whatsapp-bot + discord-bot | **atomic-change drift** |

Each class produces recurring G9s. v2 phases target one class at a time.

---

## Hard constraint: isolation contract per phase

Every v2 phase MUST satisfy **all five**:

1. **Additive deploy.** New code lives alongside old; old code unchanged at deploy time.
2. **Single-flag activation.** One env var or DB row toggles new path per target.
3. **Per-target rollout.** Pod 1 → Pod 2 → … → POS → Bono VPS; never fleet-wide in one shot.
4. **Kill-switch = redeploy-free revert.** Flip flag, old path resumes within 60 s. No rebuild.
5. **Observability before traffic.** Metrics + logs for new path exist and are scraped **before** any target flips.

If a phase cannot satisfy these five, it is NOT a v2 phase — it is a v1 refactor and belongs in normal milestone flow.

---

## Sequencing gates (in-flight work)

v2 Phase 1 kickoff is **gated** on these landing or being explicitly parked:

- [ ] **Phase 445 Wave 5** — split `fix/pos-kiosk-disable-20260421` branch before cloud deploy
- [ ] **Pattern I Part 5** — deploy to 0/8 pods (currently coded, 0 deployed)
- [ ] **Pattern I Part 4** — MMA Steps 1+2 before any code
- [ ] **Phase 414** — Uday deploy of continuous billing session
- [ ] **F4 PR #1** — merge or close
- [ ] **POS .130 audit** — resume Step 5 when venue returns from Tailscale outage

Rationale: each v2 phase will introduce commits that MUST NOT be tangled with in-flight work. Clean base branch is non-negotiable.

---

## Phase plan (revised 2026-04-21 after velocity calibration)

**Timeline correction.** Previous estimate (9 months) used generic enterprise pacing. Actual RP velocity ships milestones in days, not weeks. Phase 177 (which v2-P1 builds on) shipped 4 plans in ~2-3 days. Revised totals below reflect observed velocity + realistic gates (soak windows, Uday approvals, parallel-session coordination).

| Phase | Dev | Rollout | Depends on | Readiness |
|---|---|---|---|---|
| **P1** Config migration onto Phase 177 | 1-2 wk | 1 wk | — | **IMMEDIATE** (4 of 7 sub-phases can start today) |
| **P2** Event ledger (integrates F1) | 3-4 wk | 2 wk | P1 | After P1 |
| **P3** Finish Phase 445 typed contracts | 1-2 wk | — | P1 | Partially in-flight (Waves 0-4 shipped) |
| **P4** Monorepo consolidation | 2 wk | — | P3 | After P3 |
| **P5** One supervisor layer | 4-6 wk | 2-3 wk | P1 | After P1 — hardest phase |
| **P6** One kiosk runtime (drops python.ini) | 2-3 wk | 1-2 wk | P2 | After P2 |
| **P7** Real CI/CD (ghcr.io + signed manifests) | 2-4 wk | 1 wk | P5 | After P5 |

**Total realistic: ~4 months code-complete, ~5-6 months verified-deployed fleet-wide.**

Phase-by-phase detail lives in `v2/V2-ROADMAP.md`. Each phase entry in the roadmap has:
- Phase number (for `/gsd:plan-phase NNN`)
- Readiness tag (IMMEDIATE / GATED)
- Entry criteria (what must be true before kickoff)
- Exit criteria (what "done" looks like)
- Isolation contract specifics (which env var / file / endpoint the phase touches)
- Rollback runbook (the one-command revert)

### P1 — Config migration onto Phase 177 (IMMEDIATE)
**Pivoted:** 2026-04-21. Phase 177 already shipped the config service (2026-03-24, 13/13 verified). P1 is now caller migration.
**Kills:** state-drift class (OPENROUTER_KEY/OPENROUTER_API_KEY drift, hardcoded secret fallbacks, kiosk_settings↔Phase 177 parallel paths)
**Detail:** `V2-P1-CONFIG-SERVICE.md` + `v2/V2-ROADMAP.md` (phases 446-452)

### P2 — Event ledger as primary
**Kills:** no-event-ledger class (Pattern I silent-loop-death, Zero Laps persist-failure)
**Scope:** Append-only ledger of all state transitions. Current tables become materialized projections.
**Integrates:** F1 event-ledger milestone work — v2 P2 IS F1 widened
**Isolation:** Dual-write + 14-day diff-clean per table before cutover

### P3 — Finish Phase 445 typed contracts
**Kills:** contract-drift class
**Scope:** Land Phase 445 Wave 5 (deferred); extend utoipa/ts-rs to PWA + comms-link + whatsapp-bot
**Isolation:** Generated code only — zero runtime behavior change

### P4 — Monorepo consolidation
**Kills:** atomic-change-drift class
**Scope:** One pnpm + cargo workspace. Shared `@racingpoint/types` fed by P3 generators
**Isolation:** git-subtree; existing CI + deploy paths keep working via symlinks

### P5 — One supervisor layer (hardest phase)
**Kills:** supervision-drift class; structurally fixes Pattern I Part 4
**Scope:** Replaces schtask + HKLM Run + pm2 + RCWatchdog + watchdog-pos.ps1 with one declarative supervisor. Dead-man's-switch as a unit primitive
**Isolation:** New supervisor runs alongside existing; units migrate one at a time with 14-day soak

### P6 — One kiosk runtime
**Kills:** runtime-drift class (Zero Laps permanent fix)
**Scope:** Chrome-only fleet; drop python.ini AC bridge (SDK or UDP-only)
**Isolation:** Per-pod swap with existing `kiosk-swap-verify.ps1` (`aade74c2`)

### P7 — Real CI/CD
**Kills:** CI-drift class (server-patch regression 2026-04-12 + 2026-04-20)
**Scope:** ghcr.io artifact registry + signed manifests + pipelines replacing SCP + pm2 + schtask
**Isolation:** Staging target first (Bono VPS); production opts in one-at-a-time

---

## What starts NOW (zero-runtime-impact, immediate value)

Before P1 kickoff, these can run **this week** with no fleet touch:

| Task | Risk | Value | Lives where |
|---|---|---|---|
| Static audit: grep source code for every config read site (HKLM, `*.ini`, `std::env::var`, `read_to_string`, `ConfigStore::get`, etc.) and produce `config-inventory.md` | Zero (read-only local grep) | Blueprint for P1 — we learn the drift surface area in hours, not weeks | racecontrol + comms-link + whatsapp-bot + discord-bot + pwa + admin repos |
| CI drift-detector: add a lint that blocks new commits introducing **any** config read outside the future config service interface | Zero (CI-only) | Freezes the drift surface so it doesn't grow during P1 dev | `.github/workflows/` |
| Observability audit: enumerate every `println!` / `tracing::info!` / `console.log` that encodes state transitions and tag those that should become events in P2 | Zero (read-only) | Blueprint for P2 | all repos |
| This milestone doc + P1 detail doc (this session) | Zero | Written, reviewable, revisable | `.planning/specs/` |

None of these touch fleet, server, or cloud apps. All are local grep + docs.

---

## Rollback posture

For each v2 phase:
- A **Go** decision before every target flip (per-pod, per-key, per-service)
- A **Revert** runbook maintained in the phase's SUMMARY.md with the exact single command
- A **Freeze** option — if two targets regress, the phase is paused fleet-wide, root-caused, and does not resume until root cause is named and fix is written

No v2 phase gets a "deploy fleet-wide by Friday" pressure. If it can't roll pod-by-pod, it isn't a v2 phase.

---

## Non-goals (explicit)

- NOT a greenfield rewrite. There is no `racecontrol-v2` repo.
- NOT a database migration. P2 dual-writes; schema changes live in individual phases.
- NOT a UI redesign. Admin + PWA look identical post-v2; P4 consolidates code, not UX.
- NOT deprecating v1. v1 keeps running through all 7 phases. If we stop at P3, what's shipped is net-positive.

---

## Success criteria

v2 is "done" when **all** true:
1. One Rust crate reads/writes every piece of config (P1)
2. Every state transition is an event in a single append-only ledger (P2)
3. Every API boundary has generated types end-to-end (P3)
4. One `pnpm`/`cargo` workspace (P4)
5. One supervisor unit file per service fleet-wide (P5)
6. One kiosk runtime, zero python.ini AC reads (P6)
7. Zero manual SCP in deploy path (P7)

If we stop at any Pn, v2 stops clean — v1 keeps operating, no partial state.

---

## Open questions for Uday

1. **Timeline.** 7 phases × 5 wk avg = ~9 months. Acceptable, or should we cut scope (e.g., skip P4 monorepo)?
2. **Parallel with v50 rc-agent-mobile?** Or serialize v50 → v2?
3. **P1 first key confirmation.** `mesh_service_key` is my pick (Gap 4 already scoped it). Override?
4. **P7 artifact registry choice.** `ghcr.io` (free, public-private) vs self-hosted on Bono VPS vs Docker Hub?
5. **Budget for MMA audits.** Each phase's design doc through MMA adds ~$0.10-0.50 per consensus run. Budget cap per phase?
