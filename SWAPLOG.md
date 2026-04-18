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
