# Phase 93: Community & Tribal Identity - Research

**Researched:** 2026-03-21
**Domain:** Discord bot automation, copy identity, track-record event propagation
**Confidence:** HIGH

## Summary

Phase 93 has four tightly-scoped requirements: weekly leaderboard posts to Discord via node-cron (COMM-01), near-real-time track record announcements on Discord within 1 hour of a record being set (COMM-02), a copy sweep replacing "customer" with "RacingPoint Driver" in PWA, WhatsApp, and Discord bot prompts (COMM-03), and weekly ritual posts for time trials and tournament brackets (COMM-04).

The Discord bot already has the `announce.js` command showing how EmbedBuilder embeds work, a `ready.js` event with auto-channel creation (exposing a `channels` export), and a `guildMemberAdd.js` event that shows the pattern for finding and posting to channels. The bot does NOT yet have node-cron installed or any scheduler service. Adding the scheduler means installing node-cron, creating `src/services/scheduler.js`, and initializing it in `src/events/ready.js` after channels are resolved.

For track record alerts (COMM-02), the Rust `persist_lap()` function in `lap_tracker.rs` already returns a bool (`is_record`) but the caller in `ws/mod.rs` ignores this return value. The bot cannot receive a push from RaceControl directly — the cleanest approach is a polling loop in the Discord bot scheduler that calls the existing `GET /api/v1/bot/leaderboard` (no track filter) every 15 minutes and compares against a persisted "last seen record" map. When a new record appears, the bot posts an embed. The alternative — adding a `DashboardEvent::TrackRecordSet` and a Discord webhook call in Rust — is heavier, requires a Rust rebuild, and couples the cloud server to a Discord channel ID.

**Primary recommendation:** Add `node-cron` to the Discord bot. Create `src/services/scheduler.js` that (1) polls `/api/v1/bot/leaderboard` every 15 min for new records, (2) posts a weekly leaderboard summary every Monday 09:00 IST, and (3) posts a weekly time trial challenge + tournament bracket update every Monday 09:05 IST. The bot connects to RaceControl using the existing `x-terminal-secret` header pattern from the WhatsApp bot's `racecontrolService.js`.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| COMM-01 | Discord bot posts weekly leaderboard summary automatically (node-cron) | node-cron v4.2.1 available, bot needs new scheduler.js + node-cron install |
| COMM-02 | Discord bot announces new track records within 1 hour of being set | Polling `/bot/leaderboard` every 15 min is simplest; no Rust changes needed |
| COMM-03 | All customer-facing copy uses "RacingPoint Driver" instead of "customer" | Scope mapped: WhatsApp systemPrompt.js (14 occurrences), Discord systemPrompt.js (3), PWA minimal (1 visible string) |
| COMM-04 | Discord community rituals: weekly time trial challenge post, tournament bracket updates | `/public/time-trial` and `/api/v1/tournaments` (authenticated) already exist |
</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| node-cron | 4.2.1 | Cron-style task scheduling in Node.js | Zero dependencies, ISC license, actively maintained, requirement specifies it by name |
| discord.js | 14.16.0 (already installed) | Discord API client | Already in use |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| node-fetch (native) | Node 18+ built-in | HTTP calls to RaceControl API | Bot already uses global `fetch` in `drive.js` — no extra install needed |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| node-cron polling for records | Rust `DashboardEvent::TrackRecordSet` + Discord webhook | Rust approach requires rebuild + deploy to venue, adds webhook_url to cloud config, tighter coupling. Polling is simpler and good enough for "within 1 hour". |
| Polling every 15 min | Polling every 5 min | 5 min = 12 API calls/hr vs 4; leaderboard endpoint is lightweight but no need to burn calls |
| `bot/leaderboard` for records | `public/leaderboard` (no auth) | `public/leaderboard` also works but `bot/leaderboard` is already authenticated and returns a cleaner top-10 format |

**Installation:**
```bash
cd /root/racingpoint-discord-bot && npm install node-cron
```

**Version verification:** node-cron latest is 4.2.1 (verified via `npm view node-cron version` = `4.2.1`).

---

## Architecture Patterns

### Discord Bot Service Structure
```
src/
├── events/
│   ├── ready.js         # Init: channels resolved here — scheduler MUST start here
│   ├── guildMemberAdd.js
│   └── interactionCreate.js
├── services/
│   ├── scheduler.js     # NEW: node-cron tasks for weekly posts + record polling
│   ├── racecontrolService.js  # NEW (copy WhatsApp bot pattern)
│   ├── bookingService.js
│   └── claudeService.js
└── commands/
    └── ...
```

### Pattern 1: Scheduler Initialization in ready.js
**What:** Start all cron tasks inside the `ClientReady` event after channels are resolved, passing the `client` object.
**When to use:** Always — ensures the Discord client is ready and channel IDs are known before scheduling runs.

```javascript
// Source: discord.js Events.ClientReady pattern (already in src/events/ready.js)
// In ready.js, after channels are resolved:
const { startScheduler } = require('../services/scheduler');
startScheduler(client, channels);
```

### Pattern 2: node-cron v4 Schedule with IST Timezone
**What:** Use `cron.schedule(expr, fn, { timezone: 'Asia/Kolkata' })`.
**When to use:** All scheduled posts — RacingPoint operates in IST.

```javascript
// Source: node-cron v4 official README (github.com/node-cron/node-cron)
const cron = require('node-cron');

// Every Monday at 09:00 IST
cron.schedule('0 9 * * 1', async () => {
  await postWeeklyLeaderboard(client, channels);
}, { timezone: 'Asia/Kolkata' });
```

### Pattern 3: RaceControl API Calls from Discord Bot (matching WhatsApp bot pattern)
**What:** Use `fetch` with `x-terminal-secret` header. Config from `.env` via `config.js`.
**When to use:** Any Discord bot → RaceControl API call.

```javascript
// Source: /root/racingpoint-whatsapp-bot/src/services/racecontrolService.js
const RC_API_URL = process.env.RC_API_URL || 'https://app.racingpoint.cloud/api/v1';
const RC_SECRET = process.env.RC_TERMINAL_SECRET;

const HEADERS = {
  'Content-Type': 'application/json',
  'x-terminal-secret': RC_SECRET,
};

async function getBotLeaderboard() {
  const res = await fetch(`${RC_API_URL}/bot/leaderboard`, { headers: HEADERS });
  return res.json(); // { entries: [{position, driver, track, time_ms, time_formatted, car}], count }
}
```

### Pattern 4: Track Record Polling with State File
**What:** Persist last-seen record map to a JSON file in `data/` so the bot survives restarts without re-announcing old records.
**When to use:** COMM-02 requirement — must not re-announce records on bot restart.

```javascript
// State file: /root/racingpoint-discord-bot/data/record_state.json
// Shape: { "[track]|[car]": { best_lap_ms, driver, announced_at } }

const STATE_FILE = path.join(__dirname, '../../data/record_state.json');

function loadState() {
  try {
    return JSON.parse(fs.readFileSync(STATE_FILE, 'utf8'));
  } catch {
    return {};
  }
}

function saveState(state) {
  fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
}
```

### Pattern 5: Discord EmbedBuilder (existing pattern)
**What:** Use `EmbedBuilder` with `.setColor(0xe10600)` (Racing Red `#E10600`), `.setTitle()`, `.setDescription()`, `.setTimestamp()`.
**When to use:** All automated posts. Note: existing commands use `0xe74c3c` — switch to brand red `0xe10600`.

```javascript
// Source: /root/racingpoint-discord-bot/src/commands/announce.js + CLAUDE.md brand colors
const { EmbedBuilder } = require('discord.js');

const embed = new EmbedBuilder()
  .setColor(0xe10600)  // Racing Red — #E10600 per CLAUDE.md
  .setTitle('Weekly Leaderboard — RacingPoint Drivers')
  .setDescription(leaderboardText)
  .setFooter({ text: 'RacingPoint eSports and Cafe — Hyderabad' })
  .setTimestamp();

const ch = client.channels.cache.get(channelId);
await ch.send({ embeds: [embed] });
```

### Pattern 6: Finding/Creating the Announcements Channel
**What:** Follow the same pattern as `ready.js` auto-creates `#ask-racing-point` and `#welcome` channels. Add a `#leaderboard` channel (or `#announcements`) similarly.
**When to use:** COMM-01, COMM-04 weekly posts.

```javascript
// Same pattern as existing ready.js channel creation
let leaderboardChannel = guild.channels.cache.find(
  ch => ch.name === 'leaderboard' && ch.type === ChannelType.GuildText
);
if (!leaderboardChannel) {
  leaderboardChannel = await guild.channels.create({
    name: 'leaderboard',
    type: ChannelType.GuildText,
    topic: 'Weekly leaderboard summaries, track records, and time trial challenges.',
  });
}
channels.leaderboard = leaderboardChannel?.id || null;
```

### Anti-Patterns to Avoid
- **Starting scheduler before `ClientReady`:** The `client.channels.cache` is empty before ready fires — scheduler must start inside the ready event handler, after channels are resolved.
- **Using `setInterval` instead of node-cron:** setInterval doesn't survive PM2 restarts cleanly and has no timezone awareness. Use node-cron.
- **Hardcoding channel IDs:** Use the `channels` object from `ready.js` — channel IDs are dynamically resolved at bot startup.
- **Re-announcing old records on restart:** Always load state from `data/record_state.json` before checking records.
- **Using `0xe74c3c` (wrong red):** Existing commands use this old red. New automated posts must use `0xe10600` (Racing Red from CLAUDE.md).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cron scheduling | Custom `setInterval` chains | node-cron 4.2.1 | Handles timezone, overlaps, restart safety |
| Time formatting | Custom ms→lap time | Already in API response as `time_formatted` field | `bot/leaderboard` returns `"1:23.456"` format directly |
| Record state persistence | In-memory only | JSON file in `data/` | Survives bot PM2 restarts without re-announcing |

**Key insight:** The RaceControl `bot/leaderboard` endpoint already returns formatted lap times (`time_formatted: "1:23.456"`). Do not re-implement time formatting in the Discord bot.

---

## Common Pitfalls

### Pitfall 1: Scheduler Starts Before Channels Are Resolved
**What goes wrong:** `client.channels.cache.get(channelId)` returns `undefined` — embed posts silently fail.
**Why it happens:** node-cron tasks initialized at module load time run before the `ClientReady` event.
**How to avoid:** Call `startScheduler(client, channels)` at the END of the `ready.js` `execute()` function, after channel IDs are assigned.
**Warning signs:** Cron tasks run (logs show) but no messages appear in Discord.

### Pitfall 2: Re-Announcing Records After Bot Restart
**What goes wrong:** Bot restarts (PM2 auto-restart), loads all current track records, and spams "new record" announcements for records set weeks ago.
**Why it happens:** In-memory record state is lost on restart.
**How to avoid:** Persist state to `data/record_state.json`. On startup, populate state map from file before first poll runs.
**Warning signs:** Burst of record announcements immediately after PM2 restart.

### Pitfall 3: Missing `RC_API_URL` or `RC_TERMINAL_SECRET` in Discord Bot .env
**What goes wrong:** Fetch to RaceControl fails with 401 Unauthorized or connection refused.
**Why it happens:** The Discord bot `.env` currently has no `RC_API_URL` or `RC_TERMINAL_SECRET` keys — only the WhatsApp bot uses these.
**How to avoid:** Add `RC_API_URL=https://app.racingpoint.cloud/api/v1` and `RC_TERMINAL_SECRET=rp-terminal-2026` to `/root/racingpoint-discord-bot/.env`.
**Warning signs:** `racecontrolService.js` logs "RC lookup failed" with 401 or network error.

### Pitfall 4: node-cron v4 Breaking Changes from v3
**What goes wrong:** Code copied from v3 examples uses `{ scheduled: true }` option or `.start()` method — in v4, tasks start immediately by default and `scheduled`/`runOnInit` options no longer exist.
**Why it happens:** Most online examples and Stack Overflow answers show v3 API.
**How to avoid:** Use v4 API: `cron.schedule(expr, fn, { timezone: 'Asia/Kolkata' })` — no `.start()` call needed.
**Warning signs:** TypeError about unknown options, or tasks not starting.

### Pitfall 5: Copy Changes Break Bot Instruction Logic
**What goes wrong:** Replacing "customer" in systemPrompt.js changes bot instructions like "Match the customer's energy" — making the bot refer to itself and its reasoning in third person.
**Why it happens:** "customer" appears both as user-facing copy AND as bot instruction language about how to behave.
**How to avoid:** Only replace "customer" in user-facing output strings (what the bot SAYS), not in instruction clauses (how the bot THINKS). Example: "Match the customer's energy" stays as-is internally, but "If a happy customer finishes a conversation" → "If a happy driver finishes a conversation".
**Warning signs:** Bot starts behaving oddly or referring to the person as "the driver" in contexts where it sounds robotic.

### Pitfall 6: No Active Time Trial — Null Guard Required
**What goes wrong:** `GET /public/time-trial` returns `{ time_trial: null, message: "No active time trial this week" }` when no trial is configured. Posting a weekly challenge embed that says "null" looks broken.
**Why it happens:** The endpoint returns null when no `time_trials` row spans the current week.
**How to avoid:** Check `data.time_trial !== null` before posting. If null, post a generic "No time trial this week — stay tuned!" message, or skip the post entirely.

---

## Code Examples

### Weekly Leaderboard Summary — Full Pattern
```javascript
// Source: /root/racecontrol/crates/racecontrol/src/api/routes.rs:11919 (bot_leaderboard)
// Response shape: { entries: [{position, driver, track, time_ms, time_formatted, car}], count }

async function postWeeklyLeaderboard(client, channels) {
  const channelId = channels.leaderboard;
  if (!channelId) return;

  const res = await fetch(`${RC_API_URL}/bot/leaderboard`, { headers: RC_HEADERS });
  const data = await res.json();

  const lines = (data.entries || []).map(e =>
    `**${e.position}.** ${e.driver} — ${e.time_formatted} on ${e.track} (${e.car})`
  );

  const embed = new EmbedBuilder()
    .setColor(0xe10600)
    .setTitle('Weekly Leaderboard — Top RacingPoint Drivers')
    .setDescription(lines.join('\n') || 'No entries yet this week.')
    .setFooter({ text: 'RacingPoint eSports and Cafe — Hyderabad' })
    .setTimestamp();

  const ch = client.channels.cache.get(channelId);
  await ch?.send({ embeds: [embed] });
}
```

### Track Record Polling — Full Pattern
```javascript
// Source: route /api/v1/bot/leaderboard (routes.rs:11919-11965)
// Runs every 15 minutes via node-cron

async function checkTrackRecords(client, channels, state) {
  const res = await fetch(`${RC_API_URL}/bot/leaderboard`, { headers: RC_HEADERS });
  const data = await res.json();

  for (const entry of (data.entries || [])) {
    const key = `${entry.track}|${entry.car}`;
    const prev = state[key];

    const isNew = !prev || entry.time_ms < prev.best_lap_ms;
    if (isNew) {
      state[key] = { best_lap_ms: entry.time_ms, driver: entry.driver };
      saveState(state);

      if (prev) { // Only announce if there was a previous record (not first-time)
        const embed = new EmbedBuilder()
          .setColor(0xe10600)
          .setTitle('New Track Record!')
          .setDescription(
            `**${entry.driver}** just set a new track record!\n\n` +
            `Track: **${entry.track}**\nCar: **${entry.car}**\nTime: **${entry.time_formatted}**`
          )
          .setFooter({ text: 'RacingPoint eSports and Cafe — Hyderabad' })
          .setTimestamp();

        const ch = client.channels.cache.get(channels.leaderboard);
        await ch?.send({ embeds: [embed] });
      }
    }
  }
}
```

### node-cron v4 Schedule Setup
```javascript
// Source: node-cron v4 README (github.com/node-cron/node-cron)
const cron = require('node-cron');

function startScheduler(client, channels) {
  const state = loadState();

  // COMM-01: Weekly leaderboard — Every Monday at 09:00 IST
  cron.schedule('0 9 * * 1', async () => {
    await postWeeklyLeaderboard(client, channels);
  }, { timezone: 'Asia/Kolkata' });

  // COMM-02: Track record polling — every 15 minutes
  cron.schedule('*/15 * * * *', async () => {
    await checkTrackRecords(client, channels, state);
  }, { timezone: 'Asia/Kolkata' });

  // COMM-04: Time trial challenge + tournament updates — Every Monday at 09:05 IST
  cron.schedule('5 9 * * 1', async () => {
    await postWeeklyTimeTrial(client, channels);
    await postTournamentUpdates(client, channels);
  }, { timezone: 'Asia/Kolkata' });
}
```

### Discord Bot config.js Addition
```javascript
// Add to /root/racingpoint-discord-bot/src/config.js
racecontrol: {
  apiUrl: process.env.RC_API_URL || 'https://app.racingpoint.cloud/api/v1',
  terminalSecret: process.env.RC_TERMINAL_SECRET,
},
```

### COMM-03 Copy Audit Results

**Files requiring changes:**

1. `/root/racingpoint-whatsapp-bot/src/prompts/systemPrompt.js`
   - Line 9: "Match the customer's energy" → "Match the driver's energy"
   - Line 15: "Respond in the SAME LANGUAGE the customer writes in" → "...the driver writes in"
   - Line 21: "Read the customer's intent" → "Read the driver's intent"
   - Line 49: "### Regular customer asking" → "### Regular driver asking"
   - Line 59: "If a happy customer finishes" → "If a happy driver finishes"
   - Line 125: "When a customer wants to book" → "When a driver wants to book"
   - Line 136: "If the customer says" → "If the driver says"
   - Line 145: "Ask the customer to confirm" → "Ask the driver to confirm"
   - Line 161: "When a customer wants to book a package" → "When a driver wants to book a package"
   - Line 167: "When a new customer wants to book" → "When a new driver wants to book"
   - Line 176-177: "If the customer is under 12"/"If the customer is 12-17" → "If the driver is under 12"/"If the driver is 12-17"
   - Line 181: "If a customer wants to book but" → "If a driver wants to book but"

2. `/root/racingpoint-whatsapp-bot/src/prompts/businessKnowledge.js`
   - Line 30: "If a customer requests a specific game" → "If a driver requests a specific game"
   - Line 68: "Every registered customer gets a unique referral code" → "Every registered driver gets a unique referral code"

3. `/root/racingpoint-discord-bot/src/prompts/systemPrompt.js`
   - Line 12: "Respond in the SAME LANGUAGE the customer writes in" → "...the driver writes in"
   - Line 46: "### 'What's new' / Returning customer" → "### 'What's new' / Returning driver"

4. `/root/racecontrol/pwa/src/app/register/page.tsx`
   - Line 55: "Guardian name is required for customers under 18" → "...for drivers under 18"

5. `/root/racecontrol/pwa/src/app/dashboard/page.tsx`
   - Line 64: "Welcome back" → "Welcome back, Driver" (or add identity reinforcement)

**Note:** API endpoint URL strings like `/customer/wallet` are NOT visible to users — do NOT change those.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual announce command | node-cron automated weekly posts | This phase | Removes human dependency for community rituals |
| No track record Discord alerts | Polling-based record detection | This phase | Community sees records within 15 min |
| "customer" language | "RacingPoint Driver" identity | This phase | Tribal identity reinforcement |
| node-cron v3 `{ scheduled: true }` | node-cron v4 auto-start (no option needed) | May 2025 | Must use v4 API |

---

## Open Questions

1. **Which Discord channel(s) for automated posts?**
   - What we know: `#ask-racing-point` and `#welcome` exist and are auto-created. No `#leaderboard` or `#announcements` channel exists yet.
   - What's unclear: Should COMM-01/02/04 posts all go to one channel or separate channels?
   - Recommendation: Auto-create a single `#leaderboard` channel (like existing pattern in ready.js). All automated community posts go there.

2. **Tournament bracket update content for COMM-04**
   - What we know: `GET /api/v1/tournaments` returns all tournaments with status field. Bracket match results need auth (`/api/v1/tournaments/{id}/matches`).
   - What's unclear: "bracket update" for weekly post — just list active tournaments or show match results?
   - Recommendation: List active tournaments with registration status and upcoming event dates. Full bracket detail requires multiple API calls and is complex for an embed — keep it simple.

3. **Track record polling frequency and 1-hour SLA**
   - What we know: COMM-02 requires "within 1 hour". 15-min polling meets this.
   - What's unclear: During peak hours, records could be set rapidly — is spam a concern?
   - Recommendation: Add a per-key 24-hour cooldown so the same track+car record only gets announced once per day even if multiple drivers beat it.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | None — Discord bot has no test framework installed |
| Config file | none — Wave 0 gap |
| Quick run command | `node src/index.js` (smoke: check bot logs) |
| Full suite command | Manual smoke test: confirm cron tasks log at startup |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| COMM-01 | Weekly leaderboard cron fires at 09:00 Monday IST | manual smoke | Trigger manually: call postWeeklyLeaderboard() directly in test script | ❌ Wave 0 |
| COMM-02 | Record announced within 15 min of being set | manual smoke | Set a dummy record via API, wait for next poll cycle | ❌ Wave 0 |
| COMM-03 | No "customer" in user-facing output strings | grep verification | `grep -rn "customer" src/prompts/` (should return 0 after changes) | ✅ (grep) |
| COMM-04 | Weekly time trial + tournament posts fire | manual smoke | Call postWeeklyTimeTrial() directly | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `pm2 logs racingpoint-discord-bot --lines 50` — verify no startup errors
- **Per wave merge:** Manual smoke: confirm at least one embed posts to `#leaderboard`
- **Phase gate:** All 4 cron task functions callable without error, copy grep shows zero "customer" in prompt files

### Wave 0 Gaps
- [ ] `tests/scheduler.test.js` — unit tests for `postWeeklyLeaderboard`, `checkTrackRecords`
- [ ] Framework install: `npm install --save-dev jest` if unit tests are required

*(No test framework currently exists in the Discord bot. The planner should decide whether to add jest or keep manual smoke tests only.)*

---

## Sources

### Primary (HIGH confidence)
- Codebase: `/root/racingpoint-discord-bot/src/` — Direct inspection of bot structure, channels, events
- Codebase: `/root/racecontrol/crates/racecontrol/src/lap_tracker.rs` — persist_lap() returns is_record bool, ignored in ws/mod.rs:313
- Codebase: `/root/racecontrol/crates/racecontrol/src/api/routes.rs:11919` — bot_leaderboard response shape verified
- Codebase: `/root/racingpoint-whatsapp-bot/src/services/racecontrolService.js` — x-terminal-secret header pattern
- `npm view node-cron version` — Confirmed 4.2.1 is current latest

### Secondary (MEDIUM confidence)
- [node-cron GitHub README](https://github.com/node-cron/node-cron) — v4 API: schedule(expr, fn, {timezone}) verified
- [node-cron npm page](https://www.npmjs.com/package/node-cron) — ISC license, zero dependencies confirmed

### Tertiary (LOW confidence)
- WebSearch results on node-cron v3→v4 migration — `scheduled` and `runOnInit` options removed in v4 (unverified via official changelog, but consistent across multiple sources)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — node-cron version verified via npm registry, Discord.js already installed
- Architecture: HIGH — All patterns derived from direct codebase inspection
- Pitfalls: HIGH — Pitfalls 1-3 derived from actual code inspection; Pitfall 4 from WebSearch (MEDIUM for that item alone)
- Copy audit (COMM-03): HIGH — direct grep of all three repos, line numbers confirmed

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable libraries, no fast-moving dependencies)
