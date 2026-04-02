#!/usr/bin/env node
// MMA audit for AC game launcher bugs — gaming/simracing focused models
// Usage: node scripts/mma-game-launcher.js

const https = require('https');
const fs = require('fs');

const OPENROUTER_KEY = process.env.OPENROUTER_KEY || fs.readFileSync('data/openrouter-mma-key.txt', 'utf8').trim();

// Gaming-relevant models — mix of reasoning + code experts
const MODELS = [
  'deepseek/deepseek-r1-0528',       // Reasoner — traces execution paths
  'deepseek/deepseek-v3-0324',       // Code expert — Rust specialist
  'qwen/qwen3-235b-a22b',            // Generalist — broad coverage
  'google/gemini-2.5-flash-preview', // Fast thinker — catches edge cases
  'nvidia/llama-3.3-nemotron-super-49b-v1', // SRE — production system bugs
];

const PROMPT = `You are auditing a Rust-based sim racing venue management system (rc-agent) that manages Assetto Corsa game sessions on Windows gaming PCs. Each PC has triple monitors (7680x1440 NVIDIA Surround), a steering wheel (Conspit Ares 8Nm), and runs rc-agent to control game launching, lock screens, and billing.

## TWO BUGS FOUND DURING E2E TESTING

### Bug 1: Lock Screen Splash Never Dismissed
When a game launches:
1. ws_handler.rs calls show_launch_splash(driver_name) — Edge browser --app overlay shows "Preparing your session..."
2. Game (acs.exe) starts and reaches AcStatus::Live (player on track with steering input)
3. event_loop.rs sets conn.launch_state = LaunchState::Live
4. BUT there is NO code to dismiss/close the Edge overlay
5. Game runs behind the overlay — customer sees the splash, not the game

The lock_screen module has methods like close_browser() and show_blank_screen(), but NONE are called when LaunchState transitions to Live.

### Bug 2: taskkill Silently Fails — Orphan acs.exe Persists
When /games/stop is called:
1. Server sends CoreToAgentMessage::StopGame to agent via WebSocket
2. Agent calls game.stop() which calls kill_process(pid)
3. kill_process() runs: hidden_cmd("taskkill").args(["/PID", &pid, "/F"]).output()?
4. The ? only catches spawn errors, NOT taskkill's exit code
5. Function returns Ok(()) even if taskkill failed
6. No post-kill verification (no is_process_alive check)
7. Next game launch fails: "orphan game process acs.exe still running"

Current kill_process code:
\`\`\`rust
fn kill_process(pid: u32) -> anyhow::Result<()> {
    hidden_cmd("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()?;
    Ok(())
}
\`\`\`

## YOUR TASK
For each bug, provide:
1. **Root cause confirmation** — do you agree with the diagnosis?
2. **Additional failure modes** we might have missed (especially Assetto Corsa-specific or NVIDIA Surround-specific)
3. **Recommended fix** — exact Rust code changes with file paths
4. **Edge cases to test** — what scenarios should we verify after fixing?
5. **Simracing-specific concerns** — anything related to AC's shared memory, CSP (Custom Shaders Patch), Content Manager, or Steam overlay that could interfere?

Focus on production reliability for a commercial sim racing venue. This runs on 8 gaming PCs simultaneously.`;

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
        'X-Title': 'RacingPoint MMA Game Launcher Audit',
      },
    }, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try {
          const json = JSON.parse(data);
          if (json.error) {
            resolve({ model, error: json.error.message || JSON.stringify(json.error) });
          } else {
            const content = json.choices?.[0]?.message?.content || '';
            const cost = json.usage?.total_tokens || 0;
            resolve({ model, content, tokens: cost });
          }
        } catch (e) {
          resolve({ model, error: `Parse error: ${e.message}` });
        }
      });
    });

    req.on('error', e => resolve({ model, error: e.message }));
    req.setTimeout(120000, () => { req.destroy(); resolve({ model, error: 'Timeout (120s)' }); });
    req.write(body);
    req.end();
  });
}

async function main() {
  console.log('=== MMA Game Launcher Audit ===');
  console.log(`Models: ${MODELS.length}`);
  console.log(`Key: ${OPENROUTER_KEY.slice(0, 15)}...`);
  console.log('');

  const results = [];
  for (const model of MODELS) {
    const shortName = model.split('/').pop();
    process.stdout.write(`[${shortName}] querying... `);
    const start = Date.now();
    const result = await queryModel(model, PROMPT);
    const elapsed = ((Date.now() - start) / 1000).toFixed(1);

    if (result.error) {
      console.log(`ERROR (${elapsed}s): ${result.error}`);
    } else {
      console.log(`OK (${elapsed}s, ${result.tokens} tokens)`);
    }
    results.push(result);
  }

  // Write results
  const outPath = 'data/mma-game-launcher-results.md';
  let md = '# MMA Game Launcher Audit Results\n\n';
  md += `Date: ${new Date().toISOString()}\n`;
  md += `Models: ${MODELS.length}\n\n`;

  for (const r of results) {
    const shortName = r.model.split('/').pop();
    md += `## ${shortName}\n\n`;
    if (r.error) {
      md += `**ERROR:** ${r.error}\n\n`;
    } else {
      md += r.content + '\n\n';
      md += `_Tokens: ${r.tokens}_\n\n`;
    }
    md += '---\n\n';
  }

  fs.writeFileSync(outPath, md);
  console.log(`\nResults written to ${outPath}`);
}

main().catch(console.error);
