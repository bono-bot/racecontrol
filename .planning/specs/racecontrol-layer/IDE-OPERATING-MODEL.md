# IDE Operating-Model — DMAIC / DMADV for tool-use, debug, deploy

> **Status:** v1 (Doctrine + tracking). **Owner:** bono. **Authored:** 2026-06-06.
> **Frame (Captain, 2026-06-06):** *all work done in this IDE is work done as part of the RaceControl Captain's Console.* This doctrine makes that operational: every piece of IDE work runs as a **DMAIC** or **DMADV** cycle and is tracked as a Console **initiative**.
>
> **This is a translation / orchestration layer.** It sequences gates we **already own** into phases. It does **not** restate them — each gate's behaviour lives in its canonical spec (cited inline). When a cited spec and this doc disagree, the **cited spec wins**.
>
> **Tracked as:** initiative `dev-ide-operating-model` (DMADV). **Discoverability:** bono memory `[[ide-operating-model-dmaic-dmadv]]`. **Not yet enforced** — the gates that already enforce keep enforcing; phase-binding hooks/skills are a deferred, Captain-auth'd step (see §6).

---

## 1. Frame & decision rule

Every unit of IDE work is one of two cycles, tracked as a Console initiative (framework + phase):

| If the work is… | Framework | Phases (gate phase **bold**) |
|---|---|---|
| **new** product / feature / mechanism / design | **DMADV** | Define · Measure · Analyze · **Design** · **Verify** |
| **improving / fixing / optimizing / debugging** something that exists | **DMAIC** | Define · Measure · Analyze · Improve · **Control** |

**Decision rule:** *Does the substrate already exist?* No → DMADV (you are designing it). Yes → DMAIC (you are improving it). **Debugging is always DMAIC** (you are restoring an existing thing). **Nested case:** a DMAIC fix that needs a brand-new mechanism spawns a child DMADV for that mechanism, inside the parent's Improve phase.

Phase definitions are canonical in [`DEV-PLATFORM-DESIGN.md §4`](../dev-platform/DEV-PLATFORM-DESIGN.md) and [`CONSOLE-DEV-MGMT-DESIGN.md §4–6`](./CONSOLE-DEV-MGMT-DESIGN.md). Phase status vocab (`done | in_phase | not_started | gated | frozen`) and `gate_state` derivation are canonical in [`SCHEMA.md`](../dev-platform/SCHEMA.md): **`gate_state: gate-clean` iff the gate phase (Control/Verify) is `done` with evidence; `blocked` if any phase is `gated`/`frozen` or `health: block`; else `open`.**

---

## 2. Phase → gate orchestration (the core)

For each phase: **entry** (what you must have to start) · **run** (the gates/tools that execute it — all pre-existing) · **done** (the exit criterion). DMAIC and DMADV share D/M/A; they diverge at the build + gate phases.

| Phase | Entry | Run these (canonical) | Done when |
|---|---|---|---|
| **Define** | a request or symptom | **CGP H1** PROBLEM+PLAN ([`COGNITIVE-GATE-PROTOCOL.md`](../../../COGNITIVE-GATE-PROTOCOL.md)) · **V2-transport check Q1–Q3** ([`racecontrol/CLAUDE.md`](../../../CLAUDE.md)) · state the **CTQ** + target app(s) · scope-freeze test (*does it close the first-INR bar, or is it V2.1+ → defer?*) | CTQ + target named; not frozen-out |
| **Measure** | a defined problem | **CGP H3** baseline from a **probe, not memory** · **capability/count-claim probes** (3-probe reach / N≥2 state / enumerate) · local `cargo test` / `tsc` baseline · **§S-146 RCA §1–3** (boundary map + inherited-issue catalogue) if a V1↔V2 surface | a baseline with a **cited source** |
| **Analyze** | a measured baseline | **§S-146 5-section RCA** (root cause) · **mechanism-trust-check (5Q)** for shared-infra fixes · **MMA** DIAGNOSE/PLAN (OpenRouter ≥5 models / ≥3 families) for foundational boundaries · **debug-skill router** (§4) for bugs | ≥1 root cause with evidence / an option chosen with rationale |
| **Improve** *(DMAIC)* / **Design** *(DMADV)* | a chosen root cause / design | smallest-reversible change · **atomic commit carrying a `Development: <id>` trailer** · **F1 scope-gate** (substrate exists before scaffolding tests, V-LBAC §14.2) · **MMA EXECUTE** · **MAOR Tier-1 review** ([`MAOR-PROTOCOL.md`](../v2/MAOR-PROTOCOL.md)) | the merged change is linked to the initiative |
| **Control** *(DMAIC)* / **Verify** *(DMADV)* — **GATE** | a merged change | **CGP H2** (verify in a *separate* message) + **H3** (exact behaviour + raw output + WHERE + NOT-TESTED) + **H4** (enumerate every target) · **MAOR pre-push gate** · **canonical-ref / branch-state / safe-trim guards** · **§3 deploy gate-chain** · a **monitor + regression guard** so the fix can't silently regress | behaviour proven on a real target + guard in place → sets `gate_state: gate-clean` |

**Why this matters:** we don't have a process gap, we have an *orchestration* gap. The gates are excellent but implicit and scattered; this table makes "which gate, when" explicit so it's teachable instead of tribal.

---

## 3. Playbook — Deploy

Deploy is the **Control/Verify gate-chain made rigorous** (new mechanism → DMADV-Verify; releasing a fix → DMAIC-Control). Run in order; any FAIL halts:

1. **Mechanism-trust-check (5Q)** on the delivery surface — ALL 5 YES, else that surface gets its own §S-146 RCA first ([`racecontrol/CLAUDE.md`](../../../CLAUDE.md) "Mechanism-trust-check"). Cache → `.planning/specs/v2/MECHANISM-TRUST/`.
2. **§S-146 cutover RCA** if the deploy touches a V1-dependent / foundational surface (billing / wallet / auth / pod-state / schema). Foundational → **MMA Step-1 DIAGNOSE** + per-PR Captain auth.
3. **MAOR** review of the cascade ([`MAOR-PROTOCOL.md`](../v2/MAOR-PROTOCOL.md)); **MAOR pre-push gate** (`pre-push-maor-check.js`) blocks the push without evidence.
4. **Guards** (PreToolUse, automatic): `pre-build-canonical-ref-guard.js` (content-diff vs canonical ref, money-path) · `pre-bash-destructive-git-branch-check.js` · `pre-bash-destructive-trim-guard.js`.
5. **Canary → N≥2 spaced behavioural verify → rollback staged → monitor.** Verify the **exact behaviour** (not health-200 / build_id theater — those prove the binary runs, not that the fix works). Pod-8 canary before fleet; record in `SWAPLOG.md`; preserve `*-prev.exe` (72h). Deploy parity: venue **and** cloud. Canonical runbook: [`racecontrol/CLAUDE.md`](../../../CLAUDE.md) "Deploy" + DMP [`docs/ARCHITECTURE.md §22`](../../../docs/ARCHITECTURE.md).

A deploy is **Control/Verify-done** only when step 5's behavioural evidence exists and a monitor/guard sustains it.

---

## 4. Playbook — Debug

Debugging is a **DMAIC sub-flow**. Primary method = **Closed-Loop Debug (CLD)** ([`docs/CLOSED-LOOP-DEBUG.md`](../../../docs/CLOSED-LOOP-DEBUG.md)): OPEN → DESCEND (6 layers) → FIX → CLOSE (re-run the *exact* Step-1 test) → SWEEP. Map to DMAIC:

| DMAIC | Debug action |
|---|---|
| **Define** | OPEN — reproduce the **exact** symptom (screenshot for UI / curl for API / tasklist for process), not a health check |
| **Measure** | DESCEND + the **4-Tier order** & **Cause-Elimination** ([`racecontrol/CLAUDE.md`](../../../CLAUDE.md) "Debugging Methodology"); gather evidence, list **all** hypotheses |
| **Analyze** | root-cause via the **debug-skill router** ↓ ; for V1↔V2 surfaces, **§S-146 RCA** |
| **Improve** | smallest reversible FIX at the layer where the cause lives; atomic commit w/ `Development:` trailer |
| **Control** | CLOSE (same test green) + SWEEP (every target) + **a regression guard** — *the fix isn't done until something prevents recurrence* (the H5 discipline) |

**Debug-skill router** (pick by symptom class):
- **`rp-incident`** — pod / ops / fleet incidents. 4-tier: deterministic → memory(LOGBOOK) → local Ollama → cloud. Auto-logs to LOGBOOK.
- **`gsd-debug`** — deep root-cause needing scientific-method + checkpoints across context resets (`--diagnose` = RCA only).
- **`tdd-debugger`** — test-driven (Red → Green → Verify) for contract/type/state-machine bugs.

---

## 5. Console tie-in & curation boundary

Every work item is an initiative (`developments.yaml` → `dev-registry.json` → `/initiatives`). Commits carry a **`Development: <id>` trailer** (the locked IDE→initiative link). A trailer-tagged commit's evidence is **auto-harvested** onto the board (see [`scripts/README-ide-initiative-sync.md`](../../../scripts/README-ide-initiative-sync.md)).

**Curation boundary — load-bearing:**

| Auto (harvested from git, advisory) | Deliberate (Captain Console sign-off, never automated) |
|---|---|
| evidence anchors (commit sha / PR / subject) | gate-phase transition (advance / sign-off Control·Verify) |
| latest-activity (last sha, ts, count) | CTQ targets · health · freeze · lifecycle |

The pre-rollout gate is a **read** of `gate_state` (advisory by default; hard per-ring toggle) — [`CONSOLE-DEV-MGMT-DESIGN.md §6`](./CONSOLE-DEV-MGMT-DESIGN.md). Automation never advances a phase or clears a gate; it only attaches objective facts. A `Development:` trailer whose id is **not** in the registry is reported as an **orphan** (curate a real initiative or fix the id) — never auto-created, so the board stays curated.

---

## 6. Scope & deferred enforcement

**v1 = Doctrine + tracking** (this doc + the harvest sync). Everything is bono-lane, reversible, **zero harness change**. The existing gates that enforce (CGP hooks, MAOR pre-push, the three guards, lane-guard) keep enforcing; this doc adds the *sequencing*, not new blocks.

**Deferred (needs Captain named-surface harness auth):** a `Phase:` commit annotation auto-surfacing live phase · the pre-rollout deploy gate reading `gate_state` as a hard block · a unified DMAIC/DMADV work-router / debug-router / deploy-gate-runner skill · phase-gating hooks · a condensed CLAUDE.md pointer. Also candidate (a later DMAIC *on this operating model*): a process-metrics surface (gate block-rate, false-negative escape rate) and a gate-failure-RCA template.

---

## 7. Canonical references (do not duplicate — these own the detail)

- **CGP H1–H5** — [`COGNITIVE-GATE-PROTOCOL.md`](../../../COGNITIVE-GATE-PROTOCOL.md)
- **§S-146 5-section RCA** + **mechanism-trust-check** — [`racecontrol/CLAUDE.md`](../../../CLAUDE.md) · `feedback_v1_dependent_v2_root_cause_before_proceeding.md`
- **MMA Protocol v4.0** (DIAGNOSE/PLAN/EXECUTE/VERIFY) — [`UNIFIED-MMA-PROTOCOL.md`](../UNIFIED-MMA-PROTOCOL.md)
- **MAOR** + **F1 scope-gate** + **V-LBAC** (10-step closed loop) — [`MAOR-PROTOCOL.md`](../v2/MAOR-PROTOCOL.md) · [`V2-LBAC-PROTOCOL.md`](../v2/V2-LBAC-PROTOCOL.md) §14.1/§14.2
- **CLD** + 4-Tier + Cause-Elimination — [`docs/CLOSED-LOOP-DEBUG.md`](../../../docs/CLOSED-LOOP-DEBUG.md) · [`racecontrol/CLAUDE.md`](../../../CLAUDE.md)
- **Deploy guards** — `~/.claude/hooks/{pre-build-canonical-ref-guard,pre-bash-destructive-git-branch-check,pre-bash-destructive-trim-guard}.js` · `SWAPLOG.md` · DMP [`docs/ARCHITECTURE.md §22`](../../../docs/ARCHITECTURE.md)
- **Phase model / Console** — [`DEV-PLATFORM-DESIGN.md`](../dev-platform/DEV-PLATFORM-DESIGN.md) · [`SCHEMA.md`](../dev-platform/SCHEMA.md) · [`CONSOLE-DEV-MGMT-DESIGN.md`](./CONSOLE-DEV-MGMT-DESIGN.md)
- **Debug skills** — `~/.claude/skills/{gsd-debug,tdd-debugger}` · `racecontrol/.claude/skills/rp-incident`
- **Scope frame** — V2 scope-freeze + first-INR definition-of-done ([`racecontrol/CLAUDE.md`](../../../CLAUDE.md))
