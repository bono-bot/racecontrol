# FLEET-STALENESS-FIX-20260515 — 6-phase plan (v0.2 MAO-corrected)

**As-of:** 2026-05-15 ~16:33 IST (Fri)
**Authored by:** james (Captain commission 2026-05-15 ~16:10 IST *"find out other things that are also not up to date. And why was this not picked up by MAO?"* + autonomous-proceed verbatim)
**Status:** v0.2 — MAO-corrected (3 local subagents · 20 findings absorbed). v0.1 chat-only draft superseded.
**Authority subordinated to:** `V2-PROGRESS-MAP.md` §0.x · `V2-LBAC-PROTOCOL.md` · CLAUDE.md Standing Rules (Deploy / Comms / Testing & Verification / OTA Pipeline / Crash Loop / Cross-Boundary Serialization) · §S-298 wallet substrate 4-week class-A soak · §S-345 Captain "soak in parallel with live"

---

## §1 Audit snapshot (all H4 targets enumerated · ground truth as of 16:21 IST)

| Target | Deployed | Main HEAD | Δ | Critical missing |
|---|---|---|---|---|
| Venue :3200 web | `c5f94e31-dirty` 2026-04-29 15:06 | `56f67a8f` 2026-05-14 | **16d** | PR #80 /login API_BASE fallback · PR #84 7 sibling V1-pattern browser-fetch fixes |
| Venue :3201 admin | `c5f94e31-dirty` 2026-04-29 15:06 | same | **16d** | same |
| Venue :3300 kiosk | `fd59cf4b-dirty` 2026-05-14 14:55 | no commits since | dirty build only |
| Venue :8080 racecontrol | `ad410a32` §S-298 soak baseline | `c037352c` 2026-05-15 | **5 code commits** | PR #85 row 1.13 · PR #83 lap-FK drop · PR #88 row 7.6 cookie-auth · PR #87 dispatch.rs PR-A · §S-331 row 1.19 |
| Pods 1-7 rc-agent | `c5f94e31-dirty` 2026-04-29 | `3e728378` 2026-05-13 | **14d** | PR #66 silent-loop-death · PR #17 Pod-undefined · PR #54 PACT-013 billing-paused config-push |
| Pod 8 rc-agent | `8e378f4d` 2026-05-09 | same | canary-divergent + 4d stale |
| Pod 9 / POS .130 | OFFLINE | — | `ws=False http=False` |
| Cloud apex `racingpoint.cloud` | `424ca3dc` 2026-04-18 21:21 UTC | various | **27d** | pre-V2 brand · pre-row 1.4 · pre-§S-304 apex flip |
| Bono VPS racecontrol :8080 | `e4145650` (post-§S-369) | `800b86f2` docs-only | **AHEAD of Server .23 by PR #85** |
| James .27 own services | UNAUDITED | — | UNKNOWN | comms-link relay :8766 · rc-watchdog · CommsLink-DaemonWatchdog scheduled task · go2rtc · Ollama models |
| .bat files on pods | UNAUDITED | — | UNKNOWN | `start-rcagent.bat` + `start-rcsentry.bat` version drift (historical Pod 1 incident: 11x ConspitLink multiplication from stale bat) |
| DB schema migrations | UNAUDITED | — | UNKNOWN | venue vs cloud unrun migration parity (historical: cloud DB missing columns → manual ALTER on 8 tables) |

---

## §2 Doctrine corrections from MAO

**WITHDRAWN: §14.6.1 §6.4 "observational overlay carve-out" citation** (G9 #2 this session, CANDIDATE-N1). V2-LBAC-PROTOCOL.md:419-441 §14.6.1 governs **cascade-class DEPRECATE thresholds for V-LBAC methodology**, NOT soak-clock resets. There is no §6.4. The plan v0.1 misread V2-PROGRESS-MAP §0.2 line 64 as `§14.6.1.§6.4` when actual reading is `(§14.6.1) + (§S-322 §6.4)`. **No doctrinal authority currently exists for "deploy onto running class-A soak without resetting clock"** — Captain must ratify a new §14.6.x carve-out if F3a/F3c path is selected, OR pick F3d which sidesteps the question.

---

## §3 6-phase plan (sequenced)

### F0 NEW — Prerequisite audit (read-only) · P0

Read-only; autonomous-eligible. Outputs feed F1/F2/F3 pre-action gates.

1. **James .27 service staleness** — comms-link relay :8766 build vs git HEAD · rc-watchdog process running + `CommsLink-DaemonWatchdog` schtask registered + log recency <5min · go2rtc :1984 healthy
2. **.bat file drift sample on Pods 1-8** — SCP-or-rc-sentry-exec hash of `start-rcagent.bat` + `start-rcsentry.bat` vs `racecontrol/scripts/deploy/start-rcagent.bat`
3. **DB schema migration parity** — venue vs cloud unrun migration list (sqlx CLI or direct query on `_sqlx_migrations` table)
4. **Tailscale node hygiene** — confirm stale `bono@` node `racing-point-server` 100.71.226.83 still present in admin console (CLAUDE.md current-blocker)
5. **Pre-commit hooks install status** — `.git/hooks/pre-commit` present + executable on both James + Bono clones of racecontrol + comms-link

Outputs: `FLEET-STALENESS-AUDIT-F0-20260515.md` audit report appended below or as sibling doc.

### F1 — Frontend redeploy (P0 customer-impact)

**Surfaces:** venue :3200 web · :3201 admin · :3300 kiosk · cloud apex
**Eligibility check (per-PR §S-186 6-gate):**
- **PR #80** (`fix(web/login): API_BASE fallback uses window.location.hostname`) — likely fast-lane eligible (bug fix, single-boundary, no schema, no protocol)
- **PR #84** (`fix(web): 7 sibling V1-pattern browser-fetch consumers → same-origin relative URL`) — **likely FAILS gate 5** per [[v1-antipattern-fix-eligibility-check]] (URL construction · topology change class); requires per-PR Captain merge auth not fast-lane
- Captain ratify on PR-#84 path required BEFORE F1 begins

**Pre-action gates (MAO finding F-2):**
1. Write `DEPLOY_IN_PROGRESS` sentinel; disable watchdog schtasks per Deploy Pipeline Hardening standing rule
2. Confirm no active billing sessions venue-wide
3. F0 outputs reviewed — bat-file + James .27 staleness do not block

**Build + deploy (serialize, NOT parallel — MAO finding F-6 WIP-cap):**
1. Clean rebuild `web/` from `800b86f2` (no `-dirty` suffix); verify `build-info.json` shows clean commit before staging
2. Deploy venue :3200 → verify build_id flip + `/login` E2E from non-James machine + API proxy via :3200/api/v1/health (NOT :8080)
3. Deploy venue :3201 admin → same verification class
4. Deploy venue :3300 kiosk → same
5. Deploy cloud apex via Bono relay → verify `racingpoint.cloud/build-info.json` flips from `424ca3dc`
6. Clear `DEPLOY_IN_PROGRESS` sentinel; re-enable watchdog

**Rollback contract:** `_next/standalone/.prev` retained 72h per OTA standing rule.

### F2 — Pod fleet rc-agent redeploy (P0 fleet-resilience)

**Target HEAD:** `3e728378`
**Pre-action per pod:** `has_active_billing_session()==false` check via fleet/health OR rc-sentry `/exec` query BEFORE atomic swap (MAO finding F-4).

1. Pod 8 canary first per "Test display changes on ONE pod before fleet-wide" standing rule
2. 24h soak gate on Pod 8 (watch for silent-loop-death regression)
3. If clean, rollout Pods 1-7 in waves of 3 to maintain ≥5 customer-ready pods
4. Verify each: build_id flip + PR #66 sentinel ID grep + `:18924/debug` `edge_process_count > 0` + behavioral blanking verify

**Rollback contract:** `rc-agent-prev.exe` retained 72h on each pod.

### F3 REVISED — Racecontrol parity → **recommended F3d (bifurcate)**

Local MAO independently recommends F3d after withdrawing §14.6.1 fabrication:

| Path | Description | Recovery cost | Bilateral churn | Soak integrity |
|---|---|---|---|---|
| F3a | forward Server .23 to current main | medium (rollback to `ad410a32` + lose 5 PR state) | medium | **REQUIRES new §14.6.x Captain ratify** — current doctrine does NOT authorize |
| F3b | backport Bono VPS to `ad410a32` | **HIGH — destructive to bono §S-369 work + schema rollback** | worst | preserved |
| F3c | incremental — deploy non-wallet-class commits only (row 7.6 cookie · row 1.19 endpoint · dispatch.rs); defer wallet-class (row 1.13) + schema-class (lap-FK) until soak end | low | low | partially preserved — needs per-class Captain disposition |
| **F3d** | **bifurcate — cloud stays at `e4145650` post-§S-345, venue stays at `ad410a32` soak baseline; forward-merge at 2026-06-11 soak close** | **lowest (contained per branch)** | **lowest (bono work intact, venue soak intact)** | **fully preserved** |

**Recommended F3d** — honors §S-345 V2-live-ASAP on cloud, preserves §S-298 class-A soak observability on venue, leaves bono's §S-369 work intact, avoids fabricated-carve-out risk.

**Open Captain questions (decision-input gap per MAO GAP-5):**
1. Is row 1.13 wallet ceiling endpoint a soak-semantic change?
2. Is lap-FK migration `d47c26ba` reversible without data loss?
3. What observability data is forfeited if soak resets to T+0?
4. Does §S-345 "V2 live ASAP" mean venue-visible or cloud-visible? (Bono VPS already has V2 live; F3d would keep it that way)
5. Does any §14.6.x carve-out text exist that I missed?

### F4 — POS .130 recovery (availability)

3-probe reach test ([[capability_claim_without_probe]] discipline):
1. `tailscale status | grep pos1`
2. `curl http://192.168.31.130:8090/health` + `http://100.95.211.1:8090/health`
3. `ssh -o ConnectTimeout=5 admin@100.95.211.1 'echo OK'`

Branches:
- All 3 fail → escalate to Uday for physical check
- 1-2 succeed → diagnose layer-precision (network up but rc-agent dead → MAINTENANCE_MODE check → clear sentinel → RCWatchdog restart)

### F5 REVISED — Root-cause + recurrence prevention

Original F5 (`frontend-staleness-check.sh` enforcement audit) plus extensions surfaced by MAO:
1. Audit `frontend-staleness-check.sh` — what does it actually probe? Why silent for 16d?
2. Extend `run-all.sh` Suite 5 to include **bat-file version drift** + **James .27 service staleness** + **DB migration parity** as new sub-checks
3. Author `pre-prompt-fleet-staleness-check.js` UserPromptSubmit hook surfacing drift >7d
4. Add MAO lens 4 (doc-vs-deployed-state) as standard MAO panel composition when reviewing deployed-component inventories (sibling lesson from this MAO miss; structural fix per RCA)

Harness self-mod requires named-surface auth — Captain explicit ratify per pilot harness gate.

---

## §4 Sequencing + bilateral

**Order:** F0 (read-only, immediate) → F1 (serialize venue → cloud) → F2 (Pod 8 canary parallel with F1) → F4 (POS probe parallel) → F3d (bilateral handshake with Bono; no Captain ratify needed since F3d preserves both states) → F5 (recurrence-prevention, deferable)

**Bilateral:**
- F0/F1/F2/F4 — james-LED autonomous
- F3d — bilateral notify (Bono should know cloud-side is stable forward; no action required from him under F3d)
- F3a/F3c — would require explicit bilateral handshake + Captain §14.6.x carve-out ratify

**WIP-cap (MAO finding F-6):** treat each deploy target as separate WIP, not phase-as-unit. F1 = 4 targets serialized stays within cap. F0 + F2 canary + F4 probe = 3 read-only parallel slots compatible.

---

## §5 What this doc does NOT do

- No removals, deploys, or merges authored in this turn
- Original recommendation F3a withdrawn pending Captain ratify on §14.6.x carve-out (or selection of F3d which sidesteps)
- No OpenRouter MMA escalation — local MAO findings converged cleanly on F3d (RoI not warranted per tiered MAO doctrine)
- No harness self-mod (F5 hook authoring deferred pending Captain harness-mechanism-auth)

---

## §6 Composes-with

- `V1-DECOMMISSION-INVENTORY.md` (sibling Phase-0 inventory class) · `V2-PROGRESS-MAP.md` §0.x · `V2-LBAC-PROTOCOL.md`
- CLAUDE.md Deploy Pipeline Hardening · OTA Pipeline · Frontend deploy verification · Crash Loop Detection · ALL target enumeration
- §S-298 4-week class-A soak · §S-345 "soak in parallel with live"
- [[v1-antipattern-fix-eligibility-check]] CANDIDATE-N1 (PR #84 fast-lane eligibility)
- [[capability-claim-without-probe]] N=2-ACTIVE (F4 3-probe rule)
- [[branch-state-mutation-by-parallel-pilot]] N≥5 BILATERAL-ACTIVE (F3 bilateral surface)

---

## §7 Stale-at

Re-evaluation fires on ANY of:
- (a) Any §S-N close-anchor naming F0/F1/F2/F3/F4/F5 surface
- (b) Captain ratify on F3 path (F3a/F3c/F3d)
- (c) Any deploy that flips a build_id named in §1 audit table
- (d) Hard 7-day fire: **2026-05-22** (planning docs decay fast on operational topics)
- (e) Any customer-visible regression on §1 surface
