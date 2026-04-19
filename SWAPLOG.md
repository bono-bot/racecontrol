# Server Binary Swap Log

Chronological record of every `racecontrol.exe` swap on server `.23`, appended
by `deploy-staging/deploy-server.sh` after successful build_id + SHA256 verification.

**Why this file exists:** multiple sessions / operators (James local, Bono VPS,
Uday manual) can each deploy to the same server. Before this file existed, a
session could read `build_id` at start, then later assert "server stays X"
without noticing that a parallel session had swapped it to Y. Tonight
(2026-04-18 → 2026-04-19) this drift caused G9s in two separate sessions.

**Rule (CLAUDE.md):** every session must `tail -20 SWAPLOG.md` at start to
detect between-session swaps before making absolute claims about the server
build_id.

**Format:** `| timestamp_ist | commit_hash | size_bytes | sha256_short | triggered_by | reason |`

- `commit_hash` may carry `-dirty` if the deploy tree had uncommitted files.
- `sha256_short` = first 16 chars of the binary SHA256 (full hash in deploy output).
- `triggered_by` = caller identity (operator email OR script name OR "manual-ssh").
- `reason` = one line on why the swap happened (fix commit, rollback, hotfix, etc.).

## Entries

| timestamp_ist | commit_hash | size_bytes | sha256_short | triggered_by | reason |
|---|---|---|---|---|---|
| 2026-04-18 23:05:20 IST | 2c27e2fc-dirty | 60505088 | (not recorded) | (unconfirmed — parallel session or manual) | fix(kiosk-dead-ends): zombie half-socket + BILL-14 sim_type=None silent abort. Reconstructed from racecontrol.exe LastWriteTime after it surfaced mid-James-session. |
| 2026-04-19 ~01:00 IST | a97c7491-dirty | (not recorded) | (not recorded) | (unconfirmed — parallel session or manual) | Observed mid-session by James during Pattern I deploy verify at 01:30 IST. No announcement in comms-link. Presumed parallel work. |
| 2026-04-19 ~03:25 IST | 84a8b69a | 60668416 | (not recorded) | james-manual-via-deploy-server.sh (bugged — no append) | Prior deploy-server.sh-bug test run; binary rebuilt from origin/main during e0cb97b deploy-server.sh iteration fixes. Should have been appended by the script but wasn't — append path confirmed broken. |
| 2026-04-19 07:29 IST | 66fec05c | 60668416 | e77304738769d4ff | james-via-claude (SENTRY_KEY from /d/racecontrol.toml) | fix(pattern-I-DiD) 66fec05c — Pattern I defense-in-depth: 09acbbe4 rc-agent WS Ping on heartbeat + 90b04d71 server clone-before-await at 12 sites. Addresses Pod 1+6 silent-reconnect-forever (stuck 4h+). Swap succeeded, fleet_connectivity 7/9 → 9/9 after subsequent pod rc-agent wave. **NOTE:** deploy-server.sh did NOT append this entry either — script's SWAPLOG append logic is broken (3 consecutive misses: 2c27e2fc, a97c7491, 84a8b69a, 66fec05c). Architectural fix needed in deploy-server.sh. |
