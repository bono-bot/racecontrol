# E2E Triage Log — Phase 175

**Phase goal:** All 231 tests executed. Every failure is either fixed (commit hash) or documented as a known issue with root cause.

**Requirements addressed:**
- E2E-04: All test failures triaged, critical failures fixed, remaining documented

---

## Triage Status

| Req | Description | Status |
|-----|-------------|--------|
| E2E-01 | POS suite executed | [ ] |
| E2E-02 | Kiosk suite executed | [ ] |
| E2E-03 | Cross-sync tests executed | [ ] |
| E2E-04 | All failures triaged | [ ] |

---

## Fixed Failures

Failures that were diagnosed and fixed before phase ships. Include the commit hash.

| Test ID | Description | Root Cause | Fix | Commit |
|---------|-------------|------------|-----|--------|
| | | | | |

_Add a row here for every FAIL that was resolved. Leave empty if no failures were fixed._

---

## Known Issues

Failures that are real but deferred. Include root cause and decision so the next triage owner has full context.

| Test ID | Description | Root Cause | Severity | Decision |
|---------|-------------|------------|----------|----------|
| | | | critical/high/low | Defer to phase {N} / Won't fix: {reason} |

_Add a row here for every FAIL that is not being fixed this phase._

---

## Triage Process

For each FAIL in `E2E-TEST-RESULTS-{date}.md`, the manual report, or the `run-cross-sync.sh` output:

### Step 1 — Reproduce

Confirm the failure is real and repeatable:
- Run the same test step again. If it passes on retry, mark as **flaky** and note in Known Issues.
- Rule out environment issues: is the server running? Are pods online? Is the correct build deployed?
- If the failure is environment-specific, note the condition in Known Issues.

### Step 2 — Root Cause

Identify the exact code or config causing the failure:
- Check the browser console (F12) for JavaScript errors
- Check server logs: `GET http://192.168.31.23:8080/api/v1/fleet/health` and look for errors in `racecontrol` output
- Check rc-agent logs on the affected pod: `ssh User@<tailscale_ip> "type C:\RacingPoint\rcagent.log"`
- For UI state issues, check the WebSocket connection in DevTools → Network → WS

### Step 3 — Classify

Use this severity guide:

| Severity | Definition | Action |
|----------|------------|--------|
| **Critical** | Blocks daily operations (can't start session, billing broken, all pods offline) | Must fix NOW, commit before phase ships |
| **High** | Customer-visible bug (session timer wrong, game won't launch, PIN entry broken) | Fix in next phase OR document clearly |
| **Low** | Cosmetic or edge case (colour wrong, tooltip missing, search quirk) | Document and defer |

### Step 4 — Record

- **If fixed:** Add to **Fixed Failures** table above with commit hash
- **If deferred:** Add to **Known Issues** table above with root cause and decision
- Every FAIL must appear in exactly one of the two tables

### Step 5 — Sign Off

Check the Phase 175 Sign-off section below once all failures are triaged.

---

## Phase 175 Sign-off

Complete these steps in order before marking Phase 175 as shipped.

**Execution:**
- [ ] `test/e2e/run-e2e.sh` executed — `E2E-TEST-RESULTS-{date}.md` created (E2E-01 + E2E-02)
- [ ] `test/e2e/E2E-REPORT-TEMPLATE.md` filled in manually (all 231 tests reviewed)
- [ ] `test/e2e/run-cross-sync.sh` executed — cross-cutting results appended to results file (E2E-03)

**Triage:**
- [ ] Every FAIL in `E2E-TEST-RESULTS-{date}.md` has a row in Fixed Failures or Known Issues
- [ ] Every FAIL in the manual report has a row in Fixed Failures or Known Issues
- [ ] Every FAIL from `run-cross-sync.sh` has a row in Fixed Failures or Known Issues
- [ ] All **Critical** failures have a fix commit hash in Fixed Failures table (E2E-04)

**Completion:**
- [ ] REQUIREMENTS.md: E2E-01 marked [x]
- [ ] REQUIREMENTS.md: E2E-02 marked [x]
- [ ] REQUIREMENTS.md: E2E-03 marked [x]
- [ ] REQUIREMENTS.md: E2E-04 marked [x]
- [ ] ROADMAP.md Phase 175 marked complete
- [ ] LOGBOOK.md entry added: `| {date} IST | James | {hash} | Phase 175 E2E validation complete |`
- [ ] `git push` executed
- [ ] Bono notified via comms-link: `cd C:/Users/bono/racingpoint/comms-link && COMMS_PSK="..." node send-message.js "Phase 175 E2E complete — TRIAGE.md signed off"`

---

## Quick Reference — Test IDs

| Test ID Range | Section | How Tested |
|---------------|---------|------------|
| E2E-01 — POS (1.1–1.13) | POS machine `:3200` | `run-e2e.sh` automated + manual report |
| E2E-02 — Kiosk (2.1–2.7) | Kiosk app `:3300` | `run-e2e.sh` automated + manual report |
| E2E-03 — Cross-sync (3.1–3.4) | Both browsers | `run-cross-sync.sh` interactive guide |
| E2E-04 — Triage | This file | Completed when all failures have rows above |

---

_Last updated: — (fill in date when triage is complete)_
