# V1 Decommission Inventory — Phase 0 read-only scoping

**As-of:** 2026-05-15 ~14:00 IST (Fri)
**Authored by:** james (Captain commission 2026-05-15 ~13:46 IST verbatim *"let's work on cleaning up Racing Point Ecosystem V1 from the venue. Come up with a plan."* + autonomous-proceed verbatim *"I authorize you to Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously"*)
**Status:** Phase 0 — READ-ONLY enumeration. No removals authored or executed. Phase 1+ Captain-gated.
**Authority subordinated to:** `V2-PROGRESS-MAP.md` §0.4/§0.7 (replacement-status canonical) · `V2-LBAC-PROTOCOL.md` §14 (gates) · [[feedback_v2_only_forward_path]] (V1 incorporated selectively, not categorically discarded) · [[feedback_v1_dependent_v2_root_cause_before_proceeding]] (foundational-boundary RCA + per-PR Captain merge auth before any FK / schema / auth / wallet / pod-state surface change).

---

## §1 — Decommission framing

"Clean up V1 from the venue" is a **decommission-by-replacement** problem at the current V2 maturity (~52-58% F3-pure DONE, ~6 LIVE-BLOCKING rows still BLOCKED). It is NOT a delete problem. Removing V1 surfaces ahead of their V2 replacements being LIVE + verified across all DEPLOY PARITY targets risks customer-facing outage.

Three categories per V1 surface:

| Tier | Criterion | Action this phase | Action when triggered |
|---|---|---|---|
| **A — SAFE-TO-REMOVE-NOW** | V2 equivalent LIVE + verified across all DEPLOY PARITY targets + V2-PROGRESS-MAP row = DONE | Document and propose for removal in Phase 1 (per-item §S-146 5-section RCA + Captain merge auth) | Stage removal PR |
| **B — V2-PARTIAL-QUARANTINE** | V2 equivalent partially replaces V1 (some surfaces flipped, others not) | Disable V1 write paths if safe; keep V1 read-only for fallback; tag with §S-N close-anchor + re-enable trigger | Wait for V2 row → DONE |
| **C — V1-LOAD-BEARING** | V1 is sole production substrate; V2 row = NOT-STARTED / TEST-SCAFFOLDED-AWAITING-SUBSTRATE / BLOCKED / ENGINEERING-IN-FLIGHT | Leave running; flag as "V1-load-bearing until Wave/Row N" | Reclassify when V2 row promotes |

**Out of scope this phase:** removals, write-path disablements, schema drops, fleet-wide cron cleanups. All gated on Captain ratify + per-item RCA + DEPLOY PARITY proof.

---

## §2 — Tier A: SAFE-TO-REMOVE-NOW (already-cleaned reference + true candidates)

### §2.1 Already-cleaned V1 surfaces (reference — no action required)

| Surface | V2 replacement | Cleanup anchor | Disposition |
|---|---|---|---|
| **Legacy ManagerXP marketing app at `racingpoint.cloud/` apex** | PR #69 PWA :3501 (Layer 1.10) | §S-303 / §S-304 Captain RATIFIED Option I 2026-05-14 | RETIRED |
| **V1 staff page kiosk-mirror (sibling V1-antipattern source-fix)** | row 1.13 PR-#85 substrate landed | PR #80 v2 MERGED `8b7c828d` (§S-330) + PR #85 §S-329 `c037352c` finalize | **PARTIALLY-CLEANED** — PR #80 source-fix MERGED but row 1.13 substrate PR #85 is DRAFT pending Captain merge auth per V2-PROGRESS-MAP §0.3 L142-143 (row IN-FLIGHT not DONE) |

Note: only ManagerXP apex flip is fully RETIRED. Staff-mirror row is partially-cleaned (source-fix landed, substrate PR Captain-gated) — kept here as exemplar of the §S-146 RCA shape, NOT as a completed Phase 1 candidate.

### §2.2 True SAFE-TO-REMOVE-NOW candidates

**Count: 0 high-confidence candidates identified in this Phase 0 pass.**

Rationale: the §0.4/§0.7 cascade snapshot shows no LIVE-BLOCKING row flipped V1→V2 cleanly enough that the V1 substrate could be removed without DEPLOY PARITY verification across Server .23 + Bono VPS + all 8 pods + POS .130 + Comms-link. Phase 1 will discover candidates by per-surface walk; this Phase 0 does not invent any.

**Action:** Phase 1 inventory walk (separate task) opens RCA per surface; SAFE-TO-REMOVE promotion happens at that walk, not here.

---

## §3 — Tier B: V2-PARTIAL-QUARANTINE candidates

| V1 surface | V2 status (per V2-PROGRESS-MAP) | Quarantine recommendation | Trigger to fully remove |
|---|---|---|---|
| V1 wallet ledger schema | Phase γ-β substrate landed (§S-285/290/295 deployed Server .23 `323b3d09` then overlaid by `ad410a32`); 4-week class-A soak 2026-05-14 → 2026-06-11; §S-345 supersedes Option E HOLD-during-soak so launch-gate lifted | Keep V1 ledger read-only during soak; no write-path disable yet — would gate top-up + redemption | Soak window close 2026-06-11 + Q-MI-A/B/C/D Captain disposition (wallet observability) |
| V1 brand colors / Enthocentric display font | V2 brand RATIFIED 2026-05-08; row 1.4 LIVE-BLOCKING `+1 DONE` per §0.2 PR #70 `a4908e44` (web-v2 frontend only) | **Source-fix REQUIRED** — kiosk `kiosk/src/components/DeployPanel.tsx:163` still has live `fontFamily: "Enthocentric, sans-serif"` inline style; Phase 1 task = remove Enthocentric from kiosk source + sweep admin frontend for residual `--rp-*` deprecated tokens | Frontend-staleness sweep across all 3 frontends (kiosk/web/admin) on all deploy targets |
| V1 cloud_sync surface (pre-Phase 1 /sync/echo) | cloud_sync Phase 1 MERGED `a22f79b2` (§S-243); §S-322 wallets-suffix observability probe overlaid `ad410a32` | Phase 1 substrate landed; V1 sync paths still execute in parallel until Phase 2/3 cutover defined | Phase 2/3 cutover ratify (not yet scheduled per §0.7 gating list) |
| **V1 WhatsApp alerter** (`crates/racecontrol/src/integrations/whatsapp_alerter.rs`) | NOT named in V2-PROGRESS-MAP; sibling row W1-S6 dispatch covers customer comms (PR #87 amendment-pending) but staff/admin alerter remains V1-shaped per §0.7 6.13 pm2-restart gating-list adjacency | Keep V1 alerter live for staff/admin alerts during W1-S6 amendment-pending window; do not disable write paths | Row W1-S6 PR #87 merge + verify + V2 alerter parity assessment (separate D-W1-S6-post) |
| **V1 scheduler / interval-driven subsystems** (`action_queue.rs` 30s HTTP polling fallback + `alert_engine.rs` 1800s ticks + `ai_behavior_batch.rs` 7d batch) | NOT covered by `/schedule` skill port DEFERRED ([[v2-progress-map-section0-refresh-durability-gap]] N≥4-ACTIVE) | Keep all 3 ticking; V2 scheduler substrate is pre-Phase-1; HTTP polling fallback is V1-shaped resilience not V1-decay | `/schedule` skill port + V2 scheduler ratify + per-subsystem cutover RCA |
| **V1 POS agent deployment target (.130)** | NOT enumerated in V2-PROGRESS-MAP layers; §5 of this doc defers per-machine walk but POS service-state belongs in Tier B | Keep V1 POS bundle deployed (`scripts/deploy/install-pos.bat` + `start-rcagent.bat` POS variant); V2 POS integration NOT defined | V2 POS integration scope defined + Captain ratify |
| **V1 camera / RTSP control** (`ac_camera.rs` periodic focus tick + go2rtc :1984 + 13 Dahua streams hardcoded RTSP) | NOT in V2-PROGRESS-MAP; venue-critical for spectator + safety; not customer-day-blocking | Keep V1 wiring; no V2 camera module exists | V2 venue-ops phase scope + camera module RCA |
| **V1 cron / scheduled-task surface** (Server .23 cron jobs: backup rotation · cloud_sync scheduling · session cleanup · process-guard tick · POS watchdog scheduled task · racecontrol watchdog mutex) | NOT enumerated; §5 originally deferred this — promoting to Tier B per MAO finding | Keep all live; data-integrity load-bearing; V2 job framework pre-defined | V2 job framework + per-job cutover RCA |

---

## §4 — Tier C: V1-LOAD-BEARING (DO NOT TOUCH)

Surfaces where V1 remains sole production substrate per V2-PROGRESS-MAP §0.4 / §0.7 BLOCKED + NOT-STARTED + ENGINEERING-IN-FLIGHT-LANDED rows:

| V1 surface | V2 row | V2 status | V1-load-bearing until |
|---|---|---|---|
| V1 cookie-auth / session model | row 7.6 | BLOCKED (5 STRUCTURAL GAPS; foundational; cascades to BLOCKED rows 1.6 + 1.8 + 1.9-partial); RCA AUTHORED, D-7.6-2 MMA Step 1 budget + D-7.6-3 per-PR merge auth Captain-pending | Row 7.6 substrate PR merged + verified |
| V1 cafe-orders kitchen flow | row 1.12 | ENGINEERING-IN-FLIGHT-LANDED (§S-341 `f7aeb41b` + `4793cdba` Phase 1 substrate; mechanism-missing Phase 2 endpoints) | Phase 2 endpoints landed + verified |
| V1 walk-in fallback flow | row 1.15 | ENGINEERING-IN-FLIGHT-LANDED (§S-339 `22df4bfb` + `a97ac4fb` Phase 1 substrate; mechanism-missing) | Phase 2 mechanism + Captain ratify |
| V1 operating-window enforcement | row 1.19 | ENGINEERING-IN-FLIGHT-LANDED (§S-334 `325f94a2` + `bbd21073`) | Phase 2 enforcement live + verified |
| V1 MI / Wave 4 ingestion | row 1.1 (Wave 4) | NOT-STARTED; Q-MI-A/B/C/D Captain-stake gating Phase 1 substrate PR D-1.1-1 (§S-338 RCA-MAOR composite) | Q-MI-A/B/C/D Captain ratify + Phase 1 PR land |
| V1 dispatch (email/WhatsApp comms) | row W1-S6 | ENGINEERING-IN-FLIGHT (PR #87 OPEN; MAOR CONDITIONAL 82% confidence — 3 IMPORTANT findings I-1/I-2/I-3 amendment-pending per §0.5) | PR #87 amended, re-MAOR, Captain merge auth |
| V1 lap-persistence FK (`laps.session_id` → `sessions(id)` gap) | foundational | RCA §S-323 5/5 MMA Step 1+2 unanimous on Candidate A; EXECUTE Captain-gated | Captain auth on EXECUTE; foundational, do NOT touch ahead of gate |
| V1 PACT-013 V2.0 wallet-credit-purchase + PACT-014 PWA-portal-customer-availability + PACT-015 failure-detection schema scope | filed §S-315 AMPLIFIER vote-batch; not yet executed as substrate PRs | Filed only; LIVE-BLOCKING when promoted to substrate PR queue | Captain promote PACT → PR + per-PR Captain merge auth |
| V1 cron jobs on Server .23 (V2 cron-port via /schedule skill DEFERRED) | V2-PROGRESS-MAP §0 refresh durability gap [[v2-progress-map-section0-refresh-durability-gap]] N≥4-in-30h structural-fix-ACTIVE; bono `/schedule` skill port pending | Manual session-start staleness check hook (CANDIDATE) | /schedule skill ports + cron durability ratify |
| All 8 pod-side V1 daemons (rc-agent, rc-sentry, RCWatchdog, etc.) | V2 Pod-Control Doctrine Wave 0 audit RATIFIED-WITH-AMENDMENTS-DEFERRED §S-182; Waves 1-FV still in progress | Pod-Control Doctrine Wave plan v0.3 (10 Waves: 0 → 0.5 → 1 → 5a → 2 → 3 → 4 → 5b → 6 → FV) | Per-Wave Captain ratify; FV is final |
| V1 racecontrol binary | same crate evolves forward — racecontrol IS V2; V1/V2 split is at *contract* + *substrate* layer not *binary* layer | NOT a removal candidate — binary is shared substrate | N/A |
| Bono VPS racecontrol :8080 (currently STALE on `98e70925`; §S-345 redeploy AUTHORIZED to parity `ad410a32`) | parity restoration pending bono-side execution | N/A — this is deploy parity, not V1 cleanup | bono executes redeploy |

---

## §5 — What this Phase 0 deliberately does NOT enumerate

- Per-file V1 code module list across `crates/racecontrol/src/`, `crates/rc-agent/src/`, `kiosk/`, `web/`, `apps/admin/`. **Reason:** the V1/V2 split is at the *boundary* + *contract* + *substrate* layer, not the *file* layer (per CLAUDE.md V2-only forward path § "V2 incorporates V1 modules" + DoD line 39+64). A per-file walk would imply a per-file V1/V2 binary classification that doesn't exist.
- DB schema drops or migration removals. **Reason:** any schema touch invokes §S-146 V1↔V2 RCA + foundational-boundary MMA Step 1 + per-PR Captain merge auth. Out of Phase 0 scope.
- ~~WhatsApp / Gmail / cron integrations.~~ **(v0.2-B amendment 2026-05-15: WhatsApp + cron PROMOTED to Tier B per MAO findings; Gmail remains deferred — no live consumer in venue path per V2 Customer Email Scope v2.1 defer.)**
- Bono VPS V1 surface enumeration. **Reason:** bono is the dominant pilot on §0.x cascade (just landed §0.7 ~09:55 IST per V2-PROGRESS-MAP); duplicating that pilot's substrate-walk would violate single-task-per-session + create bilateral coordination drift.

---

## §6 — Recommended Phase 1 entry conditions (when this doc gets promoted)

**v0.2-C restructure (MAO general-purpose findings 8-18): merged single-decision-package + multi-axis scope + per-tier authority matrix + abort criteria + rollback contract + soak + billing guard + bono handshake + MMA budget.**

### §6.1 Captain decision package (single ratify)

The 4 v0.1 entry conditions were partially circular and offered false dichotomies. Replaced with one composite decision package:

| Axis | Options | Recommended default |
|---|---|---|
| **Scope** (multi-select) | (a) services/binaries · (b) DB schema · (c) config + registry + scheduled tasks + bat files · (d) feature flags (FF-01+) · (e) WS protocol surfaces · (f) frontend deprecated tokens/components | **(a)+(c)+(d)+(f)** default; (b)+(e) need separate foundational-boundary auth per §S-146 |
| **Authority matrix** (per tier) | Tier A non-foundational → james + Captain merge · Tier A foundational → james RCA + MAOR Tier-1 + Captain merge · Tier B → +bono AMPLIFIER · any schema/wallet/auth → +MMA Step 1 | full matrix |
| **Cadence trigger** | (a) tied to Pod-Control Wave 1+5a completion · (b) open kaizen between waves bounded by WIP-cap 3 · (c) hybrid | **(c) hybrid** — Wave-tied for foundational, kaizen for Tier A non-foundational |
| **MMA budget envelope** | per Phase 1 batch; declare expected $ at PLAN time | $5/session cap per V4.0; Captain pre-approve for >$5 |

### §6.2 Rollback contract (MANDATORY per removal)

Every removal PR MUST include in description:
1. **Revert SHA** — pre-staged `git revert <commit>` ready to push
2. **Redeploy script** — exact deploy-server.sh / deploy-pod.sh invocation to restore prev binary (`*-prev.exe` retained 72h per OTA standing rule)
3. **Verification curl** — the single curl that proves V1 surface is back online if rollback fires
4. **DEPLOY PARITY rollback** — venue + cloud both restorable in same session

Staged BEFORE Captain merge auth. No rollback contract = merge auth denied.

### §6.3 Phase 1 abort criteria (halt + reassess)

Phase 1 halts immediately if ANY of:
- (a) Any removal triggers customer-visible regression → full G9, Phase 1 PAUSED
- (b) ≥2 RCAs reclassify Tier A items → Tier B/C → Phase 1 PAUSED for re-tier
- (c) MMA Step 1 verify FAILS on any foundational-boundary removal → halt
- (d) Bono parallel-pilot conflict on same surface (concurrent §S-N anchor or PR) → halt + bilateral sync per V2-LBAC §3 step 8
- (e) Post-fix gap rate ≥20% per cascade (sibling to §14.4 DEPRECATE-trigger watch)

### §6.4 Pre-action gates per removal

- **Billing-active guard** — `has_active_billing_session()==false` across all 8 pods + POS .130 before any binary swap (OTA pipeline standing rule)
- **Inter-removal soak** — ≥48h venue-active per Tier A removal; ≥1 customer day for any cafe/billing/wallet-adjacent surface; compounded blast-radius forbidden
- **Bilateral handshake** — bono §S-N ledger entry confirming james-LEAD on Phase 1 walks + non-conflicting surface list (V2-LBAC §3 step 8 bilateral close-loop)
- **MMA budget pre-declare** — per Phase 1 batch in PLAN; Captain pre-approve >$5

### §6.5 Phase 1 first task (when authorized)

Per-surface walk producing one RCA per Tier A / Tier B candidate, following §6.1-§6.4 gates. Walk order: lowest blast-radius first.

---

## §7 — Composes-with

- `V2-PROGRESS-MAP.md` §0.4 / §0.7 (replacement-status canonical; this doc cites it, does not re-derive it)
- `V2-LBAC-PROTOCOL.md` §14 (F1 SCOPE GATE + F3 ACCOUNTING REFORM apply at Phase 1 RCA time)
- [[feedback_v2_only_forward_path]] — V1 incorporated selectively, not discarded categorically
- [[feedback_v1_dependent_v2_root_cause_before_proceeding]] — every Tier A / Tier B removal needs a 5-section RCA
- [[feedback_pre_s146_small_fix_fastlane_20260511]] — does NOT apply (removals are not bug fixes)
- [[feedback_mechanism_trust_check_upstream_of_fix_rca_20260510]] — applies to any cleanup that touches shared infrastructure
- §S-345 Captain "soak in parallel with live" supersedes §S-307 — but does NOT authorize V1 substrate removal; only lifts Bono VPS redeploy gate
- DEPLOY PARITY rule — every V1 removal needs venue + cloud verify in same session

---

## §8 — Stale-at (v0.2-C tightened)

Re-evaluation fires on ANY of (whichever first; silent stale-pass banned — every fire produces written tier-reassignment delta OR explicit NO-CHANGE confirmation):

- (a) Any §S-N close-anchor whose subject line names a row in §3 or §4 tables above (by V1 surface name or V2 row-ID, e.g. `row 7.6` / `row 1.13` / `V1 wallet ledger`)
- (b) Any V2-PROGRESS-MAP §0.X refresh that flips a row to DONE for any Tier B or Tier C entry above (by row-ID)
- (c) **Hard time-fire: 2026-05-29** (14 days from authoring) regardless of (a)/(b)
- (d) Any customer-visible regression on a surface listed in §3/§4 → immediate re-eval + tier reassessment

Re-evaluation produces either: a tier-flip with §S-N close-anchor citation, OR a written NO-CHANGE confirmation appended below as a dated section. Re-evaluate Tier C → B and B → A promotion candidates at each fire.
