#!/usr/bin/env node
// tests/e2e/ws/ws-collect.mjs — Collect multiple WS messages over a time window.
//
// Usage:
//   node ws-collect.mjs <ws_url> '<options_json>'
//
// Options:
//   collect_ms: how long to collect messages (default 10000)
//   max_messages: cap on messages to collect (default 100)
//
// Output: JSON object with messages array, parse_errors count, close_code if disconnected.
// Exit 0 = collection completed, exit 1 = connection failed.

const url = process.argv[2];
const opts = JSON.parse(process.argv[3] || '{}');

if (!url) {
    console.error('Usage: node ws-collect.mjs <ws_url> [options_json]');
    process.exit(1);
}

const collectMs = opts.collect_ms || 10000;
const maxMessages = opts.max_messages || 100;
const messages = [];
let parseErrors = 0;
let closeCode = null;
let closeReason = null;

try {
    const ws = new WebSocket(url);

    ws.addEventListener('message', (event) => {
        if (messages.length >= maxMessages) return;
        const raw = typeof event.data === 'string' ? event.data : event.data.toString();
        try {
            const parsed = JSON.parse(raw);
            messages.push(parsed);
        } catch {
            parseErrors++;
        }
    });

    ws.addEventListener('close', (event) => {
        closeCode = event.code;
        closeReason = event.reason || null;
    });

    ws.addEventListener('error', () => {
        if (messages.length === 0) {
            console.log(JSON.stringify({ messages: [], parse_errors: 0, error: 'connection_failed' }));
            process.exit(1);
        }
    });

    setTimeout(() => {
        const result = {
            messages,
            parse_errors: parseErrors,
            duration_ms: collectMs,
        };
        if (closeCode !== null) {
            result.close_code = closeCode;
            result.close_reason = closeReason;
        }
        console.log(JSON.stringify(result));
        ws.close();
        setTimeout(() => process.exit(0), 50);
    }, collectMs);
} catch (e) {
    console.log(JSON.stringify({ messages: [], parse_errors: 0, error: e.message }));
    process.exit(1);
}
