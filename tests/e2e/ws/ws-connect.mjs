#!/usr/bin/env node
// tests/e2e/ws/ws-connect.mjs — WebSocket connection helper for E2E tests.
//
// Usage:
//   node ws-connect.mjs <ws_url> '<options_json>'
//
// Options:
//   expect_type: wait for a message with this "type" field, then exit 0
//   expect_close: expect the connection to close (auth rejection), exit 0
//   timeout_ms: max wait time before giving up (default 10000)
//   validate_schema: if true, output the full first matching message as JSON
//   send_message: JSON string to send after connect (for round-trip tests)
//   expect_response_type: after sending, wait for this response type
//
// Exit codes:
//   0 = expected behavior observed
//   1 = timeout or unexpected behavior
//   2 = connection error

const url = process.argv[2];
const opts = JSON.parse(process.argv[3] || '{}');

if (!url) {
    console.error('Usage: node ws-connect.mjs <ws_url> [options_json]');
    process.exit(2);
}

const timeoutMs = opts.timeout_ms || 10000;
let resolved = false;

function finish(code, msg) {
    if (resolved) return;
    resolved = true;
    if (msg) console.log(msg);
    // Give the event loop a tick to flush stdout
    setTimeout(() => process.exit(code), 50);
}

const timer = setTimeout(() => {
    finish(1, 'TIMEOUT: no expected message within ' + timeoutMs + 'ms');
}, timeoutMs);

try {
    const ws = new WebSocket(url);

    ws.addEventListener('open', () => {
        // Send a message if requested (for round-trip tests)
        if (opts.send_message) {
            ws.send(typeof opts.send_message === 'string'
                ? opts.send_message
                : JSON.stringify(opts.send_message));
        }
    });

    ws.addEventListener('message', (event) => {
        const raw = typeof event.data === 'string' ? event.data : event.data.toString();

        try {
            const parsed = JSON.parse(raw);
            const msgType = parsed.type || parsed.event;

            // If we're waiting for a specific type
            if (opts.expect_type && msgType === opts.expect_type) {
                clearTimeout(timer);
                finish(0, JSON.stringify(parsed));
                ws.close();
                return;
            }

            // If we sent a message and are waiting for a response type
            if (opts.expect_response_type && msgType === opts.expect_response_type) {
                clearTimeout(timer);
                finish(0, JSON.stringify(parsed));
                ws.close();
                return;
            }

            // If no specific type expected but we got a valid JSON message, report it
            if (!opts.expect_type && !opts.expect_response_type && !opts.expect_close) {
                clearTimeout(timer);
                finish(0, JSON.stringify(parsed));
                ws.close();
                return;
            }
        } catch {
            // Not JSON — binary message or malformed
            if (!opts.expect_type && !opts.expect_response_type) {
                clearTimeout(timer);
                finish(0, 'BINARY_MESSAGE: ' + raw.length + ' bytes');
                ws.close();
                return;
            }
        }
    });

    ws.addEventListener('close', (event) => {
        clearTimeout(timer);
        if (opts.expect_close) {
            finish(0, 'CLOSED: code=' + event.code + ' reason=' + (event.reason || 'none'));
        } else {
            finish(1, 'UNEXPECTED_CLOSE: code=' + event.code + ' reason=' + (event.reason || 'none'));
        }
    });

    ws.addEventListener('error', (event) => {
        clearTimeout(timer);
        if (opts.expect_close) {
            finish(0, 'CONNECTION_REJECTED: ' + (event.message || 'connection refused'));
        } else {
            finish(2, 'ERROR: ' + (event.message || 'connection failed'));
        }
    });
} catch (e) {
    clearTimeout(timer);
    finish(2, 'FATAL: ' + e.message);
}
