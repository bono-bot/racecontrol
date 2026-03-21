# Requirements: v18.0 Seamless Execution

**Defined:** 2026-03-22
**Core Value:** When either AI needs something done on the other's machine, it delegates the task, the remote side executes, and results flow back seamlessly — no manual relay.

## v18.0 Requirements

Requirements for full bidirectional dynamic execution between James and Bono.

### Dynamic Registry

- [x] **DREG-01**: Either side can register a new command at runtime without redeploying
- [x] **DREG-02**: Dynamic commands use a binary allowlist — only permitted binaries can be registered
- [x] **DREG-03**: Static COMMAND_REGISTRY (20 commands) remains frozen and unmodified
- [x] **DREG-04**: Lookup order: dynamic registry first, static registry fallback
- [x] **DREG-05**: Dynamic commands can specify allowed env var keys merged with safeEnv at execution
- [ ] **DREG-06**: Either AI can query the other's full command registry (name, description, tier — never binary/args)

### Shell Relay

- [x] **SHRL-01**: Either side can send arbitrary binary+args to the other for execution
- [x] **SHRL-02**: Shell relay always uses APPROVE tier — never AUTO or NOTIFY
- [x] **SHRL-03**: Binary must be in allowlist (node, git, pm2, cargo, systemctl, curl, sqlite3, taskkill, shutdown, net, wmic)
- [x] **SHRL-04**: Uday receives WhatsApp notification with full command text before approval
- [x] **SHRL-05**: Shell relay uses same sanitized env + no-shell execution model as static commands

### Execution Chains

- [ ] **CHAIN-01**: Multi-step chains: step N+1 receives step N output, executed sequentially
- [ ] **CHAIN-02**: Chain aborts on step failure by default (exit code != 0)
- [ ] **CHAIN-03**: Per-step continue_on_error flag overrides abort behavior
- [ ] **CHAIN-04**: Structured chain_result returns all step outputs as single response
- [ ] **CHAIN-05**: Chain-level timeout caps entire chain duration regardless of step count
- [ ] **CHAIN-06**: Named chain templates loadable from config file, invocable by name
- [ ] **CHAIN-07**: Output templating: {{prev_stdout}} in step args substituted with previous step output
- [ ] **CHAIN-08**: Per-step retry with configurable count and backoff
- [ ] **CHAIN-09**: Chain state survives WebSocket disconnects — pause/resume across reconnects

### Claude-to-Claude Delegation

- [ ] **DELEG-01**: James can send a chain_request to Bono; Bono executes and returns chain_result
- [ ] **DELEG-02**: Bono can send a chain_request to James; James executes and returns chain_result
- [ ] **DELEG-03**: Delegation is transparent — requesting AI integrates response without exposing relay to user

### Observability

- [ ] **AUDIT-01**: Every remote execution logged to append-only audit file on both machines
- [ ] **AUDIT-02**: Audit entries include: timestamp, execId, command, requester, exitCode, durationMs, tier
- [ ] **AUDIT-03**: Chain executions include chainId and stepIndex in audit entries

## Future Requirements (v18.1+)

None — all features scoped into v18.0.

## Out of Scope

| Feature | Reason |
|---------|--------|
| Shell-mode execution (shell: true) | Breaks entire security model — shell injection risk. Use binary+args array instead. |
| Shared registry across machines | Windows/Linux command mismatch. Registries stay local; use introspection to discover. |
| Conditional chain branching (if/else) | Workflow engine scope creep. Conditional logic stays in Claude reasoning layer. |
| Cross-session chain state across process restarts | Chains bounded by TASK_TIMEOUT_MS (5 min). Restart = failure; AI re-issues chain. |
| Real-time chain progress streaming | chain_result already returns all steps. Complexity for marginal UX benefit. |
| Arbitrary env passthrough from payload | Security risk. Only allowlisted env keys permitted; values from local env. |
| AUTO/NOTIFY tier for shell relay | Shell relay must always be APPROVE. Non-negotiable security gate. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DREG-01 | Phase 130 | Complete |
| DREG-02 | Phase 130 | Complete |
| DREG-03 | Phase 130 | Complete |
| DREG-04 | Phase 130 | Complete |
| DREG-05 | Phase 130 | Complete |
| DREG-06 | Phase 134 | Pending |
| SHRL-01 | Phase 131 | Complete |
| SHRL-02 | Phase 131 | Complete |
| SHRL-03 | Phase 131 | Complete |
| SHRL-04 | Phase 131 | Complete |
| SHRL-05 | Phase 131 | Complete |
| CHAIN-01 | Phase 132 | Pending |
| CHAIN-02 | Phase 132 | Pending |
| CHAIN-03 | Phase 132 | Pending |
| CHAIN-04 | Phase 132 | Pending |
| CHAIN-05 | Phase 132 | Pending |
| CHAIN-06 | Phase 134 | Pending |
| CHAIN-07 | Phase 134 | Pending |
| CHAIN-08 | Phase 134 | Pending |
| CHAIN-09 | Phase 134 | Pending |
| DELEG-01 | Phase 133 | Pending |
| DELEG-02 | Phase 133 | Pending |
| DELEG-03 | Phase 133 | Pending |
| AUDIT-01 | Phase 133 | Pending |
| AUDIT-02 | Phase 133 | Pending |
| AUDIT-03 | Phase 133 | Pending |

**Coverage:**
- v18.0 requirements: 26 total
- Mapped to phases: 26
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation (Phase 130-134 mapped)*
