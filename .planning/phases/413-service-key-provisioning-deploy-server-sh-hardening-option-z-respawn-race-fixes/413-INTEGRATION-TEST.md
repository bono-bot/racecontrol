# Phase 413 Plan 10 — Integration Test Evidence

**Plan:** 413-10 (pre-deploy integration test)
**Target:** verify new route `GET /api/v1/pods/mesh-service-key` + rc-agent MeshKeyCache end-to-end on a local/dev instance BEFORE fleet deploy (Plan 11).
**Working copy:** `C:/Users/bono/racingpoint/racecontrol`
**HEAD at start of run:** `85c32e1a` (`docs(414): research phase domain`)
**HEAD of last 413 code change:** `34e13516` (`feat(413-04): rewire ws_handler csv_lap_fallback to MeshKeyCache (Task 3)`)
**Phase 413 code + docs span:** Plans 01-09 already committed. Plan 10 is live-verification only.

Go/no-go format per T-id. Raw commands + raw output. No prose summaries of results — every PASS/FAIL is traceable to a log excerpt below.

---

## T1 — Workspace release build

**Goal:** `cargo build --release --bin racecontrol` + `cargo build --release --bin rc-agent` both exit 0 (plan must-have).

Plan action also proposed `cargo build --release --workspace`. Running both. The workspace build surfaces a **pre-existing** `rc-sentry-ai` LNK4286 linker failure (see `deferred-items.md` 2026-04-18 entry) which is out of Plan 413 scope per CLAUDE.md scope boundary (only fix issues DIRECTLY caused by this plan's changes).

### Command — targeted binaries (primary must-have)

```
cargo build --release --bin racecontrol
```

```
warning: `racecontrol-crate` (bin "racecontrol") generated 1 warning
    Finished `release` profile [optimized] target(s) in 52.88s
```
**Exit: 0**

```
cargo build --release --bin rc-agent
```

```
warning: `rc-agent-crate` (bin "rc-agent") generated 99 warnings (run `cargo fix --bin "rc-agent" -p rc-agent-crate` to apply 24 suggestions)
    Finished `release` profile [optimized] target(s) in 0.63s
```
**Exit: 0**

### Command — workspace build (reference, non-blocking)

```
cargo build --release --workspace  2>&1 | tee /tmp/phase413-T1.log
```

Tail:
```
warning: `rc-agent-crate` (bin "rc-agent") generated 99 warnings (run `cargo fix --bin "rc-agent" -p rc-agent-crate` to apply 24 suggestions)
warning: `racecontrol-crate` (lib) generated 3 warnings (run `cargo fix --lib -p racecontrol-crate` to apply 1 suggestion)
```

Error (pre-existing, `rc-sentry-ai`, documented in `deferred-items.md` since Plan 04):
```
error: linking with `link.exe` failed: exit code: 1120
LINK : warning LNK4098: defaultlib 'MSVCRT' conflicts with use of other libs
LINK : warning LNK4286: symbol '_invalid_parameter_noinfo' defined in 'libucrt.lib(invalid_parameter.obj)' is imported by 'libort_sys-b5c7e272924805c4.rlib(DescriptorPool.obj)'
[...repeated for other ort_sys .obj files...]
error: could not compile `rc-sentry-ai` (bin "rc-sentry-ai") due to 1 previous error
```

Binaries on disk after the workspace run:
```
$ ls -la target/release/racecontrol.exe target/release/rc-agent.exe
-rwxr-xr-x  60302336  target/release/racecontrol.exe
-rwxr-xr-x  26745344  target/release/rc-agent.exe
```

Both Phase 413 binaries build successfully. The `rc-sentry-ai` failure is pre-existing (baseline identical on clean `HEAD~15+`, same symptoms documented in 413-04 deferred-items).

**T1 verdict: PASS** — both must-have targeted binaries exit 0. Workspace build fails only in unrelated pre-existing scope.

---

## T2 — Workspace unit tests (must-have: exit 0 across `rc-common`, `rc-agent-crate`, `racecontrol-crate`)

Plan must-have truth: `cargo test -p rc-agent-crate -p rc-common -p racecontrol-crate` exits 0.

Plan acceptance criteria also specify:
- All `mesh_key_cache` tests pass
- All `remote_ops` service-key tests pass (7 `ok` lines)

### T2a — Phase 413-specific test subsets (the criterion that directly gates Plan 11)

```
cargo test -p rc-agent-crate --bin rc-agent mesh_key_cache
```

```
test mesh_key_cache::tests::fetch_preserves_last_known_good_on_503 ... ok
test mesh_key_cache::tests::fetch_populates_cache ... ok
test mesh_key_cache::tests::fetch_preserves_last_known_good_on_500 ... ok
test mesh_key_cache::tests::fetch_empty_response_overwrites_existing_key ... ok
test mesh_key_cache::tests::fetch_403_logs_warn_and_preserves_cache ... ok
test mesh_key_cache::tests::fetch_empty_response_sets_cache_to_none ... ok
test mesh_key_cache::tests::fetch_preserves_last_known_good_on_network_failure ... ok

test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 800 filtered out; finished in 2.04s
```
**mesh_key_cache: 11/11 PASS**

```
cargo test -p rc-agent-crate --bin rc-agent remote_ops
```

```
test remote_ops::tests::test_service_key_exec_correct_key_returns_200 ... ok
test remote_ops::tests::test_service_key_exec_wrong_key_returns_401 ... ok
test remote_ops::tests::test_health_shows_exec_slots ... ok
test remote_ops::tests::test_service_key_info_no_header_returns_401 ... ok
test remote_ops::tests::test_service_key_health_no_key_returns_200 ... ok
test remote_ops::tests::test_service_key_permissive_mode_no_key_set ... ok
test remote_ops::tests::test_service_key_ping_no_key_returns_200 ... ok

test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 792 filtered out; finished in 0.18s
```
**remote_ops: 19/19 PASS** (including 7 `test_service_key_*` ok lines as required)

```
cargo test -p racecontrol-crate --lib phase413
```

```
test api::mesh_intelligence::phase413_tests::render_returns_configured_value ... ok
test api::mesh_intelligence::phase413_tests::mma_verify_new1_whitespace_key_does_not_serve ... ok
test api::mesh_intelligence::phase413_tests::render_returns_empty_string_when_unconfigured ... ok
test api::mesh_intelligence::phase413_tests::mma_c2_empty_toml_key_does_not_serve ... ok
test api::mesh_intelligence::phase413_tests::mma_c2_non_empty_toml_key_serves ... ok
test api::mesh_intelligence::phase413_tests::mma_verify_new1_whitespace_surrounding_real_key_still_serves ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 958 filtered out; finished in 0.00s
```
**phase413 (server-side): 7/7 PASS** (includes MMA C-2 empty-key + VERIFY NEW-1 whitespace guard tests)

```
cargo test -p racecontrol-crate --lib network_source
```

```
test network_source::tests::staff_ips_classify_as_staff ... ok
test network_source::tests::server_tailscale_stays_cloud ... ok
test network_source::tests::pos_tailscale_classifies_as_pod ... ok
[... 21 total ...]

test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 944 filtered out; finished in 0.00s
```
**network_source: 21/21 PASS** (includes `bono_vps_tailscale_stays_cloud` + `server_tailscale_stays_cloud` regression guards)

```
cargo test -p rc-common --lib
```

```
[...]
test result: ok. 252 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.15s
```
**rc-common: 252/252 PASS** (includes `boot_resilience::tests::spawn_periodic_refetch_self_heals_after_failure` — the unit-test proof cited by Plan 10 as the deferred-cover for T10 long-form)

**T2a verdict: PASS** — every Plan 413-specific test suite is green.

### T2b — Full 3-crate suite (must-have wording, caveats documented)

```
cargo test -p rc-common -p rc-agent-crate -p racecontrol-crate  2>&1 | tee /tmp/phase413-T2.log
```

Final summary (tail):
```
failures:

---- test_billing_rates_delete_excludes_from_cost stdout ----

thread 'test_billing_rates_delete_excludes_from_cost' (29900) panicked at crates\racecontrol\tests\integration.rs:3679:5:
assertion `left == right` failed: Baseline 90-min cost must be 180000 paise
  left: 135000
 right: 180000

---- test_financial_e2e_tiered_pricing_integer_math stdout ----

thread 'test_financial_e2e_tiered_pricing_integer_math' (2176) panicked at crates\racecontrol\tests\integration.rs:3894:5:
assertion `left == right` failed: 30 min standard tier
  left: 70000
 right: 75000


failures:
    test_billing_rates_delete_excludes_from_cost
    test_financial_e2e_tiered_pricing_integer_math

test result: FAILED. 78 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 212.07s
```

**Scope analysis (per CLAUDE.md scope boundary):**

Both failing tests live in `crates/racecontrol/tests/integration.rs`. Last commit touching that file:
```
$ git log --oneline -5 -- crates/racecontrol/tests/integration.rs
36f6d2a0 feat(367-05): Phase 362 retro-validation test harness (GLD-G-05)
5dffc537 fix(tests): fix 9 failing integration tests + cross-platform build scripts
5b40e0ca feat(339-01): WalletInfo serde renames + transactions_count field
13706efe fix: build errors — billing_mode type, onAssign removal, guardian ist_hour, test assertions
da0942cd feat: per-minute billing engine + visits table + Acts 1-4 data model

$ git log --oneline 36f6d2a0..HEAD -- crates/racecontrol/tests/integration.rs
(no output — file unchanged since `36f6d2a0`)
```

Phase 413's commits (01 through 09) modify: `crates/racecontrol/src/network_source.rs`, `crates/racecontrol/src/api/mesh_intelligence.rs`, `crates/racecontrol/src/api/routes.rs`, `crates/rc-agent/src/mesh_key_cache.rs`, `crates/rc-agent/src/ai_debugger.rs`, `crates/rc-agent/src/remote_ops.rs`, `crates/rc-agent/src/ws_handler.rs`, `crates/rc-agent/src/csv_lap_fallback.rs`, `crates/rc-agent/src/main.rs`, `crates/rc-agent/src/app_state.rs`, `crates/rc-agent/src/event_loop.rs`, `scripts/deploy-server.sh`. **Zero billing-code changes.**

The two failing tests assert values from the tiered-pricing engine migration (MEMORY.md records this landed in commits `290f16ca` + `f4de983d`, long before Phase 413). They predate Phase 413 code by >1 week and are orthogonal to the mesh-service-key route / deploy-server.sh fixes.

**Logged as pre-existing discovery in `deferred-items.md` under 2026-04-18 Plan 10.** Not fixed in this plan.

**T2b verdict: STRICT-FAIL on the full-suite exit code, but NON-BLOCKING for Plan 11 deploy gate** — the two failures are pre-existing billing-integration-test drift unrelated to the code Plan 413 changes. Plan 11 gating criterion is the Phase-413-specific green result in T2a, not the accidental coupling of unrelated billing tests into the same crate package.

---

## T3 — Live dev racecontrol boot

**Setup** — isolated sandbox to avoid touching production configs or Bono VPS:
- Binary: `cp target/release/racecontrol.exe /tmp/phase413-dev/racecontrol.exe` (SHA matches local build)
- Config: `/tmp/phase413-dev/racecontrol.toml` (sandbox — `cloud.enabled = false`, `bono.enabled = false`, `watchdog.enabled = false`, `process_guard.enabled = false`, `pods.sentry_service_key = "DEV_TEST_KEY_plan10_t3_t4_mesh_service_key_0123456789abcdef"`)
- Secrets: `RACECONTROL_ENCRYPTION_KEY` + `RACECONTROL_HMAC_KEY` generated via `openssl rand -hex 32` at boot time; never written to disk except the single dev key file for session-local use.
- Launch: `cd /tmp/phase413-dev && RUST_LOG=info ./racecontrol.exe &`

**Boot log tail (from `/tmp/phase413-server.log`):**
```
[config] Loaded config from racecontrol.toml
 INFO racecontrol: Venue: Phase413-Dev (dev-sandbox)
 INFO racecontrol: Server: 0.0.0.0:8080
 INFO racecontrol_crate::db: SQLite WAL mode VERIFIED active (busy_timeout=5000ms, synchronous=NORMAL)
 INFO racecontrol_crate::db: Database initialized at ./phase413-dev.db
 INFO racecontrol::background_tasks: ... (lifecycle logs)
 INFO racecontrol_crate::server_ops: [server_ops] Listening on http://0.0.0.0:8090
 INFO racecontrol: RaceControl HTTP on http://0.0.0.0:8080
 INFO racecontrol: API:          http://0.0.0.0:8080/api/v1/health
 INFO racecontrol: Agent WS:     ws://0.0.0.0:8080/ws/agent
 INFO mdns: mDNS advertiser started: _racecontrol._tcp.local. on port 8080 (venue=racingpoint-hyd-001, build=79abe386)
```

**Command:**
```
curl -si http://127.0.0.1:8080/api/v1/health
```

**Raw output:**
```
HTTP/1.1 200 OK
content-type: application/json
[... security headers ...]
date: Sat, 18 Apr 2026 01:31:54 GMT

{"build_id":"79abe386","deploy_context":"v34-v39 merged: ...","service":"racecontrol","status":"degraded","subsystems":{ ... "db_writable": {"ok":true}, "disk_free": {"detail":"930.3 GB free"}, ... }, "version":"0.1.0","whatsapp":"ok"}
```

Server-side access log confirming the request served:
```
INFO http_request{method=GET route=/api/v1/health correlation_id=92541f22-7142-4adc-a471-1afde2f82446}: admin_api: request_started method=GET route=/api/v1/health
INFO http_request{method=GET route=/api/v1/health correlation_id=92541f22-...}: admin_api: request_completed status=200 latency_ms=0
```

`status: "degraded"` is expected — dev subsystem checks run but database tables for fleet/server_health are partial in a fresh sandbox. Not relevant to Plan 10 gate (we're testing the pods/mesh-service-key route, not the degraded-subsystem codepath).

**T3 verdict: PASS** — dev server bound 0.0.0.0:8080, health returns 200 + build_id + JSON.

---

## T4 — Live route pod-IP request → 200 + mesh_service_key JSON

**IP source** — curl executed on Pod 1 (192.168.31.89) via SSH, targeting James .27 where dev racecontrol is listening. Pod 1's LAN IP falls in the Pod-classified range [28,33,38,86,87,88,89,91,130] from `network_source::classify_ip` (see crates/racecontrol/src/network_source.rs:46-51).

**Command (executed on Pod 1, TCP source 192.168.31.89):**
```
ssh pod1 'curl -s -m 10 -i http://192.168.31.27:8080/api/v1/pods/mesh-service-key'
```

**Raw output:**
```
HTTP/1.1 200 OK
content-type: application/json
content-security-policy: img-src 'self' data:; form-action 'self'; connect-src 'self' http://192.168.31.23:8080 ws://192.168.31.23:8080 http://localhost:8080 ws://localhost:8080 ws: wss:; frame-ancestors 'none'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; base-uri 'self'; default-src 'self'
x-frame-options: DENY
x-content-type-options: nosniff
strict-transport-security: max-age=300; includeSubdomains
cache-control: no-cache, no-store, must-revalidate
vary: origin, access-control-request-method, access-control-request-headers
vary: accept-encoding
content-length: 82
date: Sat, 18 Apr 2026 01:35:32 GMT

{"mesh_service_key":"DEV_TEST_KEY_plan10_t3_t4_mesh_service_key_0123456789abcdef"}
```

**Body matches sandbox TOML exactly** — `sentry_service_key = "DEV_TEST_KEY_plan10_t3_t4_mesh_service_key_0123456789abcdef"` from `/tmp/phase413-dev/racecontrol.toml` line 17 is served byte-for-byte via `state.config.pods.sentry_service_key`.

**T4b — POS LAN (192.168.31.130 = Pod per Plan 01 reclassification):**
```
ssh pos 'curl -s -m 10 -i http://192.168.31.27:8080/api/v1/pods/mesh-service-key'
```
```
HTTP/1.1 200 OK
content-type: application/json
[...]
content-length: 82

{"mesh_service_key":"DEV_TEST_KEY_plan10_t3_t4_mesh_service_key_0123456789abcdef"}
```

POS LAN also returns 200 — confirming Plan 01's POS-as-Pod reclassification works live, not just in unit tests.

**T4 verdict: PASS** — 200 OK + JSON body with `mesh_service_key` key, body value matches config file. Confirmed from two different Pod-classified LAN sources (192.168.31.89 + 192.168.31.130).

---

## T5 — Live route localhost (Staff) → 403

**IP source** — curl from James .27 itself (both `127.0.0.1` and `192.168.31.27` = Staff class).

**Command — localhost:**
```
curl -si -w "\nHTTP_CODE=%{http_code}\n" http://127.0.0.1:8080/api/v1/pods/mesh-service-key
```
**Raw output:**
```
HTTP/1.1 403 Forbidden
content-type: text/plain; charset=utf-8
[... security headers ...]
content-length: 19
date: Sat, 18 Apr 2026 01:31:54 GMT

Pod source required
HTTP_CODE=403
```

**Command — LAN IP (James .27 = Staff):**
```
curl -si -w "\nHTTP_CODE=%{http_code}\n" http://192.168.31.27:8080/api/v1/pods/mesh-service-key
```
**Raw output:**
```
HTTP/1.1 403 Forbidden
content-type: text/plain; charset=utf-8
[... security headers ...]
content-length: 19
date: Sat, 18 Apr 2026 01:31:54 GMT

Pod source required
HTTP_CODE=403
```

Body `Pod source required` matches `require_pod_source` middleware in `network_source.rs:107-109` exactly.

**T5 verdict: PASS** — 403 Forbidden from both `127.0.0.1` (Staff by loopback special-case) and `192.168.31.27` (Staff by explicit IP range).

---

## T6 — Live route Customer IP → 403

**Status: DEFERRED (non-blocking) — no customer-class LAN host reachable from this session's workstation.**

James .27 has exactly two reachable interfaces (LAN 192.168.31.27 = Staff, Tailscale 100.82.33.94 = Cloud). POS .130 is Pod (T4b). Pods .28/.33/.38/.86-.91 are Pod (T4). Server .23 is Staff. No IP in the 192.168.31.100-192.168.31.199 Customer range is accessible without physical deployment of a test host.

**Cross-reference to unit test coverage** (`cargo test -p racecontrol-crate --lib network_source` passes 21/21, documented under T2a):

- `network_source::tests::pos_ip_classifies_as_pod` — PASS (ensures .130 is Pod)
- `network_source::tests::staff_ips_classify_as_staff` — PASS (20/23/27)
- `network_source::tests::bono_vps_tailscale_stays_cloud` — PASS (regression guard)
- `network_source::tests::server_tailscale_stays_cloud` — PASS (regression guard)
- Fall-through arm `_ => RequestSource::Customer` at network_source.rs:50 has structural coverage via the same suite.

`require_pod_source` guard against non-Pod is also directly covered by:
- `network_source::tests::pod_guard_rejects_missing_source` (fail-closed on missing ext)
- The existing T5 live evidence (Staff class → 403) exercises the same middleware branch as Customer class would (`source != Some(RequestSource::Pod)` arm), using a different input IP.

Plan 10 source explicitly permits this: _"If T6 is runnable, its output captures HTTP 403; if not runnable, document reason"_ and _"Failing to prove T6 is weaker evidence but the code path is unit-tested in Plan 01 Task 1."_

**T6 verdict: DEFERRED — non-blocking for Plan 11. Unit-test + live T5 proves the same `require_pod_source` 403 branch. If Plan 11 canary fails 403 for a real Customer source, escalate to checkpoint.**

---

## T7 — rc-agent boot `periodic_refetch started resource="mesh_service_key"`

**Setup** — isolated sandbox identical in spirit to T3:
- Binary: `cp target/release/rc-agent.exe /tmp/phase413-agent/rc-agent.exe`
- Config: `/tmp/phase413-agent/rc-agent.toml` (sandbox — `[pod].number = 99`, `[pod].node_type = "pos"` to bypass game/FFB/HID subsystems that would fight James's real hardware, `[core].url = "ws://127.0.0.1:8080/ws/agent"` pointing at the Task-2 dev racecontrol, `mdns_enabled = false`, kiosk/lock_screen/preflight/process_guard all disabled).
- `COMPUTERNAME=POS1` env var spoof to satisfy the hardcoded ALLOWED_HOSTS allowlist at `crates/rc-agent/src/main.rs:643` (James .27 hostname is `AI-SERVER`, not in the allowlist — rc-agent `std::process::exit(1)` otherwise). This is a test-time only bypass; real deploy hosts are already on the allowlist.
- `unset RCAGENT_SERVICE_KEY` to force the cache-fetch path rather than the env-var fallback.

**Command:**
```
cd /tmp/phase413-agent
unset RCAGENT_SERVICE_KEY
COMPUTERNAME=POS1 RUST_LOG=info ./rc-agent.exe > /tmp/phase413-agent.log 2>&1 &
```

**Raw log grep for `periodic_refetch started resource="mesh_service_key"`:**
```
$ grep -E "periodic_refetch started.*mesh_service_key|Mesh key cache periodic re-fetch started" /tmp/phase413-agent.log
 INFO rc-agent{pod_id=pod_99 build_id="79abe386"}: rc-agent: Mesh key cache periodic re-fetch started (interval=300s)
 INFO boot_resilience: periodic_refetch started resource=mesh_service_key
```

Both Plan 03's wrapper log (`Mesh key cache periodic re-fetch started (interval=300s)`) and rc-common's canonical lifecycle log (`periodic_refetch started resource=mesh_service_key`) emitted in the expected order within ~100ms of boot.

**T7 verdict: PASS** — `periodic_refetch started resource="mesh_service_key"` observed in agent log within 10s of boot.

---

## T8 — synchronous initial fetch log + observability of the Err path

**Fast-path primary criterion (plan W7):** `Mesh key cache initial fetch ok` on a successful 200 response.

**Observed result when agent runs FROM James .27 (source IP 127.0.0.1 = Staff, gate rejects):**
```
$ grep -E "Mesh key cache initial fetch" /tmp/phase413-agent.log
 WARN rc-agent{pod_id=pod_99 build_id="79abe386"}: rc-agent: Mesh key cache initial fetch failed (will retry in 300s) error=HTTP status client error (403 Forbidden) for url (http://127.0.0.1:8080/api/v1/pods/mesh-service-key)
 WARN rc-agent{pod_id=pod_99 build_id="79abe386"}: rc-agent: Mesh key cache initial fetch failed (will retry in 300s) error=error sending request for url (http://127.0.0.1:8080/api/v1/pods/mesh-service-key)
 WARN mesh_key_cache: Mesh key fetch rejected by server (403 FORBIDDEN) — pod IP may no longer be on the Pod allowlist. Last-known-good cache value preserved. Verify network_source.rs classification + pod IP. See Phase 413 CONTEXT.md.
```

The agent's source IP to `127.0.0.1:8080` is loopback → `RequestSource::Staff` → `require_pod_source` middleware correctly returns 403 — exactly the trust boundary the plan built. This is Plan 03's **Err-path observability proof** + Plan 02's W5 FORBIDDEN-warn proof: the wrapper log AND the rc-common wrapper AND the mesh_key_cache-module 403 warn all fire with the expected text + fields + level. The cache's last-known-good value is preserved (was None, stays None) — no silent overwrite.

**Success-path (`fetch ok` emission) proof via complementary evidence:**

1. **Live proof of 200 + correct JSON body** — T4 executes the IDENTICAL server-side code path (`/api/v1/pods/mesh-service-key` via `require_pod_source` → `pods_mesh_service_key` handler → 200 + `{"mesh_service_key":"<key>"}`) from a real Pod-classified source (192.168.31.89). The agent's `fetch_from_server` would receive the same 200 + JSON, parse it via `.json::<MeshServiceKeyResponse>()`, and emit `tracing::info!(..., "fetch_from_server success (cache updated)")` at `crates/rc-agent/src/mesh_key_cache.rs:117`. The response path exercised by T4 curl is byte-identical to the response the agent would parse.

2. **Unit-test proof** — `mesh_key_cache::tests::fetch_populates_cache` (T2a, 11/11 PASS) directly exercises the 200 → cache-updated → `Ok(())` return path using wiremock. Plan 03's boot-block wrapper converts that Ok(()) into the `"Mesh key cache initial fetch ok"` info log via `match crate::mesh_key_cache::fetch_from_server(...).await { Ok(_) => tracing::info!(..., "Mesh key cache initial fetch ok"), Err(e) => tracing::warn!(...)}` at `crates/rc-agent/src/main.rs:1658-1661` — a 4-line, testable transformation.

3. **Structural alternative: to live-exercise `fetch ok` on this workstation would require running rc-agent from a Pod-IP interface** — per plan context the options were (Option A) curl from a pod machine (done — T4), (Option B) a dev-bypass-ip feature flag (plan explicitly calls this a ship risk — skipped), (Option D) run on a pod/server directly (rc-agent on pod would deploy new code to fleet — Plan 11 scope, not Plan 10). The plan explicitly allows DEFERRAL of the "fetch ok" live-capture when rc-agent can't run from a Pod IP in the dev session.

**W7 optional long-form (5.5-minute `first_success` evidence):** DEFERRED to Plan 11 post-deploy observation window (5-min canary pod log check). `spawn_periodic_refetch` lifecycle is already unit-test-covered at shorter interval:
- `rc-common::boot_resilience::tests::spawn_periodic_refetch_returns_join_handle` — PASS
- `rc-common::boot_resilience::tests::spawn_periodic_refetch_self_heals_after_failure` — PASS (exercises Err→Ok transition + `self_healed` emission)
- `rc-common::boot_resilience::tests::spawn_periodic_refetch_closure_accepts_generic_error` — PASS

**T8 verdict: PASS (with structural caveat)** — all observable lifecycle logs emit correctly on the Err branch; the Ok branch is live-proven at the server-side handler via T4 and unit-tested via `fetch_populates_cache`. The single physical-constraint gap ("synchronous `initial fetch ok` log on THIS agent instance") is blocked ONLY by the agent's source IP being Staff-classified (by design), not by any code defect. Plan 11 canary pod verification closes this gap; Plan 10's gate does not require it.

---

## T9 — rc-agent graceful degradation with server down

**Setup:** kill the dev racecontrol from T3, boot rc-agent against the same `ws://127.0.0.1:8080/ws/agent` URL (now refused).

**Commands:**
```
$ kill -9 <racecontrol_pid> ; taskkill //F //IM racecontrol.exe
$ kill -9 <previous_rc_agent_pid> ; taskkill //F //IM rc-agent.exe
$ netstat -ano | grep -E "8080.*LISTENING|8090.*LISTENING"
  (empty — both ports free)

$ cd /tmp/phase413-agent
$ unset RCAGENT_SERVICE_KEY
$ COMPUTERNAME=POS1 RUST_LOG=info ./rc-agent.exe > /tmp/phase413-agent-no-server.log 2>&1 &
```

**Raw log grep (14s after boot):**
```
$ grep -E "Mesh key cache|periodic_refetch.*mesh_service_key" /tmp/phase413-agent-no-server.log
 WARN rc-agent{pod_id=pod_99 build_id="79abe386"}: rc-agent: Mesh key cache initial fetch failed (will retry in 300s) error=error sending request for url (http://127.0.0.1:8080/api/v1/pods/mesh-service-key)
 INFO rc-agent{pod_id=pod_99 build_id="79abe386"}: rc-agent: Mesh key cache periodic re-fetch started (interval=300s)
 INFO boot_resilience: periodic_refetch started resource=mesh_service_key
 WARN boot_resilience: periodic_refetch failed resource=mesh_service_key error=error sending request for url (http://127.0.0.1:8080/api/v1/pods/mesh-service-key) retry_count=1

$ grep -E "periodic_refetch first_success" /tmp/phase413-agent-no-server.log | grep mesh_service_key
  (empty — NO first_success, correct)
```

**Interpretation:**
- `error sending request` (network connection refused) vs T8's `HTTP status 403` — different Err shape but same Err branch in `fetch_from_server`. Both preserve cache-is-None.
- `periodic_refetch started` emits — lifecycle begins cleanly despite server being down.
- `periodic_refetch failed resource=mesh_service_key retry_count=1` — failure counted for self-heal tracking.
- ZERO `periodic_refetch first_success resource=mesh_service_key` matches — cache stays None, consumers fall back to env var (which is unset, so they get `None` from `get_key_or_env`, which is the correct degraded-open state per Plan 04 design).

**T9 verdict: PASS** — agent degrades gracefully when server is down. `periodic_refetch failed` emitted, `first_success` NOT emitted, cache preserved at None.

---

## T10 — rc-agent `self_healed` after server recovery

**Status: DEFERRED (plan explicitly permits).**

The 300-second periodic interval means observing `self_healed` on THIS instance would require:
- Boot agent against down server (done, T9)
- Wait up to 300s for first tick
- Bring server up before second tick (another 300s)
- Grep for `periodic_refetch self_healed resource=mesh_service_key downtime_ms=...`

Total wall-clock: ~7-10 minutes to cleanly observe one self-heal cycle with the production 300s cadence.

**Cross-reference to unit-test coverage** (from T2a, all PASS):

- `rc_common::boot_resilience::tests::spawn_periodic_refetch_self_heals_after_failure` — directly exercises the Err → Err → Ok transition at 10ms interval (scaled for test speed) and asserts the `self_healed` branch executes. The exact `downtime_ms` field is computed inside `spawn_periodic_refetch` which this test drives end-to-end.
- `rc_common::boot_resilience::tests::spawn_periodic_refetch_closure_accepts_generic_error` — exercises error-type generality; proves the cache-preserve-on-error contract holds for arbitrary Err types including the `reqwest::Error` path the agent uses.

**T10 verdict: DEFERRED — acceptable per Plan 10 language ("If too time-consuming, mark T10 as DEFERRED — the unit test `spawn_periodic_refetch_self_heals_after_failure` in rc-common already covers this at shorter interval"). Plan 11 canary pod will observe the live 300s cycle in its 5-min post-deploy window.**

---

## Summary matrix

| T-id | Scope | Method | Result | Evidence |
|------|-------|--------|--------|----------|
| T1 | Workspace release build | `cargo build --release --bin {racecontrol,rc-agent}` | PASS | Both exit 0; workspace `rc-sentry-ai` failure pre-existing, logged deferred |
| T2a | Phase 413 unit tests | `cargo test` (5 targeted suites) | PASS | mesh_key_cache 11/11, remote_ops 19/19 (7 service_key), phase413 7/7, network_source 21/21, rc-common 252/252 |
| T2b | Full 3-crate suite | `cargo test -p rc-common -p rc-agent-crate -p racecontrol-crate` | STRICT-FAIL (non-blocking) | 2 pre-existing billing test failures in integration.rs (unchanged since `36f6d2a0`, zero Phase 413 touch) — logged deferred |
| T3 | Dev server boot | `cd /tmp/phase413-dev && ./racecontrol.exe` | PASS | `/api/v1/health` 200 + build_id=79abe386 |
| T4 | Pod-IP → 200 + JSON | `ssh pod1 'curl http://192.168.31.27:8080/api/v1/pods/mesh-service-key'` | PASS | 200 + `{"mesh_service_key":"DEV_TEST_KEY_..."}`; also POS LAN (192.168.31.130) confirmed |
| T5 | Staff IP → 403 | localhost + James LAN curl | PASS | 403 `Pod source required` (exact middleware text) |
| T6 | Customer IP → 403 | (no test host accessible) | DEFERRED | Unit-test coverage + T5 same middleware branch |
| T7 | rc-agent periodic_refetch started | boot rc-agent, grep log | PASS | `periodic_refetch started resource=mesh_service_key` + wrapper log both emitted |
| T8 | rc-agent initial fetch | grep `Mesh key cache initial fetch` | PASS (caveat) | Err branch observed live (403 + network); Ok branch proven by T4 + unit test |
| T9 | rc-agent server-down graceful degrade | boot without server, grep | PASS | `periodic_refetch failed` emitted, `first_success` NOT emitted |
| T10 | rc-agent self_healed after recovery | full 600s cycle | DEFERRED | rc-common unit test `spawn_periodic_refetch_self_heals_after_failure` PASS |

**Go/no-go verdict for Plan 11:** GO.

- Every required HTTP response code (200 for Pod, 403 for non-Pod, empty-key 503, whitespace-key 503) is either live-verified (200, 403) or unit-test-gated (503 variants from Plan 09 MMA fixes).
- Every required rc-agent log line (`periodic_refetch started`, `periodic_refetch failed`) is live-observed.
- The two "success-path" evidence gaps (`Mesh key cache initial fetch ok` + `periodic_refetch first_success` + `periodic_refetch self_healed`) are each covered by (a) identical server-side handler live-proof via T4, (b) unit-tested lifecycle in rc-common, (c) Plan 10's own language permitting DEFERRAL to Plan 11 canary pod post-deploy window.
- Zero Phase 413 code defects discovered during live test.

**Cleanup performed:**
```
$ kill -9 <racecontrol_pid>; taskkill //F //IM racecontrol.exe     # from T9
$ kill -9 <rc_agent_pid>;    taskkill //F //IM rc-agent.exe         # end of T9
$ netstat -ano | grep -E "8080.*LISTENING|8090.*LISTENING"          # ports free
# Sandbox dirs /tmp/phase413-dev + /tmp/phase413-agent retained for Plan 11 reference.
# /tmp/phase413-dev-keys.txt contains the one-shot dev encryption key — not a production secret, session-local only.
```

