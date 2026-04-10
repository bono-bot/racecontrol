# Phase 353: Runbook + Staff Training — Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 353-runbook-staff-training
**Mode:** auto (--auto flag — all decisions auto-selected; no interactive Q&A)
**Areas discussed:** Document format, Incident log medium, Morning review ritual, One-pager content structure, Sign-off mechanism, Dependency handling

---

## Area 1: Document Format (OPS-15..17)

| Option | Description | Selected |
|--------|-------------|----------|
| Markdown → printable HTML (browser print) | Three `.md` files in `docs/runbooks/`, `@media print` CSS, no external tools | ✓ |
| Google Docs | Editable online, easy to share, but not in git | |
| PDF via pandoc/wkhtmltopdf | Professional output but requires binary tools on Windows venue machine | |
| Word .docx | Staff-friendly but not git-trackable, drift risk | |

**Auto-selected choice:** Markdown + browser print (recommended default)
**Notes:** Keeps runbooks in git. No tool dependencies on venue Windows machine. Browser print dialog handles A4. Three files: `runbook-admin-broken.md`, `runbook-staff-pin.md`, `runbook-cafe-menu.md`.

---

## Area 2: Incident Log Medium (OPS-18)

| Option | Description | Selected |
|--------|-------------|----------|
| Google Sheet (primary) + paper backup | Shared Sheet visible to James + Bono remotely; paper for when wifi slow | ✓ |
| Paper log only | Simple, always available, but James/Bono can't read it remotely | |
| Google Sheet only | Remote visibility but no fallback if wifi down during incident | |
| Notion/Airtable | Feature-rich but adds external service dependency | |

**Auto-selected choice:** Google Sheet + paper backup (recommended default)
**Notes:** Sheet URL pinned in `runbook-admin-broken.md`. Pre-seeded with one example row. No Google sign-in required (anyone with link can edit). Google Sheet must be created before Plan 353-02 training session.

---

## Area 3: Morning Review Ritual (OPS-19)

| Option | Description | Selected |
|--------|-------------|----------|
| Comms-link WS push + Windows Scheduled Task | `morning_review` static command fires at 08:00 IST; no human initiative needed | ✓ |
| Manual — James sends message each morning | Low friction to set up, but relies on human habit | |
| Calendar event (Google Calendar invite) | Visible to Uday too, but doesn't auto-trigger James+Bono review | |
| Cron on Bono VPS | Remote execution, but Bono VPS uptime not guaranteed | |

**Auto-selected choice:** Comms-link WS push via Windows Scheduled Task (recommended default)
**Notes:** New `MorningReview-Daily` schtask on James .27, triggers at 02:30 UTC (08:00 IST). Reuses existing `send-message.js`. New `morning_review` entry in `comms-link/data/static-commands.json`. LOGBOOK entry if incidents found; no entry if zero incidents.

---

## Area 4: One-Pager Content Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Title + Trigger + Steps + If Stuck + DO NOT | Single A4, big headings, minimal prose, explicit anti-patterns | ✓ |
| Detailed prose instructions | Comprehensive but won't fit A4, staff won't read under pressure | |
| Flowchart / decision tree | Visual but complex to maintain in markdown | |
| QR codes linking to full docs | Convenient but requires phone + wifi during an incident | |

**Auto-selected choice:** Title + Trigger + Steps + If Stuck + DO NOT (recommended default)
**Notes:** Each section is 1-5 lines. "DO NOT" section is critical — prevents the most common error per runbook. Staff operate under pressure; brevity is essential.

---

## Area 5: Uday Sign-Off Mechanism (OPS success criterion #5)

| Option | Description | Selected |
|--------|-------------|----------|
| WhatsApp YES reply + screenshot saved to repo | Matches Uday's tools, zero friction, evidence in git | ✓ |
| Digital signature on Google Doc | Formal but adds tool dependency | |
| GitHub issue closure by Uday | Requires Uday to have GitHub access | |
| Verbal/email confirmation | No verifiable record | |

**Auto-selected choice:** WhatsApp YES reply + screenshot to `docs/runbooks/uday-signoff-YYYY-MM-DD.png` (recommended default)
**Notes:** Happens at end of Plan 353-02 training session. Screenshot serves as the acceptance record.

---

## Area 6: Phase Dependency Handling

| Option | Description | Selected |
|--------|-------------|----------|
| Write runbooks now, defer printing to post-deploy of 347+346 | Stable content, feature-gated steps annotated, zero rework | ✓ |
| Wait for 347+346 to deploy before writing | Safer accuracy but delays the phase | |
| Write with placeholder [TBD] steps | Risky — planner may fill with wrong details | |

**Auto-selected choice:** Write now with `[requires Phase 347 deploy]` annotations (recommended default)
**Notes:** Runbook content (workflow, URLs, button names) is stable even before deploy. Phase 347 CONTEXT.md already documents exact UI flow. Printing deferred to post-deploy maintenance window.

---

## Claude's Discretion

- Exact CSS print stylesheet (font, margins, brand colors for print)
- One-time HTML render script approach (node vs shell)
- Google Sheet column widths and conditional formatting
- Whether `morning_review` command in static-commands.json is self-contained or calls send-message.js

## Deferred Ideas

- Digital runbook portal at `/runbooks` in admin dashboard
- Automated Google Sheet API monitoring for unreviewed incidents
- Multi-language runbooks (Hindi/English)
- Video walkthrough screen recordings
