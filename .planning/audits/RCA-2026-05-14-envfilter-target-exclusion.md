# §S-146 V1↔V2 RCA — EnvFilter target-exclusion silenced metric_alert_task spawn evidence

**Anchor:** §S-307 (NF-1 closure derived from §S-298 deploy / §S-300 parallel-bono RCA cross-reference)
**Pilot:** james
**Date:** 2026-05-14 IST
**Class:** V1↔V2 boundary · non-foundational (tracing init not in {billing/wallet/auth/pod-state-channel/WhatsApp identity/DB schema}) · §S-186 fast-lane NOT eligible (PR post-2026-05-09)
**Stale-at:** 2026-08-12 (90 days) — re-read against current EnvFilter directive set + against `tracing_subscriber` version semantics if upgraded

---

## §1 — Boundary map

**Files touched:**

| File | Lines | Era | Role |
|---|---|---|---|
| `crates/racecontrol/src/startup.rs` | 182-183 (now 183-189 after RCA comment) | V1-era (pre-2026-05-01 V2-planning) | tracing-subscriber EnvFilter default; runtime log filter for entire racecontrol binary |
| `crates/racecontrol/src/background_tasks.rs` | 57-61 | V2 (Phase 289 ALRT-01..05) | `metric_alert_task` spawn-gate + spawn-time info-log with explicit `target: "startup"` |
| `crates/racecontrol/src/metric_alerts.rs` | 14,17-21,32-36,48-53,67-72,94 | V2 (Phase 289 ALRT-01..05) | `LOG_TARGET = "metric_alerts"` literal applied to all 5 task-emit points |

**API routes touched:** none.

**DB tables touched:** none. The fix is observability-only; no schema/protocol change.

**Config keys touched:** none. The fix is in default-EnvFilter directive set; `RUST_LOG` env override behavior preserved.

**IPC seams touched:** none.

**V1↔V2 crossing:** `startup::init_tracing` is V1-era shared infrastructure (predates V2 planning 2026-05-01). V2 metric_alert_task (Phase 289 module) is a V1-retained organ that V2 Phase 2 observability (§S-272 PR #75) repurposes for V2 discount-clamp alerting. The crossing is "V2 observability primitives emit through V1 tracing init" — V1 filter defaults dropped V2 emits.

---

## §2 — Inherited-issue catalogue

Sources surveyed: V2-MASTER-STATE §S-N entries naming startup/tracing/EnvFilter/log-filter · LOGBOOK.md `grep "tracing" "EnvFilter" "RUST_LOG" "log filter"` · G9/UCA tagged with tracing or filter component · `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` candidate categories A-J.

**Issues found at this exact boundary:**

| # | Issue | Source | Disposition (§3) |
|---|---|---|---|
| I-1 | "Check live console, not just JSONL logs" — JSONL log files may use a different tracing filter that excludes some WARN targets. Process guard violations flood server console but don't appear in JSONL file. | racecontrol/CLAUDE.md Standing Rule (Testing & Verification) — empirical anchor v17.0/v17.1 verification declared "0 WARNs" while server console flooded with process guard violations. | ROOT-CAUSED-PARTIAL — same class as NF-1 (target-exclusion class); process_guard module emits with target unrecognized by default EnvFilter. NOT-FIXED in this RCA scope (separate target). |
| I-2 | "Rolling appender changed from `racecontrol.log.*` to `racecontrol-*.jsonl` but `/api/v1/logs` reader still searched for old name" — Cascade-Update standing rule trigger; tracing change broke downstream log reader for 3+ days. | racecontrol/CLAUDE.md Standing Rule (Cascade updates) — empirical anchor v23 example. | NOT-APPLICABLE-TO-V2 — different surface (appender filename pattern, not filter directive set). |
| I-3 | "Static-vs-Dynamic env_filter" — `try_from_default_env()` uses `RUST_LOG` env var at process-start; ANY hot-reload requires reload-handle. | tracing-subscriber 0.3.x documented behavior. | NOT-APPLICABLE — this RCA accepts static load; hot-reload is a separate enhancement. |
| I-4 | "load_or_default silent-fallback class" — `Config::load_or_default()` at config/mod.rs:269 falls back to `Default` on parse error. If toml parse fails, alert_rules become empty Vec → spawn-gate at background_tasks.rs:57 is false. | source-evidence this session + racecontrol/CLAUDE.md "Verify Before Generate" doctrine. | NOT-APPLICABLE-TO-§S-307 — this RCA-confirmed via Server .23 probe (ssh probe #1) that TOML parses correctly: `[[alert_rules]]` canonical + body well-formed; alert_rules.is_empty() is FALSE at runtime. I-4 is a sibling concern that did NOT fire in NF-1 instance. |
| I-5 | V1 process guard with empty allowlist → 28,749 false violations/day for 2 days before noticed. Sibling pattern to NF-1: V1 behavior emits but observers structurally couldn't see it. | racecontrol/CLAUDE.md "First-run verification after enabling any guard/filter/blocklist" standing rule. | NOT-APPLICABLE-TO-§S-307 surface — different module; but same META-class (observer-side gap renders subject-side runtime behavior invisible). |

**No prior §S-146 or G9 entries at the startup.rs:182-183 EnvFilter default specifically.** This is the first surfaced root cause at this exact line.

---

## §3 — Past-bug review

| Issue | Disposition | Justification |
|---|---|---|
| I-1 (JSONL filter excludes WARN targets — process_guard class) | ROOT-CAUSED-PARTIAL | Standing rule captures the symptom + advises "check live console, not just JSONL logs." Underlying mechanism is the SAME class as NF-1 (target-exclusion). This RCA's fix structurally addresses the metric_alerts and startup targets; process_guard target remains in same class. Follow-up PR may extend EnvFilter to include process_guard target if/when LIVE-BLOCKING observability needs it. Out-of-scope for §S-307. |
| I-2 (appender filename change broke API reader) | NOT-APPLICABLE-TO-V2 | Filename pattern issue; unrelated to filter directive set. |
| I-3 (static EnvFilter no hot-reload) | NOT-APPLICABLE | Hot-reload is a separate enhancement; out-of-scope for NF-1 closure. |
| I-4 (load_or_default silent fallback) | NOT-APPLICABLE-TO-§S-307 | Verified via ssh probe #1 (2026-05-14 ~10:20 IST): TOML parses correctly, spawn-gate true. I-4 is the alternate hypothesis-class (H-C) that ssh probe RULED OUT. |
| I-5 (process guard empty allowlist) | NOT-APPLICABLE-TO-§S-307 surface | Different module; META-class relationship only. |

**Conclusion:** No unfixed past bug at the EnvFilter default surface specifically. NF-1 surfaces a new instance of a known META-class (observer-side filter excludes subject-side emit) that prior fixes did not pre-emptively cover. The CLAUDE.md standing rule "Check live console, not just JSONL logs" anticipates the issue at runtime but does not propose a permanent fix; this RCA proposes the permanent fix for the metric_alerts + startup targets specifically.

---

## §4 — V2-alignment delta

**Current state (V1 default):** EnvFilter default directive set excludes V2-introduced observability targets. The `discount_clamp_storm` alert rule (§S-272 Phase 2 observability) cannot emit spawn-evidence into the canonical JSONL appender. The 4-week class-A soak clock started by §S-298 is observably indistinguishable from "task never started."

**V2 doctrine alignment:**
- V2-MASTER-STATE §S-272 ratified the discount-clamp alert as Phase 2 observability primitive
- V2-LBAC §14.3 F3 ACCOUNTING REFORM: `DONE = behavior observable at V2 entry point + acceptance test passes (no SKIP)` — without spawn-evidence, the §S-272 row cannot reach DONE under F3 semantics
- §S-298 4-week soak clock requires evidence that the task IS running; current state makes that evidence-class assertion impossible from JSONL log
- §S-186 V2-VELOCITY doctrine: observability gaps that prevent customer-visible-impact verification are LIVE-BLOCKING by frame, not back-burner

**Gap explicitly named:** V1 EnvFilter default did not anticipate explicit `target: "..."` literals on V2 observability primitives. V2 primitives use explicit target literals (Phase 289 pattern) to namespace cross-cutting events; V1 filter defaults match on Rust module-path prefix, not literal target strings.

---

## §5 — Proposed change framed against V2 doctrine

**Change:** Extend the default EnvFilter directive at `crates/racecontrol/src/startup.rs:183` to admit two literal targets: `startup=info` and `metric_alerts=info`.

**Diff size:** 1 line (the directive string literal). Plus an inline `// §S-307` comment (5 lines) documenting the reason.

**V2-frame justification:**
1. Moves V1-era tracing init forward by recognizing V2-introduced target literals as first-class — not retroactively patching V1 behavior, but extending V1 substrate to admit V2 primitives.
2. Preserves V1 behavior for unspecified targets (RUST_LOG override behavior unchanged; previously-emitting modules continue to emit identically).
3. Single boundary (tracing init), no schema/protocol/IPC change — kaizen-minimal.
4. Permanence-gate: change is in source (git), survives every redeploy.
5. Follows §S-186 V2-VELOCITY substrate-PR-shape (≤200 LOC, single boundary, no schema, no protocol, bug fix only) — but disqualified from §S-186 fast-lane RCA scope-narrowing because PR is created post-2026-05-09. Full §S-146 5-section RCA = this document.

**Why not "switch the targets to default (module-path) instead of literal":**
- Phase 289 deliberately chose literal targets `"startup"` + `"metric_alerts"` to namespace cross-cutting events at the consumer-of-logs layer (JSONL grep, dashboard filters, alert correlators). Changing emit-targets back to module-paths would break any downstream consumer that grep'd by literal target — a wider blast radius than extending the filter.
- The EnvFilter directive extension preserves the namespacing benefit AND restores visibility — kaizen-correct.

**Follow-up trigger conditions:**
- If a NEW Phase introduces another target literal (e.g., `target: "wallet_anomaly_watchpoint"` per §S-306 design): extend EnvFilter default in the SAME PR as the new target literal, OR add a regression-test that lists EnvFilter directives + asserts coverage matches all `target: ` literals in the crate.
- If RUST_LOG env override is set on Server .23 with insufficient directives, surface as observability gap (not a code regression).
- Stale-at this RCA 2026-08-12; if `tracing_subscriber` major-version upgrade lands before then, re-read filter semantics.

---

## §6 — Verify-by (post-merge cascade)

**Build verification:**
1. `cargo check -p racecontrol-crate` clean
2. Unit test in `startup.rs` test mod (added in this PR) asserts default directive string contains `startup=info` and `metric_alerts=info`
3. `cargo test -p racecontrol-crate` clean (no regressions)

**Runtime verification (post-Server-.23 deploy, separate cascade):**
1. Deploy via `deploy-server.sh` (DEPLOY PARITY: cloud parity check on Bono VPS racecontrol)
2. Probe `findstr /C:"metric alert task spawned" C:\RacingPoint\logs\racecontrol-.YYYY-MM-DD.jsonl` → ≥1 line at racecontrol startup time (pattern #1 — spawn-time emit from background_tasks.rs:60)
3. Probe +90s later `findstr /C:"first evaluation cycle"` → ≥1 line (pattern #3 — first-cycle marker from metric_alerts.rs:32, validates 60s sleep + snapshot path)
4. Probe `findstr /C:"metric alert task started"` → ≥1 line (pattern #2 — task body emit from metric_alerts.rs:17)
5. §S-N CLOSE-ANCHOR with raw output paste of the 3 patterns + build_id new + soak-clock continuation note

**NOT tested in §S-307 cascade (out-of-scope):**
- Pattern #4 (`alert.fired` from metric_alerts.rs:94) — requires actual threshold trigger; `discount_clamp_count_daily > 10/day` not expected to fire under normal venue traffic; tested only when threshold organically crosses or via fixture injection
- I-1 process_guard target visibility — sibling target-exclusion class; out-of-scope for §S-307 (separate follow-up PR if/when LIVE-BLOCKING priority)
- Hot-reload of EnvFilter without process restart — separate enhancement
- Cloud racecontrol on Bono VPS — DEPLOY PARITY check is part of post-merge cascade, not in this PR's verify-by

---

## §7 — Composes-with

- §S-272 PR #75 — parent observability cascade (discount_clamp_storm alert rule)
- §S-298 — james deploy that surfaced NF-1 (this RCA closes the open thread)
- §S-300 / §S-302 — parallel-bono NF-1 RCA; AMPLIFIER cross-reference path (independent diagnostic; H5 binary-version RULED-OUT per §S-305 supplement)
- §S-303 (mine, earlier today, different cascade — PR #69/#70 routing-gap)
- §S-305 / §S-306 — Phase-1 HOLD-soak posture + wallet-anomaly watchpoint design (sibling observability surface; this RCA's pattern composes with that surface's eventual EnvFilter target if landed)
- V-LBAC §14.1 MAOR — Tier-1 self-review on this PR's commit set (mandatory per every-iter)
- V-LBAC §14.2 F1 SCOPE GATE — G-F1-1 endpoint N/A (no API change) · G-F1-2 constant = EnvFilter directive string · G-F1-3 shape = directive set · G-F1-4 mechanism = tracing-subscriber filter · G-F1-5 §S-146 RCA composes-with (this document)
- §S-146 V1↔V2 RCA gate (this RCA itself)
- "Check live console, not just JSONL logs" standing rule — same META-class precedent

---

## §8 — Eligibility classifier audit trail

| Gate | Verdict | Note |
|---|---|---|
| Q1 V2-aligned | YES | Closes §S-272 / §S-298 NF-1 |
| Q2 info-complete | YES | Root cause confirmed via 4 probes (ssh×2 + http + source-read) |
| Q3 canonical-boundary | NOT-foundational | tracing init not in {billing/wallet/auth/pod-state-channel/WhatsApp/DB-schema} |
| §S-186 fast-lane | NOT-eligible | PR post-2026-05-09 (criterion #1 fails); ≤200 LOC + single-boundary + no schema + no protocol + bug-fix all true but date disqualifies; full §S-146 required |
| §S-146 V1↔V2 RCA | REQUIRED | V1-era startup.rs touched by V2-aligned change (= this document) |
| MMA Step 1 DIAGNOSE | NOT-required | Non-foundational |
| V-LBAC classifier | CAPTAIN-GATED-MERGE | Author + stage AUTO-JAMES (this PR); halt at merge for explicit Captain verb |
| Pre-commitment exception sub-clause | LIKELY-CARRIED | Captain 2026-05-13 ~19:15 IST "complete LIVE-BLOCKING autonomously" + 2026-05-14 verbatim "Proceed §S-303" + "Proceed with your recommendation aligned with V2" — NF-1 is enumerated as §S-298 open thread restated in §S-307 OPEN-CLAIM proposal this session |

---

**Authoring metadata:** james · 2026-05-14 IST · written from worktree `racecontrol-s307` based on origin/main `340fd1a0` · pre-MAOR-Tier-1
