# Feature Research

**Domain:** Debug-First-Time-Right — Verification & Debugging Quality System
**Project:** v25.0 — systematic elimination of multi-attempt debugging patterns
**Researched:** 2026-03-26
**Confidence:** HIGH (based on 11 documented incidents from this codebase, not generic research)

---

## Context: What Already Exists

The system is not starting from zero. Before categorizing features, this is the
verification/debugging infrastructure that already exists:

| Component | What It Does | Gap |
|-----------|-------------|-----|
| `startup_log.rs` | Phase-by-phase startup log, crash detection via last-phase check | No alerting on crash recovery, no fleet visibility |
| `self_heal.rs` | Config + bat + registry repair on every boot | Baked-in bat content diverges from deployed content |
| `failure_monitor.rs` | Game freeze, launch timeout, USB reconnect detection | Monitors telemetry state, not parse/transform chains |
| `pre_flight.rs` | HID + ConspitLink + orphan game checks before billing | 3 checks only — no config validity, no context mismatch detection |
| `self_monitor.rs` | CLOSE_WAIT detection + WS dead detection, LLM-gated relaunch | Checks health proxies, not the actual parse/decision path |
| `process_guard.rs` | Allowlist enforcement with periodic re-fetch (5 min) | First-run verification absent — empty allowlist proceeds silently |
| `4-tier AI debugger` | Deterministic → memory → Ollama → cloud escalation | No structured Cause Elimination Process enforcement |
| `Cause Elimination Process` | Documented in CLAUDE.md — 5 step process | Not enforced in code — developer discipline only |
| `audit.sh` | 60-phase automated fleet audit | Identifies state, does not enforce verification discipline |

The 11 multi-attempt incidents had 7 root cause categories. Features below map directly to those categories.

---

## Feature Landscape

### Table Stakes (Must Have or Debugging Patterns Recur)

These directly prevent the 6 documented failure categories. Without them, the
same multi-attempt patterns will recur in v25.0+.

| Feature | Why Expected | Root Cause Category | Complexity | Notes |
|---------|-------------|--------------------|-----------| ------|
| **Chain-of-verification in Rust** | Every fix declaration requires verifying the specific parse/transform path that was broken, not just health endpoints | Proxy verification (8+ incidents) | MEDIUM | Add `verify_parse_chain()` helpers to rc-agent and racecontrol that test input→transform→parse→decision→action with real data samples. Distinguish from health probe. |
| **MAINTENANCE_MODE write alert** | MAINTENANCE_MODE silently killed 3 pods for 1.5+ hours with no staff notification | Silent failures (6+ incidents) | LOW | When `C:\RacingPoint\MAINTENANCE_MODE` is written, emit WhatsApp alert immediately. Currently only logs to tracing (not yet initialized path). Use `eprintln!` + WhatsApp via existing channel. |
| **Config fallback observability** | `unwrap_or("http://127.0.0.1:8080")` silently uses dev URL in production — caused "page loads but no data" for full sessions | Context/semantic mismatch (2+ incidents) | LOW | When any config field falls back to a hardcoded default, emit a `warn!` before logging init and record to startup_log. Currently 6 `.unwrap_or("http://127.0.0.1:8080")` and 1 `.unwrap_or("127.0.0.1")` in main.rs silently proceed. |
| **Empty allowlist on boot alert** | Empty allowlist + enabled guard = 28,749 false violations/day for 2 days before detection | Silent failures (6+ incidents) | LOW | Process guard: when fetch returns empty allowlist and `enabled=true`, emit `error!` before logging init + write to startup_log. First-run check: if scan produces >50% violations, alert staff and enter report_only mode automatically. |
| **Startup enforcement bat scanner** | Manual fixes regress on reboot because they are not in the bat file | Manual fixes without code enforcement (5+ incidents) | MEDIUM | Script that compares deployed `start-rcagent.bat` on all 8 pods against the canonical bat embedded in `self_heal.rs`. Flag pods where deployed bat diverges from expected. Run as part of fleet audit. |
| **Boot-time periodic re-fetch for all startup-fetched data** | Allowlist, feature flags, and config each fetched once at startup — if server is down, pod runs on stale/empty data indefinitely | Boot-time transient failures (2+ incidents) | MEDIUM | Allowlist already has 5-min periodic re-fetch (821c3031). Feature flags need same pattern. Any new startup-fetched data must follow this pattern by default. Document as architectural rule. |
| **Cause Elimination template enforcement** | Developers skip straight from symptom to fix — root cause analysis is documented in CLAUDE.md but not enforced | Incomplete root cause analysis (3+ incidents) | LOW | Pre-ship verification gate: structured template output in the fix description before marking any non-trivial bug as resolved. Can be a bash script that prompts for the 5 steps and writes to LOGBOOK.md. |
| **Domain-matched verification gate** | Visual changes verified with health checks; billing chain verified with endpoint pings — wrong domain every time | Proxy verification (8+ incidents) | MEDIUM | Pre-ship verification checklist that categorizes the change type (display/network/parse/billing/config) and requires corresponding check type before marking shipped. Build into GSD execute-phase output. |
| **Sentinel file creation alerts** | GRACEFUL_RELAUNCH, MAINTENANCE_MODE, OTA_DEPLOYING sentinels written silently — no fleet visibility | Silent failures (6+ incidents) | LOW | Any sentinel file write should also emit a structured event to racecontrol's fleet event log (existing `fleet_alert.rs` channel). Server can surface sentinel state in fleet health dashboard. |

### Differentiators (Competitive Advantage / Long-Term Quality)

These features provide systemic quality improvements beyond just fixing the 11
incidents. They make the debugging system self-improving.

| Feature | Value Proposition | Complexity | Notes |
|---------|------------------|------------|-------|
| **Fix attempt counter per incident** | Measures debugging quality over time — track avg attempts-to-fix by category | MEDIUM | Add `fix_attempt` field to LOGBOOK entries. Track running average. Alert when a category regresses. This is how v25.0 validates itself — if avg attempts drop from 2.4 to 1.x, the system is working. |
| **Context snapshot at fix declaration** | When a fix is declared, capture full system state: build_ids, sentinel file states, config hash, allowlist size, WS connection state | MEDIUM | Prevents "it works on my machine" context mismatch. Attach snapshot to LOGBOOK entry automatically on fix commit. |
| **Automated parse-chain smoke tests** | For each major parse chain (UDP→telemetry, WS→billing, config→load, curl→stdout→parse), a runnable smoke test that verifies end-to-end | HIGH | Most impactful differentiator. Catches proxy verification at the test level before deploy. Hard to add retroactively — must be added alongside each new feature in v25.0+. |
| **Boot resilience scorecard** | Fleet dashboard widget showing: for each pod, what data was fetched successfully at boot vs fell back to defaults | MEDIUM | Makes invisible boot failures visible. Shows at a glance which pods are running on stale config. Requires startup_log entries to be enriched with fetch outcomes. |
| **Bat drift detector in fleet audit** | Automated check that deployed bat files match expected content — runs as part of the 60-phase audit | LOW | Extends existing `audit.sh`. Calls rc-sentry `/files` endpoint to read deployed bat, compares hash against canonical in repo. Already has the tooling (rc-sentry /files + audit phases). |
| **First-scan validation for any new guard/filter** | When any filtering system flips from disabled to enabled, mandatory first-scan output review before proceeding | LOW | Add as a standing rule with a code-enforced warning: if `enabled` just changed from `false` to `true`, log first 10 violations before enforcement begins, and require operator acknowledgment. |
| **Structured fix log with hypothesis tracking** | LOGBOOK.md entries include: hypotheses listed, hypotheses eliminated (with evidence), confirmed cause | LOW | Pure process enforcement. Add `fix_log.sh` helper that prompts for Cause Elimination steps and appends structured markdown to LOGBOOK.md. |

### Anti-Features (Things to Deliberately NOT Build)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|--------------|-----------------|-------------|
| **Universal "verify everything" health endpoint** | Seems like a single endpoint that checks all chains would catch proxy verification failures | Creates a new proxy to replace old proxies — the health endpoint IS the thing that failed. Adding a "super health" endpoint just moves the problem one level up | Domain-specific verification scripts that test the exact data path relevant to each change type |
| **Real-time debugging dashboard with live state streams** | Seeing all system state in real time would make debugging faster | High complexity Rust change (new WebSocket channels, new frontend page), high ongoing maintenance cost, and the 11 incidents all had enough information in existing logs — the issue was process, not data availability | Enrich existing fleet health dashboard with sentinel states and boot fetch outcomes (covered by the Differentiator above) |
| **LLM-gated fix declarations** | Using Ollama to validate that a fix description is complete before allowing the commit | Adds external dependency to every debugging session, adds latency, and the LLM cannot actually verify that the fix works | Structured template with bash script enforcement — same outcome, zero dependencies |
| **Per-change automated regression test generation** | Auto-generate regression tests from fix descriptions to prevent the same bug recurring | Test generation from natural language descriptions produces low-quality tests that test the wrong thing. The real fix is domain-matched verification at fix time, not auto-generated tests after | Add specific regression tests for each incident as part of the fix commit |
| **Global debug mode flag** | A single flag to enable "verbose everything" across all pods | Creates a feature that is only useful during debugging but runs in production, adds performance overhead, and requires deploy to enable/disable | The tracing framework already supports per-target log levels. Use `RUST_LOG=rc_agent::billing_guard=debug` instead. |
| **Autonomous fix application without human review** | AI debugger auto-applies fixes to production pods | Tier 3/4 fixes (Ollama/Claude) are already gated by the safe-action whitelist. Expanding this risks auto-applying wrong fixes to billing-critical systems | Keep the whitelist conservative (kill_edge, clear_sentinel, restart_rcagent) — never extend to billing or config mutations without human review |

---

## Feature Dependencies

```
[Domain-matched verification gate]
    └──requires──> [Change type taxonomy] (display/network/parse/billing/config)
                       └──requires──> [existing standing rules categories] (already exist)

[Chain-of-verification helpers in Rust]
    └──requires──> [identification of which parse chains are critical]
                       └──informed by──> [11 incident post-mortem]

[Boot resilience scorecard in dashboard]
    └──requires──> [startup_log enrichment with fetch outcomes]
                       └──requires──> [periodic re-fetch for feature flags] (boot resilience feature)

[Fix attempt counter]
    └──requires──> [structured fix log]
                       └──enhances──> [Cause Elimination template enforcement]

[Bat drift detector in fleet audit]
    └──requires──> [canonical bat content in self_heal.rs] (already exists)
    └──uses──> [rc-sentry /files endpoint] (already exists)
    └──uses──> [audit.sh phase framework] (already exists)

[Empty allowlist on boot alert]
    └──requires──> [eprintln before logging init] (pattern already used in main.rs panic hook)
    └──enhances──> [first-scan validation for any new guard/filter]

[Sentinel file creation alerts]
    └──requires──> [fleet_alert.rs event channel] (already exists)
    └──surfaces in──> [fleet health dashboard] (already exists)
```

### Dependency Notes

- **Boot resilience for feature flags requires boot resilience scorecard:** The scorecard only has value if the fetch outcomes are actually logged. Both should be in the same phase.
- **Domain-matched verification gate requires change type taxonomy:** The taxonomy (display, network, parse, billing, config) maps exactly to the 5 verification modes already in the standing rules. No new categories needed.
- **Chain-of-verification helpers require knowing which chains to cover:** The 11-incident post-mortem already identifies the 4 most critical chains — UDP→billing, config→URL, curl→stdout→parse, allowlist→enforcement. Start with those 4.
- **Fix attempt counter conflicts with informal LOGBOOK entries:** If LOGBOOK entries are free-form, you cannot parse attempt counts. The structured fix log must be adopted before the counter can work.
- **Sentinel file creation alerts require fleet_alert.rs to be reachable at write time:** MAINTENANCE_MODE writes can happen before WS connection is established. The alert should use eprintln + a queued retry, not direct WS send.

---

## MVP Definition

This is a quality system, not a user-facing feature. "MVP" means: what is the
minimum set of features that prevents the 11 incident patterns from recurring?

### Launch With (Phase 1 — highest incident reduction per effort)

- [ ] **MAINTENANCE_MODE write alert** — prevents 1.5-hour silent pod death. One WhatsApp call plus one `eprintln!`. LOW complexity, HIGH impact.
- [ ] **Config fallback observability** — prevents dev URL in production. Add `warn!` to 6 fallback sites. LOW complexity, prevents entire class of context mismatch.
- [ ] **Empty allowlist on boot alert** — prevents 28,749 false violations. Add guard at process_guard enable. LOW complexity.
- [ ] **Sentinel file creation alerts via fleet_alert** — makes GRACEFUL_RELAUNCH + OTA_DEPLOYING + MAINTENANCE_MODE visible in fleet health. LOW complexity, reuses existing channel.
- [ ] **Cause Elimination template enforcement** — structured bash helper for LOGBOOK. LOW complexity, zero Rust changes.

### Add After Validation (Phase 2 — code enforcement)

- [ ] **Chain-of-verification helpers** — add `verify_parse_chain()` for the 4 critical chains. MEDIUM complexity.
- [ ] **Domain-matched verification gate** — integrate into GSD execute-phase output template. MEDIUM complexity.
- [ ] **Startup enforcement bat scanner** — extend fleet audit to compare deployed bat hash. MEDIUM complexity.
- [ ] **Boot resilience for feature flags** — periodic re-fetch to match allowlist pattern. MEDIUM complexity.

### Future Consideration (Phase 3 — quality metrics)

- [ ] **Fix attempt counter per incident** — requires structured LOGBOOK adoption. MEDIUM complexity.
- [ ] **Boot resilience scorecard in dashboard** — requires startup_log enrichment. MEDIUM complexity.
- [ ] **Automated parse-chain smoke tests** — per-chain regression tests. HIGH complexity, add alongside each chain as fixed.

---

## Feature Prioritization Matrix

| Feature | Incident Reduction | Implementation Cost | Priority |
|---------|-------------------|--------------------| ---------|
| MAINTENANCE_MODE write alert | HIGH (3 pods, 1.5h dark) | LOW | P1 |
| Config fallback observability | HIGH (full sessions wrong behavior) | LOW | P1 |
| Empty allowlist on boot alert | HIGH (2 days false violations) | LOW | P1 |
| Sentinel file creation alerts | HIGH (cross all sentinel incidents) | LOW | P1 |
| Cause Elimination template | MEDIUM (discipline enforcement) | LOW | P1 |
| Domain-matched verification gate | HIGH (8+ proxy verification incidents) | MEDIUM | P1 |
| Chain-of-verification helpers | HIGH (prevents proxy verification at code level) | MEDIUM | P2 |
| Startup enforcement bat scanner | MEDIUM (prevents regression, not new bugs) | MEDIUM | P2 |
| Boot resilience for feature flags | MEDIUM (prevents transient boot failures) | MEDIUM | P2 |
| Bat drift detector in fleet audit | MEDIUM (catches bat drift before incidents) | LOW | P2 |
| First-scan validation for guards | MEDIUM (new guard enablement safety) | LOW | P2 |
| Structured fix log | LOW (process improvement) | LOW | P2 |
| Fix attempt counter | LOW (quality measurement) | MEDIUM | P3 |
| Context snapshot at fix | MEDIUM (context mismatch prevention) | MEDIUM | P3 |
| Boot resilience scorecard | MEDIUM (visibility) | MEDIUM | P3 |
| Automated parse-chain smoke tests | HIGH (but delayed ROI) | HIGH | P3 |

**Priority key:**
- P1: Must have for v25.0 — directly prevents documented incident patterns
- P2: Should have — closes structural gaps that cause incident categories
- P3: Nice to have — quality measurement and longer-term improvement

---

## Incident-to-Feature Mapping

Mapping the 7 root cause categories from the audit directly to features:

| Incident Category | Count | Feature That Prevents Recurrence |
|------------------|-------|----------------------------------|
| Proxy verification (build_id OK does not mean bug fixed) | 8+ | Chain-of-verification helpers, Domain-matched verification gate |
| Manual fixes without code enforcement | 5+ | Startup enforcement bat scanner, Bat drift detector in audit |
| Incomplete root cause analysis | 3+ | Cause Elimination template enforcement, Structured fix log |
| Silent failures (no observable state transitions) | 6+ | MAINTENANCE_MODE alert, Sentinel file alerts, Config fallback observability, Empty allowlist alert |
| Boot-time transient failures | 2+ | Boot resilience for feature flags, Boot resilience architectural rule |
| Context/semantic mismatch (dev vs production) | 2+ | Config fallback observability, Context snapshot at fix declaration |
| Missing first-run verification after enabling guards | 1 (structural) | Empty allowlist alert, First-scan validation for guards |

---

## Sources

- `.planning/PROJECT.md` — v25.0 milestone definition with 7 root cause categories and 11 incidents
- `CLAUDE.md` standing rules — 12 debugging rules born from specific incidents, each naming the incident that caused it
- `crates/rc-agent/src/main.rs` — 6x `unwrap_or("http://127.0.0.1:8080")` and 1x `unwrap_or("127.0.0.1")` silent fallbacks confirmed by code inspection
- `crates/rc-agent/src/startup_log.rs` — phase-based startup logging (already exists, gap is fleet visibility and alerting on crash recovery)
- `crates/rc-agent/src/self_heal.rs` — canonical bat content embedded in binary (bat drift detector can diff against this)
- `crates/racecontrol/src/process_guard.rs` line 164 — "empty allowlist is almost certainly a config loading failure" comment with no enforcement action
- `crates/racecontrol/src/fleet_alert.rs` — existing WhatsApp alert channel that sentinel alerts can reuse
- Post-mortem evidence: MAINTENANCE_MODE incident (3 pods, 1.5h), empty allowlist incident (28,749 false violations/day for 2 days), pod healer curl-stdout incident (8 deploy rounds), ConspitLink regression incident (fixed 4 times in one day per CLAUDE.md 2026-03-25 note)

---

*Feature research for: v25.0 Debug-First-Time-Right — Systematic Debugging Quality*
*Researched: 2026-03-26 IST*
