# partner-memory-read.js v0.2 — scope extension spec

**Status:** INSTALLED 2026-05-16T04:30:44Z · Captain HOOK-PATCH composite ratify · ledger entry at `~/.claude/state/harness-auth-ledger.jsonl` · install verified this session (parse-OK + behavior-fixtures-PASS) · source-tracked (NOT installed to `~/.claude/hooks/` until Captain named-surface auth)
**Author:** bono · 2026-05-16 ~09:43 IST
**Purpose:** Extend partner-memory-read SessionStart hook to surface (i) inbox-msg files, (ii) recent OpenRouter spend tail, (iii) partner git log since last bono session — closes 10h-stale-MEMORY.md gap that hid morning MAO from bono session-start
**Captain auth required for install:** named-surface auth on `~/.claude/hooks/partner-memory-read.js` (per Harness Self-Mod Auth Protocol)
**Empirical anchor:** §S-372 G9 #1 ("you missed") · 2026-05-15 ~05:25 IST · Captain corrected bilateral live-sync check at 11:11Z missing james MAO because partner-memory-read.js v0.1 reads ONLY `briefings/<partner>/memory/MEMORY.md` (mtime 06:22 IST = 10h stale at session start)

## Background

v0.1 reads exactly one path: `briefings/<partner>/memory/MEMORY.md`. This misses:

1. **Inbox-msg files:** `briefings/inbox-msg-<partner>-*.md` — partner-authored bilateral messages that arrive between MEMORY.md updates
2. **OpenRouter spend log:** `data/openrouter-spend-<partner>.jsonl` tail — partner's recent MMA spend reveals what they investigated
3. **Partner git activity:** `git log --author=<partner-github-handle> --since "$LAST_BONO_SESSION" --name-only` for new `.planning/` artifacts (MAOs, RCAs, design contracts)

## v0.2 scope extension

After current v0.1 MEMORY.md output, append 3 NEW sections:

### Section A — Inbox-msg-since-cursor list

```javascript
// New scope: surface inbox-msg-<partner>-*.md files modified since last bono session
const INBOX_MSG_DIR = path.join(COMMS_LINK, 'briefings');
const cursorFile = '/root/.claude/state/partner-memory-read-cursor.txt';
let lastSessionTs = 0;
try {
  lastSessionTs = parseInt(fs.readFileSync(cursorFile, 'utf8').trim(), 10) || 0;
} catch { /* first run */ }

if (fs.existsSync(INBOX_MSG_DIR)) {
  const matchPattern = new RegExp(`^inbox-msg-${PARTNER}-.*\\.md$`);
  const files = fs.readdirSync(INBOX_MSG_DIR)
    .filter(f => matchPattern.test(f))
    .map(f => {
      const fp = path.join(INBOX_MSG_DIR, f);
      const st = fs.statSync(fp);
      return { name: f, mtime: st.mtimeMs, size: st.size };
    })
    .filter(x => x.mtime > lastSessionTs)
    .sort((a, b) => b.mtime - a.mtime)
    .slice(0, 10);
  if (files.length > 0) {
    console.log(`\n--- INBOX-MSG-${PARTNER.toUpperCase()} since last session (${files.length} new) ---`);
    files.forEach(f => console.log(`  ${f.name} (${f.size}B, ${new Date(f.mtime).toISOString()})`));
  }
}
// Update cursor for next run
try { fs.writeFileSync(cursorFile, String(Date.now())); } catch { /* fail-quiet */ }
```

### Section B — OpenRouter spend tail

```javascript
const SPEND_LOG = path.join(COMMS_LINK, 'data', `openrouter-spend-${PARTNER}.jsonl`);
if (fs.existsSync(SPEND_LOG)) {
  const tail5 = run(`tail -5 ${SPEND_LOG}`, COMMS_LINK);
  if (tail5) {
    console.log(`\n--- OPENROUTER-SPEND-${PARTNER.toUpperCase()} last 5 entries ---`);
    console.log(tail5);
  }
}
```

### Section C — Partner git log since cursor

```javascript
const partnerGhHandle = PARTNER === 'james' ? 'james-racingpoint' : 'bono-bot';
const RACECONTROL = IS_WINDOWS
  ? path.join(os.homedir(), 'racingpoint', 'racecontrol')
  : '/root/racecontrol';
const since = lastSessionTs ? new Date(lastSessionTs).toISOString() : '24 hours ago';
for (const repo of [COMMS_LINK, RACECONTROL]) {
  if (fs.existsSync(repo)) {
    const log = run(
      `git log --author="${partnerGhHandle}" --since="${since}" --name-only --pretty=format:"%h %s"`,
      repo
    );
    if (log && log.trim().length > 0) {
      console.log(`\n--- PARTNER GIT LOG ${path.basename(repo)} since ${since} ---`);
      console.log(log);
    }
  }
}
```

## Composes-with

- §S-372 G9 #1 anchor · `feedback_capability_claim_without_probe_20260514.md` parent rule (probe partner state before claiming bilateral sync) · existing `partner-memory-read.js` v0.1 baseline · `~/.claude/state/partner-memory-read-cursor.txt` (NEW state file · session-cursor)

## Install procedure (when Captain authorizes)

Captain verb required: `"I authorize HOOK-PATCH for hooks: partner-memory-read.js v0.2 scope extension"`

Steps:
1. Read current `~/.claude/hooks/partner-memory-read.js` v0.1
2. After existing MEMORY.md output block, insert Section A + B + C code
3. Add header version comment v0.1 → v0.2
4. Touch cursor file `/root/.claude/state/partner-memory-read-cursor.txt` (empty initially — first run will write current ts)
5. Append HARNESS-AUTH-CLAIM ledger entry
6. Validate via `node --check`
7. Smoke-test by triggering session-start (or run hook directly via Node and verify 3 new sections appear)

## Test scenarios (post-install verification)

| Scenario | Expected output |
|---|---|
| No new inbox-msg files since cursor | Section A omitted |
| 2 new inbox-msg files | Section A lists 2 files with mtime + size |
| openrouter-spend-james.jsonl empty | Section B omitted |
| openrouter-spend-james.jsonl has entries | Section B prints last 5 |
| james has 0 commits since cursor | Section C omitted per repo |
| james has commits in racecontrol since cursor | Section C lists hash + subject + files |

## Idempotency

- Cursor file is atomically updated at end of each run · stale cursor degrades gracefully (just shows more history)
- All output is advisory (exit 0 unconditionally) · matches v0.1 behavior
- If cursor file is missing, defaults to "24 hours ago" SQL-style relative time
