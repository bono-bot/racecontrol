---
phase: 130
status: passed
score: 5/5
verified: 2026-03-22
---

# Phase 130: Protocol Foundation + Dynamic Registry — Verification

## Must-Haves

| # | Truth | Verified | Evidence |
|---|-------|----------|----------|
| 1 | Either side can register a runtime command without redeploying | YES | HTTP POST /relay/registry/register on James, WS registry_register handler on both sides, DynamicCommandRegistry.register() |
| 2 | Binary not in ALLOWED_BINARIES rejected on register | YES | DynamicCommandRegistry.register() throws, 18 unit tests including allowlist rejection, HTTP returns 400 |
| 3 | All 20 static commands work identically | YES | exec-protocol.js untouched, exec-handler.test.js passes (0 failures), COMMAND_REGISTRY still frozen |
| 4 | Dynamic command with allowedEnvKeys gets isolated env | YES | buildCommandEnv() merges only listed keys from process.env into safeEnv copy, unit test verifies isolation |
| 5 | Lookup: dynamic first, static fallback | YES | ExecHandler.handleExecRequest checks dynamicRegistry?.get(command) first, falls through to commandRegistry[command] |

## Requirement Coverage

| ID | Description | Status |
|----|-------------|--------|
| DREG-01 | Runtime registration without redeploy | Verified |
| DREG-02 | Binary allowlist enforcement | Verified |
| DREG-03 | Static COMMAND_REGISTRY untouched | Verified |
| DREG-04 | Dynamic-first lookup order | Verified |
| DREG-05 | Per-command env key isolation | Verified |

## Gap Closure

Two gaps found in initial verification were fixed:
1. **Protocol tests**: Added 5 MessageType assertions for v18.0 constants (ec9bfd3)
2. **Exec-handler test regression**: Fixed safeEnv assertion to expect EXEC_REASON when reason is non-empty (ec9bfd3)

All tests now passing: protocol (12/12), exec-handler (0 failures), dynamic-registry (18/18), dynamic-registry-integration (6/6).

## Artifacts

| File | Location |
|------|----------|
| DynamicCommandRegistry | comms-link/shared/dynamic-registry.js |
| Protocol types | comms-link/shared/protocol.js (5 new MessageType constants) |
| ExecHandler integration | comms-link/james/exec-handler.js (dynamic lookup + LRU eviction) |
| James wiring | comms-link/james/index.js (HTTP endpoints + WS handler + persistence) |
| Bono wiring | comms-link/bono/index.js (WS handler + persistence) |
| Unit tests | comms-link/test/dynamic-registry.test.js (18 tests) |
| Integration tests | comms-link/test/dynamic-registry-integration.test.js (6 tests) |
| Protocol tests | comms-link/test/protocol.test.js (5 new assertions) |
