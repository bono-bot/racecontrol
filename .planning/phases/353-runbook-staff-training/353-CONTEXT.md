# Phase 353: Runbook + Staff Training — Context

**Gathered:** 2026-04-11
**Mode:** auto
**Status:** Ready for planning

<domain>
## Phase Boundary

Produce three printed one-pagers physically present at the POS station, establish a staff incident log, and set up a morning review ritual for James + Bono.

This phase delivers **OPS artifacts only** — printed docs, a log sheet or Google Sheet, and a recurring review process. No code changes required in racecontrol or racingpoint-admin (those are provided by dependency phases 347 + 346). The runbook content references features that will be live once 347 + 346 deploy.

**Two repos involved:**
- `racecontrol/` — stores runbook markdown sources under `docs/runbooks/`
- `racingpoint-admin/` — no changes needed (Phase 347 delivers the staff page this runbook references)

**Not in scope:**
- Building the `/admin/staff` page (Phase 347)
- Building the cafe proxy (Phase 346)
- Any new API endpoints or Rust code
- Digital dashboards or PDF-serving infrastructure

</domain>

<decisions>
## Implementation Decisions

### D-01: Runbook Document Format
[auto] **Markdown source → printable HTML via CSS print stylesheet.** Three `.md` files in `docs/runbooks/` that can be rendered in any browser and printed to A4. No PDF toolchain dependency.

**Why:** No external tools needed. Browser print dialog handles A4 layout with `@media print` CSS. The markdown source stays in git — any future update is a git commit, not a file hunt. Avoids pandoc/wkhtmltopdf binary dependencies on the Windows venue machine.

**Three runbook files:**
1. `docs/runbooks/runbook-admin-broken.md` — OPS-15: "If admin is slow/broken: refresh → WhatsApp Bono → don't restart anything"
2. `docs/runbooks/runbook-staff-pin.md` — OPS-16: "How to change a staff PIN" — use `/admin/staff` page, NOT curl/sqlite3/scripts
3. `docs/runbooks/runbook-cafe-menu.md` — OPS-17: "How to change a cafe menu item" — open Admin → Cafe → edit → verify on POS within 10s

**Print path:** Open the `.md` file in VS Code → Export as HTML, OR use a one-command script: `node scripts/render-runbook.js docs/runbooks/runbook-admin-broken.md | Start-Process msedge.exe -ArgumentList @('--print-to-pdf')`

### D-02: Incident Log Medium
[auto] **Google Sheet (primary) + paper backup at POS.** Single shared Google Sheet with one tab, pre-formatted with columns: `Date | Time (IST) | Staff | What happened | What we did | Resolved Y/N`. Staff add rows in real time; James + Bono read the sheet each morning.

**Why:** Paper-only risks illegibility and loss. Google Sheet is visible to James + Bono without being physically present. Paper backup (a printout of the template) stays at POS for when wifi is slow — staff can fill paper, then transcribe later.

**OPS-18 specifics:**
- Google Sheet URL pinned in the `runbook-admin-broken.md` one-pager
- Sheet name: "Racing Point Incident Log"
- Share settings: anyone with link can edit (no Google sign-in required for staff)
- Pre-seed with one example row so staff see the expected format

**Note:** The Google Sheet URL must be created by Uday or James before Plan 353-02 (training session). The 353-01-PLAN must include a placeholder indicating where the URL goes.

### D-03: Morning Review Ritual (OPS-19)
[auto] **Comms-link WS message + calendar reminder.** Each morning before venue opens:
1. A cron-style check or scheduled task on James .27 sends a comms-link WS message to Bono: "Morning review — check incident log: [SHEET_URL]"
2. James reads the sheet, Bono reads the sheet (or James summarises in INBOX.md)
3. If incidents require follow-up, James opens a GSD issue or adds to LOGBOOK.md

**Why:** The ritual must be automatic — if it requires human initiative every day it won't happen. The comms-link WS push ensures James + Bono both get a nudge. No new infrastructure needed (comms-link already runs on James .27).

**Implementation:** A new static command `morning_review` added to `data/static-commands.json` in comms-link. A Windows Scheduled Task on James .27 triggers it at `08:00 IST` (= 02:30 UTC) daily. The task runs: `node send-message.js "Morning review: check incident log [SHEET_URL]"`.

### D-04: One-Pager Content Structure
[auto] **Single A4 page per runbook, big headings, minimal prose.** Each one-pager follows this template:

```
# [Title] — Racing Point
## When to use this: [1-line trigger]
## Step by step:
1. [Step] → [expected result]
2. [Step] → [expected result]
...
## If stuck: WhatsApp Bono @ bono@racingpoint.in
## DO NOT: [1-2 anti-patterns to avoid]
```

**Why:** Staff are not technical. They need: know when to use this → follow steps → know who to call. The "DO NOT" section prevents the most common error per runbook (e.g., "DO NOT restart the server", "DO NOT edit the PIN in the database").

**Specific content per runbook:**

`runbook-admin-broken.md` (OPS-15):
- Trigger: "Admin page loads slowly, shows error, or is blank"
- Steps: Refresh → wait 10s → try incognito → WhatsApp Bono
- DO NOT: restart the server, restart the PC, edit the database

`runbook-staff-pin.md` (OPS-16):
- Trigger: "Staff member forgot PIN or PIN needs to change"
- Steps: Open `http://192.168.31.23:3201/staff/manage` → find staff → click "Change PIN" → enter new 4-digit PIN → confirm → wait for "Verified on venue" message
- DO NOT: use sqlite3, use deploy-staging scripts, ask James to SSH

`runbook-cafe-menu.md` (OPS-17):
- Trigger: "Add, edit, or remove a cafe menu item"
- Steps: Open Admin → Cafe → edit item → save → open POS billing page → verify item appears within 10s
- DO NOT: edit menu_items.db directly, use any script

### D-05: Uday Sign-Off Mechanism
[auto] **WhatsApp confirmation from Uday.** After the training session (Plan 353-02), James sends Uday a WhatsApp message with the three one-pager URLs + asks: "Are you satisfied with these runbooks? Reply YES to confirm." The conversation screenshot is saved as `docs/runbooks/uday-signoff-2026-04-XX.png`.

**Why:** Uday is on WhatsApp, not GitHub. Screenshot evidence is sufficient. No need for digital signature infrastructure.

**OPS acceptance criteria #5:** "Uday signs off on the runbook content" — met by the WhatsApp screenshot.

### D-06: Phase Dependency Handling
[auto] **Write runbooks NOW, note feature-gated steps.** Since Phase 347 (staff page) and Phase 346 (cafe proxy) are code-complete but not deployed, the runbooks are written referencing the future feature state. Steps that require deployed Phase 347 are annotated with `[requires Phase 347 deploy]`. Physical printing happens AFTER those phases deploy.

**Why:** The runbook content is stable (the workflow doesn't change). Writing now means zero rework after deploy. Printing is deferred to the maintenance window when 347 + 346 ship.

### Claude's Discretion
- Exact CSS print stylesheet details (font size, margins, color scheme for print)
- Whether to use a shell script or node script for the one-time HTML render
- Column widths and conditional formatting in the Google Sheet template
- Whether the scheduled task for morning review is a schtask XML or a cron-style comms-link timer

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Dependencies (content referenced in runbooks)
- `.planning/phases/347-admin-staff-management/347-CONTEXT.md` — Exact URL (`/admin/staff/manage`) and workflow for PIN change modal (D-01..D-05). Runbook steps must match this exactly.
- `.planning/phases/346-cafe-menu-proxy/346-CONTEXT.md` — Cafe menu workflow and `CAFE_PROXY_ENABLED` state. Runbook steps for OPS-17 depend on Phase 346-02 cutover.

### Existing Docs (format reference)
- `docs/DEPLOY-RUNBOOK.md` — Existing runbook format used in the project (checklist + command tables style). One-pagers should match this visual style.
- `docs/DIAGNOSTIC-PLAYBOOK.md` — How current operational docs are structured.

### Requirements
- `.planning/REQUIREMENTS.md:101-105` — OPS-15..19 (the five requirements this phase closes)
- `.planning/ROADMAP.md:1139-1154` — Phase 353 goal and success criteria

### Infrastructure (morning review ritual)
- `comms-link/data/static-commands.json` — Where `morning_review` command will be added
- `comms-link/send-message.js` — Script used by the scheduled task to send WS push
- `~/.claude/comms-link.env` — PSK and URL (never commit)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `docs/DEPLOY-RUNBOOK.md` — Existing runbook in the repo. Same format, structure, and Markdown conventions should be used for the three new one-pagers.
- `comms-link/send-message.js` — Already used for James→Bono messaging. The morning review scheduled task reuses this directly.
- `comms-link/data/static-commands.json` — Static command registry. `morning_review` command goes here (fires `send-message.js` with the incident log URL).

### Established Patterns
- **Scheduled tasks on James .27:** comms-link is started via `start-comms-link.bat` + Task Scheduler. A new `MorningReview-Daily` task follows this pattern.
- **Git-tracked docs:** All operational docs live in `docs/`. New runbooks at `docs/runbooks/`.
- **IST time math:** Scheduled tasks on Windows use UTC internally. 08:00 IST = 02:30 UTC. Use `schtasks /Create /SC DAILY /ST 02:30`.

### Integration Points
- Phase 353 does not touch any Rust or TypeScript code.
- The `morning_review` comms-link command is the only semi-code artifact — a JSON entry in `static-commands.json`.

</code_context>

<specifics>
## Specific Requirements

### Physical Printing
- Runbooks must fit on a single A4 page each (one-sided)
- Print on laminated card or at least page-protected sleeve — POS environment gets splashes
- Place at POS station on a small stand or taped inside a visible cabinet door
- Uday to decide physical placement during training session

### Incident Log Pre-Population
- Google Sheet gets one seed example row: `2026-04-11 | 09:00 | [Staff Name] | Admin page showed 503 on kiosk tab | Waited 30s, refreshed, resolved itself | Y`
- This teaches the format without being confusing

### Morning Review Timing
- 08:00 IST = venue opens at ~10:00 IST — gives 2h buffer for James + Bono to review before customers arrive
- If incident log has new rows since previous review: James adds note to LOGBOOK.md (`| 2026-XX-XX HH:MM IST | James | review | N incidents since last review |`)
- If zero incidents: no LOGBOOK entry needed

### Sign-Off Timing
- Training session (Plan 353-02) happens AFTER runbooks are written and printed
- Sign-off conversation with Uday happens at end of training session
- Screenshot of WhatsApp confirmation saved to repo at `docs/runbooks/uday-signoff-YYYY-MM-DD.png`

</specifics>

<deferred>
## Deferred Ideas

- **Digital runbook portal** — a `/runbooks` page in the admin dashboard showing the three one-pagers — separate phase, adds infrastructure without clear need when print is sufficient
- **Automated incident log check** — reading the Google Sheet via API and alerting if rows go unreviewed for >24h — low priority, comms-link WS push is sufficient for now
- **Multi-language runbooks** (Hindi/English) — deferred until staff composition is known
- **Video walkthroughs** — screen recordings of PIN change + cafe edit workflows — nice-to-have, not OPS requirement

</deferred>

---

*Phase: 353-runbook-staff-training*
*Context gathered: 2026-04-11*
*Dependencies: 347 (code-complete, not deployed), 346 (code-complete, not deployed)*
*Note: Runbooks can be written now; printing deferred to post-deploy of 347 + 346*
