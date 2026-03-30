# Research Summary: v31.0 Autonomous Survival System

**Researched:** 2026-03-30
**Sources:** 4 researcher agents + 3-model MMA validation (Qwen3 235B, DeepSeek R1, Gemini 2.5 Pro)
**Confidence:** HIGH

---

## Key Findings

### Stack
- **Only 1 new crate:** `goblin` for PE header parsing (verify latest version). All others (sha2, reqwest::blocking, serde, toml) already in workspace.
- **rc-watchdog has NO tokio runtime.** Must use `reqwest::blocking` or create explicit `Runtime::new()` for async OpenRouter calls.
- **OpenRouter child keys** via management API for per-pod budget isolation ($10/day cap).
- **Cross-layer HTTP:** watchdog POSTs directly to server `/api/v1/pods/{id}/survival-report`.

### Architecture
- **Layer 1:** Extend existing rc-watchdog (NOT a new binary) with `smart_watchdog.rs` + `survival_state.rs`.
- **Layer 2:** Extend `pod_healer.rs` + new `survival_coordinator.rs` in racecontrol.
- **Layer 3:** New `rc-guardian` crate for Bono VPS (Linux target, pm2/systemd).
- **OpenRouter client trait** in rc-common (trait only, not reqwest dependency) — avoids circular deps.
- **Build order:** Foundation (rc-common types) → Layer 2 Server endpoints → Layer 1 Watchdog → Layer 2 Integration → Layer 3 Guardian.

### Features (MVP for v31.0)
- **Layer 1:** SHA256 validation + rollback, startup health poll, direct HTTP reporting, MAINTENANCE_MODE auto-clear.
- **Layer 2:** SSH diagnostic runner, fleet-pattern detection, repair confidence gate, canary rollout, post-fix behavioral verification.
- **Layer 3:** Server health polling (60s/3-miss), restart via SSH with billing safety check, WhatsApp escalation.
- **Unified MMA Protocol:** 5th model (thinking variant), fact-checker role, cost guard, Unified Protocol v3.1 integration.

### Pitfalls (Top 5)
1. **Recovery System Fight:** 5 healers on same patient. Need HEAL_IN_PROGRESS sentinel or server-arbitrated heal lease.
2. **MAINTENANCE_MODE lockout:** Watchdog must NOT write it during MMA; only read and escalate.
3. **Windows SYSTEM HTTP failures:** Certificate validation issues in Session 0.
4. **Rollback loop:** Need depth tracking + "both binaries bad" escalation to Layer 2.
5. **Split-brain guardians:** Need GUARDIAN_ACTING coordination via comms-link.

---

## MMA Consensus Findings (3-model: Qwen3, DeepSeek R1, Gemini 2.5 Pro)

### P1 — Consensus Gaps (2+ models agree)
| Finding | Models | Action |
|---------|--------|--------|
| OpenRouter fallback when API is down | 3/3 | Add deterministic rule-based fallback engine for AI outages |
| Budget persistence across reboots | 2/3 | Store daily usage in `budget_state.json` on disk |
| Watchdog thread blocking from reqwest::blocking | 3/3 | Use dedicated async runtime thread for OpenRouter calls |
| API key lifecycle (provisioning, rotation, revocation) | 2/3 | Define key management protocol in deploy pipeline |
| Layer 3 state communication back to fleet | 2/3 | Define GUARDIAN_ACTING protocol via comms-link WS |
| HEAL_IN_PROGRESS stale lock risk | 3/3 | Use server-arbitrated heal lease with TTL renewal |

### P2 — Notable Findings (1 model, high confidence)
| Finding | Model | Action |
|---------|-------|--------|
| Watchdog self-integrity check | R1 | Consider embedded hash in PE resource section |
| OpenRouter model latency matters for survival | Gemini | Measure TTFT, not just cost per token |
| Training period doesn't make cheap models smart | Gemini | Define formal validation gate with benchmarks |
| Safe-window check for active sim sessions | Qwen3 | Check running game processes before healing |
| Structured logging with action_id correlation | Qwen3 | Add to all cross-layer operations |

### Corrections Applied
- **OpenRouter client in rc-common:** Define a TRAIT only, not the full reqwest client — avoid heavy deps in shared core.
- **MAINTENANCE_MODE rule refined:** Watchdog should not *initiate* MAINTENANCE_MODE for its own diagnostic cycle, but CAN set it when commanded by Layer 2/3 for fleet operations.
- **Model validation gate:** Don't switch to cheaper models after fixed 30 days — require formal benchmark pass (>90% agreement with top-tier on same incidents).

---

## Requirements Implications

### Must-Have (v31.0)
1. HEAL_IN_PROGRESS sentinel with TTL (or server-arbitrated lease)
2. Binary SHA256 validation + PE header check in rc-watchdog
3. Automatic rollback with depth tracking (max 3)
4. Dedicated async runtime thread in watchdog for OpenRouter calls
5. Direct HTTP survival reporting (watchdog → server)
6. SSH diagnostic runner with structured JSON output
7. Fleet-pattern detection with single MMA session for fleet events
8. Repair confidence gate (>= 0.8) + canary rollout
9. External Guardian health polling + restart + WhatsApp escalation
10. Unified MMA Protocol with 5-model roster + fact-checker + cost guard
11. OpenRouter fallback to deterministic rules when API unreachable
12. Budget persistence to disk
13. Billing safety check before any restart (server or pod)

### Should-Have (v31.x)
1. Server-arbitrated heal lease (replaces file sentinel)
2. Model validation gate before downgrade
3. Night-ops autonomous maintenance window
4. Graduated repair scope (pod → class → fleet)
5. API key lifecycle management in deploy pipeline
6. Safe-window check for active game sessions

---

*Summary for: v31.0 Autonomous Survival System — 3-Layer MI Independence*
*Synthesized: 2026-03-30*
