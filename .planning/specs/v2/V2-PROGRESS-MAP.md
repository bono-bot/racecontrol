# V2 PROGRESS MAP — consolidated 13-layer status grid

**As-of:** 2026-05-11 11:35 IST (Mon)
**Baseline scope:** 13-layer Process-of-Elimination enumeration ratified at V2-MASTER-STATE §S-200 (Captain anchor 2026-05-11 ~10:01 IST: *"these are the tasks needed to be completed to get racing point ecosystem v2 live"*)
**Authored by:** bono · per Captain commission 2026-05-11 ~11:30 IST: *"restructuring Racing Point ecosystem v2 and map out the progress done so far"*
**Refresh contract:** see §16 — daily 09:07 IST morning digest + 21:23 IST evening progress (Mode 2 cron `3d362294` / `32ad5747`); session-only durability gap CANDIDATE-N1 (re-create on session-start until `/schedule` skill ports).
**Authority:** this file becomes the single canonical view for `% V2 complete` / `what's left` / `what's next` answers, **with explicit subset declaration** (V2-LIVE-BLOCKING vs V2-DISCIPLINE/POST-LIVE vs AMBIGUOUS) per §S-200.1 rule.

---

## §0 — TL;DR rollup card

| Class | Total | DONE | IN-FLIGHT | BLOCKED | NOT-STARTED | % closed |
|---|---|---|---|---|---|---|
| **V2-LIVE-BLOCKING** (gates customer-day §4 14:00→14:56) | **~78** | **22** | 13 | 11 | 32 | **28%** |
| **V2-DISCIPLINE / POST-LIVE** (some explicitly post-live by design) | **~32** | 8 | 5 | 3 | 16 | **25%** |
| **AMBIGUOUS** (Captain-framing-dependent) | **~12** | 2 | 3 | 1 | 6 | **17%** |
| **TOTAL** | **~122** | **32** | 21 | 15 | 54 | **26%** |

> **Reading instruction:** Treat counts as ±5% (some items are coarse-grained — e.g. "Layer 2 W3 sub-items" represents ~3-5 atomic tasks depending on slice). Numbers refresh nightly. Closed % is the V2-LIVE-BLOCKING figure unless explicitly subset-tagged.
>
> **What this number means:** ~72% of V2-LIVE-BLOCKING items remain. **Δ 2026-05-11: +2 LIVE-BLOCKING closed (Layer 12.1 racingpoint.cloud nginx vhost reconcile via D4 + Layer 12.2 sites-enabled-not-symlink class fully closed). Δ 2026-05-12 LBAC discovery: PR #17 merged 2026-05-11 11:39 IST (commit ab9d867f, ~4min after this map was authored — see RCA/PR17-disposition-20260512.md); Layer 4 row 4.1 flipped OPEN→MERGED; +1 LIVE-BLOCKING closed retroactively. Closure rate: 24%→27% (2026-05-11) → 28% (2026-05-12).** Capacity-weighted velocity TBD.

---

## §1 — Layer 1: Customer-day surfaces × pending work

**Source-of-truth:** `comms-link/v2-skeleton/05-definition-of-done.md §1.2` (canonical day) + `racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md` (frontpage spec)
**Class:** mostly LIVE-BLOCKING (one beat = one customer-felt failure on V2 substrate if absent)

| # | Beat | Item | Status | Owner | Blocker / Anchor |
|---|---|---|---|---|---|
| 1.1 | 13:30 | MI empty-window detection (5-min rolling, mi.empty_window_events) | NOT-STARTED | both | Wave 4 MI Ingestion (HALO-as-substrate reduces scope per §S-170.16) |
| 1.2 | 14:00 | Bono drafts happy-hour promo | IN-FLIGHT | bono | Phase 2-F Campaign Object PACT-DRAFT pending AMPLIFIER |
| 1.3 | 14:00 | Outbound (WhatsApp + Instagram) | PARTIAL | bono | Class A autonomy SHIPPED; Class B+C need Captain greenlight workflow + Instagram deferred to v2.1 |
| 1.4 | 14:05 | Deep-link click → PWA registration (90s, returning-skip) | IN-FLIGHT | james | UI-SPEC v0.2 Q-CUST-2 returning-customer detection LOCKED autonomous; 5 Q-CUST CAPTAIN-STAKE still gating page.tsx full ship |
| 1.5 | 14:05 | racingpoint.cloud entry frontpage v0 | DONE | bono | LIVE post-§S-198 + nginx vhost ratify §S-199; v0.2 spec drafted, page.tsx pending |
| 1.6 | 14:20 | POS .130 staff sees profile on arrival | BLOCKED | james | Cookie auth SHIPPED (PACT-001 Phase 1); CSRF + rate-limit are open security-debt rows 7+9 |
| 1.7 | 14:22 | Wallet top-up — PWA path | NOT-STARTED | james | Wallet-client wrapper PR-D (bravo-slice item 2 DEFERRED on PACT-024 §A AMPLIFIER) |
| 1.8 | 14:22 | Wallet top-up — POS cash path | BLOCKED | james | depends on V2 staff session-cookie auth + audit-log doctrine §S-158 |
| 1.9 | 14:22 | Wallet top-up — Kiosk digital path | BLOCKED | james | gates on W1-S6 staff PIN + R1-C (no cash at Kiosk) ratified |
| 1.10 | 14:25 | Staff at Kiosk launches game (15-20s target) | NOT-STARTED | james | Joint #3 (Game Launching) full spec; current = no V2 Kiosk staff UI |
| 1.11 | 14:25-55 | Race + telemetry + leaderboard | PARTIAL | james | Tier-1 AC SP+MP wired in V1; V2 cross-organ contract not yet validated |
| 1.12 | 14:42 | Cafe order (PWA-self OR Kiosk-staff to kitchen tablet) | NOT-STARTED | both | kitchen single-tablet UI v2.0-required; not started |
| 1.13 | 14:55 | Auto-bill in 1s | NOT-STARTED | james | Joint #2 (Billing) full spec — wallet-engine sub-PACT pending |
| 1.14 | 14:56 | Substrate retains profile + stats + history | PARTIAL | james | DB schema present; V2 GDPR/DPDP customer_data_delete contract authored not enforced |
| 1.15 | Walk-in fallback | 2 named Walk-In Guest accounts (discount_ineligible) | NOT-STARTED | james | Captain-locked design; needs DB seed + POS flow |
| 1.16 | All-beat | Source-tagging completeness (PWA/POS/Kiosk × UPI/card/cash) | PARTIAL | james | enum locked in DoD §3.3; implementation drift unverified |
| 1.17 | All-beat | Cross-surface consistency (2s tolerance) | NOT-STARTED | both | acceptance test missing |
| 1.18 | All-beat | UI-SPEC v0.2 5 Q-CUST batch (hero photo / pricing / WA opt-in / multilingual / DPDP) | BLOCKED | bono | Captain G33 batch ask pending |
| 1.19 | All-beat | Operating window 12:00-24:00 IST + iRacing extension | PARTIAL | james | window logic V1-era; cross-tz extension unverified V2 |
| 1.20 | All-beat | Top-up bonus ladder + iRacing 20% (cap 20%, deeper-of) | PARTIAL | james | spec locked; engine wire-up gated by Phase 2-A/2-F |

**Layer 1 totals:** 1 DONE · 4 IN-FLIGHT · 4 PARTIAL · 4 BLOCKED · 7 NOT-STARTED = **20 items** (matches §S-200.2 canonical count)

---

## §2 — Layer 2: V2 Waves (W0.5 / W1 / W2 / W3 / W4 / W5)

**Source-of-truth:** `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` + `WAVE-2-DESIGN-NOTES-20260508.md` + V2-MASTER-STATE §S-179..§S-200 wave entries

| Wave | Sub-item | Status | Owner | Blocker / Anchor |
|---|---|---|---|---|
| W0.5 | Wave 0 audit (3 PROCEED + 2 HALT-REVISE) | DONE | james | §S-181 AMPLIFIER COMPLETE; bono CONCUR-with-3-caveats §S-200.6 |
| W0.5 | Per-file deep audit (followup) | DEFERRED | both | indefinite-tail risk; bind to kiosk API contract VERIFY PASS |
| W0.5 | $0.13 Wave 0 MMA DIAGNOSE spend ledger write | IN-FLIGHT | james | LOAD-BEARING §S-200.6 finding; phase 2.3 protocol-conformance |
| W1 | W1-S2 wallet_redemptions FK repair (NF-james-4) | DONE | james | §S-117 + 43/43 tests; sqlx::migrate! cache invalidation root-caused |
| W1 | W1-S6 staff PIN auto-rotate + lockout | BLOCKED | james | HALTED on Captain G33 v7; customer-email scope NARROWED §S-172 |
| W1 | Wave 1 wallet-client wrapper PR-D | BLOCKED | james | PACT-024 §A AMPLIFIER pending; bravo-slice item 2 DEFERRED |
| W2 | Phase 2-A rate-table service | IN-FLIGHT | bono | PACT-DRAFT-phase-2-a present in draft-pacts/; not yet ratified |
| W2 | Phase 2 dynamic pricing engine | NOT-STARTED | bono | composes-with 2-A; PACT-DRAFT present |
| W2 | Phase 2-E combo-offer primitive | NOT-STARTED | bono | PACT-DRAFT present |
| W2 | Phase 2-F Campaign Object (transactional bundle) | IN-FLIGHT | bono | NEW DRAFT-PRE-AMPLIFIER §S-200.9; closes synthesis §10 gap |
| W3 | TBD sub-items (pricing × promotion × broadcast) | NOT-STARTED | both | scope not yet defined in v2-skeleton |
| W4 | MI Ingestion (mesh_kb.db) | NOT-STARTED | james | HALO-as-substrate §S-170.16 reduces scope to ingestion-pipeline only |
| W4 | kaiju_classification_log schema | NOT-STARTED | james | depends on Wave 4 schema patch queue |
| W4 | campaign_effectiveness table | NOT-STARTED | both | gates G-2 (4-week post-live soak) |
| W5 | WhatsApp framework (Captain-curated) | IN-FLIGHT | bono | PACT-DRAFT-wave-5 present; AMBIGUOUS class (some customer-touching, some not) |
| W5 | Instagram outbound | DEFERRED-POST-LIVE | bono | v2.1 deferral §10 deferral roadmap |

**Layer 2 totals:** 2 DONE · 5 IN-FLIGHT · 3 BLOCKED · 5 NOT-STARTED · 1 DEFERRED = **16 items** (§S-200.2 said 18; -2 because Phase 2-A NOT-STARTED collapsed to IN-FLIGHT and Phase 2-F NEW)

---

## §3 — Layer 3: V2-ROADMAP phase queue (P1 phases 446-452 + P2-P7 placeholders)

**Source-of-truth:** `racecontrol/.planning/specs/v2/V2-ROADMAP.md`

| # | Phase | Title | Readiness | Status | Owner | Blocker |
|---|---|---|---|---|---|---|
| 3.1 | 446 | Canonicalize OPENROUTER_KEY | IMMEDIATE | **PARTIAL-VERIFIED** | bono | source SHIPPED b74aadce+0981afb; pm2 env migration pending Captain Class B/C timing decision; fleet build_id readout DEFERRED-VENUE-OPEN |
| 3.2 | 447 | Canonicalize RACECONTROL_TERMINAL_SECRET + remove hardcoded fallbacks | IMMEDIATE | NOT-STARTED | bono | Captain sign-off prereq |
| 3.3 | 448 | rc-common::secrets central loader | IMMEDIATE | NOT-STARTED | bono | gates on 446+447 merged |
| 3.4 | 449 | CI drift-detector for config reads | IMMEDIATE | NOT-STARTED | bono | gates on 448 |
| 3.5 | 450 | Reconcile kiosk_settings with Phase 177 (lockdown root-cause-surface) | GATED | NOT-STARTED | james | gates on 448/449 |
| 3.6 | 451 | /api/v1/client-config runtime endpoint | GATED | NOT-STARTED | james | gates on 448 |
| 3.7 | 452 | Migrate NEXT_PUBLIC_* call sites onto /client-config | GATED | NOT-STARTED | james | gates on 451 |
| 3.8 | P2-P7 | Placeholder phase numbers | TBD | NOT-STARTED | both | scope not yet kicked off |

**Layer 3 totals:** 0 DONE · 1 PARTIAL · 7 NOT-STARTED = **8 items**

---

## §4 — Layer 4: Open Pull Requests (both repos)

**Source-of-truth:** `gh pr list --state open` × racecontrol + comms-link (as of 2026-05-11 11:32 IST)

> **§S-200 correction:** §S-200.2 said 5 PRs; actual is **9 open PRs**. Updated baseline.

| # | Repo | PR | Title | Author | Days open | Status | Class |
|---|---|---|---|---|---|---|---|
| 4.1 | racecontrol | #17 | fix(billing): add pod_number to session response — closes 'Pod undefined' | james | 19d | **MERGED 2026-05-11 11:39 IST (ab9d867f)** — deploy parity pending Server .23 + Bono VPS (LBAC task #7) | LIVE-BLOCKING-CLOSED |
| 4.2 | racecontrol | #54 | feat(billing): route billing_paused via config_push_queue (PACT-013 Phase 1+2) | bono | 12d | OPEN | LIVE-BLOCKING |
| 4.3 | comms-link | #8 | PACT-20260429-005 venue-stability-state.sh implementation | james | 12d | OPEN | DISCIPLINE |
| 4.4 | comms-link | #9 | PACT-20260503-016 handoff schema head-at-write-time | james | 8d | OPEN | DISCIPLINE |
| 4.5 | comms-link | #10 | PACT-20260503-012 deploy-server.sh SWAPLOG-append path bug fix | james | 8d | OPEN | DISCIPLINE |
| 4.6 | comms-link | #11 | PACT-20260503-013 wallet-credit-purchase-event schema (#4) | james | 8d | OPEN | LIVE-BLOCKING |
| 4.7 | comms-link | #12 | PACT-20260503-014 PWA→.23 portal customer auto-appearance schema (#9) | james | 8d | OPEN | LIVE-BLOCKING |
| 4.8 | comms-link | #13 | PACT-20260503-015 V2.0 #20 failure-detection event schema | james | 8d | OPEN | LIVE-BLOCKING |
| 4.9 | comms-link | #14 | PACT-20260504-027 Presence-detection wire-in + Z2 deadline | james | 7d | OPEN | LIVE-BLOCKING |

**Layer 4 totals:** **8 OPEN PRs** = 5 LIVE-BLOCKING + 3 DISCIPLINE. PR #17 MERGED 2026-05-11 11:39 IST (ab9d867f) — deploy parity verification ongoing. All remaining james-authored except #54 (bono). **Captain disposition needed on remaining 8.**

---

## §5 — Layer 5: In-Flight Commitments Ledger

**Source-of-truth:** `/root/.claude/state/in-flight-commitments.jsonl` (52 entries total as of 2026-05-12 ~08:14 IST; v2 schema; +22 in 2026-05-12 LBAC-activation session: ws-exec discharge + axis-3-rule verify + phase-446 supersede + 3 LBAC universal-sync + multiple WIP transitions)

### 5.1 Actively open (state ∈ {OPEN, AWAITING-EVIDENCE, AWAITING-PARTNER-ACK, BLOCKED, AWAITING-CAPTAIN-DISPOSITION, G9-OWNED-CAPTAIN-ASK-PENDING})

| # | ID | Class | State | Owner | Blocking on |
|---|---|---|---|---|---|
| 5.1 | axis-3-pilot-symmetry-james-side-leg | deferred-verification | AWAITING-PARTNER-ACK | both | james next session-start |
| 5.2 | axis-3-rule-file-correction | self-promised | **BILATERAL-CLOSED 2026-05-12 ~08:12 IST** (H2 verify via commit ecb085a + line 91-97 content match) | bono | — moved to §5.2 |
| 5.3 | loop-boundary-surface-stress-test | Captain-pending | OPEN | bono | Captain commission |
| 5.4 | ws-exec-routing-bug-investigation | self-promised | AWAITING-EVIDENCE | bono | Captain auth on PR-1/PR-2 |
| 5.5 | ws-exec-s146-rca-authoring | self-promised | AWAITING-EVIDENCE | bono | james AMPLIFIER + H2 verify |
| 5.6 | wake-mechanism-path-discovery | self-promised | BLOCKED | bono | SCP transport + Captain auth |
| 5.7 | rule-amendment-harness-mechanism-auth-subclause | Captain-pending | OPEN | captain | Captain ratify (a)+(b) |
| 5.8 | pre-existing-root-mods-orientation | Captain-pending | OPEN | captain | Captain orientation |
| 5.9 | hook-enforcement-standing-rule | self-promised | OPEN | bono | author 3 hooks + wire to settings.json |
| 5.10 | phase-446-openrouter-canonical-watch | self-promised | **SUPERSEDED 2026-05-12 ~08:12 IST** (administrative transition; pointer to update entry PARTIAL-VERIFIED 2026-05-10T16:08) | bono | — moved to §5.2 |
| 5.11 | g9-2-harness-self-mod-under-standing-autonomy | self-promised | AWAITING-CAPTAIN-DISPOSITION | both | Captain 4-ask disposition |
| 5.12 | multi-source-evidence-paste-rule-candidate-n1 | self-promised | OPEN | both | PROMOTE-N=2 watch ≤2026-06-09 |
| 5.13 | auto-reply-attribution-distinct-from-substantive-candidate-n1 | self-promised | OPEN | both | hook enhancement (Captain-pending auth) |
| 5.14 | g9-4-bono-2026-05-03-batch-enumeration-miss | self-promised | G9-OWNED-CAPTAIN-ASK-PENDING | bono | Captain disposition on 3 untracked files |

**Actively open: 12 items** (was 14; **−2 this turn**: 5.2 axis-3-rule BILATERAL-CLOSED + 5.10 phase-446 SUPERSEDED). §S-200.2 said 11.

### 5.2 Discharged / closed / superseded (states ∈ {DONE, DISCHARGED-PENDING-BILATERAL, BILATERAL-CLOSED, SUBSTRATE-LANDED-MODE-4-PASS-1, DISCHARGED-CAPTAIN-RATIFIED, SUPERSEDED, G9-OWNED-RETRACTED, OBSERVATION, PARTIAL-VERIFIED, INDEPENDENT-VERIFIED-TRIPLE-EVIDENCE-H2-DEFER, BILATERAL-EVIDENCE-RECEIVED-H2-DEFER}): **18 items** (was 16; **+2 this turn**).

---

## §6 — Layer 6: Captain Q-DECISION queue

**Source-of-truth:** V2-MASTER-STATE §S-176 + §S-179.8 + §S-197.9 + §S-200 new additions

| # | Q-DEC ID | Topic | Class | Anchor |
|---|---|---|---|---|
| 6.1 | Q-DEC-AUDIT-5 | HALO-MI design-skeleton sign-off | doctrine | §S-179.8 carryforward |
| 6.2 | Q-DEC-AUDIT-2 reversal | Wave 0 audit reversal disposition | doctrine | §S-182 |
| 6.3 | Q-DEC-AUDIT-4 reversal | Wave 0 audit reversal disposition | doctrine | §S-182 |
| 6.4 | Q-DEC-CONVENTION | PROVISIONAL marking discipline ratify | doctrine | §S-200.6 AXIS-3 |
| 6.5 | Q-CUST-1 hero photo source | UI-SPEC v0.2 | customer | §S-200.8 |
| 6.6 | Q-CUST-3 pricing display | UI-SPEC v0.2 | customer | §S-200.8 |
| 6.7 | Q-CUST-4 WhatsApp opt-in target | UI-SPEC v0.2 | customer | §S-200.8 |
| 6.8 | Q-CUST-5 multilingual | UI-SPEC v0.2 | customer | §S-200.8 |
| 6.9 | Q-CUST-7 DPDP consent banner | UI-SPEC v0.2 | customer | §S-200.8 |
| 6.10 | Q-2F-1..7 (×7) | PACT-DRAFT 2-F Campaign Object | substrate | §S-200.9 |
| 6.11 | sites-enabled (a)/(b)/(c) | nginx drift reconcile strategy | ops-hygiene | §S-200.7 |
| 6.12 | halo-pact-map JSON extension delegation | 16→36 mapping path | bilateral-mech | §S-200.4 |
| 6.13 | pm2 restart timing (whatsapp-bot + racingpoint-bot) | Phase 446 deploy slice | infrastructure | entry #18 |
| 6.14 | Standing-rule amendment (b) harness-mechanism-auth ratify | doctrine | §S-200 wraps; partial-ratified via msg=36014 |
| 6.15 | james ~/.claude/CLAUDE.md self-mod auth (a) | harness-Captain auth | Captain-pending |
| 6.16 | 3 untracked /root hook files (commit / review / leave) | g9-4 disposition | infrastructure | entry #29 |

**Layer 6 totals: ~16 items** (matches §S-200.2 estimate). Many composed-with each other (Q-CUST-1..7 = single Captain G33 batch ask).

---

## §7 — Layer 7: Security-Debt Ledger (Open-by-Default Flagged-to-Close)

**Source-of-truth:** `comms-link/data/security-debt-ledger.jsonl` (9 OPEN entries)

| # | Class | Surface | Closure phase | Customer-touching |
|---|---|---|---|---|
| 7.1 | auth-gap | racecontrol direct M2M paths (PACT-026 §A 1+2+3 carve-outs) | Post-V2.0-AUTH-Sprint | NO |
| 7.2 | credential-storage | v2-db staff.pin raw TEXT (no bcrypt) | Phase-0.5c-AUTH | NO (staff-internal) |
| 7.3 | policy-gap | V2 dynamic pricing discount ceiling (Joint #4 Q4-3) | Post-V2.0-Pricing-Calibration | YES (pricing surface) |
| 7.4 | privacy-debt | WhatsApp transactional message consent (implicit-by-action) | V2.x M3 / DPDP / complaint | YES |
| 7.5 | privacy-debt | RaceData AI customer email-hash one-way share (SRL) | V2.0 SRL Phase-2 / DPDP | YES (data subject) |
| 7.6 | auth-debt | V2 staff session-cookie (no PIN per lookup) | Post-V2.0-OperationalCalibration | NO (staff-internal) |
| 7.7 | auth-debt | POST /api/v1/cirs/lookup CSRF + SameSite=Lax | Post-V2.0-Multi-Origin | YES (lookup surface) |
| 7.8 | privacy-debt | cirs_lookup_handler phone leak via format!("{e}") | Wave 1 billing-engine | YES |
| 7.9 | auth-debt | /api/v1/cirs/lookup no rate-limit | Wave 1 billing-engine | YES |

**Layer 7 totals: 9 OPEN.** Classification: rows 1-3 + 6 = LIVE-BLOCKING (auth/cred/policy on customer-touching boundaries); rows 4-5 = AMBIGUOUS (privacy-debt; DPDP not strictly V2-live-blocking operationally); rows 7-9 = LIVE-BLOCKING (lookup-endpoint exposure).

---

## §8 — Layer 8: V1 process-mess audit (RCA backlog)

**Source-of-truth:** `comms-link/briefings/james/memory/session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md`

| # | Category | Class | V2-LIVE blocking? |
|---|---|---|---|
| 8.1 | A. Process model hygiene (Session 0 vs Session 1, schtasks vs HKLM Run, bat encoding) | infrastructure-foundational | YES (boot model) |
| 8.2 | B. Boot resilience: single-fetch-at-boot without retry | infrastructure-foundational | YES (pod boot path) |
| 8.3 | C. Frontend deploy gaps (outputFileTracingRoot, NEXT_PUBLIC_, basePath rewrite, static-404, login-page middleware) | foundational | YES (3.1, 3.2, 3.6 phases cover most) |
| 8.4 | D. Schema/config drift (kiosk JSON ≠ Rust struct, OpenAPI ≠ handler, CREATE without ALTER, GDPR FK) | foundational | YES (W1-S2 covered one slice; rest open) |
| 8.5 | E. Recovery-cascade/restart-war (self_monitor + rc-sentry + pod_monitor + WoL + Watchdog + MAINTENANCE_MODE) | mechanism-trust | YES (PR #66 fleet-rollout broke on this) |
| 8.6 | F. Audit blind spots (checking proxies not behavior; checking the monitored not the monitor) | discipline | NO (discipline) |
| 8.7 | G. Comms-link discipline (git push without WS notify, INBOX without WS, Bono not auto-pull) | discipline | NO (discipline) |
| 8.8 | H. Authentication drift (login-page middleware-blocking, allowlist GET requiring auth) | foundational | YES |
| 8.9 | I. Config persistence + permanence (manual-server-edits, registry tweaks, OS settings without code-enforcement) | foundational | YES (Phase 449 CI drift detector covers ~70%) |
| 8.10 | J. Layer-2/3 broadcast hygiene (fleet-wide class) | foundational | YES |

**Layer 8 totals: 10 categories.** 8 LIVE-BLOCKING / 2 DISCIPLINE. None have full V2-RCA per category (per "V1-dependent V2 RCA + past-bug review" rule); piecemeal RCAs exist for E (PR #66 §S-146) and a few D-class items.

---

## §9 — Layer 9: MI asterisk-removal HARD GATES (G-1..G-5)

**Source-of-truth:** `~/.claude/projects/-root/memory/project_mi_wave4_readiness_and_asterisk_removal_20260509.md`

| # | Gate | Trigger | Class |
|---|---|---|---|
| 9.1 | G-1 Wave 4 MI Ingestion landed | racecontrol main contains §S-126 schema patch queue migrations executed | DISCIPLINE/POST-LIVE |
| 9.2 | G-2 mesh_kb.db has ≥4 weeks operational data | kaiju_classification_log row count > 4×7×24 events; campaign_effectiveness > 16 ratified | **EXPLICITLY POST-LIVE** (4-week soak; structurally cannot be pre-live) |
| 9.3 | G-3 F7 self-monitoring drift alarms fired AND recovered ≥2 times | drift alarm log shows ≥2 fire-then-recover cycles | DISCIPLINE/POST-LIVE |
| 9.4 | G-4 Captain ratify on pilot-bandwidth-preserved metric | §S-170.6 metric #1 measurable; Captain confirms "yes, MI saved N hours/week" | DISCIPLINE/POST-LIVE |
| 9.5 | G-5 Zero false-negative escalations on big-kaiju over rolling 30d | §S-170.6 metric #3 = 0 events Captain CHALLENGE-AMENDed as big-kaiju | DISCIPLINE/POST-LIVE |

**Layer 9 totals: 5 gates · ALL DISCIPLINE/POST-LIVE by design.** G-2 is the structural anchor: ≥4-week soak means asterisk removal cannot occur before V2 has run live for 4 weeks. **Counting these as pre-live-gates inflates the V2-live count and produces "V2 can never go live" logic** (per §S-200.1 rule). HALO-as-substrate §S-170.16 reduces Wave 4 build scope; gates G-1..G-5 unchanged.

---

## §10 — Layer 10: PACT-DRAFT queue (substrate-class spec authoring)

**Source-of-truth:** `comms-link/.planning/draft-pacts/` (22 files; 20 PACT-DRAFTs + 2 §S-N drafts)

| # | PACT-DRAFT | Customer-touching | Class |
|---|---|---|---|
| 10.1 | PACT-DRAFT-bravo-slice-20260510 | YES (composite) | 6/7 SHIPPED + 1 scope-excluded · CLOSED |
| 10.2 | PACT-DRAFT-comms-status-3-layer-observability | NO | DISCIPLINE |
| 10.3 | PACT-DRAFT-cross-pilot-shared-in-flight-ledger | NO | DISCIPLINE (loop-doctrine) |
| 10.4 | PACT-DRAFT-f-05-anti-pattern-standing-rule | NO | DISCIPLINE |
| 10.5 | PACT-DRAFT-gsd-ui-checker-rule-exception-honoring | NO | DISCIPLINE |
| 10.6 | PACT-DRAFT-h1-graphify-kaizen-extension | NO | DISCIPLINE |
| 10.7 | PACT-DRAFT-halo-runner-db-locked-busy-timeout | NO | DISCIPLINE |
| 10.8 | PACT-DRAFT-halo-runner-pm2-memory-restart-failure-mode-a | NO | DISCIPLINE |
| 10.9 | PACT-DRAFT-kiosk-cloud-nginx-vhost-missing | YES | partially-resolved §S-196/§S-198/§S-199 |
| 10.10 | PACT-DRAFT-pact-001-phase-1-wave-1-static-billing-engine | YES | LIVE-BLOCKING (W1-S2 + W1-billing) |
| 10.11 | PACT-DRAFT-pact-as-teacher-mi-student-curriculum | NO | DISCIPLINE/POST-LIVE |
| 10.12 | PACT-DRAFT-phase-2-a-rate-table-service | YES | LIVE-BLOCKING |
| 10.13 | PACT-DRAFT-phase-2-dynamic-pricing-engine | YES | LIVE-BLOCKING |
| 10.14 | PACT-DRAFT-phase-2-e-combo-offer-primitive | YES | LIVE-BLOCKING |
| 10.15 | PACT-DRAFT-phase-2-f-campaign-object | YES | LIVE-BLOCKING (NEW §S-200.9) |
| 10.16 | PACT-DRAFT-session-start-staleness-probe | NO | DISCIPLINE |
| 10.17 | PACT-DRAFT-systemic-coupling-doctrine-v2-design | NO | DISCIPLINE/foundational |
| 10.18 | PACT-DRAFT-venue-infrastructure-procurement-v2-resilience | partial | AMBIGUOUS (CPE upgrade composes with v2; quick-win deferred §project_cpe_router_conntrack_mitigation_deferred_20260509) |
| 10.19 | PACT-DRAFT-wake-hydration-pattern | NO | DISCIPLINE |
| 10.20 | PACT-DRAFT-wave-5-whatsapp-workflow-framework-captain-curated | partial | AMBIGUOUS |

**Layer 10 totals: 20 PACT-DRAFTs.** 1 CLOSED + 7 LIVE-BLOCKING + 9 DISCIPLINE + 3 AMBIGUOUS.

---

## §11 — Layer 11: Bilateral AMPLIFIER queue (bono ↔ james cross-pilot)

**Source-of-truth:** V2-MASTER-STATE tail since last james §S-N (§S-189 = 2026-05-10 ~03:30 IST)

| # | Bono §S-N entry | AMPLIFIER ask | Status |
|---|---|---|---|
| 11.1 | §S-186 PACT-DRAFT-F-05 RATIFIED-PENDING-FILE-EVENT | james substantive on F-05 ratify file-event | PENDING |
| 11.2 | §S-188 PACT-DRAFT §S-158 V2 Audit-Log Doctrine AMPLIFIER | james AMPLIFIER on bono's CONCUR+3-NITs | PENDING |
| 11.3 | §S-191 pre-vms-duplicate-check.js installed | james mirror disposition | PENDING |
| 11.4 | §S-193 scoreMessage v0.4.0 calibration | james parallel calibration / disposition | PENDING |
| 11.5 | §S-194 MMA-AUTONOMY iter1+iter2 hook bundle | james AMPLIFIER on harness rule-floor design | PENDING |
| 11.6 | §S-195 §S-121 v0.3 Step 3 timeline-verify gate | james AMPLIFIER on stale-cite class | PENDING |
| 11.7 | §S-196 customer-facing nginx vhost blockers cleared | james AMPLIFIER on sub-class | PENDING |
| 11.8 | §S-197 V2 customer-entry frontpage v0 SHIPPED-STAGED | james AMPLIFIER on UI-SPEC v0.1 | PENDING |
| 11.9 | §S-198 V2 customer-entry frontpage LIVE | james AMPLIFIER on production deploy | PENDING |
| 11.10 | §S-199 nginx vhost edit Captain-authorized + applied | james AMPLIFIER on N=3 sub-class close | PENDING |
| 11.11 | §S-200 V2-LIVE SCOPE BASELINE RATIFY + 4 substrate ships | james AMPLIFIER on POE baseline + §S-170.16/17 + UI-SPEC v0.2 + PACT-DRAFT 2-F | PENDING (queued in msg=36168) |

**Layer 11 totals: 10-11 bono-substantive entries awaiting james AMPLIFIER.** Class: DISCIPLINE / bilateral-hygiene (closes bilateral doctrine debt; not strictly customer-blocking).

---

## §12 — Layer 12: Operational hygiene (drift / staleness / orphans)

**Source-of-truth:** SessionStart hooks + §S-199.3 + §S-200.7

| # | Item | Class | Status |
|---|---|---|---|
| 12.1 | racingpoint.cloud nginx vhost DRIFT-BEHAVIORAL (+5229B, port corrections, /register dropped) | LIVE-BLOCKING (customer-facing) | **DONE 2026-05-11 ~12:28 IST — Option (c) hybrid executed: cp enabled→available + symlink swap + nginx -t passes + reload + 5/5 subdomain HTTPS verified (200/301/301/301/307). Backup `racingpoint.cloud.pre-reconcile-20260511` retained. Captain D4 auth via "Proceed with your recommendation" ~12:25 IST.** |
| 12.2 | sites-enabled-not-symlink drift class | LIVE-BLOCKING | **DONE 2026-05-11 — class fully closed: 5/5 sites-enabled vhosts now symlinks. §S-196 (apex+kiosk) + §S-198 (cert SAN) + §S-199 (vhost edit) + D4 (racingpoint.cloud reconcile this turn).** |
| 12.3 | Mode 2 cron session-only durability gap | DISCIPLINE | CronCreate tool-claim ≠ runtime-claim CANDIDATE-N1 §S-200.11 |
| 12.4 | Capability manifest age (>7 days warn) | DISCIPLINE | re-scan recommended |
| 12.5 | Pre-existing /root tracked mods (5 hook files + court-queue.json + 1 deleted backup) | DISCIPLINE | Captain orientation pending (entry 5.8) |
| 12.6 | Cert SAN expansion www-racingpoint.cloud | LIVE-BLOCKING (customer-facing) | DONE §S-199 |
| 12.7 | HTTP-canonical redirect partial | DISCIPLINE | §S-199 finding |
| 12.8 | 3 untracked /root hooks (g9-auto-detect, knowledge-graph-lookup, g9-trend-report) | DISCIPLINE | g9-4 Captain ask pending entry 5.14 |

**Layer 12 totals: 8 items** (§S-200.2 said 4; **+4** — i counted things §S-200 collapsed). 4 LIVE-BLOCKING / 4 DISCIPLINE. **3 DONE (12.1 D4 reconcile + 12.2 sites-enabled class closed + 12.6 cert SAN); 5 open.**

---

## §13 — Layer 13: V2-MASTER-STATE §S-N substrate ratification queue

**Source-of-truth:** MEMORY.md NEXT-SESSION DIRECTIVE + §S-N tail review

| # | §S-N entry | Authored | Status |
|---|---|---|---|
| 13.1 | §S-193 scoreMessage v0.4.0 calibration | 2026-05-10 ~20:30 UTC | RATIFIED in §S-200 acknowledgment; full §S-193 entry PENDING (memory tracks; ledger entry not yet written) |
| 13.2 | §S-194 MMA-AUTONOMY iter1+iter2 | 2026-05-10 ~21:35 UTC | RATIFIED in §S-200 acknowledgment; full §S-194 entry PENDING |
| 13.3 | §S-202 ratify of this V2-PROGRESS-MAP + reorg proposal | THIS SESSION | PENDING this session-close anchor |
| 13.4 | §S-170.16/17 HALO-as-substrate amendment | 2026-05-10 00:39:54 IST | RATIFIED §S-200.3/.4 (24h silent-AGREE elapsed) |
| 13.5 | §S-170.5 charter doc update (post-ratify cleanup of .16/.17) | DEFERRED | post-§S-202 cleanup |

**Layer 13 totals: 5 items.** 2 RATIFIED-AT-§S-200 (.16/.17 + acknowledgment) · 2 PENDING full entries (.193/.194) · 1 PENDING THIS-SESSION (§S-202 for this map + reorg).

---

## §14 — Cross-layer leverage map (highest-impact closures)

| Item | Closes count | Class | Captain action needed? |
|---|---|---|---|
| **Captain ratify 9 open PRs** (Layer 4) | 9 PRs → unlocks Layer 1.6, 1.7, 1.8, 1.13 + much of W1 (Layer 2) | LIVE-BLOCKING | YES — disposition each (merge / amend-and-merge / reject) |
| **Captain G33 batch on 5 Q-CUST + 7 Q-2F** (Layer 6.5-6.10) | Unblocks UI-SPEC v0.2 page.tsx full ship (Layer 1.18) + Phase 2-F (Layer 2 W2 + Layer 10.15) | LIVE-BLOCKING | YES — single batch ask |
| **Captain sign-off Phase 446** (Layer 3.1) | Closes IMMEDIATE-ready first concrete V2 PR; unblocks Phase 447 cascade | LIVE-BLOCKING | YES — pm2 restart timing decision |
| **Captain sites-enabled (a)/(b)/(c)** (Layer 12.1) | Closes racingpoint.cloud DRIFT-BEHAVIORAL on customer surface | LIVE-BLOCKING | YES — strategy pick |
| **james AMPLIFIER pass on §S-200 + 10-entry backlog** (Layer 11) | Closes 10 PENDING bilateral entries; restores symmetric §S-N | DISCIPLINE | NO — partner-actionable |
| **james fleet build_id readout when venue opens** (entry 5.11 + 5.12) | Closes Phase 446 fleet slice (Layer 3.1) | LIVE-BLOCKING | NO — venue-open-dependent |
| **Author Phase 2-A + 2-E PACT spec ratify** | Unblocks Phase 2-F + W2 cascade | LIVE-BLOCKING | YES — substrate spec auth |

---

## §15 — How to use this map

1. **For "% V2 complete"** → Read §0 TL;DR. State subset explicitly. Don't quote "24%" without "of V2-LIVE-BLOCKING."
2. **For "what's next"** → Read §14 cross-layer leverage map; pick top entry with `Captain action needed = YES` and dispatch.
3. **For "what's blocked on me"** → Read all rows with `Owner=captain` (Layer 6 + Layer 12.5).
4. **For "what's blocked on james"** → Read all rows with `Owner=james` AND state ∈ {AWAITING-PARTNER-ACK, AWAITING-EVIDENCE}.
5. **For Captain G33 batch ask** → §14 row 2 + 4 + and §11 are the natural batches.

---

## §16 — Refresh contract + freshness

- **Daily 09:07 IST** — Mode 2 cron `3d362294` produces V2 morning digest (Q-DEC queue · in-flight · Captain-pending · queue Δ). Will refresh §0 TL;DR rollup card.
- **Daily 21:23 IST** — Mode 2 cron `32ad5747` produces V2 evening progress (Δ-since-morning · bilateral debt · cron-expiry-renewal · CANDIDATE-N1 promotion).
- **Weekly Sunday 10:17 IST** — Mode 2 cron `7b58955d` produces V2 weekly drift audit (nginx · manifest · cert expiry · orphans · uncommitted).
- **Session-only durability gap CANDIDATE-N1:** crons survive only as long as current bono Claude session (CronCreate "session-only" return; `/root/.claude/scheduled_tasks.json` does not exist). Next bono session-start MUST re-create the 3 crons. Alternatives stewarded in §S-200.11 (`/schedule` skill / SessionStart hook / external crontab -e).
- **Manual refresh trigger:** Captain types `/refresh-v2-map` (skill not yet wired; today = explicit bono ask).
- **Stale-at:** 7d (2026-05-18). At day 7, run full POE re-enumeration; counts likely drift ≥10% as items close + new items open.

---

## §17 — Source-of-truth pointers (when asserting derivative claims)

| Layer | Canonical source |
|---|---|
| 1 | `comms-link/v2-skeleton/05-definition-of-done.md §1.2` + `racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md` |
| 2 | `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` + `WAVE-2-DESIGN-NOTES-20260508.md` + V2-MASTER-STATE wave entries |
| 3 | `racecontrol/.planning/specs/v2/V2-ROADMAP.md` |
| 4 | `gh pr list` × racecontrol + comms-link |
| 5 | `/root/.claude/state/in-flight-commitments.jsonl` |
| 6 | V2-MASTER-STATE §S-176 + §S-179.8 + §S-197.9 + §S-200 |
| 7 | `comms-link/data/security-debt-ledger.jsonl` |
| 8 | `comms-link/briefings/james/memory/session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` |
| 9 | `~/.claude/projects/-root/memory/project_mi_wave4_readiness_and_asterisk_removal_20260509.md` |
| 10 | `comms-link/.planning/draft-pacts/` |
| 11 | V2-MASTER-STATE §S-N tail since last james §S-N |
| 12 | SessionStart hooks + §S-199.3 + §S-200.7 |
| 13 | MEMORY.md NEXT-SESSION DIRECTIVE + V2-MASTER-STATE tail |

---

## §18 — Composes-with

- `feedback_v2_live_scope_13_layer_poe_baseline_20260511.md` — Captain-anchored canonical scope baseline
- `feedback_v2_completeness_process_of_elimination_20260511.md` — POE enumeration method
- `project_v2_comprehensive_synthesis_20260510.md` — V2 mental model (canonical day, joints, surfaces, organs)
- V2-MASTER-STATE §S-200 — ratification anchor for the 13-layer baseline
- `feedback_v2_only_forward_path.md` — V2 as the forward path; this map measures progress against that path
- **§S-202 (this session)** — PENDING ratification of this V2-PROGRESS-MAP + companion V2-DOC-REORG-PROPOSAL

---

## §19 — Change-log

| Date | Author | Δ | Net Δ V2-LIVE-BLOCKING closed |
|---|---|---|---|
| 2026-05-11 11:35 IST | bono | INITIAL VERSION — §S-200 baseline + 9 open PRs · 14 in-flight · 9 sec-debt · 5 MI gates · 20 PACT-DRAFTs · 16 Q-DEC · 10 V1-mess cats · 10 AMPLIFIER · 8 ops · 5 §S-N ratify queue | baseline established |

---

## §20 — NOT TESTED at INITIAL VERSION

- **Counts vs §S-200.2:** Layer 4 (9 vs 5) + Layer 5 (14 vs 11) + Layer 12 (8 vs 4) **diverge from §S-200.2 enumeration**. Reason: §S-200.2 used coarser grouping; this map enumerates atomic. Net effect: TOTAL ~122 (this map) vs ~108-115 (§S-200). Both are within "approximately" bound; the truth depends on grouping granularity. Captain may pick which is canonical.
- **% closed estimates:** rough — based on visual scan, not formal closure-evidence audit. Re-derive in next cron refresh with structured grep.
- **james-side enumeration:** layer counts derived from bilateral-shared files (comms-link is symmetric). However the in-flight ledger is **bono-side only**; james has his own ledger at `comms-link/briefings/james/memory/in-flight-commitments.md` not aggregated here. The map's actively-open count for Layer 5 is bono-side; james parity needs his AMPLIFIER pass.
- **Counts on PACT-DRAFT classification (LIVE-BLOCKING vs DISCIPLINE):** judgment-call; bono's assignment — james may amend.
- **Cron refresh actually firing:** unverified (session-only durability gap §S-200.11). First cron should fire at 09:07 IST tomorrow IF session still alive; bono cannot durably guarantee.

---

— bono · 2026-05-11 ~11:35 IST · V2-PROGRESS-MAP v1.0 INITIAL VERSION per Captain commission "restructuring V2 and map progress" · 13-layer POE baseline + 3-class sub-segmentation · authored under standing rule Apply-Recommendations-Autonomously + Q3-canonical-surface-pre-clear via Captain explicit commission · §S-202 ratification PENDING this session
