# NEXT-SESSION PICKUP HANDOFF — Map Pod Programs Goal + Roles in V2 Ecosystem

- **Author:** james
- **Date:** 2026-05-09 ~23:25 IST
- **Captain trigger:** "Produce a handoff to continue in another session. In a new session lets map out a proper goal for Pod programs and understand their roles in Racing Point ecosystem v2." 2026-05-09 ~23:24 IST
- **Session type:** DISCOVERY / ARCHITECTURE — produce a written goal + role map; NOT execution
- **Distinct track from:** `session_handoff_20260509_post_3block_lockfileex_pivot_NEXT_SESSION_PICKUP.md` (deploy-mechanism LockFileEx pivot — separate runway)

## Deploy-state enumeration

**Live-probe markers (run 2026-05-09 ~23:24 IST from James .27):**

```
$ git -C ~/racingpoint/racecontrol rev-parse --short HEAD
35890998   # feat/v2-wave-1-w1-s1-billing-service (parallel-james advanced from 638ef2da)
$ git -C ~/racingpoint/racecontrol rev-parse --short origin/main
8e378f4d   # PR #66 silent-loop-death
$ git -C ~/racingpoint/racecontrol rev-parse --short refs/remotes/origin/chore/mma-deploy-rca-artifacts
6ead86eb   # my push earlier this session — includes BRIDGE Step 4 BLOCK + tonight's other handoff
$ git -C ~/racingpoint/comms-link rev-parse --short HEAD
1e770cfb   # auto-synced with my MEMORY updates this session

$ curl -s --connect-timeout 5 --max-time 10 http://192.168.31.23:8080/api/v1/health
{"build_id":"c43459c8","deploy_context":"v34-v39 merged...","service":"racecontrol","status":"degraded",...

$ curl -s --connect-timeout 5 --max-time 10 http://192.168.31.23:8080/api/v1/fleet/health | head -c 200
{"dashboard_clients":0,"dashboard_ws_churn":...,"pods":[{"active_sentinels":[],...,"build_id":null,"http_reachable":false,...
```

**Per-target enumeration:**

| Target | Live state | Source |
|--------|-----------|--------|
| racecontrol feat HEAD | `35890998` (parallel-james advanced again from `638ef2da` since prior handoff at 23:05) | git rev-parse |
| racecontrol origin/main | `8e378f4d` | git rev-parse |
| racecontrol chore (remote) | `6ead86eb` (this session's push) | git rev-parse |
| comms-link HEAD = origin/main | `1e770cfb` | git rev-parse |
| Server .23 racecontrol | ALIVE — build=`c43459c8`, status=degraded (admin_db missing — separate concern) | curl /api/v1/health |
| Pods 1-8 | UNREACHABLE — physically off per Captain "shutdown for today" 22:59 IST; fleet-health confirms `build_id=null + http_reachable=false` | curl /api/v1/fleet/health |
| POS .130 (Pod 9) | NOT PROBED (independent of this discovery work) | n/a |
| Comms-link relay :8766 | NOT PROBED (independent of discovery work) | n/a |
| Bono VPS | NOT PROBED (assumed reachable) | n/a |

**Pod-related crates inventory (live `ls crates/`):**
- `rc-agent/` — Pod main process (port 8090) — runs in Session 1 via WTSQueryUserToken+CreateProcessAsUser
- `rc-sentry/` — Pod HTTP exec endpoint (port 8091) — X-Service-Key auth, /exec + /status + /healthz
- `rc-sentry-ai/` — Face detection on cameras (cam2, cam9, entrance) — runs on James .27, NOT pods
- `rc-watchdog/` — Pod Windows Service for auto-restart of rc-agent (Session 1 spawn via WTS)
- `rc-common/` — Shared types
- `rc-guardian/` — purpose unclear from inventory; investigate
- `rc-installer/` — Pod installer (D:\pod-deploy\install.bat v5)
- `rc-process-guard/` — Process allowlist enforcement
- `rc-process-manager/` — Process lifecycle (likely game launcher subsystem)
- `racecontrol/` — Server binary (NOT a pod program; Server .23)
- `v2-db/` — V2 schema crate (NOT a pod program; both Server + cloud)
- `weekly-report/` — Reporting (NOT a pod program)

**V2 skeleton docs inventory (`ls comms-link/v2-skeleton/`):**
- `01-skeleton-architecture.md`
- `02-flows-and-roles.md`
- `03-principles-and-philosophy.md`
- `04-connection-matrix.md`
- `05-definition-of-done.md`
- `06-vms-srl-cloud-migration-analysis.md`
- `10-ui-design-system.md`

## Coupling

- **Depends on:** V2 doctrine substrate already ratified — `project_v2_pod_display_state_channel_premise.md` (2026-05-08 21:17 IST Captain ratified) · `project_v2_customer_workflows_consolidated_20260503.md` · `project_v2_core_product_definition.md` · V2-MASTER-STATE canonical-source ledger · 7 v2-skeleton docs above · Wave 0a kiosk FSM foundation MERGED PR #65 `9f1c0a37`
- **Composes-with:** §S-146 V1↔V2 RCA doctrine (boundary-class change to a Pod program will fire this rule) · sentinel discipline + dry-test rules · per-PR Captain auth
- **Independent of (parallel tracks):** LockFileEx deploy-mechanism pivot (see sibling handoff `session_handoff_20260509_post_3block_lockfileex_pivot_NEXT_SESSION_PICKUP.md`) · Wave 1 PR-A through PR-E auth/lockout work · F25b billing-strategy substrate · Pod 5 physical recovery
- **Will block:** any new Pod-program-touching feature that needs to know "is this Pod program in scope, where does it sit in the customer journey?" — until this map exists, V1↔V2 RCA doctrine §1 (boundary map) cannot be authored properly for Pod-program changes
- **Will inform:** future Wave plans (Wave 0b–0f kiosk surfaces, Wave 2 dynamic pricing pods role, Wave 3 cafe/wallet pod display, V2.1 HUD plug-in via `DrivingViewRouter`)

## Verification

| Activity | What was tested | Observed |
|----------|-----------------|----------|
| Live preflight branch HEADs | git rev-parse for racecontrol + comms-link + chore | racecontrol feat=35890998, main=8e378f4d, chore=6ead86eb, comms-link=1e770cfb |
| Server .23 health | curl /api/v1/health | 200, build_id=c43459c8, status=degraded |
| Fleet-health pods-off confirm | curl /api/v1/fleet/health | pod entries with build_id=null + http_reachable=false → corroborates Captain "shutdown for today" |
| Pod-related crates inventory | ls crates/ | 9 candidate Pod programs surfaced (rc-agent, rc-sentry, rc-sentry-ai, rc-watchdog, rc-common, rc-guardian, rc-installer, rc-process-guard, rc-process-manager) |
| V2-skeleton docs inventory | ls comms-link/v2-skeleton/ | 7 docs available for next session to load |

## Null-test audit

- **What this handoff does NOT do:** pre-answer "what is a Pod program" — that is the discovery work for next session. This handoff frames the question and points at substrate.
- **What's NOT in the inventory above:** non-Rust pod-side programs (Edge browser, acs.exe, Content Manager, ConspitLink, AI launcher batch files, lock-screen overlay) — these need next-session enumeration via `tasklist` on a live pod (when pods come back online) AND grep of `start-rcagent.bat` + game launcher source
- **What success criteria for next session might miss:** "goal for Pod programs" could be parsed as (a) THE goal of the Pod machine in the V2 ecosystem (singular), (b) per-program goals (one goal per crate), or (c) the V2-aligned future-state goal vs current-state inventory. Captain may want to disambiguate this at session start.
- **What memory-rule promotion is now warranted:** none specifically from this discovery framing; depends on what next session reveals.

## Per-target evidence

### racecontrol chore branch (audit-trail home)
- This handoff committed to chore branch alongside tonight's deploy-pivot handoff
- Evidence: `git log --oneline chore/mma-deploy-rca-artifacts | head -5` next session

### racecontrol/.planning/handoffs/
- Existing parallel handoff: `session_handoff_20260509_post_3block_lockfileex_pivot_NEXT_SESSION_PICKUP.md` (deploy-mechanism)
- THIS handoff: `session_handoff_20260509_pod_programs_v2_role_mapping_NEXT_SESSION_PICKUP.md` (pod-programs discovery)
- Both NEXT_SESSION_PICKUP — Captain chooses which to pick up first (or run them in parallel sessions)

### memory MEMORY.md
- Index entry for THIS handoff to be added; auto-synced via live-sync hook

## NOT tested

- **Live tasklist on a pod** — pods physically off; non-Rust Pod programs (Edge / acs.exe / ConspitLink / etc.) cannot be enumerated this session
- **Pod 5 reachability** — separate work item, deferred
- **Pod 8 NEW binary `8e378f4d` watchdog parity vs c5f94e31** — relevant to Pod programs because rc-watchdog is a Pod program; deferred to LockFileEx track preflight
- **Whether `rc-guardian` is currently deployed to pods** — name suggests guard but inventory alone doesn't say; grep + git log next session
- **Customer-journey mapping on Pod surfaces** — V2 customer workflows are documented but Pod-side surfaces per state may not have a written role map per program
- **Bilateral cross-pilot bono-side perspective on Pod programs** — bono picks up via session-start git_pull; AMPLIFIER input expected
- **PWA / Kiosk frontend program inventory** — NOT in `crates/`; lives in `kiosk/` + `web-v2/` + `apps/` — needs separate inventory next session
- **Pod ↔ POS interactions** — POS.130 is technically Pod 9 in fleet-health but is functionally a different surface; relationship needs explicit map

## 1. Captain context — what's being asked

Verbatim 23:24 IST: "Produce a handoff to continue in another session. In a new session lets map out a proper goal for Pod programs and understand their roles in Racing Point ecosystem v2."

**Discovery scope (what I read into the ask):**
- "Pod programs" — plural; suggests multiple programs running on/for the Pod machines
- "Proper goal" — a singular V2-aligned goal that anchors the program set; OR per-program goals that compose into Pod's ecosystem role
- "Their roles in Racing Point ecosystem v2" — how each Pod program fits the V2 customer journey, V2-MASTER-STATE doctrine, and the other surfaces (POS, PWA, Kiosk, WhatsApp, cloud)
- "In a new session" — clean context window, fresh pickup, NOT continued from this session's deploy-mechanism BLOCK arc

**Captain disambiguation needed at session start (suggested clarifier):**
- (a) "Pod programs" = Rust crates only (rc-*) ?
- (b) "Pod programs" = everything that runs ON a pod (Rust + Edge + acs.exe + ConspitLink + game launchers + lock screen + bat files) ?
- (c) Both — separate map per program category ?

## 2. V2 substrate to load FIRST next session

In this loading order (cheapest → most-foundational):

1. **MEMORY.md** auto-loaded (index will have this handoff entry at top of Active section)
2. **`project_v2_pod_display_state_channel_premise.md`** — "Pod = state-channel surface NOT live HUD" — Captain ratified 2026-05-08 21:17 IST. THE foundational doctrine for Pod's role in V2. 9-state composition list. Wave 0a FSM foundation MERGED PR #65 `9f1c0a37`.
3. **`project_v2_customer_workflows_consolidated_20260503.md`** — 5 base + 6 missed customer scenarios; PWA/POS/portal/pods/Kiosk surfaces. AUTHORITATIVE customer-journey reference.
4. **`project_v2_core_product_definition.md`** — single-roof game launch + billing + cafe + WhatsApp marketing; Foundation/Profit/Customer-Adaptive axes.
5. **`comms-link/V2-MASTER-STATE.md`** — canonical-source ledger; %complete / surface count / phase status / F1-F12 / in-flight PACTs. **READ FIRST for any V2-state question.**
6. **`comms-link/v2-skeleton/01-skeleton-architecture.md`** + **`02-flows-and-roles.md`** + **`04-connection-matrix.md`** — architecture/flow/connection substrate; skeleton ADDS the missing connective tissue per `01-skeleton-architecture.md` §40.
7. **`comms-link/v2-skeleton/05-definition-of-done.md`** — keep/mold/discard filter; explicit V2 carry-forwards.
8. **CLAUDE.md (racecontrol + comms-link)** — Network Map (pod IPs), Crate Names table, Server Services table, Standing Rules.

Out-of-scope for first read (pull only if surfacing during discovery):
- Wave 1 RCAs (W1-S5/S6/W3) — auth/lockout work; not Pod-program centered
- F25 billing strategy substrate — billing-class, orthogonal to pod-program-role
- LockFileEx pivot handoff — sibling track; only relevant if rc-watchdog is in scope of "Pod programs"

## 3. Open architecture questions (for next-session discovery — DO NOT pre-answer)

**Q-PP-1: What IS a "Pod program" canonically?**
- (a) Rust crates only (rc-agent, rc-sentry, rc-watchdog, rc-process-guard, rc-process-manager, rc-installer, rc-guardian, rc-common-as-shared-lib)?
- (b) (a) + Edge browser instance + game launchers (acs.exe, ConspitLink, Content Manager) + bat files (start-rcagent.bat, launch-ac.bat)?
- (c) (b) + invisible OS-level (RCWatchdog Windows Service, HKLM Run key entries, scheduled tasks, registry guards)?

**Q-PP-2: What is the SINGULAR goal of the Pod machine in V2?**
- Per `project_v2_pod_display_state_channel_premise.md`: "Pod = state-channel surface NOT live driving HUD"
- Does this premise EXTEND from display-only to all Pod programs (state-channel surface across the WHOLE program set)?
- Or are there programs whose goal is NOT state-channel (e.g., game launchers exist to launch games — that's not state-channel; that's instrumentation)?

**Q-PP-3: How does each Pod program serve the V2 customer journey?**
- 5 base + 6 missed scenarios in `project_v2_customer_workflows_consolidated_20260503.md`
- For each scenario, which Pod programs fire? In what order? With what inputs/outputs?
- Pod display state #3 DRIVING is V2.1 HUD plug-in via `DrivingViewRouter` — for V2.0, what Pod programs are responsible for DRIVING-state behavior?

**Q-PP-4: What's the Pod's relationship to other V2 surfaces?**
- Pod ↔ Server .23 (racecontrol fleet APIs, billing, telemetry)
- Pod ↔ POS .130 (staff actions affecting Pod sessions; concurrency: HTTP 409 upgrade Phase 366)
- Pod ↔ PWA (V2 customer-facing; Wave 1 W1-S5+S6 auth/lockout/PIN)
- Pod ↔ Kiosk (currently same machine? separate? Wave 0a FSM foundation lives in `kiosk/`)
- Pod ↔ WhatsApp (Wave 1 W1-S7+S8 PIN delivery; staff alerts)
- Pod ↔ cloud Bono VPS (cloud_sync 30s; cloud authoritative on drivers/pricing, local on billing/laps/game state)
- For each edge: what protocol? what state? what failure mode? what V2 doctrine alignment?

**Q-PP-5: Which Pod programs are V1-inheritance vs V2-native?**
- Per V1-dependent V2 RCA doctrine (`feedback_v1_dependent_v2_root_cause_before_proceeding.md`), every Pod program touching V1-era code/schema needs RCA before change
- Need a written status per program: V1-inherited / V2-native / hybrid
- Which programs have OPEN bugs from V1 era that propagate forward unless RCA'd?

**Q-PP-6: What does "shipped" look like for the Pod-programs map?**
- An ARCHITECTURE.md section?
- A new V2-skeleton doc (e.g., `comms-link/v2-skeleton/07-pod-programs.md`)?
- A V2-MASTER-STATE §S-N entry?
- A doctrine PACT-PROPOSE for cross-pilot ratification?
- All of the above?

**Q-PP-7: What's the failure mode of NOT having this map?**
- Today: 7 Pod-related crates + ~6 OS-level non-Rust programs; their interactions are mostly tribal knowledge
- Tonight's 3 consecutive Step 4 BLOCKs on deploy mechanism caught structural flaws BECAUSE no map existed showing rc-watchdog SF-05 logic depends on rollback_manager.rs sentinel discipline (CR-FL-X)
- Future similar BLOCKs likely without this map; map closes the class

## 4. Success criteria for next session

The next session is COMPLETE when:

1. **Q-PP-1 disposed** — Captain ratifies which scope of "Pod programs" is in-scope for the map (a) / (b) / (c)
2. **Per-program inventory written** — for each in-scope Pod program: (i) name, (ii) location (path or process name), (iii) start mechanism, (iv) failure mode, (v) what it produces/consumes
3. **Singular V2 goal articulated** — the proper goal that anchors all Pod programs, traceable to V2-MASTER-STATE doctrine
4. **Per-program role mapped to V2** — for each in-scope Pod program: which V2 customer scenario(s) it serves; which V2 surface(s) it interacts with; V1-inheritance status
5. **Cross-pilot artifact path chosen** — Q-PP-6 disposed (ARCHITECTURE.md / new v2-skeleton doc / V2-MASTER-STATE §S-N / PACT-PROPOSE / combination)
6. **Substrate shipped** — committed + pushed; bono notify (responsive-to may apply since this is bilateral V2 architecture)
7. **Open follow-up trigger** named — e.g., "next session triggered when Wave 0b first surface PR opens, since Wave 0b is first Pod-program-touching V2 substrate to test this map against"

## 5. Verb shortcuts for next session

| Verb | Action |
|------|--------|
| **`begin pod-map`** | Run §2 substrate load + Q-PP-1 disposition then start §3 walkthrough |
| **`scope a only`** | Set Q-PP-1 disposition = (a) Rust crates only; proceed |
| **`scope b`** | Set Q-PP-1 disposition = (b) Rust + Edge + game launchers + bats; proceed |
| **`scope c`** | Set Q-PP-1 disposition = (c) everything Pod-side; proceed |
| **`enumerate pods live`** | If pods back online: tasklist /V on Pod 2 + 8 (canary) for non-Rust programs |
| **`load lockfileex track`** | Pivot to sibling handoff (deploy-mechanism); pod-programs map deferred |
| **`check bono progress`** | git_pull comms-link + V2-MASTER-STATE updates |
| **`end session`** | Commit + push + LOGBOOK + INBOX + metrics |

## 6. Open Captain Q-DECISIONs (carry-forward to next session)

- **Q-PP-1 to Q-PP-7** above
- **Q-PP-MERGE-1:** if next session produces a doctrine PACT-PROPOSE or new v2-skeleton doc, does it require MMA Step 1 DIAGNOSE on the doctrine itself (foundational-boundary class per V1↔V2 RCA rule)?
- **Q-PP-PARALLEL-1:** can the pod-programs map session run in parallel with the LockFileEx deploy-mechanism session, or do they share enough substrate that sequencing matters?

## 7. Composes-with (canonical anchors)

- `project_v2_pod_display_state_channel_premise.md` (Captain ratified 2026-05-08 21:17 IST) — THE foundational doctrine for Pod's role in V2
- `project_v2_customer_workflows_consolidated_20260503.md` — 5 base + 6 missed customer scenarios
- `project_v2_core_product_definition.md` — V2 core
- V2-MASTER-STATE canonical-source ledger
- 7 v2-skeleton docs (01-06 + 10)
- Wave 0a kiosk FSM foundation MERGED PR #65 `9f1c0a37`
- Sibling handoff (separate track): `session_handoff_20260509_post_3block_lockfileex_pivot_NEXT_SESSION_PICKUP.md`
- §S-146 V1↔V2 RCA doctrine (boundary-class change to any Pod program will fire)
