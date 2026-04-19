# Deploy plan — AI Debugger refactor `148e595d`

Status: STAGED, NOT DEPLOYED · prepared 2026-04-19 20:00 IST by James

## What's in this build

Four commits since the `0306fe17` (safety-net-01) deploy to pods 2-8:

| Commit     | Scope                                                         |
|------------|---------------------------------------------------------------|
| e098fa9b   | fix(alerting): aggregator ISO/space time-window mismatch      |
| de9b4108   | refactor(ai-debugger): GAP-9 dead tier3/tier4 + GAP-10/11 cfg(test) |
| a647d021   | docs(logbook)                                                 |
| 45f31355   | fix(ai-debugger): GAP-2 monotonic Instant cooldown + GAP-8/12 design note |
| 148e595d   | docs(logbook)                                                 |

Runtime-impact commits: `e098fa9b` (server-side, aggregator SQL — separate
deploy track) + `45f31355` (pod-side, cooldown NTP-safety). `de9b4108` is
dead-code removal (zero behaviour change) + test-harness-only cfg(test) guards.

**Pod runtime behaviour delta relative to `0306fe17`:**
- NTP-jump-safe quarantine cooldown (`45f31355`). NTP jumps rare on venue
  Windows; real-world impact small. Structural correctness win.
- Nothing else — all other changes are test-only or dead-code removal.

## Staged artefact

- Path: `C:\Users\bono\racingpoint\deploy-staging\rc-agent-148e595d.exe`
- Size: 26,736,128 bytes
- SHA256: `b187ae431ffc69f7b3818aff3e1e9cc77de7c7dbe985cfb221b30823d8c43c1a`
- Embedded GIT_HASH: `148e595d` (clean, no -dirty)
- Mirror: `deploy-staging/rc-agent.exe` (identical bytes)

## Pre-flight checks (run before touching pods)

```bash
# 1. Fleet baseline — who's where
curl -s --max-time 8 http://192.168.31.23:8080/api/v1/fleet/health \
  | python -c "import json,sys;[print(f\"pod_{p.get('pod_number')}: {p.get('build_id')} ws={p.get('ws_connected')}\") for p in json.load(sys.stdin)]"

# 2. MAINTENANCE_MODE sweep — any pod blocked?
for N in 1 2 3 4 5 6 7 8; do
  echo "---pod $N---"
  ssh -o ConnectTimeout=3 pod$N "if exist C:\\RacingPoint\\MAINTENANCE_MODE echo BLOCKED" 2>/dev/null
done

# 3. Service-key parity (LOGBOOK calls out this has been a 401-blocker)
bash scripts/deploy-preflight.sh 148e595d
```

## Deploy sequence (CANARY first)

**PER USER DIRECTIVE, POD 1 IS HELD ON 66fec05c. DO NOT INCLUDE IN WAVE.**

### Wave 0 — Pod 8 canary (MANDATORY first)

```bash
# Start staging HTTP server
bash scripts/start-staging-server.sh 18889

# Pod 8 download + atomic swap via rc-sentry
curl -s -X POST http://192.168.31.91:8091/exec \
  -H "Content-Type: application/json" \
  -d @- <<'JSON'
{
  "cmd": "curl.exe -s -o C:\\RacingPoint\\rc-agent-148e595d.exe http://192.168.31.27:18889/rc-agent-148e595d.exe"
}
JSON

# Verify download size before swap (prevents a 0306fe17-style 335-byte-HTML-404 deploy)
curl -s -X POST http://192.168.31.91:8091/exec \
  -d '{"cmd":"dir C:\\RacingPoint\\rc-agent-148e595d.exe"}'

# Atomic swap — SINGLE `&` chain only (rc-sentry BLOCKED_PATTERNS rejects `&&`)
curl -s -X POST http://192.168.31.91:8091/exec \
  -d '{"cmd":"taskkill /F /IM rc-agent.exe & del /Q C:\\RacingPoint\\rc-agent-prev.exe & ren C:\\RacingPoint\\rc-agent.exe rc-agent-prev.exe & ren C:\\RacingPoint\\rc-agent-148e595d.exe rc-agent.exe"}'

# Verify
sleep 15
curl -s --max-time 5 http://192.168.31.91:8090/health
```

**Canary gate:** 30-min soak. Pod 8 must show `ws_connected=true`,
`build_id=148e595d`, `crashes_1h=0`, no MAINTENANCE_MODE, no
`channel closed` broadcast errors in server console.

### Wave 1 — Pods 2, 3, 4, 5, 7

Same sequence per pod. Stagger 60s between pods so RCWatchdog on each has
time to settle.

### Wave 2 — Pod 6 (last; has historic AC SHM issues)

Same sequence. Extra scrutiny on first launch post-swap.

### Pod 1 — HOLD

Pod 1 stays on `66fec05c` per earlier user directive. Do not touch.

## Server + Cloud

- **Server .23 (racecontrol binary):** this deploy is pod-only.
  `ai_debugger.rs` and `tier_engine.rs` both live in `crates/rc-agent/`.
  Server racecontrol binary does not contain AI Debugger code and does not
  need a rebuild for `45f31355` / `de9b4108`.
- **BUT** `e098fa9b` (aggregator SQL fix) IS server-side. Per LOGBOOK 19:16
  IST entry, server .23 already shows `build_id=e098fa9b` — it was deployed
  earlier. No action needed.
- **Cloud VPS:** cloud racecontrol on `b978747b`, 2 commits behind HEAD but
  those 2 commits are LOGBOOK-only. No code change. Cloud parity optional.
- **Cloud rc-agent:** cloud is a server, not a pod. No rc-agent runs there.

## Rollback (if Wave 0 fails)

```bash
curl -s -X POST http://192.168.31.91:8091/exec \
  -d '{"cmd":"taskkill /F /IM rc-agent.exe & ren C:\\RacingPoint\\rc-agent.exe rc-agent-failed.exe & ren C:\\RacingPoint\\rc-agent-prev.exe rc-agent.exe"}'
sleep 15
curl -s --max-time 5 http://192.168.31.91:8090/health
```

RCWatchdog will pick up `rc-agent.exe` (now = the previous known-good
`0306fe17` build) within ~5s polling window.

## What this deploy does NOT need

- schema migration (none)
- frontend rebuild (none)
- cloud parity (pod-only binary)
- POS deploy (POS `.130` is not in the pod fleet rollover path)
- config push (no TOML change)

## Known hazards (from recent LOGBOOK entries)

- `deploy-pod.sh` SHA256 compare is broken per 14:35 IST entry (reads JSON
  prefix, not hash). Use manual direct-curl + atomic-swap as above, NOT the
  script.
- rc-sentry `BLOCKED_PATTERNS` rejects `&&` in exec payloads — use single
  `&` only.
- Session 0 vs Console session — RCWatchdog restores rc-agent to Console
  session 1 automatically after taskkill; do NOT use `schtasks /Run /TN
  StartRCAgent` which runs as SYSTEM in Session 0.
- MAINTENANCE_MODE sentinel is a silent killer — sweep before swap.

## Open gates

- **GAP-8 (parallel debug systems coordination):** design note complete at
  `.planning/scratch/AI-DEBUGGER-GAP-8-GAP-12-DESIGN-NOTE.md` — awaits
  Uday α/β/γ decision. Not blocking this deploy.
- **GAP-12 (pattern key coarseness):** deferred per handoff recommendation.
  Not blocking this deploy.
- **User go-ahead for fleet rollout:** awaiting explicit approval.

## Do-not-autonomously list

- No pod swap without user go-ahead (weekend deploy script fragility +
  Pod 1 hold directive both argue for explicit sign-off)
- No server redeploy (no server-side changes in this build)
- No cloud deploy (LOGBOOK-only commits since cloud's current build)
- Do not message Uday directly (comms-link is James↔Bono; Uday via
  WhatsApp/phone and he's optimising for daughter-time — no routine asks)
