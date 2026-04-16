# Phase 362 — Post-Launch Config Verification — PLAN

## Status: RETROACTIVE (shipped before plan artifact created)

## Goal
Verify post-launch game configuration across fleet by adding SessionConfig struct, per-sim shared-memory readers, a 5-stage launch verification pipeline, ConfigMismatchDetected WS events with WhatsApp alerting, and atomic race.ini writes with AI car content validation.

## Steps
1. Add SessionConfig struct to rc-common protocol + implement read_session_config() on all 5 sim adapters (AC, AC Evo, F1 25, iRacing, LMU)
2. Add verify_launch_config() Stage 5 to launch_verifier.rs comparing requested vs actual config with fuzzy matching
3. Add ConfigMismatchDetected WS message with server handler (log, admin broadcast, WhatsApp alert, DB persist)
4. Implement atomic race.ini write (temp-file-then-rename) with readback verification and AI car content validation
5. Add session type + car/track normalization and remove unnecessary 1s sleep from launch path

## Outcome
See SUMMARY.md for results.
