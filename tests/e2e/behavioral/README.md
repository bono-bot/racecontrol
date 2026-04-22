# Behavioral E2E Suite

Overnight-runnable E2E tests that exercise real user-visible behaviour
on a canary pod, not code paths or proxies.

## Why this exists

The Steam-popup-loop incident (2026-04-22) shipped because three
independent code paths managed `steam.exe` and nobody noticed until a
customer-facing pod was visibly broken. Static analysis + memory + MMA
all failed to catch it. What catches emergent bugs is behaviour: launch
a game, watch what processes and windows actually appear, assert on
that.

This suite runs against Pod 8 (canary) via server-mediated `/exec` calls
so we don't need pod-local service keys.

## Running

### Single pass

```bash
bash tests/e2e/behavioral/run-all.sh
```

Exits 0 if all tests pass, 1 otherwise. Output is a JSONL log of
{test_id, phase, observation, evidence, result}.

### Continuous (overnight loop)

```bash
bash tests/e2e/behavioral/loop.sh
```

Re-runs the suite every 15 min until terminated or 10:00 IST (hard
stop before venue-open hours). Log: `tests/e2e/behavioral/logs/overnight.jsonl`.

### Single test

```bash
bash tests/e2e/behavioral/tests/game-01-no-auto-steam-spawn.sh
```

## Design

- **Observations, not verdicts.** Each test reports what it saw
  (PIDs, window titles, API bodies). Pass/fail is computed on the raw
  observations, not on proxies like HTTP 200 or `ws=true`.
- **Evidence in the log.** Every assertion records the command run and
  raw output, so morning triage doesn't have to re-run to understand
  what happened.
- **No fakes.** Tests hit real pods, real Steam, real rc-agent. A test
  that can't be run without a real customer (PIN, wheel input) is
  marked `MANUAL-VERIFY` and skipped with a log note instead of faked.
- **Isolated state.** Each test does its own setup (reset session,
  kill test processes) and teardown. No test depends on another's
  leftover state.
- **Canary only.** Default target is Pod 8. `TARGET_POD=pod_N` env var
  overrides. Never defaults to Pods 1–7 (production risk).

## Environment

- `TARGET_POD` — pod_id to test against (default `pod_8`)
- `ADMIN_PIN` — admin PIN for JWT login (default `261121`, overridable)
- `SERVER_URL` — racecontrol API base (default `http://192.168.31.23:8080`)
- `LOG_DIR` — where JSONL logs go (default `tests/e2e/behavioral/logs`)

## NOT an exhaustive test suite

Covers the customer-impact workflows that have broken recently:
- Game launch (no auto-Steam, single-game, supersede)
- Billing (start, tier-snap, end-refund)
- Kiosk auth (staff PIN valid, staff PIN lockout)
- Pod lifecycle (WS reconnect, rc-agent survival)

Does NOT cover every endpoint or every UI flow. Expand per incident.
