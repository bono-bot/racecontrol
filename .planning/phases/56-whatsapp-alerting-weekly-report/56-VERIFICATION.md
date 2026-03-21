---
phase: 56-whatsapp-alerting-weekly-report
verified: 2026-03-20T11:30:00+05:30
status: gaps_found
score: 12/14 must-haves verified
re_verification: false
gaps:
  - truth: "Windows Task Scheduler fires weekly-report.exe every Monday at 08:00 IST"
    status: failed
    reason: "Deployment checkpoint (Plan 02 Task 2) was auto-approved without execution. Task RacingPoint-WeeklyReport is not registered on server .23. weekly-report.exe has not been built or deployed."
    artifacts:
      - path: "crates/weekly-report/src/main.rs"
        issue: "Binary compiles but has not been built in release or deployed to C:\\RacingPoint\\ on server .23"
    missing:
      - "Run: cargo build --release --bin weekly-report on James workstation"
      - "Transfer weekly-report.exe to C:\\RacingPoint\\ on server .23"
      - "Register task: schtasks /create /tn RacingPoint-WeeklyReport /sc WEEKLY /d MON /st 08:00 /ru ADMIN /rl HIGHEST /f /tr \"\\\"C:\\RacingPoint\\weekly-report.exe\\\"\""
      - "Verify registration: schtasks /query /tn RacingPoint-WeeklyReport /v"
  - truth: "racecontrol-crate compiles with Phase 56 changes (chrono-tz dependency)"
    status: failed
    reason: "App Control policy on James workstation blocks chrono-tz v0.9.0 build script. cargo build --release -p racecontrol-crate fails with: An Application Control policy has blocked this file. The existing racecontrol.exe (built at 14:53 Mar 20) predates Phase 56 commits (16:12 Mar 20) and does not include whatsapp_alerter."
    artifacts:
      - path: "crates/racecontrol/Cargo.toml"
        issue: "chrono-tz = \"0.9\" dependency added by Phase 56 has build script blocked by Windows App Control on James workstation"
    missing:
      - "Resolve App Control policy blocking chrono-tz 0.9.0 build-script-build.exe"
      - "Alternative: upgrade chrono-tz to 0.10 in racecontrol/Cargo.toml (weekly-report already uses 0.10 which compiles)"
      - "Build and deploy new racecontrol.exe to server .23 after fix"
human_verification:
  - test: "WhatsApp delivery to Uday within 60 seconds of P0"
    expected: "Stop racecontrol service on server .23 — all pods lose WS connection within seconds. Within 60 seconds, Uday's phone receives a WhatsApp message containing: '[RP ALERT] All Pods Offline', pod count, and IST timestamp. Source: racingpoint-whatsapp-bot Evolution API instance."
    why_human: "Requires live Evolution API, configured uday_phone in racecontrol.toml [alerting] section, and real phone number. Cannot verify programmatically."
  - test: "Recovery notification after all pods reconnect"
    expected: "After restarting racecontrol service, all pods reconnect. Within 60 seconds, Uday receives '[RP RESOLVED] All Pods Offline cleared' with duration in minutes and IST timestamp."
    why_human: "Requires live fleet and Evolution API credentials in production racecontrol.toml."
  - test: "Weekly report email arrives in Uday inbox Monday 08:00 IST"
    expected: "Email from James Vowles arrives at usingh@racingpoint.in between 08:00 and 08:05 IST Monday. Subject: 'Racing Point Weekly Report - Week of {date}'. Body contains: sessions count, revenue in Rs., per-pod uptime %, incident list (or 'No incidents this week'). Racing Point brand colors (#E10600, #1A1A1A)."
    why_human: "Requires Task Scheduler on server .23, live send_email.js, and real SMTP credentials."
---

# Phase 56: WhatsApp Alerting + Weekly Report — Verification Report

**Phase Goal:** Uday receives a WhatsApp message within 60 seconds of a P0 event and a recovery notification when it clears; every Monday morning an email lands in Uday's inbox summarizing the previous week's fleet performance

**Verified:** 2026-03-20T11:30:00 IST

**Status:** gaps_found

**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

#### Plan 01 — MON-06: WhatsApp P0 Alerter

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | WhatsApp alert fires when all pods are offline (all agent_senders entries removed) | VERIFIED | `whatsapp_alerter.rs` L161-181: `PodOffline` branch, 2s debounce, `count_online_pods()`, cooldown gate, `send_whatsapp()` call with `[RP ALERT]` message |
| 2 | WhatsApp resolved fires when all pods reconnect after all-pods-offline P0 | VERIFIED | `whatsapp_alerter.rs` L183-194: `PodOnline` branch, online==total guard, `[RP RESOLVED]` message with duration |
| 3 | WhatsApp alert fires on error rate threshold breach (via broadcast channel) | VERIFIED | `whatsapp_alerter.rs` L207-223: `error_rate_rx.recv()` branch, cooldown gate, `[RP ALERT] High Error Rate` message |
| 4 | Rate limiting prevents more than 1 alert per P0 type per 30 minutes | VERIFIED | `whatsapp_alerter.rs` L167-171 (all-pods) and L210-213 (error-rate): `last_alert.elapsed() > cooldown` gate; default cooldown 1800s from `config.rs` L327 |
| 5 | All WhatsApp messages use IST timestamps | VERIFIED | `ist_now_string()` at `whatsapp_alerter.rs` L37-42: `chrono_tz::Asia::Kolkata`, format `%d %b %Y %H:%M IST` |
| 6 | Alert includes event type, summary, pod count, IST timestamp | VERIFIED | `whatsapp_alerter.rs` L172-175: `[RP ALERT] All Pods Offline - All {total} pods lost WS connection. {total} pods affected. {ist_now_string()}` |
| 7 | Resolved includes event type, duration in minutes, IST timestamp | VERIFIED | `whatsapp_alerter.rs` L186-190: `[RP RESOLVED] All Pods Offline cleared. All {total} pods online. Duration: {duration_mins}m. {ist_now_string()}` |

#### Plan 02 — MON-07: Weekly Report

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 8 | Weekly report binary queries billing_sessions for total session count in previous IST week | VERIFIED | `weekly-report/src/main.rs` L55-63: `SELECT COUNT(*) FROM billing_sessions WHERE started_at >= ? AND started_at < ?` |
| 9 | Weekly report queries pod_uptime_samples for per-pod uptime percentage | VERIFIED | `weekly-report/src/main.rs` L80-89: `SELECT pod_id, ROUND(AVG(ws_connected) * 100.0, 1) FROM pod_uptime_samples` |
| 10 | Weekly report queries alert_incidents for numbered incident list | VERIFIED | `weekly-report/src/main.rs` L91-102: `SELECT alert_type, started_at, resolved_at, pod_count, description FROM alert_incidents` |
| 11 | Weekly report queries billing_sessions for total credits billed (wallet_debit_paise) | VERIFIED | `weekly-report/src/main.rs` L65-75: `COALESCE(SUM(COALESCE(wallet_debit_paise, custom_price_paise, 0)))` |
| 12 | Report is delivered as HTML email to usingh@racingpoint.in via send_email.js | VERIFIED | `weekly-report/src/main.rs` L136-149: `tokio::process::Command::new("node").arg(&email_script_path)...`; default recipient `usingh@racingpoint.in` at L18-19 |
| 13 | Windows Task Scheduler fires weekly-report.exe every Monday at 08:00 IST | FAILED | Task `RacingPoint-WeeklyReport` not found in Task Scheduler. Plan 02 Task 2 (deployment checkpoint) was auto-approved but never executed. weekly-report.exe not built or deployed to server .23. |
| 14 | SQLite opened read-only to avoid contention with live racecontrol | VERIFIED | `weekly-report/src/main.rs` L22: `format!("sqlite:{}?mode=ro", db_path)` |

**Score:** 12/14 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|---------|--------|---------|
| `crates/racecontrol/src/whatsapp_alerter.rs` | P0 detection + Evolution API WhatsApp delivery | VERIFIED | 258 lines, exports `whatsapp_alerter_task`, full implementation with P0State, send_whatsapp, record_incident, resolve_incident |
| `crates/racecontrol/src/config.rs` | AlertingConfig struct with uday_phone, cooldown_secs, enabled | VERIFIED | `pub struct AlertingConfig` at L316, all three fields present, default cooldown 1800s |
| `crates/racecontrol/src/db/mod.rs` | pod_uptime_samples and alert_incidents tables | VERIFIED | Both tables at L2015 and L2025 with correct schema and indexes |
| `crates/weekly-report/Cargo.toml` | New Rust binary crate for weekly report | VERIFIED | `name = "weekly-report"`, sqlx 0.8, chrono-tz 0.10, workspace conventions |
| `crates/weekly-report/src/main.rs` | SQLite query + HTML construction + send_email.js shell-out | VERIFIED | 330 lines, all four queries, branded HTML with `#E10600`, send_email.js shell-out |
| `Cargo.toml` (workspace) | Workspace member registration | VERIFIED | `"crates/weekly-report"` present in workspace members |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `whatsapp_alerter.rs` | `state.bono_event_tx` | `broadcast::Receiver` subscription | WIRED | L152: `let mut bono_rx = state.bono_event_tx.subscribe()` |
| `whatsapp_alerter.rs` | Evolution API | reqwest POST to `/message/sendText/{instance}` | WIRED | L70: `format!("{}/message/sendText/{}", evo_url, evo_instance)` |
| `main.rs` | `whatsapp_alerter_task` | `tokio::spawn` | WIRED | L374: `tokio::spawn(racecontrol_crate::whatsapp_alerter::whatsapp_alerter_task(...))` |
| `error_rate.rs` | `whatsapp_alerter.rs` | broadcast channel (converted from mpsc) | WIRED | `error_rate.rs` L39: `broadcast::Sender<()>`; `main.rs` L318-320: channel created, two `.subscribe()` calls before ErrorCountLayer |
| `weekly-report/src/main.rs` | SQLite DB | sqlx read-only connection | WIRED | L22: `mode=ro` in connection URL; L23-26: `SqlitePoolOptions::new().connect(&db_url).await` |
| `weekly-report/src/main.rs` | `send_email.js` | `tokio::process::Command` shell-out | WIRED | L136-149: `Command::new("node").arg(&email_script_path).arg(&recipient).arg(&subject).arg(&html_body)` |
| Windows Task Scheduler | `weekly-report.exe` | schtasks WEEKLY MON trigger | NOT WIRED | Task `RacingPoint-WeeklyReport` not registered on server .23 (deployment pending) |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| MON-06 | 56-01-PLAN.md | P0 WhatsApp alert to Uday within 60s; recovery notification on clear | PARTIAL — code complete, build blocked | `whatsapp_alerter.rs` implements full P0 detection and Evolution API delivery. Blocked: racecontrol-crate cannot be rebuilt due to App Control blocking chrono-tz 0.9.0 build script. Deployed racecontrol.exe predates Phase 56. |
| MON-07 | 56-02-PLAN.md | Weekly report email every Monday 08:00 IST with sessions, uptime, credits, incidents | PARTIAL — code complete, deployment pending | `weekly-report/src/main.rs` implements all queries and HTML email. Blocked: binary not built, not deployed to server .23, Task Scheduler not registered. |

**Note:** MON-06 and MON-07 are defined in ROADMAP.md Phase 56 section. They do not appear in REQUIREMENTS.md, which tracks v10.0 requirements (different milestone). No orphaned requirements found for Phase 56.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|---------|--------|
| `crates/racecontrol/Cargo.toml` | 55 | `chrono-tz = "0.9"` — different version from weekly-report (0.10.4) | WARNING | Two chrono-tz versions in workspace; 0.9 build script blocked by App Control on James workstation; racecontrol-crate cannot be rebuilt in release |

No stubs, placeholders, empty handlers, or TODO comments found in Phase 56 code.

---

### Human Verification Required

#### 1. WhatsApp P0 Alert Delivery (MON-06 live test)

**Test:** With racecontrol.toml `[alerting] enabled = true` and `uday_phone = "919XXXXXXXXX"` on server .23, stop the racecontrol service (all pods lose WS connection within seconds).

**Expected:** Within 60 seconds, Uday's phone receives a WhatsApp from racingpoint-whatsapp-bot with text matching: `[RP ALERT] All Pods Offline - All N pods lost WS connection. N pods affected. DD Mon YYYY HH:MM IST`

**Why human:** Requires live Evolution API credentials, configured uday_phone, and a real phone receiving the message. Cannot verify programmatically.

#### 2. Recovery Notification (MON-06 resolved test)

**Test:** After the P0 test above, restart racecontrol — all pods reconnect.

**Expected:** Within 60 seconds of all pods reconnecting, Uday receives: `[RP RESOLVED] All Pods Offline cleared. All N pods online. Duration: Xm. DD Mon YYYY HH:MM IST`

**Why human:** Requires live fleet, Evolution API, and real phone.

#### 3. Weekly Report Email Delivery (MON-07 live test)

**Test:** After deploying weekly-report.exe to server .23 and registering the Task Scheduler task, manually trigger: `schtasks /run /tn RacingPoint-WeeklyReport` on server .23.

**Expected:** Within 2 minutes, usingh@racingpoint.in receives an email with subject `Racing Point Weekly Report - Week of DD Mon YYYY`. Body shows: Total Sessions, Total Revenue (Rs.), Fleet Avg Uptime %, Pod Uptime table per pod, Incidents table (or "No incidents this week"). Racing Point brand colors (#E10600 header, #1A1A1A background, #222222 cards).

**Why human:** Requires deployed binary, live send_email.js on server .23 with valid SMTP credentials, and checking a real inbox.

---

### Gaps Summary

Two gaps block full goal achievement:

**Gap 1 — Deployment not executed (MON-07 Task Scheduler):**
Plan 02 Task 2 is a `checkpoint:human-verify gate="blocking"` task covering deployment of weekly-report.exe to server .23 and Task Scheduler registration. The SUMMARY notes this as "auto-approved checkpoint (deployment task)" but the deployment steps were never performed. The Task Scheduler task `RacingPoint-WeeklyReport` does not exist. The weekly-report.exe binary has not been built in release mode and is not present in `target/release/`.

**Gap 2 — racecontrol-crate build failure (MON-06 deployed):**
Adding `chrono-tz = "0.9"` to racecontrol-crate introduced a build dependency whose build script is blocked by Windows App Control policy on James' workstation. `cargo build --release -p racecontrol-crate` fails with "An Application Control policy has blocked this file." The existing racecontrol.exe (built 14:53 Mar 20) predates Phase 56 commits (16:12 Mar 20) and does NOT include `whatsapp_alerter_task`. Until racecontrol can be rebuilt and deployed, MON-06 is undeliverable on server .23.

**Suggested fix for Gap 2:** Upgrade `chrono-tz` in `crates/racecontrol/Cargo.toml` from `"0.9"` to `"0.10"` — weekly-report already uses 0.10.4 which compiles successfully on this machine. This consolidates to one version and avoids the blocked build script.

**Code quality:** All implementation code is substantive, fully wired, and free of stubs or placeholders. The logic correctly implements the P0 state machine with cooldown, IST timestamps, broadcast channel subscription, and incident recording. Once the build issue is resolved and deployment completes, all automated checks will pass.

---

*Verified: 2026-03-20T11:30:00 IST*
*Verifier: Claude (gsd-verifier)*
