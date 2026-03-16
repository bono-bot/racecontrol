---
phase: 27-tailscale-mesh-internet-fallback
verified: 2026-03-16T12:30:00Z
status: gaps_found
score: 5/7 must-haves verified
re_verification: false
gaps:
  - truth: "cloud_sync routes through Tailscale IP (Bono's Tailscale mesh path active)"
    status: failed
    reason: "racecontrol.toml [cloud].api_url still points to https://app.racingpoint.cloud/api/v1 — Bono's Tailscale IP has not been substituted. The TODO comment is present but the switch has not happened. This is by design (IPs unknown), but the goal states cloud_sync routes through Tailscale."
    artifacts:
      - path: "racecontrol.toml"
        issue: "api_url = \"https://app.racingpoint.cloud/api/v1\" — public URL retained with TODO comment, Tailscale IP placeholder not filled"
    missing:
      - "Bono's Tailscale IP (100.x.x.x) must be known and substituted into [cloud].api_url"
      - "Alternatively: TS-05/TS-06 can be satisfied by confirming this is deferred pending live deployment — but the goal explicitly states cloud_sync routes through Tailscale"

  - truth: "All 8 pods, server, and Bono's VPS have joined the Tailscale mesh (Tailscale installed and enrolled)"
    status: failed
    reason: "Plan 27-04 created the deploy script (scripts/deploy-tailscale.ps1) but 27-05-SUMMARY.md does not exist — Plan 05 Task 3 (deploy to server) and Task 4 (human checkpoint) were never completed. No evidence that Tailscale was actually installed on any pod or server. The script is a prerequisite, not the enrollment itself."
    artifacts:
      - path: "scripts/deploy-tailscale.ps1"
        issue: "Script exists and is correct, but has never been executed — PREAUTH_KEY_REPLACE_ME guard still in place, no Tailscale IPs known"
    missing:
      - "Execute scripts/deploy-tailscale.ps1 with real pre-auth key (Pod 8 canary first)"
      - "Confirm 100.x.x.x Tailscale IPs for server and all 8 pods"
      - "Confirm Bono's VPS has joined the tailnet"

human_verification:
  - test: "Relay endpoint returns 401 without X-Relay-Secret"
    expected: "curl -X POST http://<server-tailscale-ip>:8099/relay/command (no header) returns HTTP 401 with body 'invalid relay secret'"
    why_human: "Requires live Tailscale network — server must be enrolled in tailnet and relay listener must be bound to Tailscale IP. Cannot test without live tailnet."

  - test: "Relay endpoint returns 200 with correct X-Relay-Secret"
    expected: "curl -X POST http://<server-tailscale-ip>:8099/relay/command -H 'X-Relay-Secret: 85c650850520f0cabf8f99b0c3cecc3e' -d '{\"type\":\"get_status\",\"data\":{\"pod_number\":8}}' returns HTTP 200 with JSON"
    why_human: "Requires live Tailscale network and enrolled server."

  - test: "Pod 8 shows Tailscale IP via tailscale.exe ip -4"
    expected: "Invoke-Command on 192.168.31.91 returns a 100.x.x.x address"
    why_human: "Requires scripts/deploy-tailscale.ps1 to have been run with a real pre-auth key."

  - test: "All 8 pods and server show 100.x.x.x Tailscale IPs"
    expected: "9 devices (8 pods + server) listed in Tailscale admin console with 100.x.x.x addresses"
    why_human: "Fleet-level deployment verification — requires running the deploy script."

  - test: "Existing LAN pods still connect after Phase 27 changes"
    expected: "All 8 pods show Online in kiosk dashboard at http://192.168.31.23:3300 — no regression from second Axum listener"
    why_human: "Requires live server and pods — regression check for main LAN listener on :8080."

  - test: "cloud_sync still works when api_url is switched to Tailscale IP"
    expected: "Server logs show 'Cloud sync enabled: http://100.x.x.x/api/v1' and sync succeeds"
    why_human: "Requires Tailscale enrollment complete and api_url updated in racecontrol.toml."
---

# Phase 27: Tailscale Mesh + Internet Fallback — Verification Report

**Phase Goal:** All 8 pods, server, and Bono's VPS join a Tailscale mesh network — installed as a Windows Service via WinRM, cloud_sync routes through Tailscale IP, and the server pushes telemetry/game state/pod health events to Bono in real time with a bidirectional command relay for PWA-triggered game launches

**Verified:** 2026-03-16T12:30:00Z
**Status:** gaps_found — 5/7 truths verified, 2 gaps blocking goal (live deployment + cloud_sync routing)
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | BonoConfig exists in config.rs with relay_port=8099 default and all Option fields | VERIFIED | `crates/racecontrol/src/config.rs` line 264 — BonoConfig struct with manual Default impl returning relay_port=8099. Tests bono_config_defaults and bono_config_explicit pass. |
| 2 | bono_relay.rs has BonoEvent + RelayCommand enums, spawn(), handle_command(), build_relay_router() | VERIFIED | `crates/racecontrol/src/bono_relay.rs` — full 262-line implementation. spawn() subscribes to broadcast channel, push_event() POSTs with 5s timeout, handle_command() validates X-Relay-Secret, build_relay_router() returns Router. |
| 3 | AppState has bono_event_tx broadcast channel for inter-module event emission | VERIFIED | `crates/racecontrol/src/state.rs` line 87 — `pub bono_event_tx: broadcast::Sender<crate::bono_relay::BonoEvent>` with capacity 256, initialized in AppState::new(). |
| 4 | bono_relay::spawn() is called in main.rs and second Axum listener is wired | VERIFIED | `crates/racecontrol/src/main.rs` line 296 — `bono_relay::spawn(state.clone())` after cloud_sync::spawn(). Lines 321-343 — conditional second listener on Tailscale IP. Crucially, second listener block appears BEFORE `.with_state(state)` at line 383. |
| 5 | pod_monitor.rs emits BonoEvent::PodOnline and PodOffline at state-transition boundaries | VERIFIED | `crates/racecontrol/src/pod_monitor.rs` lines 147-151 (PodOnline at offline->online recovery) and lines 212-216 (PodOffline at heartbeat-stale transition). Both use `let _ = state.bono_event_tx.send(...)` — non-fatal. |
| 6 | scripts/deploy-tailscale.ps1 exists with canary-first WinRM deploy logic | VERIFIED | `scripts/deploy-tailscale.ps1` — 220 lines, CRLF endings. Contains 4 Invoke-Command calls. Guards: PREAUTH_KEY_REPLACE_ME exits with error at line 58. ADMIN_PASSWORD_REPLACE_ME exits at line 63. Pod 8 canary first, 'yes' gate before fleet rollout. 4-step: download MSI, msiexec /quiet TS_UNATTENDEDMODE=always, Start-Sleep 5, tailscale up --unattended --auth-key --hostname --reset. Verifies `tailscale.exe ip -4` matches `^100\.`. |
| 7 | cloud_sync routes through Tailscale IP (Bono's mesh path active) | FAILED | `racecontrol.toml` line 17 — `api_url = "https://app.racingpoint.cloud/api/v1"`. Lines 15-16 show the intended Tailscale URL commented out as a TODO. The switch to Bono's Tailscale IP has not happened — IP is unknown. |

**Score: 5/7 truths verified**

The two gaps share a root cause: the live Tailscale deployment (Plan 27-05 Tasks 2-4) was never executed. 27-05-SUMMARY.md does not exist — only a commit (8fbef99) shows the [bono] section was added to racecontrol.toml, but the section has `enabled = false` and placeholder IPs.

---

## Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/bono_relay.rs` | BonoEvent enum + spawn() + handle_command() + build_relay_router() | VERIFIED | Full 262-line implementation. Exports: BonoEvent, RelayCommand, spawn, handle_command, build_relay_router, relay_health. |
| `crates/racecontrol/src/config.rs` | BonoConfig struct with relay_port=8099 default | VERIFIED | Lines 264-289. Manual Default impl. Field on Config struct at line 25 with `#[serde(default)]`. |
| `crates/racecontrol/src/state.rs` | bono_event_tx broadcast::Sender<BonoEvent> field in AppState | VERIFIED | Line 87. Initialized at line 145, used in AppState::new() at line 155. |
| `crates/racecontrol/src/main.rs` | bono_relay::spawn() call + optional second Axum listener | VERIFIED | Lines 296 (spawn), 319-343 (second listener). Ordering confirmed: second listener block (line 321) before `.with_state(state)` (line 383). |
| `crates/racecontrol/src/pod_monitor.rs` | BonoEvent::PodOnline / PodOffline emissions into bono_event_tx | VERIFIED | Lines 27 (import), 147-151 (PodOnline), 212-216 (PodOffline). |
| `scripts/deploy-tailscale.ps1` | WinRM fleet deploy script — canary Pod 8, fleet rollout, PREAUTH guard | VERIFIED | Exists, 10,410 bytes, CRLF. All required elements confirmed. |
| `racecontrol.toml` | [bono] section with enabled=true, real Tailscale IPs, relay_secret | PARTIAL | [bono] section exists (lines 72-80). relay_secret is a real hex string (85c650850520f0cabf8f99b0c3cecc3e). BUT: enabled=false, webhook_url and tailscale_bind_ip contain placeholder text "100.BONO.IP.HERE" and "100.SERVER.IP.HERE". |

---

## Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `crates/racecontrol/src/config.rs` | `crates/racecontrol/src/bono_relay.rs` | BonoConfig used by spawn() | WIRED | bono_relay.rs line 57: `let bono = &state.config.bono;`. BonoConfig accessed correctly. |
| `crates/racecontrol/src/bono_relay.rs` | `state::AppState.bono_event_tx` | spawn() subscribes via bono_event_tx.subscribe() | WIRED | Line 75: `let mut rx = state.bono_event_tx.subscribe();` — broadcast loop verified. |
| `crates/racecontrol/src/bono_relay.rs` | `state::AppState.http_client` | push_event() uses state.http_client.post() | WIRED | Lines 103-110: `state.http_client.post(webhook_url).json(event).timeout(Duration::from_secs(5)).send().await` |
| `crates/racecontrol/src/main.rs` | `crates/racecontrol/src/bono_relay.rs` | bono_relay::spawn(state.clone()) and bono_relay::build_relay_router() | WIRED | Line 296: spawn. Lines 325, 330: build_relay_router, axum::serve. |
| Second TcpListener (Tailscale IP:8099) | bono_relay::build_relay_router() | axum::serve(ts_listener, relay_router) | WIRED (code only) | Line 325-330: relay_router built from build_relay_router(state.clone()), served on ts_listener. Code is correct but listener never binds at runtime because tailscale_bind_ip is a placeholder. |
| `crates/racecontrol/src/pod_monitor.rs` | `state.bono_event_tx` | bono_event_tx.send(BonoEvent::PodOnline/PodOffline) | WIRED | Lines 147 and 212: send() calls at both transition sites. |
| `racecontrol.toml [cloud].api_url` | `cloud_sync::spawn()` | state.config.cloud.api_url | PARTIAL | cloud_sync.rs reads api_url from config (no hardcoded IPs — TS-06 satisfied). However, api_url still points to public internet, not Tailscale mesh. |

---

## Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| TS-01 | 27-01 | BonoConfig deserializes from TOML with correct defaults | SATISFIED | bono_config_defaults test passes (confirmed by cargo test). relay_port=8099, all Options None. |
| TS-02 | 27-01, 27-02, 27-03 | bono_relay::spawn() no-ops when enabled=false | SATISFIED | spawn_disabled test passes. spawn() has guard at line 59-62. |
| TS-03 | 27-01, 27-02, 27-03 | bono_relay::spawn() no-ops when webhook_url=None | SATISFIED | spawn_no_url test passes. spawn() guard at lines 64-70. |
| TS-04 | 27-01, 27-02 | BonoEvent serializes to expected JSON shape | SATISFIED | event_serialization test passes. `{type: "session_start", data: {...}}` shape confirmed. |
| TS-05 | 27-05 | Relay endpoint returns 401 with wrong secret | NEEDS HUMAN | handle_command() code verified: lines 151-153 return StatusCode::UNAUTHORIZED when secret missing or wrong. Live network required to confirm endpoint is reachable. |
| TS-06 | 27-03, 27-05 | cloud_sync.rs uses api_url from config (no hardcoded IP) | SATISFIED (partial) | cloud_sync.rs reads from state.config.cloud.api_url — no hardcoded IPs. However, api_url still points to public internet, not Tailscale mesh. The code path is correct; the config value is not yet switched. |
| TS-DEPLOY | 27-04, 27-05 | All 8 pods show Tailscale IP tailscale ip -4 | BLOCKED | scripts/deploy-tailscale.ps1 exists and is correct. But Plan 27-05 Task 3 and Task 4 (deploy + human verify) were never executed — 27-05-SUMMARY.md is absent. No pods have been enrolled. |

**Orphaned requirements:** None. All 7 TS- requirement IDs appear in at least one plan's `requirements` field. None are mapped to Phase 27 in REQUIREMENTS.md traceability table (TS- requirements are defined in 27-RESEARCH.md, not the main REQUIREMENTS.md) — this is a documentation gap but not a blocking issue.

---

## Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `crates/racecontrol/src/bono_relay.rs` | 168-174 | `// TODO Phase 28: wire to game_launcher::launch_game_on_pod()` — LaunchGame returns "queued" with note | INFO | Expected and documented. Phase 27 establishes the relay channel; actual game launch dispatch is Phase 28 scope. Not a blocker. |
| `racecontrol.toml` | 75, 77 | `webhook_url = "http://100.BONO.IP.HERE/..."` and `tailscale_bind_ip = "100.SERVER.IP.HERE"` — placeholder values | WARNING | Second Axum listener never binds at runtime (tailscale_bind_ip is not a valid IP). Bono relay event push also never fires (enabled=false + no valid webhook_url). These are deployment-time placeholders, not code bugs, but they mean the Phase 27 goals are not operational. |
| `racecontrol.toml` | 73 | `enabled = false` in [bono] section | WARNING | Relay is disabled until Tailscale IPs are confirmed. Correct behavior, but means relay is not yet live. |

---

## Human Verification Required

### 1. Tailscale Fleet Enrollment

**Test:** Run `scripts/deploy-tailscale.ps1` with a real reusable pre-auth key from Tailscale Admin Console. Deploy Pod 8 canary first, verify 100.x.x.x IP, then confirm rollout to all 8 pods and server.

**Expected:** 9 devices (pod-1 through pod-8 + racing-point-server) appear in Tailscale admin console at https://login.tailscale.com/admin/machines, each showing a 100.x.x.x IP.

**Why human:** Requires real pre-auth key, live WinRM connections to all pods, and network access to Tailscale coordination servers.

### 2. Relay Endpoint Auth Check

**Test:** After updating racecontrol.toml with real Tailscale IPs and restarting racecontrol, from any machine on the tailnet:
- `curl -X POST http://<server-tailscale-ip>:8099/relay/command -H "Content-Type: application/json" -d '{"type":"get_status","data":{"pod_number":8}}'`
- Expected: HTTP 401 with body "invalid relay secret"
- Then add `-H "X-Relay-Secret: 85c650850520f0cabf8f99b0c3cecc3e"` — expected: HTTP 200 with JSON pod status.

**Why human:** Requires live Tailscale network, server enrolled, and racecontrol running with real tailscale_bind_ip.

### 3. Relay Health Endpoint

**Test:** `curl http://<server-tailscale-ip>:8099/relay/health`

**Expected:** `{"status":"ok","service":"bono-relay"}`

**Why human:** Same as above — requires live tailnet.

### 4. cloud_sync Tailscale Switch

**Test:** After updating `[cloud].api_url` in racecontrol.toml to Bono's 100.x.x.x Tailscale IP and restarting, check server logs for: `"Cloud sync enabled: http://100.x.x.x/api/v1"` and confirm sync succeeds.

**Expected:** Sync works over Tailscale mesh. Billing/driver data still syncing correctly.

**Why human:** Requires Bono's VPS on tailnet, URL switch in config, and log access on server.

### 5. No Regression on LAN Pod Connections

**Test:** Check kiosk at http://192.168.31.23:3300 (or :8080) shows all 8 pods online after deploying Phase 27 binary.

**Expected:** All pods connect via WebSocket as before — second Axum listener does not disrupt main :8080 listener.

**Why human:** Requires live server with new binary deployed.

---

## Gaps Summary

Two gaps block full goal achievement, both stemming from the same root cause: **Plan 27-05 (the live deployment and human-verify plan) was partially executed — the config file was updated but Tasks 3 and 4 were never completed.**

**Gap 1 — Tailscale not deployed to fleet (TS-DEPLOY blocked)**

The deploy script (`scripts/deploy-tailscale.ps1`) is complete and ready. However, it has never been run. No pod or server has Tailscale installed. The PREAUTH_KEY placeholder is still in place and must be replaced with a real key before the script can execute. 27-05-SUMMARY.md does not exist, confirming Plan 05 was not finished.

**Gap 2 — cloud_sync still uses public internet URL (TS-05/TS-06 partial)**

`racecontrol.toml [cloud].api_url` still points to `https://app.racingpoint.cloud/api/v1`. The TODO comment and placeholder Tailscale URL are in place, but Bono's Tailscale IP is unknown because the tailnet has not yet been bootstrapped. This is a sequencing dependency: deploy Tailscale first (Gap 1), then get Bono's IP, then update this config value.

**What works today (code-complete):**
- BonoConfig deserialization with relay_port=8099 (TS-01: green)
- spawn() guard logic for disabled/no-url cases (TS-02, TS-03: green)
- BonoEvent/RelayCommand JSON serialization (TS-04: green)
- Full broadcast loop + push_event() implementation wired
- handle_command() with X-Relay-Secret auth wired (TS-05: code verified, live test pending)
- cloud_sync reads api_url from config — no hardcoded IPs (TS-06: code verified)
- PodOnline/PodOffline events emitted at correct transition sites
- Second Axum listener code is correct — activates when tailscale_bind_ip is set to a real IP
- Deploy script ready and correct

**What needs to happen:**
1. Get a Tailscale pre-auth key from admin console
2. Run `scripts/deploy-tailscale.ps1` (Pod 8 canary first)
3. Confirm all 9 devices enrolled with 100.x.x.x IPs
4. Update racecontrol.toml: fill in real `tailscale_bind_ip` (server's IP), real `webhook_url` (Bono's IP), set `enabled = true`
5. Update `[cloud].api_url` to Bono's Tailscale IP
6. Build and deploy new racecontrol binary to server
7. Human verify: relay endpoint auth, cloud sync over Tailscale, LAN pod regression check

---

_Verified: 2026-03-16T12:30:00Z_
_Verifier: Claude (gsd-verifier)_
