#!/usr/bin/env node
// MMA audit for billing/game timer sync — gaming-focused models
const https = require('https');
const fs = require('fs');

const OPENROUTER_KEY = process.env.OPENROUTER_KEY || fs.readFileSync('data/openrouter-mma-key.txt', 'utf8').trim();

const MODELS = [
  'deepseek/deepseek-r1-0528',
  'qwen/qwen3-235b-a22b',
  'x-ai/grok-3-mini',
];

const PROMPT = `You are auditing a sim racing venue system where 8 gaming PCs run Assetto Corsa. The system has:
- **Server** (Rust/Axum, port 8080): manages billing, tracks session timers
- **rc-agent** (Rust, on each pod): monitors game state, sends AcStatus::Live when player is on track
- **Kiosk** (Next.js): displays countdown timer to staff

## CURRENT ARCHITECTURE

**Billing Timer (server-side):**
- Starts when server receives AcStatus::Live from pod via WebSocket
- Uses \`started_at = Utc::now()\` (server's clock at message receipt)
- Ticks every 1 second, broadcasts BillingTick to kiosk/POS
- Controls session end (kills game when time expires)

**SessionEnforcer (pod-side, for non-AC games):**
- Starts when LaunchGame command arrives at pod
- Uses \`Instant::now()\` (pod's local clock)
- Independent 1-second tick
- Terminates game locally when duration expires

**Kiosk Display:**
- Receives server BillingTick every 1 second via WebSocket
- Uses local interpolation (decrement every 1000ms between server ticks)

## PROBLEM
1. Billing timer starts at server time (50-200ms AFTER game is actually playable)
2. SessionEnforcer starts at LaunchGame receipt (BEFORE game is playable)
3. No shared "game started" timestamp between pod and server
4. Pod clock and server clock may drift (no NTP coordination)

## THE USER'S PROPOSAL
"What if the game timer starts AFTER the game launches successfully?"
- Both billing and game enforcement should start at the same moment: when AcStatus::Live is confirmed
- The pod should send its local timestamp of when the game became playable
- The server should use this pod timestamp (or at minimum, both should agree on the same moment)

## CANDIDATE APPROACHES

**A: Pod sends playable_at timestamp in AcStatus::Live message**
- Agent records \`Instant::now()\` when game goes Live
- Converts to wall-clock time and includes in the GameStatusUpdate message
- Server uses this timestamp as billing \`started_at\`
- Risk: pod clock may be wrong

**B: Server records receipt time but defers SessionEnforcer to Live**
- Current billing behavior (server time at receipt) stays
- Change SessionEnforcer to NOT start at LaunchGame, but at AcStatus::Live
- Agent resets SessionEnforcer when game goes Live
- Both count from approximately the same moment, but different clocks

**C: Single authoritative timer on server only**
- Remove SessionEnforcer entirely
- Server sends "stop game" command to pod when billing expires
- Pod obeys server-side timer exclusively
- Simpler, but depends on WS connectivity

**D: Bidirectional time sync (NTP-style)**
- Server periodically pings pod with timestamp
- Pod measures round-trip and estimates clock offset
- All timestamps adjusted by offset
- Complex, probably overkill

## YOUR TASK
1. Which approach (A, B, C, D) is best for a commercial sim racing venue with 8 PCs?
2. What are the edge cases for your recommended approach?
3. What should happen if the game loads but the player doesn't drive (AC False-Live guard holds for up to 5 seconds)?
4. What about games without telemetry (Forza, FH5) that use the 90-second process-based fallback?
5. Provide concrete Rust code changes for the recommended approach.

Prioritize: simplicity > accuracy > complexity. A 200ms desync is acceptable. A 5-second desync is not.`;

async function queryModel(model, prompt) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model,
      messages: [{ role: 'user', content: prompt }],
      max_tokens: 3000,
      temperature: 0.3,
    });
    const req = https.request({
      hostname: 'openrouter.ai',
      path: '/api/v1/chat/completions',
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${OPENROUTER_KEY}`,
        'Content-Type': 'application/json',
        'HTTP-Referer': 'https://racingpoint.cloud',
        'X-Title': 'RacingPoint MMA Timer Sync',
      },
    }, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try {
          const json = JSON.parse(data);
          if (json.error) resolve({ model, error: json.error.message || JSON.stringify(json.error) });
          else resolve({ model, content: json.choices?.[0]?.message?.content || '', tokens: json.usage?.total_tokens || 0 });
        } catch (e) { resolve({ model, error: `Parse: ${e.message}` }); }
      });
    });
    req.on('error', e => resolve({ model, error: e.message }));
    req.setTimeout(120000, () => { req.destroy(); resolve({ model, error: 'Timeout' }); });
    req.write(body);
    req.end();
  });
}

async function main() {
  console.log('=== MMA Timer Sync Audit ===');
  const results = [];
  for (const model of MODELS) {
    const short = model.split('/').pop();
    process.stdout.write(`[${short}] querying... `);
    const start = Date.now();
    const r = await queryModel(model, PROMPT);
    const elapsed = ((Date.now() - start) / 1000).toFixed(1);
    console.log(r.error ? `ERROR (${elapsed}s): ${r.error}` : `OK (${elapsed}s, ${r.tokens} tok)`);
    results.push(r);
  }
  let md = '# MMA Timer Sync Audit\n\n';
  for (const r of results) {
    md += `## ${r.model.split('/').pop()}\n\n`;
    md += r.error ? `**ERROR:** ${r.error}\n\n` : `${r.content}\n\n_Tokens: ${r.tokens}_\n\n`;
    md += '---\n\n';
  }
  fs.writeFileSync('data/mma-timer-sync-results.md', md);
  console.log('\nResults: data/mma-timer-sync-results.md');
}
main().catch(console.error);
