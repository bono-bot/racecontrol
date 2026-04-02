# Deployment Changelog: 3d36200d

**Build:** `3d36200d` | **Date:** 2026-04-02 | **Previous:** Server `0267ce15`, Pods `5db7804d`, Sentry `25310c2a`

## Milestones Included

| Milestone | Phases | Key Changes |
|-----------|--------|-------------|
| v34.0 Time-Series Metrics | 285-291 | SQLite TSDB, metrics query API, Next.js dashboard, Prometheus export, WhatsApp alerts |
| v35.0 Structured Retraining | 290-294 | Model eval rollups, KB promotion persistence, model reputation sync, retrain export, intelligence report v2 |
| v36.0 Config Management | 295-299 | AgentConfig in rc-common, server-pushed config via WS, admin config editor, game presets, policy rules engine |
| v37.0 Data Durability | 300-304 | SQLite backup pipeline, cloud sync v2, event archive, venue_id migrations (44 tables), fleet deploy automation |
| v38.0 Security Hardening | 305-309 | mTLS internal HTTP, per-pod JWT WS auth, audit log hash chain, RBAC, security audit script |
| v38.0+ Meshed Intelligence v2 | 60df0f53 | Semantic health probes, dependency chain alerting, pm2 auto-restart, staff alert banners, synthetic monitors |
| v39.0 Session Traceability | Phase 310 | session_id propagation through activity_log, GameTracker, LaunchEvent (81 call sites) |

## DB Schema Changes (AUTO-MIGRATE on first run)

All use idempotent `ALTER TABLE ADD COLUMN` / `CREATE TABLE IF NOT EXISTS`:

| Table | New Columns | Migration |
|-------|-------------|-----------|
| ALL 44 major tables | `venue_id TEXT DEFAULT 'racingpoint-hyd-001'` | v37.0 Phase 303 |
| `pod_activity_log` | `session_id TEXT` + `entry_hash TEXT` + `previous_hash TEXT` | v38/310 |
| `launch_events` | `session_id TEXT` | v39.0 Phase 310 |
| `model_evaluations` | `model_id`, `trigger_type`, `actual_outcome` (unified schema) | v35+v37 merge |
| `system_events` | NEW TABLE — structured event archive | v37.0 Phase 302 |
| `pod_configs` | NEW TABLE — server-pushed config storage | v36.0 Phase 296 |
| `policy_rules` | NEW TABLE — policy engine rules | v36.0 Phase 299 |
| `game_presets` | NEW TABLE — preset library | v36.0 Phase 298 |
| `backup_status` | NEW TABLE — backup pipeline tracking | v37.0 Phase 300 |
| `metrics_*` | NEW TABLES — TSDB ring buffer + rollups | v34.0 |
| `fleet_solutions`, `model_evaluations`, `metrics_rollups` | Cloud sync v2 columns | v37.0 Phase 301 |
| `app_health_history` | NEW TABLE — semantic health probe results | v38.0+ |
| `synthetic_probes` | NEW TABLE — golden-path probe results | v38.0+ |

## Behavioral Changes (CRITICAL for post-deploy monitoring)

### Billing & Launch
- **BillingTick now includes `tick_seq: u64`** — monotonic counter for ordering. Old agents/kiosk ignore it (serde default=0).
- **LaunchGame WS send retries once** with 3s delay before falling through to 90-120s timeout.
- **Crash relaunch already has 5s cooldown** — no change, was already implemented.
- **GameTracker now tracks `billing_session_id`** — captured from billing timer at launch time.
- **`log_pod_activity()` now takes 8 params** (was 7) — new `session_id: Option<&str>` last param. All 81 callers updated.

### Monitoring & Health
- **pod_monitor: skip-once pattern** — first stale heartbeat is logged but NOT marked Offline. Second consecutive stale → Offline. **Doubles initial offline detection time from 1 cycle to 2 cycles.**
- **app_health_monitor: retry-once** — single HTTP timeout retries after 2s before marking "unreachable".
- **pod_healer: retry-once** — lock screen HTTP check retries after 2s before ForceRelaunchBrowser.
- **deploy.rs: retry-once** — WS connectivity check retries after 2s before excluding pod.
- **Meshed Intelligence v2**: semantic health validation, dependency chain batched alerts, pm2 auto-restart (budget-limited), synthetic transaction monitor every 5min.

### Security
- **Audit log hash chain** — every activity_log entry includes SHA-256 `entry_hash` + `previous_hash`. `GET /api/v1/audit/verify` checks chain integrity.
- **Per-pod JWT WS auth** — pods get JWT after PSK-authenticated connection, auto-rotates 1hr before expiry.
- **mTLS config structs** — TLS config present but NOT enforced (enabled=false default). No behavioral change until certs generated.

### Data
- **venue_id on all tables** — default `racingpoint-hyd-001`, no functional change. All INSERTs now include venue_id.
- **Event archive** — `event_archive::spawn()` runs in background, daily JSONL export + 90-day retention.
- **Backup pipeline** — hourly VACUUM INTO, 7-daily + 4-weekly rotation, nightly SCP to VPS.
- **Cloud sync v2** — fleet_solutions, model_evaluations, metrics_rollups now synced to VPS.

## Known Risks & Watchpoints

| Risk | Severity | What to Watch | Rollback |
|------|----------|---------------|----------|
| 44-table venue_id migration | LOW | First run only — ALTER TABLE on all tables. May take 5-10s on large DBs. | No rollback needed — additive only |
| pod_monitor skip-once | LOW | Offline detection now takes 2 cycles (~60s default) instead of 1. Acceptable tradeoff. | Revert `first_stale_at` check in pod_monitor.rs |
| BillingTick tick_seq field | LOW | Old kiosk/agent ignores new field (serde default). No breakage. | Remove field from protocol.rs |
| log_pod_activity 8th param | NONE | All callers updated. Compile-enforced. | N/A |
| Meshed Intelligence v2 pm2 restart | MEDIUM | New auto-restart logic for server apps. Budget-limited with cooldown. Watch for restart storms. | Set `restart_enabled: false` in config |
| Synthetic monitor | LOW | New 5-min golden-path probes. Extra HTTP load on kiosk/web health endpoints. | Comment out `spawn_synthetic_monitor()` in main.rs |

## Post-Deploy Verification Checklist

```bash
# 1. Build ID matches
curl -s http://192.168.31.23:8080/api/v1/health | jq .build_id
# Expected: "3d36200d"

# 2. DB migrations ran (check new tables exist)
# venue_id column:
ssh server "sqlite3 C:/RacingPoint/racecontrol.db \"PRAGMA table_info(billing_sessions)\"" | grep venue_id
# system_events table:
ssh server "sqlite3 C:/RacingPoint/racecontrol.db \"SELECT count(*) FROM system_events\""

# 3. Audit hash chain
curl -s http://192.168.31.23:8080/api/v1/audit/verify | jq .

# 4. Fleet health (all pods connected)
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.pods[] | {pod_number, ws_connected, build_id}'

# 5. Metrics endpoint
curl -s http://192.168.31.23:8080/api/v1/metrics/names | jq .

# 6. App health (Meshed Intelligence v2)
curl -s http://192.168.31.23:8080/api/v1/app-health | jq .

# 7. Kiosk session_id trace (after a test session)
# ssh server "sqlite3 C:/RacingPoint/racecontrol.db \"SELECT session_id, category, action FROM pod_activity_log WHERE session_id IS NOT NULL LIMIT 5\""
```

## Meshed Intelligence Context Update

The following should be incorporated into FLEET_CONTEXT for AI diagnosis after this deploy:

```
DEPLOY CONTEXT (3d36200d, 2026-04-02): 7 milestones merged (v34-v39). Key behavioral changes:
- pod_monitor uses skip-once pattern (2 cycles to mark Offline, not 1)
- app_health_monitor retries once before "unreachable"
- BillingTick includes tick_seq for ordering
- LaunchGame WS send retries once with 3s delay
- All activity_log entries now include session_id (NULL for non-billing events)
- 44 tables have venue_id column (default: racingpoint-hyd-001)
- Audit log hash chain active (entry_hash + previous_hash on pod_activity_log)
- Meshed Intelligence v2: semantic probes, dependency chain, pm2 restart, synthetic monitor
Known post-deploy risks: initial DB migration may take 5-10s, skip-once doubles offline detection time.
```
