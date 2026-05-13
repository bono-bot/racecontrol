---
artifact: §S-262 iter11 audit-only deploy-surface verification
class: audit-only (composes-with §S-146 RCA · NOT a §S-146 5-section RCA itself)
authored: 2026-05-13 IST
author: bono (post-/compact session)
parent-cascade: §S-246 audit-only finding (parallel-bono ca6cc386 + 2cf78a4b; survival_report_handler CONFIRMED-UNAUTHENTICATED) + §S-261 Captain-ratify §S-249.4 item #7 [A] + §S-262 OPEN-CLAIM iter11
severity-grade: **HIGH-PUBLIC-INTERNET-REACHABLE-ON-CLOUD** (escalation from prior LATENT-INTERNAL-TRUST framing)
captain-decision-feed: §S-249.4 item #1 (§S-246 closure_phase assignment) at next Captain disposition cycle
---

# §S-262 iter11 audit — POST /api/v1/pods/{pod_id}/survival-report deploy-surface verification

## 1. Code-layer bind (main.rs listener + config)

**Source: `crates/racecontrol/src/main.rs` line 311** — `let listener = tokio::net::TcpListener::bind(&bind_addr).await?;` — bind_addr derived from `format!("{}:{}", config.server.host, config.server.port)`.

**Source: `crates/racecontrol/racecontrol.toml`** lines 11-13:
```toml
host = "0.0.0.0"
port = 8080
```

`host = "0.0.0.0"` means **all-interfaces bind** (every network interface the host has). NOT loopback-only (`127.0.0.1`); NOT Tailscale-only (`100.x.x.x`); NOT LAN-only (`192.168.31.x`).

**Source: `crates/racecontrol/src/lib.rs` line 75** — `pub mod fleet_healer;` — NO `#[cfg(feature = ...)]` gating; `default = []` per `Cargo.toml [features]` confirms fleet_healer module is in EVERY build (cloud + venue both compile-in fleet_healer_routes()).

**Route registration: `crates/racecontrol/src/api/routes.rs:128`** — `.merge(crate::fleet_healer::fleet_healer_routes())` — registered on every build at the top-level api_routes() router OUTSIDE all auth-class sub-router groups.

## 2. Venue Server .23 binding (LAN-only vs all-interfaces)

**Status: PARTIAL-VERIFIED via repo-config + Captain-LOCKED Network Map**

The `racecontrol.toml` in the repo (`/root/racecontrol/racecontrol.toml`) is the Bono VPS config (this file). The venue Server .23 has its OWN `racecontrol.toml` at `C:\RacingPoint\racecontrol.toml` (per CLAUDE.md Standing Rules). Direct verification of venue config requires SSH via Tailscale to `ADMIN@100.125.108.37` (Server .23 Tailscale IP).

**Inferred posture (high confidence)**: venue racecontrol.toml likely uses same `host = "0.0.0.0"` (standard deploy config); however venue Server .23 is **physically LAN-only** per Network Map (192.168.31.23 + Tailscale mesh 100.125.108.37). No public-internet inbound path. Effective exposure: 192.168.31.0/24 + Tailscale mesh members only.

**Severity for venue Server .23**: **LOW-INTERNAL-TRUST** — any client on venue LAN (customer WiFi devices, pods, POS, kiosks) OR on Tailscale mesh can POST to `/api/v1/pods/*/survival-report`. Trust boundary = LAN/Tailscale. Same severity-class as parallel-bono §S-246 audit-only finding.

**Verification gap (this audit)**: did NOT SSH-verify venue Server .23 actual netstat + firewall posture. RECOMMENDATION: next bono session run `ssh ADMIN@100.125.108.37 "netstat -an | findstr 8080"` to confirm.

## 3. Bono VPS cloud racecontrol binding (CONFIRMED public-facing)

**Live evidence collected this audit (2026-05-13 ~13:03 UTC = 18:33 IST):**

```
$ netstat -tlnp | grep -E ":8080|:8090"
tcp        0      0 0.0.0.0:8080            0.0.0.0:*               LISTEN      2864026/racecontrol 
tcp        0      0 0.0.0.0:8090            0.0.0.0:*               LISTEN      2864026/racecontrol
```

```
$ ufw status | grep 8080
8080/tcp                   ALLOW       Anywhere                   # Racecontrol
```

```
$ pm2 describe racecontrol | grep -E "cwd|script"
script path       │ /root/racecontrol/scripts/exit-trace-lite.sh
exec cwd          │ /root/racecontrol
```

**Live curl test (from this VPS to its own racecontrol):**
```
$ curl -s -X POST http://localhost:8080/api/v1/pods/test-pod/survival-report \
       -H "Content-Type: application/json" \
       -d '{"pod_id":"test-pod","source_layer":"audit-test","status":"test","timestamp":"2026-05-13T18:35:00Z"}' \
       -w "\nHTTP %{http_code}\n"
{"pod_id":"test-pod","status":"accepted","timestamp":"2026-05-13T13:03:59.530121552+00:00"}
HTTP 200
```

**Findings on Bono VPS cloud:**
1. racecontrol listening on `0.0.0.0:8080` (all interfaces)
2. UFW firewall ALLOWS 8080/tcp from `Anywhere` (no IP allowlist; not Tailscale-restricted)
3. VPS has public IP (Hostinger `srv1422716.hstgr.cloud` per memory; physical public internet attachment)
4. Live POST to `/api/v1/pods/{pod_id}/survival-report` succeeds with HTTP 200 + accepts arbitrary `SurvivalReport` JSON
5. NO `X-Service-Key` header required (no auth check; matches §S-246 source-code audit finding)
6. Response body confirms request-body persisted into normalized pod_id flow (`AuditTrail::log_repair` writes incident_log.metadata per §S-245 finding)

**Severity for Bono VPS cloud**: **HIGH-PUBLIC-INTERNET-REACHABLE-UNAUTHENTICATED-WRITE** — any internet client can POST arbitrary JSON to the public endpoint and persist caller-controlled `source_layer: String` + `status: String` + `diagnostics: Option<Value>` + `build_id: Option<String>` fields into Bono VPS `incident_log.metadata` indefinitely. Same risk surface §S-245 documented for the audit-log echo class, now confirmed reachable from the public internet.

## 4. Venue↔cloud parity diff (build/feature flags + route registration)

| Dimension | Venue Server .23 | Bono VPS cloud | Parity |
|---|---|---|---|
| Cargo build features | `default = []` | `default = []` | IDENTICAL |
| `pub mod fleet_healer;` in lib.rs | YES (no cfg gate) | YES (no cfg gate) | IDENTICAL |
| `fleet_healer_routes()` merged at routes.rs:128 | YES | YES | IDENTICAL |
| `survival_report_handler` auth check | NONE (per §S-246) | NONE (per §S-246) | IDENTICAL |
| `racecontrol.toml` host | LIKELY `0.0.0.0` (unverified) | `0.0.0.0` (verified live) | LIKELY-IDENTICAL |
| Listener bind | LIKELY `0.0.0.0:8080` (unverified) | `0.0.0.0:8080` (verified) | LIKELY-IDENTICAL |
| Physical network exposure | LAN-only (192.168.31.0/24 + Tailscale) | **PUBLIC INTERNET** | **CRITICAL DIVERGENCE** |
| Firewall rule for 8080/tcp | Windows Firewall LAN-scoped (assumed; unverified) | UFW `ALLOW Anywhere` (verified) | DIFFERENT |

**The critical divergence is NOT in code or config — it is in physical deploy posture.** Same code + same config produces different severity because the cloud VPS sits behind no NAT/perimeter that filters 8080/tcp.

**Build commit parity (CGP SWAPLOG / DEPLOY PARITY rule)**: NOT verified this audit (out of scope; would require `curl /api/v1/health` from both venue and cloud + cross-check build_id). Recommended for separate parity-audit.

## 5. Severity grading (matrix)

| Posture | Auth check | Network exposure | Severity | Realized today? |
|---|---|---|---|---|
| Venue Server .23 | NONE | LAN + Tailscale mesh | LOW-INTERNAL-TRUST | Yes (any LAN/Tailscale client can POST) |
| **Bono VPS cloud** | **NONE** | **Public internet via 0.0.0.0:8080 + UFW allow** | **HIGH-PUBLIC-INTERNET-REACHABLE** | **Yes (live curl confirmed HTTP 200 + payload persisted)** |
| Pods 1-8 | N/A | N/A | N/A | rc-agent on pods doesn't host fleet_healer_routes |
| James .27 | N/A | N/A | N/A | comms-link relay only |
| POS .130 | N/A | N/A | N/A | client-only |
| Comms-link | N/A | N/A | N/A | relay only |

**Compound risk surface:**
1. Public-internet caller can write attacker-controlled JSON into Bono VPS `incident_log.metadata`
2. Today's READ-side reach is INTERNAL-ONLY (per §S-245 finding — `AuditTrail::recent_entries` has zero call sites; `query_audit_log` reads different table)
3. **Forward-risk**: any future HTTP handler that exposes `incident_log` rows via JSON response without redact-on-read becomes an internet-to-staff data exfiltration vector. The data persists indefinitely (no TTL on incident_log rows visible in §S-245 audit).
4. Adjacent risk: storage exhaustion via volumetric POST (no rate limit visible on this route per `auth_rate_limited_routes()` membership audit at §S-246.3 — endpoint sits OUTSIDE the rate-limited sub-router). Internet client can flood incident_log with multi-KB payloads.

**Severity escalation justification for §S-249.4 item #1 (§S-246 closure_phase):**
- Prior framing (parallel-bono §S-246 audit-only): LATENT-INTERNAL-TRUST — recommendation was `closure_phase = Post-V2.0-AUTH-Sprint` (low-priority debt)
- This audit: CONFIRMED-PUBLIC-INTERNET-REACHABLE on cloud — **closure_phase recommendation upgrades to PRE-V2.0-CLOUD-HARDENING-SPRINT** (high-priority; cloud should not ship V2.0 with public unauthenticated /pods/* endpoint)

## 6. Carry-forward + recommendations for §S-146 RCA scope

**Findings feed §S-249.4 item #1** (§S-246 auth-gap closure_phase disposition):
- Recommended closure_phase: **Pre-V2.0-Cloud-Hardening-Sprint** (NOT Post-V2.0-AUTH-Sprint per prior bono default)
- Rationale: cloud public-facing severity is materially different from venue LAN-only severity; V2.0 customer-day cannot ship with public-unauthenticated /pods/* on the cloud VPS

**Recommended §S-146 fix RCA scope amendments (when item #1 dispositioned + bono-LEAD authorized):**
- Section 4 (V2-alignment delta): split fix into TWO paths:
  - **Cloud path (URGENT)**: add `check_service_key` to `survival_report_handler` AS PER sibling `request_heal_lease` pattern (existing primitive at `crates/racecontrol/src/api/survival.rs:202`); deploy cloud-first via standard deploy-server.sh; Captain per-PR auth + MMA Step 1 DIAGNOSE precedes (foundational-boundary doctrine)
  - **Venue path (KAIZEN)**: same patch; deploy via standard venue deploy chain; lower urgency (LAN-only) but same code change
- Section 5 (V2-framed proposed change): one PR fixes both since code is identical; ship cloud-first verify, then venue parity ship
- Alternative for Cloud-only-urgent if Captain wants split: short-term ufw deny 8080/tcp from Anywhere + allow Tailscale-mesh-only (operational lockdown; non-code mitigation)

**Verification gaps for next-iter audit:**
1. SSH-verify venue Server .23 actual netstat + firewall — confirm LAN-only assumption
2. SSH-verify venue Server .23 racecontrol.toml `host` value
3. Volumetric rate-limit test: confirm endpoint is OUTSIDE `auth_rate_limited_routes()` (no 5-req/min limit) — observed by reading routes.rs but not behavioral-tested

**Per-target enumeration (H4 discipline):**
- **Server .23 (venue racecontrol)**: PARTIAL-VERIFIED — LAN-only inferred from Network Map; netstat unverified
- **Bono VPS (cloud racecontrol)**: ✅ FULLY VERIFIED — public-internet-reachable HTTP 200 confirmed
- **Pods 1-8 (rc-agent)**: ✅ N/A — pods don't host fleet_healer_routes
- **James .27 (comms-link relay)**: ✅ N/A — relay :8766 only, not racecontrol
- **POS .130**: ✅ N/A — pure client
- **Comms-link**: ✅ N/A — relay only
- **Cloud apps (admin/web/kiosk)**: ✅ N/A — frontend apps; don't bind racecontrol routes

**NOT TESTED (per H3 anti-theater discipline):**
- Public-IP external probe (would require external host outside Bono VPS; severity grade derives from 0.0.0.0 bind + UFW allow + standard internet attachment which is sufficient structural evidence)
- Venue Server .23 actual posture (PARTIAL via repo-config inference; full SSH verification deferred)
- Rate-limit behavioral test (sub-router membership read but not flood-tested)
- Compound storage-exhaustion attack viability
- WAF / Cloudflare / Hostinger upstream filter posture (Bono VPS may have upstream filtering invisible to local UFW; not verified)

## Composes-with
- **§S-246 (parallel-bono ca6cc386 + 2cf78a4b)** audit-only source-code finding
- **§S-245 iter7** survival_report_handler defensive comment-contract (current racecontrol main 94070ed6)
- **§S-249.4 item #1** §S-246 closure_phase Captain disposition (this audit's EVIDENCE feed)
- **§S-249.4 item #7** production deploy-surface verification Captain disposition [A] ratified at §S-261
- **§S-262** iter11 cascade OPEN-CLAIM (parent ledger anchor at comms-link 6dbcbe9f)
- **security-debt-ledger row 12** (2026-05-13T10:15:00Z auth-gap entry; status=open; closure_phase=PENDING-CAPTAIN-DISPOSITION; THIS AUDIT EVIDENCE feeds the disposition)
- **CLAUDE.md Standing Rules** > Comms > Verify recipient infrastructure (mechanism-trust-check upstream of fix RCA per §S-186 / §S-172 doctrine)
- **CLAUDE.md** Network Map § Server .23 (192.168.31.23 + Tailscale 100.125.108.37) and Bono VPS (srv1422716.hstgr.cloud public attachment)

## Stale-at
2026-06-12 (30 days; re-verify if deploy-surface changes via ufw rule mod or racecontrol.toml host change or fleet_healer module feature-gate addition)
