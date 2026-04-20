# Bono — Workflow Monitor Duty

**Owner:** Bono (Peter Bonnington), AI on VPS (srv1422716.hstgr.cloud).
**Status:** Day 1 partial-live (James shipped the generators + checker in
`3b006fea`; Bono picks up the pm2 monitor + AI-review layer).
**Context:** see `project_workflow_plan_day1.md` in James's memory for
the chess-analogy plan and the 9 open DPDP gaps the checker has surfaced.

---

## What this file is

James runs the board-state generators and checkers on his workstation as
a commit-time gate. That's necessary but not sufficient — it only fires
when a human opens a terminal. The fleet drifts in the background (the
graphify MCP stale-post-merge class, the SWAPLOG-vs-HEAD divergence, cron
jobs adding tables to live DBs without matching migrations landing in
git). We need a pulse that runs whether or not James is at his keyboard.

Bono's VPS runs pm2 and doesn't sleep. That makes the VPS the right host
for the background monitor. Bono-the-AI is also the right reviewer for
cross-system alerts because (a) Perplexity MCP has different blind spots
than James's OpenRouter pool, and (b) the VPS has read access to both
venue and cloud state simultaneously.

This document is the duty list Bono should take on. It is NOT a
checklist James reviews — James wrote it as a proposal; Bono should edit
it in-place if the approach needs to change.

---

## Duty 1 — pm2 workflow-monitor daemon

### Registration

On VPS, from `/root/racecontrol`:

```bash
# Pull the latest generators + checker + orchestrator.
git pull

# Install workflow-monitor as a pm2 process if not already registered.
# The process lives at scripts/monitor/workflow-monitor.js (to be written
# by Bono — see "Implementation sketch" below).
pm2 start scripts/monitor/workflow-monitor.js \
  --name workflow-monitor \
  --cron '*/5 * * * *' \
  --no-autorestart
pm2 save
```

Cron cadence: every 5 min. A full run currently takes <2s (Python + jq);
even with 10 surfaces it should stay under 30s. If runtime grows past
60s, lower the cadence to 10 min — the alternative is overlapping runs
fighting over `board-state.json`.

### What the daemon does each tick

1. `git fetch && git reset --hard origin/main` in both `/root/racecontrol`
   AND `/root/comms-link`.
   _Why both:_ checkers need comms-link contract definitions too, and the
   graphify stale-post-merge class means Bono's local clone drifts.
2. `bash scripts/audit/run-all-checkers.sh` in `/root/racecontrol`.
3. Read `.planning/board-state/status.json`. Compare to previous tick's
   snapshot saved at `/root/racecontrol/.planning/board-state/status-prev.json`.
4. If `overall_exit` differs (red↔green transition) OR a new check
   entered red state:
   - Append one line to `/root/comms-link/INBOX.md` via
     `node scripts/inbox-append.js --from bono` describing the
     transition (commit, check name, gap count).
   - Send a WS event `workflow_alert` through the comms-link relay to
     James.
   - If red→green and the triggering commit was James's, post a thumbs-up
     to the admin dashboard `/workflow-status` tile (to be added, Duty 4).
5. Overwrite `status-prev.json` with the new status.
6. Commit `.planning/board-state/*.json` to git with author "bono" if
   content changed. Git co-change history then carries the check
   transitions as first-class commits.

### Failure modes to handle

- **Generator parse error** (`overall_exit=2`): don't overwrite
  `status-prev.json` — the previous good state is the truth. Alert
  James with the stderr so he can fix the migration parser.
- **git pull conflict**: log + skip this tick. Don't auto-resolve.
- **Simultaneous James push**: pm2 cron + James push race is fine —
  next tick just has a newer commit. Status file is the tiebreaker.
- **Perplexity API budget exhausted**: skip MCP review steps; keep the
  generator + checker pipeline running unaltered.

### Implementation sketch

`scripts/monitor/workflow-monitor.js` — ~80 lines Node.js:

```javascript
const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = process.env.RC_ROOT || '/root/racecontrol';
const STATUS_PATH = path.join(ROOT, '.planning/board-state/status.json');
const PREV_PATH = path.join(ROOT, '.planning/board-state/status-prev.json');

function tick() {
  try {
    execSync('git fetch && git reset --hard origin/main', { cwd: ROOT, stdio: 'pipe' });
    execSync('git fetch && git reset --hard origin/main', { cwd: '/root/comms-link', stdio: 'pipe' });
    execSync('bash scripts/audit/run-all-checkers.sh', { cwd: ROOT, stdio: 'pipe' });
  } catch (e) {
    // Generator or git failure — log + bail this tick.
    console.error(`[workflow-monitor] tick aborted:`, e.message);
    return;
  }

  const current = JSON.parse(fs.readFileSync(STATUS_PATH, 'utf8'));
  let previous = null;
  if (fs.existsSync(PREV_PATH)) previous = JSON.parse(fs.readFileSync(PREV_PATH, 'utf8'));

  const transitions = diffChecks(previous, current);
  if (transitions.length) {
    for (const t of transitions) announce(t, current.commit);
  }
  fs.copyFileSync(STATUS_PATH, PREV_PATH);
}

// diffChecks returns [{name, from, to, gaps}] for each check whose status changed.
// announce → comms-link relay WS + INBOX append + optional Perplexity MCP review.

tick();
setInterval(tick, 5 * 60 * 1000);
```

---

## Duty 2 — Perplexity MCP second-opinion

When a new red check appears, query ONE specialist (not all four — cost
cap $1/hr):

| Red check class | Perplexity model | Prompt template |
|---|---|---|
| DPDP / auth / sql-injection | pplx_gpt54_thinking | "Review this SQL/auth change for compliance gaps: ..." |
| Cascade / recovery / deploy | pplx_claude_sonnet | "Review this change for cascade failure modes: ..." |
| Type drift / integration | pplx_gemini_pro_think | "Review this cross-boundary change for drift: ..." |
| Windows / hardware | pplx_nemotron_thinking | "Review this pod/Windows change for driver/HID issues: ..." |

Budget: source_focus:"none", max 3 queries per red transition, log to
`/root/.claude/projects/-root/memory/mma-log.jsonl`. Skip if budget for
the day is exhausted.

Append the specialist's response to the INBOX alert so James sees both.

---

## Duty 3 — MMA Step 4 VERIFY on James's push

When James pushes a commit touching:
- `crates/racecontrol/src/api/customer_legal.rs` (DPDP)
- `crates/rc-agent/src/main.rs` or `ws_handler.rs` (WS protocol)
- `crates/racecontrol/src/ws/*.rs` (server WS)
- Any cross-system bridge (kiosk ↔ rc-agent ↔ server)

Bono runs the VERIFY step of the MMA protocol: 3 adversarial models
(different from whatever James used at fix time). Log to LOGBOOK with
`| bono-mma | $commit | VERIFY | 3 models | score |`. If score < 4.0,
post RED alert to James — he must address before merge.

This replaces today's ad-hoc "Bono reviews when they get to it" with a
hook-triggered gate.

---

## Duty 4 — Admin dashboard tile

Add `/workflow-status` to the admin dashboard (:3201). Polls
`GET /api/v1/health` (which now includes the overall_exit from
status.json — backend change James needs to make). Shows a single
traffic light + the list of red checks with links to the offending
files. On red→green transition, flashes green for 10s.

This is the least-important duty of the four — the INBOX + WS pipe
already covers the "got a human's attention" channel. Dashboard is for
passive awareness.

---

## What James does

1. Extend the generator surfaces (types, protocol, http, fleet). Day 2+.
2. Close the 9 open DPDP gaps so the checker goes green. Validates the
   red→green pipeline end-to-end.
3. Write pre-commit hook that runs the orchestrator on touched surfaces
   only (fast path; Bono runs the full battery authoritatively).
4. Add `overall_exit` to `/api/v1/health` response so Duty 4 dashboard
   tile has something to read.

---

## Why this split works

- Bono's VPS never sleeps → catches drift James misses by not being at
  the keyboard.
- Bono-the-AI has different reasoning mode (Perplexity's search-grounded
  vs. James's OpenRouter model pool) → catches blind spots.
- Comms-link WS already carries low-latency events between the two.
- PM2 is already running 20+ processes; adding one more daemon is free.
- Everything generator/checker-side is **source-of-truth driven** — no
  live DB probes in the hot path, so the monitor works even when venue
  is off.

## Where to push back

If Bono thinks any of the 4 duties is too expensive or steps on existing
work, edit this doc in-place with reasoning. The 6-phase plan (see
James's memory `project_workflow_plan_day1.md`) is the contract; the
split of work between James and Bono inside it is negotiable.
