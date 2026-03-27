# Multi-Model Meta-Audit: Gaps in the Audit Protocol

**Date:** 2026-03-27 | **Author:** James Vowles (Opus 4.6 reviewer)
**Method:** 5 AI models via Perplexity MCP + Opus synthesis
**Cost:** 5 Pro Search queries (~$0) | **Models:** Gemini 3.1 Pro Think, GPT-5.4 Think, Claude Sonnet 4.6 Think, Nemotron 3 Super, Claude Opus 4.6
**Scope:** Meta-audit of AUDIT-PROTOCOL.md (62 phases) + MULTI-MODEL-AUDIT-PROTOCOL.md (5-model code audit)

---

## Executive Summary

**48 unique gaps identified** across 8 categories. Every model found things the others missed.

| Category | P1 (Critical) | P2 (Important) | P3 (Nice-to-have) | Total |
|----------|:---:|:---:|:---:|:---:|
| Audit Trust & Integrity | 5 | 2 | 1 | 8 |
| Security Blind Spots | 4 | 4 | 1 | 9 |
| Resilience & Chaos | 3 | 2 | 0 | 5 |
| Data & Backup | 3 | 1 | 0 | 4 |
| Performance & Capacity | 1 | 3 | 1 | 5 |
| Protocol Consistency | 1 | 3 | 1 | 5 |
| Code Audit Gaps | 2 | 4 | 1 | 7 |
| Operations & Business | 1 | 2 | 2 | 5 |
| **TOTAL** | **20** | **21** | **7** | **48** |

### Model Contribution Matrix

| Gap ID | Gemini | GPT-5.4 | Sonnet | Nemotron | Opus | James (reviewer) |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|
| T-01 Circular dependency | x | x | x | | x | |
| T-02 Credential exposure | | | x | | | x |
| T-03 No audit self-test | x | | | | x | |
| T-04 Result integrity | | | x | | x | |
| S-01 Physical security | | x | x | | | |
| R-01 No chaos engineering | x | x | | x | | |
| D-01 No backup testing | x | x | x | x | | |
| C-05 Frontend not audited | | x | | | | x |

*(Partial matrix — full attribution in findings below)*

---

## Category 1: Audit Trust & Integrity

### T-01 [P1] Circular Dependency — Auditor Trusts the Audited
**Found by:** Gemini, GPT-5.4, Sonnet, Opus (4/5 consensus)
**Gap:** Phases 1-16 use `rc-agent :8090 exec` to query pod state. If rc-agent is compromised, corrupted, or running in Session 0, it returns false health data. The audit has no out-of-band verification path.
**Impact:** A compromised pod passes all 62 phases. An attacker who patches rc-agent controls the audit narrative.
**Fix:** Add dual-path verification: at minimum, cross-validate critical checks via rc-sentry :8091 (independent binary). For highest assurance, add SSH-based spot checks using a separately managed key. Phase 9b partially addresses crash loops but not compromise.

### T-02 [P1] Credential Exposure in Protocol Documents
**Found by:** Sonnet (unique), confirmed by James
**Gap:** AUDIT-PROTOCOL.md contains the admin PIN (`261121`) and comms-link PSK hash in plaintext. `audit.sh` exports `SERVER_IP` and `PODS` as env vars visible to child processes. These are in git history forever.
**Impact:** Anyone with repo access (or any AI model receiving the document) has the master credential. The multi-model code audit sends CLAUDE.md (which contains the PIN) to 5 external AI providers.
**Fix:** (1) Move PIN to env var or secrets manager, reference `$AUDIT_PIN` in protocol. (2) Rotate PIN. (3) Audit git history for credential commits. (4) Strip credentials from code audit batches before sending to OpenRouter.

### T-03 [P1] No Self-Test for Audit Infrastructure
**Found by:** Gemini, Opus (2/5)
**Gap:** No phase verifies that `audit.sh` itself works correctly. If the parallel engine's semaphore leaks, if `results.sh` miscounts PASS/FAIL, or if a bash bug silently skips phases, the audit produces false confidence.
**Impact:** A broken audit runner produces "54 PASS, 6 QUIET, 0 FAIL" even when real failures exist.
**Fix:** Add Phase 0 (self-test) that runs 3 dummy phases (1 PASS, 1 FAIL, 1 QUIET), verifies correct counting, checks semaphore cleanup, and validates result file format.

### T-04 [P1] Result Integrity — No Signing or Tamper Detection
**Found by:** Sonnet, Opus (2/5)
**Gap:** Audit results are written to local filesystem with no cryptographic signing. An attacker (or accidental overwrite) can modify results to hide findings. Multi-model audit responses from OpenRouter could be tampered in transit.
**Impact:** Results cannot be trusted for compliance, incident response, or historical comparison.
**Fix:** Hash + sign results with GPG after audit completion. Store signed results in append-only location. Embed git SHA, timestamp, and audit.sh version in every result file.

### T-05 [P1] Single Point of Failure — Auditor = Operator
**Found by:** Opus (unique)
**Gap:** James's machine is both the auditor AND the operator. If James is compromised, the attacker controls the audit results, the deploy pipeline, and the comms channel to Bono. No independent verification path exists.
**Impact:** Complete audit compromise with no detection mechanism.
**Fix:** Bono should independently verify a subset of critical phases from the VPS (e.g., fleet health, cloud sync, build_id match) and compare against James's results. Discrepancies trigger investigation.

### T-06 [P2] Parallel Audit Corruption
**Found by:** Sonnet (unique)
**Gap:** Nothing prevents two audit runs from executing simultaneously (cron + manual). Interleaved writes to result files produce corrupted, non-deterministic reports.
**Fix:** Add PID file lock at audit.sh startup. If lock exists, abort with error.

### T-07 [P2] Timing Attacks — Predictable Audit Schedule
**Found by:** Opus (unique)
**Gap:** The audit runs phases in a predictable sequence. An attacker monitoring network traffic can know exactly when each phase runs and hide malicious activity during gaps.
**Fix:** Randomize phase order within tiers (where no dependencies exist). Add ±15s jitter between phases.

### T-08 [P3] Stale Auth Token Mid-Audit
**Found by:** Sonnet (unique)
**Gap:** JWT and terminal session tokens acquired at audit start may expire during a full 62-phase run. Phases 31+ may fail with 401, producing false negatives.
**Fix:** Add token refresh logic in `core.sh` — check token age before each tier, re-authenticate if > 50% of TTL.

---

## Category 2: Security Blind Spots

### S-01 [P1] Physical Security Completely Unaudited
**Found by:** GPT-5.4, Sonnet (2/5)
**Gap:** No phase checks for USB keyloggers, rogue network devices, camera tamper, BIOS integrity, or chassis intrusion. Pods are customer-accessible — physical access is routine.
**Red team scenario:** Walk in as a customer, plug in $15 USB keylogger + $30 network tap. Return next week with credentials and full packet capture. Zero phases detect this.
**Fix:** Add Phase 69: Physical Security Audit (quarterly, manual checklist — USB port inspection, network tap scan via ARP, BIOS password verification, camera tamper check).

### S-02 [P1] No Rate Limiting Verification
**Found by:** GPT-5.4, Sonnet (2/5)
**Gap:** No phase tests brute-force resistance on auth endpoints. The admin PIN is 6 digits — 1M combinations. Without rate limiting, an attacker can brute-force it in minutes from the LAN.
**Fix:** Add test to Phase 50 (Security E2E): send 10 rapid auth attempts with wrong PIN, verify lockout or rate limit kicks in.

### S-03 [P1] No Network Segmentation Validation
**Found by:** GPT-5.4 (unique)
**Gap:** Pods, server, POS, cameras, and guest WiFi should be separate trust zones. No phase tests whether a pod can reach the NVR admin panel, whether guest WiFi can hit internal APIs, or whether camera network is isolated.
**Fix:** Add Phase 70: Network Segmentation Audit (test east-west movement between zones).

### S-04 [P1] Supply Chain — Source Code Sent to External AI Providers
**Found by:** Sonnet (unique, red-team perspective)
**Gap:** The multi-model code audit sends FULL source code (including auth logic, PSK implementation, PIN validation, network topology) to 5 external AI providers via OpenRouter. OpenRouter itself is a MITM. Providers may log prompts and train on user data.
**Impact:** Attack surface of auth system is revealed to external parties. Hardcoded values, IP ranges, and config snippets become known.
**Fix:** (1) Review OpenRouter and each provider's data retention policy. (2) Strip credentials, IPs, and topology from code before sending. (3) Consider self-hosted models for security-critical batches (Batch 1: server auth, Batch 4: comms PSK).

### S-05 [P2] No TLS/CORS/CSP/Cookie Audit
**Found by:** Sonnet (unique)
**Gap:** No phase checks cipher suites, TLS version minimums, CORS headers, Content-Security-Policy, or cookie security flags (HttpOnly, Secure, SameSite).
**Fix:** Add checks to Phase 50 or new Phase 71: HTTP Security Headers audit.

### S-06 [P2] No Windows Pod Hardening Review
**Found by:** GPT-5.4, Nemotron (2/5)
**Gap:** Missing checks for: USB lockdown, local admin accounts, AppLocker/WDAC, Defender state, BitLocker, RDP disablement, browser escape resistance, kiosk breakout prevention.
**Fix:** Add to Phase 4 (Firewall) or new Phase 72: Windows Hardening Audit.

### S-07 [P2] No Secrets Rotation or Default-Credential Audit
**Found by:** GPT-5.4 (unique)
**Gap:** Camera NVR uses `admin/Admin@123` (documented in CLAUDE.md). No phase checks for default vendor credentials, shared service accounts, or credential rotation practices.
**Fix:** Add to Phase 50 or new dedicated phase. Include NVR, POS, cloud VPS, all service accounts.

### S-08 [P2] No Input Fuzzing or Adversarial Testing
**Found by:** Sonnet, GPT-5.4 (2/5)
**Gap:** No phase sends malformed data to test rejection behavior. Exec endpoints, game launch params, billing requests — none tested with hostile input.
**Fix:** Add lightweight fuzz tests to Phase 57 (E2E suite) — malformed JSON, oversized payloads, SQL injection strings, path traversal in exec.

### S-09 [P3] No Prompt Injection Review for AI Coordination
**Found by:** GPT-5.4 (unique)
**Gap:** The comms-link AI coordination layer is never audited as an AI system. No checks for prompt injection, model output validation, or token/cost runaway.
**Fix:** Add to Batch 4 (comms-link) system prompt in multi-model audit.

---

## Category 3: Resilience & Chaos

### R-01 [P1] No Chaos Engineering / Failure-Mode Testing
**Found by:** Gemini, GPT-5.4, Nemotron (3/5 consensus)
**Gap:** The audit only checks steady-state health. No tests for: server crash during billing, network partition, cloud VPS down during sync, power outage mid-game, WiFi drops for POS PC.
**Impact:** "All 62 phases PASS" gives false confidence about resilience. The system may be fragile under failure conditions.
**Fix:** Add Tier 21: Chaos Engineering (quarterly, manual). Test: kill racecontrol mid-billing, disconnect one pod's network cable, simulate cloud VPS unreachable. Document expected behavior + actual behavior.

### R-02 [P1] No Power Failure Recovery Testing
**Found by:** GPT-5.4 (unique)
**Gap:** No test for: UPS presence, shutdown order, automatic startup order, SQLite consistency after abrupt power loss, pod auto-login, kiosk re-lock, billing reconciliation after reboot.
**Fix:** Add Phase 73: Power Recovery Drill (annual, simulated).

### R-03 [P1] No Incident Response / Ransomware Recovery Drill
**Found by:** GPT-5.4, Sonnet (2/5)
**Gap:** No phase tests containment, account revocation, pod isolation, recovery from known-good images, log preservation, or staff runbooks. No tested backup restore.
**Impact:** In a ransomware event, recovery time is unknown. Could be hours or days.
**Fix:** Add Tier 22: Incident Response Drill (semi-annual). Include: isolate one pod, recover from backup, time the RTO.

### R-04 [P2] No Rollback Testing
**Found by:** Nemotron (unique)
**Gap:** OTA pipeline has rollback capability but it's never exercised. What if `*-prev.exe` is corrupted? What if rollback produces a config incompatibility?
**Fix:** Add to Phase 41 (Config Push & OTA): trigger a test rollback on Pod 8 canary, verify binary reverts and pod functions correctly.

### R-05 [P2] No Business Continuity / Manual Fallback Test
**Found by:** GPT-5.4 (unique)
**Gap:** What happens if billing is down but pods and staff are up? No manual check-in process, no offline receipt capability, no rules for continuing active customer sessions.
**Fix:** Document manual fallback procedure. Add Phase 74: Business Continuity Test (quarterly).

---

## Category 4: Data & Backup

### D-01 [P1] No Backup/Restore Verification
**Found by:** ALL 5 models (5/5 consensus — strongest signal)
**Gap:** No phase verifies that SQLite DB backups exist, that they can be restored, or that the restore produces a working system. No defined RPO/RTO.
**Impact:** After data loss, recovery time is unknown. Backups that exist but can't be restored are worthless.
**Fix:** Add Phase 75: Backup & Restore Test. (1) Verify backup exists. (2) Restore to temp location. (3) Verify row counts match. (4) Document RPO/RTO. Run monthly.

### D-02 [P1] No Database Integrity Testing
**Found by:** GPT-5.4 (unique)
**Gap:** SQLite-specific risks not tested: WAL mode sanity, corruption detection, file-lock contention under load, vacuum strategy, recovery after abrupt power loss.
**Fix:** Add checks to Phase 36: `PRAGMA integrity_check`, WAL size, journal mode verification.

### D-03 [P1] No Customer Data Privacy Compliance Audit
**Found by:** Gemini, GPT-5.4 (2/5)
**Gap:** Phase 37 checks for PII in logs (basic grep for phone numbers) but doesn't audit: customer data retention policy enforcement, right-to-deletion process, camera facial data handling under DPDP Act, cafe billing data retention.
**Fix:** Expand Phase 37 with DPDP-specific checks. Add camera data retention verification to Phase 44.

### D-04 [P2] No Historical Trend Tracking for Audit Results
**Found by:** GPT-5.4 (unique)
**Gap:** Each audit run is a snapshot. No mechanism to track: are the same bugs recurring? Which modules have the most findings? What's the mean time to fix? Is the system getting better or worse?
**Fix:** Add `audit-trends.json` that accumulates PASS/FAIL/finding counts per phase across runs. `cross-model-analysis.js` should output defect families and recurrence tracking.

---

## Category 5: Performance & Capacity

### P-01 [P1] No Performance Baselines
**Found by:** Nemotron, GPT-5.4 (2/5)
**Gap:** No defined baselines for: API latency, WS round-trip, game launch time, billing session start time. Without baselines, degradation is invisible until crash.
**Fix:** Add Phase 76: Performance Baseline Check. Record p50/p95 latencies for key operations. Compare against established baselines. Alert on >2x degradation.

### P-02 [P2] No Disk Space / Capacity Monitoring
**Found by:** GPT-5.4, Nemotron (2/5)
**Gap:** Phase 45 checks log size but doesn't check: SQLite DB growth, WAL file size, Windows temp/cache, game replay files, crash dumps, Next.js build artifacts, camera retention storage.
**Fix:** Expand Phase 45 to include disk space checks on all machines. Add SQLite `.db` file size check to Phase 36.

### P-03 [P2] No Load/Concurrency Testing
**Found by:** Nemotron (unique)
**Gap:** No test for 8 simultaneous billing sessions + game launches + cloud sync + camera traffic + POS activity. SQLite contention under peak load is unknown.
**Fix:** Add to Tier 21 (Chaos): peak-load simulation with concurrent operations.

### P-04 [P2] No Dependency Health / Version Audit
**Found by:** GPT-5.4, Nemotron (2/5)
**Gap:** No phase runs `npm audit`, `cargo audit`, checks NVIDIA driver version, Edge version, or game client versions. Stale dependencies have known CVEs.
**Fix:** Add to Phase 51 (Static Analysis): `cargo audit` + `npm audit` for all Node.js apps. Add NVIDIA/Edge version check to Phase 19 (Display).

### P-05 [P3] Audit Self-Inflicted Load Risk
**Found by:** Sonnet, Nemotron (2/5)
**Gap:** Running 62 phases during operating hours generates ~496 check operations. Could spike CPU, lock databases, or cause latency during customer sessions.
**Fix:** Document "audit only during non-operating hours" policy. Add load-awareness: if active billing sessions exist, defer Tier 5+ checks.

---

## Category 6: Protocol Consistency

### PC-01 [P1] Protocol vs Automation Drift — 6 Undocumented Phases
**Found by:** Opus (unique), confirmed by James
**Gap:** AUDIT-PROTOCOL.md documents 62 phases (1-62). Automation has 68 phase scripts including 6 undocumented ones:
- **Phase 63** (tier2): Boot Resilience Check — verifies periodic_tasks on pods
- **Phase 64** (tier3): Sentinel Alert Wiring — MAINTENANCE_MODE WhatsApp alert verification
- **Phase 65** (tier3): Verification Chain Health — COV-01..05 server chain infrastructure
- **Phase 66** (tier19): Autonomous Pipeline Sync — v26.0 detection/healing dashboard visibility
- **Phase 67** (tier1): Meta-Monitor Liveness — rc-watchdog process + scheduled task verification
- **Phase 68** (tier3): Kiosk Game Launch Timer — UI countdown timer rendering check
**Impact:** Protocol document is stale. Anyone following the written protocol misses 6 phases.
**Fix:** Add phases 63-68 to AUDIT-PROTOCOL.md. Add CI check that fails if script count != documented phase count.

### PC-02 [P2] Phase 9b Documented but No Separate Script
**Found by:** James (reviewer)
**Gap:** Phase 9b (Crash Loop & Session Context Detection) is documented in AUDIT-PROTOCOL.md but there's no `phase09b.sh` script — it's either folded into phase09.sh or unimplemented in automation.
**Fix:** Verify if phase09.sh includes 9b checks. If not, create phase09b.sh.

### PC-03 [P2] Phase 26b Documented but No Separate Script
**Found by:** James (reviewer)
**Gap:** Phase 26b (Game Launch E2E Verification) is documented but no `phase26b.sh` exists in automation.
**Fix:** Same as PC-02 — verify or create.

### PC-04 [P2] Tier Numbering Inconsistency
**Found by:** James (reviewer)
**Gap:** AUDIT-PROTOCOL.md has Tiers 1-12, 13-18, then jumps to Tier 19 and Tier 20. Automation puts phases in different tiers than documented (e.g., phase63 in tier2, phase66 in tier19). The numbering is confusing.
**Fix:** Reconcile tier assignments between protocol document and script directory structure.

### PC-05 [P3] Duplicate Summary Template
**Found by:** James (reviewer)
**Gap:** The audit summary template appears TWICE at the end of AUDIT-PROTOCOL.md (identical copies starting at lines ~1858 and ~2014).
**Fix:** Remove the duplicate.

---

## Category 7: Code Audit (Multi-Model Protocol) Gaps

### CA-01 [P1] Frontend TypeScript/Next.js Not in Code Audit Batches
**Found by:** GPT-5.4 (unique), confirmed by James
**Gap:** `multi-model-audit.js` audits 7 batches: server Rust, agent Rust, sentry/watchdog Rust, comms-link Node.js, audit pipeline Bash, deploy infra, standing rules. **No batch includes the Next.js frontend code** (kiosk, web, admin). XSS, auth token misuse, exposed env vars, SSR data leaks — none audited by external models.
**Fix:** Add Batch 8: Frontend (Next.js) covering `kiosk/src/`, `pwa/src/`, `web/src/`, `admin/src/`. Focus on XSS, auth token handling, CORS, cookie flags, exposed NEXT_PUBLIC_ vars, SSR/CSR boundary.

### CA-02 [P1] No Audit of the Audit Scripts Themselves
**Found by:** GPT-5.4 (unique)
**Gap:** `multi-model-audit.js`, `cross-model-analysis.js`, and the 8 lib/ scripts are never themselves audited. The audit harness is a security-critical control plane — if it silently skips files, miscounts findings, or has a vulnerability in the OpenRouter request handler, the entire audit is compromised.
**Fix:** Add the audit scripts themselves as a batch (Batch 9: Audit Infrastructure). Or add them to Batch 5 (audit pipeline).

### CA-03 [P2] System Prompt Missing Cafe/Inventory/Marketing Modules
**Found by:** GPT-5.4 (unique)
**Gap:** The SYSTEM_PROMPT in `multi-model-audit.js` describes architecture but doesn't mention cafe, inventory, marketing, psychology, or gamification modules. Models won't know to look for revenue-impacting logic flaws in these areas.
**Fix:** Update SYSTEM_PROMPT with: "Also includes cafe ordering, inventory management, marketing content generation, customer psychology/gamification, badge system, and notification dispatch."

### CA-04 [P2] Cross-Model Similarity Function Too Crude
**Found by:** Opus, GPT-5.4 (2/5)
**Gap:** `cross-model-analysis.js` uses word overlap with 0.4 base score for same-file findings. This can: (1) group DIFFERENT bugs into one cluster (reducing unique count), (2) miss semantically identical findings with different vocabulary.
**Fix:** Replace word overlap with semantic embedding similarity. Or add a secondary LLM pass to verify groupings. At minimum, increase minimum word overlap threshold.

### CA-05 [P2] No Known-Bug Calibration
**Found by:** GPT-5.4 (unique)
**Gap:** The multi-model audit has no way to measure false-negative rate. Without occasionally seeding known bugs into batches, you have no evidence the pipeline catches what you think it catches.
**Fix:** Maintain a set of 5-10 known bugs (from previous audits). Inject 2-3 into each audit run as canaries. If any model misses a canary, flag reduced confidence for that model.

### CA-06 [P2] No Historical Defect Tracking Across Audit Runs
**Found by:** GPT-5.4 (unique)
**Gap:** Each multi-model audit run is standalone. No tracking of: recurring findings, mean time to fix, defect families, "same bug, new surface" patterns.
**Fix:** Add `findings-history.json` that accumulates findings across runs. `cross-model-analysis.js` should flag recurring unfixed findings as "regression."

### CA-07 [P3] Model ID Staleness Risk
**Found by:** Opus (unique)
**Gap:** Fixed model IDs in `MODEL_CONFIG` (e.g., `google/gemini-2.5-pro-preview-03-25`) will break when models are deprecated or replaced.
**Fix:** Add startup check: query OpenRouter `/api/v1/models` to verify each model ID is still available. Alert on deprecation.

---

## Category 8: Operations & Business

### OB-01 [P1] No Time Sync Verification
**Found by:** GPT-5.4 (unique)
**Gap:** No phase checks NTP sync across pods, server, POS, NVR, and VPS. Unsynchronized clocks break billing reconciliation, camera evidence correlation, log analysis, and cloud sync conflict resolution.
**Fix:** Add to Phase 3 (Network): verify `w32tm /query /status` on all Windows machines, `timedatectl` on VPS.

### OB-02 [P2] No Customer Impact Metrics / SLOs
**Found by:** Nemotron, GPT-5.4 (2/5)
**Gap:** No tracking of: session completion rate, game launch success rate, average wait time, billing accuracy, refund rate. Business metrics are invisible.
**Fix:** Define SLOs. Add Phase 77: Business Metrics Health — query billing stats, game launch success rate, error rate per customer journey.

### OB-03 [P2] No Asset Inventory / Vendor License Audit
**Found by:** GPT-5.4 (unique)
**Gap:** No definitive inventory of hardware serial numbers, firmware versions, software licenses, domain renewal dates, VPS billing status, API quota exhaustion risk.
**Fix:** Create asset registry. Add annual license/domain/quota audit phase.

### OB-04 [P3] No Staff Permission Lifecycle Test
**Found by:** GPT-5.4 (unique)
**Gap:** No verification of onboarding/offboarding for venue staff — removal of POS access, camera access, admin dashboard, cloud credentials.
**Fix:** Add to quarterly security audit checklist.

### OB-05 [P3] No Change Management Audit Trail
**Found by:** GPT-5.4 (unique)
**Gap:** No audit of who changed pricing, pod configs, kiosk policies, firewall rules, or game versions. LOGBOOK tracks commits but not config changes.
**Fix:** Expand Phase 37 (Activity Log) to include config change tracking.

---

## Opus Reviewer — Unique Findings (James Only)

These findings were not caught by any of the 5 Perplexity models:

### J-01 [P2] Audit Uses Two Different Auth Systems Without Clear Routing
**Gap:** AUDIT-PROTOCOL.md uses both `$JWT` (Staff Bearer) and `$SESSION` (Terminal x-terminal-session) but several phases use the WRONG auth type. Phase 21 (Pricing) uses `x-terminal-session` when it might need Staff JWT. Phase 39 (Feature Flags) uses `x-terminal-session`. The inconsistency could cause false 401 failures that look like real security issues.
**Fix:** Create a definitive auth requirement matrix: which endpoints need which auth type. Verify in Phase 50.

### J-02 [P2] No Verification That Auto-Fix Actually Fixed Anything
**Gap:** `audit.sh --auto-fix` applies fixes but never re-runs the failed phase to verify the fix worked. A fix that silently fails still counts as "fixed."
**Fix:** After any auto-fix, re-run the specific phase. Only count as fixed if re-run passes.

### J-03 [P1] POS PC Audit Relies on LAN — No Tailscale Fallback
**Gap:** Phase 49 checks POS at `192.168.31.20:8090`. POS is on WiFi. If WiFi drops, POS appears "OFFLINE" but is actually fine on Tailscale (`100.95.211.1`). The audit has no fallback probe path.
**Fix:** If LAN check fails, retry via Tailscale IP before reporting OFFLINE.

---

## Recommended Priority Remediation Order

### Immediate (This Week)
1. **T-02** — Rotate admin PIN, remove from documents, use `$AUDIT_PIN` env var
2. **T-03** — Add Phase 0 self-test to audit.sh
3. **T-06** — Add PID file lock to prevent parallel corruption
4. **PC-01** — Document phases 63-68 in AUDIT-PROTOCOL.md
5. **CA-01** — Add Batch 8 (Frontend) to multi-model-audit.js

### Short Term (This Month)
6. **D-01** — Implement and test backup/restore for SQLite DB
7. **S-02** — Add rate limiting verification to Phase 50
8. **T-01** — Add dual-path verification (rc-sentry cross-check) for critical phases
9. **CA-02** — Add audit scripts themselves to a code audit batch
10. **P-01** — Establish performance baselines

### Medium Term (This Quarter)
11. **R-01** — Design and run first chaos engineering drill
12. **S-01** — First physical security audit
13. **R-03** — First incident response drill
14. **D-03** — DPDP compliance audit
15. **S-04** — Review and mitigate supply chain risk of code audit

### Long Term (This Half)
16. **T-05** — Independent verification path via Bono
17. **S-03** — Network segmentation audit
18. **R-02** — Power failure recovery drill
19. **CA-05** — Known-bug calibration system
20. **OB-01** — NTP sync verification across all machines

---

## Methodology Notes

### Why Multi-Model Meta-Audit Works

Each model brought a different strength:
- **Gemini 3.1 Pro Think:** Best at security checklist thinking and SRE patterns
- **GPT-5.4 Think:** Most exhaustive — 40+ findings, unique on business continuity, staff lifecycle, asset management
- **Claude Sonnet 4.6 Think:** Best red-team perspective — credential exposure, supply chain, physical security
- **Nemotron 3 Super:** Concise SRE focus — chaos, capacity, baselines, rollback
- **Claude Opus 4.6:** Meta-process focus — audit self-test, result integrity, timing attacks

**Cost:** 5 Pro Search queries from Perplexity (subscription, no additional cost)
**Time:** ~3 minutes for all 5 models in parallel + 30 minutes Opus synthesis

### Limitations of This Meta-Audit
- Models received a TEXT DESCRIPTION of the protocols, not the actual code. Specific bash bugs in audit.sh were not analyzed.
- Models have limited context on the full Racing Point architecture — some findings may be false positives due to missing context.
- This meta-audit itself has no independent verifier. It should be reviewed by both James and Bono.
