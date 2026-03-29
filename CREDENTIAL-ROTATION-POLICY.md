# Credential Rotation Policy — Racing Point eSports

## M14-SEC: All credentials must be rotated on the schedule below.

### Rotation Schedule

| Credential | Location | Rotation | Last Rotated | Owner |
|-----------|----------|----------|--------------|-------|
| JWT Secret | racecontrol.toml `[auth].jwt_secret` | 90 days | UNKNOWN | Uday |
| Admin PIN | racecontrol.toml `[auth].admin_pin_hash` | 90 days | UNKNOWN | Uday |
| COMMS_PSK | comms-link .env / james watchdog | 90 days | UNKNOWN | James |
| Sentry Service Key | racecontrol.toml `[pods].sentry_service_key` | 90 days | 2026-03-28 | James |
| Evolution API Key | racecontrol.toml `[whatsapp].evolution_api_key` | 90 days | UNKNOWN | Uday |
| Relay Secret | racecontrol.toml `[bono].relay_secret` | 90 days | UNKNOWN | James |
| Terminal Secret | racecontrol.toml `[cloud].terminal_secret` | 90 days | UNKNOWN | Uday |
| NVR Password | rc-sentry-ai env `NVR_PASSWORD` | 180 days | NEVER (hardcoded until M11 fix) | Uday |
| Google OAuth (marketing) | marketing env vars | 180 days | NEVER (hardcoded until M9 fix) | Uday |
| OpenRouter API Key | env `OPENROUTER_KEY` | On compromise only | N/A | James |

### Rotation Procedure

1. Generate new credential (use `openssl rand -hex 32` for secrets, `openssl rand -hex 3` for PINs)
2. Update the config file / environment on ALL machines that use it:
   - Server .23 (`racecontrol.toml`)
   - James .27 (comms-link .env, watchdog env)
   - Bono VPS (pm2 env, comms-link)
   - All 8 pods (rc-agent.toml — for sentry_service_key)
   - POS .20 (rc-pos-agent.toml)
3. Restart affected services on ALL machines
4. Verify connectivity between all pairs (exec relay, fleet health, cloud sync)
5. Update this table with the rotation date
6. **Git filter-repo** old values from git history if they were ever committed

### Credentials That Were Hardcoded (Immediate Rotation Required)

These credentials appeared in source code and MUST be rotated:

| Credential | Where It Was | Commit That Fixed It | Status |
|-----------|-------------|---------------------|--------|
| Admin PIN `261121` | auth/admin.rs test, venue_shutdown.rs | `2dfa3394` | **ROTATE NOW** |
| Google CLIENT_SECRET | marketing/download-photos.js | This session | **ROTATE NOW** |
| Google REFRESH_TOKEN | marketing/download-photos.js | This session | **ROTATE NOW** |
| NVR `Admin@123` | rc-sentry-ai/config.rs default | `a076e474` | **ROTATE NOW** |
| Terminal `rp-terminal-2026` | NEXT_PUBLIC_ env, racecontrol.toml | Known — needs architecture change | **ACCEPTED RISK** |
