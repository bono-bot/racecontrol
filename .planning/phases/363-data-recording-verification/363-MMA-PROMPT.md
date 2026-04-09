# Phase 363 MMA Audit — Diagnostic Prompt (prepared 2026-04-10, NOT YET RUN)

**Scope:** Phase 363 Data Recording Verification — 3 plans, 6 commits, F-05 regression + GLD-C-04 grace window + CSV fallback cross-system bridge.

**Why MMA:** CLAUDE.md rule — MMA audit MANDATORY before deploying new cross-system bridges. Phase 363 introduces:
- NEW rc-agent → server HTTP bridge (`POST /api/v1/sessions/{id}/telemetry-fallback`)
- NEW racecontrol restart-safe hydration path (`hydrate_active_timers_from_db` — FIRST EVER active_timers hydration)
- MODIFIED billing finalize path (5s deferred finalize via tick loop re-check)
- MODIFIED lap_rejections INSERT path (grace_window_caught column)

## Ready-to-run commands

### Option A: Full consensus audit ($5-10 budget)

```bash
cd ~/racingpoint/racecontrol
export PATH="$PATH:/c/Users/bono/.cargo/bin"

# Uses saved key auto-loaded from data/openrouter-mma-key.txt
# If no saved key, auto-provisions via OPENROUTER_MGMT_KEY or Bono relay
MMA_SESSION_BUDGET=10 AUDIT_ALLOW_STALE=1 node scripts/multi-model-audit.js
```

This runs the full 14-batch consensus audit (8 code areas × 2 parts) — overkill for Phase 363 alone, but catches cross-cutting issues the targeted prompt might miss.

### Option B: Targeted single-model audit (cheapest, ~$0.50)

```bash
cd ~/racingpoint/racecontrol

# Load saved key
export OPENROUTER_KEY="$(cat data/openrouter-mma-key.txt 2>/dev/null)"
[ -z "$OPENROUTER_KEY" ] && { echo "No saved key — run full audit once to auto-provision"; exit 1; }

# Write diagnostic prompt to file (heredoc to avoid bash escaping)
cat > /tmp/phase-363-audit-prompt.txt <<'EOF'
[See "Diagnostic Prompt" section below — copy-paste from 363-MMA-PROMPT.md]
EOF

# Fire single model
curl -s -m 180 https://openrouter.ai/api/v1/chat/completions \
  -H "Authorization: Bearer $OPENROUTER_KEY" \
  -H "Content-Type: application/json" \
  -d "$(cat /tmp/phase-363-audit-prompt.txt | jq -Rs --arg model "deepseek/deepseek-r1-0528" '{
    "model": $model,
    "messages": [{"role":"user","content":.}],
    "max_tokens": 6000
  }')" | jq -r '.choices[0].message.content' > .planning/audits/PHASE-363-MMA-R1.md

# Review
cat .planning/audits/PHASE-363-MMA-R1.md
```

### Option C: Dual-reasoner audit ($1-2)

Run R1 + GPT-5.4 Nano in parallel. Compare findings. If both flag the same issue → high confidence. If one flags but not the other → medium confidence, manual review.

```bash
cd ~/racingpoint/racecontrol
export OPENROUTER_KEY="$(cat data/openrouter-mma-key.txt)"

for MODEL in "deepseek/deepseek-r1-0528" "openai/gpt-5.4-nano"; do
  SAFE_NAME=$(echo "$MODEL" | tr '/' '-')
  curl -s -m 180 https://openrouter.ai/api/v1/chat/completions \
    -H "Authorization: Bearer $OPENROUTER_KEY" \
    -H "Content-Type: application/json" \
    -d "$(cat /tmp/phase-363-audit-prompt.txt | jq -Rs --arg m "$MODEL" '{
      "model": $m, "messages": [{"role":"user","content":.}], "max_tokens": 6000
    }')" | jq -r '.choices[0].message.content' > ".planning/audits/PHASE-363-MMA-${SAFE_NAME}.md" &
done
wait
```

## Diagnostic Prompt

Copy the content below into the prompt file:

```
You are auditing Phase 363 of the Racing Point RaceControl monorepo (Rust/Axum backend + rc-agent pod binary + SQLite). Phase 363 closes three P0 data-loss gaps in the game-launch/billing pipeline and prevents a regression of F-05 (a real incident where the billing finalize path overwrote wallet_debit_paise before compute_refund() read it, losing ~Rs.162 per early-ended 30min session for an unknown duration).

# What changed

## Plan 01 — Session audit foundation (commits: e4784c51, 0b4e356c)

- 8 new billing_sessions columns: lap_count_expected, lap_count_actual, lap_count_flag, telemetry_coverage_pct, suspect, suspect_reasons, csv_fallback_received_at, lap_reject_grace_until
- New lap_rejections table with session_id column (per internal D-12 decision — column is named session_id, holds billing_session_id value at runtime)
- phase363_session_audit feature flag (kill switch)
- New session_audit.rs module: expected_laps, compute_lap_flag, coverage_pct, compute_suspect, run_session_audit
- BillingTimer.telemetry_seconds_covered: HashSet<u32> coverage histogram
- WS Telemetry handler updates coverage bucket via try_write() (non-blocking)
- post_session_hooks calls run_session_audit at session end
- cloud_sync.rs billing_sessions push payload extended with all 8 new columns

## Plan 02 — CSV fallback auto-sync (commits: 09be10e6, aadefeb6)

NEW cross-system bridge: rc-agent pod → server HTTP POST.

- Server: POST /api/v1/sessions/{id}/telemetry-fallback — service-key-gated endpoint in service_routes() alongside mesh_audit_seed_service, 50MB body limit via DefaultBodyLimit layer, multipart body, path traversal guard, writes to C:\RacingPoint\telemetry-fallback\{session_id}.csv, updates billing_sessions.csv_fallback_received_at = now()
- rc-agent: push_csv_fallback_inner (testable), push_csv_fallback (production wrapper), detached tokio::spawn in SessionEnded WS handler
- Read-before-clear pattern: reqwest multipart POST, remove_file ONLY on HTTP 200
- 7-attempt exponential retry: PRODUCTION_BACKOFFS = 2,4,8,16,32,64,128s = ~254s envelope
- Server URL derivation: ws_handler derives http:// base from config.core.url (ws:// → http://, split /ws)
- Auth: X-Service-Key header from RCAGENT_SERVICE_KEY env var (same value as server's sentry_service_key in racecontrol.toml)
- reqwest multipart feature added to rc-agent Cargo.toml (was missing, json feature alone insufficient)

## Plan 03 — Grace window + F-05 regression + restart hydration (commits: 7e46227b, 11450490, 1e3eff44)

- BillingTimer: 2 new fields (lap_reject_grace_until: Option<DateTime<Utc>>, pending_end_status: Option<BillingSessionStatus>)
- Default::default() added to BillingTimer so hydration can use ..Default::default()
- 4 BillingTimer construction sites updated with explicit None defaults + comments
- Session-end NORMAL paths set grace_until = now+5s, persist to billing_sessions.lap_reject_grace_until via UPDATE, DO NOT call end_billing_session directly
- Cancel/force-end paths bypass grace window (only normal "ran out of time" / "driver stopped" ends get deferred)
- Billing tick_all_timers loop (1s cadence) does snapshot-drop-then-finalize: collects (session_id, end_status) tuples under read guard, drops guard, iterates finalize loop
- hydrate_active_timers_from_db(billing, pool) — FIRST EVER startup hydration path for active_timers. SELECTs non-terminal rows OR rows with non-null lap_reject_grace_until, rebuilds BillingTimer with HashSet::new() (telemetry bucket lost on crash per D-05), Completed as conservative default for pending_end_status
- Wired into main.rs init (line 768) post-AppState construction, non-fatal on error
- record_lap_rejection() with grace_window_caught bool computed under short read guard, INSERT into lap_rejections with session_id column (NOT billing_session_id)
- F-05 CAS UPDATE at billing.rs:4154 UNCHANGED — SET clause is status/driving_seconds/ended_at/end_reason, wallet_debit_paise deliberately excluded
- F-05 regression tests: formula test (compute_refund(1800, 900, 70000) == 32500) + SQL invariant test (in-memory sqlite replays CAS UPDATE, verifies wallet_debit_paise retains pre-value)
- mod billing_grace submodule with 3 integration tests (catches_reject, expires_normally, restart_safe)
- Total 7 new tests, all green. racecontrol-crate full suite: 891 passed 0 failed. rc-agent-crate + rc-common: 254 passed.

# What I need you to audit

Rank findings P0 (blocker, block deploy) / P1 (important, fix in same phase) / P2 (minor, can defer) / P3 (nit). For each finding give: file + approximate line, why it's a problem, concrete fix recommendation, confidence (low/med/high), severity reasoning.

Focus areas (go deep, not shallow):

## 1. Concurrency + lock safety
- All new code in billing.rs involving active_timers.read()/write()
- Verify the snapshot-drop-then-finalize pattern in the tick loop — is the drop actually happening before any .await? Are there nested guards?
- Can hydrate_active_timers_from_db race against incoming WS messages that create new timers during the hydration window?
- What happens if two tick loop iterations both see the same expired timer between the read-guard release and the write-guard acquire?

## 2. Restart-safety correctness
- hydrate_active_timers_from_db conservatively defaults pending_end_status = Completed if grace_until was set. Is Completed actually the right status? What if the session was being cancelled when the crash hit?
- telemetry_seconds_covered = HashSet::new() on hydration. Does this mean coverage drops to 0 and every hydrated session becomes suspect=true?
- What if the server crash happened DURING end_billing_session() finalize (between CAS UPDATE and row-status-transition)? Does hydration correctly resume or does it leave the session in a half-finalized limbo?
- Is there a window where a hydrated timer + a fresh WS-created timer for the same pod_id can both exist?

## 3. Cross-system bridge (CSV fallback)
- Auth: service-key is shared between rc-agent → server and sentry → server. If service_key leaks via a compromised pod, can an attacker POST arbitrary telemetry-fallback files to any session_id? Is there session ownership validation?
- 50MB body limit: what if a malicious pod sends 50MB of data 1000x per second? Is there rate limiting?
- Path traversal guard: is it sufficient? The filename is {session_id}.csv — what if session_id contains ".." or "/"?
- Retry envelope is 254s. What if the session ends, push_csv_fallback is spawned, but rc-agent crashes before the retry loop completes? Is the CSV preserved on the pod for next-boot retry, or lost?
- HTTP client: does it validate the server's TLS cert? (If the bridge is plaintext http://, is that acceptable inside the LAN?)

## 4. F-05 regression test strength
- Test 1 (compute_refund formula): uses compute_refund(1800, 900, 70000) == 32500. But the PLAN said 35000. The auto-fix doc claims 32500 is right because compute_refund internally calls best_rate_for_minutes(15, 2500, 75000, 90000) = 37500. **Is this actually correct?** Walk through the rate calculation manually. If 32500 is wrong, the entire F-05 regression test is locking the wrong value.
- Test 2 (SQL invariant): copies the UPDATE SET clause shape. If a future developer refactors billing.rs:4154 and the test hardcodes a stale shape, does the test still catch the bug? Or does it rot?
- Does the test cover the interaction with grace window deferred finalize? What if the deferred finalize path itself reintroduces the F-05 bug?

## 5. Billing FSM invariants
- Scope guard: "cancel/force-end paths bypass grace window". What if a cancel arrives DURING the grace window (after grace was set but before tick loop fires)? Does the cancel override grace window, race with the deferred finalize, or something else?
- Grace window sets pending_end_status = Some(Completed). What if the session was actually ending as EndedEarly? Does that end_status flow through correctly?
- The 5s grace + 1s tick cadence = worst-case 6s additional customer-visible latency per D-10. For a driver who ended early and wants their refund NOW, is that 6s visible as "refund processing..." or does it appear as a broken state?

## 6. Lap rejection column naming (D-12)
- Column is session_id per D-12 decision. Is this consistent with laps.session_id (also stores billing_session_id value)? Grep for any code that treats lap_rejections.session_id as a driver_session_id or a sessions.id — if there's ambiguity, flag it.
- Is there a foreign key constraint? What happens if a lap_rejection arrives for a billing_session_id that doesn't exist in billing_sessions?

## 7. Test coverage gaps
- The 7 new tests cover the happy path. List what's NOT tested:
  - Concurrent session-ends on multiple pods
  - Server restart mid-CSV-push
  - Lap reject during the 1ms window between tick loop finalize detection and guard acquisition
  - Hydration with corrupt lap_reject_grace_until string (invalid RFC3339)
  - CSV fallback when the session_id doesn't exist in DB
  - Service-key rotation mid-push
  Rank each gap by risk.

# Output format

Respond in this exact structure:

```
## Phase 363 MMA Audit — Findings

### P0 blockers (must fix before deploy)
1. [area] [file:line] - description
   - why: ...
   - fix: ...
   - confidence: ...

### P1 important (fix in phase)
...

### P2 minor (can defer)
...

### P3 nits
...

### Test coverage gaps (ranked by risk)
...

### F-05 formula verification
- compute_refund(1800, 900, 70000) = ? (show math)
- is 32500 correct? or is the test wrong?
- recommended assertion value:

### Deploy readiness score (0-10)
- concurrency: X/10
- restart safety: X/10
- cross-system bridge: X/10
- F-05 regression: X/10
- overall: X/10

### Ready to ship?
YES / NO + 1-line reason.
```
```

## Files + line ranges to feed into the model

If using a tool that automates file packaging (the multi-model-audit.js script does this), point it at:
- `crates/racecontrol/src/billing.rs` (full file — ~9000 lines, the entire billing module)
- `crates/racecontrol/src/main.rs` lines 760-800 (hydration call site + surrounding init)
- `crates/racecontrol/src/api/routes.rs` — search for `telemetry_fallback_handler` and `service_routes()`
- `crates/rc-agent/src/csv_lap_fallback.rs`
- `crates/rc-agent/src/ws_handler.rs` — search for `push_csv_fallback` and `SessionEnded`
- `crates/rc-agent/Cargo.toml` (for multipart feature verification)
- `crates/racecontrol/Cargo.toml`
- `crates/rc-common/src/protocol.rs` (for CoreToAgentMessage + AgentMessage shape)
- `.planning/phases/363-data-recording-verification/363-01-PLAN.md` + `363-02-PLAN.md` + `363-03-PLAN.md` (for plan-vs-code comparison)
- `.planning/phases/363-data-recording-verification/363-01-SUMMARY.md` + `363-02-SUMMARY.md` + `363-03-SUMMARY.md` (for executor deviation notes)
- `.planning/audits/ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md` (for F-05 historical context)

## Pre-run checklist

Before actually running the audit (not done yet — user must approve spend):

1. Check saved key exists: `ls -la data/openrouter-mma-key.txt`
2. Confirm test suite still green: `cargo test -p racecontrol-crate --lib -- billing::` (should be 891 passed)
3. Confirm no uncommitted changes to billing.rs or main.rs: `git status crates/racecontrol/src/billing.rs crates/racecontrol/src/main.rs`
4. Set budget ceiling: `export MMA_SESSION_BUDGET=10`
5. Pick mode: full consensus (A) / targeted single (B) / dual-reasoner (C)
6. Fire command (see Options A/B/C above)
7. Review findings: `cat .planning/audits/PHASE-363-MMA-*.md`
8. Triage P0/P1/P2/P3, fix P0+P1 before deploy, log P2+P3 as deferred

## Expected cost

- Option A (full consensus): $5-10, 14 batches, ~60 minutes wall clock (5 parallel API calls per batch × 14 batches)
- Option B (single R1): ~$0.30-0.50, 1 API call, ~3 minutes
- Option C (dual reasoner): ~$1-2, 2 parallel API calls, ~3 minutes

## Status

**DRY-RUN PASSED 2026-04-10.** 14 audit batches validated, vendor diversity 4-5 families per batch, budget default $5, saved OpenRouter key loaded successfully, script healthy.

**NOT YET RUN.** Waiting for user approval to spend OpenRouter credits.

**Recommendation:** Option C (dual R1 + GPT-5.4 Nano) — cheap ($1-2), fast (~3min), catches most real bugs via consensus, explicit dual reasoning mode covers both architecture-level and trace-level bugs per CLAUDE.md.
