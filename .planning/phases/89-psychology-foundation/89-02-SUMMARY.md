---
phase: 89-psychology-foundation
plan: 89-02
subsystem: psychology-engine
tags: [psychology, badges, streaks, notifications, whatsapp, discord, pwa, dispatch]
dependency_graph:
  requires: [89-01]
  provides: [evaluate_badges, update_streak, queue_notification, is_whatsapp_budget_exceeded, spawn_dispatcher]
  affects: [billing.rs, psychology.rs]
tech_stack:
  added: []
  patterns:
    - sqlx query_scalar for single-value DB lookups
    - INSERT OR IGNORE for idempotent badge awards
    - tokio::spawn + interval loop for dispatcher
    - IST FixedOffset east_opt(5*3600+30*60) for timezone-aware date comparison
    - pub(crate) fn promotion for cross-module reuse
key_files:
  created: []
  modified:
    - crates/racecontrol/src/psychology.rs
    - crates/racecontrol/src/billing.rs
decisions:
  - format_wa_phone promoted to pub(crate) in billing.rs — avoids duplication, keeps phone formatting logic in one place
  - STREAK_GRACE_DAYS+7 = 14-day total window — weekly visits + 1-week grace for customers who miss a week
  - send_pwa_notification uses DB-record pattern (not WebSocket) — deferred to Phase 3 per plan design note
  - drain_notification_queue processes expiry before batch fetch — ensures clean state before routing
  - is_whatsapp_budget_exceeded uses date(sent_at) = date('now') — SQL-native date comparison, no Rust date math
metrics:
  duration_minutes: 12
  completed_date: "2026-03-21"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
  tests_added: 15
  tests_total: 28
---

# Phase 89 Plan 02: Psychology Engine Logic Implementation Summary

**One-liner:** Full badge evaluation + streak tracking + WhatsApp budget + multi-channel notification dispatcher implemented via sqlx queries and IST-aware date logic.

## What Was Built

### Task 1: Badge Evaluation and Streak Tracking

**`evaluate_badges(state, driver_id)`**
- Loads all active badge definitions from `achievements WHERE is_active = 1`
- Loads already-earned badge IDs for the driver from `driver_achievements`
- For each unearned badge: calls `resolve_metric` to get the live value, evaluates criteria, awards via `INSERT OR IGNORE INTO driver_achievements`

**`resolve_metric(state, driver_id, metric)`** (private helper)
- Handles all 7 MetricType variants: TotalLaps, UniqueTracks, UniqueCars, SessionCount, PbCount, StreakWeeks, FirstLap
- Each variant maps to a specific SQL query against the appropriate table

**`update_streak(state, driver_id)`**
- Gets today's date in IST (UTC+5:30 via `FixedOffset::east_opt(5*3600+30*60)`)
- Fetches existing streak: id, current_streak, longest_streak, last_visit_date, grace_expires_date
- Same-day visit: returns immediately (idempotent)
- Within grace (`today <= grace_expires_date`): increments current_streak, updates longest if beaten
- Past grace: resets to 1, preserves longest_streak, resets streak_started_at
- New driver: inserts with current_streak=1, longest_streak=1, grace = today + 14 days

### Task 2: Notification Budget, Queue, and Dispatcher

**`is_whatsapp_budget_exceeded(state, driver_id)`**
- Counts rows in nudge_queue WHERE channel='whatsapp' AND status='sent' AND date(sent_at) = date('now')
- Returns true if count >= WHATSAPP_DAILY_BUDGET (2)

**`queue_notification(state, driver_id, channel, priority, template, payload_json)`**
- Inserts into nudge_queue with status='pending' and expires_at = now + 1 day

**`spawn_dispatcher(state)`**
- Spawns a tokio task that runs `drain_notification_queue` every 30 seconds
- Also calls `cleanup_old_nudges` each cycle

**`drain_notification_queue(state)`** (private)
1. Marks expired entries (expires_at < now) as 'expired'
2. Fetches batch of DISPATCHER_BATCH_SIZE=10 pending entries, ordered by priority ASC, scheduled_at ASC
3. For each: checks WhatsApp budget → marks 'throttled' if exceeded
4. Routes to channel: WhatsApp (Evolution API), Discord (webhook), PWA (DB record)
5. Marks 'sent' on success or 'failed' on delivery failure

**`send_whatsapp`**: POST to `{evolution_url}/message/sendText/{evolution_instance}` with `apikey` header, 5s timeout

**`send_discord`**: POST to `config.integrations.discord.webhook_url` with `{content}` body

**`send_pwa_notification`**: INSERT into nudge_queue with status='sent' (PWA polls this table)

**`resolve_template`**: Replaces `{key}` placeholders with values from JSON payload object

**`cleanup_old_nudges`**: DELETEs resolved entries older than NUDGE_TTL_DAYS=7

**billing.rs change**: `format_wa_phone` promoted from `fn` to `pub(crate) fn` so psychology.rs can call `crate::billing::format_wa_phone`

## Test Coverage

28 tests total (13 from Plan 01 + 15 new):

| Test | What it verifies |
|------|-----------------|
| test_evaluate_badges_awards_badge_for_100_laps | Badge awarded when driver meets threshold |
| test_evaluate_badges_skips_already_earned | No duplicate badge rows (INSERT OR IGNORE) |
| test_evaluate_badges_does_not_award_below_threshold | Non-qualifying drivers not awarded |
| test_update_streak_creates_new_row | First visit creates streak with current=1, longest=1 |
| test_update_streak_same_date_does_not_change | Same-day visit is idempotent |
| test_update_streak_within_grace_increments | Visit within grace window increments streak |
| test_update_streak_after_grace_resets | Expired grace resets to 1, longest preserved |
| test_budget_not_exceeded_with_zero_sent | 0 sent → not exceeded |
| test_budget_not_exceeded_with_one_sent | 1 sent → not exceeded |
| test_budget_exceeded_with_two_sent | 2 sent → exceeded |
| test_queue_notification_inserts_pending_row | queue_notification creates pending row |
| test_drain_throttles_whatsapp_when_budget_exceeded | Over-budget WhatsApp marked throttled |
| test_drain_marks_expired_entries | Entries past expires_at marked expired |
| test_resolve_template_substitutes_placeholders | {name} → "Uday" substitution works |
| test_resolve_template_plain_string_passthrough | Template with no placeholders unchanged |

## Verification

```
cargo check -p racecontrol-crate → Finished (0 errors)
cargo test -p racecontrol-crate --lib psychology::tests → 28 passed; 0 failed
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] billing.rs format_wa_phone visibility**
- **Found during:** Task 2 implementation
- **Issue:** `format_wa_phone` in billing.rs was private (`fn`), blocking psychology.rs from calling `crate::billing::format_wa_phone`
- **Fix:** Promoted to `pub(crate) fn` per plan's own note ("If so, either make it `pub(crate)` or copy the logic")
- **Files modified:** `crates/racecontrol/src/billing.rs`
- **Commit:** e98b011

## Self-Check: PASSED
