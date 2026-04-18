#!/usr/bin/env node
// Parse the 5 DIAGNOSE JSON responses and extract model JSON findings.
const fs = require('fs');
const path = require('path');
const dir = __dirname;

const files = fs.readdirSync(dir).filter(f => f.startsWith('diagnose-') && f.endsWith('.json'));
const results = [];

for (const f of files) {
    const raw = fs.readFileSync(path.join(dir, f), 'utf8');
    let parsed;
    try { parsed = JSON.parse(raw); }
    catch (e) { console.error(`[parse-fail] ${f}: ${e.message}`); continue; }

    if (parsed.error) {
        console.error(`[api-error] ${f}: ${JSON.stringify(parsed.error).slice(0, 200)}`);
        continue;
    }

    const content = parsed.choices?.[0]?.message?.content || '';
    const usage = parsed.usage || {};
    const model = parsed.model || f;

    // Try to extract a JSON block from the content
    let findings = null;
    // Try direct JSON first
    try { findings = JSON.parse(content); } catch (e) {}
    // Try fenced JSON
    if (!findings) {
        const m = content.match(/```(?:json)?\s*([\s\S]+?)\s*```/);
        if (m) { try { findings = JSON.parse(m[1]); } catch (e) {} }
    }
    // Try naked {...} span
    if (!findings) {
        const start = content.indexOf('{');
        const end = content.lastIndexOf('}');
        if (start >= 0 && end > start) {
            try { findings = JSON.parse(content.slice(start, end + 1)); } catch (e) {}
        }
    }

    results.push({
        file: f,
        model,
        usage,
        raw_content: content,
        findings,
    });
}

// Write a combined output
fs.writeFileSync(path.join(dir, 'diagnose-combined.json'), JSON.stringify(results, null, 2));

// Summary to stdout
console.log('\n=== DIAGNOSE parse summary ===\n');
for (const r of results) {
    const f = r.findings;
    if (!f) {
        console.log(`${r.model}: [NO PARSED JSON] content preview: ${r.raw_content.slice(0, 200).replace(/\n/g, ' ')}`);
        continue;
    }
    const score = f.score_deploy_ready;
    const concerns = f.consensus_concerns || [];
    const high = concerns.filter(c => c.severity === 'HIGH').length;
    const med = concerns.filter(c => c.severity === 'MEDIUM').length;
    const low = concerns.filter(c => c.severity === 'LOW').length;
    const tokens = r.usage.prompt_tokens + r.usage.completion_tokens;
    console.log(`${r.model}: score=${score} | HIGH=${high} MED=${med} LOW=${low} | tokens=${tokens} | summary=${f.one_liner_summary?.slice(0, 80) || ''}`);
}

// Consensus analysis — group concerns by question number + severity
const concernsByQ = {};
for (const r of results) {
    const f = r.findings;
    if (!f || !f.consensus_concerns) continue;
    for (const c of f.consensus_concerns) {
        const key = `Q${c.question}_${c.severity}`;
        if (!concernsByQ[key]) concernsByQ[key] = [];
        concernsByQ[key].push({ model: r.model.split('/').pop(), issue: c.issue });
    }
}

console.log('\n=== Consensus (3+ models = HIGH) ===\n');
const sorted = Object.entries(concernsByQ).sort((a, b) => b[1].length - a[1].length);
for (const [key, arr] of sorted) {
    if (arr.length >= 3) {
        console.log(`[CONSENSUS ${arr.length}/5] ${key}`);
        for (const e of arr) console.log(`  - ${e.model}: ${e.issue.slice(0, 180)}`);
    } else if (arr.length === 2) {
        console.log(`[partial 2/5] ${key}`);
        for (const e of arr) console.log(`  - ${e.model}: ${e.issue.slice(0, 150)}`);
    }
}
console.log('');
