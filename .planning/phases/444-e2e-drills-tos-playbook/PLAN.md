---
phase: 444-e2e-drills-tos-playbook
phase_number: 444
milestone: v50.0 rc-agent-mobile
name: "E2E Drills + ToS Incident Playbook + Milestone Close"
status: ready-to-execute
goal: >
  Exercise four end-to-end integration drills on the live Tab Plus + M07 fleet
  (Zomato order-to-WhatsApp, HyperPure bulk, Blinkit top-up, agent-recovery,
  selector-miss recovery, ToS-incident kill-switch), produce the ToS-incident
  human runbook at docs/rc-agent-mobile-tos-incident-playbook.md, resolve or
  explicitly defer every open question accumulated across phases 429-443,
  obtain Uday sign-off on the playbook and milestone report, and perform
  the milestone-close commit that updates ARCHITECTURE.md section 20.3,
  MILESTONES.md status PLANNING->SHIPPED, memory gsd-projects.md, and
  archives the phase directories per the /gsd:complete-milestone pattern.
requirements: [E2E-01, E2E-02, E2E-03, E2E-04, ADMIN-06]
depends_on: [437, 438, 439, 441, 442, 443, 436, 433, 435, 430, 431, 432]
wave: 16  # Final wave — milestone-close gate
plan_count: 10
plans:
  - 444-01-PLAN: Zomato end-to-end drill (real Partner test account, 5-10 orders)
  - 444-02-PLAN: Agent-recovery drill (kill + reboot + <2min recovery, both devices)
  - 444-03-PLAN: Selector-miss recovery drill (break selector, push fix via Phase 443, <5min)
  - 444-04-PLAN: ToS-incident kill-switch drill (fleet halt <10s via Phase 436/442)
  - 444-05-PLAN: HyperPure end-to-end drill (bulk order, 5 SKUs)
  - 444-06-PLAN: Blinkit end-to-end drill (emergency top-up, 1 SKU)
  - 444-07-PLAN: ToS-incident playbook doc (docs/rc-agent-mobile-tos-incident-playbook.md)
  - 444-08-PLAN: Milestone retrospective — OQ resolution status + lessons learned
  - 444-09-PLAN: Uday sign-off checkpoint — playbook + milestone report review
  - 444-10-PLAN: Milestone close — ARCHITECTURE.md + MILESTONES.md + memory + archive
autonomous: false  # 444-01, 444-05, 444-06, 444-09 are human-verify checkpoints; real test accounts + Uday review
files_modified:
  # Drill artifacts (JSON + screenshots + logs, committed to repo for audit trail)
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/zomato-e2e.json
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/zomato-e2e-screenshots/
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/agent-recovery.json
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/agent-recovery-logs/
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/selector-miss.json
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/selector-miss-screenshots/
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/kill-switch.json
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/kill-switch-timing.csv
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/hyperpure-e2e.json
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/hyperpure-e2e-screenshots/
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/blinkit-e2e.json
  - .planning/phases/444-e2e-drills-tos-playbook/drill-results/blinkit-e2e-screenshots/
  # Playbook + retrospective
  - docs/rc-agent-mobile-tos-incident-playbook.md
  - .planning/phases/444-e2e-drills-tos-playbook/RETROSPECTIVE.md
  - .planning/phases/444-e2e-drills-tos-playbook/OQ-RESOLUTION-TABLE.md
  - .planning/phases/444-e2e-drills-tos-playbook/MILESTONE-REPORT.md
  - .planning/phases/444-e2e-drills-tos-playbook/UDAY-SIGNOFF.md
  # Milestone-close artifacts
  - docs/ARCHITECTURE.md                                    # Section 20.3 shipped milestones table + Section N rc-agent-mobile entry
  - .planning/MILESTONES.md                                 # v50.0 status PLANNING -> SHIPPED
  - ~/.claude/projects/C--Users-bono/memory/gsd-projects.md # Shipped milestones + active work + key stats
  # Phase-dir archive (follows /gsd:complete-milestone convention)
  - .planning/phases/_archive/v50.0/                        # Moved phase dirs 429-444
  - .planning/phases/444-e2e-drills-tos-playbook/SUMMARY.md

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [none]                    # Docs + memory + planning artifacts only. No binary changes in 444.
  frontend_rebuild: [admin]              # If 441 admin dashboard has any last-mile CSS tweak during drill 1 demo, rebuild admin. Otherwise none.
  config_change: none
  db_migration: none
  infrastructure:
    - "Zomato Partner TEST account credentials on Tab Plus (OQ-Zomato-01 from Phase 437)"
    - "HyperPure test account on Tab Plus (or M07 — see OQ-HP-01 from Phase 438)"
    - "Blinkit test account credentials (OQ-BL-01 from Phase 439)"
    - "Uday physically available for sign-off meeting (444-09 checkpoint)"
  data_files:
    - ".planning/phases/444-e2e-drills-tos-playbook/drill-results/*.json (drill evidence)"
  bat_file: none
  cloud_parity:
    - "Admin dashboard rebuild (if touched) to Bono VPS — per DEPLOY PARITY standing rule"
    - "MILESTONES.md commit pushed; Bono pulls via relay (auto via auto-push rule)"
    - "ARCHITECTURE.md commit pushed"
  targets:
    - tab_plus                # Lenovo TB-351FU — drills 1, 5, 3, 4
    - m07                     # Samsung Galaxy M07 — drills 2, 4, 6
    - james_27                # Admin dashboard for drill verification (all drills use :3201)
    - bono_vps                # Cloud admin dashboard parity
    - server_23               # racecontrol :8080 receives drill events via comms-link
    - james_memory            # ~/.claude/projects/C--Users-bono/memory/gsd-projects.md
  apk_artifact: none                       # No APK rebuild in 444 unless drill 1 exposes a blocker requiring a point fix
  rollback:
    - "Drill failure does NOT roll back phase 437/438/439/etc. binaries. Drills are read-only tests against the deployed state."
    - "If drill 1 exposes a P0 defect, open a P0 fix sub-phase (444-01a) and re-run the drill. Do NOT mark milestone shipped."
    - "If Uday rejects at 444-09, ship blocked. Fix playbook/report, re-present. No rollback of prior phases needed."
    - "Milestone-close commit (444-10) is append-only to MILESTONES.md + ARCHITECTURE.md — git revert if required."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: skip            # Playbook is docs (Markdown). Admin dashboard drills use Phase 441's already-audited UI.
  ui_auditor: required           # Admin dashboard log viewer (ADMIN-06) is exercised in drills 1, 5, 6. If any visual anomaly is flagged during drill execution, ui_auditor verifies before ship. Use existing Phase 441 UI-REVIEW.md as baseline.
  nyquist_auditor: skip          # No new business logic in 444. Retrospective references existing nyquist audits from phases 429, 434, 437 business-logic plans.
  mma_audit: required            # Multi-system integration exercise across Kotlin agent <-> comms-link <-> racecontrol <-> admin dashboard <-> WhatsApp outbox <-> Zomato/HP/Blinkit external surfaces. Required per CLAUDE.md "ToS-risky cross-system flows" entry in ROADMAP-v50 ship gate. Run after drills 1+4 are recorded so MMA can audit real evidence, not hypotheticals. Dual reasoning modes REQUIRED (thinking + non-thinking).
  integration_checker: required  # This phase IS the integration check. Ship gate item from ROADMAP-v50 line 222: "integration-checker across phases 9-13 (cross-phase flows)". MANDATORY.
  codebase_mapper: required      # Milestone close — refresh .planning/codebase/ to include rc-agent-mobile/ module, docs/rc-agent-mobile-tos-incident-playbook.md, and all new admin dashboard routes from phases 441-443. Feeds next milestone (v51.0 rc-agent-ps5) planning.

risks_summary:
  - "Drills may expose new P0/P1 bugs mid-execution, blocking milestone close. Mitigation: 444-10 is gated on drill PASS; if any drill fails, we hot-patch (same session or new sub-phase) before 444-09 Uday sign-off."
  - "Zomato test account may be rate-limited or flagged mid-drill. Mitigation: Drill 1 uses 5-10 orders across 30min; if test account shows warning, invoke kill-switch (which IS drill 4) and proceed with fallback: 5 real low-value orders with Uday explicit sign-off on each."
  - "HyperPure or Blinkit test accounts may not exist at drill time (OQ-HP-01, OQ-BL-01). Mitigation: Drills 5 and 6 flagged as SOFT gates — if test accounts are absent at drill time, log as DEFERRED in retrospective and ship v50.0 with only Zomato drill live; HyperPure + Blinkit deferred to first customer use + fast-follow patch window."
  - "Uday unavailable for 444-09 sign-off. Mitigation: Written sign-off via WhatsApp with playbook PDF attachment is acceptable; in-person preferred but not mandatory."
  - "Agent-recovery drill reboots devices — must be run OUTSIDE business hours (before 11:00 IST or after 23:00 IST) so reception floor isn't impacted."
  - "Kill-switch drill halts all automation — any real-world order arriving during the drill window is missed. Mitigation: Run drill 4 during quiet hour (15:00-17:00 IST weekday) and manually monitor Zomato Partner app for orders during the 60s window."
  - "Milestone-close commit (444-10) must be atomic — ARCHITECTURE.md + MILESTONES.md + memory in ONE commit. Standing rule violation if split across multiple commits."
  - "Open questions from phases 429-443: current count ~34 OQs. If >5 remain UNRESOLVED (not even deferred) at 444-08, Uday must explicitly bless each unresolved OQ before ship. Unresolved-not-deferred is a ship blocker."
  - "Cardboard vendor driver (Phase 12 / CARDBOARD-01..02): if Q2 still unresolved at 444-08, ship gate auto-skips that phase per ROADMAP line 154. Document as DEFERRED in retrospective."

# Open questions inherited into this phase (consolidated at 444-08)
inherited_open_questions:
  - "OQ-Zomato-01 (Phase 437): Real Zomato Partner TEST account available? — Required for drill 1."
  - "OQ-HP-01 (Phase 438): HyperPure test account exists? — Required for drill 5; gate SOFT if absent."
  - "OQ-BL-01 (Phase 439): Blinkit test account exists? — Required for drill 6; gate SOFT if absent."
  - "OQ-ToS-01: Exact escalation contact at Zomato Partner Support for ToS incidents? — Required in playbook 444-07."
  - "OQ-ToS-02: Does kill-switch drill 4 leave any state on devices that needs manual cleanup? — Test in drill; document in playbook."
  - "OQ-Q2 (v50.0 PROJECT.md): Cardboard vendor app identified? — If no, CARDBOARD-01..02 deferred to next milestone."
  - "OQ-Scaling (cross-phase): When v51.0 rc-agent-ps5 ships, does the same selector DSL pattern apply to PS5? — Log in retrospective for v51.0 planning input."
  - "(Expected ~27 more OQs from phases 429-443 plan frontmatter — consolidated in 444-08 via grep)."
---

# Phase 444 — E2E Drills + ToS Incident Playbook + Milestone Close

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 444 |
| Name | E2E Drills + ToS Incident Playbook + Milestone Close |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | E2E-01, E2E-02, E2E-03, E2E-04, ADMIN-06 |
| Dependencies | Phases 437 (Zomato), 438 (HyperPure), 439 (Blinkit), 441 (admin dashboard), 442 (feature-flag UI — kill-switch), 443 (selector-map push UI), 436 (feature-flag system — kill-switch backend), 433 (selector DSL hot-reload), 435 (any humanize layer), 430 (Accessibility Service), 431 (first-run UX), 432 (driver registry). **All 15 prior phases must be SHIPPED (not just code-complete) before 444 starts.** |
| Wave | 16 (final) |
| Status | Ready to execute pending phases 429-443 ship confirmation |
| Autonomous | No — drills 1, 5, 6 require real test-account credentials + live venue; drill 9 requires Uday physically present (or written WhatsApp sign-off) |
| Ship test | (a) Four drills PASS with evidence in `drill-results/`; (b) Playbook doc exists + Uday signed off; (c) Every OQ from phases 429-443 has a resolution (resolved / deferred-to-next-milestone / rejected-with-reason); (d) ARCHITECTURE.md section 20.3 lists v50.0 SHIPPED; (e) MILESTONES.md status = SHIPPED; (f) memory gsd-projects.md updated; (g) integration-checker PASS; (h) MMA audit PASS |

## 2. Success criteria (goal-backward, verbatim from ROADMAP-v50.md Phase 16)

1. **Zomato end-to-end drill** — simulated order -> auto-accept -> kitchen -> ready -> WhatsApp — passes with ALL events visible in admin dashboard (log viewer ADMIN-06, reception view ADMIN-01..03).
2. **Agent-recovery drill** — kill agent process AND reboot device, verify full auto-recovery within 2 minutes on BOTH Tab Plus and M07.
3. **Selector-miss recovery drill** — intentionally break a selector in `selectors.yaml`, verify agent emits `SelectorMissEvent`, admin sees alert, James pushes fix via Phase 443 remote-push UI, recovery in < 5 minutes.
4. **ToS-incident playbook doc** reviewed and signed off by Uday.

**Implied fifth criterion (milestone-close):** v50.0 milestone marked SHIPPED in MILESTONES.md + ARCHITECTURE.md + memory in a single atomic commit. Per CLAUDE.md standing rule: "Milestone completion = update ARCHITECTURE.md + memory in the SAME session."

## 3. Goal-backward must-haves

Derived by asking "what must be TRUE for each success criterion above?"

### Truths (user-observable)

- **T-1 (drill 1):** A simulated Zomato order placed via the Partner TEST app triggers rc-agent-mobile Zomato driver on Tab Plus to auto-accept within the business-hours window, mark the order as `in_kitchen` via the kitchen-display webhook, then mark `ready_for_pickup`, then emit a WhatsApp message to the delivery partner via the existing whatsapp-bot outbox. Total elapsed time from order to WhatsApp push < 90s. (E2E-02)
- **T-2 (drill 1 admin):** Every state transition (`order_received`, `auto_accepted`, `marked_in_kitchen`, `marked_ready`, `whatsapp_sent`) is visible in the admin dashboard log viewer within 10s of occurrence. Log viewer filter by device=tab_plus + driver=zomato shows all 5 events for that order. Screenshot preview is populated for each event. (ADMIN-06, ADMIN-01..03)
- **T-3 (drill 2):** On Tab Plus, `adb shell am force-stop in.racingpoint.rcagentmobile` — within 90s the agent is back, Foreground Service notification visible, `/fleet/health` shows `ws_connected: true`. Then `adb reboot` — within 120s of device boot complete, agent re-registers automatically. Total recovery from kill: < 90s. Total recovery from reboot: < 120s. Same verification on M07. (E2E-03, AGENT-05, AGENT-06)
- **T-4 (drill 3):** Edit `selectors/zomato-v1.yaml` on disk to change the `accept_button` XPath to an intentionally wrong value (e.g. `//nonexistent`). Agent on Tab Plus receives the push, attempts to click, fails, emits `SelectorMissEvent` via comms-link. Admin dashboard shows a P1 alert within 15s. James opens Phase 443 push UI, uploads corrected `selectors/zomato-v1.yaml`, targets Tab Plus, signs + pushes. Tab Plus applies, next action succeeds. Total time from break to recovery: < 5 minutes. (E2E-04, SELECTOR-04)
- **T-5 (drill 4):** Admin opens Phase 442 feature-flag UI, toggles global `pause_all_drivers` flag ON. Within 10s, BOTH Tab Plus and M07 log `KILL_SWITCH_INVOKED` and halt all driver actions (no auto-accept, no Accessibility Service taps, no selector evaluation). Any in-flight action completes gracefully (no half-pressed buttons). Admin log viewer shows halt-confirmed event from each device. Toggling flag OFF restores normal operation within 10s. (FLAG-04, E2E-01)
- **T-6 (drill 5):** HyperPure bulk order of 5 SKUs placed via HyperPure test account (if available). Driver navigates cart, applies promo if any, submits order, captures order confirmation ID, emits `hyperpure_order_placed` event visible in admin dashboard. Total time < 3 min. (Partially covers ADMIN-06 for HyperPure driver.)
- **T-7 (drill 6):** Blinkit emergency top-up of 1 SKU placed via Blinkit test account (if available). Driver searches, adds to cart, submits, captures order ID, emits `blinkit_order_placed` event visible in admin. Total time < 2 min.
- **T-8 (playbook):** `docs/rc-agent-mobile-tos-incident-playbook.md` exists. Contains: (a) escalation contact list (Zomato Partner Support phone/email, HyperPure account manager, Blinkit support), (b) step-by-step kill-switch invocation (including exact admin URL + toggle path), (c) manual-fallback workflow for each app (how reception takes over manually while bot is paused), (d) audit-log reconstruction guide (SQL queries + admin dashboard filters to reconstruct what the bot did in the last N hours), (e) ToS-incident escalation tree (who calls Uday, who calls Bono, who calls the platform support line, in what order). Minimum 400 lines. Uday reads it end-to-end and signs off. (E2E-01)
- **T-9 (retrospective):** `RETROSPECTIVE.md` + `OQ-RESOLUTION-TABLE.md` exist. Every OQ across phases 429-443 has a row with: phase, OQ-ID, text, resolution (resolved / deferred-to-v51.0 / rejected), resolution-evidence (commit hash / decision doc / "deferred per Uday email 2026-MM-DD"). No UNRESOLVED rows unless Uday explicitly blesses.
- **T-10 (milestone close):** `docs/ARCHITECTURE.md` section 20.3 shipped-milestones table has a new row for v50.0 with SHIPPED status + date. `.planning/MILESTONES.md` v50.0 entry changed from `status: PLANNING` to `status: SHIPPED` with date. `~/.claude/projects/C--Users-bono/memory/gsd-projects.md` shipped-milestones table + active-work section updated to reflect v50.0 shipped. ALL three updates in ONE commit.

### Required artifacts (files that must exist)

| Path | Provides | Min lines / size | Contains |
|------|----------|------------------|----------|
| `drill-results/zomato-e2e.json` | Drill 1 evidence | 50 lines JSON | `{ drill: "zomato-e2e", orders: [{order_id, states_observed, latencies_ms, whatsapp_sent_ts, admin_visible: true}], summary: {pass_count, fail_count, avg_order_to_whatsapp_ms} }` |
| `drill-results/zomato-e2e-screenshots/` | Drill 1 screenshots | >=5 files | 1 screenshot per order state transition + admin dashboard log viewer filtered to this drill |
| `drill-results/agent-recovery.json` | Drill 2 evidence | 40 lines JSON | `{ device: "tab_plus"/"m07", kill_recovery_ms, reboot_recovery_ms, ws_reconnected_ts, foreground_service_restart_count }` for each device |
| `drill-results/agent-recovery-logs/` | Drill 2 logs | 2 files min | logcat tail from each device during kill + reboot sequence |
| `drill-results/selector-miss.json` | Drill 3 evidence | 40 lines JSON | `{ selector_broken: "accept_button", break_ts, miss_detected_ts, admin_alerted_ts, fix_pushed_ts, recovered_ts, total_recovery_ms }` — total must be <300000 (5 min) |
| `drill-results/selector-miss-screenshots/` | Drill 3 screenshots | >=4 files | admin alert, Phase 443 push UI upload moment, signed-push success toast, recovered-action confirmation |
| `drill-results/kill-switch.json` | Drill 4 evidence | 40 lines JSON | `{ toggle_on_ts, tab_plus_halt_confirmed_ts, m07_halt_confirmed_ts, max_halt_latency_ms, graceful: true, toggle_off_ts, tab_plus_resume_ts, m07_resume_ts }` — max_halt_latency must be <10000 |
| `drill-results/kill-switch-timing.csv` | Drill 4 raw timing | 20+ rows | per-device per-event row with epoch-ms timestamps for audit |
| `drill-results/hyperpure-e2e.json` | Drill 5 evidence | 40 lines JSON | Same shape as zomato-e2e.json. OR empty with `{ status: "DEFERRED", reason: "HyperPure test account not available on drill date" }` if SOFT gate missed. |
| `drill-results/blinkit-e2e.json` | Drill 6 evidence | 40 lines JSON | Same shape. OR empty with DEFERRED status if SOFT gate missed. |
| `docs/rc-agent-mobile-tos-incident-playbook.md` | ToS playbook | 400 | Sections: 1 Purpose, 2 Incident types, 3 Kill-switch SOP (step-by-step with screenshots), 4 Manual-fallback per app, 5 Audit-log reconstruction, 6 Escalation tree + contacts, 7 Post-incident review template |
| `RETROSPECTIVE.md` | Milestone retrospective | 300 | What shipped, what deferred, what surprised, what to change for v51.0, lessons learned (categorized: planning / execution / ToS / DevOps / docs) |
| `OQ-RESOLUTION-TABLE.md` | OQ table | rows=count(OQs across 429-443) + header | Columns: Phase, OQ-ID, Text, Resolution, Evidence, Date |
| `MILESTONE-REPORT.md` | Uday-facing summary | 150 | One-pager: what works, what doesn't, what's deferred, what the ongoing maintenance cost looks like (~1-2 hrs/month/app per PROJECT.md line 45), what to watch for in the first 30 days |
| `UDAY-SIGNOFF.md` | Sign-off record | 30 | Timestamp, Uday confirmation method (in-person / WhatsApp / email), text of confirmation, sha256 of playbook + report at time of sign-off (so any future tampering is detectable) |
| `docs/ARCHITECTURE.md` | System architecture | +30 lines net | Section 20.3 shipped-milestones row + new section N (rc-agent-mobile subsystem) |
| `.planning/MILESTONES.md` | Milestone index | +5 lines edit | v50.0 status: PLANNING -> SHIPPED, shipped_date: 2026-MM-DD |
| `~/.claude/projects/C--Users-bono/memory/gsd-projects.md` | Memory file | +15 lines | Shipped-milestones table row + active-work moved v50.0 out + key stats updated |
| `.planning/phases/_archive/v50.0/` (directory) | Phase-dir archive | 16 subdirs | 429..444 moved here; or symlinked; per /gsd:complete-milestone convention |
| `.planning/phases/444-e2e-drills-tos-playbook/SUMMARY.md` | Phase summary | 80 | Per-plan status, G4 NOT TESTED list, deploy evidence, open follow-ups |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| Drill 1 Zomato test order | Zomato driver on Tab Plus | Accessibility Service | Logcat on Tab Plus shows `ZomatoDriver.onOrderReceived` within 5s of order placement |
| Zomato driver action | admin dashboard log viewer | comms-link WS -> racecontrol :8080 -> admin :3201 | `curl http://192.168.31.23:8080/api/v1/agent-mobile/events?device=tab_plus&driver=zomato&since=<ts>` returns the event |
| Zomato driver `marked_ready` | whatsapp-bot outbox | racecontrol -> comms-link -> whatsapp-bot | INBOX log on whatsapp-bot shows the message within 10s |
| `adb force-stop` | Foreground Service auto-restart | Android START_STICKY | Logcat shows `onCreate` -> `START_STICKY` in AgentForegroundService within 90s |
| `adb reboot` | BOOT_COMPLETED -> startForegroundService | Phase 429 boot receiver | Logcat boot_completed + AgentForegroundService onCreate within 120s of boot |
| Broken selector YAML | SelectorMissEvent | agent selector-engine from Phase 433 | Server `/api/v1/agent-mobile/selector-miss-events` endpoint shows the event within 15s |
| SelectorMissEvent | admin P1 alert | Phase 441 admin dashboard | Admin `/dashboard/alerts` shows row within 15s of emission |
| Phase 443 signed-push upload | agent applies new selector | comms-link signed-patch channel | Tab Plus logcat `SelectorHotReload applied v=<new_version>` within 30s of push |
| kill-switch flag toggle | driver halt on both devices | Phase 436 feature-flag WS + Phase 442 UI | Both devices log `KILL_SWITCH_INVOKED` + stop all driver loops within 10s; admin dashboard shows halt-confirmed per device |
| Drill JSON files | git commit | manual | `git status` shows `drill-results/*.json` staged; SHA256 in commit message |
| Playbook sign-off | Uday | in-person or WhatsApp | `UDAY-SIGNOFF.md` contains sign-off text; SHA256 matches playbook at that time |
| Milestone-close commit | ARCHITECTURE.md + MILESTONES.md + memory | single git commit | `git show HEAD --stat` shows all three files in one commit |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/PROJECT.md
@./.planning/REQUIREMENTS-v50.md
@./.planning/ROADMAP-v50.md
@./.planning/MILESTONES.md
@./.planning/phases/437-zomato-partner-driver/PLAN.md
@./.planning/phases/437-zomato-partner-driver/SUMMARY.md     # Required before drill 1 — driver behavior + known quirks
@./.planning/phases/438-hyperpure-driver/PLAN.md
@./.planning/phases/438-hyperpure-driver/SUMMARY.md
@./.planning/phases/439-blinkit-driver/PLAN.md
@./.planning/phases/439-blinkit-driver/SUMMARY.md
@./.planning/phases/436-feature-flag-system/PLAN.md          # Kill-switch semantics
@./.planning/phases/436-feature-flag-system/SUMMARY.md
@./.planning/phases/442-feature-flag-capability-ui/PLAN.md   # Admin UI for kill-switch toggle
@./.planning/phases/442-feature-flag-capability-ui/SUMMARY.md
@./.planning/phases/433-selector-dsl-hot-reload/PLAN.md
@./.planning/phases/433-selector-dsl-hot-reload/SUMMARY.md
@./.planning/phases/443-selector-map-remote-push-ui/PLAN.md  # UI drill 3 uses for push
@./.planning/phases/443-selector-map-remote-push-ui/SUMMARY.md
@./.planning/phases/441-admin-dashboard-reception-view/PLAN.md
@./.planning/phases/441-admin-dashboard-reception-view/SUMMARY.md   # Log viewer used in drills 1, 5, 6 + alerts in drill 3 + halt-confirmed in drill 4
@./.planning/phases/429-kotlin-scaffold-http-comms-link/PLAN.md     # STRUCTURE TEMPLATE

### Interfaces the executor will need

**Admin dashboard endpoints (Phase 441):**
```
GET  /api/v1/agent-mobile/events?device=<id>&driver=<name>&since=<epoch_ms>&until=<epoch_ms>
     -> [{ event_id, device_id, driver, event_type, payload, screenshot_hash, ts }]
GET  /api/v1/agent-mobile/alerts?severity=P1&since=<epoch_ms>
     -> [{ alert_id, device_id, type, message, ts, acknowledged }]
POST /api/v1/agent-mobile/flags/global
     { flag_name: "pause_all_drivers", value: true }
     -> { applied_to: ["tab_plus", "m07"], confirmed_ts: [...] }
```

**Selector push (Phase 443):**
```
POST /api/v1/agent-mobile/selectors/push
     (multipart) yaml file + signature + targets[]
     -> { patch_id, pushed_to: [devices], accepted_at: [...] }
```

**Selector miss (Phase 433):**
```
-- Emitted by agent on selector miss
{ v: 1, type: "selector_miss", from: "tab_plus", payload: {
    selector_id: "accept_button", yaml_version: "v1.3.2",
    attempted_at: <ts>, screen_hash: "sha256...", last_known_good_ts: <ts> }}
```

**Kill-switch flag (Phase 436):**
```
-- Global flag, WS-pushed to all devices within 10s of toggle
{ v: 1, type: "flag_update", payload: { flag: "pause_all_drivers", value: true, ts: <epoch_ms> }}
-- Agent ACKs:
{ v: 1, type: "flag_ack", from: "tab_plus", payload: { flag: "pause_all_drivers", value_applied: true, halted_drivers: ["zomato", "hyperpure", "blinkit"], ts: <epoch_ms> }}
```

**Kotlin agent /health response (Phase 429):**
```
GET http://<device_lan_ip>:8090/health
-> { ok: true, build_id: "...", protocol_version: 1, device_id: "...", drivers_active: ["..."], kill_switch: false }
```

## 5. Atomic plan breakdown (10 plans)

Each plan is ONE session, ONE commit, ONE acceptance criterion. Waves within the phase listed below.

### Sub-wave summary

- **Sub-wave A (within 444, parallel eligible):** 444-01, 444-02, 444-03, 444-04 — drills 1-4, independent, each on a defined device/window. Run sequentially in practice because they share the physical venue floor and admin operator time; but no code dependency between them.
- **Sub-wave B:** 444-05, 444-06 — drills 5, 6. Depend on 444-01 (Zomato drill ergonomics/learnings apply; also confirms comms-link stable). SOFT gates (may be DEFERRED).
- **Sub-wave C:** 444-07 — playbook doc. Depends on drills 1, 4 complete (uses real evidence). Could be drafted in parallel with drills but finalized after.
- **Sub-wave D:** 444-08 — retrospective. Depends on ALL drills complete (444-01..06) and playbook draft (444-07). Consolidates OQs.
- **Sub-wave E:** 444-09 — Uday sign-off checkpoint. Depends on 444-07 + 444-08 complete.
- **Sub-wave F:** 444-10 — milestone close. Depends on 444-09 PASS.

---

### 444-01-PLAN — Zomato end-to-end drill

**Goal:** Run the full Zomato order -> auto-accept -> kitchen -> ready -> WhatsApp flow 5-10 times on Tab Plus with the real Zomato Partner TEST account, record evidence, verify every state transition is visible in the admin dashboard log viewer. All within business-hours window.

**Covers:** E2E-02, ADMIN-06 (partial — log viewer usage), ADMIN-01..03 (reception view)

**Dependencies:** Phases 437 + 441 + 442 shipped and running on Tab Plus + admin :3201

**Type:** `checkpoint:human-verify` at end (physical orders on live test account + admin-operator verification)

**Tasks:**

1. **Pre-flight (5 min):**
   - Verify Tab Plus `/health`: `curl http://<tab_plus_lan_ip>:8090/health` returns `ok:true` + `drivers_active` includes `"zomato"` + `kill_switch: false`.
   - Verify admin dashboard log viewer loads + filter UI works: visit `http://192.168.31.23:3201/dashboard/agent-mobile/events`, filter device=tab_plus, driver=zomato, time=last-1h. If the filter UI returns 500 / empty / loader-stuck, STOP — file against Phase 441 and block.
   - Verify Zomato test account is active: log into Zomato Partner TEST portal from a separate device (laptop), confirm test-order placement UI is reachable.
   - Verify whatsapp-bot outbox is running: `curl http://192.168.31.27:<whatsapp_port>/health` returns ok (actual port from comms-link registry).
   - Kill-switch OFF, confirmed via admin UI.

2. **Drill execution (20 min, 5-10 orders with 2-3min spacing):**
   - For each test order placed:
     - Record `t0 = <epoch_ms>` when order placed in Zomato Partner TEST portal.
     - Observe Tab Plus screen: auto-accept should fire within 5s. Screenshot Tab Plus (via `adb exec-out screencap -p > order-<N>-step1.png`).
     - Mark as `in_kitchen` — observe state change in admin log viewer. Screenshot admin dashboard filtered to this order.
     - Mark as `ready_for_pickup` — observe state change; WhatsApp should fire within 10s.
     - Check whatsapp-bot INBOX: `curl http://<whatsapp-host>/outbox/recent | jq` — message with this order's delivery partner number present.
     - Record `t1 = <epoch_ms>` when WhatsApp sent. Compute `order_to_whatsapp_ms = t1 - t0`. Expected < 90000.
     - Verify all 5 events (`order_received`, `auto_accepted`, `marked_in_kitchen`, `marked_ready`, `whatsapp_sent`) appear in admin log viewer within 10s of their respective action times.
     - Verify each event has `screenshot_hash` populated (ADMIN-06 screenshot preview).

3. **Evidence capture:**
   - Write `drill-results/zomato-e2e.json`: per-order row + summary with `pass_count`, `fail_count`, `avg_order_to_whatsapp_ms`, `p95_latency_ms`.
   - Move screenshots into `drill-results/zomato-e2e-screenshots/order-<N>-step<K>.png` + `admin-<N>-filtered.png`.

4. **Post-flight verification:**
   - Admin log viewer filter by this drill's 30-minute window returns exactly (orders x 5) events. Any mismatch = fail.
   - Zomato Partner TEST portal shows all orders in final `delivered` or `cancelled` (dev-cleaned) state.
   - No ToS warnings on the test account (check Zomato Partner account-health page).

**Verify (automated where possible):**
```bash
# Event count matches expected
EXPECTED=$(( ORDER_COUNT * 5 ))
ACTUAL=$(curl -s "http://192.168.31.23:8080/api/v1/agent-mobile/events?device=tab_plus&driver=zomato&since=<drill_start_ts>&until=<drill_end_ts>" | jq 'length')
[ "$ACTUAL" -eq "$EXPECTED" ] || exit 1

# P95 latency under threshold
P95=$(jq '.summary.p95_latency_ms' drill-results/zomato-e2e.json)
[ "$P95" -lt 90000 ] || exit 1

# WhatsApp outbox has one entry per order
OUTBOX=$(curl -s http://<whatsapp>/outbox/recent?since=<drill_start_ts> | jq 'length')
[ "$OUTBOX" -eq "$ORDER_COUNT" ] || exit 1
```

**Acceptance:**
- All 5-10 orders completed without manual intervention.
- Every order under 90s total.
- Every state transition visible in admin within 10s.
- WhatsApp outbox has 1 message per order.
- No ToS warning on test account.
- `drill-results/zomato-e2e.json` committed with all evidence fields populated.

**Checkpoint (human-verify):** James + reception operator physically observe Tab Plus screen during drill + admin dashboard. Operator confirms "yes, each order went through visible states and WhatsApp was sent." If test account has NO test-order capability and Uday has pre-approved real low-value orders fallback, use 5 real orders with Uday sign-off on each (logged in drill-results/zomato-e2e.json `fallback_mode: "real_orders_with_uday_signoff"`).

**G4 NOT TESTED list:**
- Failure modes: network drop mid-order (tested in drill 2 separately via recovery drill).
- Scale: only 5-10 orders; production volume is 50-80/day. Document as G4 in retrospective.
- Edge cases: order cancellation by customer mid-flow; order modification by customer; Zomato UI language=Hindi (we tested English default only).

**Commit message template:**
```
test(444): zomato end-to-end drill - <N>/<N> orders passed, p95=<ms>ms

Covers E2E-02 + ADMIN-06 (log viewer) + ADMIN-01..03 (reception view).
All orders: order->WhatsApp under 90s (target met).
All events visible in admin within 10s.
Evidence: .planning/phases/444-e2e-drills-tos-playbook/drill-results/zomato-e2e.json

G4 NOT TESTED: network drop mid-order, scale >10 orders/hr, Hindi UI.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

**Open questions (inherited, must resolve by end of plan):**
- OQ-Zomato-01: test account available? (ASK UDAY IN PREFLIGHT; if NO and real-order fallback not approved, BLOCK and file hotfix sub-phase)

---

### 444-02-PLAN — Agent-recovery drill

**Goal:** Kill agent process AND reboot device on BOTH Tab Plus + M07, verify full auto-recovery within 2 minutes in both cases on both devices.

**Covers:** E2E-03, AGENT-05, AGENT-06

**Dependencies:** Phase 429 (boot receiver + foreground service) shipped.

**Type:** `auto` — fully automatable via adb (no UI-operator needed)

**Constraint:** Run OUTSIDE business hours (before 11:00 IST or after 23:00 IST) — devices will be offline briefly.

**Tasks:**

1. **Pre-flight:**
   - Confirm both devices online: `curl http://<tab_plus_ip>:8090/health` and `curl http://<m07_ip>:8090/health` — both `ok:true`.
   - Confirm `/fleet/health` on racecontrol shows both with `ws_connected: true, http_reachable: true`.
   - Start logcat capture on both: `adb -s <serial> logcat -v time > drill-results/agent-recovery-logs/tab_plus-<ts>.log &` (same for M07).

2. **Kill test (per device):**
   - Record `t0 = <epoch_ms>`.
   - `adb -s <serial> shell am force-stop in.racingpoint.rcagentmobile`
   - Poll `/health` every 5s up to 120s. Record `t1 = <first ok:true>`.
   - `kill_recovery_ms = t1 - t0`. Assert < 90000.
   - Poll `/fleet/health` on server, wait for `ws_connected: true` for this device. Record `ws_reconnected_ts`.

3. **Reboot test (per device):**
   - Record `t0 = <epoch_ms>`.
   - `adb -s <serial> reboot`
   - Poll adb state via `adb -s <serial> wait-for-device` (this is reboot-complete proxy; may return before agent).
   - Poll `/health` every 10s up to 180s. Record `t1 = <first ok:true>`.
   - `reboot_recovery_ms = t1 - t0`. Assert < 120000.
   - Check logcat for `BootCompletedReceiver.onReceive` -> `startForegroundService` -> `AgentForegroundService.onCreate` sequence.

4. **Evidence capture:**
   - Write `drill-results/agent-recovery.json`:
     ```json
     {
       "devices": [
         { "id": "tab_plus", "kill_recovery_ms": N, "reboot_recovery_ms": N,
           "ws_reconnected_ts_kill": N, "ws_reconnected_ts_reboot": N,
           "foreground_service_restart_count": N, "logs": "agent-recovery-logs/tab_plus-<ts>.log" },
         { "id": "m07", ... }
       ],
       "summary": { "all_under_90s_kill": true, "all_under_120s_reboot": true }
     }
     ```

**Verify:**
```bash
jq '.summary.all_under_90s_kill and .summary.all_under_120s_reboot' drill-results/agent-recovery.json
# Must return true

# Logs must show restart sequence
grep -q "AgentForegroundService.onCreate" drill-results/agent-recovery-logs/tab_plus-*.log
grep -q "BootCompletedReceiver" drill-results/agent-recovery-logs/tab_plus-*.log
```

**Acceptance:**
- Both devices recover from kill in <90s.
- Both devices recover from reboot in <120s.
- Foreground Service restart visible in logcat (kill case).
- BOOT_COMPLETED -> startForegroundService visible in logcat (reboot case).
- Post-drill `/fleet/health` shows both devices back to normal heartbeat cadence.

**G4 NOT TESTED:**
- Battery pull (vs soft reboot) — expected same behavior but physical battery-pull-test not done on Tab Plus (has non-removable battery).
- Network outage during reboot (WS reconnection backoff is tested in Phase 429, not here).
- Multiple-rapid-kill (e.g. kill x3 in 30s) — could trigger Android foreground-service rate limit; document as G4.

---

### 444-03-PLAN — Selector-miss recovery drill

**Goal:** Intentionally break a Zomato selector on Tab Plus, verify agent emits `SelectorMissEvent`, admin sees alert, push fix via Phase 443 UI, total recovery < 5 min.

**Covers:** E2E-04, SELECTOR-04, ADMIN-05 (push UI exercise)

**Dependencies:** Phases 433 + 443 shipped.

**Type:** `auto` (agent side) + `checkpoint:human-verify` (operator uses Phase 443 UI)

**Tasks:**

1. **Pre-flight:**
   - Note current selector-map version on Tab Plus: `curl http://<tab_plus_ip>:8090/selectors/version` -> `v1.3.2` (example).
   - Export current selectors: `curl http://<tab_plus_ip>:8090/selectors/current -o /tmp/zomato-v1-good.yaml`.
   - Create a broken copy: `sed 's|//android.widget.Button\[@text="Accept"\]|//nonexistent-break-for-drill|' /tmp/zomato-v1-good.yaml > /tmp/zomato-v1-broken.yaml`.
   - Sign the broken YAML with the selector-signing key (per Phase 433 protocol).

2. **Break (push the broken selector via Phase 443 UI):**
   - James opens `http://192.168.31.23:3201/dashboard/agent-mobile/selectors/push`.
   - Uploads `/tmp/zomato-v1-broken.yaml`, targets `tab_plus`, signs, clicks "Push".
   - Record `break_ts`.
   - Confirm Tab Plus logcat shows `SelectorHotReload applied v=<new>`.

3. **Trigger miss (place test order on Zomato Partner TEST portal):**
   - Test order placed -> Zomato app on Tab Plus shows Accept button.
   - Agent attempts the (now-broken) XPath.
   - Agent should log `SelectorMissEvent` and emit via comms-link.
   - Poll admin dashboard `/api/v1/agent-mobile/alerts?since=<break_ts>` every 5s — first matching alert = `miss_detected_ts + admin_alerted_ts`.

4. **Fix (push the good selector via Phase 443 UI):**
   - James opens Phase 443 UI again.
   - Uploads `/tmp/zomato-v1-good.yaml`, signs, pushes to `tab_plus`.
   - Record `fix_pushed_ts`.
   - Place another test order. Verify auto-accept succeeds.
   - Record `recovered_ts`.

5. **Evidence:**
   - Write `drill-results/selector-miss.json`:
     ```json
     {
       "selector_broken": "accept_button",
       "break_ts": N, "miss_detected_ts": N, "admin_alerted_ts": N,
       "fix_pushed_ts": N, "recovered_ts": N,
       "total_recovery_ms": recovered_ts - break_ts
     }
     ```
   - Screenshots: admin alert row, push-UI upload moment, push-success toast, recovered-action confirmation.
   - Total recovery must be < 300000 (5 min).

**Verify:**
```bash
TOTAL=$(jq '.total_recovery_ms' drill-results/selector-miss.json)
[ "$TOTAL" -lt 300000 ] || exit 1

# SelectorMissEvent actually happened
jq -e '.miss_detected_ts > .break_ts' drill-results/selector-miss.json

# Admin alert fired
jq -e '.admin_alerted_ts - .miss_detected_ts < 15000' drill-results/selector-miss.json
```

**Acceptance:**
- SelectorMissEvent emitted.
- Admin alert visible within 15s of miss.
- Phase 443 push UI accepted the fix.
- Tab Plus applied the fix within 30s.
- Next order succeeded.
- Total < 5 min.

**G4 NOT TESTED:**
- Multiple simultaneous breaks (e.g. 3 selectors at once).
- Network drop during push.
- Signing-key rotation mid-push.

**Open questions:**
- OQ-Selector-Drill-01: Does the agent roll back to last-known-good automatically if fix is NOT pushed within N minutes? If yes, drill should also verify rollback kicks in. — Check Phase 433 SUMMARY before drill; if yes, extend drill.

---

### 444-04-PLAN — ToS-incident kill-switch drill

**Goal:** Toggle global `pause_all_drivers` flag via Phase 442 UI, verify ALL drivers on BOTH devices halt within 10s. Toggle off, verify resume within 10s.

**Covers:** FLAG-04, E2E-01 (operational exercise; playbook doc comes in 444-07)

**Dependencies:** Phases 436 + 442 shipped.

**Type:** `auto` + `checkpoint:human-verify` (admin operator toggles flag + verifies)

**Constraint:** Run during quiet hour (15:00-17:00 IST weekday). Pre-announce to reception operator. Manually monitor Zomato Partner app for real orders during the 60s drill window — if a real order arrives, abort drill and process manually.

**Tasks:**

1. **Pre-flight:**
   - Both devices `/health`: ok, `kill_switch: false`.
   - Admin UI: `http://192.168.31.23:3201/dashboard/agent-mobile/flags`, global flags section accessible.
   - Logcat capture on both devices.

2. **Kill-switch ON:**
   - Admin operator toggles `pause_all_drivers = true`.
   - Record `toggle_on_ts`.
   - Poll both devices' logs for `KILL_SWITCH_INVOKED`:
     - Tab Plus: first occurrence -> `tab_plus_halt_confirmed_ts`.
     - M07: first occurrence -> `m07_halt_confirmed_ts`.
   - Compute `max_halt_latency_ms = max(tab_plus_halt - toggle_on, m07_halt - toggle_on)`. Assert < 10000.
   - Verify `/health` on both returns `kill_switch: true` and `drivers_active: []`.
   - Verify admin dashboard shows halt-confirmed events.
   - Attempt a test order on Zomato Partner TEST -> agent must NOT auto-accept. Order stays in pending state (30s observation).
   - Cancel the test order manually from the portal.

3. **Graceful halt check:**
   - Review logcat for "half-pressed" or abandoned taps. There must be NO partial action logs in the 10s window after toggle.

4. **Kill-switch OFF:**
   - Toggle `pause_all_drivers = false`.
   - Record `toggle_off_ts`.
   - Poll devices for `KILL_SWITCH_CLEARED`:
     - `tab_plus_resume_ts`, `m07_resume_ts`.
   - Place another test order to confirm automation resumed.

5. **Evidence:**
   - Write `drill-results/kill-switch.json` + `drill-results/kill-switch-timing.csv`.

**Verify:**
```bash
MAX_HALT=$(jq '.max_halt_latency_ms' drill-results/kill-switch.json)
[ "$MAX_HALT" -lt 10000 ] || exit 1

# Graceful
GRACEFUL=$(jq '.graceful' drill-results/kill-switch.json)
[ "$GRACEFUL" = "true" ] || exit 1

# Both devices resumed
jq -e '.tab_plus_resume_ts and .m07_resume_ts' drill-results/kill-switch.json
```

**Acceptance:**
- Max halt latency < 10s on both devices.
- No half-pressed actions in halt window.
- Devices resume cleanly after toggle-off.
- Admin dashboard shows halt-confirmed + resume-confirmed events.
- **Any** real Zomato order that arrived during the 60s drill window must be processed manually by reception.

**G4 NOT TESTED:**
- Flag propagation when admin dashboard itself is down (requires device-side local override — is there one? — OQ-KillSwitch-01).
- Partial-pod kill-switch (only `pause_zomato_on_tab_plus`) — tested by Phase 442 but not in this drill.
- 10s threshold on cellular (this drill on LAN only).

**Open questions:**
- OQ-KillSwitch-01: If server is unreachable when operator toggles flag, does kill-switch still propagate (via secondary channel)? If NO, the playbook must document the manual device-side halt procedure (turn off WiFi / power off devices).
- OQ-KillSwitch-02: Does kill-switch drill 4 leave any state that needs manual cleanup? (e.g. WS reconnect counters, flag cache). Test during drill by inspecting `/health` and `/debug/state` on both devices before/after.

---

### 444-05-PLAN — HyperPure end-to-end drill

**Goal:** Place one bulk order of 5 SKUs via HyperPure test account (if available). If not available, document as DEFERRED.

**Covers:** ADMIN-06 (partial — HyperPure driver log events), validates Phase 438 HyperPure driver against live infrastructure.

**Dependencies:** Phase 438 shipped. Phase 444-01 completed (Zomato drill learnings apply).

**Type:** `checkpoint:human-verify` (real test account) — **SOFT gate**: if OQ-HP-01 = "no test account", log DEFERRED + continue.

**Tasks:**

1. **Pre-flight OQ-HP-01:** Is HyperPure test account available TODAY?
   - If NO: write `drill-results/hyperpure-e2e.json` with `{ status: "DEFERRED", reason: "HyperPure test account unavailable" }`. Skip to 444-06.
   - If YES: proceed.

2. **Drill execution (10 min, 1 order):**
   - Pick 5 SKUs from the HyperPure standard catalog (e.g. flour 25kg, milk 30L, paneer 5kg, tomatoes 20kg, cooking oil 15L).
   - Trigger the HyperPure driver via admin dashboard "Place Order" button (or whatever Phase 438 exposes).
   - Driver navigates HyperPure Partner app on Tab Plus or M07 (check Phase 438 SUMMARY for which device).
   - Records: search-add-cart-checkout flow, promo applied, order confirmation ID captured, `hyperpure_order_placed` event emitted.
   - Verify admin dashboard log viewer shows all transitions.
   - Record total time.

3. **Evidence:**
   - `drill-results/hyperpure-e2e.json` with order ID, latencies, admin events matched.
   - Screenshots: app cart, confirmation, admin dashboard filtered.

**Verify:** Event-count match (expected K events for this flow, actual = same); total time < 180s.

**Acceptance:**
- Order confirmed by HyperPure (real order ID returned).
- All events in admin.
- < 3 min total.

**G4 NOT TESTED:**
- Out-of-stock SKU handling (depends on live inventory).
- Promo code edge cases.
- Payment-failure path (we use stored credit).

**Open questions:**
- OQ-HP-01: test account available?

---

### 444-06-PLAN — Blinkit end-to-end drill

**Goal:** Place one emergency top-up order of 1 SKU via Blinkit test account (if available). SOFT gate.

**Covers:** ADMIN-06 (partial — Blinkit driver log events), validates Phase 439.

**Dependencies:** Phase 439 shipped.

**Type:** `checkpoint:human-verify` — SOFT gate.

**Tasks:** Mirror 444-05 structure. 1 SKU, search-add-checkout, record evidence. If OQ-BL-01 = no test account, DEFERRED + continue.

**Acceptance:** Order confirmed by Blinkit, events in admin, < 2 min total.

---

### 444-07-PLAN — ToS-incident playbook doc

**Goal:** Produce the human runbook at `docs/rc-agent-mobile-tos-incident-playbook.md`. Min 400 lines. All 7 sections per must-haves T-8.

**Covers:** E2E-01

**Dependencies:** Drills 1 + 4 complete (uses real evidence: actual escalation screenshots from drill 4, actual WhatsApp fallback steps from drill 1 observations).

**Type:** `auto` (doc-writing)

**Tasks:**

1. **Draft Section 1 — Purpose:**
   - Why this doc exists (ToS risk per PROJECT.md line 43: HIGH Zomato, MEDIUM HyperPure/Blinkit).
   - Who uses it: Uday (primary owner), James (first-responder AI), reception operators, Bono (cloud-side support).
   - When to invoke: ANY platform warning, account restriction, account ban, ToS-violation notice, unusual app behavior (forced logout, CAPTCHA challenge, IP-flag).

2. **Draft Section 2 — Incident types:**
   - Type A: Account warning email/in-app (non-blocking). Response: halt specific driver, investigate, may resume with adjusted humanize settings.
   - Type B: Account restricted (reduced functionality). Response: halt driver fleet-wide for that app, engage platform support, switch to manual.
   - Type C: Account banned. Response: halt driver fleet-wide permanently, engage support to appeal, escalate to Uday, prepare manual-only operation.
   - Type D: Unusual behavior without warning (forced logout, CAPTCHA). Response: halt that device's driver, investigate, do not resume until root cause found.
   - Type E: Legal notice / Cease-and-desist. Response: IMMEDIATE full kill-switch + halt ALL drivers fleet-wide + escalate to Uday within 5 min + do not resume any automation until Uday clears.

3. **Draft Section 3 — Kill-switch SOP (with screenshots from drill 4):**
   - Exact URL: `http://192.168.31.23:3201/dashboard/agent-mobile/flags`.
   - Credentials: use admin JWT (Uday knows where the master cred is).
   - Toggle: `pause_all_drivers` = true. Save.
   - Verification: wait 10s, confirm both devices show `kill_switch: true` in `/health`.
   - Embed screenshots from drill 4 (`drill-results/kill-switch-screenshots/`).
   - Fallback if admin dashboard is down: device-side manual halt procedure (OQ-KillSwitch-01 — answer before writing this section).
   - Partial halt (per-driver-per-device): toggle specific flag e.g. `enable_zomato_on_tab_plus = false`.

4. **Draft Section 4 — Manual-fallback workflow per app:**
   - Zomato: reception operator opens Zomato Partner app on phone/Tab Plus manually, processes orders. WhatsApp to delivery partner manually. Kitchen status via shouting / physical board.
   - HyperPure: reception operator places orders via HyperPure Partner app manually. Delivery date noted in paper log.
   - Blinkit: reception operator places top-ups manually from the Blinkit partner app on a staff phone.
   - For each: include link/screenshot of the manual app, who knows the credentials, what log to keep during the fallback window so the audit trail survives.

5. **Draft Section 5 — Audit-log reconstruction guide:**
   - Admin dashboard log viewer: filter by device + driver + time range.
   - Direct DB queries (SQLite racecontrol.db): `SELECT * FROM agent_mobile_events WHERE ts > ? ORDER BY ts`.
   - Screenshot archive: `/var/racingpoint/agent-screenshots/<yyyy-mm-dd>/`.
   - WhatsApp outbox: `comms-link/whatsapp-bot/outbox.log`.
   - Comms-link relay log: `comms-link/logs/relay.log`.
   - 5-step reconstruction procedure for any incident window.

6. **Draft Section 6 — Escalation tree + contacts:**
   - **Level 1 (James AI, 0-5 min):** detect, halt driver, log.
   - **Level 2 (reception operator, 2-10 min):** verify halt, switch to manual, call staff-on-duty.
   - **Level 3 (Uday, 10-30 min):** decide response (reinstate / permanent halt / appeal).
   - **Level 4 (Uday + Bono, 30+ min):** vendor escalation.
   - **Contacts table:**
     - Zomato Partner Support — phone: `[OQ-ToS-01 — fill from Uday]`, email: `[fill]`, escalation code: `[if any]`.
     - HyperPure account manager — name + phone + email (ask Uday).
     - Blinkit partner support — phone + email (ask Uday).
     - Uday primary: `[WhatsApp number from PROJECT.md / memory]`.
     - Bono: VPS access via comms-link relay; escalation via INBOX.md + WhatsApp.

7. **Draft Section 7 — Post-incident review template:**
   - Timeline (when detected / halted / investigated / resumed).
   - Root cause hypothesis.
   - Evidence captured (log links, screenshot links).
   - Action items (playbook update, selector fix, humanize-layer tuning, kill-switch tuning).
   - Uday sign-off on review.

8. **Verify doc completeness:**
   - `wc -l docs/rc-agent-mobile-tos-incident-playbook.md` -> >= 400.
   - All 7 sections present (grep section headers).
   - All OQs resolved: OQ-ToS-01 (Zomato contact) filled; OQ-ToS-02 (drill 4 cleanup state) filled.
   - Every screenshot referenced exists in `drill-results/`.

**Verify:**
```bash
wc -l docs/rc-agent-mobile-tos-incident-playbook.md  # expect >= 400
grep -c "^## " docs/rc-agent-mobile-tos-incident-playbook.md  # expect >= 7
grep -q "OQ-ToS-01" docs/rc-agent-mobile-tos-incident-playbook.md && exit 1 || true  # must be resolved (no OQ-ToS-01 marker left)
```

**Acceptance:**
- Doc exists, >= 400 lines, 7 sections.
- All embedded screenshot paths exist.
- All contacts filled (no `[fill]` placeholders left).
- Kill-switch SOP references drill-4 evidence.

**G4 NOT TESTED:**
- Playbook has not been exercised in a REAL ToS incident (only drill). Note in Section 7.

**Open questions (must be resolved before commit):**
- OQ-ToS-01: Zomato Partner Support escalation contact (need from Uday).
- OQ-ToS-02: Drill 4 post-state cleanup requirements (answered in drill 4 execution).

---

### 444-08-PLAN — Milestone retrospective + OQ resolution table

**Goal:** Every OQ across phases 429-443 gets a row with resolution. Retrospective covers: shipped / deferred / lessons / cross-milestone trends.

**Covers:** Milestone-close hygiene (no explicit REQ-ID but required by CLAUDE.md standing rule "Milestone completion = update ARCHITECTURE.md + memory").

**Dependencies:** All drill plans (444-01..06) complete + 444-07 draft exists.

**Type:** `auto`

**Tasks:**

1. **Consolidate OQs:**
   ```bash
   grep -rn "^\s*-\s.*OQ-" .planning/phases/{429,430,431,432,433,434,435,436,437,438,439,440,441,442,443,444}*/PLAN.md \
     > /tmp/all-oqs.txt
   # Expected count: 34+ per the prompt
   ```
   For each OQ, determine resolution by reading phase SUMMARY + git log:
   - `resolved` — fix commit exists, evidence in SUMMARY.
   - `deferred-to-v51.0` — marked for rc-agent-ps5 milestone (document in v51.0 PROJECT.md).
   - `deferred-to-next-milestone` (no v51.0 yet) — tracked in .planning/BACKLOG.md.
   - `rejected` — decided not to address, reason logged.

2. **Write `OQ-RESOLUTION-TABLE.md`:**
   ```
   | Phase | OQ-ID | Text | Resolution | Evidence | Date |
   |-------|-------|------|------------|----------|------|
   | 429   | OQ-1  | ... | resolved   | commit abc1234 | 2026-04-18 |
   | 432   | OQ-3  | ... | deferred-to-v51.0 | .planning/PROJECT-v51.md §2 | 2026-MM-DD |
   | ...   |       |      |            |          |      |
   ```
   Every OQ from phases 429-444 present. Zero UNRESOLVED (or Uday blesses each).

3. **Write `RETROSPECTIVE.md`:**
   Sections:
   - **What shipped (by requirement ID):** 54 - deferred_count requirements delivered.
   - **What deferred:** CARDBOARD-01..02 (if Q2 unresolved), any other phases that couldn't complete (e.g. Blinkit drill SOFT-gate missed).
   - **What surprised us (positive):** e.g. Phase 429 shipped faster than estimated; selector DSL proved more robust than feared.
   - **What surprised us (negative):** e.g. Zomato Partner TEST portal quirks; M07 OEM battery-killer aggressive.
   - **Lessons — planning:** e.g. "goal-backward artifacts saved 3 wave-1 plans from scope creep".
   - **Lessons — execution:** e.g. "Kotlin unit tests caught 4 bugs that MMA would have missed".
   - **Lessons — ToS:** e.g. "Humanize layer required tighter tuning than defaults; see Phase 435 commit X".
   - **Lessons — DevOps:** e.g. "APK deploy via ADB + HKLM-equivalent on Android (DPC) needed setup".
   - **Lessons — docs:** e.g. "playbook should have been started in Phase 437 not 444".
   - **Cross-milestone trends:** pull last 3 milestones (v47, v48, v49) retrospectives + note recurring themes (e.g. frontend-staleness, session-0 context, MMA catches late bugs).
   - **Input into v51.0 (rc-agent-ps5):** what this milestone taught that matters for the next one.

4. **Write `MILESTONE-REPORT.md`** (Uday-facing, 1-page):
   - What works (one para).
   - What doesn't / is deferred (one para).
   - Ongoing maintenance cost (~1-2 hrs/month/app).
   - 30-day watch list.
   - Sign-off block for Uday.

**Verify:**
```bash
# Every OQ accounted for
grep -c "^| " OQ-RESOLUTION-TABLE.md  # >= OQ count + 1 (header)
grep -c "UNRESOLVED" OQ-RESOLUTION-TABLE.md  # must be 0 unless explicitly allowed

# Retrospective length
wc -l .planning/phases/444-e2e-drills-tos-playbook/RETROSPECTIVE.md  # >= 300

# Report length
wc -l .planning/phases/444-e2e-drills-tos-playbook/MILESTONE-REPORT.md  # >= 150
```

**Acceptance:**
- Every OQ has a row + resolution.
- Retrospective covers all required sections.
- Milestone report is Uday-readable in < 10 min.

---

### 444-09-PLAN — Uday sign-off checkpoint

**Goal:** Uday reviews + signs off on ToS playbook + milestone report.

**Covers:** E2E-01 (explicit ROADMAP criterion 4: "ToS-incident playbook doc reviewed and signed off by Uday")

**Dependencies:** 444-07 + 444-08 complete.

**Type:** `checkpoint:human-action` (genuine — only Uday can sign off)

**Tasks:**

1. **Prepare review packet:**
   - `docs/rc-agent-mobile-tos-incident-playbook.md`
   - `MILESTONE-REPORT.md`
   - `RETROSPECTIVE.md` (optional deep-dive)
   - `OQ-RESOLUTION-TABLE.md` (reference)
   - Drill results summary (pass/fail matrix across 444-01..06)

2. **Delivery channel:**
   - **Preferred:** in-person meeting — Uday reads the playbook + report in one sitting; James answers questions live.
   - **Acceptable fallback:** WhatsApp + PDF attachments — Uday confirms by WhatsApp message.
   - **Not acceptable:** email-only with no acknowledgement.

3. **Capture sign-off:**
   - Write `UDAY-SIGNOFF.md`:
     ```
     Date: 2026-MM-DD HH:MM IST
     Method: in-person | whatsapp | email
     Uday's confirmation text: "<paste exactly>"
     Playbook SHA256 at sign-off: <sha256 of docs/rc-agent-mobile-tos-incident-playbook.md>
     Report SHA256 at sign-off: <sha256 of MILESTONE-REPORT.md>
     Conditions attached (if any): <...>
     James: [signed]
     ```

4. **If Uday flags concerns:**
   - Block 444-10.
   - Open sub-phase 444-09a addressing concerns.
   - Re-present. Re-sign.

**Verify:**
- `UDAY-SIGNOFF.md` exists, contains SHA256 matching current doc state.
- Playbook SHA256 matches: `sha256sum docs/rc-agent-mobile-tos-incident-playbook.md | grep <sha>`.

**Acceptance:**
- Uday's confirmation captured verbatim.
- SHA256 matches (tamper-evident).
- If conditions attached, they're listed and either met or deferred with explicit Uday bless.

**Resume signal:** Uday replies `approved` or equivalent; or describes required changes.

---

### 444-10-PLAN — Milestone close (ARCHITECTURE + MILESTONES + memory + archive)

**Goal:** Single atomic commit that marks v50.0 SHIPPED across all three sources of truth, then archives phase dirs per `/gsd:complete-milestone` convention.

**Covers:** Milestone-completion standing rule (CLAUDE.md "Milestone completion = update ARCHITECTURE.md + memory" + "ROADMAP plan checkbox sync").

**Dependencies:** 444-09 PASS.

**Type:** `auto`

**Tasks:**

1. **Update `docs/ARCHITECTURE.md`:**
   - Section 20.3 shipped-milestones table: add row `| v50.0 | rc-agent-mobile | 2026-MM-DD | Reception automation: Zomato + HyperPure + Blinkit via Kotlin Android agent on Tab Plus + M07 |`.
   - Add new section (locate appropriate slot, probably after existing agent/service sections) describing rc-agent-mobile subsystem: binary (APK), targets (Tab Plus + M07), boundary (comms-link WS at :8765), drivers registry, selector DSL, feature flags, ToS risk posture, maintenance model (1-2 hrs/month/app).
   - If section 20.3 doesn't exist, create it (keeps standing rule alive).

2. **Update `.planning/MILESTONES.md`:**
   - v50.0 row: `status: PLANNING` -> `status: SHIPPED`.
   - Add `shipped_date: 2026-MM-DD`.
   - Add `ship_commit: <hash-of-this-commit-pending>` — filled after commit by amending OR reference in the SUMMARY.

3. **Update `~/.claude/projects/C--Users-bono/memory/gsd-projects.md`:**
   - Shipped-milestones table: add row for v50.0.
   - Active-work section: remove v50.0 entry.
   - Key stats: increment milestone count.
   - If the file structure has changed since last milestone, adapt (read first).

4. **Update `.planning/ROADMAP-v50.md`:**
   - Ship-gate checklist (lines 217-225) — all boxes checked.
   - Per-phase plan-checkbox sync (standing rule): every `- [ ] <phase-NN-plan>` in ROADMAP-v50.md set to `- [x]`.
   - `grep "^- \[ \]" .planning/ROADMAP-v50.md` must return 0 lines post-close.

5. **Update `.planning/REQUIREMENTS-v50.md`:**
   - Every delivered requirement `- [ ]` -> `- [x]`.
   - Deferred requirements: add inline note `(DEFERRED to v51.0 — see OQ-RESOLUTION-TABLE)`.
   - Coverage table (line 189-192): status `Pending` -> `Shipped` or `Deferred`.

6. **Archive phase dirs:**
   - Create `.planning/phases/_archive/v50.0/` directory.
   - Move phase dirs 429..444 into `_archive/v50.0/` (git mv to preserve history).
   - `/gsd:complete-milestone` convention: preserve directory names, add `_archive/v50.0/INDEX.md` listing all 16 phases + ship date.
   - **Safety:** verify all PLAN.md + SUMMARY.md + drill-results/* carried over. `find .planning/phases/_archive/v50.0 -name "SUMMARY.md" | wc -l` should return 16 (one per phase).

7. **Write `.planning/phases/_archive/v50.0/444-e2e-drills-tos-playbook/SUMMARY.md`:**
   - Per-plan status (444-01..10 all shipped).
   - G4 NOT TESTED consolidated list (all drills).
   - Deploy evidence: commits + hashes + SHA256 of playbook.
   - Open follow-ups -> v51.0 input.

8. **Git commit (ONE atomic commit):**
   - Staged files: `docs/ARCHITECTURE.md`, `.planning/MILESTONES.md`, `~/.claude/projects/C--Users-bono/memory/gsd-projects.md`, `.planning/ROADMAP-v50.md`, `.planning/REQUIREMENTS-v50.md`, archived phase dirs.
   - Commit message:
     ```
     ship(v50.0): rc-agent-mobile milestone close

     v50.0 rc-agent-mobile SHIPPED.

     - 54 reqs delivered (CARDBOARD-01..02 deferred to v51.0 per OQ-Q2)
     - 16 phases (429-444) complete
     - 4 E2E drills PASS (Zomato, agent-recovery, selector-miss, kill-switch)
     - 2 SOFT drills: HyperPure [PASS|DEFERRED], Blinkit [PASS|DEFERRED]
     - ToS-incident playbook signed off by Uday (see UDAY-SIGNOFF.md)
     - 34+ OQs consolidated in OQ-RESOLUTION-TABLE.md

     Updates:
     - ARCHITECTURE.md §20.3 + rc-agent-mobile section
     - MILESTONES.md v50.0: PLANNING -> SHIPPED
     - memory gsd-projects.md shipped table + active work
     - ROADMAP-v50.md + REQUIREMENTS-v50.md checkbox sync
     - Phase dirs 429-444 archived under _archive/v50.0/

     Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
     ```

9. **Auto-push + notify (per CLAUDE.md standing rule):**
   - `git push`.
   - Append to `C:\Users\bono\racingpoint\comms-link\INBOX.md` with v50.0 SHIPPED announcement (SHA256 of playbook, signoff date).
   - Send WS message to Bono via comms-link relay.
   - Append to `LOGBOOK.md`: `| YYYY-MM-DD HH:MM IST | James | <commit-hash> | v50.0 rc-agent-mobile SHIPPED |`.

10. **Post-ship verification (in a SEPARATE message per CGP H2):**
    - `git log -1 --stat` — confirm all 3 mandatory files in one commit.
    - `grep -q "v50.0.*SHIPPED" docs/ARCHITECTURE.md`
    - `grep -q "status: SHIPPED" .planning/MILESTONES.md` (v50 row)
    - `grep -q "v50.0" ~/.claude/projects/C--Users-bono/memory/gsd-projects.md` shipped table
    - Bono ACK via INBOX.md or WS within 24hr.

**Verify:**
```bash
# Atomic commit
git show HEAD --stat | grep -E "ARCHITECTURE\.md|MILESTONES\.md|gsd-projects\.md" | wc -l  # >= 3

# Ship checklist all checked
grep "^- \[ \]" .planning/ROADMAP-v50.md | wc -l  # 0

# Requirements all checked or explicitly deferred
grep "^- \[ \].*DEFERRED" .planning/REQUIREMENTS-v50.md || echo "ok"
grep "^- \[ \]" .planning/REQUIREMENTS-v50.md | grep -v "DEFERRED" | wc -l  # 0

# Archive dir populated
ls .planning/phases/_archive/v50.0/ | wc -l  # >= 16

# Pushed
git status  # clean, up-to-date with origin
```

**Acceptance:**
- Single commit touches ARCHITECTURE.md + MILESTONES.md + memory.
- Ship gate all checked.
- Archive populated with all 16 phases.
- Pushed + Bono notified + LOGBOOK entry.

**G4 NOT TESTED:**
- Cloud DB doesn't need an update for this milestone (it's a new subsystem, no migration).
- Bono ACK is async — milestone "shipped" before ACK. If Bono flags concern within 24hr, a fast-follow sub-phase is opened; milestone stays shipped unless critical.

---

## 6. Overall phase verification

- [ ] All 10 plans committed.
- [ ] All 4 HARD drills PASS (444-01, 02, 03, 04).
- [ ] SOFT drills status recorded (444-05, 06 — PASS or DEFERRED).
- [ ] `docs/rc-agent-mobile-tos-incident-playbook.md` exists, >=400 lines, 7 sections, no placeholders.
- [ ] `RETROSPECTIVE.md` + `OQ-RESOLUTION-TABLE.md` exist; every OQ has resolution.
- [ ] `UDAY-SIGNOFF.md` exists with matching SHA256.
- [ ] ARCHITECTURE.md §20.3 updated.
- [ ] MILESTONES.md v50.0 = SHIPPED.
- [ ] memory gsd-projects.md updated.
- [ ] Integration-checker subagent PASS.
- [ ] MMA audit subagent PASS (dual reasoning modes).
- [ ] UI-auditor PASS (only if admin dashboard touched during drills).
- [ ] codebase-mapper refreshed.
- [ ] Ship commit pushed to origin.
- [ ] Bono ACK'd via INBOX.md + WS (24hr window).

## 7. Success criteria (measurable, phase-level)

1. Four mandatory drills recorded with PASS evidence in `drill-results/`.
2. Playbook doc exists, reviewed, signed off by Uday with SHA256 match.
3. Every OQ across phases 429-443 has a resolution entry.
4. ARCHITECTURE.md + MILESTONES.md + memory updated in a single atomic commit.
5. All ship-gate items in ROADMAP-v50.md checked.
6. Phase dirs 429-444 archived.
7. Bono notified + auto-push completed.

## 8. Output

After completion, this phase archives itself under `.planning/phases/_archive/v50.0/444-e2e-drills-tos-playbook/`. Next milestone (v51.0 rc-agent-ps5) starts from the lessons in this retrospective.

The v50.0 milestone is SHIPPED when commit 444-10 is pushed AND verified in a separate message per CGP H2.
