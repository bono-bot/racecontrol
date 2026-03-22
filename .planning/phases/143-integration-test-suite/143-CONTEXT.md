# Phase 143: Integration Test Suite - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a comms-link integration test script that starts real daemons (or connects to running ones), sends actual WS messages (exec_request, chain_request, message), and verifies round-trip results. Also add cross-platform syntax checks and contract tests.

All implementation in C:/Users/bono/racingpoint/comms-link (test/ directory).

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — infrastructure phase.

Key guidance:
- Integration test file: test/integration.test.js (Node.js test runner, same as unit tests)
- Tests connect to the RUNNING daemon (localhost:8766 relay + ws://srv1422716.hstgr.cloud:8765) — do NOT start a new daemon
- Use actual WS connection with PSK auth to send/receive messages
- Tests must be runnable with `node --test test/integration.test.js`
- Cross-platform check: SSH to Bono and run `node --check` on all source files
- Contract tests can be in same file or separate test/contract.test.js
- ENV: COMMS_PSK must be set for tests to connect

### Test Cases (from requirements)
1. INTEG-01: Send exec_request (command: node_version), verify exec_result has execId, command, exitCode, stdout, durationMs
2. INTEG-02: Send chain_request with 2 steps (node_version, git_status), verify chain_result has matching chainId, both step outputs, status OK
3. INTEG-03: Send message with from:james, verify relay (check Bono logs or comms.db)
4. INTEG-04: Run `node --check` on all .js files in shared/, james/, bono/ — both locally and via SSH on Bono
5. INTEG-05: Contract tests — chainId passthrough (send X, get X back), from field preserved, all v18.0 MessageTypes defined

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- comms-link/send-exec.js — one-shot WS exec pattern (connect, send, wait for result)
- comms-link/send-message.js — one-shot WS message pattern
- shared/protocol.js — createMessage, parseMessage, MessageType
- test/ directory — existing test patterns with node:test

### Integration Points
- localhost:8766 — James relay HTTP (exec/run, chain/run, health)
- ws://srv1422716.hstgr.cloud:8765 — Bono WS server
- PSK auth via Authorization header

</code_context>

<specifics>
## Specific Ideas

The simplest approach: use curl for HTTP relay tests (exec/run, chain/run, health) and WebSocket for message/contract tests. HTTP tests are simpler and more reliable.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
